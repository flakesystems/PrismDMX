//! `Patch → tick → triple buffer → driver thread → a datagram somebody
//! received`, end to end.
//!
//! The unit tests in `artnet.rs` assert the packet a driver builds. This target
//! asserts a packet that was **received**: a `std::net::UdpSocket` bound to
//! `127.0.0.1` is a real socket, so the bytes checked here went out through the
//! operating system's network stack and came back through it.
//!
//! That distinction is S8's lesson, paid for once already. A mock confirms the
//! calls a driver makes and says nothing about what actually leaves the machine;
//! for a network output the capture is cheap enough that there is no excuse for
//! not taking one. Nothing here needs a network *device* — loopback is not the
//! network, and no test in this repository sends a datagram anywhere else.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use prism_domain::{
    AttributeDef, AttributeType, Fixture, FixtureId, FixtureType, OutputId, SequenceId, UniverseId,
    Vec3,
};
use prism_engine::{
    Clock, Engine, FrameLayout, FramePublisher, ManualClock, MergeBody, TICK_PERIOD, TickCommand,
    command_queue,
};
use prism_protocols::{
    ART_DMX_BYTES, ART_DMX_HEADER, ART_NET_ID, ArtNetConfig, ArtNetOutput, MockUdp, MockUdpHandle,
    OutputRunner, PortAddress, RunnerConfig, StepOutcome, SystemUdp,
};

/// The same 16-bit moving head the Open DMX target patches: dimmer on footprint
/// channels 1-2, pan on 3-4, tilt on 5-6. Using the same rig means the two
/// outputs can be compared byte for byte if they ever disagree.
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
        ranges: Vec::new(),
    }
}

fn fixture(id: u32, universe: u32, address: u16) -> Fixture {
    Fixture {
        software_dimmer: true,
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

/// One head in universe 1 and one in universe 2, an engine on a simulated
/// clock, and an Art-Net output unicasting to a socket this test is holding.
struct Rig {
    engine: Engine<MergeBody>,
    commands: prism_engine::Producer<TickCommand>,
    runner: OutputRunner<ArtNetOutput<SystemUdp>, ManualClock>,
    node: UdpSocket,
    clock: ManualClock,
}

impl Rig {
    fn new(universes: &[u32]) -> Self {
        let layout = FrameLayout::new([UniverseId::new(1), UniverseId::new(2)]).unwrap();
        let head = moving_head();
        let fixtures = [fixture(1, 1, 1), fixture(2, 2, 1)];
        let patch: Vec<(&Fixture, &FixtureType)> =
            fixtures.iter().map(|fixture| (fixture, &head)).collect();
        let body = MergeBody::for_patch(&layout, patch, (1..=2).map(SequenceId::new)).unwrap();

        let mut publisher = FramePublisher::new(Arc::new(layout));
        let subscriber = publisher.subscribe();
        let (commands, consumer) = command_queue(16);

        // A real socket, standing in for the Art-Net node: it is bound before
        // the output is built, so the address the output unicasts to is one
        // that genuinely exists.
        let node = UdpSocket::bind("127.0.0.1:0").unwrap();
        node.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let config = ArtNetConfig {
            destination: prism_protocols::Destination::Unicast(vec![node.local_addr().unwrap()]),
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            ..ArtNetConfig::default()
        };
        let output = ArtNetOutput::new(
            OutputId::new(1),
            universes.iter().copied().map(UniverseId::new),
            SystemUdp::new(),
            config,
        );
        let runner = OutputRunner::new(
            output,
            subscriber,
            ManualClock::new(),
            RunnerConfig::default(),
        );

        Self {
            engine: Engine::new(body, consumer, publisher),
            commands,
            runner,
            node,
            clock: ManualClock::new(),
        }
    }

    /// Runs the engine for one tick, then lets the driver take a turn.
    fn tick_and_send(&mut self) -> StepOutcome {
        self.engine.run_ticks(&self.clock, 1);
        self.runner.step()
    }

    /// The next datagram this node receives.
    fn receive(&self) -> Vec<u8> {
        let mut buffer = [0u8; 2048];
        let (len, from) = self
            .node
            .recv_from(&mut buffer)
            .expect("a datagram must arrive on the loopback socket");
        assert_eq!(from.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        buffer[..len].to_vec()
    }
}

#[test]
fn the_frame_the_engine_publishes_is_the_datagram_that_arrives() {
    let mut rig = Rig::new(&[1]);
    assert_eq!(rig.tick_and_send(), StepOutcome::Connected);
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);

    let packet = rig.receive();
    assert_eq!(packet.len(), ART_DMX_BYTES);
    assert_eq!(&packet[0..8], &ART_NET_ID);
    assert_eq!(&packet[8..10], &[0x00, 0x50], "OpDmx, low byte first");
    assert_eq!(&packet[10..12], &[0x00, 14], "protocol version 14");
    assert_eq!(packet[12], 1, "the first sequence number is 1");
    assert_eq!(packet[13], 0, "physical");
    assert_eq!(packet[14], 0, "universe 1 is port address 0");
    assert_eq!(packet[15], 0, "net");
    assert_eq!(&packet[16..18], &[0x02, 0x00], "512 channels");

    // The home layer: dimmer dark, pan and tilt centred at 32768 — coarse 0x80
    // with a fine byte of zero. Listed literally, so a byte leaking into a
    // channel nobody patched fails here.
    assert_eq!(written(&packet[ART_DMX_HEADER..]), [(3, 0x80), (5, 0x80)]);
}

#[test]
fn a_value_the_operator_sets_reaches_the_node_on_the_next_frame() {
    let mut rig = Rig::new(&[1]);
    rig.tick_and_send();
    rig.tick_and_send();
    let home = rig.receive();

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
    let lit = rig.receive();
    assert_eq!(
        written(&lit[ART_DMX_HEADER..]),
        [(1, 0xFF), (2, 0xFF), (3, 0x80), (5, 0x80)]
    );
    // A changed look is a new datagram, and it carries the next sequence
    // number: a node that received them out of order can tell.
    assert_eq!(home[12], 1);
    assert_eq!(lit[12], 2);
}

#[test]
fn every_universe_the_output_carries_leaves_on_its_own_port_address() {
    // Unlike an Open DMX cable, one Art-Net output serves several universes —
    // and the runner already sends every universe an output declares.
    let mut rig = Rig::new(&[1, 2]);
    rig.tick_and_send();
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);
    assert_eq!(rig.runner.unmapped(), []);

    let first = rig.receive();
    let second = rig.receive();
    assert_eq!(first[14], 0, "universe 1 → port address 0");
    assert_eq!(second[14], 1, "universe 2 → port address 1");
    // Both heads are patched at address 1 of their own universe, so the two
    // datagrams carry the same channel data at different addresses.
    assert_eq!(written(&first[ART_DMX_HEADER..]), [(3, 0x80), (5, 0x80)]);
    assert_eq!(written(&second[ART_DMX_HEADER..]), [(3, 0x80), (5, 0x80)]);
    // One sequence counter per port address, not one per output.
    assert_eq!(first[12], 1);
    assert_eq!(second[12], 1);
}

/// A clock two things can share.
///
/// `ManualClock` is a `Cell` and belongs to one owner, which is right for the
/// engine. Here the runner and the output it drives both have to read the same
/// simulated time — the runner to pace its cadence, the output to decide
/// whether the 800 ms refresh is due — so the time lives behind a mutex and
/// both hold a handle to it. Simulated throughout: the ten seconds below cost
/// microseconds and are exact.
#[derive(Clone, Default)]
struct SharedClock(Arc<Mutex<Duration>>);

impl Clock for SharedClock {
    fn now(&self) -> Duration {
        *self.0.lock().unwrap()
    }

    fn sleep_until(&self, deadline: Duration) {
        let mut now = self.0.lock().unwrap();
        if deadline > *now {
            *now = deadline;
        }
    }
}

/// A runner over an Art-Net output on a shared simulated clock, with a
/// published frame that never changes again.
fn static_rig(
    config: ArtNetConfig,
) -> (
    FramePublisher,
    OutputRunner<ArtNetOutput<MockUdp, SharedClock>, SharedClock>,
    MockUdpHandle,
) {
    let layout = FrameLayout::new([UniverseId::new(1)]).unwrap();
    let mut publisher = FramePublisher::new(Arc::new(layout));
    let subscriber = publisher.subscribe();
    publisher.frame_mut().universe_mut(0).unwrap().fill(0x77);
    publisher.publish();

    let socket = MockUdp::new();
    let handle = socket.handle();
    let clock = SharedClock::default();
    let output = ArtNetOutput::with_clock(
        OutputId::new(1),
        [(
            UniverseId::new(1),
            PortAddress::for_universe(UniverseId::new(1)),
        )],
        socket,
        config,
        clock.clone(),
    );
    let runner = OutputRunner::new(output, subscriber, clock, RunnerConfig::default());
    // The publisher goes back to the caller: the frame above is the look the
    // rig is holding, and the engine's end of the channel outliving the test
    // would be a lie about what a driver reads from.
    (publisher, runner, handle)
}

#[test]
fn a_rig_that_is_not_moving_still_reaches_the_node_every_eight_hundred_milliseconds() {
    // ARCHITECTURE_SPEC.md §7.2's forced refresh, asserted where it matters:
    // through the runner, at the cadence the daemon will actually use, on the
    // gap between datagrams rather than on a call count.
    let config = ArtNetConfig::unicast([IpAddr::V4(Ipv4Addr::LOCALHOST)]);
    let (_publisher, mut runner, handle) = static_rig(config);

    let mut previous: Option<Duration> = None;
    let mut longest_gap = Duration::ZERO;
    // Ten seconds at the engine's own tick period.
    for _ in 0..441 {
        let before = handle.datagram_count();
        runner.step();
        if handle.datagram_count() > before {
            let now = runner.now();
            if let Some(previous) = previous {
                longest_gap = longest_gap.max(now - previous);
            }
            previous = Some(now);
        }
    }

    assert!(
        longest_gap <= Duration::from_millis(800),
        "a static universe went {longest_gap:?} without a datagram"
    );
    assert!(
        longest_gap >= Duration::from_millis(700),
        "the refresh is a keep-alive, not a stream: {longest_gap:?}"
    );

    // The other half of §7.2: the runner sent a frame on every one of its
    // cadences, and the network saw about thirteen datagrams rather than 440.
    // That difference is what keeps a school's switch usable.
    assert_eq!(runner.status().frames_sent(), 440);
    let datagrams = handle.datagram_count();
    assert!(
        (12..=14).contains(&datagrams),
        "{datagrams} datagrams in ten seconds"
    );
}

#[test]
fn the_cadence_of_a_network_output_is_the_engines_own() {
    // ARCHITECTURE_SPEC.md §3.2: Art-Net keeps up with 44 Hz, which is the
    // whole reason S9 follows S8. Nothing has to be configured for that — the
    // default cadence is the tick period.
    assert_eq!(RunnerConfig::default().cadence, TICK_PERIOD);

    let config = ArtNetConfig::unicast([IpAddr::V4(Ipv4Addr::LOCALHOST)]);
    let (_publisher, mut runner, _) = static_rig(config);
    for step in 0..5u32 {
        runner.step();
        assert_eq!(runner.now(), TICK_PERIOD * step);
    }
}

#[test]
fn an_output_that_is_never_told_where_to_send_stays_red() {
    // The default destination is unicast to nobody, so a configuration nobody
    // finished is a disconnected output rather than a broadcasting one.
    let (_publisher, mut runner, handle) = static_rig(ArtNetConfig::default());
    for _ in 0..20 {
        assert_ne!(runner.step(), StepOutcome::Sent);
    }
    assert_eq!(
        runner.status().health(),
        prism_domain::OutputHealth::Disconnected
    );
    assert_eq!(handle.datagram_count(), 0);
    assert!(handle.binds().is_empty(), "no socket was ever opened");
}
