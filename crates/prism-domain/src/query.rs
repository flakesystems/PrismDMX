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

use crate::{FeatureGroup, FixtureId, PresetId, SequenceId, UniverseId};

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

/// One profile in the desk's library, as a menu shows it.
///
/// Deliberately **not** a [`crate::FixtureType`]: the library is some two
/// thousand profiles (S44), and a client that was sent whole profiles could not
/// hold them and could not receive them either — `docs/IPC_PROTOCOL.md` §3 caps
/// a frame at 1 MiB. So a search answers with these, and
/// `Command::EmbedFixtureType` names the one that was chosen by its key. The
/// daemon is the only thing that ever holds a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    /// The key `EmbedFixtureType` names it by — `manufacturer/fixture/mode` for
    /// a profile out of the Open Fixture Library.
    pub id: String,
    /// Who makes it.
    pub manufacturer: String,
    /// What it is called.
    pub name: String,
    /// Which mode of it, which is what makes two entries of one fixture
    /// different things to patch.
    pub mode: String,
    /// How many channels one of them occupies.
    pub footprint: u16,
}

/// What a store would be filed under — the cue or the preset it would land in.
///
/// Carries the same fields the command does, minus the ones a preview cannot be
/// affected by: `Command::StorePreset`'s name and colour are written whatever is
/// already there, so asking about them would be asking about the client's own
/// text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum StoreTarget {
    /// A cue of a sequence, by the number an operator types.
    Cue {
        /// The sequence it would go into.
        sequence_id: SequenceId,
        /// The cue number, as typed.
        cue_number: String,
    },
    /// A preset of a pool.
    Preset {
        /// The preset number.
        preset_id: PresetId,
        /// The pool, which is **what decides which programmer values are
        /// taken**: a colour preset stores the colour values and nothing else,
        /// and which bank an attribute is on is the *profile's* answer rather
        /// than the attribute name's.
        pool: FeatureGroup,
    },
}

/// How a store combines with what is already there.
///
/// **One value, and that is the point of the type.** `prism_core::Programmer`
/// merges unconditionally and its own documentation names Merge / Override /
/// Remove as the distinction a console makes — which is **S39**'s to build. Until
/// then the daemon answers with the mode it will actually use and the interface
/// puts that word on the button, rather than offering a choice it cannot honour.
///
/// A client must not spell the word itself: when S39 adds the other two, a
/// hard-coded "Merge" in an interface would go on being right-looking and wrong.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum StoreMode {
    /// What is stored is added to what is there; nothing is taken away.
    ///
    /// The programmer is sparse by specification, so a store carries only what
    /// was touched this time — and overwriting would delete every value in the
    /// cue the operator did not happen to touch, which is data loss the command
    /// has no way to ask for.
    #[default]
    Merge,
}

/// What storing the programmer into a cue or a preset *would* do.
///
/// S28's exit criterion in one type: **a store that would overwrite says what it
/// will do before it does it, even where the only mode available is Merge.** So
/// the three counts are the whole of what Merge means, said as numbers rather
/// than as a warning an operator learns to click past.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct StorePreview {
    /// Whether the store would be applied.
    pub accepted: bool,
    /// Why it would not be, in words an operator can read. `None` when it would.
    pub refusal: Option<String>,
    /// Whether something is already filed under that number.
    ///
    /// The difference between *create* and *overwrite*, which is the first thing
    /// an operator needs to know and the one a button label can carry.
    pub exists: bool,
    /// What is filed there now, or empty when nothing is.
    pub name: String,
    /// How this store would combine with what is there.
    pub mode: StoreMode,
    /// Values the programmer would add that are not stored yet.
    pub added: u32,
    /// Values already stored that the programmer would write over.
    pub replaced: u32,
    /// Values already stored that this store would **leave alone**.
    ///
    /// The number that makes Merge legible: it is exactly what an Override would
    /// have thrown away.
    pub kept: u32,
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
    /// Profiles in the desk's library matching what has been typed.
    ///
    /// Asked rather than mirrored because the library is **large** (S44): some
    /// two thousand profiles, which is neither a frame nor a menu. An empty
    /// `text` answers with the first `limit` entries, so a client that has not
    /// typed anything has something to show.
    SearchLibrary {
        /// What the operator typed. Words, in any order, matched against the
        /// manufacturer, the name, the mode and the key.
        text: String,
        /// How many to answer with. Clamped by the daemon, so a client cannot
        /// ask for an answer that would not fit in a frame.
        limit: u32,
    },
    /// What storing the programmer into this cue or preset would do.
    ///
    /// Asked rather than worked out because the answer depends on three things a
    /// client holds none of together: what the programmer holds, what is already
    /// filed under that number, and **which store mode this build actually has**
    /// (`StoreMode`). S28's criterion is that a store says what it will do
    /// before it does it, and a client that counted the overlap itself would be
    /// a second opinion about `prism_core::Programmer`'s own merge.
    StorePreview {
        /// Where it would go.
        target: StoreTarget,
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
    /// The profiles that matched, best first.
    LibraryMatches {
        /// At most the number asked for.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        matches: Vec<LibraryEntry>,
        /// How many profiles the library holds in total, so a client can say
        /// *50 of 2 084* rather than implying the list is all there is.
        total: u32,
    },
    /// What that store would do.
    StorePreview {
        /// The whole of it.
        preview: StorePreview,
    },
}

#[cfg(test)]
mod tests {
    use super::{Answer, PatchConflict, PatchPreview, Query, StoreMode, StorePreview, StoreTarget};
    use crate::{FeatureGroup, FixtureId, PresetId, SequenceId, UniverseId};

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

    /// A store target names the cue or the preset, and nothing about the store.
    #[test]
    fn a_store_target_is_where_it_would_go_and_not_what_would_go_there() {
        assert_eq!(
            serde_json::to_string(&StoreTarget::Cue {
                sequence_id: SequenceId::new(5),
                cue_number: "1.5".to_owned(),
            })
            .unwrap(),
            r#"{"t":"Cue","sequenceId":5,"cueNumber":"1.5"}"#
        );
        assert_eq!(
            serde_json::to_string(&StoreTarget::Preset {
                preset_id: PresetId::new(4),
                pool: FeatureGroup::Color,
            })
            .unwrap(),
            r#"{"t":"Preset","presetId":4,"pool":"Color"}"#
        );
    }

    /// **The mode is the daemon's word, and today there is one of them.**
    ///
    /// Asserted rather than assumed, because the whole reason `StoreMode` is a
    /// type is that S39 will add Override and Remove — and an interface that had
    /// spelled `"Merge"` itself would go on looking right after that. When this
    /// assertion fails, every reader of `StorePreview::mode` has a second case
    /// to answer for.
    #[test]
    fn merge_is_the_only_store_mode_this_build_has() {
        assert_eq!(
            serde_json::to_string(&StoreMode::Merge).unwrap(),
            r#""Merge""#
        );
        assert_eq!(StoreMode::default(), StoreMode::Merge);
    }

    /// A preview says what a store would do, and *would overwrite* is not the
    /// same fact as *would be refused*.
    #[test]
    fn a_store_preview_can_overwrite_and_still_be_accepted() {
        let preview = StorePreview {
            accepted: true,
            refusal: None,
            exists: true,
            name: "Warm wash".to_owned(),
            mode: StoreMode::Merge,
            added: 2,
            replaced: 1,
            kept: 7,
        };
        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains(r#""exists":true"#), "{json}");
        assert!(json.contains(r#""kept":7"#), "{json}");
        assert_eq!(
            serde_json::from_str::<StorePreview>(&json).unwrap(),
            preview
        );

        let packed = rmp_serde::to_vec_named(&Answer::StorePreview {
            preview: preview.clone(),
        })
        .unwrap();
        assert_eq!(
            rmp_serde::from_slice::<Answer>(&packed).unwrap(),
            Answer::StorePreview { preview }
        );
    }

    /// The question travels in the same envelope as the other three.
    #[test]
    fn a_store_preview_is_asked_like_every_other_question() {
        let query = Query::StorePreview {
            target: StoreTarget::Cue {
                sequence_id: SequenceId::new(1),
                cue_number: "2".to_owned(),
            },
        };
        assert_eq!(
            serde_json::to_string(&query).unwrap(),
            r#"{"t":"StorePreview","target":{"t":"Cue","sequenceId":1,"cueNumber":"2"}}"#
        );
        let packed = rmp_serde::to_vec_named(&query).unwrap();
        assert_eq!(rmp_serde::from_slice::<Query>(&packed).unwrap(), query);
    }
}
