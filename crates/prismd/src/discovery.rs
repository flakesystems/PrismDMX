//! The Art-Net discovery thread, and the table it keeps — S46.
//!
//! `prism_protocols::NodeDiscovery` is the conversation; this is where it runs
//! and where its answers are kept for a client to ask about.
//!
//! # Why a thread and a table, rather than an enumeration on the asking thread
//!
//! `Query::MidiPorts` (S36) enumerates on the thread that asked, and that is
//! right there because *what is plugged into this machine* is one system call
//! away. There is no call that answers *what is on this network*: discovery is a
//! conversation over time — poll, wait, hear — so somebody has to be listening
//! before the question is asked. A query that started a conversation and waited
//! for it would hold the IPC thread for a poll interval and would still answer
//! *nothing* the first time, which is the one answer that must never be wrong.
//!
//! So the daemon listens continuously and the question **reads** what has been
//! heard. That also keeps §5.2's first rule intact: the query changes nothing and
//! sends nothing.
//!
//! # When it runs, and when it does not
//!
//! Only when the rig has an Art-Net output. A desk with a DMX cable and nothing
//! else has no business holding UDP port 6454, and a daemon started with
//! `--mock-devices` must touch no socket at all — which is what
//! [`Discovery::idle`] is, and what every test in this workspace that starts a
//! daemon gets.
//!
//! # A socket that will not bind degrades alone
//!
//! S33's rule for every driver. Art-Net's port is a fixed number, so *another
//! program on this machine already has it* is an ordinary outcome rather than a
//! fault: the outputs go on sending, the daemon goes on running, and the table
//! says `listening: false` with the reason in words. A panel that drew an empty
//! list without reading that flag would be making B6's mistake in the other
//! direction, which is why `Answer::ArtNetNodes` carries the flag at all.

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use prism_domain::{NodeHealth, NodeReach};
use prism_protocols::{DiscoveryConfig, DiscoveryCounters, NodeDiscovery, SystemUdpNode, UdpNode};

use crate::log;

/// One node the daemon has heard from, in the vocabulary a client is answered in.
///
/// The `Vec`s here are built when the thread publishes rather than per datagram —
/// see `prism_protocols::DiscoveredNode`, which holds the reply as the fixed
/// arrays it arrived in precisely so that this conversion happens once per
/// change rather than once per reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRow {
    /// Where the reply came from.
    pub address: SocketAddr,
    /// The address the node says it has.
    pub ip: IpAddr,
    /// `ShortName`.
    pub short_name: String,
    /// `LongName`.
    pub long_name: String,
    /// `MAC`, as six hexadecimal pairs.
    pub mac: String,
    /// `VersInfoH`/`VersInfoL`.
    pub firmware: u16,
    /// `Style`.
    pub style: u8,
    /// `Status1`.
    pub status1: u8,
    /// `Status2`.
    pub status2: u8,
    /// The node's output port addresses.
    pub ports: Vec<u16>,
    /// The node's input port addresses.
    pub inputs: Vec<u16>,
    /// How many replies have arrived.
    pub replies: u64,
    /// When the last one did, on **this machine's** clock.
    ///
    /// An `Instant` rather than an age, because an age would be as old as the
    /// publish that wrote it. Every age a client is told is measured against
    /// `Instant::now()` at the moment the question is answered, which is S33's
    /// rule for `OutputStatusInfo::last_error_ago_ms` one field along.
    pub last_seen: Instant,
}

/// What the discovery thread has heard, as of its last publish.
#[derive(Debug, Clone)]
pub struct DiscoveryTable {
    /// Whether the receive socket is bound.
    pub listening: bool,
    /// Why it is not, in the daemon's own words.
    pub error: Option<String>,
    /// How long a node may be silent before it reads as *stopped*.
    pub reply_timeout: Duration,
    /// One row per node heard from, in the order they were first heard.
    pub nodes: Vec<NodeRow>,
    /// What the thread has done.
    pub counters: DiscoveryCounters,
}

impl Default for DiscoveryTable {
    /// The table of a daemon that is not listening: nothing heard, and **no
    /// error**, because *nothing is configured* and *the port was taken* are
    /// different facts and only the second one has a reason.
    fn default() -> Self {
        Self {
            listening: false,
            error: None,
            reply_timeout: DiscoveryConfig::default().reply_timeout,
            nodes: Vec::new(),
            counters: DiscoveryCounters::default(),
        }
    }
}

impl DiscoveryTable {
    /// Whether the node at `address` answers, as of `now`.
    ///
    /// Matched on the **IP alone**, for `NodeDiscovery::reach`'s reason: a node
    /// behind anything that rewrites ports answers from another one, and *the
    /// port differed* is not a reason to tell an installer their node is dead.
    ///
    /// `now` is passed in rather than taken here so that one answer's rows are
    /// all measured against one moment.
    #[must_use]
    pub fn reach(&self, address: SocketAddr, now: Instant) -> NodeReach {
        let Some(node) = self.node_at(address) else {
            return NodeReach {
                address: address.to_string(),
                health: NodeHealth::NeverAnswered,
                name: None,
                last_reply_ago_ms: None,
            };
        };
        let ago = now.saturating_duration_since(node.last_seen);
        NodeReach {
            address: address.to_string(),
            health: if ago <= self.reply_timeout {
                NodeHealth::Answering
            } else {
                NodeHealth::Stopped
            },
            name: Some(node.short_name.clone()),
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a reply 584 million years ago is not the number that is wrong"
            )]
            last_reply_ago_ms: Some(ago.as_millis() as u64),
        }
    }

    /// The row for the node at `address`, matched on the IP.
    #[must_use]
    pub fn node_at(&self, address: SocketAddr) -> Option<&NodeRow> {
        self.nodes
            .iter()
            .filter(|node| node.address.ip() == address.ip())
            .max_by_key(|node| node.last_seen)
    }
}

/// Where the discovery thread gets its socket.
///
/// `OutputFactory`'s seam, one thread along and for the same reason: `CLAUDE.md`
/// forbids a test from touching a device or a network, and this is the only place
/// in the daemon that opens a listening socket. [`SystemSockets`] is the real
/// one; a test hands one that answers with a `prism_protocols::MockUdpNode`.
pub trait SocketSource: Send + Sync + 'static {
    /// A socket to listen on. Called once per thread start, and again on a
    /// restart, so it must be able to answer more than once.
    fn open(&self) -> Box<dyn UdpNode>;
}

/// The real listening socket.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemSockets;

impl SocketSource for SystemSockets {
    fn open(&self) -> Box<dyn UdpNode> {
        Box::new(SystemUdpNode::new())
    }
}

/// State the thread writes and the daemon reads.
#[derive(Debug)]
struct Shared {
    table: Mutex<DiscoveryTable>,
    targets: Mutex<Vec<SocketAddr>>,
    running: AtomicBool,
}

/// Takes a lock without caring whether a previous holder panicked — the rule the
/// rest of this workspace's shared state follows.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The daemon's handle on Art-Net discovery.
///
/// An **idle** one is the whole of what a daemon with no Art-Net output has: no
/// thread, no socket, and a table that says nothing is listening. That is the
/// state every test in this workspace that starts a daemon is in, which is what
/// keeps `CLAUDE.md`'s *no test touches a network* true without a flag.
pub struct Discovery {
    shared: Arc<Shared>,
    source: Option<Arc<dyn SocketSource>>,
    config: DiscoveryConfig,
    thread: Option<JoinHandle<()>>,
}

impl core::fmt::Debug for Discovery {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Discovery")
            .field("running", &self.thread.is_some())
            .field("enabled", &self.source.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for Discovery {
    fn default() -> Self {
        Self::idle()
    }
}

impl Discovery {
    /// A discovery that will never open a socket.
    #[must_use]
    pub fn idle() -> Self {
        Self {
            shared: Arc::new(Shared {
                table: Mutex::new(DiscoveryTable::default()),
                targets: Mutex::new(Vec::new()),
                running: AtomicBool::new(false),
            }),
            source: None,
            config: DiscoveryConfig::default(),
            thread: None,
        }
    }

    /// A discovery that will never open a socket, and **says why**.
    ///
    /// The difference from [`idle`](Self::idle) is the whole point: *nothing is
    /// configured* and *this run was told not to listen* are different facts,
    /// and only the second one has a reason. A panel drawing an empty node list
    /// under the first would be right to say *once an Art-Net output exists*;
    /// under the second it would be telling an operator to do something that
    /// would not help.
    #[must_use]
    pub fn disabled(reason: impl Into<String>) -> Self {
        let discovery = Self::idle();
        lock(&discovery.shared.table).error = Some(reason.into());
        discovery
    }

    /// A discovery that will open a socket from `source` once the rig has an
    /// Art-Net output.
    #[must_use]
    pub fn with_source(source: Arc<dyn SocketSource>, config: DiscoveryConfig) -> Self {
        Self {
            shared: Arc::new(Shared {
                table: Mutex::new(DiscoveryTable::default()),
                targets: Mutex::new(Vec::new()),
                running: AtomicBool::new(false),
            }),
            source: Some(source),
            config,
            thread: None,
        }
    }

    /// The real one: a system socket on Art-Net's own port.
    #[must_use]
    pub fn system() -> Self {
        Self::with_source(Arc::new(SystemSockets), DiscoveryConfig::default())
    }

    /// Whether a thread is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.thread.is_some()
    }

    /// What has been heard.
    #[must_use]
    pub fn table(&self) -> DiscoveryTable {
        lock(&self.shared.table).clone()
    }

    /// The addresses being polled.
    #[must_use]
    pub fn targets(&self) -> Vec<SocketAddr> {
        lock(&self.shared.targets).clone()
    }

    /// Points the discovery at the addresses the rig's Art-Net outputs send to,
    /// starting or stopping the thread as that list becomes non-empty or empty.
    ///
    /// **A rig with no Art-Net row stops the thread and gives the port back.**
    /// The table is left standing rather than cleared, because an installer who
    /// has just removed the wrong row should see the node they were looking at
    /// rather than an empty panel — and `listening: false` beside it is what
    /// says the list is a memory rather than a reading.
    ///
    /// A socket that opens **again** starts a new table, and that is not the
    /// same decision reversed: a node heard through a socket that has since been
    /// closed is one this desk can say nothing *current* about, and an age that
    /// went on climbing across the gap would be a number nobody could act on.
    /// So the memory survives exactly as long as nothing has replaced it.
    pub fn retarget(&mut self, targets: Vec<SocketAddr>) {
        let wanted = !targets.is_empty();
        *lock(&self.shared.targets) = targets;
        if wanted {
            self.start();
        } else {
            self.stop();
        }
    }

    /// Starts the thread, if there is a source and it is not already running.
    fn start(&mut self) {
        let Some(source) = self.source.clone() else {
            return;
        };
        if self.thread.is_some() {
            return;
        }
        self.shared.running.store(true, Ordering::Release);
        let shared = Arc::clone(&self.shared);
        let config = self.config.clone();
        match std::thread::Builder::new()
            .name("artnet-discovery".to_owned())
            .spawn(move || run(&source, &config, &shared))
        {
            Ok(thread) => self.thread = Some(thread),
            Err(error) => {
                // The same shape as an output whose thread will not start: said
                // out loud, and the daemon goes on without it.
                self.shared.running.store(false, Ordering::Release);
                log::error(
                    "artnet",
                    &format!("node discovery could not be started: {error}"),
                );
                let mut table = lock(&self.shared.table);
                table.listening = false;
                table.error = Some(format!(
                    "the discovery thread could not be started: {error}"
                ));
            }
        }
    }

    /// Stops the thread and gives the socket back. Safe on one that is not
    /// running, which is what makes it usable on both the retarget and the
    /// shutdown path.
    pub fn stop(&mut self) {
        self.shared.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            // Joined rather than detached: the socket has to be closed before a
            // later `retarget` tries to bind the same port again, and a daemon
            // that left a thread holding 6454 behind would be the fault this
            // module reports rather than causes.
            let _ = thread.join();
            let mut table = lock(&self.shared.table);
            table.listening = false;
            log::info("artnet", "node discovery stopped");
        }
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The thread body: open, then poll and listen until told to stop.
fn run(source: &Arc<dyn SocketSource>, config: &DiscoveryConfig, shared: &Arc<Shared>) {
    let mut discovery = NodeDiscovery::new(source.open(), config.clone());
    if let Err(error) = discovery.open() {
        // **Degrades alone.** The outputs go on sending and the daemon goes on
        // running; what is lost is the diagnosis, and it is lost out loud.
        log::warn(
            "artnet",
            &format!(
                "node discovery is not listening on {}: {error}. Art-Net outputs will send, \
                 and this desk cannot tell whether anything receives them",
                config.bind
            ),
        );
        let mut table = lock(&shared.table);
        table.listening = false;
        table.error = Some(error.to_string());
        shared.running.store(false, Ordering::Release);
        return;
    }
    log::info(
        "artnet",
        &format!(
            "node discovery listening on {}",
            discovery
                .local_addr()
                .map_or_else(|| config.bind.to_string(), |address| address.to_string())
        ),
    );
    publish(&discovery, shared, true);

    let mut known = 0usize;
    while shared.running.load(Ordering::Acquire) {
        let targets = lock(&shared.targets).clone();
        discovery.set_targets(targets);
        // `service` waits at most `config.wait` inside the socket read, so this
        // loop notices a stop within that — no sleep, and no spin.
        discovery.service();
        if discovery.nodes().len() != known {
            known = discovery.nodes().len();
            if let Some(node) = discovery.nodes().last() {
                log::info(
                    "artnet",
                    &format!(
                        "node discovered: \"{}\" at {}",
                        node.reply.short_name(),
                        node.address
                    ),
                );
            }
        }
        publish(&discovery, shared, true);
    }
    discovery.close();
    publish(&discovery, shared, false);
}

/// Copies what the discovery holds into the table a client is answered from.
fn publish<S: UdpNode>(discovery: &NodeDiscovery<S>, shared: &Arc<Shared>, listening: bool) {
    // The discovery's clock is monotonic from its own start; the table carries
    // `Instant`s so that every age a client is told is measured at the moment the
    // question is answered. The two are joined here, once per publish.
    let now = Instant::now();
    let reference = discovery.now();
    let nodes = discovery
        .nodes()
        .iter()
        .map(|node| NodeRow {
            address: node.address,
            ip: IpAddr::V4(node.reply.ip),
            short_name: node.reply.short_name().to_owned(),
            long_name: node.reply.long_name().to_owned(),
            mac: node.reply.mac_address(),
            firmware: node.reply.firmware,
            style: node.reply.style,
            status1: node.reply.status1,
            status2: node.reply.status2,
            ports: node
                .output_ports()
                .into_iter()
                .map(prism_protocols::PortAddress::get)
                .collect(),
            inputs: node
                .input_ports()
                .into_iter()
                .map(prism_protocols::PortAddress::get)
                .collect(),
            replies: node.replies,
            last_seen: now
                .checked_sub(reference.saturating_sub(node.last_seen))
                .unwrap_or(now),
        })
        .collect();
    let mut table = lock(&shared.table);
    table.listening = listening;
    table.reply_timeout = discovery.config().reply_timeout;
    table.nodes = nodes;
    table.counters = discovery.counters();
    if listening {
        table.error = None;
    }
}

/// A network this crate's tests can put a node on.
///
/// Lives beside the code rather than inside one `mod tests` because two modules
/// need it: this one, and `crate::outputs`, where the **health fold** of
/// punch-list B6 is decided and therefore has to be asserted against a discovery
/// that is genuinely listening. A second copy of a mock node would be a second
/// idea of what a reply looks like.
#[cfg(test)]
pub(crate) mod testing {
    use super::{Discovery, DiscoveryTable, SocketSource};
    use prism_protocols::{
        ART_NET_ID, ART_POLL_REPLY_BYTES, DiscoveryConfig, MockUdpNode, MockUdpNodeHandle,
        OP_POLL_REPLY, UdpNode,
    };
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// A node's address on loopback, on Art-Net's own port.
    pub(crate) fn node_at(last: u8) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, last], 6454))
    }

    /// A reply as a node at `last` would send it, carrying `ports` output
    /// universes from sub-net 0.
    pub(crate) fn reply_from(last: u8, name: &str, ports: &[u8]) -> Vec<u8> {
        let mut packet = vec![0u8; ART_POLL_REPLY_BYTES];
        packet[..8].copy_from_slice(&ART_NET_ID);
        packet[8..10].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
        packet[10..14].copy_from_slice(&[127, 0, 0, last]);
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

    /// A source handing out one mock socket, so a test can feed the thread.
    pub(crate) struct MockSockets(Mutex<Option<MockUdpNode>>);

    impl MockSockets {
        /// A source and the handle a test delivers replies through.
        pub(crate) fn new() -> (Arc<Self>, MockUdpNodeHandle) {
            let socket = MockUdpNode::new();
            let handle = socket.handle();
            (Arc::new(Self(Mutex::new(Some(socket)))), handle)
        }
    }

    impl SocketSource for MockSockets {
        fn open(&self) -> Box<dyn UdpNode> {
            self.0
                .lock()
                .ok()
                .and_then(|mut slot| slot.take())
                .map_or_else(
                    || Box::new(MockUdpNode::new()) as Box<dyn UdpNode>,
                    |socket| Box::new(socket) as Box<dyn UdpNode>,
                )
        }
    }

    /// A discovery configuration that binds loopback on a port the operating
    /// system chooses — never Art-Net's own, and never `0.0.0.0`.
    pub(crate) fn config() -> DiscoveryConfig {
        DiscoveryConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            wait: Duration::from_millis(1),
            ..DiscoveryConfig::default()
        }
    }

    /// Waits for `check` to hold, or gives up. A deadline, not a measurement.
    pub(crate) fn until(
        discovery: &Discovery,
        check: impl Fn(&DiscoveryTable) -> bool,
    ) -> DiscoveryTable {
        let started = Instant::now();
        loop {
            let table = discovery.table();
            if check(&table) {
                return table;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "the discovery thread never reached the expected state: {table:?}"
            );
            std::thread::yield_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{MockSockets, config, node_at, reply_from, until};
    use super::{Discovery, DiscoveryTable, NodeRow, SocketSource, SystemSockets};
    use prism_domain::NodeHealth;
    use prism_protocols::{MockUdpNode, UdpNode};
    use std::net::{IpAddr, SocketAddr};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    fn an_idle_discovery_opens_no_socket_whatever_the_rig_says() {
        // What every daemon in this workspace's tests holds, and the reason none
        // of them touches a network.
        let mut discovery = Discovery::idle();
        discovery.retarget(vec![node_at(5)]);
        assert!(!discovery.is_running());
        let table = discovery.table();
        assert!(!table.listening);
        assert!(table.nodes.is_empty());
        assert_eq!(table.error, None, "nothing configured is not an error");
        assert_eq!(discovery.targets(), vec![node_at(5)]);
    }

    #[test]
    fn a_rig_with_an_art_net_output_starts_the_thread_and_one_without_stops_it() {
        let (source, socket) = MockSockets::new();
        let mut discovery = Discovery::with_source(source, config());
        assert!(!discovery.is_running(), "nothing is configured yet");

        discovery.retarget(vec![node_at(5)]);
        assert!(discovery.is_running());
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0, 1]));
        let table = until(&discovery, |table| !table.nodes.is_empty());
        assert!(table.listening);

        discovery.retarget(Vec::new());
        assert!(!discovery.is_running(), "the port is given back");
        let table = discovery.table();
        assert!(!table.listening);
        assert_eq!(
            table.nodes.len(),
            1,
            "what was heard stands after the row was removed, and `listening: \
             false` beside it is what says it is a memory"
        );
    }

    #[test]
    fn a_discovered_node_is_carried_whole_into_the_table() {
        let (source, socket) = MockSockets::new();
        let mut discovery = Discovery::with_source(source, config());
        discovery.retarget(vec![node_at(7)]);
        socket.deliver(node_at(7), &reply_from(7, "Dock two", &[3, 4]));
        let table = until(&discovery, |table| !table.nodes.is_empty());

        let node = &table.nodes[0];
        assert_eq!(node.address, node_at(7));
        assert_eq!(node.ip, IpAddr::from([127, 0, 0, 7]));
        assert_eq!(node.short_name, "Dock two");
        assert_eq!(node.mac, "00:00:00:00:00:07");
        assert_eq!(node.ports, vec![3, 4]);
        assert!(node.inputs.is_empty());
        assert_eq!(node.replies, 1);

        let reach = table.reach(node_at(7), Instant::now());
        assert_eq!(reach.health, NodeHealth::Answering);
        assert_eq!(reach.name.as_deref(), Some("Dock two"));
    }

    /// The entry, at the layer the daemon answers from — punch-list **B6**.
    #[test]
    fn a_configured_node_that_never_answers_reads_as_never_answered() {
        let (source, _socket) = MockSockets::new();
        let mut discovery = Discovery::with_source(source, config());
        discovery.retarget(vec![node_at(9)]);
        let table = until(&discovery, |table| table.counters.polls_sent > 0);
        let reach = table.reach(node_at(9), Instant::now());
        assert_eq!(reach.health, NodeHealth::NeverAnswered);
        assert!(!reach.health.is_answering());
        assert_eq!(reach.last_reply_ago_ms, None);
        assert_eq!(reach.address, node_at(9).to_string());
    }

    #[test]
    fn a_node_the_daemon_has_not_heard_from_lately_reads_as_stopped_with_the_age() {
        // The age is the **daemon's**, measured at the moment the question is
        // answered — which is what this asserts by asking about a moment that
        // has not arrived yet on any clock the thread holds.
        let table = DiscoveryTable {
            listening: true,
            error: None,
            reply_timeout: Duration::from_millis(3_500),
            nodes: vec![NodeRow {
                address: node_at(5),
                ip: IpAddr::from([127, 0, 0, 5]),
                short_name: "Stage left".to_owned(),
                long_name: String::new(),
                mac: "00:00:00:00:00:05".to_owned(),
                firmware: 0x0104,
                style: 0,
                status1: 0,
                status2: 0,
                ports: vec![0],
                inputs: Vec::new(),
                replies: 3,
                last_seen: Instant::now(),
            }],
            counters: prism_protocols::DiscoveryCounters::default(),
        };
        let now = Instant::now();
        assert_eq!(table.reach(node_at(5), now).health, NodeHealth::Answering);

        let later = now + Duration::from_secs(20);
        let reach = table.reach(node_at(5), later);
        assert_eq!(reach.health, NodeHealth::Stopped);
        assert!(
            reach.last_reply_ago_ms.unwrap_or(0) >= 20_000,
            "the age is how long ago the reply was, not how old the publish is"
        );
        assert_eq!(
            reach.name.as_deref(),
            Some("Stage left"),
            "a node that has stopped keeps the name it gave"
        );
    }

    #[test]
    fn a_reply_is_matched_on_the_address_whatever_port_it_used() {
        let (source, socket) = MockSockets::new();
        let mut discovery = Discovery::with_source(source, config());
        discovery.retarget(vec![node_at(5)]);
        socket.deliver(
            SocketAddr::from(([127, 0, 0, 5], 40_000)),
            &reply_from(5, "Stage left", &[0]),
        );
        let table = until(&discovery, |table| !table.nodes.is_empty());
        assert_eq!(
            table.reach(node_at(5), Instant::now()).health,
            NodeHealth::Answering
        );
        assert!(table.node_at(node_at(5)).is_some());
        assert!(table.node_at(node_at(6)).is_none());
    }

    #[test]
    fn a_socket_that_will_not_bind_leaves_a_reason_and_no_thread_that_pretends() {
        struct Refuses;
        impl SocketSource for Refuses {
            fn open(&self) -> Box<dyn UdpNode> {
                let socket = MockUdpNode::new();
                socket
                    .handle()
                    .fail_bind(64, prism_protocols::UdpError::Bind);
                Box::new(socket)
            }
        }
        let mut discovery = Discovery::with_source(Arc::new(Refuses), config());
        discovery.retarget(vec![node_at(5)]);
        let table = until(&discovery, |table| table.error.is_some());
        assert!(!table.listening);
        assert_eq!(
            table.error.as_deref(),
            Some("the local address could not be bound")
        );
        assert!(table.nodes.is_empty());
        // And nothing else has stopped: the handle is still usable and stopping
        // it is not an error.
        discovery.stop();
        assert!(!discovery.is_running());
    }

    /// *Nothing configured* and *told not to listen* are different facts — S46.
    #[test]
    fn a_discovery_that_was_turned_off_says_so_and_one_with_no_rig_does_not() {
        let idle = Discovery::idle();
        assert_eq!(
            idle.table().error,
            None,
            "nothing configured is not a reason, it is an absence"
        );

        let off = Discovery::disabled("node discovery is off for this run (--mock-devices)");
        assert!(!off.table().listening);
        assert_eq!(
            off.table().error.as_deref(),
            Some("node discovery is off for this run (--mock-devices)")
        );
        assert!(off.table().nodes.is_empty());
    }

    /// A rig that grows a second Art-Net row does not grow a second thread, and
    /// a row that comes back after the last one went opens a **new** socket.
    #[test]
    fn retargeting_starts_one_thread_and_a_row_that_comes_back_opens_again() {
        let (source, socket) = MockSockets::new();
        let mut discovery = Discovery::with_source(source, config());
        discovery.retarget(vec![node_at(5)]);
        assert!(discovery.is_running());
        // A second Art-Net row is a second target and the same thread.
        discovery.retarget(vec![node_at(5), node_at(6)]);
        assert!(discovery.is_running());
        assert_eq!(discovery.targets(), vec![node_at(5), node_at(6)]);
        socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
        until(&discovery, |table| !table.nodes.is_empty());

        // The last row goes, the port goes back, and the row comes back: the
        // source is asked for a socket a second time, which is why it has to be
        // able to answer more than once.
        discovery.retarget(Vec::new());
        assert!(!discovery.is_running());
        discovery.retarget(vec![node_at(7)]);
        assert!(discovery.is_running());
        until(&discovery, |table| table.listening);
        assert!(
            discovery.table().nodes.is_empty(),
            "a new socket starts a new table: a node heard through one that has \
             since been closed is not something this desk can say anything \
             current about"
        );
    }

    /// A handle says what it is without saying what it holds — the shape every
    /// `Debug` in this workspace has where a lock is involved.
    #[test]
    fn a_discovery_describes_itself_without_taking_its_own_lock() {
        let idle = format!("{:?}", Discovery::idle());
        assert!(idle.contains("running: false"), "{idle}");
        assert!(idle.contains("enabled: false"), "{idle}");

        let (source, _socket) = MockSockets::new();
        let mut discovery = Discovery::with_source(source, config());
        assert!(format!("{discovery:?}").contains("enabled: true"));
        discovery.retarget(vec![node_at(5)]);
        assert!(format!("{discovery:?}").contains("running: true"));
    }

    #[test]
    fn stopping_a_discovery_that_never_started_is_not_an_error() {
        let mut discovery = Discovery::default();
        discovery.stop();
        discovery.stop();
        assert!(!discovery.is_running());
        assert!(!discovery.table().listening);
    }

    #[test]
    fn the_real_source_answers_with_a_socket() {
        // The one line of `SystemSockets` there is, exercised without binding
        // anything: a socket that has not been opened has no address.
        let socket = SystemSockets.open();
        assert_eq!(socket.local_addr(), None);
    }
}
