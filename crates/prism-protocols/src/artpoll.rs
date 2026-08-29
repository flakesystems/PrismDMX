//! `ArtPoll` and `ArtPollReply`: asking a network what is on it — S46.
//!
//! `ARCHITECTURE_SPEC.md` §7.2, and §6 of the Art-Net 4 specification, which is
//! where every field order below comes from. This is the **first receive path in
//! the workspace**: everything in `prism-protocols` before it wrote bytes and
//! never read any, which is why `Health::Ok` on an Art-Net output has meant *the
//! socket accepted the datagram* since S9 — and UDP always accepts it. That is
//! punch-list **B6**.
//!
//! ```text
//!   ArtPoll, 14 bytes                        ArtPollReply, 239 bytes
//!   ┌────────────┬────┬────┬────┬────┐       ┌────────────┬────┬──────┬────┬───┈
//!   │ "Art-Net\0"│OpCo│ProV│Flag│Prio│       │ "Art-Net\0"│OpCo│IP[4] │Port│
//!   │  8 bytes   │0x20│ 14 │ 0  │ 0  │       │  8 bytes   │0x21│      │6454│
//!   └────────────┴────┴────┴────┴────┘       └────────────┴────┴──────┴────┴───┈
//!     0        7   8 9 10 11  12   13          0        7   8 9  10  13 14 15
//! ```
//!
//! # Two rules that are not in the packet
//!
//! **A reply is an input from outside, so it is parsed and never believed.**
//! [`parse_art_poll_reply`] answers `Option`, it reads only fixed offsets inside
//! a length it has checked, and it **allocates nothing at all**: the three names
//! stay as the byte arrays they arrived in and are handed out as `&str`. That is
//! what lets `tests/artpoll_fuzz.rs` push a quarter of a million random bytes
//! through it and assert an allocation count of zero, which is the rule
//! `prism-surface` established for the other stream of bytes a stranger controls
//! (`docs/MCU_MAPPING.md` §6).
//!
//! **This desk polls where it already sends.** Nothing here broadcasts and
//! nothing above it does either — see [`crate::NodeDiscovery`] for the argument.
//! The `Flags` byte is therefore **zero**: bit 1 would ask every node to send an
//! unsolicited `ArtPollReply` whenever anything about it changes, which on a rig
//! of forty nodes is a burst this desk did not need and cannot pace.
//!
//! # The port-address table
//!
//! A node's universes are spread over three fields that look unrelated:
//! `NetSwitch` is the top seven bits of every port address it carries,
//! `SubSwitch` is the next four, and `SwOut[i]` is the bottom four of output
//! port *i*. [`ArtPollReply::output_ports`] puts them back together into the
//! [`PortAddress`] this crate already speaks, which is what makes *this node
//! outputs universe 3 and nothing here sends it one* a comparison rather than an
//! interpretation.

use core::fmt;
use std::net::Ipv4Addr;

use crate::artnet::{ART_NET_ID, PortAddress};

/// `OpPoll` — the packet that asks every node to describe itself.
pub const OP_POLL: u16 = 0x2000;

/// `OpPollReply` — a node describing itself.
pub const OP_POLL_REPLY: u16 = 0x2100;

/// Bytes in an ArtPoll packet.
pub const ART_POLL_BYTES: usize = 14;

/// The shortest ArtPollReply this parser will accept: through `MAC`.
///
/// The specification's packet is [`ART_POLL_REPLY_BYTES`] long and the fields
/// after the MAC address — `BindIp`, `BindIndex`, `Status2` and the Art-Net 4
/// additions — were added over successive revisions. Nodes in the field send
/// short ones, so a reply that carries everything this desk actually reads is
/// accepted and the rest is taken as zero, rather than a working node being
/// dropped for a field written after it was manufactured.
pub const ART_POLL_REPLY_MIN: usize = 207;

/// Bytes in a full Art-Net 4 ArtPollReply.
pub const ART_POLL_REPLY_BYTES: usize = 239;

/// Bytes in the `ShortName` field.
pub const SHORT_NAME_BYTES: usize = 18;

/// Bytes in the `LongName` and `NodeReport` fields.
pub const LONG_NAME_BYTES: usize = 64;

/// Ports one ArtPollReply can describe. A node with more sends one reply per
/// group of four, told apart by `BindIndex`.
pub const MAX_NODE_PORTS: usize = 4;

/// `Flags` bit 1: *send me an ArtPollReply whenever anything about you changes*.
///
/// Named and never set by this desk — see the module documentation.
pub const POLL_FLAG_REPLY_ON_CHANGE: u8 = 0x02;

/// `PortTypes` bit 7: this port can output DMX from Art-Net.
const PORT_TYPE_OUTPUT: u8 = 0x80;

/// `PortTypes` bit 6: this port can input DMX into Art-Net.
const PORT_TYPE_INPUT: u8 = 0x40;

// Field offsets, written out rather than accumulated, so a reader can check them
// against §6 of the specification without adding anything up.
const OFF_OPCODE: usize = 8;
const OFF_IP: usize = 10;
const OFF_PORT: usize = 14;
const OFF_VERSION: usize = 16;
const OFF_NET_SWITCH: usize = 18;
const OFF_SUB_SWITCH: usize = 19;
const OFF_OEM: usize = 20;
const OFF_STATUS1: usize = 23;
const OFF_ESTA: usize = 24;
const OFF_SHORT_NAME: usize = 26;
const OFF_LONG_NAME: usize = 44;
const OFF_NODE_REPORT: usize = 108;
const OFF_NUM_PORTS: usize = 172;
const OFF_PORT_TYPES: usize = 174;
const OFF_GOOD_INPUT: usize = 178;
const OFF_GOOD_OUTPUT: usize = 182;
const OFF_SW_IN: usize = 186;
const OFF_SW_OUT: usize = 190;
const OFF_STYLE: usize = 200;
const OFF_MAC: usize = 201;
const OFF_BIND_INDEX: usize = 211;
const OFF_STATUS2: usize = 212;

/// One ArtPoll packet.
///
/// `flags` is the specification's `Flags` byte and `priority` its
/// `DiversityPriority`. Both are zero for this desk — see the module
/// documentation for why the first one is, and the second is only read by nodes
/// doing sACN priority arbitration, which this packet is not part of.
#[must_use]
pub const fn art_poll(flags: u8, priority: u8) -> [u8; ART_POLL_BYTES] {
    [
        // ID[8]: "Art-Net\0".
        0x41, 0x72, 0x74, 0x2D, 0x4E, 0x65, 0x74, 0x00,
        // OpCode: 0x2000, transmitted low byte first.
        0x00, 0x20, // ProtVerHi, ProtVerLo: 14, transmitted high byte first.
        0x00, 14, // Flags, DiversityPriority.
        flags, priority,
    ]
}

/// One node's answer, as it arrived.
///
/// **Plain data with no owned allocation in it**, which is the whole design: the
/// names stay as the fixed byte arrays the packet carries them in and are handed
/// out as `&str` by [`short_name`](Self::short_name) and friends. A parser that
/// built three `String`s per datagram would hand anything that can reach UDP
/// port 6454 a way to make this process call the allocator in a loop — the fault
/// `docs/MCU_MAPPING.md` §2.4 names one protocol along, and the reason
/// `tests/artpoll_fuzz.rs` can assert zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtPollReply {
    /// `IP` — the address the node says it has, which is not always the address
    /// the datagram came from.
    pub ip: Ipv4Addr,
    /// `Port` — Art-Net's own, on every node that follows the specification.
    pub port: u16,
    /// `VersInfoH`/`VersInfoL` — the node's firmware revision.
    pub firmware: u16,
    /// `NetSwitch` — the top seven bits of every port address this node carries.
    pub net_switch: u8,
    /// `SubSwitch` — the next four bits.
    pub sub_switch: u8,
    /// `OemHi`/`OemLo` — which product it is, as the OEM table numbers it.
    pub oem: u16,
    /// `EstaManLo`/`EstaManHi` — the manufacturer's ESTA code.
    pub esta: u16,
    /// `Status1` — the node's own diagnostics, carried and not interpreted.
    pub status1: u8,
    /// `Status2` — the same, and zero on a reply too short to carry it.
    pub status2: u8,
    /// `Style` — `StNode`, `StController`, `StMedia`…
    pub style: u8,
    /// `BindIndex` — which group of four ports this reply describes, `1` for the
    /// first. Zero on a reply too short to carry it, which the specification
    /// says is to be read as 1.
    pub bind_index: u8,
    /// `NumPortsLo`, clamped to [`MAX_NODE_PORTS`].
    pub ports: u8,
    /// `PortTypes[4]`.
    pub port_types: [u8; MAX_NODE_PORTS],
    /// `GoodInput[4]`.
    pub good_input: [u8; MAX_NODE_PORTS],
    /// `GoodOutputA[4]`.
    pub good_output: [u8; MAX_NODE_PORTS],
    /// `SwIn[4]` — the bottom four bits of each input port's address.
    pub sw_in: [u8; MAX_NODE_PORTS],
    /// `SwOut[4]` — the bottom four bits of each output port's address.
    pub sw_out: [u8; MAX_NODE_PORTS],
    /// `MAC[6]` — the one identifier that survives the node being re-addressed.
    pub mac: [u8; 6],
    /// `ShortName`, as bytes. Read through [`short_name`](Self::short_name).
    short_name: [u8; SHORT_NAME_BYTES],
    /// `LongName`, as bytes. Read through [`long_name`](Self::long_name).
    long_name: [u8; LONG_NAME_BYTES],
    /// `NodeReport`, as bytes. Read through [`node_report`](Self::node_report).
    node_report: [u8; LONG_NAME_BYTES],
}

impl ArtPollReply {
    /// `ShortName` — what the node's front panel calls itself.
    #[must_use]
    pub fn short_name(&self) -> &str {
        text(&self.short_name)
    }

    /// `LongName`.
    #[must_use]
    pub fn long_name(&self) -> &str {
        text(&self.long_name)
    }

    /// `NodeReport` — the node's own last status line, in its own words.
    #[must_use]
    pub fn node_report(&self) -> &str {
        text(&self.node_report)
    }

    /// The MAC address as six lower-case hexadecimal pairs.
    ///
    /// The one thing on this type that allocates, and it allocates **once**, in
    /// a capacity it knows: it is called when a node is written into the
    /// discovery table, not once per datagram. Written out rather than through
    /// `format!` for that reason — `format!` in a six-iteration loop is six
    /// allocations to build one seventeen-byte string.
    #[must_use]
    pub fn mac_address(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(17);
        for (index, byte) in self.mac.iter().enumerate() {
            if index > 0 {
                text.push(':');
            }
            for nibble in [byte >> 4, byte & 0x0f] {
                text.push(char::from(
                    HEX.get(usize::from(nibble)).copied().unwrap_or(b'0'),
                ));
            }
        }
        text
    }

    /// The port addresses this node **outputs** DMX on.
    ///
    /// `NetSwitch` and `SubSwitch` are the same for every port of one reply; only
    /// the bottom nibble differs, which is why a node's four universes are
    /// consecutive unless somebody has been at the front panel.
    #[must_use]
    pub fn output_ports(&self) -> Vec<PortAddress> {
        self.addresses(&self.sw_out, PORT_TYPE_OUTPUT)
    }

    /// The port addresses this node sends DMX **into** the network on.
    #[must_use]
    pub fn input_ports(&self) -> Vec<PortAddress> {
        self.addresses(&self.sw_in, PORT_TYPE_INPUT)
    }

    /// The ports of one direction, as fifteen-bit addresses.
    fn addresses(&self, switches: &[u8; MAX_NODE_PORTS], direction: u8) -> Vec<PortAddress> {
        let count = usize::from(self.ports).min(MAX_NODE_PORTS);
        let mut addresses = Vec::with_capacity(count);
        for index in 0..count {
            let kind = self.port_types.get(index).copied().unwrap_or(0);
            if kind & direction == 0 {
                continue;
            }
            let switch = switches.get(index).copied().unwrap_or(0);
            if let Some(address) = PortAddress::from_parts(
                self.net_switch & 0x7f,
                self.sub_switch & 0x0f,
                switch & 0x0f,
            ) {
                addresses.push(address);
            }
        }
        addresses
    }
}

impl fmt::Display for ArtPollReply {
    /// `"Node name" at 10.0.0.9`, which is the log line an installer reads.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\" at {}", self.short_name(), self.ip)
    }
}

/// Reads one ArtPollReply out of a datagram, or answers `None`.
///
/// **Three ways to be dropped rather than believed**, and none of them is a
/// panic: a datagram shorter than [`ART_POLL_REPLY_MIN`], one whose first eight
/// bytes are not `Art-Net\0`, and one whose opcode is not
/// [`OP_POLL_REPLY`] — which is how an ArtDmx frame this desk is echoing to
/// itself, or another controller's ArtPoll, is stepped over rather than read as
/// a node.
///
/// The protocol version is deliberately **not** checked. §6 puts `ProtVer` in
/// ArtPoll and not in ArtPollReply at all, so a parser demanding one would be
/// demanding a field the specification does not send.
#[must_use]
pub fn parse_art_poll_reply(datagram: &[u8]) -> Option<ArtPollReply> {
    if datagram.len() < ART_POLL_REPLY_MIN {
        return None;
    }
    if datagram.get(..ART_NET_ID.len())? != ART_NET_ID {
        return None;
    }
    if u16::from_le_bytes([byte(datagram, OFF_OPCODE), byte(datagram, OFF_OPCODE + 1)])
        != OP_POLL_REPLY
    {
        return None;
    }
    Some(ArtPollReply {
        ip: Ipv4Addr::from(fixed::<4>(datagram, OFF_IP)),
        port: u16::from_le_bytes([byte(datagram, OFF_PORT), byte(datagram, OFF_PORT + 1)]),
        // VersInfo is high byte first, unlike Port.
        firmware: u16::from_be_bytes([
            byte(datagram, OFF_VERSION),
            byte(datagram, OFF_VERSION + 1),
        ]),
        net_switch: byte(datagram, OFF_NET_SWITCH),
        sub_switch: byte(datagram, OFF_SUB_SWITCH),
        oem: u16::from_be_bytes([byte(datagram, OFF_OEM), byte(datagram, OFF_OEM + 1)]),
        esta: u16::from_le_bytes([byte(datagram, OFF_ESTA), byte(datagram, OFF_ESTA + 1)]),
        status1: byte(datagram, OFF_STATUS1),
        status2: byte(datagram, OFF_STATUS2),
        style: byte(datagram, OFF_STYLE),
        bind_index: byte(datagram, OFF_BIND_INDEX),
        // NumPortsHi is reserved and NumPortsLo is the count; a node claiming
        // more than four is describing a packet that cannot hold them, so the
        // claim is clamped rather than trusted.
        ports: byte(datagram, OFF_NUM_PORTS + 1)
            .min(u8::try_from(MAX_NODE_PORTS).unwrap_or(u8::MAX)),
        port_types: fixed(datagram, OFF_PORT_TYPES),
        good_input: fixed(datagram, OFF_GOOD_INPUT),
        good_output: fixed(datagram, OFF_GOOD_OUTPUT),
        sw_in: fixed(datagram, OFF_SW_IN),
        sw_out: fixed(datagram, OFF_SW_OUT),
        mac: fixed(datagram, OFF_MAC),
        short_name: fixed(datagram, OFF_SHORT_NAME),
        long_name: fixed(datagram, OFF_LONG_NAME),
        node_report: fixed(datagram, OFF_NODE_REPORT),
    })
}

/// One byte, or zero past the end.
///
/// Every field after [`ART_POLL_REPLY_MIN`] is optional by construction, so
/// reading past a short reply answers the zero the specification's own default
/// is rather than refusing a node for a field written after it was made. It is
/// also why nothing in this module indexes a slice: a hostile datagram is
/// exactly a short one, and `CLAUDE.md`'s zero-crash invariant does not make
/// exceptions for a parser.
fn byte(datagram: &[u8], at: usize) -> u8 {
    datagram.get(at).copied().unwrap_or(0)
}

/// `N` bytes, zero-filled past the end.
fn fixed<const N: usize>(datagram: &[u8], at: usize) -> [u8; N] {
    let mut bytes = [0u8; N];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = byte(datagram, at + index);
    }
    bytes
}

/// A node's name, out of a fixed field a stranger filled in.
///
/// The field is NUL-padded ASCII by specification and is whatever arrived in
/// fact, so the run this reads is the leading one of **printable ASCII**: it
/// stops at the terminator, at a control byte, and at the first byte with the
/// top bit set. Every byte in that run is below 0x80, so the slice is valid
/// UTF-8 by construction and there is no failure path to have an opinion about —
/// and a node whose name is a stream of control characters reads as empty rather
/// than as a row that puts them into a log line and a browser.
fn text(field: &[u8]) -> &str {
    let end = field
        .iter()
        .position(|&byte| !(0x20..0x7f).contains(&byte))
        .unwrap_or(field.len());
    field
        .get(..end)
        .and_then(|run| core::str::from_utf8(run).ok())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::{
        ART_POLL_BYTES, ART_POLL_REPLY_BYTES, ART_POLL_REPLY_MIN, LONG_NAME_BYTES, MAX_NODE_PORTS,
        OFF_BIND_INDEX, OFF_ESTA, OFF_GOOD_INPUT, OFF_GOOD_OUTPUT, OFF_IP, OFF_LONG_NAME, OFF_MAC,
        OFF_NET_SWITCH, OFF_NODE_REPORT, OFF_NUM_PORTS, OFF_OEM, OFF_OPCODE, OFF_PORT,
        OFF_PORT_TYPES, OFF_SHORT_NAME, OFF_STATUS1, OFF_STATUS2, OFF_STYLE, OFF_SUB_SWITCH,
        OFF_SW_IN, OFF_SW_OUT, OFF_VERSION, OP_POLL, OP_POLL_REPLY, POLL_FLAG_REPLY_ON_CHANGE,
        SHORT_NAME_BYTES, art_poll, parse_art_poll_reply, text,
    };
    use crate::artnet::{ART_NET_ID, ART_NET_PORT, PortAddress};
    use proptest::prelude::*;
    use std::net::Ipv4Addr;

    /// A reply a well-behaved four-port node would send.
    fn reply() -> Vec<u8> {
        let mut packet = vec![0u8; ART_POLL_REPLY_BYTES];
        packet[..8].copy_from_slice(&ART_NET_ID);
        packet[OFF_OPCODE..OFF_OPCODE + 2].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
        packet[OFF_IP..OFF_IP + 4].copy_from_slice(&[10, 0, 0, 9]);
        packet[OFF_PORT..OFF_PORT + 2].copy_from_slice(&ART_NET_PORT.to_le_bytes());
        packet[OFF_VERSION..OFF_VERSION + 2].copy_from_slice(&0x0104u16.to_be_bytes());
        packet[OFF_NET_SWITCH] = 0;
        packet[OFF_SUB_SWITCH] = 1;
        packet[OFF_OEM..OFF_OEM + 2].copy_from_slice(&0x00ffu16.to_be_bytes());
        packet[OFF_STATUS1] = 0xd0;
        packet[OFF_ESTA..OFF_ESTA + 2].copy_from_slice(&0x7a70u16.to_le_bytes());
        packet[OFF_SHORT_NAME..OFF_SHORT_NAME + 10].copy_from_slice(b"Stage left");
        packet[OFF_LONG_NAME..OFF_LONG_NAME + 22].copy_from_slice(b"Stage left node, dock2");
        packet[OFF_NODE_REPORT..OFF_NODE_REPORT + 14].copy_from_slice(b"#0001 [0002] O");
        packet[OFF_NUM_PORTS + 1] = 4;
        packet[OFF_PORT_TYPES..OFF_PORT_TYPES + 4].copy_from_slice(&[0x80, 0x80, 0x80, 0x40]);
        packet[OFF_GOOD_INPUT..OFF_GOOD_INPUT + 4].copy_from_slice(&[0, 0, 0, 0x80]);
        packet[OFF_GOOD_OUTPUT..OFF_GOOD_OUTPUT + 4].copy_from_slice(&[0x80, 0x80, 0x80, 0]);
        packet[OFF_SW_IN..OFF_SW_IN + 4].copy_from_slice(&[0, 0, 0, 7]);
        packet[OFF_SW_OUT..OFF_SW_OUT + 4].copy_from_slice(&[0, 1, 2, 0]);
        packet[OFF_STYLE] = 0;
        packet[OFF_MAC..OFF_MAC + 6].copy_from_slice(&[0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]);
        packet[OFF_BIND_INDEX] = 1;
        packet[OFF_STATUS2] = 0x0e;
        packet
    }

    #[test]
    fn an_art_poll_is_the_fourteen_bytes_the_specification_names() {
        let poll = art_poll(0, 0);
        assert_eq!(poll.len(), ART_POLL_BYTES);
        assert_eq!(&poll[..8], &ART_NET_ID);
        assert_eq!(&poll[8..10], &OP_POLL.to_le_bytes());
        assert_eq!(&poll[10..12], &14u16.to_be_bytes());
        assert_eq!(
            (poll[12], poll[13]),
            (0, 0),
            "this desk asks for nothing unsolicited and arbitrates no priority"
        );
    }

    #[test]
    fn a_poll_can_still_ask_for_unsolicited_replies_when_somebody_wants_them() {
        // Carried as an argument rather than hard-coded, so the choice in the
        // module documentation is a decision the caller makes rather than a
        // constant nobody can see.
        let poll = art_poll(POLL_FLAG_REPLY_ON_CHANGE, 200);
        assert_eq!((poll[12], poll[13]), (0x02, 200));
    }

    #[test]
    fn a_node_reply_is_read_field_by_field() {
        let parsed = parse_art_poll_reply(&reply()).expect("a well-formed reply parses");
        assert_eq!(parsed.ip, Ipv4Addr::new(10, 0, 0, 9));
        assert_eq!(parsed.port, ART_NET_PORT);
        assert_eq!(parsed.firmware, 0x0104);
        assert_eq!(parsed.oem, 0x00ff);
        assert_eq!(parsed.esta, 0x7a70);
        assert_eq!(parsed.status1, 0xd0);
        assert_eq!(parsed.status2, 0x0e);
        assert_eq!(parsed.style, 0);
        assert_eq!(parsed.bind_index, 1);
        assert_eq!(parsed.short_name(), "Stage left");
        assert_eq!(parsed.long_name(), "Stage left node, dock2");
        assert_eq!(parsed.node_report(), "#0001 [0002] O");
        assert_eq!(parsed.mac_address(), "00:1a:2b:3c:4d:5e");
        assert_eq!(parsed.to_string(), "\"Stage left\" at 10.0.0.9");
    }

    /// The three fields that look unrelated in the packet and are one number.
    #[test]
    fn the_port_address_table_is_net_sub_net_and_the_switch_put_back_together() {
        let parsed = parse_art_poll_reply(&reply()).expect("a well-formed reply parses");
        let outputs = parsed.output_ports();
        assert_eq!(
            outputs,
            vec![
                PortAddress::from_parts(0, 1, 0).unwrap(),
                PortAddress::from_parts(0, 1, 1).unwrap(),
                PortAddress::from_parts(0, 1, 2).unwrap(),
            ],
            "the fourth port is an input and is not an output"
        );
        assert_eq!(outputs[0].get(), 0x10);
        assert_eq!(
            parsed.input_ports(),
            vec![PortAddress::from_parts(0, 1, 7).unwrap()]
        );
    }

    #[test]
    fn a_node_claiming_more_ports_than_a_packet_can_hold_is_clamped() {
        let mut packet = reply();
        packet[OFF_NUM_PORTS + 1] = 255;
        packet[OFF_PORT_TYPES..OFF_PORT_TYPES + 4].copy_from_slice(&[0x80; 4]);
        let parsed = parse_art_poll_reply(&packet).expect("still a reply");
        assert_eq!(usize::from(parsed.ports), MAX_NODE_PORTS);
        assert_eq!(parsed.output_ports().len(), MAX_NODE_PORTS);
    }

    #[test]
    fn a_node_with_no_ports_at_all_is_a_node_with_no_universes() {
        let mut packet = reply();
        packet[OFF_NUM_PORTS + 1] = 0;
        let parsed = parse_art_poll_reply(&packet).expect("still a reply");
        assert!(parsed.output_ports().is_empty());
        assert!(parsed.input_ports().is_empty());
    }

    /// The three ways a datagram is dropped rather than believed.
    #[test]
    fn a_malformed_reply_is_dropped_rather_than_read() {
        let good = reply();
        assert!(parse_art_poll_reply(&good[..ART_POLL_REPLY_MIN - 1]).is_none());
        assert!(parse_art_poll_reply(&[]).is_none());

        let mut wrong_id = good.clone();
        wrong_id[0] = b'B';
        assert!(parse_art_poll_reply(&wrong_id).is_none());

        let mut wrong_opcode = good.clone();
        wrong_opcode[OFF_OPCODE..OFF_OPCODE + 2].copy_from_slice(&OP_POLL.to_le_bytes());
        assert!(
            parse_art_poll_reply(&wrong_opcode).is_none(),
            "another controller's ArtPoll is not a node"
        );

        let mut art_dmx = good;
        art_dmx[OFF_OPCODE..OFF_OPCODE + 2].copy_from_slice(&crate::OP_DMX.to_le_bytes());
        assert!(
            parse_art_poll_reply(&art_dmx).is_none(),
            "a frame this desk sent and heard back is not a node"
        );
    }

    /// A node made before the fields after the MAC address existed.
    #[test]
    fn a_short_reply_is_a_node_with_the_late_fields_at_zero() {
        let packet = reply();
        let parsed = parse_art_poll_reply(&packet[..ART_POLL_REPLY_MIN]).expect("a short reply");
        assert_eq!(parsed.short_name(), "Stage left");
        assert_eq!(parsed.mac_address(), "00:1a:2b:3c:4d:5e");
        assert_eq!(parsed.bind_index, 0);
        assert_eq!(parsed.status2, 0);
    }

    #[test]
    fn a_name_stops_at_the_terminator_and_at_anything_unprintable() {
        assert_eq!(text(b"Node\0\0\0"), "Node");
        assert_eq!(text(b"Node"), "Node");
        assert_eq!(text(b""), "");
        assert_eq!(text(&[0x01, b'N']), "", "a control byte ends it at once");
        assert_eq!(
            text(&[b'N', 0xff, b'o']),
            "N",
            "the top bit set ends it, so the slice is ASCII by construction"
        );
    }

    /// A name field a node filled with rubbish must not reach a log line.
    #[test]
    fn a_reply_whose_names_are_rubbish_reads_as_a_node_with_no_name() {
        let mut packet = reply();
        packet[OFF_SHORT_NAME..OFF_SHORT_NAME + SHORT_NAME_BYTES]
            .copy_from_slice(&[0xffu8; SHORT_NAME_BYTES]);
        packet[OFF_LONG_NAME..OFF_LONG_NAME + LONG_NAME_BYTES]
            .copy_from_slice(&[0x07u8; LONG_NAME_BYTES]);
        let parsed = parse_art_poll_reply(&packet).expect("still a reply");
        assert_eq!(parsed.short_name(), "");
        assert_eq!(parsed.long_name(), "");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        /// The claim the whole receive path rests on: **anything at all** may
        /// arrive on a UDP port, and reading it must not panic.
        #[test]
        fn any_bytes_at_all_are_parsed_or_dropped_without_panicking(
            datagram in proptest::collection::vec(any::<u8>(), 0..600)
        ) {
            if let Some(parsed) = parse_art_poll_reply(&datagram) {
                // Everything a caller may read is read, so a panic hiding in an
                // accessor is a failure here rather than in the daemon.
                let _ = parsed.short_name();
                let _ = parsed.long_name();
                let _ = parsed.node_report();
                let _ = parsed.mac_address();
                prop_assert!(parsed.output_ports().len() <= MAX_NODE_PORTS);
                prop_assert!(parsed.input_ports().len() <= MAX_NODE_PORTS);
                prop_assert!(usize::from(parsed.ports) <= MAX_NODE_PORTS);
            }
        }

        /// A reply with its header intact and everything else random is still a
        /// reply, which is the case a purely random stream almost never reaches.
        #[test]
        fn a_reply_with_a_valid_header_and_a_random_body_is_always_read(
            body in proptest::collection::vec(any::<u8>(), 200..400)
        ) {
            let mut datagram = Vec::with_capacity(10 + body.len());
            datagram.extend_from_slice(&ART_NET_ID);
            datagram.extend_from_slice(&OP_POLL_REPLY.to_le_bytes());
            datagram.extend_from_slice(&body);
            let parsed = parse_art_poll_reply(&datagram).expect("the header makes it a reply");
            prop_assert!(parsed.output_ports().len() <= MAX_NODE_PORTS);
            prop_assert!(parsed.short_name().len() <= SHORT_NAME_BYTES);
            prop_assert!(parsed.long_name().len() <= LONG_NAME_BYTES);
            prop_assert_eq!(parsed.mac_address().len(), 17);
        }
    }
}
