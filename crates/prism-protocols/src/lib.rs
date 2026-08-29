//! DMX outputs and tracker input.
//!
//! Implements the `DmxOutput` trait for Open DMX USB (FTDI FT232R, e.g. the
//! DSD TECH SH-RS09B), ArtNet and sACN, plus PSN/OSC position input.
//! See `ARCHITECTURE_SPEC.md` sections 7-8.
//!
//! Every driver runs on its own thread with `catch_unwind` and reconnect
//! backoff: a USB adapter being unplugged mid-show is the expected case, not an
//! exception.
//!
//! This is one of only two crates permitted to contain `#[cfg(target_os = ...)]`
//! (see `ARCHITECTURE_SPEC.md` section 10.1) - the FTDI backend differs by
//! platform: D2XX on Windows, libftdi on Linux.
//!
//! Sessions **S7-S10**.
//!
//! # Shape
//!
//! ```text
//!   prism-engine                       prism-protocols
//!   ────────────                       ───────────────
//!   FramePublisher ──▶ [`FrameSubscriber`] ──▶ [`OutputRunner`] ──▶ [`DmxOutput`]
//!        44 Hz          triple buffer,          own cadence,          ├── [`OpenDmxUsb`]
//!                       wait-free               catch_unwind,         │      │
//!                                               reconnect backoff     │      ▼
//!                                                                     │  [`FtdiBackend`]
//!                                                                     │  D2XX / libftdi
//!                                                                     │  / [`MockFtdi`]
//!                                                                     ├── [`ArtNetOutput`]
//!                                                                     └── [`SacnOutput`]
//!                                                                            │
//!                                                                            ▼
//!                                                                        [`UdpSender`]
//!                                                                        [`SystemUdp`]
//!                                                                        / [`MockUdp`]
//! ```
//!
//! Since **S46** there is one thread in this crate that is not an output at all:
//!
//! ```text
//!                          ┌─────────────────┐   ArtPoll   ┌──────┐
//!   the configured rig ───▶│ [`NodeDiscovery`]│ ──────────▶ │ node │
//!                          │  own thread     │ ◀ArtPollReply└──────┘
//!                          └─────────────────┘
//!                                   │ [`UdpNode`]
//!                                   ▼  [`SystemUdpNode`] / [`MockUdpNode`]
//! ```
//!
//! It is a **second seam** rather than a `recv` on [`UdpSender`], because an
//! output's thread must never be handed a call that can block — see `udp.rs`.
//!
//! The engine publishes at a fixed 44 Hz and never waits for an output. An
//! output reads the *most recent* frame at whatever rate its hardware allows
//! and re-sends it when the engine has published nothing new — a DMX line has
//! to keep being driven, and `ARCHITECTURE_SPEC.md` §3.2 says a slow adapter
//! must fall behind rather than hold the show up.
//!
//! # Nothing here needs hardware
//!
//! `CLAUDE.md` requires every test to run deterministically with no device
//! attached, and this is the crate where that is hardest and matters most. It
//! is arranged so that the only code that touches USB is a [`FtdiBackend`]
//! implementation:
//!
//! - [`OpenDmxUsb`] holds the DMX512 knowledge — the port parameters, the break
//!   and mark-after-break, the 513-byte packet — and talks to a backend.
//! - [`MockFtdi`] is a backend that records the call sequence and fails
//!   whenever a test tells it to, including in the middle of a frame.
//! - [`MockOutput`] is the same idea one level up: a whole `DmxOutput` for
//!   driving the daemon headlessly.
//! - [`OutputRunner`] is the thread body, and it is stepped against a
//!   `prism_engine::Clock`, so a reconnect backoff of 100 ms → 5 s is asserted
//!   in microseconds of real time rather than waited out.
//!
//! There is deliberately **no timing measurement** in this crate's tests. Every
//! assertion about time here is a floor ("this did not return early") or is
//! taken on a simulated clock; the jitter gates live in `prism-engine`, behind
//! the mutex that stops two timing runs measuring each other.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::integer_division,
    )
)]

mod artnet;
mod artpoll;
#[cfg(windows)]
mod d2xx;
mod device;
mod discovery;
mod ftdi;
mod opendmx;
mod output;
mod runner;
mod sacn;
mod system;
mod udp;
#[cfg(windows)]
mod vcp;

pub use artnet::{
    ART_DMX_BYTES, ART_DMX_HEADER, ART_NET_ID, ART_NET_PORT, ART_SYNC_BYTES, ArtNetConfig,
    ArtNetOutput, Destination, OP_DMX, OP_SYNC, PROTOCOL_VERSION, PortAddress, art_sync,
    write_art_dmx,
};
pub use artpoll::{
    ART_POLL_BYTES, ART_POLL_REPLY_BYTES, ART_POLL_REPLY_MIN, ArtPollReply, LONG_NAME_BYTES,
    MAX_NODE_PORTS, OP_POLL, OP_POLL_REPLY, POLL_FLAG_REPLY_ON_CHANGE, SHORT_NAME_BYTES, art_poll,
    parse_art_poll_reply,
};
#[cfg(windows)]
pub use d2xx::D2xxBackend;
pub use device::{
    AccessPath, AttachedDevice, DeviceDescriptor, DeviceProfile, DmxTiming, SH_RS09B,
};
pub use discovery::{
    DiscoveredNode, DiscoveryConfig, DiscoveryCounters, NODE_TEXT_BYTES, NodeDiscovery,
    RECV_BUFFER_BYTES,
};
pub use ftdi::{
    BITS_PER_SLOT, FlowControl, FtdiBackend, FtdiCall, FtdiError, MockFtdi, MockFtdiHandle, Parity,
    PortConfig, StopBits, spin_wait, transmission_time,
};
pub use opendmx::{DMX_PACKET_BYTES, OpenDmxUsb, START_CODE};
pub use output::{DmxOutput, FrameRecord, MockOutput, MockOutputHandle, OutputError};
pub use runner::{
    Backoff, BackoffConfig, OutputFault, OutputRunner, OutputStatus, OutputThread, RunnerConfig,
    StepOutcome, spawn,
};
pub use sacn::{
    ACN_PACKET_IDENTIFIER, Cid, DMP_PDU_BYTES, E131_DATA_BYTES, E131_DATA_HEADER, E131_PORT,
    E131Header, FRAMING_PDU_BYTES, OPTION_FORCE_SYNCHRONIZATION, OPTION_PREVIEW_DATA,
    OPTION_STREAM_TERMINATED, Priority, ROOT_PDU_BYTES, SOURCE_NAME_BYTES, SacnConfig,
    SacnDestination, SacnOutput, SacnPort, SacnUniverse, TERMINATION_PACKETS,
    VECTOR_DMP_SET_PROPERTY, VECTOR_E131_DATA_PACKET, VECTOR_ROOT_E131_DATA, flags_and_length,
    source_name_field, write_e131_data,
};
pub use system::{FallbackFtdi, UnsupportedBackend, list_devices, system_backend};
pub use udp::{
    MockUdp, MockUdpHandle, MockUdpNode, MockUdpNodeHandle, SystemUdp, SystemUdpNode, UdpError,
    UdpNode, UdpSender, classify,
};
#[cfg(windows)]
pub use vcp::VcpBackend;
