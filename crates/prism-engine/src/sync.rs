//! The synchronisation primitives used on the tick path.
//!
//! Everything the engine shares between threads goes through this module, for
//! one reason: under `--cfg loom` the aliases resolve to `loom`'s instrumented
//! atomics instead of the standard library's, and the model checker can then
//! explore every legal interleaving of the triple buffer and the command queue.
//! See the crate documentation for how to run that suite.
//!
//! There is deliberately no mutex here. `ARCHITECTURE_SPEC.md` §3.1 forbids the
//! tick from taking a lock, so the only primitives available to it are atomics.

#[cfg(loom)]
pub(crate) use loom::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering},
};
#[cfg(not(loom))]
pub(crate) use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering},
};
