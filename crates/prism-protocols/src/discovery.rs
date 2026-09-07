//! Art-Net node discovery: the table that knows whether anything is listening —
//! S46.
//!
//! `ARCHITECTURE_SPEC.md` §7.2 and punch-list **B6**. [`ArtNetOutput`] reports
//! `OutputHealth::Ok` as soon as its socket has taken the datagram, and UDP takes
//! every datagram there is; an installer reading *OK* over an empty rack is being
//! told something the program does not know. Knowing needs an answer from the far
//! end, and this is where one is asked for and remembered.
//!
//! ```text
//!   configured nodes ──ArtPoll──▶  ┌──────┐
//!        (unicast)                 │ node │
//!   NodeDiscovery  ◀─ArtPollReply─ └──────┘
//!        │
//!        ├─ nodes()   what is out there        (the discovery table)
//!        └─ reach(a)  whether a answers        (Answering / Never / Stopped)
//! ```
//!
//! # Where the poll goes, and why it is never a broadcast
//!
//! **This desk polls where it already sends.** [`NodeDiscovery::set_targets`]
//! takes the addresses the rig's Art-Net outputs are configured for, and the poll
//! is unicast to each. Nothing here broadcasts, which keeps two rules that S9 and
//! S10 wrote down and that a school network depends on: `Destination::Unicast` is
//! the default because broadcast Art-Net is a denial of service performed by the
//! lighting desk, and **no test may put a broadcast or a multicast datagram on
//! the network it runs on** (S10). A poll to an address this desk is already
//! sending 530 bytes to forty-four times a second needs no new permission and can
//! reach nothing new.
//!
//! What that costs is named rather than hidden: a node at an address nobody has
//! typed is found only when it **announces itself**, which nodes do at power-up
//! and whenever their configuration changes. That covers the case the deliverable
//! is about — plug a node in, see it, add it — because the socket is bound to
//! Art-Net's own port and hears the announcement. A node that neither answers
//! where this desk sends nor announces itself is not discovered, and finding one
//! would need the broadcast this desk deliberately does not send.
//!
//! # The three states, and which one silence is
//!
//! [`prism_domain::NodeHealth`]. A node is *answering* while its last reply is
//! younger than [`DiscoveryConfig::reply_timeout`], *never answered* if it has
//! never sent one, and *stopped* otherwise — with the age of the last reply
//! beside it, because *it stopped four minutes ago* and *it stopped as I walked
//! to the rack* are different faults.
//!
//! The timeout is **one poll interval and a little**, which is what the exit
//! criterion asks for: a node that stops is reported stopped within one interval.
//! The price is stated rather than smoothed away — one lost reply reads as
//! *stopped* for one interval — and it is the right price, because a node
//! answering one poll in two is not a node an installer should be told is well.
//!
//! # Nothing here is the tick's
//!
//! [`NodeDiscovery::service`] blocks on a socket read. It runs on a thread of its
//! own (`prismd::discovery`), never the engine's and never an output's, and a
//! socket that will not bind degrades **alone** — S33's rule for every driver,
//! and it matters more here because Art-Net's port is a fixed number that another
//! program on the machine may already hold.
//!
//! # A reply is an input from outside
//!
//! Two defences, both measured. [`crate::parse_art_poll_reply`] allocates nothing
//! and drops what it cannot read; and the table is **bounded**
//! ([`DiscoveryConfig::max_nodes`]), because a stream of replies from made-up
//! source addresses would otherwise be an unbounded `Vec` grown by a stranger.
//! Beyond the bound a node is counted and dropped, and the counter is what a
//! settings panel would show if this ever happened in a hall.

use std::net::SocketAddr;
use std::time::Duration;

use prism_domain::{NodeHealth, NodeReach};
use prism_engine::{Clock, SystemClock};

use crate::artnet::{ART_NET_PORT, PortAddress};
use crate::artpoll::{
    ArtPollReply, LONG_NAME_BYTES, MAX_NODE_PORTS, art_poll, parse_art_poll_reply,
};
use crate::udp::{UdpError, UdpNode};

/// Bytes read from the socket at once.
///
/// One Ethernet MTU, which is more than any Art-Net packet and enough that a
/// node padding its reply is read rather than refused — see
/// `NodeDiscovery::buffer`, which is private. A datagram larger than this is
/// still dropped, and
/// dropped **without ending the pass**, because a stranger sending one giant
/// packet must not stop the reply behind it being read.
pub const RECV_BUFFER_BYTES: usize = 1_500;

/// How a discovery behaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryConfig {
    /// The local address to listen on.
    ///
    /// Art-Net's own port, because a node's reply is addressed to it and not to
    /// wherever the poll came from — which is the one thing that makes this a
    /// bound listener rather than a request and a response.
    pub bind: SocketAddr,
    /// How often each configured node is polled. The specification's own
    /// guidance is every 2.5 to 3 seconds.
    pub poll_interval: Duration,
    /// How long a node may be silent before it reads as *stopped*.
    ///
    /// One poll interval and a margin — see this module's documentation for the
    /// trade that number is.
    pub reply_timeout: Duration,
    /// The longest one [`NodeDiscovery::service`] pass will wait for a datagram.
    ///
    /// It is what bounds how long the thread takes to notice it has been asked
    /// to stop, so it is much shorter than the poll interval.
    pub wait: Duration,
    /// Datagrams read in one pass before the pass ends.
    ///
    /// A bound rather than *drain the socket*: a stream of rubbish arriving
    /// faster than it can be read must not be able to hold this loop inside one
    /// pass for ever, which is the same reasoning `prism_ipc`'s backpressure
    /// applies to a client that will not read.
    pub recv_budget: usize,
    /// The most nodes the table will hold. See this module's documentation.
    pub max_nodes: usize,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from(([0, 0, 0, 0], ART_NET_PORT)),
            poll_interval: Duration::from_secs(3),
            reply_timeout: Duration::from_millis(3_500),
            wait: Duration::from_millis(100),
            recv_budget: 64,
            max_nodes: 64,
        }
    }
}

/// One node this desk has heard from.
///
/// Holds the reply's fields as the reply held them — fixed arrays, no `Vec` —
/// so that a node answering every three seconds for a week costs the allocator
/// nothing after the first reply. The `Vec`s a client sees are built when the
/// table is **read**, which happens only while somebody has the panel open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredNode {
    /// Where the reply came from.
    pub address: SocketAddr,
    /// The last reply, whole.
    pub reply: ArtPollReply,
    /// When the first one arrived, on the discovery's clock.
    pub first_seen: Duration,
    /// When the last one did.
    pub last_seen: Duration,
    /// How many have arrived.
    pub replies: u64,
}

impl DiscoveredNode {
    /// The node's output port addresses.
    #[must_use]
    pub fn output_ports(&self) -> Vec<PortAddress> {
        self.reply.output_ports()
    }

    /// The node's input port addresses.
    #[must_use]
    pub fn input_ports(&self) -> Vec<PortAddress> {
        self.reply.input_ports()
    }
}

/// What a discovery has done and been sent.
///
/// `malformed` and `dropped` are the two an operator would ever be shown, and
/// they are the two that say *somebody is putting rubbish on this network*.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiscoveryCounters {
    /// ArtPoll datagrams that have gone out.
    pub polls_sent: u64,
    /// Polls that could not be sent — a node whose route has gone.
    pub polls_failed: u64,
    /// ArtPollReply datagrams that were read.
    pub replies: u64,
    /// Datagrams that were not an ArtPollReply, dropped rather than believed.
    pub malformed: u64,
    /// Replies from a node the table had no room for.
    pub dropped: u64,
    /// Reads the socket refused. Counted and carried on from, because a UDP
    /// read error on Windows is routinely the *previous* send being refused.
    pub read_errors: u64,
}

/// Polls the configured nodes and remembers what answers.
///
/// The type parameters are the socket and the clock, for [`ArtNetOutput`]'s
/// reason exactly: [`crate::MockUdpNode`] makes the conversation assertable with
/// no network, and `prism_engine::ManualClock` makes *stopped answering three and
/// a half seconds ago* assertable without waiting three and a half seconds.
///
/// [`ArtNetOutput`]: crate::ArtNetOutput
pub struct NodeDiscovery<S: UdpNode, C: Clock = SystemClock> {
    socket: S,
    clock: C,
    config: DiscoveryConfig,
    open: bool,
    /// The addresses the rig sends Art-Net to, in rig order.
    targets: Vec<SocketAddr>,
    nodes: Vec<DiscoveredNode>,
    counters: DiscoveryCounters,
    /// When the last round of polls went out, or `None` before the first.
    polled_at: Option<Duration>,
    /// The receive buffer, allocated once.
    ///
    /// **An MTU, not the size of the packet this expects**, and that is a fix
    /// rather than a margin. An ArtPollReply is 239 bytes *at least*: §6's
    /// `Filler` is "transmit as zero, receivers do not test", so vendors pad,
    /// and a reply of 240 or 512 bytes is an ordinary reply. Reading it into 239
    /// bytes truncates on Unix and **fails** on Windows (`WSAEMSGSIZE`, and the
    /// data is discarded), so a padded node worked on Linux and read *Degraded*
    /// for ever on the release target — which is exactly what S46 shipped and a
    /// real node found. See [`crate::UdpError::Oversized`].
    buffer: Box<[u8; RECV_BUFFER_BYTES]>,
}

impl<S: UdpNode> NodeDiscovery<S, SystemClock> {
    /// A discovery on the real clock.
    #[must_use]
    pub fn new(socket: S, config: DiscoveryConfig) -> Self {
        Self::build(socket, config, SystemClock::new())
    }
}

impl<S: UdpNode, C: Clock> NodeDiscovery<S, C> {
    /// A discovery on a clock of its own, which is how the timeout is asserted
    /// against `prism_engine::ManualClock` rather than waited out.
    #[must_use]
    pub fn with_clock(socket: S, config: DiscoveryConfig, clock: C) -> Self {
        Self::build(socket, config, clock)
    }

    /// The one constructor — S4's rule: a generic constructor is compiled once
    /// per caller and llvm-cov counts every copy's untaken branches separately.
    fn build(socket: S, config: DiscoveryConfig, clock: C) -> Self {
        Self {
            socket,
            clock,
            config,
            open: false,
            targets: Vec::new(),
            nodes: Vec::new(),
            counters: DiscoveryCounters::default(),
            polled_at: None,
            buffer: Box::new([0; RECV_BUFFER_BYTES]),
        }
    }

    /// Binds the listening socket.
    ///
    /// # Errors
    ///
    /// [`UdpError::Bind`] when the address cannot be taken, which is the
    /// ordinary failure: Art-Net's port is a fixed number and another lighting
    /// program on the machine may already hold it. The caller's answer is to
    /// **say so and carry on** — the outputs go on sending and the panel says
    /// that nothing is listening, which is a smaller lie than a green light.
    pub fn open(&mut self) -> Result<(), UdpError> {
        if self.open {
            self.socket.close();
            self.open = false;
        }
        // No broadcast permission is asked for, because nothing here broadcasts.
        // A test asserts the flag is `false`, which is how "this desk does not
        // flood the hall" stays true rather than staying written down.
        self.socket.bind(self.config.bind, false)?;
        self.open = true;
        Ok(())
    }

    /// Whether the socket is bound.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Closes the socket.
    pub fn close(&mut self) {
        if self.open {
            self.socket.close();
            self.open = false;
        }
    }

    /// The address the socket is listening on, once it is open.
    #[must_use]
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.socket.local_addr()
    }

    /// How this discovery is configured.
    #[must_use]
    pub const fn config(&self) -> &DiscoveryConfig {
        &self.config
    }

    /// The clock, for a test that drives it.
    #[must_use]
    pub const fn clock(&self) -> &C {
        &self.clock
    }

    /// The addresses to poll: what the rig's Art-Net outputs send to.
    ///
    /// Setting them does **not** clear the table. A node dropped from the rig is
    /// still a node that is out there, and forgetting it because nobody is
    /// addressed to it any more would be exactly the discovery/configuration
    /// confusion this session exists to take apart.
    pub fn set_targets(&mut self, targets: impl IntoIterator<Item = SocketAddr>) {
        self.targets.clear();
        for target in targets {
            if !self.targets.contains(&target) {
                self.targets.push(target);
            }
        }
    }

    /// The addresses being polled.
    #[must_use]
    pub fn targets(&self) -> &[SocketAddr] {
        &self.targets
    }

    /// Everything that has answered, in the order it was first heard.
    #[must_use]
    pub fn nodes(&self) -> &[DiscoveredNode] {
        &self.nodes
    }

    /// What this discovery has done.
    #[must_use]
    pub const fn counters(&self) -> DiscoveryCounters {
        self.counters
    }

    /// The discovery's own view of the time.
    #[must_use]
    pub fn now(&self) -> Duration {
        self.clock.now()
    }

    /// One pass: send the polls that are due, then read what is there.
    ///
    /// Answers how many replies were read, which is what a thread logs on its
    /// first one. Does nothing at all on a socket that is not open, rather than
    /// reporting an error every hundred milliseconds for the life of a daemon
    /// whose port was taken.
    pub fn service(&mut self) -> usize {
        if !self.open {
            return 0;
        }
        self.poll_if_due();
        self.receive()
    }

    /// Sends one ArtPoll to every configured node, if the interval has passed.
    fn poll_if_due(&mut self) {
        let now = self.clock.now();
        let due = match self.polled_at {
            None => true,
            Some(last) => now.saturating_sub(last) >= self.config.poll_interval,
        };
        if !due {
            return;
        }
        self.polled_at = Some(now);
        let poll = art_poll(0, 0);
        for index in 0..self.targets.len() {
            let Some(&target) = self.targets.get(index) else {
                continue;
            };
            // One node refusing a poll does not stop the others being polled —
            // `ArtNetOutput::send_datagram`'s rule, and for the same reason: a
            // node whose route has gone must not take the rest of the rig's
            // diagnosis with it.
            match self.socket.send_to(&poll, target) {
                Ok(sent) if sent == poll.len() => self.counters.polls_sent += 1,
                Ok(_) | Err(_) => self.counters.polls_failed += 1,
            }
        }
    }

    /// Reads up to the budget's worth of datagrams.
    fn receive(&mut self) -> usize {
        let mut replies = 0;
        let wait = self.config.wait;
        for _ in 0..self.config.recv_budget {
            let outcome = {
                let Self { socket, buffer, .. } = self;
                socket.recv_from(buffer.as_mut_slice(), wait)
            };
            match outcome {
                // Nothing there, which is what a lighting network looks like
                // almost all of the time. The pass ends: waiting again would
                // multiply the wait by the budget.
                Ok(None) => break,
                Ok(Some((len, from))) => {
                    if self.accept(len, from) {
                        replies += 1;
                    }
                }
                // A datagram that did not fit. Dropped and counted as what it
                // is — rubbish this desk cannot read — and the pass **carries
                // on**, because the reply worth having may be the next one in
                // the queue and one oversized packet must not cost it.
                Err(UdpError::Oversized) => self.counters.malformed += 1,
                Err(_) => {
                    // Counted and carried on from. A read error here is
                    // routinely the operating system reporting a *previous*
                    // send to a host that refused it, and closing the socket
                    // for that would mean one absent node stopping the
                    // diagnosis of every present one.
                    self.counters.read_errors += 1;
                    break;
                }
            }
        }
        replies
    }

    /// Takes one datagram into the table, or counts why it was not.
    fn accept(&mut self, len: usize, from: SocketAddr) -> bool {
        let Some(datagram) = self.buffer.get(..len) else {
            self.counters.malformed += 1;
            return false;
        };
        let Some(reply) = parse_art_poll_reply(datagram) else {
            self.counters.malformed += 1;
            return false;
        };
        self.counters.replies += 1;
        let now = self.clock.now();
        // Matched on the **address the datagram came from**, not on the address
        // the node claims: the claim is a field a stranger filled in, and a
        // second node claiming the first one's IP must not be able to rewrite
        // its row. What the claim is good for is being shown when the two
        // differ, which is a node behind a router doing translation.
        if let Some(node) = self.nodes.iter_mut().find(|node| node.address == from) {
            node.reply = reply;
            node.last_seen = now;
            node.replies += 1;
            return true;
        }
        if self.nodes.len() >= self.config.max_nodes {
            self.counters.dropped += 1;
            return false;
        }
        self.nodes.push(DiscoveredNode {
            address: from,
            reply,
            first_seen: now,
            last_seen: now,
            replies: 1,
        });
        true
    }

    /// Whether the node at `address` is answering, and how long ago it last did.
    ///
    /// Matched on the **IP alone**: a node's reply is addressed to Art-Net's own
    /// port and arrives from it, but a node behind anything that rewrites ports
    /// answers from another one, and *the port differed* is not a reason to tell
    /// an installer their node is dead.
    #[must_use]
    pub fn reach(&self, address: SocketAddr) -> NodeReach {
        let now = self.clock.now();
        let seen = self
            .nodes
            .iter()
            .filter(|node| node.address.ip() == address.ip())
            .max_by_key(|node| node.last_seen);
        let Some(node) = seen else {
            return NodeReach {
                address: address.to_string(),
                health: NodeHealth::NeverAnswered,
                name: None,
                last_reply_ago_ms: None,
            };
        };
        let ago = now.saturating_sub(node.last_seen);
        let health = if ago <= self.config.reply_timeout {
            NodeHealth::Answering
        } else {
            NodeHealth::Stopped
        };
        NodeReach {
            address: address.to_string(),
            health,
            name: Some(node.reply.short_name().to_owned()),
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a reply 584 million years ago is not the number that is wrong"
            )]
            last_reply_ago_ms: Some(ago.as_millis() as u64),
        }
    }

    /// How long ago a node was last heard from, for a caller building its own
    /// row. `None` for one that has never answered.
    #[must_use]
    pub fn last_reply_ago(&self, address: SocketAddr) -> Option<Duration> {
        let now = self.clock.now();
        self.nodes
            .iter()
            .filter(|node| node.address.ip() == address.ip())
            .map(|node| now.saturating_sub(node.last_seen))
            .min()
    }
}

/// The most bytes a node's three name fields can hold, as one number.
///
/// Not used by the parser — it is here so a caller sizing a buffer for a whole
/// table has the figure to hand rather than a guess.
pub const NODE_TEXT_BYTES: usize = LONG_NAME_BYTES * 2 + MAX_NODE_PORTS;

#[cfg(test)]
mod tests {
    use super::{DiscoveryConfig, NodeDiscovery};
    use crate::artnet::{ART_NET_ID, ART_NET_PORT, PortAddress};
    use crate::artpoll::{
        ART_POLL_BYTES, ART_POLL_REPLY_BYTES, OP_POLL, OP_POLL_REPLY, parse_art_poll_reply,
    };
    use crate::udp::{MockUdpNode, MockUdpNodeHandle, UdpError};
    use prism_domain::NodeHealth;
    use prism_engine::ManualClock;
    use std::net::SocketAddr;
    use std::time::Duration;

    fn node_at(last: u8) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, last], ART_NET_PORT))
    }

    /// A reply as a node at `last` would send it, carrying `ports` output
    /// universes from sub-net 0.
    fn reply_from(last: u8, name: &str, ports: &[u8]) -> Vec<u8> {
        let mut packet = vec![0u8; ART_POLL_REPLY_BYTES];
        packet[..8].copy_from_slice(&ART_NET_ID);
        packet[8..10].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
        packet[10..14].copy_from_slice(&[127, 0, 0, last]);
        packet[14..16].copy_from_slice(&ART_NET_PORT.to_le_bytes());
        packet[26..26 + name.len()].copy_from_slice(name.as_bytes());
        packet[173] = u8::try_from(ports.len()).unwrap();
        for (index, port) in ports.iter().enumerate() {
            packet[174 + index] = 0x80;
            packet[182 + index] = 0x80;
            packet[190 + index] = *port;
        }
        packet[201..207].copy_from_slice(&[0, 0, 0, 0, 0, last]);
        packet
    }

    /// A discovery over a socket a test feeds, on a clock a test moves.
    ///
    /// The clock is reached through `discovery.clock()` rather than handed back
    /// beside it, so there is only ever one of them.
    fn discovery() -> (NodeDiscovery<MockUdpNode, ManualClock>, MockUdpNodeHandle) {
        let socket = MockUdpNode::new();
        let handle = socket.handle();
        let discovery = NodeDiscovery::with_clock(
            socket,
            DiscoveryConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                wait: Duration::from_millis(5),
                ..DiscoveryConfig::default()
            },
            ManualClock::new(),
        );
        (discovery, handle)
    }

    /// The exit criterion, first half: a mock node that answers is discovered
    /// and named.
    #[test]
    fn a_node_that_answers_a_poll_is_discovered_and_named() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        discovery.set_targets([node_at(5)]);

        discovery.service();
        let sent = socket.sent();
        assert_eq!(sent.len(), 1, "one poll to the one configured node");
        assert_eq!(sent[0].0, node_at(5));
        assert_eq!(sent[0].1.len(), ART_POLL_BYTES);
        assert_eq!(&sent[0].1[8..10], &OP_POLL.to_le_bytes());

        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0, 1]));
        assert_eq!(discovery.service(), 1);

        let nodes = discovery.nodes();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].address, node_at(5));
        assert_eq!(nodes[0].reply.short_name(), "Stage left");
        assert_eq!(nodes[0].replies, 1);
        assert_eq!(
            nodes[0].output_ports(),
            vec![
                PortAddress::from_parts(0, 0, 0).unwrap(),
                PortAddress::from_parts(0, 0, 1).unwrap()
            ]
        );
        assert!(
            nodes[0].input_ports().is_empty(),
            "a node sending DMX into the network is a different row, and this is not one"
        );
        assert_eq!(nodes[0].first_seen, nodes[0].last_seen);

        let reach = discovery.reach(node_at(5));
        assert_eq!(reach.health, NodeHealth::Answering);
        assert_eq!(reach.name.as_deref(), Some("Stage left"));
        assert_eq!(reach.last_reply_ago_ms, Some(0));
    }

    /// The entry, asserted: a configured node that never answers never reads
    /// well — punch-list **B6** at the level the fact is decided.
    #[test]
    fn a_configured_node_that_never_answers_never_reads_as_answering() {
        let (mut discovery, _socket) = discovery();
        discovery.open().unwrap();
        discovery.set_targets([node_at(9)]);
        for _ in 0..10 {
            discovery.service();
            discovery.clock().advance(Duration::from_secs(1));
        }
        let reach = discovery.reach(node_at(9));
        assert_eq!(reach.health, NodeHealth::NeverAnswered);
        assert!(!reach.health.is_answering());
        assert_eq!(reach.name, None);
        assert_eq!(reach.last_reply_ago_ms, None);
        assert!(discovery.nodes().is_empty());
        assert!(discovery.counters().polls_sent >= 3, "it did keep asking");
    }

    /// The exit criterion, second half: one that stops answering is reported as
    /// stopped, with the time, inside one poll interval.
    #[test]
    fn a_node_that_stops_answering_is_stopped_within_one_poll_interval() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        discovery.set_targets([node_at(5)]);
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
        discovery.service();
        assert_eq!(discovery.reach(node_at(5)).health, NodeHealth::Answering);

        let interval = discovery.config().poll_interval;
        // Still answering right up to the timeout, so a node answering every
        // interval never blinks.
        discovery.clock().advance(discovery.config().reply_timeout);
        discovery.service();
        assert_eq!(discovery.reach(node_at(5)).health, NodeHealth::Answering);

        discovery.clock().advance(Duration::from_millis(1));
        let reach = discovery.reach(node_at(5));
        assert_eq!(reach.health, NodeHealth::Stopped);
        assert_eq!(
            reach.last_reply_ago_ms,
            Some(u64::try_from(discovery.config().reply_timeout.as_millis()).unwrap() + 1),
            "the age is the daemon's, and it is the age of the last reply"
        );
        assert!(
            discovery.config().reply_timeout <= interval + Duration::from_secs(1),
            "the timeout has to be about one poll interval for this to be inside one"
        );
        assert_eq!(
            reach.name.as_deref(),
            Some("Stage left"),
            "a node that has stopped keeps the name it gave"
        );
    }

    #[test]
    fn a_node_that_comes_back_is_answering_again() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        discovery.set_targets([node_at(5)]);
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
        discovery.service();
        discovery.clock().advance(Duration::from_secs(60));
        assert_eq!(discovery.reach(node_at(5)).health, NodeHealth::Stopped);

        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
        discovery.service();
        assert_eq!(discovery.reach(node_at(5)).health, NodeHealth::Answering);
        assert_eq!(discovery.nodes().len(), 1, "one node, not two");
        assert_eq!(discovery.nodes()[0].replies, 2);
    }

    #[test]
    fn the_poll_goes_out_on_the_interval_and_not_faster() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        discovery.set_targets([node_at(5), node_at(6)]);
        discovery.service();
        assert_eq!(socket.sent_count(), 2, "one poll per node");
        discovery.service();
        discovery.service();
        assert_eq!(socket.sent_count(), 2, "and not one per pass");

        discovery.clock().advance(discovery.config().poll_interval);
        discovery.service();
        assert_eq!(socket.sent_count(), 4);
    }

    #[test]
    fn a_node_that_announces_itself_is_discovered_without_being_configured() {
        // The case the deliverable is about: plug a node in and it says so.
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        socket.deliver(node_at(11), &reply_from(11, "New node", &[3]));
        assert_eq!(discovery.service(), 1);
        assert_eq!(discovery.nodes().len(), 1);
        assert_eq!(discovery.nodes()[0].address, node_at(11));
        assert!(
            discovery.targets().is_empty(),
            "nothing was configured, and it was still heard"
        );
    }

    #[test]
    fn a_target_dropped_from_the_rig_is_still_a_node_that_is_out_there() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        discovery.set_targets([node_at(5)]);
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
        discovery.service();
        discovery.set_targets([]);
        assert_eq!(discovery.nodes().len(), 1);
        assert_eq!(discovery.reach(node_at(5)).health, NodeHealth::Answering);
    }

    #[test]
    fn a_target_named_twice_is_polled_once() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        // Two outputs to one node is an ordinary rig — a main and a spare
        // universe on the same box — and it is not a reason to poll it twice.
        discovery.set_targets([node_at(5), node_at(5)]);
        assert_eq!(discovery.targets().len(), 1);
        discovery.service();
        assert_eq!(socket.sent_count(), 1);
    }

    #[test]
    fn rubbish_on_the_port_is_dropped_and_counted() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        socket.deliver(node_at(5), b"not art-net at all");
        socket.deliver(node_at(5), &vec![0xffu8; ART_POLL_REPLY_BYTES]);
        // This desk's own ArtDmx, echoed back by a switch.
        let mut art_dmx = vec![0u8; 530];
        art_dmx[..8].copy_from_slice(&ART_NET_ID);
        art_dmx[8..10].copy_from_slice(&crate::OP_DMX.to_le_bytes());
        socket.deliver(node_at(5), &art_dmx);

        assert_eq!(discovery.service(), 0);
        assert!(discovery.nodes().is_empty());
        assert_eq!(discovery.counters().malformed, 3);
        assert_eq!(
            discovery.reach(node_at(5)).health,
            NodeHealth::NeverAnswered
        );
    }

    #[test]
    fn the_table_is_bounded_so_a_stranger_cannot_grow_it() {
        let socket = MockUdpNode::new();
        let handle = socket.handle();
        let mut discovery = NodeDiscovery::with_clock(
            socket,
            DiscoveryConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                max_nodes: 3,
                recv_budget: 32,
                ..DiscoveryConfig::default()
            },
            ManualClock::new(),
        );
        discovery.open().unwrap();
        for last in 1..=10u8 {
            handle.deliver(node_at(last), &reply_from(last, "Node", &[0]));
        }
        discovery.service();
        assert_eq!(discovery.nodes().len(), 3);
        assert_eq!(discovery.counters().dropped, 7);
        assert_eq!(
            discovery.counters().replies,
            10,
            "every well-formed reply is counted as read; seven of them found no room"
        );
    }

    #[test]
    fn one_pass_reads_no_more_than_its_budget() {
        let socket = MockUdpNode::new();
        let handle = socket.handle();
        let mut discovery = NodeDiscovery::with_clock(
            socket,
            DiscoveryConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                recv_budget: 2,
                ..DiscoveryConfig::default()
            },
            ManualClock::new(),
        );
        discovery.open().unwrap();
        for last in 1..=5u8 {
            handle.deliver(node_at(last), &reply_from(last, "Node", &[0]));
        }
        assert_eq!(discovery.service(), 2);
        assert_eq!(discovery.nodes().len(), 2);
    }

    /// S33's rule for every driver, one thread along.
    #[test]
    fn a_socket_that_will_not_bind_degrades_alone() {
        let socket = MockUdpNode::new();
        let handle = socket.handle();
        handle.fail_bind(1, UdpError::Bind);
        let mut discovery =
            NodeDiscovery::with_clock(socket, DiscoveryConfig::default(), ManualClock::new());
        assert_eq!(discovery.open(), Err(UdpError::Bind));
        assert!(!discovery.is_open());
        discovery.set_targets([node_at(5)]);
        // Every pass afterwards is a no-op rather than an error a second.
        assert_eq!(discovery.service(), 0);
        assert_eq!(discovery.counters(), Default::default());
        assert_eq!(
            discovery.reach(node_at(5)).health,
            NodeHealth::NeverAnswered
        );

        // And it can be opened later, which is what a retry is.
        discovery.open().unwrap();
        assert!(discovery.is_open());
        discovery.service();
        assert_eq!(discovery.counters().polls_sent, 1);
    }

    #[test]
    fn nothing_here_asks_for_broadcast_permission() {
        // The rule S9 and S10 wrote down, asserted at the socket rather than
        // stated in a comment: a poll goes where this desk already sends.
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        assert_eq!(
            socket.binds(),
            vec![("127.0.0.1:0".parse().unwrap(), false)]
        );
    }

    #[test]
    fn a_node_that_refuses_a_poll_does_not_stop_the_others_being_polled() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        discovery.set_targets([node_at(5), node_at(6)]);
        socket.fail_send(1, UdpError::Unreachable);
        discovery.service();
        assert_eq!(socket.sent_count(), 1, "the second node still got its poll");
        assert_eq!(discovery.counters().polls_failed, 1);
        assert_eq!(discovery.counters().polls_sent, 1);
    }

    #[test]
    fn a_read_error_is_counted_and_the_thread_carries_on() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        socket.fail_recv(1, UdpError::Io);
        assert_eq!(discovery.service(), 0);
        assert_eq!(discovery.counters().read_errors, 1);
        assert!(
            discovery.is_open(),
            "a refused read does not close the socket"
        );
        socket.deliver(node_at(5), &reply_from(5, "Back", &[0]));
        assert_eq!(discovery.service(), 1);
    }

    #[test]
    fn a_reply_claiming_another_nodes_address_does_not_rewrite_its_row() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
        // A second sender claiming to be 127.0.0.5 in the packet body.
        socket.deliver(node_at(6), &reply_from(5, "Impostor", &[0]));
        discovery.service();
        assert_eq!(discovery.nodes().len(), 2, "two senders are two rows");
        assert_eq!(discovery.nodes()[0].reply.short_name(), "Stage left");
        assert_eq!(discovery.nodes()[1].address, node_at(6));
    }

    #[test]
    fn closing_the_socket_stops_the_conversation_and_keeps_the_table() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
        discovery.service();
        discovery.close();
        assert!(!discovery.is_open());
        assert_eq!(socket.closes(), 1);
        assert_eq!(discovery.service(), 0);
        assert_eq!(discovery.nodes().len(), 1);
        assert_eq!(discovery.local_addr(), None);
        discovery.close();
        assert_eq!(socket.closes(), 1, "closing twice closes once");
    }

    #[test]
    fn reopening_replaces_the_socket_rather_than_leaking_it() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        discovery.open().unwrap();
        assert_eq!(socket.closes(), 1);
        assert_eq!(socket.binds().len(), 2);
        assert!(discovery.local_addr().is_some());
    }

    #[test]
    fn the_last_reply_age_is_the_number_the_daemon_carries() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        assert_eq!(discovery.last_reply_ago(node_at(5)), None);
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
        discovery.service();
        discovery.clock().advance(Duration::from_millis(1_250));
        assert_eq!(
            discovery.last_reply_ago(node_at(5)),
            Some(Duration::from_millis(1_250))
        );
        assert_eq!(discovery.now(), Duration::from_millis(1_250));
    }

    #[test]
    fn a_reply_is_matched_on_the_address_it_came_from_whatever_port_it_used() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        // A node behind something that rewrites ports.
        socket.deliver(
            SocketAddr::from(([127, 0, 0, 5], 40_000)),
            &reply_from(5, "Stage left", &[0]),
        );
        discovery.service();
        assert_eq!(discovery.reach(node_at(5)).health, NodeHealth::Answering);
    }

    /// **The fault a real node found**, as a test.
    ///
    /// §6's `Filler` is *transmit as zero, receivers do not test*, so a node may
    /// pad its ArtPollReply past 239 bytes and many do. Reading that into a
    /// 239-byte buffer truncates on Unix and fails on Windows, so before S46's
    /// fix a padded node answered every poll and read *Degraded* for ever on the
    /// release target. The buffer is an MTU now, and a padded reply is a reply.
    #[test]
    fn a_node_that_pads_its_reply_is_still_a_node_that_answered() {
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        let mut padded = reply_from(5, "Stage left", &[0]);
        padded.extend(std::iter::repeat_n(0u8, 273));
        assert_eq!(padded.len(), 512, "a size real nodes actually send");
        socket.deliver(node_at(5), &padded);

        assert_eq!(discovery.service(), 1);
        assert_eq!(discovery.nodes().len(), 1);
        assert_eq!(discovery.nodes()[0].reply.short_name(), "Stage left");
        assert_eq!(
            discovery.reach(node_at(5)).health,
            NodeHealth::Answering,
            "which is the whole of the bug: it answered, and the desk said Degraded"
        );
        assert!(parse_art_poll_reply(&padded).is_some());
    }

    #[test]
    fn a_datagram_too_big_even_for_the_mtu_is_dropped_without_ending_the_pass() {
        // A stranger's giant packet must not cost the reply behind it. Before
        // the fix an unreadable datagram ended the receive pass.
        let (mut discovery, socket) = discovery();
        discovery.open().unwrap();
        socket.deliver(node_at(9), &vec![0u8; super::RECV_BUFFER_BYTES + 1]);
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));

        assert_eq!(discovery.service(), 1, "the reply behind it was still read");
        assert_eq!(discovery.counters().malformed, 1);
        assert_eq!(discovery.counters().read_errors, 0);
        assert_eq!(discovery.reach(node_at(5)).health, NodeHealth::Answering);
    }
}
