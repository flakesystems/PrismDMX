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
