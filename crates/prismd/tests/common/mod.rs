//! Shared scenery for the daemon's integration targets.
//!
//! Each target compiles its own copy and uses a subset, which is what the allow
//! below is for — the pattern `prism-core` established in S11.

#![allow(dead_code)]

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use prism_core::{SessionState, Show, ShowFile, ShowStore};
use prism_domain::{
    AttributeDef, AttributeType, Cue, CuePart, CueTrigger, Executor, ExecutorEncoderFunction,
    ExecutorFaderFunction, ExecutorId, Fixture, FixtureId, FixtureType, Sequence, SequenceId,
    UniverseId, Vec3,
};

/// One daemon in this process at a time.
///
/// Not fastidiousness: a daemon owns a tick thread raised to the priority
/// `ARCHITECTURE_SPEC.md` §3 asks for, and it spins for the last millisecond of
/// every 22.7 ms slot. Eight of those in one test process do not measure the
/// daemon, they measure each other — which is S6's finding about two timing
/// tests in one binary, one level up, and it showed up here as a client task
/// that never got a core to run on.
///
/// So the daemon tests take turns, the way `prism-protocols`' hardware tests
/// do. Poisoning is ignored: a test that panicked while holding this has
/// finished, and its daemon has been dropped by the unwinding.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Takes the turn. Held for as long as the returned guard lives.
pub fn one_daemon_at_a_time() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An 8-bit attribute at a given offset.
fn attribute(attribute: AttributeType, coarse_offset: u16, home: u16) -> AttributeDef {
    AttributeDef {
        switched: None,
        attribute,
        label: None,
        occurrence: 0,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value: home,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
        ranges: Vec::new(),
    }
}

pub fn dimmer_type(id: &str, home: u16) -> FixtureType {
    FixtureType {
        id: id.to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "1ch".to_owned(),
        footprint: 1,
        attributes: vec![attribute(AttributeType::Dimmer, 0, home)],
        physical: None,
    }
}

pub fn fixture(id: u32, type_id: &str, universe: u32, address: u16) -> Fixture {
    Fixture {
        software_dimmer: true,
        id: FixtureId::new(id),
        name: format!("Fixture {id}"),
        type_id: type_id.to_owned(),
        universe: UniverseId::new(universe),
        address,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        invert_pan: false,
        invert_tilt: false,
    }
}

/// The rig every daemon test opens: three dimmers in universe 1, one of them at
/// full when nothing is overriding it, one dark so a cue raising it can be
/// seen, and a sequence on executor 0.
///
/// The session is deliberately **not** at its defaults. A fixture built out of
/// default values cannot tell "carried correctly" from "never touched" (S14's
/// finding, a rule since S15), and this show file is what the snapshot test
/// asserts against.
pub fn show_file() -> ShowFile {
    let mut show = Show::new();
    show.embed_fixture_type(dimmer_type("generic.dimmer", 65535))
        .unwrap();
    show.embed_fixture_type(dimmer_type("generic.dimmer.dark", 0))
        .unwrap();
    show.patch_fixture(fixture(1, "generic.dimmer", 1, 1))
        .unwrap();
    show.patch_fixture(fixture(2, "generic.dimmer.dark", 1, 5))
        .unwrap();
    show.patch_fixture(fixture(3, "generic.dimmer.dark", 2, 1))
        .unwrap();
    show.store_sequence(Sequence {
        id: SequenceId::new(1),
        name: "Sequence 1".to_owned(),
        color: None,
        cues: vec![Cue {
            number: "1".to_owned(),
            name: "Cue 1".to_owned(),
            fade_in: 0.0,
            fade_out: 0.0,
            delay: 0.0,
            trigger: CueTrigger::Go,
            trigger_time: None,
            parts: vec![CuePart {
                fixture: FixtureId::new(2),
                attribute: AttributeType::Dimmer,
                occurrence: 0,
                value: 65535,
                preset_ref: None,
                tracking: prism_domain::CueTracking::Track,
            }],
        }],
        looping: false,
        master_level: u16::MAX,
        speed: prism_domain::SPEED_UNITY,
        is_active: false,
        current_cue_index: None,
        crossfade_position: 0,
    })
    .unwrap();
    show.store_executor(Executor {
        id: ExecutorId::new(0),
        sequence_id: Some(SequenceId::new(1)),
        fader_function: ExecutorFaderFunction::Master,
        button_functions: Vec::new(),
        encoder_function: ExecutorEncoderFunction::Empty,
    })
    .unwrap();

    let mut session = SessionState::new();
    session.set_executor_page(3).unwrap();
    session.set_command_line("fixture 1 at full").unwrap();

    ShowFile {
        show,
        session,
        ..ShowFile::new()
    }
}

/// Writes that rig into a `.prism` file, the way an operator's last save would
/// have left it.
pub fn write_show(path: &Path) {
    let mut file = show_file();
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

/// How many universes the wide rig spans.
///
/// Sized by what it is for rather than by realism, though it is realistic
/// enough: a telemetry frame carries 514 bytes per **patched** universe
/// (`docs/IPC_PROTOCOL.md` §7), so twenty-four of them is about twelve
/// kilobytes thirty times a second — enough that a client which has stopped
/// reading fills a socket buffer in a fraction of a second instead of several.
/// A backpressure test that took ten seconds to reach the state it is about
/// would be a test nobody runs.
pub const WIDE_UNIVERSES: u32 = 24;

/// How many commands the backpressure gate sends past a client that is not
/// reading.
///
/// Comfortably under `prism_ipc::DEFAULT_CONTROL_QUEUE`, because the claim being
/// measured is *no control message is dropped* — a client pushed past the queue
/// limit is disconnected on purpose and would be measuring the other half of §8.
pub const SLOW_CLIENT_COMMANDS: u32 = 40;

/// The rig of [`show_file`] with one more dimmer in every universe up to
/// `universes`, written to a `.prism` file.
///
/// Patched rather than merely configured: `prismd` filters telemetry down to the
/// universes the show actually patches, so a wide *layout* over a narrow show
/// would carry exactly as little as before.
pub fn write_wide_show(path: &Path, universes: u32) {
    let mut file = show_file();
    for universe in 3..=universes {
        file.show
            .patch_fixture(fixture(100 + universe, "generic.dimmer", universe, 1))
            .unwrap();
    }
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

/// The last frame the output was given for one universe.
///
/// Not simply the last frame: an output carrying several universes sends them
/// in turn, so "the last one" is whichever happened to be at the end of the
/// cadence.
pub fn last_frame_of(
    output: &prism_protocols::MockOutputHandle,
    universe: UniverseId,
) -> Option<Vec<u8>> {
    output
        .frames()
        .into_iter()
        .rev()
        .find(|(sent, _)| *sent == universe)
        .map(|(_, data)| data)
}

/* -------------------------------------------------------------------------- */
/* Base64, so a recorded payload can live in a text file                      */
/* -------------------------------------------------------------------------- */

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (RFC 4648 §4).
///
/// Eleven lines rather than a dependency, for the reason the recordings exist at
/// all: a fixture regenerated next year has to come out byte for byte the same,
/// and that is a shorter promise to keep here than across a version bump.
pub fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let (a, b, c) = (
            u32::from(chunk[0]),
            chunk.get(1).map_or(0, |byte| u32::from(*byte)),
            chunk.get(2).map_or(0, |byte| u32::from(*byte)),
        );
        let triple = (a << 16) | (b << 8) | c;
        for shift in [18, 12, 6, 0] {
            let index = usize::try_from((triple >> shift) & 0x3f).unwrap_or(0);
            out.push(char::from(ALPHABET[index]));
        }
        let written = out.len();
        for missing in chunk.len()..3 {
            out.replace_range(written - 3 + missing..written - 2 + missing, "=");
        }
    }
    out
}

/// The other direction, refusing anything that is not base64.
pub fn decode_base64(text: &str) -> Vec<u8> {
    let mut bits = 0_u32;
    let mut held = 0_u32;
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    for character in text.bytes() {
        if character == b'=' {
            break;
        }
        let index = ALPHABET
            .iter()
            .position(|letter| *letter == character)
            .unwrap_or_else(|| panic!("{} is not base64", char::from(character)));
        bits = (bits << 6) | u32::try_from(index).unwrap_or(0);
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push(u8::try_from((bits >> held) & 0xff).unwrap_or(0));
        }
    }
    out
}

/// Where the interface's fixtures live, from a test target in this crate.
pub fn ui_fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("ui/tests/fixtures")
        .join(name)
}

/// The local endpoint, read the way `docs/IPC_PROTOCOL.md` §2.2 says a client
/// reads it: out of the lock file the daemon wrote.
pub fn local_address(data_dir: &Path) -> String {
    let text = std::fs::read_to_string(data_dir.join("prismd.lock"))
        .expect("the daemon publishes a lock file for clients to find it by");
    let document: serde_json::Value = serde_json::from_str(&text).unwrap();
    document["local"]
        .as_str()
        .expect("the daemon published a local endpoint")
        .to_owned()
}

/// A `.gdtf` archive with one mode of `footprint` dimmers — **S62**.
///
/// Built here rather than taken from `prism-core`: a `#[cfg(test)]` module is
/// not compiled into the library an integration test links against, which is
/// the same reason `crates/prism-core/src/testkit.rs` says its callers carry
/// their own copy.
#[must_use]
pub fn gdtf_archive(manufacturer: &str, name: &str, footprint: u16) -> Vec<u8> {
    let channels: String = (1..=footprint)
        .map(|offset| {
            format!(
                r#"<DMXChannel Offset="{offset}"><LogicalChannel Attribute="Dimmer">
                     <ChannelFunction Attribute="Dimmer"/></LogicalChannel></DMXChannel>"#
            )
        })
        .collect();
    let description = format!(
        r#"<GDTF DataVersion="1.2"><FixtureType Name="{name}" Manufacturer="{manufacturer}"
             FixtureTypeID="GUID"><DMXModes><DMXMode Name="Mode 1">
             <DMXChannels>{channels}</DMXChannels></DMXMode></DMXModes>
           </FixtureType></GDTF>"#
    );
    zip_of(&[("description.xml", description.as_bytes())])
}

/// A ZIP of the members given, written by hand and **stored**.
///
/// Stored rather than deflated so this needs no compressor: `prism_core`'s
/// reader takes both, and a test dependency added for a test helper is a
/// dependency the shipping crate then carries in its lock file. Everything
/// here is the container's own fixed-width records — the same shape
/// `prism_core::library::zip`'s own testkit builds, which an integration test
/// cannot reach.
#[must_use]
pub fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let mut count: u16 = 0;
    for (name, body) in files {
        let name = name.as_bytes();
        let crc = crc32(body);
        let size = u32::try_from(body.len()).expect("a small member");
        let at = u32::try_from(out.len()).expect("a small archive");

        out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        // version needed, flags, method (0 = stored), time, date
        for value in [20_u16, 0, 0, 0, 0] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(body);

        central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        // made by, version needed, flags, method, time, date
        for value in [20_u16, 20, 0, 0, 0, 0] {
            central.extend_from_slice(&value.to_le_bytes());
        }
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        // extra, comment, disk, internal attributes
        for value in [0_u16, 0, 0, 0] {
            central.extend_from_slice(&value.to_le_bytes());
        }
        central.extend_from_slice(&0_u32.to_le_bytes());
        central.extend_from_slice(&at.to_le_bytes());
        central.extend_from_slice(name);
        count += 1;
    }
    let directory_at = u32::try_from(out.len()).expect("a small archive");
    let size = u32::try_from(central.len()).expect("a small directory");
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    for value in [0_u16, 0, count, count] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&directory_at.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out
}

/// CRC-32 as ZIP states it.
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
