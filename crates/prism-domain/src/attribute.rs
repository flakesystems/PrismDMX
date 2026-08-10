//! Attributes and fixture types — `ARCHITECTURE_SPEC.md` §6.
//!
//! An attribute is the smallest addressable idea in the show ("this fixture's
//! tilt"). Everything above it — presets, cues, the programmer — is expressed in
//! attributes, and only the encoding step in `prism-engine` turns them into DMX
//! channels.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a channel controls.
///
/// This is the closed set from `ARCHITECTURE_SPEC.md` §6. It is deliberately not
/// `#[non_exhaustive]`: the TypeScript union must stay exhaustive so the UI gets
/// a compile error when a new attribute appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum AttributeType {
    /// Intensity. The one attribute that merges HTP.
    Dimmer,
    /// Horizontal movement.
    Pan,
    /// Vertical movement.
    Tilt,
    /// Red emitter.
    Red,
    /// Green emitter.
    Green,
    /// Blue emitter.
    Blue,
    /// White emitter.
    White,
    /// Amber emitter.
    Amber,
    /// Iris aperture.
    Iris,
    /// Beam angle.
    Zoom,
    /// Beam sharpness.
    Focus,
    /// Gobo wheel.
    Gobo,
    /// Prism wheel.
    Prism,
    /// Shutter and strobe.
    Shutter,
    /// Fixture control channel (reset, lamp on, ...).
    Control,
}

impl AttributeType {
    /// Every attribute type, in specification order.
    pub const ALL: [Self; 15] = [
        Self::Dimmer,
        Self::Pan,
        Self::Tilt,
        Self::Red,
        Self::Green,
        Self::Blue,
        Self::White,
        Self::Amber,
        Self::Iris,
        Self::Zoom,
        Self::Focus,
        Self::Gobo,
        Self::Prism,
        Self::Shutter,
        Self::Control,
    ];

    /// The encoder bank this attribute appears on by default.
    ///
    /// A fixture type may override it per attribute — see [`AttributeDef`].
    #[must_use]
    pub const fn feature_group(self) -> FeatureGroup {
        match self {
            Self::Dimmer => FeatureGroup::Dimmer,
            Self::Pan | Self::Tilt => FeatureGroup::Position,
            Self::Red | Self::Green | Self::Blue | Self::White | Self::Amber => FeatureGroup::Color,
            Self::Iris | Self::Zoom | Self::Gobo | Self::Prism | Self::Shutter | Self::Control => {
                FeatureGroup::Beam
            }
            Self::Focus => FeatureGroup::Focus,
        }
    }

    /// How this attribute merges between playbacks by default.
    ///
    /// `docs/DMX_MERGE.md`: intensity merges HTP, everything else merges LTP by
    /// activation order.
    #[must_use]
    pub const fn default_merge_mode(self) -> MergeMode {
        match self {
            Self::Dimmer => MergeMode::Htp,
            _ => MergeMode::Ltp,
        }
    }
}

/// The five encoder banks. Also the pool kinds presets are filed under.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum FeatureGroup {
    /// Intensity.
    #[default]
    Dimmer,
    /// Pan and tilt.
    Position,
    /// Colour mixing and colour wheels.
    Color,
    /// Gobo, prism, iris, zoom, shutter.
    Beam,
    /// Beam sharpness.
    Focus,
}

impl FeatureGroup {
    /// Every feature group, in encoder-bank order.
    pub const ALL: [Self; 5] = [
        Self::Dimmer,
        Self::Position,
        Self::Color,
        Self::Beam,
        Self::Focus,
    ];
}

/// How two playback sources combine for one attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "UPPERCASE")]
pub enum MergeMode {
    /// Highest takes precedence — the maximum of all sources.
    Htp,
    /// Latest takes precedence — the most recently activated source wins.
    Ltp,
}

/// One attribute of a fixture type: where it sits in the footprint and how it
/// behaves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct AttributeDef {
    /// What this attribute controls.
    pub attribute: AttributeType,
    /// Which encoder bank it appears on.
    pub feature_group: FeatureGroup,
    /// 0-based offset of the coarse channel within the fixture footprint.
    pub coarse_offset: u16,
    /// 0-based offset of the fine channel, or `None` for an 8-bit attribute.
    pub fine_offset: Option<u16>,
    /// Home value, `0..=65535`. Used by the bottom layer of the merge.
    pub default_value: u16,
    /// How this attribute merges between playbacks.
    pub merge_mode: MergeMode,
    /// Whether the encoded DMX value is inverted.
    pub invert: bool,
    /// Physical value at 0, e.g. `-270` degrees for pan.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub physical_from: f64,
    /// Physical value at 65535.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub physical_to: f64,
}

impl AttributeDef {
    /// Whether this attribute occupies a coarse **and** a fine channel.
    #[must_use]
    pub const fn is_sixteen_bit(&self) -> bool {
        self.fine_offset.is_some()
    }
}

/// A fixture type — the profile a patched fixture instantiates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct FixtureType {
    /// Stable key, e.g. `generic.rgbw.par`. Referenced by [`crate::Fixture`].
    pub id: String,
    /// Manufacturer name, for the patch UI.
    pub manufacturer: String,
    /// Product name.
    pub name: String,
    /// Mode name, e.g. `4ch`.
    pub mode: String,
    /// Number of DMX channels this mode occupies.
    pub footprint: u16,
    /// The attributes in this mode.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub attributes: Vec<AttributeDef>,
}

#[cfg(test)]
mod tests {
    use crate::{AttributeDef, AttributeType, FeatureGroup, FixtureType, MergeMode};
    use ts_rs::{Config, TS};

    fn tilt() -> AttributeDef {
        AttributeDef {
            attribute: AttributeType::Tilt,
            feature_group: FeatureGroup::Position,
            coarse_offset: 2,
            fine_offset: Some(3),
            default_value: 32768,
            merge_mode: MergeMode::Ltp,
            invert: false,
            physical_from: -135.0,
            physical_to: 135.0,
        }
    }

    #[test]
    fn attribute_types_are_plain_strings() {
        assert_eq!(
            serde_json::to_string(&AttributeType::Dimmer).unwrap(),
            "\"Dimmer\""
        );
        assert_eq!(
            serde_json::to_string(&AttributeType::Control).unwrap(),
            "\"Control\""
        );
    }

    #[test]
    fn all_fifteen_attribute_types_exist() {
        assert_eq!(AttributeType::ALL.len(), 15);
        assert_eq!(AttributeType::ALL[0], AttributeType::Dimmer);
        assert_eq!(AttributeType::ALL[14], AttributeType::Control);
    }

    #[test]
    fn every_attribute_type_has_a_default_feature_group() {
        assert_eq!(AttributeType::Dimmer.feature_group(), FeatureGroup::Dimmer);
        assert_eq!(AttributeType::Pan.feature_group(), FeatureGroup::Position);
        assert_eq!(AttributeType::Tilt.feature_group(), FeatureGroup::Position);
        assert_eq!(AttributeType::Red.feature_group(), FeatureGroup::Color);
        assert_eq!(AttributeType::Gobo.feature_group(), FeatureGroup::Beam);
        assert_eq!(AttributeType::Focus.feature_group(), FeatureGroup::Focus);
    }

    #[test]
    fn intensity_merges_htp_and_everything_else_ltp() {
        // docs/DMX_MERGE.md: this split is the whole merge rule in one line.
        assert_eq!(AttributeType::Dimmer.default_merge_mode(), MergeMode::Htp);
        for attribute in AttributeType::ALL {
            if attribute != AttributeType::Dimmer {
                assert_eq!(attribute.default_merge_mode(), MergeMode::Ltp);
            }
        }
    }

    #[test]
    fn merge_mode_is_upper_case_on_the_wire() {
        assert_eq!(serde_json::to_string(&MergeMode::Htp).unwrap(), "\"HTP\"");
        assert_eq!(serde_json::to_string(&MergeMode::Ltp).unwrap(), "\"LTP\"");
    }

    #[test]
    fn feature_groups_are_the_five_encoder_banks() {
        assert_eq!(FeatureGroup::ALL.len(), 5);
        assert_eq!(
            serde_json::to_string(&FeatureGroup::Position).unwrap(),
            "\"Position\""
        );
    }

    #[test]
    fn attribute_def_uses_camel_case_field_names() {
        let json = serde_json::to_value(tilt()).unwrap();
        let object = json.as_object().unwrap();
        let keys: Vec<&str> = object.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "attribute",
                "coarseOffset",
                "defaultValue",
                "featureGroup",
                "fineOffset",
                "invert",
                "mergeMode",
                "physicalFrom",
                "physicalTo",
            ]
        );
    }

    #[test]
    fn an_eight_bit_attribute_has_no_fine_offset() {
        let mut def = tilt();
        def.fine_offset = None;
        assert!(!def.is_sixteen_bit());
        assert!(tilt().is_sixteen_bit());
    }

    #[test]
    fn fixture_type_carries_its_footprint_and_attributes() {
        let fixture_type = FixtureType {
            id: "generic.rgbw.par".to_owned(),
            manufacturer: "Generic".to_owned(),
            name: "RGBW PAR".to_owned(),
            mode: "4ch".to_owned(),
            footprint: 4,
            attributes: vec![tilt()],
        };
        let json = serde_json::to_value(&fixture_type).unwrap();
        assert_eq!(json["id"], "generic.rgbw.par");
        assert_eq!(json["footprint"], 4);
        assert_eq!(json["attributes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn typescript_unions_match_the_specification() {
        let cfg = Config::new();
        assert_eq!(
            AttributeType::inline(&cfg),
            "\"Dimmer\" | \"Pan\" | \"Tilt\" | \"Red\" | \"Green\" | \"Blue\" | \"White\" \
             | \"Amber\" | \"Iris\" | \"Zoom\" | \"Focus\" | \"Gobo\" | \"Prism\" \
             | \"Shutter\" | \"Control\""
        );
        assert_eq!(
            FeatureGroup::inline(&cfg),
            "\"Dimmer\" | \"Position\" | \"Color\" | \"Beam\" | \"Focus\""
        );
        assert_eq!(MergeMode::inline(&cfg), "\"HTP\" | \"LTP\"");
    }
}
