//! `Patch → Merge → Frame`, end to end.
//!
//! The unit tests take the chain apart: the plan flattens a patch, the playback
//! layer resolves values, the channel plan turns values into bytes. This target
//! puts it back together and asserts the bytes a real rig would receive — from a
//! patch, through the engine's tick, to what a driver reads out of the triple
//! buffer.
//!
//! It is deliberately literal. Every non-zero channel of every frame is listed,
//! so a change that leaks a byte into a channel nobody patched fails here rather
//! than in a venue.

#![cfg(not(loom))]

use std::sync::Arc;

use prism_domain::{
    AttributeDef, AttributeType, ExecutorId, Fixture, FixtureId, FixtureType, UniverseId, Vec3,
};
use prism_engine::{
    DmxFrame, Engine, FrameLayout, FramePublisher, FrameSubscriber, ManualClock, MergeBody,
    PatchError, TickCommand, command_queue,
};

/// A 16-bit moving head: dimmer on footprint channels 1-2, pan on 3-4, tilt on
/// 5-6. Dimmer homes dark, pan and tilt home centred.
fn moving_head() -> FixtureType {
    fixture_type(
        "test.head",
        6,
        vec![
            attribute(AttributeType::Dimmer, 0, 0, Some(1)),
            attribute(AttributeType::Pan, 32_768, 2, Some(3)),
            attribute(AttributeType::Tilt, 32_768, 4, Some(5)),
        ],
    )
}

/// An 8-bit RGB par: one channel per emitter, all dark at home.
fn rgb_par() -> FixtureType {
    fixture_type(
        "test.par",
        3,
        vec![
            attribute(AttributeType::Red, 0, 0, None),
            attribute(AttributeType::Green, 0, 1, None),
            attribute(AttributeType::Blue, 0, 2, None),
        ],
    )
}

fn attribute(
    attribute: AttributeType,
    default_value: u16,
    coarse_offset: u16,
    fine_offset: Option<u16>,
) -> AttributeDef {
    AttributeDef {
        attribute,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset,
        default_value,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

fn fixture_type(id: &str, footprint: u16, attributes: Vec<AttributeDef>) -> FixtureType {
    FixtureType {
        id: id.to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Test".to_owned(),
        mode: "test".to_owned(),
        footprint,
        attributes,
    }
}

fn fixture(id: u32, type_id: &str, universe: u32, address: u16) -> Fixture {
    Fixture {
        id: FixtureId::new(id),
        name: format!("Fixture {id}"),
        type_id: type_id.to_owned(),
        universe: UniverseId::new(universe),
        address,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        invert_pan: false,
        invert_tilt: false,
    }
}

/// Every channel of the frame that is not zero, as `(universe position,
/// channel, value)` with channels numbered the way an operator numbers them.
fn written(frame: &DmxFrame) -> Vec<(usize, u16, u8)> {
    let mut written = Vec::new();
    for position in 0..frame.universe_count() {
        let Some(universe) = frame.universe(position) else {
            continue;
        };
        for (index, byte) in universe.iter().enumerate() {
            if *byte != 0 {
                written.push((position, index as u16 + 1, *byte));
            }
        }
    }
    written
}

/// The rig: two moving heads in universe 1, the second one hung upside down,
/// and a par in universe 2.
fn rig() -> (FixtureType, FixtureType, Vec<Fixture>) {
    let head = moving_head();
    let par = rgb_par();
    let mut inverted = fixture(2, "test.head", 1, 7);
    inverted.invert_pan = true;
    inverted.invert_tilt = true;
    let fixtures = vec![
        fixture(1, "test.head", 1, 1),
        inverted,
        fixture(3, "test.par", 2, 100),
    ];
    (head, par, fixtures)
}

fn body(
    head: &FixtureType,
    par: &FixtureType,
    fixtures: &[Fixture],
    layout: &FrameLayout,
) -> MergeBody {
    let types: Vec<(&Fixture, &FixtureType)> = fixtures
        .iter()
        .map(|fixture| {
            let fixture_type = if fixture.type_id == head.id {
                head
            } else {
                par
            };
            (fixture, fixture_type)
        })
        .collect();
    MergeBody::for_patch(layout, types, (1..=2).map(ExecutorId::new)).unwrap()
}

/// The engine, its publisher and one driver's view of the output.
struct Rig {
    engine: Engine<MergeBody>,
    producer: prism_engine::Producer<TickCommand>,
    driver: FrameSubscriber,
    clock: ManualClock,
}

impl Rig {
    fn tick(&mut self) -> &DmxFrame {
        self.engine.run_ticks(&self.clock, 1);
        self.driver.refresh();
        self.driver.frame()
    }
}

fn engine(body: MergeBody, layout: FrameLayout) -> Rig {
    let mut publisher = FramePublisher::new(Arc::new(layout));
    let driver = publisher.subscribe();
    let (producer, consumer) = command_queue(16);
    Rig {
        engine: Engine::new(body, consumer, publisher),
        producer,
        driver,
        clock: ManualClock::new(),
    }
}

#[test]
fn the_home_layer_reaches_the_wire_as_bytes() {
    // Nothing is running: every patched channel carries its home value, every
    // other channel is dark, and the two universes do not bleed into each other.
    let layout = FrameLayout::new([UniverseId::new(1), UniverseId::new(2)]).unwrap();
    let (head, par, fixtures) = rig();
    let body = body(&head, &par, &fixtures, &layout);
    let mut rig = engine(body, layout);

    assert_eq!(
        written(rig.tick()),
        [
            // Fixture 1 at address 1: dimmer dark, pan and tilt centred at
            // 32768, which is coarse 0x80 and a fine byte of zero.
            (0, 3, 0x80),
            (0, 5, 0x80),
            // Fixture 2 at address 7, hung upside down: the same centre, seen
            // from the other side, is 65535 - 32768 = 32767.
            (0, 9, 0x7F),
            (0, 10, 0xFF),
            (0, 11, 0x7F),
            (0, 12, 0xFF),
        ],
        "the home layer"
    );
    // The par is at home too, and its home is dark.
    assert_eq!(rig.tick().channel(1, 100), Some(0));
}

#[test]
fn an_active_executor_changes_exactly_the_channels_it_touches() {
    let layout = FrameLayout::new([UniverseId::new(1), UniverseId::new(2)]).unwrap();
    let (head, par, fixtures) = rig();
    let mut body = body(&head, &par, &fixtures, &layout);

    // A cue on executor 1: both heads to full, panned to 20000. It says nothing
    // about tilt, and nothing at all about the par.
    let plan = body.plan().clone();
    let slot = |fixture: u32, attribute| plan.index_of(FixtureId::new(fixture), attribute).unwrap();
    {
        let source = body.layer_mut().source_mut(ExecutorId::new(1)).unwrap();
        for fixture in [1, 2] {
            source.set(slot(fixture, AttributeType::Dimmer), 65_535);
            source.set(slot(fixture, AttributeType::Pan), 20_000);
        }
    }

    let mut rig = engine(body, layout);
    let home = rig.tick().clone();

    rig.producer
        .push(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1),
            on: true,
        })
        .unwrap();
    let live = rig.tick().clone();

    assert_eq!(
        written(&live),
        [
            // Fixture 1: full, and panned to 20000 = 0x4E20.
            (0, 1, 0xFF),
            (0, 2, 0xFF),
            (0, 3, 0x4E),
            (0, 4, 0x20),
            // Tilt was not in the cue, so it is still home.
            (0, 5, 0x80),
            // Fixture 2: the same cue, but hung upside down. Its dimmer is not
            // affected by that - only pan and tilt are - so it is full as well,
            // and its pan is 65535 - 20000 = 45535 = 0xB1DF.
            (0, 7, 0xFF),
            (0, 8, 0xFF),
            (0, 9, 0xB1),
            (0, 10, 0xDF),
            (0, 11, 0x7F),
            (0, 12, 0xFF),
        ],
        "with executor 1 active"
    );

    // Stated as a difference as well: exactly the six channels of the two
    // dimmers and the two pans moved, and nothing else in either universe.
    let moved: Vec<usize> = home
        .channels()
        .iter()
        .zip(live.channels())
        .enumerate()
        .filter_map(|(index, (before, after))| (before != after).then_some(index))
        .collect();
    assert_eq!(moved, [0, 1, 2, 3, 6, 7, 8, 9]);

    // Switching it off puts the rig back exactly where it was: byte-identical,
    // which is the determinism `docs/DMX_MERGE.md` §6.4 asks for.
    rig.producer
        .push(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1),
            on: false,
        })
        .unwrap();
    assert_eq!(written(rig.tick()), written(&home));
}

#[test]
fn htp_between_two_executors_reaches_the_wire() {
    // The merge is S3's, but that it survives the encoder is this session's.
    let layout = FrameLayout::new([UniverseId::new(1), UniverseId::new(2)]).unwrap();
    let (head, par, fixtures) = rig();
    let mut body = body(&head, &par, &fixtures, &layout);
    let plan = body.plan().clone();
    let dimmer = plan
        .index_of(FixtureId::new(1), AttributeType::Dimmer)
        .unwrap();
    let pan = plan
        .index_of(FixtureId::new(1), AttributeType::Pan)
        .unwrap();

    body.layer_mut()
        .source_mut(ExecutorId::new(1))
        .unwrap()
        .set(dimmer, 65_535);
    {
        let second = body.layer_mut().source_mut(ExecutorId::new(2)).unwrap();
        second.set(dimmer, 30_000);
        second.set(pan, 45_000);
    }

    let mut rig = engine(body, layout);
    for executor in [1u32, 2] {
        rig.producer
            .push(TickCommand::SetExecutorActive {
                executor: ExecutorId::new(executor),
                on: true,
            })
            .unwrap();
    }
    // Executor 1's master at half: 65535 x 0.5 = 32767 (§2.1, applied before the
    // maximum), which still beats executor 2's 30000.
    rig.producer
        .push(TickCommand::SetExecutorLevel {
            executor: ExecutorId::new(1),
            level: 32_767,
        })
        .unwrap();

    let frame = rig.tick();
    assert_eq!(
        (frame.channel(0, 1), frame.channel(0, 2)),
        (Some(0x7F), Some(0xFF))
    );
    // Pan is LTP and only executor 2 provides it: 45000 = 0xAFC8.
    assert_eq!(
        (frame.channel(0, 3), frame.channel(0, 4)),
        (Some(0xAF), Some(0xC8))
    );
}

#[test]
fn a_fixture_that_does_not_fit_never_becomes_an_engine() {
    // The address arithmetic is checked once, here, so that the tick never has
    // to. A rejected patch is a patch dialogue error, not a runtime surprise.
    let layout = FrameLayout::new([UniverseId::new(1)]).unwrap();
    let head = moving_head();
    let overrun = fixture(1, "test.head", 1, 508);
    assert_eq!(
        MergeBody::for_patch(&layout, [(&overrun, &head)], []).unwrap_err(),
        PatchError::AddressOutOfRange {
            fixture: FixtureId::new(1),
            address: 508,
            footprint: 6,
        }
    );
    // One channel lower it fits exactly, ending on 512.
    let fits = fixture(1, "test.head", 1, 507);
    let body = MergeBody::for_patch(&layout, [(&fits, &head)], []).unwrap();
    assert_eq!(body.channels().target_count(), 3);

    // And a fixture in a universe that has no frame to be written into is
    // rejected the same way, rather than being silently dropped.
    let elsewhere = fixture(2, "test.head", 9, 1);
    assert_eq!(
        MergeBody::for_patch(&layout, [(&elsewhere, &head)], []).unwrap_err(),
        PatchError::UniverseNotPatched {
            fixture: FixtureId::new(2),
            universe: UniverseId::new(9),
        }
    );
}
