//! `Patch → tick → triple buffer → driver thread → a datagram somebody
//! received`, for sACN.
//!
//! The unit tests in `sacn.rs` assert the packet a driver builds. This target
//! asserts a packet that was **received**: a `std::net::UdpSocket` bound to
//! `127.0.0.1` is a real socket, so the 638 bytes checked here went out through
//! the operating system's network stack and came back through it. S8's lesson,
//! and `tests/artnet_wire.rs` is where its shape comes from.
//!
//! # Why nothing here multicasts
//!
//! sACN's ordinary destination is `239.255.x.x`, and that is exactly what a test
//! suite must not put on the network it is running on — the same reasoning that
//! keeps Art-Net's broadcast out of the suite. E1.31 permits unicast and this
//! target uses it, so the datagrams stay on the loopback interface. What the
//! group address *is* per universe is asserted in the unit tests, against a
//! socket that records rather than sends.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use prism_domain::{
    AttributeDef, AttributeType, ExecutorId, Fixture, FixtureId, FixtureType, OutputId, UniverseId,
    Vec3,
};
use prism_engine::{
    Clock, Engine, FrameLayout, FramePublisher, ManualClock, MergeBody, TICK_PERIOD, TickCommand,
    command_queue,
};
use prism_protocols::{
    ACN_PACKET_IDENTIFIER, Cid, DMP_PDU_BYTES, E131_DATA_BYTES, E131_DATA_HEADER,
    FRAMING_PDU_BYTES, MockUdp, MockUdpHandle, OPTION_STREAM_TERMINATED, OutputRunner, Priority,
    ROOT_PDU_BYTES, RunnerConfig, SacnConfig, SacnDestination, SacnOutput, SacnPort, SacnUniverse,
    StepOutcome, SystemUdp, TERMINATION_PACKETS, flags_and_length,
};

/// The identity this desk would hold in its show file.
const CID_TEXT: &str = "6f2a1c34-9b5e-4d71-8a03-1e5c7b9d2f48";

/// The same 16-bit moving head the Art-Net and Open DMX targets patch: dimmer
/// on footprint channels 1-2, pan on 3-4, tilt on 5-6. Using the same rig means
/// the three outputs can be compared byte for byte if they ever disagree.
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
/// clock, and an sACN output unicasting to a socket this test is holding.
struct Rig {
    engine: Engine<MergeBody>,
    commands: prism_engine::Producer<TickCommand>,
    runner: OutputRunner<SacnOutput<SystemUdp>, ManualClock>,
    receiver: UdpSocket,
    clock: ManualClock,
}

impl Rig {
    fn new(ports: &[SacnPort]) -> Self {
        let layout = FrameLayout::new([UniverseId::new(1), UniverseId::new(2)]).unwrap();
        let head = moving_head();
        let fixtures = [fixture(1, 1, 1), fixture(2, 2, 1)];
        let patch: Vec<(&Fixture, &FixtureType)> =
            fixtures.iter().map(|fixture| (fixture, &head)).collect();
        let body = MergeBody::for_patch(&layout, patch, (1..=2).map(ExecutorId::new)).unwrap();

        let mut publisher = FramePublisher::new(Arc::new(layout));
        let subscriber = publisher.subscribe();
        let (commands, consumer) = command_queue(16);

        // A real socket standing in for the gateway: bound before the output is
        // built, so the address it unicasts to genuinely exists.
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let config = SacnConfig {
            destination: SacnDestination::Unicast(vec![receiver.local_addr().unwrap()]),
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            ..SacnConfig::source(Cid::parse(CID_TEXT).unwrap(), "PrismDMX Aula")
        };
        let output = SacnOutput::with_ports(
            OutputId::new(1),
            ports.iter().copied(),
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
            receiver,
            clock: ManualClock::new(),
        }
    }

    /// Runs the engine for one tick, then lets the driver take a turn.
    fn tick_and_send(&mut self) -> StepOutcome {
        self.engine.run_ticks(&self.clock, 1);
        self.runner.step()
    }

    /// The next datagram this receiver is given.
    fn receive(&self) -> Vec<u8> {
        let mut buffer = [0u8; 2048];
        let (len, from) = self
            .receiver
            .recv_from(&mut buffer)
            .expect("a datagram must arrive on the loopback socket");
        assert_eq!(from.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        buffer[..len].to_vec()
    }
}

#[test]
fn the_frame_the_engine_publishes_is_the_datagram_that_arrives() {
    let ports = [SacnPort::new(UniverseId::new(1))];
    let mut rig = Rig::new(&ports);
    assert_eq!(rig.tick_and_send(), StepOutcome::Connected);
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);

    let packet = rig.receive();
    assert_eq!(packet.len(), E131_DATA_BYTES);

    // Root layer.
    assert_eq!(&packet[0..2], &[0x00, 0x10], "preamble size");
    assert_eq!(&packet[2..4], &[0x00, 0x00], "post-amble size");
    assert_eq!(&packet[4..16], &ACN_PACKET_IDENTIFIER);
    assert_eq!(&packet[16..18], &flags_and_length(ROOT_PDU_BYTES));
    assert_eq!(&packet[18..22], &[0x00, 0x00, 0x00, 0x04], "root vector");
    assert_eq!(
        &packet[22..38],
        &Cid::parse(CID_TEXT).unwrap().into_bytes(),
        "the desk's identity"
    );

    // Framing layer.
    assert_eq!(&packet[38..40], &flags_and_length(FRAMING_PDU_BYTES));
    assert_eq!(&packet[40..44], &[0x00, 0x00, 0x00, 0x02], "framing vector");
    assert_eq!(&packet[44..57], b"PrismDMX Aula", "source name");
    assert_eq!(&packet[57..108], &[0u8; 51], "and null padding");
    assert_eq!(packet[108], 100, "the default priority");
    assert_eq!(&packet[109..111], &[0x00, 0x00], "no sync address");
    assert_eq!(packet[111], 0, "the first sequence number");
    assert_eq!(packet[112], 0x00, "no options");
    assert_eq!(&packet[113..115], &[0x00, 0x01], "universe 1");

    // DMP layer.
    assert_eq!(&packet[115..117], &flags_and_length(DMP_PDU_BYTES));
    assert_eq!(packet[117], 0x02, "set property");
    assert_eq!(packet[118], 0xA1, "one-octet properties");
    assert_eq!(&packet[119..121], &[0x00, 0x00], "first property address");
    assert_eq!(&packet[121..123], &[0x00, 0x01], "address increment");
    assert_eq!(&packet[123..125], &[0x02, 0x01], "513 values");
    assert_eq!(packet[125], 0x00, "the DMX512 start code");

    // The home layer: dimmer dark, pan and tilt centred at 32768 — coarse 0x80
    // with a fine byte of zero. Listed literally, so a byte leaking into a
    // channel nobody patched fails here.
    assert_eq!(written(&packet[E131_DATA_HEADER..]), [(3, 0x80), (5, 0x80)]);
}

#[test]
fn a_value_the_operator_sets_reaches_the_receiver_on_the_next_frame() {
    let ports = [SacnPort::new(UniverseId::new(1))];
    let mut rig = Rig::new(&ports);
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
        written(&lit[E131_DATA_HEADER..]),
        [(1, 0xFF), (2, 0xFF), (3, 0x80), (5, 0x80)]
    );
    // A changed look is a new datagram, and it carries the next sequence
    // number: a receiver that got them out of order can tell.
    assert_eq!(home[111], 0);
    assert_eq!(lit[111], 1);
}

#[test]
fn every_universe_leaves_as_its_own_e131_universe_at_its_own_priority() {
    // One output serves several universes, and both the E1.31 universe number
    // and the priority are data per universe rather than per output.
    let ports = [
        SacnPort::new(UniverseId::new(1)).at_priority(Priority::new(150).unwrap()),
        SacnPort::new(UniverseId::new(2))
            .at_universe(SacnUniverse::new(63_999).unwrap())
            .at_priority(Priority::new(50).unwrap()),
    ];
    let mut rig = Rig::new(&ports);
    rig.tick_and_send();
    assert_eq!(rig.tick_and_send(), StepOutcome::Sent);
    assert_eq!(rig.runner.unmapped(), []);

    let first = rig.receive();
    let second = rig.receive();
    assert_eq!(&first[113..115], &1u16.to_be_bytes());
    assert_eq!(&second[113..115], &63_999u16.to_be_bytes());
    assert_eq!(first[108], 150);
    assert_eq!(second[108], 50);
    // Both heads are patched at address 1 of their own universe, so the two
    // datagrams carry the same channel data.
    assert_eq!(written(&first[E131_DATA_HEADER..]), [(3, 0x80), (5, 0x80)]);
    assert_eq!(written(&second[E131_DATA_HEADER..]), [(3, 0x80), (5, 0x80)]);
    // One sequence counter per universe, not one per output.
    assert_eq!(first[111], 0);
    assert_eq!(second[111], 0);
}

#[test]
fn stopping_the_driver_ends_the_stream_on_the_wire() {
    // The exit criterion, at the level the daemon will actually shut down at:
    // `OutputRunner::run` returns when a stop is requested and shuts its output
    // down on the way out, and what a receiver then gets is three terminated
    // packets rather than silence and a two-and-a-half-second timeout.
    let ports = [SacnPort::new(UniverseId::new(1))];
    let mut rig = Rig::new(&ports);
    rig.tick_and_send();
    rig.tick_and_send();
    let data = rig.receive();
    assert_eq!(data[112], 0x00, "an ordinary packet has no options set");

    rig.runner.status().request_stop();
    rig.runner.run();

    for expected_sequence in 1..=TERMINATION_PACKETS {
        let packet = rig.receive();
        assert_eq!(packet.len(), E131_DATA_BYTES, "a whole packet");
        assert_eq!(
            packet[112] & OPTION_STREAM_TERMINATED,
            OPTION_STREAM_TERMINATED,
            "options bit 6"
        );
        // Each one moves the sequence on: three identical numbers would be
        // discarded as duplicates and only the first would end anything.
        assert_eq!(packet[111], expected_sequence as u8);
        assert_eq!(&packet[113..115], &1u16.to_be_bytes(), "universe 1");
        assert_eq!(&packet[22..38], &Cid::parse(CID_TEXT).unwrap().into_bytes());
        // The last look, unchanged: whether the stage goes dark is S17's
        // decision, not this driver's.
        assert_eq!(&packet[E131_DATA_HEADER..], &data[E131_DATA_HEADER..]);
    }
    assert_eq!(
        rig.runner.status().health(),
        prism_domain::OutputHealth::Disconnected
    );
}

/// A clock two things can share.
///
/// `ManualClock` is a `Cell` and belongs to one owner, which is right for the
/// engine. Here the runner and the output it drives both have to read the same
/// simulated time — the runner to pace its cadence, the output to decide whether
/// the keep-alive is due — so the time lives behind a mutex and both hold a
/// handle to it. Simulated throughout: the ten seconds below cost microseconds
/// and are exact.
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

/// A runner over an sACN output on a shared simulated clock, with a published
/// frame that never changes again.
fn static_rig(
    config: SacnConfig,
) -> (
    FramePublisher,
    OutputRunner<SacnOutput<MockUdp, SharedClock>, SharedClock>,
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
    let output = SacnOutput::with_clock(
        OutputId::new(1),
        [SacnPort::new(UniverseId::new(1))],
        socket,
        config,
        clock.clone(),
    );
    let runner = OutputRunner::new(output, subscriber, clock, RunnerConfig::default());
    // The publisher goes back to the caller: the frame above is the look the rig
    // is holding, and the engine's end of the channel outliving the test would
    // be a lie about what a driver reads from.
    (publisher, runner, handle)
}

#[test]
fn a_rig_that_is_not_moving_still_reaches_the_receiver_every_second() {
    // E1.31's own keep-alive, asserted where it matters: through the runner, at
    // the cadence the daemon will actually use, on the gap between datagrams
    // rather than on a call count.
    let config = SacnConfig::source(Cid::parse(CID_TEXT).unwrap(), "PrismDMX Aula");
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
        longest_gap <= Duration::from_secs(1),
        "a static universe went {longest_gap:?} without a datagram"
    );
    assert!(
        longest_gap >= Duration::from_millis(900),
        "the refresh is a keep-alive, not a stream: {longest_gap:?}"
    );

    // The runner sent a frame on every one of its cadences, and the network saw
    // about eleven datagrams rather than 440.
    assert_eq!(runner.status().frames_sent(), 440);
    let datagrams = handle.datagram_count();
    assert!(
        (10..=12).contains(&datagrams),
        "{datagrams} datagrams in ten seconds"
    );
    // Multicast is the default and it went to universe 1's group — which is
    // where the address is asserted without a datagram leaving this machine.
    assert_eq!(
        handle.last_datagram().unwrap().0,
        SacnUniverse::new(1).unwrap().multicast_target()
    );
}

#[test]
fn the_cadence_of_an_sacn_output_is_the_engines_own() {
    // ARCHITECTURE_SPEC.md §3.2: sACN keeps up with 44 Hz. Nothing has to be
    // configured for that — the default cadence is the tick period.
    assert_eq!(RunnerConfig::default().cadence, TICK_PERIOD);

    let config = SacnConfig::source(Cid::parse(CID_TEXT).unwrap(), "PrismDMX Aula");
    let (_publisher, mut runner, _) = static_rig(config);
    for step in 0..5u32 {
        runner.step();
        assert_eq!(runner.now(), TICK_PERIOD * step);
    }
}

#[test]
fn an_output_with_no_identity_stays_red() {
    // The CID is not a formality: a source that has not been given one would be
    // indistinguishable from every other desk that was never configured either.
    let (_publisher, mut runner, handle) = static_rig(SacnConfig::default());
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
