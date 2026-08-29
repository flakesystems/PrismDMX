//! `ArtPoll` out of a real socket, `ArtPollReply` back into one — S46.
//!
//! `artnet_wire.rs`'s argument, in the direction that did not exist before this
//! session. The unit tests in `discovery.rs` assert the conversation against
//! [`MockUdpNode`], which says what calls were made and nothing about what left
//! the machine; a `std::net::UdpSocket` bound to `127.0.0.1` is a real socket, so
//! the poll asserted here went out through the operating system's network stack
//! and the reply came back through it.
//!
//! **Loopback, and never anything else.** The mock node is a socket on
//! `127.0.0.1` with an ephemeral port, and it answers the address the poll came
//! from. Nothing here broadcasts, nothing here multicasts and nothing here binds
//! `0.0.0.0` — S10's rule, which matters twice over for a session about a
//! protocol whose ordinary discovery mechanism *is* a broadcast.
//!
//! Every timing assertion is a floor or is bounded by a generous deadline, which
//! is `prism-protocols`' own rule: the jitter gates live in `prism-engine`,
//! behind the mutex that stops two timing runs measuring each other.

use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use prism_domain::NodeHealth;
use prism_protocols::{
    ART_NET_ID, ART_POLL_BYTES, ART_POLL_REPLY_BYTES, DiscoveryConfig, NodeDiscovery, OP_POLL,
    OP_POLL_REPLY, PortAddress, SystemUdpNode,
};

/// How long a loopback exchange is given before the test calls it a failure.
/// Generous on purpose: this is a deadline, not a measurement.
const DEADLINE: Duration = Duration::from_secs(5);

/// A node that answers, as a real socket on loopback.
struct MockNode {
    socket: UdpSocket,
    address: SocketAddr,
    short_name: &'static str,
    ports: Vec<u8>,
}

impl MockNode {
    fn bind(short_name: &'static str, ports: &[u8]) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("a loopback socket");
        socket
            .set_read_timeout(Some(DEADLINE))
            .expect("a read timeout");
        let address = socket.local_addr().expect("a bound address");
        Self {
            socket,
            address,
            short_name,
            ports: ports.to_vec(),
        }
    }

    /// Waits for one ArtPoll and answers it. Returns what it was sent.
    fn answer_one_poll(&self) -> Vec<u8> {
        let mut buffer = [0u8; 64];
        let (len, from) = self.socket.recv_from(&mut buffer).expect("a poll arrives");
        let poll = buffer
            .get(..len)
            .expect("a prefix of its own length")
            .to_vec();
        self.socket
            .send_to(&self.reply(), from)
            .expect("the reply goes back to whoever asked");
        poll
    }

    /// The ArtPollReply this node sends.
    fn reply(&self) -> Vec<u8> {
        let mut packet = vec![0u8; ART_POLL_REPLY_BYTES];
        packet[..8].copy_from_slice(&ART_NET_ID);
        packet[8..10].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
        packet[10..14].copy_from_slice(&[127, 0, 0, 1]);
        packet[14..16].copy_from_slice(&6454u16.to_le_bytes());
        // VersInfo, high byte first.
        packet[16..18].copy_from_slice(&0x0104u16.to_be_bytes());
        packet[19] = 2; // SubSwitch
        packet[23] = 0xd0; // Status1
        packet[26..26 + self.short_name.len()].copy_from_slice(self.short_name.as_bytes());
        packet[44..44 + 15].copy_from_slice(b"A node on 127.1");
        packet[173] = u8::try_from(self.ports.len()).expect("at most four ports");
        for (index, port) in self.ports.iter().enumerate() {
            packet[174 + index] = 0x80; // PortTypes: an output
            packet[182 + index] = 0x80; // GoodOutputA
            packet[190 + index] = *port; // SwOut
        }
        packet[201..207].copy_from_slice(&[0x02, 0x1f, 0x8a, 0x00, 0x11, 0x22]);
        packet[211] = 1; // BindIndex
        packet[212] = 0x0e; // Status2
        packet
    }
}

/// A discovery on a real socket, listening on loopback with a port the operating
/// system chooses — never Art-Net's own, because a test suite has no business
/// taking a fixed port on the machine it runs on, and never `0.0.0.0`.
fn discovery() -> NodeDiscovery<SystemUdpNode> {
    let mut discovery = NodeDiscovery::new(
        SystemUdpNode::new(),
        DiscoveryConfig {
            bind: "127.0.0.1:0".parse().expect("a loopback address"),
            wait: Duration::from_millis(20),
            ..DiscoveryConfig::default()
        },
    );
    discovery.open().expect("a loopback socket binds");
    discovery
}

/// Services the discovery until it has read a reply, or the deadline passes.
fn service_until_a_reply(discovery: &mut NodeDiscovery<SystemUdpNode>) {
    let started = Instant::now();
    while started.elapsed() < DEADLINE {
        if discovery.service() > 0 {
            return;
        }
    }
    panic!("no reply arrived within {DEADLINE:?}");
}

/// The exit criterion, over a socket rather than over a double: a mock node that
/// answers ArtPoll is discovered, named and shown.
#[test]
fn a_node_on_a_real_socket_is_polled_answered_and_discovered() {
    let node = MockNode::bind("Stage left", &[0, 1, 2]);
    let mut discovery = discovery();
    discovery.set_targets([node.address]);

    // The poll leaves the machine and arrives as the fourteen bytes §6 names.
    discovery.service();
    let poll = node.answer_one_poll();
    assert_eq!(poll.len(), ART_POLL_BYTES);
    assert_eq!(poll.get(..8), Some(ART_NET_ID.as_slice()));
    assert_eq!(poll.get(8..10), Some(OP_POLL.to_le_bytes().as_slice()));
    assert_eq!(poll.get(10..12), Some(14u16.to_be_bytes().as_slice()));
    assert_eq!(
        (poll.get(12), poll.get(13)),
        (Some(&0), Some(&0)),
        "this desk asks for nothing unsolicited"
    );

    service_until_a_reply(&mut discovery);

    let nodes = discovery.nodes();
    assert_eq!(nodes.len(), 1);
    let found = nodes.first().expect("the node that answered");
    assert_eq!(found.address, node.address);
    assert_eq!(found.reply.short_name(), "Stage left");
    assert_eq!(found.reply.long_name(), "A node on 127.1");
    assert_eq!(found.reply.firmware, 0x0104);
    assert_eq!(found.reply.status1, 0xd0);
    assert_eq!(found.reply.status2, 0x0e);
    assert_eq!(found.reply.mac_address(), "02:1f:8a:00:11:22");
    assert_eq!(
        found.output_ports(),
        vec![
            PortAddress::from_parts(0, 2, 0).expect("a port address"),
            PortAddress::from_parts(0, 2, 1).expect("a port address"),
            PortAddress::from_parts(0, 2, 2).expect("a port address"),
        ],
        "the node's front panel says sub-net 2, universes 0 to 2"
    );

    let reach = discovery.reach(node.address);
    assert_eq!(reach.health, NodeHealth::Answering);
    assert_eq!(reach.name.as_deref(), Some("Stage left"));
    assert_eq!(reach.address, node.address.to_string());
}

/// The entry, over a socket: a configured node that never answers never reads
/// as answering — punch-list **B6**.
#[test]
fn a_real_socket_pointed_at_nothing_never_reports_a_node() {
    // A loopback port nothing is bound to. Taking one and dropping it is how the
    // test knows the number is free and that nothing will reply on it.
    let dead = {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("a loopback socket");
        socket.local_addr().expect("a bound address")
    };
    let mut discovery = discovery();
    discovery.set_targets([dead]);

    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(200) {
        discovery.service();
    }

    assert!(discovery.nodes().is_empty());
    let reach = discovery.reach(dead);
    assert_eq!(
        reach.health,
        NodeHealth::NeverAnswered,
        "a socket that took the datagram is not a node that received it"
    );
    assert_eq!(reach.last_reply_ago_ms, None);
    // The poll did go out, which is what makes the silence evidence.
    assert!(discovery.counters().polls_sent >= 1);
}

/// A node that announces itself, which is what a node does at power-up: the
/// socket is listening, so it is heard without having been asked.
#[test]
fn an_unsolicited_reply_on_a_real_socket_discovers_a_node_nobody_configured() {
    let node = MockNode::bind("New node", &[3]);
    let mut discovery = discovery();
    let listening = discovery.local_addr().expect("a bound discovery");
    node.socket
        .send_to(&node.reply(), listening)
        .expect("the announcement goes out");

    service_until_a_reply(&mut discovery);
    assert_eq!(discovery.nodes().len(), 1);
    assert_eq!(
        discovery
            .nodes()
            .first()
            .map(|found| found.reply.short_name()),
        Some("New node")
    );
    assert!(
        discovery.targets().is_empty(),
        "nothing was configured, and it was still heard"
    );
    assert_eq!(discovery.counters().polls_sent, 0, "and nothing was asked");
}

/// Rubbish on a real socket is dropped rather than believed.
#[test]
fn rubbish_delivered_over_a_real_socket_is_dropped_and_counted() {
    let sender = UdpSocket::bind("127.0.0.1:0").expect("a loopback socket");
    let mut discovery = discovery();
    let listening = discovery.local_addr().expect("a bound discovery");
    for datagram in [
        b"not art-net at all".to_vec(),
        vec![0xffu8; ART_POLL_REPLY_BYTES],
        Vec::new(),
    ] {
        sender
            .send_to(&datagram, listening)
            .expect("the rubbish goes out");
    }

    let started = Instant::now();
    while discovery.counters().malformed < 3 && started.elapsed() < DEADLINE {
        discovery.service();
    }
    assert_eq!(discovery.counters().malformed, 3);
    assert_eq!(discovery.counters().replies, 0);
    assert!(discovery.nodes().is_empty());
    assert!(
        discovery.is_open(),
        "rubbish does not close the socket, or one packet would end discovery"
    );
}
