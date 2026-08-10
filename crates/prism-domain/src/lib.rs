//! Shared domain vocabulary for PrismDMX.
//!
//! Every other crate speaks these types. They are defined once here and exported
//! to TypeScript so the UI cannot drift from the daemon.
//!
//! See `ARCHITECTURE_SPEC.md` section 6 for the model and `docs/IPC_PROTOCOL.md`
//! sections 5-6 for the `Command` and `Delta` wire types.
//!
//! Implemented in session **S1**. This crate is platform-neutral: it must
//! compile on every target, so it may not contain `#[cfg(target_os = ...)]`.
