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
    assert_eq!(show_commands().len() + session_commands().len(), 36);
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

// -------------------------------------------------- S34: the executor decides

/// A show with one executor whose four buttons and fader are set by the caller.
fn desk_with(
    buttons: Vec<prism_domain::ExecutorButtonFunction>,
    fader: prism_domain::ExecutorFaderFunction,
) -> Show {
    let mut show = populated_show();
    let mut slot = executor(0, Some(1));
    slot.button_functions = buttons;
    slot.fader_function = fader;
    show.store_executor(slot).unwrap();
    show.mark_saved();
    show
}

fn press(show: &mut Show, index: u8, pressed: bool) -> Vec<Effect> {
    show.apply(&Command::ExecutorButton {
        executor_id: ExecutorId::new(0),
        button: prism_domain::ExecutorButtonRef::Slot { index },
        pressed,
    })
    .expect("the executor plays a sequence")
    .effects
}

/// **The executor decides what a press means, and the eight functions are the
/// eight answers.**
///
/// The expectations are written out by hand rather than derived from the enum,
/// for `prism-surface`'s reason (S19): a table that read the mapping it is
/// checking would pass for any mapping, including one where Off and On had been
/// swapped.
#[test]
fn every_button_function_resolves_to_the_effect_its_name_says() {
    use prism_domain::ExecutorButtonFunction as Fn;
    let executor = ExecutorId::new(0);
    for (function, expected) in [
        (Fn::Empty, Vec::new()),
        (
            Fn::GoForward,
            vec![Effect::ExecutorGo {
                executor,
                direction: GoDirection::Next,
            }],
        ),
        (
            Fn::GoBack,
            vec![Effect::ExecutorGo {
                executor,
                direction: GoDirection::Prev,
            }],
        ),
        (Fn::On, vec![Effect::ExecutorOn { executor }]),
        (Fn::Off, vec![Effect::ExecutorOff { executor }]),
        (
            Fn::Flash,
            vec![Effect::ExecutorFlash { executor, on: true }],
        ),
        (Fn::LearnSpeed, vec![Effect::ExecutorTapSpeed { executor }]),
        // Not running, so a toggle starts it. The other half is below.
        (Fn::Toggle, vec![Effect::ExecutorOn { executor }]),
    ] {
        let mut show = desk_with(vec![function], prism_domain::ExecutorFaderFunction::Master);
        assert_eq!(press(&mut show, 0, true), expected, "{function:?}");
    }
}

/// **`Toggle` is resolved against `is_active`, here and nowhere else.**
///
/// The state a toggle reads is the one the *engine* reported through
/// `Show::record_executor_state`, so a second client that started the executor
/// is a second client this one has already been told about. A client resolving
/// the toggle for itself would send a start, because it had never pressed
/// anything.
#[test]
fn a_toggle_reads_the_state_the_daemon_holds() {
    use prism_domain::ExecutorButtonFunction as Fn;
    let executor = ExecutorId::new(0);
    let mut show = desk_with(
        vec![Fn::Toggle],
        prism_domain::ExecutorFaderFunction::Master,
    );
    assert_eq!(
        press(&mut show, 0, true),
        vec![Effect::ExecutorOn { executor }]
    );

    // Somebody — a tick readback, which is the only author — says it is running.
    show.record_executor_state(executor, true, Some(0)).unwrap();
    assert_eq!(
        press(&mut show, 0, true),
        vec![Effect::ExecutorOff { executor }]
    );

    show.record_executor_state(executor, false, None).unwrap();
    assert_eq!(
        press(&mut show, 0, true),
        vec![Effect::ExecutorOn { executor }]
    );
}

/// Only `Flash` hears a release, and a key with nothing on it hears neither.
#[test]
fn a_release_is_half_a_flash_and_nothing_at_all_to_anything_else() {
    use prism_domain::ExecutorButtonFunction as Fn;
    let executor = ExecutorId::new(0);
    let mut show = desk_with(
        vec![Fn::Flash, Fn::GoForward, Fn::Toggle, Fn::Empty],
        prism_domain::ExecutorFaderFunction::Master,
    );
    assert_eq!(
        press(&mut show, 0, false),
        vec![Effect::ExecutorFlash {
            executor,
            on: false,
        }]
    );
    for index in 1..=3 {
        assert!(press(&mut show, index, false).is_empty(), "button {index}");
    }
    // A position this executor has no button for is a key with nothing on it,
    // which is an ordinary answer rather than a refusal.
    assert!(press(&mut show, 0, true).len() == 1);
    assert!(press(&mut show, 7, true).is_empty());
    assert!(press(&mut show, u8::MAX, true).is_empty());
}

/// A profile may name the function outright; the daemon still decides what it
/// comes out as.
#[test]
fn a_named_function_is_resolved_here_as_well() {
    use prism_domain::ExecutorButtonFunction as Fn;
    let executor = ExecutorId::new(0);
    let mut show = desk_with(Vec::new(), prism_domain::ExecutorFaderFunction::Master);
    let effects = |show: &mut Show, function| {
        show.apply(&Command::ExecutorButton {
            executor_id: executor,
            button: prism_domain::ExecutorButtonRef::Function { function },
            pressed: true,
        })
        .expect("the executor plays a sequence")
        .effects
    };
    // The executor has *no* buttons assigned, so this cannot be coming from the
    // slot table: it is the profile's own row.
    assert_eq!(
        effects(&mut show, Fn::On),
        vec![Effect::ExecutorOn { executor }]
    );
    show.record_executor_state(executor, true, Some(0)).unwrap();
    assert_eq!(
        effects(&mut show, Fn::Toggle),
        vec![Effect::ExecutorOff { executor }],
        "a named Toggle was not resolved against is_active"
    );
}

/// A press on an executor that has no sequence, or none at all, is refused —
/// the same refusal `ExecutorGo` gives, for the same reason.
#[test]
fn a_button_on_an_executor_with_nothing_to_play_is_refused() {
    use prism_domain::ExecutorButtonFunction as Fn;
    let mut show = desk_with(
        vec![Fn::GoForward],
        prism_domain::ExecutorFaderFunction::Master,
    );
    let before = snapshot(&show);
    // Executor 1 exists and has no sequence. Its buttons are unassigned, so a
    // *slot* press there is a key with nothing on it; the refusal is reached
    // with a named function, which is what a transport key sends.
    assert_eq!(
        show.apply(&Command::ExecutorButton {
            executor_id: ExecutorId::new(1),
            button: prism_domain::ExecutorButtonRef::Function {
                function: Fn::GoForward,
            },
            pressed: true,
        }),
        Err(ShowError::ExecutorHasNoSequence(ExecutorId::new(1)))
    );
    assert_eq!(
        show.apply(&Command::ExecutorButton {
            executor_id: ExecutorId::new(1),
            button: prism_domain::ExecutorButtonRef::Slot { index: 0 },
            pressed: true,
        })
        .unwrap(),
        prism_core::Applied::default(),
        "a key with nothing on it is not a refusal"
    );
    // And neither is the *release* of a function that is not momentary, on the
    // same executor: a key coming up is not an instruction, so refusing it would
    // put a message on a screen for a gesture that had already ended.
    assert_eq!(
        show.apply(&Command::ExecutorButton {
            executor_id: ExecutorId::new(1),
            button: prism_domain::ExecutorButtonRef::Function {
                function: Fn::GoForward,
            },
            pressed: false,
        })
        .unwrap(),
        prism_core::Applied::default()
    );
    assert_eq!(
        show.apply(&Command::ExecutorButton {
            executor_id: ExecutorId::new(77),
            button: prism_domain::ExecutorButtonRef::Slot { index: 0 },
            pressed: true,
        }),
        Err(ShowError::UnknownExecutor(ExecutorId::new(77)))
    );
    assert_eq!(snapshot(&show), before);
}

/// **One fader command, four meanings, and the executor picks.**
#[test]
fn what_a_fader_does_is_the_executors_own_setting() {
    use prism_domain::ExecutorFaderFunction as Fader;
    let executor = ExecutorId::new(0);

    let mut master = desk_with(Vec::new(), Fader::Master);
    let applied = master
        .apply(&Command::SetExecutorMaster {
            executor_id: executor,
            level: 30_000,
        })
        .unwrap();
    assert_eq!(
        applied.effects,
        vec![Effect::SetExecutorMaster {
            executor,
            level: 30_000
        }]
    );
    assert_eq!(master.executor(executor).unwrap().master_level, 30_000);
    assert_eq!(
        master.executor(executor).unwrap().speed,
        prism_domain::SPEED_UNITY
    );

    let mut speed = desk_with(Vec::new(), Fader::Speed);
    let applied = speed
        .apply(&Command::SetExecutorMaster {
            executor_id: executor,
            level: 30_000,
        })
        .unwrap();
    assert_eq!(
        applied.effects,
        vec![Effect::ExecutorSpeed {
            executor,
            speed: 30_000
        }]
    );
    assert_eq!(speed.executor(executor).unwrap().speed, 30_000);
    assert_eq!(
        speed.executor(executor).unwrap().master_level,
        65_535,
        "the master moved"
    );

    // A crossfade in progress is a gesture rather than show state — a show file
    // that remembered one would reload holding half a cue — so it produces an
    // effect and no patch at all.
    let mut crossfade = desk_with(Vec::new(), Fader::XFade);
    let before = snapshot(&crossfade);
    let applied = crossfade
        .apply(&Command::SetExecutorMaster {
            executor_id: executor,
            level: 30_000,
        })
        .unwrap();
    assert_eq!(
        applied.effects,
        vec![Effect::ExecutorXFade {
            executor,
            position: 30_000
        }]
    );
    assert!(applied.deltas.is_empty(), "{applied:?}");
    assert_eq!(snapshot(&crossfade), before);

    // A fader with nothing on it: accepted, and it does nothing. Refusing would
    // put a message on a screen for a fader the operator can see is dead.
    let mut empty = desk_with(Vec::new(), Fader::Empty);
    let before = snapshot(&empty);
    let applied = empty
        .apply(&Command::SetExecutorMaster {
            executor_id: executor,
            level: 30_000,
        })
        .unwrap();
    assert_eq!(applied, prism_core::Applied::default());
    assert_eq!(snapshot(&empty), before);
}
