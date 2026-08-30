//! The daemon's half — `docs/IPC_PROTOCOL.md` §4, §5 and §8.
//!
//! Accepts a [`Wire`] from any transport, runs the handshake, serves the
//! snapshot, and then carries commands inwards and deltas outwards until one end
//! or the other stops. It holds no show, no session and no engine: everything it
//! knows about the daemon is the [`ServerHandler`] it was given, which is what
//! lets the whole of §4 be tested with no daemon in existence.
//!
//! # What the server decides, and what it refuses to decide
//!
//! Decided here, because they are protocol:
//!
//! - the handshake and its three refusals — wrong version, missing token,
//!   anything before `Hello` (§4.1, §4.2, §2.1);
//! - that deltas reach **every** client and an `Ack` reaches one (D3: a client
//!   is a view onto the daemon, not the owner of its own change);
//! - that a delta caused by a command goes out **before** that command's `Ack`,
//!   so a client that has been told its command succeeded has already been told
//!   what it did;
//! - that a full control queue ends the connection with a `Reject`, and a slow
//!   client loses telemetry and nothing else (§8, and [`crate::Outbound`]).
//!
//! Refused here, and left to the handler: whether a command is valid, what it
//! changes, what the world looks like. Those are `prism-core`'s and S17's, and
//! a server that had an opinion about them would be a second source of truth.
//!
//! # The rule about broken messages
//!
//! An error reported by the *wire* ends the connection: the framing could not
//! find the next boundary, so there is nothing left to read. A payload the wire
//! delivered whole and the server could not decode is answered with a `Reject`
//! and the connection carries on — see [`crate::FrameError::loses_the_frame_boundary`].
//! The distinction matters because the second case is reachable by an honest
//! client of the wrong version, and disconnecting it would hide the reason.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use core::fmt;
use prism_domain::{Answer, Command, Delta, Query, StorePreview};
use tokio::sync::{Mutex, Notify};

use crate::backpressure::{Outbound, OutboundStats};
use crate::frame::{decode, encode};
use crate::message::{
    ClientMessage, Hello, PROTOCOL_VERSION, RejectReason, ServerMessage, Snapshot,
};
use crate::transport::{Wire, WireSender};

/// Which client. Unique within one run of the daemon; not persisted and not
/// meaningful to anybody outside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientId(u64);

impl ClientId {
    /// The raw number, for logs and for a status panel.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "client {}", self.0)
    }
}

/// What the daemon did with a command.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandOutcome {
    /// Applied. The deltas describe what changed and go to every client.
    Applied {
        /// What changed, in order.
        deltas: Vec<Delta>,
    },
    /// Refused, and nothing changed (§5).
    Refused {
        /// Why, in words an operator can read.
        message: String,
    },
}

impl CommandOutcome {
    /// Applied, with nothing to tell anybody about.
    #[must_use]
    pub const fn applied() -> Self {
        Self::Applied { deltas: Vec::new() }
    }

    /// Refused, with a reason.
    #[must_use]
    pub fn refused(message: impl Into<String>) -> Self {
        Self::Refused {
            message: message.into(),
        }
    }
}

/// What the daemon has to supply for the protocol to mean anything.
///
/// Deliberately four methods, two of which have a default: everything else the
/// server needs it already knows. S17 implements this over `prism_core::ShowFile`
/// and the engine.
pub trait ServerHandler: Send + Sync + 'static {
    /// The world, for a client that has just been accepted.
    fn snapshot(&self, client: ClientId, hello: &Hello) -> Snapshot;

    /// Apply a command, or refuse it. A refusal must change nothing (§5).
    fn command(&self, client: ClientId, command: Command) -> CommandOutcome;

    /// Answer a question. **Must change nothing at all** (§5.2).
    ///
    /// There is no refusal shape, and that is deliberate: every [`Query`] is
    /// answerable against any state, so a daemon that could not answer one
    /// would be a daemon with a defect rather than a client with a bad
    /// question. The default answers the only way a handler with no opinion
    /// can, which is what keeps the test handlers in this crate short.
    fn query(&self, client: ClientId, query: Query) -> Answer {
        let _ = client;
        match query {
            Query::PatchConflicts | Query::PatchPreview { .. } => Answer::PatchConflicts {
                conflicts: Vec::new(),
            },
            Query::SearchLibrary { .. } => Answer::LibraryMatches {
                matches: Vec::new(),
                total: 0,
            },
            // A handler with no machine behind it has no ports and has chosen
            // none — which is the same answer a laptop with nothing plugged in
            // gives, so there is nothing here for a client to special-case.
            Query::MidiPorts => Answer::MidiPorts {
                ports: Vec::new(),
                configured: None,
                open: None,
                status: None,
            },
            // And no rig, so nothing is dark: a handler with no machine behind
            // it has no patch to have left anywhere (S37).
            Query::DarkUniverses => Answer::DarkUniverses {
                universes: Vec::new(),
            },
            Query::OutputStatus => Answer::OutputStatus {
                outputs: Vec::new(),
            },
            // And nothing listening, which is the honest answer for a handler
            // with no machine behind it — and a different one from *no nodes*.
            // The flag is what keeps the two apart (S46).
            Query::ArtNetNodes => Answer::ArtNetNodes {
                nodes: Vec::new(),
                listening: false,
                error: None,
                counters: prism_domain::ArtNetCounters::default(),
                remedy: None,
            },
            // And no cue list, so no rows. An empty list is the same shape a
            // sequence with no cues gives, which is what a handler with no show
            // honestly has (S48).
            Query::CueTracking { sequence_id } => Answer::CueTracking {
                sequence_id,
                cues: Vec::new(),
            },
            // And no surface, so no table. An empty list is the same shape a
            // desk with a table gives and is the honest answer for a handler
            // that has never had one (S38).
            Query::SurfaceBindings => Answer::SurfaceBindings {
                controls: Vec::new(),
                device: String::new(),
                device_key: String::new(),
                profile_version: 0,
                profile: None,
                revision: 0,
                learning: false,
            },
            // And no parser: reading a line is `prism_core::console`'s, which a
            // protocol crate deliberately does not depend on. An empty reading
            // is the same shape an empty line gives, and it is the honest answer
            // for a handler that holds nothing to read one against (S49).
            Query::CommandLineReading { text } => Answer::CommandLineReading {
                text,
                reading: String::new(),
                kind: prism_domain::CommandLineReadingKind::Empty,
                commands: 0,
                verb: false,
                clearing: false,
                question: None,
                completions: Vec::new(),
            },
            // The mode is **echoed**, not chosen: a handler with no show still
            // has to answer about the mode it was asked about, or a client
            // would draw a refusal beside a word nobody typed (S39).
            Query::StorePreview { mode, .. } => Answer::StorePreview {
                preview: StorePreview {
                    accepted: false,
                    refusal: Some("this handler holds no show".to_owned()),
                    exists: false,
                    name: String::new(),
                    mode,
                    added: 0,
                    replaced: 0,
                    kept: 0,
                    removed: 0,
                },
            },
        }
    }

    /// A client was accepted.
    fn connected(&self, client: ClientId, hello: &Hello) {
        let _ = (client, hello);
    }

    /// A client has gone. §8: the daemon frees its state and carries on;
    /// nothing about the show changes.
    fn disconnected(&self, client: ClientId) {
        let _ = client;
    }
}

/// How the server behaves, independent of which transport a client arrived on.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Control messages one client may be behind before it is disconnected.
    pub control_queue: usize,
    /// The token of §2.1, when the listener is reachable off the machine. `None`
    /// on a loopback or local endpoint, where §2.1 does not ask for one.
    pub token: Option<String>,
    /// How long to wait for a client to take the `Reject` that ends its
    /// connection.
    ///
    /// **The message is best-effort; the disconnection is not.** The commonest
    /// reason to send one is §8's: the client's control queue filled because it
    /// stopped reading — and a client that stopped reading is exactly the client
    /// that cannot be told why. Waiting indefinitely would leave the daemon
    /// holding a connection it has already given up on, for as long as the
    /// process lives.
    pub goodbye: std::time::Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            control_queue: crate::backpressure::DEFAULT_CONTROL_QUEUE,
            token: None,
            goodbye: std::time::Duration::from_secs(1),
        }
    }
}

/// One accepted connection's outbound state.
#[derive(Debug)]
struct Connection {
    outbound: Mutex<Outbound>,
    /// Rung whenever something is queued, so the connection task stops waiting.
    wake: Notify,
}

impl Connection {
    fn new(limit: usize) -> Self {
        Self {
            outbound: Mutex::new(Outbound::new(limit)),
            wake: Notify::new(),
        }
    }

    /// Queues a message, ending the connection if the control queue is full.
    async fn push(&self, message: ServerMessage) {
        let mut outbound = self.outbound.lock().await;
        if let Err(full) = outbound.push(message) {
            outbound.fail(ServerMessage::Reject {
                seq: None,
                reason: RejectReason::Backpressure,
                message: full.to_string(),
            });
        }
        drop(outbound);
        self.wake.notify_one();
    }
}

/// The daemon's IPC server.
///
/// Cheap to clone: every clone is a handle onto the same set of connections, so
/// the engine thread, the surface controller and the autosave timer can all
/// broadcast without passing one object around.
#[derive(Clone)]
pub struct ServerHandle {
    inner: Arc<Inner>,
}

struct Inner {
    handler: Arc<dyn ServerHandler>,
    config: ServerConfig,
    clients: Mutex<HashMap<ClientId, Arc<Connection>>>,
    next_id: AtomicU64,
}

/// The server, which is the handle: there is nothing else to hold.
///
/// A daemon keeps one and clones it wherever a delta comes from — the engine
/// thread, the surface controller, the autosave timer. `Server::new` reads the
/// way the rest of the workspace does; [`ServerHandle`] is the name in the
/// signatures.
pub type Server = ServerHandle;

impl ServerHandle {
    /// A server that serves `handler`.
    #[must_use]
    pub fn new(handler: impl ServerHandler) -> Self {
        Self::with_config(handler, ServerConfig::default())
    }

    /// A server that serves `handler` under `config`.
    #[must_use]
    pub fn with_config(handler: impl ServerHandler, config: ServerConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                handler: Arc::new(handler),
                config,
                clients: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(1),
            }),
        }
    }
}

impl fmt::Debug for ServerHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerHandle")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

impl ServerHandle {
    /// Runs one connection to completion. Returns when it has closed.
    pub async fn serve(&self, wire: Wire) {
        let (sender, mut receiver) = wire.split();

        let hello = match handshake(&self.inner, &sender, &mut receiver).await {
            Some(hello) => hello,
            None => {
                sender.shutdown_within(self.inner.config.goodbye).await;
                return;
            }
        };

        let id = ClientId(self.inner.next_id.fetch_add(1, Ordering::Relaxed));
        let connection = Arc::new(Connection::new(self.inner.config.control_queue));
        self.inner
            .clients
            .lock()
            .await
            .insert(id, Arc::clone(&connection));
        self.inner.handler.connected(id, &hello);

        connection
            .push(ServerMessage::Snapshot {
                snapshot: Box::new(self.inner.handler.snapshot(id, &hello)),
            })
            .await;

        self.pump(id, &connection, &sender, &mut receiver).await;

        self.inner.clients.lock().await.remove(&id);
        self.inner.handler.disconnected(id);
        // Bounded by the same deadline as the queue: a client that is not
        // reading cannot be waited for, however much one would like it to hear
        // the reason.
        sender.shutdown_within(self.inner.config.goodbye).await;
    }

    /// Spawns [`Self::serve`] on the runtime, for a caller running an accept
    /// loop.
    pub fn spawn(&self, wire: Wire) -> tokio::task::JoinHandle<()> {
        let handle = self.clone();
        tokio::spawn(async move { handle.serve(wire).await })
    }

    /// The loop: read commands, write whatever is queued, stop when either end
    /// does.
    async fn pump(
        &self,
        id: ClientId,
        connection: &Arc<Connection>,
        sender: &WireSender,
        receiver: &mut crate::transport::WireReceiver,
    ) {
        loop {
            // Created before the queue is inspected, so a message queued between
            // the two is not missed.
            let woken = connection.wake.notified();
            tokio::pin!(woken);

            let (waiting, closing) = {
                let outbound = connection.outbound.lock().await;
                (!outbound.is_empty(), outbound.is_closing())
            };

            tokio::select! {
                biased;

                incoming = receiver.recv() => {
                    match incoming {
                        None => return,
                        Some(Err(error)) => {
                            // The wire itself failed: there is no next frame to
                            // find, so there is nothing to say and nowhere to
                            // say it.
                            let _ = error;
                            return;
                        }
                        Some(Ok(payload)) => {
                            if !self.handle_payload(id, connection, &payload).await {
                                // A terminal refusal was queued. Fall through so
                                // it is written before the connection closes.
                            }
                        }
                    }
                }

                permit = room_to_send(sender, closing, self.inner.config.goodbye), if waiting => {
                    let Some(Ok(permit)) = permit else { return };
                    let message = connection.outbound.lock().await.take();
                    let Some(message) = message else { continue };
                    let terminal = message.is_terminal();
                    // A message this daemon cannot encode is a defect in the
                    // daemon rather than in the client, so it is dropped rather
                    // than closing a working connection over it. S17 logs it.
                    if let Ok(payload) = encode(&message) {
                        permit.send(payload);
                    }
                    if terminal {
                        return;
                    }
                }

                () = &mut woken => {}
            }
        }
    }

    /// Handles one delivered payload. Answers `false` when the connection is
    /// ending.
    async fn handle_payload(
        &self,
        id: ClientId,
        connection: &Arc<Connection>,
        payload: &[u8],
    ) -> bool {
        match decode::<ClientMessage>(payload) {
            Ok(ClientMessage::Command { seq, command }) => {
                match self.inner.handler.command(id, command) {
                    CommandOutcome::Applied { deltas } => {
                        // The fact first, then the receipt: a client that has
                        // been told its command succeeded has already been told
                        // what it did.
                        for delta in deltas {
                            self.broadcast(delta).await;
                        }
                        connection.push(ServerMessage::Ack { seq }).await;
                    }
                    CommandOutcome::Refused { message } => {
                        connection
                            .push(ServerMessage::Reject {
                                seq: Some(seq),
                                reason: RejectReason::CommandRefused,
                                message,
                            })
                            .await;
                    }
                }
                true
            }
            Ok(ClientMessage::Query { seq, query }) => {
                // No broadcast and no ack: a question changes nothing, so the
                // answer goes to the one client that asked and to nobody else.
                let answer = self.inner.handler.query(id, query);
                connection.push(ServerMessage::Answer { seq, answer }).await;
                true
            }
            Ok(ClientMessage::Hello { .. }) => {
                connection
                    .push(ServerMessage::Reject {
                        seq: None,
                        reason: RejectReason::OutOfOrder,
                        message: "this connection has already said hello".to_owned(),
                    })
                    .await;
                false
            }
            Err(error) => {
                connection
                    .push(ServerMessage::Reject {
                        seq: None,
                        reason: RejectReason::Undecodable,
                        message: error.to_string(),
                    })
                    .await;
                !error.loses_the_frame_boundary()
            }
        }
    }

    /// Sends a delta to every connected client (§6: deltas are ordered per
    /// connection, and every client is a view onto the same daemon).
    pub async fn broadcast(&self, delta: Delta) {
        let clients: Vec<Arc<Connection>> =
            self.inner.clients.lock().await.values().cloned().collect();
        for connection in clients {
            connection
                .push(ServerMessage::Delta {
                    delta: delta.clone(),
                })
                .await;
        }
    }

    /// Sends a telemetry frame to every client. Droppable: a client that is
    /// behind gets the newest frame and never a queue of old ones (§7).
    pub async fn telemetry(&self, encoded: Vec<u8>) {
        let clients: Vec<Arc<Connection>> =
            self.inner.clients.lock().await.values().cloned().collect();
        for connection in clients {
            connection
                .push(ServerMessage::Telemetry {
                    data: encoded.clone(),
                })
                .await;
        }
    }

    /// Ends every connection with a `Reject`, so clients show *the daemon
    /// stopped* rather than *the connection broke*.
    pub async fn shutdown(&self) {
        let clients: Vec<Arc<Connection>> =
            self.inner.clients.lock().await.values().cloned().collect();
        for connection in clients {
            let mut outbound = connection.outbound.lock().await;
            outbound.fail(ServerMessage::Reject {
                seq: None,
                reason: RejectReason::ShuttingDown,
                message: "the daemon is shutting down".to_owned(),
            });
            drop(outbound);
            connection.wake.notify_one();
        }
    }

    /// How many clients are connected. Zero is an ordinary state: D2 exists so
    /// that it changes nothing about the show.
    pub async fn client_count(&self) -> usize {
        self.inner.clients.lock().await.len()
    }

    /// Which clients are connected, oldest first.
    ///
    /// A status panel (S27) shows one row per connection, and S18's gate needs
    /// to name a particular one to ask [`Self::stats`] about it — a counter that
    /// can only be read by a caller who already knows the id is a counter
    /// nothing outside this module can read at all. Ordered by id, which is the
    /// order the connections were accepted in.
    pub async fn clients(&self) -> Vec<ClientId> {
        let mut ids: Vec<ClientId> = self.inner.clients.lock().await.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// The queue counters for one client, for a status panel and for S18's gate.
    pub async fn stats(&self, id: ClientId) -> Option<OutboundStats> {
        let clients = self.inner.clients.lock().await;
        let connection = clients.get(&id)?;
        let stats = connection.outbound.lock().await.stats();
        Some(stats)
    }
}

/// Waits for the transport to be free, giving up after `goodbye` once the
/// connection is ending.
///
/// `None` means *stop waiting and close*: the client is not reading, so the
/// message it is being sent cannot reach it, and holding the connection open for
/// it would be holding it open forever.
async fn room_to_send<'a>(
    sender: &'a WireSender,
    closing: bool,
    goodbye: std::time::Duration,
) -> Option<Result<crate::transport::WirePermit<'a>, crate::WireError>> {
    if closing {
        tokio::time::timeout(goodbye, sender.ready()).await.ok()
    } else {
        Some(sender.ready().await)
    }
}

/// Runs the handshake. Answers `Some` when the client was accepted, having
/// already sent the `Reject` when it was not.
async fn handshake(
    inner: &Arc<Inner>,
    sender: &WireSender,
    receiver: &mut crate::transport::WireReceiver,
) -> Option<Hello> {
    let payload = match receiver.recv().await? {
        Ok(payload) => payload,
        Err(_) => return None,
    };

    let refusal = match decode::<ClientMessage>(&payload) {
        Ok(ClientMessage::Hello { hello }) => {
            if hello.protocol_version == PROTOCOL_VERSION {
                match inner.config.token.as_deref() {
                    // §2.1: a token is only asked for when the daemon was
                    // configured with one, which is when the listener is
                    // reachable from another machine.
                    Some(expected) if hello.token.as_deref() != Some(expected) => {
                        Some((RejectReason::Unauthorised, "this connection needs the access token shown in the daemon's settings".to_owned()))
                    }
                    _ => None,
                }
            } else {
                Some((
                    RejectReason::ProtocolVersion,
                    format!(
                        "the interface speaks protocol version {} and the engine speaks {PROTOCOL_VERSION}",
                        hello.protocol_version
                    ),
                ))
            }
            .map_or_else(|| Ok(hello.clone()), Err)
        }
        Ok(ClientMessage::Command { .. } | ClientMessage::Query { .. }) => Err((
            RejectReason::OutOfOrder,
            "the first message on a connection must be Hello".to_owned(),
        )),
        Err(error) => Err((RejectReason::Undecodable, error.to_string())),
    };

    match refusal {
        Ok(hello) => Some(hello),
        Err((reason, message)) => {
            // Sent rather than queued: there is no connection state yet, and the
            // client is entitled to know why it was turned away.
            let _ = sender
                .send_message(&ServerMessage::Reject {
                    seq: None,
                    reason,
                    message,
                })
                .await;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClientId, CommandOutcome, Server, ServerConfig, ServerHandle, ServerHandler, Snapshot,
    };
    use crate::message::{
        ClientKind, ClientMessage, DaemonHealth, Hello, RejectReason, ServerMessage,
    };
    use crate::transport::memory;
    use prism_domain::{Command, Delta, JsonValue, NoticeLevel, ProgrammerState};
    use std::sync::Mutex;

    /// A daemon that records what it was asked and answers as it was told to.
    ///
    /// The equivalent of `prism-protocols`' `MockOutput`: the server's whole
    /// contract is what it does with this, so the tests need one that says
    /// exactly what happened rather than one that pretends to be a show.
    #[derive(Default)]
    struct Recording {
        seen: Mutex<Vec<Command>>,
        connected: Mutex<Vec<ClientId>>,
        disconnected: Mutex<Vec<ClientId>>,
        refuse: bool,
    }

    impl Recording {
        fn commands(&self) -> Vec<Command> {
            self.seen.lock().unwrap().clone()
        }
    }

    impl ServerHandler for std::sync::Arc<Recording> {
        fn snapshot(&self, _client: ClientId, _hello: &Hello) -> Snapshot {
            Snapshot {
                show: JsonValue::String("the show".to_owned()),
                session: JsonValue::String("the session".to_owned()),
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

        fn command(&self, _client: ClientId, command: Command) -> CommandOutcome {
            self.seen.lock().unwrap().push(command);
            if self.refuse {
                return CommandOutcome::refused("not now");
            }
            CommandOutcome::Applied {
                deltas: vec![Delta::DirtyFlag {
                    unsaved_changes: true,
                }],
            }
        }

        fn connected(&self, client: ClientId, _hello: &Hello) {
            self.connected.lock().unwrap().push(client);
        }

        fn disconnected(&self, client: ClientId) {
            self.disconnected.lock().unwrap().push(client);
        }
    }

    fn server(handler: &std::sync::Arc<Recording>) -> ServerHandle {
        Server::new(std::sync::Arc::clone(handler))
    }

    fn hello() -> ClientMessage {
        ClientMessage::Hello {
            hello: Hello::new(ClientKind::Desktop),
        }
    }

    #[tokio::test]
    async fn a_hello_is_answered_with_the_world() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);

        client.send_message(&hello()).await.unwrap();
        let answer = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();
        let ServerMessage::Snapshot { snapshot } = answer else {
            panic!("the answer to a hello is a snapshot, not {answer:?}");
        };
        assert_eq!(snapshot.show, JsonValue::String("the show".to_owned()));
        assert_eq!(
            snapshot.session,
            JsonValue::String("the session".to_owned())
        );

        drop(client);
        task.await.unwrap();
        assert_eq!(handler.connected.lock().unwrap().len(), 1);
        assert_eq!(
            *handler.disconnected.lock().unwrap(),
            *handler.connected.lock().unwrap(),
            "every client that connected must be reported gone"
        );
    }

    #[tokio::test]
    async fn a_command_is_applied_and_answered() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);

        client.send_message(&hello()).await.unwrap();
        let _snapshot = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();

        client
            .send_message(&ClientMessage::Command {
                seq: 42,
                command: Command::ClearProgrammer,
            })
            .await
            .unwrap();

        // The fact, then the receipt.
        assert_eq!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Delta {
                delta: Delta::DirtyFlag {
                    unsaved_changes: true
                }
            }
        );
        assert_eq!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Ack { seq: 42 }
        );
        assert_eq!(handler.commands(), vec![Command::ClearProgrammer]);

        drop(client);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn a_refused_command_is_rejected_and_the_connection_carries_on() {
        let handler = std::sync::Arc::new(Recording {
            refuse: true,
            ..Recording::default()
        });
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);

        client.send_message(&hello()).await.unwrap();
        let _snapshot = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();

        for seq in 0..2 {
            client
                .send_message(&ClientMessage::Command {
                    seq,
                    command: Command::ClearProgrammer,
                })
                .await
                .unwrap();
            assert_eq!(
                client
                    .recv_message::<ServerMessage>()
                    .await
                    .unwrap()
                    .unwrap(),
                ServerMessage::Reject {
                    seq: Some(seq),
                    reason: RejectReason::CommandRefused,
                    message: "not now".to_owned(),
                }
            );
        }
        assert_eq!(
            handler.commands().len(),
            2,
            "the second one was still heard"
        );

        drop(client);
        task.await.unwrap();
    }

    /// §4.2, and the sentence that motivates it: undefined behaviour from a
    /// silent mismatch is not acceptable in software that controls a show.
    #[tokio::test]
    async fn a_version_mismatch_is_rejected_in_words_and_the_connection_ends() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);

        client
            .send_message(&ClientMessage::Hello {
                hello: Hello {
                    protocol_version: 99,
                    client_kind: ClientKind::WebRemote,
                    token: None,
                },
            })
            .await
            .unwrap();

        let answer = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();
        let ServerMessage::Reject {
            reason, message, ..
        } = answer
        else {
            panic!("expected a rejection, not {answer:?}");
        };
        assert_eq!(reason, RejectReason::ProtocolVersion);
        assert!(message.contains("99"), "{message}");
        assert!(reason.closes_the_connection());
        assert_eq!(client.recv().await, None);

        task.await.unwrap();
        assert!(
            handler.connected.lock().unwrap().is_empty(),
            "a client that was turned away was never connected"
        );
    }

    #[tokio::test]
    async fn a_command_before_the_handshake_is_refused() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);

        client
            .send_message(&ClientMessage::Command {
                seq: 1,
                command: Command::ClearProgrammer,
            })
            .await
            .unwrap();

        assert!(matches!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Reject {
                reason: RejectReason::OutOfOrder,
                ..
            }
        ));
        assert!(
            handler.commands().is_empty(),
            "it must not have been applied"
        );
        task.await.unwrap();
    }

    #[tokio::test]
    async fn a_second_hello_ends_the_connection() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);

        client.send_message(&hello()).await.unwrap();
        let _snapshot = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();
        client.send_message(&hello()).await.unwrap();

        assert!(matches!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Reject {
                reason: RejectReason::OutOfOrder,
                ..
            }
        ));
        assert_eq!(client.recv().await, None);
        task.await.unwrap();
    }

    /// The frame boundary is intact, so the client is told and kept.
    #[tokio::test]
    async fn a_payload_that_will_not_decode_is_rejected_without_closing() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);

        client.send_message(&hello()).await.unwrap();
        let _snapshot = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();

        client.send(vec![0xc1]).await.unwrap();
        assert!(matches!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Reject {
                reason: RejectReason::Undecodable,
                ..
            }
        ));

        // Still usable, which is the point.
        client
            .send_message(&ClientMessage::Command {
                seq: 5,
                command: Command::ClearProgrammer,
            })
            .await
            .unwrap();
        let _delta = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Ack { seq: 5 }
        );

        drop(client);
        task.await.unwrap();
    }

    /// §2.1: a listener reachable from another machine needs a token.
    #[tokio::test]
    async fn a_configured_token_is_required_and_a_wrong_one_is_refused() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = Server::with_config(
            std::sync::Arc::clone(&handler),
            ServerConfig {
                token: Some("secret".to_owned()),
                ..ServerConfig::default()
            },
        );

        for (token, accepted) in [
            (None, false),
            (Some("wrong"), false),
            (Some("secret"), true),
        ] {
            let (mut client, daemon) = memory::pair();
            let task = server.spawn(daemon);
            let mut hello = Hello::new(ClientKind::WebRemote);
            hello.token = token.map(str::to_owned);
            client
                .send_message(&ClientMessage::Hello { hello })
                .await
                .unwrap();

            let answer = client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap();
            if accepted {
                assert!(
                    matches!(answer, ServerMessage::Snapshot { .. }),
                    "{answer:?}"
                );
            } else {
                assert!(
                    matches!(
                        answer,
                        ServerMessage::Reject {
                            reason: RejectReason::Unauthorised,
                            ..
                        }
                    ),
                    "{answer:?}"
                );
            }
            drop(client);
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn a_delta_reaches_every_client() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);

        let mut clients = Vec::new();
        let mut tasks = Vec::new();
        for _ in 0..3 {
            let (mut client, daemon) = memory::pair();
            tasks.push(server.spawn(daemon));
            client.send_message(&hello()).await.unwrap();
            let _snapshot = client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap();
            clients.push(client);
        }
        assert_eq!(server.client_count().await, 3);

        let notice = Delta::Notice {
            level: NoticeLevel::Info,
            message: "everybody".to_owned(),
        };
        server.broadcast(notice.clone()).await;

        for client in &mut clients {
            assert_eq!(
                client
                    .recv_message::<ServerMessage>()
                    .await
                    .unwrap()
                    .unwrap(),
                ServerMessage::Delta {
                    delta: notice.clone()
                }
            );
        }

        drop(clients);
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(server.client_count().await, 0);
    }

    #[tokio::test]
    async fn a_shutdown_tells_every_client_why() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);
        client.send_message(&hello()).await.unwrap();
        let _snapshot = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();

        server.shutdown().await;

        assert!(matches!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Reject {
                reason: RejectReason::ShuttingDown,
                ..
            }
        ));
        assert_eq!(client.recv().await, None, "the connection ends after it");
        task.await.unwrap();
    }

    #[tokio::test]
    async fn telemetry_reaches_every_client_and_the_counters_say_so() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);
        client.send_message(&hello()).await.unwrap();
        let _snapshot = client
            .recv_message::<ServerMessage>()
            .await
            .unwrap()
            .unwrap();

        server.telemetry(vec![1, 2, 3]).await;
        assert_eq!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Telemetry {
                data: vec![1, 2, 3]
            }
        );

        let stats = server.stats(ClientId(1)).await.unwrap();
        assert_eq!(stats.telemetry_sent, 1);
        assert_eq!(server.stats(ClientId(999)).await, None);
        assert_eq!(server.clients().await, vec![ClientId(1)]);

        drop(client);
        task.await.unwrap();
    }

    /// A caller that wants a client's counters has to be able to find out which
    /// clients there are, and in which order they arrived.
    #[tokio::test]
    async fn the_connected_clients_are_listed_oldest_first() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        assert!(server.clients().await.is_empty());

        let mut clients = Vec::new();
        let mut tasks = Vec::new();
        for _ in 0..3 {
            let (mut client, daemon) = memory::pair();
            tasks.push(server.spawn(daemon));
            client.send_message(&hello()).await.unwrap();
            let _snapshot = client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap();
            clients.push(client);
        }
        assert_eq!(
            server.clients().await,
            vec![ClientId(1), ClientId(2), ClientId(3)]
        );

        // The middle one leaves, and the list is what is left rather than what
        // there once was.
        clients.remove(1);
        tasks.remove(1).await.unwrap();
        assert_eq!(server.clients().await, vec![ClientId(1), ClientId(3)]);

        drop(clients);
        for task in tasks {
            task.await.unwrap();
        }
        assert!(server.clients().await.is_empty());
    }

    /// §3, from the daemon's side: an oversized frame closes the connection.
    ///
    /// Nothing is said back, and nothing can be: the length prefix was believed
    /// far enough to know it was absurd, so where the next frame starts is no
    /// longer known and every byte after it is rubbish of unknown length.
    #[tokio::test]
    async fn a_client_that_sends_an_oversized_frame_is_disconnected() {
        use tokio::io::AsyncWriteExt as _;

        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut raw, there) = tokio::io::duplex(64 * 1024);
        let task = server.spawn(crate::transport::stream::spawn(there));

        // A proper handshake first, so this is a connected client rather than a
        // refused one.
        let hello = crate::encode(&hello()).unwrap();
        raw.write_all(&(hello.len() as u32).to_le_bytes())
            .await
            .unwrap();
        raw.write_all(&hello).await.unwrap();
        while server.client_count().await == 0 {
            tokio::task::yield_now().await;
        }

        raw.write_all(&u32::MAX.to_le_bytes()).await.unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await
            .expect("the connection must be closed")
            .unwrap();
        assert_eq!(server.client_count().await, 0);
        assert!(
            handler.commands().is_empty(),
            "nothing the client sent was acted on"
        );
    }

    /// The handshake's own version of the rule about broken messages: a first
    /// message that will not decode is refused in words.
    #[tokio::test]
    async fn a_first_message_that_is_not_a_message_is_refused() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = server(&handler);
        let (mut client, daemon) = memory::pair();
        let task = server.spawn(daemon);

        // Well-formed MessagePack that is not a `ClientMessage`.
        client.send(crate::encode(&7_u8).unwrap()).await.unwrap();

        assert!(matches!(
            client
                .recv_message::<ServerMessage>()
                .await
                .unwrap()
                .unwrap(),
            ServerMessage::Reject {
                reason: RejectReason::Undecodable,
                ..
            }
        ));
        assert_eq!(client.recv().await, None);
        task.await.unwrap();
        assert!(handler.connected.lock().unwrap().is_empty());
    }

    /// The disconnection is not best-effort even though the message is: a
    /// client that never reads must not hold a connection open for ever.
    #[tokio::test]
    async fn a_client_that_never_takes_its_rejection_is_dropped_anyway() {
        let handler = std::sync::Arc::new(Recording::default());
        let server = Server::with_config(
            std::sync::Arc::clone(&handler),
            ServerConfig {
                control_queue: 2,
                goodbye: std::time::Duration::from_millis(20),
                ..ServerConfig::default()
            },
        );

        // A transport so small that nothing can be written into it once the
        // client stops reading — which it does immediately after its hello.
        let (client, there) = tokio::io::duplex(32);
        let client = crate::transport::stream::spawn(client);
        let task = server.spawn(crate::transport::stream::spawn(there));

        client.send_message(&hello()).await.unwrap();
        // The handshake has to finish before there is anybody to fall behind.
        while server.client_count().await == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        for _ in 0..64 {
            server
                .broadcast(Delta::DirtyFlag {
                    unsaved_changes: true,
                })
                .await;
        }

        // Without the `goodbye` deadline this never returns.
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await
            .expect("the connection must be given up on")
            .unwrap();
        assert_eq!(server.client_count().await, 0);
        assert_eq!(
            *handler.disconnected.lock().unwrap(),
            *handler.connected.lock().unwrap(),
            "the handler must be told the client has gone"
        );
        drop(client);
    }

    #[test]
    fn a_client_id_says_which_client_it_is() {
        assert_eq!(ClientId(7).get(), 7);
        assert_eq!(ClientId(7).to_string(), "client 7");
        assert_eq!(
            CommandOutcome::applied(),
            CommandOutcome::Applied { deltas: vec![] }
        );
        assert_eq!(
            CommandOutcome::refused("no"),
            CommandOutcome::Refused {
                message: "no".to_owned()
            }
        );
        assert!(
            format!(
                "{:?}",
                Server::new(std::sync::Arc::new(Recording::default()))
            )
            .contains("ServerHandle")
        );
    }
}
