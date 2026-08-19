//! S11 exit criterion: every command in the show group applies or is rejected,
//! and a rejection leaves the show **byte-identical**.
//!
//! "Byte-identical" is taken literally here: the show is serialised with
//! `rmp_serde::to_vec_named` before and after, and the two `Vec<u8>` are
//! compared. That is a stronger assertion than comparing the model — it would
//! also catch a rejection that reordered a map or normalised a string — and it
//! is the same encoding `prism-ipc` will put on the wire.
//!
//! The two fields that are deliberately *not* serialised are asserted
//! separately, because a rejection that bumped the patch revision or lit the
//! Save LED would be invisible in the bytes and very visible on a console.

mod common;

use common::{
    executor, fixture, group, par_type, patch_command, populated_show, preset, sequence,
    session_commands, show_commands,
};
use prism_core::{Effect, Show, ShowError};
use prism_domain::{
    AttributeType, Command, Delta, ExecutorId, FixtureId, GoDirection, PresetId, SelectionMode,
    SequenceId, UniverseId,
};
use proptest::prelude::*;

/// The show as bytes, plus the two fields that are not part of them.
fn snapshot(show: &Show) -> (Vec<u8>, u64, bool) {
    (
        rmp_serde::to_vec_named(show).unwrap(),
        show.patch_revision(),
        show.is_dirty(),
    )
}

#[test]
fn the_two_groups_together_are_the_whole_protocol() {
    // A new command variant has to be given a home here, or this fails.
    assert_eq!(show_commands().len() + session_commands().len(), 30);
    for command in show_commands() {
        assert!(!command.is_session_command(), "{command:?}");
    }
    for command in session_commands() {
        assert!(command.is_session_command(), "{command:?}");
    }
}

/// `EmbedFixtureType` is the one show command the show model cannot finish on
/// its own, and it is refused one layer up.
///
/// The show has no library — it has no disk either, which is why `SaveShow` is
/// the same shape. What it can say is *a profile is wanted*, and
/// `ShowFile::apply` is where the key is looked up and where a key that names
/// nothing is refused. `src/file.rs` asserts that half, on the file's bytes.
#[test]
fn a_profile_key_is_not_something_the_show_model_can_refuse() {
    let mut show = populated_show();
    let before = snapshot(&show);
    let applied = show
        .apply(&Command::EmbedFixtureType {
            type_id: "nothing.at.all".to_owned(),
        })
        .expect("the show cannot tell: it has no library");
    assert_eq!(applied.effects, vec![Effect::EmbedProfile]);
    assert!(applied.deltas.is_empty());
    assert_eq!(snapshot(&show), before, "and it wrote nothing either");
}

#[test]
fn every_show_command_is_decided_rather_than_ignored() {
    for command in show_commands() {
        let mut show = populated_show();
        let applied = show
            .apply(&command)
            .unwrap_or_else(|error| panic!("{command:?} was refused: {error}"));
        // An empty answer would be a command that was quietly dropped.
        assert!(
            !applied.effects.is_empty(),
            "{command:?} produced no effect"
        );
    }
}

#[test]
fn every_session_command_is_refused_and_changes_nothing() {
    for command in session_commands() {
        let mut show = populated_show();
        let before = snapshot(&show);
        assert_eq!(
            show.apply(&command),
            Err(ShowError::NotAShowCommand),
            "{command:?}"
        );
        assert_eq!(snapshot(&show), before, "{command:?}");
    }
}

#[test]
fn every_rejection_leaves_the_show_byte_identical() {
    // One case per way a show command can be refused. The list is the point:
    // a property test over arbitrary commands would mostly generate nonsense
    // that is rejected for the same one or two reasons.
    let cases: Vec<(Command, ShowError)> = vec![
        (
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1), FixtureId::new(99)],
                mode: SelectionMode::Add,
            },
            ShowError::UnknownFixture(FixtureId::new(99)),
        ),
        (
            Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                value: 70000,
                relative: false,
            },
            ShowError::ValueOutOfRange(70000),
        ),
        (
            Command::ApplyPreset {
                preset_id: PresetId::new(99),
            },
            ShowError::UnknownPreset(PresetId::new(99)),
        ),
        (
            Command::StoreCue {
                sequence_id: SequenceId::new(99),
                cue_number: "1".to_owned(),
            },
            ShowError::UnknownSequence(SequenceId::new(99)),
        ),
        (
            Command::StoreCue {
                sequence_id: SequenceId::new(1),
                cue_number: String::new(),
            },
            ShowError::EmptyCueNumber,
        ),
        (
            Command::ExecutorGo {
                executor_id: ExecutorId::new(99),
                direction: GoDirection::Prev,
            },
            ShowError::UnknownExecutor(ExecutorId::new(99)),
        ),
        (
            Command::ExecutorGo {
                executor_id: ExecutorId::new(1),
                direction: GoDirection::Next,
            },
            ShowError::ExecutorHasNoSequence(ExecutorId::new(1)),
        ),
        (
            Command::ExecutorOff {
                executor_id: ExecutorId::new(1),
            },
            ShowError::ExecutorHasNoSequence(ExecutorId::new(1)),
        ),
        (
            Command::SetExecutorMaster {
                executor_id: ExecutorId::new(99),
                level: 0,
            },
            ShowError::UnknownExecutor(ExecutorId::new(99)),
        ),
        (
            Command::PatchFixture {
                id: FixtureId::new(9),
                name: "Nine".to_owned(),
                type_id: "nothing.at.all".to_owned(),
                universe: UniverseId::new(1),
                address: 1,
            },
            ShowError::UnknownFixtureType("nothing.at.all".to_owned()),
        ),
        (
            patch_command(9, 1, 511),
            ShowError::AddressOutOfRange {
                fixture: FixtureId::new(9),
                address: 511,
                footprint: 4,
            },
        ),
        (
            patch_command(9, 65, 1),
            ShowError::UniverseOutOfRange {
                fixture: FixtureId::new(9),
                universe: UniverseId::new(65),
            },
        ),
        // The three ways an S27 command can be refused.
        (
            Command::UnpatchFixture {
                id: FixtureId::new(99),
            },
            ShowError::UnknownFixture(FixtureId::new(99)),
        ),
        (
            Command::RenumberFixture {
                id: FixtureId::new(1),
                to: FixtureId::new(2),
            },
            ShowError::FixtureNumberInUse(FixtureId::new(2)),
        ),
        (
            Command::RenumberFixture {
                id: FixtureId::new(99),
                to: FixtureId::new(100),
            },
            ShowError::UnknownFixture(FixtureId::new(99)),
        ),
    ];

    for (command, expected) in cases {
        let mut show = populated_show();
        // Dirty and undirty alike: the flag must not move either way.
        for dirty in [false, true] {
            if dirty {
                show.apply(&patch_command(8, 3, 1)).unwrap();
            }
            let before = snapshot(&show);
            assert_eq!(show.apply(&command), Err(expected.clone()), "{command:?}");
            assert_eq!(snapshot(&show), before, "{command:?}");
        }
    }
}

#[test]
fn a_direct_edit_that_is_refused_changes_nothing_either() {
    // The same promise one level down, where S13, S14 and S27 will call in.
    let mut show = populated_show();
    let before = snapshot(&show);

    let mut broken = par_type();
    broken.footprint = 1;
    show.embed_fixture_type(broken).unwrap_err();
    show.remove_fixture_type("generic.rgbw.par").unwrap_err();
    show.patch_fixture(fixture(9, "nothing", 1, 1)).unwrap_err();
    show.unpatch_fixture(FixtureId::new(99)).unwrap_err();
    show.store_group(group(2, &[99])).unwrap_err();
    show.remove_group(prism_domain::GroupId::new(99))
        .unwrap_err();
    show.store_preset(preset(5, 99, AttributeType::Red, 0))
        .unwrap_err();
    show.remove_preset(PresetId::new(99)).unwrap_err();
    show.store_sequence(sequence(
        2,
        vec![common::cue("1", 99, AttributeType::Red, 0)],
    ))
    .unwrap_err();
    show.store_sequence(sequence(
        2,
        vec![
            common::cue("1", 1, AttributeType::Red, 0),
            common::cue("1", 2, AttributeType::Red, 0),
        ],
    ))
    .unwrap_err();
    show.remove_sequence(SequenceId::new(99)).unwrap_err();
    show.store_cue(
        SequenceId::new(99),
        common::cue("1", 1, AttributeType::Red, 0),
    )
    .unwrap_err();
    show.store_cue(
        SequenceId::new(1),
        common::cue("", 1, AttributeType::Red, 0),
    )
    .unwrap_err();
    show.store_executor(executor(2, Some(99))).unwrap_err();
    show.set_executor_master(ExecutorId::new(99), 0)
        .unwrap_err();
    show.record_executor_state(ExecutorId::new(99), true, None)
        .unwrap_err();

    assert_eq!(snapshot(&show), before);
}

#[test]
fn a_cue_part_linked_to_a_preset_that_is_gone_is_refused() {
    let mut show = populated_show();
    let mut linked = common::cue("5", 1, AttributeType::Red, 0);
    linked.parts[0].preset_ref = Some(PresetId::new(99));
    let before = snapshot(&show);
    assert_eq!(
        show.store_cue(SequenceId::new(1), linked),
        Err(ShowError::UnknownPreset(PresetId::new(99)))
    );
    assert_eq!(snapshot(&show), before);
}

#[test]
fn an_applied_command_reports_what_the_daemon_still_has_to_do() {
    let mut show = populated_show();
    assert_eq!(
        show.apply(&patch_command(9, 3, 1)).unwrap().effects,
        vec![Effect::Repatch]
    );
    // The patch moved, so everything derived from it is stale: two profiles
    // embedded and four fixtures patched by `populated_show`, then this one.
    assert_eq!(show.patch_revision(), 7);
    assert!(show.is_dirty());
    assert!(show.apply(&Command::SaveShow).unwrap().deltas.is_empty());
    assert!(show.mark_saved());
}

proptest! {
    /// The broad net: whatever arrives, a refusal costs the show nothing.
    #[test]
    fn a_rejected_command_never_changes_the_show(command in any::<Command>()) {
        let mut show = populated_show();
        let before = snapshot(&show);
        if show.apply(&command).is_err() {
            prop_assert_eq!(snapshot(&show), before);
        }
    }

    /// And an accepted one only ever changes it in ways it announced: a command
    /// that produced no `ShowPatch` may not have touched the show.
    #[test]
    fn a_command_that_reports_no_change_made_none(command in any::<Command>()) {
        let mut show = populated_show();
        let before = rmp_serde::to_vec_named(&show).unwrap();
        if let Ok(applied) = show.apply(&command) {
            let announced = applied
                .deltas
                .iter()
                .any(|delta| matches!(delta, Delta::ShowPatch { .. }));
            if !announced {
                prop_assert_eq!(rmp_serde::to_vec_named(&show).unwrap(), before);
            }
        }
    }
}
