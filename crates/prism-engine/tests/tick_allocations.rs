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

use prism_domain::{ExecutorId, UniverseId};
use prism_engine::{
    Clock, DmxFrame, Engine, FrameLayout, FramePublisher, ManualClock, SystemClock, TickBody,
    TickCommand, TickInfo, command_queue,
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
            TickCommand::SetExecutorLevel { .. } | TickCommand::Go { .. } => {}
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
