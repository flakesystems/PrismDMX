//! Eight colours, and an arbitrary one to choose between them.
//!
//! A scribble strip's backlight is three lamps, so the colours it can show are
//! exactly the eight corners of the RGB cube ([`StripColor`]). A preset, an
//! executor or a fixture carries a [`RgbColor`] — twenty-four bits. Turning one
//! into the other is a quantisation, and `docs/MCU_MAPPING.md` §2.3 says which
//! one: **hue first**.
//!
//! # Why hue first, and not the obvious thing
//!
//! The obvious thing is nearest corner by distance in RGB, which is what
//! Ardour does: normalise to the brightest component and threshold each channel
//! at half. It collapses a pale orange to white, because a pale orange really is
//! nearer to white than to red in that space. §2.3 records the Ableton script's
//! author finding the same and preferring hue — and for a lighting desk that is
//! not a matter of taste. **A pastel is still the colour it is a pastel of**: an
//! operator who put a pale amber on an executor is looking for the amber one, and
//! a strip that went white has lost the only thing they were looking at it for.
//!
//! So the rule here is: take the hue, snap it to the nearest of the six
//! chromatic corners, and only fall back to white when there is no hue worth
//! keeping.
//!
//! # The two ends of the range, which are decisions rather than mathematics
//!
//! **Grey goes to white.** A colour with no chroma has no hue to keep, and white
//! is the readable one.
//!
//! **Black is only ever chosen, never computed.** `StripColor::Off` is the
//! backlight *off*, and the text on an unlit strip cannot be read at all
//! (§2.3) — so a strip must never arrive there by rounding. Only an exactly black
//! [`RgbColor`] quantises to it, and a caller who means *nothing here* is
//! expected to say [`StripColor::Off`] outright. Everything else that is dark but
//! not black keeps its hue: a strip showing a deep blue is legible, a strip
//! showing nothing is not.

use prism_domain::RgbColor;

use crate::feedback::StripColor;

/// How much chroma a colour needs before it counts as a colour rather than a
/// grey, as a fraction of the brightest channel out of 255.
///
/// 32/255 is one eighth. Below that the strip would be showing a hue nobody
/// asked for: `#C8C6C3` is a warm grey to the eye and would otherwise land on
/// red. Above it the hue is deliberate — `#FFC8B4`, a pale orange, has a chroma
/// of 75/255 and stays red, which is the whole point of quantising by hue.
///
/// A constant rather than a profile field: it describes how people see colour,
/// not what a device can display.
pub const WHITE_CHROMA_FLOOR: u16 = 32;

/// The colour the scribble strips can show that is closest to this one.
///
/// Hue first, as the module documentation explains: greys become
/// [`StripColor::White`], and only an exactly black colour becomes
/// [`StripColor::Off`].
///
/// Total, integer, and free of floating point — it runs on the surface thread
/// beside a 44 Hz tick, and two hosts computing a hue in `f32` is a difference
/// nobody wants to debug from a photograph of a desk.
#[must_use]
pub const fn quantize(color: RgbColor) -> StripColor {
    let r = color.r as u16;
    let g = color.g as u16;
    let b = color.b as u16;

    let max = max3(r, g, b);
    if max == 0 {
        // The one input that is allowed to turn a backlight off.
        return StripColor::Off;
    }
    let min = min3(r, g, b);
    let chroma = max - min;
    if chroma * 255 < max * WHITE_CHROMA_FLOOR {
        return StripColor::White;
    }

    // Which lamp is fully on decides the primary; the middle channel decides
    // whether the answer is that primary or the mix of the two. The sixth of
    // the colour wheel the hue sits in is exactly this pair, and where it sits
    // inside that sixth is `(mid - min) / chroma` — so half a sector is the
    // whole comparison, and no angle is ever computed.
    let (primary, middle, mid) = if r >= g && r >= b {
        if b <= g {
            (StripColor::Red, StripColor::Green, g)
        } else {
            (StripColor::Red, StripColor::Blue, b)
        }
    } else if g >= b {
        if b <= r {
            (StripColor::Green, StripColor::Red, r)
        } else {
            (StripColor::Green, StripColor::Blue, b)
        }
    } else if r <= g {
        (StripColor::Blue, StripColor::Green, g)
    } else {
        (StripColor::Blue, StripColor::Red, r)
    };

    if (mid - min) * 2 < chroma {
        return primary;
    }
    // The mixed corner: the primary's lamp and the middle channel's, together.
    // Both are corners of the cube, so the union of their bits is one as well.
    let secondary = primary.bits() | middle.bits();
    match StripColor::from_bits(secondary) {
        Some(color) => color,
        // Unreachable: `secondary` is the union of two single-bit values, so it
        // is at most `0b111`. Answering white rather than panicking is the
        // crate's rule — `CLAUDE.md`'s zero-crash invariant reaches every line
        // that runs on the surface thread.
        None => StripColor::White,
    }
}

/// The largest of three.
const fn max3(a: u16, b: u16, c: u16) -> u16 {
    let ab = if a > b { a } else { b };
    if ab > c { ab } else { c }
}

/// The smallest of three.
const fn min3(a: u16, b: u16, c: u16) -> u16 {
    let ab = if a < b { a } else { b };
    if ab < c { ab } else { c }
}

#[cfg(test)]
mod tests {
    use super::{WHITE_CHROMA_FLOOR, quantize};
    use crate::feedback::StripColor;
    use prism_domain::RgbColor;

    /// A colour, written the way a person writes one.
    const fn rgb(r: u8, g: u8, b: u8) -> RgbColor {
        RgbColor { r, g, b }
    }

    #[test]
    fn the_eight_corners_quantise_to_themselves() {
        // The identity case, and the one a table cannot get wrong quietly: a
        // saturated red must be red, not "nearly red".
        for color in StripColor::ALL {
            let (r, g, b) = color.rgb();
            let full = rgb(
                if r { 255 } else { 0 },
                if g { 255 } else { 0 },
                if b { 255 } else { 0 },
            );
            assert_eq!(quantize(full), color, "{color:?} did not survive");
        }
    }

    #[test]
    fn a_pastel_keeps_the_colour_it_is_a_pastel_of() {
        // The reason §2.3 chose hue over distance. Every one of these is nearer
        // to white in RGB than to the corner it is named after, and every one of
        // them is what the operator was looking at when they chose it.
        assert_eq!(quantize(rgb(255, 200, 180)), StripColor::Red); // pale orange
        assert_eq!(quantize(rgb(255, 210, 210)), StripColor::Red); // pink
        assert_eq!(quantize(rgb(200, 255, 200)), StripColor::Green); // mint
        assert_eq!(quantize(rgb(200, 220, 255)), StripColor::Blue); // ice blue
        assert_eq!(quantize(rgb(255, 245, 200)), StripColor::Yellow); // straw
        assert_eq!(quantize(rgb(240, 200, 255)), StripColor::Magenta); // lilac
        assert_eq!(quantize(rgb(200, 250, 255)), StripColor::Cyan); // pale cyan
    }

    #[test]
    fn a_dark_colour_keeps_its_hue_rather_than_going_out() {
        // Brightness is thrown away deliberately: the backlight has one level.
        // A deep blue strip is readable and says "blue"; a dark strip says
        // nothing and cannot be read at all.
        assert_eq!(quantize(rgb(0, 0, 20)), StripColor::Blue);
        assert_eq!(quantize(rgb(12, 0, 0)), StripColor::Red);
        assert_eq!(quantize(rgb(1, 1, 0)), StripColor::Yellow);
    }

    #[test]
    fn grey_goes_to_white_and_only_black_goes_out() {
        // §2.3: greys to white, and black only when it was chosen. A colour that
        // rounded itself onto an unlit backlight would be an executor whose name
        // cannot be read.
        for level in [1u8, 8, 64, 128, 200, 255] {
            assert_eq!(
                quantize(rgb(level, level, level)),
                StripColor::White,
                "grey {level}"
            );
        }
        assert_eq!(quantize(rgb(0, 0, 0)), StripColor::Off);
        // Near-greys as well: chroma below the floor has no hue worth keeping.
        assert_eq!(quantize(rgb(200, 198, 195)), StripColor::White);
        assert_eq!(quantize(rgb(255, 250, 248)), StripColor::White);
    }

    #[test]
    fn the_white_floor_is_where_the_constant_says_it_is() {
        // Both sides of the threshold, computed from the constant rather than
        // from a remembered number, so moving the constant moves the test with
        // it instead of breaking it.
        let max = 255u16;
        let below = max - (max * WHITE_CHROMA_FLOOR / 255) + 1;
        let above = max - (max * WHITE_CHROMA_FLOOR / 255) - 1;
        assert_eq!(
            quantize(rgb(255, below as u8, below as u8)),
            StripColor::White
        );
        assert_eq!(
            quantize(rgb(255, above as u8, above as u8)),
            StripColor::Red
        );
    }

    #[test]
    fn a_hue_halfway_between_two_corners_lands_on_the_mixed_one() {
        // Orange is 30 degrees, exactly between red and yellow. The rule has to
        // answer *something* deterministically, and the mixed corner is the one
        // that keeps the green lamp the colour asked for: an operator's amber
        // reads as yellow rather than as plain red.
        assert_eq!(quantize(rgb(255, 128, 0)), StripColor::Yellow);
        // ...and one step either side of the tie goes where it should.
        assert_eq!(quantize(rgb(255, 100, 0)), StripColor::Red);
        assert_eq!(quantize(rgb(255, 160, 0)), StripColor::Yellow);
    }

    #[test]
    fn every_sector_of_the_wheel_is_reached() {
        // Six sectors, and the code picks between them with four comparisons -
        // exactly the shape where one branch is silently unreachable. Walking
        // the wheel in 15-degree steps says that all six appear and that the
        // sequence goes round in order rather than jumping about.
        let wheel = [
            (255, 0, 0),
            (255, 64, 0),
            (255, 191, 0),
            (255, 255, 0),
            (191, 255, 0),
            (64, 255, 0),
            (0, 255, 0),
            (0, 255, 64),
            (0, 255, 191),
            (0, 255, 255),
            (0, 191, 255),
            (0, 64, 255),
            (0, 0, 255),
            (64, 0, 255),
            (191, 0, 255),
            (255, 0, 255),
            (255, 0, 191),
            (255, 0, 64),
        ];
        let expected = [
            StripColor::Red,
            StripColor::Red,
            StripColor::Yellow,
            StripColor::Yellow,
            StripColor::Yellow,
            StripColor::Green,
            StripColor::Green,
            StripColor::Green,
            StripColor::Cyan,
            StripColor::Cyan,
            StripColor::Cyan,
            StripColor::Blue,
            StripColor::Blue,
            StripColor::Blue,
            StripColor::Magenta,
            StripColor::Magenta,
            StripColor::Magenta,
            StripColor::Red,
        ];
        for ((r, g, b), want) in wheel.into_iter().zip(expected) {
            assert_eq!(quantize(rgb(r, g, b)), want, "#{r:02X}{g:02X}{b:02X}");
        }
    }

    #[test]
    fn nothing_in_the_whole_cube_quantises_to_black_but_black() {
        // The strongest form of the rule, and cheap: a coarse walk of the cube
        // plus every colour that has a single unit of light in it.
        for r in (0..=255u8).step_by(17) {
            for g in (0..=255u8).step_by(17) {
                for b in (0..=255u8).step_by(17) {
                    let color = quantize(rgb(r, g, b));
                    assert_eq!(
                        color == StripColor::Off,
                        (r, g, b) == (0, 0, 0),
                        "#{r:02X}{g:02X}{b:02X} became {color:?}"
                    );
                }
            }
        }
        for channel in 0..3 {
            let color = rgb(
                u8::from(channel == 0),
                u8::from(channel == 1),
                u8::from(channel == 2),
            );
            assert_ne!(quantize(color), StripColor::Off);
        }
    }
}
