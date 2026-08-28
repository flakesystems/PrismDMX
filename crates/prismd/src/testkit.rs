//! Scenery for this crate's own tests: a small rig, built once.
//!
//! The same shape `prism-core`'s `tests/common/mod.rs` has, and for the same
//! reason — a daemon test is about the daemon, and a fixture type written out
//! in the middle of one is thirty lines nobody reads.
//!
//! **A fixture built out of default values cannot tell a restore from a
//! no-op** (S14's finding, a rule since S15). So nothing here is at its type's
//! default: the dimmers have home values that differ from each other and from
//! zero, the executor has a master that is not full, and the session starts on
//! a page that is not the first.

#![allow(dead_code, reason = "each test module uses a subset of the scenery")]

use prism_core::{SessionState, Show, ShowFile};
use prism_domain::{
    AttributeDef, AttributeType, Cue, CuePart, CueTrigger, Executor, ExecutorEncoderFunction,
    ExecutorFaderFunction, ExecutorId, Fixture, FixtureId, FixtureType, Group, GroupId, Preset,
    PresetId, PresetValue, Sequence, SequenceId, UniverseId, Vec3,
};

/// An 8-bit attribute at a given offset, with everything else neutral.
#[must_use]
pub fn attribute(attribute: AttributeType, coarse_offset: u16, home: u16) -> AttributeDef {
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

/// A one-channel dimmer whose home value is given, so two rigs can be told
/// apart by the byte they put on the wire.
#[must_use]
pub fn dimmer_type(id: &str, home: u16) -> FixtureType {
    FixtureType {
        id: id.to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "1ch".to_owned(),
        footprint: 1,
        attributes: vec![attribute(AttributeType::Dimmer, 0, home)],
    }
}

/// A four-channel RGBW PAR.
#[must_use]
pub fn par_type() -> FixtureType {
    FixtureType {
        id: "generic.rgbw.par".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "RGBW PAR".to_owned(),
        mode: "4ch".to_owned(),
        footprint: 4,
        attributes: vec![
            attribute(AttributeType::Red, 0, 0),
            attribute(AttributeType::Green, 1, 0),
            attribute(AttributeType::Blue, 2, 0),
            attribute(AttributeType::White, 3, 0),
        ],
    }
}

/// A patched fixture at home geometry.
#[must_use]
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

/// A cue with one part and no preset link.
#[must_use]
pub fn cue(number: &str, fixture: u32, attribute: AttributeType, value: u16) -> Cue {
    Cue {
        number: number.to_owned(),
        name: format!("Cue {number}"),
        fade_in: 0.0,
        fade_out: 0.0,
        delay: 0.0,
        trigger: CueTrigger::Go,
        trigger_time: None,
        parts: vec![CuePart {
            fixture: FixtureId::new(fixture),
            attribute,
            value,
            preset_ref: None,
        }],
    }
}

/// A sequence carrying the cues given.
#[must_use]
pub fn sequence(id: u32, cues: Vec<Cue>) -> Sequence {
    Sequence {
        id: SequenceId::new(id),
        name: format!("Sequence {id}"),
        color: None,
        cues,
        looping: false,
        is_active: false,
        current_cue_index: None,
    }
}

/// A group of the fixtures given.
#[must_use]
pub fn group(id: u32, fixtures: &[u32]) -> Group {
    Group {
        id: GroupId::new(id),
        name: format!("Group {id}"),
        fixtures: fixtures.iter().copied().map(FixtureId::new).collect(),
    }
}

/// A preset holding one value.
#[must_use]
pub fn preset(id: u32, fixture: u32, attribute: AttributeType, value: u16) -> Preset {
    Preset {
        id: PresetId::new(id),
        pool: attribute.feature_group().into(),
        name: format!("Preset {id}"),
        color: None,
        values: vec![PresetValue {
            fixture: FixtureId::new(fixture),
            attribute,
            value,
        }],
    }
}

/// An executor, optionally playing a sequence.
#[must_use]
pub fn executor(id: u32, sequence_id: Option<u32>) -> Executor {
    Executor {
        id: ExecutorId::new(id),
        sequence_id: sequence_id.map(SequenceId::new),
        fader_function: ExecutorFaderFunction::Master,
        button_functions: Vec::new(),
        encoder_function: ExecutorEncoderFunction::Empty,
        master_level: 40000,
        speed: prism_domain::SPEED_UNITY,
        is_active: false,
        current_cue_index: None,
    }
}

/// A rig: a dimmer at home in universe 1 channel 1, a dark one at channel 5, a
/// PAR at channel 10, a group, a preset, a sequence and an executor playing it.
///
/// The two dimmers have different home values on purpose. Channel 1 is at full
/// whenever nothing is overriding it, so a test can say *the rig is at home* in
/// one assertion; channel 5 is dark at home, so a **cue** raising it is visible
/// — the playbacks merge HTP against the home layer, and a cue below home can
/// never show.
#[must_use]
pub fn show() -> Show {
    let mut show = Show::new();
    show.embed_fixture_type(dimmer_type("generic.dimmer", 65535))
        .unwrap();
    show.embed_fixture_type(dimmer_type("generic.dimmer.dark", 0))
        .unwrap();
    show.embed_fixture_type(par_type()).unwrap();
    show.patch_fixture(fixture(1, "generic.dimmer", 1, 1))
        .unwrap();
    // **The PAR's colour channels are driven directly here** — S43. A fixture
    // with no dimmer of its own is given one by the desk, resting at nought and
    // scaling its colour (`Fixture::software_dimmer`); this rig switches that
    // off, because the tests in this crate are about the journal, the wire and
    // the session, and a supplied dimmer would put a multiplier between every
    // one of them and the byte they assert on. Where the supplied dimmer itself
    // is asserted is `prism-engine`, which owns the arithmetic, and the
    // end-to-end suite, which owns the rig an operator patches.
    let mut par = fixture(2, "generic.rgbw.par", 1, 10);
    par.software_dimmer = false;
    show.patch_fixture(par).unwrap();
    show.patch_fixture(fixture(4, "generic.dimmer.dark", 1, 5))
        .unwrap();
    show.store_group(group(1, &[1, 2])).unwrap();
    show.store_preset(preset(4, 2, AttributeType::Red, 65535))
        .unwrap();
    show.store_sequence(sequence(1, vec![cue("1", 4, AttributeType::Dimmer, 65535)]))
        .unwrap();
    show.store_executor(executor(0, Some(1))).unwrap();
    show.mark_saved();
    show
}

/// A session that is not at its defaults, so a snapshot that lost it would
/// look different from one that carried it.
#[must_use]
pub fn session() -> SessionState {
    let mut session = SessionState::new();
    session.set_executor_page(3).unwrap();
    session.set_programmer_page(2).unwrap();
    session
        .set_command_line("fixture 1 at full", false)
        .unwrap();
    session.mark_saved();
    session
}

/// The rig and the session it is operated in.
#[must_use]
pub fn show_file() -> ShowFile {
    ShowFile {
        show: show(),
        session: session(),
        ..ShowFile::new()
    }
}
