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
