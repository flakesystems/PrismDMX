//! A rig being built — recorded off a running daemon, so the browser's patch
//! window is held to the daemon's answers rather than to a second opinion in
//! TypeScript.
//!
//! # Why a fifth recording
//!
//! The four before it record a conversation (S23), a canvas (S25), a desk being
//! operated (S26) and a telemetry channel (S24). This one records the two things
//! S27 needed and nothing else could answer:
//!
//! - **what a patch edit does to the show** — including the two commands the
//!   protocol did not have before this session, `UnpatchFixture` and
//!   `RenumberFixture`, and the third, `EmbedFixtureType`, which is what lets a
//!   brand-new show be patched at all;
//! - **what the daemon says a patch *would* do, before it does it** — which is
//!   the whole of the exit criterion *address conflicts are shown before they
//!   are committed*, and which arrives over the protocol's third shape, the
//!   `Query` S27 added.
//!
//! A preview is exactly the kind of answer a client could plausibly work out for
//! itself — it is one interval intersection — which is why it is recorded rather
//! than reimplemented. `prism_core::conflict` decides; the browser draws.
//!
//! # It is frozen, and the guards run on every commit
//!
//! [`record_the_patch_script_for_the_interface`] is `#[ignore]`d like the other
//! four regenerators. What runs always replays the deltas through
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
use prism_domain::{Answer, Command, FixtureId, FixtureType, JsonValue, Query, UniverseId};
use prism_ipc::{ClientKind, ClientMessage, Hello, ServerMessage, Snapshot, Wire, local};
use prismd::cli::{Options, OutputSpec};
use prismd::daemon::Daemon;

mod common;

/// How long any wait here may take before it is a named failure.
const PATIENCE: Duration = Duration::from_secs(20);

/// Where the interface reads the recording from.
fn recording_path() -> PathBuf {
    common::ui_fixture("patch-recording.json")
}

/// Where the end-to-end suite reads the rig from.
fn rig_path() -> PathBuf {
    common::ui_fixture("patch-rig.prism")
}

/* -------------------------------------------------------------------------- */
/* The rig                                                                    */
/* -------------------------------------------------------------------------- */

/// The show the script starts from: **two profiles and three fixtures**, and
/// nothing overlapping.
///
/// Deliberately small and deliberately clean. Everything interesting in this
/// recording is something the *script* does to it, so a browser reading the
/// first step is reading a rig rather than a rig plus a mystery. The profiles
/// are the desk's own — `prism_core::fixture_library` — embedded here the way
/// `Command::EmbedFixtureType` embeds them, so the recorded rig and the recorded
/// library agree by construction.
fn patch_show() -> ShowFile {
    let mut show = Show::new();
    for type_id in ["generic.dimmer", "generic.rgbw.par"] {
        show.embed_fixture_type(
            prism_core::library_type(type_id).expect("the desk carries this profile"),
        )
        .expect("a library profile is one a show accepts");
    }
    for (id, type_id, universe, address) in [
        (1_u32, "generic.dimmer", 1_u32, 1_u16),
        (2, "generic.dimmer", 1, 2),
        (5, "generic.rgbw.par", 1, 20),
    ] {
        show.patch_fixture(prism_domain::Fixture {
            id: FixtureId::new(id),
            name: format!("Fixture {id}"),
            type_id: type_id.to_owned(),
            universe: UniverseId::new(universe),
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

/// Writes the rig into a `.prism` file for the daemon and the browser to open.
fn write_rig(path: &Path) {
    let mut file = patch_show();
    let mut store = ShowStore::open(path).expect("a show file opens");
    store.save(&mut file).expect("a show file saves");
}

/* -------------------------------------------------------------------------- */
/* The file                                                                   */
/* -------------------------------------------------------------------------- */

/// One line of the patch sheet, as a reader has to produce it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedRow {
    id: u32,
    name: String,
    type_id: String,
    /// The profile's **name**, which is what a sheet shows — falling back to the
    /// key when the show does not carry the profile.
    type_name: String,
    universe: u32,
    address: u16,
    /// How wide it is, or 0 when the show cannot say.
    footprint: u16,
}

/// One embedded profile, as the type menu has to produce it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedProfile {
    id: String,
    manufacturer: String,
    name: String,
    mode: String,
    footprint: u16,
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
    /// **by construction**: a question changes nothing, so there is nothing to
    /// broadcast.
    deltas: Vec<String>,
    /// The `ServerMessage::Answer` payload, base64, when this was a query.
    answer: Option<String>,
    /// Whether the daemon refused it.
    refused: bool,
    /// The patch afterwards — the daemon's own answer, from a fresh snapshot.
    rows: Vec<RecordedRow>,
    /// The profiles the show carries afterwards.
    profiles: Vec<RecordedProfile>,
}

/// The whole file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recording {
    /// What made it, and how to make it again.
    note: String,
    /// The protocol version these payloads belong to.
    protocol_version: u32,
    /// The profiles **this desk** offers, as the snapshot carries them.
    ///
    /// Recorded so the browser's menu is compared with `prism_core::library`
    /// rather than merely derived from the same idea of it.
    fixture_library: Vec<FixtureType>,
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

/// A preview of patching a PAR, which is the query the form asks per keystroke.
fn preview_at(id: u32, universe: u32, address: u16) -> Query {
    Query::PatchPreview {
        id: FixtureId::new(id),
        type_id: "generic.rgbw.par".to_owned(),
        universe: UniverseId::new(universe),
        address,
    }
}

/// The script, in order: **a rig being built, corrected and taken apart.**
///
/// Written out rather than generated. Every step is one a patch window can get
/// wrong, and the queries are interleaved where a form would actually ask them —
/// before the patch, not after it.
fn script() -> Vec<Scripted> {
    vec![
        Scripted::Ask(
            "what overlaps in the rig as it stands: nothing",
            Query::PatchConflicts,
        ),
        Scripted::Ask(
            "would a PAR fit at address 30? free, and the daemon says where it ends",
            preview_at(6, 1, 30),
        ),
        Scripted::Do(
            "patch it there",
            Command::PatchFixture {
                id: FixtureId::new(6),
                name: "PAR 6".to_owned(),
                type_id: "generic.rgbw.par".to_owned(),
                universe: UniverseId::new(1),
                address: 30,
            },
        ),
        Scripted::Ask(
            "would a second PAR at 32 clash? **yes, and it is still allowed** — this is \
             the answer the operator is shown before they commit",
            preview_at(7, 1, 32),
        ),
        Scripted::Do(
            "patch it anyway: cloning a fixture onto another is an ordinary technique",
            Command::PatchFixture {
                id: FixtureId::new(7),
                name: "PAR 7".to_owned(),
                type_id: "generic.rgbw.par".to_owned(),
                universe: UniverseId::new(1),
                address: 32,
            },
        ),
        Scripted::Ask(
            "and now the show has an overlap in it",
            Query::PatchConflicts,
        ),
        Scripted::Ask(
            "would it fit at 510? no — four channels from 510 runs past the end",
            preview_at(7, 1, 510),
        ),
        Scripted::Ask(
            "and a profile the show has not got is refused before it is sent",
            Query::PatchPreview {
                id: FixtureId::new(7),
                type_id: "generic.movinghead".to_owned(),
                universe: UniverseId::new(1),
                address: 100,
            },
        ),
        Scripted::Do(
            "so embed the profile the desk has and the show has not",
            Command::EmbedFixtureType {
                type_id: "generic.movinghead".to_owned(),
            },
        ),
        Scripted::Ask(
            "the same question again, now that the show carries it: eleven channels",
            Query::PatchPreview {
                id: FixtureId::new(7),
                type_id: "generic.movinghead".to_owned(),
                universe: UniverseId::new(1),
                address: 100,
            },
        ),
        Scripted::Do(
            "patch a moving head",
            Command::PatchFixture {
                id: FixtureId::new(8),
                name: "Head 8".to_owned(),
                type_id: "generic.movinghead".to_owned(),
                universe: UniverseId::new(2),
                address: 1,
            },
        ),
        Scripted::Do(
            "correct a name and an address in one go: a repatch, not a new fixture",
            Command::PatchFixture {
                id: FixtureId::new(7),
                name: "PAR 7 (moved)".to_owned(),
                type_id: "generic.rgbw.par".to_owned(),
                universe: UniverseId::new(1),
                address: 40,
            },
        ),
        Scripted::Ask("which takes the overlap away again", Query::PatchConflicts),
        Scripted::Do(
            "renumber 7 to 70: one command, because the number is the key",
            Command::RenumberFixture {
                id: FixtureId::new(7),
                to: FixtureId::new(70),
            },
        ),
        Scripted::Do(
            "renumber onto a number that is taken: refused, and nothing moves",
            Command::RenumberFixture {
                id: FixtureId::new(70),
                to: FixtureId::new(1),
            },
        ),
        Scripted::Do(
            "unpatch a fixture",
            Command::UnpatchFixture {
                id: FixtureId::new(2),
            },
        ),
        Scripted::Do(
            "unpatch one that is not there: refused, and nothing moves",
            Command::UnpatchFixture {
                id: FixtureId::new(99),
            },
        ),
        Scripted::Do(
            "a profile the desk does not carry: refused",
            Command::EmbedFixtureType {
                type_id: "nothing.at.all".to_owned(),
            },
        ),
        Scripted::Do("and take the whole edit back", Command::Oops),
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

/// **The regenerator.** Runs a daemon and writes the rig and the recording.
///
/// ```text
/// cargo test -p prismd --test ui_patch -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rewrites two committed fixtures; run it deliberately"]
async fn record_the_patch_script_for_the_interface() {
    let _turn = common::one_daemon_at_a_time();
    let rig = rig_path();
    std::fs::create_dir_all(rig.parent().expect("the fixture lives in a directory")).unwrap();
    let _ = std::fs::remove_file(&rig);
    write_rig(&rig);

    let dir = tempfile::tempdir().unwrap();
    let show = dir.path().join("patch.prism");
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
    let library = snapshot_of_payload(&initial_snapshot).fixture_library;
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
            rows: rows_of(&snapshot.show),
            profiles: profiles_of(&snapshot.show),
        });
    }

    let (final_wire, final_snapshot) = connect(&address).await;
    final_wire.shutdown().await;
    wire.shutdown().await;

    let recording = Recording {
        note: "Recorded off a running prismd by crates/prismd/tests/ui_patch.rs. \
               The rows, the profiles and every answer are the daemon's own, taken from \
               a fresh client's snapshot. \
               Regenerate with: cargo test -p prismd --test ui_patch -- --ignored"
            .to_owned(),
        protocol_version: prism_ipc::PROTOCOL_VERSION,
        fixture_library: library,
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

/// The patch, out of the show **document** — read by pointer, the way a client
/// reads it, rather than out of a `Show`.
fn rows_of(show: &JsonValue) -> Vec<RecordedRow> {
    let mirror = prism_core::JsonMirror::new(show.clone());
    let Some(JsonValue::Object(fixtures)) = mirror.get("/fixtures").ok().cloned() else {
        return Vec::new();
    };
    let mut rows: Vec<RecordedRow> = fixtures
        .iter()
        .map(|(key, value)| {
            let type_id = string_at(value, "typeId");
            RecordedRow {
                id: key.parse().expect("a fixture is keyed by its number"),
                name: string_at(value, "name"),
                type_name: mirror
                    .get(&format!("/fixtureTypes/{type_id}/name"))
                    .ok()
                    .and_then(|found| match found {
                        JsonValue::String(text) => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| type_id.clone()),
                universe: u32::try_from(int_at(value, "universe")).unwrap_or(0),
                address: u16::try_from(int_at(value, "address")).unwrap_or(0),
                footprint: mirror
                    .get(&format!("/fixtureTypes/{type_id}/footprint"))
                    .ok()
                    .and_then(|found| match found {
                        JsonValue::Int(number) => u16::try_from(*number).ok(),
                        _ => None,
                    })
                    .unwrap_or(0),
                type_id,
            }
        })
        .collect();
    rows.sort_by_key(|row| row.id);
    rows
}

/// The embedded profiles, out of the show document.
fn profiles_of(show: &JsonValue) -> Vec<RecordedProfile> {
    let mirror = prism_core::JsonMirror::new(show.clone());
    let Some(JsonValue::Object(types)) = mirror.get("/fixtureTypes").ok().cloned() else {
        return Vec::new();
    };
    let mut profiles: Vec<RecordedProfile> = types
        .iter()
        .map(|(key, value)| RecordedProfile {
            id: key.clone(),
            manufacturer: string_at(value, "manufacturer"),
            name: string_at(value, "name"),
            mode: string_at(value, "mode"),
            footprint: u16::try_from(int_at(value, "footprint")).unwrap_or(0),
        })
        .collect();
    profiles.sort_by(|left, right| left.id.cmp(&right.id));
    profiles
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

/// **The recorded rows are what the show document actually says.**
///
/// The deltas are replayed through `prism_core`'s own `ShowMirror` and every
/// step's rows and profiles are read back out of it, so the two claims the
/// browser is held to are checked here first, in the language that produced
/// them.
#[test]
fn the_recorded_rows_are_what_the_show_document_says() {
    let recording = recording();
    assert_eq!(
        recording.protocol_version,
        prism_ipc::PROTOCOL_VERSION,
        "the recording is of another protocol version"
    );
    assert_eq!(recording.steps.len(), script().len());
    assert_eq!(
        recording.fixture_library,
        prism_core::fixture_library(),
        "the recorded library is not this desk's"
    );

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
            rows_of(show.value()),
            step.rows,
            "step {index} ({what}): the patch"
        );
        assert_eq!(
            profiles_of(show.value()),
            step.profiles,
            "step {index} ({what}): the profiles"
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

/// **A question changes nothing, and the recording is what says so.**
///
/// Every query step carries an answer, no deltas at all — not an empty list of
/// them, none — and leaves the patch exactly as the step before it did. That is
/// the property the whole third message shape rests on: an interface may ask
/// what a patch would do as often as it likes, on a desk that is running a show.
#[test]
fn a_question_changes_nothing() {
    let recording = recording();
    let queries: Vec<usize> = recording
        .steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.is_query)
        .map(|(index, _)| index)
        .collect();
    assert!(queries.len() >= 6, "only {} questions", queries.len());
    for index in queries {
        let step = &recording.steps[index];
        assert!(step.deltas.is_empty(), "step {index} broadcast something");
        assert!(step.answer.is_some(), "step {index} was not answered");
        assert!(!step.refused, "step {index} was refused");
        if index > 0 {
            let before = &recording.steps[index - 1];
            assert_eq!(before.rows, step.rows, "step {index} moved the patch");
            assert_eq!(
                before.profiles, step.profiles,
                "step {index} moved the profiles"
            );
        }
    }
}

/// **The preview is what the patch that follows it does.**
///
/// The claim the exit criterion rests on, checked against the recording rather
/// than against `prism_core`: step 1 previews a PAR at 30 and step 2 patches it
/// there, step 3 previews one at 32 — reporting an overlap — and step 4 patches
/// it anyway. So the preview's own `lastAddress` and `conflicts` are compared
/// with the rows the daemon actually ended up with.
#[test]
fn a_preview_says_what_the_patch_that_follows_it_does() {
    let recording = recording();
    let preview = |index: usize| match answer_of(
        recording.steps[index]
            .answer
            .as_deref()
            .unwrap_or_else(|| panic!("step {index} has no answer")),
    ) {
        Answer::PatchPreview { preview } => preview,
        other => panic!("step {index} is not a preview: {other:?}"),
    };

    // A clear address: accepted, no overlap, and the end channel the daemon
    // worked out is `address + footprint - 1`.
    let clear = preview(1);
    assert!(clear.accepted);
    assert_eq!(clear.footprint, 4);
    assert_eq!(clear.last_address, Some(33));
    assert!(clear.conflicts.is_empty());
    // The patch that followed put fixture 6 exactly there.
    let after = &recording.steps[2].rows;
    let six = after
        .iter()
        .find(|row| row.id == 6)
        .expect("fixture 6 was patched");
    assert_eq!((six.address, six.footprint), (30, 4));

    // An overlapping address: **accepted all the same**, and the overlap is
    // named before it happens.
    let clashing = preview(3);
    assert!(
        clashing.accepted,
        "cloning a fixture onto another is legal and the preview has to say so"
    );
    assert_eq!(clashing.conflicts.len(), 1);
    let conflict = clashing.conflicts[0];
    assert_eq!(
        (conflict.first, conflict.second),
        (FixtureId::new(6), FixtureId::new(7))
    );
    assert_eq!((conflict.from, conflict.to), (32, 33));
    // And the show the daemon ended up with reports the same pair.
    let Answer::PatchConflicts { conflicts } = answer_of(
        recording.steps[5]
            .answer
            .as_deref()
            .expect("step 5 is a conflicts query"),
    ) else {
        panic!("step 5 is not a conflicts answer");
    };
    assert_eq!(conflicts, vec![conflict]);

    // A refusal, said before the command was ever sent.
    let past_the_end = preview(6);
    assert!(!past_the_end.accepted);
    assert_eq!(past_the_end.last_address, None);
    assert!(past_the_end.conflicts.is_empty());
    assert!(
        past_the_end
            .refusal
            .as_deref()
            .is_some_and(|why| why.contains("510")),
        "a refusal has to say what is wrong: {:?}",
        past_the_end.refusal
    );

    // A profile the show has not got — and the same question again after
    // `EmbedFixtureType`, which is the pair that makes the library worth having.
    let missing = preview(7);
    assert!(!missing.accepted);
    assert_eq!(missing.footprint, 0);
    let embedded = preview(9);
    assert!(embedded.accepted);
    assert_eq!(embedded.footprint, 11);
    assert_eq!(embedded.last_address, Some(110));
}

/// The script is a rig being built, not a list that happens to apply.
///
/// Every claim the browser's tests lean on is asserted here to be *in* the
/// recording, so a fixture that quietly stopped exercising the interesting case
/// would not leave those tests passing over nothing.
#[test]
fn the_recording_is_of_a_rig_being_built() {
    let recording = recording();
    let steps = &recording.steps;

    // The patch grows, shrinks, and is corrected.
    let counts: Vec<usize> = steps.iter().map(|step| step.rows.len()).collect();
    let most = counts.iter().max().copied().unwrap_or(0);
    assert!(most > counts[0], "nothing was ever patched: {counts:?}");
    assert!(
        counts.iter().any(|count| *count < most),
        "nothing was ever unpatched: {counts:?}"
    );

    // A fixture that changed its **number**, which is what `RenumberFixture` is
    // for and what nothing before S27 could express.
    let numbers: Vec<Vec<u32>> = steps
        .iter()
        .map(|step| step.rows.iter().map(|row| row.id).collect())
        .collect();
    assert!(
        numbers.iter().any(|ids| ids.contains(&70)),
        "no fixture was renumbered"
    );
    assert!(
        numbers.iter().any(|ids| ids.contains(&7)),
        "the renumbered fixture never had its old number"
    );

    // A fixture that changed its name and its address without changing number.
    assert!(
        steps
            .iter()
            .any(|step| step.rows.iter().any(|row| row.name.contains("moved"))),
        "no fixture was ever corrected in place"
    );

    // A profile that was embedded, which is what makes a fresh show patchable.
    let profiles: Vec<usize> = steps.iter().map(|step| step.profiles.len()).collect();
    assert!(
        profiles.iter().any(|count| *count > profiles[0]),
        "no profile was embedded: {profiles:?}"
    );
    assert!(
        steps.last().is_some_and(|step| step
            .profiles
            .iter()
            .any(|profile| profile.id == "generic.movinghead")),
        "the moving head is not in the show at the end"
    );

    // Refusals, and none of them changes anything at all.
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
        assert_eq!(
            steps[index - 1].rows,
            steps[index].rows,
            "step {index} moved the patch"
        );
        assert_eq!(
            steps[index - 1].profiles,
            steps[index].profiles,
            "step {index} moved the profiles"
        );
    }

    // And an Oops at the end, so the browser can see that a patch edit is
    // undoable like every other show edit (`ARCHITECTURE_SPEC.md` §6.1).
    let last = steps.last().expect("there are steps");
    assert!(!last.is_query);
    assert!(!last.refused, "the Oops was refused");
    assert_ne!(
        last.rows,
        steps[steps.len() - 2].rows,
        "the Oops took nothing back"
    );
}

/// The rig the browser opens is the rig this file writes.
///
/// Checked on the committed file rather than on the function that wrote it, so
/// a fixture that was regenerated against a different show fails here.
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

    assert_eq!(file.show.fixtures().count(), 3, "the rig is three fixtures");
    assert_eq!(file.show.fixture_types().count(), 2);
    // Clean: everything interesting in the recording is something the script
    // did, so a rig that already overlapped would hide it.
    assert_eq!(file.show.conflicts(), Vec::new());
    // And both profiles are the desk's own, so the recorded library and the
    // recorded rig cannot describe two different dimmers.
    for fixture_type in file.show.fixture_types() {
        assert_eq!(
            prism_core::library_type(&fixture_type.id).as_ref(),
            Some(fixture_type),
            "{} is not the desk's own profile",
            fixture_type.id
        );
    }
}
