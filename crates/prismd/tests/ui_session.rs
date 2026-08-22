//! What the canvas commands do to a session, recorded off a running daemon —
//! and the check that the recording still says what this build would say.
//!
//! # Why this exists beside `ui_recording.rs`
//!
//! That target records a *conversation*: snapshots, deltas, and the snapshot a
//! second client was served, which is what a mirror is held to. This one records
//! the **answers to the questions the canvas asks**: after this command, which
//! windows are open, in what order, at what coordinates, and which one has the
//! focus.
//!
//! The distinction matters because S25 is the first session with readers over
//! the session document — `openWindows`, `focusedWindow`, `activeViewId` — and a
//! reader tested against a document the same language built is a test of
//! nothing. So the expectations here are `prism_core::SessionState`'s own, taken
//! off the wire, and the browser compares what its readers produce against them.
//!
//! # What is in the file
//!
//! - `initialSnapshot` — the session document the recording starts from, base64.
//! - `steps[]` — one command each: the **client payload** `rmp-serde` made of it
//!   (so the interface can compare its encoder byte for byte), the deltas the
//!   daemon answered with, whether it was refused, and then the three answers
//!   above **as the daemon holds them afterwards**.
//! - `finalSnapshot` — what a fresh client is served at the end, which is §9's
//!   *snapshot completeness* row for the session document.
//!
//! # It is frozen, and the guard runs on every commit
//!
//! [`record_the_session_script_for_the_interface`] is `#[ignore]`d, like the
//! other two fixtures' regenerators. What runs always is
//! [`the_recorded_windows_are_what_the_session_document_says`], which replays
//! the deltas through `prism_core::SessionMirror` and checks the recorded
//! answers against the mirrored document — so a session shape that moved fails
//! here, in Rust, rather than going stale in `ui/`.

// This target *writes a file for a person to commit* and says where.
#![allow(clippy::print_stdout)]
// Every test holds `common::one_daemon_at_a_time` across its awaits.
#![allow(clippy::await_holding_lock)]

use std::path::PathBuf;
use std::time::Duration;

use prism_core::SessionMirror;
use prism_domain::{
    Command, JsonValue, ObjectRef, OverwriteMode, ViewId, WindowInstanceId, WindowType,
};
use prism_ipc::{ClientKind, ClientMessage, Hello, ServerMessage, Snapshot, Wire, local};
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;

mod common;

/// How long any wait here may take before it is a named failure.
const PATIENCE: Duration = Duration::from_secs(20);

/// Where the interface reads it from.
fn recording_path() -> PathBuf {
    common::ui_fixture("session-recording.json")
}

/* -------------------------------------------------------------------------- */
/* The file                                                                   */
/* -------------------------------------------------------------------------- */

/// One window, in the shape a canvas asks about it.
///
/// Deliberately not `WindowInstance`: this is what a *reader* is supposed to
/// produce, so it carries exactly the six things the canvas needs and nothing
/// else. `params` is left out because no window in this script has any and a
/// field that is always empty proves nothing.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedWindow {
    instance_id: u32,
    #[serde(rename = "type")]
    window_type: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// One command, and the session it left behind.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    /// What it is, for a person reading the file.
    what: String,
    /// The `ClientMessage::Command` payload, base64.
    client: String,
    /// The same command with its whole-number floats encoded as MessagePack
    /// **integers**, base64 — or `null` for a command that has no floats.
    ///
    /// This is not a curiosity. `rmp-serde` writes an `f64` as a float64
    /// whatever its value; `@msgpack/msgpack` writes the JavaScript number 240
    /// as a uint8, because in JavaScript there is only one kind of number. Both
    /// are valid MessagePack for the same value and the daemon accepts both —
    /// which is a claim about `prism-ipc`, so it is asserted **here**, in
    /// `the_integer_form_of_a_command_is_the_same_command`, rather than assumed
    /// in a browser. The interface compares its own bytes against this member
    /// when it is present, so its encoder is still held to a byte for byte
    /// comparison against something Rust produced.
    client_integer: Option<String>,
    /// The deltas the daemon answered with, in order, base64.
    deltas: Vec<String>,
    /// Whether the daemon refused it. A refusal is part of the script: a canvas
    /// that dragged a window which had just been closed must see nothing move.
    refused: bool,
    /// The open windows afterwards, in stacking order — **the daemon's answer**.
    windows: Vec<RecordedWindow>,
    /// The focused window afterwards, or `null`.
    focused_window: Option<u32>,
    /// The active view afterwards.
    active_view_id: u32,
    /// The stored views afterwards, by number, with how many windows each holds.
    stored_views: Vec<(u32, String, usize)>,
}

/// The whole file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recording {
    /// What made it, and how to make it again.
    note: String,
    /// The protocol version these payloads belong to.
    protocol_version: u32,
    /// The canvas coordinate space these numbers are in, which is nobody's
    /// screen: `WindowInstance` calls them canvas units and the client scales
    /// them. Recorded so the interface cannot quietly decide they are pixels.
    default_window: [f64; 4],
    /// The session document the script starts from, base64 of the `Snapshot`.
    initial_snapshot: String,
    /// The script.
    steps: Vec<Step>,
    /// What a fresh client is served at the end.
    final_snapshot: String,
}

/* -------------------------------------------------------------------------- */
/* The script                                                                 */
/* -------------------------------------------------------------------------- */

/// The commands, in order, with what each one is for.
///
/// Written out rather than generated: this is a *canvas being used* — two
/// windows opened, one dragged, one resized, the one behind brought to the
/// front, the layout stored as a view, the canvas changed, the view selected
/// back — and each step is chosen because a reader could get it wrong.
fn script() -> Vec<(&'static str, Command)> {
    vec![
        (
            "open a fixture sheet: the canvas has one window, at the daemon's default",
            Command::OpenWindow {
                window: WindowType::FixtureSheet,
                params: None,
            },
        ),
        (
            "open a DMX sheet: two windows, the new one in front and focused",
            Command::OpenWindow {
                window: WindowType::DmxSheet,
                params: None,
            },
        ),
        (
            "drag the fixture sheet: the position is the daemon's, not the canvas's",
            Command::PlaceWindow {
                instance_id: WindowInstanceId::new(1),
                x: 240.0,
                y: 120.0,
                w: 640.0,
                h: 480.0,
            },
        ),
        (
            "resize the DMX sheet by its corner",
            Command::PlaceWindow {
                instance_id: WindowInstanceId::new(2),
                x: 0.0,
                y: 0.0,
                w: 1280.0,
                h: 720.0,
            },
        ),
        (
            "focus the fixture sheet: it goes to the end, which is the front",
            Command::FocusWindow {
                instance_id: WindowInstanceId::new(1),
            },
        ),
        (
            "store the canvas as view 2",
            Command::StoreView {
                view_id: ViewId::new(2),
                name: "Programming".to_owned(),
            },
        ),
        (
            "open a patch window on top of it",
            Command::OpenWindow {
                window: WindowType::Patch,
                params: None,
            },
        ),
        (
            "close the DMX sheet",
            Command::CloseWindow {
                instance_id: WindowInstanceId::new(2),
            },
        ),
        (
            "drag the window that was just closed: refused, and nothing moves",
            Command::PlaceWindow {
                instance_id: WindowInstanceId::new(2),
                x: 900.0,
                y: 900.0,
                w: 100.0,
                h: 100.0,
            },
        ),
        (
            "select view 2: the layout comes back, the patch window is gone",
            Command::SelectView {
                view_id: ViewId::new(2),
            },
        ),
        (
            "select a view that was never stored: refused, the canvas stands still",
            Command::SelectView {
                view_id: ViewId::new(9),
            },
        ),
        (
            "select view 1: the canvas empties, because view 1 was stored empty",
            Command::SelectView {
                view_id: ViewId::new(1),
            },
        ),
        // S35's three. Appended rather than woven in, so every index the
        // browser's tests already look a step up by stays where it was — and
        // read by their `what` text all the same, which is S44's finding.
        (
            "store the canvas as view 5, leaving a gap: views are not consecutive",
            Command::StoreView {
                view_id: ViewId::new(5),
                name: "Busking".to_owned(),
            },
        ),
        (
            "rename view 5: the name changes and its windows do not",
            Command::Label {
                target: ObjectRef::View {
                    view_id: ViewId::new(5),
                },
                name: "Front of house".to_owned(),
            },
        ),
        (
            "rename a view that was never stored: refused, nothing changes",
            Command::Label {
                target: ObjectRef::View {
                    view_id: ViewId::new(9),
                },
                name: "Nowhere".to_owned(),
            },
        ),
        (
            "move view 5 onto view 2: the two swap contents and keep their numbers",
            Command::Move {
                from: ObjectRef::View {
                    view_id: ViewId::new(5),
                },
                to: ObjectRef::View {
                    view_id: ViewId::new(2),
                },
                mode: OverwriteMode::Merge,
            },
        ),
        (
            "move view 1 onto itself: nothing at all happens",
            Command::Move {
                from: ObjectRef::View {
                    view_id: ViewId::new(1),
                },
                to: ObjectRef::View {
                    view_id: ViewId::new(1),
                },
                mode: OverwriteMode::Merge,
            },
        ),
        (
            "copy view 2 onto a number nobody has used: view 7 appears",
            Command::Copy {
                from: ObjectRef::View {
                    view_id: ViewId::new(2),
                },
                to: ObjectRef::View {
                    view_id: ViewId::new(7),
                },
                mode: OverwriteMode::Merge,
            },
        ),
        (
            "select view 2 — which is now the layout that used to be view 5",
            Command::SelectView {
                view_id: ViewId::new(2),
            },
        ),
        (
            "delete the active view: the daemon decides what the canvas then shows",
            Command::Delete {
                target: ObjectRef::View {
                    view_id: ViewId::new(2),
                },
            },
        ),
        (
            "delete a view that was never stored: refused",
            Command::Delete {
                target: ObjectRef::View {
                    view_id: ViewId::new(9),
                },
            },
        ),
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

/// **The regenerator.** Runs a daemon and writes the recording.
///
/// ```text
/// cargo test -p prismd --test ui_session -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rewrites a committed fixture; run it deliberately"]
async fn record_the_session_script_for_the_interface() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&Options {
        data_dir: Some(dir.path().to_path_buf()),
        show: Some(dir.path().join("aula.prism")),
        universes: 2,
        outputs: vec![mock_output(1)],
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
    for (index, (what, command)) in script().into_iter().enumerate() {
        let seq = u64::try_from(index).unwrap_or(0) + 1;
        let message = ClientMessage::Command {
            seq,
            command: command.clone(),
        };
        let client = common::encode_base64(&prism_ipc::encode(&message).expect("it encodes"));
        let client_integer = integer_form(seq, &command).map(|bytes| common::encode_base64(&bytes));
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

        // And the answers, out of a **fresh** snapshot: what the daemon would
        // tell a client that had never seen a delta. The one thing a reader
        // must agree with.
        let (second, payload) = connect(&address).await;
        second.shutdown().await;
        let snapshot = snapshot_of_payload(&payload);
        let (windows, focused_window, active_view_id, stored_views) = answers(&snapshot.session);

        steps.push(Step {
            what: what.to_owned(),
            client,
            client_integer,
            deltas,
            refused,
            windows,
            focused_window,
            active_view_id,
            stored_views,
        });
    }

    let (final_wire, final_snapshot) = connect(&address).await;
    final_wire.shutdown().await;
    wire.shutdown().await;

    let recording = Recording {
        note: "Recorded off a running prismd by crates/prismd/tests/ui_session.rs. \
               The windows, focus and view of every step are the daemon's own answers, \
               taken from a fresh client's snapshot. \
               Regenerate with: cargo test -p prismd --test ui_session -- --ignored"
            .to_owned(),
        protocol_version: prism_ipc::PROTOCOL_VERSION,
        default_window: default_window(&steps),
        initial_snapshot: common::encode_base64(&initial_snapshot),
        steps,
        final_snapshot: common::encode_base64(&final_snapshot),
    };

    let path = recording_path();
    std::fs::create_dir_all(path.parent().expect("the fixture lives in a directory")).unwrap();
    let mut text = serde_json::to_string_pretty(&recording).unwrap();
    text.push('\n');
    std::fs::write(&path, text.replace("\r\n", "\n")).unwrap();
    println!(
        "wrote {} ({} steps, {} bytes)",
        path.display(),
        recording.steps.len(),
        std::fs::metadata(&path).unwrap().len()
    );

    let _ = stop.send(());
    let daemon = tokio::time::timeout(PATIENCE, running)
        .await
        .expect("the daemon must stop when it is told to")
        .unwrap();
    daemon.shutdown().await;
}

/// The same command with whole-number floats written as MessagePack integers,
/// for a command that has any.
///
/// Hand-built rather than derived, because the whole point is to produce bytes
/// `Command`'s own `Serialize` cannot: the fields are named and ordered exactly
/// as the enum declares them, and only the four coordinates change kind.
fn integer_form(seq: u64, command: &Command) -> Option<Vec<u8>> {
    let &Command::PlaceWindow {
        instance_id,
        x,
        y,
        w,
        h,
    } = command
    else {
        return None;
    };
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct PlaceWindowAsIntegers {
        t: &'static str,
        instance_id: u32,
        x: i64,
        y: i64,
        w: i64,
        h: i64,
    }
    #[derive(serde::Serialize)]
    struct Envelope {
        t: &'static str,
        seq: u64,
        command: PlaceWindowAsIntegers,
    }
    // Only for a rectangle that is whole, which every one a canvas sends is:
    // `rounded()` is applied before a `PlaceWindow` leaves the interface.
    #[allow(clippy::float_cmp)]
    let whole = |value: f64| (value.trunc() == value).then_some(value as i64);
    Some(
        prism_ipc::encode(&Envelope {
            t: "Command",
            seq,
            command: PlaceWindowAsIntegers {
                t: "PlaceWindow",
                instance_id: instance_id.get(),
                x: whole(x)?,
                y: whole(y)?,
                w: whole(w)?,
                h: whole(h)?,
            },
        })
        .expect("it encodes"),
    )
}

/// Where the daemon puts a window that was opened rather than placed.
///
/// Read back off the first step rather than written down, because it is
/// `prism_core::session::DEFAULT_WINDOW` and this file has no business knowing
/// it — but the interface does need to be told, or it cannot tell a window it
/// has never moved from one it has.
fn default_window(steps: &[Step]) -> [f64; 4] {
    let first = steps
        .first()
        .and_then(|step| step.windows.first())
        .expect("the first step opens a window");
    [first.x, first.y, first.w, first.h]
}

/* -------------------------------------------------------------------------- */
/* The answers, read out of a session document                                */
/* -------------------------------------------------------------------------- */

/// The four things the canvas asks a session document, read the way a client
/// reads it: by pointer, out of the JSON the daemon sent.
type Answers = (
    Vec<RecordedWindow>,
    Option<u32>,
    u32,
    Vec<(u32, String, usize)>,
);

fn answers(session: &JsonValue) -> Answers {
    let mirror = prism_core::JsonMirror::new(session.clone());
    let JsonValue::Array(open) = mirror
        .get("/session/openWindows")
        .expect("a session has open windows")
    else {
        panic!("openWindows is an array");
    };
    let windows = open
        .iter()
        .map(|window| RecordedWindow {
            instance_id: u32::try_from(int_at(window, "instanceId")).expect("a window number"),
            window_type: string_at(window, "type"),
            x: float_at(window, "x"),
            y: float_at(window, "y"),
            w: float_at(window, "w"),
            h: float_at(window, "h"),
        })
        .collect();
    let focused = match mirror.get("/session/focusedWindow") {
        Ok(JsonValue::Int(number)) => u32::try_from(*number).ok(),
        _ => None,
    };
    let JsonValue::Int(view) = mirror
        .get("/session/activeViewId")
        .expect("a session has an active view")
    else {
        panic!("activeViewId is a number");
    };
    let mut stored = Vec::new();
    if let Ok(JsonValue::Object(views)) = mirror.get("/views") {
        for (number, view) in views {
            let count = match member(view, "windows") {
                Some(JsonValue::Array(windows)) => windows.len(),
                _ => 0,
            };
            stored.push((
                number.parse().expect("a view is numbered"),
                string_at(view, "name"),
                count,
            ));
        }
    }
    stored.sort_unstable();
    (
        windows,
        focused,
        u32::try_from(*view).expect("a view number"),
        stored,
    )
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

/// The float member at `key`. A whole number arrives as an integer, because
/// that is what MessagePack does with `0.0` after a JSON round trip — and a
/// canvas that only accepted floats would find every unmoved window missing.
#[allow(clippy::cast_precision_loss)]
fn float_at(value: &JsonValue, key: &str) -> f64 {
    match member(value, key) {
        Some(&JsonValue::Float(number)) => number,
        Some(&JsonValue::Int(number)) => number as f64,
        other => panic!("{key} is not a number: {other:?}"),
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

/// **The recorded answers are what a session document actually says.**
///
/// The deltas are replayed through `prism_core::SessionMirror` — the same
/// applier the interface's mirror is modelled on — and every step's recorded
/// windows, focus and view are read back out of the mirrored document. So the
/// three claims the browser is held to are checked here first, in the language
/// that produced them.
#[test]
fn the_recorded_windows_are_what_the_session_document_says() {
    let recording = recording();
    assert_eq!(
        recording.protocol_version,
        prism_ipc::PROTOCOL_VERSION,
        "the recording is of another protocol version"
    );
    assert_eq!(recording.steps.len(), script().len());

    let start = snapshot_of(&recording.initial_snapshot);
    let mut session = SessionMirror::new(start.session.clone());

    for (index, (step, (what, command))) in recording.steps.iter().zip(script()).enumerate() {
        assert_eq!(step.what, what, "step {index} is not the one in the script");
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
            session
                .apply_delta(&delta)
                .unwrap_or_else(|error| panic!("step {index} delta {order}: {error}"));
        }

        let (windows, focused, view, stored) = answers(session.value());
        assert_eq!(windows, step.windows, "step {index} ({what}): the windows");
        assert_eq!(focused, step.focused_window, "step {index}: the focus");
        assert_eq!(view, step.active_view_id, "step {index}: the active view");
        assert_eq!(stored, step.stored_views, "step {index}: the stored views");
    }

    // And what the deltas built is what a client that never saw one is served.
    let end = snapshot_of(&recording.final_snapshot);
    assert_eq!(
        session.value(),
        &end.session,
        "the deltas and the final snapshot disagree"
    );
}

/// **The two encodings of a whole number are the same command.**
///
/// The interface has no choice about this: JavaScript has one number type, so a
/// client sends the integer 240 where `rmp-serde` sends the float 240.0. That
/// is only safe if the daemon reads both the same way, and this is where that
/// is checked — against `prism_ipc::decode`, which is the function the daemon
/// actually uses.
#[test]
fn the_integer_form_of_a_command_is_the_same_command() {
    let recording = recording();
    let mut checked = 0;
    for (index, (step, (_, command))) in recording.steps.iter().zip(script()).enumerate() {
        let Some(encoded) = &step.client_integer else {
            assert!(
                !matches!(command, Command::PlaceWindow { .. }),
                "step {index} carries floats and no integer form"
            );
            continue;
        };
        let expected = ClientMessage::Command {
            seq: u64::try_from(index).unwrap_or(0) + 1,
            command,
        };
        let bytes = common::decode_base64(encoded);
        // Different bytes, and it matters that they are different: if the two
        // encodings were identical there would be nothing to check.
        assert_ne!(bytes, common::decode_base64(&step.client), "step {index}");
        let decoded: ClientMessage =
            prism_ipc::decode(&bytes).unwrap_or_else(|error| panic!("step {index}: {error}"));
        assert_eq!(decoded, expected, "step {index}");
        checked += 1;
    }
    assert!(checked >= 3, "only {checked} commands carried coordinates");
}

/// The script is a canvas being used, not a list that happens to parse.
///
/// Every claim the browser's tests lean on is asserted here to be *in* the
/// recording: a fixture that quietly stopped exercising the interesting case
/// would otherwise leave those tests passing over nothing.
#[test]
fn the_recording_is_of_a_canvas_being_used() {
    let recording = recording();
    let steps = &recording.steps;

    // Windows are opened, and a window that has never been moved sits at the
    // daemon's default rather than anywhere the interface chose.
    let first = steps.first().expect("a first step");
    assert_eq!(first.windows.len(), 1);
    assert_eq!(
        [
            first.windows[0].x,
            first.windows[0].y,
            first.windows[0].w,
            first.windows[0].h
        ],
        recording.default_window
    );

    // Something is dragged, and it moves.
    assert!(
        steps.iter().any(|step| step
            .windows
            .iter()
            .any(|window| window.x != recording.default_window[0])),
        "no window was ever moved"
    );
    // Something is resized.
    assert!(
        steps.iter().any(|step| step
            .windows
            .iter()
            .any(|window| window.w != recording.default_window[2])),
        "no window was ever resized"
    );
    // The stacking order changes without the set of windows changing, which is
    // the one thing a canvas that sorted by number would get wrong.
    assert!(
        steps.windows(2).any(|pair| {
            let ids = |step: &Step| -> Vec<u32> {
                step.windows
                    .iter()
                    .map(|window| window.instance_id)
                    .collect()
            };
            let (before, after) = (ids(&pair[0]), ids(&pair[1]));
            before != after && {
                let mut sorted = (before.clone(), after.clone());
                sorted.0.sort_unstable();
                sorted.1.sort_unstable();
                sorted.0 == sorted.1
            }
        }),
        "nothing was ever brought to the front"
    );
    // Four refusals, and none of them changes anything: a window dragged after
    // it closed, a view selected that was never stored, and S35's two — a
    // rename and a delete naming a view that is not there.
    let refusals: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.refused)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(refusals.len(), 4, "the script has four refusals in it");
    for index in refusals {
        assert!(
            steps[index].deltas.is_empty(),
            "step {index} said something"
        );
        let before = &steps[index - 1];
        let after = &steps[index];
        assert_eq!(
            before.windows, after.windows,
            "step {index} moved something"
        );
        assert_eq!(before.active_view_id, after.active_view_id);
        assert_eq!(before.focused_window, after.focused_window);
    }
    // A view is stored and selected back, and selecting it restores a layout
    // that had changed in between.
    assert!(
        steps.iter().any(|step| step.stored_views.len() > 1),
        "no view was ever stored"
    );
    // More than one kind of window appears, so a canvas that drew everything
    // the same could not pass.
    let kinds: std::collections::BTreeSet<&str> = steps
        .iter()
        .flat_map(|step| {
            step.windows
                .iter()
                .map(|window| window.window_type.as_str())
        })
        .collect();
    assert!(kinds.len() >= 3, "only {kinds:?} ever appeared");

    // -- S35: managing a view, and the ordering decision ----------------------
    //
    // Every assertion below reads the *daemon's* answer out of the recording.
    // The browser compares its readers against the same file, so neither side is
    // checking itself. Steps are found by what they are *for* rather than by
    // number, which is S44's finding.
    let at = |what: &str| {
        steps
            .iter()
            .position(|step| step.what.contains(what))
            .unwrap_or_else(|| panic!("no step is about {what:?}"))
    };
    let named = |step: &Step| -> Vec<(u32, String)> {
        let mut pairs: Vec<(u32, String)> = step
            .stored_views
            .iter()
            .map(|(id, name, _)| (*id, name.clone()))
            .collect();
        pairs.sort_by_key(|(id, _)| *id);
        pairs
    };

    // A rename changes the name and leaves the windows alone — the reason it is
    // not `StoreView` with the old name.
    let renamed = at("rename view 5");
    let windows_of = |step: &Step, id: u32| {
        step.stored_views
            .iter()
            .find(|(view, _, _)| *view == id)
            .map(|(_, _, count)| *count)
    };
    assert_eq!(
        steps[renamed - 1]
            .stored_views
            .iter()
            .find(|(id, _, _)| *id == 5)
            .map(|(_, name, _)| name.as_str()),
        Some("Busking")
    );
    assert_eq!(
        steps[renamed]
            .stored_views
            .iter()
            .find(|(id, _, _)| *id == 5)
            .map(|(_, name, _)| name.as_str()),
        Some("Front of house")
    );
    assert_eq!(
        windows_of(&steps[renamed - 1], 5),
        windows_of(&steps[renamed], 5),
        "a rename changed the view's windows"
    );
    assert_eq!(
        steps[renamed - 1].stored_views.len(),
        steps[renamed].stored_views.len(),
        "a rename added or removed a view"
    );

    // **The ordering decision.** A move exchanges the two numbers, so the names
    // travel and the numbers stay where they are on the bar. S40 made the
    // command absolute — `Move View 5 View 2` rather than `Prev` — and the
    // decision did not move with it: the bar knows its neighbour's number and
    // writes the line, which is `ARCHITECTURE_SPEC.md` §4.5.
    let moved = at("move view 5 onto view 2");
    assert_eq!(
        named(&steps[moved - 1]),
        vec![
            (1, "View 1".to_owned()),
            (2, "Programming".to_owned()),
            (5, "Front of house".to_owned()),
        ]
    );
    assert_eq!(
        named(&steps[moved]),
        vec![
            (1, "View 1".to_owned()),
            (2, "Front of house".to_owned()),
            (5, "Programming".to_owned()),
        ],
        "a move must exchange the two numbers, not record an order beside them"
    );
    // Views are **not** consecutive, so a move steps to the next stored number
    // rather than to `id ± 1`. The gap between 2 and 5 is what makes the
    // assertion above mean something.
    assert!(
        steps[moved].stored_views.iter().any(|(id, _, _)| *id == 5),
        "the script lost the gap that gives the previous assertion its meaning"
    );

    // A view moved onto **itself** does not move, and says nothing at all — a
    // no-op rather than a refusal, and the only one the absolute form has. The
    // relative form this replaced had one more (the end of the bar); an
    // absolute move always has somewhere to go, and a number nobody has stored
    // is a move into an empty place rather than a refusal.
    let stuck = at("move view 1 onto itself");
    assert!(
        steps[stuck].deltas.is_empty(),
        "a move that could not happen spoke"
    );
    assert!(!steps[stuck].refused, "it is a no-op, not a refusal");
    assert_eq!(named(&steps[stuck]), named(&steps[stuck - 1]));

    // **A copy makes a view that was not there** (S40), and leaves the source
    // exactly as it was — which is the whole difference between a copy and a
    // move, said on the recording rather than in a comment.
    let copied = at("copy view 2 onto a number nobody has used");
    assert!(
        !steps[copied - 1]
            .stored_views
            .iter()
            .any(|(id, _, _)| *id == 7),
        "view 7 was already there"
    );
    assert!(
        steps[copied].stored_views.iter().any(|(id, _, _)| *id == 7),
        "the copy did not arrive"
    );
    assert_eq!(
        windows_of(&steps[copied], 2),
        windows_of(&steps[copied - 1], 2),
        "a copy changed the view it was copied from"
    );

    // **Deleting the active view: what the canvas then shows is the daemon's.**
    let deleted = at("delete the active view");
    assert_eq!(
        steps[deleted - 1].active_view_id,
        2,
        "view 2 was not active"
    );
    assert!(
        !steps[deleted]
            .stored_views
            .iter()
            .any(|(id, _, _)| *id == 2),
        "view 2 is still stored"
    );
    assert_ne!(
        steps[deleted].active_view_id, 2,
        "`activeViewId` still names a view that was deleted"
    );
    assert!(
        steps[deleted]
            .stored_views
            .iter()
            .any(|(id, _, _)| *id == steps[deleted].active_view_id),
        "the daemon left `activeViewId` naming nothing"
    );
    // And the canvas is that view's layout rather than whatever was open: the
    // interface does not have to invent one, and this is the answer it reads.
    assert_eq!(
        Some(steps[deleted].windows.len()),
        windows_of(&steps[deleted], steps[deleted].active_view_id),
        "the canvas after a delete is not the successor view's layout"
    );
}
