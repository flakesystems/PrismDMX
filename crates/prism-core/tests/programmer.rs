//! S13 exit criteria: the three-stage Clear through **all** its transitions
//! including the reset-to-0 rule, `ApplyPreset` recording `presetRef` so cues
//! stay live-updatable, and the programmer being **sparse** — an untouched
//! attribute absent rather than zero.
//!
//! "Byte-identical" is taken literally, as in S11 and S12: the programmer is
//! serialised with `rmp_serde::to_vec_named` before and after a refusal and the
//! two `Vec<u8>` are compared.

mod common;

use common::{par_type, populated_show, preset, show_commands};
use prism_core::{Programmer, ProgrammerError, ShowFile};
use prism_domain::{
    AttributeType, ClearStage, Command, Delta, FeatureGroup, FixtureId, PresetId,
    ProgrammerValueSource, SelectionMode, SequenceId,
};
use proptest::prelude::*;

/// A show file with everything patched, saved, and nothing programmed yet.
fn file() -> ShowFile {
    let mut file = ShowFile::new();
    file.show = populated_show();
    file
}

/// `SelectFixtures` for the fixture numbers given.
fn select(ids: &[u32], mode: SelectionMode) -> Command {
    Command::SelectFixtures {
        ids: ids.iter().copied().map(FixtureId::new).collect(),
        mode,
    }
}

/// `SetAttribute`, absolute.
fn set(attribute: AttributeType, value: i32) -> Command {
    Command::SetAttribute {
        attribute,
        value,
        relative: false,
    }
}

/// The programmer as bytes: what "byte-identical" is asserted on.
fn snapshot(file: &ShowFile) -> Vec<u8> {
    rmp_serde::to_vec_named(file.programmer.state()).unwrap()
}

// -- the group ------------------------------------------------------------

#[test]
fn the_six_programmer_commands_are_the_ones_the_show_hands_on() {
    // S11 decided the split by answering `Effect::Programmer`; this session is
    // the other half of exactly those five. Pinning the two lists together
    // means a sixth command cannot be given to one and not the other.
    let programmer_commands = [
        select(&[1], SelectionMode::Set),
        set(AttributeType::Red, 65535),
        Command::ApplyPreset {
            preset_id: PresetId::new(4),
        },
        Command::ClearProgrammer,
        Command::StoreCue {
            sequence_id: SequenceId::new(1),
            cue_number: "3".to_owned(),
        },
        // S28's sixth, and it is the mirror of `StoreCue`: the show can say
        // which pool and which number, and only the programmer knows what would
        // go in it.
        Command::StorePreset {
            preset_id: PresetId::new(4),
            pool: FeatureGroup::Color,
            name: "Deep blue".to_owned(),
            color: None,
        },
    ];

    for command in show_commands() {
        let mut show = populated_show();
        let hands_on = show
            .apply(&command)
            .unwrap()
            .effects
            .contains(&prism_core::Effect::Programmer);
        assert_eq!(
            hands_on,
            programmer_commands.contains(&command),
            "{command:?}"
        );
    }
    assert_eq!(programmer_commands.len(), 6);
}

#[test]
fn a_command_that_is_not_the_programmers_is_refused_here() {
    let show = populated_show();
    let mut programmer = Programmer::new();
    for command in show_commands()
        .into_iter()
        .chain(common::session_commands())
    {
        if matches!(
            command,
            Command::SelectFixtures { .. }
                | Command::SetAttribute { .. }
                | Command::ApplyPreset { .. }
                | Command::ClearProgrammer
                | Command::StoreCue { .. }
                | Command::StorePreset { .. }
        ) {
            continue;
        }
        assert_eq!(
            programmer.apply(&command, &show),
            Err(ProgrammerError::NotAProgrammerCommand),
            "{command:?}"
        );
    }
}

// -- sparse ---------------------------------------------------------------

#[test]
fn selecting_fixtures_creates_no_values_at_all() {
    // The whole point of the layer: an absent attribute means "the playbacks
    // decide". A programmer that laid down zeros on selection would black the
    // stage out the moment an operator touched a fixture number.
    let mut file = file();
    file.apply(&select(&[1, 2, 3], SelectionMode::Set)).unwrap();
    assert_eq!(file.programmer.state().selection.len(), 3);
    assert!(
        file.programmer.state().values.is_empty(),
        "selecting wrote values"
    );
}

#[test]
fn an_untouched_attribute_is_absent_rather_than_zero() {
    let mut file = file();
    file.apply(&select(&[1, 2], SelectionMode::Set)).unwrap();
    file.apply(&set(AttributeType::Red, 65535)).unwrap();

    let state = file.programmer.state();
    // Touched.
    for id in [1, 2] {
        assert_eq!(
            state
                .value(FixtureId::new(id), AttributeType::Red)
                .map(|value| value.value),
            Some(65535)
        );
    }
    // Untouched, on a fixture that *is* selected and *does* have the attribute.
    assert!(
        state
            .value(FixtureId::new(1), AttributeType::Green)
            .is_none(),
        "green was written as zero"
    );
    // Untouched, on a fixture that is not selected.
    assert!(state.value(FixtureId::new(3), AttributeType::Red).is_none());
    // And nothing else exists anywhere: two fixtures, one attribute each.
    assert_eq!(state.values.len(), 2);
    assert!(
        state
            .values
            .values()
            .all(|attributes| attributes.len() == 1)
    );
}

#[test]
fn an_attribute_the_fixture_does_not_have_is_not_written() {
    // A PAR has no pan. A programmer value for it would name a merge slot that
    // does not exist, which S6 already has to drop on the way into the engine.
    let mut file = file();
    file.apply(&select(&[1, 4], SelectionMode::Set)).unwrap();
    file.apply(&set(AttributeType::Dimmer, 30000)).unwrap();

    let state = file.programmer.state();
    assert!(
        state
            .value(FixtureId::new(1), AttributeType::Dimmer)
            .is_none()
    );
    assert_eq!(
        state
            .value(FixtureId::new(4), AttributeType::Dimmer)
            .map(|value| value.value),
        Some(30000)
    );
}

#[test]
fn setting_an_attribute_with_nothing_selected_changes_nothing() {
    let mut file = file();
    let applied = file.apply(&set(AttributeType::Red, 65535)).unwrap();
    assert!(applied.deltas.is_empty());
    assert!(file.programmer.state().is_empty());
}

// -- selection modes ------------------------------------------------------

#[test]
fn set_add_and_toggle_are_three_different_things() {
    let mut file = file();
    file.apply(&select(&[1, 2], SelectionMode::Set)).unwrap();
    assert_eq!(
        file.programmer.state().selection,
        vec![FixtureId::new(1), FixtureId::new(2)]
    );

    file.apply(&select(&[3], SelectionMode::Add)).unwrap();
    assert_eq!(
        file.programmer.state().selection,
        vec![FixtureId::new(1), FixtureId::new(2), FixtureId::new(3)]
    );
    // Adding one that is already there does not select it twice.
    file.apply(&select(&[3], SelectionMode::Add)).unwrap();
    assert_eq!(file.programmer.state().selection.len(), 3);

    file.apply(&select(&[2, 4], SelectionMode::Toggle)).unwrap();
    assert_eq!(
        file.programmer.state().selection,
        vec![FixtureId::new(1), FixtureId::new(3), FixtureId::new(4)],
        "toggle removes what is selected and appends what is not"
    );

    file.apply(&select(&[1], SelectionMode::Set)).unwrap();
    assert_eq!(file.programmer.state().selection, vec![FixtureId::new(1)]);
}

#[test]
fn a_selection_that_names_an_unpatched_fixture_is_refused_whole() {
    let mut file = file();
    file.apply(&select(&[1], SelectionMode::Set)).unwrap();
    let before = snapshot(&file);
    assert!(file.apply(&select(&[2, 99], SelectionMode::Add)).is_err());
    assert_eq!(snapshot(&file), before, "fixture 2 was selected anyway");
}

// -- the three-stage clear ------------------------------------------------

#[test]
fn the_three_stage_clear_runs_through_every_transition() {
    // docs/DMX_MERGE.md §3.1, taken one row at a time and then round again.
    let mut file = file();
    file.apply(&select(&[1, 2], SelectionMode::Set)).unwrap();
    file.apply(&set(AttributeType::Red, 65535)).unwrap();
    file.apply(&Command::SetEncoderBank {
        group: FeatureGroup::Color,
    })
    .unwrap();
    file.session.set_programmer_page(3).unwrap();
    file.session.set_programmer_param_index(2).unwrap();
    assert_eq!(file.programmer.state().clear_stage, ClearStage::Idle);

    // 0 → 1: values go, the selection stays.
    file.apply(&Command::ClearProgrammer).unwrap();
    assert_eq!(
        file.programmer.state().clear_stage,
        ClearStage::ValuesCleared
    );
    assert!(file.programmer.state().values.is_empty());
    assert_eq!(file.programmer.state().selection.len(), 2);
    assert_eq!(
        file.programmer.state().active_feature_group,
        FeatureGroup::Color
    );

    // 1 → 2: the selection goes.
    file.apply(&Command::ClearProgrammer).unwrap();
    assert_eq!(
        file.programmer.state().clear_stage,
        ClearStage::SelectionCleared
    );
    assert!(file.programmer.state().selection.is_empty());
    assert_eq!(
        file.programmer.state().active_feature_group,
        FeatureGroup::Color,
        "the feature group survives until the third press"
    );

    // 2 → 0: everything, including the feature group and the page state.
    file.apply(&Command::ClearProgrammer).unwrap();
    assert_eq!(file.programmer.state().clear_stage, ClearStage::Idle);
    assert_eq!(
        file.programmer.state().active_feature_group,
        FeatureGroup::default()
    );
    assert!(file.programmer.state().is_empty());
    assert_eq!(file.session.session().programmer_page, 0);
    assert_eq!(file.session.session().programmer_param_index, 0);

    // And round again: the machine is a cycle, not a ladder with an end.
    file.apply(&Command::ClearProgrammer).unwrap();
    assert_eq!(
        file.programmer.state().clear_stage,
        ClearStage::ValuesCleared
    );
}

#[test]
fn any_other_programmer_interaction_puts_the_clear_stage_back_to_zero() {
    // The rule exists so an operator who clears once and then grabs a fader
    // does not find a later Clear press in an unexpected stage. It is asserted
    // from **both** non-zero stages, for each of the interactions that can
    // reach them — see the test below for the one that cannot.
    let interactions: Vec<Command> = vec![
        select(&[1], SelectionMode::Set),
        select(&[2], SelectionMode::Toggle),
        set(AttributeType::Red, 100),
        Command::ApplyPreset {
            preset_id: PresetId::new(4),
        },
    ];

    for command in interactions {
        for stage in [ClearStage::ValuesCleared, ClearStage::SelectionCleared] {
            let mut file = file();
            file.apply(&select(&[1], SelectionMode::Set)).unwrap();
            file.apply(&set(AttributeType::Blue, 4096)).unwrap();
            file.apply(&Command::ClearProgrammer).unwrap();
            if stage == ClearStage::SelectionCleared {
                file.apply(&Command::ClearProgrammer).unwrap();
            }
            assert_eq!(file.programmer.state().clear_stage, stage);

            file.apply(&command)
                .unwrap_or_else(|error| panic!("{command:?}: {error}"));
            assert_eq!(
                file.programmer.state().clear_stage,
                ClearStage::Idle,
                "{command:?} from {stage:?}"
            );
        }
    }
}

#[test]
fn a_store_can_never_meet_a_non_zero_clear_stage() {
    // The fifth interaction is the exception that proves the rule rather than
    // an untested case: the only way to a non-zero stage is a Clear, and the
    // first Clear has already emptied the values — so a store from stage 1 or
    // 2 has nothing to store and is refused before the stage is reached at all.
    let mut file = file();
    file.apply(&select(&[1], SelectionMode::Set)).unwrap();
    file.apply(&set(AttributeType::Blue, 4096)).unwrap();
    file.apply(&Command::ClearProgrammer).unwrap();

    let before = snapshot(&file);
    assert!(
        file.apply(&Command::StoreCue {
            sequence_id: SequenceId::new(1),
            cue_number: "3".to_owned(),
        })
        .is_err()
    );
    assert_eq!(snapshot(&file), before);
    assert_eq!(
        file.programmer.state().clear_stage,
        ClearStage::ValuesCleared,
        "a refused store must not move the stage either"
    );
}

#[test]
fn clearing_an_empty_programmer_still_advances_the_stage() {
    // The stage belongs to the button, not to the contents.
    let mut file = file();
    for expected in [
        ClearStage::ValuesCleared,
        ClearStage::SelectionCleared,
        ClearStage::Idle,
    ] {
        let applied = file.apply(&Command::ClearProgrammer).unwrap();
        assert_eq!(file.programmer.state().clear_stage, expected);
        assert!(
            applied
                .deltas
                .iter()
                .any(|delta| matches!(delta, Delta::ProgrammerChanged { .. })),
            "the stage moved and nobody was told"
        );
    }
}

// -- presets --------------------------------------------------------------

#[test]
fn applying_a_preset_records_the_preset_reference() {
    let mut file = file();
    file.apply(&select(&[1], SelectionMode::Set)).unwrap();
    file.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    })
    .unwrap();

    let value = file
        .programmer
        .state()
        .value(FixtureId::new(1), AttributeType::Red)
        .expect("the preset was not applied");
    assert_eq!(value.value, 65535);
    assert_eq!(value.source, ProgrammerValueSource::Preset);
    assert_eq!(value.preset_ref, Some(PresetId::new(4)));
}

#[test]
fn a_preset_reaches_the_cue_with_its_link_intact() {
    // The exit criterion: "cues stay live-updatable". The link is what makes a
    // later edit of the preset move the cue with it, so it has to survive the
    // store.
    let mut file = file();
    file.apply(&select(&[1], SelectionMode::Set)).unwrap();
    file.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    })
    .unwrap();
    file.apply(&Command::StoreCue {
        sequence_id: SequenceId::new(1),
        cue_number: "3".to_owned(),
    })
    .unwrap();

    let cue = file
        .show
        .sequence(SequenceId::new(1))
        .unwrap()
        .cues
        .iter()
        .find(|cue| cue.number == "3")
        .expect("cue 3 was not stored");
    let part = &cue.parts[0];
    assert_eq!(part.fixture, FixtureId::new(1));
    assert_eq!(part.attribute, AttributeType::Red);
    assert_eq!(part.value, 65535);
    assert_eq!(part.preset_ref, Some(PresetId::new(4)));
}

#[test]
fn a_manual_change_on_top_of_a_preset_breaks_the_link() {
    // It is no longer the preset's value, so a later edit of the preset must
    // not move it. Keeping the reference would make the cue follow a preset it
    // no longer agrees with.
    let mut file = file();
    file.apply(&select(&[1], SelectionMode::Set)).unwrap();
    file.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    })
    .unwrap();
    file.apply(&set(AttributeType::Red, 100)).unwrap();

    let value = file
        .programmer
        .state()
        .value(FixtureId::new(1), AttributeType::Red)
        .unwrap();
    assert_eq!(value.source, ProgrammerValueSource::Manual);
    assert_eq!(value.preset_ref, None);
}

#[test]
fn a_preset_only_reaches_the_fixtures_that_are_selected() {
    // `ApplyPreset` is "apply a preset to the current selection"
    // (ARCHITECTURE_SPEC.md §6 and docs/IPC_PROTOCOL.md §5).
    let mut file = file();
    file.show
        .store_preset(preset(5, 2, AttributeType::Green, 30000))
        .unwrap();
    // Preset 5 names fixture 2; fixture 3 is selected.
    file.apply(&select(&[3], SelectionMode::Set)).unwrap();
    let applied = file
        .apply(&Command::ApplyPreset {
            preset_id: PresetId::new(5),
        })
        .unwrap();
    assert!(applied.deltas.is_empty());
    assert!(file.programmer.state().values.is_empty());
}

// -- storing --------------------------------------------------------------

#[test]
fn storing_merges_into_the_cue_that_is_already_there() {
    // The programmer is sparse, so a store carries only what was touched.
    // Overwriting would delete every value the operator did not happen to
    // touch this time round.
    let mut file = file();
    file.apply(&select(&[3], SelectionMode::Set)).unwrap();
    file.apply(&set(AttributeType::Blue, 111)).unwrap();
    file.apply(&Command::StoreCue {
        sequence_id: SequenceId::new(1),
        cue_number: "1".to_owned(),
    })
    .unwrap();

    let cue = file
        .show
        .sequence(SequenceId::new(1))
        .unwrap()
        .cues
        .iter()
        .find(|cue| cue.number == "1")
        .unwrap();
    // Cue 1 held fixture 1's red; the store added fixture 3's blue and left
    // the cue's name and times where they were.
    assert_eq!(cue.parts.len(), 2);
    assert_eq!(cue.name, "Cue 1");
    assert!((cue.fade_in - 3.0).abs() < f64::EPSILON);
    assert!(
        cue.parts
            .iter()
            .any(|part| part.fixture == FixtureId::new(1) && part.value == 65535)
    );
    assert!(
        cue.parts
            .iter()
            .any(|part| part.fixture == FixtureId::new(3) && part.value == 111)
    );
}

#[test]
fn storing_an_empty_programmer_is_refused_rather_than_storing_nothing() {
    let mut file = file();
    assert!(
        file.apply(&Command::StoreCue {
            sequence_id: SequenceId::new(1),
            cue_number: "9".to_owned(),
        })
        .is_err()
    );
    assert!(
        file.show
            .sequence(SequenceId::new(1))
            .unwrap()
            .cues
            .iter()
            .all(|cue| cue.number != "9")
    );
}

#[test]
fn values_naming_a_fixture_that_has_been_unpatched_are_dropped_and_reported() {
    // S6 dropped them on the way into the engine and asked S13 to surface it:
    // a value that does nothing, dropped silently, is something an operator
    // cannot diagnose.
    let mut file = file();
    file.apply(&select(&[1, 3], SelectionMode::Set)).unwrap();
    file.apply(&set(AttributeType::Red, 65535)).unwrap();
    file.show.unpatch_fixture(FixtureId::new(1)).unwrap();

    let applied = file
        .apply(&Command::StoreCue {
            sequence_id: SequenceId::new(1),
            cue_number: "4".to_owned(),
        })
        .unwrap();
    assert!(
        applied
            .deltas
            .iter()
            .any(|delta| matches!(delta, Delta::Notice { .. })),
        "nothing was said about the dropped value"
    );

    let cue = file
        .show
        .sequence(SequenceId::new(1))
        .unwrap()
        .cues
        .iter()
        .find(|cue| cue.number == "4")
        .unwrap();
    assert_eq!(cue.parts.len(), 1);
    assert_eq!(cue.parts[0].fixture, FixtureId::new(3));
}

// -- the session side -----------------------------------------------------

#[test]
fn a_change_of_selection_moves_the_jog_wheel_back_to_the_first_parameter() {
    // S12 requirement: the parameter index is session state, and a wheel left
    // pointing at a parameter the new selection does not have is a console
    // that lies.
    let mut file = file();
    file.apply(&select(&[1], SelectionMode::Set)).unwrap();
    file.session.set_programmer_param_index(4).unwrap();

    let applied = file.apply(&select(&[2], SelectionMode::Set)).unwrap();
    assert_eq!(file.session.session().programmer_param_index, 0);
    assert!(
        applied
            .deltas
            .iter()
            .any(|delta| matches!(delta, Delta::SessionPatch { .. })),
        "the wheel moved and no client was told"
    );

    // A selection command that changes nothing leaves the wheel alone.
    file.session.set_programmer_param_index(4).unwrap();
    file.apply(&select(&[2], SelectionMode::Set)).unwrap();
    assert_eq!(file.session.session().programmer_param_index, 4);
}

// -- rejections -----------------------------------------------------------

#[test]
fn every_rejection_leaves_the_programmer_byte_identical() {
    let cases: Vec<Command> = vec![
        select(&[99], SelectionMode::Set),
        Command::ApplyPreset {
            preset_id: PresetId::new(99),
        },
        Command::StoreCue {
            sequence_id: SequenceId::new(99),
            cue_number: "1".to_owned(),
        },
        Command::StoreCue {
            sequence_id: SequenceId::new(1),
            cue_number: "  ".to_owned(),
        },
        Command::SetAttribute {
            attribute: AttributeType::Red,
            value: 65536,
            relative: false,
        },
    ];

    for command in cases {
        let mut file = file();
        file.apply(&select(&[1], SelectionMode::Set)).unwrap();
        file.apply(&set(AttributeType::Red, 4096)).unwrap();
        file.apply(&Command::ClearProgrammer).unwrap();
        let before = snapshot(&file);
        let show_before = rmp_serde::to_vec_named(&file.show).unwrap();

        assert!(file.apply(&command).is_err(), "{command:?}");
        assert_eq!(snapshot(&file), before, "{command:?}");
        assert_eq!(
            rmp_serde::to_vec_named(&file.show).unwrap(),
            show_before,
            "{command:?}"
        );
    }
}

#[test]
fn the_programmer_is_not_part_of_the_show_file() {
    // What is in the programmer has not been stored yet, by definition. A file
    // that carried it would restore an absolute override of every playback on
    // load — and it would change the file's bytes without the Save LED moving,
    // because setting a value is not an edit to the show.
    let mut file = file();
    let empty = rmp_serde::to_vec_named(&file).unwrap();
    file.apply(&select(&[1, 2], SelectionMode::Set)).unwrap();
    file.apply(&set(AttributeType::Red, 65535)).unwrap();
    assert!(!file.programmer.state().is_empty());
    assert!(!file.is_dirty(), "programming is not an unsaved edit");
    assert_eq!(rmp_serde::to_vec_named(&file).unwrap(), empty);

    let reopened: ShowFile = rmp_serde::from_slice(&empty).unwrap();
    assert!(reopened.programmer.state().is_empty());
}

/// An arbitrary programmer command against the populated show.
fn any_programmer_command() -> impl Strategy<Value = Command> {
    prop_oneof![
        (
            proptest::collection::vec(1u32..=5, 0..4),
            any::<SelectionMode>()
        )
            .prop_map(|(ids, mode)| Command::SelectFixtures {
                ids: ids.into_iter().map(FixtureId::new).collect(),
                mode,
            }),
        (any::<AttributeType>(), -70000i32..70000, any::<bool>()).prop_map(
            |(attribute, value, relative)| Command::SetAttribute {
                attribute,
                value,
                relative,
            }
        ),
        (1u32..=5).prop_map(|id| Command::ApplyPreset {
            preset_id: PresetId::new(id)
        }),
        Just(Command::ClearProgrammer),
        (1u32..=2, "[0-9]").prop_map(|(id, number)| Command::StoreCue {
            sequence_id: SequenceId::new(id),
            cue_number: number,
        }),
    ]
}

proptest! {
    #[test]
    fn the_invariants_hold_however_the_programmer_is_driven(
        commands in proptest::collection::vec(any_programmer_command(), 0..24)
    ) {
        let mut file = file();
        for command in &commands {
            // A refusal is an ordinary answer here — fixture 5 is not patched,
            // preset 3 does not exist — and it must leave the state alone.
            let before = snapshot(&file);
            if file.apply(command).is_err() {
                prop_assert_eq!(snapshot(&file), before);
            }

            let state = file.programmer.state();
            // The S1 invariant: no fixture maps to an empty attribute map, or
            // the state would carry something with no representation on the
            // wire.
            prop_assert!(state.values.values().all(|attributes| !attributes.is_empty()));
            // Nothing is ever selected twice, and everything selected is
            // patched.
            for (index, id) in state.selection.iter().enumerate() {
                prop_assert!(!state.selection[..index].contains(id));
                prop_assert!(file.show.fixture(*id).is_some());
            }
            // And every value the programmer holds is one the show can resolve:
            // sparse means touched, and only a touched attribute of a patched
            // fixture can be touched.
            prop_assert!(file.programmer.unresolved(&file.show).is_empty());
        }
    }
}

#[test]
fn the_feature_groups_the_programmer_holds_are_the_ones_it_was_given() {
    let mut file = file();
    assert!(file.programmer.feature_groups(&file.show).is_empty());

    file.apply(&select(&[1, 4], SelectionMode::Set)).unwrap();
    file.apply(&set(AttributeType::Red, 65535)).unwrap();
    assert_eq!(
        file.programmer.feature_groups(&file.show),
        vec![FeatureGroup::Color]
    );
    assert_eq!(
        file.programmer.state().active_feature_group,
        FeatureGroup::Color
    );

    file.apply(&set(AttributeType::Dimmer, 100)).unwrap();
    assert_eq!(
        file.programmer.feature_groups(&file.show),
        vec![FeatureGroup::Dimmer, FeatureGroup::Color],
        "in encoder-bank order, whatever order they were touched in"
    );
    assert_eq!(
        file.programmer.state().active_feature_group,
        FeatureGroup::Dimmer
    );
}

#[test]
fn a_relative_move_starts_from_home_and_saturates() {
    let mut file = file();
    file.show
        .embed_fixture_type({
            let mut fixture_type = par_type();
            fixture_type.attributes[0].default_value = 1000;
            fixture_type
        })
        .unwrap();
    file.apply(&select(&[1], SelectionMode::Set)).unwrap();

    // Nothing touched yet: the move starts at the attribute's home value.
    file.apply(&Command::SetAttribute {
        attribute: AttributeType::Red,
        value: -400,
        relative: true,
    })
    .unwrap();
    assert_eq!(
        file.programmer
            .state()
            .value(FixtureId::new(1), AttributeType::Red)
            .map(|value| value.value),
        Some(600)
    );

    // And from there on, from the value the programmer is holding.
    file.apply(&Command::SetAttribute {
        attribute: AttributeType::Red,
        value: 400,
        relative: true,
    })
    .unwrap();
    assert_eq!(
        file.programmer
            .state()
            .value(FixtureId::new(1), AttributeType::Red)
            .map(|value| value.value),
        Some(1000)
    );

    // A wheel spun past either end saturates rather than wrapping.
    for (delta, expected) in [(-70000, 0), (70000, 65535)] {
        file.apply(&Command::SetAttribute {
            attribute: AttributeType::Red,
            value: delta,
            relative: true,
        })
        .unwrap();
        assert_eq!(
            file.programmer
                .state()
                .value(FixtureId::new(1), AttributeType::Red)
                .map(|value| value.value),
            Some(expected)
        );
    }
}
