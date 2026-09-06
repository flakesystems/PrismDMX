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
    // -- S52: the two the colour list still collapsed -----------------------
    //
    // The Open Fixture Library names thirteen emitter colours and this model
    // had eleven of them: `Warm White` and `Cold White` both became
    // [`Self::White`]. On a lamp with one of them that is a wrong label; on a
    // lamp with **both** it is a dropped channel, because the second one is
    // then a duplicate. Six profiles of the installed corpus have both.
    /// Warm white emitter — its own channel, not a shade of [`Self::White`].
    WarmWhite,
    /// Cold white emitter.
    ColdWhite,
    // -- S53: the four the capability table was folding together -----------
    //
    // The Open Fixture Library gives several capability types a
    // **discriminator** — a property that splits one type into distinct
    // physical parameters. S52's table read the type and threw the
    // discriminator away, so a colour wheel arrived as a gobo, every framing
    // blade arrived as the same blade, and a hazer arrived as a fogger. See
    // `prism_core::library::ofl::attribute_of_capability`.
    /// Rotation or scroll of a **colour** wheel.
    ///
    /// Told from [`Self::GoboRotation`] by which wheel the capability names and
    /// what that wheel's slots are.
    ///
    /// Four channels of the installed corpus, and the small number is the
    /// interesting part: most `WheelRotation` capabilities are a **range at the
    /// top of a wheel-select channel** — *slot 1 … slot 8, then rotate CW* —
    /// and such a channel is one knob. Only where a fixture gives the rotation
    /// a channel of its own is there a second one.
    ColorWheelRotation,
    /// A haze machine's output, as against [`Self::Fog`]'s.
    ///
    /// `Fog.fogType` says which, and a hazer and a fogger are not the same
    /// machine to anybody standing in front of them.
    Haze,
    /// Rotation of **one** framing blade.
    ///
    /// [`Self::Blade`] is its insertion. Which blade is the attribute's
    /// *occurrence*, out of the file's own `blade` property.
    BladeRotation,
    /// Rotation of the whole framing **system**, not of one blade.
    ///
    /// `BladeSystemRotation` carries no `blade`, so it has no occurrence to be
    /// told apart by — folding it in with [`Self::BladeRotation`] would have
    /// put it on top of blade one.
    BladeSystem,
    /// **A DMX slot of a fixture that this desk has no word for** — S54, and
    /// the one row here that is not a *kind* of parameter.
    ///
    /// Every other row says what a channel **does**. This one says only *there
    /// is a channel here*, and it exists so that the sentence **no slot of a
    /// patched fixture is out of reach** can be true without exception. Before
    /// S54 a channel the reader could not place was dropped: it kept its place
    /// in the footprint and the desk drove it to nought for ever, with no knob
    /// and **no counter that noticed** — 707 slots of the installed library,
    /// 34 of the 35 channels of a `glp/knv-cube`.
    ///
    /// Three kinds of slot land here, and the [`AttributeDef::label`] tells them
    /// apart in the operator's own words:
    ///
    /// - a channel whose meaning **switches** on another channel's value, where
    ///   the file's own resolutions do not agree on what it is (S54, `B49`);
    /// - a slot the mode leaves **unused**, or that the file states does
    ///   **nothing** — *Reserved for future use* is a channel a firmware update
    ///   may give a meaning to, and an operator who needs it then should not
    ///   need a new build of this desk;
    /// - anything a **later** version of the format describes that the table in
    ///   `prism_core::library::ofl` has not been taught yet.
    ///
    /// It is **LTP** and it rests where the file says, or at nought. Its
    /// occurrence counts the raw channels of one fixture in channel order, and
    /// its label carries the manufacturer's own name for the channel — or
    /// `Ch 7`, the channel's place in the fixture, which is the number an
    /// operator is holding a patch sheet for.
    Raw,
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
    ///
    /// **Thirty-six since S52**, and the two are appended for the same reason
    /// the nineteen were: `Warm White` and `Cold White` are colours the format
    /// names and this model collapsed into [`Self::White`]. Appending keeps
    /// *Red, Green, Blue, White* the first page of the colour bank.
    ///
    /// **Forty since S53**, and those four are a different kind of gap. Nothing
    /// was *unmapped* before them — the four are things the table folded
    /// together because it read a capability's **type** and discarded the
    /// property the format uses to tell one physical parameter from another. A
    /// colour wheel and a gobo wheel are both `WheelSlot`; a hazer and a fogger
    /// are both `Fog`; four framing blades are all `BladeInsertion`. Counted
    /// over the installed corpus: **121 wheel channels on the wrong bank** —
    /// 115 colour wheels and two prism wheels an operator could not find under
    /// *Colour* or *Prism*, and four colour-wheel rotations — 53 blade channels
    /// with no relation to the blade they drive, and 13 haze channels called
    /// fog.
    pub const ALL: [Self; 41] = [
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
        Self::WarmWhite,
        Self::ColdWhite,
        Self::ColorWheelRotation,
        Self::Haze,
        Self::BladeRotation,
        Self::BladeSystem,
        Self::Raw,
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
                | Self::WarmWhite
                | Self::ColdWhite
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
            // **S53.** A colour wheel's rotation is a colour gesture, so it is
            // on the colour bank beside the wheel itself — which is where the
            // wheel now is too.
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
            | Self::ColorTemperature
            | Self::WarmWhite
            | Self::ColdWhite
            | Self::ColorWheelRotation => FeatureGroup::Color,
            // The size and the shape of the beam.
            Self::Iris
            | Self::Zoom
            | Self::Shutter
            | Self::Frost
            | Self::Blade
            | Self::BladeRotation
            | Self::BladeSystem
            | Self::BeamPosition => FeatureGroup::Beam,
            Self::Focus => FeatureGroup::Focus,
            // The row that is touched once a show: lamp, reset, fan — and the
            // machine channels that belong to the same kind of moment.
            // **S54.** A slot this desk has no word for is reached where the
            // other once-a-show rows are. It is not given a bank of its own,
            // because the seven are one per Encoder Assign key of the surface
            // (S43) and an eighth would have nowhere to live on the X-Touch.
            Self::Control | Self::Fog | Self::Haze | Self::Speed | Self::Sound | Self::Raw => {
                FeatureGroup::Control
            }
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

/// How deep a bank's repeats may go before they stop being knobs — **S52**.
///
/// A fixture may have two of a parameter, and *two* is not the same problem as
/// *sixteen*. A head with two colour wheels wants both knobs side by side; an
/// LED tube with a red per pixel would give the colour bank six pages of things
/// called *Red* and bury the four knobs anybody actually reaches for.
///
/// So a bank whose deepest repeat is at most this many draws them all,
/// numbered; past it the bank draws **one** occurrence at a time and the band
/// grows a part stepper. Three, because that is a head with three wheels and
/// still under a page of four.
///
/// **This is not the interface's decision**, unlike `ENCODERS_PER_PAGE`: it
/// changes which parameters exist on a bank, so the jog wheel and the encoder
/// bar have to agree about it. `prism_core::Programmer::bank_parameters` reads
/// it, and it is exported to TypeScript with the rest of the bindings.
pub const INLINE_OCCURRENCES: u8 = 3;

/// One attribute of one fixture, told apart from the next one like it — **S52**.
///
/// # Why the key grew a second half
///
/// Until S52 an [`AttributeType`] *was* the key. A fixture had one dimmer, one
/// pan, one gobo wheel, and the rule was enforced rather than assumed:
/// `prism_engine::MergeError::DuplicateAttribute` refused a profile that named
/// one twice, and `prism_core::library::ofl` therefore **dropped** the second
/// channel of a kind rather than build a fixture that could not be patched.
/// Measured over the installed library that was **2 679 channels** — a head
/// with two colour wheels kept the lower one, an LED tube that writes out a red
/// per pixel kept one pixel.
///
/// So an attribute now carries an **occurrence**: which channel of that kind
/// this is, in the order the manufacturer wrote them down.
///
/// # It is nought-based, and exactly one place says otherwise
///
/// Zero is the first, which is what makes `#[serde(default)]` mean *the one
/// there has always been* — a `.prism` file written before this existed opens
/// with every value on the first occurrence and nothing has to be rewritten.
/// People count from one, so [`Display`](std::fmt::Display) writes `Gobo` for
/// the first and `Gobo 2` for the second, and that method is the **only** place
/// the two numberings meet. Anywhere else adding or subtracting one is a bug
/// waiting for a rig with three colour wheels on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub struct AttributeKey {
    /// What this attribute controls.
    pub attribute: AttributeType,
    /// Which channel of that kind it is, counted from nought.
    pub occurrence: u8,
}

impl AttributeKey {
    /// The first — and until S52 the only — attribute of this kind.
    #[must_use]
    pub const fn first(attribute: AttributeType) -> Self {
        Self {
            attribute,
            occurrence: 0,
        }
    }

    /// One of a kind, counted from nought.
    #[must_use]
    pub const fn new(attribute: AttributeType, occurrence: u8) -> Self {
        Self {
            attribute,
            occurrence,
        }
    }

    /// Whether this is the first of its kind.
    #[must_use]
    pub const fn is_first(&self) -> bool {
        self.occurrence == 0
    }

    /// Whether an occurrence field is the first — for `skip_serializing_if`.
    ///
    /// A free-standing predicate rather than a method because serde hands the
    /// field and not the key: every struct that carries the two halves flat on
    /// the wire (`CuePart`, `ProgrammerEntry`, `AttributeDef`, …) points its
    /// `skip_serializing_if` here, so *what an absent occurrence means* is
    /// written down once.
    #[must_use]
    pub const fn occurrence_is_first(occurrence: &u8) -> bool {
        *occurrence == 0
    }
}

impl From<AttributeType> for AttributeKey {
    fn from(attribute: AttributeType) -> Self {
        Self::first(attribute)
    }
}

impl std::fmt::Display for AttributeKey {
    /// `Gobo` for the first, `Gobo 2` for the second.
    ///
    /// The first is unqualified on purpose: a desk where every knob had a `1`
    /// after it would be a desk that had made a rare case everybody's problem.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.occurrence == 0 {
            write!(f, "{:?}", self.attribute)
        } else {
            write!(f, "{:?} {}", self.attribute, u16::from(self.occurrence) + 1)
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
                AttributeType::WarmWhite,
                AttributeType::ColdWhite,
                AttributeType::ColorWheelRotation,
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
                AttributeType::BladeRotation,
                AttributeType::BladeSystem,
            ],
            Self::Focus => &[AttributeType::Focus],
            Self::Control => &[
                AttributeType::Control,
                AttributeType::Fog,
                AttributeType::Speed,
                AttributeType::Sound,
                AttributeType::Haze,
                // **Last on the last bank** — S54. A slot this desk has no
                // word for is the thing an operator reaches for least often, so
                // it sits where the paging puts it last, and a venue that never
                // patches a fixture with one never sees it: the band draws only
                // what the selection has (S52, B46).
                AttributeType::Raw,
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
    /// **What the manufacturer calls this channel** — S53.
    ///
    /// The profile's own channel name — *Rotating Gobo*, *Color Wheel 2*,
    /// *Frost / Prism* — shown on the encoder in place of the attribute's own
    /// name, which is this desk's word rather than the operator's.
    ///
    /// It is a **label and not a key**: nothing is looked up by it, two
    /// fixtures whose reds are called different things still share the
    /// attribute `Red`, and a preset still means the same on both. That
    /// separation is the whole reason a desk can address a rig at all — see
    /// `prism_core::library::ofl` for why the key stays this model's own
    /// vocabulary.
    ///
    /// `None` for a generic profile, which stands in for a light nobody has
    /// told the desk about and so has no manufacturer's word to carry, and for
    /// every profile a show embedded before S53.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(any(test, feature = "proptest"), proptest(value = "None"))]
    pub label: Option<String>,
    /// **Which channel of that kind this is** — S52, counted from nought.
    ///
    /// A head with two colour wheels has two `ColorWheel` definitions, the
    /// lower-addressed one at nought. `#[serde(default)]` and skipped when it
    /// is nought, so a profile embedded in a `.prism` file before this existed
    /// reads back as the first of its kind and the file round-trips unchanged.
    #[serde(default, skip_serializing_if = "AttributeKey::occurrence_is_first")]
    pub occurrence: u8,
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
    /// The key this definition is filed under — S52.
    #[must_use]
    pub const fn key(&self) -> AttributeKey {
        AttributeKey::new(self.attribute, self.occurrence)
    }

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
    use crate::{AttributeDef, AttributeKey, AttributeType, FeatureGroup, FixtureType, MergeMode};
    use ts_rs::{Config, TS};

    fn tilt() -> AttributeDef {
        AttributeDef {
            attribute: AttributeType::Tilt,
            label: None,
            occurrence: 0,
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

    /// **Forty-one since S54, and the first fifteen have not moved** — B38,
    /// B50, B51.
    ///
    /// The count is not the interesting half. The *order* is: the encoder bar
    /// pages a bank four at a time out of `FeatureGroup::attributes`, which is
    /// this array filtered, so a nineteen appended keeps `Red, Green, Blue,
    /// White` the first page of the colour bank and an interleaved nineteen
    /// would not. A venue that never patches a CMY head must not have to page
    /// past indigo to find blue.
    #[test]
    fn the_forty_one_attribute_types_exist_and_the_first_fifteen_are_where_they_were() {
        assert_eq!(AttributeType::ALL.len(), 41);
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
        // **S52's two and S53's four are the last six**, appended for the
        // reason the nineteen before them were: the first page of the colour
        // bank stays what it was.
        //
        // S53's are not types the format gained — they are distinctions it has
        // always drawn beside a type (a wheel's slots, a `blade`, a `fogType`)
        // and that this model used to throw away. See `library::ofl`.
        //
        // **S54's `Raw` is the last for a reason**: it is the only row that is
        // not a kind of parameter at all. It says *there is a channel here* and
        // nothing else, and it exists so that no slot of a patched fixture can
        // be out of reach — including one described by a version of the format
        // nobody has written yet.
        assert_eq!(
            &AttributeType::ALL[34..],
            &[
                AttributeType::WarmWhite,
                AttributeType::ColdWhite,
                AttributeType::ColorWheelRotation,
                AttributeType::Haze,
                AttributeType::BladeRotation,
                AttributeType::BladeSystem,
                AttributeType::Raw,
            ]
        );
        // Every one of them exactly once, which is what stops a copy-and-paste
        // in a forty-one-line array from going unnoticed.
        let mut sorted = AttributeType::ALL;
        sorted.sort_unstable();
        let mut unique = sorted.to_vec();
        unique.dedup();
        assert_eq!(unique.len(), 41);
    }

    /// **An absent occurrence is the first one, and the first one writes
    /// nothing** — S52, and this is the whole of the migration.
    ///
    /// The occurrence had to reach the key every value in a show is filed
    /// under, and S34 recorded what adding a field to a persisted type costs:
    /// the frozen version-1 fixture goes red. It costs nothing here because
    /// *absent* and *the one there has always been* are made the same
    /// statement — nought-based, `#[serde(default)]`, skipped when it is
    /// nought. So a `.prism` file written by `v0.9.1` opens with every value
    /// where it was, and a show with no repeats serialises byte for byte as it
    /// did. No `MIGRATIONS` row, and nothing rewritten.
    #[test]
    fn an_absent_occurrence_is_the_first_and_the_first_writes_nothing() {
        let first = AttributeDef {
            attribute: AttributeType::Gobo,
            label: None,
            occurrence: 0,
            feature_group: FeatureGroup::Gobo,
            coarse_offset: 3,
            fine_offset: None,
            default_value: 0,
            merge_mode: MergeMode::Ltp,
            invert: false,
            physical_from: 0.0,
            physical_to: 100.0,
            ranges: Vec::new(),
        };
        let written = serde_json::to_value(&first).unwrap();
        assert!(
            written.get("occurrence").is_none(),
            "the first of a kind writes no occurrence: {written}"
        );

        // And a profile embedded before S52 has no such key at all.
        let older = serde_json::json!({
            "attribute": "Gobo",
            "featureGroup": "Gobo",
            "coarseOffset": 3,
            "fineOffset": null,
            "defaultValue": 0,
            "mergeMode": "LTP",
            "invert": false,
            "physicalFrom": 0.0,
            "physicalTo": 100.0
        });
        let read: AttributeDef = serde_json::from_value(older).unwrap();
        assert_eq!(read, first);
        assert_eq!(read.key(), AttributeKey::first(AttributeType::Gobo));

        // A second of a kind does write one, or there would be nothing to tell
        // the two apart on the wire.
        let second = AttributeDef {
            label: None,
            occurrence: 1,
            ..first
        };
        assert_eq!(
            serde_json::to_value(&second).unwrap().get("occurrence"),
            Some(&serde_json::json!(1))
        );
    }

    /// **The first of a kind is unqualified, and the second counts from one.**
    ///
    /// The one place in the program where the nought-based key is read out
    /// one-based. A desk where every knob carried a `1` would have made a rare
    /// case everybody's problem.
    #[test]
    fn a_key_reads_out_one_based_and_only_here() {
        assert_eq!(AttributeKey::first(AttributeType::Gobo).to_string(), "Gobo");
        assert_eq!(
            AttributeKey::new(AttributeType::Gobo, 1).to_string(),
            "Gobo 2"
        );
        assert_eq!(
            AttributeKey::new(AttributeType::ColorWheel, 2).to_string(),
            "ColorWheel 3"
        );
        assert!(AttributeKey::first(AttributeType::Red).is_first());
        assert!(!AttributeKey::new(AttributeType::Red, 1).is_first());
        // The ordering is the one `MergePlan` sorts by: by kind, then by which
        // one of that kind.
        assert!(
            AttributeKey::new(AttributeType::Red, 0) < AttributeKey::new(AttributeType::Red, 1)
        );
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
        // **Eight on the beam bank since S53**: one framing blade turning and
        // the whole frame turning are two gestures the format distinguishes and
        // this model used to fold onto `Blade`, which numbered them by the
        // order the reader met the channels.
        assert_eq!(FeatureGroup::Beam.attributes().len(), 8);
        assert_eq!(FeatureGroup::Gobo.attributes().len(), 6);
        // **Sixteen on the colour bank since S53.** S52 made it fifteen, not
        // thirteen: `Warm White` and `Cold White` are colours the format names
        // and this model used to fold into `White`, taking the second of the
        // two with them. S53 adds the colour wheel's **rotation**, which is a
        // colour gesture and used to be filed beside the gobos.
        assert_eq!(FeatureGroup::Color.attributes().len(), 16);
        // **Six on the control bank since S54.** S53 made it five — a hazer is
        // not a fogger — and S54 adds the raw channel, which is filed among the
        // once-a-show rows rather than given a bank of its own: the seven are
        // one per Encoder Assign key of the X-Touch (S43), and an eighth would
        // have nowhere on the surface to live.
        assert_eq!(FeatureGroup::Control.attributes().len(), 6);
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
             | \"Speed\" | \"Sound\" | \"WarmWhite\" | \"ColdWhite\" \
             | \"ColorWheelRotation\" | \"Haze\" | \"BladeRotation\" \
             | \"BladeSystem\" | \"Raw\""
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
