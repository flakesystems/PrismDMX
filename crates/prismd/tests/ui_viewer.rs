//! A rig being hung and lit — recorded off a running daemon, so the 3D viewer
//! is held to the daemon's bytes rather than to a second opinion in TypeScript.
//!
//! # What the viewer needs to be held to, and why each is recorded
//!
//! - **Where a fixture hangs.** `Command::PlaceFixtures` is new in S30, and the
//!   answer to it is a `ShowPatch` of two `replace`s per fixture. The browser
//!   applies those to its mirror and reads `position` and `rotation` out of the
//!   document; that is the path recorded here, B55's rule applied to a new
//!   command: the interface decodes these bytes through `decodeServerMessage`
//!   and never an object a fake daemon made up.
//! - **What a rotation means.** `prism_domain::orientation` is the one place the
//!   Euler order is written down, and the viewer builds the same matrix in
//!   `ui/src/viewer/space.ts`. The recording carries **this build's matrix** for
//!   every rotation the script uses, so the two languages are held to each
//!   other rather than to their comments.
//! - **What a GDTF profile says about its beam.** The heads are a `.gdtf` built
//!   byte by byte — there is no corpus and cannot be one — and the profile the
//!   show embeds carries `physical`, which is where the viewer reads the beam's
//!   place, direction and angle from.
//! - **What is on the cable.** One real telemetry frame with the first head
//!   lit and panned, so the viewer's reading of pan, tilt and intensity is
//!   tested against `prism_ipc::TelemetryFrame::encode`'s bytes.
//!
//! # It is frozen, and the guards run on every commit
//!
//! [`record_the_viewer_script_for_the_interface`] is `#[ignore]`d like every
//! other regenerator. What runs always replays the deltas through
//! `prism_core::ShowMirror`, recomputes every recorded matrix and decodes the
//! recorded frame, so a shape that moved fails here, in Rust, first.

// This target *writes a file for a person to commit* and says where.
#![allow(clippy::print_stdout)]
// Every test holds `common::one_daemon_at_a_time` across its awaits.
#![allow(clippy::await_holding_lock)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use prism_core::ShowMirror;
use prism_domain::{
    AttributeType, Command, FixtureId, FixturePlace, JsonValue, Orientation, PatchPlacement,
    SelectionMode, UniverseId, Vec3, orientation,
};
use prism_ipc::{ClientKind, ClientMessage, Hello, ServerMessage, TelemetryFrame, Wire, local};
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;

mod common;

/// How long any wait here may take before it is a named failure.
const PATIENCE: Duration = Duration::from_secs(20);

/// The key the heads' `.gdtf` files under: `slug(manufacturer)/slug(name)/slug(mode)`.
const HEAD: &str = "prism-test/viewer-head/standard";

/// The pan value the first head is lit at: three quarters of the travel.
///
/// The profile says −270…+270°, so this is **+135°** — a number the viewer's
/// reading of a 16-bit channel through a physical range has to arrive at.
const PAN: u16 = 49_151;

/// How many telemetry frames must agree before one is kept.
///
/// A fade has no time here, but a frame already on its way when the last command
/// landed is not a picture of it. Three identical frames in a row are — S46's
/// lesson, and S59's again: a number the daemon owns is read once it has
/// stopped moving.
const STEADY_FRAMES: usize = 3;

/// Where the interface reads the recording from.
fn recording_path() -> PathBuf {
    common::ui_fixture("viewer-recording.json")
}

/* -------------------------------------------------------------------------- */
/* The library: one moving head, as GDTF                                      */
/* -------------------------------------------------------------------------- */

/// The head's description: pan and tilt at sixteen bits, a dimmer and a zoom,
/// in a body 0.3 × 0.5 × 0.3 m with its beam **0.2 m below** the origin.
///
/// The beam's translation is written in millimetres (`-200`), which is what
/// `MATRIX_TO_METRES` assumes of GDTF; `PROGRESS.md` §5 carries that as the one
/// unchecked number, and this recording is where a browser would first see a
/// head drawn a kilometre from its beam if it were wrong.
const DESCRIPTION: &str = r#"<GDTF DataVersion="1.2">
  <FixtureType Name="Viewer Head" Manufacturer="Prism Test"
               FixtureTypeID="5A1E0000-0000-4000-8000-0000000030AA">
    <Models><Model Name="Body" File="viewer-head" Length="0.3" Width="0.3" Height="0.5"/></Models>
    <Geometries>
      <Geometry Name="Body" Model="Body" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
        <Beam Name="Beam" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,-200,1}"
              BeamAngle="18" LuminousFlux="9000" ColorTemperature="6500"/>
      </Geometry>
    </Geometries>
    <DMXModes>
      <DMXMode Name="Standard" Geometry="Body">
        <DMXChannels>
          <DMXChannel Offset="1,2">
            <LogicalChannel Attribute="Pan">
              <ChannelFunction Attribute="Pan" PhysicalFrom="-270" PhysicalTo="270"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="3,4">
            <LogicalChannel Attribute="Tilt">
              <ChannelFunction Attribute="Tilt" PhysicalFrom="-135" PhysicalTo="135"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="5">
            <LogicalChannel Attribute="Dimmer">
              <ChannelFunction Attribute="Dimmer"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="6">
            <LogicalChannel Attribute="Zoom">
              <ChannelFunction Attribute="Zoom" PhysicalFrom="8" PhysicalTo="40"/>
            </LogicalChannel>
          </DMXChannel>
        </DMXChannels>
      </DMXMode>
    </DMXModes>
  </FixtureType>
</GDTF>"#;

/// Writes the head as a `.gdtf` — a stored ZIP of one entry — byte by byte.
///
/// A copy of `ui_patch.rs`'s writer rather than a shared one: an integration
/// target links a crate's library and not another target's helpers, and the
/// two recordings are allowed to move independently.
fn write_library(root: &Path) {
    let name = b"description.xml";
    let body = DESCRIPTION.as_bytes();
    let crc = crc32(body);
    let size = u32::try_from(body.len()).expect("a small description");
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes()); // local header
    out.extend_from_slice(&20_u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0_u16.to_le_bytes()); // flags
    out.extend_from_slice(&0_u16.to_le_bytes()); // stored
    out.extend_from_slice(&0_u32.to_le_bytes()); // time and date
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&15_u16.to_le_bytes()); // name length
    out.extend_from_slice(&0_u16.to_le_bytes()); // extra
    out.extend_from_slice(name);
    out.extend_from_slice(body);

    let directory_at = u32::try_from(out.len()).expect("a small archive");
    let mut central: Vec<u8> = Vec::new();
    central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
    central.extend_from_slice(&20_u16.to_le_bytes()); // made by
    central.extend_from_slice(&20_u16.to_le_bytes()); // needed
    central.extend_from_slice(&0_u16.to_le_bytes()); // flags
    central.extend_from_slice(&0_u16.to_le_bytes()); // stored
    central.extend_from_slice(&0_u32.to_le_bytes()); // time and date
    central.extend_from_slice(&crc.to_le_bytes());
    central.extend_from_slice(&size.to_le_bytes());
    central.extend_from_slice(&size.to_le_bytes());
    central.extend_from_slice(&15_u16.to_le_bytes()); // name length
    central.extend_from_slice(&0_u16.to_le_bytes()); // extra
    central.extend_from_slice(&0_u16.to_le_bytes()); // comment
    central.extend_from_slice(&0_u16.to_le_bytes()); // disk
    central.extend_from_slice(&0_u16.to_le_bytes()); // internal
    central.extend_from_slice(&0_u32.to_le_bytes()); // external
    central.extend_from_slice(&0_u32.to_le_bytes()); // local header offset
    central.extend_from_slice(name);

    let directory = u32::try_from(central.len()).expect("a small directory");
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes()); // end record
    out.extend_from_slice(&0_u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0_u16.to_le_bytes()); // directory's disk
    out.extend_from_slice(&1_u16.to_le_bytes()); // entries here
    out.extend_from_slice(&1_u16.to_le_bytes()); // entries in all
    out.extend_from_slice(&directory.to_le_bytes());
    out.extend_from_slice(&directory_at.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes()); // comment

    std::fs::create_dir_all(root).expect("the library directory");
    std::fs::write(root.join("viewer-head.gdtf"), out).expect("a .gdtf file");
}

/// CRC-32 as ZIP states it, by the table-free definition.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let carry = crc & 1;
            crc >>= 1;
            if carry != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}

/* -------------------------------------------------------------------------- */
/* The script                                                                 */
/* -------------------------------------------------------------------------- */

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

fn place(id: u32, position: Vec3, rotation: Vec3) -> FixturePlace {
    FixturePlace {
        id: FixtureId::new(id),
        position,
        rotation,
    }
}

fn placed(id: u32, address: u16) -> PatchPlacement {
    PatchPlacement {
        id: FixtureId::new(id),
        universe: UniverseId::new(1),
        address,
    }
}

/// The hang: two heads on a truss six metres up, the second tipped towards the
/// audience, and a PAR standing on the floor upstage washing the back wall.
fn the_hang() -> Vec<FixturePlace> {
    vec![
        place(1, v(-2.0, 6.0, 1.0), Vec3::ZERO),
        place(2, v(2.0, 6.0, 1.0), v(20.0, 0.0, 0.0)),
        place(10, v(0.0, 0.2, 4.0), v(-150.0, 0.0, 0.0)),
    ]
}

/// The script, in order: **a rig patched, hung, refused, taken back, put back
/// and lit.**
fn script() -> Vec<(&'static str, Command)> {
    vec![
        (
            "two GDTF heads, patched as one gesture",
            Command::PatchFixtures {
                type_id: HEAD.to_owned(),
                name: "Head".to_owned(),
                software_dimmer: true,
                placements: vec![placed(1, 1), placed(2, 11)],
            },
        ),
        (
            "a PAR the library has no picture of",
            Command::PatchFixtures {
                type_id: "generic.rgbw.par".to_owned(),
                name: "Floor PAR".to_owned(),
                software_dimmer: true,
                placements: vec![placed(10, 101)],
            },
        ),
        (
            "the hang, as one gesture",
            Command::PlaceFixtures {
                placements: the_hang(),
            },
        ),
        (
            "a spread naming a fixture that is not patched: refused whole",
            Command::PlaceFixtures {
                placements: vec![
                    place(1, Vec3::ZERO, Vec3::ZERO),
                    place(99, Vec3::ZERO, Vec3::ZERO),
                ],
            },
        ),
        ("one Oops takes the whole hang back", Command::Oops),
        ("and Redo hangs it again", Command::Redo),
        (
            "select the first head",
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1)],
                mode: SelectionMode::Set,
            },
        ),
        (
            "open it",
            Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                occurrence: 0,
                value: i32::from(u16::MAX),
                relative: false,
            },
        ),
        (
            "and pan it three quarters of the way",
            Command::SetAttribute {
                attribute: AttributeType::Pan,
                occurrence: 0,
                value: i32::from(PAN),
                relative: false,
            },
        ),
    ]
}

/* -------------------------------------------------------------------------- */
/* The file                                                                   */
/* -------------------------------------------------------------------------- */

/// One step of the script as it was recorded.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    /// What the step is for.
    what: String,
    /// The client's message, base64.
    client: String,
    /// Every delta the daemon broadcast in answer, base64, in order.
    deltas: Vec<String>,
    /// Whether the daemon refused it.
    refused: bool,
}

/// One rotation and what this build says it means.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedOrientation {
    /// The rotation, as a fixture carries it.
    rotation: Vec3,
    /// `prism_domain::orientation`'s answer, by rows.
    matrix: Orientation,
}

/// The lit frame, and what is on the cable in it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LitFrame {
    /// The whole `ServerMessage::Telemetry` payload, base64 — the message and
    /// not only the frame inside it, so the interface decodes it through
    /// `decodeServerMessage` like every other byte it is held to.
    message: String,
    /// The universe the heads are in.
    universe: u32,
    /// The first head's six channels, as the frame carries them.
    first_head: Vec<u8>,
    /// The second head's six channels.
    second_head: Vec<u8>,
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
    /// What a fresh client is served at the end, base64.
    final_snapshot: String,
    /// Every rotation the hang uses, with this build's matrix for it.
    orientations: Vec<RecordedOrientation>,
    /// The frame with the first head lit.
    lit: LitFrame,
}

/// What this build says every rotation of the hang means.
fn orientations() -> Vec<RecordedOrientation> {
    the_hang()
        .into_iter()
        .map(|place| RecordedOrientation {
            rotation: place.rotation,
            matrix: orientation(place.rotation),
        })
        .collect()
}

/// The six channels of the head at `address` in a decoded frame.
fn head_at(frame: &TelemetryFrame, universe: u32, address: usize) -> Vec<u8> {
    let levels = frame
        .universes
        .iter()
        .find(|section| section.universe.get() == universe)
        .map(|section| &section.levels)
        .expect("the frame carries the heads' universe");
    levels[address - 1..address - 1 + 6].to_vec()
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
/// cargo test -p prismd --test ui_viewer -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rewrites a committed fixture; run it deliberately"]
async fn record_the_viewer_script_for_the_interface() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library");
    write_library(&library);

    let mut daemon = Daemon::start(&Options {
        data_dir: Some(dir.path().to_path_buf()),
        show: Some(dir.path().join("viewer.prism")),
        fixtures: Some(library),
        universes: Some(1),
        outputs: vec![mock_output(1)],
        local: Some(true),
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
    for (index, (what, command)) in script().into_iter().enumerate() {
        let seq = u64::try_from(index).unwrap_or(0) + 1;
        let message = ClientMessage::Command { seq, command };
        let client = common::encode_base64(&prism_ipc::encode(&message).expect("it encodes"));
        wire.send_message(&message)
            .await
            .expect("a message must reach the daemon");

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
        println!(
            "step {index} ({what}): {} deltas, refused: {refused}",
            deltas.len()
        );
        steps.push(Step {
            what: what.to_owned(),
            client,
            deltas,
            refused,
        });
    }

    // The lit frame: kept once `STEADY_FRAMES` in a row carry the same levels
    // and the first head is open, so it is a picture of the script and not of
    // a frame that was already on its way.
    let mut last: Option<Vec<u8>> = None;
    let mut agreeing = 0_usize;
    let mut looked = 0_usize;
    let lit = loop {
        let payload = next_payload(&mut wire).await;
        let ServerMessage::Telemetry { data } = decode(&payload) else {
            continue;
        };
        let frame = TelemetryFrame::decode(&data).expect("a frame this build can read");
        let head = head_at(&frame, 1, 1);
        looked += 1;
        // Ten seconds of telemetry is far past any settling this desk does.
        assert!(looked < 300, "the first head never opened: {head:?}");
        if head[4] != u8::MAX {
            continue;
        }
        if last.as_ref() == Some(&head) {
            agreeing += 1;
        } else {
            agreeing = 1;
            last = Some(head);
        }
        if agreeing >= STEADY_FRAMES {
            break LitFrame {
                first_head: head_at(&frame, 1, 1),
                second_head: head_at(&frame, 1, 11),
                message: common::encode_base64(&payload),
                universe: 1,
            };
        }
    };

    let (final_wire, final_snapshot) = connect(&address).await;
    final_wire.shutdown().await;
    wire.shutdown().await;

    let recording = Recording {
        note: "Recorded off a running prismd by crates/prismd/tests/ui_viewer.rs. \
               Every matrix is prism_domain::orientation's own answer and the lit frame \
               is prism_ipc::TelemetryFrame::encode's bytes. \
               Regenerate with: cargo test -p prismd --test ui_viewer -- --ignored"
            .to_owned(),
        protocol_version: prism_ipc::PROTOCOL_VERSION,
        initial_snapshot: common::encode_base64(&initial_snapshot),
        steps,
        final_snapshot: common::encode_base64(&final_snapshot),
        orientations: orientations(),
        lit,
    };

    let path = recording_path();
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

/// The show document inside a recorded snapshot.
fn show_of(encoded: &str) -> JsonValue {
    let ServerMessage::Snapshot { snapshot } = decode(&common::decode_base64(encoded)) else {
        panic!("that payload is not a snapshot");
    };
    snapshot.show
}

/// A fixture's `position` or `rotation`, read by pointer the way a client reads
/// it.
fn vec3_at(show: &JsonValue, id: u32, field: &str) -> Vec3 {
    let mirror = prism_core::JsonMirror::new(show.clone());
    let number = |axis: &str| -> f64 {
        match mirror.get(&format!("/fixtures/{id}/{field}/{axis}")).ok() {
            Some(JsonValue::Float(value)) => *value,
            Some(JsonValue::Int(value)) => *value as f64,
            other => panic!("fixture {id} has no {field}/{axis}: {other:?}"),
        }
    };
    v(number("x"), number("y"), number("z"))
}

/// **The deltas are the hang, and the hang survives Oops and Redo.**
#[test]
fn the_recorded_deltas_hang_the_rig_where_the_script_says() {
    let recording = recording();
    assert_eq!(recording.protocol_version, prism_ipc::PROTOCOL_VERSION);
    assert_eq!(recording.steps.len(), script().len());

    let mut show = ShowMirror::new(show_of(&recording.initial_snapshot));
    for (index, (step, (what, command))) in recording.steps.iter().zip(script()).enumerate() {
        assert_eq!(step.what, what, "step {index} is not the one in the script");
        let expected = ClientMessage::Command {
            seq: u64::try_from(index).unwrap_or(0) + 1,
            command,
        };
        assert_eq!(
            common::decode_base64(&step.client),
            prism_ipc::encode(&expected).expect("it encodes"),
            "step {index}: the recorded payload is not this message"
        );
        for encoded in &step.deltas {
            if let ServerMessage::Delta { delta } = decode(&common::decode_base64(encoded)) {
                show.apply_delta(&delta)
                    .unwrap_or_else(|error| panic!("step {index}: {error}"));
            }
        }
        match index {
            // The refusal wrote nothing, and fixture 1 did not move to the
            // origin on its way to being refused.
            3 => {
                assert!(step.refused, "a spread naming fixture 99 was accepted");
                assert!(
                    step.deltas.is_empty(),
                    "a refused spread broadcast something"
                );
                assert_eq!(vec3_at(show.value(), 1, "position"), v(-2.0, 6.0, 1.0));
            }
            // The Oops put all three back where the patch left them.
            4 => {
                for id in [1, 2, 10] {
                    assert_eq!(vec3_at(show.value(), id, "position"), Vec3::ZERO);
                    assert_eq!(vec3_at(show.value(), id, "rotation"), Vec3::ZERO);
                }
            }
            _ => assert!(!step.refused, "step {index} ({what}) was refused"),
        }
    }

    for place in the_hang() {
        let id = place.id.get();
        assert_eq!(vec3_at(show.value(), id, "position"), place.position);
        assert_eq!(vec3_at(show.value(), id, "rotation"), place.rotation);
    }
    assert_eq!(
        show.value(),
        &show_of(&recording.final_snapshot),
        "the deltas and a fresh client's snapshot disagree"
    );
}

/// **A placement is two `replace`s per fixture and nothing the engine reads.**
///
/// Asserted on the recorded bytes: the hang's answer is one `ShowPatch` whose
/// every operation is a `replace` of a `position` or a `rotation` that moved.
#[test]
fn the_hang_is_answered_with_replaces_of_the_place_only() {
    let recording = recording();
    let hang = &recording.steps[2];
    assert_eq!(hang.deltas.len(), 1, "one gesture is one delta");
    let ServerMessage::Delta { delta } = decode(&common::decode_base64(&hang.deltas[0])) else {
        panic!("not a delta");
    };
    let prism_domain::Delta::ShowPatch { ops } = delta else {
        panic!("a placement answered with {delta:?}");
    };
    // Five and not six: the first head keeps the rotation it was patched with,
    // and a field a gesture did not change is not written.
    assert_eq!(ops.len(), 5);
    for op in ops {
        let prism_domain::JsonPatchOp::Replace { path, .. } = &op else {
            panic!("a placement wrote {op:?}");
        };
        assert!(
            path.ends_with("/position") || path.ends_with("/rotation"),
            "{path}"
        );
    }
}

/// **The recorded matrices are this build's.** A change to the Euler order is
/// a change to what every show on every desk means, and it fails here before
/// the viewer can agree with it by accident.
#[test]
fn every_recorded_matrix_is_what_this_build_computes() {
    assert_eq!(recording().orientations, orientations());
}

/// **The profile the show embedded carries the beam the `.gdtf` states**, in
/// show space and in metres — `MATRIX_TO_METRES` applied once.
#[test]
fn the_embedded_head_carries_its_beam() {
    let show = show_of(&recording().final_snapshot);
    let mirror = prism_core::JsonMirror::new(show);
    let key = HEAD.replace('~', "~0").replace('/', "~1");
    let at = |pointer: &str| {
        mirror
            .get(&format!("/fixtureTypes/{key}/physical{pointer}"))
            .ok()
    };
    assert!(
        matches!(at("/beams/0/position/y"), Some(JsonValue::Float(y)) if (*y + 0.2).abs() < 1e-9),
        "{:?}",
        at("/beams/0/position/y")
    );
    assert!(
        matches!(at("/beams/0/direction/y"), Some(JsonValue::Float(y)) if (*y + 1.0).abs() < 1e-9),
        "{:?}",
        at("/beams/0/direction/y")
    );
    assert!(matches!(at("/model"), Some(JsonValue::String(model)) if model == "viewer-head"));
}

/// **The lit frame is a picture of the script.** The first head is open and
/// panned to `PAN`, big-endian across its two channels; the second is dark.
#[test]
fn the_lit_frame_is_the_first_head_open_and_panned() {
    let lit = recording().lit;
    let ServerMessage::Telemetry { data } = decode(&common::decode_base64(&lit.message)) else {
        panic!("the lit payload is not a telemetry message");
    };
    let frame = TelemetryFrame::decode(&data).expect("the frame is one this build can read");
    assert_eq!(head_at(&frame, lit.universe, 1), lit.first_head);
    assert_eq!(head_at(&frame, lit.universe, 11), lit.second_head);
    assert_eq!(lit.first_head[..2], PAN.to_be_bytes());
    assert_eq!(lit.first_head[4], u8::MAX, "the dimmer is open");
    assert_eq!(lit.second_head[4], 0, "the second head is dark");
}
