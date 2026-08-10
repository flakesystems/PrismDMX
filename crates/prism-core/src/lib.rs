//! Show model, session state and persistence - the single source of truth.
//!
//! Holds everything the daemon is authoritative about: the patch, sequences,
//! presets, the programmer, the Oops journal, and the session state that lets
//! the X-Touch operate the UI (decision D11, `ARCHITECTURE_SPEC.md` section 4).
//!
//! Platform-neutral: no `#[cfg(target_os = ...)]` here.
//!
//! Sessions **S11-S15**.
