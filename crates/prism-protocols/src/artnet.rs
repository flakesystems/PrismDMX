//! Art-Net: DMX512 over UDP, and the output that keeps up with the engine.
//!
//! `ARCHITECTURE_SPEC.md` §7.2. Unlike the Open DMX cable of §7.1 this output
//! has no timing to generate — the network carries the frame and a node
//! regenerates DMX512 at the far end — so what is left is the packet, the
//! addressing, and knowing when *not* to send.
//!
//! ```text
//!   ArtDmx, 530 bytes                              ArtSync, 14 bytes
//!   ┌────────────┬────┬────┬───┬───┬──────┬───┬──────┬─────────┐
//!   │ "Art-Net\0"│OpCo│ProV│Seq│Phy│SubUni│Net│Length│ 512 ch. │
//!   │  8 bytes   │0x50│ 14 │1..│ 0 │ low  │hi │ 0x0200│         │
//!   └────────────┴────┴────┴───┴───┴──────┴───┴──────┴─────────┘
//!     0        7   8 9 10 11  12  13   14  15  16  17  18   529
//! ```
//!
//! # Three rules that are not obvious from the packet
//!
//! **Unicast is the default.** Art-Net began as a broadcast protocol and most
//! consoles still offer broadcast first. A broadcast Art-Net frame is 530 bytes
//! to *every* machine on the segment, 44 times a second, per universe — which
//! on a school network is a denial of service performed by the lighting desk.
//! [`Destination::Unicast`] is therefore what [`ArtNetConfig::default`] holds,
//! and broadcast is something an operator has to ask for by name.
//!
//! **A frame that has not changed is not sent, until it has to be.** Sending
//! every cadence is the flood again in a quieter form. So a universe goes out
//! when its data changes, and otherwise at least every
//! [`ArtNetConfig::refresh_interval`] — §7.2's 800 ms — so a node that timed out
//! or was switched on late converges on the current look rather than on
//! whatever it was holding.
//!
//! **The refresh goes out *before* the interval expires, not after.** A driver
//! that waited for the full 800 ms would send at 800 ms plus however long until
//! its next cadence, which is not "at least every 800 ms". The margin is
//! [`ArtNetConfig::refresh_margin`], one engine tick by default, and the test
//! that pins it measures the gap between datagrams on a simulated clock.
//!
//! # Sequence numbers
//!
//! Art-Net's `Sequence` field lets a node discard a datagram that overtook
//! another in the network. It counts 1 to 255 and wraps to **1**, never to 0,
//! because 0 means "this sender does not do sequence numbers" — a wrap to 0
//! would tell every node on the network to stop checking for exactly one frame.
//! It is counted per port address, so two universes on one output do not share
//! a counter.

use core::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::slice;
use std::time::Duration;

use prism_domain::{OutputHealth, OutputId, UniverseId};
use prism_engine::{Clock, SystemClock, TICK_PERIOD, UNIVERSE_CHANNELS};

use crate::output::{DmxOutput, OutputError};
use crate::udp::{UdpError, UdpSender};

/// The eight bytes every Art-Net packet starts with: `Art-Net` and a null.
///
/// Spelled out rather than derived from the string so the test that checks it
/// is checking the specification rather than checking itself.
pub const ART_NET_ID: [u8; 8] = [0x41, 0x72, 0x74, 0x2D, 0x4E, 0x65, 0x74, 0x00];

/// The UDP port Art-Net is defined on: `0x1936`.
pub const ART_NET_PORT: u16 = 6454;

/// `OpDmx` — a packet carrying channel data.
pub const OP_DMX: u16 = 0x5000;

/// `OpSync` — the packet that tells nodes to display what they are holding.
pub const OP_SYNC: u16 = 0x5200;

/// The protocol revision this output speaks, sent high byte first.
pub const PROTOCOL_VERSION: u16 = 14;

/// Bytes before the channel data in an ArtDmx packet.
pub const ART_DMX_HEADER: usize = 18;

/// Bytes in an ArtDmx packet carrying a whole universe.
pub const ART_DMX_BYTES: usize = ART_DMX_HEADER + UNIVERSE_CHANNELS;

/// Bytes in an ArtSync packet.
pub const ART_SYNC_BYTES: usize = 14;

/// The `Length` field of a full universe, high byte first: 512 as `0x0200`.
const LENGTH_HI: u8 = ((UNIVERSE_CHANNELS as u16) >> 8) as u8;
const LENGTH_LO: u8 = (UNIVERSE_CHANNELS as u16 & 0x00FF) as u8;

/// Where one universe goes on an Art-Net network.
///
/// Fifteen bits: seven of `Net`, four of `Sub-Net`, four of `Universe`. The
/// bottom byte travels as `SubUni` and the top seven bits as `Net`, which is
/// why the two halves are split across two fields that look unrelated in the
/// packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortAddress(u16);

impl PortAddress {
    /// The highest port address the fifteen bits can hold.
    pub const MAX: u16 = 0x7FFF;

    /// A port address from its raw fifteen-bit value.
    ///
    /// Returns `None` above [`MAX`](Self::MAX): a sixteenth bit would land in
    /// the `Net` field's top bit, which the specification reserves, and a
    /// silently truncated address is a universe going to the wrong node.
    #[must_use]
    pub const fn new(raw: u16) -> Option<Self> {
        if raw > Self::MAX {
            None
        } else {
            Some(Self(raw))
        }
    }

    /// A port address from the three numbers a node's front panel shows.
    ///
    /// Returns `None` if any part is out of range: `net` is 7 bits, `sub_net`
    /// and `universe` 4 bits each.
    #[must_use]
    pub const fn from_parts(net: u8, sub_net: u8, universe: u8) -> Option<Self> {
        if net > 0x7F || sub_net > 0x0F || universe > 0x0F {
            return None;
        }
        Some(Self(
            ((net as u16) << 8) | ((sub_net as u16) << 4) | universe as u16,
        ))
    }

    /// The default address for a PrismDMX universe: **one less than its
    /// number**.
    ///
    /// PrismDMX numbers universes from 1 (`UniverseId::MIN`) and Art-Net
    /// numbers port addresses from 0, so universe 1 is port address 0 — which
    /// is what a node's documentation assumes when it says "universe 0". Nodes
    /// disagree about this often enough that it is only the *default*:
    /// [`ArtNetOutput::with_ports`] takes the mapping explicitly.
    #[must_use]
    pub const fn for_universe(universe: UniverseId) -> Self {
        let raw = universe.get().saturating_sub(1);
        Self(if raw > Self::MAX as u32 {
            Self::MAX
        } else {
            raw as u16
        })
    }

    /// The raw fifteen-bit address.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// The `Net` field: the top seven bits.
    #[must_use]
    pub const fn net(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// The `SubUni` field: sub-net in the high nibble, universe in the low one.
    #[must_use]
    pub const fn sub_uni(self) -> u8 {
        (self.0 & 0x00FF) as u8
    }

    /// The `Sub-Net` nibble on its own, as a node's front panel shows it.
    #[must_use]
    pub const fn sub_net(self) -> u8 {
        (self.sub_uni() >> 4) & 0x0F
    }

    /// The `Universe` nibble on its own, as a node's front panel shows it.
    #[must_use]
    pub const fn universe(self) -> u8 {
        self.sub_uni() & 0x0F
    }
}

impl fmt::Display for PortAddress {
    /// `net:sub:universe`, which is how a node labels its ports.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.net(), self.sub_net(), self.universe())
    }
}

/// The eighteen bytes before the channel data.
///
/// Written out field by field rather than assembled from `to_le_bytes` calls,
/// because this is the one place where the specification is quoted rather than
/// implemented, and a reader should be able to check it against the document
/// without following two layers of conversion. The tests assert the literals
/// against `OP_DMX.to_le_bytes()` and friends, so the two cannot drift.
const fn art_dmx_header(port: PortAddress, sequence: u8, physical: u8) -> [u8; ART_DMX_HEADER] {
    [
        // ID[8]: "Art-Net\0".
        0x41,
        0x72,
        0x74,
        0x2D,
        0x4E,
        0x65,
        0x74,
        0x00,
        // OpCode: 0x5000, transmitted low byte first.
        0x00,
        0x50,
        // ProtVerHi, ProtVerLo: 14, transmitted high byte first.
        0x00,
        14,
        // Sequence, then Physical — the input port a controller would have read
        // this from, informational only and 0 for a desk.
        sequence,
        physical,
        // SubUni, then Net: the fifteen-bit port address, low byte first.
        port.sub_uni(),
        port.net(),
        // LengthHi, LengthLo: 512, high byte first.
        LENGTH_HI,
        LENGTH_LO,
    ]
}

/// Builds one ArtDmx packet into `packet`.
///
/// Takes the buffer rather than returning one: a driver thread sends 44 of
/// these a second per universe, and a 530-byte return value copied on every
/// frame is work for nothing.
pub fn write_art_dmx(
    packet: &mut [u8; ART_DMX_BYTES],
    port: PortAddress,
    sequence: u8,
    physical: u8,
    data: &[u8; UNIVERSE_CHANNELS],
) {
    let (header, channels) = packet.split_at_mut(ART_DMX_HEADER);
    header.copy_from_slice(&art_dmx_header(port, sequence, physical));
    channels.copy_from_slice(data);
}

/// One ArtSync packet.
///
/// Fourteen bytes with nothing in them but the header: it carries no address,
/// which is why the specification broadcasts it — one packet tells every node
/// on the network to display what it is holding. What this output does with
/// that is [`ArtNetConfig::sync`]'s documentation.
#[must_use]
pub const fn art_sync() -> [u8; ART_SYNC_BYTES] {
    [
        // ID[8]: "Art-Net\0".
        0x41, 0x72, 0x74, 0x2D, 0x4E, 0x65, 0x74, 0x00,
        // OpCode: 0x5200, low byte first.
        0x00, 0x52, // ProtVerHi, ProtVerLo: 14, high byte first.
        0x00, 14, // Aux1, Aux2: reserved, transmitted as zero.
        0x00, 0x00,
    ]
}

/// Where an output's packets go.
///
/// Two variants rather than a flag and an address, so that "broadcast" cannot
/// be a bit somebody flips while an unrelated list of unicast targets sits
/// beside it doing nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// One datagram per node. The default, and what a school network needs.
    Unicast(Vec<SocketAddr>),
    /// One datagram to a broadcast address, reaching every machine on the
    /// segment. **Opt-in only** — see this module's documentation.
    Broadcast(SocketAddr),
}

impl Destination {
    /// Unicast to the given nodes, on Art-Net's own port.
    #[must_use]
    pub fn nodes(addresses: impl IntoIterator<Item = IpAddr>) -> Self {
        Self::Unicast(
            addresses
                .into_iter()
                .map(|address| SocketAddr::new(address, ART_NET_PORT))
                .collect(),
        )
    }

    /// Broadcast to `address` on Art-Net's own port.
    ///
    /// Art-Net's recommended broadcast addresses are the directed ones —
    /// `2.255.255.255` or `10.255.255.255` — rather than the limited broadcast
    /// `255.255.255.255`, because a directed broadcast at least stays inside
    /// the subnet it names.
    #[must_use]
    pub const fn broadcast(address: Ipv4Addr) -> Self {
        Self::Broadcast(SocketAddr::new(IpAddr::V4(address), ART_NET_PORT))
    }

    /// Whether this destination needs the socket's broadcast permission.
    #[must_use]
    pub const fn is_broadcast(&self) -> bool {
        matches!(self, Self::Broadcast(_))
    }

    /// Every address a datagram goes to.
    // Not `const`: `Vec::as_slice` only became const in 1.87 and the workspace
    // MSRV is 1.85.
    #[must_use]
    pub fn addresses(&self) -> &[SocketAddr] {
        match self {
            Self::Unicast(addresses) => addresses.as_slice(),
            Self::Broadcast(address) => slice::from_ref(address),
        }
    }
}

impl Default for Destination {
    /// Unicast, to nobody. An output has to be told where its nodes are; the
    /// default cannot be "everyone", and this is the whole point of the
    /// variant being the one it is.
    fn default() -> Self {
        Self::Unicast(Vec::new())
    }
}

/// How an Art-Net output behaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtNetConfig {
    /// Where the packets go. Unicast by default.
    pub destination: Destination,
    /// The local address to send from. `0.0.0.0:0` lets the routing table
    /// choose the interface and the operating system choose the port, which is
    /// right until a machine has two networks — then an operator names the
    /// interface here, and [`UdpSender::local_addr`] shows what happened.
    pub bind: SocketAddr,
    /// Whether to follow each frame with an ArtSync.
    ///
    /// Off by default: a node that does not understand ArtSync ignores it, but
    /// a node that *does* stops displaying data until one arrives, so switching
    /// it on is a decision about the rig rather than a free improvement.
    ///
    /// When it is on, the sync goes to the same addresses the data went to. The
    /// specification broadcasts it; doing that here would mean enabling ArtSync
    /// silently turned a unicast configuration into a broadcasting one, which
    /// is the one thing §7.2 asks this output never to do by itself.
    pub sync: bool,
    /// The longest a universe may go without a datagram. §7.2: 800 ms.
    pub refresh_interval: Duration,
    /// How early the refresh may go out.
    ///
    /// The interval above is a *maximum* gap, and a driver only gets to decide
    /// at its own cadence. Refreshing when the interval has already expired
    /// puts the datagram at `interval + cadence`, which breaks the guarantee by
    /// however fast the thread happens to run. One engine tick of headroom
    /// covers any cadence at or below 44 Hz.
    pub refresh_margin: Duration,
    /// The `Physical` field: the input port a controller read this from.
    /// Informational, and 0 for a desk that is the source.
    pub physical: u8,
}

impl ArtNetConfig {
    /// A configuration sending to the given nodes, everything else as
    /// [`default`](Self::default) has it.
    #[must_use]
    pub fn unicast(nodes: impl IntoIterator<Item = IpAddr>) -> Self {
        Self {
            destination: Destination::nodes(nodes),
            ..Self::default()
        }
    }
}

impl Default for ArtNetConfig {
    fn default() -> Self {
        Self {
            destination: Destination::default(),
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            sync: false,
            refresh_interval: Duration::from_millis(800),
            refresh_margin: TICK_PERIOD,
            physical: 0,
        }
    }
}

/// One universe on the wire, and what has already gone out on it.
#[derive(Debug)]
struct UniversePort {
    universe: UniverseId,
    port: PortAddress,
    /// The last sequence number sent. 0 until the first datagram, which is also
    /// the value that means "not in use" — so it is never transmitted.
    sequence: u8,
    /// The channel data of the last datagram, for deciding whether anything has
    /// changed.
    last: [u8; UNIVERSE_CHANNELS],
    /// When that datagram went out, on the driver's own clock. `None` means
    /// nothing has been sent since the socket was opened, so the next frame
    /// goes out whatever it contains.
    sent_at: Option<Duration>,
    /// Whether this universe has been offered a frame in the current cycle.
    /// Only used to find the end of one — see [`ArtNetOutput::end_cycle`].
    served: bool,
}

impl UniversePort {
    fn new(universe: UniverseId, port: PortAddress) -> Self {
        Self {
            universe,
            port,
            sequence: 0,
            last: [0; UNIVERSE_CHANNELS],
            sent_at: None,
            served: false,
        }
    }

    /// Whether this frame has to go out: because it is different, because the
    /// refresh is due, or because nothing has gone out at all yet.
    fn is_due(&self, now: Duration, data: &[u8; UNIVERSE_CHANNELS], config: &ArtNetConfig) -> bool {
        let Some(sent_at) = self.sent_at else {
            return true;
        };
        if &self.last != data {
            return true;
        }
        now.saturating_sub(sent_at) + config.refresh_margin >= config.refresh_interval
    }

    /// 1 to 255 and back to 1. Never 0: that value tells a node this sender
    /// does not number its packets.
    fn advance_sequence(&mut self) {
        self.sequence = if self.sequence == u8::MAX {
            1
        } else {
            self.sequence + 1
        };
    }
}

/// An Art-Net output: several universes, one socket, one thread.
///
/// The type parameters are the socket and the clock. Both are parameters for
/// the same reason: [`MockUdp`](crate::MockUdp) makes the packets assertable
/// with no network, and `prism_engine::ManualClock` makes the 800 ms refresh
/// assertable without waiting 800 ms for it.
///
/// It carries **several** universes, unlike the one-per-cable Open DMX adapter
/// of §7.1. [`OutputRunner`](crate::OutputRunner) already sends every universe
/// an output declares, in the order [`universes`](DmxOutput::universes) gives
/// them, so nothing above this type changes.
pub struct ArtNetOutput<S: UdpSender, C: Clock = SystemClock> {
    id: OutputId,
    /// The universes, in send order. Kept beside `ports` because
    /// [`DmxOutput::universes`] hands out a slice of them.
    universes: Vec<UniverseId>,
    ports: Vec<UniversePort>,
    config: ArtNetConfig,
    socket: S,
    clock: C,
    open: bool,
    health: OutputHealth,
    /// The packet buffer, allocated once and rewritten per frame.
    packet: [u8; ART_DMX_BYTES],
    /// Whether any data has gone out since the last ArtSync. A sync that
    /// followed a cycle in which nothing was sent would be 44 packets a second
    /// onto an idle network, which is the flood this output exists to avoid.
    pending_sync: bool,
    datagrams: u64,
}

impl<S: UdpSender> ArtNetOutput<S, SystemClock> {
    /// An output carrying `universes`, each at its default port address —
    /// [`PortAddress::for_universe`].
    #[must_use]
    pub fn new(
        id: OutputId,
        universes: impl IntoIterator<Item = UniverseId>,
        socket: S,
        config: ArtNetConfig,
    ) -> Self {
        Self::build(
            id,
            universes
                .into_iter()
                .map(|universe| (universe, PortAddress::for_universe(universe)))
                .collect(),
            socket,
            config,
            SystemClock::new(),
        )
    }

    /// An output whose universes are mapped onto port addresses explicitly,
    /// for a node that numbers them its own way.
    #[must_use]
    pub fn with_ports(
        id: OutputId,
        ports: impl IntoIterator<Item = (UniverseId, PortAddress)>,
        socket: S,
        config: ArtNetConfig,
    ) -> Self {
        Self::build(
            id,
            ports.into_iter().collect(),
            socket,
            config,
            SystemClock::new(),
        )
    }
}

impl<S: UdpSender, C: Clock> ArtNetOutput<S, C> {
    /// An output on a clock of its own, which is how the refresh timer is
    /// tested against `prism_engine::ManualClock` rather than waited out.
    #[must_use]
    pub fn with_clock(
        id: OutputId,
        ports: impl IntoIterator<Item = (UniverseId, PortAddress)>,
        socket: S,
        config: ArtNetConfig,
        clock: C,
    ) -> Self {
        Self::build(id, ports.into_iter().collect(), socket, config, clock)
    }

    /// The one constructor, and deliberately not generic.
    ///
    /// S4's decision log: a generic constructor is compiled once per caller and
    /// llvm-cov counts every copy's untaken branches separately, so the three
    /// above collect their iterator and hand the work to this.
    fn build(
        id: OutputId,
        ports: Vec<(UniverseId, PortAddress)>,
        socket: S,
        config: ArtNetConfig,
        clock: C,
    ) -> Self {
        let mut universes = Vec::with_capacity(ports.len());
        let mut mapped = Vec::with_capacity(ports.len());
        for (universe, port) in ports {
            // A universe named twice would be sent twice per cycle, with two
            // sequence numbers a node would then see as reordering. The first
            // mapping wins and the duplicate is dropped.
            if !universes.contains(&universe) {
                universes.push(universe);
                mapped.push(UniversePort::new(universe, port));
            }
        }
        Self {
            id,
            universes,
            ports: mapped,
            config,
            socket,
            clock,
            open: false,
            health: OutputHealth::Disconnected,
            packet: [0; ART_DMX_BYTES],
            pending_sync: false,
            datagrams: 0,
        }
    }

    /// How this output is configured.
    #[must_use]
    pub const fn config(&self) -> &ArtNetConfig {
        &self.config
    }

    /// The port address a universe is sent to, if this output carries it.
    #[must_use]
    pub fn port_address(&self, universe: UniverseId) -> Option<PortAddress> {
        self.ports
            .iter()
            .find(|port| port.universe == universe)
            .map(|port| port.port)
    }

    /// Datagrams that have actually left the socket — data and sync together.
    ///
    /// Not the same number as `OutputStatus::frames_sent`, and the difference
    /// is the point of this output: a frame that has not changed is counted as
    /// sent by the runner and produces no datagram at all.
    #[must_use]
    pub const fn datagrams_sent(&self) -> u64 {
        self.datagrams
    }

    /// The address datagrams are leaving from, once the socket is open.
    #[must_use]
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.socket.local_addr()
    }

    /// The driver's own view of the time, for tests on a simulated clock.
    #[must_use]
    pub fn clock(&self) -> &C {
        &self.clock
    }

    /// Closes the socket and records the output as silent.
    fn drop_link(&mut self) {
        if self.open {
            self.socket.close();
            self.open = false;
        }
        self.pending_sync = false;
        self.health = OutputHealth::Disconnected;
    }

    /// Turns a socket error into an output error, leaving the driver in the
    /// state that error implies — the same split as `OpenDmxUsb::fault`.
    fn fault(&mut self, error: UdpError) -> OutputError {
        if error.is_link_lost() {
            self.drop_link();
            OutputError::Disconnected
        } else {
            self.health = OutputHealth::Degraded;
            OutputError::Faulted
        }
    }

    /// Sends one datagram to every address the destination names.
    ///
    /// An associated function so the caller can hold the packet buffer and the
    /// socket at the same time. One node refusing a datagram does not stop the
    /// others getting theirs — a dead node must not black out the rig — so the
    /// send is attempted everywhere and the worst error is reported, a lost
    /// link being the worst there is.
    fn send_datagram(
        socket: &mut S,
        destination: &Destination,
        datagram: &[u8],
        datagrams: &mut u64,
    ) -> Result<(), UdpError> {
        let mut worst: Option<UdpError> = None;
        for &target in destination.addresses() {
            let outcome = match socket.send_to(datagram, target) {
                Ok(sent) if sent == datagram.len() => {
                    *datagrams += 1;
                    continue;
                }
                Ok(sent) => UdpError::ShortSend {
                    sent,
                    expected: datagram.len(),
                },
                Err(error) => error,
            };
            worst = match worst {
                Some(previous) if previous.is_link_lost() => Some(previous),
                _ => Some(outcome),
            };
        }
        worst.map_or(Ok(()), Err)
    }

    /// Closes off a cycle: clears the per-universe marks and, if anything went
    /// out and ArtSync is on, sends the sync.
    ///
    /// A cycle is one pass of the runner over the universes this output
    /// carries. Its end is found in one of two ways, because the
    /// [`DmxOutput`] trait has no "the frame is complete" call and adding one
    /// would change a trait that three drivers already implement: normally it
    /// is the last universe in the list, and if that one never arrives — the
    /// engine does not publish it, so the runner skips it — it is the moment a
    /// universe comes round for the second time.
    fn end_cycle(&mut self) -> Result<(), OutputError> {
        for port in &mut self.ports {
            port.served = false;
        }
        if !self.pending_sync {
            return Ok(());
        }
        self.pending_sync = false;
        if !self.config.sync {
            return Ok(());
        }
        let sync = art_sync();
        let outcome = {
            let Self {
                socket,
                config,
                datagrams,
                ..
            } = self;
            Self::send_datagram(socket, &config.destination, &sync, datagrams)
        };
        match outcome {
            Ok(()) => Ok(()),
            Err(error) => Err(self.fault(error)),
        }
    }
}

impl<S: UdpSender, C: Clock + Send> DmxOutput for ArtNetOutput<S, C> {
    fn id(&self) -> OutputId {
        self.id
    }

    fn universes(&self) -> &[UniverseId] {
        &self.universes
    }

    /// Opens the socket.
    ///
    /// Also the reconnect path, so it forgets what it has sent: a node that has
    /// been unreachable must be given the current look on the first frame after
    /// it comes back, not told that nothing has changed since a datagram it
    /// never received.
    fn connect(&mut self) -> Result<(), OutputError> {
        if self.open {
            self.socket.close();
            self.open = false;
        }
        // An output with nowhere to send is not an output. Reported as a lost
        // link rather than accepted quietly, because the alternative is a green
        // light over a rig that never receives anything.
        if self.config.destination.addresses().is_empty() {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }
        if self
            .socket
            .bind(self.config.bind, self.config.destination.is_broadcast())
            .is_err()
        {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }
        self.open = true;
        self.pending_sync = false;
        for port in &mut self.ports {
            port.sent_at = None;
            port.served = false;
        }
        self.health = OutputHealth::Ok;
        Ok(())
    }

    /// Puts one universe on the network — or deliberately does not.
    ///
    /// `Ok(())` means "this universe is up to date at the far end", which is
    /// not the same as "a datagram just went out": an unchanged universe whose
    /// refresh is not due yet produces no packet. That is the whole design of
    /// this output, and [`datagrams_sent`](Self::datagrams_sent) is where the
    /// difference can be seen.
    fn send_frame(
        &mut self,
        universe: UniverseId,
        data: &[u8; UNIVERSE_CHANNELS],
    ) -> Result<(), OutputError> {
        let Some(index) = self.ports.iter().position(|port| port.universe == universe) else {
            let error = OutputError::UniverseNotCarried(universe);
            if self.health.is_sending() {
                self.health = error.health();
            }
            return Err(error);
        };
        if !self.open {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }

        // This universe coming round again is the start of a new cycle, so the
        // previous one is closed off first.
        if self
            .ports
            .iter()
            .any(|port| port.universe == universe && port.served)
        {
            self.end_cycle()?;
        }

        let now = self.clock.now();
        let mut ready = false;
        {
            let Self {
                ports,
                packet,
                config,
                ..
            } = self;
            // Filtered rather than indexed: the port was located above and the
            // list holds each universe exactly once, so this body runs exactly
            // once — and there is no second lookup with a branch no test can
            // reach.
            for port in ports.iter_mut().filter(|port| port.universe == universe) {
                if port.is_due(now, data, config) {
                    port.advance_sequence();
                    write_art_dmx(packet, port.port, port.sequence, config.physical, data);
                    port.last = *data;
                    port.sent_at = Some(now);
                    ready = true;
                }
                port.served = true;
            }
        }

        if ready {
            let outcome = {
                let Self {
                    socket,
                    config,
                    packet,
                    datagrams,
                    ..
                } = self;
                Self::send_datagram(socket, &config.destination, packet, datagrams)
            };
            if let Err(error) = outcome {
                // A datagram that did not go out was not sent, whatever this
                // driver has just written down. Forgetting it is what stops the
                // next frame being suppressed as unchanged and the failed look
                // sitting out the whole refresh interval.
                for port in self
                    .ports
                    .iter_mut()
                    .filter(|port| port.universe == universe)
                {
                    port.sent_at = None;
                }
                return Err(self.fault(error));
            }
            self.pending_sync = true;
        }
        self.health = OutputHealth::Ok;

        if index + 1 == self.ports.len() {
            self.end_cycle()?;
        }
        Ok(())
    }

    fn health(&self) -> OutputHealth {
        self.health
    }

    /// Closes the socket.
    ///
    /// Art-Net has no goodbye packet — a node holds its last look and, if it
    /// has one, times out into its own failover. sACN does have one, and S10
    /// has to send it here.
    fn shutdown(&mut self) {
        self.drop_link();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ART_DMX_BYTES, ART_DMX_HEADER, ART_NET_ID, ART_NET_PORT, ART_SYNC_BYTES, ArtNetConfig,
        ArtNetOutput, Destination, OP_DMX, OP_SYNC, PROTOCOL_VERSION, PortAddress, art_sync,
        write_art_dmx,
    };
    use crate::output::{DmxOutput, OutputError};
    use crate::udp::{MockUdp, MockUdpHandle, UdpError};
    use prism_domain::{OutputHealth, OutputId, UniverseId};
    use prism_engine::{Clock, ManualClock, TICK_PERIOD, UNIVERSE_CHANNELS};
    use proptest::prelude::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;

    fn universe(id: u32) -> UniverseId {
        UniverseId::new(id)
    }

    fn node(last_octet: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, last_octet))
    }

    fn target(last_octet: u8) -> SocketAddr {
        SocketAddr::new(node(last_octet), ART_NET_PORT)
    }

    fn config() -> ArtNetConfig {
        ArtNetConfig::unicast([node(1)])
    }

    /// An output on a clock that only moves when a test tells it to, with a
    /// socket that records instead of sending.
    fn output(
        universes: &[u32],
        config: ArtNetConfig,
    ) -> (ArtNetOutput<MockUdp, ManualClock>, MockUdpHandle) {
        let socket = MockUdp::new();
        let handle = socket.handle();
        let output = ArtNetOutput::with_clock(
            OutputId::new(1),
            universes
                .iter()
                .copied()
                .map(|id| (universe(id), PortAddress::for_universe(universe(id)))),
            socket,
            config,
            ManualClock::new(),
        );
        (output, handle)
    }

    /// A connected output over universe 1 alone.
    fn connected() -> (ArtNetOutput<MockUdp, ManualClock>, MockUdpHandle) {
        let (mut output, handle) = output(&[1], config());
        output.connect().unwrap();
        (output, handle)
    }

    fn frame(value: u8) -> [u8; UNIVERSE_CHANNELS] {
        [value; UNIVERSE_CHANNELS]
    }

    #[test]
    fn the_packet_is_the_specifications_packet_field_by_field() {
        // The exit criterion of S9, and the answer to S8's lesson: what is
        // asserted here is the datagram, not the fact that a send happened.
        let (mut output, handle) = output(&[7], config());
        output.connect().unwrap();
        let mut data = frame(0);
        data[0] = 0xAA;
        data[511] = 0x55;
        output.send_frame(universe(7), &data).unwrap();

        let (to, packet) = handle.last_datagram().unwrap();
        assert_eq!(to, target(1));
        assert_eq!(packet.len(), ART_DMX_BYTES);
        assert_eq!(packet.len(), 530);

        // ID[8]: "Art-Net" and a null terminator.
        assert_eq!(&packet[0..8], b"Art-Net\0");
        assert_eq!(&packet[0..8], &ART_NET_ID);
        // OpCode: OpDmx, low byte first.
        assert_eq!(&packet[8..10], &[0x00, 0x50]);
        assert_eq!(&packet[8..10], &OP_DMX.to_le_bytes());
        // ProtVerHi, ProtVerLo: 14, high byte first.
        assert_eq!(&packet[10..12], &[0x00, 14]);
        assert_eq!(&packet[10..12], &PROTOCOL_VERSION.to_be_bytes());
        // Sequence: the first datagram of a universe is 1, never 0.
        assert_eq!(packet[12], 1);
        // Physical: informational, and this desk is the source.
        assert_eq!(packet[13], 0);
        // SubUni and Net: universe 7 defaults to port address 6.
        assert_eq!(packet[14], 6);
        assert_eq!(packet[15], 0);
        // LengthHi, LengthLo: 512, high byte first.
        assert_eq!(&packet[16..18], &[0x02, 0x00]);
        assert_eq!(&packet[16..18], &512u16.to_be_bytes());
        // Data: the universe, unchanged.
        assert_eq!(packet[18], 0xAA);
        assert_eq!(packet[529], 0x55);
        assert_eq!(&packet[18..], &data[..]);
    }

    #[test]
    fn the_sync_packet_is_the_specifications_sync_packet() {
        let packet = art_sync();
        assert_eq!(packet.len(), ART_SYNC_BYTES);
        assert_eq!(packet.len(), 14);
        assert_eq!(&packet[0..8], b"Art-Net\0");
        assert_eq!(&packet[8..10], &OP_SYNC.to_le_bytes());
        assert_eq!(&packet[8..10], &[0x00, 0x52]);
        assert_eq!(&packet[10..12], &PROTOCOL_VERSION.to_be_bytes());
        // Aux1 and Aux2 are reserved and transmitted as zero.
        assert_eq!(&packet[12..14], &[0x00, 0x00]);
    }

    #[test]
    fn the_header_is_eighteen_bytes_and_the_data_starts_after_it() {
        let mut packet = [0xFFu8; ART_DMX_BYTES];
        let data = frame(0x11);
        write_art_dmx(&mut packet, PortAddress::new(0x1234).unwrap(), 42, 3, &data);
        assert_eq!(ART_DMX_HEADER, 18);
        assert_eq!(packet[12], 42);
        assert_eq!(packet[13], 3);
        // 0x1234: SubUni is the low byte, Net the top seven bits.
        assert_eq!(packet[14], 0x34);
        assert_eq!(packet[15], 0x12);
        assert_eq!(&packet[ART_DMX_HEADER..], &data[..]);
    }

    proptest! {
        #[test]
        fn every_channel_arrives_unchanged(channels in prop::collection::vec(any::<u8>(), UNIVERSE_CHANNELS)) {
            let mut data = [0u8; UNIVERSE_CHANNELS];
            data.copy_from_slice(&channels);
            let mut packet = [0u8; ART_DMX_BYTES];
            write_art_dmx(&mut packet, PortAddress::for_universe(universe(1)), 1, 0, &data);
            prop_assert_eq!(&packet[ART_DMX_HEADER..], &data[..]);
        }
    }

    #[test]
    fn sequence_numbers_count_from_one_and_wrap_to_one() {
        // 255 → 1, not 255 → 0: zero tells a node this sender does not number
        // its packets, so a wrap through it would switch the check off for
        // exactly one frame.
        let (mut output, handle) = connected();
        for step in 0..300u32 {
            // A different frame every time, so nothing is suppressed.
            let mut data = frame(0);
            data[0] = (step % 251) as u8;
            data[1] = (step / 251) as u8;
            output.send_frame(universe(1), &data).unwrap();
        }
        let sequences: Vec<u8> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| packet[12])
            .collect();
        assert_eq!(sequences.len(), 300);
        assert_eq!(sequences[0], 1);
        assert_eq!(sequences[253], 254);
        assert_eq!(sequences[254], 255);
        assert_eq!(sequences[255], 1);
        assert_eq!(sequences[256], 2);
        assert!(!sequences.contains(&0));
    }

    #[test]
    fn every_universe_counts_its_own_sequence() {
        // The specification counts per port address. A shared counter would
        // make a node see every second packet as out of order.
        let (mut output, handle) = output(&[1, 2], config());
        output.connect().unwrap();
        for step in 1..4u8 {
            output.send_frame(universe(1), &frame(step)).unwrap();
            output.send_frame(universe(2), &frame(step)).unwrap();
        }
        let sequences: Vec<(u8, u8)> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| (packet[14], packet[12]))
            .collect();
        assert_eq!(
            sequences,
            vec![(0, 1), (1, 1), (0, 2), (1, 2), (0, 3), (1, 3)]
        );
    }

    #[test]
    fn a_universe_is_one_less_than_its_number_by_default() {
        // PrismDMX numbers universes from 1; Art-Net numbers port addresses
        // from 0, which is what a node's front panel shows.
        assert_eq!(PortAddress::for_universe(universe(1)).get(), 0);
        assert_eq!(PortAddress::for_universe(universe(64)).get(), 63);
        // Universe 0 is not a valid `UniverseId`, and saturates rather than
        // wrapping to 65535.
        assert_eq!(PortAddress::for_universe(universe(0)).get(), 0);
        // And a number larger than fifteen bits clamps instead of aliasing
        // some other node's universe.
        assert_eq!(
            PortAddress::for_universe(universe(1_000_000)).get(),
            PortAddress::MAX
        );
    }

    #[test]
    fn a_port_address_splits_into_the_three_numbers_a_node_shows() {
        let address = PortAddress::from_parts(3, 5, 9).unwrap();
        assert_eq!(address.get(), 0x0359);
        assert_eq!(address.net(), 3);
        assert_eq!(address.sub_net(), 5);
        assert_eq!(address.universe(), 9);
        assert_eq!(address.sub_uni(), 0x59);
        assert_eq!(address.to_string(), "3:5:9");

        let top = PortAddress::new(PortAddress::MAX).unwrap();
        assert_eq!(top.net(), 0x7F);
        assert_eq!(top.sub_net(), 0x0F);
        assert_eq!(top.universe(), 0x0F);
    }

    #[test]
    fn a_port_address_out_of_range_is_refused_rather_than_truncated() {
        // A sixteenth bit lands in the Net field's reserved top bit, and a
        // silently truncated address is a universe arriving at the wrong node.
        assert_eq!(PortAddress::new(0x8000), None);
        assert_eq!(PortAddress::from_parts(0x80, 0, 0), None);
        assert_eq!(PortAddress::from_parts(0, 0x10, 0), None);
        assert_eq!(PortAddress::from_parts(0, 0, 0x10), None);
        assert!(PortAddress::new(PortAddress::MAX).is_some());
    }

    #[test]
    fn a_universe_can_be_mapped_onto_any_port_address() {
        // Nodes disagree about the 0-based/1-based question often enough that
        // the default has to be overridable.
        let socket = MockUdp::new();
        let handle = socket.handle();
        let mut output = ArtNetOutput::with_ports(
            OutputId::new(1),
            [(universe(1), PortAddress::from_parts(2, 1, 5).unwrap())],
            socket,
            config(),
        );
        assert_eq!(
            output.port_address(universe(1)),
            PortAddress::from_parts(2, 1, 5)
        );
        assert_eq!(output.port_address(universe(2)), None);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        let (_, packet) = handle.last_datagram().unwrap();
        assert_eq!(packet[14], 0x15);
        assert_eq!(packet[15], 0x02);
    }

    #[test]
    fn a_universe_named_twice_is_carried_once() {
        // Sent twice per cycle it would arrive with two sequence numbers, which
        // a node reads as reordering.
        let (mut output, handle) = output(&[1, 1, 2], config());
        assert_eq!(output.universes(), [universe(1), universe(2)]);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        assert_eq!(handle.datagram_count(), 1);
    }

    #[test]
    fn broadcast_is_never_the_default() {
        // ARCHITECTURE_SPEC.md §7.2. Not a formality: this is the assertion
        // that stops somebody turning a school's network into a broadcast
        // storm by changing a default.
        let default = ArtNetConfig::default();
        assert!(!default.destination.is_broadcast());
        assert_eq!(default.destination, Destination::Unicast(Vec::new()));
        assert!(!Destination::default().is_broadcast());
        assert!(!config().destination.is_broadcast());

        let (mut output, handle) = output(&[1], config());
        output.connect().unwrap();
        // The socket is never even asked for broadcast permission.
        assert_eq!(handle.binds(), vec![(ArtNetConfig::default().bind, false)]);
        output.send_frame(universe(1), &frame(1)).unwrap();
        assert_eq!(handle.last_datagram().unwrap().0, target(1));
    }

    #[test]
    fn broadcast_has_to_be_asked_for_by_name() {
        let mut config = ArtNetConfig {
            destination: Destination::broadcast(Ipv4Addr::new(2, 255, 255, 255)),
            ..ArtNetConfig::default()
        };
        config.bind = SocketAddr::new(node(1), 0);
        assert!(config.destination.is_broadcast());
        let (mut output, handle) = output(&[1], config.clone());
        output.connect().unwrap();
        // Only then is the operating system asked for the permission.
        assert_eq!(handle.binds(), vec![(config.bind, true)]);
        output.send_frame(universe(1), &frame(1)).unwrap();
        assert_eq!(
            handle.last_datagram().unwrap().0,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(2, 255, 255, 255)), ART_NET_PORT)
        );
    }

    #[test]
    fn a_destination_lists_every_address_a_datagram_goes_to() {
        let unicast = Destination::nodes([node(1), node(2)]);
        assert_eq!(unicast.addresses(), [target(1), target(2)]);
        let broadcast = Destination::broadcast(Ipv4Addr::BROADCAST);
        assert_eq!(broadcast.addresses().len(), 1);
        assert_eq!(broadcast.addresses()[0].port(), ART_NET_PORT);
    }

    #[test]
    fn every_node_gets_the_datagram() {
        let (mut output, handle) = output(&[1], ArtNetConfig::unicast([node(1), node(2), node(3)]));
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(9)).unwrap();
        let sent = handle.datagrams();
        assert_eq!(sent.len(), 3);
        assert_eq!(sent[0].0, target(1));
        assert_eq!(sent[1].0, target(2));
        assert_eq!(sent[2].0, target(3));
        // The same packet to each: one sequence number per universe per frame,
        // not one per node.
        assert_eq!(sent[0].1, sent[1].1);
        assert_eq!(sent[1].1, sent[2].1);
        assert_eq!(output.datagrams_sent(), 3);
    }

    #[test]
    fn an_output_with_nowhere_to_send_refuses_to_connect() {
        // A green light over a rig receiving nothing is worse than a red one.
        let (mut output, handle) = output(&[1], ArtNetConfig::default());
        assert_eq!(output.connect(), Err(OutputError::Disconnected));
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert!(handle.binds().is_empty());
    }

    #[test]
    fn an_unchanged_universe_is_not_sent_every_cadence() {
        // Sending 530 bytes 44 times a second per universe to a rig that is
        // not moving is the flood in a quieter form.
        let (mut output, handle) = connected();
        for _ in 0..10 {
            output.send_frame(universe(1), &frame(3)).unwrap();
            output.clock().advance(TICK_PERIOD);
        }
        assert_eq!(handle.datagram_count(), 1);
        // And a change goes out at once, on the very next call.
        output.send_frame(universe(1), &frame(4)).unwrap();
        assert_eq!(handle.datagram_count(), 2);
        assert_eq!(handle.last_datagram().unwrap().1[18], 4);
    }

    #[test]
    fn an_unchanged_universe_is_refreshed_before_the_interval_expires() {
        // ARCHITECTURE_SPEC.md §7.2's 800 ms is a *maximum* gap. Refreshing
        // when it has already expired would put the datagram at 800 ms plus
        // however long until the next cadence.
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(3)).unwrap();
        assert_eq!(handle.datagram_count(), 1);

        let config = output.config().clone();
        let due_at = config.refresh_interval - config.refresh_margin;
        output.clock().advance(due_at - Duration::from_nanos(1));
        output.send_frame(universe(1), &frame(3)).unwrap();
        assert_eq!(handle.datagram_count(), 1, "not due yet");

        output.clock().advance(Duration::from_nanos(1));
        output.send_frame(universe(1), &frame(3)).unwrap();
        assert_eq!(handle.datagram_count(), 2, "due, and one tick early");
        // The keep-alive is a full frame, not a shortened one: a node that has
        // just been switched on has to be able to build the whole look from it.
        assert_eq!(handle.last_datagram().unwrap().1.len(), ART_DMX_BYTES);
        assert_eq!(handle.last_datagram().unwrap().1[12], 2);
    }

    #[test]
    fn a_static_universe_still_reaches_the_node_every_eight_hundred_milliseconds() {
        // The exit criterion, measured the way a network engineer would: the
        // gap between consecutive datagrams, over ten seconds of a rig that is
        // not moving, on a clock that costs nothing to advance.
        let (mut output, handle) = connected();
        let cadence = TICK_PERIOD;
        let mut previous: Option<Duration> = None;
        let mut longest_gap = Duration::ZERO;
        for _ in 0..441 {
            let before = handle.datagram_count();
            output.send_frame(universe(1), &frame(3)).unwrap();
            if handle.datagram_count() > before {
                let now = output.clock().now();
                if let Some(previous) = previous {
                    longest_gap = longest_gap.max(now - previous);
                }
                previous = Some(now);
            }
            output.clock().advance(cadence);
        }
        assert!(
            longest_gap <= Duration::from_millis(800),
            "a static universe went {longest_gap:?} without a datagram"
        );
        // And it is a refresh rather than a stream: ten seconds at 44 Hz would
        // be 440 datagrams.
        let datagrams = handle.datagram_count();
        assert!(
            (12..=14).contains(&datagrams),
            "{datagrams} datagrams in ten seconds"
        );
    }

    #[test]
    fn a_frame_that_changes_by_one_channel_goes_out() {
        let (mut output, handle) = connected();
        let mut data = frame(0);
        output.send_frame(universe(1), &data).unwrap();
        data[511] = 1;
        output.send_frame(universe(1), &data).unwrap();
        assert_eq!(handle.datagram_count(), 2);
        assert_eq!(handle.last_datagram().unwrap().1[529], 1);
    }

    #[test]
    fn there_is_no_sync_unless_it_is_asked_for() {
        assert!(!ArtNetConfig::default().sync);
        let (mut output, handle) = output(&[1, 2], config());
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.send_frame(universe(2), &frame(2)).unwrap();
        assert_eq!(handle.datagram_count(), 2);
        for (_, packet) in handle.datagrams() {
            assert_eq!(packet.len(), ART_DMX_BYTES);
        }
    }

    #[test]
    fn a_sync_follows_the_last_universe_of_a_cycle() {
        let config = ArtNetConfig {
            sync: true,
            ..config()
        };
        let (mut output, handle) = output(&[1, 2], config);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        assert_eq!(handle.datagram_count(), 1, "no sync mid-cycle");
        output.send_frame(universe(2), &frame(2)).unwrap();

        let sent = handle.datagrams();
        assert_eq!(sent.len(), 3);
        assert_eq!(sent[0].1.len(), ART_DMX_BYTES);
        assert_eq!(sent[1].1.len(), ART_DMX_BYTES);
        assert_eq!(sent[2].1, art_sync().to_vec());
        // To the same addresses the data went to — enabling ArtSync must not
        // turn a unicast configuration into a broadcasting one.
        assert_eq!(sent[2].0, target(1));
    }

    #[test]
    fn a_sync_is_not_sent_after_a_cycle_in_which_nothing_moved() {
        // Otherwise a rig at rest costs 44 packets a second, which is the
        // flood this output exists to avoid, wearing a smaller hat.
        let config = ArtNetConfig {
            sync: true,
            ..config()
        };
        let (mut output, handle) = output(&[1, 2], config);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.send_frame(universe(2), &frame(2)).unwrap();
        assert_eq!(handle.datagram_count(), 3);
        handle.clear();

        for _ in 0..5 {
            output.send_frame(universe(1), &frame(1)).unwrap();
            output.send_frame(universe(2), &frame(2)).unwrap();
        }
        assert_eq!(handle.datagram_count(), 0);
    }

    #[test]
    fn a_cycle_that_never_reaches_its_last_universe_still_syncs() {
        // The runner skips a universe the engine does not publish, so the last
        // universe on the list may never be offered. The next cycle beginning
        // is then what closes the previous one.
        let config = ArtNetConfig {
            sync: true,
            ..config()
        };
        let (mut output, handle) = output(&[1, 2, 3], config);
        output.connect().unwrap();
        for step in 1..4u8 {
            output.send_frame(universe(1), &frame(step)).unwrap();
            output.send_frame(universe(2), &frame(step)).unwrap();
        }
        let lengths: Vec<usize> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| packet.len())
            .collect();
        assert_eq!(
            lengths,
            vec![
                ART_DMX_BYTES,
                ART_DMX_BYTES,
                ART_SYNC_BYTES,
                ART_DMX_BYTES,
                ART_DMX_BYTES,
                ART_SYNC_BYTES,
                ART_DMX_BYTES,
                ART_DMX_BYTES,
            ]
        );
    }

    #[test]
    fn a_universe_this_output_does_not_carry_is_refused() {
        let (mut output, handle) = connected();
        assert_eq!(
            output.send_frame(universe(2), &frame(1)),
            Err(OutputError::UniverseNotCarried(universe(2)))
        );
        assert_eq!(output.health(), OutputHealth::Degraded);
        assert_eq!(handle.datagram_count(), 0);
    }

    #[test]
    fn nothing_goes_out_before_the_socket_is_open() {
        let (mut output, handle) = output(&[1], config());
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert_eq!(
            output.send_frame(universe(1), &frame(1)),
            Err(OutputError::Disconnected)
        );
        assert_eq!(handle.datagram_count(), 0);
        assert_eq!(output.id(), OutputId::new(1));
    }

    #[test]
    fn a_socket_that_will_not_open_is_a_disconnected_output() {
        let (mut output, handle) = output(&[1], config());
        handle.fail_bind(1, UdpError::Bind);
        assert_eq!(output.connect(), Err(OutputError::Disconnected));
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert_eq!(output.connect(), Ok(()));
        assert_eq!(output.health(), OutputHealth::Ok);
    }

    #[test]
    fn a_network_that_has_gone_away_disconnects_the_output() {
        let (mut output, handle) = connected();
        handle.fail_send(1, UdpError::Unreachable);
        assert_eq!(
            output.send_frame(universe(1), &frame(1)),
            Err(OutputError::Disconnected)
        );
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert!(!handle.is_open(), "the socket was closed");
        assert_eq!(handle.closes(), 1);
    }

    #[test]
    fn a_refused_datagram_degrades_the_output_without_dropping_it() {
        let (mut output, handle) = connected();
        handle.fail_send(1, UdpError::Io);
        assert_eq!(
            output.send_frame(universe(1), &frame(1)),
            Err(OutputError::Faulted)
        );
        assert_eq!(output.health(), OutputHealth::Degraded);
        assert!(handle.is_open(), "the socket is still there");
        // The next frame is simply sent.
        assert_eq!(output.send_frame(universe(1), &frame(2)), Ok(()));
        assert_eq!(output.health(), OutputHealth::Ok);
    }

    #[test]
    fn a_short_send_is_a_truncated_packet_rather_than_a_partial_success() {
        let (mut output, handle) = connected();
        handle.fail_send(
            1,
            UdpError::ShortSend {
                sent: 100,
                expected: ART_DMX_BYTES,
            },
        );
        assert_eq!(
            output.send_frame(universe(1), &frame(1)),
            Err(OutputError::Faulted)
        );
        assert_eq!(output.health(), OutputHealth::Degraded);
        assert!(handle.is_open());
    }

    #[test]
    fn one_dead_node_does_not_stop_the_others() {
        // A rig does not go dark because one node was switched off.
        let (mut output, handle) = output(&[1], ArtNetConfig::unicast([node(1), node(2)]));
        output.connect().unwrap();
        handle.fail_send(1, UdpError::Io);
        assert_eq!(
            output.send_frame(universe(1), &frame(1)),
            Err(OutputError::Faulted)
        );
        let sent = handle.datagrams();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, target(2));
    }

    #[test]
    fn a_lost_network_outranks_a_refused_datagram() {
        // Two nodes, two different failures: the one that means "reconnect"
        // has to win, or the output would carry on sending into a network that
        // is not there.
        let (mut output, handle) = output(&[1], ArtNetConfig::unicast([node(1), node(2)]));
        output.connect().unwrap();
        handle.fail_send(1, UdpError::Unreachable);
        handle.fail_send(1, UdpError::Io);
        assert_eq!(
            output.send_frame(universe(1), &frame(1)),
            Err(OutputError::Disconnected)
        );
        assert_eq!(output.health(), OutputHealth::Disconnected);
    }

    #[test]
    fn a_refused_datagram_is_sent_again_on_the_next_frame() {
        // The suppression asks "has this changed since the last datagram?", so
        // a failed send has to be forgotten — otherwise a look that never left
        // the machine waits out the whole refresh interval before it is tried
        // again.
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(1)).unwrap();
        handle.fail_send(1, UdpError::Io);
        assert_eq!(
            output.send_frame(universe(1), &frame(2)),
            Err(OutputError::Faulted)
        );
        handle.clear();
        assert_eq!(output.send_frame(universe(1), &frame(2)), Ok(()));
        assert_eq!(handle.datagram_count(), 1);
        assert_eq!(handle.last_datagram().unwrap().1[18], 2);
    }

    #[test]
    fn a_sync_that_cannot_be_sent_reports_the_fault() {
        // The sync of a cycle whose last universe never arrives goes out at the
        // start of the next one, which is where this failure can be arranged.
        let config = ArtNetConfig {
            sync: true,
            ..config()
        };
        let (mut output, handle) = output(&[1, 2, 3], config);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.send_frame(universe(2), &frame(1)).unwrap();
        assert_eq!(handle.datagram_count(), 2);

        handle.fail_send(1, UdpError::Io);
        assert_eq!(
            output.send_frame(universe(1), &frame(2)),
            Err(OutputError::Faulted)
        );
        assert_eq!(handle.datagram_count(), 2, "the sync did not go out");
        assert_eq!(output.health(), OutputHealth::Degraded);
    }

    #[test]
    fn a_reconnected_output_sends_the_current_look_at_once() {
        // A node that has been away must not be told "nothing has changed
        // since a datagram you never received".
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(3)).unwrap();
        output.shutdown();
        handle.clear();

        output.connect().unwrap();
        output.send_frame(universe(1), &frame(3)).unwrap();
        assert_eq!(handle.datagram_count(), 1);
        assert_eq!(handle.last_datagram().unwrap().1[18], 3);
    }

    #[test]
    fn reconnecting_replaces_the_socket_rather_than_stacking_one_on_it() {
        let (mut output, handle) = connected();
        output.connect().unwrap();
        assert_eq!(handle.closes(), 1);
        assert_eq!(handle.binds().len(), 2);
        assert!(handle.is_open());
    }

    #[test]
    fn a_reconnect_does_not_resume_a_half_finished_cycle() {
        // The sync belongs to the frame that produced it. After a
        // reconnection, that frame is gone.
        let config = ArtNetConfig {
            sync: true,
            ..config()
        };
        let (mut output, handle) = output(&[1, 2], config);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        handle.clear();
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        // One datagram: the universe again, and no sync for the cycle that was
        // interrupted.
        assert_eq!(handle.datagram_count(), 1);
        assert_eq!(handle.last_datagram().unwrap().1.len(), ART_DMX_BYTES);
    }

    #[test]
    fn shutting_down_closes_the_socket() {
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.shutdown();
        assert!(!handle.is_open());
        assert_eq!(handle.closes(), 1);
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert_eq!(
            output.send_frame(universe(1), &frame(2)),
            Err(OutputError::Disconnected)
        );
        // Shutting down twice is not two closes: there is only one socket.
        output.shutdown();
        assert_eq!(handle.closes(), 1);
    }

    #[test]
    fn a_datagram_count_is_not_a_frame_count() {
        // The difference between them is what this output is for.
        let config = ArtNetConfig {
            sync: true,
            ..config()
        };
        let (mut output, _) = output(&[1], config);
        output.connect().unwrap();
        for _ in 0..10 {
            output.send_frame(universe(1), &frame(1)).unwrap();
        }
        // One ArtDmx and one ArtSync out of ten frames.
        assert_eq!(output.datagrams_sent(), 2);
    }

    #[test]
    fn an_output_knows_where_its_datagrams_come_from() {
        let (mut output, _) = output(&[1], config());
        assert_eq!(output.local_addr(), None);
        output.connect().unwrap();
        assert_eq!(output.local_addr(), Some(ArtNetConfig::default().bind));
    }

    #[test]
    fn an_artnet_output_can_be_used_through_a_trait_object() {
        // The daemon holds outputs of different kinds in one list.
        let (output, handle) = output(&[1], config());
        let mut outputs: Vec<Box<dyn DmxOutput>> = vec![Box::new(output)];
        for out in &mut outputs {
            out.connect().unwrap();
            out.send_frame(universe(1), &frame(1)).unwrap();
            assert_eq!(out.universes(), [universe(1)]);
            assert_eq!(out.health(), OutputHealth::Ok);
        }
        assert_eq!(handle.datagram_count(), 1);
    }

    #[test]
    fn the_configuration_is_readable_and_says_what_the_specification_says() {
        let config = ArtNetConfig::default();
        assert_eq!(config.refresh_interval, Duration::from_millis(800));
        assert_eq!(config.refresh_margin, TICK_PERIOD);
        assert_eq!(config.physical, 0);
        assert!(!config.sync);
        assert_eq!(config.bind.port(), 0);
        assert_eq!(ART_NET_PORT, 0x1936);
        let (output, _) = output(&[1], config.clone());
        assert_eq!(output.config(), &config);
    }
}
