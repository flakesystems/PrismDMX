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
        attribute,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value: home,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

fn dimmer_type(id: &str, home: u16) -> FixtureType {
    FixtureType {
        id: id.to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "1ch".to_owned(),
        footprint: 1,
        attributes: vec![attribute(AttributeType::Dimmer, 0, home)],
    }
}

fn fixture(id: u32, type_id: &str, universe: u32, address: u16) -> Fixture {
    Fixture {
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
                value: 65535,
                preset_ref: None,
            }],
        }],
        looping: false,
    })
    .unwrap();
    show.store_executor(Executor {
        id: ExecutorId::new(0),
        sequence_id: Some(SequenceId::new(1)),
        fader_function: ExecutorFaderFunction::Master,
        button_functions: Vec::new(),
        encoder_function: ExecutorEncoderFunction::Empty,
        master_level: 65535,
        is_active: false,
        current_cue_index: None,
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
