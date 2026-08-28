//! What is wrong with a show, reported rather than refused.
//!
//! # Overlapping addresses are not an error
//!
//! S4 settled this in the engine and the show model follows it: patching a
//! second fixture onto the first is how an operator clones one, and it is a
//! technique in daily use. The engine allows it and makes the outcome
//! deterministic — targets are written in merge-slot order, so the **higher
//! fixture number wins** the shared channel. What an operator needs is not a
//! refusal but a list they can read, which is what [`PatchConflict`] is: the
//! patch sheet (S27) shows it, and an accidental overlap becomes visible
//! instead of becoming a mystery on stage.
//!
//! # Dangling references are not an error either
//!
//! A show outlives the rig it was written on. S5 already decided that a cue
//! naming a fixture that is no longer patched is dropped when the sequence is
//! compiled rather than refusing the whole cue list, and the same reasoning
//! applies to a group, a preset or an executor. [`ShowIssue`] is the list of
//! them, so nothing is dropped *silently*.

use core::fmt;

use prism_domain::{
    ExecutorId, FixtureId, GroupId, PatchConflict, PatchPreview, PresetId, SequenceId, UniverseId,
};

use crate::show::Show;

/// One fixture's address span, as the overlap search walks them.
///
/// `(universe, first channel, last channel, fixture)` in that order, so sorting
/// a list of them puts a universe's fixtures together and in address order —
/// which is what lets the search below stop at the first fixture that starts
/// past the end of the one being examined.
type Span = (UniverseId, u16, u16, FixtureId);

/// Something a patch sheet should show in red.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowIssue {
    /// Two fixtures share channels.
    AddressOverlap(PatchConflict),
    /// A patched fixture instantiates a profile the show does not carry, so it
    /// cannot be addressed at all. Only reachable through a hand-edited file.
    MissingFixtureType {
        /// The fixture.
        fixture: FixtureId,
        /// The profile key it names.
        type_id: String,
    },
    /// A group lists a fixture that is not patched.
    GroupMemberUnpatched {
        /// The group.
        group: GroupId,
        /// The member.
        fixture: FixtureId,
    },
    /// A preset holds a value for a fixture that is not patched.
    PresetValueUnpatched {
        /// The preset.
        preset: PresetId,
        /// The fixture.
        fixture: FixtureId,
    },
    /// A cue part names a fixture that is not patched. The cue still runs; the
    /// part is dropped when the sequence is compiled (S5).
    CuePartUnpatched {
        /// The sequence.
        sequence: SequenceId,
        /// The cue number.
        cue: String,
        /// The fixture.
        fixture: FixtureId,
    },
    /// A cue part is linked to a preset that has been deleted. The part keeps
    /// the value it was stored with and stops following the preset.
    CuePresetMissing {
        /// The sequence.
        sequence: SequenceId,
        /// The cue number.
        cue: String,
        /// The preset it points at.
        preset: PresetId,
    },
    /// An executor plays a sequence that no longer exists.
    ExecutorSequenceMissing {
        /// The executor.
        executor: ExecutorId,
        /// The sequence it points at.
        sequence: SequenceId,
    },
    /// The show patches a universe that **no output carries** — S33.
    ///
    /// A legitimate state and not an error: a rig is built over an afternoon,
    /// and refusing to patch a fixture into universe 7 until somebody had wired
    /// universe 7 would be a desk that cannot be prepared in advance. What it
    /// must never be is *silent* — an operator whose universe 7 goes nowhere has
    /// to read that before the show rather than discover it when the light does
    /// not come up. See [`crate::outputs`].
    UniverseNotOutput {
        /// The universe the patch uses and nothing sends.
        universe: UniverseId,
    },
}

impl fmt::Display for ShowIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddressOverlap(conflict) => write!(f, "{conflict}"),
            Self::MissingFixtureType { fixture, type_id } => {
                write!(f, "fixture {fixture} needs the profile {type_id:?}")
            }
            Self::GroupMemberUnpatched { group, fixture } => {
                write!(
                    f,
                    "group {group} lists fixture {fixture}, which is not patched"
                )
            }
            Self::PresetValueUnpatched { preset, fixture } => {
                write!(
                    f,
                    "preset {preset} holds a value for fixture {fixture}, which is not patched"
                )
            }
            Self::CuePartUnpatched {
                sequence,
                cue,
                fixture,
            } => write!(
                f,
                "sequence {sequence} cue {cue} sets fixture {fixture}, which is not patched"
            ),
            Self::CuePresetMissing {
                sequence,
                cue,
                preset,
            } => write!(
                f,
                "sequence {sequence} cue {cue} is linked to preset {preset}, which is gone"
            ),
            Self::UniverseNotOutput { universe } => {
                write!(f, "universe {universe} is patched and no output carries it")
            }
            Self::ExecutorSequenceMissing { executor, sequence } => write!(
                f,
                "executor {executor} plays sequence {sequence}, which is gone"
            ),
        }
    }
}

/// Every fixture's address span, sorted.
fn spans(show: &Show) -> Vec<Span> {
    let mut spans: Vec<Span> = show
        .patched()
        .filter_map(|(fixture, fixture_type)| {
            fixture
                .last_address(fixture_type.footprint)
                .map(|last| (fixture.universe, fixture.address, last, fixture.id))
        })
        .collect();
    spans.sort_unstable();
    spans
}

/// Every pair of fixtures sharing channels, in patch-sheet order.
pub(crate) fn conflicts(show: &Show) -> Vec<PatchConflict> {
    let spans = spans(show);
    let mut found = Vec::new();
    for (index, &(universe, _, end, id)) in spans.iter().enumerate() {
        for &(other_universe, other_start, other_end, other_id) in &spans[index + 1..] {
            if other_universe != universe || other_start > end {
                break;
            }
            found.push(PatchConflict {
                universe,
                from: other_start,
                to: end.min(other_end),
                first: id.min(other_id),
                second: id.max(other_id),
            });
        }
    }
    found.sort_unstable();
    found
}

/// What patching `id` at `universe`/`address` would do — worked out **without
/// doing it**.
///
/// This is the daemon's half of S27's *address conflicts are shown before they
/// are committed*. It answers three things a client must not work out for
/// itself: whether the patch would be accepted at all and why not, how many
/// channels it would occupy, and which fixtures it would overlap.
///
/// The fixture is excluded from its own overlap search, because a repatch of
/// fixture 3 onto the channels fixture 3 already has is not a conflict — it is
/// the same fixture, in the same place, and reporting it would put a red line
/// under every row an operator opened and did not change.
pub(crate) fn preview(
    show: &Show,
    id: FixtureId,
    type_id: &str,
    universe: UniverseId,
    address: u16,
) -> PatchPreview {
    // The refusal is the show model's own, taken by asking it: a second copy of
    // the validation here would be the duplication this whole preview exists to
    // avoid. `check_patch` writes nothing.
    let refusal = show
        .check_patch(id, type_id, universe, address)
        .err()
        .map(|error| error.to_string());
    let footprint = show
        .fixture_type(type_id)
        .map_or(0, |fixture_type| fixture_type.footprint);
    let last_address = (footprint > 0 && address > 0)
        .then(|| address.checked_add(footprint - 1))
        .flatten()
        .filter(|last| *last <= prism_domain::CHANNELS_PER_UNIVERSE);

    let mut conflicts = Vec::new();
    if let Some(end) = last_address {
        for (other_universe, other_start, other_end, other_id) in spans(show) {
            if other_universe != universe
                || other_id == id
                || other_start > end
                || other_end < address
            {
                continue;
            }
            conflicts.push(PatchConflict {
                universe,
                from: other_start.max(address),
                to: end.min(other_end),
                first: id.min(other_id),
                second: id.max(other_id),
            });
        }
        conflicts.sort_unstable();
    }

    PatchPreview {
        accepted: refusal.is_none(),
        refusal,
        footprint,
        last_address,
        conflicts,
    }
}

/// Everything wrong with the show, conflicts included.
pub(crate) fn issues(show: &Show) -> Vec<ShowIssue> {
    let mut issues: Vec<ShowIssue> = conflicts(show)
        .into_iter()
        .map(ShowIssue::AddressOverlap)
        .collect();

    for fixture in show.fixtures() {
        if show.fixture_type(&fixture.type_id).is_none() {
            issues.push(ShowIssue::MissingFixtureType {
                fixture: fixture.id,
                type_id: fixture.type_id.clone(),
            });
        }
    }
    for group in show.groups() {
        for fixture in &group.fixtures {
            if show.fixture(*fixture).is_none() {
                issues.push(ShowIssue::GroupMemberUnpatched {
                    group: group.id,
                    fixture: *fixture,
                });
            }
        }
    }
    for preset in show.presets() {
        for value in &preset.values {
            if show.fixture(value.fixture).is_none() {
                issues.push(ShowIssue::PresetValueUnpatched {
                    preset: preset.id,
                    fixture: value.fixture,
                });
            }
        }
    }
    for sequence in show.sequences() {
        for cue in &sequence.cues {
            for part in &cue.parts {
                if show.fixture(part.fixture).is_none() {
                    issues.push(ShowIssue::CuePartUnpatched {
                        sequence: sequence.id,
                        cue: cue.number.clone(),
                        fixture: part.fixture,
                    });
                }
                if let Some(preset) = part.preset_ref
                    && show.preset(preset).is_none()
                {
                    issues.push(ShowIssue::CuePresetMissing {
                        sequence: sequence.id,
                        cue: cue.number.clone(),
                        preset,
                    });
                }
            }
        }
    }
    for executor in show.executors() {
        if let Some(sequence) = executor.sequence_id
            && show.sequence(sequence).is_none()
        {
            issues.push(ShowIssue::ExecutorSequenceMissing {
                executor: executor.id,
                sequence,
            });
        }
    }
    issues
}

/// The universes this show patches that `carried` does not cover — S33.
///
/// A separate function rather than a branch inside [`issues`] because it needs
/// something [`Show`] deliberately does not hold: the output patch belongs to the
/// **building** and lives in [`crate::MachineConfig`] (see [`crate::outputs`]).
/// Folding it in would have meant giving `Show::issues` an argument that is not
/// the show's, which is the seam this whole session is about.
///
/// It takes the universes rather than the configuration so that the caller
/// decides *whose* answer it is: [`crate::MachineConfig::dark_universes`] asks it
/// about the configured rig, and `prismd` asks it about the rig that is actually
/// running — which differ when a row was refused or a thread would not start,
/// and the second is the one an operator needs before a show.
///
/// A **disabled** output carries nothing either way, on purpose:
/// [`crate::MachineConfig::carried_universes`] answers *where the light actually
/// goes*, and a row that has been switched off is not sending.
pub fn dark_universes(show: &Show, carried: &[UniverseId]) -> Vec<ShowIssue> {
    let mut dark = Vec::new();
    for universe in show.universes() {
        if !carried.contains(&universe) && !dark.contains(&universe) {
            dark.push(universe);
        }
    }
    dark.into_iter()
        .map(|universe| ShowIssue::UniverseNotOutput { universe })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{PatchConflict, ShowIssue};
    use crate::Show;
    use crate::testkit::{cue, dimmer_type, executor, fixture, par_type, preset, sequence};
    use prism_domain::{
        AttributeType, ExecutorId, FixtureId, GroupId, PresetId, SequenceId, UniverseId,
    };

    fn patched(addresses: &[(u32, u32, u16)]) -> Show {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        for &(id, universe, address) in addresses {
            show.patch_fixture(fixture(id, "generic.rgbw.par", universe, address))
                .unwrap();
        }
        show
    }

    #[test]
    fn fixtures_that_do_not_touch_do_not_conflict() {
        let show = patched(&[(1, 1, 1), (2, 1, 5), (3, 1, 9)]);
        assert_eq!(show.conflicts(), Vec::new());
        assert_eq!(show.issues(), Vec::new());
    }

    #[test]
    fn a_cloned_fixture_is_reported_rather_than_refused() {
        // Patching a second fixture onto the first is the ordinary way to clone
        // one, so this has to succeed — and has to be visible.
        let show = patched(&[(1, 1, 1), (2, 1, 1)]);
        assert_eq!(
            show.conflicts(),
            vec![PatchConflict {
                universe: UniverseId::new(1),
                from: 1,
                to: 4,
                first: FixtureId::new(1),
                second: FixtureId::new(2),
            }]
        );
        assert_eq!(
            show.conflicts()[0].to_string(),
            "fixtures 1 and 2 share universe 1 channels 1-4; 2 wins"
        );
    }

    #[test]
    fn a_partial_overlap_names_only_the_shared_channels() {
        let show = patched(&[(1, 1, 1), (2, 1, 3)]);
        assert_eq!(
            show.conflicts(),
            vec![PatchConflict {
                universe: UniverseId::new(1),
                from: 3,
                to: 4,
                first: FixtureId::new(1),
                second: FixtureId::new(2),
            }]
        );
    }

    #[test]
    fn the_same_address_in_another_universe_is_not_a_conflict() {
        let show = patched(&[(1, 1, 1), (2, 2, 1)]);
        assert_eq!(show.conflicts(), Vec::new());
    }

    #[test]
    fn three_fixtures_on_one_address_are_three_pairs() {
        let show = patched(&[(1, 1, 1), (2, 1, 1), (3, 1, 1)]);
        let conflicts = show.conflicts();
        assert_eq!(conflicts.len(), 3);
        assert_eq!(
            conflicts
                .iter()
                .map(|conflict| (conflict.first, conflict.second))
                .collect::<Vec<_>>(),
            vec![
                (FixtureId::new(1), FixtureId::new(2)),
                (FixtureId::new(1), FixtureId::new(3)),
                (FixtureId::new(2), FixtureId::new(3)),
            ]
        );
    }

    #[test]
    fn a_narrow_fixture_inside_a_wide_one_is_found() {
        let mut show = patched(&[(1, 1, 1)]);
        show.embed_fixture_type(dimmer_type()).unwrap();
        show.patch_fixture(fixture(2, "generic.dimmer", 1, 3))
            .unwrap();
        assert_eq!(
            show.conflicts(),
            vec![PatchConflict {
                universe: UniverseId::new(1),
                from: 3,
                to: 3,
                first: FixtureId::new(1),
                second: FixtureId::new(2),
            }]
        );
    }

    #[test]
    fn the_conflict_list_is_in_patch_sheet_order() {
        let show = patched(&[(1, 2, 9), (2, 2, 9), (3, 1, 5), (4, 1, 5)]);
        let conflicts = show.conflicts();
        assert_eq!(
            conflicts
                .iter()
                .map(|conflict| (conflict.universe, conflict.from))
                .collect::<Vec<_>>(),
            vec![(UniverseId::new(1), 5), (UniverseId::new(2), 9)]
        );
    }

    #[test]
    fn everything_dangling_is_listed() {
        let mut show = patched(&[(1, 1, 1), (2, 1, 5)]);
        show.store_group(prism_domain::Group {
            id: GroupId::new(1),
            name: "Wash".to_owned(),
            fixtures: vec![FixtureId::new(1), FixtureId::new(2)],
        })
        .unwrap();
        show.store_preset(preset(1, 2, AttributeType::Red, 65535))
            .unwrap();
        let mut linked = cue("1", 2, AttributeType::Red, 65535);
        linked.parts[0].preset_ref = Some(PresetId::new(1));
        show.store_sequence(sequence(1, vec![linked])).unwrap();
        show.store_executor(executor(0, Some(1))).unwrap();

        assert_eq!(show.issues(), Vec::new());

        // Take the rig apart underneath the show.
        show.unpatch_fixture(FixtureId::new(2)).unwrap();
        show.remove_sequence(SequenceId::new(1)).unwrap();

        assert_eq!(
            show.issues(),
            vec![
                ShowIssue::GroupMemberUnpatched {
                    group: GroupId::new(1),
                    fixture: FixtureId::new(2),
                },
                ShowIssue::PresetValueUnpatched {
                    preset: PresetId::new(1),
                    fixture: FixtureId::new(2),
                },
                ShowIssue::ExecutorSequenceMissing {
                    executor: ExecutorId::new(0),
                    sequence: SequenceId::new(1),
                },
            ]
        );
    }

    #[test]
    fn a_cue_that_lost_its_fixture_and_its_preset_says_both() {
        let mut show = patched(&[(1, 1, 1)]);
        show.store_preset(preset(1, 1, AttributeType::Red, 1))
            .unwrap();
        let mut linked = cue("1", 1, AttributeType::Red, 65535);
        linked.parts[0].preset_ref = Some(PresetId::new(1));
        show.store_sequence(sequence(1, vec![linked])).unwrap();
        show.unpatch_fixture(FixtureId::new(1)).unwrap();
        show.remove_preset(PresetId::new(1)).unwrap();

        assert_eq!(
            show.issues(),
            vec![
                ShowIssue::CuePartUnpatched {
                    sequence: SequenceId::new(1),
                    cue: "1".to_owned(),
                    fixture: FixtureId::new(1),
                },
                ShowIssue::CuePresetMissing {
                    sequence: SequenceId::new(1),
                    cue: "1".to_owned(),
                    preset: PresetId::new(1),
                },
            ]
        );
    }

    #[test]
    fn a_fixture_whose_profile_is_gone_is_an_issue_rather_than_a_conflict() {
        let mut show = patched(&[(1, 1, 1)]);
        let mut json = serde_json::to_value(&show).unwrap();
        json["fixtureTypes"] = serde_json::json!({});
        show = serde_json::from_value(json).unwrap();

        assert_eq!(show.conflicts(), Vec::new());
        assert_eq!(
            show.issues(),
            vec![ShowIssue::MissingFixtureType {
                fixture: FixtureId::new(1),
                type_id: "generic.rgbw.par".to_owned(),
            }]
        );
        assert_eq!(
            show.issues()[0].to_string(),
            "fixture 1 needs the profile \"generic.rgbw.par\""
        );
    }

    /* -- the preview: what a patch would do, before it does it ------------ */

    /// The claim S27's exit criterion rests on: the answer a preview gives is
    /// the answer the patch that follows it produces.
    ///
    /// Asserted by *doing* both — the preview, then the command — over four
    /// addresses on the same rig, rather than by comparing two functions.
    #[test]
    fn a_preview_says_what_the_patch_that_follows_it_does() {
        for address in [1, 3, 5, 509] {
            let mut show = patched(&[(1, 1, 1), (2, 1, 5)]);
            let preview = show.preview_patch(
                FixtureId::new(3),
                "generic.rgbw.par",
                UniverseId::new(1),
                address,
            );
            let patched_ok = show
                .patch_fixture(fixture(3, "generic.rgbw.par", 1, address))
                .is_ok();
            assert_eq!(preview.accepted, patched_ok, "address {address}");
            if !patched_ok {
                assert!(preview.refusal.is_some(), "address {address}");
                continue;
            }
            let after: Vec<PatchConflict> = show
                .conflicts()
                .into_iter()
                .filter(|conflict| {
                    conflict.first == FixtureId::new(3) || conflict.second == FixtureId::new(3)
                })
                .collect();
            assert_eq!(preview.conflicts, after, "address {address}");
        }
    }

    #[test]
    fn a_preview_of_a_clean_address_is_accepted_and_empty() {
        let show = patched(&[(1, 1, 1)]);
        let preview =
            show.preview_patch(FixtureId::new(2), "generic.rgbw.par", UniverseId::new(1), 9);
        assert!(preview.accepted);
        assert_eq!(preview.refusal, None);
        assert_eq!(preview.footprint, 4);
        assert_eq!(preview.last_address, Some(12));
        assert_eq!(preview.conflicts, Vec::new());
    }

    /// **An overlap is not a refusal**, and the preview has to say both things
    /// at once — otherwise a patch sheet could not offer to clone a fixture.
    #[test]
    fn a_preview_can_be_accepted_and_still_report_an_overlap() {
        let show = patched(&[(1, 1, 1)]);
        let preview =
            show.preview_patch(FixtureId::new(2), "generic.rgbw.par", UniverseId::new(1), 3);
        assert!(preview.accepted, "cloning a fixture is legal");
        assert_eq!(
            preview.conflicts,
            vec![PatchConflict {
                universe: UniverseId::new(1),
                from: 3,
                to: 4,
                first: FixtureId::new(1),
                second: FixtureId::new(2),
            }]
        );
    }

    /// A fixture does not conflict with itself.
    ///
    /// The case that decides whether an operator can open the row of a patched
    /// fixture, change its name and press Enter without being told it clashes
    /// with something — namely with itself.
    #[test]
    fn repatching_a_fixture_where_it_already_is_reports_nothing() {
        let show = patched(&[(1, 1, 1), (2, 1, 5)]);
        let preview =
            show.preview_patch(FixtureId::new(1), "generic.rgbw.par", UniverseId::new(1), 1);
        assert!(preview.accepted);
        assert_eq!(preview.conflicts, Vec::new());
        // And moving it onto its neighbour still reports the neighbour.
        let moved =
            show.preview_patch(FixtureId::new(1), "generic.rgbw.par", UniverseId::new(1), 6);
        assert_eq!(moved.conflicts.len(), 1);
        assert_eq!(moved.conflicts[0].first, FixtureId::new(1));
        assert_eq!(moved.conflicts[0].second, FixtureId::new(2));
    }

    /// A wide fixture laid over a narrow one that starts before it: the shared
    /// range starts where the *new* fixture does, not where the old one does.
    #[test]
    fn the_shared_range_is_the_intersection_from_either_side() {
        let mut show = patched(&[(1, 1, 1)]);
        show.embed_fixture_type(dimmer_type()).unwrap();
        let preview =
            show.preview_patch(FixtureId::new(9), "generic.dimmer", UniverseId::new(1), 3);
        assert_eq!(
            preview.conflicts,
            vec![PatchConflict {
                universe: UniverseId::new(1),
                from: 3,
                to: 3,
                first: FixtureId::new(1),
                second: FixtureId::new(9),
            }]
        );
    }

    #[test]
    fn a_preview_of_something_that_would_be_refused_says_why_and_lists_nothing() {
        let show = patched(&[(1, 1, 1)]);

        let unknown =
            show.preview_patch(FixtureId::new(2), "nothing.at.all", UniverseId::new(1), 1);
        assert!(!unknown.accepted);
        assert_eq!(unknown.footprint, 0);
        assert_eq!(unknown.last_address, None);
        assert_eq!(unknown.conflicts, Vec::new());
        assert!(
            unknown
                .refusal
                .is_some_and(|why| why.contains("nothing.at.all")),
            "a refusal has to name what is wrong"
        );

        // Past the end of the universe, and outside the universe range: both
        // are refusals with no conflict list, because a fixture that cannot be
        // patched cannot overlap anything.
        let past = show.preview_patch(
            FixtureId::new(2),
            "generic.rgbw.par",
            UniverseId::new(1),
            510,
        );
        assert!(!past.accepted);
        assert_eq!(past.footprint, 4);
        assert_eq!(past.last_address, None);
        assert_eq!(past.conflicts, Vec::new());

        let nowhere = show.preview_patch(
            FixtureId::new(2),
            "generic.rgbw.par",
            UniverseId::new(65),
            1,
        );
        assert!(!nowhere.accepted);
        assert_eq!(nowhere.conflicts, Vec::new());

        let zero = show.preview_patch(FixtureId::new(2), "generic.rgbw.par", UniverseId::new(1), 0);
        assert!(!zero.accepted);
        assert_eq!(zero.last_address, None);
    }

    #[test]
    fn a_preview_never_writes_anything() {
        let mut show = patched(&[(1, 1, 1), (2, 1, 5)]);
        let before = rmp_serde::to_vec_named(&show).unwrap();
        let revision = show.patch_revision();
        let dirty = show.is_dirty();
        for address in [0, 1, 3, 500, 512] {
            let _ = show.preview_patch(
                FixtureId::new(3),
                "generic.rgbw.par",
                UniverseId::new(1),
                address,
            );
        }
        assert_eq!(rmp_serde::to_vec_named(&show).unwrap(), before);
        assert_eq!(show.patch_revision(), revision);
        assert_eq!(show.is_dirty(), dirty);
        // And the borrow is shared, which is what lets a query answer while a
        // command is not being applied.
        let _ = &mut show;
    }

    #[test]
    fn every_issue_says_something_readable() {
        let conflict = PatchConflict {
            universe: UniverseId::new(1),
            from: 1,
            to: 2,
            first: FixtureId::new(1),
            second: FixtureId::new(2),
        };
        for issue in [
            ShowIssue::AddressOverlap(conflict),
            ShowIssue::MissingFixtureType {
                fixture: FixtureId::new(1),
                type_id: "x".to_owned(),
            },
            ShowIssue::GroupMemberUnpatched {
                group: GroupId::new(1),
                fixture: FixtureId::new(1),
            },
            ShowIssue::PresetValueUnpatched {
                preset: PresetId::new(1),
                fixture: FixtureId::new(1),
            },
            ShowIssue::CuePartUnpatched {
                sequence: SequenceId::new(1),
                cue: "1".to_owned(),
                fixture: FixtureId::new(1),
            },
            ShowIssue::CuePresetMissing {
                sequence: SequenceId::new(1),
                cue: "1".to_owned(),
                preset: PresetId::new(1),
            },
            ShowIssue::ExecutorSequenceMissing {
                executor: ExecutorId::new(0),
                sequence: SequenceId::new(1),
            },
            ShowIssue::UniverseNotOutput {
                universe: UniverseId::new(7),
            },
        ] {
            assert!(!issue.to_string().is_empty(), "{issue:?}");
        }
    }

    /// S33: a patched universe nothing carries is **reported**, and reporting it
    /// is the whole of the criterion — it is not an error, nothing is refused,
    /// and nothing is dropped.
    #[test]
    fn a_patched_universe_that_no_output_carries_is_reported() {
        use prism_domain::{Command, OutputId, OutputInstance, OutputKind};

        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        for (id, universe) in [(1u32, 1u32), (2, 7)] {
            show.patch_fixture(prism_domain::Fixture {
                software_dimmer: true,
                id: FixtureId::new(id),
                name: format!("Par {id}"),
                type_id: "generic.rgbw.par".to_owned(),
                universe: UniverseId::new(universe),
                address: 1,
                position: prism_domain::Vec3::ZERO,
                rotation: prism_domain::Vec3::ZERO,
                invert_pan: false,
                invert_tilt: false,
            })
            .unwrap();
        }

        // No rig at all: both universes go nowhere, and both are named.
        let mut machine = crate::MachineConfig::default();
        assert_eq!(
            machine.dark_universes(&show),
            vec![
                ShowIssue::UniverseNotOutput {
                    universe: UniverseId::new(1)
                },
                ShowIssue::UniverseNotOutput {
                    universe: UniverseId::new(7)
                },
            ]
        );

        // One cable for universe 1 leaves exactly one complaint standing.
        machine
            .apply(&Command::AddOutput {
                output: OutputInstance::new(
                    OutputId::new(1),
                    "Hall",
                    OutputKind::Mock,
                    [UniverseId::new(1)],
                ),
            })
            .unwrap();
        assert_eq!(
            machine.dark_universes(&show),
            vec![ShowIssue::UniverseNotOutput {
                universe: UniverseId::new(7)
            }]
        );
        assert!(
            show.issues().is_empty(),
            "and none of it is a fault of the show"
        );

        // A universe an output carries and the patch does not use is *not* an
        // issue: an installer wires a hall before the show is written.
        machine
            .apply(&Command::AddOutput {
                output: OutputInstance::new(
                    OutputId::new(2),
                    "Spare",
                    OutputKind::Mock,
                    [UniverseId::new(7), UniverseId::new(9)],
                ),
            })
            .unwrap();
        assert!(machine.dark_universes(&show).is_empty());

        // Switching that output off puts universe 7 back in the dark, which is
        // what `carried_universes` answering only about enabled rows is for.
        machine
            .apply(&Command::SetOutputEnabled {
                id: OutputId::new(2),
                enabled: false,
            })
            .unwrap();
        assert_eq!(
            machine.dark_universes(&show),
            vec![ShowIssue::UniverseNotOutput {
                universe: UniverseId::new(7)
            }]
        );
    }
}
