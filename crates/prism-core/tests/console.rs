//! The command line, held to a daemon's own answers and to one absolute rule.
//!
//! # Where the expectations come from
//!
//! `ui/tests/fixtures/desk-recording.json` carries every line an operator typed
//! in the recorded script **beside the commands a real `prismd` accepted for
//! it**, and `crates/prismd/tests/ui_programmer.rs` asserts that those payloads
//! are the commands the script names. So the central test here decodes the
//! recorded bytes and compares the parser's answer against them.
//!
//! **It is the same table `ui/src/desk/console.test.ts` used to read**, and that
//! is deliberate: S49 moved the parser out of TypeScript, and a session that
//! moved the expectations with it would have moved a parser and its own opinion
//! of itself. The recording is a running daemon's answer, it did not move, and
//! it is what says the parser means today what it meant yesterday.
//!
//! # And the rule
//!
//! **It never fails.** Not for a typo, not for a control character, not for a
//! hundred-thousand-fixture range. That is S26's third exit criterion, checked
//! here over ten thousand generated strings rather than over the six a person
//! would think of — and it is worth more in Rust than it was in TypeScript,
//! because the thing that would fall over is now the daemon.

use prism_core::console::{
    CLEARING_VERBS, CONSOLE_WORDS, MAX_RANGE, VERB_WORDS, apply_mode, completions,
    is_clearing_line, is_verb_line, parse_command_line, reading_text,
};
use prism_core::{ConsoleReading, ModeQuestionKind};
use prism_domain::{
    AttributeType, Command, CommandLineMode, ExecutorChange, ExecutorFaderFunction, ExecutorId,
    FixtureId, GoDirection, GroupId, ObjectRef, OverwriteMode, PlaybackTarget, PresetPool,
    SelectionMode, SequenceId, SequenceStoreMode, StoreMode, ViewId,
};

/* -------------------------------------------------------------------------- */
/* The recording                                                              */
/* -------------------------------------------------------------------------- */

/// Where the shared table lives — the interface's fixtures, written by
/// `crates/prismd/tests/ui_programmer.rs`.
fn recording() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("ui/tests/fixtures/desk-recording.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the recording at {} is readable: {error}", path.display()));
    serde_json::from_str(&text).expect("the recording is JSON")
}

/// Base64, decoded by hand.
///
/// A dependency for four lines of arithmetic would be a dependency in the show
/// model's test build, and the recording is base64 because a browser reads it
/// with `atob`.
fn payload(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut held = 0u32;
    let mut out = Vec::new();
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        let value = ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)
            .unwrap_or_else(|| panic!("{byte} is not base64"));
        bits = (bits << 6) | u32::try_from(value).expect("a base64 digit is 0..64");
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push(u8::try_from((bits >> held) & 0xff).expect("a masked byte"));
        }
    }
    out
}

/// One recorded client message, as far as this test needs to read it.
#[derive(serde::Deserialize)]
struct RecordedMessage {
    /// The message kind — `Command` for the ones this reads.
    t: String,
    /// The command it carried.
    #[serde(default)]
    command: Option<Command>,
}

/// The command inside a recorded client payload.
fn recorded_command(encoded: &str) -> Command {
    let message: RecordedMessage =
        rmp_serde::from_slice(&payload(encoded)).expect("a recorded client message");
    assert_eq!(message.t, "Command", "that payload is not a command");
    message.command.expect("a command message carries one")
}

/// One typed line of the recording, with the commands the daemon was sent for it.
struct TypedLine {
    /// The recorded line number, which is what groups the steps.
    number: i64,
    /// The line as it was typed.
    line: String,
    /// What a real `prismd` was sent for it, in order.
    commands: Vec<Command>,
}

/// The typed lines of the recording.
///
/// Grouped by the recorded line **number** rather than by equal text: `clear` is
/// pressed three times in a row and those are three lines, not one line with
/// three commands on it.
fn typed_lines() -> Vec<TypedLine> {
    let recording = recording();
    let steps = recording["steps"].as_array().expect("the recorded steps");
    let mut lines: Vec<TypedLine> = Vec::new();
    for step in steps {
        let (Some(typed), Some(number)) = (step["typed"].as_str(), step["typedLine"].as_i64())
        else {
            continue;
        };
        let command = recorded_command(step["client"].as_str().expect("a recorded payload"));
        match lines.last_mut() {
            Some(last) if last.number == number => last.commands.push(command),
            _ => lines.push(TypedLine {
                number,
                line: typed.to_owned(),
                commands: vec![command],
            }),
        }
    }
    lines
}

/// The commands a line parses to, or a panic naming the line that did not.
fn commands(line: &str) -> Vec<Command> {
    match parse_command_line(line) {
        ConsoleReading::Commands { commands, .. } => commands,
        other => panic!("\"{line}\" did not parse: {other:?}"),
    }
}

/// The complaint a line answers with, or a panic naming the line that parsed.
fn message(line: &str) -> String {
    match parse_command_line(line) {
        ConsoleReading::Refused(message) => message,
        other => panic!("\"{line}\" parsed: {other:?}"),
    }
}

/* -------------------------------------------------------------------------- */
/* What a line means                                                          */
/* -------------------------------------------------------------------------- */

/// **The central test.** Every line the recording carries, against the commands
/// a running daemon was sent for it.
///
/// The `clear` line appears three times in the script — the button is a
/// three-stage machine and the stages differ — and the parser answers the same
/// way each time, which is right: the stage is the *daemon's*, and a parser that
/// tracked it would be holding programmer state.
#[test]
fn every_recorded_line_means_what_the_daemon_was_sent() {
    let lines = typed_lines();
    // S26 recorded six shapes; S40's script carries the whole vocabulary, so
    // this is a floor rather than a count.
    assert!(lines.len() >= 30, "{} recorded lines", lines.len());
    for TypedLine { line, commands, .. } in &lines {
        // The **commands**, not the whole reading: a line that carries a mode
        // also carries the question somebody may have to answer about it, and
        // the recording holds what the daemon was sent rather than what was
        // asked.
        assert_eq!(
            &parse_command_line(line).commands().to_vec(),
            commands,
            "\"{line}\" means something else"
        );
    }
    // And one of them is a line that produced two commands, which a parser
    // answering with a single command could never have passed.
    assert!(
        lines.iter().any(|line| line.commands.len() == 2),
        "no recorded line produced two commands"
    );
}

/// **Every word of the vocabulary is in the recording**, so the test above is a
/// check on all of it rather than on the six shapes S26 had.
#[test]
fn the_recording_covers_the_whole_vocabulary() {
    let typed: Vec<String> = typed_lines().into_iter().map(|line| line.line).collect();
    for word in [
        "sequence 2",
        "group 1",
        "store group 1",
        "store cue 2",
        "store sequence 4",
        "store preset 1",
        "edit cue 2",
        "update",
        "label cue 2",
        "label group 1",
        "label view 1",
        "move cue 2 cue 3",
        "move sequence 5 sequence 6",
        "copy group 1 group 2",
        "copy sequence 1 sequence 5",
        "delete group 2",
        "delete executor 9",
        "delete sequence 404",
        "color sequence 1 red",
        "color executor 2 #ff8800",
        "goto cue 1",
        "on sequence 1",
        "on sequence 4",
        "off sequence 1",
        "go+ executor 0",
        "assign sequence 4 executor 9",
        "assign executor 9 fader speed",
        "preset 1",
        "full",
        "oops",
        "clear",
        "page 0",
    ] {
        assert!(
            typed.iter().any(|line| line.starts_with(word)),
            "no recorded line starts with \"{word}\""
        );
    }
}

/// **B15, at the grammar.** What a fader, an encoder and the four keys do is
/// sayable, so the control editor can write a line rather than send a command of
/// its own — which is `ARCHITECTURE_SPEC.md` §4.5's test applied rather than
/// assumed.
#[test]
fn a_function_is_assigned_to_one_of_an_executors_controls() {
    assert_eq!(
        commands("assign executor 1 fader master"),
        vec![Command::ConfigureExecutor {
            executor_id: ExecutorId::new(1),
            change: ExecutorChange::Fader {
                function: ExecutorFaderFunction::Master
            },
        }]
    );
    assert_eq!(
        commands("Assign Executor 9 Encoder Speed"),
        vec![Command::ConfigureExecutor {
            executor_id: ExecutorId::new(9),
            change: ExecutorChange::Encoder {
                function: prism_domain::ExecutorEncoderFunction::Speed
            },
        }]
    );
    // A key is numbered **from one** on the line, because that is how an
    // operator counts the keys under a fader, and from zero in the command,
    // because that is how the hardware counts them.
    assert_eq!(
        commands("assign executor 3 button 1 go+"),
        vec![Command::ConfigureExecutor {
            executor_id: ExecutorId::new(3),
            change: ExecutorChange::Button {
                index: 0,
                function: prism_domain::ExecutorButtonFunction::GoForward,
            },
        }]
    );
    assert_eq!(
        commands("assign executor 3 button 4 empty"),
        vec![Command::ConfigureExecutor {
            executor_id: ExecutorId::new(3),
            change: ExecutorChange::Button {
                index: 3,
                function: prism_domain::ExecutorButtonFunction::Empty,
            },
        }]
    );
}

/// The custom row — a key that sends a line an operator wrote.
#[test]
fn a_key_is_given_a_command_line_quoted_or_not() {
    let expected = |line: &str| {
        vec![Command::ConfigureExecutor {
            executor_id: ExecutorId::new(1),
            change: ExecutorChange::Button {
                index: 3,
                function: prism_domain::ExecutorButtonFunction::CommandLine {
                    line: line.to_owned(),
                },
            },
        }]
    };
    assert_eq!(
        commands("assign executor 1 button 4 command \"Go+ Sequence 3\""),
        expected("Go+ Sequence 3")
    );
    // **A line with punctuation in it has to be quoted**, and that is the
    // tokeniser's rule rather than this verb's: `go+` becomes `go` and `+`
    // becomes a separator, so that `1 + 2` and `go+ executor 0` mean what they
    // say. A quoted chunk is one token, kept exactly as it was typed.
    assert_eq!(
        commands("assign executor 1 button 4 command clear"),
        expected("clear")
    );
}

/// A selection is read in the direction it is written, and holds no fixture
/// twice.
#[test]
fn a_range_reads_in_the_direction_it_is_written() {
    let selection = |ids: &[u32]| {
        vec![Command::SelectFixtures {
            ids: ids.iter().copied().map(FixtureId::new).collect(),
            mode: SelectionMode::Set,
        }]
    };
    assert_eq!(commands("1 thru 3"), selection(&[1, 2, 3]));
    // Downwards, because selection order is what an operator sees when a value
    // is fanned across it.
    assert_eq!(commands("3 thru 1"), selection(&[3, 2, 1]));
    assert_eq!(commands("1 thru 3 + 2"), selection(&[1, 2, 3]));
    assert_eq!(commands("1 thru 1"), selection(&[1]));
}

/// Spacing and case are the keypad's problem, not the operator's.
#[test]
fn spacing_and_case_do_not_matter() {
    let spaced = commands("1 thru 3 + 5 at 50");
    assert_eq!(commands("1THRU3,5 AT 50"), spaced);
    assert_eq!(commands("  1 thru 3 + 5 at 50  "), spaced);
}

/// `fixture` at the front is noise an operator types out of habit.
#[test]
fn the_word_fixture_is_the_noise_it_is() {
    assert_eq!(commands("fixture 4"), commands("4"));
    assert_eq!(commands("fixtures 4"), commands("4"));
}

/// An attribute sits on either side of `at`, and the dimmer is the default.
#[test]
fn an_attribute_is_named_on_either_side_of_at() {
    let pan = Command::SetAttribute {
        attribute: AttributeType::Pan,
        occurrence: 0,
        value: 16_383,
        relative: false,
    };
    let five = Command::SelectFixtures {
        ids: vec![FixtureId::new(5)],
        mode: SelectionMode::Set,
    };
    assert_eq!(commands("5 pan at 25"), vec![five.clone(), pan.clone()]);
    assert_eq!(commands("5 at pan 25"), vec![five, pan.clone()]);
    // With no fixtures, which is the ordinary way to trim what is selected.
    assert_eq!(commands("pan at 25"), vec![pan]);
    assert_eq!(
        commands("at 50"),
        vec![Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            occurrence: 0,
            value: 32_767,
            relative: false,
        }]
    );
}

/// **A number after an attribute names which one of that kind** — S52.
///
/// A head may have two colour wheels now, so the line needs a way to say which.
/// `1 gobo 2 at 50` is the second gobo wheel; the line counts from one and the
/// key from nought, and `prism_core::console::occurrence_of` is the one place
/// the two meet.
///
/// The rule is narrow on purpose, because the shortest thing an occurrence
/// could be is also a fixture number — see `attribute_in` for the three
/// conditions. What matters here is that everything the line could say before
/// S52 still says it.
#[test]
fn a_number_after_an_attribute_names_which_one_of_that_kind() {
    let five = Command::SelectFixtures {
        ids: vec![FixtureId::new(5)],
        mode: SelectionMode::Set,
    };
    let gobo = |occurrence: u8| Command::SetAttribute {
        attribute: AttributeType::Gobo,
        occurrence,
        value: 32_767,
        relative: false,
    };
    assert_eq!(commands("5 gobo 2 at 50"), vec![five.clone(), gobo(1)]);
    // `gobo 1` is the wheel a profile lists first, which is the same as writing
    // no number at all.
    assert_eq!(commands("5 gobo 1 at 50"), vec![five.clone(), gobo(0)]);
    assert_eq!(commands("5 gobo at 50"), vec![five.clone(), gobo(0)]);

    // **And the forms that worked before still do.** `5 at pan 25` puts the
    // attribute after `at`, where the next word is the level and never an
    // occurrence; `pan 5 at 25` starts with the attribute, where the next word
    // is a fixture and never an occurrence.
    let pan = Command::SetAttribute {
        attribute: AttributeType::Pan,
        occurrence: 0,
        value: 16_383,
        relative: false,
    };
    assert_eq!(commands("5 at pan 25"), vec![five.clone(), pan.clone()]);
    assert_eq!(commands("pan 5 at 25"), vec![five, pan]);
}

/// The three words an operator says instead of a number.
#[test]
fn the_ends_of_the_range_have_words() {
    let level = |value: i32| {
        vec![Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            occurrence: 0,
            value,
            relative: false,
        }]
    };
    assert_eq!(commands("at full"), level(65_535));
    assert_eq!(commands("at out"), level(0));
    assert_eq!(commands("at zero"), level(0));
    assert_eq!(commands("at 100"), level(65_535));
    assert_eq!(commands("at 0"), level(0));
}

/// A percentage is **truncated**, so nothing rounds up past what was asked for.
#[test]
fn a_percentage_is_truncated() {
    let level = |line: &str| match commands(line).first() {
        Some(Command::SetAttribute { value, .. }) => *value,
        other => panic!("{other:?} is not a level"),
    };
    assert_eq!(level("at 50"), 32_767);
    assert_eq!(level("at 25"), 16_383);
    assert_eq!(level("at 33.3"), 21_823);
    assert_eq!(level("at 100"), 65_535);
    assert_eq!(level("at 0"), 0);
}

/// **S26's forms, kept and unbroken**: a bare number after a playback word is an
/// executor, which is what it meant when there was nothing else it could be.
#[test]
fn the_executor_and_page_words_still_mean_what_they_meant() {
    let executor = |id: u32| PlaybackTarget::of_executor(ExecutorId::new(id));
    assert_eq!(
        commands("go 3"),
        vec![Command::ExecutorGo {
            target: executor(3),
            direction: GoDirection::Next,
        }]
    );
    assert_eq!(commands("go+ 3"), commands("go 3"));
    assert_eq!(
        commands("go- 3"),
        vec![Command::ExecutorGo {
            target: executor(3),
            direction: GoDirection::Prev,
        }]
    );
    assert_eq!(
        commands("off 3"),
        vec![Command::ExecutorOff {
            target: executor(3)
        }]
    );
    assert_eq!(
        commands("page 4"),
        vec![Command::SetExecutorPage { page: 4 }]
    );
    assert_eq!(commands("clear"), vec![Command::ClearProgrammer]);
}

/// The parser does not validate against the show, because it must not.
#[test]
fn the_parser_does_not_read_the_show() {
    // Fixture 9 is not in the rig, and the daemon refused this very line in the
    // recording. The parser is right and the refusal is the daemon's — that
    // split is **D3**, and it is why a console must not consult the patch.
    assert_eq!(
        commands("9"),
        vec![Command::SelectFixtures {
            ids: vec![FixtureId::new(9)],
            mode: SelectionMode::Set,
        }]
    );
}

/* -------------------------------------------------------------------------- */
/* A line that is not one                                                     */
/* -------------------------------------------------------------------------- */

/// Every refusal names the word that was wrong and shows a line that would work.
///
/// A desk is operated in the dark by somebody with their hands full; "syntax
/// error" is not an answer, and neither is a silence.
#[test]
fn a_line_that_is_not_one_says_what_is_wrong_and_runs_nothing() {
    let cases: &[(&str, &str)] = &[
        ("at", "percentage"),
        ("at 200", "outside 0 to 100"),
        ("at -1", "outside 0 to 100"),
        ("at banana", "not a percentage"),
        ("at 50 60", "more than a level"),
        ("1 thru", "thru what"),
        ("1 thru banana", "not a fixture number"),
        ("1 +", "something is missing"),
        ("banana", "not a fixture number"),
        ("fixture", "not a command"),
        ("go banana", "not something to name"),
        ("go 1 2", "more than a playback takes"),
        ("go- 1 2", "more than a playback takes"),
        ("cue 5", "ambiguous"),
        ("delete", "which one"),
        ("delete banana", "not something to name"),
        ("delete sequence", "sequence which one"),
        ("delete sequence banana", "not a sequence number"),
        ("copy sequence 1", "where"),
        ("copy sequence 1 group 2", "not the same kind of thing"),
        ("edit sequence 1", "edit takes a cue"),
        ("goto sequence 1", "goto takes a cue"),
        ("assign cue 1 executor 1", "assign takes a sequence"),
        ("assign sequence 1 group 1", "assigned to an executor"),
        // S45's half of the verb, and the complaints an operator can act on.
        ("assign executor 1", "assign what on it"),
        (
            "assign executor 1 knob master",
            "not one of an executor's controls",
        ),
        ("assign executor 1 fader", "a fader does what"),
        (
            "assign executor 1 fader sideways",
            "not something a fader does",
        ),
        (
            "assign executor 1 encoder xfade",
            "not something an encoder does",
        ),
        ("assign executor 1 button", "which button"),
        ("assign executor 1 button 5 go+", "which button"),
        ("assign executor 1 button 0 go+", "which button"),
        (
            "assign executor 1 button 1 hologram",
            "not something a button does",
        ),
        ("assign executor 1 button 1 command", "send which line"),
        ("assign executor 1 fader master and then some", "says more"),
        ("store executor 1", "assigned rather than stored"),
        ("on group 1", "not something that plays back"),
        ("page", "which page"),
        ("page banana", "not a page number"),
        ("page 1 2", "more than page takes"),
        ("clear everything", "more than clear takes"),
        ("1 2", "Separate fixtures"),
        // Reading an object: the number, the cue keyword, and the cue number.
        ("copy sequence x cue 1 cue 2", "sequence number"),
        ("copy sequence 5 cue", "cue which one"),
        ("copy sequence 5 cue @ cue 2", "not a cue number"),
        ("goto cue @", "not a cue number"),
        // The verbs, each turned away for its own reason.
        ("edit nonsense 3", "nonsense"),
        ("edit cue 3 and then some", "says more"),
        ("edit group 3", "edit takes a cue"),
        ("goto executor x cue 5", "executor which one"),
        ("goto executor 1 nonsense 5", "nonsense"),
        ("goto executor 1 group 3", "goto takes a cue"),
        ("goto nonsense 5", "nonsense"),
        ("goto group 3", "goto takes a cue"),
        ("delete group 3 and then some", "says more"),
        ("delete nonsense 3", "nonsense"),
        ("copy nonsense 3 sequence 6", "nonsense"),
        ("copy sequence 2", "copy sequence 2 where?"),
        ("copy sequence 2 sequence 6 and then some", "says more"),
        ("copy sequence 2 group 6", "not the same kind"),
        ("label nonsense 3 \"x\"", "nonsense"),
        ("color nonsense 3 red", "nonsense"),
        ("color sequence 4 puce", "is not a colour"),
        ("color sequence 4 #ff88", "is not a colour"),
        ("color sequence 4 red and then some", "says more"),
        ("assign nonsense 5 executor 1", "nonsense"),
        ("assign sequence 5", "assign it where?"),
        ("assign sequence 5 executor 1 and then some", "says more"),
        // A playback, which takes one object and no more.
        (
            "on sequence 5 executor 1",
            "says more than a playback takes",
        ),
        ("on nonsense 5", "nonsense"),
        ("on group 3", "group"),
        // And a bare object, which selects.
        ("preset @", "not a preset number"),
        ("group 3 and then some", "says more"),
        ("+ view 2", "adds to a selection"),
        ("new sequence 5", "new takes a view"),
    ];
    for (line, fragment) in cases {
        assert!(
            message(line)
                .to_lowercase()
                .contains(&fragment.to_lowercase()),
            "\"{line}\" said {:?}, which does not mention {fragment:?}",
            message(line)
        );
    }
    assert!(message(&format!("1 thru {}", MAX_RANGE + 2)).contains(&MAX_RANGE.to_string()));
}

/// An empty line says nothing at all.
#[test]
fn an_empty_line_says_nothing() {
    for line in ["", "   ", "\t\n"] {
        assert_eq!(parse_command_line(line), ConsoleReading::Empty, "{line:?}");
    }
}

/// **The exit criterion.** Ten thousand generated lines, and not one failure.
///
/// The alphabet is deliberately made of the pieces a real line is made of —
/// keywords, digits, separators — plus the things that break parsers: control
/// characters, huge numbers, lone separators. A console that panicked would take
/// the **daemon** down over a typo in the middle of a show, which is a worse
/// outcome than the browser it used to take down.
#[test]
fn it_never_fails_for_any_line_there_is() {
    let alphabet = [
        "1",
        "0",
        "999999999999999999999",
        "-1",
        "1.5",
        "thru",
        "+",
        ",",
        "at",
        "full",
        "out",
        "clear",
        "go",
        "go-",
        "off",
        "page",
        "fixture",
        "pan",
        "dimmer",
        "banana",
        "",
        " ",
        "\u{a0}",
        "\u{0}",
        "🌈",
        "NaN",
        "Infinity",
        "1e400",
        "0x10",
        "01",
        "\"",
        "\"unterminated",
    ];
    // A deterministic generator, so a failure is reproducible: a seeded xorshift
    // rather than anything sampled, which would make a red run unusable.
    let mut seed: u32 = 0x5eed_1234;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for _ in 0..10_000 {
        let count = next() % 7;
        let mut words: Vec<&str> = Vec::new();
        for _ in 0..count {
            words.push(alphabet[(next() as usize) % alphabet.len()]);
        }
        let line = words.join(if next() % 2 == 0 { " " } else { "" });
        let reading = parse_command_line(&line);
        // And whatever it answered can be shown to somebody, and completed
        // against, and read by a pointer.
        let _ = reading_text(&reading);
        let _ = completions(&line);
        let _ = is_verb_line(&line);
        let _ = is_clearing_line(&line);
    }
}

/// A range nobody meant is refused before it is built.
#[test]
fn an_enormous_range_is_refused_rather_than_built() {
    let started = std::time::Instant::now();
    assert!(matches!(
        parse_command_line("1 thru 999999999"),
        ConsoleReading::Refused(_)
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

/* -------------------------------------------------------------------------- */
/* What the line says it will do                                              */
/* -------------------------------------------------------------------------- */

/// A parsed line reads back in words a person can act on.
#[test]
fn a_line_reads_back_in_words() {
    let reading = |line: &str| reading_text(&parse_command_line(line));
    assert_eq!(reading("1 thru 3 at 50"), "select 1 + 2 + 3 · dimmer → 50%");
    assert_eq!(reading("clear"), "clear");
    assert_eq!(reading("go 2"), "go on executor 2");
    assert_eq!(reading("go- 2"), "back on executor 2");
    assert_eq!(reading("off 2"), "off on executor 2");
    assert_eq!(reading("page 2"), "page 2");
    assert_eq!(reading(""), "");
    assert!(reading("banana").contains("not a fixture number"));
}

/// Every one of S40's verbs reads back as something a person can act on.
#[test]
fn s40s_verbs_read_back_too() {
    let reading = |line: &str| reading_text(&parse_command_line(line));
    assert_eq!(reading("group 3"), "select group 3");
    assert_eq!(reading("preset 4"), "apply preset 4");
    assert_eq!(reading("sequence 5"), "select sequence 5");
    assert_eq!(reading("view 2"), "view 2");
    assert_eq!(reading("executor 3"), "select executor 3");
    assert_eq!(reading("store cue 5"), "store cue 5");
    assert_eq!(
        reading("store sequence 5 cue 2"),
        "store cue 2 of sequence 5"
    );
    assert_eq!(reading("store sequence 4"), "store sequence 4");
    assert_eq!(reading("store preset 1"), "store preset 1");
    assert_eq!(reading("store group 3"), "store group 3");
    assert_eq!(reading("store view 2"), "store view 2");
    assert_eq!(reading("new view 2"), "new, empty view 2");
    assert_eq!(reading("edit cue 3"), "edit cue 3");
    assert_eq!(reading("update"), "update the cue being edited");
    assert_eq!(reading("oops"), "oops");
    assert_eq!(reading("goto cue 5"), "goto cue 5 on the selected sequence");
    assert_eq!(reading("goto executor 1 cue 5"), "goto cue 5 on executor 1");
    assert_eq!(reading("delete group 3"), "delete group 3");
    assert_eq!(
        reading("copy sequence 2 sequence 6"),
        "copy sequence 2 to sequence 6"
    );
    assert_eq!(
        reading("move executor 1 executor 5"),
        "move executor 1 to executor 5"
    );
    assert_eq!(
        reading("label view 1 \"Programmer\""),
        "label view 1 \"Programmer\""
    );
    assert_eq!(reading("color sequence 4 red"), "colour sequence 4 #ff0000");
    assert_eq!(
        reading("color executor 1 #f80"),
        "colour executor 1 #ff8800"
    );
    assert_eq!(
        reading("color sequence 4"),
        "take the colour off sequence 4"
    );
    assert_eq!(
        reading("assign sequence 5 executor 1"),
        "assign sequence 5 to executor 1"
    );
    assert_eq!(
        reading("assign executor 1 fader xfade"),
        "set the fader to Crossfade of executor 1"
    );
    assert_eq!(
        reading("assign executor 1 button 3 flash"),
        "set the button 3 to Flash of executor 1"
    );
    assert_eq!(
        reading("assign executor 1 button 4 command \"Go+ Sequence 3\""),
        "set the button 4 to send \"Go+ Sequence 3\" of executor 1"
    );
    assert_eq!(reading("on"), "on the selected sequence");
    assert_eq!(reading("on sequence 2"), "on sequence 2");
    assert_eq!(reading("full"), "dimmer → 100%");
}

/// A command the readout was not written for still says **something**.
///
/// `Command` is the whole protocol and a readout is not where a command added to
/// it should become a compile error.
#[test]
fn the_readout_has_an_arm_for_a_command_it_was_not_written_for() {
    assert_eq!(
        reading_text(&ConsoleReading::Commands {
            commands: vec![Command::SaveShow],
            question: None,
        }),
        "SaveShow"
    );
}

/* -------------------------------------------------------------------------- */
/* The question, and the mode that answers it                                 */
/* -------------------------------------------------------------------------- */

/// A line that would write over something says so, and says which words to ask
/// with.
#[test]
fn a_line_that_may_overwrite_carries_its_question() {
    let question = |line: &str| match parse_command_line(line) {
        ConsoleReading::Commands {
            question: Some(question),
            ..
        } => question,
        other => panic!("\"{line}\" carries no question: {other:?}"),
    };
    let store = question("store cue 5");
    assert_eq!(store.kind, ModeQuestionKind::Store);
    assert_eq!(store.what, "cue 5");
    assert_eq!(
        store.at,
        ObjectRef::Cue {
            sequence_id: None,
            cue_number: "5".to_owned()
        }
    );
    assert_eq!(
        question("store sequence 2").kind,
        ModeQuestionKind::Sequence
    );
    assert_eq!(question("store preset 1").kind, ModeQuestionKind::Store);
    assert_eq!(question("store group 3").kind, ModeQuestionKind::Overwrite);
    assert_eq!(
        question("copy sequence 2 sequence 6").kind,
        ModeQuestionKind::Overwrite
    );
    assert_eq!(
        question("copy view 1 view 3").at,
        ObjectRef::View {
            view_id: ViewId::new(3)
        }
    );
    // **An executor and a view swap rather than overwriting**, so a move has
    // nothing to ask: nothing is lost either way.
    assert!(
        parse_command_line("move executor 1 executor 5")
            .question()
            .is_none()
    );
    assert!(
        parse_command_line("move view 1 view 3")
            .question()
            .is_none()
    );
    // And a store into a view has no mode at all: storing a view *is*
    // overwriting the layout.
    assert!(parse_command_line("store view 2").question().is_none());
    assert!(parse_command_line("1 thru 3 at 50").question().is_none());
}

/// The words each kind of question offers, in the order it offers them.
#[test]
fn each_question_offers_its_own_words() {
    assert_eq!(
        ModeQuestionKind::Store.modes(),
        vec![
            CommandLineMode::Merge,
            CommandLineMode::Override,
            CommandLineMode::Remove
        ]
    );
    assert_eq!(
        ModeQuestionKind::Sequence.modes(),
        vec![
            CommandLineMode::Append,
            CommandLineMode::Override,
            CommandLineMode::Merge
        ]
    );
    assert_eq!(
        ModeQuestionKind::Overwrite.modes(),
        vec![CommandLineMode::Merge, CommandLineMode::Override]
    );
}

/// Three commands carry three different mode types, and each takes only its own
/// words.
///
/// A mode that does not belong to the command is **left alone** rather than
/// forced in: the question is built from the command, so the case cannot arise
/// from an interface, and the type is what stops it arising from anywhere else.
#[test]
fn the_chosen_word_reaches_the_command_that_has_a_mode_for_it() {
    assert_eq!(
        apply_mode(commands("store cue 5"), CommandLineMode::Remove),
        vec![Command::StoreCue {
            sequence_id: None,
            cue_number: "5".to_owned(),
            mode: StoreMode::Remove,
        }]
    );
    // A sequence store takes Append, and not a cue store's Remove.
    let sequence = commands("store sequence 2");
    assert!(matches!(
        apply_mode(sequence.clone(), CommandLineMode::Append).first(),
        Some(Command::StoreSequence {
            mode: SequenceStoreMode::Append,
            ..
        })
    ));
    assert_eq!(
        apply_mode(sequence.clone(), CommandLineMode::Remove),
        sequence
    );

    // A copy takes Merge and Override, and not Append.
    let copy = commands("copy sequence 2 sequence 6");
    assert!(matches!(
        apply_mode(copy.clone(), CommandLineMode::Override).first(),
        Some(Command::Copy {
            mode: OverwriteMode::Override,
            ..
        })
    ));
    assert_eq!(apply_mode(copy.clone(), CommandLineMode::Append), copy);

    // A group store and a move take the same two.
    assert!(matches!(
        apply_mode(commands("store group 3"), CommandLineMode::Override).first(),
        Some(Command::StoreGroup {
            mode: OverwriteMode::Override,
            ..
        })
    ));
    assert!(matches!(
        apply_mode(commands("move cue 2 cue 8"), CommandLineMode::Override).first(),
        Some(Command::Move {
            mode: OverwriteMode::Override,
            ..
        })
    ));
    // A preset store carries a `StoreMode` like a cue's.
    assert!(matches!(
        apply_mode(commands("store preset 1"), CommandLineMode::Remove).first(),
        Some(Command::StorePreset {
            mode: StoreMode::Remove,
            ..
        })
    ));

    // And a command with no mode at all is handed back untouched.
    assert_eq!(
        apply_mode(vec![Command::ClearProgrammer], CommandLineMode::Merge),
        vec![Command::ClearProgrammer]
    );
}

/* -------------------------------------------------------------------------- */
/* The tables a pointer reads                                                 */
/* -------------------------------------------------------------------------- */

/// **Every word the console knows is either a verb or an argument** — the check
/// that stops a verb entering the grammar without entering the table a pointer
/// reads.
///
/// A list beside the `match` rather than one derived from it, because a `match`
/// is not a value; this walks both and asserts they agree.
#[test]
fn the_verb_table_partitions_every_word_the_console_knows() {
    for word in CONSOLE_WORDS {
        // Every console word is either a head word of the grammar's own `match`
        // or a noun one of them takes.
        let verb = VERB_WORDS.contains(&word);
        let argument = [
            "at", "cue", "executor", "fixture", "group", "preset", "sequence", "thru", "view",
        ]
        .contains(&word);
        assert!(verb ^ argument, "{word} is neither a verb nor an argument");
    }
    // The two clearing verbs are verbs.
    for word in CLEARING_VERBS {
        assert!(VERB_WORDS.contains(&word), "{word} is not a verb");
    }
}

/// The two clearing verbs are exactly the ones whose missing argument means
/// *take it away*.
#[test]
fn a_clearing_verb_is_one_whose_absent_argument_is_an_instruction() {
    // Both parse without a trailing word, and both take something away.
    assert_eq!(
        commands("label view 1"),
        vec![Command::Label {
            target: ObjectRef::View {
                view_id: ViewId::new(1)
            },
            name: String::new(),
        }]
    );
    assert_eq!(
        commands("color sequence 4"),
        vec![Command::Color {
            target: ObjectRef::Sequence {
                sequence_id: SequenceId::new(4)
            },
            color: None,
        }]
    );
    for word in CLEARING_VERBS {
        assert!(is_clearing_line(word), "{word}");
        assert!(is_verb_line(word), "{word}");
    }
    // And nothing else is one.
    for word in VERB_WORDS {
        assert_eq!(
            is_clearing_line(word),
            CLEARING_VERBS.contains(&word),
            "{word}"
        );
    }
}

/// A line that does **not** begin with a verb is a fixture selection, and a
/// pointer does its own thing on one.
#[test]
fn a_line_that_is_a_selection_is_not_a_verb_line() {
    for line in ["", "1 thru", "1 + 2", "5", "group 3", "at 50"] {
        assert!(!is_verb_line(line), "{line:?}");
    }
    for line in ["Store", "store sequence 2", "GO+ executor 1", "Delete "] {
        assert!(is_verb_line(line), "{line:?}");
    }
}

/// Completion is a **grammar** answer and not a show answer.
#[test]
fn completion_offers_words_and_never_numbers() {
    assert_eq!(completions("st"), vec!["Store"]);
    assert!(completions("store ").contains(&"Sequence".to_owned()));
    assert!(completions("store ").contains(&"Cue".to_owned()));
    // The one verb that takes a view and nothing else.
    assert_eq!(completions("new "), vec!["View"]);
    // A colour is the one last argument that is a word.
    assert!(completions("color sequence 4 ").contains(&"Red".to_owned()));
    assert!(completions("color ").contains(&"Sequence".to_owned()));
    // After a number, the words a selection can continue with.
    assert_eq!(completions("1 "), vec!["At", "Thru", "Full"]);
    // Nothing to offer, rather than everything.
    assert!(completions("banana ").is_empty());
    // The word already typed is not offered back.
    assert!(!completions("store").contains(&"Store".to_owned()));
}

/* -------------------------------------------------------------------------- */
/* The names a store carries, and the shapes an answer is built from           */
/* -------------------------------------------------------------------------- */

/// A store carries the name the line ends with, and a default when it does not.
///
/// Five destinations and five rules, and they differ: a cue and a group take the
/// name as typed and nothing when it is absent, a sequence and a view invent one
/// so a pool has something to draw, and a preset may name its **pool** first —
/// in which case the pool word is not part of the name.
#[test]
fn a_store_carries_the_name_the_line_ends_with() {
    assert_eq!(
        commands("store sequence 4 \"Act one\""),
        vec![Command::StoreSequence {
            sequence_id: SequenceId::new(4),
            name: "Act one".to_owned(),
            mode: SequenceStoreMode::Append,
        }]
    );
    // A sequence with no name is called after its number, so a sheet has
    // something to draw rather than a blank tile.
    assert!(matches!(
        commands("store sequence 4").first(),
        Some(Command::StoreSequence { name, .. }) if name == "Sequence 4"
    ));
    assert!(matches!(
        commands("store view 2 \"Programmer\"").first(),
        Some(Command::StoreView { name, .. }) if name == "Programmer"
    ));
    assert!(matches!(
        commands("store view 2").first(),
        Some(Command::StoreView { name, .. }) if name == "View 2"
    ));
    assert!(matches!(
        commands("new view 5 \"Busking\"").first(),
        Some(Command::NewView { name, .. }) if name == "Busking"
    ));
    assert!(matches!(
        commands("store group 3 \"Front wash\"").first(),
        Some(Command::StoreGroup { name, .. }) if name == "Front wash"
    ));
    // **The pool may be named, and then it is not the name** — S43. A colour
    // preset called *Deep blue* is `Store Preset 1 Color "Deep blue"`, and the
    // pool an operator did not name is the daemon's `Session::encoderBank`.
    assert!(matches!(
        commands("store preset 1 color \"Deep blue\"").first(),
        Some(Command::StorePreset { pool: Some(PresetPool::Color), name, color: None, .. })
            if name == "Deep blue"
    ));
    assert!(matches!(
        commands("store preset 1 \"Warm\"").first(),
        Some(Command::StorePreset { pool: None, name, .. }) if name == "Warm"
    ));
    // `Multi` is a word an operator can type and is not an encoder bank, which
    // is exactly why a line that names no pool can never mean it.
    assert!(matches!(
        commands("store preset 1 multi \"The look\"").first(),
        Some(Command::StorePreset {
            pool: Some(PresetPool::Multi),
            ..
        })
    ));
}

/// A cue number may carry a point, which is what puts 1.5 between 1 and 2.
#[test]
fn a_cue_number_may_be_fractional() {
    assert_eq!(
        commands("goto cue 1.5"),
        vec![Command::Goto {
            target: PlaybackTarget::Selected,
            cue_number: "1.5".to_owned(),
        }]
    );
    // A point with nothing after it is not a number.
    assert!(message("goto cue 1.").contains("not a cue number"));
}

/// **A leading `+` adds to the selection** — S43, punch-list B17, and it reaches
/// a group as well as a fixture.
#[test]
fn a_leading_plus_adds_rather_than_replacing() {
    assert_eq!(
        commands("+ 5"),
        vec![Command::SelectFixtures {
            ids: vec![FixtureId::new(5)],
            mode: SelectionMode::Toggle,
        }]
    );
    assert_eq!(commands("+ fixture 5"), commands("+ 5"));
    assert_eq!(
        commands("+ group 3"),
        vec![Command::SelectGroup {
            group_id: GroupId::new(3),
            mode: SelectionMode::Toggle,
        }]
    );
    // And what it reads as, which is the other half of the same word.
    assert_eq!(
        reading_text(&parse_command_line("+ 5")),
        "add 5 to the selection"
    );
    assert_eq!(
        reading_text(&parse_command_line("+ group 3")),
        "add group 3 to the selection"
    );
}

/// A cue on its own is ambiguous **whether or not it names its list**.
#[test]
fn a_cue_on_its_own_is_ambiguous_either_way() {
    assert!(message("cue 5").contains("ambiguous"));
    assert!(message("sequence 5 cue 3").contains("ambiguous"));
}

/// An executor's control takes one function and no more.
#[test]
fn a_control_takes_one_function_and_no_more() {
    assert!(message("assign executor 1 fader master and then some").contains("says more"));
    assert!(message("assign executor 1 encoder speed and then some").contains("says more"));
    assert!(message("assign executor 1 button 1 go+ and then some").contains("says more"));
}

/// Every function of every control reads back in words a person can act on.
///
/// A loop rather than three assertions, because what is being checked is that
/// **no function has no name**: a chooser that offered a word the readout could
/// not spell would be a line an operator cannot check before Enter.
#[test]
fn every_control_function_reads_back_in_words() {
    let reading = |line: &str| reading_text(&parse_command_line(line));
    for (word, name) in [
        ("empty", "Nothing"),
        ("master", "Master"),
        ("speed", "Speed"),
        ("xfade", "Crossfade"),
    ] {
        assert_eq!(
            reading(&format!("assign executor 1 fader {word}")),
            format!("set the fader to {name} of executor 1"),
        );
    }
    for (word, name) in [
        ("empty", "Nothing"),
        ("master", "Master"),
        ("speed", "Speed"),
    ] {
        assert_eq!(
            reading(&format!("assign executor 1 encoder {word}")),
            format!("set the encoder to {name} of executor 1"),
        );
    }
    for (word, name) in [
        ("empty", "Nothing"),
        ("go+", "Go forward"),
        ("go-", "Go back"),
        ("learnspeed", "Learn speed"),
        ("off", "Off"),
        ("on", "On"),
        ("flash", "Flash"),
        ("toggle", "Toggle"),
    ] {
        assert_eq!(
            reading(&format!("assign executor 2 button 2 {word}")),
            format!("set the button 2 to {name} of executor 2"),
        );
    }
}

/// A command the parser cannot produce still reads back as something.
///
/// `AssignExecutor` with no sequence is *take the list off that fader*: no line
/// says it today and the protocol has it, and a readout that fell silent on a
/// command `Command` carries would be a readout that cannot be trusted.
#[test]
fn a_command_with_no_line_of_its_own_still_reads_back() {
    assert_eq!(
        reading_text(&ConsoleReading::Commands {
            commands: vec![Command::AssignExecutor {
                executor_id: ExecutorId::new(1),
                sequence_id: None,
            }],
            question: None,
        }),
        "assign nothing to executor 1"
    );
}

/// The three shapes a reading has, as the wire carries them.
///
/// `prismd` builds `Answer::CommandLineReading` out of these four accessors, so
/// they are asserted here rather than only through a daemon: a `kind` that said
/// *Commands* for a refusal would draw a green box over a complaint.
#[test]
fn a_reading_says_which_shape_it_is() {
    use prism_domain::CommandLineReadingKind as Kind;
    let empty = parse_command_line("");
    assert_eq!(empty.kind(), Kind::Empty);
    assert!(empty.commands().is_empty());
    assert!(empty.question().is_none());

    let refused = parse_command_line("banana");
    assert_eq!(refused.kind(), Kind::Error);
    assert!(refused.commands().is_empty());
    assert!(refused.question().is_none());

    let two = parse_command_line("1 thru 3 at 50");
    assert_eq!(two.kind(), Kind::Commands);
    assert_eq!(two.commands().len(), 2);
    assert!(two.question().is_none());
}

/// The wire form of a question is the words and what they are about — and
/// **not** where to look, which is the daemon's to know.
#[test]
fn a_question_travels_as_its_words_and_not_as_a_reference() {
    let question = parse_command_line("store cue 5")
        .question()
        .expect("a store carries a question")
        .question();
    assert_eq!(question.what, "cue 5");
    assert_eq!(
        question.modes,
        vec![
            CommandLineMode::Merge,
            CommandLineMode::Override,
            CommandLineMode::Remove
        ]
    );
}

/// A line with a character whose case folding is not one-for-one still
/// tokenises, and the keypad rewrites still fire around it.
///
/// `İ` lower-cases to two `char`s, which is why the tokeniser folds ASCII only:
/// a fully folded copy would be longer than the line and every offset in it
/// would be wrong.
#[test]
fn a_line_with_an_awkward_capital_still_tokenises() {
    assert!(matches!(
        parse_command_line("İ thru 3"),
        ConsoleReading::Refused(_)
    ));
    assert_eq!(commands("1THRU3"), commands("1 thru 3"));
    assert_eq!(commands("GO+ EXECUTOR 1"), commands("go+ executor 1"));
}

/// Every mode word reaches exactly the commands whose mode type has it.
///
/// Four words and six commands, walked as a grid rather than as three examples:
/// what makes `CommandLineMode` safe is that a word which does not fit is left
/// alone, and *left alone* is only a claim if the misses are checked too.
#[test]
fn each_mode_word_reaches_only_the_commands_that_have_it() {
    let cases: &[(&str, CommandLineMode, &str)] = &[
        ("store cue 5", CommandLineMode::Merge, "Merge"),
        ("store cue 5", CommandLineMode::Override, "Override"),
        ("store cue 5", CommandLineMode::Remove, "Remove"),
        ("store preset 1", CommandLineMode::Override, "Override"),
        ("store sequence 2", CommandLineMode::Append, "Append"),
        ("store sequence 2", CommandLineMode::Override, "Override"),
        ("store sequence 2", CommandLineMode::Merge, "Merge"),
        ("store group 3", CommandLineMode::Merge, "Merge"),
        ("store group 3", CommandLineMode::Override, "Override"),
        (
            "copy sequence 2 sequence 6",
            CommandLineMode::Merge,
            "Merge",
        ),
        ("move cue 2 cue 8", CommandLineMode::Override, "Override"),
    ];
    for (line, word, wanted) in cases {
        let applied = apply_mode(commands(line), *word);
        let json = serde_json::to_value(&applied[0]).expect("a command serialises");
        assert_eq!(json["mode"], *wanted, "{line} with {word:?}");
    }
    // And the misses: a word a command's mode type has not got leaves it alone.
    for (line, word) in [
        ("store cue 5", CommandLineMode::Append),
        ("store preset 1", CommandLineMode::Append),
        ("store sequence 2", CommandLineMode::Remove),
        ("store group 3", CommandLineMode::Remove),
        ("store group 3", CommandLineMode::Append),
        ("copy sequence 2 sequence 6", CommandLineMode::Remove),
        ("move cue 2 cue 8", CommandLineMode::Append),
    ] {
        assert_eq!(apply_mode(commands(line), word), commands(line), "{line}");
    }
}
