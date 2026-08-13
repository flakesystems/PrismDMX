//! The PrismDMX engine daemon, as a library.
//!
//! `prismd` is a process, and this is everything that process is made of. The
//! binary beside it reads a command line and calls [`daemon::Daemon::start`];
//! nothing else lives there, because a binary target has no tests and every
//! exit criterion this session was set is a statement about a *running* daemon:
//! that it drives DMX with no client connected, that a second instance refuses
//! to start, that a stale lock file is taken over, that the handshake serves the
//! world.
//!
//! Session **S17**.
//!
//! # Shape
//!
//! ```text
//!   [`cli`] ──▶ [`daemon::Daemon`] ──┬──▶ [`lock`]     one daemon, and the file
//!                                    │                 clients find it by
//!                                    ├──▶ [`machine`]  the desk's sACN identity
//!                                    ├──▶ [`core`]     ShowFile + ShowStore,
//!                                    │        │        and the effects that follow
//!                                    │        ▼
//!                                    ├──▶ [`engine`]   the 44 Hz thread,
//!                                    │                 at its own priority
//!                                    ├──▶ [`server`]   the ServerHandler
//!                                    │        │        prism-ipc asked for
//!                                    │        ▼
//!                                    │   clients, or none
//!                                    └──▶ [`surface`]  the X-Touch, whose
//!                                                      presses become the same
//!                                                      commands a client sends
//! ```
//!
//! # What this crate is not allowed to contain
//!
//! `ARCHITECTURE_SPEC.md` §10.1 confines `#[cfg(target_os = …)]` to
//! `prism-protocols`, `prism-app` and `prism-ipc`'s `transport/local.rs`. It
//! does not name this crate, so there is none here — and the three places it
//! would otherwise have been are worth naming, because each was solved rather
//! than avoided:
//!
//! - **The user data directory** is resolved from the environment
//!   ([`paths`]), which is what the platform conventions are actually written
//!   in.
//! - **The tick thread's priority** is `thread_priority`'s, a dependency whose
//!   whole purpose is to hold that split ([`engine`]).
//! - **The IPC endpoint** — a named pipe or a Unix domain socket — is
//!   `prism_ipc::local::daemon_address`, in the module §10.1 already exempts.
//!
//! # The order everything happens in
//!
//! Written out in [`daemon`], and it is not arbitrary: the lock before anything
//! else, the desk identity before any output, every output attached before the
//! tick starts, and the listeners' addresses published only once they are bound.

pub mod cli;
pub mod core;
pub mod daemon;
pub mod engine;
pub mod lock;
pub mod log;
pub mod machine;
pub mod paths;
pub mod server;
pub mod surface;
#[cfg(test)]
mod testkit;
