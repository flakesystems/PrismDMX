//! Transport parity — `IMPLEMENTATION_PLAN.md` S16, third exit criterion, and
//! `docs/IPC_PROTOCOL.md` §9's last row.
//!
//! > Run the full suite over both named pipe / UDS and WebSocket; results must
//! > be identical.
//!
//! # How parity is obtained, rather than merely checked
//!
//! The weak reading of that row is two suites that look alike. This file takes
//! the strong one: there is **one** suite, [`the_full_suite`], and it is
//! generic over nothing at all — it takes a [`Desk`], which is the same type
//! whichever transport built it. The transport disappears in
//! [`prism_ipc::Wire`], four functions below the suite, and everything the suite
//! touches is above that line.
//!
//! So the tests below cannot drift apart, because there is nothing to drift:
//! `over_the_local_transport`, `over_the_websocket_transport` and
//! `over_the_memory_transport` are three calls to one function.
//!
//! # And a transcript, because "identical" is a claim about results
//!
//! A suite that passes three times proves the three transports are each
//! acceptable. `the_three_transports_produce_byte_identical_results` proves they
//! are the same: one scripted session is run over each, every message the client
//! received is recorded, and the three recordings are compared as **encoded
//! bytes**. A transport that reordered a delta, dropped a telemetry frame or
//! rounded a float would fail there and nowhere else.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::Mutex;

use prism_domain::{
    Command, Delta, ExecutorId, FeatureGroup, FixtureId, JsonValue, NoticeLevel, OutputHealth,
    OutputId, ProgrammerState, UniverseId,
};
use prism_ipc::telemetry::{TelemetryFrame, UniverseLevels};
use prism_ipc::transport::{local, memory, websocket};
use prism_ipc::{
    Client, ClientError, ClientEvent, ClientId, ClientKind, CommandOutcome, DaemonHealth, Hello,
    OutputSnapshot, RejectReason, Server, ServerHandle, ServerHandler, Snapshot,
};

// ---------------------------------------------------------------------------
// The daemon under test
// ---------------------------------------------------------------------------

/// A daemon that answers the way `prism-core` will, without being one.
///
/// **Nothing in it is a default value.** S14's finding, taken as a rule since
/// S15: a fixture built out of zeros cannot tell "carried across the wire" from
/// "never filled in", and a snapshot is exactly the message that failure mode
/// would hide in.
struct Desk0 {
    seen: Mutex<Vec<(ClientId, Command)>>,
}

impl Desk0 {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            seen: Mutex::new(Vec::new()),
        })
    }

    fn commands(&self) -> Vec<Command> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .map(|(_, command)| command.clone())
            .collect()
    }
}

fn the_world() -> Snapshot {
    Snapshot {
        show: JsonValue::Object(
            [
                (
                    "fixtures".to_owned(),
                    JsonValue::Array(vec![JsonValue::Int(11), JsonValue::Int(12)]),
                ),
                ("name".to_owned(), JsonValue::String("Aula".to_owned())),
                ("panTilt".to_owned(), JsonValue::Float(-12.5)),
            ]
            .into_iter()
            .collect(),
        ),
        session: JsonValue::Object(
            [
                ("executorPage".to_owned(), JsonValue::Int(3)),
                (
                    "commandLine".to_owned(),
                    JsonValue::String("go 1".to_owned()),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        programmer: ProgrammerState {
            selection: vec![FixtureId::new(11), FixtureId::new(12)],
            active_feature_group: FeatureGroup::Color,
            ..ProgrammerState::default()
        },
        outputs: vec![
            OutputSnapshot {
                id: OutputId::new(1),
                name: "Open DMX USB".to_owned(),
                health: OutputHealth::Ok,
            },
            OutputSnapshot {
                id: OutputId::new(2),
                name: "sACN universe 5".to_owned(),
                health: OutputHealth::Degraded,
            },
        ],
        health: DaemonHealth {
            protocol_version: prism_ipc::PROTOCOL_VERSION,
            tick_hz: 43.98,
            missed_ticks: 4,
            unsaved_changes: true,
        },
    }
}

/// The command the desk refuses, so a rejection is reachable from the suite.
fn the_refused_command() -> Command {
    Command::ExecutorOff {
        executor_id: ExecutorId::new(999),
    }
}

/// The handler the server holds. A newtype because `ServerHandler` and `Arc`
/// are both somebody else's from in here, and the orphan rule is right about
/// that.
struct Handler(Arc<Desk0>);

impl ServerHandler for Handler {
    fn snapshot(&self, _client: ClientId, _hello: &Hello) -> Snapshot {
        the_world()
    }

    fn command(&self, client: ClientId, command: Command) -> CommandOutcome {
        self.0.seen.lock().unwrap().push((client, command.clone()));
        if command == the_refused_command() {
            return CommandOutcome::refused("there is no executor 999");
        }
        CommandOutcome::Applied {
            deltas: vec![
                Delta::DirtyFlag {
                    unsaved_changes: true,
                },
                Delta::Notice {
                    level: NoticeLevel::Info,
                    message: format!("{command:?}"),
                },
            ],
        }
    }
}

/// A telemetry frame with something in it.
fn the_lights() -> TelemetryFrame {
    let mut first = UniverseLevels::blackout(UniverseId::new(1));
    first.levels[0] = 255;
    first.levels[42] = 17;
    let mut second = UniverseLevels::blackout(UniverseId::new(5));
    second.levels[511] = 200;
    TelemetryFrame {
        sequence: 9_000_000_000,
        universes: vec![first, second],
    }
}

// ---------------------------------------------------------------------------
// The transports, and the one thing they all become
// ---------------------------------------------------------------------------

/// Which transport a [`Desk`] is listening on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    /// Named pipe on Windows, Unix domain socket elsewhere.
    Local,
    /// WebSocket over `axum`.
    WebSocket,
    /// The in-process duplex, which is a transport like the others and is here
    /// so that "the suite runs over three" is not a claim about two.
    Memory,
}

/// A running daemon, reachable however it is reachable.
struct Desk {
    server: ServerHandle,
    handler: Arc<Desk0>,
    reach: Reach,
    accepting: Option<tokio::task::JoinHandle<()>>,
}

enum Reach {
    Local(String),
    WebSocket(SocketAddr),
    Memory,
}

impl Desk {
    async fn start(transport: Transport, label: &str) -> Self {
        let handler = Desk0::new();
        let server = Server::new(Handler(Arc::clone(&handler)));

        let (reach, accepting) = match transport {
            Transport::Local => {
                let address = local::scratch_address(label);
                let mut listener = local::LocalListener::bind(&address).expect("bind");
                let accepting = {
                    let server = server.clone();
                    tokio::spawn(async move {
                        while let Ok(wire) = listener.accept().await {
                            server.spawn(wire);
                        }
                    })
                };
                (Reach::Local(address), Some(accepting))
            }
            Transport::WebSocket => {
                let mut listener =
                    websocket::WebSocketListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
                        .await
                        .expect("bind");
                let addr = listener.local_addr();
                let accepting = {
                    let server = server.clone();
                    tokio::spawn(async move {
                        while let Some(wire) = listener.accept().await {
                            server.spawn(wire);
                        }
                    })
                };
                (Reach::WebSocket(addr), Some(accepting))
            }
            Transport::Memory => (Reach::Memory, None),
        };

        Self {
            server,
            handler,
            reach,
            accepting,
        }
    }

    /// A connected client, handshake completed.
    async fn connect(&self) -> Result<(Client, Snapshot), ClientError> {
        self.connect_saying(Hello::new(ClientKind::Desktop)).await
    }

    /// A connected client, saying whatever it was told to say.
    async fn connect_saying(&self, hello: Hello) -> Result<(Client, Snapshot), ClientError> {
        match &self.reach {
            Reach::Local(address) => {
                Client::connect(&prism_ipc::Endpoint::Local(address.clone()), hello).await
            }
            Reach::WebSocket(addr) => {
                Client::connect(&prism_ipc::Endpoint::WebSocket(*addr), hello).await
            }
            Reach::Memory => {
                let (client_wire, daemon_wire) = memory::pair();
                self.server.spawn(daemon_wire);
                Client::handshake(client_wire, hello).await
            }
        }
    }
}

impl Drop for Desk {
    fn drop(&mut self) {
        if let Some(accepting) = self.accepting.take() {
            accepting.abort();
        }
    }
}

/// Waits for the next event, failing rather than hanging if the daemon has gone.
async fn next(client: &mut Client) -> ClientEvent {
    match client.next_event().await {
        Some(Ok(event)) => event,
        Some(Err(error)) => panic!("the daemon reported {error}"),
        None => panic!("the daemon closed the connection"),
    }
}

// ---------------------------------------------------------------------------
// The suite
// ---------------------------------------------------------------------------

/// Everything the protocol promises, over whichever transport it was given.
async fn the_full_suite(transport: Transport) {
    the_handshake_serves_the_world(transport).await;
    a_command_is_applied_and_answered_in_that_order(transport).await;
    a_refused_command_changes_nothing_and_says_why(transport).await;
    a_version_mismatch_is_refused_in_words(transport).await;
    a_delta_reaches_every_client(transport).await;
    telemetry_arrives_decoded(transport).await;
    a_client_that_leaves_takes_nothing_with_it(transport).await;
}

/// §4.1: the client asks for the world and receives it.
async fn the_handshake_serves_the_world(transport: Transport) {
    let desk = Desk::start(transport, "handshake").await;
    let (client, snapshot) = desk.connect().await.expect("the daemon is listening");

    assert_eq!(snapshot, the_world(), "{transport:?}");
    // Named field by field as well, because `assert_eq!` on a whole struct
    // passes just as happily when the fixture is empty — and this fixture is
    // deliberately not.
    assert_eq!(snapshot.outputs.len(), 2);
    assert_eq!(snapshot.outputs[1].name, "sACN universe 5");
    assert_eq!(snapshot.outputs[1].health, OutputHealth::Degraded);
    assert_eq!(snapshot.programmer.selection.len(), 2);
    assert_eq!(
        snapshot.programmer.active_feature_group,
        FeatureGroup::Color
    );
    assert!(snapshot.health.unsaved_changes);
    assert_eq!(snapshot.health.missed_ticks, 4);
    assert!((snapshot.health.tick_hz - 43.98).abs() < f64::EPSILON);

    client.disconnect().await;
}

/// §5 and §6: the fact travels before the receipt.
async fn a_command_is_applied_and_answered_in_that_order(transport: Transport) {
    let desk = Desk::start(transport, "command").await;
    let (mut client, _snapshot) = desk.connect().await.unwrap();

    let seq = client
        .send(Command::SetExecutorPage { page: 7 })
        .await
        .unwrap();
    assert_eq!(seq, 0, "{transport:?}");

    assert_eq!(
        next(&mut client).await,
        ClientEvent::Delta(Delta::DirtyFlag {
            unsaved_changes: true
        })
    );
    assert!(matches!(
        next(&mut client).await,
        ClientEvent::Delta(Delta::Notice { .. })
    ));
    assert_eq!(next(&mut client).await, ClientEvent::Ack { seq });

    assert_eq!(
        desk.handler.commands(),
        vec![Command::SetExecutorPage { page: 7 }]
    );
    client.disconnect().await;
}

/// §5: a command that cannot be applied yields a `Reject` and changes nothing.
async fn a_refused_command_changes_nothing_and_says_why(transport: Transport) {
    let desk = Desk::start(transport, "refused").await;
    let (mut client, _snapshot) = desk.connect().await.unwrap();

    let seq = client.send(the_refused_command()).await.unwrap();
    assert_eq!(
        next(&mut client).await,
        ClientEvent::Refused {
            seq: Some(seq),
            reason: RejectReason::CommandRefused,
            message: "there is no executor 999".to_owned(),
        },
        "{transport:?}"
    );

    // The connection is still usable: a refused command is an ordinary answer.
    let seq = client.send(Command::Oops).await.unwrap();
    let _first = next(&mut client).await;
    let _second = next(&mut client).await;
    assert_eq!(next(&mut client).await, ClientEvent::Ack { seq });

    client.disconnect().await;
}

/// §4.2, which exists because undefined behaviour from a silent mismatch is not
/// acceptable in software that controls a show.
async fn a_version_mismatch_is_refused_in_words(transport: Transport) {
    let desk = Desk::start(transport, "version").await;
    let mut hello = Hello::new(ClientKind::WebRemote);
    hello.protocol_version = prism_ipc::PROTOCOL_VERSION + 1;

    let error = desk.connect_saying(hello).await.unwrap_err();
    let ClientError::Rejected { reason, message } = error else {
        panic!("{transport:?}: expected a rejection, got {error:?}");
    };
    assert_eq!(reason, RejectReason::ProtocolVersion);
    assert!(
        message.contains(&(prism_ipc::PROTOCOL_VERSION + 1).to_string()),
        "{message}"
    );
}

/// D3: every client is a view onto one daemon, so every client sees the change.
async fn a_delta_reaches_every_client(transport: Transport) {
    let desk = Desk::start(transport, "broadcast").await;
    let (mut first, _) = desk.connect().await.unwrap();
    let (mut second, _) = desk.connect().await.unwrap();
    assert_eq!(desk.server.client_count().await, 2, "{transport:?}");

    let notice = Delta::Notice {
        level: NoticeLevel::Warn,
        message: "the recovery copy outlived its show".to_owned(),
    };
    desk.server.broadcast(notice.clone()).await;

    assert_eq!(next(&mut first).await, ClientEvent::Delta(notice.clone()));
    assert_eq!(next(&mut second).await, ClientEvent::Delta(notice));

    first.disconnect().await;
    second.disconnect().await;
}

/// §7: binary, fixed layout, and the same bytes whichever transport carried it.
async fn telemetry_arrives_decoded(transport: Transport) {
    let desk = Desk::start(transport, "telemetry").await;
    let (mut client, _snapshot) = desk.connect().await.unwrap();

    desk.server.telemetry(the_lights().encode()).await;
    let ClientEvent::Telemetry(frame) = next(&mut client).await else {
        panic!("{transport:?}: expected telemetry");
    };
    assert_eq!(frame, the_lights());
    assert_eq!(frame.universes[0].levels[42], 17);
    assert_eq!(frame.universes[1].levels[511], 200);
    assert_eq!(frame.sequence, 9_000_000_000);

    client.disconnect().await;
}

/// §8: the daemon frees the client's state and carries on. Nothing about the
/// show changes, and no other client notices.
async fn a_client_that_leaves_takes_nothing_with_it(transport: Transport) {
    let desk = Desk::start(transport, "leaving").await;
    let (staying, _) = desk.connect().await.unwrap();
    let (leaving, _) = desk.connect().await.unwrap();
    let mut staying = staying;

    leaving.disconnect().await;
    // The daemon notices in its own time; the assertion is that it does.
    for _ in 0..200 {
        if desk.server.client_count().await == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(desk.server.client_count().await, 1, "{transport:?}");

    let notice = Delta::Notice {
        level: NoticeLevel::Info,
        message: "still here".to_owned(),
    };
    desk.server.broadcast(notice.clone()).await;
    assert_eq!(next(&mut staying).await, ClientEvent::Delta(notice));

    staying.disconnect().await;
}

// ---------------------------------------------------------------------------
// The three runs
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn over_the_local_transport() {
    the_full_suite(Transport::Local).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn over_the_websocket_transport() {
    the_full_suite(Transport::WebSocket).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn over_the_memory_transport() {
    the_full_suite(Transport::Memory).await;
}

// ---------------------------------------------------------------------------
// "Results must be identical", taken literally
// ---------------------------------------------------------------------------

/// One event, as the bytes it arrived as.
///
/// Every arm is a real wire encoding rather than a debug rendering: the deltas
/// and the answers go back through the MessagePack the daemon sent them in, and
/// the telemetry frame back through its own fixed layout. A transport that
/// altered anything at all shows up here.
fn record(event: &ClientEvent) -> Vec<u8> {
    match event {
        ClientEvent::Delta(delta) => prism_ipc::encode(delta).unwrap(),
        ClientEvent::Telemetry(frame) => frame.encode(),
        ClientEvent::Ack { seq } => prism_ipc::encode(seq).unwrap(),
        ClientEvent::Refused {
            seq,
            reason,
            message,
        } => prism_ipc::encode(&(seq, reason, message)).unwrap(),
    }
}

/// One scripted session, recorded as the bytes the client's events encode to.
async fn transcript(transport: Transport) -> Vec<Vec<u8>> {
    let desk = Desk::start(transport, "transcript").await;
    let (mut client, snapshot) = desk.connect().await.unwrap();

    let mut recorded = vec![prism_ipc::encode(&snapshot).unwrap()];

    // A command that is applied, a command that is refused, a broadcast delta
    // and a telemetry frame — one of each kind of thing the daemon can say.
    let expected_events = {
        client
            .send(Command::SetEncoderBank {
                group: FeatureGroup::Beam,
            })
            .await
            .unwrap();
        client.send(the_refused_command()).await.unwrap();
        3 + 1
    };
    for _ in 0..expected_events {
        recorded.push(record(&next(&mut client).await));
    }

    desk.server
        .broadcast(Delta::ProgrammerChanged {
            state: ProgrammerState {
                selection: vec![FixtureId::new(4)],
                active_feature_group: FeatureGroup::Position,
                ..ProgrammerState::default()
            },
        })
        .await;
    desk.server.telemetry(the_lights().encode()).await;

    recorded.push(record(&next(&mut client).await));
    recorded.push(record(&next(&mut client).await));

    client.disconnect().await;
    recorded
}

/// The parity criterion, as a claim about results rather than about coverage.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_three_transports_produce_byte_identical_results() {
    let local = transcript(Transport::Local).await;
    let web = transcript(Transport::WebSocket).await;
    let in_memory = transcript(Transport::Memory).await;

    assert_eq!(local, web, "named pipe / UDS and WebSocket disagree");
    assert_eq!(
        local, in_memory,
        "the local transport and the duplex disagree"
    );

    // Not vacuous: a transcript of nothing would be equal to a transcript of
    // nothing.
    assert_eq!(
        local.len(),
        7,
        "the script produced the wrong number of messages"
    );
    assert!(
        local.iter().all(|message| !message.is_empty()),
        "an empty message is not a recording"
    );
    assert!(
        local.iter().map(Vec::len).sum::<usize>() > 1200,
        "the transcript is too small to have carried a telemetry frame"
    );
}

/// The recorder itself, so a transcript of four identical empty vectors could
/// not pass the comparison above.
#[test]
fn the_recorder_tells_the_events_apart() {
    let events = [
        ClientEvent::Delta(Delta::DirtyFlag {
            unsaved_changes: true,
        }),
        ClientEvent::Telemetry(the_lights()),
        ClientEvent::Ack { seq: 3 },
        ClientEvent::Refused {
            seq: Some(3),
            reason: RejectReason::CommandRefused,
            message: "no".to_owned(),
        },
    ];
    let recorded: Vec<Vec<u8>> = events.iter().map(record).collect();
    for (index, bytes) in recorded.iter().enumerate() {
        assert!(!bytes.is_empty(), "event {index} recorded as nothing");
        assert_eq!(
            recorded.iter().filter(|other| *other == bytes).count(),
            1,
            "event {index} records the same as another"
        );
    }
}
