//! The tracker receiver: PSN off the network and into the table the tick reads -
//! **S32**.
//!
//! `ARCHITECTURE_SPEC.md` §8 and §3's `psn-osc-in` row: *tracker positions into a
//! ring buffer, latest value only*. [`crate::psn`] is the codec, and this is the
//! conversation around it - one socket, one pass at a time - which keeps three
//! things.
//!
//! ```text
//!   tracking system ──PSN──▶ [`TrackerReceiver`] ──mapped, show space──▶ `TrackerTable`
//!    (multicast group)          │                                          (the tick reads)
//!                               └─ rows()  what has been heard, for the settings panel
//! ```
//!
//! # What is written to the table, and what to the rows
//!
//! Every position is **mapped** ([`prism_domain::TrackerMapping`]) and then
//! written to the `TrackerTable` the tick reads - the latest of each tracker and
//! nothing else, so a tracker that sends a hundred times between two ticks costs
//! the tick one read. The *rows* are the same positions for people: the panel's
//! list of what is out there, with each tracker's name from its info packet, its
//! position in show space, and how long ago it last spoke.
//!
//! # The timeout is a *warning*, never a decision about the head
//!
//! `ARCHITECTURE_SPEC.md` §8: on a 500 ms timeout the last position is **held**,
//! a warning appears, and **nothing jumps**. The holding is free - the table
//! keeps the last position of a tracker that has gone quiet, so the tick has
//! nothing to do - and the warning is [`TrackerReceiver::health`], which compares
//! a row's age with the configured timeout. Neither moves a head, which is why
//! the tick needs no clock for it.
//!
//! # It is a stranger's bytes
//!
//! The group is one anybody on the network can write to. [`crate::psn::decode`]
//! reads without allocating and without panicking; this counts what it refuses
//! ([`TrackingCounters`]) and goes on, and the table of rows is **bounded** - a
//! stream of made-up tracker numbers must not be able to grow it. A position the
//! table cannot hold (not a number, a kilometre away) is counted as refused and
//! **leaves the last good one standing**.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use prism_domain::{MAX_TRACKER, TrackerHealth, TrackerMapping, Vec3};
use prism_engine::{Clock, SystemClock, TrackerTable};
use std::sync::Arc;

use crate::psn::{self, Event, MAX_PACKET};
use crate::udp::{UdpError, UdpNode};

/// Where the receiver listens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listen {
    /// A multicast group, joined on an interface - what a PSN system sends to.
    Multicast {
        /// The group, `236.10.10.10` by default.
        group: Ipv4Addr,
        /// The UDP port.
        port: u16,
        /// The interface to join on, or the operating system's choice.
        interface: Option<Ipv4Addr>,
    },
    /// One address, listened on directly: a tracker that sends to this machine
    /// alone, and the way a test receives one without putting a multicast
    /// datagram on the network it runs on.
    Unicast(SocketAddr),
}

/// How a receiver behaves.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackingConfig {
    /// Where it listens.
    pub listen: Listen,
    /// How the system's axes line up with the stage.
    pub mapping: TrackerMapping,
    /// How long a tracker may be quiet before it reads as such.
    pub timeout: Duration,
    /// The longest one [`TrackerReceiver::service`] pass waits for a datagram -
    /// what bounds how long the thread takes to notice it was asked to stop.
    pub wait: Duration,
    /// Datagrams read in one pass before it ends, so that a flood cannot hold
    /// the thread inside one pass for ever.
    pub recv_budget: usize,
    /// The most trackers the rows will hold.
    pub max_rows: usize,
}

impl TrackingConfig {
    /// A receiver on `listen` with the usual bounds.
    #[must_use]
    pub fn new(listen: Listen, mapping: TrackerMapping, timeout: Duration) -> Self {
        Self {
            listen,
            mapping,
            timeout,
            wait: Duration::from_millis(50),
            recv_budget: 128,
            max_rows: usize::from(MAX_TRACKER) + 1,
        }
    }
}

/// What a receiver has been sent and what it did with it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrackingCounters {
    /// Datagrams read.
    pub datagrams: u64,
    /// Positions taken into the table.
    pub positions: u64,
    /// Datagrams that were not PSN, or ran off their own end. **The one an
    /// installer is sent to look at**: a tracking system that "sends" and is
    /// never seen is, nine times in ten, this counting up - a different
    /// protocol, PSN version 1, or another system on the same group.
    pub rejected: u64,
    /// Positions the table would not hold: not a number, or a kilometre away.
    pub refused: u64,
    /// Positions for a tracker past the table - `MAX_TRACKER`.
    pub out_of_range: u64,
    /// Reads the socket refused.
    pub read_errors: u64,
}

/// One tracker this receiver has heard.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackerRow {
    /// The tracker's number.
    pub id: u16,
    /// Its name, once an info packet has said.
    pub name: Option<String>,
    /// Where it last was, in show space.
    pub position: Vec3,
    /// When it last spoke, on the receiver's clock.
    pub last_seen: Duration,
    /// How many positions it has sent.
    pub positions: u64,
}

/// Listens for PSN and keeps the table of where each tracker is.
///
/// The type parameters are the socket and the clock, for
/// [`NodeDiscovery`](crate::NodeDiscovery)'s reason: a `MockUdpNode` makes the
/// conversation assertable with no network, and a `ManualClock` makes *quiet for
/// half a second* assertable without waiting half a second.
pub struct TrackerReceiver<S: UdpNode, C: Clock = SystemClock> {
    socket: S,
    clock: C,
    config: TrackingConfig,
    table: Arc<TrackerTable>,
    open: bool,
    /// Heard trackers, in the order they were first heard.
    rows: Vec<TrackerRow>,
    /// What the system calls itself, once an info packet has said.
    system: Option<String>,
    counters: TrackingCounters,
    buffer: Box<[u8; MAX_PACKET]>,
}

impl<S: UdpNode> TrackerReceiver<S, SystemClock> {
    /// A receiver on the real clock.
    #[must_use]
    pub fn new(socket: S, config: TrackingConfig, table: Arc<TrackerTable>) -> Self {
        Self::build(socket, config, table, SystemClock::new())
    }
}

impl<S: UdpNode, C: Clock> TrackerReceiver<S, C> {
    /// A receiver on a clock of its own.
    #[must_use]
    pub fn with_clock(
        socket: S,
        config: TrackingConfig,
        table: Arc<TrackerTable>,
        clock: C,
    ) -> Self {
        Self::build(socket, config, table, clock)
    }

    fn build(socket: S, config: TrackingConfig, table: Arc<TrackerTable>, clock: C) -> Self {
        Self {
            socket,
            clock,
            config,
            table,
            open: false,
            rows: Vec::new(),
            system: None,
            counters: TrackingCounters::default(),
            buffer: Box::new([0; MAX_PACKET]),
        }
    }

    /// Opens the socket.
    ///
    /// # Errors
    ///
    /// [`UdpError::Bind`] when the port or the group cannot be taken. The
    /// caller's answer is to **say so and carry on**: a desk whose tracker
    /// receiver would not start is a desk that still runs the show.
    pub fn open(&mut self) -> Result<(), UdpError> {
        self.close();
        match self.config.listen {
            Listen::Multicast {
                group,
                port,
                interface,
            } => self.socket.listen_multicast(group, port, interface)?,
            Listen::Unicast(address) => self.socket.bind(address, false)?,
        }
        self.open = true;
        Ok(())
    }

    /// Closes the socket.
    pub fn close(&mut self) {
        if self.open {
            self.socket.close();
            self.open = false;
        }
    }

    /// Whether the socket is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// The address the socket is listening on, once it is open.
    #[must_use]
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.socket.local_addr()
    }

    /// What has been sent and what was done with it.
    #[must_use]
    pub const fn counters(&self) -> TrackingCounters {
        self.counters
    }

    /// This receiver's configuration.
    #[must_use]
    pub const fn config(&self) -> &TrackingConfig {
        &self.config
    }

    /// The receiver's own view of the time.
    #[must_use]
    pub fn now(&self) -> Duration {
        self.clock.now()
    }

    /// The clock - which is how a test moves it.
    #[must_use]
    pub const fn clock(&self) -> &C {
        &self.clock
    }

    /// Every tracker heard, in the order first heard.
    #[must_use]
    pub fn rows(&self) -> &[TrackerRow] {
        &self.rows
    }

    /// What the tracking system calls itself, once its info packet has arrived.
    #[must_use]
    pub fn system(&self) -> Option<&str> {
        self.system.as_deref()
    }

    /// Whether a tracker is being heard, by the configured timeout.
    #[must_use]
    pub fn health(&self, row: &TrackerRow) -> TrackerHealth {
        if self.clock.now().saturating_sub(row.last_seen) <= self.config.timeout {
            TrackerHealth::Live
        } else {
            TrackerHealth::Quiet
        }
    }

    /// One pass: read what is there and take it into the table. Answers how
    /// many positions were taken.
    ///
    /// Waits at most [`TrackingConfig::wait`] inside the socket read, so the
    /// loop that calls it notices a stop within that - no sleep and no spin.
    /// Does nothing on a socket that is not open.
    pub fn service(&mut self) -> usize {
        if !self.open {
            return 0;
        }
        let mut taken = 0;
        let wait = self.config.wait;
        for _ in 0..self.config.recv_budget {
            let outcome = {
                let Self { socket, buffer, .. } = self;
                socket.recv_from(buffer.as_mut_slice(), wait)
            };
            match outcome {
                Ok(None) => break,
                Ok(Some((len, _from))) => taken += self.accept(len),
                // One datagram too large for the buffer is rubbish, and the pass
                // goes on: the next one in the queue may be a position.
                Err(UdpError::Oversized) => {
                    self.counters.datagrams += 1;
                    self.counters.rejected += 1;
                }
                Err(_) => {
                    self.counters.read_errors += 1;
                    break;
                }
            }
        }
        taken
    }

    /// Takes one datagram into the table, or counts why it was not.
    fn accept(&mut self, len: usize) -> usize {
        self.counters.datagrams += 1;
        let Some(datagram) = self.buffer.get(..len) else {
            self.counters.rejected += 1;
            return 0;
        };
        let now = self.clock.now();
        // The decode borrows the buffer and this borrows `self` mutably to file
        // what it finds, so the finds are gathered first. A frame is a few
        // trackers, and `SmallBuffer`'s fixed array keeps this off the heap.
        let mut found: [Option<(u16, [f32; 3])>; MAX_PER_DATAGRAM] = [None; MAX_PER_DATAGRAM];
        let mut found_len = 0;
        let mut names: Vec<(u16, String)> = Vec::new();
        let mut system: Option<String> = None;
        let result = psn::decode(datagram, &mut |event| match event {
            Event::Position { tracker, position } => {
                if let Some(slot) = found.get_mut(found_len) {
                    *slot = Some((tracker, position));
                    found_len += 1;
                }
            }
            Event::Name { tracker, name } => names.push((tracker, name.to_owned())),
            Event::System(name) => system = Some(name.to_owned()),
        });
        if result.is_err() {
            self.counters.rejected += 1;
        }
        if let Some(system) = system {
            self.system = Some(system);
        }
        for (tracker, name) in names {
            self.name(tracker, name);
        }
        let mut taken = 0;
        for (tracker, raw) in found.iter().take(found_len).flatten() {
            if self.position(*tracker, *raw, now) {
                taken += 1;
            }
        }
        taken
    }

    fn position(&mut self, tracker: u16, raw: [f32; 3], now: Duration) -> bool {
        if tracker > MAX_TRACKER {
            self.counters.out_of_range += 1;
            return false;
        }
        let mapped = self.config.mapping.apply(raw);
        if !self.table.publish(tracker, mapped) {
            self.counters.refused += 1;
            return false;
        }
        self.counters.positions += 1;
        if let Some(row) = self.rows.iter_mut().find(|row| row.id == tracker) {
            row.position = mapped;
            row.last_seen = now;
            row.positions += 1;
        } else if self.rows.len() < self.config.max_rows {
            self.rows.push(TrackerRow {
                id: tracker,
                name: None,
                position: mapped,
                last_seen: now,
                positions: 1,
            });
        }
        true
    }

    /// Files a name. A tracker that has not spoken yet gets a row anyway: a
    /// named tracker is worth showing before it moves.
    fn name(&mut self, tracker: u16, name: String) {
        if tracker > MAX_TRACKER {
            self.counters.out_of_range += 1;
            return;
        }
        if let Some(row) = self.rows.iter_mut().find(|row| row.id == tracker) {
            if row.name.as_deref() != Some(name.as_str()) {
                row.name = Some(name);
            }
            return;
        }
        if self.rows.len() < self.config.max_rows {
            self.rows.push(TrackerRow {
                id: tracker,
                name: Some(name),
                position: Vec3::ZERO,
                // Never moved: a row with no position has never spoken, which
                // `positions == 0` says and an age cannot.
                last_seen: Duration::ZERO,
                positions: 0,
            });
        }
    }
}

/// The most positions one datagram is read for.
///
/// A PSN packet fits an Ethernet frame and a position chunk with its tracker
/// header is twenty bytes, so no datagram holds more than seventy-odd; a system
/// with more trackers sends them in several packets of one frame.
const MAX_PER_DATAGRAM: usize = 80;

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use std::time::Duration;

    use prism_domain::{TrackerHealth, TrackerMapping, Vec3};
    use prism_engine::{ManualClock, TrackerTable};

    use super::{Listen, TrackerReceiver, TrackingConfig};
    use crate::psn::{encode_data, encode_info};
    use crate::udp::{MockUdpNode, MockUdpNodeHandle, UdpError};

    const FROM: SocketAddr =
        SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)), 56565);
    const GROUP: Ipv4Addr = Ipv4Addr::new(236, 10, 10, 10);

    fn bench(
        listen: Listen,
    ) -> (
        TrackerReceiver<MockUdpNode, ManualClock>,
        MockUdpNodeHandle,
        Arc<TrackerTable>,
    ) {
        let socket = MockUdpNode::new();
        let handle = socket.handle();
        let table = Arc::new(TrackerTable::new());
        let config = TrackingConfig::new(
            listen,
            TrackerMapping::default(),
            Duration::from_millis(500),
        );
        let receiver =
            TrackerReceiver::with_clock(socket, config, Arc::clone(&table), ManualClock::new());
        (receiver, handle, table)
    }

    fn multicast() -> Listen {
        Listen::Multicast {
            group: GROUP,
            port: 56_565,
            interface: None,
        }
    }

    #[test]
    fn opening_joins_the_group_it_was_told_to() {
        let (mut receiver, socket, _) = bench(Listen::Multicast {
            group: GROUP,
            port: 56_565,
            interface: Some(Ipv4Addr::new(192, 168, 1, 20)),
        });
        receiver.open().unwrap();
        assert_eq!(
            socket.joins(),
            vec![(GROUP, 56_565, Some(Ipv4Addr::new(192, 168, 1, 20)))]
        );
        assert!(
            socket.binds().is_empty(),
            "a group is joined, not bound plainly"
        );
    }

    #[test]
    fn a_unicast_address_is_bound_directly_and_without_broadcast() {
        let address = SocketAddr::from(([127, 0, 0, 1], 0));
        let (mut receiver, socket, _) = bench(Listen::Unicast(address));
        receiver.open().unwrap();
        assert_eq!(socket.binds(), vec![(address, false)]);
        assert!(socket.joins().is_empty());
    }

    #[test]
    fn a_socket_that_will_not_open_leaves_the_receiver_closed() {
        let (mut receiver, socket, _) = bench(multicast());
        socket.fail_bind(1, UdpError::Bind);
        assert_eq!(receiver.open(), Err(UdpError::Bind));
        assert!(!receiver.is_open());
        assert_eq!(receiver.service(), 0, "and a closed one does nothing");
    }

    #[test]
    fn a_position_is_mapped_into_show_space_and_written_to_the_table() {
        let (mut receiver, socket, table) = bench(multicast());
        receiver.open().unwrap();
        // The default mapping swaps y and z: a tracker 3 m up (z) and 2 m
        // upstage (y) is show space (1, 3, 2).
        socket.deliver(FROM, &encode_data(0, 0, &[(4, [1.0, 2.0, 3.0])]));
        assert_eq!(receiver.service(), 1);
        assert_eq!(
            table.read(4),
            Some(Vec3 {
                x: 1.0,
                y: 3.0,
                z: 2.0
            })
        );
        let row = &receiver.rows()[0];
        assert_eq!((row.id, row.positions), (4, 1));
        assert_eq!(receiver.counters().positions, 1);
    }

    #[test]
    fn only_the_latest_position_is_kept() {
        let (mut receiver, socket, table) = bench(multicast());
        receiver.open().unwrap();
        for step in 0..100 {
            socket.deliver(FROM, &encode_data(0, 0, &[(1, [step as f32, 0.0, 0.0])]));
        }
        receiver.service();
        assert_eq!(table.read(1).unwrap().x, 99.0);
        assert_eq!(table.count(1), 100);
        assert_eq!(receiver.rows().len(), 1, "a hundred positions are one row");
    }

    #[test]
    fn a_name_arrives_in_an_info_packet_and_is_filed_on_the_tracker() {
        let (mut receiver, socket, _) = bench(multicast());
        receiver.open().unwrap();
        socket.deliver(FROM, &encode_info(0, 0, "OpenFollow", &[(7, "Anna")]));
        receiver.service();
        assert_eq!(receiver.system(), Some("OpenFollow"));
        let row = &receiver.rows()[0];
        assert_eq!(
            (row.id, row.name.as_deref(), row.positions),
            (7, Some("Anna"), 0)
        );
        socket.deliver(FROM, &encode_data(0, 0, &[(7, [0.0, 0.0, 0.0])]));
        receiver.service();
        assert_eq!(receiver.rows().len(), 1, "the position joins the named row");
        assert_eq!(receiver.rows()[0].name.as_deref(), Some("Anna"));
    }

    #[test]
    fn a_tracker_is_live_until_it_has_been_quiet_for_the_timeout_and_then_it_is_quiet() {
        let (mut receiver, socket, table) = bench(multicast());
        receiver.open().unwrap();
        socket.deliver(FROM, &encode_data(0, 0, &[(2, [1.0, 1.0, 1.0])]));
        receiver.service();
        let row = receiver.rows()[0].clone();
        assert_eq!(receiver.health(&row), TrackerHealth::Live);
        receiver.clock().advance(Duration::from_millis(500));
        assert_eq!(
            receiver.health(&row),
            TrackerHealth::Live,
            "at the timeout, not past it"
        );
        receiver.clock().advance(Duration::from_millis(1));
        assert_eq!(receiver.health(&row), TrackerHealth::Quiet);
        // **And holding is not a thing this does**: the table has kept the last
        // position by itself, which is what the head is aimed at.
        assert_eq!(
            table.read(2),
            Some(Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0
            })
        );
    }

    #[test]
    fn what_is_not_psn_is_counted_and_the_pass_goes_on() {
        let (mut receiver, socket, table) = bench(multicast());
        receiver.open().unwrap();
        socket.deliver(FROM, b"not a tracker");
        socket.deliver(FROM, &[0x55, 0x67, 0xFF, 0xFF]);
        socket.deliver(FROM, &encode_data(0, 0, &[(3, [1.0, 2.0, 3.0])]));
        assert_eq!(receiver.service(), 1);
        assert_eq!(receiver.counters().rejected, 2);
        assert_eq!(receiver.counters().datagrams, 3);
        assert!(
            table.read(3).is_some(),
            "the position behind the rubbish arrived"
        );
    }

    #[test]
    fn a_position_that_is_not_a_number_is_refused_and_the_last_good_one_stands() {
        let (mut receiver, socket, table) = bench(multicast());
        receiver.open().unwrap();
        socket.deliver(FROM, &encode_data(0, 0, &[(5, [1.0, 2.0, 3.0])]));
        socket.deliver(FROM, &encode_data(0, 0, &[(5, [f32::NAN, 2.0, 3.0])]));
        socket.deliver(FROM, &encode_data(0, 0, &[(5, [1.0e9, 2.0, 3.0])]));
        receiver.service();
        assert_eq!(receiver.counters().refused, 2);
        assert_eq!(
            table.read(5),
            Some(Vec3 {
                x: 1.0,
                y: 3.0,
                z: 2.0
            })
        );
    }

    #[test]
    fn a_tracker_past_the_table_is_counted_and_ignored() {
        let (mut receiver, socket, _) = bench(multicast());
        receiver.open().unwrap();
        socket.deliver(FROM, &encode_data(0, 0, &[(5_000, [1.0, 2.0, 3.0])]));
        socket.deliver(FROM, &encode_info(0, 0, "x", &[(6_000, "Far")]));
        receiver.service();
        assert_eq!(receiver.counters().out_of_range, 2);
        assert!(receiver.rows().is_empty());
    }

    #[test]
    fn the_rows_are_bounded() {
        let socket = MockUdpNode::new();
        let handle = socket.handle();
        let table = Arc::new(TrackerTable::new());
        let mut config = TrackingConfig::new(
            multicast(),
            TrackerMapping::default(),
            Duration::from_millis(500),
        );
        config.max_rows = 3;
        let mut receiver =
            TrackerReceiver::with_clock(socket, config, Arc::clone(&table), ManualClock::new());
        receiver.open().unwrap();
        for tracker in 0..10_u16 {
            handle.deliver(FROM, &encode_data(0, 0, &[(tracker, [1.0, 1.0, 1.0])]));
        }
        receiver.service();
        assert_eq!(receiver.rows().len(), 3);
        // The table still has every one of them: the bound is on what is *shown*.
        assert!(table.read(9).is_some());
    }

    #[test]
    fn a_read_error_is_counted_and_ends_the_pass() {
        let (mut receiver, socket, _) = bench(multicast());
        receiver.open().unwrap();
        socket.fail_recv(1, UdpError::Io);
        assert_eq!(receiver.service(), 0);
        assert_eq!(receiver.counters().read_errors, 1);
        assert!(
            receiver.is_open(),
            "one failed read does not close the socket"
        );
    }

    #[test]
    fn an_oversized_datagram_is_rubbish_and_the_next_one_is_still_read() {
        let (mut receiver, socket, table) = bench(multicast());
        receiver.open().unwrap();
        socket.deliver(FROM, &[0u8; 4_000]);
        socket.deliver(FROM, &encode_data(0, 0, &[(8, [1.0, 2.0, 3.0])]));
        assert_eq!(receiver.service(), 1);
        assert_eq!(receiver.counters().rejected, 1);
        assert!(table.read(8).is_some());
    }
}
