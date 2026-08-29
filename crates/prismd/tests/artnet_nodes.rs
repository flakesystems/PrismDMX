//! Art-Net node discovery over the protocol, against a running daemon — S46.
//!
//! `crates/prism-protocols/src/discovery.rs` asserts the conversation and
//! `crates/prismd/src/outputs.rs` asserts the health fold. This target is the
//! thing an installer actually meets: **the answers a settings window is drawn
//! from**, `Query::ArtNetNodes` and `Query::OutputStatus`, off a daemon that is
//! running a show.
//!
//! # Punch-list B6, asserted where the operator reads it
//!
//! *ArtNet zeigt Health OK ohne angeschlossene Node.* The entry is asserted here
//! rather than only one layer down, because what the entry is about is a line in
//! a panel and a panel draws an answer. So: configure a node, let the daemon poll
//! it, answer nothing, and read the same two questions the panel asks.
//!
//! # Nothing here touches a network
//!
//! `CLAUDE.md`'s rule, and S10's sharper one — no test may put a broadcast or a
//! multicast datagram on the network it runs on. The daemon runs with
//! `--mock-devices`, so its own discovery is idle and opens no socket at all;
//! the discovery this file gives it is over `prism_protocols::MockUdpNode`, which
//! is a queue and not a socket. The node addresses are loopback because they have
//! to be *some* address, and no datagram is ever sent to one.

// Every test holds `common::one_daemon_at_a_time` across its awaits, for
// `wiring.rs`'s reason: a daemon owns a real-time tick thread, and several in
// one process measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use prism_core::{ShowFile, ShowStore};
use prism_domain::{
    Answer, ArtNetPort, Command, NodeHealth, OutputHealth, OutputId, OutputInstance, OutputKind,
    Query, UniverseId,
};
use prism_protocols::{
    ART_NET_ID, ART_POLL_REPLY_BYTES, DiscoveryConfig, MockUdpNode, MockUdpNodeHandle,
    OP_POLL_REPLY, UdpNode,
};
use prismd::cli::Options;
use prismd::daemon::Daemon;
use prismd::discovery::{Discovery, SocketSource};

mod common;

/// A node's address on loopback, on Art-Net's own port.
fn node_at(last: u8) -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, last], 6454))
}

/// A reply as a node at `last` would send it, carrying `ports` output universes
/// from sub-net 0.
fn reply_from(last: u8, name: &str, ports: &[u8]) -> Vec<u8> {
    let mut packet = vec![0u8; ART_POLL_REPLY_BYTES];
    packet[..8].copy_from_slice(&ART_NET_ID);
    packet[8..10].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
    packet[10..14].copy_from_slice(&[127, 0, 0, last]);
    packet[26..26 + name.len()].copy_from_slice(name.as_bytes());
    packet[44..44 + name.len()].copy_from_slice(name.as_bytes());
    packet[173] = u8::try_from(ports.len()).unwrap();
    for (index, port) in ports.iter().enumerate() {
        packet[174 + index] = 0x80;
        packet[182 + index] = 0x80;
        packet[190 + index] = *port;
    }
    packet[201..207].copy_from_slice(&[0, 0, 0, 0, 0, last]);
    packet
}

/// A socket source handing out one mock socket a test feeds.
struct MockSockets(Mutex<Option<MockUdpNode>>);

impl MockSockets {
    fn new() -> (Arc<Self>, MockUdpNodeHandle) {
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

fn discovery() -> (Discovery, MockUdpNodeHandle) {
    let (source, handle) = MockSockets::new();
    (
        Discovery::with_source(
            source,
            DiscoveryConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                wait: Duration::from_millis(1),
                ..DiscoveryConfig::default()
            },
        ),
        handle,
    )
}

fn write_show(path: &Path) {
    let mut file = ShowFile::new();
    file.show
        .embed_fixture_type(common::dimmer_type("generic.dimmer", 65_535))
        .unwrap();
    for universe in 1..=4u32 {
        file.show
            .patch_fixture(common::fixture(universe, "generic.dimmer", universe, 1))
            .unwrap();
    }
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: Some(4),
        outputs: Vec::new(),
        // Every driver is a double, and the daemon's own discovery is idle: no
        // cable is opened, no port is bound and no datagram leaves this process.
        mock_devices: true,
        local: Some(false),
        websocket: prismd::cli::Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
        ..Options::default()
    }
}

/// The Art-Net output of the worked example, addressed to one node.
fn node_output(id: u32, last: u8, universes: &[u32], ports: Vec<ArtNetPort>) -> OutputInstance {
    OutputInstance::new(
        OutputId::new(id),
        format!("Node {id}"),
        OutputKind::ArtNet {
            nodes: vec![node_at(last)],
            sync: false,
            ports,
        },
        universes.iter().copied().map(UniverseId::new),
    )
}

async fn until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}");
}

/// The nodes an answer lists, or a panic naming what came back instead.
fn nodes_of(answer: &Answer) -> &[prism_domain::ArtNetNodeInfo] {
    match answer {
        Answer::ArtNetNodes { nodes, .. } => nodes,
        other => panic!("a question about nodes was answered with {other:?}"),
    }
}

/// One output's status row.
fn status_of(answer: &Answer, id: u32) -> prism_domain::OutputStatusInfo {
    match answer {
        Answer::OutputStatus { outputs } => outputs
            .iter()
            .find(|row| row.id == OutputId::new(id))
            .cloned()
            .unwrap_or_else(|| panic!("output {id} has no status row")),
        other => panic!("a question about output status was answered with {other:?}"),
    }
}

/// **Punch-list B6, over the protocol.** A configured node that never answers
/// never reads *OK* — asserted on the two answers a settings window draws.
#[tokio::test]
async fn a_configured_node_that_never_answers_never_reads_ok() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let (discovery, _socket) = discovery();
    daemon.desk().core().adopt_discovery(discovery);
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            output: node_output(1, 5, &[1, 2], Vec::new()),
        })
        .unwrap();

    // The driver is perfectly happy: the socket takes every datagram, which is
    // exactly what the entry is about.
    until("the driver to start sending", || {
        daemon.desk().core().outputs().health(OutputId::new(1)) == OutputHealth::Ok
    })
    .await;
    until("the first poll to go out", || {
        daemon
            .desk()
            .core()
            .outputs()
            .discovered()
            .counters
            .polls_sent
            > 0
    })
    .await;

    let status = status_of(&daemon.desk().query(&Query::OutputStatus), 1);
    assert_eq!(
        status.health,
        OutputHealth::Degraded,
        "a node nothing answers at must never read OK — punch-list B6"
    );
    assert_eq!(status.nodes.len(), 1, "one row per configured node");
    assert_eq!(status.nodes[0].health, NodeHealth::NeverAnswered);
    assert!(!status.nodes[0].health.is_answering());
    assert_eq!(status.nodes[0].address, node_at(5).to_string());
    assert_eq!(status.nodes[0].name, None, "there is nothing to call it");
    assert_eq!(status.nodes[0].last_reply_ago_ms, None);

    // …and the snapshot every client is handed says the same thing, so a delta
    // and an answer cannot disagree about one output.
    let snapshot = daemon.desk().output_snapshots();
    assert_eq!(
        snapshot
            .iter()
            .find(|row| row.id == OutputId::new(1))
            .map(|row| row.health),
        Some(OutputHealth::Degraded)
    );

    // The node list is empty, and it says out loud that it *is* listening — so
    // the emptiness is evidence rather than ignorance.
    let answer = daemon.desk().query(&Query::ArtNetNodes);
    assert!(nodes_of(&answer).is_empty());
    let Answer::ArtNetNodes {
        listening,
        error,
        counters,
        ..
    } = &answer
    else {
        panic!("a question about nodes was answered with {answer:?}")
    };
    assert!(*listening);
    assert_eq!(*error, None);
    // **The numbers that make this diagnosable**, and they are here because the
    // first thing S46 got wrong in a hall could not be seen from outside: polls
    // going out with nothing coming back is a different fault from nothing
    // being asked, and from something arriving and being dropped.
    assert!(counters.polls_sent >= 1, "{counters:?}");
    assert_eq!(counters.replies, 0);
    assert_eq!(counters.malformed, 0, "nothing arrived to be dropped");
    assert_eq!(counters.read_errors, 0);

    daemon.shutdown().await;
}

/// **The fault a real node found.** A node that pads its `ArtPollReply` past the
/// 239 bytes §6 lists is a node that answered, and it must not read *Degraded*.
///
/// Before the fix the reply was read into a buffer exactly the size of the
/// packet, which truncates on Unix and **fails** on Windows (`WSAEMSGSIZE`, and
/// the data is discarded) — so the node answered every poll, the desk heard
/// nothing, and CI was green because CI is Linux.
#[tokio::test]
async fn a_node_that_pads_its_reply_is_answering_and_not_degraded() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let (discovery, socket) = discovery();
    daemon.desk().core().adopt_discovery(discovery);
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            output: node_output(1, 5, &[1], Vec::new()),
        })
        .unwrap();
    until("the driver to start sending", || {
        daemon.desk().core().outputs().health(OutputId::new(1)) == OutputHealth::Ok
    })
    .await;

    let mut padded = reply_from(5, "Stage left", &[0]);
    padded.extend(std::iter::repeat_n(0u8, 273));
    assert_eq!(padded.len(), 512, "a size real nodes actually send");
    socket.deliver(node_at(5), &padded);
    until("the padded reply to be heard", || {
        !daemon.desk().core().outputs().discovered().nodes.is_empty()
    })
    .await;

    let status = status_of(&daemon.desk().query(&Query::OutputStatus), 1);
    assert_eq!(
        status.health,
        OutputHealth::Ok,
        "it answered, and a reply this desk could not read is not the node's fault"
    );
    assert_eq!(status.nodes[0].health, NodeHealth::Answering);
    assert_eq!(status.nodes[0].name.as_deref(), Some("Stage left"));

    daemon.shutdown().await;
}

/// The other half: a node that answers is discovered, named and shown, and the
/// output goes green.
#[tokio::test]
async fn a_node_that_answers_is_discovered_named_and_shown() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let (discovery, socket) = discovery();
    daemon.desk().core().adopt_discovery(discovery);
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            // Universes 1 and 2, no rows of their own, so they take the default
            // ARCHITECTURE_SPEC.md §7.0 states: port addresses 0 and 1.
            output: node_output(1, 5, &[1, 2], Vec::new()),
        })
        .unwrap();
    until("the driver to start sending", || {
        daemon.desk().core().outputs().health(OutputId::new(1)) == OutputHealth::Ok
    })
    .await;

    // The node answers, and says it outputs three universes — one more than this
    // desk sends it.
    socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0, 1, 2]));
    until("the node to be heard", || {
        !daemon.desk().core().outputs().discovered().nodes.is_empty()
    })
    .await;

    let answer = daemon.desk().query(&Query::ArtNetNodes);
    let nodes = nodes_of(&answer);
    assert_eq!(nodes.len(), 1);
    let node = &nodes[0];
    assert_eq!(node.address, node_at(5).to_string());
    assert_eq!(node.ip, "127.0.0.5");
    assert_eq!(node.short_name, "Stage left");
    assert_eq!(node.long_name, "Stage left");
    assert_eq!(node.mac, "00:00:00:00:00:05");
    assert_eq!(node.ports, vec![0, 1, 2], "its own port-address table");
    assert!(node.inputs.is_empty());
    assert!(node.configured, "the rig addresses this node");
    assert_eq!(
        node.unaddressed_ports,
        vec![2],
        "it outputs a universe this desk sends nothing on, which is the \
         disagreement an installer is looking for"
    );
    assert!(
        node.missing_ports.is_empty(),
        "and everything this desk sends it, it lists"
    );
    assert_eq!(
        node.suggested_universes,
        vec![UniverseId::new(1), UniverseId::new(2), UniverseId::new(3)],
        "one click: the default mapping run backwards, decided by the daemon"
    );
    assert!(node.replies >= 1);

    let status = status_of(&daemon.desk().query(&Query::OutputStatus), 1);
    assert_eq!(status.health, OutputHealth::Ok, "something is listening");
    assert_eq!(status.nodes[0].health, NodeHealth::Answering);
    assert_eq!(status.nodes[0].name.as_deref(), Some("Stage left"));
    assert!(status.nodes[0].last_reply_ago_ms.is_some());

    daemon.shutdown().await;
}

/// A node that is out there and that nothing is addressed to — the other half of
/// *discovered is not configured*, and the row the one-click output is made from.
#[tokio::test]
async fn a_node_nothing_is_addressed_to_is_listed_and_marked_unconfigured() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let (discovery, socket) = discovery();
    daemon.desk().core().adopt_discovery(discovery);
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            output: node_output(1, 5, &[1], Vec::new()),
        })
        .unwrap();
    until("the discovery to be listening", || {
        daemon.desk().core().outputs().discovered().listening
    })
    .await;

    // A node announces itself, the way one does at power-up.
    socket.deliver(node_at(11), &reply_from(11, "New node", &[3, 4]));
    until("the announcement to be heard", || {
        !daemon.desk().core().outputs().discovered().nodes.is_empty()
    })
    .await;

    let answer = daemon.desk().query(&Query::ArtNetNodes);
    let node = nodes_of(&answer)
        .iter()
        .find(|node| node.address == node_at(11).to_string())
        .expect("the node that announced itself");
    assert!(!node.configured, "nothing in the rig is addressed to it");
    assert_eq!(
        node.unaddressed_ports,
        vec![3, 4],
        "this desk sends it nothing at all, so both its universes are unaddressed"
    );
    assert!(node.missing_ports.is_empty());
    assert_eq!(
        node.suggested_universes,
        vec![UniverseId::new(4), UniverseId::new(5)],
        "what an output made from this row would carry"
    );

    daemon.shutdown().await;
}

/// The disagreement in the other direction: this desk sends a node a universe
/// the node does not list.
#[tokio::test]
async fn a_universe_this_desk_sends_that_the_node_does_not_list_is_reported() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let (discovery, socket) = discovery();
    daemon.desk().core().adopt_discovery(discovery);
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            // Universe 1 re-addressed to port 9 — the row an installer types
            // after reading the back of the box, and the row that is wrong when
            // they read it off the wrong box.
            output: node_output(
                1,
                5,
                &[1],
                vec![ArtNetPort {
                    universe: UniverseId::new(1),
                    net: 0,
                    sub_net: 0,
                    port: 9,
                }],
            ),
        })
        .unwrap();
    until("the discovery to be listening", || {
        daemon.desk().core().outputs().discovered().listening
    })
    .await;
    socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
    until("the node to be heard", || {
        !daemon.desk().core().outputs().discovered().nodes.is_empty()
    })
    .await;

    let answer = daemon.desk().query(&Query::ArtNetNodes);
    let node = &nodes_of(&answer)[0];
    assert_eq!(
        node.missing_ports,
        vec![9],
        "this desk sends port address 9 and the node does not have one"
    );
    assert_eq!(
        node.unaddressed_ports,
        vec![0],
        "and the node's only port gets nothing"
    );

    daemon.shutdown().await;
}

/// A daemon with no Art-Net row listens to nothing, and says so — which is a
/// different fact from *nothing answers*.
#[tokio::test]
async fn a_desk_with_no_art_net_output_is_not_listening_and_that_is_not_an_error() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let (discovery, _socket) = discovery();
    daemon.desk().core().adopt_discovery(discovery);
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            output: OutputInstance::new(
                OutputId::new(1),
                "Hall",
                OutputKind::Mock,
                [UniverseId::new(1)],
            ),
        })
        .unwrap();
    until("the driver to start sending", || {
        daemon.desk().core().outputs().health(OutputId::new(1)) == OutputHealth::Ok
    })
    .await;

    let answer = daemon.desk().query(&Query::ArtNetNodes);
    assert!(
        matches!(
            answer,
            Answer::ArtNetNodes {
                listening: false,
                error: None,
                ..
            }
        ),
        "nothing configured is not an error, and it is not a bound socket either: {answer:?}"
    );
    assert!(nodes_of(&answer).is_empty());

    // And nothing about the mock output's health is folded, because a mock
    // output has no far end to be silent.
    let status = status_of(&daemon.desk().query(&Query::OutputStatus), 1);
    assert_eq!(status.health, OutputHealth::Ok);
    assert!(status.nodes.is_empty());

    daemon.shutdown().await;
}

/// Removing the last Art-Net row gives the port back, and what was heard stays
/// true.
#[tokio::test]
async fn removing_the_last_art_net_row_stops_listening_and_keeps_the_table() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let (discovery, socket) = discovery();
    daemon.desk().core().adopt_discovery(discovery);
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            output: node_output(1, 5, &[1], Vec::new()),
        })
        .unwrap();
    until("the discovery to be listening", || {
        daemon.desk().core().outputs().discovered().listening
    })
    .await;
    socket.deliver(node_at(5), &reply_from(5, "Stage left", &[0]));
    until("the node to be heard", || {
        !daemon.desk().core().outputs().discovered().nodes.is_empty()
    })
    .await;

    daemon
        .desk()
        .core()
        .apply(&Command::RemoveOutput {
            id: OutputId::new(1),
        })
        .unwrap();

    let answer = daemon.desk().query(&Query::ArtNetNodes);
    assert!(
        matches!(
            answer,
            Answer::ArtNetNodes {
                listening: false,
                ..
            }
        ),
        "{answer:?}"
    );
    let node = &nodes_of(&answer)[0];
    assert_eq!(node.short_name, "Stage left");
    assert!(
        !node.configured,
        "the row that addressed it is gone, and the node is still out there"
    );

    daemon.shutdown().await;
}
