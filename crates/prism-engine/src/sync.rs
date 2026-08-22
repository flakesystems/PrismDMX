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
//! The one place the tick touches a `std::sync::Mutex` at all is
//! [`crate::FrameEnrolment`], and it touches it with `try_lock`: a lock it
//! cannot have this instant is one it does not take, so it still never waits.
//! That is `prismd`'s `BodySwap` pattern and the reasoning is written out
//! there and in `triple_buffer`'s own documentation. It is not aliased here
//! because loom has nothing to model about a lock nobody blocks on.

#[cfg(loom)]
pub(crate) use loom::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
};
#[cfg(not(loom))]
pub(crate) use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
};
