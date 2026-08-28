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
}

/// The default jog curve.
///
/// The wheel reports every detent it passes, so the interval *is* the speed. A
/// deliberate click lands well beyond 40 ms; a fast spin puts messages 10 ms
/// apart or less, and the spread between the two rows is eightfold — which is
/// the punch list's *the rate scales with how fast the wheel is turned*.
///
/// # Where the four numbers come from — S43, punch-list B20
///
/// The owner measured the wheel: **a full turn moved a value by about 1 %**, and
/// asked that a fast full turn be worth **about 20 %**. Those two numbers are
/// enough to size the table without counting detents anywhere: the ratio is
/// twenty, so every row is what it was times twenty. The shape — the eightfold
/// spread, the interval boundaries S21's clock reads — is unchanged, because
/// the shape was never what was wrong.
///
/// The slowest row is [`COARSE`] / 13, deliberately finer than the V-Pot's
/// smallest move: the wheel is the control an operator reaches for to *trim*,
/// and a full slow turn is about 2.5 %.
///
/// **This is a calibration, not a measurement**, and it is the one number in
/// this module that wants a real wheel under a real hand to confirm. `CLAUDE.md`
/// forbids a test touching the device, so what holds it is the ratio, asserted
/// below, and the round of hand-testing recorded in `PROGRESS.md` §2.41.
pub const JOG_ACCELERATION: JogAcceleration = JogAcceleration {
    rows: &[
        (Duration::from_millis(40), 20),
        (Duration::from_millis(20), 40),
        (Duration::from_millis(10), 80),
        (Duration::ZERO, 160),
    ],
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
    /// attribute unit if the table is empty.
    fn slowest(&self) -> i16 {
        self.rows.first().map_or(1, |(_, steps)| *steps)
    }
}

#[cfg(test)]
mod tests {
    use super::{COARSE, JOG_ACCELERATION, JogAcceleration, VPOT_ACCELERATION, VPotAcceleration};
    use std::time::Duration;

    /// The floor of both curves, and **S43 changed what the floor is**.
    ///
    /// This test used to say *one detent is one step* and mean one part in
    /// 65 535, which is what punch-list B20 was reporting from the other end of
    /// the cable: a control that could not move a lamp. The claim it makes now
    /// is the same claim about a different number — a single click is the
    /// smallest move an operator can *see*, which on a coarse channel is
    /// [`COARSE`], and the wheel's slowest row is deliberately finer than that
    /// because the wheel is the one that trims.
    #[test]
    fn one_detent_is_a_move_that_can_be_seen_on_both_curves() {
        assert_eq!(VPOT_ACCELERATION.steps(1), i32::from(COARSE));
        assert_eq!(VPOT_ACCELERATION.steps(-1), -i32::from(COARSE));
        assert_eq!(
            JOG_ACCELERATION.steps(1, Some(Duration::from_millis(500))),
            20
        );
        assert_eq!(
            JOG_ACCELERATION.steps(-1, Some(Duration::from_millis(500))),
            -20
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
            (500u64, 20),
            (41, 20),
            (40, 20),
            (39, 40),
            (21, 40),
            (20, 40),
            (19, 80),
            (11, 80),
            (10, 80),
            (9, 160),
            (1, 160),
            (0, 160),
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
        assert_eq!(JOG_ACCELERATION.steps(1, None), 20);
        assert_eq!(JOG_ACCELERATION.steps(-1, None), -20);
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
        assert_eq!(fast_jog, 160);
        assert_eq!(same_magnitude_on_a_pot, i32::from(COARSE));
    }

    /// **The punch list's two numbers, as arithmetic** — S43, B20.
    ///
    /// The owner measured a full turn of the wheel at about 1 % and asked for
    /// about 20 % when it is spun. No test can count the detents in a
    /// revolution without a wheel to turn, and `CLAUDE.md` forbids a test that
    /// opens the device — so what is held here is the part that *is* knowable
    /// from the two numbers: every row is twenty times the row it replaced, so
    /// whatever a revolution is worth, it is worth twenty times what the owner
    /// measured. The detent count cancels out, which is the whole reason the
    /// table could be sized from a report rather than from a measurement.
    #[test]
    fn every_jog_row_is_twenty_times_the_row_the_owner_measured() {
        let measured = [1i16, 2, 4, 8];
        assert_eq!(JOG_ACCELERATION.rows.len(), measured.len());
        for (row, before) in JOG_ACCELERATION.rows.iter().zip(measured) {
            assert_eq!(row.1, before * 20, "{:?}", row.0);
        }
    }

    /// And the spread is untouched, because the spread was never what was wrong.
    ///
    /// *The rate scales with how fast the wheel is turned* is the entry's own
    /// wording, and it was already true — eightfold between the slowest row and
    /// the fastest. A retune that flattened it would satisfy `every_jog_row_is_
    /// twenty_times…` and lose the thing the operator actually asked for.
    #[test]
    fn the_wheel_still_answers_eightfold_between_a_click_and_a_spin() {
        let click = JOG_ACCELERATION.steps(1, Some(Duration::from_millis(500)));
        let spin = JOG_ACCELERATION.steps(1, Some(Duration::ZERO));
        assert_eq!(spin, click * 8);
    }
}
