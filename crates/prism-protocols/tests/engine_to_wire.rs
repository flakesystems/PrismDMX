//! `Patch → tick → triple buffer → driver thread → cable`, end to end.
//!
//! The unit tests take the output side apart: the driver builds a packet, the
//! runner reconnects, the mock records calls. This target puts it together with
//! the real engine in front of it and asserts the bytes that leave the port —
//! which is the only place where "the encoder is right" and "the driver is
//! right" become one claim rather than two.
//!
//! It also asserts the property the whole shape exists for: while the cable is
//! unplugged, the engine goes on ticking, publishing and holding its deadline,
//! and does not learn about it.

use std::sync::Arc;

use prism_domain::{
    AttributeDef, AttributeType, ExecutorId, Fixture, FixtureId, FixtureType, OutputHealth,
    OutputId, UniverseId, Vec3,
};
use prism_engine::{
    Engine, FrameLayout, FramePublisher, ManualClock, MergeBody, TickCommand, command_queue,
};
use prism_protocols::{
    BackoffConfig, FtdiError, MockFtdi, MockFtdiHandle, OpenDmxUsb, OutputRunner, RunnerConfig,
    StepOutcome,
};

/// A 16-bit moving head: dimmer on footprint channels 1-2, pan on 3-4, tilt on
/// 5-6. Dimmer homes dark, pan and tilt home centred — the same rig the
/// engine's own `patch_to_frame` target uses, so the two agree on what the
/// bytes should be.
fn moving_head() -> FixtureType {
    FixtureType {
        id: "test.head".to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Test".to_owned(),
        mode: "test".to_owned(),
        footprint: 6,
        attributes: vec![
            attribute(AttributeType::Dimmer, 0, 0, Some(1)),
            attribute(AttributeType::Pan, 32_768, 2, Some(3)),
            attribute(AttributeType::Tilt, 32_768, 4, Some(5)),
        ],
    }
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

fn fixture(id: u32, universe: u32, address: u16) -> Fixture {
    Fixture {
        id: FixtureId::new(id),
        name: format!("Fixture {id}"),
        type_id: "test.head".to_owned(),
        universe: UniverseId::new(universe),
        address,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        invert_pan: false,
        invert_tilt: false,
    }
}

/// One head in universe 1 and one in universe 2, an engine on a simulated
/// clock, and an Open DMX adapter on universe 1 with a mock cable.
struct Rig {
    engine: Engine<MergeBody>,
    commands: prism_engine::Producer<TickCommand>,
    runner: OutputRunner<OpenDmxUsb<MockFtdi>, ManualClock>,
    cable: MockFtdiHandle,
    clock: ManualClock,
}

impl Rig {
    fn new() -> Self {
        let layout = FrameLayout::new([UniverseId::new(1), UniverseId::new(2)]).unwrap();
        let head = moving_head();
        let fixtures = [fixture(1, 1, 1), fixture(2, 2, 1)];
        let patch: Vec<(&Fixture, &FixtureType)> =
            fixtures.iter().map(|fixture| (fixture, &head)).collect();
        let body = MergeBody::for_patch(&layout, patch, (1..=2).map(ExecutorId::new)).unwrap();

        let mut publisher = FramePublisher::new(Arc::new(layout));
        // Attached during set-up: `subscribe` allocates, so it must not happen
        // while the tick is running.
        let subscriber = publisher.subscribe();
        let (commands, consumer) = command_queue(16);

        let ftdi = MockFtdi::new();
        let cable = ftdi.handle();
        let driver = OpenDmxUsb::new(OutputId::new(1), UniverseId::new(1), ftdi);
        let runner = OutputRunner::new(
            driver,
            subscriber,
            ManualClock::new(),
            RunnerConfig {
                cadence: prism_protocols::SH_RS09B.timing.min_frame_interval,
                backoff: BackoffConfig::default(),
            },
        );

        Self {
            engine: Engine::new(body, consumer, publisher),
            commands,
            runner,
            cable,
            clock: ManualClock::new(),
        }
    }

    /// Runs the engine for one tick, then lets the driver take a turn.
    fn tick_and_send(&mut self) -> StepOutcome {
        self.engine.run_ticks(&self.clock, 1);
        self.runner.step()
    }

    /// The channel bytes of the last packet that reached the port, without the
    /// start code.
    fn last_channels(&self) -> Vec<u8> {
        let mut writes = self.cable.writes();
        let packet = writes.pop().expect("a packet must have reached the port");
        assert_eq!(packet.len(), 513);
        assert_eq!(packet[0], 0x00, "the DMX start code is 0x00");
        packet[1..].to_vec()
    }
}

/// Every non-zero channel of a universe, numbered the way an operator numbers
/// them.
fn written(channels: &[u8]) -> Vec<(u16, u8)> {
    channels
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte != 0)
        .map(|(index, byte)| (index as u16 + 1, *byte))
        .collect()
}

#[test]
fn the_frame_the_engine_publishes_is_the_packet_that_leaves_the_port() {
    let mut rig = Rig::new();
    assert_eq!(rig.tick_and_send(), StepOutcome::Connected);
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);

    // The home layer: dimmer dark, pan and tilt centred at 32768 — coarse 0x80
    // with a fine byte of zero. Listed literally, so a byte leaking into a
    // channel nobody patched fails here.
    assert_eq!(rig.last_channels().len(), 512);
    assert_eq!(written(&rig.last_channels()), [(3, 0x80), (5, 0x80)]);

    // And universe 2's head, which is on no cable at all, has not bled into it.
    assert_eq!(rig.runner.unmapped(), []);
}

#[test]
fn a_value_the_operator_sets_reaches_the_cable_on_the_next_frame() {
    let mut rig = Rig::new();
    rig.tick_and_send();

    // Slot 0 of the merge plan is fixture 1's dimmer: the plan is ordered by
    // fixture and then by attribute, which is part of its contract.
    assert!(
        rig.commands
            .push(TickCommand::SetProgrammerValue {
                slot: 0,
                value: 65_535,
            })
            .is_ok()
    );
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);
    assert_eq!(
        written(&rig.last_channels()),
        [(1, 0xFF), (2, 0xFF), (3, 0x80), (5, 0x80)]
    );

    // Half way is 32768: coarse 0x80, fine 0x00 — the same arithmetic
    // `docs/DMX_MERGE.md` §7 quotes.
    assert!(
        rig.commands
            .push(TickCommand::SetProgrammerValue {
                slot: 0,
                value: 32_768,
            })
            .is_ok()
    );
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);
    assert_eq!(
        written(&rig.last_channels()),
        [(1, 0x80), (3, 0x80), (5, 0x80)]
    );
}

#[test]
fn the_cable_being_pulled_mid_show_is_invisible_to_the_engine() {
    let mut rig = Rig::new();
    rig.tick_and_send();
    rig.commands
        .push(TickCommand::SetProgrammerValue {
            slot: 0,
            value: 65_535,
        })
        .unwrap();
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);

    // Unplugged, in the middle of a frame: the break has already gone out. The
    // cable then stays out, so every reconnection finds nothing on the bus.
    rig.cable.fail_write(1, FtdiError::Disconnected);
    rig.cable.fail_open(1000, FtdiError::NotFound);
    assert_eq!(rig.tick_and_send(), StepOutcome::Disconnected);
    assert_eq!(rig.runner.status().health(), OutputHealth::Disconnected);
    // Counted from here: the frame that was interrupted did reach the port as a
    // write call, and it is the frames *after* it that must not.
    let sent_before = rig.cable.writes().len();

    // The show goes on. Six seconds of ticks with no cable attached, and the
    // engine misses nothing, panics not at all, and is never asked to wait.
    rig.engine.reset_stats();
    let mut attempts = 0;
    for tick in 0..264 {
        rig.engine.run_ticks(&rig.clock, 1);
        if rig.runner.step() == StepOutcome::ConnectFailed {
            attempts += 1;
        }
        // Values keep moving while the output is down, so this is a running
        // desk rather than a frozen one.
        rig.commands
            .push(TickCommand::SetProgrammerValue {
                slot: 0,
                value: 32_768 + (tick % 2) * 16,
            })
            .unwrap();
    }
    let stats = rig.engine.stats();
    assert_eq!(stats.ticks, 264);
    assert_eq!(stats.missed, 0);
    assert_eq!(stats.panics, 0);
    assert_eq!(rig.cable.writes().len(), sent_before, "nothing went out");
    // 100 ms → 5 s over six seconds of outage: a handful of attempts, not one
    // per 25 ms cadence, which would have been 240 of them.
    assert!(
        (2..=8).contains(&attempts),
        "{attempts} reconnection attempts in six seconds"
    );

    // Plugged back in: the driver finds it on its own.
    rig.cable.clear_faults();
    let mut steps = 0;
    while rig.runner.status().health() != OutputHealth::Ok {
        rig.engine.run_ticks(&rig.clock, 1);
        rig.runner.step();
        steps += 1;
        assert!(steps < 1000, "the driver never came back");
    }
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);

    // And what goes out is the *current* look, not the one it was holding when
    // the cable was pulled.
    let channels = rig.last_channels();
    assert_eq!(channels[0], 0x80);
    assert!(rig.runner.status().connections() >= 2);
}

#[test]
fn a_second_adapter_on_the_other_universe_gets_its_own_bytes() {
    // Two cables, two universes, one engine. ARCHITECTURE_SPEC.md §7.1 allows
    // exactly one universe per adapter, so this is what a two-universe rig
    // looks like.
    let layout = FrameLayout::new([UniverseId::new(1), UniverseId::new(2)]).unwrap();
    let head = moving_head();
    let fixtures = [fixture(1, 1, 1), fixture(2, 2, 100)];
    let patch: Vec<(&Fixture, &FixtureType)> =
        fixtures.iter().map(|fixture| (fixture, &head)).collect();
    let body = MergeBody::for_patch(&layout, patch, [ExecutorId::new(1)]).unwrap();

    let mut publisher = FramePublisher::new(Arc::new(layout));
    let mut runners = Vec::new();
    let mut cables = Vec::new();
    for (index, universe) in [1u32, 2].into_iter().enumerate() {
        let ftdi = MockFtdi::new();
        cables.push(ftdi.handle());
        let driver = OpenDmxUsb::new(
            OutputId::new(index as u32 + 1),
            UniverseId::new(universe),
            ftdi,
        );
        runners.push(OutputRunner::new(
            driver,
            publisher.subscribe(),
            ManualClock::new(),
            RunnerConfig::for_profile(&prism_protocols::SH_RS09B),
        ));
    }

    let (mut commands, consumer) = command_queue(16);
    let mut engine = Engine::new(body, consumer, publisher);
    let clock = ManualClock::new();
    // Fixture 2's dimmer is slot 3: three attributes each, ordered by fixture.
    commands
        .push(TickCommand::SetProgrammerValue {
            slot: 3,
            value: 65_535,
        })
        .unwrap();
    for _ in 0..3 {
        engine.run_ticks(&clock, 1);
        for runner in &mut runners {
            runner.step();
        }
    }

    // Universe 1: the head at address 1, at home. Universe 2: the head at
    // address 100, at full.
    let first = cables[0].writes().pop().unwrap();
    let second = cables[1].writes().pop().unwrap();
    assert_eq!(written(&first[1..]), [(3, 0x80), (5, 0x80)]);
    assert_eq!(
        written(&second[1..]),
        [(100, 0xFF), (101, 0xFF), (102, 0x80), (104, 0x80)]
    );
}
