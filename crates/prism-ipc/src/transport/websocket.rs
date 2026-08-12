//! The WebSocket transport — `docs/IPC_PROTOCOL.md` §2, over `axum`.
//!
//! What the Web Remote uses, and anything else that speaks from a browser. It
//! binds `127.0.0.1` by default, and §2.1 says why in as many words: school
//! networks are shared, and an unauthenticated lighting console reachable from
//! any classroom machine is not acceptable.
//!
//! # Why `axum` and not the WebSocket library alone
//!
//! A WebSocket server needs an HTTP server under it, and this one will need more
//! than an upgrade route: S31 serves the Web Remote's own assets from the same
//! port, because a second listener is a second thing to firewall and a second
//! thing to get wrong. [`router`] therefore hands back a `Router` rather than
//! hiding one, so a later session adds routes to the listener that already
//! exists instead of starting another.
//!
//! # The size limit is enforced by the WebSocket, not after it
//!
//! [`crate::MAX_FRAME_BYTES`] is given to the WebSocket implementation on both
//! sides, as `max_message_size` and `max_frame_size`. That is the same
//! guarantee [`super::stream`] gets from the length prefix and it is obtained
//! the same way — the limit is checked against what the peer *announced*, before
//! a buffer for it exists. A megabyte cannot be allocated by a peer that merely
//! says it will send one.

use std::io;
use std::net::SocketAddr;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::any;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::frame::MAX_FRAME_BYTES;
use crate::transport::{Endpoint, Wire, WireError, WirePump};

/// The path clients upgrade on.
pub const IPC_PATH: &str = "/ipc";

/// Connections accepted but not yet taken by the server.
///
/// Small on purpose: a listener that queued connections would hide a daemon
/// that has stopped accepting them.
const BACKLOG: usize = 8;

/// The router carrying the IPC upgrade route.
///
/// Public so `prismd` can nest it under a router of its own — S31's Web Remote
/// serves its assets from the same listener.
pub fn router(incoming: mpsc::Sender<Wire>) -> Router {
    Router::new()
        .route(IPC_PATH, any(upgrade))
        .with_state(incoming)
}

/// Answers an upgrade request by handing the connection to the accept queue.
async fn upgrade(ws: WebSocketUpgrade, State(incoming): State<mpsc::Sender<Wire>>) -> Response {
    ws.max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| serve(socket, incoming))
}

/// Runs one accepted connection for as long as it lasts.
async fn serve(socket: WebSocket, incoming: mpsc::Sender<Wire>) {
    let (wire, pump) = Wire::pair();
    if incoming.send(wire).await.is_err() {
        // Nobody is accepting any more: the daemon is shutting down. Closing
        // without a message is right — there is no connection to reject.
        return;
    }
    drive_server(socket, pump).await;
}

/// Moves payloads between an accepted `axum` socket and its wire.
async fn drive_server(socket: WebSocket, pump: WirePump) {
    let WirePump {
        mut outbound,
        inbound,
        flushed,
    } = pump;
    let (mut sink, mut stream) = socket.split();

    loop {
        tokio::select! {
            payload = outbound.recv() => match payload {
                Some(payload) => {
                    if sink.send(Message::Binary(payload.into())).await.is_err() {
                        break;
                    }
                }
                // The wire was dropped: this connection is over.
                None => break,
            },
            message = stream.next() => match message {
                Some(Ok(Message::Binary(data))) => {
                    if inbound.send(Ok(data.to_vec())).await.is_err() {
                        break;
                    }
                }
                Some(Ok(Message::Text(_))) => {
                    let _ = inbound
                        .send(Err(WireError::Transport(
                            "this protocol carries binary frames only".to_owned(),
                        )))
                        .await;
                    break;
                }
                // Ping and pong are answered by the WebSocket implementation.
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(error)) => {
                    let _ = inbound.send(Err(WireError::Transport(error.to_string()))).await;
                    break;
                }
            },
        }
    }

    let _ = sink.close().await;
    let _ = flushed.send(());
}

/// A WebSocket listener, in the shape [`super::local::LocalListener`] has.
///
/// The two are not one type behind a trait, because they are constructed
/// differently and nothing above them holds both: `prismd` opens whichever
/// endpoints its settings ask for and puts the [`Wire`]s into one server. That
/// is where the transport disappears, and it disappears completely.
#[derive(Debug)]
pub struct WebSocketListener {
    incoming: mpsc::Receiver<Wire>,
    local_addr: SocketAddr,
    server: tokio::task::JoinHandle<()>,
}

impl WebSocketListener {
    /// Binds and starts serving.
    ///
    /// Passing port 0 asks the operating system for a free port, which
    /// [`Self::local_addr`] then reports — that is how a test binds without
    /// choosing a number that another test might also choose.
    ///
    /// # Errors
    ///
    /// Whatever the operating system says about the address.
    pub async fn bind(addr: SocketAddr) -> io::Result<Self> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        let (incoming_tx, incoming_rx) = mpsc::channel(BACKLOG);
        let app = router(incoming_tx);
        let server = tokio::spawn(async move {
            // The error is not propagated because there is nobody left to
            // propagate it to: the listener has already been handed out, and a
            // caller learns the server is gone when `accept` answers `None`.
            let _ = axum::serve(listener, app).await;
        });
        Ok(Self {
            incoming: incoming_rx,
            local_addr,
            server,
        })
    }

    /// The next client, or `None` once the listener has stopped.
    pub async fn accept(&mut self) -> Option<Wire> {
        self.incoming.recv().await
    }

    /// The address actually bound, port included.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// The address as an endpoint, for the lock file of §2.2.
    #[must_use]
    pub const fn endpoint(&self) -> Endpoint {
        Endpoint::WebSocket(self.local_addr)
    }

    /// The URL a client connects to.
    #[must_use]
    pub fn url(&self) -> String {
        format!("ws://{}{IPC_PATH}", self.local_addr)
    }
}

impl Drop for WebSocketListener {
    fn drop(&mut self) {
        // A dropped listener stops listening. Without this the task outlives it
        // and the port stays bound, which in a test suite means the next test
        // binds a different port and the leak is never noticed.
        self.server.abort();
    }
}

/// Connects to a daemon's WebSocket listener.
///
/// # Errors
///
/// [`WireError::Transport`] if the connection or the upgrade fails, which is
/// how a client discovers the daemon is not running.
pub async fn connect(url: &str) -> Result<Wire, WireError> {
    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    let (socket, _response) =
        tokio_tungstenite::connect_async_with_config(url, Some(config), false)
            .await
            .map_err(|error| WireError::Transport(error.to_string()))?;

    let (wire, pump) = Wire::pair();
    tokio::spawn(drive_client(socket, pump));
    Ok(wire)
}

/// The client's half of [`drive_server`], against the other library's message
/// type.
async fn drive_client<S>(socket: S, pump: WirePump)
where
    S: futures_util::Sink<
            tokio_tungstenite::tungstenite::Message,
            Error = tokio_tungstenite::tungstenite::Error,
        > + futures_util::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + Send
        + 'static,
{
    use tokio_tungstenite::tungstenite::Message as Wsm;

    let WirePump {
        mut outbound,
        inbound,
        flushed,
    } = pump;
    let (mut sink, mut stream) = socket.split();

    loop {
        tokio::select! {
            payload = outbound.recv() => match payload {
                Some(payload) => {
                    if sink.send(Wsm::Binary(payload.into())).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
            message = stream.next() => match message {
                Some(Ok(Wsm::Binary(data))) => {
                    if inbound.send(Ok(data.to_vec())).await.is_err() {
                        break;
                    }
                }
                Some(Ok(Wsm::Text(_))) => {
                    let _ = inbound
                        .send(Err(WireError::Transport(
                            "this protocol carries binary frames only".to_owned(),
                        )))
                        .await;
                    break;
                }
                Some(Ok(Wsm::Ping(_) | Wsm::Pong(_) | Wsm::Frame(_))) => {}
                Some(Ok(Wsm::Close(_))) | None => break,
                Some(Err(error)) => {
                    let _ = inbound.send(Err(WireError::Transport(error.to_string()))).await;
                    break;
                }
            },
        }
    }

    let _ = sink.close().await;
    let _ = flushed.send(());
}

#[cfg(test)]
mod tests {
    use super::{IPC_PATH, WebSocketListener, connect};
    use crate::message::{ClientKind, ClientMessage, Hello};
    use crate::transport::Endpoint;
    use std::net::{Ipv4Addr, SocketAddr};

    fn loopback() -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
    }

    #[tokio::test]
    async fn a_browser_client_reaches_the_daemon_and_messages_travel_both_ways() {
        let mut listener = WebSocketListener::bind(loopback()).await.unwrap();
        let url = listener.url();
        assert!(url.ends_with(IPC_PATH));

        let dialling = tokio::spawn(async move { connect(&url).await.unwrap() });
        let mut server_side = listener.accept().await.unwrap();
        let mut client_side = dialling.await.unwrap();

        let hello = ClientMessage::Hello {
            hello: Hello::new(ClientKind::WebRemote),
        };
        client_side.send_message(&hello).await.unwrap();
        assert_eq!(
            server_side
                .recv_message::<ClientMessage>()
                .await
                .unwrap()
                .unwrap(),
            hello
        );

        server_side.send(vec![9, 9]).await.unwrap();
        assert_eq!(client_side.recv().await, Some(Ok(vec![9, 9])));
    }

    #[tokio::test]
    async fn the_bound_port_is_reported_so_nothing_has_to_guess_one() {
        let listener = WebSocketListener::bind(loopback()).await.unwrap();
        let addr = listener.local_addr();
        assert_ne!(addr.port(), 0, "port 0 means 'choose one', not 'use zero'");
        assert!(addr.ip().is_loopback(), "§2.1: loopback is the default");
        assert_eq!(listener.endpoint(), Endpoint::WebSocket(addr));
        assert!(!listener.endpoint().needs_a_token());
    }

    #[tokio::test]
    async fn several_clients_are_served_at_once() {
        let mut listener = WebSocketListener::bind(loopback()).await.unwrap();
        let url = listener.url();

        let mut clients = Vec::new();
        for _ in 0..3 {
            let url = url.clone();
            clients.push(tokio::spawn(async move { connect(&url).await.unwrap() }));
        }
        let mut servers = Vec::new();
        for _ in 0..3 {
            servers.push(listener.accept().await.unwrap());
        }
        for (n, client) in clients.into_iter().enumerate() {
            let client = client.await.unwrap();
            client.send(vec![u8::try_from(n).unwrap()]).await.unwrap();
        }
        // Each server-side wire belongs to exactly one client, so every one of
        // them has something and none has two.
        let mut seen = Vec::new();
        for server in &mut servers {
            seen.push(server.recv().await.unwrap().unwrap());
        }
        seen.sort();
        assert_eq!(seen, vec![vec![0], vec![1], vec![2]]);
    }

    #[tokio::test]
    async fn connecting_to_nothing_fails_rather_than_waiting() {
        let error = connect("ws://127.0.0.1:1/ipc").await.unwrap_err();
        assert!(matches!(error, crate::WireError::Transport(_)));
    }

    #[tokio::test]
    async fn a_dropped_listener_stops_listening() {
        let listener = WebSocketListener::bind(loopback()).await.unwrap();
        let url = listener.url();
        drop(listener);
        // The port is released, so the connection is refused rather than hanging.
        assert!(connect(&url).await.is_err());
    }

    /// The WebSocket half of "an oversized frame closes the connection rather
    /// than allocating".
    ///
    /// On a byte stream the guard is the length prefix, and `tests/oversized_frame.rs`
    /// measures what refusing one costs in memory. Here the guard is the
    /// WebSocket implementation's own `max_message_size`, which refuses while
    /// reading rather than after — so what is left to assert is that the limit is
    /// actually configured, and that is what this does. The client below is
    /// deliberately built **without** a limit, because a client that shared the
    /// daemon's would refuse to send the message and the test would pass without
    /// the server having done anything.
    #[tokio::test]
    async fn a_message_larger_than_the_limit_closes_the_connection() {
        use futures_util::SinkExt as _;
        use tokio_tungstenite::tungstenite::Message;

        let mut listener = WebSocketListener::bind(loopback()).await.unwrap();
        let url = listener.url();

        let unlimited = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(None)
            .max_frame_size(None);
        let dialling = tokio::spawn(async move {
            let (socket, _) =
                tokio_tungstenite::connect_async_with_config(url, Some(unlimited), false)
                    .await
                    .unwrap();
            socket
        });
        let mut server_side = listener.accept().await.unwrap();
        let mut client_side = dialling.await.unwrap();

        let too_much = vec![0_u8; crate::MAX_FRAME_BYTES + 1];
        // The send itself may fail once the peer has already hung up, which is
        // the same answer by a different route.
        let _ = client_side.send(Message::Binary(too_much.into())).await;

        // Whatever happens, the daemon must not have received a message a
        // megabyte and a byte long.
        match server_side.recv().await {
            None => {}
            Some(Err(_)) => {}
            Some(Ok(payload)) => panic!(
                "the daemon accepted {} bytes, and the limit is {}",
                payload.len(),
                crate::MAX_FRAME_BYTES
            ),
        }
    }

    /// This protocol is binary. A browser calling `socket.send("hello")` is an
    /// ordinary mistake rather than an attack, and it gets an answer that says
    /// which mistake it was.
    #[tokio::test]
    async fn a_text_message_is_not_carried() {
        use futures_util::SinkExt as _;
        use tokio_tungstenite::tungstenite::Message;

        let mut listener = WebSocketListener::bind(loopback()).await.unwrap();
        let url = listener.url();
        let dialling = tokio::spawn(async move {
            let (socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
            socket
        });
        let mut server_side = listener.accept().await.unwrap();
        let mut client_side = dialling.await.unwrap();

        client_side
            .send(Message::Text("hello".into()))
            .await
            .unwrap();

        let reported = server_side.recv().await;
        let Some(Err(crate::WireError::Transport(why))) = reported else {
            panic!("a text frame should have been refused, not {reported:?}");
        };
        assert!(why.contains("binary"), "{why}");
        assert_eq!(server_side.recv().await, None, "and the connection ends");
    }

    #[tokio::test]
    async fn closing_the_client_end_ends_the_server_end() {
        let mut listener = WebSocketListener::bind(loopback()).await.unwrap();
        let url = listener.url();
        let dialling = tokio::spawn(async move { connect(&url).await.unwrap() });
        let mut server_side = listener.accept().await.unwrap();
        let client_side = dialling.await.unwrap();

        client_side.shutdown().await;
        assert_eq!(server_side.recv().await, None);
    }
}
