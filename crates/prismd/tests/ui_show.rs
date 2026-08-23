//! A show being written — recorded off a running daemon, so the browser's
//! sequence sheet, cue sheet and preset pools are held to the daemon's answers
//! rather than to a second opinion in TypeScript.
//!
//! # Why a sixth recording
//!
//! The five before it record a conversation (S23), a telemetry channel (S24), a
//! canvas (S25), a desk being operated (S26) and a rig being built (S27). This
//! one records the two things S28 needed and nothing else could answer:
//!
//! - **what a store does to the show** — which cue exists afterwards, what it
//!   holds, and *which cues moved because a preset was edited*, which is the
//!   session's hardest claim and one no client may work out for itself;
//! - **what the daemon says a store *would* do, before it does it** — the
//!   `Query::StorePreview` S28 added to the protocol's third shape, which is the
//!   whole of the exit criterion *a store that would overwrite says what it will
//!   do first, even where the only mode available is Merge*.
//!
//! # It is frozen, and the guards run on every commit
//!
//! [`record_the_show_script_for_the_interface`] is `#[ignore]`d like the other
//! five regenerators. What runs always replays the deltas through
//! `prism_core::ShowMirror` and checks every recorded row and every recorded
//! answer, so a shape that moved fails here, in Rust, rather than going stale in
//! `ui/`.

// This target *writes files for a person to commit* and says where.
#![allow(clippy::print_stdout)]
// Every test holds `common::one_daemon_at_a_time` across its awaits.
#![allow(clippy::await_holding_lock)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use prism_core::{Show, ShowFile, ShowMirror, ShowStore};
use prism_domain::{
    Answer, AttributeType, Command, CueProperty, CueTrigger, ExecutorId, FeatureGroup, FixtureId,
    GoDirection, JsonValue, ObjectRef, OverwriteMode, PlaybackTarget, PresetId, RgbColor,
    SelectionMode, SequenceId, SequenceStoreMode, StoreMode, StoreTarget, UniverseId,
};
use prism_ipc::{ClientKind, ClientMessage, Hello, ServerMessage, Snapshot, Wire, local};
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;

mod common;

/// How long any wait here may take before it is a named failure.
const PATIENCE: Duration = Duration::from_secs(20);

/// Where the interface reads the recording from.
fn recording_path() -> PathBuf {
    common::ui_fixture("show-recording.json")
}

/// Where the end-to-end suite reads the rig from.
fn rig_path() -> PathBuf {
    common::ui_fixture("show-rig.prism")
}

/* -------------------------------------------------------------------------- */
/* The rig                                                                    */
/* -------------------------------------------------------------------------- */

/// The show the script starts from: **four fixtures and nothing stored.**
///
/// Deliberately empty of looks. Every sequence, cue and preset in this recording
/// is something the *script* made, so a browser reading the first step is
/// reading a rig with an empty pool rather than a rig plus somebody else's show
/// — and the sheets' *nothing here yet* state is the first thing recorded rather
/// than a state nobody ever sees.
///
/// Two profiles, both the desk's own, so the recorded rig and the desk's library
/// cannot describe two different PARs. The dimmer is there because a preset pool
/// has to be shown taking **its own pool's** values and leaving the rest, which
/// needs a value on a bank the pool is not.
fn show_rig() -> ShowFile {
    let mut show = Show::new();
    for type_id in ["generic.dimmer", "generic.rgbw.par"] {
        show.embed_fixture_type(generic(type_id))
            .expect("a library profile is one a show accepts");
    }
    for (id, type_id, address) in [
        (1_u32, "generic.rgbw.par", 1_u16),
        (2, "generic.rgbw.par", 5),
        (3, "generic.rgbw.par", 9),
        (4, "generic.dimmer", 20),
    ] {
        show.patch_fixture(prism_domain::Fixture {
            id: FixtureId::new(id),
            name: format!("Fixture {id}"),
            type_id: type_id.to_owned(),
            universe: UniverseId::new(1),
            address,
            position: prism_domain::Vec3::ZERO,
            rotation: prism_domain::Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
        })
        .expect("the address is free");
    }
    ShowFile {
        show,
        ..ShowFile::new()
    }
}

/// One of the desk's built-in profiles, by key.
fn generic(type_id: &str) -> prism_domain::FixtureType {
    prism_core::generic_profiles()
        .into_iter()
        .find(|profile| profile.id == type_id)
        .unwrap_or_else(|| panic!("the desk carries no {type_id}"))
}

/// Writes the rig into a `.prism` file for the daemon and the browser to open.
fn write_rig(path: &Path) {
    let mut file = show_rig();
    let mut store = ShowStore::open(path).expect("a show file opens");
    store.save(&mut file).expect("a show file saves");
}

/* -------------------------------------------------------------------------- */
/* The file                                                                   */
/* -------------------------------------------------------------------------- */

/// One line of a cue sheet, as a reader has to produce it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedCue {
    number: String,
    name: String,
    fade_in: f64,
    fade_out: f64,
    delay: f64,
    trigger: String,
    trigger_time: Option<f64>,
    /// What it sets, in fixture then attribute order.
    ///
    /// The **values** are here rather than a count of them, because the claim
    /// this recording exists to check is that editing a preset moves them: two
    /// cues with the same number of parts and different values are exactly what
    /// a preset edit produces, and a summary could not tell them apart.
    parts: Vec<RecordedPart>,
}

/// One value inside a cue — what a Cue Viewer draws a row of.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedPart {
    fixture: u32,
    attribute: String,
    value: u16,
    /// The preset this value follows, which is what keeps a cue live-updatable.
    preset_ref: Option<u32>,
}

/// One line of a sequence sheet.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedSequence {
    id: u32,
    name: String,
    looping: bool,
    cues: Vec<RecordedCue>,
}

/// One box of a preset pool.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedPreset {
    id: u32,
    pool: String,
    name: String,
    color: Option<RgbColor>,
    values: usize,
}

/// One executor slot, as far as a cue sheet cares: which list is on it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedExecutor {
    id: u32,
    sequence_id: Option<u32>,
    is_active: bool,
    /// **Always `null`** until S34 builds the channel back from the tick — see
    /// [`the_cue_index_is_a_number_now`].
    current_cue_index: Option<u32>,
}

/// The **update state** after a step — S39's `Session::editingCue`.
///
/// Recorded beside the show rather than derived from it, because it is the one
/// answer in this file that is not in the show document at all: which cue the
/// programmer is editing lives in the session, and an interface blinking an
/// Update key reads it there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedCueEdit {
    sequence_id: u32,
    cue_number: String,
    modified: bool,
}

/// One step: either a command that changed the show, or a question that did not.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    /// What it is, for a person reading the file.
    what: String,
    /// The `ClientMessage` payload, base64 — a `Command` or a `Query`.
    client: String,
    /// Whether this step asked a question rather than giving an instruction.
    is_query: bool,
    /// The deltas the daemon answered with, in order, base64. Empty for a query,
    /// **by construction**.
    deltas: Vec<String>,
    /// The `ServerMessage::Answer` payload, base64, when this was a query.
    answer: Option<String>,
    /// Whether the daemon refused it.
    refused: bool,
    /// The sequences afterwards, from a fresh client's snapshot.
    sequences: Vec<RecordedSequence>,
    /// The presets afterwards.
    presets: Vec<RecordedPreset>,
    /// The executor grid afterwards.
    executors: Vec<RecordedExecutor>,
    /// The cue list in force afterwards — S39's `Session::selectedSequence`.
    selected_sequence: Option<u32>,
    /// The update state afterwards — S39's `Session::editingCue`.
    editing_cue: Option<RecordedCueEdit>,
}

/// The whole file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recording {
    /// What made it, and how to make it again.
    note: String,
    /// The protocol version these payloads belong to.
    protocol_version: u32,
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

/// A step of the script: what it is for, and what is sent.
enum Scripted {
    /// An instruction.
    Do(&'static str, Command),
    /// A question.
    Ask(&'static str, Query),
}

use prism_domain::Query;

/// Selecting fixtures, which every store in this script begins with.
fn select(ids: &[u32]) -> Command {
    Command::SelectFixtures {
        ids: ids.iter().copied().map(FixtureId::new).collect(),
        mode: SelectionMode::Set,
    }
}

/// Setting one attribute on the selection.
fn dial(attribute: AttributeType, value: u16) -> Command {
    Command::SetAttribute {
        attribute,
        value: i32::from(value),
        relative: false,
    }
}

/// The question a Store button asks before it lights up, in the mode the
/// operator has chosen — S39.
fn preview_cue(sequence: u32, number: &str, mode: StoreMode) -> Query {
    Query::StorePreview {
        target: StoreTarget::Cue {
            sequence_id: SequenceId::new(sequence),
            cue_number: number.to_owned(),
        },
        mode,
    }
}

/// The same question for a preset pool.
fn preview_preset(preset: u32, pool: FeatureGroup, mode: StoreMode) -> Query {
    Query::StorePreview {
        target: StoreTarget::Preset {
            preset_id: PresetId::new(preset),
            pool,
        },
        mode,
    }
}

/// Storing the programmer into a cue, in a mode.
fn store_cue(sequence: u32, number: &str, mode: StoreMode) -> Command {
    Command::StoreCue {
        sequence_id: Some(SequenceId::new(sequence)),
        cue_number: number.to_owned(),
        mode,
    }
}

/// The script, in order: **a show being written, corrected and played.**
///
/// Written out rather than generated. Every step is one a cue sheet or a preset
/// pool can get wrong, and the questions are interleaved where an interface
/// would actually ask them — before the store, not after it.
fn script() -> Vec<Scripted> {
    vec![
        Scripted::Do(
            "make a cue list to store into: a fresh show has none at all",
            Command::StoreSequence {
                sequence_id: SequenceId::new(1),
                name: "Act 1".to_owned(),
                mode: SequenceStoreMode::Append,
            },
        ),
        Scripted::Do(
            "put it on an executor, which is what makes it playable",
            Command::AssignExecutor {
                executor_id: ExecutorId::new(0),
                sequence_id: Some(SequenceId::new(1)),
            },
        ),
        Scripted::Do(
            "and put it in force, which is what a store with no cue list named \
             goes into — S39's session field, and the decision S28 marked",
            Command::SelectSequence {
                sequence_id: SequenceId::new(1),
            },
        ),
        Scripted::Do("select three PARs", select(&[1, 2, 3])),
        Scripted::Do("and dial them red", dial(AttributeType::Red, 65535)),
        Scripted::Ask(
            "what would storing cue 1 do? it does not exist yet, so this is a create",
            preview_cue(1, "1", StoreMode::Merge),
        ),
        Scripted::Do("store it", store_cue(1, "1", StoreMode::Merge)),
        Scripted::Do("select two of them again", select(&[1, 2])),
        Scripted::Do("and dial them green", dial(AttributeType::Green, 40000)),
        Scripted::Ask(
            "what would storing cue 1 again do? **this is the overwrite an operator \
             has to be told about**: the three reds are still in the programmer, so \
             they are replaced, and the two greens are added",
            preview_cue(1, "1", StoreMode::Merge),
        ),
        Scripted::Ask(
            "and the same store as an Override? the same two added and the same \
             three replaced, and **nothing thrown away** — because this \
             programmer happens to hold everything the cue holds, which is the \
             case where the two modes coincide and an operator needs to be able \
             to see that they do",
            preview_cue(1, "1", StoreMode::Override),
        ),
        Scripted::Ask(
            "and as a Remove? it writes nothing at all: the three values the cue \
             and the programmer share are what goes",
            preview_cue(1, "1", StoreMode::Remove),
        ),
        Scripted::Do(
            "store it as a Merge, which is what the operator chose",
            store_cue(1, "1", StoreMode::Merge),
        ),
        Scripted::Do("store a second cue", store_cue(1, "2", StoreMode::Merge)),
        Scripted::Do(
            "name it",
            Command::Label {
                target: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "2".to_owned(),
                },
                name: "Green wash".to_owned(),
            },
        ),
        Scripted::Do(
            "give it a fade",
            Command::SetCueProperty {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "2".to_owned(),
                property: CueProperty::FadeIn { seconds: 5.5 },
            },
        ),
        Scripted::Do(
            "and a trigger that carries a time with it",
            Command::SetCueProperty {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "2".to_owned(),
                property: CueProperty::Trigger {
                    trigger: CueTrigger::Time,
                    trigger_time: Some(4.0),
                },
            },
        ),
        Scripted::Do(
            "renumber it to 1.5, which moves it up the list",
            Command::Move {
                from: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "2".to_owned(),
                },
                to: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "1.5".to_owned(),
                },
                mode: OverwriteMode::Override,
            },
        ),
        Scripted::Do(
            "a fade that runs backwards: refused, and nothing moves",
            Command::SetCueProperty {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
                property: CueProperty::FadeIn { seconds: -1.0 },
            },
        ),
        // **A number that is taken is a question rather than a refusal** (S40).
        // S28 refused a renumber onto an occupied number, for
        // `RenumberFixture`'s reason: the number is the key, and replacing the
        // other cue deletes a look nobody asked to delete. S40 keeps the
        // *protection* and moves it one layer out — the line asks *merge,
        // override or cancel* first, and the answer travels in the command. So
        // what the recording holds here is the harmless one: a **copy** in
        // Merge mode, which leaves the source where it is and adds to the
        // destination.
        Scripted::Do(
            "a copy onto a number that is taken: it merges, and the source stays",
            Command::Copy {
                from: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "1.5".to_owned(),
                },
                to: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "1".to_owned(),
                },
                mode: OverwriteMode::Merge,
            },
        ),
        Scripted::Do("clear the programmer", Command::ClearProgrammer),
        Scripted::Do("select the PARs again", select(&[1, 2, 3])),
        Scripted::Do("dial a blue", dial(AttributeType::Blue, 60000)),
        Scripted::Do(
            "and a dimmer value on a PAR's white",
            dial(AttributeType::White, 100),
        ),
        Scripted::Ask(
            "what would storing preset 1 in the Colour pool do? a create, and it \
             counts the colour values only",
            preview_preset(1, FeatureGroup::Color, StoreMode::Merge),
        ),
        Scripted::Do(
            "store it, with a name and a scribble-strip colour",
            Command::StorePreset {
                preset_id: PresetId::new(1),
                pool: Some(FeatureGroup::Color),
                name: "Deep blue".to_owned(),
                color: Some(RgbColor { r: 0, g: 0, b: 255 }),
                mode: StoreMode::Merge,
            },
        ),
        Scripted::Do("clear again", Command::ClearProgrammer),
        Scripted::Do("select the PARs", select(&[1, 2, 3])),
        Scripted::Do(
            "and apply the preset, which is what puts a **link** in the programmer",
            Command::ApplyPreset {
                preset_id: PresetId::new(1),
            },
        ),
        Scripted::Do(
            "store a cue out of it: its parts carry the preset reference",
            store_cue(1, "3", StoreMode::Merge),
        ),
        Scripted::Do("clear once more", Command::ClearProgrammer),
        Scripted::Do("select the PARs", select(&[1, 2, 3])),
        Scripted::Do("dial a different blue", dial(AttributeType::Blue, 20000)),
        Scripted::Ask(
            "what would storing over preset 1 do? an edit of a preset: the three \
             blues are replaced and the three whites are kept",
            preview_preset(1, FeatureGroup::Color, StoreMode::Merge),
        ),
        Scripted::Do(
            "edit the preset — **and cue 3 follows it**, which is the whole claim",
            Command::StorePreset {
                preset_id: PresetId::new(1),
                pool: Some(FeatureGroup::Color),
                name: "Darker blue".to_owned(),
                color: Some(RgbColor { r: 0, g: 0, b: 120 }),
                mode: StoreMode::Merge,
            },
        ),
        Scripted::Ask(
            "a preview of a store into a sequence that is not there: refused, in \
             the daemon's own words",
            preview_cue(404, "1", StoreMode::Merge),
        ),
        Scripted::Do(
            "clear the programmer for the last time",
            Command::ClearProgrammer,
        ),
        Scripted::Ask(
            "and a preview with nothing to store: refused, but it still says what \
             is filed under that number",
            preview_cue(1, "1", StoreMode::Merge),
        ),
        // ---- S39: cue editing, the update state, and the store modes -------
        Scripted::Do(
            "load cue 3 back into the programmer — **its preset links come with \
             it**, which is the claim `EditCue` exists to keep",
            Command::EditCue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "3".to_owned(),
            },
        ),
        Scripted::Do(
            "put it straight back with nothing changed: **byte-identical**, links \
             and all, and the Update key stops blinking",
            Command::Update,
        ),
        Scripted::Do(
            "now change something while the cue is loaded, which is what makes \
             the key blink",
            dial(AttributeType::White, 30000),
        ),
        Scripted::Do("and put that back", Command::Update),
        Scripted::Do(
            "an Update after the programmer has been cleared: refused, because \
             clearing the programmer clears the update state",
            Command::ClearProgrammer,
        ),
        Scripted::Do("...and this is the refusal", Command::Update),
        Scripted::Do("select a PAR", select(&[1])),
        Scripted::Do("dial a green on it", dial(AttributeType::Green, 12345)),
        Scripted::Ask(
            "what would an Override of cue 1 do now? **this is the one an \
             operator has to be warned about**: one value replaced and four \
             thrown away",
            preview_cue(1, "1", StoreMode::Override),
        ),
        Scripted::Do(
            "store it as an Override: the cue ends up holding exactly this one \
             value, and the four that were there are gone",
            store_cue(1, "1", StoreMode::Override),
        ),
        Scripted::Ask(
            "and a Remove of the same value from cue 1.5, which holds four \
             others: one goes and four stay",
            preview_cue(1, "1.5", StoreMode::Remove),
        ),
        Scripted::Do(
            "take it out with a **Remove**: the cue keeps everything else",
            store_cue(1, "1.5", StoreMode::Remove),
        ),
        Scripted::Do(
            "the same Remove again: refused, because the value is gone and a \
             store that removes nothing is one an operator would press twice",
            store_cue(1, "1.5", StoreMode::Remove),
        ),
        Scripted::Do(
            "append the programmer to the cue list: a new cue at the highest \
             number, which is 4",
            Command::StoreSequence {
                sequence_id: SequenceId::new(1),
                name: String::new(),
                mode: SequenceStoreMode::Append,
            },
        ),
        Scripted::Do(
            "and merge it into every cue there is",
            Command::StoreSequence {
                sequence_id: SequenceId::new(1),
                name: String::new(),
                mode: SequenceStoreMode::Merge,
            },
        ),
        Scripted::Do(
            "delete a cue: the numbers left do not close up",
            Command::Delete {
                target: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "1.5".to_owned(),
                },
            },
        ),
        Scripted::Do(
            "delete a cue that is not there: refused, and nothing moves",
            Command::Delete {
                target: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "99".to_owned(),
                },
            },
        ),
        Scripted::Do(
            "a sequence number that is taken: refused, and the cue list survives",
            Command::StoreSequence {
                sequence_id: SequenceId::new(1),
                name: "Over the top".to_owned(),
                mode: SequenceStoreMode::Append,
            },
        ),
        Scripted::Do(
            "fire the list",
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
                direction: GoDirection::Next,
            },
        ),
        Scripted::Do(
            "step it again",
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
                direction: GoDirection::Next,
            },
        ),
        Scripted::Do(
            "and stop it",
            Command::ExecutorOff {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            },
        ),
        Scripted::Do(
            "a Go on an executor with nothing on it: refused",
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(4)),
                direction: GoDirection::Next,
            },
        ),
        Scripted::Do("and take the last edit back", Command::Oops),
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
/// cargo test -p prismd --test ui_show -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rewrites two committed fixtures; run it deliberately"]
async fn record_the_show_script_for_the_interface() {
    let _turn = common::one_daemon_at_a_time();
    let rig = rig_path();
    std::fs::create_dir_all(rig.parent().expect("the fixture lives in a directory")).unwrap();
    let _ = std::fs::remove_file(&rig);
    write_rig(&rig);

    let dir = tempfile::tempdir().unwrap();
    let show = dir.path().join("show.prism");
    std::fs::copy(&rig, &show).unwrap();

    let mut daemon = Daemon::start(&Options {
        data_dir: Some(dir.path().to_path_buf()),
        show: Some(show),
        universes: Some(1),
        outputs: vec![mock_output(1)],
        local: Some(true),
        // No WebSocket listener: since S37 the *setting* opens one, and a
        // recording target that said nothing would bind 127.0.0.1:7373 for the
        // length of the run.
        websocket: prismd::cli::Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
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
    for (index, scripted) in script().into_iter().enumerate() {
        let seq = u64::try_from(index).unwrap_or(0) + 1;
        let (what, message, is_query) = match scripted {
            Scripted::Do(what, command) => (what, ClientMessage::Command { seq, command }, false),
            Scripted::Ask(what, query) => (what, ClientMessage::Query { seq, query }, true),
        };
        let client = common::encode_base64(&prism_ipc::encode(&message).expect("it encodes"));
        wire.send_message(&message)
            .await
            .expect("a message must reach the daemon");

        let mut deltas = Vec::new();
        let mut answer = None;
        let refused = loop {
            let payload = next_payload(&mut wire).await;
            match decode(&payload) {
                ServerMessage::Delta { .. } => deltas.push(common::encode_base64(&payload)),
                ServerMessage::Answer { seq: answered, .. } if answered == seq => {
                    answer = Some(common::encode_base64(&payload));
                    break false;
                }
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
            client,
            is_query,
            deltas,
            answer,
            refused,
            sequences: sequences_of(&snapshot.show),
            presets: presets_of(&snapshot.show),
            executors: executors_of(&snapshot.show),
            selected_sequence: selected_sequence_of(&snapshot.session),
            editing_cue: editing_cue_of(&snapshot.session),
        });
    }

    let (final_wire, final_snapshot) = connect(&address).await;
    final_wire.shutdown().await;
    wire.shutdown().await;

    let recording = Recording {
        note: "Recorded off a running prismd by crates/prismd/tests/ui_show.rs. \
               The sequences, the presets, the executors and every answer are the \
               daemon's own, taken from a fresh client's snapshot. \
               Regenerate with: cargo test -p prismd --test ui_show -- --ignored"
            .to_owned(),
        protocol_version: prism_ipc::PROTOCOL_VERSION,
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

/* -------------------------------------------------------------------------- */
/* The answers, read out of the document                                      */
/* -------------------------------------------------------------------------- */

/// The sequences, out of the show **document** — read by pointer, the way a
/// client reads it, rather than out of a `Show`.
fn sequences_of(show: &JsonValue) -> Vec<RecordedSequence> {
    let JsonValue::Object(sequences) = member_or_null(show, "sequences") else {
        return Vec::new();
    };
    let mut rows: Vec<RecordedSequence> = sequences
        .iter()
        .map(|(key, value)| RecordedSequence {
            id: key.parse().expect("a sequence is keyed by its number"),
            name: string_at(value, "name"),
            looping: bool_at(value, "loop"),
            cues: cues_of(value),
        })
        .collect();
    rows.sort_by_key(|row| row.id);
    rows
}

/// The cues of one sequence, in the order the document holds them — which is
/// playback order, because `Show::store_cue` sorts before it writes.
fn cues_of(sequence: &JsonValue) -> Vec<RecordedCue> {
    let Some(JsonValue::Array(cues)) = member(sequence, "cues") else {
        return Vec::new();
    };
    cues.iter()
        .map(|cue| RecordedCue {
            number: string_at(cue, "number"),
            name: string_at(cue, "name"),
            fade_in: float_at(cue, "fadeIn"),
            fade_out: float_at(cue, "fadeOut"),
            delay: float_at(cue, "delay"),
            trigger: string_at(cue, "trigger"),
            trigger_time: optional_float_at(cue, "triggerTime"),
            parts: parts_of(cue),
        })
        .collect()
}

/// The values of one cue, out of the show document.
fn parts_of(cue: &JsonValue) -> Vec<RecordedPart> {
    let Some(JsonValue::Array(parts)) = member(cue, "parts") else {
        return Vec::new();
    };
    parts
        .iter()
        .map(|part| RecordedPart {
            fixture: u32::try_from(int_at(part, "fixture")).unwrap_or(0),
            attribute: string_at(part, "attribute"),
            value: u16::try_from(int_at(part, "value")).unwrap_or(0),
            preset_ref: match member(part, "presetRef") {
                Some(&JsonValue::Int(number)) => u32::try_from(number).ok(),
                _ => None,
            },
        })
        .collect()
}

/// The preset pools, out of the show document.
fn presets_of(show: &JsonValue) -> Vec<RecordedPreset> {
    let JsonValue::Object(presets) = member_or_null(show, "presets") else {
        return Vec::new();
    };
    let mut rows: Vec<RecordedPreset> = presets
        .iter()
        .map(|(key, value)| RecordedPreset {
            id: key.parse().expect("a preset is keyed by its number"),
            pool: string_at(value, "pool"),
            name: string_at(value, "name"),
            color: match member(value, "color") {
                Some(JsonValue::Object(_)) => Some(RgbColor {
                    r: u8::try_from(int_at(member(value, "color").expect("checked"), "r"))
                        .unwrap_or(0),
                    g: u8::try_from(int_at(member(value, "color").expect("checked"), "g"))
                        .unwrap_or(0),
                    b: u8::try_from(int_at(member(value, "color").expect("checked"), "b"))
                        .unwrap_or(0),
                }),
                _ => None,
            },
            values: match member(value, "values") {
                Some(JsonValue::Array(values)) => values.len(),
                _ => 0,
            },
        })
        .collect();
    rows.sort_by_key(|row| row.id);
    rows
}

/// The cue list in force, out of the **session** document — S39.
fn selected_sequence_of(session: &JsonValue) -> Option<u32> {
    match member(member(session, "session")?, "selectedSequence")? {
        &JsonValue::Int(number) => u32::try_from(number).ok(),
        _ => None,
    }
}

/// The update state, out of the session document — S39.
fn editing_cue_of(session: &JsonValue) -> Option<RecordedCueEdit> {
    // The member is there and it is `null` when nothing is being edited, which
    // is the ordinary case for most of this script — so the absence is read
    // rather than treated as a missing key.
    let editing = match member(member(session, "session")?, "editingCue")? {
        JsonValue::Null => return None,
        value => value,
    };
    Some(RecordedCueEdit {
        sequence_id: u32::try_from(int_at(editing, "sequenceId")).ok()?,
        cue_number: string_at(editing, "cueNumber"),
        modified: bool_at(editing, "modified"),
    })
}

/// The executor grid, out of the show document.
fn executors_of(show: &JsonValue) -> Vec<RecordedExecutor> {
    let JsonValue::Object(executors) = member_or_null(show, "executors") else {
        return Vec::new();
    };
    let mut rows: Vec<RecordedExecutor> = executors
        .iter()
        .map(|(key, value)| RecordedExecutor {
            id: key.parse().expect("an executor is keyed by its number"),
            sequence_id: match member(value, "sequenceId") {
                Some(&JsonValue::Int(number)) => u32::try_from(number).ok(),
                _ => None,
            },
            is_active: bool_at(value, "isActive"),
            current_cue_index: match member(value, "currentCueIndex") {
                Some(&JsonValue::Int(number)) => u32::try_from(number).ok(),
                _ => None,
            },
        })
        .collect();
    rows.sort_by_key(|row| row.id);
    rows
}

/// The member at `key`, or `JsonValue::Null` when there is none.
fn member_or_null(value: &JsonValue, key: &str) -> JsonValue {
    member(value, key).cloned().unwrap_or(JsonValue::Null)
}

/// The member at `key` of an object.
fn member<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    match value {
        JsonValue::Object(members) => members.get(key),
        _ => None,
    }
}

/// The string member at `key`.
fn string_at(value: &JsonValue, key: &str) -> String {
    match member(value, key) {
        Some(JsonValue::String(text)) => text.clone(),
        other => panic!("{key} is not a string: {other:?}"),
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
    matches!(member(value, key), Some(&JsonValue::Bool(true)))
}

/// The number at `key`, whether the document wrote it as an integer or a float.
///
/// A fade of exactly 3 seconds is an `Int` in the projection and 5.5 is a
/// `Float`, so a reader that only understood one of them would report a whole
/// number of seconds as no seconds at all.
fn float_at(value: &JsonValue, key: &str) -> f64 {
    match member(value, key) {
        Some(&JsonValue::Float(number)) => number,
        #[expect(clippy::cast_precision_loss, reason = "a cue time is small")]
        Some(&JsonValue::Int(number)) => number as f64,
        other => panic!("{key} is not a number: {other:?}"),
    }
}

/// The same, where absence is an ordinary answer.
fn optional_float_at(value: &JsonValue, key: &str) -> Option<f64> {
    match member(value, key) {
        None | Some(&JsonValue::Null) => None,
        Some(_) => Some(float_at(value, key)),
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

/// The answer inside a recorded payload.
fn answer_of(encoded: &str) -> Answer {
    match decode(&common::decode_base64(encoded)) {
        ServerMessage::Answer { answer, .. } => answer,
        other => panic!("that payload is not an answer: {other:?}"),
    }
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

/// The step whose `what` contains this text.
///
/// Found by what the step is *for* rather than by a number — S44's finding: the
/// script grows, and an index written down here would silently start pointing at
/// another step.
fn step<'a>(recording: &'a Recording, about: &str) -> &'a Step {
    recording
        .steps
        .iter()
        .find(|step| step.what.contains(about))
        .unwrap_or_else(|| panic!("no step is about {about:?}"))
}

/// The preview a query step was answered with.
fn preview_at(recording: &Recording, about: &str) -> prism_domain::StorePreview {
    let step = step(recording, about);
    match answer_of(
        step.answer
            .as_deref()
            .unwrap_or_else(|| panic!("the step about {about:?} has no answer")),
    ) {
        Answer::StorePreview { preview } => preview,
        other => panic!("the step about {about:?} is not a store preview: {other:?}"),
    }
}

/// **A cue loaded with `EditCue` and put straight back is byte-identical** —
/// S39's exit criterion, asserted on the recording rather than in a unit test,
/// so it is true of the cue a *daemon* ended up holding.
///
/// The one that carries the session: a cue's parts include their `presetRef`,
/// and cue 3 is the one the script stored out of an applied preset — so if the
/// load dropped the links, the cue after the update would differ from the cue
/// before it in the only field that is easy to lose and impossible to see.
#[test]
fn a_cue_loaded_and_put_back_unchanged_is_the_cue_that_was_loaded() {
    let recording = recording();
    let before = cue_named(step(&recording, "load cue 3 back into the programmer"), "3");
    let after = cue_named(step(&recording, "put it straight back"), "3");
    assert_eq!(
        before, after,
        "a load and an update with nothing changed moved the cue"
    );
    assert!(
        before.parts.iter().any(|part| part.preset_ref.is_some()),
        "the cue this is asserted on carries no preset link, so it could not \
         have caught one being dropped: {before:?}"
    );
    // And **every** part of it kept its link, not merely one of them. Cue 3 was
    // stored out of an applied preset, so all six of its parts carry one — the
    // three whites included, because `White` is a *colour* attribute on an RGBW
    // profile and the pool is the profile's answer rather than the name's (S28).
    assert_eq!(
        before
            .parts
            .iter()
            .filter(|part| part.preset_ref == Some(1))
            .count(),
        before.parts.len(),
        "a load-and-update dropped a preset link: {before:?}"
    );
    assert_eq!(before.parts.len(), 6);
}

/// **The update state, and the three things that clear it** — S39.
///
/// Read off the session document a *fresh* client is served, so it is state the
/// daemon holds rather than something a client worked out: an Update key blinks
/// on `editingCue.modified`, and a second screen has to blink it too.
#[test]
fn the_update_state_says_which_cue_is_loaded_and_whether_it_has_moved() {
    let recording = recording();

    // Nothing is loaded until something loads one.
    assert!(
        recording.steps[0].editing_cue.is_none(),
        "a fresh desk is editing nothing"
    );

    let loaded = step(&recording, "load cue 3 back into the programmer")
        .editing_cue
        .clone()
        .expect("EditCue puts the desk into an edit");
    assert_eq!(loaded.sequence_id, 1);
    assert_eq!(loaded.cue_number, "3");
    assert!(
        !loaded.modified,
        "a cue just loaded has not been changed since it was loaded"
    );

    // An Update leaves the edit standing and stops the key blinking.
    let updated = step(&recording, "put it straight back")
        .editing_cue
        .clone()
        .expect("an update does not end the edit");
    assert!(!updated.modified);

    // And a programmer edit while a cue is loaded is what makes it blink.
    let touched = step(&recording, "now change something while the cue is loaded")
        .editing_cue
        .clone()
        .expect("the edit survives an encoder");
    assert_eq!(touched.cue_number, "3");
    assert!(
        touched.modified,
        "this is the state an Update key blinks on, and it did not arrive"
    );
    assert!(
        !step(&recording, "and put that back")
            .editing_cue
            .clone()
            .expect("still editing")
            .modified,
        "the second update did not stop the key blinking"
    );

    // Clearing the programmer clears it — the values it was holding are gone.
    assert!(
        step(
            &recording,
            "an Update after the programmer has been cleared"
        )
        .editing_cue
        .is_none(),
        "a cleared programmer is not editing a cue"
    );
    // ...and the Update that follows is refused rather than storing something.
    assert!(step(&recording, "...and this is the refusal").refused);
}

/// **The cue list in force is the desk's, and it is a field of its own** — S39's
/// other decision, seen from the wire, with S40's one amendment.
///
/// The amendment: `Store Sequence 1` on a **free** number both makes the cue
/// list and puts it in force, which is why the very first step of the script
/// already has one. A bare `Store Cue 1` names no list and means the selected
/// one (§4.1), so a desk that made a cue list and went on pointing somewhere
/// else would send the next store into the wrong place. A store into a list that
/// already exists leaves the selection alone, and the last three `StoreSequence`
/// steps of the script are exactly that.
#[test]
fn the_selected_sequence_is_session_state() {
    let recording = recording();
    assert_eq!(
        recording.steps[0].selected_sequence,
        Some(1),
        "making a cue list did not put it in force"
    );
    let chosen = step(&recording, "and put it in force");
    assert_eq!(chosen.selected_sequence, Some(1));
    // And it stays chosen for the rest of the script, because nothing else in
    // it selects one — including the steps that assign executors, which is the
    // coupling S39 deliberately did not build, and the later stores into a cue
    // list that is already there.
    assert!(
        recording
            .steps
            .iter()
            .all(|step| step.selected_sequence == Some(1)),
        "something moved the selected sequence that was not a SelectSequence"
    );
}

/// **A store into a whole cue list** — S39's `StoreSequence`.
#[test]
fn appending_adds_a_cue_at_the_highest_number_and_merging_reaches_every_cue() {
    let recording = recording();
    let before = &step(&recording, "the same Remove again").sequences[0].cues;
    let appended = &step(&recording, "append the programmer to the cue list").sequences[0].cues;
    assert_eq!(
        appended.len(),
        before.len() + 1,
        "an Append leaves every cue that was there and adds one"
    );
    assert_eq!(
        appended.last().expect("a cue was appended").number,
        "4",
        "the highest number in 1, 1.5, 3 is 3, so the new cue is 4"
    );

    // And a Merge writes into **every** cue, which is what makes it the
    // sequence-level Merge rather than a second Append.
    let merged = &step(&recording, "merge it into every cue there is").sequences[0].cues;
    assert_eq!(merged.len(), appended.len(), "a Merge adds no cue");
    for (was, now) in appended.iter().zip(merged) {
        assert_eq!(was.number, now.number, "a Merge renumbered a cue");
        assert!(
            now.parts
                .iter()
                .any(|part| part.attribute == "Green" && part.value == 12345),
            "cue {} did not get the merged value",
            now.number
        );
        assert!(
            now.parts.len() >= was.parts.len(),
            "a Merge took something out of cue {}",
            now.number
        );
    }
}

/// The cue with this number, out of the sequences a step recorded.
fn cue_named(step: &Step, number: &str) -> RecordedCue {
    step.sequences
        .iter()
        .flat_map(|sequence| sequence.cues.iter())
        .find(|cue| cue.number == number)
        .unwrap_or_else(|| panic!("no cue {number} in {:?}", step.what))
        .clone()
}

/// **The recorded rows are what the show document actually says.**
///
/// The deltas are replayed through `prism_core`'s own `ShowMirror` and every
/// step's sequences, presets and executors are read back out of it, so the three
/// claims the browser is held to are checked here first, in the language that
/// produced them.
#[test]
fn the_recorded_rows_are_what_the_show_document_says() {
    let recording = recording();
    assert_eq!(
        recording.protocol_version,
        prism_ipc::PROTOCOL_VERSION,
        "the recording is of another protocol version"
    );
    assert_eq!(recording.steps.len(), script().len());

    let start = snapshot_of(&recording.initial_snapshot);
    let mut show = ShowMirror::new(start.show.clone());

    for (index, (step, scripted)) in recording.steps.iter().zip(script()).enumerate() {
        let (what, expected, is_query) = match scripted {
            Scripted::Do(what, command) => (
                what,
                ClientMessage::Command {
                    seq: u64::try_from(index).unwrap_or(0) + 1,
                    command,
                },
                false,
            ),
            Scripted::Ask(what, query) => (
                what,
                ClientMessage::Query {
                    seq: u64::try_from(index).unwrap_or(0) + 1,
                    query,
                },
                true,
            ),
        };
        assert_eq!(step.what, what, "step {index} is not the one in the script");
        assert_eq!(step.is_query, is_query, "step {index}: query or command");
        // The payload is the message the script names, and not merely *a*
        // message: a recording whose bytes had drifted from the script would
        // otherwise be compared against itself.
        assert_eq!(
            common::decode_base64(&step.client),
            prism_ipc::encode(&expected).expect("it encodes"),
            "step {index}: the recorded payload is not this message"
        );

        for (order, encoded) in step.deltas.iter().enumerate() {
            let ServerMessage::Delta { delta } = decode(&common::decode_base64(encoded)) else {
                panic!("step {index} delta {order} is not a delta");
            };
            show.apply_delta(&delta)
                .unwrap_or_else(|error| panic!("step {index} delta {order}: {error}"));
        }

        assert_eq!(
            sequences_of(show.value()),
            step.sequences,
            "step {index} ({what}): the sequences"
        );
        assert_eq!(
            presets_of(show.value()),
            step.presets,
            "step {index} ({what}): the presets"
        );
        assert_eq!(
            executors_of(show.value()),
            step.executors,
            "step {index} ({what}): the executors"
        );
    }

    // And what the deltas built is what a client that never saw one is served.
    let end = snapshot_of(&recording.final_snapshot);
    assert_eq!(
        show.value(),
        &end.show,
        "the show deltas and the snapshot disagree"
    );
}

/// **A question changes nothing**, which is what makes it safe to ask one per
/// keystroke on a desk that is running a show — §5.2's first rule, re-asserted
/// for the variant S28 added.
#[test]
fn a_store_preview_changes_nothing() {
    let recording = recording();
    let queries: Vec<usize> = recording
        .steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.is_query)
        .map(|(index, _)| index)
        .collect();
    assert!(queries.len() >= 5, "only {} questions", queries.len());
    for index in queries {
        let step = &recording.steps[index];
        assert!(step.deltas.is_empty(), "step {index} broadcast something");
        assert!(step.answer.is_some(), "step {index} was not answered");
        assert!(!step.refused, "step {index} was refused");
        let before = &recording.steps[index - 1];
        assert_eq!(
            before.sequences, step.sequences,
            "step {index} moved the sequences"
        );
        assert_eq!(
            before.presets, step.presets,
            "step {index} moved the presets"
        );
        assert_eq!(
            before.executors, step.executors,
            "step {index} moved the executors"
        );
    }
}

/// **A preview is what the store that follows it does.**
///
/// The claim the exit criterion rests on, checked against the recording rather
/// than against `prism_core`: the counts a preview answered with are compared
/// with the cue the *daemon* ended up holding after the store.
#[test]
fn a_preview_says_what_the_store_that_follows_it_does() {
    let recording = recording();

    // A create: three values in, nothing there, nothing kept.
    let create = preview_at(&recording, "it does not exist yet");
    assert!(create.accepted);
    assert!(!create.exists);
    assert_eq!((create.added, create.replaced, create.kept), (3, 0, 0));
    assert_eq!(create.mode, StoreMode::Merge);
    let after = &step(&recording, "store it").sequences;
    let cue = &after[0].cues[0];
    assert_eq!(cue.number, "1");
    assert_eq!(
        cue.parts.len(),
        usize::try_from(create.added + create.replaced + create.kept).expect("a small count")
    );

    // And an overwrite: **the counts are what an operator is told before they
    // press the button**, and the cue afterwards is their sum.
    let overwrite = preview_at(&recording, "this is the overwrite an operator");
    assert!(overwrite.accepted, "merge is not a refusal");
    assert!(overwrite.exists);
    assert_eq!(overwrite.mode, StoreMode::Merge);
    assert_eq!(
        (overwrite.added, overwrite.replaced, overwrite.kept),
        (2, 3, 0),
        "the two greens added and the three reds replaced"
    );
    let merged = &step(&recording, "store it as a Merge").sequences[0].cues[0];
    assert_eq!(merged.number, "1");
    assert_eq!(
        merged.parts.len(),
        usize::try_from(overwrite.added + overwrite.replaced + overwrite.kept)
            .expect("a small count"),
        "the merged cue is not the sum the preview promised"
    );

    // A preset edit: everything replaced and nothing added, which is exactly
    // what makes it an *edit* rather than a second preset.
    // **The number that makes Merge legible.** The programmer holds three blues
    // and the preset holds those three plus three whites, so the store replaces
    // three and *keeps* three — which is exactly what an Override would have
    // thrown away, and the reason S28 answers with counts rather than a warning.
    let edit = preview_at(&recording, "the three blues are replaced");
    assert_eq!((edit.added, edit.replaced, edit.kept), (0, 3, 3));
    assert_eq!(
        edit.name, "Deep blue",
        "the preview names what is there now"
    );

    // **The same store in the other two modes, asked one after the other** —
    // S39, and the whole of what a mode chooser is for: the operator sees what
    // each choice costs before choosing. The keys are identical in all three, so
    // the only thing that moves is which column the numbers land in.
    let overriding = preview_at(&recording, "the same store as an Override");
    assert_eq!(overriding.mode, StoreMode::Override);
    assert!(overriding.accepted);
    assert_eq!(
        (
            overriding.added,
            overriding.replaced,
            overriding.kept,
            overriding.removed
        ),
        (2, 3, 0, 0),
        "this programmer holds every value the cue holds, so an Override throws \
         nothing away and reads exactly like the Merge beside it"
    );
    let removing = preview_at(&recording, "and as a Remove");
    assert_eq!(removing.mode, StoreMode::Remove);
    assert!(removing.accepted);
    assert_eq!(
        (
            removing.added,
            removing.replaced,
            removing.kept,
            removing.removed
        ),
        (0, 0, 0, 3),
        "a Remove writes nothing at all: the three shared values are what goes"
    );
    // And the three describe one cue: whatever the mode, the counts account for
    // every value that is filed under that number.
    for preview in [&overwrite, &overriding, &removing] {
        assert_eq!(
            preview.kept + preview.replaced + preview.removed,
            3,
            "the four counts do not add up to the cue that is there: {preview:?}"
        );
    }

    // **And the pair where the modes really differ**, which is the pair an
    // operator is choosing between: the programmer holds one value, and the cue
    // holds four others.
    let destructive = preview_at(
        &recording,
        "one an \
             operator has to be warned about",
    );
    assert_eq!(
        (
            destructive.added,
            destructive.replaced,
            destructive.kept,
            destructive.removed
        ),
        (0, 1, 0, 4),
        "an Override here throws four values away, and that is the number the \
         Store button has to be able to say"
    );
    let after_override = &step(&recording, "store it as an Override").sequences[0].cues[0];
    assert_eq!(after_override.number, "1");
    assert_eq!(
        after_override.parts.len(),
        1,
        "the cue holds exactly what the programmer held"
    );
    let taking_out = preview_at(&recording, "a Remove of the same value from cue 1.5");
    assert_eq!(
        (
            taking_out.added,
            taking_out.replaced,
            taking_out.kept,
            taking_out.removed
        ),
        (0, 0, 4, 1),
        "a Remove takes one value out and leaves the four the programmer never \
         mentioned"
    );
    let after_remove = &step(&recording, "take it out with a **Remove**").sequences[0].cues[1];
    assert_eq!(after_remove.number, "1.5");
    assert_eq!(
        after_remove.parts.len(),
        usize::try_from(taking_out.kept).expect("a small count"),
        "the cue after a Remove is what the preview called kept"
    );
    // And the second Remove is refused, having nothing left to take.
    assert!(
        step(&recording, "the same Remove again").refused,
        "a store that would remove nothing is refused rather than applied"
    );

    // Two refusals, each said before anything was sent.
    let missing = preview_at(&recording, "a sequence that is not there");
    assert!(!missing.accepted);
    assert!(!missing.exists);
    assert!(
        missing
            .refusal
            .as_deref()
            .is_some_and(|why| why.contains("404")),
        "{:?}",
        missing.refusal
    );
    let nothing = preview_at(&recording, "nothing to store");
    assert!(!nothing.accepted);
    assert!(
        nothing.exists,
        "a refused store still says what is filed under that number"
    );
}

/// **A preset link is alive: editing the preset moved the cue that references
/// it.**
///
/// The session's hardest claim, asserted on the recording — so the browser is
/// held to a show the daemon actually produced rather than to an expectation
/// written twice.
#[test]
fn editing_a_preset_moved_the_cue_that_references_it() {
    let recording = recording();
    let stored = step(&recording, "its parts carry the preset reference");
    let cue = stored.sequences[0]
        .cues
        .iter()
        .find(|cue| cue.number == "3")
        .expect("cue 3 was stored");
    // Six, not three: applying preset 1 put **everything the preset holds** into
    // the programmer, and the preset holds the blues and the whites — the second
    // of which is on the Colour bank for this profile, which is the profile's
    // answer and not the attribute name's.
    assert_eq!(cue.parts.len(), 6);
    assert!(
        cue.parts.iter().all(|part| part.preset_ref == Some(1)),
        "the parts do not carry the preset reference: {:?}",
        cue.parts
    );
    let blue_before = cue
        .parts
        .iter()
        .find(|part| part.fixture == 1 && part.attribute == "Blue")
        .expect("fixture 1's blue")
        .value;

    // The preset is edited, and the *cue values* are what change with it —
    // which the recorded document says because the deltas said so, replayed by
    // `the_recorded_rows_are_what_the_show_document_says`.
    let edited = step(&recording, "which is the whole claim").sequences[0]
        .cues
        .iter()
        .find(|cue| cue.number == "3")
        .expect("cue 3 is still there")
        .clone();
    let blue_after = edited
        .parts
        .iter()
        .find(|part| part.fixture == 1 && part.attribute == "Blue")
        .expect("fixture 1's blue")
        .value;
    assert_ne!(
        blue_before, blue_after,
        "the preset edit did not reach the cue at all"
    );
    // Every one of the three blues followed, and the three whites the store did
    // not mention stayed exactly where they were — which is the *kept* the
    // preview promised, seen in the show rather than in a number.
    for part in &edited.parts {
        assert_eq!(part.preset_ref, Some(1), "the edit broke a link");
        if part.attribute == "Blue" {
            assert_eq!(part.value, blue_after, "a blue was left behind");
        }
    }
    assert_eq!(
        edited
            .parts
            .iter()
            .filter(|part| part.attribute == "White")
            .map(|part| part.value)
            .collect::<Vec<_>>(),
        cue.parts
            .iter()
            .filter(|part| part.attribute == "White")
            .map(|part| part.value)
            .collect::<Vec<_>>(),
        "the whites moved, and the store never mentioned them"
    );

    // And the preset itself is the one that was stored over, not a second one.
    let presets = &step(&recording, "which is the whole claim").presets;
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Darker blue");
    assert_eq!(presets[0].pool, "Color");
}

/// A preset takes the values of its own pool and leaves the rest.
///
/// The programmer held a `White` value at the moment preset 1 was stored, and
/// `White` is on the Colour bank for this profile — so the assertion is the
/// other way round: the recorded preset holds exactly the colour values the
/// *daemon* selected, which is four rather than the three an interface counting
/// blues would have guessed.
#[test]
fn a_preset_holds_what_the_daemon_put_in_it() {
    let recording = recording();
    let created = &step(
        &recording,
        "store it, with a name and a scribble-strip colour",
    )
    .presets;
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].id, 1);
    assert_eq!(created[0].name, "Deep blue");
    assert_eq!(
        created[0].color,
        Some(RgbColor { r: 0, g: 0, b: 255 }),
        "the scribble-strip colour did not survive"
    );
    let preview = preview_at(&recording, "a create, and it counts the colour values only");
    assert_eq!(
        created[0].values,
        usize::try_from(preview.added).expect("a small count"),
        "the preset holds a different number of values from the one previewed"
    );
}

/// **The cue index is a number**, and this test is what a note that cannot be
/// forgotten turns into once it has been read.
///
/// Until S34 it said the opposite: `Executor::currentCueIndex` was in the domain
/// and on the wire with nothing filling it — what cue a playback is on lives on
/// the tick thread and there was no channel back (S26's finding) — so this
/// demanded that every recorded index was `null`, and said in its own message
/// that whoever built the channel would find it red and should regenerate the
/// recording. `prism_engine::PlaybackReport` is that channel.
///
/// What it demands now is the claim in both directions, which is stronger than
/// *some index is filled in*: a strip that is running is on a cue, one that is
/// not is on none, and the number **moves**.
#[test]
fn the_cue_index_is_a_number_now() {
    let recording = recording();

    // **The inverse of what this test said until S34**, which is why it is here
    // at all: S28 wrote it to demand that every recorded `currentCueIndex` was
    // `null`, with a message telling whoever built the readback to regenerate
    // the recording and turn it round. This is that.
    for (index, step) in recording.steps.iter().enumerate() {
        for executor in &step.executors {
            assert_eq!(
                executor.current_cue_index.is_some(),
                executor.is_active,
                "step {index}: executor {} is active={} and its cue index is {:?}",
                executor.id,
                executor.is_active,
                executor.current_cue_index
            );
        }
    }

    // A running executor is on a cue, a stopped one is on none, and the script
    // contains both — so neither half of the claim above is vacuous.
    assert!(
        recording
            .steps
            .iter()
            .any(|step| step.executors.iter().any(|executor| executor.is_active)),
        "nothing was ever running, so the readback was never exercised"
    );
    assert!(
        recording
            .steps
            .iter()
            .any(|step| step.executors.iter().all(|executor| !executor.is_active)),
        "nothing was ever stopped"
    );

    // And the index **moved**, which is the part a readback that reported a
    // constant zero would fail.
    let seen: std::collections::BTreeSet<u32> = recording
        .steps
        .iter()
        .flat_map(|step| {
            step.executors
                .iter()
                .filter_map(|executor| executor.current_cue_index)
        })
        .collect();
    assert!(
        seen.len() >= 2,
        "every recorded cue index was the same: {seen:?}"
    );
}

/// The script is a show being written, not a list that happens to apply.
///
/// Every claim the browser's tests lean on is asserted here to be *in* the
/// recording, so a fixture that quietly stopped exercising the interesting case
/// would not leave those tests passing over nothing.
#[test]
fn the_recording_is_of_a_show_being_written() {
    let recording = recording();
    let steps = &recording.steps;

    // It starts from nothing at all, which is the state a fresh show is in and
    // the one the sheets have to be legible in.
    assert!(steps[0].sequences.is_empty() || steps[0].sequences[0].cues.is_empty());
    assert!(
        recording.steps.iter().all(|step| step.presets.len() <= 1),
        "the script was meant to write one preset"
    );

    // The cue list grows, is corrected, and shrinks.
    let counts: Vec<usize> = steps
        .iter()
        .map(|step| step.sequences.iter().map(|row| row.cues.len()).sum())
        .collect();
    let most = counts.iter().max().copied().unwrap_or(0);
    assert!(most >= 3, "the cue list never grew: {counts:?}");
    assert!(
        counts
            .iter()
            .skip_while(|count| **count < most)
            .any(|count| *count < most),
        "no cue was ever deleted: {counts:?}"
    );

    // A cue that changed its **number**, which is what `SetCueProperty::Number`
    // is for and what nothing before S28 could express.
    assert!(
        steps.iter().any(|step| step
            .sequences
            .iter()
            .any(|row| row.cues.iter().any(|cue| cue.number == "1.5"))),
        "no cue was renumbered"
    );
    // A cue with a name, a fade and a trigger time — the three columns a cue
    // sheet has that a cue list does not.
    assert!(
        steps.iter().any(|step| step
            .sequences
            .iter()
            .any(|row| row.cues.iter().any(|cue| cue.name == "Green wash"
                && (cue.fade_in - 5.5).abs() < f64::EPSILON
                && cue.trigger == "Time"
                && cue.trigger_time == Some(4.0)))),
        "no cue ever carried a name, a fade and a trigger together"
    );

    // Refusals, and none of them changes anything at all.
    let refusals: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.refused)
        .map(|(index, _)| index)
        .collect();
    assert!(refusals.len() >= 5, "only {} refusals", refusals.len());
    for index in refusals {
        assert!(
            steps[index].deltas.is_empty(),
            "step {index} said something"
        );
        assert_eq!(
            steps[index - 1].sequences,
            steps[index].sequences,
            "step {index} moved the sequences"
        );
        assert_eq!(
            steps[index - 1].presets,
            steps[index].presets,
            "step {index} moved the presets"
        );
    }

    // An executor gained a sequence, which is what makes a cue list firable.
    assert!(
        steps
            .iter()
            .any(|step| step.executors.iter().any(|row| row.sequence_id == Some(1))),
        "no cue list ever reached an executor"
    );

    // And an Oops at the end, so the browser can see that a show edit is
    // undoable like every other one (`ARCHITECTURE_SPEC.md` §6.1).
    let last = steps.last().expect("there are steps");
    assert!(!last.is_query);
    assert!(!last.refused, "the Oops was refused");
    assert_ne!(
        (&last.sequences, &last.presets),
        (
            &steps[steps.len() - 2].sequences,
            &steps[steps.len() - 2].presets
        ),
        "the Oops took nothing back"
    );
}

/// The rig the browser opens is the rig this file writes.
///
/// Checked on the committed file rather than on the function that wrote it, so a
/// fixture regenerated against a different show fails here.
#[test]
fn the_committed_rig_is_the_one_this_file_describes() {
    let path = rig_path();
    let store = ShowStore::open(&path).unwrap_or_else(|error| {
        panic!("{} is missing: {error}", path.display());
    });
    let mut file = ShowFile::new();
    store
        .load(&mut file)
        .expect("the rig opens with this build");

    assert_eq!(file.show.fixtures().count(), 4, "the rig is four fixtures");
    assert_eq!(file.show.fixture_types().count(), 2);
    assert_eq!(file.show.conflicts(), Vec::new(), "the rig overlaps");
    // **Nothing is stored**, which is what makes every look in the recording one
    // the script made.
    assert_eq!(file.show.sequences().count(), 0);
    assert_eq!(file.show.presets().count(), 0);
    assert_eq!(file.show.executors().count(), 0);
    // And both profiles are the desk's own.
    for fixture_type in file.show.fixture_types() {
        assert_eq!(
            prism_core::generic_profiles()
                .into_iter()
                .find(|profile| profile.id == fixture_type.id)
                .as_ref(),
            Some(fixture_type),
            "{} is not the desk's own profile",
            fixture_type.id
        );
    }
}
