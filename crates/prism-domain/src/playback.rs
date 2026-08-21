//! What a playback is, and how a command names one — S40.
//!
//! # A playback used to be an executor, and now it is not quite
//!
//! Until S40 every playback in PrismDMX was an executor: `PlaybackLayer` keyed
//! its sources by [`ExecutorId`], `CueLayer` keyed its players the same way, and
//! every playback command in `docs/IPC_PROTOCOL.md` §5 carried an executor
//! number. That is a good model of a desk — a fader, four keys and a cue list —
//! and it has one hole in it, which S40's command line walks straight into:
//! **`On Sequence 1` for a cue list nobody has put on a fader had no
//! representation at all.**
//!
//! S39 had already decided the half of this that is about *editing*:
//! `Session::selectedSequence` exists so a cue list can be written before
//! anybody decides which fader it goes on (`ARCHITECTURE_SPEC.md` §4.1). S40 is
//! the same sentence about *playing* it, and it is the other half of the same
//! argument — a list you can write but not hear is a list you cannot check.
//!
//! So a playback is one of two things, and [`PlaybackId`] is that choice:
//!
//! - an **executor** — a slot on the desk, with a master, a speed, a crossfade
//!   and four keys, which is what `docs/DMX_MERGE.md` §2 has always merged;
//! - a **sequence** — a cue list playing on nothing, with the master at full and
//!   no keys, which exists exactly while no executor holds it.
//!
//! **The two are never both live for one cue list.** The daemon resolves a
//! [`PlaybackTarget::Sequence`] to the executor that plays it when there is one,
//! and to the sequence's own playback when there is not. Two players of one list
//! running side by side would fight over the same slots in the merge and neither
//! would be wrong, which is the worst kind of bug a lighting desk can have.
//!
//! # Ordering
//!
//! [`PlaybackId`] is `Ord` and the order is *every executor, then every
//! sequence*, each by number. Two things depend on it and both are load-bearing:
//! the engine keeps its sources sorted so a lookup is a binary search, and
//! `docs/DMX_MERGE.md` §2.2's LTP tie-break reads the ordering key when two
//! sources went active on the same tick. The tie-break was already arbitrary —
//! it exists so the merge is a function of the source set rather than of the
//! iteration order — and putting the sequence playbacks after the executors
//! keeps a desk's own faders winning it, which is the answer an operator would
//! guess.

use core::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ExecutorId, SequenceId};

/// Which playback: a desk executor, or a sequence playing on no fader.
///
/// See the module documentation for why there are two and when each exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum PlaybackId {
    /// A slot on the desk: `page * 8 + slot` (**D7**).
    Executor {
        /// The executor.
        executor_id: ExecutorId,
    },
    /// A cue list playing without an executor.
    ///
    /// Exists only while **no** executor holds this sequence — see the module
    /// documentation.
    Sequence {
        /// The cue list.
        sequence_id: SequenceId,
    },
}

impl PlaybackId {
    /// The executor, if this is one.
    #[must_use]
    pub const fn executor(self) -> Option<ExecutorId> {
        match self {
            Self::Executor { executor_id } => Some(executor_id),
            Self::Sequence { .. } => None,
        }
    }

    /// The sequence, if this is a sequence's own playback.
    #[must_use]
    pub const fn sequence(self) -> Option<SequenceId> {
        match self {
            Self::Sequence { sequence_id } => Some(sequence_id),
            Self::Executor { .. } => None,
        }
    }

    /// A wrapper for an executor number.
    #[must_use]
    pub const fn of_executor(executor_id: ExecutorId) -> Self {
        Self::Executor { executor_id }
    }

    /// A wrapper for a sequence number.
    #[must_use]
    pub const fn of_sequence(sequence_id: SequenceId) -> Self {
        Self::Sequence { sequence_id }
    }

    /// The ordering key, as one number.
    ///
    /// The engine packs this into a word it can publish with a single relaxed
    /// store (`prism_engine::readback`) and uses it as `docs/DMX_MERGE.md`
    /// §2.2's LTP tie-break, so it has to be total, cheap and stable. Bit 32 is
    /// the kind, which is what puts every sequence playback after every
    /// executor.
    #[must_use]
    pub const fn key(self) -> u64 {
        match self {
            Self::Executor { executor_id } => executor_id.get() as u64,
            Self::Sequence { sequence_id } => (1 << 32) | sequence_id.get() as u64,
        }
    }

    /// The inverse of [`Self::key`].
    #[must_use]
    pub const fn from_key(key: u64) -> Self {
        if key & (1 << 32) == 0 {
            Self::Executor {
                executor_id: ExecutorId::new(key as u32),
            }
        } else {
            Self::Sequence {
                sequence_id: SequenceId::new(key as u32),
            }
        }
    }
}

/// **An executor *is* a playback**, so the conversion is free and implicit.
///
/// This is what lets `PlaybackLayer`, `CueLayer` and `MergeBody` take
/// `impl Into<PlaybackId>` and go on reading as they did before S40 at every
/// call site that names an executor - which is nearly all of them. It converts
/// in one direction only: [`PlaybackId::executor`] answers `None` for a
/// sequence, and a caller that needs an executor number has to say what it
/// means by one.
impl From<ExecutorId> for PlaybackId {
    fn from(executor_id: ExecutorId) -> Self {
        Self::Executor { executor_id }
    }
}

/// A cue list on no fader is a playback too - see the module documentation.
impl From<SequenceId> for PlaybackId {
    fn from(sequence_id: SequenceId) -> Self {
        Self::Sequence { sequence_id }
    }
}

impl fmt::Display for PlaybackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Executor { executor_id } => write!(f, "executor {executor_id}"),
            Self::Sequence { sequence_id } => write!(f, "sequence {sequence_id}"),
        }
    }
}

/// What a playback command is addressed to — S40's `On`, `Off`, `Go+`, `Go-`
/// and `Goto`.
///
/// **This is what a client sends; [`PlaybackId`] is what the daemon resolves it
/// to.** The two are deliberately different types, because a client may not
/// resolve one into the other:
///
/// - [`Self::Sequence`] becomes the executor that holds that cue list, or the
///   sequence's own playback when nothing does — which is a fact about the
///   *show*, and a client that worked it out would race an `AssignExecutor` from
///   a second client (**D3**);
/// - [`Self::Selected`] becomes whatever `Session::selectedSequence` names —
///   which is *session* state, so a client filling it in would be sending a
///   command whose meaning had already moved.
///
/// The last one is what makes a bare `Go+` on the command line mean something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum PlaybackTarget {
    /// One executor of the desk — `Go+ Executor 1`, and every press of a strip.
    Executor {
        /// The executor.
        executor_id: ExecutorId,
    },
    /// One cue list, wherever it is playing — `Go+ Sequence 1`.
    Sequence {
        /// The cue list.
        sequence_id: SequenceId,
    },
    /// The cue list the session has selected — a bare `Go+`, `On`, `Off` or
    /// `Goto Cue 5`.
    ///
    /// Refused when nothing is selected, which is a message rather than a
    /// silence: *no cue list is selected* is a complaint an operator can act on.
    Selected,
}

impl PlaybackTarget {
    /// A target naming one executor.
    #[must_use]
    pub const fn of_executor(executor_id: ExecutorId) -> Self {
        Self::Executor { executor_id }
    }

    /// A target naming one cue list.
    #[must_use]
    pub const fn of_sequence(sequence_id: SequenceId) -> Self {
        Self::Sequence { sequence_id }
    }
}

impl fmt::Display for PlaybackTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Executor { executor_id } => write!(f, "executor {executor_id}"),
            Self::Sequence { sequence_id } => write!(f, "sequence {sequence_id}"),
            Self::Selected => f.write_str("the selected sequence"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlaybackId, PlaybackTarget};
    use crate::{ExecutorId, SequenceId};
    use proptest::prelude::*;

    #[test]
    fn every_executor_sorts_before_every_sequence() {
        let mut ids = [
            PlaybackId::of_sequence(SequenceId::new(1)),
            PlaybackId::of_executor(ExecutorId::new(u32::MAX)),
            PlaybackId::of_executor(ExecutorId::new(2)),
            PlaybackId::of_sequence(SequenceId::new(0)),
        ];
        ids.sort();
        assert_eq!(
            ids,
            [
                PlaybackId::of_executor(ExecutorId::new(2)),
                PlaybackId::of_executor(ExecutorId::new(u32::MAX)),
                PlaybackId::of_sequence(SequenceId::new(0)),
                PlaybackId::of_sequence(SequenceId::new(1)),
            ]
        );
    }

    #[test]
    fn the_ordering_key_agrees_with_the_ordering() {
        let executor = PlaybackId::of_executor(ExecutorId::new(u32::MAX));
        let sequence = PlaybackId::of_sequence(SequenceId::new(0));
        assert!(executor < sequence);
        assert!(executor.key() < sequence.key());
    }

    #[test]
    fn the_two_halves_are_each_other_s_inverse() {
        assert_eq!(PlaybackId::of_executor(ExecutorId::new(3)).sequence(), None);
        assert_eq!(
            PlaybackId::of_executor(ExecutorId::new(3)).executor(),
            Some(ExecutorId::new(3))
        );
        assert_eq!(PlaybackId::of_sequence(SequenceId::new(3)).executor(), None);
        assert_eq!(
            PlaybackId::of_sequence(SequenceId::new(3)).sequence(),
            Some(SequenceId::new(3))
        );
    }

    #[test]
    fn reads_as_words_an_operator_could_be_shown() {
        assert_eq!(
            PlaybackId::of_executor(ExecutorId::new(3)).to_string(),
            "executor 3"
        );
        assert_eq!(
            PlaybackId::of_sequence(SequenceId::new(7)).to_string(),
            "sequence 7"
        );
        assert_eq!(
            PlaybackTarget::of_executor(ExecutorId::new(3)).to_string(),
            "executor 3"
        );
        assert_eq!(
            PlaybackTarget::of_sequence(SequenceId::new(7)).to_string(),
            "sequence 7"
        );
        assert_eq!(
            PlaybackTarget::Selected.to_string(),
            "the selected sequence"
        );
    }

    #[test]
    fn an_executor_number_converts_into_a_playback_and_a_sequence_number_does_too() {
        assert_eq!(
            PlaybackId::from(ExecutorId::new(3)),
            PlaybackId::of_executor(ExecutorId::new(3))
        );
        assert_eq!(
            PlaybackId::from(SequenceId::new(3)),
            PlaybackId::of_sequence(SequenceId::new(3))
        );
    }

    #[test]
    fn serialises_as_a_tagged_object() {
        assert_eq!(
            serde_json::to_string(&PlaybackId::of_executor(ExecutorId::new(3))).unwrap(),
            r#"{"t":"Executor","executorId":3}"#
        );
        assert_eq!(
            serde_json::to_string(&PlaybackTarget::Selected).unwrap(),
            r#"{"t":"Selected"}"#
        );
    }

    proptest! {
        /// The key is what crosses into the tick and comes back out of the
        /// readback, so it has to survive both ways for every playback there is.
        #[test]
        fn the_key_round_trips(raw in any::<u32>(), sequence in any::<bool>()) {
            let id = if sequence {
                PlaybackId::of_sequence(SequenceId::new(raw))
            } else {
                PlaybackId::of_executor(ExecutorId::new(raw))
            };
            prop_assert_eq!(PlaybackId::from_key(id.key()), id);
        }
    }
}
