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
    ///
    /// **Seven banks since S43**, and the change is the owner's: the drawing in
    /// `design/skeleton/programmer.pdf` names seven, and what had been one
    /// `Beam` bank of six knobs is three. The split is the one a person makes
    /// standing at a desk — the gobo wheel and the prism are one thing you
    /// reach for, the size of the beam is another, and `Control` is the row of
    /// lamp-on, reset and fan that you touch once a show and never during one.
    /// Six knobs on one bank meant two pages of `Beam` and a `Control` channel
    /// filed behind the shutter.
    #[must_use]
    pub const fn feature_group(self) -> FeatureGroup {
        match self {
            Self::Dimmer => FeatureGroup::Dimmer,
            Self::Pan | Self::Tilt => FeatureGroup::Position,
            Self::Gobo | Self::Prism => FeatureGroup::Gobo,
            Self::Red | Self::Green | Self::Blue | Self::White | Self::Amber => FeatureGroup::Color,
            Self::Iris | Self::Zoom | Self::Shutter => FeatureGroup::Beam,
            Self::Focus => FeatureGroup::Focus,
            Self::Control => FeatureGroup::Control,
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

/// The seven encoder banks. Also the pool kinds presets are filed under.
///
/// # Seven since S43, and the order is the owner's drawing
///
/// It was five, with everything that was not intensity, position, colour or
/// sharpness on one `Beam` bank — six knobs, which is two pages of four and a
/// `Control` channel filed behind a shutter. `design/skeleton/programmer.pdf`
/// names seven, in two columns, and the order below is that drawing read across
/// and down: Dimmer and Position, Gobo and Color, Beam and Focus, then Control.
/// The interface draws the keys straight out of this array, so the arrangement
/// on the screen *is* this order rather than a second copy of it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
pub enum FeatureGroup {
    /// Intensity.
    #[default]
    Dimmer,
    /// Pan and tilt.
    Position,
    /// Gobo wheels and the prism — the pattern in the beam.
    Gobo,
    /// Colour mixing and colour wheels.
    Color,
    /// The size and shape of the beam: iris, zoom, shutter.
    Beam,
    /// Beam sharpness.
    Focus,
    /// Lamp control — the channel a fixture is reset and struck from.
    Control,
}

impl FeatureGroup {
    /// Every feature group, in encoder-bank order.
    pub const ALL: [Self; 7] = [
        Self::Dimmer,
        Self::Position,
        Self::Gobo,
        Self::Color,
        Self::Beam,
        Self::Focus,
        Self::Control,
    ];

    /// The attributes on this encoder bank, in [`AttributeType::ALL`]'s order.
    ///
    /// **This is the order the encoder bar shows and the order the jog wheel
    /// walks, and it has to be one order.** `prismd::surface::parameter_of`
    /// resolves *encoder bank plus parameter index* to the attribute the wheel
    /// turns; the interface's encoder bar numbers its encoders the same way. If
    /// the two disagreed, an operator would turn the wheel and watch a
    /// parameter other than the one that is highlighted change — which is a
    /// fault nobody would attribute to a table. S22 left the warning; S26 made
    /// it one table, exported to TypeScript with the rest of the bindings.
    ///
    /// Written out rather than filtered, because it is `const` and the callers
    /// want a slice. `every_bank_is_the_attributes_that_name_it` asserts it is
    /// exactly the filter, so the two cannot drift.
    ///
    /// A fixture *profile* may file an individual attribute under a different
    /// group ([`AttributeDef::feature_group`]); that is a statement about one
    /// fixture's channel and does not move the encoder. What this answers is
    /// which knobs the bank has.
    #[must_use]
    pub const fn attributes(self) -> &'static [AttributeType] {
        match self {
            Self::Dimmer => &[AttributeType::Dimmer],
            Self::Position => &[AttributeType::Pan, AttributeType::Tilt],
            Self::Color => &[
                AttributeType::Red,
                AttributeType::Green,
                AttributeType::Blue,
                AttributeType::White,
                AttributeType::Amber,
            ],
            Self::Gobo => &[AttributeType::Gobo, AttributeType::Prism],
            Self::Beam => &[
                AttributeType::Iris,
                AttributeType::Zoom,
                AttributeType::Shutter,
            ],
            Self::Focus => &[AttributeType::Focus],
            Self::Control => &[AttributeType::Control],
        }
    }

    /// The attribute at `index` on this bank, or `None` past the end.
    ///
    /// Past the end is nothing rather than the last one: a wheel turned past
    /// the parameters does nothing, which is what an operator who has paged
    /// off the end should feel.
    #[must_use]
    pub fn parameter(self, index: u32) -> Option<AttributeType> {
        self.attributes().get(index as usize).copied()
    }
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
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub physical_from: f64,
    /// Physical value at 65535.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
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

impl FixtureType {
    /// Whether this mode has a dimmer channel of its own.
    ///
    /// The question [`crate::Fixture::has_software_dimmer`] asks, and the reason
    /// it is here rather than written out at each call site.
    ///
    /// # It asks about the attribute and not about the bank
    ///
    /// *Intensity* elsewhere in this project means [`FeatureGroup::Dimmer`] —
    /// `plan.rs`'s `is_intensity` says so, and it is right about what the
    /// masters may scale. This is a different question with a different answer,
    /// and the difference is a profile that files its dimmer channel somewhere
    /// odd: a head whose intensity is on the colour bank still **has** a
    /// [`AttributeType::Dimmer`], and a desk that supplied it a second one would
    /// name the same attribute twice — which is `MergeError::DuplicateAttribute`
    /// and a rig that will not patch at all.
    ///
    /// So the test is the attribute, which is the thing that would collide, and
    /// which is also what an operator means by *it has a dimmer channel*.
    #[must_use]
    pub fn has_dimmer(&self) -> bool {
        self.attributes
            .iter()
            .any(|def| def.attribute == AttributeType::Dimmer)
    }
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
        // **S43 moved three of them.** The gobo wheel and the prism are one
        // thing an operator reaches for and the size of the beam is another, so
        // `Beam`'s six knobs are three banks now — and `Control`, which is
        // touched once a show, is no longer filed behind the shutter.
        assert_eq!(AttributeType::Gobo.feature_group(), FeatureGroup::Gobo);
        assert_eq!(AttributeType::Prism.feature_group(), FeatureGroup::Gobo);
        assert_eq!(AttributeType::Iris.feature_group(), FeatureGroup::Beam);
        assert_eq!(AttributeType::Zoom.feature_group(), FeatureGroup::Beam);
        assert_eq!(AttributeType::Shutter.feature_group(), FeatureGroup::Beam);
        assert_eq!(
            AttributeType::Control.feature_group(),
            FeatureGroup::Control
        );
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
    fn feature_groups_are_the_seven_encoder_banks() {
        // Seven since S43 — the owner's `design/skeleton/programmer.pdf`, whose
        // left-hand box names seven and is drawn straight out of `ALL`.
        assert_eq!(FeatureGroup::ALL.len(), 7);
        assert_eq!(
            serde_json::to_string(&FeatureGroup::Position).unwrap(),
            "\"Position\""
        );
        // **`Beam` fits on one page of encoders now**, which is what the split
        // was for: it had six knobs against `ENCODERS_PER_PAGE`'s four, so
        // reaching a shutter meant paging. `Color` still has five and still
        // pages — five is what an RGBWA fixture *has*, and no arrangement of
        // banks makes it four.
        assert_eq!(FeatureGroup::Beam.attributes().len(), 3);
        assert_eq!(FeatureGroup::Gobo.attributes().len(), 2);
        assert_eq!(FeatureGroup::Control.attributes().len(), 1);
        // And the split moved knobs about rather than inventing or losing any.
        let banked: usize = FeatureGroup::ALL
            .iter()
            .map(|group| group.attributes().len())
            .sum();
        assert_eq!(banked, AttributeType::ALL.len());
    }

    /// The written-out table is exactly the filter, so it cannot drift from
    /// [`AttributeType::feature_group`] — which is what a `const` slice buys
    /// speed at the cost of, and what this pays back.
    #[test]
    fn every_bank_holds_the_attributes_that_name_it_in_all_order() {
        let mut seen = Vec::new();
        for group in FeatureGroup::ALL {
            let filtered: Vec<AttributeType> = AttributeType::ALL
                .into_iter()
                .filter(|attribute| attribute.feature_group() == group)
                .collect();
            assert_eq!(group.attributes(), filtered, "{group:?}");
            assert!(!filtered.is_empty(), "{group:?} has no encoders at all");
            seen.extend(filtered);
        }
        // And between them the five banks reach every attribute exactly once:
        // an encoder bar built from these is a complete one.
        seen.sort_unstable();
        let mut all = AttributeType::ALL.to_vec();
        all.sort_unstable();
        assert_eq!(seen, all);
    }

    #[test]
    fn a_parameter_index_past_the_end_of_a_bank_names_nothing() {
        // The jog wheel's rule (`prismd::surface::parameter_of`) and the
        // encoder bar's, now the same function.
        assert_eq!(
            FeatureGroup::Position.parameter(0),
            Some(AttributeType::Pan)
        );
        assert_eq!(
            FeatureGroup::Position.parameter(1),
            Some(AttributeType::Tilt)
        );
        assert_eq!(FeatureGroup::Position.parameter(2), None);
        assert_eq!(FeatureGroup::Dimmer.parameter(u32::MAX), None);
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
            "\"Dimmer\" | \"Position\" | \"Gobo\" | \"Color\" | \"Beam\" | \"Focus\" | \"Control\""
        );
        assert_eq!(MergeMode::inline(&cfg), "\"HTP\" | \"LTP\"");
    }
}

#[cfg(any(test, feature = "proptest"))]
crate::arb::arbitrary_from_list!(AttributeType);
#[cfg(any(test, feature = "proptest"))]
crate::arb::arbitrary_from_list!(FeatureGroup);
