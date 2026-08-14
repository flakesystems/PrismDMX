//! The Oops journal — `ARCHITECTURE_SPEC.md` §6.1 and session S14.
//!
//! Four claims are checked here, and they are the session's exit criteria:
//! *n* commands followed by *n* Oops presses leave the state byte for byte
//! where it started; a Redo puts back exactly what the Oops took away; a
//! playback action is **not** taken back by an Oops; and a journal that has run
//! past its two hundred entries has dropped the oldest and nothing else.
//!
//! "Byte for byte" is taken literally, as it was in S11, S12 and S13: the file
//! is serialised with `rmp_serde::to_vec_named` before and after and the two
//! `Vec<u8>` compared — plus the programmer, which is deliberately not part of
//! those bytes (S13) and would otherwise be the half nobody checked.

mod common;

use common::{
    executor, fixture, par_type, patch_command, populated_session, populated_show, sequence,
};
use prism_core::{Effect, Journal, JournalError, ShowFile, ShowFileError, UndoScope};
use prism_domain::{
    AttributeType, ClearStage, Command, Delta, ExecutorId, FixtureId, GoDirection, PresetId,
    ProgrammerState, SelectionMode, SequenceId, UniverseId,
};
use proptest::prelude::*;

/// A saved show in a populated session: one of everything, Save LED dark.
///
/// The programmer page and the jog wheel start somewhere other than zero on
/// purpose. They are the session's half of a programmer command's scope — a new
/// selection resets the wheel, the third Clear resets both — and a file that
/// started at zero would let a journal that never recorded them pass every test
/// in here, because resetting zero to zero changes nothing.
fn file() -> ShowFile {
    let mut file = ShowFile {
        show: populated_show(),
        session: populated_session(),
        ..ShowFile::new()
    };
    file.session.set_programmer_page(3).unwrap();
    file.session.set_programmer_param_index(5).unwrap();
    file
}

/// The whole state of a file, as far as anything can be compared.
///
/// The bytes are what goes to disk. The programmer is not in them — S13 decided
/// that and asserted it — so it travels beside them, and the dirty flag beside
/// that, because a `#[serde(skip)]` field cannot be seen in a serialised form.
fn snapshot(file: &ShowFile) -> (Vec<u8>, ProgrammerState, bool) {
    (
        rmp_serde::to_vec_named(file).unwrap(),
        file.programmer.state().clone(),
        file.is_dirty(),
    )
}

/// The bytes and the programmer, without the flag: what an undo has to restore.
///
/// The Save LED is deliberately excluded. Undoing back to the state that was
/// last written does not put the file back on disk, so the lamp stays lit —
/// `undoing_back_to_the_saved_state_does_not_clean_the_file` asserts that
/// separately.
fn state(file: &ShowFile) -> (Vec<u8>, ProgrammerState) {
    let (bytes, programmer, _) = snapshot(file);
    (bytes, programmer)
}

/// Selects fixtures 1 and 2 and puts red on them.
fn program(file: &mut ShowFile) {
    file.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(1), FixtureId::new(2)],
        mode: SelectionMode::Set,
    })
    .unwrap();
    file.apply(&Command::SetAttribute {
        attribute: AttributeType::Red,
        value: 65535,
        relative: false,
    })
    .unwrap();
}

/// **The three patch commands S27 added are taken back, whole.**
///
/// The interesting one is the renumber: it is a remove and an insert, so its
/// record carries *two* fixture images, and a journal that imaged only the
/// number the operator typed would put the fixture back and leave a copy of it
/// on the new number. Each is asserted on the file's **bytes**, so a restore
/// that got the position, the rotation or the inverts wrong would fail here
/// too.
#[test]
fn the_patch_edits_of_s27_are_undone_and_redone_byte_for_byte() {
    for command in [
        Command::UnpatchFixture {
            id: FixtureId::new(1),
        },
        Command::RenumberFixture {
            id: FixtureId::new(1),
            to: FixtureId::new(77),
        },
        Command::EmbedFixtureType {
            type_id: "generic.rgb.par".to_owned(),
        },
    ] {
        let mut file = file();
        // A fixture with geometry on it, so a restore that rebuilt the fixture
        // from the command rather than from the image would be visible.
        let mut hung = fixture(1, "generic.rgbw.par", 1, 1);
        hung.position = prism_domain::Vec3::new(1.5, 4.0, -2.0);
        hung.invert_tilt = true;
        file.show.patch_fixture(hung).unwrap();
        let before = state(&file);

        file.apply(&command)
            .unwrap_or_else(|error| panic!("{command:?}: {error}"));
        assert_ne!(state(&file), before, "{command:?} changed nothing");
        let after = state(&file);

        file.apply(&Command::Oops).unwrap();
        assert_eq!(state(&file), before, "{command:?} was not taken back");
        file.apply(&Command::Redo).unwrap();
        assert_eq!(state(&file), after, "{command:?} was not put back");
    }
}

/// A renumber's record names **both** numbers, which is what makes the undo
/// above possible rather than lucky.
#[test]
fn a_renumber_is_journalled_over_both_numbers() {
    let mut renumbered = file();
    renumbered
        .apply(&Command::RenumberFixture {
            id: FixtureId::new(1),
            to: FixtureId::new(77),
        })
        .unwrap();
    let record = renumbered.journal.undoable().expect("a renumber is a step");
    assert_eq!(
        record.scope(),
        vec![
            UndoScope::Fixture(FixtureId::new(1)),
            UndoScope::Fixture(FixtureId::new(77)),
        ]
    );

    let mut embedded = file();
    embedded
        .apply(&Command::EmbedFixtureType {
            type_id: "generic.rgb.par".to_owned(),
        })
        .unwrap();
    assert_eq!(
        embedded
            .journal
            .undoable()
            .expect("an embed is a step")
            .scope(),
        vec![UndoScope::FixtureType("generic.rgb.par".to_owned())]
    );
}

/// Renumbering a fixture to the number it already has is accepted, changes
/// nothing, and is **not** a step: an operator who pressed Oops after it would
/// otherwise watch nothing happen and press it again, losing the edit they
/// actually meant to take back.
#[test]
fn a_renumber_to_the_same_number_is_not_a_step() {
    let mut file = file();
    let before = snapshot(&file);
    let applied = file
        .apply(&Command::RenumberFixture {
            id: FixtureId::new(1),
            to: FixtureId::new(1),
        })
        .expect("asking for the number it has is not an error");
    assert!(applied.deltas.is_empty(), "{applied:?}");
    assert!(applied.effects.is_empty(), "{applied:?}");
    assert_eq!(snapshot(&file), before);
    assert!(file.journal.is_empty());
}

// -- the four exit criteria ------------------------------------------------

/// Commands a populated file can actually apply, so a property run reaches the
/// journal instead of bouncing off the validators.
///
/// `any::<Command>()` is in the mix as well — it is the broad net, and it is
/// what an arbitrary `PatchFixture` naming a profile the show has never heard
/// of looks like — but on its own it would journal almost nothing: a fixture
/// type key drawn from arbitrary text is never one of this show's two.
fn undoable_command() -> impl Strategy<Value = Command> {
    let plausible = prop_oneof![
        (
            proptest::collection::vec(1u32..=4, 1..3),
            any::<SelectionMode>()
        )
            .prop_map(|(ids, mode)| Command::SelectFixtures {
                ids: ids.into_iter().map(FixtureId::new).collect(),
                mode,
            }),
        (any::<AttributeType>(), -70_000i32..70_000, any::<bool>()).prop_map(
            |(attribute, value, relative)| Command::SetAttribute {
                attribute,
                value,
                relative,
            }
        ),
        Just(Command::ApplyPreset {
            preset_id: PresetId::new(4),
        }),
        Just(Command::ClearProgrammer),
        (1u32..=3).prop_map(|number| Command::StoreCue {
            sequence_id: SequenceId::new(1),
            cue_number: number.to_string(),
        }),
        (1u32..=6, 1u32..=2, 1u16..=20)
            .prop_map(|(id, universe, address)| patch_command(id, universe, address)),
    ];
    prop_oneof![
        4 => plausible,
        1 => any::<Command>().prop_filter("only undoable commands", Command::is_undoable),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// **Exit criterion.** Apply *n* commands, press Oops *n* times, and the
    /// state is the one the file started in — byte for byte.
    ///
    /// *n* is the number of journal entries rather than the number of commands,
    /// and that is the criterion rather than a weakening of it: a command that
    /// was refused, and a command that changed nothing, are not steps to take
    /// back. What the property really pins is that **every** state change an
    /// undoable command makes is inside the record it produced — if any of it
    /// were outside, the bytes would differ here.
    #[test]
    fn undoing_every_command_returns_the_state_it_started_in(
        commands in proptest::collection::vec(undoable_command(), 1..24)
    ) {
        let mut file = file();
        let start = state(&file);

        for command in &commands {
            let _ = file.apply(command);
        }
        let programmed = state(&file);

        let entries = file.journal.len();
        for _ in 0..entries {
            file.apply(&Command::Oops).unwrap();
        }
        prop_assert_eq!(state(&file), start, "an undo did not restore everything");
        prop_assert_eq!(
            file.apply(&Command::Oops),
            Err(ShowFileError::Journal(JournalError::NothingToUndo))
        );

        // **Exit criterion.** And a Redo of each puts the state back where the
        // command left it.
        prop_assert_eq!(file.journal.redo_len(), entries);
        for _ in 0..entries {
            file.apply(&Command::Redo).unwrap();
        }
        prop_assert_eq!(state(&file), programmed, "a redo did not put everything back");
        prop_assert_eq!(
            file.apply(&Command::Redo),
            Err(ShowFileError::Journal(JournalError::NothingToRedo))
        );
    }
}

/// **Exit criterion**, scripted rather than generated, so the reader can see
/// which state each step is about.
#[test]
fn a_redo_puts_back_exactly_what_the_oops_took_away() {
    let mut file = file();
    program(&mut file);
    file.apply(&Command::StoreCue {
        sequence_id: SequenceId::new(1),
        cue_number: "3".to_owned(),
    })
    .unwrap();
    let stored = state(&file);
    assert_eq!(
        file.show.sequence(SequenceId::new(1)).unwrap().cues.len(),
        3
    );

    file.apply(&Command::Oops).unwrap();
    assert_eq!(
        file.show.sequence(SequenceId::new(1)).unwrap().cues.len(),
        2
    );

    file.apply(&Command::Redo).unwrap();
    assert_eq!(state(&file), stored);
    assert_eq!(
        file.show.sequence(SequenceId::new(1)).unwrap().cues.len(),
        3
    );
}

/// **Exit criterion.** An executor that is running goes on running through an
/// Oops, and so does the master the operator has just moved.
///
/// This is the one the whole exclusion exists for: `ARCHITECTURE_SPEC.md` §6.1
/// says an undo must not change light the operator is currently driving.
/// `SetExecutorMaster` is the sharp case, because it *is* show state and it *is*
/// written into the show — a journal that snapshotted the show rather than the
/// scope the command touched would take the fader back down with the patch.
#[test]
fn a_running_executor_survives_an_oops() {
    let mut file = file();
    // Something to undo, so the Oops has work to do and cannot pass by being
    // refused.
    file.apply(&patch_command(9, 3, 1)).unwrap();

    file.apply(&Command::SetExecutorMaster {
        executor_id: ExecutorId::new(0),
        level: 32_768,
    })
    .unwrap();
    let applied = file
        .apply(&Command::ExecutorGo {
            executor_id: ExecutorId::new(0),
            direction: GoDirection::Next,
        })
        .unwrap();
    assert_eq!(
        applied.effects,
        vec![Effect::ExecutorGo {
            executor: ExecutorId::new(0),
            direction: GoDirection::Next,
        }]
    );
    // The engine has answered: the executor is running on cue 0.
    file.show
        .record_executor_state(ExecutorId::new(0), true, Some(0))
        .unwrap();

    // Two playback commands and a Go, and the journal has one entry: the patch.
    assert_eq!(file.journal.len(), 1);
    let applied = file.apply(&Command::Oops).unwrap();

    let executor = file.show.executor(ExecutorId::new(0)).unwrap();
    assert!(executor.is_active, "the Oops stopped the executor");
    assert_eq!(executor.current_cue_index, Some(0));
    assert_eq!(
        executor.master_level, 32_768,
        "the Oops moved a master the operator had set"
    );
    assert!(
        !applied.effects.iter().any(|effect| matches!(
            effect,
            Effect::ExecutorGo { .. }
                | Effect::ExecutorOff { .. }
                | Effect::SetExecutorMaster { .. }
        )),
        "the Oops reached into playback: {:?}",
        applied.effects
    );
    // What it did undo is the patch, and nothing else.
    assert!(file.show.fixture(FixtureId::new(9)).is_none());
    assert_eq!(
        file.apply(&Command::Oops),
        Err(ShowFileError::Journal(JournalError::NothingToUndo))
    );
}

/// The other half of the exclusion: none of the eleven §4.4 commands is
/// journaled, so an Oops never pulls a window out from under the operator.
#[test]
fn a_session_command_is_never_taken_back() {
    let mut file = file();
    file.apply(&patch_command(9, 3, 1)).unwrap();
    for command in common::session_commands() {
        let _ = file.apply(&command);
    }
    let session_after = rmp_serde::to_vec_named(&file.session).unwrap();
    assert_eq!(file.journal.len(), 1, "a session command was journaled");

    file.apply(&Command::Oops).unwrap();
    assert_eq!(
        rmp_serde::to_vec_named(&file.session).unwrap(),
        session_after,
        "the Oops moved the session"
    );
    assert!(file.show.fixture(FixtureId::new(9)).is_none());
}

/// **Exit criterion.** The ring holds two hundred entries; the two hundred and
/// first pushes the oldest out, and what is left is a journal that still works
/// in both directions.
#[test]
fn the_ring_drops_the_oldest_entry_and_stays_usable() {
    const EXTRA: usize = 50;

    let mut file = file();
    let mut after_the_dropped = None;
    for step in 0..Journal::CAPACITY + EXTRA {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the loop bound is 250, and the ids and addresses derived from it are \
                      chosen to stay inside a universe"
        )]
        let command = patch_command(
            100 + step as u32,
            10 + step as u32 / 125,
            1 + (step as u16 % 125) * 4,
        );
        file.apply(&command).unwrap();
        if step + 1 == EXTRA {
            // The state the journal can no longer reach: the fifty oldest
            // records have been pushed out by the time the run ends.
            after_the_dropped = Some(state(&file));
        }
    }
    assert_eq!(file.journal.len(), Journal::CAPACITY);
    let full = state(&file);

    for _ in 0..Journal::CAPACITY {
        file.apply(&Command::Oops).unwrap();
    }
    assert_eq!(
        state(&file),
        after_the_dropped.unwrap(),
        "the journal did not walk back exactly the entries it still held"
    );
    assert_eq!(
        file.apply(&Command::Oops),
        Err(ShowFileError::Journal(JournalError::NothingToUndo)),
        "the ring handed out more entries than it holds"
    );
    // The fifty dropped commands are still applied, and the fixtures they
    // patched are still patched.
    assert!(file.show.fixture(FixtureId::new(100)).is_some());
    assert!(file.show.fixture(FixtureId::new(149)).is_some());
    assert!(file.show.fixture(FixtureId::new(150)).is_none());

    for _ in 0..Journal::CAPACITY {
        file.apply(&Command::Redo).unwrap();
    }
    assert_eq!(state(&file), full, "the redo stack lost entries on the way");
}

// -- what an undo has to tell the rest of the system -----------------------

/// An undo produces deltas like any other command, and the effects the layers
/// above need — otherwise a client mirrors a show that no longer exists and the
/// engine keeps a rig the show has stopped describing.
#[test]
fn undoing_a_patch_reports_the_repatch_and_the_change() {
    let mut file = file();
    let revision = file.show.patch_revision();
    file.apply(&patch_command(9, 3, 1)).unwrap();

    let applied = file.apply(&Command::Oops).unwrap();
    assert_eq!(applied.effects, vec![Effect::Repatch]);
    assert!(matches!(
        applied.deltas.as_slice(),
        [Delta::ShowPatch { ops }] if matches!(
            ops.as_slice(),
            [prism_domain::JsonPatchOp::Remove { path }] if path == "/fixtures/9"
        )
    ));
    // The revision counts changes to the patch, and an undo is one. It never
    // walks backwards: everything derived from the patch has to be rebuilt
    // whichever direction the change went in.
    assert!(file.show.patch_revision() > revision + 1);

    // The effect that named the journal is carried out here, never handed on.
    assert!(!applied.effects.contains(&Effect::Undo));
}

/// Undoing a store hands the sequence back to the engine, because an executor
/// playing it is holding a compiled copy of the cue list that has just changed.
#[test]
fn undoing_a_store_reloads_the_sequence() {
    let mut file = file();
    program(&mut file);
    file.apply(&Command::StoreCue {
        sequence_id: SequenceId::new(1),
        cue_number: "3".to_owned(),
    })
    .unwrap();

    let applied = file.apply(&Command::Oops).unwrap();
    assert!(
        applied
            .effects
            .contains(&Effect::ReloadSequence(SequenceId::new(1)))
    );
    assert!(
        applied
            .deltas
            .iter()
            .any(|delta| matches!(delta, Delta::ShowPatch { .. }))
    );
    // And nothing about the programmer, which is correct rather than a gap: a
    // store leaves the programmer exactly as it found it unless the Clear
    // button was standing at a stage, and this one was not. An undo that
    // broadcast a `ProgrammerChanged` here would be telling every client about
    // a change that did not happen.
    assert!(
        !applied
            .deltas
            .iter()
            .any(|delta| matches!(delta, Delta::ProgrammerChanged { .. }))
    );
}

/// A store puts a cue in the show *and* a stage on the Clear button, and an
/// undo has to take back both — the record covers all three models.
#[test]
fn one_record_can_cover_the_show_the_programmer_and_the_session() {
    let mut file = file();
    program(&mut file);
    file.apply(&Command::SetProgrammerPage { page: 3 }).unwrap();
    file.apply(&Command::ClearProgrammer).unwrap();
    file.apply(&Command::ClearProgrammer).unwrap();
    let before_the_third = state(&file);

    // The third press takes the values, the selection, the feature group and
    // the session's page state with it.
    file.apply(&Command::ClearProgrammer).unwrap();
    assert_eq!(file.session.session().programmer_page, 0);
    assert_eq!(file.programmer.state().clear_stage, ClearStage::Idle);

    let record = file.journal.undoable().unwrap();
    assert_eq!(record.command(), &Command::ClearProgrammer);
    assert_eq!(
        record.scope(),
        vec![UndoScope::Programmer, UndoScope::ProgrammerPage]
    );

    file.apply(&Command::Oops).unwrap();
    assert_eq!(state(&file), before_the_third);
    assert_eq!(file.session.session().programmer_page, 3);
    assert_eq!(
        file.programmer.state().clear_stage,
        ClearStage::SelectionCleared,
        "the Clear button came back in a stage the operator never left it in"
    );
}

/// The scope is what the command touched, and nothing else. That is the whole
/// difference between this journal and two hundred copies of the show.
#[test]
fn a_record_covers_the_scope_of_its_command_and_no_more() {
    let mut file = file();

    file.apply(&patch_command(9, 3, 1)).unwrap();
    let record = file.journal.undoable().unwrap();
    assert_eq!(record.scope(), vec![UndoScope::Fixture(FixtureId::new(9))]);

    program(&mut file);
    let record = file.journal.undoable().unwrap();
    assert_eq!(
        record.scope(),
        vec![UndoScope::Programmer, UndoScope::ProgrammerPage]
    );

    file.apply(&Command::StoreCue {
        sequence_id: SequenceId::new(1),
        cue_number: "3".to_owned(),
    })
    .unwrap();
    let record = file.journal.undoable().unwrap();
    assert_eq!(
        record.scope(),
        vec![
            UndoScope::Sequence(SequenceId::new(1)),
            UndoScope::Programmer,
            UndoScope::ProgrammerPage,
        ]
    );
}

// -- refusals, and what they may not do ------------------------------------

/// An Oops with nothing to undo is refused, and a refusal changes nothing —
/// the rule S11, S12 and S13 each asserted on the bytes.
#[test]
fn an_oops_with_an_empty_journal_changes_nothing() {
    let mut file = file();
    let before = snapshot(&file);
    assert_eq!(
        file.apply(&Command::Oops),
        Err(ShowFileError::Journal(JournalError::NothingToUndo))
    );
    assert_eq!(
        file.apply(&Command::Redo),
        Err(ShowFileError::Journal(JournalError::NothingToRedo))
    );
    assert_eq!(snapshot(&file), before);
    assert_eq!(
        JournalError::NothingToUndo.to_string(),
        "there is nothing to undo"
    );
    assert_eq!(
        JournalError::NothingToRedo.to_string(),
        "there is nothing to redo"
    );
    assert_eq!(
        ShowFileError::from(JournalError::NothingToRedo).to_string(),
        JournalError::NothingToRedo.to_string()
    );
}

/// A **redo** that cannot be carried out is refused the same way, and the
/// record stays on the redo stack.
#[test]
fn a_redo_that_is_refused_leaves_everything_where_it_was() {
    let mut file = file();
    program(&mut file);
    file.apply(&Command::StoreCue {
        sequence_id: SequenceId::new(1),
        cue_number: "3".to_owned(),
    })
    .unwrap();
    file.apply(&Command::Oops).unwrap();
    assert_eq!(file.journal.redo_len(), 1);

    // The cue the redo would put back names fixture 2, which is no longer
    // patched by the time the operator changes their mind again.
    file.show.unpatch_fixture(FixtureId::new(2)).unwrap();
    let before = snapshot(&file);
    assert_eq!(
        file.apply(&Command::Redo),
        Err(ShowFileError::Show(prism_core::ShowError::UnknownFixture(
            FixtureId::new(2)
        )))
    );
    assert_eq!(
        snapshot(&file),
        before,
        "a refused redo half-applied itself"
    );
    assert_eq!(file.journal.redo_len(), 1);

    file.show
        .patch_fixture(fixture(2, "generic.rgbw.par", 1, 5))
        .unwrap();
    file.apply(&Command::Redo).unwrap();
    assert_eq!(
        file.show.sequence(SequenceId::new(1)).unwrap().cues.len(),
        3
    );
}

/// An undo that cannot be carried out leaves the state byte-identical **and**
/// leaves the record in the journal, so the operator can try again once the
/// reason is gone.
///
/// The reason is reachable rather than hypothetical: a cue part names a
/// fixture, so unpatching that fixture makes the older cue list something the
/// show would refuse to store.
#[test]
fn an_undo_that_is_refused_leaves_everything_where_it_was() {
    let mut file = file();
    program(&mut file);
    file.apply(&Command::StoreCue {
        sequence_id: SequenceId::new(1),
        cue_number: "3".to_owned(),
    })
    .unwrap();
    // Cue 1 names fixture 1, and the record's inverse is the cue list that
    // contains it.
    file.show.unpatch_fixture(FixtureId::new(1)).unwrap();
    let before = snapshot(&file);

    assert_eq!(
        file.apply(&Command::Oops),
        Err(ShowFileError::Show(prism_core::ShowError::UnknownFixture(
            FixtureId::new(1)
        )))
    );
    assert_eq!(
        snapshot(&file),
        before,
        "a refused undo half-applied itself"
    );
    assert_eq!(
        file.journal.len(),
        3,
        "the refused record was thrown away rather than left where it was"
    );

    // Put the fixture back and the same Oops goes through.
    file.show
        .patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
        .unwrap();
    file.apply(&Command::Oops).unwrap();
    assert_eq!(
        file.show.sequence(SequenceId::new(1)).unwrap().cues.len(),
        2
    );
}

/// Undoing back to the state that was last written does not put the file back
/// on disk, so the Save LED stays lit. Claiming otherwise would be a lie about
/// the platter.
#[test]
fn undoing_back_to_the_saved_state_does_not_clean_the_file() {
    let mut file = file();
    assert!(!file.is_dirty());
    let saved = state(&file);

    let applied = file.apply(&patch_command(9, 3, 1)).unwrap();
    assert!(applied.deltas.contains(&Delta::DirtyFlag {
        unsaved_changes: true
    }));

    let applied = file.apply(&Command::Oops).unwrap();
    assert_eq!(state(&file), saved);
    assert!(file.is_dirty(), "the undo cleaned a file nobody wrote");
    assert!(
        !applied
            .deltas
            .iter()
            .any(|delta| matches!(delta, Delta::DirtyFlag { .. })),
        "the lamp was already lit"
    );
}

/// A new command after an Oops forgets the Redo: the operator has taken a
/// different branch, and putting back a command from the branch they left would
/// interleave two histories.
#[test]
fn a_command_after_an_undo_forgets_the_redo() {
    let mut file = file();
    file.apply(&patch_command(9, 3, 1)).unwrap();
    file.apply(&Command::Oops).unwrap();
    assert_eq!(file.journal.redo_len(), 1);

    file.apply(&patch_command(10, 3, 5)).unwrap();
    assert_eq!(file.journal.redo_len(), 0);
    assert_eq!(
        file.apply(&Command::Redo),
        Err(ShowFileError::Journal(JournalError::NothingToRedo))
    );
    assert!(file.show.fixture(FixtureId::new(9)).is_none());
    assert!(file.show.fixture(FixtureId::new(10)).is_some());
}

/// A command that was refused is not a step, and neither is one that changed
/// nothing. A journal that recorded either would make the operator press Oops
/// twice to see anything happen.
#[test]
fn nothing_is_journaled_for_a_command_that_changed_nothing() {
    let mut file = file();
    assert!(
        file.apply(&Command::ApplyPreset {
            preset_id: PresetId::new(99),
        })
        .is_err()
    );
    assert_eq!(file.journal.len(), 0);

    // Accepted, and changes nothing: nothing is selected, so there is nowhere
    // for the value to go.
    file.apply(&Command::SetAttribute {
        attribute: AttributeType::Red,
        value: 65535,
        relative: false,
    })
    .unwrap();
    assert_eq!(file.journal.len(), 0);

    // Accepted, and repatches a fixture exactly where it already is.
    file.apply(&patch_command(1, 1, 1)).unwrap();
    assert_eq!(file.journal.len(), 0);
}

/// The journal is not in the file, and a reopened show has nothing to undo.
///
/// A record describes a step between two states of *this* show in *this* run of
/// the daemon. Restored from disk it would name a fixture that may have been
/// edited by hand since, and the first Oops would write it back.
#[test]
fn the_journal_is_not_part_of_the_show_file() {
    let mut file = file();
    file.apply(&patch_command(9, 3, 1)).unwrap();
    assert_eq!(file.journal.len(), 1);

    let bytes = rmp_serde::to_vec_named(&file).unwrap();
    let reopened: ShowFile = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(reopened.journal.len(), 0);
    assert_eq!(reopened.journal.redo_len(), 0);
    assert!(reopened.show.fixture(FixtureId::new(9)).is_some());

    // And a loader that replaces the models in place empties it explicitly.
    file.journal.clear();
    assert_eq!(file.journal.len(), 0);
    assert_eq!(
        file.apply(&Command::Oops),
        Err(ShowFileError::Journal(JournalError::NothingToUndo))
    );
}

/// Undoing a patch that *replaced* a fixture restores the fixture that was
/// there, geometry and all — the field the command does not even carry.
#[test]
fn undoing_a_repatch_restores_the_fixture_that_was_there() {
    let mut file = file();
    let mut hung = fixture(1, "generic.rgbw.par", 1, 1);
    hung.position = prism_domain::Vec3::new(1.5, 4.0, -2.0);
    hung.invert_tilt = true;
    file.show.patch_fixture(hung.clone()).unwrap();

    file.apply(&patch_command(1, 2, 100)).unwrap();
    assert_eq!(
        file.show.fixture(FixtureId::new(1)).unwrap().universe,
        UniverseId::new(2)
    );

    file.apply(&Command::Oops).unwrap();
    assert_eq!(file.show.fixture(FixtureId::new(1)), Some(&hung));
}

/// An inverse can be an absence. Undoing the *first* patch of a fixture has to
/// take it out again, and undoing the first store into an empty sequence has to
/// leave the sequence empty — neither has an earlier value to put back.
#[test]
fn an_undo_can_take_something_away_as_well_as_put_it_back() {
    let mut file = ShowFile::new();
    file.show.embed_fixture_type(par_type()).unwrap();
    file.show.store_sequence(sequence(1, Vec::new())).unwrap();
    file.show.store_executor(executor(0, Some(1))).unwrap();
    file.mark_saved();
    let empty = state(&file);

    file.apply(&patch_command(1, 1, 1)).unwrap();
    file.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(1)],
        mode: SelectionMode::Set,
    })
    .unwrap();
    file.apply(&Command::SetAttribute {
        attribute: AttributeType::Red,
        value: 65535,
        relative: false,
    })
    .unwrap();
    file.apply(&Command::StoreCue {
        sequence_id: SequenceId::new(1),
        cue_number: "1".to_owned(),
    })
    .unwrap();
    assert_eq!(
        file.show.sequence(SequenceId::new(1)).unwrap().cues.len(),
        1
    );

    let entries = file.journal.len();
    assert_eq!(entries, 4);
    for _ in 0..entries {
        file.apply(&Command::Oops).unwrap();
    }
    assert_eq!(state(&file), empty);
    assert!(file.show.fixture(FixtureId::new(1)).is_none());
    assert!(
        file.show
            .sequence(SequenceId::new(1))
            .unwrap()
            .cues
            .is_empty()
    );
}
