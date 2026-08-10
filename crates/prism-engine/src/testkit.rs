//! Fixtures for this crate's own tests.
//!
//! The merge is specified over a patch, so almost every test in `plan`,
//! `playback` and `body` needs a fixture type to merge against. Building one by
//! hand in each of them would bury the assertion under nine fields of scenery.

use prism_domain::{AttributeDef, AttributeType, FixtureType};

/// An attribute definition with everything but the merge-relevant fields at a
/// neutral value. `coarse_offset` and the physical range belong to S4's
/// encoding and mean nothing here.
pub(crate) fn attribute_def(attribute: AttributeType, home: u16) -> AttributeDef {
    AttributeDef {
        attribute,
        feature_group: attribute.feature_group(),
        coarse_offset: 0,
        fine_offset: None,
        default_value: home,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

/// A fixture type carrying exactly the attributes given.
pub(crate) fn fixture_type(id: &str, attributes: Vec<AttributeDef>) -> FixtureType {
    FixtureType {
        id: id.to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Test".to_owned(),
        mode: "test".to_owned(),
        footprint: attributes.len() as u16,
        attributes,
    }
}

/// A moving head with a dimmer that homes dark and a pan that homes centred —
/// the fixture from the worked example in `docs/DMX_MERGE.md` §7.
pub(crate) fn moving_head() -> FixtureType {
    fixture_type(
        "test.movinghead",
        vec![
            attribute_def(AttributeType::Dimmer, 0),
            attribute_def(AttributeType::Pan, 32_768),
        ],
    )
}
