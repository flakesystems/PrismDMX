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
    // -- S51, punch-list B38: the nineteen that made the corpus whole --------
    //
    // Everything below was added because the Open Fixture Library has a
    // capability type for it and this model had nowhere to put it, so the
    // channel was dropped and the operator could not reach it. See
    // `crate::AttributeType::ALL` for the counting and
    // `prism_core::library::ofl` for the table that maps them.
    /// Subtractive cyan — a CMY colour-mixing flag.
    Cyan,
    /// Subtractive magenta.
    Magenta,
    /// Subtractive yellow.
    Yellow,
    /// Ultraviolet emitter.
    Uv,
    /// Lime emitter.
    Lime,
    /// Indigo emitter.
    Indigo,
    /// A colour wheel, or a channel of named colour presets.
    ColorWheel,
    /// Correlated colour temperature — warm to cold on one channel.
    ColorTemperature,
    /// How fast the head moves between positions.
    PositionSpeed,
    /// Rotation of the gobo in the gate.
    GoboRotation,
    /// Rotation of the prism.
    PrismRotation,
    /// A built-in effect or macro.
    Effect,
    /// How fast that effect runs.
    EffectSpeed,
    /// Frost, or a diffusion flag.
    Frost,
    /// A framing shutter or blade.
    Blade,
    /// Where the beam sits within the fixture.
    BeamPosition,
    /// A fog or haze machine's output.
    Fog,
    /// A speed or time channel that names no particular effect.
    Speed,
    /// Sound-to-light sensitivity.
    Sound,
}

impl AttributeType {
    /// Every attribute type, in specification order.
    ///
    /// # Thirty-four since S51, and the order is a promise — B38
    ///
    /// It was fifteen, and fifteen is what made punch-list entry **B38** true:
    /// five thousand channels of the installed library mapped to nothing at all
    /// and were dropped, taking CMY colour mixing, every colour wheel, every
    /// effect, frost, fog and the framing shutters with them. The nineteen
    /// added here are one per thing the Open Fixture Library actually has a
    /// capability type for, and with them **no channel in the 634-fixture
    /// corpus is left without an attribute** — asserted over the corpus in
    /// `crates/prism-core/tests/fixture_library.rs`, because a claim about a
    /// library can only be refuted by the library.
    ///
    /// **The first fifteen keep their places, and that is not sentiment.**
    /// [`FeatureGroup::attributes`] is this array filtered, and the encoder bar
    /// draws four at a time out of that — so appending rather than interleaving
    /// is what keeps *Red, Green, Blue, White* the first page of the colour
    /// bank on a desk that now knows about indigo. A venue that never patches a
    /// CMY head never pages past the four knobs it had.
    pub const ALL: [Self; 34] = [
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
        Self::Cyan,
        Self::Magenta,
        Self::Yellow,
        Self::Uv,
        Self::Lime,
        Self::Indigo,
        Self::ColorWheel,
        Self::ColorTemperature,
        Self::PositionSpeed,
        Self::GoboRotation,
        Self::PrismRotation,
        Self::Effect,
        Self::EffectSpeed,
        Self::Frost,
        Self::Blade,
        Self::BeamPosition,
        Self::Fog,
        Self::Speed,
        Self::Sound,
    ];

    /// Whether this attribute is an **additive emitter** — a lamp that makes
    /// more light the higher it is driven.
    ///
    /// The question punch-list **B1** answers with *a colour rests open*, and
    /// S51 is where it stopped being the same question as *is this on the
    /// colour bank*. Cyan, magenta and yellow are **subtractive**: they are
    /// filters, so open is nought and full is opaque — a CMY head parked at
    /// full on all three is not white, it is black, and giving them B1's
    /// resting value would have blacked out every CMY rig the moment this model
    /// learned to see them. A colour **wheel** is neither: its value is a slot
    /// number and there is no *open* to rest at.
    ///
    /// So the rule that reaches a profile is this one and not the bank.
    #[must_use]
    pub const fn is_additive_emitter(self) -> bool {
        matches!(
            self,
            Self::Red
                | Self::Green
                | Self::Blue
                | Self::White
                | Self::Amber
                | Self::Uv
                | Self::Lime
                | Self::Indigo
        )
    }

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
    /// **Still seven banks after S51's nineteen** — B38. The owner's drawing
    /// (`design/skeleton/programmer.pdf`) names seven and the programmer band
    /// is built out of them, so the new attributes were filed among the seven
    /// rather than given an eighth: a bank is *what an operator reaches for*,
    /// and a built-in effect is reached for at the same moment as the gobo it
    /// replaces.
    #[must_use]
    pub const fn feature_group(self) -> FeatureGroup {
        match self {
            Self::Dimmer => FeatureGroup::Dimmer,
            Self::Pan | Self::Tilt | Self::PositionSpeed => FeatureGroup::Position,
            // The pattern in the beam, and how fast it moves.
            Self::Gobo
            | Self::Prism
            | Self::GoboRotation
            | Self::PrismRotation
            | Self::Effect
            | Self::EffectSpeed => FeatureGroup::Gobo,
            Self::Red
            | Self::Green
            | Self::Blue
            | Self::White
            | Self::Amber
            | Self::Cyan
            | Self::Magenta
            | Self::Yellow
            | Self::Uv
            | Self::Lime
            | Self::Indigo
            | Self::ColorWheel
            | Self::ColorTemperature => FeatureGroup::Color,
            // The size and the shape of the beam.
            Self::Iris
            | Self::Zoom
            | Self::Shutter
            | Self::Frost
            | Self::Blade
            | Self::BeamPosition => FeatureGroup::Beam,
            Self::Focus => FeatureGroup::Focus,
            // The row that is touched once a show: lamp, reset, fan — and the
            // machine channels that belong to the same kind of moment.
            Self::Control | Self::Fog | Self::Speed | Self::Sound => FeatureGroup::Control,
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
            Self::Position => &[
                AttributeType::Pan,
                AttributeType::Tilt,
                AttributeType::PositionSpeed,
            ],
            Self::Color => &[
                AttributeType::Red,
                AttributeType::Green,
                AttributeType::Blue,
                AttributeType::White,
                AttributeType::Amber,
                AttributeType::Cyan,
                AttributeType::Magenta,
                AttributeType::Yellow,
                AttributeType::Uv,
                AttributeType::Lime,
                AttributeType::Indigo,
                AttributeType::ColorWheel,
                AttributeType::ColorTemperature,
            ],
            Self::Gobo => &[
                AttributeType::Gobo,
                AttributeType::Prism,
                AttributeType::GoboRotation,
                AttributeType::PrismRotation,
                AttributeType::Effect,
                AttributeType::EffectSpeed,
            ],
            Self::Beam => &[
                AttributeType::Iris,
                AttributeType::Zoom,
                AttributeType::Shutter,
                AttributeType::Frost,
                AttributeType::Blade,
                AttributeType::BeamPosition,
            ],
            Self::Focus => &[AttributeType::Focus],
            Self::Control => &[
                AttributeType::Control,
                AttributeType::Fog,
                AttributeType::Speed,
                AttributeType::Sound,
            ],
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

/// One **named range** of a channel — an Open Fixture Library *capability*,
/// read at last in S51 (punch-list **B38**).
///
/// # What a range is for, and where this session drew the line
///
/// An OFL channel is a list of capabilities: *0–7 closed, 8–134 dimmer,
/// 135–239 strobe*, or *0–9 open, 10–19 gobo 1, 20–29 gobo 2*. Until S51 none
/// of it was read, so a gobo wheel was a number between nought and full and an
/// operator had to know that gobo 3 lives at 27 %.
///
/// **The line this session drew** is at the *reading*: a range has a name and
/// two ends, the profile carries them, and the interface says which range an
/// encoder is standing in and offers the list to pick from. What it is
/// deliberately **not** is a change to what a value *is*: a gobo is still one
/// number in `0..=65535`, a cue still stores that number, and nothing in the
/// engine or the merge knows a range exists. The alternative — a value that is
/// *a slot* rather than a number — would reach `CuePart`, the programmer, the
/// wire and the tick, and it is a different session.
///
/// So a range is a **label on a number**, and picking one writes the middle of
/// it. That is enough for the fault B38 reports and it costs the model nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct AttributeRange {
    /// What the range is called, in the words the manufacturer used — *Gobo 3*,
    /// *Strobe slow*, *Open*.
    pub name: String,
    /// The lowest value in the range, `0..=65535`.
    pub from: u16,
    /// The highest value in the range, `0..=65535`.
    pub to: u16,
}

impl AttributeRange {
    /// Whether `value` falls in this range.
    #[must_use]
    pub const fn holds(&self, value: u16) -> bool {
        self.from <= value && value <= self.to
    }

    /// The value picking this range writes: **the middle of it**.
    ///
    /// The middle rather than the bottom, because a range's ends are where it
    /// meets its neighbours and a fixture whose thresholds are a step out from
    /// what its manual says would land on the wrong slot. The middle is the
    /// furthest any single value can be from both edges.
    #[must_use]
    pub const fn middle(&self) -> u16 {
        let (low, high) = if self.from <= self.to {
            (self.from, self.to)
        } else {
            (self.to, self.from)
        };
        low.wrapping_add((high - low) / 2)
    }
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
    /// The channel's named ranges, lowest first — **S51, B38**.
    ///
    /// Empty for a channel that has none, which is every continuous parameter
    /// and every profile written before this existed. `#[serde(default)]`, so a
    /// `.prism` file that embedded its profiles before S51 (S11: a show embeds)
    /// opens unchanged and simply offers no ranges — the fixture works exactly
    /// as it did, which is the right answer for a show somebody is about to run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub ranges: Vec<AttributeRange>,
}

impl AttributeDef {
    /// Whether this attribute occupies a coarse **and** a fine channel.
    #[must_use]
    pub const fn is_sixteen_bit(&self) -> bool {
        self.fine_offset.is_some()
    }

    /// The named range `value` is standing in, if the channel has one — B38.
    ///
    /// The **first** that holds it. OFL ranges do not overlap, and a
    /// hand-written profile whose ranges do gets the lower one rather than an
    /// argument: an encoder that named two things at once would be worse than
    /// one that named the wrong one.
    #[must_use]
    pub fn range_at(&self, value: u16) -> Option<&AttributeRange> {
        self.ranges.iter().find(|range| range.holds(value))
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
            ranges: Vec::new(),
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

    /// **Thirty-four since S51, and the first fifteen have not moved** — B38.
    ///
    /// The count is not the interesting half. The *order* is: the encoder bar
    /// pages a bank four at a time out of `FeatureGroup::attributes`, which is
    /// this array filtered, so a nineteen appended keeps `Red, Green, Blue,
    /// White` the first page of the colour bank and an interleaved nineteen
    /// would not. A venue that never patches a CMY head must not have to page
    /// past indigo to find blue.
    #[test]
    fn the_thirty_four_attribute_types_exist_and_the_first_fifteen_are_where_they_were() {
        assert_eq!(AttributeType::ALL.len(), 34);
        assert_eq!(
            &AttributeType::ALL[..15],
            &[
                AttributeType::Dimmer,
                AttributeType::Pan,
                AttributeType::Tilt,
                AttributeType::Red,
                AttributeType::Green,
                AttributeType::Blue,
                AttributeType::White,
                AttributeType::Amber,
                AttributeType::Iris,
                AttributeType::Zoom,
                AttributeType::Focus,
                AttributeType::Gobo,
                AttributeType::Prism,
                AttributeType::Shutter,
                AttributeType::Control,
            ]
        );
        // Every one of them exactly once, which is what stops a copy-and-paste
        // in a thirty-four-line array from going unnoticed.
        let mut sorted = AttributeType::ALL;
        sorted.sort_unstable();
        let mut unique = sorted.to_vec();
        unique.dedup();
        assert_eq!(unique.len(), 34);
    }

    /// **The first page of every bank is what it was** — the promise above,
    /// said in the terms the operator meets it in.
    #[test]
    fn the_first_four_of_every_bank_are_the_ones_that_were_there_before() {
        assert_eq!(
            &FeatureGroup::Color.attributes()[..4],
            &[
                AttributeType::Red,
                AttributeType::Green,
                AttributeType::Blue,
                AttributeType::White,
            ]
        );
        assert_eq!(
            &FeatureGroup::Beam.attributes()[..3],
            &[
                AttributeType::Iris,
                AttributeType::Zoom,
                AttributeType::Shutter,
            ]
        );
        assert_eq!(
            &FeatureGroup::Position.attributes()[..2],
            &[AttributeType::Pan, AttributeType::Tilt]
        );
        assert_eq!(
            &FeatureGroup::Gobo.attributes()[..2],
            &[AttributeType::Gobo, AttributeType::Prism]
        );
        assert_eq!(
            FeatureGroup::Control.attributes()[0],
            AttributeType::Control
        );
    }

    /// **A filter is not an emitter** — S51, B38, and the rule that stops a CMY
    /// rig going black at home.
    #[test]
    fn only_an_additive_emitter_rests_open() {
        for emitter in [
            AttributeType::Red,
            AttributeType::Green,
            AttributeType::Blue,
            AttributeType::White,
            AttributeType::Amber,
            AttributeType::Uv,
            AttributeType::Lime,
            AttributeType::Indigo,
        ] {
            assert!(emitter.is_additive_emitter(), "{emitter:?}");
        }
        // The three filters and the two colour channels that are neither.
        for other in [
            AttributeType::Cyan,
            AttributeType::Magenta,
            AttributeType::Yellow,
            AttributeType::ColorWheel,
            AttributeType::ColorTemperature,
        ] {
            assert!(
                !other.is_additive_emitter(),
                "{other:?} is on the colour bank and is not an emitter"
            );
            assert_eq!(other.feature_group(), FeatureGroup::Color);
        }
        // And nothing off the colour bank is one, which is what makes the rule
        // safe to apply without asking about the bank at all.
        for attribute in AttributeType::ALL {
            if attribute.is_additive_emitter() {
                assert_eq!(
                    attribute.feature_group(),
                    FeatureGroup::Color,
                    "{attribute:?}"
                );
            }
        }
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
        // **The banks grew in S51 and there are still seven of them** (B38).
        // The owner's `design/skeleton/programmer.pdf` names seven and the
        // programmer band is drawn out of `ALL`, so the nineteen new attributes
        // were filed among the seven rather than given an eighth. What that
        // costs is paging on the colour bank, and the page an operator meets
        // first is unchanged — `the_first_four_of_every_bank_are_the_ones_that_
        // were_there_before` is the half that matters.
        assert_eq!(FeatureGroup::Beam.attributes().len(), 6);
        assert_eq!(FeatureGroup::Gobo.attributes().len(), 6);
        assert_eq!(FeatureGroup::Color.attributes().len(), 13);
        assert_eq!(FeatureGroup::Control.attributes().len(), 4);
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
        assert_eq!(
            FeatureGroup::Position.parameter(2),
            Some(AttributeType::PositionSpeed),
            "S51 gave the position bank a third knob (B38)"
        );
        assert_eq!(FeatureGroup::Position.parameter(3), None);
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
             | \"Shutter\" | \"Control\" | \"Cyan\" | \"Magenta\" | \"Yellow\" | \"Uv\" \
             | \"Lime\" | \"Indigo\" | \"ColorWheel\" | \"ColorTemperature\" \
             | \"PositionSpeed\" | \"GoboRotation\" | \"PrismRotation\" | \"Effect\" \
             | \"EffectSpeed\" | \"Frost\" | \"Blade\" | \"BeamPosition\" | \"Fog\" \
             | \"Speed\" | \"Sound\""
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
