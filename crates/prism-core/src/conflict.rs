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

use prism_domain::{ExecutorId, FixtureId, GroupId, PresetId, SequenceId, UniverseId};

use crate::show::Show;

/// Two fixtures sharing DMX channels.
///
/// Ordered by universe and then by the first shared channel, so the list reads
/// like a patch sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl fmt::Display for PatchConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fixtures {} and {} share universe {} channels {}-{}; {} wins",
            self.first, self.second, self.universe, self.from, self.to, self.second
        )
    }
}

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
            Self::ExecutorSequenceMissing { executor, sequence } => write!(
                f,
                "executor {executor} plays sequence {sequence}, which is gone"
            ),
        }
    }
}

/// Every pair of fixtures sharing channels, in patch-sheet order.
pub(crate) fn conflicts(show: &Show) -> Vec<PatchConflict> {
    // (universe, first channel, last channel, fixture). Sorted, so the search
    // for an overlap stops at the first fixture that starts past the end of the
    // one being examined rather than walking the rest of the universe.
    let mut spans: Vec<(UniverseId, u16, u16, FixtureId)> = show
        .patched()
        .filter_map(|(fixture, fixture_type)| {
            fixture
                .last_address(fixture_type.footprint)
                .map(|last| (fixture.universe, fixture.address, last, fixture.id))
        })
        .collect();
    spans.sort_unstable();

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
        ] {
            assert!(!issue.to_string().is_empty(), "{issue:?}");
        }
    }
}
