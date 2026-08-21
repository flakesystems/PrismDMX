//! Strategies for property tests.
//!
//! Available to this crate's own tests and, behind the `proptest` feature, to
//! downstream crates: `prism-engine` verifies the merge invariants in
//! `docs/DMX_MERGE.md` over arbitrary domain values, and generating those values
//! is a property of the domain, not of the engine.
//!
//! Three constraints shape everything here:
//!
//! - **Finite floats only.** JSON has no encoding for NaN or infinity, and
//!   `NaN != NaN` would break every round-trip assertion for reasons that have
//!   nothing to do with the code under test.
//! - **Bounded decimal precision.** `serde_json`'s float *parser* is not
//!   correctly rounded: it re-reads its own output of a full-precision `f64` one
//!   ULP off. See `wire.rs::json_floats_are_only_exact_to_bounded_precision`,
//!   which pins that behaviour. Millithousandths of a degree, a metre or a
//!   second are far finer than anything a lighting desk resolves, so the
//!   strategies here generate values at that precision and the round-trip
//!   property stays an exact equality rather than a tolerance.
//! - **Small collections.** The model nests (sequence → cue → part), so the
//!   default size range of up to 100 elements per level would produce cases
//!   large enough to make shrinking useless.

use proptest::prelude::*;

/// Decimal places generated floats are limited to.
const SCALE: f64 = 1000.0;

/// A finite `f64` in a range wide enough for coordinates, angles and pixels.
pub fn finite_f64() -> impl Strategy<Value = f64> {
    (-1_000_000_000i64..1_000_000_000i64).prop_map(|units| units as f64 / SCALE)
}

/// A finite, non-negative `f64` for durations in seconds.
pub fn seconds() -> impl Strategy<Value = f64> {
    (0i64..600_000i64).prop_map(|units| units as f64 / SCALE)
}

/// A vector of at most `max` arbitrary values.
pub fn small_vec<T>(max: usize) -> impl Strategy<Value = Vec<T>>
where
    T: Arbitrary + 'static,
{
    proptest::collection::vec(any::<T>(), 0..max)
}

/// A map of at most `max` arbitrary entries.
pub fn small_map<K, V>(max: usize) -> impl Strategy<Value = std::collections::BTreeMap<K, V>>
where
    K: Arbitrary + Ord + 'static,
    V: Arbitrary + 'static,
{
    proptest::collection::btree_map(any::<K>(), any::<V>(), 0..max)
}

/// An arbitrary value, **behind a box**.
///
/// # This is a stack budget, not a style
///
/// `proptest_derive` builds one strategy value holding every variant's strategy
/// at once, so an enum's strategy type is the *sum* of its variants'. `Command`
/// is forty-two variants deep and in a debug build on Windows the sum has now
/// twice been enough to overflow a test thread's stack while the strategy is
/// still being constructed — S34's finding at the thirty-sixth variant, and
/// S40's again with [`crate::ObjectRef`] appearing twice inside `Copy` and twice
/// inside `Move`.
///
/// Boxing a *field's* strategy replaces its whole tree with one pointer, which
/// is why this is applied to the `ObjectRef` and `PlaybackTarget` fields rather
/// than to the enum as a whole. **A session adding a command with a large
/// payload reaches for this and not for `RUST_MIN_STACK`**, which only moves the
/// cliff a few variants further along.
pub fn boxed<T>() -> BoxedStrategy<T>
where
    T: Arbitrary + core::fmt::Debug + 'static,
{
    any::<T>().boxed()
}

/// A map of between one and `max` arbitrary entries.
///
/// For nested maps whose inner map must not be empty — see the invariant on
/// [`crate::ProgrammerState::values`].
pub fn non_empty_map<K, V>(max: usize) -> impl Strategy<Value = std::collections::BTreeMap<K, V>>
where
    K: Arbitrary + Ord + 'static,
    V: Arbitrary + 'static,
{
    proptest::collection::btree_map(any::<K>(), any::<V>(), 1..max.max(2))
}
