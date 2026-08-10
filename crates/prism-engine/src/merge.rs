//! The merge, as arithmetic.
//!
//! `docs/DMX_MERGE.md` §2 in the smallest form it has: a handful of pure
//! functions over values, with no state, no clock and no buffers. Everything
//! above this module — the home layer in [`crate::MergePlan`], the source set in
//! [`crate::PlaybackLayer`] — is bookkeeping that decides *which* values are
//! handed to these functions. The rules themselves live here, where they can be
//! stated as algebra and checked as algebra.
//!
//! The split matters for one reason: `docs/DMX_MERGE.md` §6.1 and §6.2 are laws
//! about operations, not about a desk. Commutativity is a property of
//! [`merge_htp`]; non-commutativity is a property of [`merge_ltp`]. Testing them
//! against a struct with an activation counter would test the counter instead.

use prism_domain::{ExecutorId, MergeMode};

/// Full scale for every internal value: attribute values, master levels and the
/// grand master are all `0..=65535`, per `ARCHITECTURE_SPEC.md` §6.
pub const FULL: u16 = u16::MAX;

/// Highest takes precedence: the larger of two contributions.
///
/// `docs/DMX_MERGE.md` §2.1. Both arguments must already have their executor
/// master applied — see [`apply_master`], and §2.3 for why the order matters.
///
/// This is a commutative, associative, idempotent monoid with identity `0`, and
/// the property tests in this module hold it to all four.
#[must_use]
pub const fn merge_htp(a: u16, b: u16) -> u16 {
    if a > b { a } else { b }
}

/// Latest takes precedence: the later contribution, whatever the earlier one was.
///
/// `docs/DMX_MERGE.md` §2.2. Deliberately *not* commutative and deliberately not
/// a maximum: a position is a single physical state, and taking the larger of
/// two pan values would invent a third position nobody programmed. The property
/// test in this module asserts the non-commutativity so that the day someone
/// notices this function "ignores an argument", the build tells them why.
#[must_use]
pub const fn merge_ltp(_earlier: u16, later: u16) -> u16 {
    later
}

/// Scales a value by an executor's master level.
///
/// `docs/DMX_MERGE.md` §2.1: applied to each source **before** the HTP maximum,
/// so a cue at full on an executor faded to half contributes half and can lose
/// to another executor. §2.3: never applied to an LTP attribute — half a pan
/// position is not a position.
///
/// Truncating, not rounding: a value can therefore fall one unit short of what
/// exact arithmetic would give, which is 1/65536 of the range and below what any
/// fixture resolves. What matters is that it is monotone in both arguments and
/// that a master at [`FULL`] is exactly the identity.
#[must_use]
pub const fn apply_master(value: u16, master: u16) -> u16 {
    // 65535 * 65535 fits a u32 with room to spare, so this cannot overflow.
    let scaled = (value as u32 * master as u32).div_euclid(FULL as u32);
    scaled as u16
}

/// What one playback source offers for one attribute.
///
/// The activation stamp is the whole of LTP ordering: `docs/DMX_MERGE.md` §2.2
/// orders sources by when their executor was switched on, never by executor
/// number, page or list position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceValue {
    /// When this source was activated. Higher is more recent.
    pub activation: u64,
    /// Which executor this source is. Only ever a tie-break — see [`Self::order`].
    pub executor: ExecutorId,
    /// The executor's master level, `0..=65535`. HTP only.
    pub master: u16,
    /// The value the source provides, `0..=65535`.
    pub value: u16,
}

impl SourceValue {
    /// The key LTP orders by.
    ///
    /// Activation stamps are unique by construction — [`crate::PlaybackLayer`]
    /// draws them from a counter that only ever increases. The executor number
    /// is appended anyway so that the key is a *total* order on any source set
    /// somebody hands in, which is what makes the result independent of the
    /// order of the slice rather than merely usually independent of it.
    #[must_use]
    pub const fn order(&self) -> (u64, u32) {
        (self.activation, self.executor.get())
    }

    /// The contribution this source makes to an HTP maximum: its value with its
    /// executor master applied.
    #[must_use]
    pub const fn mastered(&self) -> u16 {
        apply_master(self.value, self.master)
    }
}

/// The playback layer for one attribute: `docs/DMX_MERGE.md` §2, entire.
///
/// A pure function of the source set and the home value. There is no hidden
/// time in it — the only notion of "later" is the activation stamp each source
/// carries — and the result does not depend on the order of `sources`.
///
/// With no sources at all the attribute falls back to `home`, which is the
/// bottom of the stack in §1 and the reason a moving head with nothing active
/// sits at its home position instead of at pan 0.
#[must_use]
pub fn merge_playbacks(mode: MergeMode, home: u16, sources: &[SourceValue]) -> u16 {
    match mode {
        MergeMode::Htp => sources
            .iter()
            .map(SourceValue::mastered)
            .reduce(merge_htp)
            .unwrap_or(home),
        MergeMode::Ltp => sources
            .iter()
            .max_by_key(|source| source.order())
            .map_or(home, |source| source.value),
    }
}

/// The programmer sitting on top of the playbacks: `docs/DMX_MERGE.md` §3.
///
/// The programmer is sparse. Where it holds a value that value is the output,
/// whatever the playbacks are doing; where it does not, the merged playback
/// result stands untouched.
///
/// Only the arithmetic lives here. The programmer *state machine* — what a
/// value's source is, the three-stage clear, what a store does — is S6; this
/// function exists in S3 because the worked example in `docs/DMX_MERGE.md` §7
/// is an exit criterion of S3 and its last two rows are about the programmer.
#[must_use]
pub const fn merge_programmer(merged: u16, programmer: Option<u16>) -> u16 {
    match programmer {
        Some(value) => value,
        None => merged,
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::{
        FULL, SourceValue, apply_master, merge_htp, merge_ltp, merge_playbacks, merge_programmer,
    };
    use prism_domain::{ExecutorId, MergeMode};
    use proptest::prelude::*;

    fn source(activation: u64, value: u16, master: u16) -> SourceValue {
        SourceValue {
            activation,
            executor: ExecutorId::new(activation as u32),
            master,
            value,
        }
    }

    /// A source set with distinct activation stamps, in a random order.
    fn source_set(max: usize) -> impl Strategy<Value = Vec<SourceValue>> {
        proptest::collection::vec((any::<u16>(), any::<u16>()), 0..max).prop_map(|values| {
            values
                .into_iter()
                .enumerate()
                .map(|(index, (value, master))| source(index as u64, value, master))
                .collect()
        })
    }

    #[test]
    fn htp_takes_the_larger_of_two_contributions() {
        assert_eq!(merge_htp(0, 0), 0);
        assert_eq!(merge_htp(100, 200), 200);
        assert_eq!(merge_htp(200, 100), 200);
        assert_eq!(merge_htp(FULL, 0), FULL);
    }

    #[test]
    fn ltp_takes_the_later_contribution_even_when_it_is_darker() {
        // The whole point: 0 arriving later wins over 65535 arriving earlier.
        // A maximum would get this wrong and nobody would notice until a
        // position snapped to a corner.
        assert_eq!(merge_ltp(FULL, 0), 0);
        assert_eq!(merge_ltp(0, FULL), FULL);
    }

    #[test]
    fn a_master_at_full_is_the_identity_and_a_master_at_zero_is_a_blackout() {
        for value in [0u16, 1, 12_345, 32_767, 32_768, 65_534, FULL] {
            assert_eq!(apply_master(value, FULL), value, "value {value}");
            assert_eq!(apply_master(value, 0), 0, "value {value}");
        }
    }

    #[test]
    fn a_master_at_half_halves_the_value() {
        // Half of full scale is 32767: 65535 is odd, so exactly half of it is
        // not representable and the lower neighbour is the one that keeps
        // `apply_master(v, FULL) == v` exact.
        assert_eq!(apply_master(FULL, 32_767), 32_767);
        assert_eq!(apply_master(30_000, 32_767), 14_999);
    }

    #[test]
    fn an_empty_source_set_falls_back_to_home_in_both_modes() {
        assert_eq!(merge_playbacks(MergeMode::Htp, 4_242, &[]), 4_242);
        assert_eq!(merge_playbacks(MergeMode::Ltp, 32_768, &[]), 32_768);
    }

    #[test]
    fn htp_applies_the_master_before_the_maximum_not_after() {
        // docs/DMX_MERGE.md 2.1. The two orders disagree here, which is what
        // makes this test worth writing: before the maximum the answer is
        // 40000, after it it would be 65535 scaled by one of the two masters.
        let sources = [source(1, FULL, 32_767), source(2, 40_000, FULL)];
        assert_eq!(merge_playbacks(MergeMode::Htp, 0, &sources), 40_000);
    }

    #[test]
    fn ltp_ignores_the_master_entirely() {
        // docs/DMX_MERGE.md 2.3: half a pan position is not a pan position.
        let sources = [source(1, 20_000, 0), source(2, 45_000, 1)];
        assert_eq!(merge_playbacks(MergeMode::Ltp, 0, &sources), 45_000);
        // Even a master at zero leaves the position where it was.
        let single = [source(1, 45_000, 0)];
        assert_eq!(merge_playbacks(MergeMode::Ltp, 0, &single), 45_000);
    }

    #[test]
    fn ltp_orders_by_activation_stamp_not_by_executor_number() {
        // The executor with the *lower* number was switched on later, so it
        // wins. Ordering by executor number would give 20000.
        let sources = [
            SourceValue {
                activation: 3,
                executor: ExecutorId::new(99),
                master: FULL,
                value: 20_000,
            },
            SourceValue {
                activation: 9,
                executor: ExecutorId::new(1),
                master: FULL,
                value: 45_000,
            },
        ];
        assert_eq!(merge_playbacks(MergeMode::Ltp, 0, &sources), 45_000);
    }

    #[test]
    fn the_programmer_overrides_the_playbacks_and_absence_leaves_them_alone() {
        assert_eq!(merge_programmer(45_000, Some(50_000)), 50_000);
        assert_eq!(merge_programmer(45_000, None), 45_000);
        // Including a programmer value of zero, which is a value, not an absence.
        assert_eq!(merge_programmer(45_000, Some(0)), 0);
    }

    proptest! {
        /// `docs/DMX_MERGE.md` §6.1 — commutative.
        #[test]
        fn htp_is_commutative(a in any::<u16>(), b in any::<u16>()) {
            prop_assert_eq!(merge_htp(a, b), merge_htp(b, a));
        }

        /// §6.1 — associative.
        #[test]
        fn htp_is_associative(a in any::<u16>(), b in any::<u16>(), c in any::<u16>()) {
            prop_assert_eq!(merge_htp(merge_htp(a, b), c), merge_htp(a, merge_htp(b, c)));
        }

        /// §6.1 — idempotent.
        #[test]
        fn htp_is_idempotent(a in any::<u16>()) {
            prop_assert_eq!(merge_htp(a, a), a);
        }

        /// §6.1 — identity. Zero is the neutral element, which is why an
        /// executor faded out contributes nothing rather than pulling the rig
        /// down with it.
        #[test]
        fn zero_is_the_htp_identity(a in any::<u16>()) {
            prop_assert_eq!(merge_htp(a, 0), a);
            prop_assert_eq!(merge_htp(0, a), a);
        }

        /// §6.1 — monotone: raising one argument can never lower the result.
        #[test]
        fn htp_is_monotone(a in any::<u16>(), b in any::<u16>(), raise in any::<u16>()) {
            let raised = a.saturating_add(raise);
            prop_assert!(merge_htp(raised, b) >= merge_htp(a, b));
        }

        /// §6.2 — LTP is **not** commutative, and this is asserted rather than
        /// assumed so that nobody later "optimises" it into a maximum.
        #[test]
        fn ltp_is_not_commutative(a in any::<u16>(), b in any::<u16>()) {
            prop_assume!(a != b);
            prop_assert_ne!(merge_ltp(a, b), merge_ltp(b, a));
        }

        /// A maximum would satisfy every HTP law; this is what separates the
        /// two operations. If `merge_ltp` ever became `max`, this fails.
        #[test]
        fn ltp_is_not_a_maximum(a in any::<u16>(), b in any::<u16>()) {
            prop_assume!(a > b);
            prop_assert_ne!(merge_ltp(a, b), merge_htp(a, b));
        }

        /// A master never raises a value, and full scale never changes one.
        #[test]
        fn a_master_only_ever_attenuates(value in any::<u16>(), master in any::<u16>()) {
            prop_assert!(apply_master(value, master) <= value);
            prop_assert_eq!(apply_master(value, FULL), value);
        }

        /// Monotone in both arguments, so a fader move cannot invert a merge.
        #[test]
        fn a_master_is_monotone(
            value in any::<u16>(),
            master in any::<u16>(),
            raise in any::<u16>(),
        ) {
            let higher = master.saturating_add(raise);
            prop_assert!(apply_master(value, higher) >= apply_master(value, master));
            let brighter = value.saturating_add(raise);
            prop_assert!(apply_master(brighter, master) >= apply_master(value, master));
        }

        /// §6.1 — the HTP merge over a whole source set is the maximum of the
        /// mastered contributions, and the order of the slice does not matter.
        #[test]
        fn htp_over_a_source_set_ignores_the_order(mut sources in source_set(8)) {
            let forward = merge_playbacks(MergeMode::Htp, 0, &sources);
            sources.reverse();
            prop_assert_eq!(merge_playbacks(MergeMode::Htp, 0, &sources), forward);
            let expected = sources.iter().map(SourceValue::mastered).max().unwrap_or(0);
            prop_assert_eq!(forward, expected);
        }

        /// §6.2 — order-dependent: the result is the value of the source with
        /// the highest activation counter, and shuffling the slice with the
        /// counters preserved changes nothing.
        #[test]
        fn ltp_is_the_value_of_the_most_recently_activated_source(
            mut sources in source_set(8),
            rotate in 0usize..8,
        ) {
            let expected = sources
                .iter()
                .max_by_key(|source| source.order())
                .map_or(1234, |source| source.value);
            let home = 1234;
            prop_assert_eq!(merge_playbacks(MergeMode::Ltp, home, &sources), expected);
            let len = sources.len();
            if len > 0 {
                sources.rotate_left(rotate % len);
                prop_assert_eq!(merge_playbacks(MergeMode::Ltp, home, &sources), expected);
            }
        }

        /// §6.2 — deactivation falls back: dropping the winner yields the next
        /// most recently activated source, all the way down to home.
        #[test]
        fn removing_the_winning_source_falls_back_to_the_next_one(
            sources in source_set(6),
            home in any::<u16>(),
        ) {
            let mut remaining = sources.clone();
            // Peel the sources off newest first. Each removal must hand the
            // attribute to the next newest, and the last one to home.
            while !remaining.is_empty() {
                let winner = remaining
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, source)| source.order())
                    .map(|(index, source)| (index, source.value));
                let Some((index, value)) = winner else { break };
                prop_assert_eq!(merge_playbacks(MergeMode::Ltp, home, &remaining), value);
                remaining.remove(index);
            }
            prop_assert_eq!(merge_playbacks(MergeMode::Ltp, home, &remaining), home);
        }
    }
}
