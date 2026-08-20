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

use crate::frame::{DmxFrame, FrameLayout, UNIVERSE_CHANNELS};
use crate::sync::{Arc, AtomicU64, AtomicUsize, Ordering};

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

/// The writer's end of one subscriber's buffer.
struct Link {
    slots: Arc<Slots>,
    owned: usize,
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
    links: Vec<Link>,
}

impl FramePublisher {
    /// Creates a publisher for a fixed layout, with no subscribers yet.
    #[must_use]
    pub fn new(layout: Arc<FrameLayout>) -> Self {
        let frame = DmxFrame::new(&layout);
        Self {
            layout,
            frame,
            links: Vec::new(),
        }
    }

    /// Adds an output driver's view of the frame stream.
    ///
    /// Allocates a buffer, so it must be called during setup — before the tick
    /// thread starts, never from inside it.
    pub fn subscribe(&mut self) -> FrameSubscriber {
        let slots = Arc::new(Slots::new(Slots::words_for(self.layout.universe_count())));
        self.links.push(Link {
            slots: Arc::clone(&slots),
            owned: 0,
        });
        FrameSubscriber {
            slots,
            owned: 2,
            frame: DmxFrame::new(&self.layout),
            layout: Arc::clone(&self.layout),
        }
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
    pub fn publish(&mut self) {
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
