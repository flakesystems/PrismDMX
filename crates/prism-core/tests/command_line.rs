//! **Running a line at the daemon** — S49, and the half `tests/console.rs` does
//! not reach.
//!
//! `console.rs` holds the grammar: what a line *means*, against a recording of
//! what a real `prismd` accepted for it. This is what `ShowFile` does with the
//! answer — which is a different question, and the one that used to be a client's
//! job:
//!
//! - a line runs, as the commands it means, in order, and the line is cleared;
//! - a line that is **not** one is written and left standing, because the
//!   reading under the box already says what is wrong with it and an operator
//!   who typed `Store` and clicked a tile wants what they built to stay there;
//! - a refusal **stops the rest of the line**, which is a change from what the
//!   client-side loop did: a line is one sentence;
//! - the mode an operator chose reaches the command that was waiting for one;
//! - and whether a store's destination is already there is `ShowFile::holds`,
//!   which is what decides whether anybody is asked at all.
//!
//! `crates/prismd/src/core.rs` has the end of the chain — a bound key with a
//! line on it putting light on the rig with no client attached.

mod common;

use common::{populated_session, populated_show};
use prism_core::{Applied, ShowFile, ShowFileError};
use prism_domain::{
    AttributeType, Command, CommandLineMode, Delta, FixtureId, JsonPatchOp, NoticeLevel, ObjectRef,
    PresetId, SequenceId, ViewId,
};

/// A file with the populated show, its session and a programmer holding nothing.
fn file() -> ShowFile {
    ShowFile {
        show: populated_show(),
        session: populated_session(),
        ..ShowFile::new()
    }
}

/// The command a keyboard's Enter, a bound key and an executor key all send.
fn run(text: &str) -> Command {
    Command::CommandLineInput {
        text: text.to_owned(),
        run: true,
        mode: None,
    }
}

/// The same, with the word an operator pressed on the prompt.
fn run_with(text: &str, mode: CommandLineMode) -> Command {
    Command::CommandLineInput {
        text: text.to_owned(),
        run: true,
        mode: Some(mode),
    }
}

/// What the console line holds.
fn line(file: &ShowFile) -> String {
    file.session.session().command_line.clone()
}

/// The messages a run answered with, if any.
fn notices(applied: &Applied) -> Vec<String> {
    applied
        .deltas
        .iter()
        .filter_map(|delta| match delta {
            Delta::Notice { level, message } if *level == NoticeLevel::Error => {
                Some(message.clone())
            }
            _ => None,
        })
        .collect()
}

/// Every path a session patch touched, so *the line was cleared* can be
/// asserted on the delta rather than only on the state.
fn session_paths(applied: &Applied) -> Vec<String> {
    applied
        .deltas
        .iter()
        .flat_map(|delta| match delta {
            Delta::SessionPatch { ops } => ops.clone(),
            _ => Vec::new(),
        })
        .filter_map(|op| match op {
            JsonPatchOp::Replace { path, .. }
            | JsonPatchOp::Add { path, .. }
            | JsonPatchOp::Remove { path } => Some(path),
            _ => None,
        })
        .collect()
}

/// **A line runs, as the commands it means, and the line is cleared.**
///
/// `1 thru 3 at 50` is two commands and one message: before S49 a client sent
/// the two and then a third clearing the line, and what a client sent is what
/// the daemon did. Now the daemon does all of it.
#[test]
fn a_line_runs_as_the_commands_it_means() {
    let mut file = file();
    // Typed first, the way an operator gets a line into the box: every keystroke
    // is mirrored, because `Session::commandLine` is shared (§4.1).
    file.apply(&Command::CommandLineInput {
        text: "1 thru 3 at 50".to_owned(),
        run: false,
        mode: None,
    })
    .expect("a keystroke");
    let deltas = file.apply(&run("1 thru 3 at 50")).expect("a good line");

    let programmer = file.programmer.state();
    assert_eq!(
        programmer.selection,
        vec![FixtureId::new(1), FixtureId::new(2), FixtureId::new(3)]
    );
    assert!(
        programmer
            .values
            .values()
            .any(|held| held.contains_key(&AttributeType::Dimmer)),
        "the level reached the programmer: {programmer:?}"
    );
    // And the line is cleared, which is what Enter has always looked like — said
    // in the delta, so every screen and the X-Touch's display are told.
    assert_eq!(line(&file), "");
    assert!(
        session_paths(&deltas)
            .iter()
            .any(|path| path == "/session/commandLine"),
        "the cleared line is broadcast: {deltas:?}"
    );
    assert!(notices(&deltas).is_empty());
}

/// A line that only writes the show reaches the show.
#[test]
fn a_line_that_edits_the_show_edits_the_show() {
    let mut file = file();
    file.apply(&run("label group 1 \"Front wash\""))
        .expect("a good line");
    assert_eq!(
        file.show
            .group(prism_domain::GroupId::new(1))
            .expect("group 1")
            .name,
        "Front wash"
    );
    assert_eq!(line(&file), "");
}

/// **A line that is not one is written and left standing.**
///
/// The half a pointer depends on: `Store` typed and a fixture tile clicked gives
/// `Store Fixture 5`, which must not vanish and must not be carried out.
#[test]
fn a_line_that_is_not_one_is_written_and_left_standing() {
    let mut file = file();
    let before = rmp_serde::to_vec_named(&file.show).expect("a show encodes");
    let deltas = file
        .apply(&run("Store Fixture 5"))
        .expect("nothing to fail");

    assert_eq!(line(&file), "Store Fixture 5");
    assert_eq!(
        rmp_serde::to_vec_named(&file.show).expect("a show encodes"),
        before,
        "the show moved"
    );
    // No complaint travels: the reading under the box is the daemon's sentence
    // for this line and every screen is already drawing it
    // (`Query::CommandLineReading`), so a notice would be the same words twice.
    assert!(notices(&deltas).is_empty());
}

/// **An empty line does nothing at all — not even to the line.**
///
/// Enter on an empty console is a key an operator presses out of habit, and a
/// desk that answered it by writing whitespace into a field every screen draws
/// would be answering something nobody asked.
#[test]
fn an_empty_line_does_nothing() {
    let mut file = file();
    file.apply(&Command::CommandLineInput {
        text: "Store ".to_owned(),
        run: false,
        mode: None,
    })
    .expect("a keystroke");
    let deltas = file.apply(&run("   ")).expect("nothing to fail");
    assert_eq!(line(&file), "Store ", "an empty line wrote over the box");
    assert!(deltas.deltas.is_empty(), "{deltas:?}");
    assert!(notices(&deltas).is_empty());
}

/// **A refusal stops the rest of the line.**
///
/// `404 at 50` on a rig with no fixture 404 is one sentence whose first half
/// cannot be carried out. The client-side loop sent both commands, so the level
/// landed on whatever happened to be selected; a line is one sentence, and the
/// refusal is said out loud because a fan-out has one outcome.
#[test]
fn a_refusal_stops_the_rest_of_the_line() {
    let mut file = file();
    // Something is selected first, so *the level landed on the old selection*
    // is a state this test could actually observe.
    file.apply(&run("1")).expect("fixture 1 is patched");
    let before = file.programmer.state().clone();

    let deltas = file.apply(&run("404 at 50")).expect("nothing to fail");
    assert_eq!(
        notices(&deltas).len(),
        1,
        "the refusal is said out loud: {deltas:?}"
    );
    assert_eq!(
        &before,
        file.programmer.state(),
        "the second half of the line was carried out"
    );
}

/// **The mode an operator chose reaches the command that was waiting for one.**
#[test]
fn the_chosen_mode_travels_with_the_line() {
    let mut file = file();
    file.apply(&Command::SelectSequence {
        sequence_id: SequenceId::new(1),
    })
    .expect("sequence 1 is there");
    file.apply(&run("1 thru 3")).expect("a good line");
    file.apply(&run("red at 100")).expect("a good line");

    // Cue 1 holds one part out of `populated_show`. An Override makes it exactly
    // what the programmer holds, which a Merge would not.
    file.apply(&run_with("store cue 1", CommandLineMode::Override))
        .expect("a good line");
    let cue = file.show.cue(SequenceId::new(1), "1").expect("cue 1");
    assert_eq!(cue.parts.len(), 3, "an Override writes the programmer");

    // And a mode a command has no use for is ignored rather than forced in: a
    // `Goto` has no mode at all and the line runs unchanged.
    file.apply(&run_with("goto cue 1", CommandLineMode::Remove))
        .expect("a good line");
}

/// **Whether anything is already there is the daemon's answer.**
///
/// It decides whether an interface asks *merge, override or cancel* at all, and
/// it moved here from `ui/src/desk/exists.ts` with the parser: a client reading
/// its own mirror can be a delta behind, and would then ask about a cue somebody
/// has just deleted.
#[test]
fn the_daemon_says_whether_the_destination_is_already_there() {
    let mut file = file();
    assert!(file.holds(&ObjectRef::Sequence {
        sequence_id: SequenceId::new(1)
    }));
    assert!(!file.holds(&ObjectRef::Sequence {
        sequence_id: SequenceId::new(404)
    }));
    assert!(file.holds(&ObjectRef::Group {
        group_id: prism_domain::GroupId::new(1)
    }));
    assert!(!file.holds(&ObjectRef::Group {
        group_id: prism_domain::GroupId::new(404)
    }));
    assert!(file.holds(&ObjectRef::Preset {
        preset_id: PresetId::new(4)
    }));
    assert!(!file.holds(&ObjectRef::Preset {
        preset_id: PresetId::new(404)
    }));
    assert!(file.holds(&ObjectRef::Executor {
        executor_id: prism_domain::ExecutorId::new(0)
    }));
    assert!(!file.holds(&ObjectRef::Executor {
        executor_id: prism_domain::ExecutorId::new(404)
    }));
    // A view is the session's, which is why this predicate is on the file.
    assert!(file.holds(&ObjectRef::View {
        view_id: ViewId::new(2)
    }));
    assert!(!file.holds(&ObjectRef::View {
        view_id: ViewId::new(404)
    }));

    // A cue that names its list is looked up in it, **trimmed**, because the
    // number is one an operator typed.
    assert!(file.holds(&ObjectRef::Cue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: " 2 ".to_owned(),
    }));
    assert!(!file.holds(&ObjectRef::Cue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "404".to_owned(),
    }));

    // **A cue that names no list means the selected one**, and with nothing
    // selected there is nothing for it to be already holding — so the answer is
    // `false` and the command goes out to be refused, which is S26's split.
    let unnamed = ObjectRef::Cue {
        sequence_id: None,
        cue_number: "1".to_owned(),
    };
    assert!(!file.holds(&unnamed));
    file.apply(&Command::SelectSequence {
        sequence_id: SequenceId::new(1),
    })
    .expect("sequence 1 is there");
    assert!(file.holds(&unnamed));
}

/// A line naming a cue list that is not there is refused, and refused **as a
/// command** rather than as a line: the fan-out is one command, so its failure
/// is one outcome.
#[test]
fn a_line_that_needs_a_selection_and_has_none_is_refused() {
    let mut file = file();
    let deltas = file.apply(&run("store cue 5")).expect("nothing to fail");
    assert_eq!(notices(&deltas).len(), 1, "{deltas:?}");
    assert!(
        notices(&deltas)[0].to_lowercase().contains("cue list"),
        "the complaint names what is missing: {:?}",
        notices(&deltas)[0]
    );
}

/// Writing a line is not running one, and the session applier says so on its
/// own — which is what keeps `CommandLineInput` a session command.
#[test]
fn a_keystroke_writes_the_line_and_nothing_else() {
    let mut file = file();
    let before = rmp_serde::to_vec_named(&file.show).expect("a show encodes");
    file.apply(&Command::CommandLineInput {
        text: "1 thru 3 at 50".to_owned(),
        run: false,
        mode: None,
    })
    .expect("a keystroke");
    assert_eq!(line(&file), "1 thru 3 at 50");
    assert_eq!(
        rmp_serde::to_vec_named(&file.show).expect("a show encodes"),
        before
    );
    assert!(file.programmer.state().selection.is_empty());
}

/// A line is journalled as the commands it meant, so an Oops takes back the
/// **edit** rather than the typing.
#[test]
fn an_oops_takes_back_what_the_line_did() {
    let mut file = file();
    file.apply(&run("label group 1 \"Front wash\""))
        .expect("a good line");
    file.apply(&Command::Oops).expect("something to undo");
    assert_ne!(
        file.show
            .group(prism_domain::GroupId::new(1))
            .expect("group 1")
            .name,
        "Front wash"
    );
}

/// A show command sent straight through still is one: the fan-out is reached by
/// `run`, and nothing else changed about how a command is routed.
#[test]
fn a_command_that_is_not_a_line_is_untouched() {
    let mut file = file();
    let refused = file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(404)),
        cue_number: "1".to_owned(),
    });
    assert!(matches!(refused, Err(ShowFileError::Show(_))));
}
