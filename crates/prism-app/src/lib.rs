//! The PrismDMX desktop shell.
//!
//! Deliberately thin: it hosts the web UI, it finds or starts `prismd`, and it
//! holds no show state and no session state — so closing it leaves the show
//! running. That is decision **D9**, and `ARCHITECTURE_SPEC.md` §10.3 is where
//! the whole of this crate's behaviour is specified.
//!
//! # What is here, and why almost none of it needs a window
//!
//! Four questions, and every one of them is arithmetic:
//!
//! - [`attach`] — *is a desk already running here, and can this window reach
//!   it?* The mechanism is S17's guard file and the discovery document beside
//!   it, and this crate is its first real caller.
//! - [`autostart`] — *does the start-up entry agree with the switch the
//!   settings window writes?* The decision is a three-way comparison; only the
//!   writing of it is platform code.
//! - [`dialogs`] — *what does the operating system's file dialogue open with,
//!   for each path an operator can type?* A table.
//! - [`spawn`] — *what arguments does a daemon this shell starts get, and where
//!   is its executable?*
//!
//! Each of those is a function a test calls with no Tauri anywhere, which is
//! `CLAUDE.md`'s rule stated for a shell: **no test may need a window.** What
//! is genuinely left to the window — that a tray menu item is clickable, that a
//! native dialogue returns what was picked — is a row in `ARCHITECTURE_SPEC.md`
//! §14 with the recipe, exactly as S20 and S36 left theirs.
//!
//! # This is one of only four crates allowed platform code
//!
//! `ARCHITECTURE_SPEC.md` §10.1 names it, for the autostart integration: a
//! `HKCU\…\Run` value is a Windows API and there is no portable spelling of it.
//! It is confined to [`autostart::entry`], and everything above the seam — the
//! comparison, the report a settings panel reads, the command line an entry
//! carries — is one code path on every target.
//!
//! Wired up in session **S29**.

#![forbid(unsafe_code)]

pub mod attach;
pub mod autostart;
pub mod dialogs;
pub mod shell;
pub mod spawn;

/// The version this build of the shell is, taken from `Cargo.toml`.
///
/// **One version, in one place** — S29's decision, and the reason
/// `tauri.conf.json` carries no `version` key of its own: the Tauri bundler
/// falls back to the crate's version when the configuration does not name one,
/// so the installer, the executable's resource block and `prismd --version` are
/// three readings of `[workspace.package] version` rather than three numbers
/// that can disagree. `tests/version.rs` is what holds the rest of the tree to
/// it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
