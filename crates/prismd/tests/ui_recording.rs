//! The delta stream the interface's mirror is tested against, recorded off a
//! running daemon — and the check that the recording is still one.
//!
//! # Why a recording, and why it is not generated in TypeScript
//!
//! S23's first exit criterion is that *deltas applied to the mirror reproduce
//! the daemon's state*, property-tested against a recorded delta stream. The
//! trap in that sentence is the one S19, S20, S21 and S22 each found in their
//! own form: **a test that uses the function under test to work out what the
//! answer should be is not a test.** A TypeScript generator producing patch
//! operations for a TypeScript applier would pass with the whole protocol
//! misunderstood, as long as it were misunderstood consistently.
//!
//! So the operations come from `prism-core`, through a real `prismd` over a
//! real socket, and the state they have to reproduce is **the daemon's own
//! snapshot** — taken by a second client that has never seen a delta. That is
//! `docs/IPC_PROTOCOL.md` §9's *snapshot completeness* row, which S18 asserted
//! in Rust; this target writes down what it looked like on the wire so the
//! browser can be held to the same sentence.
//!
//! # What is in the file
//!
//! The **payloads**, as base64, exactly as they came off the socket:
//!
//! - `clientMessages` — what a client sends: a hello, and one of every command
//!   the interface can issue. The browser encodes the same messages and
//!   compares bytes, which is the only way to check an encoder against a
//!   decoder that is not there.
//! - `cases[]` — a snapshot, the deltas a scripted command sequence produced,
//!   and the snapshot a **fresh client** was served afterwards.
//!
//! # It is frozen, and there are two tests
//!
//! [`record_the_delta_stream_for_the_interface`] is `#[ignore]`d: it starts a
//! daemon and rewrites the file, which is a deliberate act and not something
//! every `cargo test` should do to the working tree (the same shape as
//! `prism-core`'s frozen migration fixture). What runs on every commit is
//! [`the_recording_is_a_delta_stream_this_build_could_have_sent`], which decodes
//! the payloads with this build's own types and replays them through
//! `prism_core`'s mirror. If the wire format moves, the recording stops
//! decoding here rather than going quietly stale over in `ui/`.

// This target *writes a file for a person to commit* and says where. The same
// allowance the timing tests carry, for the same reason.
#![allow(clippy::print_stdout)]
// Every test holds `common::one_daemon_at_a_time` across its awaits.
#![allow(clippy::await_holding_lock)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use prism_core::{JsonMirror, SessionMirror, ShowMirror};
use prism_domain::{
    AttributeType, Command, Delta, ExecutorId, FeatureGroup, FixtureId, GoDirection, JsonValue,
    PlaybackTarget, ProgrammerState, SelectionMode, SequenceId, StoreMode, UniverseId, ViewId,
    WindowType,
};
use prism_ipc::{ClientKind, ClientMessage, Hello, ServerMessage, Snapshot, Wire, local};
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;

mod common;

/// How long any wait here may take before it is a named failure.
const PATIENCE: Duration = Duration::from_secs(20);

/// How many scripted sequences the recording holds.
///
/// Each one is a fresh client against the same daemon, so the documents grow
/// through the file and the later cases patch a show the earlier ones built —
/// which is what a session at a desk looks like and what a mirror has to
/// survive.
const CASES: usize = 12;

/// How many commands each case sends.
const COMMANDS_PER_CASE: usize = 10;

/// Where the interface reads it from.
fn recording_path() -> PathBuf {
    common::ui_fixture("daemon-recording.json")
}

/* -------------------------------------------------------------------------- */
/* The recording                                                              */
/* -------------------------------------------------------------------------- */

/// One scripted sequence: the world, what happened to it, and the world again.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Case {
    /// What the script asked for, in words, so the file can be read.
    commands: Vec<String>,
    /// The `ServerMessage::Snapshot` this client was served, base64.
    snapshot: String,
    /// Every `ServerMessage::Delta` it was sent afterwards, in order, base64.
    deltas: Vec<String>,
    /// The `ServerMessage::Snapshot` a **fresh** client was served at the end.
    final_snapshot: String,
}

/// One message a client sends, and the bytes `rmp-serde` made of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ClientRecord {
    /// What it is, for a person reading the file.
    what: String,
    /// The payload, base64.
    payload: String,
}

/// The whole file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recording {
    /// What made it, and how to make it again.
    note: String,
    /// The protocol version these payloads belong to.
    protocol_version: u32,
    /// What a client sends.
    client_messages: Vec<ClientRecord>,
    /// What the daemon sent back.
    cases: Vec<Case>,
}

/* -------------------------------------------------------------------------- */
/* The script                                                                 */
/* -------------------------------------------------------------------------- */

/// A reproducible sequence of numbers.
///
/// A named constant rather than a crate: the recording has to be reproducible
/// by whoever regenerates it, and "the same numbers on every machine and every
/// version" is a shorter promise to keep with eleven lines than with a
/// dependency.
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        // Not zero, and odd: a xorshift* state of zero stays there.
        Self(seed.wrapping_mul(2) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, limit: u64) -> u64 {
        self.next() % limit.max(1)
    }
}

/// One command from the vocabulary a client has, chosen by `rng`.
///
/// Every branch is something the interface can actually issue
/// (`docs/IPC_PROTOCOL.md` §5), and several of them are refusable on purpose —
/// a recording in which nothing is ever refused would not be a recording of a
/// desk being used.
fn some_command(rng: &mut Rng, patched: &mut u32) -> Command {
    match rng.below(14) {
        0 => Command::SelectFixtures {
            ids: vec![FixtureId::new(1 + u32::try_from(rng.below(3)).unwrap_or(0))],
            mode: match rng.below(3) {
                0 => SelectionMode::Set,
                1 => SelectionMode::Add,
                _ => SelectionMode::Toggle,
            },
        },
        1 => Command::SetAttribute {
            attribute: AttributeType::ALL
                [usize::try_from(rng.below(3)).unwrap_or(0) % AttributeType::ALL.len()],
            value: i32::try_from(rng.below(65536)).unwrap_or(0),
            relative: false,
        },
        2 => Command::ClearProgrammer,
        3 => Command::SetExecutorPage {
            page: u32::try_from(rng.below(4)).unwrap_or(0),
        },
        4 => Command::SetExecutorMaster {
            executor_id: ExecutorId::new(u32::try_from(rng.below(2)).unwrap_or(0)),
            level: u16::try_from(rng.below(65536)).unwrap_or(0),
        },
        5 => Command::ExecutorGo {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            direction: if rng.below(2) == 0 {
                GoDirection::Next
            } else {
                GoDirection::Prev
            },
        },
        6 => Command::ExecutorOff {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
        },
        7 => Command::CommandLineInput {
            text: format!("fixture {} at {}", rng.below(4), rng.below(101)),
        },
        8 => Command::SetEncoderBank {
            group: FeatureGroup::ALL[usize::try_from(rng.below(5)).unwrap_or(0)],
        },
        9 => Command::StoreView {
            view_id: ViewId::new(1 + u32::try_from(rng.below(3)).unwrap_or(0)),
            name: format!("View {}", rng.below(10)),
        },
        10 => Command::OpenWindow {
            window: WindowType::FixtureSheet,
            params: None,
        },
        11 => {
            *patched += 1;
            Command::PatchFixture {
                id: FixtureId::new(100 + *patched),
                name: format!("Fixture {}", 100 + *patched),
                type_id: "generic.dimmer".to_owned(),
                universe: UniverseId::new(1),
                address: u16::try_from(100 + *patched * 3).unwrap_or(100),
            }
        }
        12 => Command::StoreCue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: format!("{}", 1 + rng.below(4)),
            mode: StoreMode::Merge,
        },
        _ => {
            if rng.below(2) == 0 {
                Command::Oops
            } else {
                Command::SaveShow
            }
        }
    }
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

/// How long the daemon has to go without a delta before a case is over.
///
/// **The playback readback is asynchronous** (S34): what cue an executor is on
/// comes back from the tick, so a `Delta::ExecutorState` arrives a poll after
/// the command that caused it rather than with its receipt. A recording that
/// stopped at the last `Ack` would lose it, and the replay would then disagree
/// with the fresh snapshot beside it.
///
/// Only a delta resets the window: the daemon also publishes a telemetry frame
/// thirty times a second, so a drain that waited for silence on the socket would
/// wait for ever.
const SETTLE: Duration = Duration::from_millis(150);

/// Collects everything the daemon says after the last receipt, until it has
/// gone [`SETTLE`] without a delta.
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
            Ok(_) | Err(_) => return,
        }
    }
}

/// **The regenerator.** Runs a daemon and writes the recording.
///
/// ```text
/// cargo test -p prismd --test ui_recording -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rewrites a committed fixture; run it deliberately"]
async fn record_the_delta_stream_for_the_interface() {
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

    let mut cases = Vec::with_capacity(CASES);
    let mut patched = 0_u32;
    for case in 0..CASES {
        // A client of its own per case, so every case begins with a snapshot
        // that was served rather than one that was worked out.
        let (mut wire, snapshot) = connect(&address).await;
        let mut rng = Rng::new(u64::try_from(case).unwrap_or(0) + 1);
        let mut commands = Vec::with_capacity(COMMANDS_PER_CASE);
        let mut deltas = Vec::new();

        for step in 0..COMMANDS_PER_CASE {
            let command = some_command(&mut rng, &mut patched);
            commands.push(format!("{command:?}"));
            let seq = u64::try_from(step).unwrap_or(0) + 1;
            wire.send_message(&ClientMessage::Command {
                seq,
                command: command.clone(),
            })
            .await
            .expect("a command must reach the daemon");

            // The deltas come before the acknowledgement (§5), so a client that
            // has its `Ack` has been told everything that command did.
            loop {
                let payload = next_payload(&mut wire).await;
                match decode(&payload) {
                    ServerMessage::Delta { .. } => deltas.push(common::encode_base64(&payload)),
                    ServerMessage::Ack { seq: acked } if acked == seq => break,
                    ServerMessage::Reject {
                        seq: Some(refused), ..
                    } if refused == seq => break,
                    // Telemetry is interleaved with all of this at 30 Hz and is
                    // not recorded: a client skips it on the control path, and
                    // a recording that carried it would be a recording of a
                    // clock rather than of a conversation.
                    _ => {}
                }
            }
        }

        // Everything the readback still has to say, before the fresh client
        // below is served — see [`SETTLE`]. A recording that stopped at the last
        // `Ack` would lose the `Delta::ExecutorState` that follows a playback
        // command by a poll, and the replay would then disagree with the
        // snapshot beside it.
        settle(&mut wire, &mut deltas).await;

        // And a fresh client, which is the whole point: what it is served is
        // the daemon's own state, arrived at without a single delta.
        let (second, final_snapshot) = connect(&address).await;
        second.shutdown().await;
        wire.shutdown().await;

        cases.push(Case {
            commands,
            snapshot: common::encode_base64(&snapshot),
            deltas,
            final_snapshot: common::encode_base64(&final_snapshot),
        });
    }

    let recording = Recording {
        note: "Recorded off a running prismd by crates/prismd/tests/ui_recording.rs. \
               Payloads are base64 of the MessagePack a client received or sent. \
               Regenerate with: cargo test -p prismd --test ui_recording -- --ignored"
            .to_owned(),
        protocol_version: prism_ipc::PROTOCOL_VERSION,
        client_messages: client_messages(),
        cases,
    };

    let path = recording_path();
    std::fs::create_dir_all(path.parent().expect("the fixture lives in a directory")).unwrap();
    let mut text = serde_json::to_string_pretty(&recording).unwrap();
    text.push('\n');
    std::fs::write(&path, text.replace("\r\n", "\n")).unwrap();
    println!(
        "wrote {} ({} cases, {} bytes)",
        path.display(),
        recording.cases.len(),
        std::fs::metadata(&path).unwrap().len()
    );

    let _ = stop.send(());
    let daemon = tokio::time::timeout(PATIENCE, running)
        .await
        .expect("the daemon must stop when it is told to")
        .unwrap();
    daemon.shutdown().await;
}

/// One of every message a client sends, encoded the way `prism-ipc` encodes it.
///
/// The interface encodes the same list and compares **bytes**. That is a strong
/// comparison on purpose: `rmp-serde`'s `to_vec_named` writes a map with the
/// tag first and the fields in declaration order, and an encoder that agreed
/// about the fields but not about the shape would produce something the daemon
/// answers with `Reject { Undecodable }` rather than something obviously wrong.
fn client_messages() -> Vec<ClientRecord> {
    let messages: Vec<(&str, ClientMessage)> = vec![
        (
            "Hello, Desktop, no token",
            ClientMessage::Hello {
                hello: Hello::new(ClientKind::Desktop),
            },
        ),
        (
            "Hello, WebRemote, with a token",
            ClientMessage::Hello {
                hello: Hello::new(ClientKind::WebRemote).with_token("hunter2"),
            },
        ),
        (
            "SelectFixtures",
            ClientMessage::Command {
                seq: 0,
                command: Command::SelectFixtures {
                    ids: vec![FixtureId::new(1), FixtureId::new(2)],
                    mode: SelectionMode::Add,
                },
            },
        ),
        (
            "SetAttribute",
            ClientMessage::Command {
                seq: 1,
                command: Command::SetAttribute {
                    attribute: AttributeType::Dimmer,
                    value: -257,
                    relative: true,
                },
            },
        ),
        (
            "ClearProgrammer",
            ClientMessage::Command {
                seq: 2,
                command: Command::ClearProgrammer,
            },
        ),
        (
            "SetExecutorMaster",
            ClientMessage::Command {
                seq: 300,
                command: Command::SetExecutorMaster {
                    executor_id: ExecutorId::new(7),
                    level: 65535,
                },
            },
        ),
        (
            "ExecutorGo",
            ClientMessage::Command {
                seq: 4,
                command: Command::ExecutorGo {
                    target: PlaybackTarget::of_executor(ExecutorId::new(0)),
                    direction: GoDirection::Prev,
                },
            },
        ),
        (
            "PatchFixture",
            ClientMessage::Command {
                seq: 5,
                command: Command::PatchFixture {
                    id: FixtureId::new(42),
                    name: "Fixture 42".to_owned(),
                    type_id: "generic.dimmer".to_owned(),
                    universe: UniverseId::new(2),
                    address: 271,
                },
            },
        ),
        (
            "OpenWindow without params",
            ClientMessage::Command {
                seq: 6,
                command: Command::OpenWindow {
                    window: WindowType::Patch,
                    params: None,
                },
            },
        ),
        (
            "OpenWindow with params",
            ClientMessage::Command {
                seq: 7,
                command: Command::OpenWindow {
                    window: WindowType::PresetPool,
                    params: Some(BTreeMap::from([(
                        "pool".to_owned(),
                        JsonValue::String("Color".to_owned()),
                    )])),
                },
            },
        ),
        (
            "CommandLineInput",
            ClientMessage::Command {
                seq: 8,
                command: Command::CommandLineInput {
                    text: "fixture 1 thru 4 at full".to_owned(),
                },
            },
        ),
        (
            "SelectView",
            ClientMessage::Command {
                seq: 9,
                command: Command::SelectView {
                    view_id: ViewId::new(2),
                },
            },
        ),
        (
            "SaveShow",
            ClientMessage::Command {
                seq: 10,
                command: Command::SaveShow,
            },
        ),
    ];
    messages
        .into_iter()
        .map(|(what, message)| ClientRecord {
            what: what.to_owned(),
            payload: common::encode_base64(
                &prism_ipc::encode(&message).expect("a message must encode"),
            ),
        })
        .collect()
}

/* -------------------------------------------------------------------------- */
/* The check that runs on every commit                                        */
/* -------------------------------------------------------------------------- */

/// Reads the committed recording.
fn recording() -> Recording {
    let path = recording_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is missing: {error}", path.display()));
    serde_json::from_str(&text).expect("the recording is JSON this build understands")
}

/// **The recording is still a conversation this build could have had.**
///
/// Every payload is decoded with this build's own message types and every case
/// is replayed through `prism_core`'s mirror — the same claim the interface
/// makes in TypeScript, made here in the language the deltas were generated in.
/// If they ever disagree, this is the one that is right.
#[test]
fn the_recording_is_a_delta_stream_this_build_could_have_sent() {
    let recording = recording();
    assert_eq!(
        recording.protocol_version,
        prism_ipc::PROTOCOL_VERSION,
        "the recording is of another protocol version"
    );
    assert_eq!(recording.cases.len(), CASES);
    assert_eq!(recording.client_messages, client_messages());

    let mut with_show = 0;
    let mut with_session = 0;
    let mut with_programmer = 0;
    let mut with_executor_state = 0;

    for (index, case) in recording.cases.iter().enumerate() {
        let start = snapshot_of(&case.snapshot);
        let end = snapshot_of(&case.final_snapshot);

        let mut show = ShowMirror::new(start.show.clone());
        let mut session = SessionMirror::new(start.session.clone());
        let mut programmer = start.programmer.clone();

        for (step, encoded) in case.deltas.iter().enumerate() {
            let ServerMessage::Delta { delta } = decode(&common::decode_base64(encoded)) else {
                panic!("case {index} step {step} is not a delta");
            };
            match &delta {
                Delta::ShowPatch { ops } if !ops.is_empty() => with_show += 1,
                Delta::SessionPatch { ops } if !ops.is_empty() => with_session += 1,
                Delta::ProgrammerChanged { .. } => with_programmer += 1,
                Delta::PlaybackState { .. } => with_executor_state += 1,
                _ => {}
            }
            show.apply_delta(&delta)
                .unwrap_or_else(|error| panic!("case {index} step {step}: {error}"));
            session
                .apply_delta(&delta)
                .unwrap_or_else(|error| panic!("case {index} step {step}: {error}"));
            if let Delta::ProgrammerChanged { state } = &delta {
                programmer = state.clone();
            }
        }

        assert_eq!(show.value(), &end.show, "case {index}: the show drifted");
        assert_eq!(
            session.value(),
            &end.session,
            "case {index}: the session drifted"
        );
        assert_eq!(
            programmer, end.programmer,
            "case {index}: the programmer drifted"
        );
    }

    // And the recording is worth having: a stream of nothing but `Notice`
    // deltas would satisfy every assertion above.
    assert!(with_show > 10, "only {with_show} show patches");
    assert!(with_session > 10, "only {with_session} session patches");
    assert!(
        with_programmer > 10,
        "only {with_programmer} programmer changes"
    );
    assert!(
        with_executor_state > 0,
        "no executor ever ran in the recording"
    );
}

/// The three documents actually move, so a mirror that ignored every delta
/// could not pass the test above.
#[test]
fn every_case_in_the_recording_changes_something() {
    for (index, case) in recording().cases.iter().enumerate() {
        let start = snapshot_of(&case.snapshot);
        let end = snapshot_of(&case.final_snapshot);
        assert!(!case.deltas.is_empty(), "case {index} recorded no deltas");
        assert!(
            start.show != end.show
                || start.session != end.session
                || start.programmer != end.programmer,
            "case {index} ended where it started"
        );
    }

    // Over the whole file, each document moves at least once — otherwise a
    // mirror that dropped one of the three would still pass.
    let cases = recording().cases;
    let moved = |get: fn(&Snapshot) -> JsonValue| {
        cases.iter().any(|case| {
            get(&snapshot_of(&case.snapshot)) != get(&snapshot_of(&case.final_snapshot))
        })
    };
    assert!(
        moved(|snapshot| snapshot.show.clone()),
        "the show never moved"
    );
    assert!(
        moved(|snapshot| snapshot.session.clone()),
        "the session never moved"
    );
    assert!(
        cases
            .iter()
            .any(|case| snapshot_of(&case.snapshot).programmer
                != snapshot_of(&case.final_snapshot).programmer),
        "the programmer never moved"
    );
}

/// The base64 in the file is base64, checked against a value rather than
/// against itself.
#[test]
fn the_encoding_is_ordinary_base64() {
    // RFC 4648 §10's own vectors, both directions.
    for (plain, encoded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(
            common::encode_base64(plain.as_bytes()),
            encoded,
            "encoding {plain:?}"
        );
        assert_eq!(
            common::decode_base64(encoded),
            plain.as_bytes(),
            "decoding {encoded:?}"
        );
    }
    // And every byte survives the round trip, which is what the payloads need.
    let every: Vec<u8> = (0..=255).collect();
    assert_eq!(common::decode_base64(&common::encode_base64(&every)), every);
}

/// The snapshot inside a recorded payload.
fn snapshot_of(encoded: &str) -> Snapshot {
    let ServerMessage::Snapshot { snapshot } = decode(&common::decode_base64(encoded)) else {
        panic!("a recorded snapshot is not a snapshot");
    };
    *snapshot
}

/// A `JsonMirror` reads the recorded documents, which is how a client reads
/// them — and it is the type the interface's own mirror is modelled on.
#[test]
fn the_recorded_documents_are_the_documents_a_client_mirrors() {
    let recording = recording();
    let first = snapshot_of(&recording.cases[0].snapshot);
    let show = JsonMirror::new(first.show.clone());
    assert_eq!(
        show.get("/fixtures/1/name").unwrap(),
        &JsonValue::String("Fixture 1".to_owned()),
        "the recording is of the rig `common::show_file` writes"
    );
    let session = JsonMirror::new(first.session.clone());
    assert!(session.get("/session/executorPage").is_ok());
    assert!(session.get("/views").is_ok());
    assert_eq!(first.programmer, ProgrammerState::default());
}
