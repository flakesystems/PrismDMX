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
//! # These numbers are taste, and they are data
//!
//! The measurements above are facts about the desk. How many parameter steps a
//! detent is worth is a decision about how a desk should feel, so both curves
//! are values a profile or a settings screen can replace rather than constants
//! compiled into the translation.

use std::time::Duration;

/// How the magnitude a V-Pot reports becomes a number of parameter steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VPotAcceleration {
    /// Steps for a reported magnitude of 1, 2, 3 … in order.
    ///
    /// A magnitude past the end of the table saturates on the last entry rather
    /// than falling back to one step: a surface that reported more than this
    /// table describes was turned *faster*, and answering "one step" would make
    /// a hard spin do less than a slow one.
    pub curve: &'static [i16],
}

/// The default V-Pot curve: the triangular numbers.
///
/// One detent is one step, and each further detent in the same message is worth
/// one more than the last — 1, 3, 6, 10, 15, 21, 28, 36 across the 1…8 the desk
/// actually sends (§2.7). A full-speed sweep therefore covers thirty-six times
/// what a careful click does, which is the ratio that lets one encoder both trim
/// a value by hand and cross a 16-bit parameter without an operator winding it
/// like a fishing reel.
///
/// Linear would be the obvious alternative and is the one to avoid: it makes the
/// fastest turn eight times the slowest, and eight is not enough range to be
/// worth the surface having measured the speed.
pub const VPOT_ACCELERATION: VPotAcceleration = VPotAcceleration {
    curve: &[1, 3, 6, 10, 15, 21, 28, 36],
};

impl VPotAcceleration {
    /// The parameter steps a reported detent count is worth, sign preserved.
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

/// How the gap between two jog messages becomes a number of parameter steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JogAcceleration {
    /// Rows of *at least this long since the last message* and *this many steps
    /// per detent*, from the longest interval to the shortest.
    ///
    /// The last row wants an interval of [`Duration::ZERO`] so that every gap
    /// matches something; a table whose rows all missed would silently stop the
    /// wheel.
    pub rows: &'static [(Duration, i16)],
}

/// The default jog curve.
///
/// The wheel reports every detent it passes, so the interval *is* the speed. A
/// deliberate click lands well beyond 40 ms and is worth one step; a fast spin
/// puts messages 10 ms apart or less and is worth eight, which is the same
/// ceiling the V-Pot curve reaches by a different route.
pub const JOG_ACCELERATION: JogAcceleration = JogAcceleration {
    rows: &[
        (Duration::from_millis(40), 1),
        (Duration::from_millis(20), 2),
        (Duration::from_millis(10), 4),
        (Duration::ZERO, 8),
    ],
};

impl JogAcceleration {
    /// The parameter steps one jog message is worth.
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
            return i32::from(detents) * i32::from(self.slowest());
        };
        let factor = self
            .rows
            .iter()
            .find(|(interval, _)| gap >= *interval)
            .map_or_else(|| self.slowest(), |(_, steps)| *steps);
        i32::from(detents) * i32::from(factor)
    }

    /// The factor for a wheel that is barely moving: the first row's, or one
    /// step if the table is empty.
    fn slowest(&self) -> i16 {
        self.rows.first().map_or(1, |(_, steps)| *steps)
    }
}

#[cfg(test)]
mod tests {
    use super::{JOG_ACCELERATION, JogAcceleration, VPOT_ACCELERATION, VPotAcceleration};
    use std::time::Duration;

    #[test]
    fn one_detent_is_one_step_on_both_curves() {
        // The floor is the part an operator feels as precision: whatever the
        // curves do at speed, a single click must move a parameter by one.
        assert_eq!(VPOT_ACCELERATION.steps(1), 1);
        assert_eq!(VPOT_ACCELERATION.steps(-1), -1);
        assert_eq!(
            JOG_ACCELERATION.steps(1, Some(Duration::from_millis(500))),
            1
        );
        assert_eq!(
            JOG_ACCELERATION.steps(-1, Some(Duration::from_millis(500))),
            -1
        );
    }

    #[test]
    fn the_v_pot_curve_reads_the_magnitude_the_desk_measured() {
        // §2.7: the desk sends 1...8 and the curve turns each into steps. Held
        // as the whole table rather than as "it grows", because the numbers are
        // what a person tunes when the desk feels wrong.
        let expected = [1, 3, 6, 10, 15, 21, 28, 36];
        for (detents, want) in (1..=8i8).zip(expected) {
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
        assert_eq!(VPOT_ACCELERATION.steps(9), 36);
        assert_eq!(VPOT_ACCELERATION.steps(63), 36);
        assert_eq!(VPOT_ACCELERATION.steps(-63), -36);
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
            (500u64, 1),
            (41, 1),
            (40, 1),
            (39, 2),
            (21, 2),
            (20, 2),
            (19, 4),
            (11, 4),
            (10, 4),
            (9, 8),
            (1, 8),
            (0, 8),
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
        // detent of every touch of the wheel jump eight steps.
        assert_eq!(JOG_ACCELERATION.steps(1, None), 1);
        assert_eq!(JOG_ACCELERATION.steps(-1, None), -1);
    }

    #[test]
    fn a_curve_with_no_rows_still_moves_by_one_and_never_panics() {
        // The crate denies panicking outside its tests, so both curves have to
        // answer something for a profile somebody half-filled in.
        let empty = VPotAcceleration { curve: &[] };
        assert_eq!(empty.steps(4), 0);
        let empty = JogAcceleration { rows: &[] };
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
        assert_eq!(fast_jog, 8);
        assert_eq!(same_magnitude_on_a_pot, 1);
    }
}
