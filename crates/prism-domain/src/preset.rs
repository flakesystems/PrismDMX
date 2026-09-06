//! Presets — named attribute values, grouped into pools by feature group.
//!
//! A preset is referenced rather than copied wherever possible: a cue part that
//! carries a `presetRef` follows later edits of the preset, which is what makes
//! "change the blue everywhere" a one-touch operation on a real console.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AttributeKey, AttributeType, FeatureGroup, FixtureId, PresetId, RgbColor};

/// One stored attribute value inside a preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct PresetValue {
    /// The fixture this value applies to.
    pub fixture: FixtureId,
    /// The attribute being set.
    pub attribute: AttributeType,
    /// Which channel of that kind — **S52**, counted from nought.
    ///
    /// A fixture may have two of a parameter (a head with two colour wheels),
    /// and this is which one. Absent means the first, so a `.prism` file
    /// written before S52 reads back with every value where it always was.
    #[serde(default, skip_serializing_if = "AttributeKey::occurrence_is_first")]
    pub occurrence: u8,
    /// The value, `0..=65535`.
    pub value: u16,
}

impl PresetValue {
    /// The key this value is filed under — attribute and occurrence, **S52**.
    #[must_use]
    pub const fn key(&self) -> AttributeKey {
        AttributeKey::new(self.attribute, self.occurrence)
    }
}

/// Which pool a preset is filed in — the seven feature groups, and *Multi*.
///
/// # Why this is not [`FeatureGroup`], and not a wrapper round it either
///
/// A preset pool is *nearly* a feature group and, until S43, was one. The
/// owner's addition is a **Multi** pool: a preset that reaches across the
/// categories, so that one number recalls a whole look — colour, position and
/// beam together — rather than the colour of one. There is nothing on an encoder
/// bank that could be *Multi*, so putting it on [`FeatureGroup`] would have put
/// an eighth bank on the programmer that no wheel can reach and no attribute
/// belongs to.
///
/// The two matches below are what keeps the seven in step. [`Self::group`] is
/// exhaustive over this enum and [`From<FeatureGroup>`] is exhaustive over that
/// one, so a bank added to either fails to compile here rather than silently
/// becoming a pool nothing files into. `preset_pool_is_the_banks_and_multi`
/// asserts the round trip over `FeatureGroup::ALL`.
///
/// On the wire it is a plain string, exactly as the feature group is, so a show
/// file says `"pool": "Multi"` and a reader does not have to know this type
/// exists to see what it means.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum PresetPool {
    /// Intensity.
    #[default]
    Dimmer,
    /// Pan and tilt.
    Position,
    /// Gobo wheels and the prism.
    Gobo,
    /// Colour mixing and colour wheels.
    Color,
    /// The size and shape of the beam.
    Beam,
    /// Beam sharpness.
    Focus,
    /// Lamp control.
    Control,
    /// **Across the categories.** Every value the programmer holds goes in, and
    /// applying one recalls the lot — which is the pool an operator files a
    /// finished look in rather than one ingredient of it.
    Multi,
}

impl PresetPool {
    /// Every pool, in the order the window draws its tabs.
    pub const ALL: [Self; 8] = [
        Self::Dimmer,
        Self::Position,
        Self::Gobo,
        Self::Color,
        Self::Beam,
        Self::Focus,
        Self::Control,
        Self::Multi,
    ];

    /// Which feature group's values this pool takes, or `None` for every one.
    ///
    /// `None` is not *nothing*: it is the filter
    /// `prism_core::Programmer::touched` already takes for a cue, where no pool
    /// is named and every touched value is a candidate. So **Multi needed no
    /// new filtering** — it is the absence of the filter the other seven apply,
    /// which is why this answers an `Option` rather than a group of its own.
    #[must_use]
    pub const fn group(self) -> Option<FeatureGroup> {
        match self {
            Self::Dimmer => Some(FeatureGroup::Dimmer),
            Self::Position => Some(FeatureGroup::Position),
            Self::Gobo => Some(FeatureGroup::Gobo),
            Self::Color => Some(FeatureGroup::Color),
            Self::Beam => Some(FeatureGroup::Beam),
            Self::Focus => Some(FeatureGroup::Focus),
            Self::Control => Some(FeatureGroup::Control),
            Self::Multi => None,
        }
    }
}

impl From<FeatureGroup> for PresetPool {
    fn from(group: FeatureGroup) -> Self {
        match group {
            FeatureGroup::Dimmer => Self::Dimmer,
            FeatureGroup::Position => Self::Position,
            FeatureGroup::Gobo => Self::Gobo,
            FeatureGroup::Color => Self::Color,
            FeatureGroup::Beam => Self::Beam,
            FeatureGroup::Focus => Self::Focus,
            FeatureGroup::Control => Self::Control,
        }
    }
}

/// A named set of attribute values, filed in one of the pools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// Preset number within its pool.
    pub id: PresetId,
    /// Which pool this preset lives in.
    pub pool: PresetPool,
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
    use crate::{
        AttributeType, FeatureGroup, FixtureId, Preset, PresetId, PresetPool, PresetValue, RgbColor,
    };

    fn preset() -> Preset {
        Preset {
            id: PresetId::new(4),
            pool: PresetPool::Color,
            name: "Deep blue".to_owned(),
            color: Some(RgbColor { r: 0, g: 0, b: 255 }),
            values: vec![PresetValue {
                fixture: FixtureId::new(1),
                attribute: AttributeType::Blue,
                occurrence: 0,
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
            occurrence: 0,
            value: u16::MAX,
        };
        assert_eq!(value.value, 65535);
    }

    /// **The seven pools are the seven banks, and the eighth is Multi.**
    ///
    /// Both directions, because both matches are what stops the two lists
    /// drifting: a feature group added to the programmer must become a pool,
    /// and a pool must either name a group or be the one that names none.
    #[test]
    fn preset_pool_is_the_banks_and_multi() {
        for group in FeatureGroup::ALL {
            assert_eq!(PresetPool::from(group).group(), Some(group));
        }
        assert_eq!(PresetPool::ALL.len(), FeatureGroup::ALL.len() + 1);
        let without = PresetPool::ALL
            .iter()
            .filter(|pool| pool.group().is_none())
            .count();
        assert_eq!(without, 1, "exactly one pool takes every value");
        assert_eq!(PresetPool::Multi.group(), None);
    }

    /// The wire spelling is a plain string, the same shape a feature group has.
    #[test]
    fn a_pool_travels_as_its_own_name() {
        assert_eq!(
            serde_json::to_string(&PresetPool::Multi).unwrap(),
            r#""Multi""#
        );
        assert_eq!(
            serde_json::to_string(&PresetPool::Color).unwrap(),
            r#""Color""#
        );
    }
}
