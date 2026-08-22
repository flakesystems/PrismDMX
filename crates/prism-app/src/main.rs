//! The PrismDMX desktop shell.
//!
//! Deliberately thin: it hosts the web UI and connects to `prismd`. It holds no
//! show state and no session state, so closing it leaves the show running
//! (decision D9, `ARCHITECTURE_SPEC.md` section 10.3).
//!
//! This crate is one of only four permitted to contain `#[cfg(target_os = ...)]`,
//! for the autostart integration. The others are `prism-protocols` (FTDI backend
//! selection), `prism-ipc`'s `transport/local.rs` (S16) and `prism-midi`'s
//! `system.rs` (S36).
//!
//! Wired up in session **S29**.

fn main() {
    // S29: find or spawn prismd via the lock file, then run the Tauri shell.
}
