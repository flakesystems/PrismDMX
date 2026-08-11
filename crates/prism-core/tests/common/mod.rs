//! Shared scenery for the integration targets.
//!
//! Each target compiles its own copy of this module and uses a subset of it,
//! which is what the allow below is for: a helper that only
//! `delta_round_trip.rs` needs is dead code in `show_to_engine.rs` and would
//! otherwise fail the `-D warnings` build.
#![allow(dead_code)]

use prism_core::Show;
use prism_domain::{
    AttributeDef, AttributeType, Command, Cue, CuePart, CueTrigger, Executor,
    ExecutorEncoderFunction, ExecutorFaderFunction, ExecutorId, Fixture, FixtureId, FixtureType,
    Group, GroupId, Preset, PresetId, PresetValue, Sequence, SequenceId, UniverseId, Vec3,
};

/// An 8-bit attribute at a given offset, with everything else neutral.
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

/// A four-channel RGBW PAR.
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

/// A one-channel dimmer whose home value is given, so two of them at one
/// address can be told apart on the wire.
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

/// A patched fixture at home geometry.
pub fn fixture(id: u32, type_id: &str, universe: u32, address: u16) -> Fixture {
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

/// A cue with one part and no preset link.
pub fn cue(number: &str, fixture: u32, attribute: AttributeType, value: u16) -> Cue {
    Cue {
        number: number.to_owned(),
        name: format!("Cue {number}"),
        fade_in: 3.0,
        fade_out: 3.0,
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
pub fn sequence(id: u32, cues: Vec<Cue>) -> Sequence {
    Sequence {
        id: SequenceId::new(id),
        name: format!("Sequence {id}"),
        cues,
        looping: false,
    }
}

/// A group of the fixtures given.
pub fn group(id: u32, fixtures: &[u32]) -> Group {
    Group {
        id: GroupId::new(id),
        name: format!("Group {id}"),
        fixtures: fixtures.iter().copied().map(FixtureId::new).collect(),
    }
}

/// A preset holding one value.
pub fn preset(id: u32, fixture: u32, attribute: AttributeType, value: u16) -> Preset {
    Preset {
        id: PresetId::new(id),
        pool: attribute.feature_group(),
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
pub fn executor(id: u32, sequence_id: Option<u32>) -> Executor {
    Executor {
        id: ExecutorId::new(id),
        sequence_id: sequence_id.map(SequenceId::new),
        fader_function: ExecutorFaderFunction::Master,
        button_functions: Vec::new(),
        encoder_function: ExecutorEncoderFunction::Empty,
        master_level: 65535,
        is_active: false,
        current_cue_index: None,
    }
}

/// The `PatchFixture` command for one PAR.
pub fn patch_command(id: u32, universe: u32, address: u16) -> Command {
    Command::PatchFixture {
        id: FixtureId::new(id),
        name: format!("Fixture {id}"),
        type_id: "generic.rgbw.par".to_owned(),
        universe: UniverseId::new(universe),
        address,
    }
}

/// A show with one of everything, saved — so a test starts from a clean dirty
/// flag and every kind of reference resolves.
pub fn populated_show() -> Show {
    let mut show = Show::new();
    show.embed_fixture_type(par_type()).unwrap();
    show.embed_fixture_type(dimmer_type("generic.dimmer", 0))
        .unwrap();
    for id in 1..=3 {
        show.patch_fixture(fixture(id, "generic.rgbw.par", 1, (id as u16 - 1) * 4 + 1))
            .unwrap();
    }
    show.patch_fixture(fixture(4, "generic.dimmer", 2, 1))
        .unwrap();
    show.store_group(group(1, &[1, 2, 3])).unwrap();
    show.store_preset(preset(4, 1, AttributeType::Red, 65535))
        .unwrap();
    show.store_sequence(sequence(
        1,
        vec![
            cue("1", 1, AttributeType::Red, 65535),
            cue("2", 2, AttributeType::Green, 32768),
        ],
    ))
    .unwrap();
    show.store_executor(executor(0, Some(1))).unwrap();
    show.store_executor(executor(1, None)).unwrap();
    show.mark_saved();
    show
}
