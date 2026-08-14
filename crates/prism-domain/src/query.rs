//! Questions — `docs/IPC_PROTOCOL.md` §5.2.
//!
//! # Why the protocol grew a third shape
//!
//! A [`crate::Command`] expresses intent and a [`crate::Delta`] describes a
//! change that has already happened. Neither of them can answer *what would
//! happen if*, and S27 needed exactly that: **an address conflict has to be
//! shown before it is committed**, not reported afterwards as a warning beside
//! a patch that has already moved.
//!
//! The two ways of getting there without a question are both worse. A client
//! that worked the overlap out for itself would be a second opinion about
//! something `prism_core::conflict` already decides — the duplication **D3**
//! exists to prevent, and the one that would drift the first time a footprint
//! rule changed. A command that patched and then offered an undo would show the
//! operator the conflict by *making* it, on a rig that is on stage.
//!
//! So a query is a third message: it changes nothing, it is answered to the one
//! client that asked, and the answer is the daemon's own arithmetic. It is
//! deliberately not a `Delta` — a delta is broadcast, and what one operator is
//! typing into a patch form is nobody else's business (`ARCHITECTURE_SPEC.md`
//! §4.2).
//!
//! # A query is not a way to read the show
//!
//! There is no `Query::Show`. The show and the session arrive as documents in
//! the snapshot and are kept current by deltas; a question that returned a
//! second copy of state a client already mirrors would be a second path to the
//! same fact. Every variant here answers something **derived** that no client
//! may derive for itself.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{FixtureId, UniverseId};

/// Two fixtures sharing DMX channels.
///
/// Not an error. Patching a second fixture onto the first is how an operator
/// clones one and it is a technique in daily use, so the engine makes the
/// outcome deterministic — targets are written in merge-slot order, so the
/// **higher fixture number wins** the shared channels (S4) — and the show model
/// reports the overlap rather than refusing it (S11). What an operator needs is
/// a list they can read, and this is one entry of it.
///
/// Ordered by universe and then by the first shared channel, so a list of them
/// reads like a patch sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct PatchConflict {
    /// The universe the overlap is in.
    pub universe: UniverseId,
    /// First shared channel, `1..=512`.
    pub from: u16,
    /// Last shared channel.
    pub to: u16,
    /// The lower of the two fixture numbers.
    pub first: FixtureId,
    /// The higher of the two fixture numbers — the one that **wins** the shared
    /// channels, because the engine writes its targets last (S4).
    pub second: FixtureId,
}

impl core::fmt::Display for PatchConflict {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "fixtures {} and {} share universe {} channels {}-{}; {} wins",
            self.first, self.second, self.universe, self.from, self.to, self.second
        )
    }
}

/// What patching one fixture *would* do, worked out without doing it.
///
/// The answer to `docs/IPC_PROTOCOL.md` §5.2's `PatchPreview`, and the whole of
/// S27's *conflicts are shown before they are committed*. Every field is the
/// daemon's arithmetic over the show it is holding: a client renders it and
/// decides nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct PatchPreview {
    /// Whether a [`crate::Command::PatchFixture`] carrying these fields would be
    /// applied.
    pub accepted: bool,
    /// Why it would not be, in words an operator can read. `None` when it would.
    pub refusal: Option<String>,
    /// How many channels the fixture would occupy, or 0 when the show does not
    /// carry the profile and therefore cannot say.
    pub footprint: u16,
    /// The last channel it would occupy. `None` when it would not fit — which is
    /// also when `accepted` is false for that reason.
    pub last_address: Option<u16>,
    /// The overlaps this patch would create, in patch-sheet order.
    ///
    /// **An overlap is not a refusal**: `accepted` can perfectly well be true
    /// with entries here, and that is the case an operator has to be shown
    /// before they commit rather than told about afterwards.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub conflicts: Vec<PatchConflict>,
}

/// Something a client asks that changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum Query {
    /// Every overlapping address pair in the show as it stands.
    ///
    /// What a patch sheet paints red. Asked rather than mirrored because it is
    /// **derived** from the patch and the embedded profiles: a document field
    /// holding it would be a computed value the daemon then had to keep in step
    /// with the two things it is computed from.
    PatchConflicts,
    /// What patching this fixture at this address would do.
    PatchPreview {
        /// The fixture number that would be patched. An existing number is a
        /// repatch, and repatching a fixture does not conflict with itself.
        id: FixtureId,
        /// The profile it would instantiate.
        type_id: String,
        /// The universe it would go into.
        universe: UniverseId,
        /// The start address it would take.
        address: u16,
    },
}

/// The daemon's answer to a [`Query`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum Answer {
    /// The overlaps in the show as it stands.
    PatchConflicts {
        /// In patch-sheet order.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        conflicts: Vec<PatchConflict>,
    },
    /// What that patch would do.
    PatchPreview {
        /// The whole of it.
        preview: PatchPreview,
    },
}

#[cfg(test)]
mod tests {
    use super::{Answer, PatchConflict, PatchPreview, Query};
    use crate::{FixtureId, UniverseId};

    fn conflict() -> PatchConflict {
        PatchConflict {
            universe: UniverseId::new(1),
            from: 3,
            to: 4,
            first: FixtureId::new(1),
            second: FixtureId::new(2),
        }
    }

    #[test]
    fn a_conflict_says_which_fixture_wins() {
        assert_eq!(
            conflict().to_string(),
            "fixtures 1 and 2 share universe 1 channels 3-4; 2 wins"
        );
    }

    #[test]
    fn conflicts_sort_into_patch_sheet_order() {
        let later = PatchConflict {
            universe: UniverseId::new(2),
            from: 1,
            ..conflict()
        };
        let further_along = PatchConflict {
            from: 9,
            ..conflict()
        };
        let mut all = vec![later, further_along, conflict()];
        all.sort_unstable();
        assert_eq!(all, vec![conflict(), further_along, later]);
    }

    #[test]
    fn queries_and_answers_are_internally_tagged_with_t() {
        assert_eq!(
            serde_json::to_string(&Query::PatchConflicts).unwrap(),
            r#"{"t":"PatchConflicts"}"#
        );
        assert_eq!(
            serde_json::to_string(&Query::PatchPreview {
                id: FixtureId::new(7),
                type_id: "generic.dimmer".to_owned(),
                universe: UniverseId::new(2),
                address: 11,
            })
            .unwrap(),
            r#"{"t":"PatchPreview","id":7,"typeId":"generic.dimmer","universe":2,"address":11}"#
        );
        assert_eq!(
            serde_json::to_string(&Answer::PatchConflicts {
                conflicts: vec![conflict()],
            })
            .unwrap(),
            r#"{"t":"PatchConflicts","conflicts":[{"universe":1,"from":3,"to":4,"first":1,"second":2}]}"#
        );
    }

    /// An overlap is **not** a refusal, and the type has to be able to say so.
    #[test]
    fn a_preview_can_be_accepted_and_still_carry_conflicts() {
        let preview = PatchPreview {
            accepted: true,
            refusal: None,
            footprint: 4,
            last_address: Some(6),
            conflicts: vec![conflict()],
        };
        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains(r#""accepted":true"#), "{json}");
        assert!(json.contains(r#""refusal":null"#), "{json}");
        assert_eq!(
            serde_json::from_str::<PatchPreview>(&json).unwrap(),
            preview
        );
    }

    #[test]
    fn every_query_and_answer_survives_both_wire_formats() {
        let queries = [
            Query::PatchConflicts,
            Query::PatchPreview {
                id: FixtureId::new(1),
                type_id: String::new(),
                universe: UniverseId::new(64),
                address: 512,
            },
        ];
        for query in queries {
            let json = serde_json::to_string(&query).unwrap();
            assert_eq!(serde_json::from_str::<Query>(&json).unwrap(), query);
            let packed = rmp_serde::to_vec_named(&query).unwrap();
            assert_eq!(rmp_serde::from_slice::<Query>(&packed).unwrap(), query);
        }

        let answers = [
            Answer::PatchConflicts {
                conflicts: Vec::new(),
            },
            Answer::PatchPreview {
                preview: PatchPreview {
                    accepted: false,
                    refusal: Some("fixture 1 does not fit".to_owned()),
                    footprint: 0,
                    last_address: None,
                    conflicts: vec![conflict()],
                },
            },
        ];
        for answer in answers {
            let json = serde_json::to_string(&answer).unwrap();
            assert_eq!(serde_json::from_str::<Answer>(&json).unwrap(), answer);
            let packed = rmp_serde::to_vec_named(&answer).unwrap();
            assert_eq!(rmp_serde::from_slice::<Answer>(&packed).unwrap(), answer);
        }
    }
}
