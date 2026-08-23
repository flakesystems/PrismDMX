//! The mechanism of `docs/IPC_PROTOCOL.md` §8, over a real connection.
//!
//! > Telemetry is coalesced then dropped. Control messages are **never**
//! > dropped; if the control queue for a client fills, that client is
//! > disconnected with a `Reject` and must reconnect and re-snapshot.
//!
//! `backpressure::Outbound`'s own tests pin the policy as a state machine. This
//! file is the other half: that the policy is actually *reached* through a
//! socket, a server task and a client that has stopped reading — which is the
//! only way to find out that the queue is bounded by the thing it was meant to
//! be bounded by.
//!
//! **S18 owns the gate**, and it is a different claim: that a slow client loses
//! telemetry, loses no commands, and does not affect other clients, measured
//! against a running daemon. S16 owes S18 a mechanism that can be measured, and
//! these are the tests that say it exists.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use prism_domain::ProgrammerState;
use prism_domain::{Command, Delta, NoticeLevel};
use prism_ipc::transport::{memory, stream};
use prism_ipc::{
    ClientId, ClientKind, ClientMessage, CommandOutcome, DaemonHealth, Hello, RejectReason, Server,
    ServerConfig, ServerHandler, ServerMessage, Snapshot,
};

/// Bytes of transport buffer in the slow-client tests.
///
/// Deliberately tiny. The whole point is to make the client's *transport* fill
/// up while it is not reading, so that the queue above it fills too — with the
/// two-megabyte duplex the other tests use, a thousand small deltas would simply
/// sit in the buffer and nothing would ever be under pressure.
const SMALL_BUFFER: usize = 512;

/// A daemon that records which clients it has, so a test can name one.
#[derive(Default)]
struct Desk {
    clients: Mutex<Vec<ClientId>>,
}

struct Handler(Arc<Desk>);

impl ServerHandler for Handler {
    fn snapshot(&self, _client: ClientId, _hello: &Hello) -> Snapshot {
        Snapshot {
            show: prism_domain::JsonValue::String("show".to_owned()),
            session: prism_domain::JsonValue::String("session".to_owned()),
            programmer: ProgrammerState::default(),
            outputs: Vec::new(),
            health: DaemonHealth {
                tick_hz: 44.0,
                ..DaemonHealth::default()
            },
            fixture_library: 0,
            machine: prism_domain::MachineSettings::default(),
            show_file: prism_domain::ShowFileInfo::default(),
        }
    }

    fn command(&self, _client: ClientId, _command: Command) -> CommandOutcome {
        CommandOutcome::applied()
    }

    fn connected(&self, client: ClientId, _hello: &Hello) {
        self.0.clients.lock().unwrap().push(client);
    }
}

fn notice(n: usize) -> Delta {
    Delta::Notice {
        level: NoticeLevel::Info,
        message: format!("delta {n}"),
    }
}

/// Waits for `condition`, rather than sleeping a guessed amount and hoping.
async fn until(mut condition: impl AsyncFnMut() -> bool) -> bool {
    for _ in 0..400 {
        if condition().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

/// How long a test waits for a message before deciding none is coming.
///
/// **Every receive in this file has a deadline, and one of them did not.** The
/// first version of `telemetry_is_coalesced_…` read a fixed number of messages
/// off a client that is behind by construction, which is a count nothing
/// guarantees: on this machine nine arrived, on a two-core CI runner fewer did,
/// and the test blocked for as long as the job was allowed to run. A test that
/// can hang is worse than a test that fails, because a failure names itself.
const PATIENCE: Duration = Duration::from_millis(500);

/// The next message, or `None` if the connection closed or nothing came.
async fn next(client: &mut prism_ipc::Wire) -> Option<ServerMessage> {
    match tokio::time::timeout(PATIENCE, client.recv_message::<ServerMessage>()).await {
        Ok(Some(Ok(message))) => Some(message),
        Ok(Some(Err(error))) => panic!("the connection reported {error}"),
        Ok(None) | Err(_) => None,
    }
}

/// §8: a client whose control queue fills is disconnected with a `Reject`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_that_stops_reading_is_disconnected_and_told_why() {
    let desk = Arc::new(Desk::default());
    let server = Server::with_config(
        Handler(Arc::clone(&desk)),
        ServerConfig {
            control_queue: 8,
            ..ServerConfig::default()
        },
    );

    let (here, there) = tokio::io::duplex(SMALL_BUFFER);
    let mut client = stream::spawn(here);
    server.spawn(stream::spawn(there));

    client
        .send_message(&ClientMessage::Hello {
            hello: Hello::new(ClientKind::Desktop),
        })
        .await
        .unwrap();
    assert!(until(async || server.client_count().await == 1).await);

    // The client reads nothing while this happens. Not even its snapshot.
    for n in 0..500 {
        server.broadcast(notice(n)).await;
    }

    // Then it starts reading again, too late. Everything that got through is on
    // the wire, the reason is the last of it, and then the connection is closed.
    //
    // The reading is what makes this deterministic rather than a race: the
    // `Reject` is best-effort by construction (see `ServerConfig::goodbye`), so
    // a client that never read at all might be disconnected without ever
    // learning why. A client that comes back is the case the message exists for.
    let mut messages = Vec::new();
    while let Some(message) = next(&mut client).await {
        messages.push(message);
    }
    assert!(
        until(async || server.client_count().await == 0).await,
        "a client that fell too far behind must be given up on"
    );

    assert!(
        matches!(messages.first(), Some(ServerMessage::Snapshot { .. })),
        "the first message is still the world"
    );
    let last = messages.last().expect("something reached the client");
    assert!(
        matches!(
            last,
            ServerMessage::Reject {
                seq: None,
                reason: RejectReason::Backpressure,
                ..
            }
        ),
        "the connection ended with {last:?} rather than a reason"
    );
    assert!(
        messages.len() < 500,
        "the client was not actually behind: it received {} of 500",
        messages.len()
    );
}

/// §8: *and does not affect other clients*. The reason the queue is per
/// connection rather than per daemon.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_that_stops_reading_does_not_take_the_others_with_it() {
    let desk = Arc::new(Desk::default());
    let server = Server::with_config(
        Handler(Arc::clone(&desk)),
        ServerConfig {
            control_queue: 8,
            ..ServerConfig::default()
        },
    );

    // The one that reads, on an ordinary connection.
    let (mut healthy, daemon) = memory::pair();
    server.spawn(daemon);
    healthy
        .send_message(&ClientMessage::Hello {
            hello: Hello::new(ClientKind::Desktop),
        })
        .await
        .unwrap();
    assert!(matches!(
        next(&mut healthy).await,
        Some(ServerMessage::Snapshot { .. })
    ));

    // The one that does not.
    let (deaf, there) = tokio::io::duplex(SMALL_BUFFER);
    let deaf = stream::spawn(deaf);
    server.spawn(stream::spawn(there));
    deaf.send_message(&ClientMessage::Hello {
        hello: Hello::new(ClientKind::WebRemote),
    })
    .await
    .unwrap();
    assert!(until(async || server.client_count().await == 2).await);

    for n in 0..500 {
        server.broadcast(notice(n)).await;
        // The healthy client keeps up, which is what makes this a test about the
        // deaf one rather than about both of them being behind.
        let _ = next(&mut healthy).await;
    }

    assert!(
        until(async || server.client_count().await == 1).await,
        "the deaf client should have been given up on"
    );

    // And the healthy one is still being served.
    server
        .broadcast(Delta::DirtyFlag {
            unsaved_changes: false,
        })
        .await;
    let mut found = false;
    while let Some(message) = next(&mut healthy).await {
        if matches!(
            message,
            ServerMessage::Delta {
                delta: Delta::DirtyFlag { .. }
            }
        ) {
            found = true;
            break;
        }
    }
    assert!(found, "the healthy client stopped receiving");

    drop(deaf);
    drop(healthy);
}

/// §7 and §8 together: telemetry is what gets dropped, and a client that is
/// behind is given the newest frame rather than a queue of old ones.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn telemetry_is_coalesced_and_the_counters_say_how_much_was_dropped() {
    let desk = Arc::new(Desk::default());
    let server = Server::new(Handler(Arc::clone(&desk)));

    let (here, there) = tokio::io::duplex(SMALL_BUFFER);
    let mut client = stream::spawn(here);
    server.spawn(stream::spawn(there));
    client
        .send_message(&ClientMessage::Hello {
            hello: Hello::new(ClientKind::Desktop),
        })
        .await
        .unwrap();
    assert!(until(async || server.client_count().await == 1).await);
    let id = desk.clients.lock().unwrap()[0];

    // Two hundred frames at a client that is reading nothing.
    for n in 0..200_u32 {
        let frame = prism_ipc::TelemetryFrame {
            sequence: u64::from(n),
            universes: Vec::new(),
        };
        server.telemetry(frame.encode()).await;
    }

    let stats = server
        .stats(id)
        .await
        .expect("the client is still connected");
    assert!(
        stats.telemetry_dropped > 100,
        "only {} of 200 telemetry frames were coalesced away, so the client was \
         not actually behind",
        stats.telemetry_dropped
    );
    assert_eq!(
        server.client_count().await,
        1,
        "telemetry must never be a reason to disconnect anybody"
    );

    // What did reach the client is a *newer* frame than the first one, because
    // the queue holds one and it is always the latest.
    //
    // Everything that arrived, not a fixed number of messages: how many get
    // through a client that is behind by construction is exactly the quantity
    // this test says nothing is guaranteed about, and the first version of it
    // read eight and blocked on a two-core runner waiting for the ninth.
    let mut latest = None;
    while let Some(message) = next(&mut client).await {
        if let ServerMessage::Telemetry { data } = message {
            latest = Some(prism_ipc::TelemetryFrame::decode(&data).unwrap().sequence);
        }
    }
    assert!(
        latest.is_some_and(|sequence| sequence > 0),
        "the client was served a stale frame, or none at all: {latest:?}"
    );
}
