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
    AttributeDef, AttributeType, Cue, CuePart, CueTrigger, ExecutorId, Fixture, FixtureId,
    FixtureType, GoDirection, Group, GroupId, Sequence, SequenceId, UniverseId, Vec3,
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
            | TickCommand::SetExecutorFlash { .. }
            | TickCommand::SetExecutorSpeed { .. }
            | TickCommand::TapExecutorSpeed { .. }
            | TickCommand::SetExecutorXFade { .. }
            | TickCommand::Go { .. }
            | TickCommand::GotoCue { .. }
            | TickCommand::SetGroupMaster { .. }
            | TickCommand::SetProgrammerValue { .. }
            | TickCommand::ClearProgrammerValue { .. }
            | TickCommand::ClearProgrammer => {}
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
        executor: ExecutorId::new(u32::from(index)).into(),
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

/// S33: an output added, removed and re-addressed while the show runs, and the
/// tick that takes it on calls the allocator **no** more than the ones around
/// it — which is zero.
///
/// The whole of the hot-reconfiguration claim is here. `FrameEnrolment` puts the
/// arriving subscriber's buffer where the tick can take it with one atomic load
/// and a `try_lock`, and takes the departing one's back so that it is freed on
/// this thread rather than on the tick's. If either half were done the obvious
/// way — `Vec::push` on a full vector, or dropping the link where it was
/// removed — this test is what would say so.
#[test]
fn taking_a_subscriber_on_and_giving_one_up_costs_the_tick_no_allocation() {
    let mut harness = harness(64, 4);
    let clock = ManualClock::new();
    let enrolment = harness.engine.publisher_mut().enrolment();

    for index in 0..200 {
        cycle(&mut harness, &clock, index);
    }
    harness.engine.reset_stats();

    // Everything the *daemon* would do is done here, off the measured region:
    // the buffers are allocated by whoever asks for the output, never by the
    // tick. What is measured is the tick that picks them up.
    let mut arriving = Vec::new();
    for _ in 0..8 {
        arriving.push(enrolment.subscribe().unwrap());
    }
    let leaving: Vec<_> = arriving.drain(..4).collect();
    for subscriber in &leaving {
        enrolment.unsubscribe(subscriber.id());
    }

    let calls = allocator_calls(|| {
        // The first of these is the tick that takes on four and gives up four
        // at once; the rest are ordinary ticks with twice the subscribers.
        for index in 0..100 {
            cycle(&mut harness, &clock, index);
        }
    });

    println!("allocator calls in the tick that took on 4 subscribers and gave up 4: {calls}");
    assert_eq!(calls, 0, "the tick called the allocator {calls} times");
    // And the frames really did change hands: the four that stayed are reading,
    // the four that left are not.
    for subscriber in &mut arriving {
        assert!(subscriber.refresh() || subscriber.frame().sequence() > 0);
    }
    let freed = enrolment.collect();
    assert_eq!(
        freed, 4,
        "the departed buffers are freed here, not on the tick"
    );
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
            invert_pan: index.is_multiple_of(2),
            invert_tilt: index.is_multiple_of(2),
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
                executor: executor.into(),
                on: index.is_multiple_of(2),
            });
            let _ = producer.push(TickCommand::SetExecutorLevel {
                executor: executor.into(),
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
                executor: executor.into(),
                on: index.is_multiple_of(2),
            });
            let _ = producer.push(TickCommand::SetExecutorLevel {
                executor: executor.into(),
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

/// A cue list whose every cue touches every attribute of every fixture, fading
/// over ten seconds and following on by itself. The worst case a player can be
/// given: nothing in it is idle, and the fades are still running at the end of
/// the measured window.
fn loaded_sequence(head: &FixtureType, fixtures: u32, seed: u16) -> Sequence {
    let cues = (0..4u16)
        .map(|number| Cue {
            number: number.to_string(),
            name: String::new(),
            fade_in: 10.0,
            fade_out: 10.0,
            delay: 0.0,
            // Every cue but the first follows on by itself, so cue traversal is
            // on the measured tick and not only in the commands.
            trigger: if number == 0 {
                CueTrigger::Go
            } else {
                CueTrigger::Follow
            },
            trigger_time: None,
            parts: (1..=fixtures)
                .flat_map(|fixture| {
                    head.attributes.iter().map(move |def| CuePart {
                        fixture: FixtureId::new(fixture),
                        attribute: def.attribute,
                        value: fixture
                            .wrapping_mul(u32::from(number))
                            .wrapping_add(u32::from(seed)) as u16,
                        preset_ref: None,
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
        is_active: false,
        current_cue_index: None,
    }
}

#[test]
fn a_tick_with_cues_and_running_fades_makes_no_allocator_call_either() {
    // The session's own criterion. A `prism_domain::Cue` owns a String for its
    // number, a String for its name and a Vec of parts; dropping one inside the
    // tick would call the allocator just as surely as building one. This counts
    // what actually happens with eight cue lists running, every one of them
    // part way through a fade over every slot in the patch.
    let head = fixture_type(6, false);
    let layout = FrameLayout::new((1..=8).map(UniverseId::new)).unwrap();
    let patched = patch(&layout, &head, 128);
    let mut body = MergeBody::for_patch(
        &layout,
        patched.iter().map(|fixture| (fixture, &head)),
        (1..=8).map(ExecutorId::new),
    )
    .unwrap();
    let slots = body.plan().slot_count();
    assert_eq!(slots, 128 * 6);
    for executor in 1..=8u32 {
        body.load_sequence(
            ExecutorId::new(executor),
            &loaded_sequence(&head, 128, executor as u16),
        )
        .unwrap();
    }

    let mut publisher = FramePublisher::new(Arc::new(layout));
    let mut subscriber = publisher.subscribe();
    let (mut producer, consumer) = command_queue(256);
    let mut engine = Engine::new(body, consumer, publisher);
    let clock = ManualClock::new();

    let mut cycle =
        |engine: &mut Engine<MergeBody>, producer: &mut prism_engine::Producer<_>, index: u16| {
            let executor = ExecutorId::new(u32::from(index % 8) + 1);
            let _ = producer.push(TickCommand::Go {
                executor: executor.into(),
                direction: if index % 16 < 8 {
                    GoDirection::Next
                } else {
                    GoDirection::Prev
                },
            });
            // And an executor going off and on underneath the fades, so the
            // release path and the activation path are measured too.
            let _ = producer.push(TickCommand::SetExecutorActive {
                executor: ExecutorId::new(u32::from(index % 8) + 1).into(),
                on: !index.is_multiple_of(32),
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

    println!("allocator calls in 1000 ticks with 8 cue lists fading over {slots} slots: {calls}");
    assert_eq!(calls, 0, "playback called the allocator {calls} times");
    assert_eq!(engine.stats().ticks, 1_000);
    assert_eq!(engine.stats().commands, 2_000);
    assert_eq!(engine.stats().panics, 0);

    // And the fades really were running: a further tick moves values that were
    // part way through a fade, so this was not a measurement of a rig at rest.
    let before = engine.body().values().to_vec();
    engine.run_ticks(&clock, 1);
    assert!(
        engine.body().values() != before.as_slice(),
        "nothing was fading, so the measurement is meaningless"
    );
    assert!(
        engine
            .body()
            .cues()
            .players()
            .iter()
            .any(|player| player.current_cue().is_some()),
        "no cue was running, so the measurement is meaningless"
    );
}

#[test]
fn a_tick_with_the_programmer_and_the_masters_makes_no_allocator_call_either() {
    // S6's own criterion, added beside the earlier ones rather than replacing
    // them. `prism_domain::ProgrammerState` owns a `BTreeMap` per fixture, so a
    // programmer that reached the tick in that form would allocate on every
    // touch and free on every clear. This counts what the flat, slot-addressed
    // layer actually does, with values arriving and being cleared on the tick,
    // group masters and the grand master moving, and the whole thing scaled and
    // encoded every frame.
    let head = fixture_type(6, false);
    let layout = FrameLayout::new((1..=8).map(UniverseId::new)).unwrap();
    let patched = patch(&layout, &head, 128);
    let mut body = MergeBody::for_patch(
        &layout,
        patched.iter().map(|fixture| (fixture, &head)),
        (1..=8).map(ExecutorId::new),
    )
    .unwrap();
    let slots = body.plan().slot_count();
    assert_eq!(slots, 128 * 6);
    for executor in 1..=8u32 {
        body.load_sequence(
            ExecutorId::new(executor),
            &loaded_sequence(&head, 128, executor as u16),
        )
        .unwrap();
    }
    // Sixteen groups over the whole rig, every fixture in two of them, so the
    // group walk on the tick is neither empty nor trivial.
    let groups: Vec<Group> = (0..16u32)
        .map(|index| Group {
            id: GroupId::new(index + 1),
            name: String::new(),
            fixtures: (1..=128u32)
                .filter(|fixture| fixture % 8 == index % 8)
                .map(FixtureId::new)
                .collect(),
        })
        .collect();
    body.load_groups(&groups);
    assert_eq!(body.masters().group_count(), 16);
    assert_eq!(body.masters().intensity_slots().len(), 128);

    let mut publisher = FramePublisher::new(Arc::new(layout));
    let mut subscriber = publisher.subscribe();
    let (mut producer, consumer) = command_queue(256);
    let mut engine = Engine::new(body, consumer, publisher);
    let clock = ManualClock::new();

    let mut cycle =
        |engine: &mut Engine<MergeBody>, producer: &mut prism_engine::Producer<_>, index: u16| {
            let executor = ExecutorId::new(u32::from(index % 8) + 1);
            let _ = producer.push(TickCommand::Go {
                executor: executor.into(),
                direction: GoDirection::Next,
            });
            // The programmer filling up and being cleared out again, which is
            // the operation a naive implementation would allocate for.
            let slot = u32::from(index) % 768;
            let _ = producer.push(if index.is_multiple_of(5) {
                TickCommand::ClearProgrammerValue { slot }
            } else if index.is_multiple_of(64) {
                TickCommand::ClearProgrammer
            } else {
                TickCommand::SetProgrammerValue { slot, value: index }
            });
            let _ = producer.push(TickCommand::SetGroupMaster {
                group: GroupId::new(u32::from(index % 16) + 1),
                level: index,
            });
            let _ = producer.push(TickCommand::SetGrandMaster(index | 0x8000));
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

    println!(
        "allocator calls in 1000 ticks with programmer and masters over {slots} slots: {calls}"
    );
    assert_eq!(
        calls, 0,
        "the programmer or the masters called the allocator {calls} times"
    );
    assert_eq!(engine.stats().ticks, 1_000);
    assert_eq!(engine.stats().commands, 4_000);
    assert_eq!(engine.stats().panics, 0);

    // And all three layers were really doing something: the programmer holds
    // values, the masters are down, and the frame is not the home layer.
    assert!(
        !engine.body().programmer().is_empty(),
        "the programmer was empty, so the measurement is meaningless"
    );
    assert!(
        engine.body().masters().grand() != u16::MAX,
        "the grand master was at full, so the masters did nothing"
    );
    assert!(
        subscriber.frame().channels().iter().any(|&byte| byte != 0),
        "nothing reached the frame, so the measurement is meaningless"
    );
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

/// **S34's criterion.** The channel back out of the tick, the flash layer, the
/// speed master, the tap and the manual crossfade — all of it inside the same
/// measured window, on top of everything the four measurements above already
/// carry.
///
/// The readback is the one that had to be counted rather than argued: a channel
/// that pushed a message per transition would allocate on a queue, and one that
/// answered with a `Vec` of running executors would allocate per tick. This is a
/// table of atomics written in place, and the number below is what says so.
#[test]
fn a_tick_publishing_its_playbacks_makes_no_allocator_call_either() {
    let head = fixture_type(6, false);
    let layout = FrameLayout::new((1..=8).map(UniverseId::new)).unwrap();
    let patched = patch(&layout, &head, 128);
    let mut body = MergeBody::for_patch(
        &layout,
        patched.iter().map(|fixture| (fixture, &head)),
        (1..=8).map(ExecutorId::new),
    )
    .unwrap();
    for executor in 1..=8u32 {
        body.load_sequence(
            ExecutorId::new(executor),
            &loaded_sequence(&head, 128, executor as u16),
        )
        .unwrap();
    }
    // The report is built at the size the daemon builds it, because a report
    // that had to grow would allocate on the thread that grew it — and that
    // thread is this one.
    let report = Arc::new(prism_engine::PlaybackReport::new(prism_engine::MAX_SOURCES));
    body.report_into(Arc::clone(&report));

    let mut publisher = FramePublisher::new(Arc::new(layout));
    let mut subscriber = publisher.subscribe();
    let (mut producer, consumer) = command_queue(256);
    let mut engine = Engine::new(body, consumer, publisher);
    let clock = ManualClock::new();

    let mut cycle =
        |engine: &mut Engine<MergeBody>, producer: &mut prism_engine::Producer<_>, index: u16| {
            let executor = ExecutorId::new(u32::from(index % 8) + 1);
            let _ = producer.push(TickCommand::Go {
                executor: executor.into(),
                direction: GoDirection::Next,
            });
            // Every S34 command, in rotation, so none of them is measured only
            // in the tick that happened to be quiet.
            let _ = producer.push(match index % 4 {
                0 => TickCommand::SetExecutorFlash {
                    executor: executor.into(),
                    on: index.is_multiple_of(8),
                },
                1 => TickCommand::SetExecutorSpeed {
                    executor: executor.into(),
                    speed: 512 + index % 2_048,
                },
                2 => TickCommand::TapExecutorSpeed {
                    executor: executor.into(),
                },
                _ => TickCommand::SetExecutorXFade {
                    executor: executor.into(),
                    position: index.wrapping_mul(577),
                },
            });
            let _ = producer.push(TickCommand::SetProgrammerValue {
                slot: u32::from(index) % 768,
                value: index,
            });
            let _ = producer.push(TickCommand::SetGrandMaster(index | 0x8000));
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

    println!("allocator calls in 1000 ticks with the readback and S34's commands: {calls}");
    assert_eq!(
        calls, 0,
        "the readback or one of S34's commands called the allocator {calls} times"
    );
    assert_eq!(engine.stats().ticks, 1_000);
    assert_eq!(engine.stats().commands, 4_000);
    assert_eq!(engine.stats().panics, 0);

    // And the measurement was of something happening: eight playbacks reported,
    // every one of them on a cue, and the frame is not the home layer.
    assert_eq!(report.len(), 8);
    assert!(
        report.states().all(|state| state.cue_index.is_some()),
        "nothing was running, so the readback was never exercised"
    );
    assert!(
        report.states().any(|state| state.is_active),
        "the report says nothing is active"
    );
    assert!(
        subscriber.frame().channels().iter().any(|&byte| byte != 0),
        "nothing reached the frame, so the measurement is meaningless"
    );
}
