//! Show model, session state and persistence - the single source of truth.
//!
//! Holds everything the daemon is authoritative about: the patch, sequences,
//! presets, the programmer, the Oops journal, and the session state that lets
//! the X-Touch operate the UI (decision D11, `ARCHITECTURE_SPEC.md` section 4).
//!
//! Platform-neutral: no `#[cfg(target_os = ...)]` here.
//!
//! Sessions **S11-S15**.
//!
//! # What S11 delivers
//!
//! - [`Show`] - the patch, the embedded fixture types, groups, presets,
//!   sequences and executors, with one validated operation per edit. Every
//!   operation answers with the [`prism_domain::JsonPatchOp`]s that describe
//!   what it changed, so a client's mirror can follow without re-sending the
//!   show.
//! - [`Show::apply`] - command validation and application. Every command in the
//!   show group of `docs/IPC_PROTOCOL.md` section 5 is applied or rejected, and
//!   a rejection leaves the show byte-identical.
//! - [`PatchConflict`] and [`ShowIssue`] - overlapping addresses and dangling
//!   references, found and reported rather than refused. Patching a second
//!   fixture onto the first is how an operator clones one; the engine (S4)
//!   makes the result deterministic, so the show model's job is to say so, not
//!   to forbid it.
//! - [`ShowMirror`] - the other end of the delta: a JSON Patch applier that
//!   turns a stream of deltas back into the state they were generated from.
//!   Used by the tests that hold delta generation to that promise, and
//!   available to any client written in Rust.
//! - [`MachineConfig`] - what belongs to *this desk* rather than to the show.
//!   The sACN CID lives here, and the reason is in the module documentation.
//!
//! # What lives elsewhere
//!
//! The programmer state machine is **S13**, the Oops journal **S14**, session
//! state **S12** and persistence **S15**. The show model is what all four sit
//! on: it validates, it applies, and it says what changed.

mod command;
mod conflict;
mod desk;
mod mirror;
mod show;
#[cfg(test)]
mod testkit;

pub use command::{Applied, Effect, show_patch_ops};
pub use conflict::{PatchConflict, ShowIssue};
pub use desk::{DeskId, InvalidDeskId, MachineConfig};
pub use mirror::{MirrorError, ShowMirror};
pub use show::{Show, ShowError};
