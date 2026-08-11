//! sACN (E1.31): DMX512 over IP, multicast, and the protocol venues standardise
//! on.
//!
//! `ARCHITECTURE_SPEC.md` §7.2. Like Art-Net this output has no DMX timing to
//! generate — a gateway regenerates DMX512 at the far end — so what is left is
//! the packet, the addressing, and knowing when not to send. Unlike Art-Net,
//! E1.31 is a real standard with a real packet: three nested PDUs, an identity
//! for the sender, a priority per universe, and a way to say goodbye.
//!
//! ```text
//!   E1.31 data packet, 638 bytes
//!   ┌───────────────────────────┬──────────────────────────────┬─────────────┐
//!   │ Root Layer      0 … 37    │ Framing Layer     38 … 114   │ DMP  115…637│
//!   ├───────────────────────────┼──────────────────────────────┼─────────────┤
//!   │ Preamble 0x0010           │ Flags+Length 0x7258          │ Flags+Len   │
//!   │ Post-amble 0x0000         │ Vector 0x00000002            │   0x720B    │
//!   │ "ASC-E1.17\0\0\0"         │ Source Name (64, UTF-8, \0)  │ Vector 0x02 │
//!   │ Flags+Length 0x726E       │ Priority · Sync Addr · Seq   │ Type  0xA1  │
//!   │ Vector 0x00000004         │ Options · Universe           │ Addr/Incr   │
//!   │ CID (16)                  │                              │ Count 0x0201│
//!   │                           │                              │ 0x00 + 512  │
//!   └───────────────────────────┴──────────────────────────────┴─────────────┘
//! ```
//!
//! # Four rules that are not obvious from the packet
//!
//! **The CID is an identity, not a nonce.** A receiver tracks sources by CID:
//! two desks sharing one look like a single source that keeps changing its
//! mind, and a desk that invents a fresh CID at every start looks like a *new*
//! source at every start — the old one then lingers until it times out, and for
//! those two and a half seconds two equal-priority sources are fighting over the
//! same universes. So nothing in this module generates a CID. It is
//! configuration ([`SacnConfig::cid`]), it belongs in the show file, and an
//! output that has not been given one refuses to connect rather than
//! transmitting under a nil identity.
//!
//! **Priority is per universe, and it is the whole point of the field.** Two
//! sources on one universe are resolved by the higher priority winning outright
//! — not merged — which is how a backup desk or a house console takes over. So
//! it lives on [`SacnPort`], one per universe, rather than once per output.
//!
//! **A stream has to be ended, not merely stopped.** A receiver that stops
//! hearing from a source waits out a network-data-loss timeout of 2.5 s before
//! it releases the universe. [`SacnOutput::shutdown`] therefore sends
//! [`TERMINATION_PACKETS`] packets per universe with the `Stream_Terminated`
//! option set, which releases it at once. Each of them carries the **next**
//! sequence number: three identical ones would be discarded as duplicates by a
//! conforming receiver, and only the first would have any effect.
//!
//! **The keep-alive fires before its interval, not after.** E1.31 requires a
//! source to transmit at least once a second per universe. A driver only gets
//! to decide at its own cadence, so waiting for the full second puts the
//! datagram at one second *plus* a cadence — [`SacnConfig::refresh_margin`],
//! the same trap and the same remedy as Art-Net's 800 ms.
//!
//! # Sequence numbers, and how they differ from Art-Net's
//!
//! E1.31 counts 0 to 255 and wraps to **0**. Art-Net wraps to 1, because there
//! 0 means "this sender does not number its packets"; here it means nothing
//! special and skipping it would be the bug. Counted per universe, since that is
//! what a receiver tracks.

use core::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::slice;
use std::time::Duration;

use prism_domain::{OutputHealth, OutputId, UniverseId};
use prism_engine::{Clock, SystemClock, TICK_PERIOD, UNIVERSE_CHANNELS};

use crate::output::{DmxOutput, OutputError};
use crate::udp::{UdpError, UdpSender};

/// The UDP port E1.31 is defined on: `0x15C0`.
pub const E131_PORT: u16 = 5568;

/// The ACN packet identifier every root layer carries: `ASC-E1.17` and three
/// nulls.
pub const ACN_PACKET_IDENTIFIER: [u8; 12] = [
    0x41, 0x53, 0x43, 0x2D, 0x45, 0x31, 0x2E, 0x31, 0x37, 0x00, 0x00, 0x00,
];

/// `VECTOR_ROOT_E131_DATA` — the root layer of a packet carrying DMX data.
pub const VECTOR_ROOT_E131_DATA: u32 = 0x0000_0004;

/// `VECTOR_E131_DATA_PACKET` — the framing layer of the same.
pub const VECTOR_E131_DATA_PACKET: u32 = 0x0000_0002;

/// `VECTOR_DMP_SET_PROPERTY` — the only DMP operation this output performs.
pub const VECTOR_DMP_SET_PROPERTY: u8 = 0x02;

/// Bytes in an E1.31 data packet carrying a whole universe.
pub const E131_DATA_BYTES: usize = 638;

/// Bytes before the channel data — the three layers and the start code.
pub const E131_DATA_HEADER: usize = 126;

/// Bytes of the source name field: 64, UTF-8, null-terminated, so 63 of text.
pub const SOURCE_NAME_BYTES: usize = 64;

/// `Preview_Data`: this packet is for a visualiser and not for the rig.
pub const OPTION_PREVIEW_DATA: u8 = 0b1000_0000;

/// `Stream_Terminated`: the source is finished with this universe.
pub const OPTION_STREAM_TERMINATED: u8 = 0b0100_0000;

/// `Force_Synchronization`: act on data even without the synchronisation
/// packets a non-zero sync address promises. Never set here — this output sends
/// no sync packets and therefore never asks a receiver to wait for one.
pub const OPTION_FORCE_SYNCHRONIZATION: u8 = 0b0010_0000;

/// How many terminated packets a universe gets on the way out. E1.31 asks for
/// three, so that ending a show survives two lost datagrams.
pub const TERMINATION_PACKETS: usize = 3;

/// The sync address of a source that sends no synchronisation packets.
const NO_SYNC: u16 = 0;

/// The default source name, until a show file supplies one.
const DEFAULT_SOURCE_NAME: &str = "PrismDMX";

/// Bytes of the root layer, CID included.
const ROOT_LAYER_BYTES: usize = 38;

/// Bytes of the framing layer.
const FRAMING_LAYER_BYTES: usize = 77;

/// The root layer's PDU length: everything from its own flags field on, 622.
pub const ROOT_PDU_BYTES: usize = E131_DATA_BYTES - 16;

/// The framing layer's PDU length, 600.
pub const FRAMING_PDU_BYTES: usize = E131_DATA_BYTES - ROOT_LAYER_BYTES;

/// The DMP layer's PDU length, 523.
pub const DMP_PDU_BYTES: usize = E131_DATA_BYTES - ROOT_LAYER_BYTES - FRAMING_LAYER_BYTES;

/// The root layer up to the CID, written out rather than assembled.
///
/// The same reasoning as `artnet::art_dmx_header`: this is the one place where
/// the specification is quoted rather than implemented, and a reader should be
/// able to check it against the document without following a layer of
/// conversion. The tests assert every literal against the constant it must
/// equal, so the two cannot drift apart.
const ROOT_PREFIX: [u8; 22] = [
    // Preamble Size: 0x0010.
    0x00, 0x10, // Post-amble Size: 0x0000.
    0x00, 0x00, // ACN Packet Identifier: "ASC-E1.17\0\0\0".
    0x41, 0x53, 0x43, 0x2D, 0x45, 0x31, 0x2E, 0x31, 0x37, 0x00, 0x00, 0x00,
    // Flags (0x7) and Length (622), high byte first.
    0x72, 0x6E, // Vector: VECTOR_ROOT_E131_DATA.
    0x00, 0x00, 0x00, 0x04,
];

/// The framing layer up to the source name.
const FRAMING_PREFIX: [u8; 6] = [
    // Flags (0x7) and Length (600).
    0x72, 0x58, // Vector: VECTOR_E131_DATA_PACKET.
    0x00, 0x00, 0x00, 0x02,
];

/// The whole DMP layer bar the channel data: fixed for every packet a DMX
/// source sends.
const DMP_LAYER: [u8; 11] = [
    // Flags (0x7) and Length (523).
    0x72, 0x0B, // Vector: VECTOR_DMP_SET_PROPERTY.
    0x02, // Address Type and Data Type: 0xA1, one-octet properties at a
    // one-octet increment.
    0xA1, // First Property Address: 0x0000, the start code slot.
    0x00, 0x00, // Address Increment: 0x0001.
    0x00, 0x01, // Property Value Count: 0x0201 — 513, the start code and 512
    // channels.
    0x02, 0x01, // The DMX512 start code: null, i.e. channel data.
    0x00,
];

// The layers have to tile the packet exactly. A constant that drifts from the
// packet it describes would produce a plausible-looking datagram with its
// lengths one out, which is the kind of fault a receiver reports as nothing at
// all.
const _: () = assert!(ROOT_PREFIX.len() + 16 == ROOT_LAYER_BYTES);
const _: () = assert!(FRAMING_PREFIX.len() + SOURCE_NAME_BYTES + 7 == FRAMING_LAYER_BYTES);
const _: () = assert!(ROOT_LAYER_BYTES + FRAMING_LAYER_BYTES + DMP_LAYER.len() == E131_DATA_HEADER);
const _: () = assert!(E131_DATA_HEADER + UNIVERSE_CHANNELS == E131_DATA_BYTES);

/// A PDU's flags and length field: flags `0x7` in the top nibble, the length of
/// the PDU in the remaining twelve bits, high byte first.
#[must_use]
pub const fn flags_and_length(pdu_bytes: usize) -> [u8; 2] {
    (0x7000 | (pdu_bytes as u16 & 0x0FFF)).to_be_bytes()
}

/// The identity of one sACN source: a UUID, stable for the life of the desk.
///
/// Held as sixteen bytes rather than as a formatted string because that is what
/// goes on the wire, and parsed from the canonical text a show file or a
/// configuration holds. There is deliberately **no** constructor that invents
/// one — see this module's documentation for what a source that changes its CID
/// does to a receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Cid([u8; 16]);

impl Cid {
    /// The all-zero CID: not a valid identity, and what
    /// [`SacnConfig::default`] holds so that an output nobody has configured
    /// cannot transmit.
    pub const NIL: Self = Self([0; 16]);

    /// A CID from its sixteen bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// A CID from a `u128`, which is how a UUID is usually written in source.
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(value.to_be_bytes())
    }

    /// The sixteen bytes, in the order they travel.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }

    /// Whether this is the nil CID, i.e. no identity at all.
    #[must_use]
    pub const fn is_nil(self) -> bool {
        u128::from_be_bytes(self.0) == 0
    }

    /// Parses the canonical UUID text, with or without its hyphens and in
    /// either case.
    ///
    /// Returns `None` for anything that is not exactly 32 hexadecimal digits,
    /// because a CID silently read as something else is a desk that identifies
    /// itself as a different desk.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut digits = text.chars().filter(|character| *character != '-');
        let mut bytes = [0u8; 16];
        for byte in &mut bytes {
            let high = digits.next().and_then(|c| c.to_digit(16))?;
            let low = digits.next().and_then(|c| c.to_digit(16))?;
            *byte = ((high << 4) | low) as u8;
        }
        if digits.next().is_some() {
            return None;
        }
        Some(Self(bytes))
    }
}

impl fmt::Display for Cid {
    /// The canonical 8-4-4-4-12 form, lower case — what a show file stores and
    /// what a receiver's source list shows beside the name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.0.iter().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                write!(f, "-")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// How much a source's data on a universe is worth, against another source's.
///
/// E1.31 resolves two sources on one universe by priority, and the higher one
/// wins the whole universe rather than merging with the lower. 0 to 200, and
/// 100 is what every desk sends unless it has been told otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priority(u8);

impl Priority {
    /// The highest priority E1.31 permits.
    pub const MAX: u8 = 200;

    /// The default every source sends unless configured: 100.
    pub const DEFAULT: Self = Self(100);

    /// A priority, or `None` above [`MAX`](Self::MAX).
    ///
    /// Refused rather than clamped: a desk asked for 255 and given 200 would
    /// silently be equal to every other desk that made the same mistake, and
    /// takeover is exactly what the field exists to decide.
    #[must_use]
    pub const fn new(raw: u8) -> Option<Self> {
        if raw > Self::MAX {
            None
        } else {
            Some(Self(raw))
        }
    }

    /// The raw value, as it travels.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A universe number as E1.31 counts them: 1 to 63999.
///
/// A separate type from `UniverseId` because the ranges genuinely differ —
/// PrismDMX carries 1 to 64 (`ARCHITECTURE_SPEC.md` §6) and E1.31 allows a
/// thousand times as many — and because the multicast address is a property of
/// *this* number, not of the desk's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SacnUniverse(u16);

impl SacnUniverse {
    /// The lowest universe E1.31 permits.
    pub const MIN: u16 = 1;

    /// The highest universe E1.31 permits for DMX data. 64000 upwards are
    /// reserved, and 64214 is the discovery universe.
    pub const MAX: u16 = 63_999;

    /// A universe number, or `None` outside `1..=63999`.
    #[must_use]
    pub const fn new(raw: u16) -> Option<Self> {
        if raw < Self::MIN || raw > Self::MAX {
            None
        } else {
            Some(Self(raw))
        }
    }

    /// The default for a PrismDMX universe: **the same number**.
    ///
    /// Unlike Art-Net, which numbers port addresses from 0 and therefore needs
    /// the desk's universe *minus one*, E1.31 numbers universes from 1 exactly
    /// as PrismDMX does. The mapping is still data — [`SacnPort::at_universe`] —
    /// because a venue's universe 1 is not always a desk's universe 1.
    #[must_use]
    pub const fn for_universe(universe: UniverseId) -> Self {
        let raw = universe.get();
        if raw < Self::MIN as u32 {
            Self(Self::MIN)
        } else if raw > Self::MAX as u32 {
            Self(Self::MAX)
        } else {
            Self(raw as u16)
        }
    }

    /// The raw universe number, as it travels.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// The multicast group this universe is carried on.
    ///
    /// E1.31 §9.3.1: `239.255.0.0` with the universe number in the low two
    /// octets, high byte first. Universe 1 is `239.255.0.1` and universe 63999
    /// is `239.255.249.255`.
    #[must_use]
    pub const fn multicast_ip(self) -> Ipv4Addr {
        let [high, low] = self.0.to_be_bytes();
        Ipv4Addr::new(239, 255, high, low)
    }

    /// That group, on E1.31's own port.
    #[must_use]
    pub const fn multicast_target(self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self.multicast_ip()), E131_PORT)
    }
}

impl fmt::Display for SacnUniverse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One of the desk's universes, as this output puts it on the network.
///
/// Three pieces of data, and all three are data on purpose: which universe of
/// the show it is, which E1.31 universe it becomes, and what it is worth against
/// another source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SacnPort {
    /// The desk's universe.
    pub universe: UniverseId,
    /// The E1.31 universe it is sent as.
    pub sacn: SacnUniverse,
    /// What this universe is worth against another source of it.
    pub priority: Priority,
}

impl SacnPort {
    /// A universe sent as itself, at the default priority.
    #[must_use]
    pub const fn new(universe: UniverseId) -> Self {
        Self {
            universe,
            sacn: SacnUniverse::for_universe(universe),
            priority: Priority::DEFAULT,
        }
    }

    /// The same universe, sent as a different E1.31 one.
    #[must_use]
    pub const fn at_universe(mut self, sacn: SacnUniverse) -> Self {
        self.sacn = sacn;
        self
    }

    /// The same universe, at a stated priority.
    #[must_use]
    pub const fn at_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }
}

/// Where an sACN output's packets go.
///
/// Multicast is the default, and that is the opposite of Art-Net's decision for
/// a reason rather than an inconsistency: an sACN multicast group carries **one
/// universe**, so a switch that knows IGMP delivers it only to the ports that
/// asked for that universe. Art-Net's broadcast carries everything to everyone,
/// which is why §7.2 makes that one opt-in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SacnDestination {
    /// The group for each universe, computed by
    /// [`SacnUniverse::multicast_target`]. The default and the standard case.
    #[default]
    Multicast,
    /// Straight to named receivers instead. Some venues forbid multicast
    /// outright, and a point-to-point link to one gateway is a legitimate
    /// configuration.
    Unicast(Vec<SocketAddr>),
}

impl SacnDestination {
    /// Unicast to the given receivers, on E1.31's own port.
    #[must_use]
    pub fn receivers(addresses: impl IntoIterator<Item = IpAddr>) -> Self {
        Self::Unicast(
            addresses
                .into_iter()
                .map(|address| SocketAddr::new(address, E131_PORT))
                .collect(),
        )
    }

    /// Whether datagrams go to a multicast group.
    #[must_use]
    pub const fn is_multicast(&self) -> bool {
        matches!(self, Self::Multicast)
    }

    /// The addresses named explicitly, which is none under
    /// [`Multicast`](Self::Multicast): there the address is a property of the
    /// universe rather than of the configuration.
    #[must_use]
    pub fn unicast_addresses(&self) -> &[SocketAddr] {
        match self {
            Self::Multicast => &[],
            Self::Unicast(addresses) => addresses.as_slice(),
        }
    }
}

/// How an sACN output behaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SacnConfig {
    /// Where the packets go. Multicast by default.
    pub destination: SacnDestination,
    /// The local address to send from. `0.0.0.0:0` lets the routing table
    /// choose; naming an interface here is how a machine with two networks is
    /// told which one the lighting network is — and for multicast that choice
    /// *is* the outgoing interface.
    pub bind: SocketAddr,
    /// This desk's identity. Stable across restarts, and there is no default:
    /// see this module's documentation.
    pub cid: Cid,
    /// What a receiver shows beside the source. From the show file.
    pub source_name: String,
    /// How far a multicast datagram may travel. 1 keeps it on the local
    /// segment, which is right for a lighting network that is one switch and
    /// wrong for a venue whose gateways are behind a router.
    pub multicast_ttl: u32,
    /// Whether to mark every packet `Preview_Data`, i.e. for a visualiser
    /// rather than for the rig. Off, obviously; a desk whose output is ignored
    /// by every gateway on the network is a fault, not a feature.
    pub preview: bool,
    /// The longest a universe may go without a datagram. E1.31 requires at
    /// least one packet per second per universe.
    pub refresh_interval: Duration,
    /// How early the refresh may go out. One engine tick by default — the
    /// interval above is a *maximum* gap and a driver only decides at its own
    /// cadence, so refreshing when it has already expired misses it by however
    /// fast the thread happens to run.
    pub refresh_margin: Duration,
}

impl SacnConfig {
    /// A configuration for a source with this identity and name, everything
    /// else as [`default`](Self::default) has it.
    #[must_use]
    pub fn source(cid: Cid, name: impl Into<String>) -> Self {
        Self {
            cid,
            source_name: name.into(),
            ..Self::default()
        }
    }

    /// The `Options` byte every data packet from this output carries.
    #[must_use]
    pub const fn options(&self) -> u8 {
        if self.preview { OPTION_PREVIEW_DATA } else { 0 }
    }
}

impl Default for SacnConfig {
    fn default() -> Self {
        Self {
            destination: SacnDestination::default(),
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            cid: Cid::NIL,
            source_name: DEFAULT_SOURCE_NAME.to_owned(),
            multicast_ttl: 1,
            preview: false,
            refresh_interval: Duration::from_secs(1),
            refresh_margin: TICK_PERIOD,
        }
    }
}

/// The fields of one E1.31 data packet that are not the same in every packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct E131Header<'a> {
    /// The sending desk's identity.
    pub cid: Cid,
    /// What a receiver shows beside the source; truncated into 63 bytes.
    pub source_name: &'a str,
    /// What this universe is worth against another source of it.
    pub priority: Priority,
    /// The universe carrying the synchronisation packets that release this
    /// data, or 0 for a source that sends none.
    pub sync_address: u16,
    /// This packet's place in the universe's sequence.
    pub sequence: u8,
    /// `Preview_Data`, `Stream_Terminated`, `Force_Synchronization`.
    pub options: u8,
    /// The E1.31 universe this packet is for.
    pub universe: SacnUniverse,
}

/// The 64-byte source name field: UTF-8, null-terminated, null-padded.
///
/// Truncation is on a character boundary, so a name whose 63rd byte falls in the
/// middle of a multi-byte character loses that character rather than half of it.
/// A receiver decoding UTF-8 would otherwise show a replacement character, or
/// refuse the name.
#[must_use]
pub fn source_name_field(name: &str) -> [u8; SOURCE_NAME_BYTES] {
    let cut = name
        .char_indices()
        .map(|(index, character)| index + character.len_utf8())
        .take_while(|end| *end < SOURCE_NAME_BYTES)
        .last()
        .unwrap_or(0);
    let (text, _) = name.as_bytes().split_at(cut);
    let mut field = [0u8; SOURCE_NAME_BYTES];
    let (head, _) = field.split_at_mut(text.len());
    head.copy_from_slice(text);
    field
}

/// Builds one E1.31 data packet into `packet`.
///
/// Takes the buffer rather than returning one, for the same reason
/// `write_art_dmx` does: a driver thread sends up to 44 of these a second per
/// universe and a 638-byte return value copied every frame is work for nothing.
pub fn write_e131_data(
    packet: &mut [u8; E131_DATA_BYTES],
    header: &E131Header<'_>,
    data: &[u8; UNIVERSE_CHANNELS],
) {
    let (layers, channels) = packet.split_at_mut(E131_DATA_HEADER);
    channels.copy_from_slice(data);

    let (root, rest) = layers.split_at_mut(ROOT_LAYER_BYTES);
    let (root_prefix, cid) = root.split_at_mut(ROOT_PREFIX.len());
    root_prefix.copy_from_slice(&ROOT_PREFIX);
    cid.copy_from_slice(&header.cid.into_bytes());

    let (framing, dmp) = rest.split_at_mut(FRAMING_LAYER_BYTES);
    let (framing_prefix, named) = framing.split_at_mut(FRAMING_PREFIX.len());
    framing_prefix.copy_from_slice(&FRAMING_PREFIX);
    let (name, tail) = named.split_at_mut(SOURCE_NAME_BYTES);
    name.copy_from_slice(&source_name_field(header.source_name));
    let [sync_high, sync_low] = header.sync_address.to_be_bytes();
    let [universe_high, universe_low] = header.universe.get().to_be_bytes();
    tail.copy_from_slice(&[
        header.priority.get(),
        sync_high,
        sync_low,
        header.sequence,
        header.options,
        universe_high,
        universe_low,
    ]);

    dmp.copy_from_slice(&DMP_LAYER);
}

/// One universe on the wire, and what has already gone out on it.
#[derive(Debug)]
struct UniverseStream {
    universe: UniverseId,
    sacn: SacnUniverse,
    priority: Priority,
    /// The multicast group for this universe, computed once. Unused when the
    /// output unicasts.
    group: SocketAddr,
    /// The sequence number the **next** packet carries. E1.31 starts anywhere
    /// and wraps through 0, unlike Art-Net.
    sequence: u8,
    /// The channel data of the last datagram, for deciding whether anything has
    /// changed.
    last: [u8; UNIVERSE_CHANNELS],
    /// When that datagram went out, on the driver's own clock. `None` means
    /// nothing has been sent since the socket was opened, so the next frame goes
    /// out whatever it contains.
    sent_at: Option<Duration>,
    /// Whether a receiver is holding data from this source on this universe, and
    /// therefore has to be told when the source stops. Set by a datagram that
    /// actually went out, and **not** cleared by a failed one: the failure is
    /// what did not arrive, not what did.
    streaming: bool,
}

impl UniverseStream {
    fn new(port: SacnPort) -> Self {
        Self {
            universe: port.universe,
            sacn: port.sacn,
            priority: port.priority,
            group: port.sacn.multicast_target(),
            sequence: 0,
            last: [0; UNIVERSE_CHANNELS],
            sent_at: None,
            streaming: false,
        }
    }

    /// Whether this frame has to go out: because it is different, because the
    /// keep-alive is due, or because nothing has gone out at all yet.
    fn is_due(&self, now: Duration, data: &[u8; UNIVERSE_CHANNELS], config: &SacnConfig) -> bool {
        let Some(sent_at) = self.sent_at else {
            return true;
        };
        if &self.last != data {
            return true;
        }
        now.saturating_sub(sent_at) + config.refresh_margin >= config.refresh_interval
    }

    /// 0 to 255 and round again. Every packet counts, terminations included: a
    /// receiver discards a packet whose sequence has not moved on.
    const fn advance_sequence(&mut self) {
        self.sequence = self.sequence.wrapping_add(1);
    }
}

/// An sACN output: several universes, one socket, one thread.
///
/// The type parameters are the socket and the clock, for the same reasons
/// [`ArtNetOutput`](crate::ArtNetOutput) has them: [`MockUdp`](crate::MockUdp)
/// makes the packets assertable with no network, and `prism_engine::ManualClock`
/// makes the keep-alive assertable without waiting a second for it.
pub struct SacnOutput<S: UdpSender, C: Clock = SystemClock> {
    id: OutputId,
    /// The universes, in send order. Kept beside `streams` because
    /// [`DmxOutput::universes`] hands out a slice of them.
    universes: Vec<UniverseId>,
    streams: Vec<UniverseStream>,
    config: SacnConfig,
    socket: S,
    clock: C,
    open: bool,
    health: OutputHealth,
    /// The packet buffer, allocated once and rewritten per frame.
    packet: [u8; E131_DATA_BYTES],
    datagrams: u64,
}

impl<S: UdpSender> SacnOutput<S, SystemClock> {
    /// An output carrying `universes`, each sent as itself at the default
    /// priority.
    #[must_use]
    pub fn new(
        id: OutputId,
        universes: impl IntoIterator<Item = UniverseId>,
        socket: S,
        config: SacnConfig,
    ) -> Self {
        Self::build(
            id,
            universes.into_iter().map(SacnPort::new).collect(),
            socket,
            config,
            SystemClock::new(),
        )
    }

    /// An output whose universes carry their own E1.31 numbers and priorities.
    #[must_use]
    pub fn with_ports(
        id: OutputId,
        ports: impl IntoIterator<Item = SacnPort>,
        socket: S,
        config: SacnConfig,
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

impl<S: UdpSender, C: Clock> SacnOutput<S, C> {
    /// An output on a clock of its own, which is how the keep-alive is tested
    /// against `prism_engine::ManualClock` rather than waited out.
    #[must_use]
    pub fn with_clock(
        id: OutputId,
        ports: impl IntoIterator<Item = SacnPort>,
        socket: S,
        config: SacnConfig,
        clock: C,
    ) -> Self {
        Self::build(id, ports.into_iter().collect(), socket, config, clock)
    }

    /// The one constructor, and deliberately not generic — S4's decision log: a
    /// generic constructor is compiled once per caller and llvm-cov counts each
    /// copy's untaken branches separately.
    fn build(id: OutputId, ports: Vec<SacnPort>, socket: S, config: SacnConfig, clock: C) -> Self {
        let mut universes = Vec::with_capacity(ports.len());
        let mut streams = Vec::with_capacity(ports.len());
        for port in ports {
            // A universe named twice would be sent twice per cycle with two
            // sequence numbers, which a receiver reads as reordering. The first
            // mapping wins.
            if !universes.contains(&port.universe) {
                universes.push(port.universe);
                streams.push(UniverseStream::new(port));
            }
        }
        Self {
            id,
            universes,
            streams,
            config,
            socket,
            clock,
            open: false,
            health: OutputHealth::Disconnected,
            packet: [0; E131_DATA_BYTES],
            datagrams: 0,
        }
    }

    /// How this output is configured.
    #[must_use]
    pub const fn config(&self) -> &SacnConfig {
        &self.config
    }

    /// How a universe of the show is put on the network, if this output carries
    /// it.
    #[must_use]
    pub fn port(&self, universe: UniverseId) -> Option<SacnPort> {
        self.streams
            .iter()
            .find(|stream| stream.universe == universe)
            .map(|stream| SacnPort {
                universe: stream.universe,
                sacn: stream.sacn,
                priority: stream.priority,
            })
    }

    /// Datagrams that have actually left the socket, terminations included.
    ///
    /// Not the same number as `OutputStatus::frames_sent`: an unchanged universe
    /// whose keep-alive is not due produces no datagram at all.
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
        for stream in &mut self.streams {
            stream.streaming = false;
        }
        self.health = OutputHealth::Disconnected;
    }

    /// Turns a socket error into an output error, leaving the driver in the
    /// state that error implies — the same split as `ArtNetOutput::fault`.
    fn fault(&mut self, error: UdpError) -> OutputError {
        if error.is_link_lost() {
            self.drop_link();
            OutputError::Disconnected
        } else {
            self.health = OutputHealth::Degraded;
            OutputError::Faulted
        }
    }

    /// Sends one datagram to every address given.
    ///
    /// An associated function so the caller can hold the packet buffer and the
    /// socket at once. One receiver refusing a datagram does not stop the others
    /// getting theirs, and the worst error is reported — a lost link being the
    /// worst there is.
    fn send_datagram(
        socket: &mut S,
        addresses: &[SocketAddr],
        datagram: &[u8],
        datagrams: &mut u64,
    ) -> Result<(), UdpError> {
        let mut worst: Option<UdpError> = None;
        for &target in addresses {
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

    /// Where a universe's datagrams go: its own group, or the configured
    /// receivers.
    fn addresses<'a>(destination: &'a SacnDestination, group: &'a SocketAddr) -> &'a [SocketAddr] {
        match destination {
            SacnDestination::Multicast => slice::from_ref(group),
            SacnDestination::Unicast(addresses) => addresses.as_slice(),
        }
    }

    /// Tells every receiver that this source is finished with the universes it
    /// has been sending.
    ///
    /// Best effort by construction: it runs on the shutdown path, where
    /// [`DmxOutput::shutdown`] cannot report a failure and there would be
    /// nothing to do about one. A universe that never produced a datagram is
    /// left alone — there is no stream to end, and a terminated packet for a
    /// universe this source never sent would tell a receiver to release
    /// somebody else's.
    fn terminate(&mut self) {
        if !self.open {
            return;
        }
        let Self {
            streams,
            config,
            socket,
            packet,
            datagrams,
            ..
        } = self;
        for stream in streams.iter_mut().filter(|stream| stream.streaming) {
            let data = stream.last;
            let group = stream.group;
            for _ in 0..TERMINATION_PACKETS {
                let header = E131Header {
                    cid: config.cid,
                    source_name: &config.source_name,
                    priority: stream.priority,
                    sync_address: NO_SYNC,
                    sequence: stream.sequence,
                    options: config.options() | OPTION_STREAM_TERMINATED,
                    universe: stream.sacn,
                };
                write_e131_data(packet, &header, &data);
                stream.advance_sequence();
                let addresses = Self::addresses(&config.destination, &group);
                let _ = Self::send_datagram(socket, addresses, packet, datagrams);
            }
            stream.streaming = false;
        }
    }
}

impl<S: UdpSender, C: Clock + Send> DmxOutput for SacnOutput<S, C> {
    fn id(&self) -> OutputId {
        self.id
    }

    fn universes(&self) -> &[UniverseId] {
        &self.universes
    }

    /// Opens the socket.
    ///
    /// Also the reconnect path, so it forgets what it has sent: a receiver that
    /// has been unreachable must be given the current look on the first frame
    /// after it comes back, not told that nothing has changed since a datagram
    /// it never received.
    fn connect(&mut self) -> Result<(), OutputError> {
        if self.open {
            self.socket.close();
            self.open = false;
        }
        // A source with no identity is not a source. Refused rather than
        // transmitted under a nil CID, which every other desk that made the same
        // mistake would also be transmitting under.
        if self.config.cid.is_nil() {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }
        // Multicast needs no addresses — they follow from the universes — but a
        // unicast configuration nobody finished is an output with nowhere to
        // send, and a green light over a rig receiving nothing is worse than a
        // red one.
        if !self.config.destination.is_multicast()
            && self.config.destination.unicast_addresses().is_empty()
        {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }
        // sACN never broadcasts, so the socket is never asked for the
        // permission — not even when it unicasts to one gateway.
        if self.socket.bind(self.config.bind, false).is_err() {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }
        // A hop limit that could not be set is a configuration that is not in
        // force: the operator asked for a routed lighting network and would get
        // datagrams that stop at the first router, under a green light.
        if self.config.destination.is_multicast()
            && self
                .socket
                .set_multicast_ttl(self.config.multicast_ttl)
                .is_err()
        {
            self.socket.close();
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }
        self.open = true;
        for stream in &mut self.streams {
            stream.sent_at = None;
            stream.streaming = false;
        }
        self.health = OutputHealth::Ok;
        Ok(())
    }

    /// Puts one universe on the network — or deliberately does not.
    ///
    /// `Ok(())` means "this universe is up to date at the far end", which is not
    /// the same as "a datagram just went out": an unchanged universe whose
    /// keep-alive is not due yet produces no packet.
    fn send_frame(
        &mut self,
        universe: UniverseId,
        data: &[u8; UNIVERSE_CHANNELS],
    ) -> Result<(), OutputError> {
        if !self
            .streams
            .iter()
            .any(|stream| stream.universe == universe)
        {
            let error = OutputError::UniverseNotCarried(universe);
            if self.health.is_sending() {
                self.health = error.health();
            }
            return Err(error);
        }
        if !self.open {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }

        let now = self.clock.now();
        let mut group = None;
        {
            let Self {
                streams,
                packet,
                config,
                ..
            } = self;
            // Filtered rather than indexed: the stream was located above and the
            // list holds each universe exactly once, so this body runs exactly
            // once — and there is no second lookup with a branch no test can
            // reach.
            for stream in streams
                .iter_mut()
                .filter(|stream| stream.universe == universe)
            {
                if stream.is_due(now, data, config) {
                    let header = E131Header {
                        cid: config.cid,
                        source_name: &config.source_name,
                        priority: stream.priority,
                        sync_address: NO_SYNC,
                        sequence: stream.sequence,
                        options: config.options(),
                        universe: stream.sacn,
                    };
                    write_e131_data(packet, &header, data);
                    stream.advance_sequence();
                    group = Some(stream.group);
                }
            }
        }

        if let Some(group) = group {
            let outcome = {
                let Self {
                    socket,
                    config,
                    packet,
                    datagrams,
                    ..
                } = self;
                let addresses = Self::addresses(&config.destination, &group);
                Self::send_datagram(socket, addresses, packet, datagrams)
            };
            if let Err(error) = outcome {
                // Nothing is written down: `last` and `sent_at` describe the
                // last datagram that actually went out, so a refused one leaves
                // them alone. That is what stops the next frame being suppressed
                // as unchanged — and it is also why a look that failed and was
                // then changed back is *not* re-sent: the receiver already has
                // it. The sequence number is spent either way, which a receiver
                // reads as a packet lost in the network, because it was.
                return Err(self.fault(error));
            }
            for stream in self
                .streams
                .iter_mut()
                .filter(|stream| stream.universe == universe)
            {
                stream.last = *data;
                stream.sent_at = Some(now);
                stream.streaming = true;
            }
        }
        self.health = OutputHealth::Ok;
        Ok(())
    }

    fn health(&self) -> OutputHealth {
        self.health
    }

    /// Ends every stream this output is running, then closes the socket.
    ///
    /// The termination is what Art-Net has no equivalent of: without it a
    /// receiver holds the last look until its network-data-loss timeout expires,
    /// and the desk is gone for two and a half seconds before the rig knows.
    ///
    /// The terminated packets carry the **last look**, not a blackout. Whether
    /// the stage goes dark when the desk stops is a decision above this driver —
    /// `IMPLEMENTATION_PLAN.md` S17 makes it configurable — and a receiver
    /// ignores the data in a terminated packet in any case.
    fn shutdown(&mut self) {
        self.terminate();
        self.drop_link();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ACN_PACKET_IDENTIFIER, Cid, DMP_PDU_BYTES, E131_DATA_BYTES, E131_DATA_HEADER, E131_PORT,
        E131Header, FRAMING_PDU_BYTES, OPTION_FORCE_SYNCHRONIZATION, OPTION_PREVIEW_DATA,
        OPTION_STREAM_TERMINATED, Priority, ROOT_PDU_BYTES, SOURCE_NAME_BYTES, SacnConfig,
        SacnDestination, SacnOutput, SacnPort, SacnUniverse, TERMINATION_PACKETS,
        VECTOR_DMP_SET_PROPERTY, VECTOR_E131_DATA_PACKET, VECTOR_ROOT_E131_DATA, flags_and_length,
        source_name_field, write_e131_data,
    };
    use crate::output::{DmxOutput, OutputError};
    use crate::udp::{MockUdp, MockUdpHandle, UdpError};
    use prism_domain::{OutputHealth, OutputId, UniverseId};
    use prism_engine::{Clock, ManualClock, TICK_PERIOD, UNIVERSE_CHANNELS};
    use proptest::prelude::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;

    /// A CID in the canonical form a show file would hold. Written as text
    /// rather than as bytes because that is the form the value has everywhere
    /// outside this module.
    const CID_TEXT: &str = "6f2a1c34-9b5e-4d71-8a03-1e5c7b9d2f48";

    fn cid() -> Cid {
        Cid::parse(CID_TEXT).unwrap()
    }

    fn universe(id: u32) -> UniverseId {
        UniverseId::new(id)
    }

    fn sacn(raw: u16) -> SacnUniverse {
        SacnUniverse::new(raw).unwrap()
    }

    fn host(last_octet: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, last_octet))
    }

    fn target(last_octet: u8) -> SocketAddr {
        SocketAddr::new(host(last_octet), E131_PORT)
    }

    fn config() -> SacnConfig {
        SacnConfig::source(cid(), "PrismDMX Test")
    }

    /// An output on a clock that only moves when a test tells it to, with a
    /// socket that records instead of sending.
    fn output(
        ports: impl IntoIterator<Item = SacnPort>,
        config: SacnConfig,
    ) -> (SacnOutput<MockUdp, ManualClock>, MockUdpHandle) {
        let socket = MockUdp::new();
        let handle = socket.handle();
        let output =
            SacnOutput::with_clock(OutputId::new(1), ports, socket, config, ManualClock::new());
        (output, handle)
    }

    /// The same, for universes that need nothing said about them.
    fn plain(
        universes: &[u32],
        config: SacnConfig,
    ) -> (SacnOutput<MockUdp, ManualClock>, MockUdpHandle) {
        output(
            universes
                .iter()
                .copied()
                .map(|id| SacnPort::new(universe(id))),
            config,
        )
    }

    /// A connected output over universe 1 alone.
    fn connected() -> (SacnOutput<MockUdp, ManualClock>, MockUdpHandle) {
        let (mut output, handle) = plain(&[1], config());
        output.connect().unwrap();
        (output, handle)
    }

    fn frame(value: u8) -> [u8; UNIVERSE_CHANNELS] {
        [value; UNIVERSE_CHANNELS]
    }

    // ----- the packet ------------------------------------------------------

    #[test]
    fn the_packet_is_the_specifications_packet_field_by_field() {
        // The exit criterion of S10, over all three layers. Every literal is
        // asserted a second time against the constant it must equal, so a byte
        // and its meaning cannot drift apart.
        let ports = [SacnPort::new(universe(7))
            .at_universe(sacn(300))
            .at_priority(Priority::new(150).unwrap())];
        let (mut output, handle) = output(ports, config());
        output.connect().unwrap();
        let mut data = frame(0);
        data[0] = 0xAA;
        data[511] = 0x55;
        output.send_frame(universe(7), &data).unwrap();

        let (to, packet) = handle.last_datagram().unwrap();
        // Universe 300 travels to its own group, 239.255.1.44.
        assert_eq!(to, sacn(300).multicast_target());
        assert_eq!(to.ip(), IpAddr::V4(Ipv4Addr::new(239, 255, 1, 44)));
        assert_eq!(packet.len(), E131_DATA_BYTES);
        assert_eq!(packet.len(), 638);

        // ---- Root layer ----
        // Preamble Size: 16, the octets before the ACN identifier.
        assert_eq!(&packet[0..2], &[0x00, 0x10]);
        assert_eq!(&packet[0..2], &16u16.to_be_bytes());
        // Post-amble Size: none.
        assert_eq!(&packet[2..4], &[0x00, 0x00]);
        // ACN Packet Identifier: "ASC-E1.17" and three nulls.
        assert_eq!(&packet[4..16], b"ASC-E1.17\0\0\0");
        assert_eq!(&packet[4..16], &ACN_PACKET_IDENTIFIER);
        // Flags 0x7 and the root PDU length, 622.
        assert_eq!(&packet[16..18], &[0x72, 0x6E]);
        assert_eq!(&packet[16..18], &flags_and_length(ROOT_PDU_BYTES));
        assert_eq!(ROOT_PDU_BYTES, 622);
        // Vector: VECTOR_ROOT_E131_DATA.
        assert_eq!(&packet[18..22], &[0x00, 0x00, 0x00, 0x04]);
        assert_eq!(&packet[18..22], &VECTOR_ROOT_E131_DATA.to_be_bytes());
        // CID: this desk's identity, as the show file spells it.
        assert_eq!(&packet[22..38], &cid().into_bytes());
        assert_eq!(packet[22], 0x6F);
        assert_eq!(packet[37], 0x48);

        // ---- Framing layer ----
        // Flags 0x7 and the framing PDU length, 600.
        assert_eq!(&packet[38..40], &[0x72, 0x58]);
        assert_eq!(&packet[38..40], &flags_and_length(FRAMING_PDU_BYTES));
        assert_eq!(FRAMING_PDU_BYTES, 600);
        // Vector: VECTOR_E131_DATA_PACKET.
        assert_eq!(&packet[40..44], &[0x00, 0x00, 0x00, 0x02]);
        assert_eq!(&packet[40..44], &VECTOR_E131_DATA_PACKET.to_be_bytes());
        // Source Name: 64 bytes, UTF-8, null-terminated and null-padded.
        assert_eq!(&packet[44..57], b"PrismDMX Test");
        assert_eq!(&packet[57..108], &[0u8; 51]);
        assert_eq!(packet[44..108].len(), SOURCE_NAME_BYTES);
        // Priority: this universe's own.
        assert_eq!(packet[108], 150);
        // Synchronization Address: none — this source sends no sync packets.
        assert_eq!(&packet[109..111], &[0x00, 0x00]);
        // Sequence Number: the first packet of a universe, and 0 is a real
        // value here rather than Art-Net's "not numbered".
        assert_eq!(packet[111], 0);
        // Options: no preview, no termination, no forced synchronisation.
        assert_eq!(packet[112], 0x00);
        // Universe: 300, high byte first.
        assert_eq!(&packet[113..115], &[0x01, 0x2C]);
        assert_eq!(&packet[113..115], &300u16.to_be_bytes());

        // ---- DMP layer ----
        // Flags 0x7 and the DMP PDU length, 523.
        assert_eq!(&packet[115..117], &[0x72, 0x0B]);
        assert_eq!(&packet[115..117], &flags_and_length(DMP_PDU_BYTES));
        assert_eq!(DMP_PDU_BYTES, 523);
        // Vector: VECTOR_DMP_SET_PROPERTY.
        assert_eq!(packet[117], 0x02);
        assert_eq!(packet[117], VECTOR_DMP_SET_PROPERTY);
        // Address Type and Data Type: one-octet properties, one-octet
        // increment.
        assert_eq!(packet[118], 0xA1);
        // First Property Address: 0, which is the start code slot.
        assert_eq!(&packet[119..121], &[0x00, 0x00]);
        // Address Increment: 1.
        assert_eq!(&packet[121..123], &[0x00, 0x01]);
        // Property Value Count: 513 — the start code and 512 channels.
        assert_eq!(&packet[123..125], &[0x02, 0x01]);
        assert_eq!(&packet[123..125], &513u16.to_be_bytes());
        // The DMX512 start code: null.
        assert_eq!(packet[125], 0x00);
        assert_eq!(E131_DATA_HEADER, 126);

        // ---- Data ----
        assert_eq!(packet[126], 0xAA);
        assert_eq!(packet[637], 0x55);
        assert_eq!(&packet[E131_DATA_HEADER..], &data[..]);
    }

    proptest! {
        #[test]
        fn every_channel_arrives_unchanged(channels in prop::collection::vec(any::<u8>(), UNIVERSE_CHANNELS)) {
            let mut data = [0u8; UNIVERSE_CHANNELS];
            data.copy_from_slice(&channels);
            let mut packet = [0u8; E131_DATA_BYTES];
            write_e131_data(&mut packet, &header(0, 0), &data);
            prop_assert_eq!(&packet[E131_DATA_HEADER..], &data[..]);
        }
    }

    /// A header with everything at its ordinary value bar the two fields a test
    /// wants to vary.
    fn header(sequence: u8, options: u8) -> E131Header<'static> {
        E131Header {
            cid: cid(),
            source_name: "PrismDMX Test",
            priority: Priority::DEFAULT,
            sync_address: 0,
            sequence,
            options,
            universe: sacn(1),
        }
    }

    #[test]
    fn the_flags_are_seven_and_the_length_is_twelve_bits() {
        // The one arithmetic field in the packet: 0x7 in the top nibble and the
        // PDU's own length in the rest, high byte first.
        assert_eq!(flags_and_length(0), [0x70, 0x00]);
        assert_eq!(flags_and_length(1), [0x70, 0x01]);
        assert_eq!(flags_and_length(622), [0x72, 0x6E]);
        assert_eq!(flags_and_length(600), [0x72, 0x58]);
        assert_eq!(flags_and_length(523), [0x72, 0x0B]);
        assert_eq!(flags_and_length(0x0FFF), [0x7F, 0xFF]);
    }

    #[test]
    fn a_sync_address_is_carried_when_there_is_one() {
        // This output always sends 0 — a non-zero address tells a receiver to
        // wait for synchronisation packets it will never get — but the field is
        // built rather than hard-coded, so it is checked at both values.
        let mut packet = [0u8; E131_DATA_BYTES];
        let mut with_sync = header(0, 0);
        with_sync.sync_address = 7;
        write_e131_data(&mut packet, &with_sync, &frame(0));
        assert_eq!(&packet[109..111], &7u16.to_be_bytes());

        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(1)).unwrap();
        let (_, sent) = handle.last_datagram().unwrap();
        assert_eq!(&sent[109..111], &[0x00, 0x00], "no synchronisation");
        assert_eq!(sent[112] & OPTION_FORCE_SYNCHRONIZATION, 0);
    }

    #[test]
    fn the_options_byte_is_three_named_bits() {
        assert_eq!(OPTION_PREVIEW_DATA, 0b1000_0000);
        assert_eq!(OPTION_STREAM_TERMINATED, 0b0100_0000);
        assert_eq!(OPTION_FORCE_SYNCHRONIZATION, 0b0010_0000);

        // Preview is off by default: a desk whose every packet is ignored by
        // every gateway is a fault, not a feature.
        assert!(!SacnConfig::default().preview);
        assert_eq!(SacnConfig::default().options(), 0);
        let preview = SacnConfig {
            preview: true,
            ..config()
        };
        assert_eq!(preview.options(), OPTION_PREVIEW_DATA);
        let (mut output, handle) = plain(&[1], preview);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        assert_eq!(handle.last_datagram().unwrap().1[112], OPTION_PREVIEW_DATA);
    }

    // ----- the source name -------------------------------------------------

    #[test]
    fn the_source_name_is_sixty_four_bytes_and_null_terminated() {
        let field = source_name_field("PrismDMX");
        assert_eq!(field.len(), 64);
        assert_eq!(&field[0..8], b"PrismDMX");
        assert_eq!(&field[8..], &[0u8; 56]);

        // Exactly 63 characters fit, and the last byte is still the terminator.
        let full = "x".repeat(63);
        let field = source_name_field(&full);
        assert_eq!(&field[0..63], full.as_bytes());
        assert_eq!(field[63], 0);

        // The empty name is all zeros rather than anything clever.
        assert_eq!(source_name_field(""), [0u8; SOURCE_NAME_BYTES]);
    }

    #[test]
    fn a_long_source_name_is_truncated_on_a_character_boundary() {
        // "Bühnenlicht" repeated: the cut lands mid-character unless the
        // truncation counts characters rather than bytes, and a receiver
        // decoding UTF-8 would then show a replacement character or refuse the
        // name outright.
        let name = "ä".repeat(40);
        let field = source_name_field(&name);
        // 31 two-byte characters is 62 bytes; a 32nd would need 64 and leave no
        // room for the terminator.
        assert_eq!(&field[0..62], "ä".repeat(31).as_bytes());
        assert_eq!(&field[62..], &[0u8, 0u8]);
        let text = std::str::from_utf8(&field[0..62]).expect("a whole number of characters");
        assert_eq!(text.chars().count(), 31);

        // And an over-long ASCII name keeps its 63rd byte.
        let long = "y".repeat(200);
        let field = source_name_field(&long);
        assert_eq!(field[62], b'y');
        assert_eq!(field[63], 0);
    }

    #[test]
    fn the_source_name_comes_from_the_configuration() {
        // ARCHITECTURE_SPEC.md §7.2: from the show file. It is what a receiver
        // shows beside the source, so it is data, not a constant.
        let (mut output, handle) = plain(&[1], SacnConfig::source(cid(), "Aula Rig"));
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        let (_, packet) = handle.last_datagram().unwrap();
        assert_eq!(&packet[44..52], b"Aula Rig");
        assert_eq!(&packet[52..108], &[0u8; 56]);
    }

    // ----- the CID ---------------------------------------------------------

    #[test]
    fn the_cid_is_the_same_after_a_restart() {
        // The exit criterion. A receiver tracks sources by CID: a desk that
        // invents a new one at every start is a new source at every start, and
        // the old one goes on holding the universe until it times out.
        let first = {
            let (mut output, handle) = plain(&[1], config());
            output.connect().unwrap();
            output.send_frame(universe(1), &frame(1)).unwrap();
            output.shutdown();
            handle.last_datagram().unwrap().1
        };
        // A whole new output, a whole new socket — the same configuration.
        let second = {
            let (mut output, handle) = plain(&[1], config());
            output.connect().unwrap();
            output.send_frame(universe(1), &frame(1)).unwrap();
            handle.datagrams().first().cloned().unwrap().1
        };
        assert_eq!(&first[22..38], &second[22..38]);
        assert_eq!(&second[22..38], &cid().into_bytes());
        // And it is the value the configuration was given, not a hash of
        // anything this process happens to know.
        assert_eq!(cid().to_string(), CID_TEXT);
    }

    #[test]
    fn an_output_with_no_identity_refuses_to_connect() {
        // There is no constructor that invents a CID, so the default is the nil
        // one — and a nil CID is not an identity. Every desk that never
        // configured one would otherwise be the same source as every other.
        assert!(SacnConfig::default().cid.is_nil());
        assert!(Cid::NIL.is_nil());
        assert!(!cid().is_nil());

        let (mut output, handle) = plain(&[1], SacnConfig::default());
        assert_eq!(output.connect(), Err(OutputError::Disconnected));
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert!(handle.binds().is_empty(), "no socket was ever opened");
    }

    #[test]
    fn a_cid_is_read_and_written_in_the_form_a_show_file_holds() {
        let parsed = Cid::parse(CID_TEXT).unwrap();
        assert_eq!(parsed.to_string(), CID_TEXT);
        // Hyphens are decoration, and case is not information.
        assert_eq!(Cid::parse("6F2A1C349B5E4D718A031E5C7B9D2F48"), Some(parsed));
        assert_eq!(Cid::parse(&CID_TEXT.to_uppercase()), Some(parsed));
        // The bytes travel in the order they are written.
        assert_eq!(parsed.into_bytes()[0], 0x6F);
        assert_eq!(parsed.into_bytes()[15], 0x48);
        assert_eq!(Cid::from_bytes(parsed.into_bytes()), parsed);
        assert_eq!(
            Cid::from_u128(0x6f2a_1c34_9b5e_4d71_8a03_1e5c_7b9d_2f48),
            parsed
        );
        assert_eq!(Cid::from_u128(0), Cid::NIL);
        assert_eq!(Cid::default(), Cid::NIL);
        assert_eq!(Cid::NIL.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn anything_that_is_not_a_uuid_is_refused_rather_than_guessed_at() {
        // A CID read as something else is a desk identifying itself as a
        // different desk, which is the one thing the field exists to prevent.
        assert_eq!(Cid::parse(""), None);
        assert_eq!(Cid::parse("6f2a1c34"), None);
        assert_eq!(Cid::parse(&format!("{CID_TEXT}00")), None);
        assert_eq!(Cid::parse(&CID_TEXT.replace('f', "z")), None);
        assert_eq!(Cid::parse("not a uuid at all, not even close"), None);
    }

    // ----- addressing ------------------------------------------------------

    #[test]
    fn the_multicast_group_is_the_universe_in_the_bottom_two_octets() {
        // The exit criterion, at both ends of the range E1.31 defines.
        assert_eq!(sacn(1).multicast_ip(), Ipv4Addr::new(239, 255, 0, 1));
        assert_eq!(
            sacn(63_999).multicast_ip(),
            Ipv4Addr::new(239, 255, 249, 255)
        );
        // 63999 is 0xF9FF, so the octets are 249 and 255 — the case a function
        // written for a desk's 64 universes would never be asked about.
        assert_eq!(63_999u16.to_be_bytes(), [249, 255]);

        // And the carry from the low octet to the high one, where an
        // implementation that treated the universe as one byte would break.
        assert_eq!(sacn(255).multicast_ip(), Ipv4Addr::new(239, 255, 0, 255));
        assert_eq!(sacn(256).multicast_ip(), Ipv4Addr::new(239, 255, 1, 0));
        assert_eq!(sacn(257).multicast_ip(), Ipv4Addr::new(239, 255, 1, 1));
        assert_eq!(sacn(1000).multicast_ip(), Ipv4Addr::new(239, 255, 3, 232));

        // On E1.31's own port, which is not Art-Net's.
        assert_eq!(
            sacn(1).multicast_target(),
            "239.255.0.1:5568".parse().unwrap()
        );
        assert_eq!(E131_PORT, 0x15C0);
    }

    proptest! {
        #[test]
        fn every_universe_lands_in_its_own_group(raw in SacnUniverse::MIN..=SacnUniverse::MAX) {
            let group = SacnUniverse::new(raw).unwrap().multicast_ip();
            let octets = group.octets();
            prop_assert_eq!([octets[0], octets[1]], [239, 255]);
            prop_assert_eq!(u16::from_be_bytes([octets[2], octets[3]]), raw);
        }
    }

    #[test]
    fn a_universe_outside_the_e131_range_is_refused() {
        assert_eq!(SacnUniverse::new(0), None);
        assert_eq!(SacnUniverse::new(64_000), None);
        assert_eq!(SacnUniverse::new(u16::MAX), None);
        assert_eq!(SacnUniverse::new(1).map(SacnUniverse::get), Some(1));
        assert_eq!(
            SacnUniverse::new(63_999).map(SacnUniverse::get),
            Some(63_999)
        );
        assert_eq!(sacn(42).to_string(), "42");
    }

    #[test]
    fn a_prismdmx_universe_keeps_its_number() {
        // Unlike Art-Net, where the default is the universe *minus one*: E1.31
        // numbers universes from 1 exactly as PrismDMX does.
        assert_eq!(SacnUniverse::for_universe(universe(1)).get(), 1);
        assert_eq!(SacnUniverse::for_universe(universe(64)).get(), 64);
        // Universe 0 is not a valid `UniverseId`, and clamps rather than
        // wrapping into a reserved universe.
        assert_eq!(SacnUniverse::for_universe(universe(0)).get(), 1);
        assert_eq!(
            SacnUniverse::for_universe(universe(1_000_000)).get(),
            SacnUniverse::MAX
        );
        assert_eq!(SacnPort::new(universe(9)).sacn.get(), 9);
        assert_eq!(SacnPort::new(universe(9)).priority, Priority::DEFAULT);
    }

    #[test]
    fn a_universe_can_be_sent_as_a_different_one() {
        // A venue's universe 1 is not always a desk's universe 1, so the
        // mapping is data — the same reasoning as Art-Net's port address.
        let ports = [SacnPort::new(universe(1)).at_universe(sacn(63_999))];
        let (mut output, handle) = output(ports, config());
        assert_eq!(
            output.port(universe(1)).map(|port| port.sacn),
            Some(sacn(63_999))
        );
        assert_eq!(output.port(universe(2)), None);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        let (to, packet) = handle.last_datagram().unwrap();
        assert_eq!(&packet[113..115], &63_999u16.to_be_bytes());
        assert_eq!(to.ip(), IpAddr::V4(Ipv4Addr::new(239, 255, 249, 255)));
    }

    #[test]
    fn multicast_is_where_a_universe_goes_by_default() {
        assert!(SacnConfig::default().destination.is_multicast());
        assert!(SacnDestination::default().is_multicast());
        assert!(SacnDestination::Multicast.unicast_addresses().is_empty());

        let (mut output, handle) = plain(&[1, 2], config());
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.send_frame(universe(2), &frame(2)).unwrap();
        let sent = handle.datagrams();
        assert_eq!(sent.len(), 2);
        // One group per universe: a switch that knows IGMP delivers each of
        // them only to the ports that asked for that universe.
        assert_eq!(sent[0].0, sacn(1).multicast_target());
        assert_eq!(sent[1].0, sacn(2).multicast_target());
    }

    #[test]
    fn a_multicast_output_asks_for_a_hop_limit_rather_than_inheriting_one() {
        // The default is 1 on every platform, which keeps the show on the local
        // segment. A venue whose gateways are behind a router needs more, and a
        // desk that never set it at all would be at the mercy of the machine.
        assert_eq!(SacnConfig::default().multicast_ttl, 1);
        let (mut output, handle) = plain(&[1], config());
        output.connect().unwrap();
        assert_eq!(handle.multicast_ttls(), vec![1]);

        let far = SacnConfig {
            multicast_ttl: 8,
            ..config()
        };
        let (mut output, handle) = plain(&[1], far);
        output.connect().unwrap();
        assert_eq!(handle.multicast_ttls(), vec![8]);
    }

    #[test]
    fn a_hop_limit_that_cannot_be_set_is_a_disconnected_output() {
        // The operator asked for a routed network and would get datagrams that
        // stop at the first router, under a green light.
        let (mut output, handle) = plain(&[1], config());
        handle.fail_multicast_ttl(1, UdpError::Io);
        assert_eq!(output.connect(), Err(OutputError::Disconnected));
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert!(!handle.is_open(), "the socket was closed again");
        assert_eq!(output.connect(), Ok(()));
        assert_eq!(output.health(), OutputHealth::Ok);
    }

    #[test]
    fn an_output_can_be_told_to_unicast_instead() {
        // Some venues forbid multicast outright, and a point-to-point link to
        // one gateway is a legitimate configuration.
        let config = SacnConfig {
            destination: SacnDestination::receivers([host(1), host(2)]),
            ..config()
        };
        assert!(!config.destination.is_multicast());
        assert_eq!(
            config.destination.unicast_addresses(),
            [target(1), target(2)]
        );
        let (mut output, handle) = plain(&[1], config);
        output.connect().unwrap();
        // No multicast option is touched at all: this socket sends nothing to a
        // group.
        assert!(handle.multicast_ttls().is_empty());
        output.send_frame(universe(1), &frame(1)).unwrap();
        let sent = handle.datagrams();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].0, target(1));
        assert_eq!(sent[1].0, target(2));
        // The same packet to each: one sequence number per universe per frame,
        // not one per receiver.
        assert_eq!(sent[0].1, sent[1].1);
    }

    #[test]
    fn an_output_told_to_unicast_to_nobody_stays_red() {
        let config = SacnConfig {
            destination: SacnDestination::Unicast(Vec::new()),
            ..config()
        };
        let (mut output, handle) = plain(&[1], config);
        assert_eq!(output.connect(), Err(OutputError::Disconnected));
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert!(handle.binds().is_empty());
    }

    #[test]
    fn the_socket_is_never_asked_for_broadcast_permission() {
        // sACN has no broadcast mode, and asking for the permission anyway
        // would make an sACN output the one place a stray datagram could flood
        // a school's segment.
        let (mut output, handle) = plain(&[1], config());
        output.connect().unwrap();
        assert_eq!(handle.binds(), vec![(SacnConfig::default().bind, false)]);

        let unicast = SacnConfig {
            destination: SacnDestination::receivers([host(1)]),
            ..config()
        };
        let (mut output, handle) = plain(&[1], unicast);
        output.connect().unwrap();
        assert_eq!(handle.binds(), vec![(SacnConfig::default().bind, false)]);
    }

    // ----- priority --------------------------------------------------------

    #[test]
    fn priority_is_per_universe() {
        // Two sources on one universe are resolved by the higher priority
        // winning outright, which is how a backup desk takes over — so it
        // belongs to the universe, not to the output.
        let ports = [
            SacnPort::new(universe(1)).at_priority(Priority::new(200).unwrap()),
            SacnPort::new(universe(2)),
            SacnPort::new(universe(3)).at_priority(Priority::new(0).unwrap()),
        ];
        let (mut output, handle) = output(ports, config());
        output.connect().unwrap();
        for id in [1, 2, 3] {
            output.send_frame(universe(id), &frame(id as u8)).unwrap();
        }
        let priorities: Vec<(u8, u8)> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| (packet[114], packet[108]))
            .collect();
        assert_eq!(priorities, vec![(1, 200), (2, 100), (3, 0)]);
        assert_eq!(
            output.port(universe(1)).map(|port| port.priority),
            Priority::new(200)
        );
    }

    #[test]
    fn a_priority_above_two_hundred_is_refused_rather_than_clamped() {
        // A desk asked for 255 and given 200 would silently be equal to every
        // other desk that made the same mistake, and takeover is exactly what
        // the field decides.
        assert_eq!(Priority::new(201), None);
        assert_eq!(Priority::new(255), None);
        assert_eq!(Priority::new(200).map(Priority::get), Some(200));
        assert_eq!(Priority::new(0).map(Priority::get), Some(0));
        assert_eq!(Priority::DEFAULT.get(), 100);
        assert_eq!(Priority::default(), Priority::DEFAULT);
        assert_eq!(Priority::MAX, 200);
        assert_eq!(Priority::DEFAULT.to_string(), "100");
        assert!(Priority::new(150).unwrap() > Priority::DEFAULT);
    }

    // ----- sequence numbers ------------------------------------------------

    #[test]
    fn sequence_numbers_wrap_through_zero() {
        // E1.31 counts 0 to 255 and round again. Art-Net skips 0 because there
        // it means "not numbered"; here skipping it would be the bug.
        let (mut output, handle) = connected();
        for step in 0..300u32 {
            let mut data = frame(0);
            data[0] = (step % 251) as u8;
            data[1] = (step / 251) as u8;
            output.send_frame(universe(1), &data).unwrap();
        }
        let sequences: Vec<u8> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| packet[111])
            .collect();
        assert_eq!(sequences.len(), 300);
        assert_eq!(sequences[0], 0);
        assert_eq!(sequences[254], 254);
        assert_eq!(sequences[255], 255);
        assert_eq!(sequences[256], 0);
        assert_eq!(sequences[257], 1);
    }

    #[test]
    fn every_universe_counts_its_own_sequence() {
        // A receiver tracks the sequence per universe, so a shared counter
        // would make every second packet look out of order.
        let (mut output, handle) = plain(&[1, 2], config());
        output.connect().unwrap();
        for step in 1..4u8 {
            output.send_frame(universe(1), &frame(step)).unwrap();
            output.send_frame(universe(2), &frame(step)).unwrap();
        }
        let sequences: Vec<(u8, u8)> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| (packet[114], packet[111]))
            .collect();
        assert_eq!(
            sequences,
            vec![(1, 0), (2, 0), (1, 1), (2, 1), (1, 2), (2, 2)]
        );
    }

    // ----- when a datagram goes out ----------------------------------------

    #[test]
    fn an_unchanged_universe_is_not_sent_every_cadence() {
        let (mut output, handle) = connected();
        for _ in 0..10 {
            output.send_frame(universe(1), &frame(3)).unwrap();
            output.clock().advance(TICK_PERIOD);
        }
        assert_eq!(handle.datagram_count(), 1);
        // And a change goes out at once, on the very next call.
        output.send_frame(universe(1), &frame(4)).unwrap();
        assert_eq!(handle.datagram_count(), 2);
        assert_eq!(handle.last_datagram().unwrap().1[126], 4);
    }

    #[test]
    fn a_frame_that_changes_by_one_channel_goes_out() {
        let (mut output, handle) = connected();
        let mut data = frame(0);
        output.send_frame(universe(1), &data).unwrap();
        data[511] = 1;
        output.send_frame(universe(1), &data).unwrap();
        assert_eq!(handle.datagram_count(), 2);
        assert_eq!(handle.last_datagram().unwrap().1[637], 1);
    }

    #[test]
    fn an_unchanged_universe_is_refreshed_before_the_interval_expires() {
        // E1.31 requires a packet per universe per second. That is a *maximum*
        // gap, and a driver only decides at its own cadence — refreshing when
        // the second has already passed puts the datagram at one second plus
        // however long until the next wake-up.
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(3)).unwrap();
        assert_eq!(handle.datagram_count(), 1);

        let config = output.config().clone();
        assert_eq!(config.refresh_interval, Duration::from_secs(1));
        assert_eq!(config.refresh_margin, TICK_PERIOD);
        let due_at = config.refresh_interval - config.refresh_margin;
        output.clock().advance(due_at - Duration::from_nanos(1));
        output.send_frame(universe(1), &frame(3)).unwrap();
        assert_eq!(handle.datagram_count(), 1, "not due yet");

        output.clock().advance(Duration::from_nanos(1));
        output.send_frame(universe(1), &frame(3)).unwrap();
        assert_eq!(handle.datagram_count(), 2, "due, and one tick early");
        // A full packet, not a shortened one: a receiver switched on late has
        // to be able to build the whole look from it.
        assert_eq!(handle.last_datagram().unwrap().1.len(), E131_DATA_BYTES);
        assert_eq!(handle.last_datagram().unwrap().1[111], 1, "and it counts");
    }

    #[test]
    fn a_static_universe_still_reaches_a_receiver_every_second() {
        // Measured the way a network engineer would: the gap between
        // consecutive datagrams over ten seconds of a rig that is not moving,
        // on a clock that costs nothing to advance.
        let (mut output, handle) = connected();
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
            output.clock().advance(TICK_PERIOD);
        }
        assert!(
            longest_gap <= Duration::from_secs(1),
            "a static universe went {longest_gap:?} without a datagram"
        );
        // And it is a keep-alive rather than a stream: ten seconds at 44 Hz
        // would be 440 datagrams.
        let datagrams = handle.datagram_count();
        assert!(
            (10..=12).contains(&datagrams),
            "{datagrams} datagrams in ten seconds"
        );
    }

    // ----- termination -----------------------------------------------------

    #[test]
    fn shutting_down_ends_every_stream_it_started() {
        // The exit criterion. Without this a receiver holds the last look until
        // its network-data-loss timeout expires — two and a half seconds in
        // which the desk is gone and the rig does not know.
        let (mut output, handle) = plain(&[1, 2], config());
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.send_frame(universe(2), &frame(2)).unwrap();
        handle.clear();

        output.shutdown();
        let sent = handle.datagrams();
        assert_eq!(sent.len(), TERMINATION_PACKETS * 2);
        assert_eq!(TERMINATION_PACKETS, 3);
        for (index, (to, packet)) in sent.iter().enumerate() {
            let universe = if index < TERMINATION_PACKETS { 1u8 } else { 2 };
            assert_eq!(packet.len(), E131_DATA_BYTES);
            assert_eq!(packet[114], universe, "universe {universe}");
            assert_eq!(*to, sacn(u16::from(universe)).multicast_target());
            // Options bit 6: Stream_Terminated, and nothing else.
            assert_eq!(
                packet[112] & OPTION_STREAM_TERMINATED,
                OPTION_STREAM_TERMINATED
            );
            assert_eq!(packet[112], 0b0100_0000);
            // Still this source, still this universe's priority.
            assert_eq!(&packet[22..38], &cid().into_bytes());
            assert_eq!(packet[108], Priority::DEFAULT.get());
            // The last look, unchanged: whether the stage goes dark on shutdown
            // is a decision above this driver.
            assert_eq!(packet[126], universe);
        }
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert!(!handle.is_open());
    }

    #[test]
    fn each_terminated_packet_carries_the_next_sequence_number() {
        // Three identical sequence numbers would be discarded as duplicates by
        // a conforming receiver, so only the first would end anything.
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.send_frame(universe(1), &frame(2)).unwrap();
        handle.clear();
        output.shutdown();

        let sequences: Vec<u8> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| packet[111])
            .collect();
        // The two data packets were 0 and 1, so the terminations carry on.
        assert_eq!(sequences, vec![2, 3, 4]);
    }

    #[test]
    fn a_universe_that_never_sent_anything_is_not_terminated() {
        // A terminated packet for a stream this source never started would tell
        // a receiver to release somebody else's.
        let (mut output, handle) = plain(&[1, 2], config());
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        handle.clear();
        output.shutdown();
        let universes: Vec<u8> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| packet[114])
            .collect();
        assert_eq!(universes, vec![1, 1, 1]);
    }

    #[test]
    fn an_output_that_never_connected_terminates_nothing() {
        let (mut output, handle) = plain(&[1], config());
        output.shutdown();
        assert_eq!(handle.datagram_count(), 0);
        assert_eq!(handle.closes(), 0);
        assert_eq!(output.health(), OutputHealth::Disconnected);
    }

    #[test]
    fn shutting_down_twice_ends_the_stream_once() {
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.shutdown();
        assert_eq!(handle.datagram_count(), 1 + TERMINATION_PACKETS);
        assert_eq!(handle.closes(), 1);
        output.shutdown();
        assert_eq!(handle.datagram_count(), 1 + TERMINATION_PACKETS);
        assert_eq!(handle.closes(), 1);
    }

    #[test]
    fn a_termination_that_cannot_be_sent_does_not_stop_the_shutdown() {
        // `DmxOutput::shutdown` is infallible on purpose: it runs where there
        // is nothing left to do about a failure. What must not happen is the
        // rest of the shutdown being skipped.
        let (mut output, handle) = plain(&[1, 2], config());
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        output.send_frame(universe(2), &frame(1)).unwrap();
        handle.clear();
        handle.fail_send(2, UdpError::Io);

        output.shutdown();
        // Two datagrams were refused; the other four went out, universe 2
        // included.
        let universes: Vec<u8> = handle
            .datagrams()
            .iter()
            .map(|(_, packet)| packet[114])
            .collect();
        assert_eq!(universes, vec![1, 2, 2, 2]);
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert!(!handle.is_open());
    }

    #[test]
    fn a_lost_network_is_not_terminated_over() {
        // The datagrams could not go out — that is what "the network is gone"
        // means — and after a reconnection it is a new stream in any case.
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(1)).unwrap();
        handle.fail_send(1, UdpError::Unreachable);
        assert_eq!(
            output.send_frame(universe(1), &frame(2)),
            Err(OutputError::Disconnected)
        );
        handle.clear();
        output.shutdown();
        assert_eq!(handle.datagram_count(), 0);
    }

    #[test]
    fn a_frame_that_failed_still_leaves_a_stream_to_terminate() {
        // The failure is what did not arrive, not what did: an earlier datagram
        // is at the receiver and has to be ended.
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(1)).unwrap();
        handle.fail_send(1, UdpError::Io);
        assert_eq!(
            output.send_frame(universe(1), &frame(2)),
            Err(OutputError::Faulted)
        );
        handle.clear();
        output.shutdown();
        assert_eq!(handle.datagram_count(), TERMINATION_PACKETS);
        // And it ends the look the receiver actually has.
        assert_eq!(handle.last_datagram().unwrap().1[126], 1);
    }

    // ----- faults ----------------------------------------------------------

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
        let (mut output, handle) = plain(&[1], config());
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
        let (mut output, handle) = plain(&[1], config());
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
                expected: E131_DATA_BYTES,
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
    fn one_dead_receiver_does_not_stop_the_others() {
        let config = SacnConfig {
            destination: SacnDestination::receivers([host(1), host(2)]),
            ..config()
        };
        let (mut output, handle) = plain(&[1], config);
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
        let config = SacnConfig {
            destination: SacnDestination::receivers([host(1), host(2)]),
            ..config()
        };
        let (mut output, handle) = plain(&[1], config);
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
        // the machine waits out the whole keep-alive interval.
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
        assert_eq!(handle.last_datagram().unwrap().1[126], 2);
    }

    #[test]
    fn a_reconnected_output_sends_the_current_look_at_once() {
        // A receiver that has been away must not be told "nothing has changed
        // since a datagram you never received".
        let (mut output, handle) = connected();
        output.send_frame(universe(1), &frame(3)).unwrap();
        output.shutdown();
        handle.clear();

        output.connect().unwrap();
        output.send_frame(universe(1), &frame(3)).unwrap();
        assert_eq!(handle.datagram_count(), 1);
        assert_eq!(handle.last_datagram().unwrap().1[126], 3);
    }

    #[test]
    fn reconnecting_replaces_the_socket_rather_than_stacking_one_on_it() {
        let (mut output, handle) = connected();
        output.connect().unwrap();
        assert_eq!(handle.closes(), 1);
        assert_eq!(handle.binds().len(), 2);
        assert!(handle.is_open());
    }

    // ----- the shape of the output -----------------------------------------

    #[test]
    fn an_output_can_be_built_from_universes_alone() {
        // The ordinary configuration: the desk's universes, sent as themselves
        // at the priority every other desk sends. `with_ports` is for the rig
        // that needs something said about it.
        let socket = MockUdp::new();
        let handle = socket.handle();
        let mut output = SacnOutput::new(
            OutputId::new(2),
            [universe(1), universe(2)],
            socket,
            config(),
        );
        assert_eq!(output.id(), OutputId::new(2));
        assert_eq!(output.universes(), [universe(1), universe(2)]);
        assert_eq!(
            output.port(universe(2)),
            Some(SacnPort {
                universe: universe(2),
                sacn: sacn(2),
                priority: Priority::DEFAULT,
            })
        );
        output.connect().unwrap();
        output.send_frame(universe(2), &frame(5)).unwrap();
        let (to, packet) = handle.last_datagram().unwrap();
        assert_eq!(to, sacn(2).multicast_target());
        assert_eq!(&packet[113..115], &2u16.to_be_bytes());
        assert_eq!(packet[108], Priority::DEFAULT.get());
    }

    #[test]
    fn a_universe_named_twice_is_carried_once() {
        // Sent twice per cycle it would arrive with two sequence numbers, which
        // a receiver reads as reordering.
        let (mut output, handle) = plain(&[1, 1, 2], config());
        assert_eq!(output.universes(), [universe(1), universe(2)]);
        output.connect().unwrap();
        output.send_frame(universe(1), &frame(1)).unwrap();
        assert_eq!(handle.datagram_count(), 1);
    }

    #[test]
    fn a_datagram_count_is_not_a_frame_count() {
        let (mut output, _) = connected();
        for _ in 0..10 {
            output.send_frame(universe(1), &frame(1)).unwrap();
        }
        assert_eq!(output.datagrams_sent(), 1);
        output.shutdown();
        assert_eq!(output.datagrams_sent(), 1 + TERMINATION_PACKETS as u64);
    }

    #[test]
    fn an_output_knows_where_its_datagrams_come_from() {
        let (mut output, _) = plain(&[1], config());
        assert_eq!(output.local_addr(), None);
        output.connect().unwrap();
        assert_eq!(output.local_addr(), Some(SacnConfig::default().bind));
    }

    #[test]
    fn an_sacn_output_can_be_used_through_a_trait_object() {
        // The daemon holds outputs of different kinds in one list.
        let (output, handle) = plain(&[1], config());
        let mut outputs: Vec<Box<dyn DmxOutput>> = vec![Box::new(output)];
        for out in &mut outputs {
            out.connect().unwrap();
            out.send_frame(universe(1), &frame(1)).unwrap();
            assert_eq!(out.universes(), [universe(1)]);
            assert_eq!(out.health(), OutputHealth::Ok);
            out.shutdown();
        }
        assert_eq!(handle.datagram_count(), 1 + TERMINATION_PACKETS);
    }

    #[test]
    fn the_configuration_is_readable_and_says_what_the_specification_says() {
        let config = config();
        assert_eq!(config.refresh_interval, Duration::from_secs(1));
        assert_eq!(config.refresh_margin, TICK_PERIOD);
        assert_eq!(config.source_name, "PrismDMX Test");
        assert_eq!(config.cid, cid());
        assert_eq!(config.bind.port(), 0);
        assert!(!config.preview);
        assert_eq!(SacnConfig::default().source_name, "PrismDMX");
        let (output, _) = plain(&[1], config.clone());
        assert_eq!(output.config(), &config);
    }
}
