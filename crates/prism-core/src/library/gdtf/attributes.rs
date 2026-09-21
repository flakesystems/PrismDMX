//! GDTF's attribute names, and which knob of this desk each one is — **S61**.
//!
//! # The format names its parameters, and that is the whole reason to read it
//!
//! The Open Fixture Library states a *capability type* and leaves the rest to
//! prose, which is why [`super::super::ofl`] has to read a channel's
//! capabilities, its wheel's slots and the colour written beside it to work out
//! what the channel is. GDTF states the parameter by name from a table the
//! format publishes: `Dimmer`, `Pan`, `ColorAdd_R`, `Gobo1Pos`,
//! `Shutter1Strobe`. So this file is a lookup, and the interesting part of it
//! is only the two rules below.
//!
//! # The number is in the name
//!
//! GDTF numbers a device's repeated parameters: a head with two gobo wheels has
//! `Gobo1` and `Gobo2`, and its second wheel's rotation is `Gobo2Pos`. This
//! desk numbers them too — [`prism_domain::AttributeKey`]'s nought-based
//! `occurrence`, S52 — so the number is **read from the name** rather than
//! counted off the channel order, and `Gobo2` is the second gobo wheel even in
//! a mode that does not carry the first.
//!
//! Every digit run is replaced by `n` before the lookup, so one row of the
//! table answers for every number of a family, and the **first** run is the
//! occurrence.
//!
//! # A name this desk has no word for is a knob, not a hole
//!
//! GDTF's table is long and this desk has forty-one attributes. An attribute
//! that maps to none of them becomes [`AttributeType::Raw`] carrying GDTF's own
//! name as its label: the channel keeps its place in the footprint, keeps its
//! name on the encoder, and an operator can still put a value on it. The same
//! answer S54 gave an OFL channel a mode had marked unused, and the reason
//! [`super::Conversion::channels_raw`] is counted rather than asserted to be
//! nought — this table falling behind the format should be visible, not fatal.

use prism_domain::AttributeType;

/// The attribute one GDTF name is, and which of its kind — **nought-based**.
///
/// `("Gobo2", …)` answers `(Gobo, 1)`; a name with no number in it answers
/// occurrence 0, and the caller renumbers where two names collide.
#[must_use]
pub fn of(name: &str) -> (AttributeType, u8) {
    let (family, number) = family_of(name);
    let attribute = TABLE
        .iter()
        .find(|(key, _)| *key == family)
        .map_or(AttributeType::Raw, |(_, attribute)| *attribute);
    (attribute, number)
}

/// The name GDTF's own table gives the parameter, spaced for a person.
///
/// `Shutter1Strobe` reads *Shutter 1 Strobe*, `ColorAdd_R` reads *Color Add R*.
/// It is what lands on [`prism_domain::AttributeDef::label`] — the
/// manufacturer's word, shown on the encoder in place of this desk's — and
/// nothing is ever looked up by it.
#[must_use]
pub fn pretty(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let mut previous = '\0';
    for character in name.chars() {
        let boundary = !out.is_empty()
            && !previous.is_whitespace()
            && ((character.is_ascii_uppercase() && !previous.is_ascii_uppercase())
                || (character.is_ascii_digit() && !previous.is_ascii_digit()));
        if character == '_' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            if boundary && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(character);
        }
        previous = character;
    }
    out.trim().to_owned()
}

/// Where an attribute rests when the file states no default.
///
/// The same rule the four built-in profiles and the OFL reader follow (B1,
/// B38): an **additive emitter** rests at full, because on every desk the owner
/// has used a colour starts open and you subtract, and everything else rests at
/// nought. Position is the exception that has to be written out: a head that is
/// patched and never touched points at the middle of its travel rather than at
/// one end stop.
#[must_use]
pub fn home(attribute: AttributeType) -> u16 {
    match attribute {
        AttributeType::Pan | AttributeType::Tilt => 32_768,
        _ if attribute.is_additive_emitter() => u16::MAX,
        _ => 0,
    }
}

/// A name with every digit run replaced by `n`, and the first run's value.
///
/// `Gobo2Pos` is `("GobonPos", 1)`; `ColorAdd_R` is `("ColorAdd_R", 0)`. The
/// number is turned into a nought-based occurrence here, so `Gobo1` and a
/// hypothetical `Gobo0` are both the first of their kind rather than the first
/// and the something-before-it.
fn family_of(name: &str) -> (String, u8) {
    let mut family = String::with_capacity(name.len());
    let mut number: Option<u8> = None;
    let mut digits = String::new();
    for character in name.chars() {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        if !digits.is_empty() {
            family.push('n');
            if number.is_none() {
                number = digits
                    .parse::<u8>()
                    .ok()
                    .map(|value| value.saturating_sub(1));
            }
            digits.clear();
        }
        family.push(character);
    }
    if !digits.is_empty() {
        family.push('n');
        if number.is_none() {
            number = digits
                .parse::<u8>()
                .ok()
                .map(|value| value.saturating_sub(1));
        }
    }
    (family, number.unwrap_or(0))
}

/// GDTF's attribute names, with every digit run written `n`.
///
/// Ordered as the format's own table is — intensity, position, colour, beam,
/// focus, control — rather than alphabetically, so a name missing from it is
/// found by reading the group it would belong to.
///
/// A name that is **not** here is [`AttributeType::Raw`], deliberately. See the
/// module documentation.
const TABLE: &[(&str, AttributeType)] = &[
    /* Intensity ---------------------------------------------------------- */
    ("Dimmer", AttributeType::Dimmer),
    ("ShutternStrobe", AttributeType::Shutter),
    ("ShutternStrobePulse", AttributeType::Shutter),
    ("ShutternStrobePulseClose", AttributeType::Shutter),
    ("ShutternStrobePulseOpen", AttributeType::Shutter),
    ("ShutternStrobeRandom", AttributeType::Shutter),
    ("ShutternStrobeRandomPulse", AttributeType::Shutter),
    ("ShutternStrobeRandomPulseClose", AttributeType::Shutter),
    ("ShutternStrobeRandomPulseOpen", AttributeType::Shutter),
    ("ShutternStrobeEffect", AttributeType::Shutter),
    ("Shuttern", AttributeType::Shutter),
    /* Position ----------------------------------------------------------- */
    ("Pan", AttributeType::Pan),
    ("Tilt", AttributeType::Tilt),
    ("PanRotate", AttributeType::PositionSpeed),
    ("TiltRotate", AttributeType::PositionSpeed),
    ("PositionEffect", AttributeType::Effect),
    ("PositionEffectRate", AttributeType::EffectSpeed),
    ("PositionEffectFade", AttributeType::EffectSpeed),
    ("PositionModes", AttributeType::Control),
    ("PanTiltMode", AttributeType::Control),
    ("PositionMSpeed", AttributeType::PositionSpeed),
    ("PanMode", AttributeType::Control),
    ("TiltMode", AttributeType::Control),
    ("XYZ_X", AttributeType::BeamPosition),
    ("XYZ_Y", AttributeType::BeamPosition),
    ("XYZ_Z", AttributeType::BeamPosition),
    /* Colour, additive --------------------------------------------------- */
    ("ColorAdd_R", AttributeType::Red),
    ("ColorAdd_G", AttributeType::Green),
    ("ColorAdd_B", AttributeType::Blue),
    ("ColorAdd_W", AttributeType::White),
    ("ColorAdd_WW", AttributeType::WarmWhite),
    ("ColorAdd_CW", AttributeType::ColdWhite),
    ("ColorAdd_A", AttributeType::Amber),
    ("ColorAdd_UV", AttributeType::Uv),
    ("ColorAdd_C", AttributeType::Cyan),
    ("ColorAdd_M", AttributeType::Magenta),
    ("ColorAdd_Y", AttributeType::Yellow),
    // The format's names for the in-between emitters. `RY` is red-yellow,
    // which is what a manufacturer sells as amber, and `GY` is green-yellow,
    // which is lime. `BM` is blue-magenta, which is indigo. This desk has a
    // word for each of the three (S51, S52), so they keep their own channel
    // rather than being folded onto a neighbour and losing one.
    ("ColorAdd_RY", AttributeType::Amber),
    ("ColorAdd_GY", AttributeType::Lime),
    ("ColorAdd_BM", AttributeType::Indigo),
    ("ColorAdd_RM", AttributeType::Magenta),
    ("ColorAdd_GC", AttributeType::Cyan),
    ("ColorAdd_BC", AttributeType::Cyan),
    /* Colour, subtractive ------------------------------------------------ */
    ("ColorSub_C", AttributeType::Cyan),
    ("ColorSub_M", AttributeType::Magenta),
    ("ColorSub_Y", AttributeType::Yellow),
    ("ColorSub_R", AttributeType::Red),
    ("ColorSub_G", AttributeType::Green),
    ("ColorSub_B", AttributeType::Blue),
    /* Colour, wheels and temperature ------------------------------------- */
    ("Colorn", AttributeType::ColorWheel),
    ("ColornWheelSpin", AttributeType::ColorWheelRotation),
    ("ColornWheelIndex", AttributeType::ColorWheelRotation),
    ("ColornWheelRandom", AttributeType::ColorWheelRotation),
    ("ColornWheelAudio", AttributeType::ColorWheelRotation),
    ("ColornMode", AttributeType::Control),
    ("ColorMacron", AttributeType::ColorWheel),
    ("ColorMacronRate", AttributeType::EffectSpeed),
    ("CTO", AttributeType::ColorTemperature),
    ("CTC", AttributeType::ColorTemperature),
    ("CTB", AttributeType::ColorTemperature),
    ("Tint", AttributeType::ColorTemperature),
    ("HSB_Hue", AttributeType::ColorWheel),
    ("HSB_Saturation", AttributeType::ColorWheel),
    ("HSB_Brightness", AttributeType::Dimmer),
    /* Gobo --------------------------------------------------------------- */
    ("Gobon", AttributeType::Gobo),
    ("GobonWheelIndex", AttributeType::GoboRotation),
    ("GobonWheelSpin", AttributeType::GoboRotation),
    ("GobonWheelShake", AttributeType::GoboRotation),
    ("GobonWheelRandom", AttributeType::GoboRotation),
    ("GobonWheelAudio", AttributeType::GoboRotation),
    ("GobonWheelMode", AttributeType::Control),
    ("GobonPos", AttributeType::GoboRotation),
    ("GobonPosRotate", AttributeType::GoboRotation),
    ("GobonPosShake", AttributeType::GoboRotation),
    ("GobonSelectSpin", AttributeType::GoboRotation),
    ("GobonSelectShake", AttributeType::GoboRotation),
    ("GobonSelectEffects", AttributeType::Effect),
    ("AnimationWheeln", AttributeType::Gobo),
    ("AnimationWheelnPos", AttributeType::GoboRotation),
    ("AnimationWheelnPosRotate", AttributeType::GoboRotation),
    /* Beam --------------------------------------------------------------- */
    ("Iris", AttributeType::Iris),
    ("IrisStrobe", AttributeType::Iris),
    ("IrisStrobeRandom", AttributeType::Iris),
    ("IrisPulseClose", AttributeType::Iris),
    ("IrisPulseOpen", AttributeType::Iris),
    ("Zoom", AttributeType::Zoom),
    ("ZoomModeSpot", AttributeType::Zoom),
    ("ZoomModeBeam", AttributeType::Zoom),
    ("Focusn", AttributeType::Focus),
    ("FocusnAdjust", AttributeType::Focus),
    ("FocusnDistance", AttributeType::Focus),
    ("Frostn", AttributeType::Frost),
    ("FrostnPulseOpen", AttributeType::Frost),
    ("FrostnPulseClose", AttributeType::Frost),
    ("FrostnRamp", AttributeType::Frost),
    ("Prismn", AttributeType::Prism),
    ("PrismnSelectSpin", AttributeType::PrismRotation),
    ("PrismnMacro", AttributeType::Effect),
    ("PrismnPos", AttributeType::PrismRotation),
    ("PrismnPosRotate", AttributeType::PrismRotation),
    ("BeamShaper", AttributeType::Blade),
    ("BeamShaperMacro", AttributeType::BladeSystem),
    ("BeamShaperPos", AttributeType::BladeRotation),
    ("BeamShaperPosRotate", AttributeType::BladeRotation),
    ("BeamEffects", AttributeType::Effect),
    ("BeamEffectsRate", AttributeType::EffectSpeed),
    ("BeamEffectsFade", AttributeType::EffectSpeed),
    /* Framing shutters --------------------------------------------------- */
    ("BladenA", AttributeType::Blade),
    ("BladenB", AttributeType::Blade),
    ("BladenRot", AttributeType::BladeRotation),
    ("ShaperRot", AttributeType::BladeRotation),
    ("ShaperMacros", AttributeType::BladeSystem),
    ("ShaperMacrosSpeed", AttributeType::EffectSpeed),
    /* Effects ------------------------------------------------------------ */
    ("Effectsn", AttributeType::Effect),
    ("EffectsnRate", AttributeType::EffectSpeed),
    ("EffectsnFade", AttributeType::EffectSpeed),
    ("EffectsnAdjustm", AttributeType::Effect),
    ("EffectsnPos", AttributeType::Effect),
    ("EffectsnPosRotate", AttributeType::EffectSpeed),
    ("EffectsSync", AttributeType::EffectSpeed),
    /* Atmospherics ------------------------------------------------------- */
    ("Fogn", AttributeType::Fog),
    ("Hazen", AttributeType::Haze),
    ("Fan", AttributeType::Speed),
    ("Fann", AttributeType::Speed),
    ("FannMode", AttributeType::Control),
    ("Blower", AttributeType::Speed),
    ("Blowern", AttributeType::Speed),
    /* Control ------------------------------------------------------------ */
    ("Control", AttributeType::Control),
    ("Controln", AttributeType::Control),
    ("Function", AttributeType::Control),
    ("DimmerMode", AttributeType::Control),
    ("DimmerCurve", AttributeType::Control),
    ("LampControl", AttributeType::Control),
    ("Reserved", AttributeType::Control),
    ("NoFeature", AttributeType::Control),
    ("DisplayIntensity", AttributeType::Control),
    ("FixtureGlobalReset", AttributeType::Control),
    ("FixtureCalibrationReset", AttributeType::Control),
    ("ShutterReset", AttributeType::Control),
    ("BeamReset", AttributeType::Control),
    ("ColorMixReset", AttributeType::Control),
    ("ColorWheelReset", AttributeType::Control),
    ("FocusReset", AttributeType::Control),
    ("FrostReset", AttributeType::Control),
    ("GoboWheelReset", AttributeType::Control),
    ("IntensityReset", AttributeType::Control),
    ("PanTiltReset", AttributeType::Control),
    ("ZoomReset", AttributeType::Control),
    ("AudioVolume", AttributeType::Sound),
    ("Input", AttributeType::Sound),
    ("Playmode", AttributeType::Control),
    ("Speed", AttributeType::Speed),
    ("Speedn", AttributeType::Speed),
];

#[cfg(test)]
mod tests {
    use prism_domain::AttributeType;

    use super::{family_of, home, of, pretty};

    #[test]
    fn a_plain_name_is_its_attribute() {
        assert_eq!(of("Dimmer"), (AttributeType::Dimmer, 0));
        assert_eq!(of("Pan"), (AttributeType::Pan, 0));
        assert_eq!(of("ColorAdd_R"), (AttributeType::Red, 0));
        assert_eq!(of("ColorSub_C"), (AttributeType::Cyan, 0));
    }

    #[test]
    fn the_number_in_the_name_is_the_occurrence() {
        assert_eq!(of("Gobo1"), (AttributeType::Gobo, 0));
        assert_eq!(of("Gobo2"), (AttributeType::Gobo, 1));
        assert_eq!(of("Gobo2PosRotate"), (AttributeType::GoboRotation, 1));
        assert_eq!(of("Color3"), (AttributeType::ColorWheel, 2));
        assert_eq!(of("Blade3A"), (AttributeType::Blade, 2));
    }

    #[test]
    fn a_name_with_no_word_here_is_a_raw_knob() {
        assert_eq!(of("SomethingNobodyHasInvented"), (AttributeType::Raw, 0));
        assert_eq!(of(""), (AttributeType::Raw, 0));
        // And it still carries its number, so two of them do not collide on
        // the strength of being equally unknown.
        assert_eq!(of("Wibble2"), (AttributeType::Raw, 1));
    }

    #[test]
    fn the_in_between_emitters_keep_their_own_channel() {
        // The three S51 and S52 gave this desk a word for. A head with a
        // dedicated lime must not come out with two greens.
        assert_eq!(of("ColorAdd_RY").0, AttributeType::Amber);
        assert_eq!(of("ColorAdd_GY").0, AttributeType::Lime);
        assert_eq!(of("ColorAdd_BM").0, AttributeType::Indigo);
        assert_eq!(of("ColorAdd_WW").0, AttributeType::WarmWhite);
        assert_eq!(of("ColorAdd_CW").0, AttributeType::ColdWhite);
    }

    #[test]
    fn a_family_is_its_name_with_the_digits_written_n() {
        assert_eq!(family_of("Gobo1"), ("Gobon".to_owned(), 0));
        assert_eq!(family_of("Gobo2Pos"), ("GobonPos".to_owned(), 1));
        assert_eq!(
            family_of("Effects1Adjust2"),
            ("EffectsnAdjustn".to_owned(), 0)
        );
        assert_eq!(family_of("Dimmer"), ("Dimmer".to_owned(), 0));
        // A number the format does not produce, and which must not wrap.
        assert_eq!(family_of("Gobo0"), ("Gobon".to_owned(), 0));
        assert_eq!(family_of("Gobo999"), ("Gobon".to_owned(), 0));
    }

    #[test]
    fn a_pretty_name_is_the_format_s_own_name_spaced() {
        assert_eq!(pretty("Shutter1Strobe"), "Shutter 1 Strobe");
        assert_eq!(pretty("ColorAdd_R"), "Color Add R");
        assert_eq!(pretty("Dimmer"), "Dimmer");
        assert_eq!(pretty("XYZ_X"), "XYZ X");
        assert_eq!(pretty("Gobo1PosRotate"), "Gobo 1 Pos Rotate");
    }

    #[test]
    fn a_colour_rests_open_and_a_head_rests_centred() {
        // B1's rule, and the one exception to it.
        assert_eq!(home(AttributeType::Red), u16::MAX);
        assert_eq!(home(AttributeType::Cyan), 0);
        assert_eq!(home(AttributeType::Dimmer), 0);
        assert_eq!(home(AttributeType::Pan), 32_768);
        assert_eq!(home(AttributeType::Tilt), 32_768);
    }
}
