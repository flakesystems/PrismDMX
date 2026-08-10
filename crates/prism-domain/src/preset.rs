//! Presets — named attribute values, grouped into pools by feature group.
//!
//! A preset is referenced rather than copied wherever possible: a cue part that
//! carries a `presetRef` follows later edits of the preset, which is what makes
//! "change the blue everywhere" a one-touch operation on a real console.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AttributeType, FeatureGroup, FixtureId, PresetId, RgbColor};

/// One stored attribute value inside a preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct PresetValue {
    /// The fixture this value applies to.
    pub fixture: FixtureId,
    /// The attribute being set.
    pub attribute: AttributeType,
    /// The value, `0..=65535`.
    pub value: u16,
}

/// A named set of attribute values, filed in the pool of its feature group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// Preset number within its pool.
    pub id: PresetId,
    /// Which pool this preset lives in.
    pub pool: FeatureGroup,
    /// Operator-facing name.
    pub name: String,
    /// Colour shown on the X-Touch scribble strip, if one was chosen.
    pub color: Option<RgbColor>,
    /// The stored values.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub values: Vec<PresetValue>,
}

#[cfg(test)]
mod tests {
    use crate::{AttributeType, FeatureGroup, FixtureId, Preset, PresetId, PresetValue, RgbColor};

    fn preset() -> Preset {
        Preset {
            id: PresetId::new(4),
            pool: FeatureGroup::Color,
            name: "Deep blue".to_owned(),
            color: Some(RgbColor { r: 0, g: 0, b: 255 }),
            values: vec![PresetValue {
                fixture: FixtureId::new(1),
                attribute: AttributeType::Blue,
                value: 65535,
            }],
        }
    }

    #[test]
    fn preset_matches_the_wire_shape() {
        assert_eq!(
            serde_json::to_string(&preset()).unwrap(),
            r#"{"id":4,"pool":"Color","name":"Deep blue","color":{"r":0,"g":0,"b":255},"values":[{"fixture":1,"attribute":"Blue","value":65535}]}"#
        );
    }

    #[test]
    fn a_preset_without_a_scribble_strip_colour_is_null() {
        let mut preset = preset();
        preset.color = None;
        let json = serde_json::to_value(&preset).unwrap();
        assert!(json["color"].is_null());
    }

    #[test]
    fn a_preset_value_is_sixteen_bit() {
        let value = PresetValue {
            fixture: FixtureId::new(1),
            attribute: AttributeType::Dimmer,
            value: u16::MAX,
        };
        assert_eq!(value.value, 65535);
    }
}
