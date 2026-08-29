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
    cue, executor, fixture, group, machine_commands, par_type, patch_command, populated_show,
    preset, sequence, session_commands, show_commands,
};
use prism_core::{Effect, Show, ShowError};
use prism_domain::{
    AttributeType, Command, Delta, ExecutorId, FixtureId, GoDirection, PlaybackTarget, PresetId,
    SelectionMode, SequenceId, StoreMode, UniverseId,
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
fn the_three_groups_together_are_the_whole_protocol() {
    // A new command variant has to be given a home here, or this fails. The
    // sum is larger than the forty-nine variants there are, because two of the
    // show forms are repeats — a `StoreSequence` into a list that exists and
    // into one that does not — and because S40's four generic verbs are in
    // **two** of the lists, once with a view as their target and once without.
    //
    // **Three groups since S33**, which gave this machine's rig an applier of
    // its own: the show's, the session's and the machine's, and every command
    // is in exactly one of the three. The machine group grew a fifth in S36 —
    // the control surface's MIDI port, which belongs to the building for the
    // same reason the cabling does.
    assert_eq!(
        show_commands().len() + session_commands().len() + machine_commands().len(),
        // Sixty-three since S45's `ConfigureExecutor`, which is in the **show**
        // group: what an executor's four keys and its fader do is show content,
        // and `docs/IPC_PROTOCOL.md` §5 has the argument against the machine
        // group in full.
        63
    );
    for command in show_commands() {
        assert!(!command.is_session_command(), "{command:?}");
        assert!(!command.is_machine_command(), "{command:?}");
    }
    for command in session_commands() {
        assert!(command.is_session_command(), "{command:?}");
        assert!(!command.is_machine_command(), "{command:?}");
    }
    for command in machine_commands() {
        assert!(command.is_machine_command(), "{command:?}");
        assert!(!command.is_session_command(), "{command:?}");
    }
}

/// The rig is refused by the show applier and leaves the show byte-identical —
/// the same claim `every_session_command_is_refused_and_changes_nothing` makes,
/// for the applier S33 added.
#[test]
fn every_machine_command_is_refused_by_the_show_and_changes_nothing() {
    for command in machine_commands() {
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
        // An empty answer would be a command that was quietly dropped. An
        // **effect or a delta**, because since S45 not every show command has to
        // reach the engine: `AssignExecutor` and `ConfigureExecutor` move a
        // handle, and a playback is the cue list's rather than the slot's.
        assert!(
            !applied.effects.is_empty() || !applied.deltas.is_empty(),
            "{command:?} produced neither an effect nor a delta"
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
                sequence_id: Some(SequenceId::new(99)),
                cue_number: "1".to_owned(),
                mode: StoreMode::Merge,
            },
            ShowError::UnknownSequence(SequenceId::new(99)),
        ),
        (
            Command::StoreCue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: String::new(),
                mode: StoreMode::Merge,
            },
            ShowError::EmptyCueNumber,
        ),
        (
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(99)),
                direction: GoDirection::Prev,
            },
            ShowError::UnknownExecutor(ExecutorId::new(99)),
        ),
        (
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(1)),
                direction: GoDirection::Next,
            },
            ShowError::ExecutorHasNoSequence(ExecutorId::new(1)),
        ),
        (
            Command::ExecutorOff {
                target: PlaybackTarget::of_executor(ExecutorId::new(1)),
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
                software_dimmer: true,
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
    show.store_sequence(sequence(2, vec![cue("1", 99, AttributeType::Red, 0)]))
        .unwrap_err();
    show.store_sequence(sequence(
        2,
        vec![
            cue("1", 1, AttributeType::Red, 0),
            cue("1", 2, AttributeType::Red, 0),
        ],
    ))
    .unwrap_err();
    show.remove_sequence(SequenceId::new(99)).unwrap_err();
    show.store_cue(SequenceId::new(99), cue("1", 1, AttributeType::Red, 0))
        .unwrap_err();
    show.store_cue(SequenceId::new(1), cue("", 1, AttributeType::Red, 0))
        .unwrap_err();
    show.store_executor(executor(2, Some(99))).unwrap_err();
    show.set_sequence_master(SequenceId::new(99), 0)
        .unwrap_err();
    show.set_sequence_speed(SequenceId::new(99), 0).unwrap_err();
    show.configure_executor(
        ExecutorId::new(99),
        &prism_domain::ExecutorChange::Encoder {
            function: prism_domain::ExecutorEncoderFunction::Speed,
        },
    )
    .unwrap_err();
    show.record_playback_state(
        prism_domain::PlaybackId::of_sequence(SequenceId::new(99)),
        true,
        None,
    )
    .unwrap_err();

    assert_eq!(snapshot(&show), before);
}

#[test]
fn a_cue_part_linked_to_a_preset_that_is_gone_is_refused() {
    let mut show = populated_show();
    let mut linked = cue("5", 1, AttributeType::Red, 0);
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
    fn a_rejected_command_never_changes_the_show(command in prism_domain::arb::command()) {
        let mut show = populated_show();
        let before = snapshot(&show);
        if show.apply(&command).is_err() {
            prop_assert_eq!(snapshot(&show), before);
        }
    }

    /// And an accepted one only ever changes it in ways it announced: a command
    /// that produced no `ShowPatch` may not have touched the show.
    #[test]
    fn a_command_that_reports_no_change_made_none(command in prism_domain::arb::command()) {
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
    // The playback `desk_with`'s executor resolves to — S45: an executor is a
    // handle on the cue list standing on it, so what an effect names is the
    // list.
    let executor = prism_domain::PlaybackId::of_sequence(SequenceId::new(1));
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
        let mut show = desk_with(
            vec![function.clone()],
            prism_domain::ExecutorFaderFunction::Master,
        );
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
    let executor = prism_domain::PlaybackId::of_sequence(SequenceId::new(1));
    let mut show = desk_with(
        vec![Fn::Toggle],
        prism_domain::ExecutorFaderFunction::Master,
    );
    assert_eq!(
        press(&mut show, 0, true),
        vec![Effect::ExecutorOn { executor }]
    );

    // Somebody — a tick readback, which is the only author — says it is running.
    show.record_playback_state(executor, true, Some(0)).unwrap();
    assert_eq!(
        press(&mut show, 0, true),
        vec![Effect::ExecutorOff { executor }]
    );

    show.record_playback_state(executor, false, None).unwrap();
    assert_eq!(
        press(&mut show, 0, true),
        vec![Effect::ExecutorOn { executor }]
    );
}

/// Only `Flash` hears a release, and a key with nothing on it hears neither.
#[test]
fn a_release_is_half_a_flash_and_nothing_at_all_to_anything_else() {
    use prism_domain::ExecutorButtonFunction as Fn;
    let executor = prism_domain::PlaybackId::of_sequence(SequenceId::new(1));
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
    let executor = prism_domain::PlaybackId::of_sequence(SequenceId::new(1));
    let mut show = desk_with(Vec::new(), prism_domain::ExecutorFaderFunction::Master);
    let effects = |show: &mut Show, function| {
        show.apply(&Command::ExecutorButton {
            executor_id: ExecutorId::new(0),
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
    show.record_playback_state(executor, true, Some(0)).unwrap();
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
    let slot = ExecutorId::new(0);
    // What a fader moves is the cue list standing on the slot — S45.
    let executor = prism_domain::PlaybackId::of_sequence(SequenceId::new(1));

    let mut master = desk_with(Vec::new(), Fader::Master);
    let applied = master
        .apply(&Command::SetExecutorMaster {
            executor_id: slot,
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
    assert_eq!(
        master.sequence(SequenceId::new(1)).unwrap().master_level,
        30_000
    );
    assert_eq!(
        master.sequence(SequenceId::new(1)).unwrap().speed,
        prism_domain::SPEED_UNITY
    );

    let mut speed = desk_with(Vec::new(), Fader::Speed);
    let applied = speed
        .apply(&Command::SetExecutorMaster {
            executor_id: slot,
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
    assert_eq!(speed.sequence(SequenceId::new(1)).unwrap().speed, 30_000);
    assert_eq!(
        speed.sequence(SequenceId::new(1)).unwrap().master_level,
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
            executor_id: slot,
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
            executor_id: slot,
            level: 30_000,
        })
        .unwrap();
    assert_eq!(applied, prism_core::Applied::default());
    assert_eq!(snapshot(&empty), before);
}

// ------------------------------------------- S40: a playback named by sequence

/// **A cue list an executor holds is played by that executor.**
///
/// `Go+ Sequence 1` and `Go+ Executor 0` are the same playback when executor 0
/// holds sequence 1, and they have to be, or the operator would have two of the
/// same cue list running out of step. `Show::playback_of` is where the two names
/// meet, and it prefers the executor because that is the one with a fader.
#[test]
fn a_sequence_an_executor_holds_resolves_to_the_one_playback_it_has() {
    let mut show = populated_show();
    let applied = show
        .apply(&Command::ExecutorOn {
            target: PlaybackTarget::of_sequence(SequenceId::new(1)),
        })
        .expect("executor 0 holds sequence 1");
    assert_eq!(
        applied.effects,
        vec![Effect::ExecutorOn {
            executor: prism_domain::PlaybackId::of_sequence(SequenceId::new(1)),
        }]
    );
}

/// **A cue list no executor holds is played anyway**, as itself.
///
/// This is what makes the whole S40 vocabulary reachable from the command line
/// on a show nobody has assigned yet: `Go+ Sequence 4` runs cue list 4 without
/// an operator first having to find a free strip for it.
#[test]
fn a_sequence_no_executor_holds_is_a_playback_of_its_own() {
    let mut show = populated_show();
    show.store_sequence(sequence(4, vec![cue("1", 1, AttributeType::Red, 65535)]))
        .unwrap();

    let applied = show
        .apply(&Command::ExecutorOn {
            target: PlaybackTarget::of_sequence(SequenceId::new(4)),
        })
        .expect("sequence 4 is there");
    assert_eq!(
        applied.effects,
        vec![Effect::ExecutorOn {
            executor: prism_domain::PlaybackId::of_sequence(SequenceId::new(4)),
        }]
    );
}

/// A cue list that is not there cannot be played, and neither can a `Selected`
/// target that reached the show — the session is what answers that one, and
/// `ShowFile::resolve` has already filled it in before anything gets here.
#[test]
fn a_playback_the_show_cannot_name_is_refused() {
    let mut show = populated_show();
    assert!(matches!(
        show.apply(&Command::ExecutorOn {
            target: PlaybackTarget::of_sequence(SequenceId::new(404)),
        }),
        Err(ShowError::UnknownSequence(_))
    ));
    assert!(matches!(
        show.apply(&Command::ExecutorOn {
            target: PlaybackTarget::Selected,
        }),
        Err(ShowError::NoSelectedSequence)
    ));
}

/// **`Goto Cue 2` is a cue *number*, resolved to the index the tick works in.**
///
/// The two are not the same thing — a cue list can be numbered 1, 2, 5, 5.5 —
/// and this is the only place they meet, so the test says it on both kinds of
/// playback and on the numbers that are not there.
#[test]
fn a_goto_turns_the_number_an_operator_typed_into_an_index() {
    let mut show = populated_show();

    // Through the executor that holds the list.
    let applied = show
        .apply(&Command::Goto {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            cue_number: "2".to_owned(),
        })
        .expect("cue 2 is in sequence 1");
    assert_eq!(
        applied.effects,
        vec![Effect::Goto {
            executor: prism_domain::PlaybackId::of_sequence(SequenceId::new(1)),
            cue_index: 1,
        }]
    );

    // And on a cue list nobody holds, addressed by name. The number is trimmed,
    // because the command line hands a word over with the spacing it was typed.
    show.store_sequence(sequence(
        4,
        vec![
            cue("1", 1, AttributeType::Red, 1),
            cue("7", 1, AttributeType::Red, 2),
        ],
    ))
    .unwrap();
    let applied = show
        .apply(&Command::Goto {
            target: PlaybackTarget::of_sequence(SequenceId::new(4)),
            cue_number: " 7 ".to_owned(),
        })
        .expect("cue 7 is in sequence 4");
    assert_eq!(
        applied.effects,
        vec![Effect::Goto {
            executor: prism_domain::PlaybackId::of_sequence(SequenceId::new(4)),
            cue_index: 1,
        }]
    );
}

/// A `Goto` that names a cue nobody wrote is refused, and so is one on a strip
/// with nothing loaded — an index into a list that is not there would be the
/// tick's problem rather than the operator's.
#[test]
fn a_goto_with_nowhere_to_go_is_refused() {
    let mut show = populated_show();
    assert!(matches!(
        show.apply(&Command::Goto {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            cue_number: "404".to_owned(),
        }),
        Err(ShowError::UnknownCue { .. })
    ));

    show.store_executor(executor(3, None)).unwrap();
    assert!(matches!(
        show.apply(&Command::Goto {
            target: PlaybackTarget::of_executor(ExecutorId::new(3)),
            cue_number: "1".to_owned(),
        }),
        Err(ShowError::ExecutorHasNoSequence(_))
    ));
}
