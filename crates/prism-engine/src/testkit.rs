//! Fixtures for this crate's own tests.
//!
//! The merge is specified over a patch, so almost every test in `plan`,
//! `playback`, `encode` and `body` needs a fixture type to merge against.
//! Building one by hand in each of them would bury the assertion under nine
//! fields of scenery.

use prism_domain::{
    AttributeDef, AttributeType, Cue, CuePart, CueTrigger, Fixture, FixtureId, FixtureType,
    Sequence, SequenceId, UniverseId, Vec3,
};

/// An attribute definition with everything but the merge-relevant fields at a
/// neutral value: 8-bit, at the start of the footprint, not inverted.
pub(crate) fn attribute_def(attribute: AttributeType, home: u16) -> AttributeDef {
    attribute_at(attribute, home, 0, None)
}

/// An attribute definition with explicit channel offsets — what the encoder
/// cares about and the merge does not.
pub(crate) fn attribute_at(
    attribute: AttributeType,
    home: u16,
    coarse_offset: u16,
    fine_offset: Option<u16>,
) -> AttributeDef {
    AttributeDef {
        attribute,
        label: None,
        occurrence: 0,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset,
        default_value: home,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
        ranges: Vec::new(),
    }
}

/// A fixture type carrying exactly the attributes given, one channel each.
pub(crate) fn fixture_type(id: &str, attributes: Vec<AttributeDef>) -> FixtureType {
    let footprint = attributes.len() as u16;
    sized_fixture_type(id, footprint, attributes)
}

/// A fixture type whose footprint is stated rather than counted — a 16-bit
/// attribute occupies two channels but is one definition.
pub(crate) fn sized_fixture_type(
    id: &str,
    footprint: u16,
    attributes: Vec<AttributeDef>,
) -> FixtureType {
    FixtureType {
        id: id.to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Test".to_owned(),
        mode: "test".to_owned(),
        footprint,
        attributes,
    }
}

/// A moving head with a dimmer that homes dark and a pan that homes centred —
/// the fixture from the worked example in `docs/DMX_MERGE.md` §7. Two 8-bit
/// channels: dimmer at offset 0, pan at offset 1.
pub(crate) fn moving_head() -> FixtureType {
    fixture_type(
        "test.movinghead",
        vec![
            attribute_at(AttributeType::Dimmer, 0, 0, None),
            attribute_at(AttributeType::Pan, 32_768, 1, None),
        ],
    )
}

/// The same head patched 16-bit: dimmer on footprint channels 1-2, pan on 3-4.
pub(crate) fn moving_head_16() -> FixtureType {
    sized_fixture_type(
        "test.movinghead16",
        4,
        vec![
            attribute_at(AttributeType::Dimmer, 0, 0, Some(1)),
            attribute_at(AttributeType::Pan, 32_768, 2, Some(3)),
        ],
    )
}

/// One value of one attribute of one fixture, as a cue holds it.
pub(crate) fn cue_part(fixture: u32, attribute: AttributeType, value: u16) -> CuePart {
    CuePart {
        fixture: FixtureId::new(fixture),
        attribute,
        occurrence: 0,
        value,
        preset_ref: None,
        tracking: prism_domain::CueTracking::Track,
    }
}

/// A cue with the same time for fade in and fade out, no delay, waiting for a Go.
pub(crate) fn cue(number: &str, fade: f64, parts: Vec<CuePart>) -> Cue {
    Cue {
        number: number.to_owned(),
        name: format!("Cue {number}"),
        fade_in: fade,
        fade_out: fade,
        delay: 0.0,
        trigger: CueTrigger::Go,
        trigger_time: None,
        parts,
    }
}

/// A cue list, numbered 1.
pub(crate) fn sequence(cues: Vec<Cue>, looping: bool) -> Sequence {
    Sequence {
        id: SequenceId::new(1),
        name: "Test".to_owned(),
        color: None,
        cues,
        looping,
        master_level: u16::MAX,
        speed: prism_domain::SPEED_UNITY,
        is_active: false,
        current_cue_index: None,
    }
}

/// A patched fixture at an address, with no geometry and no inverts.
pub(crate) fn fixture(id: u32, type_id: &str, universe: u32, address: u16) -> Fixture {
    Fixture {
        // **Off, and asked for explicitly by the tests that are about it.** The
        // desk-supplied intensity adds a merge slot (S43), and a slot changes
        // every index after it — so a helper that switched it on would quietly
        // make every test about addressing a test about two features.
        software_dimmer: false,
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
