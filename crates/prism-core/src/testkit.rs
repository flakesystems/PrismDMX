//! Fixtures for this crate's own unit tests.
//!
//! A show needs a profile before it can hold a fixture, and a fixture is nine
//! fields of which two matter to any given assertion. Building them here keeps
//! the assertion visible.
//!
//! The integration targets carry their own copy in `tests/common/mod.rs`: a
//! `#[cfg(test)]` module is not compiled into the library an integration test
//! links against.

use prism_domain::{
    AttributeDef, AttributeType, Cue, CuePart, CueTrigger, Executor, ExecutorEncoderFunction,
    ExecutorFaderFunction, ExecutorId, Fixture, FixtureId, FixtureType, Preset, PresetValue,
    Sequence, SequenceId, UniverseId, Vec3,
};

/// An 8-bit attribute at a given offset, with everything else neutral.
pub(crate) fn attribute(attribute: AttributeType, coarse_offset: u16) -> AttributeDef {
    AttributeDef {
        attribute,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value: 0,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

/// A four-channel RGBW PAR: the ordinary case.
pub(crate) fn par_type() -> FixtureType {
    FixtureType {
        id: "generic.rgbw.par".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "RGBW PAR".to_owned(),
        mode: "4ch".to_owned(),
        footprint: 4,
        attributes: vec![
            attribute(AttributeType::Red, 0),
            attribute(AttributeType::Green, 1),
            attribute(AttributeType::Blue, 2),
            attribute(AttributeType::White, 3),
        ],
    }
}

/// A one-channel dimmer: the smallest profile that controls anything.
pub(crate) fn dimmer_type() -> FixtureType {
    FixtureType {
        id: "generic.dimmer".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "1ch".to_owned(),
        footprint: 1,
        attributes: vec![attribute(AttributeType::Dimmer, 0)],
    }
}

/// A patched fixture at home geometry.
pub(crate) fn fixture(id: u32, type_id: &str, universe: u32, address: u16) -> Fixture {
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

/// A cue with one part and no times.
pub(crate) fn cue(number: &str, fixture: u32, attribute: AttributeType, value: u16) -> Cue {
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
pub(crate) fn sequence(id: u32, cues: Vec<Cue>) -> Sequence {
    Sequence {
        id: SequenceId::new(id),
        name: format!("Sequence {id}"),
        cues,
        looping: false,
    }
}

/// A preset holding one value.
pub(crate) fn preset(id: u32, fixture: u32, attribute: AttributeType, value: u16) -> Preset {
    Preset {
        id: prism_domain::PresetId::new(id),
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
pub(crate) fn executor(id: u32, sequence_id: Option<u32>) -> Executor {
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
