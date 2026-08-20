//! The real-time DMX engine.
//!
//! Pure calculation: merging, playback evaluation and timing. No I/O, no UI
//! dependency, no platform-specific code. This is the crate the zero-crash
//! invariant in `CLAUDE.md` is really about, and it carries the project's
//! strictest coverage requirement (> 95 %).
//!
//! Merge semantics are specified in `docs/DMX_MERGE.md`; the tick contract is
//! in `ARCHITECTURE_SPEC.md` section 3.1, and the pipeline order in section 5.
//!
//! Sessions **S2-S6**.
//!
//! # Shape
//!
//! ```text
//!   core thread ──push──▶ [`Producer`] ─┐
//!                                       │  SPSC ring, wait-free
//!                                       ▼
//!                        [`Engine`] ── 44 Hz tick ──▶ [`FramePublisher`]
//!                                       ▲                     │
//!                                  [`TickBody`]                │ triple buffer,
//!                                 ([`MergeBody`])              │ wait-free
//!                                                              ▼
//!                                                     [`FrameSubscriber`]
//!                                                      one per output driver
//! ```
//!
//! Nothing on that path takes a lock, allocates, or waits for anything else.
//!
//! # The merge
//!
//! `docs/DMX_MERGE.md` is the specification; this crate implements it in three
//! layers, smallest first:
//!
//! - [`merge`](self#reexports) — the arithmetic. [`merge_htp`], [`merge_ltp`],
//!   [`apply_master`] and [`merge_playbacks`]: pure functions with no state and
//!   no clock, which is what lets §6.1 and §6.2 be checked as algebra.
//! - [`MergePlan`] — the home layer. The patch flattened into one
//!   [`AttributeSlot`] per fixture and attribute, built before the tick starts.
//! - [`PlaybackLayer`] — the source set, and [`PlaybackLayer::resolve`], which
//!   turns it into one value per slot without allocating.
//! - [`ProgrammerLayer`] — the operator's absolute override, sparse by
//!   specification: an untouched attribute is absent, not zero.
//! - [`MasterLayer`] — the group masters and the grand master, which scale
//!   intensity and nothing else.
//!
//! The order those last two run in is the specification's, and it is not
//! symmetric: the programmer beats every playback, and the masters beat the
//! programmer. A grand master that a programmer value could survive would not
//! be a grand master.
//!
//! # The encoding
//!
//! The merge answers in attribute values, 16-bit and address-free.
//! [`ChannelPlan`] turns those into channel bytes: the coarse/fine split, the
//! attribute and per-fixture inverts, and the frame position each write lands
//! at. Everything that can be decided from the patch is decided when the plan is
//! built — including whether the patch is legal at all, which is why a fixture
//! running past channel 512 is a [`PatchError`] at patch time and never a
//! problem in the tick.
//!
//! # The playback
//!
//! The merge answers over a *set* of values; what puts values into that set is
//! the playback. [`SequencePlan`] compiles a `prism_domain::Sequence` against
//! the same patch — fixtures and attributes become slot indices, seconds become
//! whole ticks — because a cue owns `String`s and `Vec`s and so cannot be
//! created *or dropped* inside a tick. [`CuePlayer`] then runs one compiled cue
//! list on the tick's own time base: fades, delays, cue traversal, and the
//! `Go`, `Follow` and `Time` triggers. The rules it settles — that cues track,
//! that a Go overtakes a running fade rather than jumping, what a release does —
//! are written out in that module's documentation.
//!
//! [`MergeBody`] joins all of it onto the tick, and
//! [`MergeBody::for_patch`] builds it from a patch in one call;
//! [`MergeBody::load_sequence`] puts a cue list on an executor,
//! [`MergeBody::load_groups`] puts the show's groups on the group masters, and
//! [`MergeBody::load_programmer`] resolves an operator-facing
//! `prism_domain::ProgrammerState` against the patch. That is
//! `ARCHITECTURE_SPEC.md` §5 complete, from the patch to the wire.
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
//!
//! [`Engine::tick`] additionally contains the body in [`std::panic::catch_unwind`]:
//! a panic in the merge costs one frame and is counted, it does not end the show.
//!
//! # The long tests
//!
//! Several of this crate's exit criteria take too long for `cargo test`, so
//! they are marked `#[ignore]` and have to be asked for by name. A criterion
//! nobody can re-run is not a measurement, so the commands are written down
//! here.
//!
//! The ten-minute deadline runs — including S6's stress gate, which is the
//! whole pipeline at 64 universes with every core busy — and the long
//! triple-buffer stress test. They need the **release** profile: an unoptimised
//! build of the full-size pipeline does not fit inside a 22.7 ms tick, which is
//! a fact about `-C opt-level=0` and not about the product.
//!
//! ```text
//! cargo test -p prism-engine --release -- --ignored --nocapture
//! ```
//!
//! The `loom` model of the two lock-free structures. It replaces the standard
//! atomics with instrumented ones and explores every legal interleaving, so it
//! is a different build of the crate rather than a different test:
//!
//! ```text
//! RUSTFLAGS="--cfg loom" cargo test -p prism-engine --release --lib
//! ```
//!
//! In PowerShell:
//!
//! ```text
//! $env:RUSTFLAGS = "--cfg loom"; cargo test -p prism-engine --release --lib
//! ```
//!
//! Under `--cfg loom` the ordinary unit tests are compiled out, so that command
//! runs the models and nothing else.

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

mod body;
mod clock;
mod command;
mod cue;
mod encode;
mod frame;
mod master;
mod merge;
mod plan;
mod playback;
mod player;
mod programmer;
mod readback;
mod spsc;
mod stats;
mod sync;
#[cfg(all(test, not(loom)))]
mod testkit;
mod tick;
mod triple_buffer;

pub use body::MergeBody;
pub use clock::{Clock, ManualClock, SystemClock};
pub use command::TickCommand;
pub use cue::{
    CueError, CuePlan, CueSlot, CueValue, MAX_CUE_PARTS, MAX_CUES, SequencePlan, interpolate,
    ticks_from_seconds,
};
pub use encode::{ChannelPlan, ChannelTarget, PatchError, coarse_byte, fine_byte, invert};
pub use frame::{DmxFrame, FrameLayout, LayoutError, MAX_UNIVERSES, UNIVERSE_CHANNELS};
pub use master::MasterLayer;
pub use merge::{
    FULL, SourceValue, apply_master, merge_htp, merge_ltp, merge_playbacks, merge_programmer,
};
pub use plan::{AttributeSlot, MAX_SLOTS, MergeError, MergePlan};
pub use playback::{MAX_SOURCES, MergeScratch, PlaybackLayer, PlaybackSource};
pub use player::{CueLayer, CuePlayer, TAP_WINDOW};
pub use programmer::ProgrammerLayer;
pub use readback::{PlaybackReport, PlaybackState};
pub use spsc::{Consumer, PAYLOAD_BYTES, Producer, TickPayload, command_queue};
pub use stats::{Histogram, TickStats};
pub use tick::{
    Engine, IdleBody, TICK_HZ, TICK_PERIOD, TickBody, TickInfo, deadline_offset, tick_index_at,
};
pub use triple_buffer::{FramePublisher, FrameSubscriber};
