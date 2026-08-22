//! Hand-off of finished frames from the engine to the output drivers.
//!
//! One writer (the tick), many readers (one per output driver). Both sides are
//! wait-free: a publish and a refresh are each a single atomic swap, so neither
//! can be delayed by the other and neither ever blocks. `ARCHITECTURE_SPEC.md`
//! §3.2 is what makes this shape the right one — the tick runs at a fixed 44 Hz
//! and every driver consumes the *most recent* frame at whatever rate its
//! hardware allows, so a slow driver must fall behind rather than hold the
//! engine up.
//!
//! # Why one buffer per subscriber
//!
//! The classic triple buffer transfers ownership of a slot through a single
//! control word, which makes it a strictly single-consumer structure: with two
//! readers swapping against the same word, one of them can be handed the slot
//! the other just gave back — an older frame than it has already shown. Rather
//! than paper over that with a retry loop (which would no longer be wait-free),
//! each subscriber gets its own three-slot buffer and the publisher fans the
//! frame out. The cost is one memory copy per subscriber per tick — 32 KB at 64
//! universes, a few megabytes a second — and the gain is that "single writer,
//! single reader" is true of every buffer in the system, which is a property
//! that can actually be model-checked.
//!
//! # Why the slots are made of atomics
//!
//! `unsafe_code` is denied workspace-wide, and this crate is the last place to
//! start making exceptions. Shared mutable slots therefore cannot be
//! `UnsafeCell`; they are arrays of [`AtomicU64`] instead. The ownership
//! discipline already guarantees that only one side touches a slot at a time,
//! so the atomic accesses can all be `Relaxed` — the control-word swap carries
//! the ordering. What the atomics buy is that the *worst* case of a bug in that
//! discipline is a stale or mixed frame, never undefined behaviour.
//!
//! # Why a subscriber may arrive and leave while the tick runs *(S33)*
//!
//! Until S33 the set of subscribers was fixed before the tick started, because
//! [`FramePublisher::subscribe`] allocates and `ARCHITECTURE_SPEC.md` §3.1
//! forbids the tick the allocator. S33 needs an output added, removed or
//! re-addressed **while the show runs**, and an output is a subscriber — so the
//! set has to change under a running tick without the tick allocating, locking
//! or waiting.
//!
//! [`FrameEnrolment`] is that channel, and it is deliberately the same shape as
//! `prismd`'s `BodySwap`: the joining subscriber's buffer is allocated by
//! whoever asked for it, on their own thread, and left in a slot behind one
//! atomic flag. The tick reads that flag once per publish — false almost
//! always, one acquire load — and only when it is set does it take the slot
//! with a [`std::sync::Mutex::try_lock`] that never blocks. A failed attempt
//! costs a compare-and-swap and the subscriber arrives one tick later instead.
//! The links it gives up go back the same way, so a departing output's buffer
//! is **freed on the caller's thread** and not on this one.
//!
//! The one thing the tick must not do is grow the vector it walks, so the
//! vector is built with room for [`MAX_SUBSCRIBERS`] and the enrolment refuses
//! the subscriber that would exceed it. That is the whole of the cost:
//! `crates/prism-engine/tests/tick_allocations.rs` asserts the tick that takes
//! one on and gives one up makes no allocator call at all.

use std::sync::Mutex;

use crate::frame::{DmxFrame, FrameLayout, UNIVERSE_CHANNELS};
use crate::sync::{Arc, AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// Slots per buffer: one the writer is filling, one published, one the reader
/// is holding. Three is the smallest number for which neither side ever waits.
const SLOTS: usize = 3;

/// Low bits of the control word: which slot is published.
const INDEX_MASK: usize = 0b11;

/// High bit of the control word: the published slot has not been read yet.
const FRESH: usize = 0b100;

/// Channel bytes packed per [`AtomicU64`].
const BYTES_PER_WORD: usize = 8;

/// Words a universe's channels occupy.
const WORDS_PER_UNIVERSE: usize = UNIVERSE_CHANNELS.div_ceil(BYTES_PER_WORD);

/// Three slots and the control word that says who owns which.
///
/// The control word holds the index of the published slot; the writer and the
/// reader each hold the index of the slot they own privately. Every transfer is
/// an atomic swap of "the slot I own" against "the slot that is published", so
/// a slot is only ever touched by one side, and the three indices are always a
/// permutation of `0..SLOTS`.
struct Slots {
    /// `SLOTS` consecutive runs of `words_per_slot` words. Word 0 of a run is
    /// the frame's sequence number; the rest are its channel bytes.
    words: Box<[AtomicU64]>,
    words_per_slot: usize,
    shared: AtomicUsize,
}

impl Slots {
    /// Words a buffer needs for a frame of `universes` universes: one for the
    /// sequence number, then the channel bytes packed eight to a word.
    const fn words_for(universes: usize) -> usize {
        1 + universes * WORDS_PER_UNIVERSE
    }

    fn new(words_per_slot: usize) -> Self {
        let words = (0..words_per_slot * SLOTS)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            words,
            words_per_slot,
            // Writer owns slot 0, slot 1 is published but stale, reader owns 2.
            shared: AtomicUsize::new(1),
        }
    }

    fn slot(&self, index: usize) -> Option<&[AtomicU64]> {
        self.words.chunks_exact(self.words_per_slot).nth(index)
    }

    /// Publishes the slot the writer owns and returns the slot it owns now.
    ///
    /// The whole write side of the protocol: one swap. `AcqRel` because it is
    /// both a release of everything just written into `owned` and an acquire of
    /// the slot coming back, which the reader was reading from until now.
    fn publish(&self, owned: usize) -> usize {
        self.shared.swap(owned | FRESH, Ordering::AcqRel) & INDEX_MASK
    }

    /// Exchanges the slot the reader owns for the published one.
    ///
    /// Returns the slot it owns now and whether that slot holds a frame it has
    /// not seen. The load first is what makes an idle refresh free: with nothing
    /// new published there is no swap and no cache line stolen from the writer.
    fn take(&self, owned: usize) -> (usize, bool) {
        if self.shared.load(Ordering::Acquire) & FRESH == 0 {
            return (owned, false);
        }
        let previous = self.shared.swap(owned, Ordering::AcqRel);
        (previous & INDEX_MASK, previous & FRESH != 0)
    }

    /// Copies a frame into a slot the caller owns.
    ///
    /// `Relaxed` throughout: the swap of the control word is what publishes
    /// these writes, and until that swap happens nobody else may look at this
    /// slot.
    fn store(&self, index: usize, frame: &DmxFrame) {
        let Some(slot) = self.slot(index) else { return };
        let Some((sequence, data)) = slot.split_first() else {
            return;
        };
        sequence.store(frame.sequence(), Ordering::Relaxed);
        // `as_chunks` gives `&[u8; 8]` rather than `&[u8]`, so the copy into a
        // fixed array that `from_le_bytes` needs is the chunk itself — one
        // fewer place where a length could be wrong.
        for (word, chunk) in data
            .iter()
            .zip(frame.channels().as_chunks::<BYTES_PER_WORD>().0)
        {
            word.store(u64::from_le_bytes(*chunk), Ordering::Relaxed);
        }
    }

    /// Copies a slot the caller owns back out into a plain frame.
    fn load(&self, index: usize, frame: &mut DmxFrame) {
        let Some(slot) = self.slot(index) else { return };
        let Some((sequence, data)) = slot.split_first() else {
            return;
        };
        frame.set_sequence(sequence.load(Ordering::Relaxed));
        for (word, chunk) in data
            .iter()
            .zip(frame.channels_mut().as_chunks_mut::<BYTES_PER_WORD>().0)
        {
            *chunk = word.load(Ordering::Relaxed).to_le_bytes();
        }
    }
}

/// Subscribers one publisher may fan a frame out to at once.
///
/// A bound rather than a product limit: the vector the tick walks is built with
/// room for this many and never grown, because growing it would allocate on the
/// tick thread (`ARCHITECTURE_SPEC.md` §3.1). Sixty-four outputs is four times
/// what the largest rig this desk is designed for needs, plus the telemetry
/// channel — and an installation that wanted more would raise the number here
/// and pay for it in start-up memory, not in jitter.
pub const MAX_SUBSCRIBERS: usize = 64;

/// Identifies one subscriber, so the one that is leaving can be named.
///
/// Handed out by the publisher and never reused: a number that came round again
/// would let a stale removal take a live output's frames away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubscriberId(u64);

impl SubscriberId {
    /// The raw number, for a caller that wants to log or index by it.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Why a subscriber could not be enrolled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnrolmentFull {
    /// The bound that was reached.
    pub limit: usize,
}

impl core::fmt::Display for EnrolmentFull {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "this engine already carries {} outputs, which is all it has room for",
            self.limit
        )
    }
}

impl std::error::Error for EnrolmentFull {}

/// The writer's end of one subscriber's buffer.
struct Link {
    id: SubscriberId,
    slots: Arc<Slots>,
    owned: usize,
}

/// What has been asked for since the tick last looked.
///
/// One vector each way rather than a queue of operations: the order within a
/// tick does not matter, because a subscriber that joined and left before the
/// tick saw either never received a frame.
#[derive(Default)]
struct Pending {
    joining: Vec<Link>,
    leaving: Vec<SubscriberId>,
}

/// The channel by which a subscriber joins or leaves a **running** publisher.
///
/// See this module's documentation for the shape and why it is that shape. A
/// handle is cheap to clone and every clone reaches the same publisher, which is
/// what lets the daemon hold one while the publisher itself has been moved onto
/// the tick thread.
#[derive(Debug, Clone)]
pub struct FrameEnrolment {
    inner: Arc<Enrolment>,
}

struct Enrolment {
    layout: Arc<FrameLayout>,
    /// Read once per publish, and false almost always.
    pending: AtomicBool,
    changes: Mutex<Pending>,
    /// Links the tick has finished with, waiting to be dropped anywhere but
    /// there: freeing a subscriber's buffer is freeing a boxed slice, and §3.1
    /// does not distinguish an allocation from a deallocation.
    retired: Mutex<Vec<Link>>,
    /// Subscribers asked for and not yet given up, which is what the bound is
    /// against. Counted here rather than on the publisher because a caller has
    /// to be refused *before* it allocates a buffer nothing would take.
    live: AtomicUsize,
    next_id: AtomicU64,
}

impl core::fmt::Debug for Enrolment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Enrolment")
            .field("live", &self.live.load(Ordering::Relaxed))
            .field("pending", &self.pending.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl Enrolment {
    fn new(layout: Arc<FrameLayout>) -> Self {
        Self {
            layout,
            pending: AtomicBool::new(false),
            changes: Mutex::new(Pending::default()),
            // Room for every place there is, reserved here so that
            // `retire` — which runs on the tick thread — pushes into capacity
            // that already exists. `collect` clears without shrinking, so the
            // room stays. A rig reconfigured more than `MAX_SUBSCRIBERS` times
            // between two `collect`s would cost one allocation on one tick,
            // which is why the daemon collects after every reconfiguration.
            retired: Mutex::new(Vec::with_capacity(MAX_SUBSCRIBERS)),
            live: AtomicUsize::new(0),
            next_id: AtomicU64::new(1),
        }
    }

    /// Takes a place and a number without asking whether there is room.
    ///
    /// Set-up only — [`FramePublisher::subscribe`] says why.
    fn claim_unbounded(&self) -> SubscriberId {
        self.live.fetch_add(1, Ordering::AcqRel);
        SubscriberId(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Claims one of the [`MAX_SUBSCRIBERS`] places, or refuses.
    fn claim(&self) -> Result<SubscriberId, EnrolmentFull> {
        let mut live = self.live.load(Ordering::Acquire);
        loop {
            if live >= MAX_SUBSCRIBERS {
                return Err(EnrolmentFull {
                    limit: MAX_SUBSCRIBERS,
                });
            }
            match self.live.compare_exchange_weak(
                live,
                live + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => live = actual,
            }
        }
        Ok(SubscriberId(self.next_id.fetch_add(1, Ordering::Relaxed)))
    }

    /// Builds the two ends of one subscriber's buffer.
    fn build(&self, id: SubscriberId) -> (Link, FrameSubscriber) {
        let slots = Arc::new(Slots::new(Slots::words_for(self.layout.universe_count())));
        (
            Link {
                id,
                slots: Arc::clone(&slots),
                owned: 0,
            },
            FrameSubscriber {
                id,
                slots,
                owned: 2,
                frame: DmxFrame::new(&self.layout),
                layout: Arc::clone(&self.layout),
            },
        )
    }

    /// Puts a link where the caller's thread will free it.
    ///
    /// A slot it cannot get this instant means the link is dropped here after
    /// all — which is one deallocation, once, on a tick that a reconfiguration
    /// has already made the most expensive of the evening. Holding it would mean
    /// carrying it through every tick until the lock came free.
    fn retire(&self, link: Link) {
        if let Ok(mut retired) = self.retired.try_lock() {
            retired.push(link);
        }
    }
}

impl FrameEnrolment {
    /// Adds a subscriber to a publisher that may already be running.
    ///
    /// The buffer is allocated **here**, on the calling thread, and left for the
    /// tick to pick up on its next publish. The subscriber is usable straight
    /// away and reads nothing until then, which is exactly what a driver whose
    /// engine has published nothing new does anyway.
    ///
    /// # Errors
    ///
    /// [`EnrolmentFull`] when [`MAX_SUBSCRIBERS`] are already enrolled. Refused
    /// before anything is allocated, so a caller that asks too often costs
    /// nothing.
    pub fn subscribe(&self) -> Result<FrameSubscriber, EnrolmentFull> {
        let id = self.inner.claim()?;
        let (link, subscriber) = self.inner.build(id);
        if let Ok(mut changes) = self.inner.changes.lock() {
            changes.joining.push(link);
        }
        self.inner.pending.store(true, Ordering::Release);
        Ok(subscriber)
    }

    /// Takes a subscriber off the publisher.
    ///
    /// The frames stop reaching it on the tick that notices; its buffer is freed
    /// by [`collect`](Self::collect), on whatever thread calls that. Naming one
    /// that was never enrolled, or one that has already left, does nothing.
    pub fn unsubscribe(&self, id: SubscriberId) {
        if let Ok(mut changes) = self.inner.changes.lock() {
            changes.leaving.push(id);
        }
        self.inner.live.fetch_sub(1, Ordering::AcqRel);
        self.inner.pending.store(true, Ordering::Release);
    }

    /// Frees the buffers the tick has given up.
    ///
    /// Called from whichever thread owns the reconfiguration — never the tick.
    /// Returns how many were freed, which is what a test counts.
    pub fn collect(&self) -> usize {
        match self.inner.retired.lock() {
            Ok(mut retired) => {
                let count = retired.len();
                retired.clear();
                count
            }
            Err(_) => 0,
        }
    }

    /// How many subscribers are enrolled, the ones still waiting to be taken up
    /// included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.live.load(Ordering::Acquire)
    }

    /// Whether nothing is enrolled at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The layout every frame on this channel has.
    #[must_use]
    pub fn layout(&self) -> &Arc<FrameLayout> {
        &self.inner.layout
    }
}

/// The engine's end of the frame hand-off.
///
/// Owns the frame the tick writes into, so a published frame can never have the
/// wrong shape for its layout. [`publish`](Self::publish) stamps the sequence
/// number and fans the frame out to every subscriber; it is wait-free and does
/// not allocate.
pub struct FramePublisher {
    layout: Arc<FrameLayout>,
    frame: DmxFrame,
    /// Built with room for [`MAX_SUBSCRIBERS`] and never grown — see the module
    /// documentation. A push into spare capacity does not allocate, which is
    /// what lets an output join a running tick.
    links: Vec<Link>,
    enrolment: Arc<Enrolment>,
}

impl FramePublisher {
    /// Creates a publisher for a fixed layout, with no subscribers yet.
    #[must_use]
    pub fn new(layout: Arc<FrameLayout>) -> Self {
        let frame = DmxFrame::new(&layout);
        let enrolment = Arc::new(Enrolment::new(Arc::clone(&layout)));
        let mut links = Vec::new();
        // Once, here, so that every later push is into capacity that is already
        // there. §3.1: the tick may not call the allocator, and `Vec::push` on a
        // full vector does exactly that.
        links.reserve_exact(MAX_SUBSCRIBERS);
        Self {
            layout,
            frame,
            links,
            enrolment,
        }
    }

    /// Adds an output driver's view of the frame stream, straight away.
    ///
    /// Allocates a buffer *and may grow the link vector*, so it is set-up work:
    /// the caller is on its own thread and the tick has not started. An output
    /// that arrives while the show is running goes through
    /// [`enrolment`](Self::enrolment) instead, which cannot grow anything and
    /// refuses past [`MAX_SUBSCRIBERS`] rather than allocating on the tick.
    ///
    /// It is infallible for that reason and not by oversight: a set-up caller
    /// has nowhere useful to put a refusal, and growing the vector here can only
    /// make the tick's push cheaper, never dearer. Every subscriber taken this
    /// way still counts against the enrolment's bound, so a publisher wired up
    /// with sixty-four outputs refuses the sixty-fifth *arriving* one — which is
    /// the case the bound exists for.
    pub fn subscribe(&mut self) -> FrameSubscriber {
        let id = self.enrolment.claim_unbounded();
        let (link, subscriber) = self.enrolment.build(id);
        self.links.push(link);
        subscriber
    }

    /// The channel by which an output joins or leaves while the tick runs.
    ///
    /// Taken before the publisher is moved onto the tick thread and kept by
    /// whoever reconfigures the rig — S33's output patch.
    #[must_use]
    pub fn enrolment(&self) -> FrameEnrolment {
        self.enrolment_handle()
    }

    fn enrolment_handle(&self) -> FrameEnrolment {
        FrameEnrolment {
            inner: Arc::clone(&self.enrolment),
        }
    }

    /// Takes on whoever has joined and lets go of whoever has left.
    ///
    /// One acquire load per publish in the ordinary case, and a `try_lock` that
    /// never blocks in the rare one. The order matters: the leavers go first, so
    /// the vector never holds more than the [`MAX_SUBSCRIBERS`] places the
    /// enrolment has handed out — a subscriber that joined and left before this
    /// ran is taken out of the joining list rather than installed and removed.
    fn take_enrolments(&mut self) {
        if !self.enrolment.pending.load(Ordering::Acquire) {
            return;
        }
        // Never `lock`: this runs on the tick thread, and a thread with a
        // deadline does not wait for one without.
        let Ok(mut changes) = self.enrolment.changes.try_lock() else {
            return;
        };
        let Pending { joining, leaving } = &mut *changes;
        for id in leaving.drain(..) {
            if let Some(index) = joining.iter().position(|link| link.id == id) {
                self.enrolment.retire(joining.remove(index));
                continue;
            }
            if let Some(index) = self.links.iter().position(|link| link.id == id) {
                self.enrolment.retire(self.links.remove(index));
            }
        }
        for link in joining.drain(..) {
            self.links.push(link);
        }
        self.enrolment.pending.store(false, Ordering::Release);
    }

    /// The layout every frame on this channel has.
    #[must_use]
    pub fn layout(&self) -> &FrameLayout {
        &self.layout
    }

    /// The frame the tick is currently building.
    #[must_use]
    pub const fn frame(&self) -> &DmxFrame {
        &self.frame
    }

    /// The frame the tick is currently building, for writing.
    pub const fn frame_mut(&mut self) -> &mut DmxFrame {
        &mut self.frame
    }

    /// Hands the current frame to every subscriber.
    ///
    /// Also the moment an output that has been added or removed since the last
    /// tick takes effect: [`take_enrolments`](Self::take_enrolments) is one
    /// atomic load away from free and is why a reconfiguration costs no tick.
    pub fn publish(&mut self) {
        self.take_enrolments();
        let Self { frame, links, .. } = self;
        frame.set_sequence(frame.sequence().wrapping_add(1));
        for link in links.iter_mut() {
            link.slots.store(link.owned, frame);
            link.owned = link.slots.publish(link.owned);
        }
    }
}

/// One output driver's view of the frame stream.
///
/// [`refresh`](Self::refresh) is wait-free and never blocks the engine. A driver
/// that cannot keep up simply misses frames — `ARCHITECTURE_SPEC.md` §3.2 says
/// that is exactly what should happen, because the alternative is a 30 Hz USB
/// adapter holding a 44 Hz engine back.
pub struct FrameSubscriber {
    id: SubscriberId,
    slots: Arc<Slots>,
    owned: usize,
    frame: DmxFrame,
    layout: Arc<FrameLayout>,
}

impl FrameSubscriber {
    /// Takes the newest published frame, if there is one that has not been read.
    ///
    /// Returns `false` when the engine has published nothing since the last
    /// call, in which case [`frame`](Self::frame) keeps its previous contents —
    /// a driver on its own cadence re-sends that frame rather than going dark.
    pub fn refresh(&mut self) -> bool {
        let (owned, fresh) = self.slots.take(self.owned);
        self.owned = owned;
        if !fresh {
            return false;
        }
        self.slots.load(owned, &mut self.frame);
        true
    }

    /// The most recent frame this subscriber has taken.
    #[must_use]
    pub const fn frame(&self) -> &DmxFrame {
        &self.frame
    }

    /// The layout of that frame.
    #[must_use]
    pub fn layout(&self) -> &FrameLayout {
        &self.layout
    }

    /// Which subscriber this is, as [`FrameEnrolment::unsubscribe`] names it.
    #[must_use]
    pub const fn id(&self) -> SubscriberId {
        self.id
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::FramePublisher;
    use crate::frame::{FrameLayout, UNIVERSE_CHANNELS};
    use crate::sync::Arc;
    use prism_domain::UniverseId;

    fn layout(count: u32) -> Arc<FrameLayout> {
        Arc::new(FrameLayout::new((1..=count).map(UniverseId::new)).unwrap())
    }

    #[test]
    fn a_subscriber_starts_blacked_out_with_nothing_to_read() {
        let mut publisher = FramePublisher::new(layout(2));
        let mut subscriber = publisher.subscribe();
        assert!(!subscriber.refresh());
        assert_eq!(subscriber.frame().sequence(), 0);
        assert!(subscriber.frame().channels().iter().all(|&v| v == 0));
    }

    #[test]
    fn a_published_frame_reaches_the_subscriber_whole() {
        let mut publisher = FramePublisher::new(layout(2));
        let mut subscriber = publisher.subscribe();

        publisher.frame_mut().fill(42);
        assert!(publisher.frame_mut().set_channel(1, 512, 7));
        publisher.publish();

        assert!(subscriber.refresh());
        assert_eq!(subscriber.frame().sequence(), 1);
        assert_eq!(subscriber.frame().channel(0, 1), Some(42));
        assert_eq!(subscriber.frame().channel(1, 512), Some(7));
    }

    #[test]
    fn a_subscriber_that_falls_behind_gets_the_newest_frame_not_the_next_one() {
        // This is the entire point of the structure: an Open DMX adapter that
        // manages 30 Hz against a 44 Hz engine must skip frames, not queue them.
        let mut publisher = FramePublisher::new(layout(1));
        let mut subscriber = publisher.subscribe();

        for value in 1..=10u8 {
            publisher.frame_mut().fill(value);
            publisher.publish();
        }

        assert!(subscriber.refresh());
        assert_eq!(subscriber.frame().sequence(), 10);
        assert_eq!(subscriber.frame().channel(0, 1), Some(10));
    }

    #[test]
    fn refreshing_twice_without_a_publish_reports_nothing_new() {
        let mut publisher = FramePublisher::new(layout(1));
        let mut subscriber = publisher.subscribe();

        publisher.frame_mut().fill(9);
        publisher.publish();

        assert!(subscriber.refresh());
        assert!(!subscriber.refresh());
        // The driver keeps holding the last frame it was given.
        assert_eq!(subscriber.frame().channel(0, 1), Some(9));
        assert_eq!(subscriber.frame().sequence(), 1);
    }

    #[test]
    fn every_subscriber_sees_every_frame_independently() {
        let mut publisher = FramePublisher::new(layout(1));
        let mut fast = publisher.subscribe();
        let mut slow = publisher.subscribe();

        publisher.frame_mut().fill(1);
        publisher.publish();
        assert!(fast.refresh());

        publisher.frame_mut().fill(2);
        publisher.publish();
        assert!(fast.refresh());

        // `slow` never read the first frame and is not owed it.
        assert!(slow.refresh());
        assert_eq!(slow.frame().channel(0, 1), Some(2));
        assert_eq!(fast.frame().channel(0, 1), Some(2));
    }

    #[test]
    fn the_publisher_never_waits_for_a_subscriber_that_never_reads() {
        let mut publisher = FramePublisher::new(layout(1));
        let _subscriber = publisher.subscribe();
        for _ in 0..1_000 {
            publisher.publish();
        }
        assert_eq!(publisher.frame().sequence(), 1_000);
    }

    #[test]
    fn sequence_numbers_are_assigned_by_the_publisher_and_only_increase() {
        let mut publisher = FramePublisher::new(layout(1));
        let mut subscriber = publisher.subscribe();
        let mut last = 0;
        for _ in 0..5 {
            publisher.publish();
            assert!(subscriber.refresh());
            assert!(subscriber.frame().sequence() > last);
            last = subscriber.frame().sequence();
        }
        assert_eq!(last, 5);
    }

    #[test]
    fn a_subscriber_knows_the_layout_it_is_reading() {
        let mut publisher = FramePublisher::new(layout(3));
        let subscriber = publisher.subscribe();
        assert_eq!(subscriber.layout().universe_count(), 3);
        assert_eq!(subscriber.frame().channels().len(), 3 * UNIVERSE_CHANNELS);
    }

    #[test]
    fn a_publisher_with_no_subscribers_still_ticks() {
        // The daemon starts before any output driver is configured.
        let mut publisher = FramePublisher::new(layout(1));
        publisher.frame_mut().fill(5);
        publisher.publish();
        assert_eq!(publisher.frame().sequence(), 1);
    }

    #[test]
    fn a_subscriber_outliving_the_publisher_keeps_its_last_frame() {
        let mut publisher = FramePublisher::new(layout(1));
        let mut subscriber = publisher.subscribe();
        publisher.frame_mut().fill(3);
        publisher.publish();
        assert!(subscriber.refresh());
        drop(publisher);
        assert!(!subscriber.refresh());
        assert_eq!(subscriber.frame().channel(0, 1), Some(3));
    }

    /// S33's hot half, in one place: a subscriber that arrives while the tick is
    /// running starts receiving on the next publish, and the ones that were
    /// already there never miss a frame while it does.
    #[test]
    fn a_subscriber_that_arrives_while_the_tick_runs_receives_from_the_next_frame() {
        let mut publisher = FramePublisher::new(layout(2));
        let enrolment = publisher.enrolment();
        let mut standing = publisher.subscribe();
        assert_eq!(enrolment.len(), 1);

        publisher.frame_mut().fill(1);
        publisher.publish();
        assert!(standing.refresh());

        // Enrolled between two publishes, which is where an `AddOutput` lands.
        let mut arriving = enrolment.subscribe().unwrap();
        assert_eq!(enrolment.len(), 2);
        assert!(
            !arriving.refresh(),
            "nothing has been published since it joined"
        );

        publisher.frame_mut().fill(2);
        publisher.publish();

        assert!(arriving.refresh());
        assert!(arriving.frame().channels().iter().all(|&value| value == 2));
        assert!(standing.refresh());
        assert!(
            standing.frame().channels().iter().all(|&value| value == 2),
            "the subscriber that did not change carries on without a gap"
        );
    }

    #[test]
    fn a_subscriber_that_leaves_stops_receiving_and_is_freed_off_the_tick() {
        let mut publisher = FramePublisher::new(layout(1));
        let enrolment = publisher.enrolment();
        let mut standing = publisher.subscribe();
        let mut leaving = enrolment.subscribe().unwrap();

        publisher.frame_mut().fill(7);
        publisher.publish();
        assert!(leaving.refresh());
        assert_eq!(leaving.frame().channels()[0], 7);

        enrolment.unsubscribe(leaving.id());
        assert_eq!(enrolment.len(), 1, "its place is given back at once");
        publisher.frame_mut().fill(9);
        publisher.publish();

        assert!(
            !leaving.refresh(),
            "a departed subscriber is handed nothing more"
        );
        assert_eq!(leaving.frame().channels()[0], 7, "it keeps its last frame");
        assert!(standing.refresh());
        assert_eq!(standing.frame().channels()[0], 9);

        // The buffer the tick gave up is freed here, on this thread, which is
        // the whole reason `retired` exists.
        assert_eq!(enrolment.collect(), 1);
        assert_eq!(enrolment.collect(), 0);
    }

    /// The awkward interleaving: asked for and given up again before the tick
    /// saw either. It must not be installed and then removed, because the
    /// vector would momentarily hold one more than the enrolment has places for.
    #[test]
    fn a_subscriber_that_joins_and_leaves_between_two_ticks_is_never_installed() {
        let mut publisher = FramePublisher::new(layout(1));
        let enrolment = publisher.enrolment();
        let mut standing = publisher.subscribe();

        let fleeting = enrolment.subscribe().unwrap();
        let id = fleeting.id();
        enrolment.unsubscribe(id);
        assert_eq!(enrolment.len(), 1);

        publisher.frame_mut().fill(3);
        publisher.publish();

        let mut fleeting = fleeting;
        assert!(!fleeting.refresh(), "it was never on the list");
        assert!(standing.refresh());
        assert_eq!(
            enrolment.collect(),
            1,
            "and its buffer came back all the same"
        );

        // Naming one that is not there twice over changes nothing.
        enrolment.unsubscribe(id);
        publisher.publish();
        assert_eq!(enrolment.collect(), 0);
    }

    /// The two paths that only run when a lock is already held: the tick's
    /// `try_lock` on the change list, and `retire`'s on the retirement slot.
    /// Neither may block and neither may lose a subscriber — a hand-over the
    /// tick could not take this instant arrives on the next publish instead.
    #[test]
    fn a_hand_over_the_tick_cannot_take_this_instant_arrives_on_the_next_one() {
        let mut publisher = FramePublisher::new(layout(1));
        let enrolment = publisher.enrolment();
        let mut arriving = enrolment.subscribe().unwrap();

        // Somebody else is holding the change list — a second client adding an
        // output on another thread is exactly this.
        let held = enrolment.inner.changes.lock().unwrap();
        publisher.frame_mut().fill(4);
        publisher.publish();
        assert!(
            !arriving.refresh(),
            "the tick did not wait for the lock, so the subscriber is not on yet"
        );
        drop(held);

        publisher.frame_mut().fill(5);
        publisher.publish();
        assert!(arriving.refresh(), "and it arrives on the next publish");
        assert_eq!(arriving.frame().channels()[0], 5);

        // The same for the retirement slot: a link the tick cannot put down is
        // dropped where it is, which is one deallocation on one tick rather
        // than a link carried through every tick until the lock comes free.
        enrolment.unsubscribe(arriving.id());
        let held = enrolment.inner.retired.lock().unwrap();
        publisher.publish();
        drop(held);
        assert_eq!(
            enrolment.collect(),
            0,
            "it was freed on the tick rather than handed back"
        );
        assert!(!arriving.refresh());
    }

    /// A publisher's own `Debug` is what a breakpoint shows, and it says the two
    /// numbers that explain a hand-over that has not happened yet.
    #[test]
    fn an_enrolment_says_how_many_are_on_it_and_whether_one_is_waiting() {
        let publisher = FramePublisher::new(layout(1));
        let enrolment = publisher.enrolment();
        let subscriber = enrolment.subscribe().unwrap();
        let shown = format!("{enrolment:?}");
        assert!(shown.contains("live: 1"), "{shown}");
        assert!(shown.contains("pending: true"), "{shown}");
        drop(subscriber);
    }

    #[test]
    fn the_enrolment_refuses_the_subscriber_it_has_no_room_for() {
        use super::MAX_SUBSCRIBERS;
        let publisher = FramePublisher::new(layout(1));
        let enrolment = publisher.enrolment();
        assert!(enrolment.is_empty());
        assert_eq!(enrolment.layout().universe_count(), 1);

        let mut held = Vec::new();
        for _ in 0..MAX_SUBSCRIBERS {
            held.push(enrolment.subscribe().unwrap());
        }
        assert_eq!(enrolment.len(), MAX_SUBSCRIBERS);

        let Err(refusal) = enrolment.subscribe() else {
            panic!("a sixty-fifth output must be refused, not carried")
        };
        assert_eq!(refusal.limit, MAX_SUBSCRIBERS);
        assert!(refusal.to_string().contains("64"), "{refusal}");

        // A place given back is a place another output can have.
        let id = held.pop().unwrap().id();
        enrolment.unsubscribe(id);
        assert!(enrolment.subscribe().is_ok());
    }

    /// Numbers are never reused, because a stale removal that came round again
    /// would take a live output's frames away.
    #[test]
    fn a_subscriber_number_is_never_handed_out_twice() {
        let mut publisher = FramePublisher::new(layout(1));
        let enrolment = publisher.enrolment();
        let first = publisher.subscribe().id();
        let second = enrolment.subscribe().unwrap().id();
        enrolment.unsubscribe(second);
        publisher.publish();
        let third = enrolment.subscribe().unwrap().id();
        assert_ne!(first, second);
        assert_ne!(second, third);
        assert!(third.get() > second.get());
        assert!(format!("{first:?}").contains("SubscriberId"));
    }
}

#[cfg(all(loom, test))]
mod loom_tests {
    use super::Slots;
    use crate::sync::{Arc, Ordering};

    /// Words per slot in the model. Two is the smallest number that can tear:
    /// one word from an old frame and one from a new one.
    ///
    /// A real frame is 4097 words, and loom explores every interleaving of every
    /// atomic access — the model would never terminate. Nothing is lost by
    /// shrinking it: the concurrency lives entirely in the control word, which
    /// [`Slots::publish`] and [`Slots::take`] own and which the code below calls
    /// unchanged. The byte packing either side of them is ordinary data movement
    /// and is covered by the unit tests.
    const WORDS: usize = 2;

    fn fill(slots: &Slots, index: usize, value: u64) {
        let Some(slot) = slots.slot(index) else {
            unreachable!()
        };
        for word in slot {
            word.store(value, Ordering::Relaxed);
        }
    }

    /// The value every word of a slot holds, or `None` if they disagree — which
    /// is exactly what a frame assembled from two publishes looks like.
    fn coherent(slots: &Slots, index: usize) -> Option<u64> {
        let slot = slots.slot(index)?;
        let mut words = slot.iter().map(|word| word.load(Ordering::Relaxed));
        let first = words.next()?;
        words.all(|word| word == first).then_some(first)
    }

    /// Whatever order the writer's and the reader's operations are interleaved
    /// in, the reader must never be handed a slot that mixes two publishes, and
    /// must never be handed the same frame twice.
    #[test]
    fn a_reader_never_observes_a_half_written_frame() {
        loom::model(|| {
            let slots = Arc::new(Slots::new(WORDS));
            let writer_slots = Arc::clone(&slots);

            let writer = loom::thread::spawn(move || {
                let mut owned = 0;
                for value in 1..=2u64 {
                    fill(&writer_slots, owned, value);
                    owned = writer_slots.publish(owned);
                }
            });

            let reader = loom::thread::spawn(move || {
                let mut owned = 2;
                let mut last = 0;
                for _ in 0..2 {
                    let (next, fresh) = slots.take(owned);
                    owned = next;
                    if fresh {
                        let value = coherent(&slots, owned).expect("torn frame");
                        assert!(value > last, "frame {value} after {last}");
                        last = value;
                    }
                }
            });

            writer.join().unwrap();
            reader.join().unwrap();
        });
    }

    /// Once both sides have stopped, the writer's slot, the reader's slot and
    /// the published slot are still a permutation of `0..3`. Any interleaving
    /// that let two of them converge would mean one side writing into a slot the
    /// other is reading.
    #[test]
    fn the_writer_and_the_reader_never_own_the_same_slot() {
        loom::model(|| {
            let slots = Arc::new(Slots::new(WORDS));
            let writer_slots = Arc::clone(&slots);
            let reader_slots = Arc::clone(&slots);

            let writer = loom::thread::spawn(move || {
                let owned = writer_slots.publish(0);
                writer_slots.publish(owned)
            });

            let reader = loom::thread::spawn(move || {
                let (owned, _) = reader_slots.take(2);
                let (owned, _) = reader_slots.take(owned);
                owned
            });

            let writer_owns = writer.join().unwrap();
            let reader_owns = reader.join().unwrap();
            let published = slots.shared.load(Ordering::Acquire) & super::INDEX_MASK;
            let mut seen = [writer_owns, reader_owns, published];
            seen.sort_unstable();
            assert_eq!(seen, [0, 1, 2], "slot indices are no longer a permutation");
        });
    }
}
