//! Two acceleration curves, because the surface has two kinds of relative
//! control and only one of them accelerates itself.
//!
//! `docs/MCU_MAPPING.md` §2.1 describes the V-Pots and the jog wheel with one
//! sentence — *relative, sign-magnitude* — and S20 measured that they are not
//! the same control at all (§2.7):
//!
//! | Control | Turned slowly | Spun hard |
//! |---|---|---|
//! | V-Pot | ±1 per message | **±1…8** per message, peaking at 4–6 |
//! | Jog wheel | ±1 per message | **±1** per message, in 404 of 404 |
//!
//! The jog wheel raises its *rate*, not its magnitude. So the two curves cannot
//! be one table with different numbers in it — they read different things:
//!
//! - [`VPotAcceleration`] maps the **magnitude the desk reported**. The
//!   surface has already measured the speed; the curve only decides what a
//!   detent is worth.
//! - [`JogAcceleration`] maps the **interval since the previous message**,
//!   because the magnitude carries no information at all. This is why S21 owns
//!   the clock: the arrival time is the only signal the wheel gives.
//!
//! A single shared curve would be wrong about one of them, and wrong in the
//! worse direction for the jog wheel: it would make the wheel a control that
//! cannot be hurried, which is exactly what an operator reaches for it to do.
//!
//! # What a curve is denominated in — S43, punch-list B20
//!
//! **Attribute units**, all the way through: the number a curve answers with is
//! added to a 16-bit parameter by `Command::SetAttribute { relative: true }`,
//! and nothing between here and `prism_core::programmer::nudge` scales it.
//!
//! Both tables were originally written as though a *step* were something larger
//! that somebody downstream would multiply out. Nobody did. So a careful V-Pot
//! click moved a parameter by one part in 65 535 — 0.0015 % — and the owner's
//! punch list found the same arithmetic on the wheel: **a full turn of the jog
//! wheel changed a value by about 1 %**. Both controls were doing exactly what
//! they were told and the telling was wrong by two orders of magnitude.
//!
//! [`COARSE`] is what fixes the size and, more usefully, what makes the numbers
//! readable: a curve entry of `COARSE` is *one DMX step of an 8-bit channel*,
//! which is the smallest move a lamp on a coarse channel can actually make.
//! Below that a turn is a number changing on a screen.
//!
//! # These numbers are taste, and they are data
//!
//! The measurements above are facts about the desk. How far a detent moves a
//! parameter is a decision about how a desk should feel, so both curves are
//! values a profile or a settings screen can replace rather than constants
//! compiled into the translation.

use std::time::Duration;

/// One DMX step of an 8-bit channel, in the 16-bit attribute range: 65 535 / 255.
///
/// The unit both curves are written in. It is the smallest move that is visible
/// on the least precise fixture in a rig, which makes it the floor worth having
/// — a control that moves a parameter by less than this has moved a number and
/// not a lamp.
pub const COARSE: i16 = 257;

/// How the magnitude a V-Pot reports becomes a distance to move a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VPotAcceleration {
    /// Attribute units for a reported magnitude of 1, 2, 3 … in order.
    ///
    /// A magnitude past the end of the table saturates on the last entry rather
    /// than falling back to one step: a surface that reported more than this
    /// table describes was turned *faster*, and answering "one step" would make
    /// a hard spin do less than a slow one.
    pub curve: &'static [i16],
}

/// The default V-Pot curve: the triangular numbers, in [`COARSE`] steps.
///
/// One detent is one coarse DMX step, and each further detent in the same
/// message is worth one more than the last — 1, 3, 6, 10, 15, 21, 28, 36 across
/// the 1…8 the desk actually sends (§2.7). A full-speed sweep therefore covers
/// thirty-six times what a careful click does, which is the ratio that lets one
/// encoder both trim a value by hand and cross a 16-bit parameter without an
/// operator winding it like a fishing reel: the top row is 9 252, or 14 % of the
/// range, per message.
///
/// Linear would be the obvious alternative and is the one to avoid: it makes the
/// fastest turn eight times the slowest, and eight is not enough range to be
/// worth the surface having measured the speed.
pub const VPOT_ACCELERATION: VPotAcceleration = VPotAcceleration {
    curve: &[
        COARSE,
        3 * COARSE,
        6 * COARSE,
        10 * COARSE,
        15 * COARSE,
        21 * COARSE,
        28 * COARSE,
        36 * COARSE,
    ],
};

impl VPotAcceleration {
    /// The attribute units a reported detent count is worth, sign preserved.
    ///
    /// Zero in, zero out — the surface never sends a zero-magnitude message
    /// (§2.7), and a codec that received one from something else must not have
    /// it turn into movement.
    #[must_use]
    pub fn steps(&self, detents: i8) -> i32 {
        let magnitude = usize::from(detents.unsigned_abs());
        let Some(index) = magnitude.checked_sub(1) else {
            return 0;
        };
        let Some(value) = self.curve.get(index).or_else(|| self.curve.last()) else {
            return 0;
        };
        let steps = i32::from(*value);
        if detents < 0 { -steps } else { steps }
    }
}

/// How the gap between two jog messages becomes a distance to move a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JogAcceleration {
    /// Rows of *at least this long since the last message* and *this many
    /// attribute units per detent*, from the longest interval to the shortest.
    ///
    /// The last row wants an interval of [`Duration::ZERO`] so that every gap
    /// matches something; a table whose rows all missed would silently stop the
    /// wheel.
    pub rows: &'static [(Duration, i16)],
    /// The operator's own multiplier, as a percentage of [`Self::rows`].
    ///
    /// **S59.** The rows are a calibration and a calibration is somebody else's
    /// taste; this is the knob that makes it theirs. It is a **machine**
    /// setting (`MachineSettings::jog_sensitivity`) rather than a show one,
    /// because a heavier hand is a property of the console somebody sits at and
    /// not of the production they are running — a show carried to another hall
    /// on a stick should not take the last operator's wheel feel with it.
    ///
    /// It is a field rather than a second table because the rows are
    /// `&'static` and a scaled table cannot be: multiplying at the point of use
    /// costs one multiply per jog message and keeps the calibration readable as
    /// the numbers that were chosen.
    pub sensitivity: u16,
}

/// The sensitivity of a wheel nobody has adjusted: the table exactly as written.
pub const JOG_SENSITIVITY_DEFAULT: u16 = 100;

/// The slowest a wheel may be made — a tenth of the table.
pub const JOG_SENSITIVITY_MIN: u16 = 10;

/// The fastest a wheel may be made — four times the table.
///
/// At the top the slowest detent is four DMX steps and the fastest sixteen,
/// which is a wheel that crosses a parameter in a flick. Past that it stops
/// being a control and starts being a switch, so the setting is clamped rather
/// than left open.
pub const JOG_SENSITIVITY_MAX: u16 = 400;

/// The default jog curve.
///
/// The wheel reports every detent it passes, so the interval *is* the speed. A
/// deliberate click lands well beyond 40 ms; a fast spin puts messages 10 ms
/// apart or less.
///
/// # The numbers are DMX steps now — S59
///
/// S43 sized this table in percent-of-a-full-turn, on the owner's measurement
/// that a full turn was worth about 1 % and their answer that it should be
/// about 20 %. That worked and it was still too slow, and the arithmetic says
/// why: twenty attribute units is **one thirteenth of one DMX step**, so on an
/// ordinary 8-bit channel a dozen detents passed before the lamp did anything at
/// all. The wheel was not slow, it was *dead* — and a control that does nothing
/// for its first twelve clicks reads as broken however fast it is afterwards.
///
/// So the table is written in the unit that decides whether anything visible
/// happens. **The slowest detent is exactly one DMX step** ([`COARSE`]), which
/// is the smallest move the least precise fixture in a rig can make: every click
/// of the wheel moves the light. The fastest is **four**, the owner's answer of
/// 2026-09-20, and the two middle rows are the geometric mean steps between
/// them — ∛4 ≈ 1.587 apart, so the wheel gathers speed evenly rather than in a
/// jump.
///
/// # What is still not measured, and is owed
///
/// **How many detents a full revolution has has never been counted.** Every
/// percentage-per-turn this module has ever claimed was back-calculated from a
/// value the owner watched move, which makes it an estimate wearing a decimal
/// point. The numbers here do not depend on it — a DMX step is a DMX step — but
/// any sentence of the form *a full turn is worth X %* does, so this module no
/// longer contains one.
///
/// **This is a calibration, not a measurement**, and `CLAUDE.md` forbids a test
/// touching the device. What holds it is the ratio, asserted below, the floor
/// being exactly [`COARSE`], and [`JogAcceleration::sensitivity`] — which is
/// the admission that the last word belongs to a hand on a real wheel.
pub const JOG_ACCELERATION: JogAcceleration = JogAcceleration {
    rows: &[
        (Duration::from_millis(40), COARSE),
        (Duration::from_millis(20), 408),
        (Duration::from_millis(10), 648),
        (Duration::ZERO, 4 * COARSE),
    ],
    sensitivity: JOG_SENSITIVITY_DEFAULT,
};

impl JogAcceleration {
    /// The attribute units one jog message is worth.
    ///
    /// `since` is the time since the previous jog message, or `None` for the
    /// first one after a pause — which is the slowest row, because a wheel that
    /// has been standing still is being started rather than spun.
    #[must_use]
    pub fn steps(&self, detents: i8, since: Option<Duration>) -> i32 {
        if detents == 0 {
            return 0;
        }
        let Some(gap) = since else {
            return self.scaled(i32::from(detents) * i32::from(self.slowest()));
        };
        let factor = self
            .rows
            .iter()
            .find(|(interval, _)| gap >= *interval)
            .map_or_else(|| self.slowest(), |(_, steps)| *steps);
        self.scaled(i32::from(detents) * i32::from(factor))
    }

    /// [`Self::sensitivity`] applied — S59.
    ///
    /// **A turned wheel never answers nought.** The division would swallow a
    /// single slow detent at a low enough setting, and a wheel that sometimes
    /// does nothing is the fault this session set out to remove rather than a
    /// milder version of it. So the sign is kept and the magnitude floors at
    /// one: at the slowest setting the wheel is fine, not broken.
    #[allow(
        clippy::integer_division,
        reason = "a percentage of a step count is a ratio, and the remainder is                   smaller than the parameter unit it would be spent on"
    )]
    fn scaled(&self, steps: i32) -> i32 {
        if steps == 0 {
            return 0;
        }
        let scaled = i64::from(steps) * i64::from(self.sensitivity) / 100;
        let clamped = i32::try_from(scaled).unwrap_or(if steps < 0 { i32::MIN } else { i32::MAX });
        if clamped == 0 {
            if steps < 0 { -1 } else { 1 }
        } else {
            clamped
        }
    }

    /// The factor for a wheel that is barely moving: the first row's, or one
    /// attribute unit if the table is empty.
    fn slowest(&self) -> i16 {
        self.rows.first().map_or(1, |(_, steps)| *steps)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        COARSE, JOG_ACCELERATION, JOG_SENSITIVITY_DEFAULT, JOG_SENSITIVITY_MIN, JogAcceleration,
        VPOT_ACCELERATION, VPotAcceleration,
    };
    use std::time::Duration;

    /// The floor of both curves, and **S59 made them the same floor**.
    ///
    /// S43 changed what the floor *is* — from one part in 65 535, which is what
    /// punch-list B20 reported from the other end of the cable, to the smallest
    /// move an operator can see. It left the wheel deliberately finer than the
    /// V-Pot, on the reasoning that the wheel is the one that trims, and that is
    /// the sentence S59 had to take back: a thirteenth of a DMX step is not
    /// *fine*, it is *nothing*, twelve times over. Both curves now start at one
    /// coarse step, and a single click of either moves the least precise fixture
    /// in a rig.
    #[test]
    fn one_detent_is_a_move_that_can_be_seen_on_both_curves() {
        assert_eq!(VPOT_ACCELERATION.steps(1), i32::from(COARSE));
        assert_eq!(VPOT_ACCELERATION.steps(-1), -i32::from(COARSE));
        assert_eq!(
            JOG_ACCELERATION.steps(1, Some(Duration::from_millis(500))),
            i32::from(COARSE)
        );
        assert_eq!(
            JOG_ACCELERATION.steps(-1, Some(Duration::from_millis(500))),
            -i32::from(COARSE)
        );
    }

    #[test]
    fn the_v_pot_curve_reads_the_magnitude_the_desk_measured() {
        // §2.7: the desk sends 1...8 and the curve turns each into steps. Held
        // as the whole table rather than as "it grows", because the numbers are
        // what a person tunes when the desk feels wrong.
        let expected = [1, 3, 6, 10, 15, 21, 28, 36];
        for (detents, want) in (1..=8i8).zip(expected) {
            let want = want * i32::from(COARSE);
            assert_eq!(VPOT_ACCELERATION.steps(detents), want, "{detents} detents");
            assert_eq!(VPOT_ACCELERATION.steps(-detents), -want);
        }
    }

    #[test]
    fn the_v_pot_curve_is_monotone_so_a_faster_turn_never_does_less() {
        // The property that survives somebody retuning the table above.
        let mut previous = 0;
        for detents in 1..=8i8 {
            let steps = VPOT_ACCELERATION.steps(detents);
            assert!(
                steps > previous,
                "{detents} detents did not exceed {previous}"
            );
            previous = steps;
        }
    }

    #[test]
    fn a_magnitude_past_the_table_saturates_rather_than_starting_again() {
        // The X-Touch stops at 8; another MCU surface might not. Falling back to
        // one step would make the fastest possible turn the slowest.
        let top = 36 * i32::from(COARSE);
        assert_eq!(VPOT_ACCELERATION.steps(9), top);
        assert_eq!(VPOT_ACCELERATION.steps(63), top);
        assert_eq!(VPOT_ACCELERATION.steps(-63), -top);
    }

    #[test]
    fn no_movement_is_no_movement_on_both_curves() {
        // 0x40 decodes to a signed zero (`control.rs`), and a curve that turned
        // it into a step would make a V-Pot drift while nobody touched it.
        assert_eq!(VPOT_ACCELERATION.steps(0), 0);
        assert_eq!(JOG_ACCELERATION.steps(0, Some(Duration::ZERO)), 0);
        assert_eq!(JOG_ACCELERATION.steps(0, None), 0);
    }

    #[test]
    fn the_jog_curve_reads_the_clock_because_the_wheel_reports_nothing_else() {
        // §2.7: +-1 in 404 messages however hard it was spun. Every row of the
        // table, on both sides of its boundary.
        for (gap, want) in [
            (500u64, 257),
            (41, 257),
            (40, 257),
            (39, 408),
            (21, 408),
            (20, 408),
            (19, 648),
            (11, 648),
            (10, 648),
            (9, 1028),
            (1, 1028),
            (0, 1028),
        ] {
            assert_eq!(
                JOG_ACCELERATION.steps(1, Some(Duration::from_millis(gap))),
                want,
                "{gap} ms"
            );
        }
    }

    #[test]
    fn the_first_message_after_a_pause_is_a_click_and_not_a_spin() {
        // There is no interval to read, and guessing "fast" would make the first
        // detent of every touch of the wheel jump eight rows.
        assert_eq!(JOG_ACCELERATION.steps(1, None), i32::from(COARSE));
        assert_eq!(JOG_ACCELERATION.steps(-1, None), -i32::from(COARSE));
    }

    #[test]
    fn a_curve_with_no_rows_still_moves_by_one_and_never_panics() {
        // The crate denies panicking outside its tests, so both curves have to
        // answer something for a profile somebody half-filled in.
        let empty = VPotAcceleration { curve: &[] };
        assert_eq!(empty.steps(4), 0);
        let empty = JogAcceleration {
            rows: &[],
            sensitivity: JOG_SENSITIVITY_DEFAULT,
        };
        assert_eq!(empty.steps(1, Some(Duration::from_millis(5))), 1);
        assert_eq!(empty.steps(1, None), 1);
    }

    #[test]
    fn the_jog_rows_run_from_the_longest_interval_to_the_shortest() {
        // `steps` takes the first row it matches, so an unsorted table would
        // answer the wrong row for every gap - and quietly, because every answer
        // is a plausible number.
        let mut previous: Option<Duration> = None;
        for (interval, _) in JOG_ACCELERATION.rows {
            if let Some(last) = previous {
                assert!(*interval < last, "{interval:?} follows {last:?}");
            }
            previous = Some(*interval);
        }
        assert_eq!(
            JOG_ACCELERATION.rows.last().map(|(interval, _)| *interval),
            Some(Duration::ZERO),
            "the last row has to match every gap"
        );
    }

    #[test]
    fn the_two_curves_are_not_the_same_function() {
        // The finding, as a test: one reads a magnitude and one reads a clock,
        // so a jog message can be worth eight steps while carrying a magnitude
        // the V-Pot curve would score as one.
        let fast_jog = JOG_ACCELERATION.steps(1, Some(Duration::from_millis(5)));
        let same_magnitude_on_a_pot = VPOT_ACCELERATION.steps(1);
        assert_eq!(fast_jog, 4 * i32::from(COARSE));
        assert_eq!(same_magnitude_on_a_pot, i32::from(COARSE));
    }

    /// **The floor is one DMX step, and that is the whole of S59's fix.**
    ///
    /// The wheel was not slow, it was dead: at twenty attribute units a detent
    /// moved one thirteenth of a step on an 8-bit channel, so a dozen clicks
    /// changed nothing an operator could see. This is the assertion that would
    /// go red if anybody sized the table in percent again — a slow detent moves
    /// the least precise fixture in a rig by exactly one step, no less.
    #[test]
    fn the_slowest_detent_moves_a_coarse_channel_by_exactly_one_step() {
        let click = JOG_ACCELERATION.steps(1, Some(Duration::from_millis(500)));
        assert_eq!(click, i32::from(COARSE));
    }

    /// And the spread is fourfold, which is the owner's answer of 2026-09-20.
    ///
    /// It was eightfold while the floor was a thirteenth of a step; raising the
    /// floor without lowering the spread would have made a spin worth eight DMX
    /// steps a detent, which crosses a parameter faster than a hand can stop.
    #[test]
    fn the_wheel_answers_fourfold_between_a_click_and_a_spin() {
        let click = JOG_ACCELERATION.steps(1, Some(Duration::from_millis(500)));
        let spin = JOG_ACCELERATION.steps(1, Some(Duration::ZERO));
        assert_eq!(spin, click * 4);
        assert_eq!(spin, 4 * i32::from(COARSE));
    }

    /// The rows gather speed evenly rather than in a jump.
    ///
    /// Geometric, ∛4 ≈ 1.587 apart, so no boundary between two rows is felt
    /// more than any other. A table that went 257, 257, 257, 1028 would satisfy
    /// both tests above and feel like a switch.
    #[test]
    fn the_rows_climb_evenly() {
        let steps: Vec<i32> = JOG_ACCELERATION
            .rows
            .iter()
            .map(|(_, steps)| i32::from(*steps))
            .collect();
        assert_eq!(steps, vec![257, 408, 648, 1028]);
        for pair in steps.windows(2) {
            // Each row is between 1.5 and 1.7 times the one before it.
            assert!(pair[1] * 10 >= pair[0] * 15, "{pair:?}");
            assert!(pair[1] * 10 <= pair[0] * 17, "{pair:?}");
        }
    }

    /// The operator's own multiplier — S59.
    #[test]
    fn sensitivity_scales_the_whole_curve() {
        let heavy = JogAcceleration {
            sensitivity: 200,
            ..JOG_ACCELERATION
        };
        let light = JogAcceleration {
            sensitivity: 50,
            ..JOG_ACCELERATION
        };
        let click = |curve: &JogAcceleration| curve.steps(1, Some(Duration::from_millis(500)));
        assert_eq!(click(&JOG_ACCELERATION), i32::from(COARSE));
        assert_eq!(click(&heavy), 2 * i32::from(COARSE));
        assert_eq!(click(&light), i32::from(COARSE) / 2);
        // The sign survives, which a scaling written as a `u32` multiply would
        // have lost on the way down.
        assert_eq!(
            heavy.steps(-1, Some(Duration::from_millis(500))),
            -2 * i32::from(COARSE)
        );
    }

    /// **A turned wheel never answers nought**, whatever the setting.
    ///
    /// The division would swallow a single detent at the bottom of the range,
    /// and a wheel that sometimes does nothing is the fault this session
    /// removed rather than a milder version of it.
    #[test]
    fn the_slowest_setting_still_moves_the_parameter() {
        let crawl = JogAcceleration {
            rows: &[(Duration::ZERO, 1)],
            sensitivity: JOG_SENSITIVITY_MIN,
        };
        assert_eq!(crawl.steps(1, Some(Duration::ZERO)), 1);
        assert_eq!(crawl.steps(-1, Some(Duration::ZERO)), -1);
        // A wheel that is not turning is still not turning.
        assert_eq!(crawl.steps(0, Some(Duration::ZERO)), 0);
    }
}
