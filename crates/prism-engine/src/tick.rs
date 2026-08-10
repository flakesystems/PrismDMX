//! The 44 Hz heartbeat.
//!
//! One tick, in the order `ARCHITECTURE_SPEC.md` §5 lays down: drain the command
//! queue, let the body evaluate, publish the frame. Steps 2-7 of that pipeline
//! arrive with the merge in S3 and the executors in S5; the seam they plug into
//! is [`TickBody`].
//!
//! # Absolute deadlines
//!
//! The deadline for tick *n* is computed from *n*, never from the previous
//! deadline. `sleep(period)` in a loop adds the scheduler's overshoot to every
//! subsequent tick, and at 44 Hz an average of half a millisecond of overshoot
//! is a whole tick lost every forty-five seconds. Here, an overshoot shortens
//! the following sleep instead, and the grid stays where it was.
//!
//! 1/44 s is not a whole number of nanoseconds, so the deadline is derived as an
//! exact rational of the tick index rather than a rounded period multiplied out:
//! the error is under a nanosecond at every tick and does not accumulate either.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::clock::Clock;
use crate::command::TickCommand;
use crate::frame::DmxFrame;
use crate::spsc::Consumer;
use crate::stats::TickStats;
use crate::triple_buffer::FramePublisher;

/// Ticks per second. `ARCHITECTURE_SPEC.md` §3.2: fixed, and the time base for
/// every fade, phaser and cue in the system.
pub const TICK_HZ: u64 = 44;

/// One tick, nominally — 1/44 s is not a whole number of nanoseconds, so this
/// is the truncated value and is only good enough for budgets and display. The
/// schedule itself uses [`deadline_offset`], which is exact.
pub const TICK_PERIOD: Duration = Duration::from_nanos(1_000_000_000_u64.div_euclid(TICK_HZ));

/// Time from the start of a run to the start of tick `index`.
///
/// Computed from the index as an exact rational rather than by adding a rounded
/// period, so the error is under a nanosecond at every tick and never
/// accumulates. Saturates rather than wrapping, at roughly 584 years.
#[must_use]
pub fn deadline_offset(index: u64) -> Duration {
    let nanos = u128::from(index) * 1_000_000_000;
    Duration::from_nanos(u64::try_from(nanos.div_euclid(u128::from(TICK_HZ))).unwrap_or(u64::MAX))
}

/// Which tick slot a time offset falls in — the inverse of [`deadline_offset`].
#[must_use]
pub fn tick_index_at(elapsed: Duration) -> u64 {
    let ticks = elapsed.as_nanos() * u128::from(TICK_HZ);
    u64::try_from(ticks.div_euclid(1_000_000_000)).unwrap_or(u64::MAX)
}

/// What a tick knows about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickInfo {
    /// Position on the 44 Hz grid since the run started. The time base S5's
    /// fades and cue timings are measured in.
    pub index: u64,
    /// When this tick was supposed to start.
    pub deadline: Duration,
    /// When it actually did.
    pub started: Duration,
    /// Tick slots skipped immediately before this one because the previous tick
    /// overran.
    pub missed: u64,
}

/// The work one tick does, between draining the queue and publishing the frame.
///
/// This is the seam the rest of Phase 1 plugs into: S3 puts the HTP/LTP merge
/// here, S5 the executors and fades, S6 the programmer layer. The engine owns
/// the timing and the hand-off; the body owns the lighting.
///
/// An implementation must obey `ARCHITECTURE_SPEC.md` §3.1 — no allocation, no
/// locks, no I/O, no logging — because it runs on the tick thread.
pub trait TickBody {
    /// Applies one queued command. Called for every command that arrived since
    /// the last tick, in order, before [`render`](Self::render).
    fn apply(&mut self, command: TickCommand);

    /// Writes this tick's output into `frame`.
    fn render(&mut self, tick: &TickInfo, frame: &mut DmxFrame);
}

/// A body that does nothing.
///
/// Useful in two places: the timing tests, where the body must not be what is
/// being measured, and `prismd` before S3 — a daemon that holds a blackout at a
/// steady 44 Hz is a working daemon.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdleBody;

impl TickBody for IdleBody {
    fn apply(&mut self, _command: TickCommand) {}

    fn render(&mut self, _tick: &TickInfo, _frame: &mut DmxFrame) {}
}

/// The tick loop: the heartbeat of the system.
///
/// Owns the command queue's consuming end, the frame publisher and the body,
/// and runs them on an absolute 44 Hz grid. Nothing it does on the tick path
/// allocates, locks or blocks.
pub struct Engine<B> {
    body: B,
    commands: Consumer<TickCommand>,
    publisher: FramePublisher,
    stats: TickStats,
    /// Set on the first tick of the first run, so a paused and resumed engine
    /// stays on the same grid.
    origin: Option<Duration>,
    next_index: u64,
    last_index: u64,
    pending_missed: u64,
}

impl<B: TickBody> Engine<B> {
    /// Assembles an engine from its three parts.
    #[must_use]
    pub fn new(body: B, commands: Consumer<TickCommand>, publisher: FramePublisher) -> Self {
        Self {
            body,
            commands,
            publisher,
            stats: TickStats::new(),
            origin: None,
            next_index: 0,
            last_index: 0,
            pending_missed: 0,
        }
    }

    /// Runs until `stop` is set. The flag is read once per tick.
    pub fn run<C: Clock>(&mut self, clock: &C, stop: &AtomicBool) {
        while !stop.load(Ordering::Relaxed) {
            self.step(clock);
        }
    }

    /// Runs exactly `ticks` ticks. Missed slots are not among them: a tick that
    /// never ran is not a tick.
    pub fn run_ticks<C: Clock>(&mut self, clock: &C, ticks: u64) {
        for _ in 0..ticks {
            self.step(clock);
        }
    }

    /// Waits for the next deadline and runs one tick.
    fn step<C: Clock>(&mut self, clock: &C) {
        let origin = *self.origin.get_or_insert_with(|| clock.now());
        let index = self.next_index;
        let deadline = origin + deadline_offset(index);
        clock.sleep_until(deadline);

        let started = clock.now();
        self.stats.jitter.record(started.saturating_sub(deadline));
        let info = TickInfo {
            index,
            deadline,
            started,
            missed: self.pending_missed,
        };
        self.tick(&info);
        self.last_index = index;

        // Where the next tick belongs on the grid. A tick that overran its slot
        // skips the slots it slept through rather than firing them back to back
        // afterwards: catching up would put several frames on the wire inside
        // one period, which is worse than the frames that were already lost.
        let elapsed = clock.now().saturating_sub(origin);
        let next = (index + 1).max(tick_index_at(elapsed) + 1);
        self.pending_missed = next - index - 1;
        self.stats.missed += self.pending_missed;
        self.next_index = next;
    }

    /// Drains the queue, renders, publishes. Exposed so a host can drive the
    /// engine on its own clock instead of using [`run`](Self::run).
    ///
    /// The body runs inside [`catch_unwind`]: `CLAUDE.md`'s zero-crash invariant
    /// says a fault in the lighting maths must not stop DMX output, so a panic
    /// costs exactly one frame. That frame is *not* published — the body was
    /// interrupted half way through writing it, and every output holding its
    /// last good frame for 23 ms is a far smaller fault than one garbled frame
    /// reaching the fixtures.
    pub fn tick(&mut self, tick: &TickInfo) {
        let Self {
            body,
            commands,
            publisher,
            stats,
            ..
        } = self;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let drained = commands.drain(|command| body.apply(command));
            body.render(tick, publisher.frame_mut());
            drained
        }));
        stats.ticks += 1;
        match outcome {
            Ok(drained) => {
                stats.commands += drained as u64;
                publisher.publish();
            }
            Err(_) => stats.panics += 1,
        }
    }

    /// What the run has recorded so far.
    #[must_use]
    pub const fn stats(&self) -> &TickStats {
        &self.stats
    }

    /// Discards the recorded statistics without disturbing the grid.
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }

    /// Grid position of the most recent tick.
    #[must_use]
    pub const fn last_index(&self) -> u64 {
        self.last_index
    }

    /// The body, for a host that needs to inspect it between runs.
    #[must_use]
    pub const fn body(&self) -> &B {
        &self.body
    }

    /// The body, mutably.
    pub const fn body_mut(&mut self) -> &mut B {
        &mut self.body
    }

    /// The publisher, for adding a subscriber before the run starts.
    pub const fn publisher_mut(&mut self) -> &mut FramePublisher {
        &mut self.publisher
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::{
        Engine, IdleBody, TICK_HZ, TICK_PERIOD, TickBody, TickInfo, deadline_offset, tick_index_at,
    };
    use crate::clock::{Clock, ManualClock};
    use crate::command::TickCommand;
    use crate::frame::{DmxFrame, FrameLayout};
    use crate::spsc::{Producer, command_queue};
    use crate::sync::{Arc, AtomicUsize, Ordering};
    use crate::triple_buffer::{FramePublisher, FrameSubscriber};
    use prism_domain::UniverseId;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    fn parts<B: TickBody>(body: B) -> (Engine<B>, Producer<TickCommand>, FrameSubscriber) {
        let layout = Arc::new(FrameLayout::new([UniverseId::new(1)]).unwrap());
        let mut publisher = FramePublisher::new(layout);
        let subscriber = publisher.subscribe();
        let (producer, consumer) = command_queue(64);
        (Engine::new(body, consumer, publisher), producer, subscriber)
    }

    /// Writes the tick index into channel 1 so a published frame can be traced
    /// back to the tick that produced it.
    struct StampBody;

    impl TickBody for StampBody {
        fn apply(&mut self, _command: TickCommand) {}

        fn render(&mut self, tick: &TickInfo, frame: &mut DmxFrame) {
            frame.fill(tick.index as u8);
        }
    }

    #[test]
    fn a_deadline_is_an_exact_fraction_of_a_second_not_a_rounded_period() {
        assert_eq!(deadline_offset(0), Duration::ZERO);
        // 44 ticks is one second, to the nanosecond.
        assert_eq!(deadline_offset(TICK_HZ), Duration::from_secs(1));
        assert_eq!(deadline_offset(TICK_HZ * 600), Duration::from_secs(600));
        // And in between, the exact rational.
        assert_eq!(deadline_offset(1), Duration::from_nanos(22_727_272));
        assert_eq!(deadline_offset(3), Duration::from_nanos(68_181_818));
    }

    #[test]
    fn the_published_period_matches_the_deadline_arithmetic() {
        assert_eq!(TICK_HZ, 44);
        let forty_four = deadline_offset(TICK_HZ) - deadline_offset(0);
        assert!(forty_four.abs_diff(TICK_PERIOD * 44) < Duration::from_micros(1));
    }

    #[test]
    fn deadlines_never_drift_over_a_hundred_thousand_ticks() {
        // The property that matters: the error at tick 100 000 is no larger
        // than the error at tick 100. An accumulating schedule fails this by
        // four orders of magnitude.
        let ideal = |n: u64| Duration::from_secs_f64(n as f64 / f64::from(TICK_HZ as u32));
        for n in [1u64, 100, 10_000, 100_000, 1_000_000] {
            assert!(
                deadline_offset(n).abs_diff(ideal(n)) < Duration::from_micros(1),
                "tick {n} deadline drifted"
            );
        }
    }

    #[test]
    fn a_hundred_thousand_ticks_end_within_one_period_of_where_they_should() {
        let clock = Rc::new(ManualClock::with_overshoot(Duration::from_micros(900)));
        let (mut engine, _producer, _subscriber) = parts(IdleBody);
        engine.run_ticks(clock.as_ref(), 100_000);

        let ideal = deadline_offset(99_999);
        assert!(
            clock.now().abs_diff(ideal) < TICK_PERIOD,
            "after 100 000 ticks the clock was at {:?}, ideal {:?}",
            clock.now(),
            ideal
        );
        assert_eq!(engine.stats().ticks, 100_000);
        assert_eq!(engine.stats().missed, 0);
    }

    #[test]
    fn every_tick_publishes_exactly_one_frame() {
        let clock = ManualClock::new();
        let (mut engine, _producer, mut subscriber) = parts(StampBody);
        engine.run_ticks(&clock, 5);
        assert_eq!(engine.stats().ticks, 5);
        assert!(subscriber.refresh());
        assert_eq!(subscriber.frame().sequence(), 5);
        assert_eq!(subscriber.frame().channel(0, 1), Some(4));
    }

    #[test]
    fn commands_are_applied_before_the_frame_they_affect_is_rendered() {
        #[derive(Default)]
        struct Order(Rc<RefCell<Vec<String>>>);

        impl TickBody for Order {
            fn apply(&mut self, command: TickCommand) {
                self.0.borrow_mut().push(format!("apply {command:?}"));
            }

            fn render(&mut self, tick: &TickInfo, _frame: &mut DmxFrame) {
                self.0.borrow_mut().push(format!("render {}", tick.index));
            }
        }

        let log = Rc::new(RefCell::new(Vec::new()));
        let clock = ManualClock::new();
        let (mut engine, mut producer, _subscriber) = parts(Order(Rc::clone(&log)));
        producer.push(TickCommand::SetBlackout(true)).unwrap();
        producer.push(TickCommand::SetGrandMaster(7)).unwrap();
        engine.run_ticks(&clock, 1);

        assert_eq!(
            log.borrow().as_slice(),
            [
                "apply SetBlackout(true)".to_owned(),
                "apply SetGrandMaster(7)".to_owned(),
                "render 0".to_owned(),
            ]
        );
        assert_eq!(engine.stats().commands, 2);
    }

    #[test]
    fn a_command_that_arrives_late_waits_for_the_next_tick_rather_than_being_lost() {
        let clock = ManualClock::new();
        let (mut engine, mut producer, _subscriber) = parts(IdleBody);
        engine.run_ticks(&clock, 1);
        assert_eq!(engine.stats().commands, 0);
        producer.push(TickCommand::SetGrandMaster(1)).unwrap();
        engine.run_ticks(&clock, 1);
        assert_eq!(engine.stats().commands, 1);
    }

    #[test]
    fn a_body_that_overruns_its_slot_skips_ticks_instead_of_bunching_them_up() {
        /// Burns two and a half tick periods on the first tick only.
        struct Slow {
            clock: Rc<ManualClock>,
            done: bool,
        }

        impl TickBody for Slow {
            fn apply(&mut self, _command: TickCommand) {}

            fn render(&mut self, _tick: &TickInfo, _frame: &mut DmxFrame) {
                if !self.done {
                    self.done = true;
                    self.clock.advance(TICK_PERIOD * 5 / 2);
                }
            }
        }

        let clock = Rc::new(ManualClock::new());
        let body = Slow {
            clock: Rc::clone(&clock),
            done: false,
        };
        let (mut engine, _producer, _subscriber) = parts(body);
        engine.run_ticks(clock.as_ref(), 3);

        // Ticks 1 and 2 were slept through, so they are counted as missed and
        // never run: catching up by firing three ticks back to back would put
        // three frames on the wire in one period.
        assert_eq!(engine.stats().ticks, 3);
        assert_eq!(engine.stats().missed, 2);
        // And the grid is intact - the third tick to run is tick 4, and it runs
        // on tick 4's deadline rather than 2.5 periods behind it.
        assert_eq!(engine.last_index(), 4);
        assert_eq!(clock.now(), deadline_offset(4));
    }

    #[test]
    fn a_panicking_body_does_not_stop_the_engine_or_publish_a_broken_frame() {
        /// Panics on tick 1, works on every other tick.
        struct Fragile;

        impl TickBody for Fragile {
            fn apply(&mut self, _command: TickCommand) {}

            fn render(&mut self, tick: &TickInfo, frame: &mut DmxFrame) {
                frame.fill(0xAA);
                assert_ne!(tick.index, 1, "deliberate test panic");
                frame.fill(tick.index as u8);
            }
        }

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let clock = ManualClock::new();
        let (mut engine, _producer, mut subscriber) = parts(Fragile);
        engine.run_ticks(&clock, 3);
        std::panic::set_hook(previous);

        assert_eq!(engine.stats().ticks, 3);
        assert_eq!(engine.stats().panics, 1);
        // Three ticks, two frames: the tick that panicked left the frame half
        // written, so it was not published. The outputs hold their last good
        // frame, which is what a DMX receiver does with a stalled line anyway.
        assert!(subscriber.refresh());
        assert_eq!(subscriber.frame().sequence(), 2);
        assert_eq!(subscriber.frame().channel(0, 1), Some(2));
    }

    #[test]
    fn a_panicking_command_handler_does_not_take_the_rest_of_the_tick_with_it() {
        struct Fragile;

        impl TickBody for Fragile {
            fn apply(&mut self, _command: TickCommand) {
                panic!("deliberate test panic");
            }

            fn render(&mut self, _tick: &TickInfo, frame: &mut DmxFrame) {
                frame.fill(1);
            }
        }

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let clock = ManualClock::new();
        let (mut engine, mut producer, mut subscriber) = parts(Fragile);
        producer.push(TickCommand::SetBlackout(true)).unwrap();
        engine.run_ticks(&clock, 2);
        std::panic::set_hook(previous);

        assert_eq!(engine.stats().panics, 1);
        assert!(subscriber.refresh());
        assert_eq!(subscriber.frame().sequence(), 1);
    }

    #[test]
    fn running_stops_when_the_flag_is_set() {
        struct StopAfter {
            stop: Arc<AtomicBool>,
            seen: Arc<AtomicUsize>,
        }

        impl TickBody for StopAfter {
            fn apply(&mut self, _command: TickCommand) {}

            fn render(&mut self, _tick: &TickInfo, _frame: &mut DmxFrame) {
                if self.seen.fetch_add(1, Ordering::Relaxed) >= 3 {
                    self.stop.store(true, Ordering::Relaxed);
                }
            }
        }

        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(AtomicUsize::new(0));
        let body = StopAfter {
            stop: Arc::clone(&stop),
            seen: Arc::clone(&seen),
        };
        let clock = ManualClock::new();
        let (mut engine, _producer, _subscriber) = parts(body);
        engine.run(&clock, &stop);
        assert_eq!(engine.stats().ticks, 4);
    }

    #[test]
    fn a_run_that_is_already_stopped_does_nothing() {
        let stop = Arc::new(AtomicBool::new(true));
        let clock = ManualClock::new();
        let (mut engine, _producer, _subscriber) = parts(IdleBody);
        engine.run(&clock, &stop);
        assert_eq!(engine.stats().ticks, 0);
        assert_eq!(clock.now(), Duration::ZERO);
    }

    #[test]
    fn jitter_is_measured_against_the_deadline_not_the_previous_tick() {
        let clock = ManualClock::with_overshoot(Duration::from_micros(500));
        let (mut engine, _producer, _subscriber) = parts(IdleBody);
        engine.run_ticks(&clock, 1_000);
        let jitter = engine.stats().jitter.percentile(0.999);
        assert!(
            jitter <= Duration::from_micros(600),
            "p99.9 jitter was {jitter:?}"
        );
        assert_eq!(engine.stats().jitter.count(), 1_000);
    }

    #[test]
    fn warm_up_ticks_can_be_discarded_before_measuring() {
        let clock = ManualClock::new();
        let (mut engine, _producer, _subscriber) = parts(IdleBody);
        engine.run_ticks(&clock, 10);
        engine.reset_stats();
        assert_eq!(engine.stats().ticks, 0);
        engine.run_ticks(&clock, 4);
        assert_eq!(engine.stats().ticks, 4);
        // Resetting the counters does not reset the grid.
        assert_eq!(engine.last_index(), 13);
    }

    #[test]
    fn the_idle_body_leaves_the_frame_alone() {
        let clock = ManualClock::new();
        let (mut engine, _producer, mut subscriber) = parts(IdleBody);
        engine.run_ticks(&clock, 2);
        assert!(subscriber.refresh());
        assert!(subscriber.frame().channels().iter().all(|&v| v == 0));
    }

    #[test]
    fn the_body_stays_reachable_for_the_thread_that_owns_the_engine() {
        let (mut engine, _producer, _subscriber) = parts(StampBody);
        let clock = ManualClock::new();
        engine.run_ticks(&clock, 1);
        let _: &StampBody = engine.body();
        let _: &mut StampBody = engine.body_mut();
    }

    #[test]
    fn a_driver_can_still_be_attached_before_the_run_starts() {
        // `prismd` builds the engine first and configures outputs afterwards,
        // so the publisher has to stay reachable until the tick thread starts.
        let (mut engine, _producer, _subscriber) = parts(StampBody);
        let mut late = engine.publisher_mut().subscribe();
        let clock = ManualClock::new();
        engine.run_ticks(&clock, 2);
        assert!(late.refresh());
        assert_eq!(late.frame().sequence(), 2);
    }

    #[test]
    fn a_time_offset_maps_back_to_the_tick_slot_it_falls_in() {
        assert_eq!(tick_index_at(Duration::ZERO), 0);
        assert_eq!(tick_index_at(Duration::from_secs(1)), TICK_HZ);
        assert_eq!(tick_index_at(Duration::from_secs(600)), TICK_HZ * 600);
        // Halfway through a slot still belongs to that slot.
        assert_eq!(tick_index_at(deadline_offset(7) + TICK_PERIOD / 2), 7);
        // An absurd offset saturates rather than wrapping into a past tick.
        assert_eq!(tick_index_at(Duration::MAX), u64::MAX);
        assert_eq!(deadline_offset(u64::MAX), Duration::from_nanos(u64::MAX));
    }

    #[test]
    fn a_tick_can_be_driven_by_hand_without_the_loop() {
        // The seam `prismd` uses when it owns the timing itself.
        let (mut engine, mut producer, mut subscriber) = parts(StampBody);
        producer.push(TickCommand::SetBlackout(false)).unwrap();
        engine.tick(&TickInfo {
            index: 12,
            deadline: TICK_PERIOD,
            started: TICK_PERIOD,
            missed: 0,
        });
        assert_eq!(engine.stats().ticks, 1);
        assert_eq!(engine.stats().commands, 1);
        assert!(subscriber.refresh());
        assert_eq!(subscriber.frame().channel(0, 1), Some(12));
    }
}
