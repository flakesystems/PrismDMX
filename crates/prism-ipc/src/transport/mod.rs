//! The transports of `docs/IPC_PROTOCOL.md` §2, and the thing they all become.
//!
//! | Transport | Used by | Module |
//! |---|---|---|
//! | Named pipe (Windows) / Unix domain socket (Linux, macOS) | the desktop shell | [`local`] |
//! | WebSocket over `axum` | the Web Remote and remote clients | [`websocket`] |
//! | An in-process duplex | every test in this crate | [`memory`] |
//!
//! # [`Wire`] is the abstraction, and it is a pair of channels
//!
//! S7 hid three DMX drivers behind a five-method `DmxOutput` trait. The same
//! question here has a different answer, because the three things being hidden
//! are not three implementations of one operation — they are two byte streams
//! and a message stream, in an async world where a trait with `async fn` is not
//! object-safe.
//!
//! So the abstraction is a **value**, not a trait. Each transport module spawns
//! a pump that owns its socket and speaks its own error vocabulary, and hands
//! back a [`Wire`]: payloads out through one channel, payloads in through
//! another. Above this line there is one code path. The parity criterion is not
//! satisfied by running two similar suites — it is satisfied because
//! [`crate::Server`] and [`crate::Client`] cannot tell which transport they are
//! on, and `tests/transport_parity.rs` runs one suite over three of them.
//!
//! # The framing is not identical, and that is deliberate
//!
//! §3 specifies `u32` little-endian length, then MessagePack, and says both
//! transports carry identical framing. The **payload** is identical, byte for
//! byte, and `tests/transport_parity.rs` asserts exactly that. The length prefix
//! is not, and putting one inside a WebSocket binary message would be a second
//! copy of a number the WebSocket frame header already carries — two lengths
//! that can disagree, in a protocol whose whole purpose is that neither end
//! guesses.
//!
//! What is genuinely shared is the limit. [`crate::MAX_FRAME_BYTES`] bounds both:
//! on a byte stream it is checked against the length prefix before a body buffer
//! exists, and on a WebSocket it is handed to the WebSocket implementation,
//! which enforces it while reading rather than after. Neither can be made to
//! allocate a megabyte by a peer that says it will send one.

pub mod local;
pub mod memory;
pub mod stream;
pub mod websocket;

use core::fmt;
use std::net::SocketAddr;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, oneshot};

use crate::frame::{FrameError, decode, encode};

/// Payloads buffered in each direction between the pump and its user.
///
/// One outbound, so that a [`Wire::ready`] permit means *the transport will take
/// this now* rather than *there is room in a buffer*. Backpressure is decided by
/// [`crate::Outbound`], one layer up, and it can only decide it if the layer
/// below is honest about being busy.
const OUTBOUND_CAPACITY: usize = 1;

/// Inbound payloads buffered before the reader stops reading.
///
/// More than one, because a client is entitled to pipeline commands and §8
/// forbids dropping any of them; small, because the socket's own buffer is the
/// right place for a backlog.
const INBOUND_CAPACITY: usize = 8;

/// How long [`WireSender::shutdown`] waits for the socket to take what is
/// already queued.
///
/// A second is long by the standards of a loopback socket and short by the
/// standards of a person noticing. See [`WireSender::shutdown_within`] for why
/// it is bounded at all.
pub const FLUSH_DEADLINE: std::time::Duration = std::time::Duration::from_secs(1);

/// Where a daemon listens, or a client connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// A named pipe on Windows, a Unix domain socket path elsewhere. The string
    /// is the operating system's own name for it — `prismd` writes it into the
    /// lock file of §2.2 and clients read it from there.
    Local(String),
    /// A TCP address for the WebSocket listener. Loopback unless the operator
    /// has explicitly enabled LAN access (§2.1).
    WebSocket(SocketAddr),
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(name) => write!(f, "{name}"),
            Self::WebSocket(addr) => write!(f, "ws://{addr}/ipc"),
        }
    }
}

impl Endpoint {
    /// Whether this endpoint is reachable from another machine.
    ///
    /// §2.1: the WebSocket listener binds loopback unless the user explicitly
    /// enables LAN access, and enabling it requires a token. A local transport
    /// is never reachable off the machine, so it never needs one.
    #[must_use]
    pub fn needs_a_token(&self) -> bool {
        match self {
            Self::Local(_) => false,
            Self::WebSocket(addr) => !addr.ip().is_loopback(),
        }
    }
}

/// Why a wire failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// The socket failed.
    Io(String),
    /// The bytes were not a frame this protocol accepts. One that has lost the
    /// frame boundary closes the connection — see
    /// [`FrameError::loses_the_frame_boundary`].
    Frame(FrameError),
    /// The transport itself objected: a WebSocket protocol violation, or a text
    /// message where only binary is carried.
    Transport(String),
    /// The other end has gone.
    Closed,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(why) => write!(f, "the connection failed: {why}"),
            Self::Frame(error) => fmt::Display::fmt(error, f),
            Self::Transport(why) => write!(f, "the transport refused the connection: {why}"),
            Self::Closed => f.write_str("the connection is closed"),
        }
    }
}

impl core::error::Error for WireError {}

impl From<FrameError> for WireError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

/// One end of a connection, with the transport removed.
///
/// Carries byte payloads. What is inside them is [`crate::ClientMessage`] or
/// [`crate::ServerMessage`], and neither this type nor any transport module
/// knows which.
#[derive(Debug)]
pub struct Wire {
    sender: WireSender,
    receiver: WireReceiver,
}

/// The writing half of a [`Wire`].
///
/// Separate from the reading half because both halves are needed at once: a
/// connection waits for something to arrive and for the transport to become
/// free to send, in the same `select!`. One value holding both could only be
/// borrowed for one of those at a time.
#[derive(Debug)]
pub struct WireSender {
    outbound: mpsc::Sender<Vec<u8>>,
    /// Completes when the writing half has drained and closed the socket, so a
    /// final `Reject` can be known to have gone out before the connection ends.
    flushed: Option<oneshot::Receiver<()>>,
}

/// The reading half of a [`Wire`].
#[derive(Debug)]
pub struct WireReceiver {
    inbound: mpsc::Receiver<Result<Vec<u8>, WireError>>,
}

/// The pump's half of a [`Wire`]: what a transport module writes into and reads
/// out of.
#[derive(Debug)]
pub struct WirePump {
    /// Payloads the user asked to send.
    pub outbound: mpsc::Receiver<Vec<u8>>,
    /// Where payloads read off the socket go.
    pub inbound: mpsc::Sender<Result<Vec<u8>, WireError>>,
    /// Signalled once the writing half has finished.
    pub flushed: oneshot::Sender<()>,
}

/// Reserved room on the transport for exactly one payload.
#[derive(Debug)]
pub struct WirePermit<'a>(mpsc::Permit<'a, Vec<u8>>);

impl WirePermit<'_> {
    /// Sends the payload into the room this permit reserved.
    pub fn send(self, payload: Vec<u8>) {
        self.0.send(payload);
    }
}

impl Wire {
    /// Builds the two halves: one for the caller, one for the transport's pump.
    #[must_use]
    pub fn pair() -> (Self, WirePump) {
        let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);
        let (inbound_tx, inbound_rx) = mpsc::channel(INBOUND_CAPACITY);
        let (flushed_tx, flushed_rx) = oneshot::channel();
        (
            Self {
                sender: WireSender {
                    outbound: outbound_tx,
                    flushed: Some(flushed_rx),
                },
                receiver: WireReceiver {
                    inbound: inbound_rx,
                },
            },
            WirePump {
                outbound: outbound_rx,
                inbound: inbound_tx,
                flushed: flushed_tx,
            },
        )
    }

    /// The two halves, for a caller that sends and receives at the same time.
    #[must_use]
    pub fn split(self) -> (WireSender, WireReceiver) {
        (self.sender, self.receiver)
    }

    /// Sends one payload, waiting until the transport can take it.
    ///
    /// # Errors
    ///
    /// [`WireError::Closed`] if the connection has gone.
    pub async fn send(&self, payload: Vec<u8>) -> Result<(), WireError> {
        self.sender.send(payload).await
    }

    /// Waits until the transport can take a payload, without deciding yet what
    /// to send.
    ///
    /// # Errors
    ///
    /// [`WireError::Closed`] if the connection has gone.
    pub async fn ready(&self) -> Result<WirePermit<'_>, WireError> {
        self.sender.ready().await
    }

    /// The next payload, or `None` when the connection has closed cleanly.
    pub async fn recv(&mut self) -> Option<Result<Vec<u8>, WireError>> {
        self.receiver.recv().await
    }

    /// Encodes and sends one message.
    ///
    /// # Errors
    ///
    /// As [`WireSender::send_message`].
    pub async fn send_message<T: Serialize + ?Sized>(&self, value: &T) -> Result<(), WireError> {
        self.sender.send_message(value).await
    }

    /// The next message, decoded.
    ///
    /// `None` at a clean close. A decode failure is reported rather than
    /// skipped: the daemon has to answer for a message it could not read.
    pub async fn recv_message<T: DeserializeOwned>(&mut self) -> Option<Result<T, WireError>> {
        self.receiver.recv_message().await
    }

    /// Closes the connection, having waited for everything already sent to reach
    /// the socket.
    ///
    /// The difference from simply dropping the wire is the last message: §8 ends
    /// a connection with a `Reject`, and a client that never received it would
    /// reconnect without knowing why it was disconnected.
    pub async fn shutdown(self) {
        drop(self.receiver);
        self.sender.shutdown().await;
    }
}

impl WireSender {
    /// Sends one payload, waiting until the transport can take it.
    ///
    /// # Errors
    ///
    /// [`WireError::Closed`] if the connection has gone.
    pub async fn send(&self, payload: Vec<u8>) -> Result<(), WireError> {
        self.outbound
            .send(payload)
            .await
            .map_err(|_| WireError::Closed)
    }

    /// Waits until the transport can take a payload, without deciding yet what
    /// to send.
    ///
    /// This is what lets [`crate::Server`] hold its own queue: it waits for the
    /// wire and for new work at the same time, and only decides which message
    /// goes next at the moment the wire is actually free. Deciding earlier is
    /// how a queue ends up sending a delta after the `Reject` that ended the
    /// connection.
    ///
    /// # Errors
    ///
    /// [`WireError::Closed`] if the connection has gone.
    pub async fn ready(&self) -> Result<WirePermit<'_>, WireError> {
        self.outbound
            .reserve()
            .await
            .map(WirePermit)
            .map_err(|_| WireError::Closed)
    }

    /// Encodes and sends one message.
    ///
    /// # Errors
    ///
    /// [`WireError::Frame`] if the value cannot be encoded — S1's non-finite
    /// float, which is refused here rather than travelling — and
    /// [`WireError::Closed`] if the connection has gone.
    pub async fn send_message<T: Serialize + ?Sized>(&self, value: &T) -> Result<(), WireError> {
        self.send(encode(value)?).await
    }

    /// Closes the writing half, having waited up to [`FLUSH_DEADLINE`] for
    /// everything already sent to reach the socket.
    pub async fn shutdown(self) {
        self.shutdown_within(FLUSH_DEADLINE).await;
    }

    /// Closes the writing half, waiting at most `deadline` for the socket to
    /// take what is already queued.
    ///
    /// **The wait has to be bounded, and the reason is the case it exists for.**
    /// Flushing matters because §8 ends a connection with a `Reject` and a client
    /// that never receives it reconnects without knowing why. But the commonest
    /// reason to send one is that the client stopped reading — and waiting for a
    /// client that has stopped reading to read is waiting for ever, in a task
    /// that is still holding a socket and a place in the daemon's client list.
    pub async fn shutdown_within(mut self, deadline: std::time::Duration) {
        let flushed = self.flushed.take();
        drop(self.outbound);
        if let Some(flushed) = flushed {
            let _ = tokio::time::timeout(deadline, flushed).await;
        }
    }
}

impl WireReceiver {
    /// The next payload, or `None` when the connection has closed cleanly.
    pub async fn recv(&mut self) -> Option<Result<Vec<u8>, WireError>> {
        self.inbound.recv().await
    }

    /// The next message, decoded.
    pub async fn recv_message<T: DeserializeOwned>(&mut self) -> Option<Result<T, WireError>> {
        match self.recv().await? {
            Ok(payload) => Some(decode(&payload).map_err(WireError::Frame)),
            Err(error) => Some(Err(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Endpoint, Wire, WireError};
    use crate::frame::FrameError;
    use crate::scan::ScanFault;
    use prism_domain::Command;
    use std::net::{Ipv4Addr, SocketAddr};

    #[test]
    fn an_endpoint_says_where_it_is() {
        assert_eq!(
            Endpoint::Local(r"\\.\pipe\prismd".to_owned()).to_string(),
            r"\\.\pipe\prismd"
        );
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 7333));
        assert_eq!(
            Endpoint::WebSocket(addr).to_string(),
            "ws://127.0.0.1:7333/ipc"
        );
    }

    /// §2.1: loopback is the default and needs no token; anything else does.
    #[test]
    fn only_a_listener_reachable_from_another_machine_needs_a_token() {
        assert!(!Endpoint::Local("/tmp/prismd.sock".to_owned()).needs_a_token());
        assert!(!Endpoint::WebSocket(SocketAddr::from((Ipv4Addr::LOCALHOST, 1))).needs_a_token());
        assert!(
            Endpoint::WebSocket(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 1))).needs_a_token(),
            "0.0.0.0 is every interface, which is the case the token exists for"
        );
        assert!(Endpoint::WebSocket(SocketAddr::from(([192, 168, 1, 20], 1))).needs_a_token());
    }

    #[tokio::test]
    async fn a_wire_carries_payloads_between_its_two_halves() {
        let (wire, mut pump) = Wire::pair();
        wire.send(vec![1, 2, 3]).await.unwrap();
        assert_eq!(pump.outbound.recv().await, Some(vec![1, 2, 3]));

        pump.inbound.send(Ok(vec![4, 5])).await.unwrap();
        let mut wire = wire;
        assert_eq!(wire.recv().await, Some(Ok(vec![4, 5])));
    }

    #[tokio::test]
    async fn a_message_is_encoded_on_the_way_out_and_decoded_on_the_way_back() {
        let (mut wire, mut pump) = Wire::pair();
        wire.send_message(&Command::ClearProgrammer).await.unwrap();
        let payload = pump.outbound.recv().await.unwrap();
        pump.inbound.send(Ok(payload)).await.unwrap();
        assert_eq!(
            wire.recv_message::<Command>().await.unwrap().unwrap(),
            Command::ClearProgrammer
        );
    }

    #[tokio::test]
    async fn a_wire_whose_pump_has_gone_reports_it_rather_than_hanging() {
        let (wire, pump) = Wire::pair();
        drop(pump);
        assert_eq!(wire.send(vec![1]).await, Err(WireError::Closed));
        assert!(matches!(wire.ready().await, Err(WireError::Closed)));

        let (mut wire, pump) = Wire::pair();
        drop(pump);
        assert_eq!(wire.recv().await, None);
        assert_eq!(wire.recv_message::<Command>().await, None);
    }

    #[tokio::test]
    async fn a_permit_is_room_that_has_already_been_reserved() {
        let (wire, mut pump) = Wire::pair();
        let permit = wire.ready().await.unwrap();
        permit.send(vec![9]);
        assert_eq!(pump.outbound.recv().await, Some(vec![9]));
    }

    #[tokio::test]
    async fn a_payload_that_is_not_the_message_is_reported_rather_than_skipped() {
        let (mut wire, pump) = Wire::pair();
        pump.inbound.send(Ok(vec![0xc1])).await.unwrap();
        assert_eq!(
            wire.recv_message::<Command>().await,
            Some(Err(WireError::Frame(FrameError::Malformed(
                ScanFault::ReservedByte
            ))))
        );
    }

    #[tokio::test]
    async fn a_message_that_cannot_be_encoded_never_reaches_the_socket() {
        let (wire, mut pump) = Wire::pair();
        let broken = prism_domain::JsonValue::Float(f64::NAN);
        assert!(matches!(
            wire.send_message(&broken).await,
            Err(WireError::Frame(FrameError::Encode(_)))
        ));
        drop(wire);
        assert_eq!(pump.outbound.recv().await, None);
    }

    #[tokio::test]
    async fn a_shutdown_waits_for_the_writing_half() {
        let (wire, pump) = Wire::pair();
        let flushed = pump.flushed;
        let writer = tokio::spawn(async move {
            let mut outbound = pump.outbound;
            while outbound.recv().await.is_some() {}
            let _ = flushed.send(());
        });
        wire.send(vec![1]).await.unwrap();
        wire.shutdown().await;
        writer.await.unwrap();
    }

    #[test]
    fn every_wire_error_says_what_went_wrong() {
        for error in [
            WireError::Io("broken pipe".to_owned()),
            WireError::Frame(FrameError::TooDeep { limit: 128 }),
            WireError::Transport("text frames are not carried".to_owned()),
            WireError::Closed,
        ] {
            assert!(error.to_string().len() > 15, "{error:?}");
        }
        assert_eq!(
            WireError::from(FrameError::TooDeep { limit: 1 }),
            WireError::Frame(FrameError::TooDeep { limit: 1 })
        );
    }
}
