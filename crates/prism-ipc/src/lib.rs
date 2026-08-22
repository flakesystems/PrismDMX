//! Daemon-to-client protocol: framing, transports, server and client halves.
//!
//! Length-prefixed MessagePack over a named pipe or Unix domain socket for the
//! local shell, and over WebSocket for the Web Remote. Both transports carry
//! identical messages - the choice is invisible above this crate.
//!
//! Specified in `docs/IPC_PROTOCOL.md`. Sessions **S16**.
//!
//! The protocol is deliberately asymmetric: clients send intent, the daemon
//! sends facts. No client computes state and expects the daemon to accept it.
//!
//! # Shape
//!
//! ```text
//!   client                        prism-ipc                        prismd
//!   ──────                        ─────────                        ──────
//!   [`Client`] ──[`ClientMessage`]──▶ [`Wire`] ──▶ [`Server`] ──▶ [`ServerHandler`]
//!        ▲                             │  ▲            │                │
//!        └────[`ServerMessage`]────────┘  │            └──[`Outbound`]──┘
//!                                         │                 control never
//!                            ┌────────────┴────────────┐    dropped,
//!                            │                         │    telemetry
//!                     [`local`] pipe/UDS         [`websocket`]  coalesced
//!                            │                         │
//!                            └────── [`memory`] ───────┘
//!                                  (the tests' transport)
//! ```
//!
//! # The transport disappears at [`Wire`]
//!
//! A named pipe, a Unix domain socket and a WebSocket have three different
//! types and three different error vocabularies, and exactly one thing in common:
//! each can carry a sequence of byte payloads in both directions. [`Wire`] is
//! that common thing — a pair of channels and a pump task that owns the socket.
//! Every transport module's whole job is to produce one, and nothing above this
//! line knows which of them did.
//!
//! This is the same move `prism-protocols` made for `DmxOutput` in S7, one layer
//! further down: the abstraction is chosen so the *tests* can implement it too.
//! [`memory::pair`] is a `Wire` over `tokio::io::duplex`, which makes the
//! handshake, the backpressure policy and every error path testable with no
//! socket, no port and no file system.
//!
//! # Platform code, and why it is allowed here
//!
//! `ARCHITECTURE_SPEC.md` §10.1 confines `#[cfg(target_os = …)]` to
//! `prism-protocols`, `prism-app` and — since S36 — `prism-midi`, and a named
//! pipe on Windows beside a Unix
//! domain socket on Linux is exactly the kind of split that rule exists to
//! contain. It is contained: the two `#[cfg]`s in this crate are both in
//! [`local`], both select a listener and a connector type, and everything above
//! them — the framing, the handshake, the backpressure policy, the server and
//! the client — is one code path on every target. `PROGRESS.md`'s decision log
//! records the reasoning.
//!
//! # What is bounded, and why
//!
//! Everything a peer can make this process allocate is bounded before it is
//! allocated:
//!
//! - [`MAX_FRAME_BYTES`] is checked against the length prefix, so an oversized
//!   frame costs four bytes and a disconnection.
//! - [`MAX_NESTING_DEPTH`] is checked by walking the payload with an explicit
//!   stack, because a stack overflow would abort the process — and this process
//!   is the one holding the DMX output up.
//! - [`Outbound`] bounds the queue behind a slow client: telemetry is coalesced
//!   and then dropped, commands and deltas never are, and a client whose control
//!   queue fills is disconnected with a `Reject` rather than being allowed to
//!   consume the daemon's memory (`docs/IPC_PROTOCOL.md` §8).

#![forbid(unsafe_code)]

pub mod backpressure;
pub mod client;
pub mod frame;
pub mod message;
mod scan;
pub mod server;
pub mod telemetry;
pub mod transport;

pub use backpressure::{Outbound, OutboundFull, OutboundStats};
pub use client::{Client, ClientError, ClientEvent};
pub use frame::{
    FrameError, LENGTH_PREFIX_BYTES, MAX_FRAME_BYTES, MAX_NESTING_DEPTH, decode, encode,
    encode_frame, payload_length,
};
pub use message::{
    ClientKind, ClientMessage, DaemonHealth, Hello, OutputSnapshot, PROTOCOL_VERSION, RejectReason,
    ServerMessage, Snapshot,
};
pub use scan::ScanFault;
pub use server::{ClientId, CommandOutcome, Server, ServerConfig, ServerHandle, ServerHandler};
pub use telemetry::{TELEMETRY_HEADER_BYTES, TELEMETRY_VERSION, TelemetryError, TelemetryFrame};
pub use transport::{Endpoint, Wire, WireError, local, memory, websocket};
