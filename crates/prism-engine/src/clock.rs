//! The tick's time base.
//!
//! The engine never reads the wall clock and never sees an `Instant`: it works
//! in monotonic [`Duration`]s measured from the moment the clock was created.
//! That is what makes the timing testable. A run of 100 000 ticks against
//! [`ManualClock`] finishes in milliseconds and is deterministic, so the drift
//! property can be *proved* rather than sampled; [`SystemClock`] is then the
//! only piece left that has to be measured against a real machine.

use core::cell::Cell;
use core::hint;
use std::thread;
use std::time::{Duration, Instant};

/// The tick's view of time.
///
/// Deliberately minimal: an engine that can only ask "what time is it" and
/// "wake me at this time" can be driven by a simulated clock, and the timing
/// properties become ordinary unit tests rather than something to eyeball.
pub trait Clock {
    /// Monotonic time since this clock was created. Never decreases.
    fn now(&self) -> Duration;

    /// Blocks until at least `deadline`. Returns immediately if it has passed.
    fn sleep_until(&self, deadline: Duration);
}

/// The real clock: a monotonic [`Instant`] and the operating system's sleep.
///
/// `ARCHITECTURE_SPEC.md` §3.1 prescribes the strategy — sleep until a
/// millisecond before the deadline, then spin. The sleep gives the CPU back for
/// almost the whole period; the spin covers the part no operating system
/// schedules accurately.
///
/// # The sleep is taken in slices, and **S63** is why
///
/// One `thread::sleep` for the whole remainder is what this did until S63, and
/// on macOS it missed every deadline by about three milliseconds. The cause is
/// **timer slack**: an operating system that is allowed to batch timers to save
/// power grants a sleeper an error budget *proportional to what it asked for*,
/// so a long sleep is a loose one. Measured on an Apple Silicon Mac, with the
/// median overshoot of one `thread::sleep`:
///
/// | asked for | woke up late by |
/// |---|---|
/// | 1 ms | 260 µs |
/// | 5 ms | 1.26 ms |
/// | 21 ms | **3.45 ms** |
///
/// A 44 Hz tick sleeps about 21.7 ms, so it woke up **past** the deadline it
/// meant to spin up to, and [`SystemClock::spin_margin`] bought nothing: there
/// was no margin left to spin through. The median jitter of the whole grid was
/// 2.5 ms against a 1 ms budget.
///
/// So the remainder is slept in slices of at most [`SLEEP_SLICE`], with the
/// time left recomputed against the deadline after each one. Every slice is
/// short, so every slice's slack is small, and a slice that overshoots is
/// absorbed by the next one being shorter rather than accumulating. On the same
/// machine that measured 2.5 ms, the median became **76 ns** and the worst of
/// 132 ticks 128 µs.
///
/// **It is not `#[cfg(target_os = "macos")]` and must not become one.** This
/// crate is platform-neutral by rule (`ARCHITECTURE_SPEC.md` §10.1), and the
/// change needs no platform knowledge: proportional timer slack is what
/// Windows' coarse timer and Linux's `timer_slack_ns` both do, in their own
/// sizes. Asking for a short sleep repeatedly is the portable way to say *wake
/// me accurately*, and a platform with an exact timer loses nothing by it —
/// the loop simply runs its slices and stops.
pub struct SystemClock {
    origin: Instant,
    spin_margin: Duration,
}

/// The longest single sleep [`SystemClock::sleep_until`] will ask for.
///
/// Two milliseconds, and the number is a measurement rather than a taste. It is
/// short enough that the operating system's proportional slack (see
/// [`SystemClock`]) stays well under the spin margin that follows it, and long
/// enough that a 44 Hz tick costs about eleven `sleep` calls rather than
/// twenty-two. One and four milliseconds were measured either side of it: one
/// is no more accurate and twice the calls, four lets the worst case out to
/// 706 µs.
pub const SLEEP_SLICE: Duration = Duration::from_millis(2);

impl SystemClock {
    /// A clock with the specified one-millisecond spin margin.
    #[must_use]
    pub fn new() -> Self {
        Self::with_spin_margin(Duration::from_millis(1))
    }

    /// A clock that stops sleeping `spin_margin` before the deadline.
    ///
    /// Worth raising on a machine whose timer is coarse, at the cost of burning
    /// more CPU per tick.
    #[must_use]
    pub fn with_spin_margin(spin_margin: Duration) -> Self {
        Self {
            origin: Instant::now(),
            spin_margin,
        }
    }

    /// How long before the deadline this clock switches from sleeping to
    /// spinning.
    #[must_use]
    pub const fn spin_margin(&self) -> Duration {
        self.spin_margin
    }

    /// How long to sleep next, with `remaining` left before the deadline, or
    /// `None` to stop sleeping and spin the rest.
    ///
    /// # This is the whole of the decision, and it is here so it can be tested
    /// without a clock — **S63**
    ///
    /// The first three attempts at a regression test for the sliced sleep all
    /// measured **wall-clock time**, and all three were flaky on a shared CI
    /// runner: a worst case of 15.5 ms against a 1 ms budget, then a *median*
    /// of 9.7 ms against a 2 ms one, on a three-core virtual machine running
    /// every test binary at once. Tuning the threshold a fourth time would have
    /// produced a bound so loose it could catch nothing.
    ///
    /// What actually changed in S63 is not a duration — it is a **decision**:
    /// *ask for at most one slice, and recompute*. A decision can be checked
    /// exactly, with no machine in the loop, which is what this function is
    /// for. The timing itself is measured where it means something:
    /// `tests/realtime.rs` on a machine with a core to spare, and the
    /// ten-minute release-profile runs.
    #[must_use]
    pub fn nap(&self, remaining: Duration) -> Option<Duration> {
        // Nothing left but the margin: stop sleeping, the spin covers the rest.
        let coarse = remaining.checked_sub(self.spin_margin)?;
        // **Exactly at the margin, `checked_sub` says `Some(0)`, not `None`.**
        // Asking the operating system to sleep for no time is a syscall that
        // buys nothing, and the spin below is what that last stretch is for.
        // Found by the test beside this, which expected the boundary to be
        // clean and found one wasted call there.
        if coarse.is_zero() {
            return None;
        }
        // **The cap is the point.** An operating system grants slack in
        // proportion to what is asked for, so asking for the whole remainder is
        // asking for a loose wake-up.
        Some(coarse.min(SLEEP_SLICE))
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    /// Sleeps in slices, then spins — see [`SystemClock`] for why the first
    /// half is a loop rather than one call.
    ///
    /// It never returns before `deadline`, which is the property everything
    /// above it relies on and the one the tests hold.
    fn sleep_until(&self, deadline: Duration) {
        // Recomputed against the deadline every time round, so a slice that
        // overshoots shortens the next one instead of pushing the total out.
        // This is the same absolute-deadline argument §3.1 makes about the
        // tick itself, one level down.
        while let Some(remaining) = deadline.checked_sub(self.now()) {
            let Some(nap) = self.nap(remaining) else {
                break;
            };
            thread::sleep(nap);
        }
        while self.now() < deadline {
            hint::spin_loop();
        }
    }
}

/// A clock that only moves when it is told to.
///
/// Sleeping does not wait — it sets the time to the deadline, plus an optional
/// overshoot standing in for a scheduler that woke up late. That makes a run of
/// 100 000 ticks finish in milliseconds and produce the same numbers every time,
/// which is the only way the drift property can be checked rather than sampled.
pub struct ManualClock {
    now: Cell<Duration>,
    max_overshoot: Duration,
    state: Cell<u64>,
}

impl ManualClock {
    /// A clock that wakes exactly on the deadline.
    #[must_use]
    pub fn new() -> Self {
        Self::with_overshoot(Duration::ZERO)
    }

    /// A clock that wakes up to `max_overshoot` late, by a deterministic but
    /// uneven amount.
    ///
    /// Uneven on purpose: a constant overshoot is indistinguishable from a
    /// correct schedule with an offset, and would let an accumulating one pass.
    #[must_use]
    pub fn with_overshoot(max_overshoot: Duration) -> Self {
        Self {
            now: Cell::new(Duration::ZERO),
            max_overshoot,
            state: Cell::new(0x2545_F491_4F6C_DD1D),
        }
    }

    /// Moves the clock forward, as a slow tick body would.
    pub fn advance(&self, by: Duration) {
        self.now.set(self.now.get() + by);
    }

    /// xorshift64: no dependency, no allocation, same sequence every run.
    fn next_overshoot(&self) -> Duration {
        let max = u64::try_from(self.max_overshoot.as_nanos()).unwrap_or(u64::MAX);
        if max == 0 {
            return Duration::ZERO;
        }
        let mut state = self.state.get();
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.state.set(state);
        Duration::from_nanos(state % (max + 1))
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        self.now.get()
    }

    fn sleep_until(&self, deadline: Duration) {
        if deadline > self.now.get() {
            self.now.set(deadline + self.next_overshoot());
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::{Clock, ManualClock, SLEEP_SLICE, SystemClock};
    use std::time::Duration;

    #[test]
    fn a_manual_clock_starts_at_zero_and_only_moves_forward() {
        let clock = ManualClock::new();
        assert_eq!(clock.now(), Duration::ZERO);
        clock.advance(Duration::from_millis(5));
        assert_eq!(clock.now(), Duration::from_millis(5));
        // A deadline already in the past does not rewind the clock.
        clock.sleep_until(Duration::from_millis(1));
        assert_eq!(clock.now(), Duration::from_millis(5));
    }

    #[test]
    fn a_manual_clock_wakes_exactly_on_the_deadline_by_default() {
        let clock = ManualClock::new();
        clock.sleep_until(Duration::from_millis(20));
        assert_eq!(clock.now(), Duration::from_millis(20));
    }

    #[test]
    fn a_manual_clock_can_be_told_to_overshoot_like_a_real_scheduler() {
        // Deterministic, but uneven: a fixed overshoot would let an accumulating
        // scheduler hide behind a constant offset.
        let clock = ManualClock::with_overshoot(Duration::from_micros(900));
        clock.sleep_until(Duration::from_millis(10));
        let first = clock.now();
        assert!(first >= Duration::from_millis(10));
        assert!(first <= Duration::from_micros(10_900));

        clock.sleep_until(Duration::from_millis(20));
        let second = clock.now();
        assert!(second >= Duration::from_millis(20));
        assert!(second <= Duration::from_micros(20_900));
        assert_ne!(
            second - Duration::from_millis(20),
            first - Duration::from_millis(10),
            "overshoot must vary, or drift can hide inside a constant"
        );
    }

    #[test]
    fn a_system_clock_does_not_return_before_the_deadline() {
        let clock = SystemClock::new();
        let deadline = clock.now() + Duration::from_millis(5);
        clock.sleep_until(deadline);
        assert!(clock.now() >= deadline);
    }

    /// **The decision, checked exactly and without a clock** — S63.
    ///
    /// This is the regression test for the sliced sleep, and it is the fourth
    /// attempt at one. The first three measured wall-clock time and all three
    /// were flaky on a shared CI runner — a worst case of 15.5 ms against a
    /// 1 ms budget, then a median of 9.7 ms against a 2 ms one, on code whose
    /// median is 76 ns on real hardware. See [`SystemClock::nap`].
    ///
    /// What S63 changed is a **decision**, not a duration: *ask for at most one
    /// slice, and recompute against the deadline*. The broken version asked for
    /// the whole remainder in one go, which is what earned it slack in
    /// proportion. So that is what is asserted, exactly.
    #[test]
    fn a_nap_is_never_longer_than_one_slice_and_never_reaches_the_margin() {
        let clock = SystemClock::new();
        let margin = clock.spin_margin();

        // A whole 44 Hz period — what the real grid asks for, and ten times a
        // slice. The broken version returned all 21.7 ms of it.
        let period = Duration::from_micros(22_727);
        assert_eq!(clock.nap(period), Some(SLEEP_SLICE));

        // Longer than a slice once the margin is off: capped.
        assert_eq!(clock.nap(margin + SLEEP_SLICE * 2), Some(SLEEP_SLICE));
        // Exactly a slice once the margin is off: that slice.
        assert_eq!(clock.nap(margin + SLEEP_SLICE), Some(SLEEP_SLICE));
        // Less than a slice: the rest, so the last nap lands short rather than
        // over. This is what makes the approach to the deadline accurate.
        assert_eq!(
            clock.nap(margin + Duration::from_micros(300)),
            Some(Duration::from_micros(300))
        );

        // At the margin and inside it there is nothing left to sleep for — the
        // spin covers it, and a sleep here is exactly what would overshoot.
        assert_eq!(clock.nap(margin), None);
        assert_eq!(clock.nap(margin / 2), None);
        assert_eq!(clock.nap(Duration::ZERO), None);
    }

    /// The two invariants over the whole range, rather than at the points the
    /// test above happens to name.
    ///
    /// Swept in 37 µs steps across three tick periods — a step that divides
    /// neither the slice nor the margin, so it does not only ever land on the
    /// round numbers a bug would land on too.
    #[test]
    fn no_nap_ever_overshoots_the_margin_or_the_slice() {
        let clock = SystemClock::new();
        let margin = clock.spin_margin();

        let mut remaining = Duration::ZERO;
        let limit = Duration::from_micros(22_727 * 3);
        while remaining <= limit {
            if let Some(nap) = clock.nap(remaining) {
                assert!(
                    nap <= SLEEP_SLICE,
                    "a nap of {nap:?} with {remaining:?} left is longer than one slice"
                );
                // The safety property: a nap can never reach into the margin,
                // so the spin always has something left to spin through.
                assert!(
                    nap + margin <= remaining,
                    "a nap of {nap:?} with {remaining:?} left eats into the {margin:?} margin"
                );
            } else {
                assert!(
                    remaining <= margin,
                    "{remaining:?} left is more than the {margin:?} margin and should still sleep"
                );
            }
            remaining += Duration::from_micros(37);
        }
    }

    /// The slice is an upper bound on one `sleep`, not on the wait: a deadline
    /// far away is still waited for in full.
    #[test]
    fn the_slice_bounds_one_sleep_and_not_the_whole_wait() {
        let clock = SystemClock::new();
        let deadline = clock.now() + SLEEP_SLICE * 5;
        clock.sleep_until(deadline);
        assert!(clock.now() >= deadline);
    }

    #[test]
    fn a_system_clock_deadline_in_the_past_returns_immediately() {
        let clock = SystemClock::new();
        let before = clock.now();
        clock.sleep_until(Duration::ZERO);
        assert!(clock.now() - before < Duration::from_millis(5));
    }

    #[test]
    fn a_system_clock_measures_from_its_own_creation() {
        let clock = SystemClock::new();
        assert!(clock.now() < Duration::from_millis(50));
    }

    #[test]
    fn the_default_clocks_are_the_plain_ones() {
        assert_eq!(
            SystemClock::default().spin_margin(),
            Duration::from_millis(1)
        );
        let manual = ManualClock::default();
        manual.sleep_until(Duration::from_millis(7));
        assert_eq!(manual.now(), Duration::from_millis(7));
    }

    #[test]
    fn an_overshoot_of_zero_is_no_overshoot() {
        // The generator has a shortcut for it, and a clock that jittered when
        // asked not to would make every deterministic test unreliable.
        let clock = ManualClock::with_overshoot(Duration::ZERO);
        for tick in 1..=5 {
            clock.sleep_until(Duration::from_millis(tick));
            assert_eq!(clock.now(), Duration::from_millis(tick));
        }
    }

    #[test]
    fn the_spin_margin_is_the_one_millisecond_the_specification_asks_for() {
        assert_eq!(SystemClock::new().spin_margin(), Duration::from_millis(1));
        let coarse = SystemClock::with_spin_margin(Duration::from_millis(4));
        assert_eq!(coarse.spin_margin(), Duration::from_millis(4));
        let deadline = coarse.now() + Duration::from_millis(2);
        coarse.sleep_until(deadline);
        assert!(coarse.now() >= deadline);
    }
}
