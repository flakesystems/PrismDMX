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
pub struct SystemClock {
    origin: Instant,
    spin_margin: Duration,
}

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

    fn sleep_until(&self, deadline: Duration) {
        let Some(remaining) = deadline.checked_sub(self.now()) else {
            return;
        };
        if let Some(coarse) = remaining.checked_sub(self.spin_margin) {
            thread::sleep(coarse);
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
    use super::{Clock, ManualClock, SystemClock};
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
