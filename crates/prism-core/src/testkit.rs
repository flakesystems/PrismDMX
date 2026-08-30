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
            tracking: prism_domain::CueTracking::Track,
        }],
    }
}

/// A sequence carrying the cues given.
pub(crate) fn sequence(id: u32, cues: Vec<Cue>) -> Sequence {
    Sequence {
        id: SequenceId::new(id),
        name: format!("Sequence {id}"),
        color: None,
        cues,
        looping: false,
        master_level: u16::MAX,
        speed: prism_domain::SPEED_UNITY,
        is_active: false,
        current_cue_index: None,
    }
}

/// A preset holding one value.
pub(crate) fn preset(id: u32, fixture: u32, attribute: AttributeType, value: u16) -> Preset {
    Preset {
        id: prism_domain::PresetId::new(id),
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

/// The show `tests/fixtures/version-1.prism` was written from.
///
/// It has something in every one of the six tables version 1 defines, and none
/// of it is at a default value — a migration checked against an empty show
/// cannot tell a table that was carried across from one that was dropped. The
/// numbers here are the ones `tests/persistence.rs` asserts after the
/// migration, and they are frozen for the same reason the file is: see
/// `store::tests::rewrites_the_version_one_fixture`.
pub(crate) fn migration_fixture() -> crate::ShowFile {
    let mut file = crate::ShowFile::new();
    file.show.embed_fixture_type(par_type()).unwrap();
    file.show.embed_fixture_type(dimmer_type()).unwrap();
    file.show
        .embed_fixture_type(FixtureType {
            id: "generic.head".to_owned(),
            manufacturer: "Aula".to_owned(),
            name: "Wash Head".to_owned(),
            mode: "6ch".to_owned(),
            footprint: 6,
            attributes: vec![
                AttributeDef {
                    attribute: AttributeType::Pan,
                    feature_group: prism_domain::FeatureGroup::Position,
                    coarse_offset: 0,
                    fine_offset: Some(1),
                    default_value: 32768,
                    merge_mode: prism_domain::MergeMode::Ltp,
                    invert: true,
                    physical_from: -270.0,
                    physical_to: 270.0,
                },
                attribute(AttributeType::Tilt, 2),
                attribute(AttributeType::Dimmer, 3),
            ],
        })
        .unwrap();
    for (id, type_id, universe, address, x) in [
        (1u32, "generic.head", 1u32, 1u16, 1.5f64),
        (2, "generic.head", 1, 7, -3.75),
        (3, "generic.rgbw.par", 2, 21, 0.125),
        (4, "generic.dimmer", 64, 512, 8.0),
    ] {
        let mut hung = fixture(id, type_id, universe, address);
        hung.position = Vec3 {
            x,
            y: 4.5,
            z: -2.25,
        };
        hung.rotation = Vec3 {
            x: 0.0,
            y: 180.0,
            z: 90.5,
        };
        hung.invert_pan = id % 2 == 0;
        file.show.patch_fixture(hung).unwrap();
    }
    file.show
        .store_group(prism_domain::Group {
            id: prism_domain::GroupId::new(7),
            name: "Front Wash".to_owned(),
            fixtures: vec![FixtureId::new(1), FixtureId::new(3)],
        })
        .unwrap();
    file.show
        .store_preset(preset(12, 3, AttributeType::Red, 65535))
        .unwrap();
    file.show
        .store_sequence(sequence(
            5,
            vec![
                cue("1", 3, AttributeType::Red, 65535),
                cue("2.5", 1, AttributeType::Pan, 12345),
            ],
        ))
        .unwrap();
    file.show.store_executor(executor(17, Some(5))).unwrap();
    // The level is the cue list's since S45, and not its default: a saved and
    // reloaded show has to carry the number an operator set.
    //
    // **What is running is deliberately not in here.** A version-1 file wrote
    // `isActive` and `currentCueIndex` onto its executor; S45 moved both onto
    // the cue list, and neither is migrated — a show reopens with nothing
    // running, which is what `Sequence::is_active` has said since S40 and what
    // makes `false` its default rather than a migration.
    file.show
        .set_sequence_master(SequenceId::new(5), 40000)
        .unwrap();
    file
}

/// An executor, optionally playing a sequence.
pub(crate) fn executor(id: u32, sequence_id: Option<u32>) -> Executor {
    Executor {
        id: ExecutorId::new(id),
        sequence_id: sequence_id.map(SequenceId::new),
        fader_function: ExecutorFaderFunction::Master,
        button_functions: Vec::new(),
        encoder_function: ExecutorEncoderFunction::Empty,
    }
}
