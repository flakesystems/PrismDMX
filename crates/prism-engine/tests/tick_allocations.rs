//! Proof that a warm tick never calls the allocator.
//!
//! `ARCHITECTURE_SPEC.md` §3.1 forbids heap allocation inside the tick, and the
//! only way to know is to count. This target installs a global allocator that
//! tallies every call made by the thread that armed it, so the count comes from
//! the allocator itself rather than from reading the code and hoping.
//!
//! Freeing counts too. A `String` arriving in the tick and being dropped there
//! is an allocator call just as surely as one being created, and it is the more
//! likely mistake — which is why [`prism_engine::TickCommand`] has no owned
//! fields at all.

#![cfg(not(loom))]
#![allow(
    clippy::print_stdout,
    reason = "a measured criterion has to print the number it measured"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use prism_domain::{
    AttributeDef, AttributeType, ExecutorId, Fixture, FixtureId, FixtureType, UniverseId, Vec3,
};
use prism_engine::{
    Clock, DmxFrame, Engine, FrameLayout, FramePublisher, ManualClock, MergeBody, SystemClock,
    TickBody, TickCommand, TickInfo, UNIVERSE_CHANNELS, command_queue,
};

/// Counts allocator calls made by whichever thread has armed the probe.
///
/// Per-thread rather than global: the test harness, the reader threads and the
/// runtime all allocate whenever they like, and none of that is the tick's
/// business.
struct CountingAllocator;

thread_local! {
    /// Whether this thread is inside a measured window. `const` initialised, so
    /// touching it cannot itself allocate.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    /// Allocator calls made by this thread while armed.
    static CALLS: Cell<u64> = const { Cell::new(0) };
}

fn record() {
    // `try_with`, not `with`: during thread teardown the local is gone, and a
    // panic from inside the allocator would be unrecoverable.
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = CALLS.try_with(|calls| calls.set(calls.get() + 1));
        }
    });
}

#[allow(
    unsafe_code,
    reason = "GlobalAlloc cannot be implemented safely; the unsafety is confined to \
              this test harness and never enters the library"
)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Runs `work` with the probe armed and returns how many allocator calls it
/// made on this thread.
fn allocator_calls(work: impl FnOnce()) -> u64 {
    CALLS.with(|calls| calls.set(0));
    ARMED.with(|armed| armed.set(true));
    work();
    ARMED.with(|armed| armed.set(false));
    CALLS.with(Cell::get)
}

/// A body doing the sort of work the merge will do: read some state, write every
/// channel of every universe.
#[derive(Default)]
struct RampBody {
    grand_master: u16,
    blackout: bool,
}

impl TickBody for RampBody {
    fn apply(&mut self, command: TickCommand) {
        match command {
            TickCommand::SetGrandMaster(level) => self.grand_master = level,
            TickCommand::SetBlackout(on) => self.blackout = on,
            TickCommand::SetExecutorLevel { .. }
            | TickCommand::SetExecutorActive { .. }
            | TickCommand::Go { .. } => {}
        }
    }

    fn render(&mut self, tick: &TickInfo, frame: &mut DmxFrame) {
        let scale = if self.blackout { 0 } else { self.grand_master };
        for index in 0..frame.universe_count() {
            let Some(universe) = frame.universe_mut(index) else {
                continue;
            };
            for (channel, slot) in universe.iter_mut().enumerate() {
                let value = (tick.index as usize).wrapping_add(channel) as u16;
                *slot = ((value.wrapping_mul(scale)) >> 8) as u8;
            }
        }
    }
}

struct Harness {
    engine: Engine<RampBody>,
    producer: prism_engine::Producer<TickCommand>,
    subscribers: Vec<prism_engine::FrameSubscriber>,
}

fn harness(universes: u32, subscribers: usize) -> Harness {
    let layout = Arc::new(FrameLayout::new((1..=universes).map(UniverseId::new)).unwrap());
    let mut publisher = FramePublisher::new(layout);
    let subscribers = (0..subscribers).map(|_| publisher.subscribe()).collect();
    let (producer, consumer) = command_queue(256);
    Harness {
        engine: Engine::new(RampBody::default(), consumer, publisher),
        producer,
        subscribers,
    }
}

/// One measured cycle: queue a command, run a tick, let every driver pick the
/// frame up. Every step of that is on the real-time path.
fn cycle<C: Clock>(harness: &mut Harness, clock: &C, index: u16) {
    let _ = harness.producer.push(TickCommand::SetExecutorLevel {
        executor: ExecutorId::new(u32::from(index)),
        level: index,
    });
    let _ = harness.producer.push(TickCommand::SetGrandMaster(index));
    harness.engine.run_ticks(clock, 1);
    for subscriber in &mut harness.subscribers {
        subscriber.refresh();
    }
}

#[test]
fn a_warm_tick_makes_no_allocator_call_at_all() {
    let mut harness = harness(64, 4);
    let clock = ManualClock::new();

    // Warm-up: page faults, cold caches and anything lazily initialised belong
    // to start-up, not to the steady state the criterion is about.
    for index in 0..200 {
        cycle(&mut harness, &clock, index);
    }
    harness.engine.reset_stats();

    let calls = allocator_calls(|| {
        for index in 0..2_000 {
            cycle(&mut harness, &clock, index);
        }
    });

    println!("allocator calls in 2000 ticks, 64 universes, 4 subscribers: {calls}");
    assert_eq!(calls, 0, "the tick called the allocator {calls} times");
    assert_eq!(harness.engine.stats().ticks, 2_000);
    assert_eq!(harness.engine.stats().commands, 4_000);
    assert_eq!(harness.engine.stats().panics, 0);
}

#[test]
fn the_real_clock_path_is_allocation_free_too() {
    // The manual clock proves the engine allocates nothing. This proves the
    // sleep-and-spin path does not either - that is the code that actually runs
    // in the daemon, and it is the one place the standard library gets involved.
    let mut harness = harness(8, 2);
    let clock = SystemClock::new();

    for index in 0..44 {
        cycle(&mut harness, &clock, index);
    }
    harness.engine.reset_stats();

    let calls = allocator_calls(|| {
        for index in 0..44 {
            cycle(&mut harness, &clock, index);
        }
    });

    println!("allocator calls in 44 ticks on the system clock: {calls}");
    assert_eq!(calls, 0);
    assert_eq!(harness.engine.stats().ticks, 44);
}

/// A fixture type with `attributes` of the merge's own attributes, so a plan
/// built from many of them exercises both merge modes. `sixteen_bit` doubles
/// the footprint and gives every attribute a fine channel, which is the
/// encoder's worst case: two writes per slot instead of one.
fn fixture_type(attributes: usize, sixteen_bit: bool) -> FixtureType {
    let width = if sixteen_bit { 2 } else { 1 };
    let attributes = AttributeType::ALL
        .iter()
        .take(attributes)
        .enumerate()
        .map(|(index, attribute)| AttributeDef {
            attribute: *attribute,
            feature_group: attribute.feature_group(),
            coarse_offset: (index * width) as u16,
            fine_offset: sixteen_bit.then(|| (index * width + 1) as u16),
            default_value: 32_768,
            merge_mode: attribute.default_merge_mode(),
            invert: false,
            physical_from: 0.0,
            physical_to: 100.0,
        })
        .collect::<Vec<_>>();
    FixtureType {
        id: "test.head".to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Test".to_owned(),
        mode: "test".to_owned(),
        footprint: (attributes.len() * width) as u16,
        attributes,
    }
}

/// `count` fixtures of `fixture_type`, patched back to back across the
/// universes of `layout`. Every other one is hung upside down, so the encoder's
/// invert path is on the measured tick as well.
fn patch(layout: &FrameLayout, fixture_type: &FixtureType, count: u32) -> Vec<Fixture> {
    let footprint = u32::from(fixture_type.footprint);
    let per_universe = (UNIVERSE_CHANNELS as u32) / footprint;
    (0..count)
        .map(|index| Fixture {
            id: FixtureId::new(index + 1),
            name: String::new(),
            type_id: fixture_type.id.clone(),
            universe: *layout
                .universes()
                .get((index / per_universe) as usize)
                .expect("the layout must be wide enough for the patch"),
            address: ((index % per_universe) * footprint) as u16 + 1,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            invert_pan: index % 2 == 0,
            invert_tilt: index % 2 == 0,
        })
        .collect()
}

/// The merge, loaded up: every source provides a value for every slot, which is
/// the worst case the resolve can be given.
fn merge_body(
    layout: &FrameLayout,
    head: &FixtureType,
    fixtures: u32,
    executors: u32,
) -> MergeBody {
    let patched = patch(layout, head, fixtures);
    let mut body = MergeBody::for_patch(
        layout,
        patched.iter().map(|fixture| (fixture, head)),
        (1..=executors).map(ExecutorId::new),
    )
    .unwrap();
    let slots = body.plan().slot_count();
    for executor in 1..=executors {
        let source = body
            .layer_mut()
            .source_mut(ExecutorId::new(executor))
            .unwrap();
        for slot in 0..slots {
            source.set(slot, (slot as u16).wrapping_mul(executor as u16));
        }
        body.layer_mut().activate(ExecutorId::new(executor));
    }
    body
}

#[test]
fn a_tick_running_the_merge_makes_no_allocator_call_either() {
    // The empty tick proving nothing about the allocator is easy. This is the
    // criterion that matters: the merge itself, resolving a full source set on
    // every tick, with executors going on and off underneath it.
    let head = fixture_type(6, false);
    let layout = FrameLayout::new((1..=8).map(UniverseId::new)).unwrap();
    let body = merge_body(&layout, &head, 128, 8);
    let slots = body.plan().slot_count();
    assert_eq!(slots, 128 * 6);

    let mut publisher = FramePublisher::new(Arc::new(layout));
    let mut subscriber = publisher.subscribe();
    let (mut producer, consumer) = command_queue(256);
    let mut engine = Engine::new(body, consumer, publisher);
    let clock = ManualClock::new();

    let mut cycle =
        |engine: &mut Engine<MergeBody>, producer: &mut prism_engine::Producer<_>, index: u16| {
            let executor = ExecutorId::new(u32::from(index % 8) + 1);
            let _ = producer.push(TickCommand::SetExecutorActive {
                executor,
                on: index % 2 == 0,
            });
            let _ = producer.push(TickCommand::SetExecutorLevel {
                executor,
                level: index,
            });
            engine.run_ticks(&clock, 1);
            subscriber.refresh();
        };

    for index in 0..200 {
        cycle(&mut engine, &mut producer, index);
    }
    engine.reset_stats();

    let calls = allocator_calls(|| {
        for index in 0..1_000 {
            cycle(&mut engine, &mut producer, index);
        }
    });

    println!("allocator calls in 1000 merged ticks, {slots} slots, 8 sources: {calls}");
    assert_eq!(calls, 0, "the merge called the allocator {calls} times");
    assert_eq!(engine.stats().ticks, 1_000);
    assert_eq!(engine.stats().commands, 2_000);
    assert_eq!(engine.stats().panics, 0);
    // And it was actually merging: with sources active the values are not the
    // home layer they started at.
    assert!(
        engine.body().values().iter().any(|value| *value != 32_768),
        "the merge produced nothing, so the measurement is meaningless"
    );
}

#[test]
fn a_tick_running_the_encoder_as_well_makes_no_allocator_call_either() {
    // The merge writing a `[u16]` allocates nothing rather easily. This is the
    // whole chain on the tick: resolve, invert, split, and 1536 channel writes
    // spread over four universes, with the frame going out to a driver.
    let head = fixture_type(6, true);
    let layout = FrameLayout::new((1..=4).map(UniverseId::new)).unwrap();
    let body = merge_body(&layout, &head, 128, 8);
    let slots = body.plan().slot_count();
    assert_eq!(slots, 128 * 6);
    assert_eq!(body.channels().target_count(), slots);

    let mut publisher = FramePublisher::new(Arc::new(layout));
    let mut subscriber = publisher.subscribe();
    let (mut producer, consumer) = command_queue(256);
    let mut engine = Engine::new(body, consumer, publisher);
    let clock = ManualClock::new();

    let mut cycle =
        |engine: &mut Engine<MergeBody>, producer: &mut prism_engine::Producer<_>, index: u16| {
            let executor = ExecutorId::new(u32::from(index % 8) + 1);
            let _ = producer.push(TickCommand::SetExecutorActive {
                executor,
                on: index % 2 == 0,
            });
            let _ = producer.push(TickCommand::SetExecutorLevel {
                executor,
                level: index,
            });
            engine.run_ticks(&clock, 1);
            subscriber.refresh();
        };

    for index in 0..200 {
        cycle(&mut engine, &mut producer, index);
    }
    engine.reset_stats();

    let calls = allocator_calls(|| {
        for index in 0..1_000 {
            cycle(&mut engine, &mut producer, index);
        }
    });

    println!("allocator calls in 1000 encoded ticks, {slots} 16-bit slots: {calls}");
    assert_eq!(calls, 0, "the encoder called the allocator {calls} times");
    assert_eq!(engine.stats().ticks, 1_000);
    assert_eq!(engine.stats().panics, 0);
    // And the bytes really travelled: the driver's copy carries what the
    // encoder wrote, in the fixtures' channels and nowhere else.
    let frame = subscriber.frame();
    assert!(
        frame.channels().iter().any(|&byte| byte != 0),
        "nothing was encoded, so the measurement is meaningless"
    );
    let footprint = usize::from(head.footprint);
    let per_universe = UNIVERSE_CHANNELS / footprint;
    for position in 0..4 {
        let universe = frame.universe(position).unwrap();
        let here = per_universe.min(128 - per_universe * position);
        assert!(
            universe[here * footprint..].iter().all(|&byte| byte == 0),
            "the encoder wrote past the patch in universe {position}"
        );
    }
}

#[test]
fn the_probe_notices_an_allocation_when_there_is_one() {
    // A counter that always reports zero would pass every test above. This is
    // the control.
    let calls = allocator_calls(|| {
        let mut values: Vec<u8> = Vec::with_capacity(16);
        values.push(1);
        drop(values);
    });
    assert!(calls >= 2, "expected an alloc and a free, counted {calls}");

    let quiet = allocator_calls(|| {
        let clock = ManualClock::new();
        clock.sleep_until(Duration::from_millis(1));
    });
    assert_eq!(quiet, 0);
}
