//! The telemetry frames the interface's decoder is tested against, recorded off
//! a running daemon — and the check that the recording is still one.
//!
//! # Why the bytes come from here
//!
//! S24's decoder reads `docs/IPC_PROTOCOL.md` §7's fixed layout in TypeScript.
//! The trap is the one every session since S19 has found in its own layer: **a
//! test that uses the function under test to work out what the answer should be
//! is not a test.** A TypeScript encoder feeding a TypeScript decoder would pass
//! with the header misread, the endianness reversed and the section stride
//! wrong, as long as all three were wrong consistently.
//!
//! So the bytes are `prism_ipc::TelemetryFrame::encode`'s, off a real `prismd`
//! over a real socket — a picture of what the fixtures were actually being given
//! — and **the expectation is `TelemetryFrame::decode`'s answer**, written down
//! beside them. Nothing in `ui/` decides what a frame means.
//!
//! # What is in the file
//!
//! - `narrow` — frames from the ordinary three-dimmer rig: two universes, with
//!   every level written out so the interface can be held to all 512 of them.
//! - `wide` — one frame from a rig patched across **all 64 universes**, which is
//!   the size §7 sizes the channel for and the size S24's budget is measured at.
//!   Its levels are described by a digest rather than written out, because the
//!   frame is 32 912 bytes and the digest catches every misplacement a copy of
//!   the bytes would.
//! - `malformed` — a real frame with one thing wrong with it, and the fault this
//!   build answers with. Telemetry is droppable by definition, so *dropped* is
//!   the right answer to a layout this build does not know, and this is where
//!   the interface is held to giving it.
//!
//! # It is frozen, and the regenerator is `#[ignore]`d
//!
//! [`record_the_telemetry_channel_for_the_interface`] starts two daemons and
//! rewrites two committed fixtures, which is a deliberate act. What runs on
//! every commit is [`the_recording_is_a_telemetry_stream_this_build_could_have_sent`],
//! which decodes every payload with this build's own layout: a layout that moved
//! stops decoding *here*, next to the daemon, rather than going quietly stale in
//! `ui/`.
//!
//! # The rig is a fixture too
//!
//! `ui/tests/fixtures/wide-rig.prism` is a show patched across 64 universes,
//! written by the same regenerator. `ui/e2e/telemetry.spec.ts` starts a real
//! daemon on a copy of it, which is how the 8 ms frame budget is measured
//! against 64 real universes at 30 Hz in a real browser rather than against a
//! frame the test made up.

// This target *writes files for a person to commit* and says where. The same
// allowance the timing tests carry, for the same reason.
#![allow(clippy::print_stdout)]
// Every test holds `common::one_daemon_at_a_time` across its awaits.
#![allow(clippy::await_holding_lock)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use prism_core::{ShowFile, ShowStore};
use prism_domain::{AttributeType, Command, Fixture, FixtureId, SelectionMode, UniverseId, Vec3};
use prism_ipc::{
    ClientKind, ClientMessage, Hello, ServerMessage, TELEMETRY_HEADER_BYTES, TELEMETRY_VERSION,
    TelemetryError, TelemetryFrame, Wire, local,
};
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;

mod common;

/// How long any wait here may take before it is a named failure.
const PATIENCE: Duration = Duration::from_secs(20);

/// Channels in one universe. `prism_domain::CHANNELS_PER_UNIVERSE` as a `usize`.
const CHANNELS: usize = 512;

/// Bytes of one universe section: the universe number, then its levels.
const SECTION_BYTES: usize = 2 + CHANNELS;

/// How many frames are kept from each daemon.
///
/// More than one, because a single frame cannot show that the sequence number
/// moves — and a decoder that read the sequence out of the wrong eight bytes
/// would answer with the same number for ever.
const NARROW_FRAMES: usize = 4;

/// One is enough for the wide rig: it is 32 912 bytes, and what it is for is the
/// count, the stride and the last universe's levels landing where they belong.
const WIDE_FRAMES: usize = 1;

/// How many frames to let pass before recording, so the engine has settled and
/// the commands the script sent are in the picture.
const SETTLING_FRAMES: usize = 6;

/// The rig the wide fixture patches: every universe the desk has room for.
const WIDE_UNIVERSES: u32 = 64;

/// Dimmers patched into each universe of the wide rig.
///
/// Alternating between the type that comes up at full and the one that comes up
/// dark, so a universe is not a block of one value: a decoder that took the
/// first byte of a section and repeated it would pass against a blackout and
/// against a full-on rig alike.
const WIDE_PER_UNIVERSE: u32 = 8;

/// Where the interface reads the frames from.
fn recording_path() -> PathBuf {
    common::ui_fixture("telemetry-recording.json")
}

/// Where the end-to-end suite reads the 64-universe rig from.
fn wide_rig_path() -> PathBuf {
    common::ui_fixture("wide-rig.prism")
}

/* -------------------------------------------------------------------------- */
/* The recording                                                              */
/* -------------------------------------------------------------------------- */

/// What one universe section decodes to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UniverseExpectation {
    /// The universe number out of the section header.
    universe: u32,
    /// Every level, base64 — `None` on the wide frame, where the digest stands
    /// in its place.
    #[serde(skip_serializing_if = "Option::is_none")]
    levels: Option<String>,
    /// The levels added up, as a whole number.
    sum: u32,
    /// How many of the 512 are not zero.
    non_zero: u32,
    /// FNV-1a (32-bit) over the 512 decoded bytes.
    ///
    /// A digest rather than a copy, so a 64-universe frame costs a line rather
    /// than 44 kB — and it fails on a single byte in the wrong place, which is
    /// the only mistake a stride error can make.
    fingerprint: u32,
    /// `[channel, level]` at four fixed channels, so the file can be read by a
    /// person and a decoder can be caught by eye.
    samples: Vec<[u32; 2]>,
}

/// What one recorded payload decodes to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrameExpectation {
    /// The sequence number out of the header.
    sequence: u64,
    /// How many universe sections the header declares.
    universe_count: u32,
    /// One per section, in the order they appear.
    universes: Vec<UniverseExpectation>,
}

/// One frame as it came off the socket, and what it means.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedFrame {
    /// What it is, for a person reading the file.
    what: String,
    /// The bytes inside `ServerMessage::Telemetry { data }`, base64.
    payload: String,
    /// `TelemetryFrame::decode`'s answer.
    expect: FrameExpectation,
}

/// One frame with something wrong with it, and what this build says about it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MalformedFrame {
    /// What was done to it.
    what: String,
    /// The bytes, base64.
    payload: String,
    /// The fault, in the spelling the interface uses.
    fault: String,
    /// The layout version the frame announced, where the fault is about one.
    #[serde(skip_serializing_if = "Option::is_none")]
    found: Option<u32>,
    /// The version or the byte count this build expected.
    #[serde(skip_serializing_if = "Option::is_none")]
    expected: Option<u32>,
}

/// The whole file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryRecording {
    /// What made it, and how to make it again.
    note: String,
    /// The layout version these frames are of.
    telemetry_version: u8,
    /// The four bytes every frame starts with.
    magic: String,
    /// Bytes of fixed header.
    header_bytes: u32,
    /// Channels in one universe.
    channels_per_universe: u32,
    /// Bytes of one universe section.
    section_bytes: u32,
    /// Frames from the ordinary rig: two universes, levels written out.
    narrow: Vec<RecordedFrame>,
    /// A frame from a rig across all 64 universes.
    wide: Vec<RecordedFrame>,
    /// Frames that must be dropped, and the fault each one produces.
    malformed: Vec<MalformedFrame>,
}

/// FNV-1a, 32-bit, over one universe's levels.
///
/// Written out rather than taken from a crate because the interface computes the
/// same digest over its own decode: two implementations of eleven lines is a
/// smaller risk than a dependency on both sides, and a mistake in either is a
/// red test rather than a quiet pass.
fn fingerprint(levels: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5_u32;
    for byte in levels {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// What one decoded frame means, as the file records it.
fn expectation(frame: &TelemetryFrame, with_levels: bool) -> FrameExpectation {
    FrameExpectation {
        sequence: frame.sequence,
        universe_count: u32::try_from(frame.universes.len()).unwrap_or(u32::MAX),
        universes: frame
            .universes
            .iter()
            .map(|universe| UniverseExpectation {
                universe: universe.universe.get(),
                levels: with_levels.then(|| common::encode_base64(&universe.levels)),
                sum: universe.levels.iter().map(|level| u32::from(*level)).sum(),
                non_zero: u32::try_from(
                    universe.levels.iter().filter(|level| **level != 0).count(),
                )
                .unwrap_or(u32::MAX),
                fingerprint: fingerprint(&universe.levels),
                samples: [0_usize, 1, 255, 511]
                    .into_iter()
                    .map(|channel| {
                        [
                            u32::try_from(channel).unwrap_or(0),
                            u32::from(universe.levels[channel]),
                        ]
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The fault this build answers with, in the spelling the interface uses.
fn malformed(what: &str, bytes: &[u8]) -> MalformedFrame {
    let error = TelemetryFrame::decode(bytes)
        .err()
        .unwrap_or_else(|| panic!("{what} was supposed to be refused and was read"));
    let (fault, found, expected) = match error {
        TelemetryError::NotTelemetry => ("not-telemetry", None, None),
        TelemetryError::UnknownVersion { found, expected } => (
            "unknown-version",
            Some(u32::from(found)),
            Some(u32::from(expected)),
        ),
        TelemetryError::Truncated { expected, found } => (
            "truncated",
            Some(u32::try_from(found).unwrap_or(u32::MAX)),
            Some(u32::try_from(expected).unwrap_or(u32::MAX)),
        ),
    };
    MalformedFrame {
        what: what.to_owned(),
        payload: common::encode_base64(bytes),
        fault: fault.to_owned(),
        found,
        expected,
    }
}

/* -------------------------------------------------------------------------- */
/* The rigs                                                                   */
/* -------------------------------------------------------------------------- */

/// The ordinary rig with one more dimmer bank in every universe up to 64.
///
/// Patched rather than merely configured: `prismd` filters telemetry down to the
/// universes the show actually patches (`crates/prismd/src/core.rs`), so a wide
/// *layout* over a narrow show would carry exactly as little as before.
fn wide_show() -> ShowFile {
    let mut file = common::show_file();
    for universe in 1..=WIDE_UNIVERSES {
        for index in 0..WIDE_PER_UNIVERSE {
            let dark = (universe + index) % 2 == 0;
            file.show
                .patch_fixture(Fixture {
                    id: FixtureId::new(1000 + universe * 16 + index),
                    name: format!("Wide {universe}.{index}"),
                    type_id: if dark {
                        "generic.dimmer.dark".to_owned()
                    } else {
                        "generic.dimmer".to_owned()
                    },
                    universe: UniverseId::new(universe),
                    // Spread across the universe so the last section of the
                    // frame has something in its second half: a stride error
                    // that lost the tail of a universe would otherwise only show
                    // up as zeros where there were already zeros.
                    address: u16::try_from(1 + index * 61).unwrap_or(1),
                    position: Vec3::ZERO,
                    rotation: Vec3::ZERO,
                    invert_pan: false,
                    invert_tilt: false,
                })
                .expect("the wide rig patches");
        }
    }
    file
}

/// Writes a show file, replacing whatever was there.
fn write_show_file(path: &Path, file: &mut ShowFile) {
    for suffix in ["", "-shm", "-wal"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    let mut store = ShowStore::open(path).expect("a show file can be written");
    store.save(file).expect("a show file can be saved");
}

/* -------------------------------------------------------------------------- */
/* Recording it                                                               */
/* -------------------------------------------------------------------------- */

/// A wire that has said hello, past the snapshot.
async fn connect(address: &str) -> Wire {
    let mut wire = local::connect(address)
        .await
        .expect("the daemon is listening");
    wire.send_message(&ClientMessage::Hello {
        hello: Hello::new(ClientKind::Desktop),
    })
    .await
    .expect("a hello must reach the daemon");
    loop {
        if matches!(
            decode(&next_payload(&mut wire).await),
            ServerMessage::Snapshot { .. }
        ) {
            return wire;
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

/// Runs a daemon on `show`, sends `script`, and keeps `frames` telemetry frames.
async fn frames_from(
    directory: &Path,
    show: &Path,
    universes: u32,
    script: &[Command],
    frames: usize,
) -> Vec<Vec<u8>> {
    let mut daemon = Daemon::start(&Options {
        data_dir: Some(directory.to_path_buf()),
        show: Some(show.to_path_buf()),
        universes,
        outputs: vec![mock_output(1)],
        local: true,
        log_level: prismd::log::Level::Warn,
        ..Options::default()
    })
    .await
    .expect("the daemon starts");
    let address = common::local_address(directory);

    let (stop, stopped) = tokio::sync::oneshot::channel();
    let running = tokio::spawn(async move {
        daemon
            .run(None, async {
                let _ = stopped.await;
            })
            .await;
        daemon
    });

    let mut wire = connect(&address).await;
    for (step, command) in script.iter().enumerate() {
        wire.send_message(&ClientMessage::Command {
            seq: u64::try_from(step).unwrap_or(0) + 1,
            command: command.clone(),
        })
        .await
        .expect("a command must reach the daemon");
    }

    let mut collected = Vec::with_capacity(frames);
    let mut seen = 0_usize;
    while collected.len() < frames {
        if let ServerMessage::Telemetry { data } = decode(&next_payload(&mut wire).await) {
            seen += 1;
            // The first frames are of a desk that has just been switched on and
            // has not yet been told anything.
            if seen > SETTLING_FRAMES {
                collected.push(data);
            }
        }
    }
    wire.shutdown().await;

    let _ = stop.send(());
    let daemon = tokio::time::timeout(PATIENCE, running)
        .await
        .expect("the daemon must stop when it is told to")
        .expect("the daemon task must not panic");
    daemon.shutdown().await;
    collected
}

/// **The regenerator.** Runs two daemons and writes both fixtures.
///
/// ```text
/// cargo test -p prismd --test ui_telemetry -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rewrites committed fixtures; run it deliberately"]
async fn record_the_telemetry_channel_for_the_interface() {
    let _turn = common::one_daemon_at_a_time();

    // The rig the end-to-end suite runs a daemon on, written first: the daemon
    // below opens a *copy*, so a recording run never modifies what it committed.
    write_show_file(&wide_rig_path(), &mut wide_show());

    let dir = tempfile::tempdir().expect("a temporary directory");
    let narrow_show = dir.path().join("aula.prism");
    common::write_show(&narrow_show);

    // Something the rig is not already doing by itself. The ordinary show comes
    // up with one dimmer at full and one at nothing, and a frame of 255s and 0s
    // would not catch a decoder reading levels through a lookup table — so the
    // dimmer in universe 2 is taken to 40 % and the one in universe 1 is left
    // where it was, which makes three distinct levels in one frame.
    let script = vec![
        Command::SelectFixtures {
            ids: vec![FixtureId::new(3)],
            mode: SelectionMode::Set,
        },
        Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            value: 26_214, // 40 % — 102 on the wire
            relative: false,
        },
    ];
    let narrow = frames_from(dir.path(), &narrow_show, 2, &script, NARROW_FRAMES).await;

    let wide_copy = dir.path().join("wide.prism");
    std::fs::copy(wide_rig_path(), &wide_copy).expect("the wide rig can be copied");
    let wide = frames_from(dir.path(), &wide_copy, WIDE_UNIVERSES, &[], WIDE_FRAMES).await;

    let sample = narrow.first().expect("a narrow frame was recorded").clone();
    let malformed = vec![
        malformed("a layout version this build does not know", &{
            let mut bytes = sample.clone();
            bytes[4] = TELEMETRY_VERSION + 1;
            bytes
        }),
        malformed("something that is not telemetry at all", &{
            let mut bytes = sample.clone();
            bytes[0] = b'X';
            bytes
        }),
        malformed("a frame one byte shorter than its header says", {
            &sample[..sample.len() - 1]
        }),
        malformed("a frame with a byte too many", &{
            let mut bytes = sample.clone();
            bytes.push(0);
            bytes
        }),
        malformed("a universe count larger than the bytes that follow", &{
            let mut bytes = sample.clone();
            bytes[6] = 64;
            bytes
        }),
        malformed("nothing at all", &[]),
    ];

    let recording = TelemetryRecording {
        note: "Recorded off a running prismd by crates/prismd/tests/ui_telemetry.rs. \
               Payloads are base64 of the binary telemetry frames a client received; \
               every expectation is prism_ipc::TelemetryFrame::decode's own answer. \
               Regenerate with: cargo test -p prismd --test ui_telemetry -- --ignored"
            .to_owned(),
        telemetry_version: TELEMETRY_VERSION,
        magic: "PTLM".to_owned(),
        header_bytes: u32::try_from(TELEMETRY_HEADER_BYTES).unwrap_or(0),
        channels_per_universe: u32::try_from(CHANNELS).unwrap_or(0),
        section_bytes: u32::try_from(SECTION_BYTES).unwrap_or(0),
        narrow: narrow
            .iter()
            .enumerate()
            .map(|(index, payload)| RecordedFrame {
                what: format!("the three-dimmer rig, frame {}", index + 1),
                payload: common::encode_base64(payload),
                expect: expectation(&read(payload), true),
            })
            .collect(),
        wide: wide
            .iter()
            .map(|payload| RecordedFrame {
                what: format!("{WIDE_UNIVERSES} universes, {WIDE_PER_UNIVERSE} dimmers in each"),
                payload: common::encode_base64(payload),
                expect: expectation(&read(payload), false),
            })
            .collect(),
        malformed,
    };

    let path = recording_path();
    std::fs::create_dir_all(path.parent().expect("the fixture lives in a directory"))
        .expect("the fixture directory can be made");
    let mut text = serde_json::to_string_pretty(&recording).expect("the recording serialises");
    text.push('\n');
    std::fs::write(&path, text.replace("\r\n", "\n")).expect("the recording can be written");
    println!(
        "wrote {} ({} narrow, {} wide, {} malformed, {} bytes)",
        path.display(),
        recording.narrow.len(),
        recording.wide.len(),
        recording.malformed.len(),
        std::fs::metadata(&path).map_or(0, |data| data.len())
    );
    println!(
        "wrote {} ({} bytes)",
        wide_rig_path().display(),
        std::fs::metadata(wide_rig_path()).map_or(0, |data| data.len())
    );
}

/// A payload as the frame it is, or a named failure.
fn read(payload: &[u8]) -> TelemetryFrame {
    TelemetryFrame::decode(payload).expect("the daemon sends frames this build can read")
}

/* -------------------------------------------------------------------------- */
/* The checks that run on every commit                                        */
/* -------------------------------------------------------------------------- */

/// Reads the committed recording.
fn recording() -> TelemetryRecording {
    let path = recording_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is missing: {error}", path.display()));
    serde_json::from_str(&text).expect("the recording is JSON this build understands")
}

/// **The recording is still a channel this build could have sent.**
///
/// Every payload is decoded with this build's own layout and compared against
/// the expectation beside it — the same claim the interface makes in TypeScript,
/// made here in the language the frames were encoded in. If the two ever
/// disagree, this is the one that is right.
#[test]
fn the_recording_is_a_telemetry_stream_this_build_could_have_sent() {
    let recording = recording();
    assert_eq!(recording.telemetry_version, TELEMETRY_VERSION);
    assert_eq!(
        recording.header_bytes,
        u32::try_from(TELEMETRY_HEADER_BYTES).unwrap_or(0)
    );
    assert_eq!(
        recording.section_bytes,
        u32::try_from(SECTION_BYTES).unwrap_or(0)
    );
    assert_eq!(recording.narrow.len(), NARROW_FRAMES);
    assert_eq!(recording.wide.len(), WIDE_FRAMES);

    for frame in recording.narrow.iter().chain(&recording.wide) {
        let payload = common::decode_base64(&frame.payload);
        let decoded = TelemetryFrame::decode(&payload)
            .unwrap_or_else(|error| panic!("{}: {error}", frame.what));
        let with_levels = frame
            .expect
            .universes
            .first()
            .is_some_and(|universe| universe.levels.is_some());
        assert_eq!(
            expectation(&decoded, with_levels),
            frame.expect,
            "{} decodes to something else now",
            frame.what
        );
        // And the bytes the interface will be handed are the bytes this crate
        // would write, which is the claim the whole file rests on.
        assert_eq!(
            decoded.encode(),
            payload,
            "{} does not re-encode",
            frame.what
        );
    }

    for frame in &recording.malformed {
        let payload = common::decode_base64(&frame.payload);
        assert_eq!(
            malformed(&frame.what, &payload),
            *frame,
            "{} produces a different fault now",
            frame.what
        );
    }
}

/// The recording is worth having.
///
/// Every assertion above passes against a file of blackouts on one universe, so
/// this is the one that says the frames are a picture of a rig that is doing
/// something: 64 universes in the wide frame, three distinct levels across the
/// narrow ones, a sequence number that moves, and levels in the *second* half of
/// a universe, which is where a stride error hides.
#[test]
fn the_recorded_frames_are_a_picture_of_a_rig_that_is_lit() {
    let recording = recording();

    let wide = recording.wide.first().expect("a wide frame is recorded");
    assert_eq!(wide.expect.universe_count, WIDE_UNIVERSES);
    assert_eq!(
        wide.expect
            .universes
            .iter()
            .map(|universe| universe.universe)
            .collect::<Vec<_>>(),
        (1..=WIDE_UNIVERSES).collect::<Vec<_>>(),
        "the wide frame carries every universe, in order"
    );
    assert!(
        wide.expect
            .universes
            .iter()
            .all(|universe| universe.non_zero > 0),
        "a universe in the wide frame is dark, so nothing there would be noticed"
    );

    let levels: Vec<u32> = recording
        .narrow
        .iter()
        .flat_map(|frame| frame.expect.universes.iter())
        .flat_map(|universe| universe.samples.iter().map(|sample| sample[1]))
        .collect();
    let mut distinct = levels.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert!(
        distinct.len() >= 3,
        "the narrow frames show only {distinct:?}, so a decoder that read one byte would pass"
    );

    let sequences: Vec<u64> = recording
        .narrow
        .iter()
        .map(|frame| frame.expect.sequence)
        .collect();
    assert!(
        sequences.windows(2).all(|pair| pair[1] > pair[0]),
        "the sequence number does not move across the narrow frames: {sequences:?}"
    );

    // The universe of a section is read out of the section rather than assumed
    // from its position, and the narrow rig is the case that would let an
    // implementation get away with assuming: it patches 1 and 2.
    let narrow = recording
        .narrow
        .first()
        .expect("a narrow frame is recorded");
    assert_eq!(narrow.expect.universe_count, 2);
    assert!(
        narrow
            .expect
            .universes
            .iter()
            .any(|universe| universe.samples.iter().any(|sample| sample[1] > 0)),
        "the narrow rig is dark"
    );
}

/// The 64-universe rig the end-to-end suite measures the frame budget against.
///
/// Checked here rather than in `ui/`, for the reason the delta recording is:
/// a show file is `prism-core`'s format, and a fixture that stopped opening
/// should fail in Rust on the next commit rather than in a browser months later.
#[test]
fn the_wide_rig_is_a_show_patched_across_every_universe() {
    let path = wide_rig_path();
    let store = ShowStore::open(&path)
        .unwrap_or_else(|error| panic!("{} does not open: {error}", path.display()));
    let mut file = ShowFile::new();
    store
        .load(&mut file)
        .unwrap_or_else(|error| panic!("{} does not load: {error}", path.display()));

    assert_eq!(
        file.show.universes(),
        (1..=WIDE_UNIVERSES)
            .map(UniverseId::new)
            .collect::<Vec<_>>(),
        "the wide rig is what telemetry at 64 universes is measured against"
    );
    assert!(
        file.show.fixtures().count()
            >= usize::try_from(WIDE_UNIVERSES * WIDE_PER_UNIVERSE).unwrap_or(0),
        "the wide rig has lost fixtures"
    );
}
