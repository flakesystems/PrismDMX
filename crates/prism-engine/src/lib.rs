//! The real-time DMX engine.
//!
//! Pure calculation: merging, playback evaluation and timing. No I/O, no UI
//! dependency, no platform-specific code. This is the crate the zero-crash
//! invariant in `CLAUDE.md` is really about, and it carries the project's
//! strictest coverage requirement (> 95 %).
//!
//! Merge semantics are specified in `docs/DMX_MERGE.md`; the tick contract is
//! in `ARCHITECTURE_SPEC.md` section 3.1.
//!
//! Sessions **S2-S6**.
//!
//! # Rules for code on the tick path
//!
//! The lints below are not style preferences. A panic on the tick thread stops
//! DMX output mid-show, so the constructs that cause one are denied outright in
//! production code and permitted only in tests.
//!
//! - no heap allocation inside a tick - buffers are sized from the patch up front
//! - no locks - SPSC ring buffer in, triple buffer out
//! - no synchronous logging - records go to a lock-free ring, a writer thread drains it
//! - absolute deadlines, never accumulated sleeps

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
