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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use prism_domain::{
    AttributeDef, AttributeType, Cue, CuePart, CueTrigger, Fixture, FixtureId, FixtureType,
    GoDirection, Group, GroupId, Sequence, SequenceId, UniverseId, Vec3,
};
use prism_engine::{
    DmxFrame, Engine, FrameLayout, FramePublisher, Histogram, MergeBody, SystemClock, TICK_HZ,
    TICK_PERIOD, TickBody, TickCommand, TickInfo, TickStats, UNIVERSE_CHANNELS, command_queue,
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
    /// How many times a second a probe thread that yields on every iteration
    /// got a turn.
    ///
    /// A stress gate that cannot tell a loaded machine from an idle one is not
    /// much of a gate, and the load comes from outside the process, so this is
    /// the one signal about the machine that the run can measure for itself.
    /// See [`BUSY_BELOW`] for what the two cases actually measure.
    probe_turns: f64,
}

/// Probe turns per second below which the machine counts as loaded.
///
/// Measured on the reference machine (8 cores) with the three-second run:
///
/// | Machine | Test process | Probe |
/// |---|---|---|
/// | idle | normal | 5 504 900/s |
/// | 8 burners at normal priority | high | 118 034/s |
/// | 8 burners at normal priority | normal | 1 004 443/s |
///
/// A factor of forty-seven between the first two rows is enough to tell them
/// apart without a delicate threshold. The number is a sanity check on the
/// harness — "did anyone actually start the load?" — not a measurement in its
/// own right.
const BUSY_BELOW: f64 = 1_000_000.0;

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
        println!("  probe thread turns: {:.0}/s", self.probe_turns);
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

/// Held for the whole of every measured run.
///
/// `cargo test` runs the tests in one binary on several threads, and two
/// deadline measurements sharing a machine measure each other. This was not
/// visible while there was one of them; adding a second made both fail on a CI
/// runner with two cores, with a median jitter of exactly one scheduler
/// quantum. Timing runs take turns.
static MEASURING: Mutex<()> = Mutex::new(());

/// How a measured run is set up.
struct Setup {
    duration: Duration,
    /// Output driver threads polling for frames.
    drivers: usize,
    /// CPU-burning threads inside this process. Almost always zero: they spin
    /// at **this process's** priority, so they do not model a busy machine —
    /// see [`the_whole_pipeline_holds_its_deadline_for_ten_minutes_under_full_cpu_load`].
    load: usize,
    /// Whether to run the busy probe. It spins, so it costs a core, and only
    /// the stress gate needs what it measures.
    probe: bool,
}

/// Runs the engine for `duration` with `universes` universes and `drivers`
/// output threads polling for frames.
fn measure(duration: Duration, universes: u32, drivers: usize) -> Measured {
    let layout = FrameLayout::new((1..=universes).map(UniverseId::new)).unwrap();
    measure_body(
        RampBody::default(),
        layout,
        &Setup {
            duration,
            drivers,
            load: 0,
            probe: false,
        },
        |_, _| {},
    )
}

/// The same run with any tick body and a hook that gets a chance to push
/// commands on every tick.
fn measure_body<B: TickBody + Send>(
    body: B,
    layout: FrameLayout,
    setup: &Setup,
    mut on_tick: impl FnMut(&mut prism_engine::Producer<TickCommand>, u64),
) -> Measured {
    // Poisoning is irrelevant here: the guard protects a schedule, not data.
    let _measuring = MEASURING.lock().unwrap_or_else(|held| held.into_inner());
    let Setup {
        duration,
        drivers,
        load,
        probe: want_probe,
    } = *setup;
    let layout = Arc::new(layout);
    let mut publisher = FramePublisher::new(layout);
    let subscribers: Vec<_> = (0..drivers).map(|_| publisher.subscribe()).collect();
    let (mut producer, consumer) = command_queue(256);
    let mut engine = Engine::new(body, consumer, publisher);
    let clock = SystemClock::new();

    let stop = AtomicBool::new(false);
    let taken: Vec<AtomicU64> = (0..drivers).map(|_| AtomicU64::new(0)).collect();
    // The busy probe: a thread that yields on every iteration. On an idle
    // machine it is rescheduled millions of times a second; with every core
    // taken it gets a turn only when something else blocks.
    let probe = AtomicU64::new(0);

    // Built inside the scope, read outside it: `thread::scope` evaluates the
    // closure's result before joining, so the drivers' counters are only final
    // once the scope has ended.
    let (stats, elapsed) = thread::scope(|scope| {
        let running = &stop;
        let counters = &taken;
        for _ in 0..load {
            scope.spawn(move || {
                // A tight arithmetic loop the optimiser cannot remove: the point
                // is to keep a core busy, not to compute anything.
                let mut value = 1u64;
                while !running.load(Ordering::Relaxed) {
                    for _ in 0..10_000 {
                        value = std::hint::black_box(
                            value
                                .wrapping_mul(6_364_136_223_846_793_005)
                                .wrapping_add(1),
                        );
                    }
                }
                std::hint::black_box(value);
            });
        }
        if want_probe {
            let probe_counter = &probe;
            scope.spawn(move || {
                while !running.load(Ordering::Relaxed) {
                    probe_counter.fetch_add(1, Ordering::Relaxed);
                    thread::yield_now();
                }
            });
        }
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
        // The engine's own loop, with the command hook run between ticks so a
        // scripted show can play through the measured window.
        while !stop.load(Ordering::Relaxed) {
            on_tick(&mut producer, engine.last_index());
            engine.run_ticks(&clock, 1);
        }
        (engine.stats().clone(), started.elapsed())
    });

    // Below a hundred thousand turns a second the probe is not getting a core
    // to itself, which is what "the machine is busy" means here. An idle
    // machine gives it tens of millions.
    let turns = probe.load(Ordering::Relaxed);
    let per_second = turns as f64 / elapsed.as_secs_f64().max(f64::MIN_POSITIVE);
    Measured {
        stats,
        elapsed,
        frames_taken: taken.iter().map(|c| c.load(Ordering::Relaxed)).collect(),
        probe_turns: per_second,
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
    // **There is deliberately no tail assertion here, and the third failure is
    // what settled it.** The bound stood at 5 ms for seven sessions, failed a
    // documentation-only commit at 16 ms, was loosened to one whole tick period
    // — and failed again two commits later at 210 ms, on a commit that touched
    // no code in this crate, with the median still at 100 µs and the identical
    // tree green on a rerun. That run printed `probe thread turns: 0/s`: the
    // machine had no core to spare at all, which is the precondition of the
    // measurement rather than a property of the schedule. `cargo test` runs the
    // workspace's test binaries in parallel, so every target a later session
    // adds is another thing this three-second window is measuring.
    //
    // A percentile is only a gate where the sample count supports it. p95 over
    // ~130 samples is the seventh-worst one, and on a shared two-core runner the
    // seventh-worst is whatever else the runner was doing. The tail number that
    // means something is the ten-minute run's p99.9 = 200 µs over 26 401
    // samples (`PROGRESS.md` §3), and it is asserted there. The distribution is
    // still printed by `report` above, so a run that goes strange can be read.
    // The share of the grid that actually ran, rather than a missed-tick budget
    // over 130 ticks — the remedy S6 applied to the pipeline run, for the same
    // reason and now with a second instance of the same failure behind it. One
    // stall costs a burst of consecutive deadlines (68 ms is three periods gone
    // before the engine gets a core back), so a 2 % budget at this sample size
    // measures the runner. Three quarters is what an engine too slow for the
    // grid destroys.
    assert!(
        stats.ticks * 4 >= expected * 3,
        "only {} of about {expected} ticks ran, {} missed",
        stats.ticks,
        stats.missed
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

/// How many CPU-burning threads the stress gate should run inside its own
/// process, from `PRISM_STRESS_LOAD`.
///
/// Zero by default, and that is deliberate — see the gate's own documentation.
fn in_process_load() -> usize {
    std::env::var("PRISM_STRESS_LOAD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// A fixture type with six 8-bit attributes: one intensity and five that are
/// not, so both merge modes and both sides of the master rule are on the tick.
fn stress_fixture_type() -> FixtureType {
    let attributes = AttributeType::ALL
        .iter()
        .take(6)
        .enumerate()
        .map(|(index, attribute)| AttributeDef {
            attribute: *attribute,
            label: None,
            occurrence: 0,
            feature_group: attribute.feature_group(),
            coarse_offset: index as u16,
            fine_offset: None,
            default_value: 32_768,
            merge_mode: attribute.default_merge_mode(),
            invert: false,
            physical_from: 0.0,
            physical_to: 100.0,
            ranges: Vec::new(),
        })
        .collect::<Vec<_>>();
    FixtureType {
        id: "stress.head".to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Test".to_owned(),
        mode: "6ch".to_owned(),
        footprint: 6,
        attributes,
    }
}

/// Every universe of `layout` filled with back-to-back fixtures.
fn stress_patch(layout: &FrameLayout, fixture_type: &FixtureType) -> Vec<Fixture> {
    let footprint = u32::from(fixture_type.footprint);
    let per_universe = (UNIVERSE_CHANNELS as u32) / footprint;
    let count = per_universe * layout.universe_count() as u32;
    (0..count)
        .map(|index| Fixture {
            software_dimmer: true,
            id: FixtureId::new(index + 1),
            name: String::new(),
            type_id: fixture_type.id.clone(),
            universe: *layout
                .universes()
                .get((index / per_universe) as usize)
                .expect("the layout is wide enough for the patch"),
            address: ((index % per_universe) * footprint) as u16 + 1,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            invert_pan: index % 2 == 0,
            invert_tilt: index % 2 == 0,
        })
        .collect()
}

/// A looping cue list over every attribute of every fixture, fading for ten
/// seconds a cue and following on by itself.
fn stress_sequence(fixture_type: &FixtureType, fixtures: u32, seed: u16) -> Sequence {
    let cues = (0..4u16)
        .map(|number| Cue {
            number: number.to_string(),
            name: String::new(),
            fade_in: 10.0,
            fade_out: 10.0,
            delay: 0.0,
            trigger: if number == 0 {
                CueTrigger::Go
            } else {
                CueTrigger::Follow
            },
            trigger_time: None,
            parts: (1..=fixtures)
                .flat_map(|fixture| {
                    fixture_type.attributes.iter().map(move |def| CuePart {
                        fixture: FixtureId::new(fixture),
                        attribute: def.attribute,
                        occurrence: 0,
                        value: fixture
                            .wrapping_mul(u32::from(number) + 1)
                            .wrapping_add(u32::from(seed)) as u16,
                        preset_ref: None,
                        tracking: prism_domain::CueTracking::Track,
                    })
                })
                .collect(),
        })
        .collect();
    Sequence {
        id: SequenceId::new(u32::from(seed)),
        name: String::new(),
        color: None,
        cues,
        looping: true,
        master_level: u16::MAX,
        speed: prism_domain::SPEED_UNITY,
        is_active: false,
        current_cue_index: None,
    }
}

/// The whole pipeline at full size: 64 universes patched to the last channel,
/// eight cue lists running over every slot, sixteen group masters, a programmer
/// with values in it.
fn stress_body(layout: &FrameLayout) -> MergeBody {
    let head = stress_fixture_type();
    let patched = stress_patch(layout, &head);
    let fixtures = patched.len() as u32;
    let mut body = MergeBody::for_patch(
        layout,
        patched.iter().map(|fixture| (fixture, &head)),
        (1..=8).map(SequenceId::new),
    )
    .unwrap();
    for executor in 1..=8u32 {
        body.load_sequence(
            SequenceId::new(executor),
            &stress_sequence(&head, fixtures, executor as u16),
        )
        .unwrap();
    }
    let groups: Vec<Group> = (0..16u32)
        .map(|index| Group {
            id: GroupId::new(index + 1),
            name: String::new(),
            fixtures: (1..=fixtures)
                .filter(|fixture| fixture % 16 == index)
                .map(FixtureId::new)
                .collect(),
        })
        .collect();
    body.load_groups(&groups);
    body
}

/// The session's stress gate: 64 universes, 100 % CPU load, ten minutes,
/// p99.9 jitter under 2 ms, no dropped frames
/// (`IMPLEMENTATION_PLAN.md` S6, `CLAUDE.md`).
///
/// Unlike the S2 deadline run, the body is the real one — cue lists fading over
/// every slot in the patch, a programmer being written and cleared, group and
/// grand masters moving — so what is measured is the product rather than a
/// placeholder.
///
/// # The CPU load has to come from outside this process
///
/// The first version of this test spun one thread per core inside the test
/// process, and it failed: 7 484 of 26 455 ticks missed, median jitter 15.5 ms,
/// p99.9 95 ms. Nothing in the engine was responsible. A Windows process
/// priority class applies to **every** thread in the process, so raising the
/// tick's priority as `ARCHITECTURE_SPEC.md` §3 requires raised the spinners'
/// too — and an equal-priority thread that never blocks is not preempted until
/// its quantum expires. The 15.5 ms median is one scheduler quantum, measured.
///
/// That is not the configuration the product ships in. `prismd` raises the tick
/// **thread**, and the load on a real machine is other work at ordinary
/// priority. So the load is applied from outside, at normal priority, exactly
/// as the priority itself is applied from outside — `prism-engine` is
/// platform-neutral by rule and can set neither.
///
/// ```text
/// cargo build -p prism-engine --release --tests
/// $exe = (Get-ChildItem target\release\deps\realtime-*.exe |
///         Sort-Object LastWriteTime -Descending)[0].FullName
/// # 100 % CPU load at ordinary priority, one burner per core
/// $load = 1..([Environment]::ProcessorCount) | ForEach-Object {
///     Start-Process powershell -ArgumentList "-NoProfile","-Command","while(1){}" `
///         -PassThru -WindowStyle Hidden
/// }
/// $p = Start-Process $exe -ArgumentList "--ignored","--nocapture","--test-threads=1", `
///        "the_whole_pipeline_holds_its_deadline_for_ten_minutes_under_full_cpu_load" `
///        -PassThru -NoNewWindow
/// $p.PriorityClass = "High"
/// $p.WaitForExit(); $load | Stop-Process
/// ```
///
/// `PRISM_STRESS_LOAD=n` adds `n` in-process burners for anyone who wants the
/// equal-priority case back. It defaults to none, so the run measures what the
/// harness sets up rather than quietly fighting itself.
#[test]
#[ignore = "long-running: the session's ten-minute stress gate under full CPU load"]
fn the_whole_pipeline_holds_its_deadline_for_ten_minutes_under_full_cpu_load() {
    let cores = thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let load = in_process_load();
    let layout = FrameLayout::new((1..=64).map(UniverseId::new)).unwrap();
    let body = stress_body(&layout);
    let slots = body.plan().slot_count();
    let intensity = body.masters().intensity_slots().len();
    println!(
        "stress rig: {slots} slots, {intensity} intensity slots, 8 cue lists, \
         16 groups, {cores} cores, {load} in-process load threads \
         (external load at normal priority is the documented configuration)"
    );

    let measured = measure_body(
        body,
        layout,
        &Setup {
            duration: LONG_RUN,
            drivers: 4,
            load,
            probe: true,
        },
        |producer, index| {
            let executor = SequenceId::new((index % 8) as u32 + 1);
            if index % 64 == 0 {
                let _ = producer.push(TickCommand::Go {
                    executor: executor.into(),
                    direction: GoDirection::Next,
                });
            }
            let slot = (index % 4096) as u32;
            let _ = producer.push(if index % 5 == 0 {
                TickCommand::ClearProgrammerValue { slot }
            } else {
                TickCommand::SetProgrammerValue {
                    slot,
                    value: index as u16,
                }
            });
            let _ = producer.push(TickCommand::SetGroupMaster {
                group: GroupId::new((index % 16) as u32 + 1),
                level: (index as u16) | 0x4000,
            });
            let _ = producer.push(TickCommand::SetGrandMaster((index as u16) | 0x8000));
        },
    );
    measured.report("64 universes, full pipeline, 4 drivers, 100 % CPU load");
    let stats = &measured.stats;

    assert_eq!(stats.panics, 0);
    assert_eq!(
        stats.missed, 0,
        "{} ticks were missed over {:?} under load",
        stats.missed, LONG_RUN
    );
    assert!(
        stats.jitter.percentile(0.999) < Duration::from_millis(2),
        "p99.9 jitter was {:?}, budget 2 ms",
        stats.jitter.percentile(0.999)
    );
    let expected = LONG_RUN.as_secs() * TICK_HZ;
    assert!(
        stats.ticks.abs_diff(expected) <= 2,
        "{} ticks in {LONG_RUN:?}, expected {expected}",
        stats.ticks
    );
    // No dropped frames: every driver kept up with the stream it was handed.
    for frames in &measured.frames_taken {
        assert!(*frames > 0, "a driver received no frames at all");
    }
    // The criterion says "under load", so a run on an idle machine has not
    // passed it, however good the numbers look.
    assert!(
        measured.probe_turns < BUSY_BELOW,
        "the machine was not under load: the probe got {:.0} turns a second, and \
         a loaded one gives well under {BUSY_BELOW:.0}. See this test's \
         documentation for how to apply the load",
        measured.probe_turns
    );
}

#[test]
fn the_whole_pipeline_fits_inside_a_tick_period() {
    // The CI-sized companion to the gate above. It asserts loosely, for the
    // reason the S2 short run does: a shared runner stalls for reasons that
    // have nothing to do with this code, and a gate that cries wolf gets
    // ignored. What it does catch is the regression that matters - a pipeline
    // that has become too slow to finish inside a tick period at all.
    //
    // No load threads and no probe. Both spin at this process's priority, and
    // an equal-priority spinner does not model a busy machine, it models a
    // priority inversion - the finding that is written up on the ten-minute
    // gate. On a two-core CI runner it turned this test into a measurement of
    // itself.
    //
    // Eight universes rather than 64, because CI runs `cargo test` without
    // optimisations and an unoptimised build of this pipeline is roughly ten
    // times slower: at full size it misses more deadlines than it holds. That
    // says nothing about the product - the release build at 64 universes holds
    // every one of them under full load - so the full-size claim belongs to the
    // `#[ignore]`d gate above, which is documented as a release run.
    let layout = FrameLayout::new((1..=8).map(UniverseId::new)).unwrap();
    let body = stress_body(&layout);
    let measured = measure_body(
        body,
        layout,
        &Setup {
            duration: Duration::from_secs(3),
            drivers: 2,
            load: 0,
            probe: false,
        },
        |producer, index| {
            if index % 64 == 0 {
                let _ = producer.push(TickCommand::Go {
                    executor: SequenceId::new((index % 8) as u32 + 1).into(),
                    direction: GoDirection::Next,
                });
            }
            let _ = producer.push(TickCommand::SetGrandMaster((index as u16) | 0x8000));
        },
    );
    measured.report("3 s, 8 universes, full pipeline, 2 drivers");
    let stats = &measured.stats;

    assert_eq!(stats.panics, 0);
    // Jitter is measured on the ticks that ran, and the engine resynchronises
    // to the grid after a missed deadline - so a body too slow to fit inside a
    // period does *not* show up in the median. The unoptimised 64-universe run
    // that established the size of this test had a median of 100 µs while
    // missing 49 of 133 ticks. What the median does catch is a schedule that
    // drifts or sleeps by period instead of to a deadline, which is worth
    // keeping, but it is not this test's headline.
    assert!(
        stats.jitter.percentile(0.50) <= Duration::from_millis(2),
        "median jitter was {:?}",
        stats.jitter.percentile(0.50)
    );
    // The headline is the share of the grid that ran at all: a pipeline that no
    // longer fits loses ticks wholesale, and the run above would fail this at
    // 84 ticks of 133. Deliberately not a missed-tick budget - on a shared
    // two-core runner one scheduler stall costs a burst of consecutive
    // deadlines (68 ms of stall is three periods gone before the engine gets a
    // core back), so a budget over 130 ticks measures the runner rather than
    // the code. It failed exactly that way once, with a median of 100 µs.
    let expected = (measured.elapsed.as_secs_f64() * TICK_HZ as f64) as u64;
    assert!(
        stats.ticks * 4 >= expected * 3,
        "only {} of about {expected} ticks ran",
        stats.ticks
    );
    for frames in &measured.frames_taken {
        assert!(*frames > 0, "a driver received no frames at all");
    }
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
