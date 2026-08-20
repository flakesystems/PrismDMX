//! What the tick says about its playbacks, read from anywhere without stopping
//! it.
//!
//! S26 recorded the gap and S34 closes it: `prism_domain::Executor` has carried
//! a `current_cue_index` since S1, `prism_domain::Delta::ExecutorState` has
//! carried one on the wire since S11, and **nothing ever filled either**,
//! because what cue a playback is on lives on the tick thread and there was no
//! channel back into the core. The desk therefore drew a dash, and two tests in
//! this repository asserted that dash so that closing the gap would be noticed.
//!
//! # Why atomics and not a queue
//!
//! `ARCHITECTURE_SPEC.md` §3.1 is the constraint and it is not negotiable: the
//! tick may not allocate, may not take a lock and may not block. A channel with
//! a queue behind it would allocate on push; a channel with a fixed ring would
//! grow stale entries nobody drained. What a reader actually wants is not a
//! history of transitions but **the current state**, so this is a table of the
//! current state — one atomic word per playback, published by the tick and
//! sampled by whoever asks.
//!
//! That is the shape of `prismd`'s `TickHealth` and, one layer down, of the
//! triple buffer S2 built: the writer never waits for a reader and a reader
//! never waits for the writer. It is the same *rule*, too — a sample may be one
//! tick out of date, and may never be wrong about a tick that happened.
//!
//! # What a reader may and may not conclude
//!
//! Each entry is written with a single relaxed store, so one entry is always
//! internally consistent: an executor number, its cue index and whether it is
//! running arrive together or not at all. Two entries are **not** guaranteed to
//! come from the same tick, and neither is [`PlaybackReport::len`]. That is
//! deliberate. Making them so would need a seqlock, which would make the reader
//! spin and buy nothing, because this is feedback for a screen rather than
//! something light is controlled with.
//!
//! # The capacity is fixed, and that is a guarantee rather than a limit
//!
//! [`PlaybackReport::new`] allocates once, on whatever thread builds it, and
//! nothing resizes it afterwards — a report that grew would allocate inside the
//! tick. Build it at [`crate::MAX_SOURCES`] and it can never be too small,
//! because `PlaybackLayer::new` refuses a show with more executors than that.

use prism_domain::ExecutorId;

use crate::sync::{AtomicU64, AtomicUsize, Ordering};

/// What one playback is doing, as the tick sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackState {
    /// Which executor.
    pub executor: ExecutorId,
    /// Whether its sequence is running — a cue is in force, or an executor with
    /// no cue list has been switched on by hand.
    pub is_active: bool,
    /// Which cue of its sequence is in force, as an index, or nothing when the
    /// playback is stopped, releasing, or has no cue list at all.
    pub cue_index: Option<u32>,
}

/// Bit of the packed word that says the playback is running.
const ACTIVE: u64 = 1 << 48;

/// Bit of the packed word that says a cue index is present. The index sits
/// above it, so an absent index is never read as a zeroth cue.
const HAS_CUE: u64 = 1 << 49;

/// Where the cue index sits in the packed word.
const CUE_SHIFT: u32 = 50;

/// Largest cue index the packed word can carry.
///
/// Fourteen bits, which is above [`crate::MAX_CUES`] — a sequence that reached
/// this would have been refused by `SequencePlan::build` long before. An index
/// past it is reported as *no cue* rather than as a wrong one.
const MAX_CUE_INDEX: u32 = (1 << 14) - 1;

/// One playback's state, packed into a word that can be published with a single
/// relaxed store.
const fn pack(state: PlaybackState) -> u64 {
    let mut word = state.executor.get() as u64;
    if state.is_active {
        word |= ACTIVE;
    }
    if let Some(index) = state.cue_index
        && index <= MAX_CUE_INDEX
    {
        word |= HAS_CUE | ((index as u64) << CUE_SHIFT);
    }
    word
}

/// The inverse of [`pack`].
const fn unpack(word: u64) -> PlaybackState {
    PlaybackState {
        executor: ExecutorId::new(word as u32),
        is_active: word & ACTIVE != 0,
        cue_index: if word & HAS_CUE == 0 {
            None
        } else {
            Some(((word >> CUE_SHIFT) as u32) & MAX_CUE_INDEX)
        },
    }
}

/// The table the tick publishes its playback state into.
///
/// Shared with [`std::sync::Arc`]: the tick writes through
/// [`crate::MergeBody::report_into`] and the core thread reads.
#[derive(Debug)]
pub struct PlaybackReport {
    entries: Box<[AtomicU64]>,
    len: AtomicUsize,
}

impl PlaybackReport {
    /// A report with room for `capacity` playbacks.
    ///
    /// Allocates, here and never again. Build it at [`crate::MAX_SOURCES`]
    /// unless there is a reason not to — see the module documentation.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: (0..capacity)
                .map(|_| AtomicU64::new(0))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            len: AtomicUsize::new(0),
        }
    }

    /// How many playbacks this report can hold.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    /// How many playbacks the tick last published.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed).min(self.entries.len())
    }

    /// Whether the tick has published nothing at all — a report nobody has
    /// written to, or an engine with no executors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One published playback, by position.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<PlaybackState> {
        if index >= self.len() {
            return None;
        }
        Some(unpack(self.entries.get(index)?.load(Ordering::Relaxed)))
    }

    /// Every published playback, in executor order.
    pub fn states(&self) -> impl Iterator<Item = PlaybackState> + '_ {
        (0..self.len()).filter_map(|index| self.get(index))
    }

    /// Publishes one playback's state. **Called from inside the tick.**
    ///
    /// Reads the word before writing it and stores only when it has changed:
    /// the common tick is one where nothing moved, and a store into a shared
    /// line nobody needed is cache traffic the output threads pay for.
    pub(crate) fn publish(&self, index: usize, state: PlaybackState) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let word = pack(state);
        if entry.load(Ordering::Relaxed) != word {
            entry.store(word, Ordering::Relaxed);
        }
    }

    /// Records how many playbacks were published. **Called from inside the
    /// tick**, after [`Self::publish`], so a reader never sees a length that
    /// reaches past what has been written this tick.
    pub(crate) fn publish_len(&self, len: usize) {
        if self.len.load(Ordering::Relaxed) != len {
            self.len.store(len, Ordering::Relaxed);
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::{MAX_CUE_INDEX, PlaybackReport, PlaybackState, pack, unpack};
    use prism_domain::ExecutorId;

    fn state(executor: u32, is_active: bool, cue_index: Option<u32>) -> PlaybackState {
        PlaybackState {
            executor: ExecutorId::new(executor),
            is_active,
            cue_index,
        }
    }

    #[test]
    fn a_state_survives_the_packing() {
        for candidate in [
            state(0, false, None),
            state(0, true, Some(0)),
            state(u32::MAX, true, Some(MAX_CUE_INDEX)),
            state(9, false, Some(3)),
            state(9, true, None),
        ] {
            assert_eq!(unpack(pack(candidate)), candidate, "{candidate:?}");
        }
    }

    #[test]
    fn an_absent_cue_index_is_not_the_zeroth_cue() {
        // The whole reason the word carries a presence bit: `Some(0)` is *cue
        // one is running* and `None` is *nothing is*, and a desk that confused
        // the two would put a number on a strip that is dark.
        assert_ne!(pack(state(4, true, None)), pack(state(4, true, Some(0))));
        assert_eq!(unpack(pack(state(4, true, None))).cue_index, None);
        assert_eq!(unpack(pack(state(4, true, Some(0)))).cue_index, Some(0));
    }

    #[test]
    fn a_cue_index_too_large_for_the_word_is_reported_as_absent() {
        // Above `MAX_CUES`, so unreachable through `SequencePlan::build` — and
        // reported as *no cue* rather than as a wrong one, because a number the
        // desk cannot trust is worse than a dash.
        let packed = pack(state(1, true, Some(MAX_CUE_INDEX + 1)));
        assert_eq!(unpack(packed).cue_index, None);
        assert!(unpack(packed).is_active);
        assert!(u32::try_from(crate::MAX_CUES).unwrap() <= MAX_CUE_INDEX);
    }

    #[test]
    fn a_report_answers_what_was_published_and_nothing_beyond_it() {
        let report = PlaybackReport::new(4);
        assert_eq!(report.capacity(), 4);
        assert!(report.is_empty());
        assert_eq!(report.get(0), None);

        report.publish(0, state(1, true, Some(2)));
        report.publish(1, state(5, false, None));
        report.publish_len(2);
        assert_eq!(report.len(), 2);
        assert_eq!(report.get(0), Some(state(1, true, Some(2))));
        assert_eq!(report.get(1), Some(state(5, false, None)));
        // Entry 2 was written to but never counted, so it is not readable: an
        // executor grid that shrank must not leave the desk reading a playback
        // the show no longer has.
        report.publish(2, state(9, true, Some(0)));
        assert_eq!(report.get(2), None);
        assert_eq!(report.states().count(), 2);

        report.publish_len(3);
        assert_eq!(report.get(2), Some(state(9, true, Some(0))));
    }

    #[test]
    fn a_report_is_bounded_by_its_own_capacity() {
        // The capacity is fixed because growing one would allocate inside the
        // tick. Publishing past the end is ignored rather than panicking, which
        // is what `CLAUDE.md`'s zero-crash invariant asks of every tick path.
        let report = PlaybackReport::new(1);
        report.publish(5, state(3, true, Some(1)));
        report.publish_len(9);
        assert_eq!(report.len(), 1);
        assert_eq!(report.states().count(), 1);
    }
}
