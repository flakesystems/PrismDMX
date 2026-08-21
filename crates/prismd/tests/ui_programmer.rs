//! What the executor bar, the encoder bar and the command line do to a desk —
//! recorded off a running daemon, so the browser is held to the daemon's
//! answers rather than to a second opinion in TypeScript.
//!
//! # Why a third recording
//!
//! `ui_recording.rs` records a *conversation* (S23) and `ui_session.rs` records
//! the answers a **canvas** asks (S25). This one records the answers the three
//! bars of S26 ask, which are of a different kind: what is on the eight strips
//! of the current page, what the programmer holds after an encoder was turned,
//! and which parameter the bank and the index name.
//!
//! Every one of those is a *reader* over a document, and a reader tested against
//! a document the same language built is a test of nothing. So the expectations
//! here are `prism_core`'s own, taken off the wire.
//!
//! # The command line is recorded as well, and that is the point of `typed`
//!
//! A console's command line is a **parser in the client**: `1 + 2 at 50` is not
//! a command, it is two of them. The parser could therefore be tested entirely
//! against itself, which is exactly the trap the sessions before this one each
//! found in their own layer. So every line an operator types appears in the
//! script beside the commands it has to produce, and those commands are the ones
//! a real daemon accepted and acted on. A line that produces two commands
//! appears as two consecutive steps carrying the same `typedLine` **number**;
//! the browser groups by that number and compares its parser's answer with the
//! group. The number rather than the text, because `clear` is pressed three
//! times in a row and those are three lines.
//!
//! # It is frozen, and the guards run on every commit
//!
//! [`record_the_desk_script_for_the_interface`] is `#[ignore]`d, like the other
//! three fixtures' regenerators. What runs always replays the deltas through
//! `prism_core`'s mirrors and checks every recorded answer — so a shape that
//! moved fails here, in Rust, rather than going stale in `ui/`.

// This target *writes files for a person to commit* and says where.
#![allow(clippy::print_stdout)]
// Every test holds `common::one_daemon_at_a_time` across its awaits.
#![allow(clippy::await_holding_lock)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use prism_core::{SessionMirror, Show, ShowFile, ShowMirror, ShowStore};
use prism_domain::{
    AttributeDef, AttributeType, Command, Executor, ExecutorButtonFunction,
    ExecutorEncoderFunction, ExecutorFaderFunction, ExecutorId, FeatureGroup, Fixture, FixtureId,
    FixtureType, GoDirection, GroupId, JsonValue, MergeMode, ObjectRef, OverwriteMode,
    PlaybackTarget, PresetId, ProgrammerState, SelectionMode, Sequence, SequenceId,
    SequenceStoreMode, StoreMode, UniverseId, Vec3,
};
use prism_domain::{Cue, CuePart, CueTrigger};
use prism_ipc::{ClientKind, ClientMessage, Hello, ServerMessage, Snapshot, Wire, local};
use prismd::cli::{Options, OutputSpec};
use prismd::daemon::Daemon;

mod common;

/// How long any wait here may take before it is a named failure.
const PATIENCE: Duration = Duration::from_secs(20);

/// Where the interface reads the recording from.
fn recording_path() -> PathBuf {
    common::ui_fixture("desk-recording.json")
}

/// Where the interface's end-to-end suite reads the rig from.
fn rig_path() -> PathBuf {
    common::ui_fixture("desk-rig.prism")
}

/* -------------------------------------------------------------------------- */
/* The rig                                                                    */
/* -------------------------------------------------------------------------- */

/// An 8-bit attribute at an offset, dark at home.
fn attribute(attribute: AttributeType, coarse_offset: u16) -> AttributeDef {
    AttributeDef {
        attribute,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value: 0,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

/// The rig this session's tests use, and the reason it is not `common`'s.
///
/// Two things `common::show_file` cannot give S26. **Everything is dark at
/// home**, so a level in the telemetry picture is one the operator put there —
/// `common`'s first dimmer sits at full, which would make "the encoder reached
/// the output" unmeasurable. And there is a fixture with attributes on **four
/// different encoder banks**, so an encoder bar has something to show on more
/// than one of them and a value set on one bank can be seen not to have moved
/// when another is selected.
fn desk_show() -> ShowFile {
    let mut show = Show::new();
    show.embed_fixture_type(FixtureType {
        id: "generic.dimmer.dark".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "1ch".to_owned(),
        footprint: 1,
        attributes: vec![attribute(AttributeType::Dimmer, 0)],
    })
    .expect("a one-channel dimmer is a fixture type");
    show.embed_fixture_type(FixtureType {
        id: "generic.movinghead".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Moving Head".to_owned(),
        mode: "7ch".to_owned(),
        footprint: 7,
        attributes: vec![
            attribute(AttributeType::Dimmer, 0),
            attribute(AttributeType::Pan, 1),
            attribute(AttributeType::Tilt, 2),
            attribute(AttributeType::Red, 3),
            attribute(AttributeType::Green, 4),
            attribute(AttributeType::Blue, 5),
            AttributeDef {
                merge_mode: MergeMode::Ltp,
                ..attribute(AttributeType::Zoom, 6)
            },
        ],
    })
    .expect("a moving head is a fixture type");

    for (id, address) in [(1_u32, 1_u16), (2, 2), (3, 3), (4, 4)] {
        show.patch_fixture(Fixture {
            id: FixtureId::new(id),
            name: format!("Dimmer {id}"),
            type_id: "generic.dimmer.dark".to_owned(),
            universe: UniverseId::new(1),
            address,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
        })
        .expect("the address is free");
    }
    show.patch_fixture(Fixture {
        id: FixtureId::new(5),
        name: "Head 5".to_owned(),
        type_id: "generic.movinghead".to_owned(),
        universe: UniverseId::new(1),
        address: 11,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        invert_pan: false,
        invert_tilt: false,
    })
    .expect("the address is free");

    for (id, name) in [(1_u32, "Warm Wash"), (2, "Cold Wash")] {
        show.store_sequence(Sequence {
            id: SequenceId::new(id),
            name: name.to_owned(),
            cues: vec![Cue {
                number: "1".to_owned(),
                name: "Cue 1".to_owned(),
                fade_in: 0.0,
                fade_out: 0.0,
                delay: 0.0,
                trigger: CueTrigger::Go,
                trigger_time: None,
                parts: vec![CuePart {
                    fixture: FixtureId::new(id),
                    attribute: AttributeType::Dimmer,
                    value: 65535,
                    preset_ref: None,
                }],
            }],
            looping: false,
            is_active: false,
            current_cue_index: None,
        })
        .expect("a sequence with one cue");
    }

    // Page 0 holds two executors and page 1 holds one, with different function
    // assignments — so an executor bar that showed the wrong page, or drew every
    // strip the same, could not pass.
    for (id, sequence, fader, buttons, encoder, level) in [
        (
            0_u32,
            Some(1_u32),
            ExecutorFaderFunction::Master,
            vec![
                ExecutorButtonFunction::GoForward,
                ExecutorButtonFunction::GoBack,
                ExecutorButtonFunction::Off,
                ExecutorButtonFunction::Empty,
            ],
            ExecutorEncoderFunction::Master,
            65535_u16,
        ),
        (
            2,
            Some(2),
            ExecutorFaderFunction::XFade,
            vec![
                ExecutorButtonFunction::Flash,
                ExecutorButtonFunction::Toggle,
                ExecutorButtonFunction::On,
                ExecutorButtonFunction::LearnSpeed,
            ],
            ExecutorEncoderFunction::Speed,
            0,
        ),
        (
            9,
            None,
            ExecutorFaderFunction::Empty,
            Vec::new(),
            ExecutorEncoderFunction::Empty,
            32768,
        ),
    ] {
        show.store_executor(Executor {
            id: ExecutorId::new(id),
            sequence_id: sequence.map(SequenceId::new),
            fader_function: fader,
            button_functions: buttons,
            encoder_function: encoder,
            master_level: level,
            speed: prism_domain::SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
        })
        .expect("the sequence exists");
    }

    ShowFile {
        show,
        ..ShowFile::new()
    }
}

/// Writes the rig into a `.prism` file for the daemon and the browser to open.
fn write_rig(path: &Path) {
    let mut file = desk_show();
    let mut store = ShowStore::open(path).expect("a show file opens");
    store.save(&mut file).expect("a show file saves");
}

/* -------------------------------------------------------------------------- */
/* The file                                                                   */
/* -------------------------------------------------------------------------- */

/// One strip of the executor bar, as a reader has to produce it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedStrip {
    /// Which of the eight it is.
    slot: u32,
    /// `page * 8 + slot` — D7's arithmetic, which the interface must not get
    /// wrong when the page moves.
    executor_id: u32,
    /// Whether the show has an executor in that slot at all.
    assigned: bool,
    /// The sequence's name, which is what an operator named. `null` for a slot
    /// with no sequence — the bar shows the number instead.
    name: Option<String>,
    /// `0..=65535`.
    master_level: u16,
    /// Whether it is running.
    is_active: bool,
    /// Which cue it is in, if any.
    current_cue_index: Option<u32>,
    /// What its fader does.
    fader_function: String,
    /// What its four buttons do, in hardware order: Rec, Solo, Mute, Select.
    button_functions: Vec<String>,
}

/// The programmer, flattened the way it travels.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedProgrammer {
    selection: Vec<u32>,
    active_feature_group: String,
    /// Fixture, attribute, value, source — in the order the wire carries them.
    values: Vec<(u32, String, u16, String)>,
    clear_stage: u8,
    /// The encoder banks the programmer is holding a value on, in bank order —
    /// `prism_core::Programmer::feature_groups`, which that type names as an
    /// **S26 requirement**: an operator looking at the Position bank can see
    /// that they have also touched colour.
    ///
    /// The group is the one the *profile* files the attribute under, not the
    /// one its name suggests, which is why this is the daemon's answer and not
    /// a filter over the attribute names.
    touched_banks: Vec<String>,
}

/// The six session fields the bars read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedSession {
    executor_page: u32,
    selected_executor: Option<u32>,
    encoder_bank: String,
    programmer_page: u32,
    programmer_param_index: u32,
    command_line: String,
}

/// One command, and the desk it left behind.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    /// What it is, for a person reading the file.
    what: String,
    /// The line an operator typed to produce it, when it came from the command
    /// line.
    typed: Option<String>,
    /// Which typed line it belongs to, counting from 0.
    ///
    /// Numbered rather than grouped by text, because `clear` is pressed three
    /// times in a row and three presses of a three-stage button are three
    /// lines. A browser that grouped by equal text would read them as one.
    typed_line: Option<u32>,
    /// The `ClientMessage::Command` payload, base64.
    client: String,
    /// The deltas the daemon answered with, in order, base64.
    deltas: Vec<String>,
    /// Whether the daemon refused it.
    refused: bool,
    /// The eight strips of the current page afterwards — **the daemon's answer**.
    strips: Vec<RecordedStrip>,
    /// The programmer afterwards.
    programmer: RecordedProgrammer,
    /// The session fields the bars read, afterwards.
    session: RecordedSession,
}

/// The whole file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recording {
    /// What made it, and how to make it again.
    note: String,
    /// The protocol version these payloads belong to.
    protocol_version: u32,
    /// Executors per page — **D7**, recorded so an interface that assumed a
    /// different number would fail here rather than address the wrong executor.
    executors_per_page: u32,
    /// The attributes of each encoder bank, in the order the jog wheel walks
    /// them (`prismd::surface::parameter_of`). The interface has the same table
    /// generated into `ui/src/bindings/variants.ts`; recording it means the two
    /// are compared rather than merely both derived.
    encoder_banks: BTreeMap<String, Vec<String>>,
    /// The snapshot the script starts from, base64.
    initial_snapshot: String,
    /// The script.
    steps: Vec<Step>,
    /// What a fresh client is served at the end.
    final_snapshot: String,
}

/* -------------------------------------------------------------------------- */
/* The script                                                                 */
/* -------------------------------------------------------------------------- */

/// A step of the script: what it is for, the line that produced it and that
/// line's number, and the command.
type Scripted = (&'static str, Option<(u32, &'static str)>, Command);

/// The commands, in order, with what each one is for.
///
/// Written out rather than generated: this is a **desk being operated** — the
/// bank paged, a master moved, a sequence started and stopped, an encoder bank
/// switched, fixtures selected and levels set from the command line, the wheel
/// moved on to the next parameter, and the three-stage Clear walked all the way
/// round — and every step is one a reader or a parser could get wrong.
fn script() -> Vec<Scripted> {
    vec![
        (
            "page the fader bank up: the eight strips are a different eight",
            Some((0, "page 1")),
            Command::SetExecutorPage { page: 1 },
        ),
        (
            "a page with nothing on it, which is an ordinary page and not an error",
            Some((1, "page 2")),
            Command::SetExecutorPage { page: 2 },
        ),
        (
            "and back down, which is where the executors are",
            Some((2, "page 0")),
            Command::SetExecutorPage { page: 0 },
        ),
        (
            "select the executor the main fader and the transport act on",
            None,
            Command::SelectExecutor {
                executor_id: ExecutorId::new(2),
            },
        ),
        (
            "pull a master to half: the level is the daemon's, not the pointer's",
            None,
            Command::SetExecutorMaster {
                executor_id: ExecutorId::new(0),
                level: 32768,
            },
        ),
        (
            "Go on executor 0: it runs and takes its first cue",
            Some((3, "go 0")),
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
                direction: GoDirection::Next,
            },
        ),
        (
            "Off again: it stops and the cue index goes",
            Some((4, "off 0")),
            Command::ExecutorOff {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            },
        ),
        (
            "Go on an executor that has no sequence: refused, nothing moves",
            None,
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(9)),
                direction: GoDirection::Next,
            },
        ),
        (
            "switch the encoder bank to Position",
            None,
            Command::SetEncoderBank {
                group: FeatureGroup::Position,
            },
        ),
        (
            "select two fixtures from the command line",
            Some((5, "1 + 2")),
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1), FixtureId::new(2)],
                mode: SelectionMode::Set,
            },
        ),
        (
            "a range: `thru` is inclusive at both ends",
            Some((6, "1 thru 3 at 50")),
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1), FixtureId::new(2), FixtureId::new(3)],
                mode: SelectionMode::Set,
            },
        ),
        (
            "and the level the same line asked for: one line, two commands",
            Some((6, "1 thru 3 at 50")),
            Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                value: 32767,
                relative: false,
            },
        ),
        (
            "a fixture the show has not got: the parser is right and the daemon refuses",
            Some((8, "9")),
            Command::SelectFixtures {
                ids: vec![FixtureId::new(9)],
                mode: SelectionMode::Set,
            },
        ),
        (
            "the moving head, and a parameter that is not a dimmer",
            Some((9, "5 pan at 25")),
            Command::SelectFixtures {
                ids: vec![FixtureId::new(5)],
                mode: SelectionMode::Set,
            },
        ),
        (
            "pan a quarter of the way over: the bank follows the attribute",
            Some((9, "5 pan at 25")),
            Command::SetAttribute {
                attribute: AttributeType::Pan,
                value: 16383,
                relative: false,
            },
        ),
        (
            "an encoder turned: relative, and it starts from what the programmer holds",
            None,
            Command::SetAttribute {
                attribute: AttributeType::Pan,
                value: 655,
                relative: true,
            },
        ),
        (
            "the same encoder the other way, past zero: it saturates rather than wrapping",
            None,
            Command::SetAttribute {
                attribute: AttributeType::Pan,
                value: -60000,
                relative: true,
            },
        ),
        (
            "the wheel moves on to the next parameter of the bank",
            None,
            Command::SelectProgrammerParam {
                direction: prism_domain::ParamDirection::Next,
            },
        ),
        (
            "an absolute value out of range: refused, and the programmer stands still",
            None,
            Command::SetAttribute {
                attribute: AttributeType::Tilt,
                value: 70000,
                relative: false,
            },
        ),
        (
            "the line as it is being typed, which every client sees",
            None,
            Command::CommandLineInput {
                text: "1 thru 4 at ".to_owned(),
            },
        ),
        (
            "Clear once: the values go and the selection stays",
            Some((11, "clear")),
            Command::ClearProgrammer,
        ),
        (
            "Clear twice: the selection goes",
            Some((12, "clear")),
            Command::ClearProgrammer,
        ),
        (
            "Clear three times: everything, including the bank",
            Some((13, "clear")),
            Command::ClearProgrammer,
        ),
        /* ------------------------------------------------------------------ */
        /* S40: the vocabulary, typed                                         */
        /* ------------------------------------------------------------------ */
        //
        // Every line below is one an operator could have typed, and every one of
        // them is here so `ui/src/desk/console.test.ts` can hold the parser to
        // the command a **running daemon** was sent for it. Nothing in
        // TypeScript decides what a line ought to mean.
        (
            "an argument keyword and a number: the cue list a store goes into",
            Some((14, "sequence 2")),
            Command::SelectSequence {
                sequence_id: SequenceId::new(2),
            },
        ),
        (
            "a selection, so there is something to store as a group",
            Some((15, "1 thru 3")),
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1), FixtureId::new(2), FixtureId::new(3)],
                mode: SelectionMode::Set,
            },
        ),
        (
            "store the selection as a group: the command `Show::store_group` \
             waited for since S11",
            Some((16, "store group 1 \"Front wash\"")),
            Command::StoreGroup {
                group_id: GroupId::new(1),
                name: "Front wash".to_owned(),
                mode: OverwriteMode::Merge,
            },
        ),
        (
            "and select it back: the daemon expands the group, never a client",
            Some((17, "group 1")),
            Command::SelectGroup {
                group_id: GroupId::new(1),
                mode: SelectionMode::Set,
            },
        ),
        (
            "name it: one verb for all six pools",
            Some((18, "label group 1 \"Front\"")),
            Command::Label {
                target: ObjectRef::Group {
                    group_id: GroupId::new(1),
                },
                name: "Front".to_owned(),
            },
        ),
        (
            "copy it onto a free number",
            Some((19, "copy group 1 group 2")),
            Command::Copy {
                from: ObjectRef::Group {
                    group_id: GroupId::new(1),
                },
                to: ObjectRef::Group {
                    group_id: GroupId::new(2),
                },
                mode: OverwriteMode::Merge,
            },
        ),
        (
            "and take it away again",
            Some((20, "delete group 2")),
            Command::Delete {
                target: ObjectRef::Group {
                    group_id: GroupId::new(2),
                },
            },
        ),
        (
            "a level, so the programmer holds something to store",
            Some((21, "1 thru 3 at 60")),
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1), FixtureId::new(2), FixtureId::new(3)],
                mode: SelectionMode::Set,
            },
        ),
        (
            "the second half of that line, which is why a line is not a command",
            Some((21, "1 thru 3 at 60")),
            Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                value: 39321,
                relative: false,
            },
        ),
        (
            "store into a cue of the **selected** sequence, which the line does \
             not name",
            Some((22, "store cue 2")),
            Command::StoreCue {
                sequence_id: None,
                cue_number: "2".to_owned(),
                mode: StoreMode::Merge,
            },
        ),
        (
            "load it back into the programmer",
            Some((23, "edit cue 2")),
            Command::EditCue {
                sequence_id: None,
                cue_number: "2".to_owned(),
            },
        ),
        (
            "and put it down again, which is what the Update key does",
            Some((24, "update")),
            Command::Update,
        ),
        (
            "name the cue: `CueProperty` lost its name in S40 and this is where \
             it went",
            Some((25, "label cue 2 \"Blue wash\"")),
            Command::Label {
                target: ObjectRef::Cue {
                    sequence_id: None,
                    cue_number: "2".to_owned(),
                },
                name: "Blue wash".to_owned(),
            },
        ),
        (
            "renumber it, which is a move onto a free number",
            Some((26, "move cue 2 cue 3")),
            Command::Move {
                from: ObjectRef::Cue {
                    sequence_id: None,
                    cue_number: "2".to_owned(),
                },
                to: ObjectRef::Cue {
                    sequence_id: None,
                    cue_number: "3".to_owned(),
                },
                mode: OverwriteMode::Merge,
            },
        ),
        (
            "jump the selected cue list straight to a cue: the command that had \
             no message at any layer before S40",
            Some((27, "goto cue 1")),
            Command::Goto {
                target: PlaybackTarget::Selected,
                cue_number: "1".to_owned(),
            },
        ),
        (
            "start a cue list by name rather than by fader",
            Some((28, "on sequence 1")),
            Command::ExecutorOn {
                target: PlaybackTarget::of_sequence(SequenceId::new(1)),
            },
        ),
        (
            "and stop it the same way",
            Some((29, "off sequence 1")),
            Command::ExecutorOff {
                target: PlaybackTarget::of_sequence(SequenceId::new(1)),
            },
        ),
        (
            "a Go addressed to an executor, which is S26's form with S40's words",
            Some((30, "go+ executor 0")),
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
                direction: GoDirection::Next,
            },
        ),
        (
            "store into a cue list nobody has made: `Store Sequence 4` is both \
             acts since S40",
            Some((31, "store sequence 4")),
            Command::StoreSequence {
                sequence_id: SequenceId::new(4),
                name: "Sequence 4".to_owned(),
                mode: SequenceStoreMode::Append,
            },
        ),
        (
            "**and play it, on no fader at all** — the hole S40 found in the \
             playback model and filled",
            Some((32, "on sequence 4")),
            Command::ExecutorOn {
                target: PlaybackTarget::of_sequence(SequenceId::new(4)),
            },
        ),
        (
            "put it on a fader, and the executor's playback is the one that runs",
            Some((33, "assign sequence 4 executor 9")),
            Command::AssignExecutor {
                executor_id: ExecutorId::new(9),
                sequence_id: Some(SequenceId::new(4)),
            },
        ),
        (
            "empty the slot: the row goes and the *place* stays, because a place \
             is arithmetic (D7)",
            Some((34, "delete executor 9")),
            Command::Delete {
                target: ObjectRef::Executor {
                    executor_id: ExecutorId::new(9),
                },
            },
        ),
        (
            "the bank a preset store goes into, set before the store so the \
             recording says which one it was",
            None,
            Command::SetEncoderBank {
                group: FeatureGroup::Dimmer,
            },
        ),
        (
            "store a preset with no pool in the line: the bank in force is the \
             desk's answer",
            Some((35, "store preset 1 \"Warm\"")),
            Command::StorePreset {
                preset_id: PresetId::new(1),
                pool: None,
                name: "Warm".to_owned(),
                color: None,
                mode: StoreMode::Merge,
            },
        ),
        (
            "apply it back to the selection",
            Some((36, "preset 1")),
            Command::ApplyPreset {
                preset_id: PresetId::new(1),
            },
        ),
        (
            "copy a whole cue list onto a free number",
            Some((37, "copy sequence 1 sequence 5")),
            Command::Copy {
                from: ObjectRef::Sequence {
                    sequence_id: SequenceId::new(1),
                },
                to: ObjectRef::Sequence {
                    sequence_id: SequenceId::new(5),
                },
                mode: OverwriteMode::Merge,
            },
        ),
        (
            "and move it, which brings every executor that played it along",
            Some((38, "move sequence 5 sequence 6")),
            Command::Move {
                from: ObjectRef::Sequence {
                    sequence_id: SequenceId::new(5),
                },
                to: ObjectRef::Sequence {
                    sequence_id: SequenceId::new(6),
                },
                mode: OverwriteMode::Merge,
            },
        ),
        (
            "a view is the same four verbs one model along, and this one is \
             session state",
            Some((39, "label view 1 \"Programmer\"")),
            Command::Label {
                target: ObjectRef::View {
                    view_id: prism_domain::ViewId::new(1),
                },
                name: "Programmer".to_owned(),
            },
        ),
        (
            "the selection to full, which is a whole command with no argument",
            Some((40, "full")),
            Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                value: 65535,
                relative: false,
            },
        ),
        (
            "**a line the daemon refuses**: there is no sequence 404, and the \
             parser was right to send it — D3",
            Some((41, "delete sequence 404")),
            Command::Delete {
                target: ObjectRef::Sequence {
                    sequence_id: SequenceId::new(404),
                },
            },
        ),
        ("take the last edit back", Some((42, "oops")), Command::Oops),
    ]
}

/* -------------------------------------------------------------------------- */
/* Recording it                                                               */
/* -------------------------------------------------------------------------- */

/// A wire that has said hello, and the snapshot payload it was served.
async fn connect(address: &str) -> (Wire, Vec<u8>) {
    let mut wire = local::connect(address)
        .await
        .expect("the daemon is listening");
    wire.send_message(&ClientMessage::Hello {
        hello: Hello::new(ClientKind::Desktop),
    })
    .await
    .expect("a hello must reach the daemon");
    loop {
        let payload = next_payload(&mut wire).await;
        if matches!(decode(&payload), ServerMessage::Snapshot { .. }) {
            return (wire, payload);
        }
    }
}

/// The next payload, or a named failure.
async fn next_payload(wire: &mut Wire) -> Vec<u8> {
    match tokio::time::timeout(PATIENCE, wire.recv()).await {
        Ok(Some(Ok(payload))) => payload,
        Ok(other) => panic!("the daemon stopped talking: {other:?}"),
        Err(_) => panic!("the daemon said nothing for {PATIENCE:?}"),
    }
}

/// A payload as the message it is.
fn decode(payload: &[u8]) -> ServerMessage {
    prism_ipc::decode(payload).expect("the daemon sends messages this build can read")
}

/// How long the daemon has to be quiet before a step is over.
///
/// **The playback readback is asynchronous** (S34): what cue an executor is on
/// comes back from the tick, so a `Delta::ExecutorState` arrives a poll after
/// the command that caused it rather than with its receipt. A recorder that
/// stopped at the `Ack` would file that delta under the *next* step, and the
/// per-step snapshot beside it would already contain it — the recording would
/// disagree with itself.
///
/// Six poll periods and a few ticks. Long enough that a settled desk is really
/// settled; short enough that a forty-step script does not take a minute.
const SETTLE: Duration = Duration::from_millis(150);

/// Collects everything the daemon says after the receipt, until it has gone
/// [`SETTLE`] without a **delta**.
///
/// Only a delta resets the window. The daemon also publishes a telemetry frame
/// thirty times a second to anybody listening, so a drain that waited for
/// silence on the socket would wait for ever.
async fn settle(wire: &mut Wire, deltas: &mut Vec<String>) {
    let mut deadline = std::time::Instant::now() + SETTLE;
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            return;
        }
        match tokio::time::timeout(deadline - now, wire.recv()).await {
            Ok(Some(Ok(payload))) => {
                if matches!(decode(&payload), ServerMessage::Delta { .. }) {
                    deltas.push(common::encode_base64(&payload));
                    deadline = std::time::Instant::now() + SETTLE;
                }
            }
            // The window closed, or the connection has gone: either way this
            // step is over.
            Ok(_) | Err(_) => return,
        }
    }
}

/// **The regenerator.** Runs a daemon and writes the rig and the recording.
///
/// ```text
/// cargo test -p prismd --test ui_programmer -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rewrites two committed fixtures; run it deliberately"]
async fn record_the_desk_script_for_the_interface() {
    let _turn = common::one_daemon_at_a_time();
    let rig = rig_path();
    std::fs::create_dir_all(rig.parent().expect("the fixture lives in a directory")).unwrap();
    let _ = std::fs::remove_file(&rig);
    write_rig(&rig);

    let dir = tempfile::tempdir().unwrap();
    let show = dir.path().join("desk.prism");
    std::fs::copy(&rig, &show).unwrap();

    let mut daemon = Daemon::start(&Options {
        data_dir: Some(dir.path().to_path_buf()),
        show: Some(show),
        universes: 2,
        outputs: vec![OutputSpec::Mock],
        local: true,
        log_level: prismd::log::Level::Warn,
        ..Options::default()
    })
    .await
    .unwrap();
    let address = common::local_address(dir.path());

    let (stop, stopped) = tokio::sync::oneshot::channel();
    let running = tokio::spawn(async move {
        daemon
            .run(None, async {
                let _ = stopped.await;
            })
            .await;
        daemon
    });

    let (mut wire, initial_snapshot) = connect(&address).await;
    let mut steps = Vec::new();
    for (index, (what, typed, command)) in script().into_iter().enumerate() {
        let seq = u64::try_from(index).unwrap_or(0) + 1;
        let message = ClientMessage::Command { seq, command };
        let client = common::encode_base64(&prism_ipc::encode(&message).expect("it encodes"));
        wire.send_message(&message)
            .await
            .expect("a command must reach the daemon");

        let mut deltas = Vec::new();
        let refused = loop {
            let payload = next_payload(&mut wire).await;
            match decode(&payload) {
                ServerMessage::Delta { .. } => deltas.push(common::encode_base64(&payload)),
                ServerMessage::Ack { seq: acked } if acked == seq => break false,
                ServerMessage::Reject {
                    seq: Some(refused), ..
                } if refused == seq => break true,
                _ => {}
            }
        };

        // Everything the readback has to say about this step, before the
        // snapshot below is taken — see [`SETTLE`].
        settle(&mut wire, &mut deltas).await;

        // And the answers, out of a **fresh** snapshot: what the daemon would
        // tell a client that had never seen a delta.
        let (second, payload) = connect(&address).await;
        second.shutdown().await;
        let snapshot = snapshot_of_payload(&payload);

        steps.push(Step {
            what: what.to_owned(),
            typed: typed.map(|(_, text)| text.to_owned()),
            typed_line: typed.map(|(line, _)| line),
            client,
            deltas,
            refused,
            strips: strips_of(&snapshot.show, &snapshot.session),
            programmer: programmer_of(&snapshot.programmer),
            session: session_of(&snapshot.session),
        });
    }

    let (final_wire, final_snapshot) = connect(&address).await;
    final_wire.shutdown().await;
    wire.shutdown().await;

    let recording = Recording {
        note: "Recorded off a running prismd by crates/prismd/tests/ui_programmer.rs. \
               The strips, the programmer and the session of every step are the daemon's \
               own answers, taken from a fresh client's snapshot. \
               Regenerate with: cargo test -p prismd --test ui_programmer -- --ignored"
            .to_owned(),
        protocol_version: prism_ipc::PROTOCOL_VERSION,
        executors_per_page: prism_domain::EXECUTORS_PER_PAGE,
        encoder_banks: encoder_banks(),
        initial_snapshot: common::encode_base64(&initial_snapshot),
        steps,
        final_snapshot: common::encode_base64(&final_snapshot),
    };

    let path = recording_path();
    let mut text = serde_json::to_string_pretty(&recording).unwrap();
    text.push('\n');
    std::fs::write(&path, text.replace("\r\n", "\n")).unwrap();
    println!(
        "wrote {} ({} steps, {} bytes) and {}",
        path.display(),
        recording.steps.len(),
        std::fs::metadata(&path).unwrap().len(),
        rig.display()
    );

    let _ = stop.send(());
    let daemon = tokio::time::timeout(PATIENCE, running)
        .await
        .expect("the daemon must stop when it is told to")
        .unwrap();
    daemon.shutdown().await;
}

/// The encoder banks and their parameters, in the order the wheel walks them.
fn encoder_banks() -> BTreeMap<String, Vec<String>> {
    FeatureGroup::ALL
        .into_iter()
        .map(|group| {
            (
                name_of(&group),
                group.attributes().iter().map(name_of).collect(),
            )
        })
        .collect()
}

/// A domain value's wire spelling, without the quotes.
fn name_of<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|json| json.as_str().map(str::to_owned))
        .expect("these are all plain string enums")
}

/* -------------------------------------------------------------------------- */
/* The answers, read out of the documents                                     */
/* -------------------------------------------------------------------------- */

/// The eight strips of the session's current page, out of the show document.
///
/// Read by pointer, the way a client reads it, rather than out of a `Show`: the
/// interface has the JSON and nothing else, so an answer taken from the typed
/// model could be right about something the browser cannot see.
fn strips_of(show: &JsonValue, session: &JsonValue) -> Vec<RecordedStrip> {
    let page = session_of(session).executor_page;
    let show = prism_core::JsonMirror::new(show.clone());
    (0..prism_domain::EXECUTORS_PER_PAGE)
        .map(|slot| {
            let id = ExecutorId::from_page_and_slot(page, slot);
            let executor = show.get(&format!("/executors/{}", id.get())).ok();
            let name = executor
                .and_then(|value| member(value, "sequenceId"))
                .and_then(|value| match value {
                    JsonValue::Int(number) => Some(*number),
                    _ => None,
                })
                .and_then(|sequence| show.get(&format!("/sequences/{sequence}/name")).ok())
                .and_then(|value| match value {
                    JsonValue::String(text) => Some(text.clone()),
                    _ => None,
                });
            RecordedStrip {
                slot,
                executor_id: id.get(),
                assigned: executor.is_some(),
                name,
                master_level: executor.map_or(0, |value| {
                    u16::try_from(int_at(value, "masterLevel")).unwrap_or(0)
                }),
                is_active: executor.is_some_and(|value| bool_at(value, "isActive")),
                current_cue_index: executor.and_then(|value| {
                    match member(value, "currentCueIndex") {
                        Some(JsonValue::Int(number)) => u32::try_from(*number).ok(),
                        _ => None,
                    }
                }),
                fader_function: executor
                    .map_or_else(String::new, |value| string_at(value, "faderFunction")),
                button_functions: executor
                    .map(|value| match member(value, "buttonFunctions") {
                        Some(JsonValue::Array(entries)) => entries
                            .iter()
                            .map(|entry| match entry {
                                JsonValue::String(text) => text.clone(),
                                other => panic!("a button function is not a string: {other:?}"),
                            })
                            .collect(),
                        other => panic!("buttonFunctions is not an array: {other:?}"),
                    })
                    .unwrap_or_default(),
            }
        })
        .collect()
}

/// The programmer, flattened — with the banks it has touched, which only the
/// show can answer.
fn programmer_of(state: &ProgrammerState) -> RecordedProgrammer {
    // The rig is not patched during the script, so the show the fixture was
    // written from is the show the daemon is holding. `Programmer::restore` is
    // S14's door and takes a whole state, which is exactly what arrived.
    let show = desk_show().show;
    let mut programmer = prism_core::Programmer::new();
    programmer.restore(state.clone());
    RecordedProgrammer {
        touched_banks: programmer
            .feature_groups(&show)
            .iter()
            .map(name_of)
            .collect(),
        selection: state.selection.iter().map(|id| id.get()).collect(),
        active_feature_group: name_of(&state.active_feature_group),
        values: state
            .values
            .iter()
            .flat_map(|(&fixture, attributes)| {
                attributes.iter().map(move |(&attribute, value)| {
                    (
                        fixture.get(),
                        name_of(&attribute),
                        value.value,
                        name_of(&value.source),
                    )
                })
            })
            .collect(),
        clear_stage: u8::from(state.clear_stage),
    }
}

/// The six session fields the bars read, out of the session document.
fn session_of(session: &JsonValue) -> RecordedSession {
    let mirror = prism_core::JsonMirror::new(session.clone());
    let number = |pointer: &str| -> u32 {
        match mirror.get(pointer) {
            Ok(JsonValue::Int(value)) => u32::try_from(*value).unwrap_or(0),
            other => panic!("{pointer} is not a number: {other:?}"),
        }
    };
    RecordedSession {
        executor_page: number("/session/executorPage"),
        selected_executor: match mirror.get("/session/selectedExecutor") {
            Ok(JsonValue::Int(value)) => u32::try_from(*value).ok(),
            _ => None,
        },
        encoder_bank: match mirror.get("/session/encoderBank") {
            Ok(JsonValue::String(text)) => text.clone(),
            other => panic!("encoderBank is not a string: {other:?}"),
        },
        programmer_page: number("/session/programmerPage"),
        programmer_param_index: number("/session/programmerParamIndex"),
        command_line: match mirror.get("/session/commandLine") {
            Ok(JsonValue::String(text)) => text.clone(),
            other => panic!("commandLine is not a string: {other:?}"),
        },
    }
}

/// The member at `key` of an object.
fn member<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    match value {
        JsonValue::Object(members) => members.get(key),
        _ => None,
    }
}

/// The integer member at `key`.
fn int_at(value: &JsonValue, key: &str) -> i64 {
    match member(value, key) {
        Some(&JsonValue::Int(number)) => number,
        other => panic!("{key} is not a number: {other:?}"),
    }
}

/// The boolean member at `key`.
fn bool_at(value: &JsonValue, key: &str) -> bool {
    match member(value, key) {
        Some(&JsonValue::Bool(flag)) => flag,
        other => panic!("{key} is not a boolean: {other:?}"),
    }
}

/// The string member at `key`.
fn string_at(value: &JsonValue, key: &str) -> String {
    match member(value, key) {
        Some(JsonValue::String(text)) => text.clone(),
        other => panic!("{key} is not a string: {other:?}"),
    }
}

/// The snapshot inside a payload.
fn snapshot_of_payload(payload: &[u8]) -> Snapshot {
    let ServerMessage::Snapshot { snapshot } = decode(payload) else {
        panic!("that payload is not a snapshot");
    };
    *snapshot
}

/// The snapshot inside a recorded payload.
fn snapshot_of(encoded: &str) -> Snapshot {
    snapshot_of_payload(&common::decode_base64(encoded))
}

/* -------------------------------------------------------------------------- */
/* The checks that run on every commit                                        */
/* -------------------------------------------------------------------------- */

/// Reads the committed recording.
fn recording() -> Recording {
    let path = recording_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is missing: {error}", path.display()));
    serde_json::from_str(&text).expect("the recording is JSON this build understands")
}

/// **The recorded answers are what the documents actually say.**
///
/// The deltas are replayed through `prism_core`'s own mirrors — the show, the
/// session, and the programmer, which arrives whole — and every step's recorded
/// strips, programmer and session are read back out of them. So the three claims
/// the browser is held to are checked here first, in the language that produced
/// them.
#[test]
fn the_recorded_answers_are_what_the_documents_say() {
    let recording = recording();
    assert_eq!(
        recording.protocol_version,
        prism_ipc::PROTOCOL_VERSION,
        "the recording is of another protocol version"
    );
    assert_eq!(recording.steps.len(), script().len());
    assert_eq!(
        recording.executors_per_page,
        prism_domain::EXECUTORS_PER_PAGE
    );
    assert_eq!(recording.encoder_banks, encoder_banks());

    let start = snapshot_of(&recording.initial_snapshot);
    let mut show = ShowMirror::new(start.show.clone());
    let mut session = SessionMirror::new(start.session.clone());
    let mut programmer = start.programmer.clone();

    for (index, (step, (what, typed, command))) in recording.steps.iter().zip(script()).enumerate()
    {
        assert_eq!(step.what, what, "step {index} is not the one in the script");
        assert_eq!(
            step.typed.as_deref(),
            typed.map(|(_, text)| text),
            "step {index}: the typed line"
        );
        assert_eq!(
            step.typed_line,
            typed.map(|(line, _)| line),
            "step {index}: the line number"
        );
        // The payload is the command the script names, and not merely *a*
        // command: a recording whose bytes had drifted from the script would
        // otherwise be compared against itself.
        let expected = ClientMessage::Command {
            seq: u64::try_from(index).unwrap_or(0) + 1,
            command,
        };
        assert_eq!(
            common::decode_base64(&step.client),
            prism_ipc::encode(&expected).expect("it encodes"),
            "step {index}: the recorded payload is not this command"
        );

        for (order, encoded) in step.deltas.iter().enumerate() {
            let ServerMessage::Delta { delta } = decode(&common::decode_base64(encoded)) else {
                panic!("step {index} delta {order} is not a delta");
            };
            show.apply_delta(&delta)
                .unwrap_or_else(|error| panic!("step {index} delta {order}: {error}"));
            session
                .apply_delta(&delta)
                .unwrap_or_else(|error| panic!("step {index} delta {order}: {error}"));
            if let prism_domain::Delta::ProgrammerChanged { state } = &delta {
                programmer = state.clone();
            }
        }

        assert_eq!(
            strips_of(show.value(), session.value()),
            step.strips,
            "step {index} ({what}): the strips"
        );
        assert_eq!(
            programmer_of(&programmer),
            step.programmer,
            "step {index} ({what}): the programmer"
        );
        assert_eq!(
            session_of(session.value()),
            step.session,
            "step {index} ({what}): the session"
        );
    }

    // And what the deltas built is what a client that never saw one is served.
    let end = snapshot_of(&recording.final_snapshot);
    assert_eq!(
        show.value(),
        &end.show,
        "the show deltas and the snapshot disagree"
    );
    assert_eq!(
        session.value(),
        &end.session,
        "the session deltas and the snapshot disagree"
    );
    assert_eq!(
        programmer, end.programmer,
        "the programmer deltas and the snapshot disagree"
    );
}

/// The script is a desk being operated, not a list that happens to parse.
///
/// Every claim the browser's tests lean on is asserted here to be *in* the
/// recording: a fixture that quietly stopped exercising the interesting case
/// would otherwise leave those tests passing over nothing.
#[test]
fn the_recording_is_of_a_desk_being_used() {
    let recording = recording();
    let steps = &recording.steps;

    // **D7**: paging changes which eight executors are on the bar, and the
    // numbering is `page * 8 + slot`.
    for step in steps {
        assert_eq!(step.strips.len(), 8, "a page is eight strips");
        for (slot, strip) in step.strips.iter().enumerate() {
            let slot = u32::try_from(slot).expect("eight fits");
            assert_eq!(strip.slot, slot);
            assert_eq!(strip.executor_id, step.session.executor_page * 8 + slot);
        }
    }
    let pages: std::collections::BTreeSet<u32> = steps
        .iter()
        .map(|step| step.session.executor_page)
        .collect();
    assert!(pages.len() >= 2, "the bank was never paged: {pages:?}");
    // And a page with nothing on it, so an interface that only ever drew
    // assigned strips could not pass.
    assert!(
        steps
            .iter()
            .any(|step| step.strips.iter().all(|strip| !strip.assigned)),
        "no page was ever empty"
    );
    assert!(
        steps
            .iter()
            .any(|step| step.strips.iter().any(|strip| strip.assigned)),
        "no strip was ever assigned"
    );

    // A master moves, an executor runs and stops, and a cue index appears.
    let levels: std::collections::BTreeSet<u16> = steps
        .iter()
        .flat_map(|step| step.strips.iter().map(|strip| strip.master_level))
        .collect();
    assert!(levels.len() >= 3, "every master read the same: {levels:?}");
    assert!(
        steps
            .iter()
            .any(|step| step.strips.iter().any(|strip| strip.is_active)),
        "nothing was ever started"
    );
    // **The finding S26 recorded, closed in S34 and asserted the other way
    // round.** Until this session `cueIndex` was never filled — what cue a
    // playback is on lives on the tick thread and there was no channel from the
    // tick to the core — so this assertion demanded that every recorded strip
    // showed a dash, with a message telling whoever built the channel to turn it
    // round. `prism_engine::PlaybackReport` is that channel, and here is the
    // other side of the claim: a strip that is running is on a cue, a strip that
    // is not is on none, and the number moves.
    assert!(
        steps.iter().all(|step| step
            .strips
            .iter()
            .all(|strip| strip.current_cue_index.is_some() == strip.is_active)),
        "a strip's cue index disagrees with whether it is running"
    );
    let indexes: std::collections::BTreeSet<u32> = steps
        .iter()
        .flat_map(|step| {
            step.strips
                .iter()
                .filter_map(|strip| strip.current_cue_index)
        })
        .collect();
    assert!(
        !indexes.is_empty(),
        "no strip ever reported a cue index, so the readback was never exercised"
    );
    // The four button functions the protocol has no command for are on the bar,
    // because an interface that never met one could not be checked for being
    // honest about it (`docs/MCU_MAPPING.md` §4.2.1).
    let functions: std::collections::BTreeSet<&str> = steps
        .iter()
        .flat_map(|step| {
            step.strips
                .iter()
                .flat_map(|strip| strip.button_functions.iter().map(String::as_str))
        })
        .collect();
    for unbindable in ["Flash", "Toggle", "On", "LearnSpeed"] {
        assert!(functions.contains(unbindable), "{functions:?}");
    }
    for bindable in ["Go+", "Go-", "Off"] {
        assert!(functions.contains(bindable), "{functions:?}");
    }

    // The programmer is exercised on more than one bank, sparsely, and the
    // three-stage Clear is walked all the way round.
    let attributes: std::collections::BTreeSet<&str> = steps
        .iter()
        .flat_map(|step| {
            step.programmer
                .values
                .iter()
                .map(|(_, attribute, _, _)| attribute.as_str())
        })
        .collect();
    assert!(
        attributes.len() >= 2,
        "only {attributes:?} was ever touched"
    );
    let stages: std::collections::BTreeSet<u8> = steps
        .iter()
        .map(|step| step.programmer.clear_stage)
        .collect();
    assert_eq!(
        stages,
        [0, 1, 2].into_iter().collect(),
        "the Clear button never went all the way round"
    );
    assert!(
        steps
            .iter()
            .any(|step| !step.programmer.selection.is_empty()),
        "nothing was ever selected"
    );
    assert!(
        steps
            .iter()
            .any(|step| step.programmer.selection.is_empty() && step.programmer.values.is_empty()),
        "the programmer was never empty"
    );

    // The session fields the bars read all move.
    let banks: std::collections::BTreeSet<&str> = steps
        .iter()
        .map(|step| step.session.encoder_bank.as_str())
        .collect();
    assert!(banks.len() >= 2, "the encoder bank never moved: {banks:?}");
    let indexes: std::collections::BTreeSet<u32> = steps
        .iter()
        .map(|step| step.session.programmer_param_index)
        .collect();
    assert!(indexes.len() >= 2, "the wheel never moved on: {indexes:?}");
    assert!(
        steps
            .iter()
            .any(|step| step.session.selected_executor.is_some()),
        "no executor was ever selected"
    );
    assert!(
        steps
            .iter()
            .any(|step| !step.session.command_line.is_empty()),
        "the command line was never written to"
    );

    // Refusals, and none of them changes anything.
    let refusals: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.refused)
        .map(|(index, _)| index)
        .collect();
    assert!(refusals.len() >= 3, "only {} refusals", refusals.len());
    for index in refusals {
        assert!(
            steps[index].deltas.is_empty(),
            "step {index} said something"
        );
        let before = &steps[index - 1];
        let after = &steps[index];
        assert_eq!(before.strips, after.strips, "step {index} moved a strip");
        assert_eq!(
            before.programmer, after.programmer,
            "step {index} moved the programmer"
        );
        assert_eq!(
            before.session, after.session,
            "step {index} moved the session"
        );
    }
}

/// **Every typed line is a line, and its commands are in order.**
///
/// The browser's parser is compared against the commands of each line, so the
/// grouping has to mean what the browser thinks it means: a line's steps are
/// consecutive, in script order, and every line has a number of its own — which
/// is why the number is recorded rather than inferred from the text. `clear` is
/// pressed three times in a row and three presses of a three-stage button are
/// three lines, not one line with three commands on it.
#[test]
fn the_typed_lines_are_the_console_being_used() {
    let recording = recording();
    let mut groups: Vec<(u32, String, usize)> = Vec::new();
    for step in &recording.steps {
        let (Some(typed), Some(number)) = (&step.typed, step.typed_line) else {
            assert_eq!(
                step.typed.is_some(),
                step.typed_line.is_some(),
                "a typed line has a number and a number has a line"
            );
            continue;
        };
        match groups.last_mut() {
            Some((at, text, count)) if *at == number => {
                assert_eq!(text, typed, "line {number} is two different lines");
                *count += 1;
            }
            _ => groups.push((number, typed.clone(), 1)),
        }
    }
    assert!(groups.len() >= 8, "only {} lines were typed", groups.len());
    // Every line's number is used once, so a browser grouping by number sees
    // exactly these groups.
    let numbers: std::collections::BTreeSet<u32> =
        groups.iter().map(|(number, _, _)| *number).collect();
    assert_eq!(numbers.len(), groups.len(), "a line number was reused");
    // A line that produces two commands, which is the case a parser returning
    // one command could never handle.
    assert!(
        groups.iter().any(|(_, _, count)| *count == 2),
        "no line produced two commands: {groups:?}"
    );
    // And the shapes the grammar has to cover, each present at least once.
    let text: Vec<&str> = groups.iter().map(|(_, line, _)| line.as_str()).collect();
    for wanted in [
        "page 1",
        "go 0",
        "off 0",
        "1 + 2",
        "1 thru 3 at 50",
        "clear",
    ] {
        assert!(text.contains(&wanted), "{text:?} has no {wanted}");
    }
    // Three presses of Clear, and three lines rather than one.
    assert_eq!(
        text.iter().filter(|line| **line == "clear").count(),
        3,
        "Clear is a three-stage button"
    );
}

/// The rig the browser opens is the rig this file writes, and it is dark.
///
/// A show whose fixtures sit at full at home would make "the encoder reached the
/// output" unmeasurable — the level would already be there. Checked on the
/// committed file rather than on the function that wrote it.
#[test]
fn the_committed_rig_is_dark_and_has_more_than_one_encoder_bank_on_it() {
    let path = rig_path();
    let store = ShowStore::open(&path).unwrap_or_else(|error| {
        panic!("{} is missing: {error}", path.display());
    });
    let mut file = ShowFile::new();
    store
        .load(&mut file)
        .expect("the rig opens with this build");

    assert_eq!(file.show.fixtures().count(), 5, "the rig is five fixtures");
    for fixture_type in file.show.fixture_types() {
        for def in &fixture_type.attributes {
            assert_eq!(
                def.default_value, 0,
                "{} {:?} is not dark at home",
                fixture_type.id, def.attribute
            );
        }
    }
    let banks: std::collections::BTreeSet<FeatureGroup> = file
        .show
        .fixture_types()
        .flat_map(|fixture_type| fixture_type.attributes.iter().map(|def| def.feature_group))
        .collect();
    assert!(banks.len() >= 3, "only {banks:?} are patched");
    assert_eq!(file.show.executors().count(), 3);
    // The programmer is deliberately not in a show file (S13), so a browser
    // opening this rig starts from an empty one and the recorded first step is
    // a statement about the script rather than about the file.
    assert_eq!(file.programmer.state(), &ProgrammerState::default());
    assert_eq!(file.session.session().executor_page, 0);
}
