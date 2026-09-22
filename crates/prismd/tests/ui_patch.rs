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
use prism_domain::{Answer, Command, FixtureId, JsonValue, LibraryEntry, Query, UniverseId};
use prism_ipc::{ClientKind, ClientMessage, Hello, ServerMessage, Snapshot, Wire, local};
use prismd::cli::{Options, mock_output};
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
        show.embed_fixture_type(generic(type_id))
            .expect("a library profile is one a show accepts");
    }
    for (id, type_id, universe, address) in [
        (1_u32, "generic.dimmer", 1_u32, 1_u16),
        (2, "generic.dimmer", 1, 2),
        (5, "generic.rgbw.par", 1, 20),
    ] {
        show.patch_fixture(prism_domain::Fixture {
            software_dimmer: true,
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

/// One of the desk's four built-in profiles, by key.
fn generic(type_id: &str) -> prism_domain::FixtureType {
    prism_core::generic_profiles()
        .into_iter()
        .find(|profile| profile.id == type_id)
        .unwrap_or_else(|| panic!("the desk carries no {type_id}"))
}

/// **The library this recording is made against**, written into a directory of
/// its own — S44.
///
/// Pinned rather than taken from the machine, and that is the whole point: the
/// Open Fixture Library is *downloaded at install time*
/// (`profiles/fixtures/SOURCE.md`), so a recording made against whatever the
/// developer happened to have installed would be a recording that fails on a
/// fresh clone and changes whenever upstream does. Two fixtures in the Open
/// Fixture Library's own format and **one GDTF** is enough to record what a
/// search answers — and since S61 the third of them is what holds the
/// interface to the two fields a GDTF profile carries that a hand-written one
/// does not.
fn write_library(root: &Path) {
    write_gdtf(root);
    std::fs::create_dir_all(root.join("robe")).expect("a directory");
    std::fs::write(
        root.join("manufacturers.json"),
        r#"{ "robe": { "name": "Robe" } }"#,
    )
    .expect("it writes");
    std::fs::write(
        root.join("robe/wash-7q5.json"),
        r#"{
          "name": "Wash 7Q5",
          "availableChannels": {
            "Pan": { "capability": { "type": "Pan", "angleStart": "0deg", "angleEnd": "540deg" } },
            "Tilt": { "capability": { "type": "Tilt", "angleStart": "0deg", "angleEnd": "180deg" } },
            "Dimmer": { "capability": { "type": "Intensity" } },
            "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } }
          },
          "modes": [
            { "shortName": "4ch", "channels": ["Pan", "Tilt", "Dimmer", "Red"] },
            { "shortName": "2ch", "channels": ["Dimmer", "Red"] }
          ]
        }"#,
    )
    .expect("it writes");
    std::fs::write(
        root.join("robe/ledbeam.json"),
        r#"{
          "name": "LEDBeam 150",
          "availableChannels": {
            "Dimmer": { "capability": { "type": "Intensity" } }
          },
          "modes": [{ "shortName": "1ch", "channels": ["Dimmer"] }]
        }"#,
    )
    .expect("it writes");
}

/// **One GDTF in the pinned library** — S61.
///
/// A `.gdtf` file is a ZIP archive holding a `description.xml`, so this writes
/// one byte by byte: an integration target links a crate's *library* and not
/// its `#[cfg(test)]` modules, which is the same reason
/// `crates/prism-core/src/testkit.rs` says its callers carry their own copy.
/// Stored rather than deflated, so it stays the format and no compression.
///
/// It is here rather than in `crates/prismd/tests/common/mod.rs` because the
/// recording is what it is for: the interface has to be held to a library with
/// a GDTF profile in it, with its beams and its format marked, and that cannot
/// be recorded off a library that has none.
fn write_gdtf(root: &Path) {
    const DESCRIPTION: &str = r#"<GDTF DataVersion="1.2">
      <FixtureType Name="Robin T1 Profile" Manufacturer="Robe Lighting"
                   FixtureTypeID="9F4A0000-0000-4000-8000-00000000T1PR">
        <Models><Model Name="Body" File="body" Length="0.34" Width="0.34" Height="0.55"/></Models>
        <Geometries>
          <Geometry Name="Body" Model="Body" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
            <Beam Name="Beam" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0.4}{0,0,0,1}"
                  BeamAngle="13" LuminousFlux="11000" ColorTemperature="6500"/>
          </Geometry>
        </Geometries>
        <DMXModes>
          <DMXMode Name="Mode 1" Geometry="Body">
            <DMXChannels>
              <DMXChannel Offset="1,2">
                <LogicalChannel Attribute="Pan">
                  <ChannelFunction Attribute="Pan" PhysicalFrom="-270" PhysicalTo="270"/>
                </LogicalChannel>
              </DMXChannel>
              <DMXChannel Offset="3">
                <LogicalChannel Attribute="Dimmer">
                  <ChannelFunction Attribute="Dimmer"/>
                </LogicalChannel>
              </DMXChannel>
            </DMXChannels>
          </DMXMode>
        </DMXModes>
      </FixtureType>
    </GDTF>"#;

    let name = b"description.xml";
    let body = DESCRIPTION.as_bytes();
    let crc = crc32(body);
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes()); // local header
    out.extend_from_slice(&20_u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0_u16.to_le_bytes()); // flags
    out.extend_from_slice(&0_u16.to_le_bytes()); // stored
    out.extend_from_slice(&0_u32.to_le_bytes()); // time and date
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
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
    central.extend_from_slice(&(body.len() as u32).to_le_bytes());
    central.extend_from_slice(&(body.len() as u32).to_le_bytes());
    central.extend_from_slice(&(name.len() as u16).to_le_bytes());
    central.extend_from_slice(&0_u16.to_le_bytes()); // extra
    central.extend_from_slice(&0_u16.to_le_bytes()); // comment
    central.extend_from_slice(&0_u16.to_le_bytes()); // disk
    central.extend_from_slice(&0_u16.to_le_bytes()); // internal
    central.extend_from_slice(&0_u32.to_le_bytes()); // external
    central.extend_from_slice(&0_u32.to_le_bytes()); // local header offset
    central.extend_from_slice(name);

    let size = u32::try_from(central.len()).expect("a small directory");
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes()); // end record
    out.extend_from_slice(&0_u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0_u16.to_le_bytes()); // directory's disk
    out.extend_from_slice(&1_u16.to_le_bytes()); // entries here
    out.extend_from_slice(&1_u16.to_le_bytes()); // entries in all
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&directory_at.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes()); // comment

    std::fs::create_dir_all(root).expect("the library directory");
    std::fs::write(root.join("robe-t1.gdtf"), out).expect("a .gdtf file");
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
    /// Whether the desk supplies this fixture's intensity — S43.
    ///
    /// **Absent means supplied**, which is the daemon's own serde default, so a
    /// recording made from a show written before S43 records `true` and the
    /// interface reading it draws the switch on. Both ends read the document
    /// the same way, which is the whole point of the recording.
    software_dimmer: bool,
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
    /// Whether the mode has a dimmer channel of its own — S43.
    ///
    /// What decides whether the patch form offers the software-dimmer switch at
    /// all, so the recording carries it: the interface must reach the same
    /// answer from the same document.
    has_intensity: bool,
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
    /// How many profiles this desk offers, as the snapshot carries it.
    ///
    /// A number and not the profiles — S44. The library is two thousand entries
    /// on an installed desk and cannot travel in a snapshot; what a client is
    /// sent is this count and the answers to its searches.
    library_size: u32,
    /// Every profile the pinned library holds, so the browser can be held to
    /// what a search *should* have found as well as to what it did.
    library: Vec<LibraryEntry>,
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
        adding: 0,
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
                software_dimmer: true,
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
                software_dimmer: true,
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
            "and a profile nobody has got is refused before it is sent",
            Query::PatchPreview {
                id: FixtureId::new(7),
                type_id: "nothing.at.all".to_owned(),
                universe: UniverseId::new(1),
                address: 100,
                adding: 0,
            },
        ),
        Scripted::Ask(
            "a profile the show has not got is answered out of the desk's library — S57",
            Query::PatchPreview {
                id: FixtureId::new(7),
                type_id: "generic.movinghead".to_owned(),
                universe: UniverseId::new(1),
                address: 100,
                adding: 0,
            },
        ),
        Scripted::Ask(
            "search the desk's library, which is where a real fixture comes from",
            Query::SearchLibrary {
                text: "robe wash".to_owned(),
                limit: 10,
            },
        ),
        Scripted::Ask(
            "a search that matches nothing answers with nothing, not with everything",
            Query::SearchLibrary {
                text: "no such light".to_owned(),
                limit: 10,
            },
        ),
        Scripted::Ask(
            "and an empty search is *show me something*: the whole library, clamped",
            Query::SearchLibrary {
                text: String::new(),
                limit: 3,
            },
        ),
        Scripted::Do(
            "embed a real fixture, by the key the search answered with",
            Command::EmbedFixtureType {
                type_id: "robe/wash-7q5/4ch".to_owned(),
            },
        ),
        Scripted::Do(
            "patch it: the channels are the manufacturer's, not this desk's",
            Command::PatchFixture {
                software_dimmer: true,
                id: FixtureId::new(11),
                name: "Wash 11".to_owned(),
                type_id: "robe/wash-7q5/4ch".to_owned(),
                universe: UniverseId::new(2),
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
                adding: 0,
            },
        ),
        Scripted::Do(
            "patch a moving head",
            Command::PatchFixture {
                software_dimmer: true,
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
                software_dimmer: true,
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
        // S57, punch-list B60: the library a fixture at a time, the next free
        // address, and several of one fixture in one gesture.
        Scripted::Ask(
            "browse the library a fixture at a time: the first page of one",
            Query::BrowseLibrary {
                text: "robe".to_owned(),
                offset: 0,
                limit: 1,
            },
        ),
        Scripted::Ask(
            "and the page after it, which is what a list scrolled to its end asks",
            Query::BrowseLibrary {
                text: "robe".to_owned(),
                offset: 1,
                limit: 1,
            },
        ),
        Scripted::Ask(
            "the third page, which is the GDTF and says so",
            Query::BrowseLibrary {
                text: "robe".to_owned(),
                offset: 2,
                limit: 1,
            },
        ),
        Scripted::Ask(
            "which fixture is this mode of, with every mode it has",
            Query::FixtureOfMode {
                type_id: "robe/wash-7q5/2ch".to_owned(),
            },
        ),
        Scripted::Ask(
            "a new wash on the moving head's channels: the overlap, and the next free address",
            Query::PatchPreview {
                id: FixtureId::new(20),
                type_id: "robe/wash-7q5/2ch".to_owned(),
                universe: UniverseId::new(2),
                address: 1,
                adding: 1,
            },
        ),
        Scripted::Ask(
            "three new washes from there: where each of them would go",
            Query::PatchPreview {
                id: FixtureId::new(20),
                type_id: "robe/wash-7q5/2ch".to_owned(),
                universe: UniverseId::new(2),
                address: 14,
                adding: 3,
            },
        ),
        Scripted::Do(
            "patch the three in one gesture, named after their type",
            Command::PatchFixtures {
                type_id: "robe/wash-7q5/2ch".to_owned(),
                name: String::new(),
                software_dimmer: true,
                placements: vec![placed(20, 2, 14), placed(21, 2, 16), placed(22, 2, 18)],
            },
        ),
        Scripted::Do(
            "a new fixture on a number that is patched: refused, and nothing moves",
            Command::PatchFixtures {
                type_id: "robe/wash-7q5/2ch".to_owned(),
                name: "Twin".to_owned(),
                software_dimmer: true,
                placements: vec![placed(20, 1, 200)],
            },
        ),
        Scripted::Do(
            "a profile the desk does not carry: refused",
            Command::EmbedFixtureType {
                type_id: "nothing.at.all".to_owned(),
            },
        ),
        Scripted::Do(
            "and take the whole edit back: all three washes and their profile",
            Command::Oops,
        ),
    ]
}

/// One fixture of a `PatchFixtures`.
fn placed(id: u32, universe: u32, address: u16) -> prism_domain::PatchPlacement {
    prism_domain::PatchPlacement {
        id: FixtureId::new(id),
        universe: UniverseId::new(universe),
        address,
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
    let library = dir.path().join("library");
    write_library(&library);

    let mut daemon = Daemon::start(&Options {
        data_dir: Some(dir.path().to_path_buf()),
        show: Some(show),
        fixtures: Some(library),
        universes: Some(2),
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
    let library_size = snapshot_of_payload(&initial_snapshot).fixture_library;
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
        library_size,
        library: pinned_library(),
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
                    .get(&format!("/fixtureTypes/{}/name", escape(&type_id)))
                    .ok()
                    .and_then(|found| match found {
                        JsonValue::String(text) => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| type_id.clone()),
                universe: u32::try_from(int_at(value, "universe")).unwrap_or(0),
                address: u16::try_from(int_at(value, "address")).unwrap_or(0),
                footprint: mirror
                    .get(&format!("/fixtureTypes/{}/footprint", escape(&type_id)))
                    .ok()
                    .and_then(|found| match found {
                        JsonValue::Int(number) => u16::try_from(*number).ok(),
                        _ => None,
                    })
                    .unwrap_or(0),
                software_dimmer: !matches!(
                    mirror.get(&format!("/fixtures/{key}/softwareDimmer")),
                    Ok(JsonValue::Bool(false))
                ),
                type_id,
            }
        })
        .collect();
    rows.sort_by_key(|row| row.id);
    rows
}

/// One key as a JSON Pointer reference token — RFC 6901 §3.
///
/// A profile key out of the Open Fixture Library is `manufacturer/fixture/mode`,
/// so a pointer built by pasting one in names three levels of a document that
/// has one. `prism_core::show::escape` is the same three lines at the other end.
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
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
            has_intensity: match member(value, "attributes") {
                Some(JsonValue::Array(defs)) => defs.iter().any(|def| {
                    matches!(member(def, "attribute"), Some(JsonValue::String(name)) if name == "Dimmer")
                }),
                _ => false,
            },
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

/// The library the recording was made against, as this build reads it.
///
/// Built from the same two files [`write_library`] writes, plus the four
/// built-in generics the daemon adds last — so the guard below compares the
/// recorded answers with what *this* build's reader and search make of the same
/// input, rather than with a list written out by hand twice.
fn pinned_library() -> Vec<LibraryEntry> {
    let dir = tempfile::tempdir().expect("a temporary directory");
    write_library(dir.path());
    let mut library = prism_core::FixtureLibrary::default();
    library.read_installed_tree(dir.path());
    for profile in prism_core::generic_profiles() {
        library.insert_profile(profile);
    }
    library.entries().to_vec()
}

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
        recording.library,
        pinned_library(),
        "the recorded library is not what this build reads from the same files"
    );
    assert_eq!(
        recording.library_size as usize,
        recording.library.len(),
        "the snapshot's count and the library disagree"
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
    // Found by what the step *says it is for* rather than by a number: the
    // script grows, and an index written down here would silently start
    // pointing at another step.
    let preview = |about: &str| {
        let step = recording
            .steps
            .iter()
            .find(|step| step.what.contains(about))
            .unwrap_or_else(|| panic!("no step is about {about:?}"));
        match answer_of(
            step.answer
                .as_deref()
                .unwrap_or_else(|| panic!("the step about {about:?} has no answer")),
        ) {
            Answer::PatchPreview { preview } => preview,
            other => panic!("the step about {about:?} is not a preview: {other:?}"),
        }
    };
    let rows_after = |about: &str| {
        &recording
            .steps
            .iter()
            .find(|step| step.what.contains(about))
            .unwrap_or_else(|| panic!("no step is about {about:?}"))
            .rows
    };

    // A clear address: accepted, no overlap, and the end channel the daemon
    // worked out is `address + footprint - 1`.
    let clear = preview("would a PAR fit at address 30");
    assert!(clear.accepted);
    assert_eq!(clear.footprint, 4);
    assert_eq!(clear.last_address, Some(33));
    assert!(clear.conflicts.is_empty());
    // The patch that followed put fixture 6 exactly there.
    let after = rows_after("patch it there");
    let six = after
        .iter()
        .find(|row| row.id == 6)
        .expect("fixture 6 was patched");
    assert_eq!((six.address, six.footprint), (30, 4));

    // An overlapping address: **accepted all the same**, and the overlap is
    // named before it happens.
    let clashing = preview("would a second PAR at 32 clash");
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
    let overlap_step = recording
        .steps
        .iter()
        .find(|step| step.what.contains("now the show has an overlap"))
        .expect("the script asks");
    let Answer::PatchConflicts { conflicts } =
        answer_of(overlap_step.answer.as_deref().expect("it was answered"))
    else {
        panic!("that step is not a conflicts answer");
    };
    assert_eq!(conflicts, vec![conflict]);

    // A refusal, said before the command was ever sent.
    let past_the_end = preview("would it fit at 510");
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

    // A profile nobody has got is refused, with nothing to measure it by.
    let missing = preview("a profile nobody has got");
    assert!(!missing.accepted);
    assert_eq!(missing.footprint, 0);
    assert_eq!(missing.next_free, None);
    // **A profile only the desk's library has is answered out of it** — S57.
    // Browsing the library embeds nothing any more, so the preview cannot wait
    // for an embed; it measures the library's copy, which is the copy the
    // patch will embed.
    let from_library = preview("answered out of the desk's library");
    assert!(from_library.accepted, "{from_library:?}");
    assert_eq!(from_library.footprint, 13);
    let embedded = preview("the same question again");
    assert!(embedded.accepted);
    // The generic moving head, which **S43 grew from eleven channels to
    // thirteen**: the encoder banks became seven and a head with no gobo wheel
    // and no lamp-control channel could no longer reach all of them. See
    // `prism_core::library::moving_head`.
    assert_eq!(embedded.footprint, 13);
    assert_eq!(embedded.last_address, Some(112));
}

/// **The next free address and the placements are the daemon's, and the patch
/// that follows them is exactly what they said** — S57, punch-list **B60**.
#[test]
fn several_of_one_fixture_go_where_the_preview_said_and_one_oops_takes_them_back() {
    let recording = recording();
    let step = |about: &str| {
        recording
            .steps
            .iter()
            .find(|step| step.what.contains(about))
            .unwrap_or_else(|| panic!("no step is about {about:?}"))
    };
    let preview = |about: &str| match answer_of(step(about).answer.as_deref().expect("answered")) {
        Answer::PatchPreview { preview } => preview,
        other => panic!("the step about {about:?} is not a preview: {other:?}"),
    };

    // On the moving head: an overlap, and where the whole wash would fit.
    let clashing = preview("on the moving head's channels");
    assert!(clashing.accepted);
    assert_eq!(clashing.conflicts.len(), 1, "{clashing:?}");
    assert_eq!(
        clashing.next_free,
        Some(prism_domain::PatchAddress {
            universe: UniverseId::new(2),
            address: 14,
        }),
        "the head is 1-13, so a two-channel wash fits at 14"
    );
    assert_eq!(clashing.placements, vec![placed(20, 2, 1)]);

    // Three, one after the other.
    let three = preview("three new washes");
    assert_eq!(
        three.placements,
        vec![placed(20, 2, 14), placed(21, 2, 16), placed(22, 2, 18)]
    );
    // The command sent is those placements, and the rows are exactly them.
    let patched = &step("patch the three in one gesture").rows;
    for placement in &three.placements {
        let row = patched
            .iter()
            .find(|row| row.id == placement.id.get())
            .unwrap_or_else(|| panic!("fixture {} was not patched", placement.id));
        assert_eq!((row.universe, row.address), (2, placement.address));
        assert_eq!(row.type_id, "robe/wash-7q5/2ch");
    }
    // Named after their type, told apart by their place in the gesture.
    let names: Vec<&str> = patched
        .iter()
        .filter(|row| (20..=22).contains(&row.id))
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(names, vec!["Wash 7Q5 1", "Wash 7Q5 2", "Wash 7Q5 3"]);
    // And the profile came with them, in the same step.
    assert!(
        step("patch the three in one gesture")
            .profiles
            .iter()
            .any(|profile| profile.id == "robe/wash-7q5/2ch")
    );
    assert!(!step("patch the three in one gesture").deltas.is_empty());
    assert!(step("on a number that is patched").refused);

    // **One Oops takes all three back, and the profile with them.**
    let last = recording.steps.last().expect("there are steps");
    assert!(
        last.rows.iter().all(|row| !(20..=22).contains(&row.id)),
        "{:?}",
        last.rows
    );
    assert!(
        last.profiles
            .iter()
            .all(|profile| profile.id != "robe/wash-7q5/2ch"),
        "the profile the gesture embedded is still there"
    );
}

/// **The library is listed a fixture at a time, a page at a time** — S57.
#[test]
fn the_library_is_browsed_a_fixture_at_a_time() {
    let recording = recording();
    let page = |about: &str| {
        let step = recording
            .steps
            .iter()
            .find(|step| step.what.contains(about))
            .unwrap_or_else(|| panic!("no step is about {about:?}"));
        match answer_of(step.answer.as_deref().expect("answered")) {
            Answer::LibraryFixtures {
                fixtures,
                matched,
                total,
            } => (fixtures, matched, total),
            other => panic!("the step about {about:?} is not a page: {other:?}"),
        }
    };
    let (first, matched, total) = page("the first page of one");
    let (second, _, _) = page("the page after it");
    // Three Robe fixtures since S61 — the two written in the Open Fixture
    // Library's format and the GDTF beside them, whose manufacturer is *Robe
    // Lighting* and so matches the same word.
    assert_eq!(matched, 3);
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].name, second[0].name);
    // The four generics, the two Robes and the GDTF.
    assert_eq!(total, 7);
    let wash = first
        .iter()
        .chain(&second)
        .find(|fixture| fixture.name == "Wash 7Q5")
        .expect("the wash is one of them");
    assert_eq!(
        wash.modes
            .iter()
            .map(|mode| (mode.mode.as_str(), mode.footprint))
            .collect::<Vec<_>>(),
        vec![("4ch", 4), ("2ch", 2)],
        "both modes, in the order the file lists them"
    );
    assert!(wash.modes.iter().all(|mode| mode.has_intensity));
    // **S61's two fields, in the recording the browser reads.** The wash is a
    // hand-written Open Fixture Library profile: channels and names, which is
    // what every desk had before this session and what the Format column says
    // *OFL* about.
    assert!(!wash.gdtf);
    assert!(wash.modes.iter().all(|mode| mode.beams == 0));

    // And the GDTF beside it carries what the viewer draws.
    let (third, _, _) = page("the third page, which is the GDTF");
    let t1 = third.first().expect("a page of one");
    assert_eq!(t1.name, "Robin T1 Profile");
    assert_eq!(t1.manufacturer, "Robe Lighting");
    assert!(t1.gdtf, "it came out of a .gdtf file");
    assert_eq!(
        t1.modes
            .iter()
            .map(|mode| (mode.id.as_str(), mode.footprint, mode.beams))
            .collect::<Vec<_>>(),
        vec![("robe-lighting/robin-t1-profile/mode-1", 3, 1)],
        "one mode, three channels, one beam"
    );

    let step = recording
        .steps
        .iter()
        .find(|step| step.what.contains("which fixture is this mode of"))
        .expect("the script asks");
    let Answer::FixtureOfMode { fixture } = answer_of(step.answer.as_deref().expect("answered"))
    else {
        panic!("that is not a fixture answer");
    };
    assert_eq!(fixture.as_ref(), Some(wash));
}

/// **A search is answered out of the desk's library, and the answer is small.**
///
/// The reason the library left the snapshot (S44): two thousand profiles do not
/// fit in a frame. What is asserted here is that the recorded answers are what
/// *this build's* search makes of the same pinned library — so a browser held to
/// them is held to the daemon, and a search that changed shape fails here first.
#[test]
fn a_search_is_answered_out_of_the_library() {
    let recording = recording();
    let library = pinned_library();
    let searched = |about: &str| -> (Vec<LibraryEntry>, u32) {
        let step = recording
            .steps
            .iter()
            .find(|step| step.what.contains(about))
            .unwrap_or_else(|| panic!("no step is about {about:?}"));
        match answer_of(step.answer.as_deref().expect("it was answered")) {
            Answer::LibraryMatches { matches, total } => (matches, total),
            other => panic!("the step about {about:?} is not a search: {other:?}"),
        }
    };

    // A search that finds something, and finds the *right* something: both
    // modes of the Robe wash, smallest first, and nothing else.
    let (found, total) = searched("search the desk's library");
    assert_eq!(total as usize, library.len());
    assert_eq!(
        found
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["robe/wash-7q5/2ch", "robe/wash-7q5/4ch"]
    );
    assert_eq!(found[0].manufacturer, "Robe");
    assert_eq!(found[0].name, "Wash 7Q5");

    // A search that finds nothing answers with nothing — **not** with
    // everything, which is what a search that ignored an unmatched word would
    // do and which an operator would read as *the library is broken*.
    let (none, _) = searched("a search that matches nothing");
    assert!(none.is_empty(), "{none:?}");

    // And an empty search is *show me something*, clamped to what was asked for.
    let (some, _) = searched("an empty search is");
    assert_eq!(some.len(), 3);
    assert!(some.len() < library.len(), "the limit was not applied");

    // The key that was embedded afterwards is one the search answered with,
    // which is the whole loop: search, choose, embed, patch.
    let embedded = recording
        .steps
        .iter()
        .find(|step| step.what.contains("embed a real fixture"))
        .expect("the script embeds one");
    assert!(
        embedded
            .profiles
            .iter()
            .any(|profile| profile.id == "robe/wash-7q5/4ch"),
        "the show did not gain the profile that was searched for"
    );
    // And patching it put the manufacturer's channel count in the sheet.
    let patched = recording
        .steps
        .iter()
        .find(|step| step.what.contains("the channels are the manufacturer's"))
        .expect("the script patches it");
    let row = patched
        .rows
        .iter()
        .find(|row| row.id == 11)
        .expect("fixture 11 is patched");
    assert_eq!(row.type_id, "robe/wash-7q5/4ch");
    assert_eq!(row.footprint, 4);
    assert_eq!(row.type_name, "Wash 7Q5");
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

/// One built-in profile by key, or `None`.
fn generic_profiles_by_key(type_id: &str) -> Option<prism_domain::FixtureType> {
    prism_core::generic_profiles()
        .into_iter()
        .find(|profile| profile.id == type_id)
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
            generic_profiles_by_key(&fixture_type.id).as_ref(),
            Some(fixture_type),
            "{} is not the desk's own profile",
            fixture_type.id
        );
    }
}
