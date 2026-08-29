//! What a playback is, and how a command names one — S40, settled in S45.
//!
//! # A playback was an executor, then nearly a sequence, and now it is one
//!
//! Until S40 every playback in PrismDMX was an executor: `PlaybackLayer` keyed
//! its sources by [`ExecutorId`], `CueLayer` keyed its players the same way, and
//! every playback command in `docs/IPC_PROTOCOL.md` §5 carried an executor
//! number. That is a good model of a desk — a fader, four keys and a cue list —
//! and it had two holes in it, a session apart.
//!
//! S40 walked into the first: **`On Sequence 1` for a cue list nobody has put on
//! a fader had no representation at all.** The answer then was a second kind of
//! playback, keyed by sequence, live exactly while no executor held that list.
//!
//! **S45 walked into the second, from the other end.** Punch-list entry B18: put
//! one cue list on two executors and `Go` on either starts a playback of its
//! own, each with its own cue pointer and its own fade. Nothing rejected it and
//! nothing merged it — `docs/DMX_MERGE.md` saw two contributors and did what it
//! was told. The rule S40 wrote down for its own two kinds is exactly the rule
//! that was being broken: *two players of one cue list would fight over the same
//! slots in the merge and neither would be wrong.*
//!
//! So a playback is **a cue list's**, always, and this type names one:
//!
//! - one cue pointer and one fade per sequence, whoever pressed Go;
//! - one master level, one rate and one crossfade per sequence, so two executors
//!   whose faders are both `Master` are two handles on one number, while a
//!   `Master` and an `XFade` on that same list stay independent — which is what
//!   B18 asks for in as many words;
//! - and `On Sequence 1` with no executor anywhere is the same playback as a Go
//!   on the fader somebody later puts it on, rather than a second one.
//!
//! **An executor is therefore a handle rather than a player.** It says which cue
//! list, which function each of its four keys has, what its fader does and what
//! its encoder does — and every one of those is editable (S45). What it no
//! longer carries is the playing: `prism_core::Show::playback_of` resolves an
//! executor to the list standing on it, and refuses an executor with none.
//!
//! # Ordering
//!
//! [`PlaybackId`] is `Ord`, by sequence number. Two things depend on it and both
//! are load-bearing: the engine keeps its sources sorted so a lookup is a binary
//! search, and `docs/DMX_MERGE.md` §2.2's LTP tie-break reads the ordering key
//! when two sources went active on the same tick. The tie-break is arbitrary by
//! design — it exists so the merge is a function of the source set rather than
//! of the iteration order — and since S45 there is only one order left to take,
//! which is the one an operator numbered.

use core::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ExecutorId, SequenceId};

/// Which playback: **a cue list's**, and there is exactly one of them per list.
///
/// See the module documentation for how it got here and what B18 was.
///
/// A newtype over [`SequenceId`] rather than the same number under another name,
/// for `crate::ids`' reason: a playback and a cue list are the same *thing* but
/// not the same *fact*, and the places that take one take `impl Into<PlaybackId>`
/// so a caller says which it means. A newtype struct is transparent to serde
/// without being told, so a client reads a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub struct PlaybackId(SequenceId);

impl PlaybackId {
    /// The cue list this playback plays.
    #[must_use]
    pub const fn sequence(self) -> SequenceId {
        self.0
    }

    /// The playback of one cue list.
    #[must_use]
    pub const fn of_sequence(sequence_id: SequenceId) -> Self {
        Self(sequence_id)
    }

    /// The ordering key, as one number.
    ///
    /// The engine packs this into a word it can publish with a single relaxed
    /// store (`prism_engine::readback`) and uses it as `docs/DMX_MERGE.md`
    /// §2.2's LTP tie-break, so it has to be total, cheap and stable. It was two
    /// halves with a kind bit between them until S45; now that a playback is a
    /// cue list, the number an operator gave the list is the whole of it.
    #[must_use]
    pub const fn key(self) -> u64 {
        self.0.get() as u64
    }

    /// The inverse of [`Self::key`].
    ///
    /// Truncating rather than fallible, because the only writer is
    /// [`Self::key`] and the only reader is the readback's relaxed load: a word
    /// that came back with rubbish in its top half is a torn read, and naming a
    /// playback that does not exist is what the reader already tolerates.
    #[must_use]
    pub const fn from_key(key: u64) -> Self {
        Self(SequenceId::new(key as u32))
    }
}

/// A cue list **is** a playback since S45, so the conversion is free and
/// implicit.
///
/// This is what lets `PlaybackLayer`, `CueLayer` and `MergeBody` take
/// `impl Into<PlaybackId>` and read as they always have at the call sites that
/// name a sequence. There is deliberately **no** `From<ExecutorId>`: an executor
/// is a handle on a list rather than a player, and resolving one to its list is
/// `prism_core::Show::playback_of` — a fact about the show, which a client may
/// not work out for itself (**D3**).
impl From<SequenceId> for PlaybackId {
    fn from(sequence_id: SequenceId) -> Self {
        Self(sequence_id)
    }
}

impl fmt::Display for PlaybackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sequence {}", self.0)
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
    fn playbacks_sort_by_the_number_an_operator_gave_the_list() {
        let mut ids = [
            PlaybackId::of_sequence(SequenceId::new(1)),
            PlaybackId::of_sequence(SequenceId::new(u32::MAX)),
            PlaybackId::of_sequence(SequenceId::new(0)),
        ];
        ids.sort();
        assert_eq!(
            ids,
            [
                PlaybackId::of_sequence(SequenceId::new(0)),
                PlaybackId::of_sequence(SequenceId::new(1)),
                PlaybackId::of_sequence(SequenceId::new(u32::MAX)),
            ]
        );
    }

    #[test]
    fn the_ordering_key_agrees_with_the_ordering() {
        let first = PlaybackId::of_sequence(SequenceId::new(0));
        let second = PlaybackId::of_sequence(SequenceId::new(7));
        assert!(first < second);
        assert!(first.key() < second.key());
    }

    #[test]
    fn a_playback_names_the_cue_list_it_plays_and_nothing_else() {
        // S45: an executor is a handle, so there is no executor to ask a
        // playback for. Which list an executor holds is `Show::playback_of`.
        assert_eq!(
            PlaybackId::of_sequence(SequenceId::new(3)).sequence(),
            SequenceId::new(3)
        );
    }

    #[test]
    fn reads_as_words_an_operator_could_be_shown() {
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
    fn a_sequence_number_converts_into_a_playback() {
        assert_eq!(
            PlaybackId::from(SequenceId::new(3)),
            PlaybackId::of_sequence(SequenceId::new(3))
        );
    }

    #[test]
    fn a_playback_is_a_number_on_the_wire_and_a_target_is_still_tagged() {
        // Transparent since S45: what used to be a two-variant tagged object is
        // the cue list's own number. `PlaybackTarget` is untouched, because a
        // *client* still says which of the three ways it means.
        assert_eq!(
            serde_json::to_string(&PlaybackId::of_sequence(SequenceId::new(3))).unwrap(),
            "3"
        );
        assert_eq!(
            serde_json::to_string(&PlaybackTarget::Selected).unwrap(),
            r#"{"t":"Selected"}"#
        );
        assert_eq!(
            serde_json::to_string(&PlaybackTarget::of_executor(ExecutorId::new(1))).unwrap(),
            r#"{"t":"Executor","executorId":1}"#
        );
    }

    proptest! {
        /// The key is what crosses into the tick and comes back out of the
        /// readback, so it has to survive both ways for every playback there is.
        #[test]
        fn the_key_round_trips(raw in any::<u32>()) {
            let id = PlaybackId::of_sequence(SequenceId::new(raw));
            prop_assert_eq!(PlaybackId::from_key(id.key()), id);
        }
    }
}
