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
//! - **What a GDTF device is** (S30b). The head is a device, not a box: a
//!   yoke and a head on two axes, a gobo wheel whose picture is in the archive,
//!   a shutter with a closed, an open and a strobe function. The show embeds
//!   the geometry tree, every channel function and every wheel, and the
//!   visualiser evaluates them against the frame (`ui/src/viewer/state.ts`).
//! - **A file out of the fixture's own archive** (S30b). Three
//!   `Query::FixtureResource` exchanges: the gobo's picture served, a picture
//!   the archive does not have, and a name that tries to climb out of the
//!   archive — the last two answered with nothing, not refused.
//! - **What is on the cable.** One real telemetry frame with the first head
//!   lit, panned and its gobo in, so the viewer's reading of it is tested
//!   against `prism_ipc::TelemetryFrame::encode`'s bytes.
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
    Answer, AttributeType, Command, FixtureId, FixturePlace, JsonValue, Orientation,
    PatchPlacement, Query, ResourceKind, SelectionMode, UniverseId, Vec3, orientation,
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

/// The head's GDTF GUID, which is how the viewer asks for its files.
const GUID: &str = "5A1E0000-0000-4000-8000-0000000030AA";

/// How many channels the head takes.
const FOOTPRINT: usize = 8;

/// The gobo channel's value with the dots in: the middle of its `10…19` set.
const DOTS: u8 = 15;

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

/// The head's description: a body with a yoke on the pan axis and a head on
/// the tilt axis, the beam **0.2 m below** the head's origin, and eight
/// channels — pan and tilt at sixteen bits, a dimmer, a zoom, a gobo wheel and
/// a shutter.
///
/// The beam's translation is written as the GDTF specification states it — the
/// fourth number of the third row, in metres (`-0.2`) — which a published
/// Robe Robin T1 Profile confirmed on 2026-09-21 (S30b). The shutter stands
/// open by default, as a real one's does, so a head with only its dimmer up is
/// lit.
const DESCRIPTION: &str = r#"<GDTF DataVersion="1.2">
  <FixtureType Name="Viewer Head" Manufacturer="Prism Test"
               FixtureTypeID="5A1E0000-0000-4000-8000-0000000030AA">
    <Wheels>
      <Wheel Name="Gobo1">
        <Slot Name="Open"/>
        <Slot Name="Dots" MediaFileName="gobo-dots"/>
        <Slot Name="Star" MediaFileName="gobo-star"/>
      </Wheel>
    </Wheels>
    <Models><Model Name="Body" File="viewer-head" Length="0.3" Width="0.3" Height="0.5"/></Models>
    <Geometries>
      <Geometry Name="Body" Model="Body" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
        <Axis Name="Yoke" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
          <Axis Name="Head" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
            <Beam Name="Beam" Position="{1,0,0,0}{0,1,0,0}{0,0,1,-0.2}{0,0,0,1}"
                  BeamAngle="18" LuminousFlux="9000" ColorTemperature="6500"/>
          </Axis>
        </Axis>
      </Geometry>
    </Geometries>
    <DMXModes>
      <DMXMode Name="Standard" Geometry="Body">
        <DMXChannels>
          <DMXChannel Offset="1,2" Geometry="Yoke">
            <LogicalChannel Attribute="Pan">
              <ChannelFunction Attribute="Pan" PhysicalFrom="-270" PhysicalTo="270"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="3,4" Geometry="Head">
            <LogicalChannel Attribute="Tilt">
              <ChannelFunction Attribute="Tilt" PhysicalFrom="-135" PhysicalTo="135"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="5" Geometry="Beam">
            <LogicalChannel Attribute="Dimmer">
              <ChannelFunction Attribute="Dimmer"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="6" Geometry="Beam">
            <LogicalChannel Attribute="Zoom">
              <ChannelFunction Attribute="Zoom" PhysicalFrom="8" PhysicalTo="40"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="7" Geometry="Head">
            <LogicalChannel Attribute="Gobo1">
              <ChannelFunction Attribute="Gobo1" Wheel="Gobo1" DMXFrom="0/1">
                <ChannelSet Name="Open" DMXFrom="0/1" WheelSlotIndex="1"/>
                <ChannelSet Name="Dots" DMXFrom="10/1" WheelSlotIndex="2"/>
                <ChannelSet Name="Star" DMXFrom="20/1" WheelSlotIndex="3"/>
              </ChannelFunction>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="8" Geometry="Beam" Default="32/1">
            <LogicalChannel Attribute="Shutter1">
              <ChannelFunction Attribute="Shutter1" DMXFrom="0/1" Default="32/1">
                <ChannelSet Name="Closed" DMXFrom="0/1"/>
                <ChannelSet Name="Open" DMXFrom="32/1"/>
              </ChannelFunction>
              <ChannelFunction Attribute="Shutter1Strobe" DMXFrom="64/1"
                               PhysicalFrom="1" PhysicalTo="20"/>
            </LogicalChannel>
          </DMXChannel>
        </DMXChannels>
      </DMXMode>
    </DMXModes>
  </FixtureType>
</GDTF>"#;

/// Writes the head as a `.gdtf` — a stored ZIP of its description and the
/// dots gobo's picture — byte by byte. The star has no picture: a slot whose
/// file the archive lacks is the ordinary case, and answered with nothing.
///
/// A copy of `ui_patch.rs`'s writer rather than a shared one: an integration
/// target links a crate's library and not another target's helpers, and the
/// two recordings are allowed to move independently.
fn write_library(root: &Path) {
    let dots = dots_picture();
    let entries: [(&[u8], &[u8]); 2] = [
        (b"description.xml", DESCRIPTION.as_bytes()),
        (b"wheels/gobo-dots.png", &dots),
    ];
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for (name, body) in entries {
        let crc = crc32(body);
        let size = u32::try_from(body.len()).expect("a small entry");
        let name_length = u16::try_from(name.len()).expect("a short name");
        let at = u32::try_from(out.len()).expect("a small archive");
        out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes()); // local header
        out.extend_from_slice(&20_u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0_u16.to_le_bytes()); // flags
        out.extend_from_slice(&0_u16.to_le_bytes()); // stored
        out.extend_from_slice(&0_u32.to_le_bytes()); // time and date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_length.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes()); // extra
        out.extend_from_slice(name);
        out.extend_from_slice(body);

        central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes()); // made by
        central.extend_from_slice(&20_u16.to_le_bytes()); // needed
        central.extend_from_slice(&0_u16.to_le_bytes()); // flags
        central.extend_from_slice(&0_u16.to_le_bytes()); // stored
        central.extend_from_slice(&0_u32.to_le_bytes()); // time and date
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_length.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes()); // extra
        central.extend_from_slice(&0_u16.to_le_bytes()); // comment
        central.extend_from_slice(&0_u16.to_le_bytes()); // disk
        central.extend_from_slice(&0_u16.to_le_bytes()); // internal
        central.extend_from_slice(&0_u32.to_le_bytes()); // external
        central.extend_from_slice(&at.to_le_bytes()); // local header offset
        central.extend_from_slice(name);
    }

    let directory_at = u32::try_from(out.len()).expect("a small archive");
    let directory = u32::try_from(central.len()).expect("a small directory");
    let count = u16::try_from(entries.len()).expect("two entries");
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes()); // end record
    out.extend_from_slice(&0_u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0_u16.to_le_bytes()); // directory's disk
    out.extend_from_slice(&count.to_le_bytes()); // entries here
    out.extend_from_slice(&count.to_le_bytes()); // entries in all
    out.extend_from_slice(&directory.to_le_bytes());
    out.extend_from_slice(&directory_at.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes()); // comment

    std::fs::create_dir_all(root).expect("the library directory");
    std::fs::write(root.join("viewer-head.gdtf"), out).expect("a .gdtf file");
}

/// The dots gobo: a 16 × 16 greyscale PNG, white dots on black — GDTF's own
/// convention, white is light — built byte by byte like the archive around it.
///
/// PNG's picture is a zlib stream; this one is a single **stored** deflate
/// block, which every PNG reader must take and which needs no compressor here.
fn dots_picture() -> Vec<u8> {
    const SIZE: usize = 16;
    let mut raw = Vec::with_capacity(SIZE * (SIZE + 1));
    for y in 0..SIZE {
        raw.push(0); // no filter
        for x in 0..SIZE {
            let lit = x % 4 != 0 && x % 4 != 3 && y % 4 != 0 && y % 4 != 3;
            raw.push(if lit { u8::MAX } else { 0 });
        }
    }
    let length = u16::try_from(raw.len()).expect("one stored block");
    let mut zlib = vec![0x78, 0x01, 0x01];
    zlib.extend_from_slice(&length.to_le_bytes());
    zlib.extend_from_slice(&(!length).to_le_bytes());
    zlib.extend_from_slice(&raw);
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let side = u32::try_from(SIZE).expect("a small picture");
    let mut header = Vec::new();
    header.extend_from_slice(&side.to_be_bytes());
    header.extend_from_slice(&side.to_be_bytes());
    header.extend_from_slice(&[8, 0, 0, 0, 0]); // 8-bit grey, no interlace

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, body) in [(b"IHDR", header), (b"IDAT", zlib), (b"IEND", Vec::new())] {
        png.extend_from_slice(
            &u32::try_from(body.len())
                .expect("a small chunk")
                .to_be_bytes(),
        );
        let mut checked = kind.to_vec();
        checked.extend_from_slice(&body);
        png.extend_from_slice(&checked);
        png.extend_from_slice(&crc32(&checked).to_be_bytes());
    }
    png
}

/// Adler-32, zlib's checksum.
fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
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
        (
            "and put the dots in",
            Command::SetAttribute {
                attribute: AttributeType::Gobo,
                occurrence: 0,
                value: i32::from(u16::from(DOTS) * 257),
                relative: false,
            },
        ),
    ]
}

/// The files the viewer asks the daemon for, and why each.
fn resource_queries() -> Vec<(&'static str, Query)> {
    let ask = |name: &str| Query::FixtureResource {
        fixture_type_id: GUID.to_owned(),
        type_id: HEAD.to_owned(),
        kind: ResourceKind::Wheel,
        name: name.to_owned(),
        offset: 0,
    };
    vec![
        (
            "the dots gobo's picture, out of the head's own archive",
            ask("gobo-dots"),
        ),
        ("a picture the archive does not have", ask("gobo-star")),
        (
            "a name that tries to climb out of the archive",
            ask("../description"),
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

/// One question the viewer asked, and the daemon's answer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Exchange {
    /// What the question is for.
    what: String,
    /// The client's message, base64.
    client: String,
    /// The daemon's `Answer` message, base64.
    answer: String,
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
    /// The first head's eight channels, as the frame carries them.
    first_head: Vec<u8>,
    /// The second head's eight channels.
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
    /// The files the viewer asked for.
    resources: Vec<Exchange>,
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

/// The channels of the head at `address` in a decoded frame.
fn head_at(frame: &TelemetryFrame, universe: u32, address: usize) -> Vec<u8> {
    let levels = frame
        .universes
        .iter()
        .find(|section| section.universe.get() == universe)
        .map(|section| &section.levels)
        .expect("the frame carries the heads' universe");
    levels[address - 1..address - 1 + FOOTPRINT].to_vec()
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
        if head[4] != u8::MAX || head[6] != DOTS {
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

    let mut resources = Vec::new();
    for (index, (what, query)) in resource_queries().into_iter().enumerate() {
        let seq = 1000 + u64::try_from(index).unwrap_or(0);
        let message = ClientMessage::Query { seq, query };
        let client = common::encode_base64(&prism_ipc::encode(&message).expect("it encodes"));
        wire.send_message(&message)
            .await
            .expect("a question must reach the daemon");
        let answer = loop {
            let payload = next_payload(&mut wire).await;
            if matches!(decode(&payload), ServerMessage::Answer { seq: answered, .. } if answered == seq)
            {
                break common::encode_base64(&payload);
            }
        };
        println!("resource {index} ({what}) answered");
        resources.push(Exchange {
            what: what.to_owned(),
            client,
            answer,
        });
    }

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
        resources,
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
/// show space and in metres, read from the matrix's fourth column.
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
    assert_eq!(lit.first_head[6], DOTS, "the dots are in");
    assert_eq!(
        lit.first_head[7], 32,
        "the shutter stands at its open default"
    );
    assert_eq!(lit.second_head[4], 0, "the second head is dark");
}

/// **The profile the show embedded is the whole device** (S30b): the tree, the
/// channels' functions with their sets, and the wheel with its pictures.
#[test]
fn the_embedded_head_is_a_device() {
    let show = show_of(&recording().final_snapshot);
    let mirror = prism_core::JsonMirror::new(show);
    let key = HEAD.replace('~', "~0").replace('/', "~1");
    let at = |pointer: &str| {
        mirror
            .get(&format!("/fixtureTypes/{key}/physical{pointer}"))
            .ok()
            .cloned()
    };
    let text = |pointer: &str| match at(pointer) {
        Some(JsonValue::String(text)) => text,
        other => panic!("{pointer}: {other:?}"),
    };
    let int = |pointer: &str| match at(pointer) {
        Some(JsonValue::Int(value)) => value,
        other => panic!("{pointer}: {other:?}"),
    };
    // Body, Yoke, Head, Beam — each hanging from the one before.
    for (index, (name, kind)) in [
        ("Body", "Geometry"),
        ("Yoke", "Axis"),
        ("Head", "Axis"),
        ("Beam", "Beam"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(text(&format!("/geometries/{index}/name")), name);
        assert_eq!(text(&format!("/geometries/{index}/kind")), kind);
        if index > 0 {
            assert_eq!(
                int(&format!("/geometries/{index}/parent")),
                index as i64 - 1
            );
        }
    }
    assert_eq!(text("/fixtureTypeId"), GUID);
    // The gobo channel: one function on the wheel, three sets onto its slots.
    assert_eq!(text("/channels/4/attribute"), "Gobo1");
    assert_eq!(text("/channels/4/functions/0/wheel"), "Gobo1");
    assert_eq!(int("/channels/4/functions/0/sets/1/slot"), 2);
    assert_eq!(text("/channels/4/functions/0/sets/1/name"), "Dots");
    // The shutter: closed-and-open, then the strobe from 64.
    assert_eq!(text("/channels/5/functions/1/attribute"), "Shutter1Strobe");
    assert_eq!(text("/wheels/0/slots/1/media"), "gobo-dots");
}

/// **A file out of the head's own archive**, and nothing for a file it has not
/// got or a name that is not a file's.
#[test]
fn the_head_s_files_are_served_out_of_its_archive() {
    let recording = recording();
    assert_eq!(recording.resources.len(), resource_queries().len());
    for (index, (exchange, (what, query))) in recording
        .resources
        .iter()
        .zip(resource_queries())
        .enumerate()
    {
        assert_eq!(exchange.what, what);
        let expected = ClientMessage::Query {
            seq: 1000 + u64::try_from(index).unwrap_or(0),
            query,
        };
        assert_eq!(
            common::decode_base64(&exchange.client),
            prism_ipc::encode(&expected).expect("it encodes"),
            "resource {index}: the recorded question is not this one"
        );
        let ServerMessage::Answer {
            answer:
                Answer::FixtureResource {
                    kind,
                    path,
                    offset,
                    total,
                    data,
                    ..
                },
            ..
        } = decode(&common::decode_base64(&exchange.answer))
        else {
            panic!("resource {index} was not answered with a file");
        };
        assert_eq!(kind, ResourceKind::Wheel);
        assert_eq!(offset, 0);
        if index == 0 {
            let picture = dots_picture();
            assert_eq!(path, "wheels/gobo-dots.png");
            assert_eq!(total as usize, picture.len());
            assert_eq!(common::decode_base64(&data), picture);
        } else {
            assert_eq!((path.as_str(), total, data.as_str()), ("", 0, ""), "{what}");
        }
    }
}
