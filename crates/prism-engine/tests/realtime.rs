//! Does the tick actually hold its deadline on a real machine?
//!
//! Everything else about the schedule is proved against a simulated clock, which
//! is the right way to test arithmetic and no way at all to test an operating
//! system. This target runs the engine on [`SystemClock`] with output drivers
//! reading from it, and reports what the machine did.
//!
//! The ten-minute run is `#[ignore]`d so `cargo test --workspace` stays quick in
//! CI. It is the session's exit criterion, so the command to run it is written
//! down in the crate documentation and in `PROGRESS.md`:
//!
//! ```text
//! cargo test -p prism-engine --release -- --ignored --nocapture
//! ```
//!
//! # Thread priority is part of the measurement
//!
//! `ARCHITECTURE_SPEC.md` §3 gives `engine-tick` realtime or high priority.
//! `prism-engine` cannot set that itself — it is platform-neutral by rule, and
//! priority is a per-operating-system call — so `prismd` does it, and a test run
//! at the shell's default priority measures a configuration the product never
//! ships in.
//!
//! It matters far more than it sounds. On this machine, ten minutes at normal
//! priority: 45 missed ticks, p99.9 jitter 54 ms. Three minutes at high
//! priority, everything else identical: no missed ticks, p99.9 jitter 200 µs.
//! The difference is entirely the Windows scheduler preempting a normal-priority
//! thread, and it shows up as roughly one stall every twenty seconds whether or
//! not any driver threads are running — which is what
//! `the_tick_alone_holds_its_deadline_for_ten_minutes` exists to establish.
//!
//! To reproduce the recorded figures, run the built test binary in a
//! high-priority process:
//!
//! ```text
//! $exe = (Get-ChildItem target\release\deps\realtime-*.exe |
//!         Sort-Object LastWriteTime -Descending)[0].FullName
//! $p = Start-Process $exe -ArgumentList `
//!        "--ignored","--nocapture","--test-threads=1", `
//!        "the_tick_holds_its_deadline_for_ten_minutes" `
//!        -PassThru -NoNewWindow
//! $p.PriorityClass = "High"
//! ```
//!
//! The short run that *does* execute in CI asserts loosely on purpose. A shared
//! runner can stall for tens of milliseconds for reasons that have nothing to do
//! with this code, and a gate that fails for that reason teaches people to
//! ignore it. It guards against a real regression - a schedule that drifts, a
//! tick that stops publishing - while the strict numbers come from the long run
//! on a known machine.

#![cfg(not(loom))]
#![allow(
    clippy::print_stdout,
    reason = "a measured criterion has to print the number it measured"
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use prism_domain::UniverseId;
use prism_engine::{
    DmxFrame, Engine, FrameLayout, FramePublisher, Histogram, SystemClock, TICK_HZ, TICK_PERIOD,
    TickBody, TickCommand, TickInfo, TickStats, command_queue,
};

/// A body doing the shape of work the merge will do: touch every channel of
/// every universe, from state the commands changed.
#[derive(Default)]
struct RampBody {
    grand_master: u16,
}

impl TickBody for RampBody {
    fn apply(&mut self, command: TickCommand) {
        if let TickCommand::SetGrandMaster(level) = command {
            self.grand_master = level;
        }
    }

    fn render(&mut self, tick: &TickInfo, frame: &mut DmxFrame) {
        for index in 0..frame.universe_count() {
            let Some(universe) = frame.universe_mut(index) else {
                continue;
            };
            for (channel, slot) in universe.iter_mut().enumerate() {
                let value = (tick.index as usize).wrapping_add(channel) as u16;
                *slot = (value.wrapping_mul(self.grand_master) >> 8) as u8;
            }
        }
    }
}

/// What the run produced.
struct Measured {
    stats: TickStats,
    elapsed: Duration,
    frames_taken: Vec<u64>,
}

impl Measured {
    fn report(&self, title: &str) {
        let stats = &self.stats;
        println!("--- {title} ---");
        println!(
            "  ran {:?}, {} ticks (expected {}), {} missed, {} panics, {} commands",
            self.elapsed,
            stats.ticks,
            (self.elapsed.as_secs_f64() * TICK_HZ as f64) as u64,
            stats.missed,
            stats.panics,
            stats.commands
        );
        println!(
            "  jitter: p50 {:?}  p99 {:?}  p99.9 {:?}  max {:?}  (bucket width {:?})",
            stats.jitter.percentile(0.50),
            stats.jitter.percentile(0.99),
            stats.jitter.percentile(0.999),
            stats.jitter.max(),
            Histogram::RESOLUTION,
        );
        println!("  frames taken by each driver: {:?}", self.frames_taken);
    }
}

/// How often a driver thread looks for a new frame.
///
/// Five milliseconds is about four times the frame rate — often enough that a
/// driver never misses a frame it wanted, and slow enough to model a real one.
/// An earlier version polled every 500 µs, which models nothing: four threads
/// waking two thousand times a second is a load the engine would never see in
/// production, and it was contending with the tick for the same cores.
const DRIVER_POLL: Duration = Duration::from_millis(5);

/// How long the long deadline runs last. The session's criterion is ten minutes.
const LONG_RUN: Duration = Duration::from_secs(600);

/// Runs the engine for `duration` with `universes` universes and `drivers`
/// output threads polling for frames.
fn measure(duration: Duration, universes: u32, drivers: usize) -> Measured {
    let layout = Arc::new(FrameLayout::new((1..=universes).map(UniverseId::new)).unwrap());
    let mut publisher = FramePublisher::new(layout);
    let subscribers: Vec<_> = (0..drivers).map(|_| publisher.subscribe()).collect();
    let (mut producer, consumer) = command_queue(256);
    let mut engine = Engine::new(RampBody::default(), consumer, publisher);
    let clock = SystemClock::new();

    let stop = AtomicBool::new(false);
    let taken: Vec<AtomicU64> = (0..drivers).map(|_| AtomicU64::new(0)).collect();

    // Built inside the scope, read outside it: `thread::scope` evaluates the
    // closure's result before joining, so the drivers' counters are only final
    // once the scope has ended.
    let (stats, elapsed) = thread::scope(|scope| {
        let running = &stop;
        let counters = &taken;
        for (index, subscriber) in subscribers.into_iter().enumerate() {
            scope.spawn(move || {
                let mut subscriber = subscriber;
                let mut frames = 0;
                while !running.load(Ordering::Relaxed) {
                    if subscriber.refresh() {
                        // Stand in for handing the frame to a driver: read it,
                        // so the compiler cannot elide the copy.
                        std::hint::black_box(subscriber.frame().channels());
                        frames += 1;
                    }
                    thread::sleep(DRIVER_POLL);
                }
                if let Some(counter) = counters.get(index) {
                    counter.store(frames, Ordering::Relaxed);
                }
            });
        }

        // Warm-up: the first ticks pay for page faults, cold caches and, on
        // Windows, the timer settling at its high resolution.
        let _ = producer.push(TickCommand::SetGrandMaster(u16::MAX));
        engine.run_ticks(&clock, TICK_HZ);
        engine.reset_stats();

        let started = Instant::now();
        scope.spawn(move || {
            thread::sleep(duration);
            running.store(true, Ordering::Relaxed);
        });
        engine.run(&clock, &stop);
        (engine.stats().clone(), started.elapsed())
    });

    Measured {
        stats,
        elapsed,
        frames_taken: taken.iter().map(|c| c.load(Ordering::Relaxed)).collect(),
    }
}

#[test]
fn the_tick_holds_its_deadline_for_a_few_seconds() {
    let measured = measure(Duration::from_secs(3), 64, 4);
    measured.report("3 s, 64 universes, 4 drivers");
    let stats = &measured.stats;

    let expected = (measured.elapsed.as_secs_f64() * TICK_HZ as f64) as u64;
    assert!(
        stats.ticks + stats.missed >= expected - 2,
        "the grid lost ticks entirely: {} ran, {} missed, {expected} expected",
        stats.ticks,
        stats.missed
    );
    assert_eq!(stats.panics, 0);

    // Three seconds is about 130 samples, so p99.9 here is simply the worst one
    // and a single scheduler stall would decide it. What this run can say
    // something about is the body of the distribution: a schedule that drifts,
    // sleeps for a period instead of to a deadline, or wakes late every time
    // moves the median, and moves it by far more than this margin. The tail is
    // the ten-minute run's business, where 26 400 samples make a p99.9 mean
    // something.
    assert!(
        stats.jitter.percentile(0.50) <= Duration::from_millis(1),
        "median jitter was {:?}",
        stats.jitter.percentile(0.50)
    );
    assert!(
        stats.jitter.percentile(0.95) < Duration::from_millis(5),
        "p95 jitter was {:?}",
        stats.jitter.percentile(0.95)
    );
    assert!(
        stats.missed * 50 <= stats.ticks,
        "{} of {} ticks were missed",
        stats.missed,
        stats.ticks
    );
    for frames in &measured.frames_taken {
        assert!(*frames > 0, "a driver received no frames at all");
    }
}

#[test]
#[ignore = "long-running: the session's ten-minute deadline criterion"]
fn the_tick_holds_its_deadline_for_ten_minutes() {
    let measured = measure(LONG_RUN, 64, 4);
    measured.report("64 universes, 4 drivers");
    let stats = &measured.stats;

    assert_eq!(stats.panics, 0);
    assert_eq!(
        stats.missed, 0,
        "{} ticks were missed over {:?}",
        stats.missed, LONG_RUN
    );
    assert!(
        stats.jitter.percentile(0.999) < Duration::from_millis(2),
        "p99.9 jitter was {:?}, budget 2 ms",
        stats.jitter.percentile(0.999)
    );
    // Ten minutes of a 44 Hz grid is 26 400 ticks, give or take the tick the
    // stop flag lands in.
    let expected = LONG_RUN.as_secs() * TICK_HZ;
    assert!(
        stats.ticks.abs_diff(expected) <= 2,
        "{} ticks in {LONG_RUN:?}, expected {expected}",
        stats.ticks
    );
}

#[test]
#[ignore = "long-running: diagnostic, run when the deadline criterion fails"]
fn the_tick_alone_holds_its_deadline_for_ten_minutes() {
    // The same run with nothing else of ours on the machine. If this is clean
    // and the four-driver run is not, the drivers are the problem; if both show
    // the same stalls, the machine is. It answers the only question worth asking
    // when the criterion fails: is it us, or is it the operating system?
    let measured = measure(LONG_RUN, 64, 0);
    measured.report("64 universes, no drivers");
    assert_eq!(measured.stats.panics, 0);
}

#[test]
#[ignore = "long-running: drift over 100 000 real ticks is nearly 38 minutes"]
fn the_grid_does_not_drift_over_a_hundred_thousand_real_ticks() {
    // The same property the simulated-clock test proves in milliseconds, on a
    // real clock: after 100 000 ticks the absolute time error must still be
    // under one tick period.
    let ticks = 100_000;
    let layout = Arc::new(FrameLayout::new([UniverseId::new(1)]).unwrap());
    let publisher = FramePublisher::new(layout);
    let (_producer, consumer) = command_queue(8);
    let mut engine = Engine::new(RampBody::default(), consumer, publisher);
    let clock = SystemClock::new();

    let started = Instant::now();
    engine.run_ticks(&clock, ticks);
    let elapsed = started.elapsed();

    let ideal = prism_engine::deadline_offset(ticks - 1);
    let error = elapsed.abs_diff(ideal);
    println!("{ticks} ticks took {elapsed:?}, ideal {ideal:?}, error {error:?}");
    assert!(
        error < TICK_PERIOD,
        "absolute time error {error:?} exceeded one tick period {TICK_PERIOD:?}"
    );
    assert_eq!(engine.stats().missed, 0);
}
