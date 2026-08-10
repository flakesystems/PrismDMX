//! The programmer layer: what the operator has touched, above every playback.
//!
//! `docs/DMX_MERGE.md` §3. The programmer holds every value the operator has
//! set but not yet stored, and it sits above the playbacks: where it holds a
//! value, that value **is** the output, whatever any cue is doing. That is what
//! makes live programming predictable — what you grab is what you see.
//!
//! # Sparse, and that is the whole point
//!
//! An attribute nobody has touched is **absent**, not zero. A layer that held a
//! value everywhere would not be a programmer at all; it would be one more
//! playback that always wins, and grabbing a single head would black the rest of
//! the stage out. So this is a sparse set over slot numbers, and
//! [`ProgrammerLayer::apply`] costs what the operator has touched rather than
//! what the patch could hold.
//!
//! # Why it is addressed by slot number
//!
//! `prism_domain::ProgrammerState` is the operator-facing form: nested
//! `BTreeMap`s keyed by fixture and attribute. It cannot come near the tick —
//! it owns maps and vectors, and *dropping* one allocates exactly as surely as
//! building one does (`ARCHITECTURE_SPEC.md` §3.1). The same answer as S2's
//! `TickCommand` and S5's [`crate::SequencePlan`]: the evaluable form is built
//! before the tick, not in it. Here that form is [`crate::MergePlan`]'s slot
//! numbering, which is stable — fixture, then attribute — and is the same
//! address the encoder resolves against.

use prism_domain::ProgrammerState;

use crate::plan::MergePlan;

/// The values the operator has touched, indexed by [`MergePlan`] slot.
///
/// Sized from the plan when the body is built. Setting, clearing and applying
/// are all allocation-free, so the whole layer runs on the tick.
#[derive(Debug, Clone)]
pub struct ProgrammerLayer {
    /// One value per slot; meaningful only where `present` is set.
    values: Box<[u16]>,
    /// Whether the operator has touched this slot.
    present: Box<[bool]>,
    /// The touched slots, in no particular order. Only the first `count` entries
    /// are live.
    touched: Box<[u32]>,
    /// For a touched slot, where it sits in `touched`, so clearing one value is
    /// a swap rather than a search.
    position: Box<[u32]>,
    count: usize,
}

impl ProgrammerLayer {
    /// An empty programmer over the slots of `plan`.
    #[must_use]
    pub fn new(plan: &MergePlan) -> Self {
        let slots = plan.slot_count();
        Self {
            values: vec![0; slots].into_boxed_slice(),
            present: vec![false; slots].into_boxed_slice(),
            touched: vec![0; slots].into_boxed_slice(),
            position: vec![0; slots].into_boxed_slice(),
            count: 0,
        }
    }

    /// How many slots this layer can address — the plan's slot count.
    #[must_use]
    pub const fn slot_count(&self) -> usize {
        self.values.len()
    }

    /// How many attributes the operator has touched.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Whether the operator has touched nothing at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Touches one slot, replacing any value already there.
    ///
    /// Returns `false` if the slot is not in the plan this layer was built for.
    /// The tick's answer to an impossible write is to ignore it, never to panic.
    pub fn set(&mut self, slot: usize, value: u16) -> bool {
        let (Some(cell), Some(present)) = (self.values.get_mut(slot), self.present.get_mut(slot))
        else {
            return false;
        };
        *cell = value;
        if !*present {
            *present = true;
            if let (Some(entry), Some(position)) = (
                self.touched.get_mut(self.count),
                self.position.get_mut(slot),
            ) {
                *entry = slot as u32;
                *position = self.count as u32;
                self.count += 1;
            }
        }
        true
    }

    /// Forgets one touched value. Returns `false` if it was not touched.
    ///
    /// The last live entry is swapped into the hole, so this costs the same
    /// whether one attribute is touched or eight thousand are.
    pub fn clear(&mut self, slot: usize) -> bool {
        match self.present.get_mut(slot) {
            Some(present) if *present => *present = false,
            _ => return false,
        }
        // One fallible expression rather than three. A slot that is present has
        // a position, and a layer with a present slot has at least one live
        // entry, so none of these lookups can fail — writing them out
        // separately would only add branches nothing can reach.
        let last = self.count.saturating_sub(1);
        let moved = self.touched.get(last).copied();
        if let Some((hole, moved)) = self.position.get(slot).copied().zip(moved) {
            if let Some(entry) = self.touched.get_mut(hole as usize) {
                *entry = moved;
            }
            if let Some(position) = self.position.get_mut(moved as usize) {
                *position = hole;
            }
        }
        self.count = last;
        true
    }

    /// Forgets everything — the first stage of the operator's Clear.
    ///
    /// Only the touched slots are visited, so clearing an empty programmer costs
    /// nothing.
    pub fn clear_all(&mut self) {
        for slot in self.touched.iter().take(self.count) {
            if let Some(present) = self.present.get_mut(*slot as usize) {
                *present = false;
            }
        }
        self.count = 0;
    }

    /// The value the programmer holds for a slot, if it holds one.
    ///
    /// `Some(0)` is a value — the operator has taken that attribute to zero —
    /// and is a different thing entirely from `None`.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<u16> {
        match self.present.get(slot) {
            Some(true) => self.values.get(slot).copied(),
            _ => None,
        }
    }

    /// Every touched slot and its value, in no defined order.
    pub fn contributions(&self) -> impl Iterator<Item = (usize, u16)> + '_ {
        self.touched.iter().take(self.count).filter_map(|slot| {
            let slot = *slot as usize;
            Some((slot, self.values.get(slot).copied()?))
        })
    }

    /// Overrides the merged playback values wherever the programmer holds one.
    ///
    /// `ARCHITECTURE_SPEC.md` §5 step 5, and `docs/DMX_MERGE.md` §3: absolute
    /// priority over everything below, and no effect at all where the operator
    /// has touched nothing. The result does not depend on the order values were
    /// touched in — each slot appears at most once.
    ///
    /// Allocation-free and lock-free: this runs on the tick.
    pub fn apply(&self, values: &mut [u16]) {
        for (slot, value) in self.contributions() {
            if let Some(out) = values.get_mut(slot) {
                *out = value;
            }
        }
    }

    /// Replaces the whole layer with the contents of an operator-facing
    /// [`ProgrammerState`], resolved against `plan`.
    ///
    /// Returns how many of its entries named a fixture or attribute this patch
    /// does not have. They are dropped rather than refused, for the reason S5
    /// gives for a cue naming an unpatched fixture: a show outlives the rig it
    /// was written on, and one dead light must not take the desk down with it.
    /// The count exists so a host can say so.
    ///
    /// Not tick work. It allocates nothing, but it walks `BTreeMap`s that only
    /// the core thread owns — the tick receives programmer changes one at a time
    /// as [`crate::TickCommand`]s.
    pub fn load(&mut self, plan: &MergePlan, state: &ProgrammerState) -> usize {
        self.clear_all();
        let mut unresolved = 0;
        for (fixture, attributes) in &state.values {
            for (attribute, value) in attributes {
                match plan.index_of(*fixture, *attribute) {
                    Some(slot) => {
                        self.set(slot, value.value);
                    }
                    None => unresolved += 1,
                }
            }
        }
        unresolved
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::merge::merge_programmer;
    use crate::plan::MergePlan;
    use crate::programmer::ProgrammerLayer;
    use crate::testkit::moving_head;
    use prism_domain::{
        AttributeType, FixtureId, ProgrammerState, ProgrammerValue, ProgrammerValueSource,
    };
    use proptest::prelude::*;

    /// Three moving heads: six slots, alternating dimmer and pan.
    fn plan() -> MergePlan {
        let head = moving_head();
        MergePlan::build((1..=3).map(|id| (FixtureId::new(id), &head))).unwrap()
    }

    fn slot(plan: &MergePlan, fixture: u32, attribute: AttributeType) -> usize {
        plan.index_of(FixtureId::new(fixture), attribute).unwrap()
    }

    fn value(value: u16) -> ProgrammerValue {
        ProgrammerValue {
            value,
            source: ProgrammerValueSource::Manual,
            preset_ref: None,
        }
    }

    #[test]
    fn a_fresh_programmer_holds_nothing_and_changes_nothing() {
        // docs/DMX_MERGE.md 3: an attribute with no programmer value is
        // unaffected. A layer that started full of zeroes would black the rig
        // out the moment it was built.
        let plan = plan();
        let programmer = ProgrammerLayer::new(&plan);
        assert!(programmer.is_empty());
        assert_eq!(programmer.len(), 0);
        assert_eq!(programmer.slot_count(), 6);

        let mut values = [1_000u16, 2_000, 3_000, 4_000, 5_000, 6_000];
        programmer.apply(&mut values);
        assert_eq!(values, [1_000, 2_000, 3_000, 4_000, 5_000, 6_000]);
    }

    #[test]
    fn a_touched_slot_overrides_and_an_untouched_one_is_left_alone() {
        let plan = plan();
        let mut programmer = ProgrammerLayer::new(&plan);
        let pan = slot(&plan, 2, AttributeType::Pan);
        assert!(programmer.set(pan, 50_000));
        assert_eq!(programmer.len(), 1);
        assert_eq!(programmer.get(pan), Some(50_000));

        let mut values = [10u16; 6];
        programmer.apply(&mut values);
        for (index, value) in values.iter().enumerate() {
            let expected = if index == pan { 50_000 } else { 10 };
            assert_eq!(*value, expected, "slot {index}");
        }
    }

    #[test]
    fn a_programmer_value_of_zero_is_a_value_and_not_an_absence() {
        // The distinction the whole layer is built on. Taking a dimmer to zero
        // in the programmer must darken it even while a cue holds it at full.
        let plan = plan();
        let mut programmer = ProgrammerLayer::new(&plan);
        let dimmer = slot(&plan, 1, AttributeType::Dimmer);
        programmer.set(dimmer, 0);
        assert_eq!(programmer.get(dimmer), Some(0));

        let mut values = [65_535u16; 6];
        programmer.apply(&mut values);
        assert_eq!(values.first().copied(), Some(0));
    }

    #[test]
    fn setting_the_same_slot_twice_replaces_the_value_without_touching_it_twice() {
        let plan = plan();
        let mut programmer = ProgrammerLayer::new(&plan);
        let dimmer = slot(&plan, 1, AttributeType::Dimmer);
        programmer.set(dimmer, 100);
        programmer.set(dimmer, 200);
        assert_eq!(programmer.len(), 1);
        assert_eq!(programmer.get(dimmer), Some(200));
    }

    #[test]
    fn clearing_one_value_leaves_the_others_and_frees_the_slot() {
        let plan = plan();
        let mut programmer = ProgrammerLayer::new(&plan);
        let dimmer = slot(&plan, 1, AttributeType::Dimmer);
        let pan = slot(&plan, 3, AttributeType::Pan);
        programmer.set(dimmer, 111);
        programmer.set(pan, 222);

        assert!(programmer.clear(dimmer));
        assert!(!programmer.clear(dimmer), "clearing twice is not a change");
        assert_eq!(programmer.len(), 1);
        assert_eq!(programmer.get(dimmer), None);
        assert_eq!(programmer.get(pan), Some(222));

        let mut values = [7u16; 6];
        programmer.apply(&mut values);
        assert_eq!(values.first().copied(), Some(7));
        assert_eq!(values.get(pan).copied(), Some(222));
    }

    #[test]
    fn clearing_any_of_several_values_leaves_exactly_the_rest() {
        // The swap-remove has to survive clearing the first, the middle and the
        // last touched slot, in any order. Clearing the first one moves the last
        // into its place, which is the case a naive implementation gets wrong.
        let plan = plan();
        for order in [[0usize, 1, 2], [2, 1, 0], [1, 0, 2], [1, 2, 0]] {
            let mut programmer = ProgrammerLayer::new(&plan);
            let slots = [
                slot(&plan, 1, AttributeType::Dimmer),
                slot(&plan, 2, AttributeType::Pan),
                slot(&plan, 3, AttributeType::Dimmer),
            ];
            for (index, slot) in slots.iter().enumerate() {
                programmer.set(*slot, index as u16 + 1);
            }
            let cleared = slots.get(order[0]).copied().unwrap();
            assert!(programmer.clear(cleared));
            assert_eq!(programmer.len(), 2);
            assert_eq!(programmer.get(cleared), None);
            for (index, slot) in slots.iter().enumerate() {
                if index != order[0] {
                    assert_eq!(
                        programmer.get(*slot),
                        Some(index as u16 + 1),
                        "slot {slot} after clearing {cleared}"
                    );
                }
            }
            // And the freed slot can be touched again, which is what proves the
            // bookkeeping is consistent rather than merely hidden.
            programmer.set(cleared, 999);
            assert_eq!(programmer.len(), 3);
            assert_eq!(programmer.get(cleared), Some(999));
        }
    }

    #[test]
    fn clearing_everything_empties_the_layer_and_leaves_the_playbacks_alone() {
        let plan = plan();
        let mut programmer = ProgrammerLayer::new(&plan);
        for slot in 0..plan.slot_count() {
            programmer.set(slot, 1_234);
        }
        assert_eq!(programmer.len(), 6);
        programmer.clear_all();
        assert!(programmer.is_empty());

        let mut values = [42u16; 6];
        programmer.apply(&mut values);
        assert_eq!(values, [42; 6]);
        // And an already-empty programmer can be cleared again.
        programmer.clear_all();
        assert!(programmer.is_empty());
    }

    #[test]
    fn a_slot_that_is_not_in_the_plan_is_ignored_rather_than_a_panic() {
        // A stale command from before a repatch must not stop the tick.
        let plan = plan();
        let mut programmer = ProgrammerLayer::new(&plan);
        assert!(!programmer.set(6, 100));
        assert!(!programmer.set(usize::MAX, 100));
        assert!(!programmer.clear(6));
        assert_eq!(programmer.get(6), None);
        assert!(programmer.is_empty());
    }

    #[test]
    fn applying_to_a_shorter_buffer_fills_what_it_can() {
        // `PlaybackLayer::resolve` makes the same promise: a tick that panics
        // costs a frame, so a mismatched buffer degrades instead.
        let plan = plan();
        let mut programmer = ProgrammerLayer::new(&plan);
        programmer.set(0, 11);
        programmer.set(5, 55);
        let mut values = [0u16; 2];
        programmer.apply(&mut values);
        assert_eq!(values, [11, 0]);
    }

    #[test]
    fn a_domain_programmer_state_resolves_into_slot_numbers() {
        let plan = plan();
        let mut state = ProgrammerState::default();
        state.set_value(FixtureId::new(1), AttributeType::Dimmer, value(60_000));
        state.set_value(FixtureId::new(3), AttributeType::Pan, value(20_000));

        let mut programmer = ProgrammerLayer::new(&plan);
        assert_eq!(programmer.load(&plan, &state), 0);
        assert_eq!(programmer.len(), 2);
        assert_eq!(
            programmer.get(slot(&plan, 1, AttributeType::Dimmer)),
            Some(60_000)
        );
        assert_eq!(
            programmer.get(slot(&plan, 3, AttributeType::Pan)),
            Some(20_000)
        );
        assert_eq!(programmer.get(slot(&plan, 2, AttributeType::Pan)), None);
    }

    #[test]
    fn loading_replaces_what_was_there_rather_than_adding_to_it() {
        let plan = plan();
        let mut programmer = ProgrammerLayer::new(&plan);
        programmer.set(slot(&plan, 2, AttributeType::Pan), 500);

        let mut state = ProgrammerState::default();
        state.set_value(FixtureId::new(1), AttributeType::Dimmer, value(60_000));
        programmer.load(&plan, &state);
        assert_eq!(programmer.len(), 1);
        assert_eq!(programmer.get(slot(&plan, 2, AttributeType::Pan)), None);

        // And loading an empty state empties the layer, which is what the
        // operator's Clear does.
        programmer.load(&plan, &ProgrammerState::default());
        assert!(programmer.is_empty());
    }

    #[test]
    fn a_value_for_a_fixture_this_patch_does_not_have_is_counted_and_dropped() {
        // Same rule as an unresolved cue part in S5: a show outlives its rig, so
        // one value naming a light that is no longer patched must not refuse the
        // whole programmer. Dropped silently, an operator could not learn why -
        // hence the count.
        let plan = plan();
        let mut state = ProgrammerState::default();
        state.set_value(FixtureId::new(9), AttributeType::Dimmer, value(1));
        state.set_value(FixtureId::new(1), AttributeType::Tilt, value(2));
        state.set_value(FixtureId::new(1), AttributeType::Dimmer, value(3));

        let mut programmer = ProgrammerLayer::new(&plan);
        assert_eq!(programmer.load(&plan, &state), 2);
        assert_eq!(programmer.len(), 1);
        assert_eq!(
            programmer.get(slot(&plan, 1, AttributeType::Dimmer)),
            Some(3)
        );
    }

    proptest! {
        /// The layer is the pure [`merge_programmer`] function, slot by slot.
        /// If the two ever disagree, the algebra in `merge.rs` stops describing
        /// what the tick does.
        #[test]
        fn applying_the_layer_agrees_with_the_pure_function_on_every_slot(
            merged in proptest::collection::vec(any::<u16>(), 6),
            touched in proptest::collection::vec(proptest::option::of(any::<u16>()), 6),
        ) {
            let plan = plan();
            let mut programmer = ProgrammerLayer::new(&plan);
            for (slot, value) in touched.iter().enumerate() {
                if let Some(value) = value {
                    programmer.set(slot, *value);
                }
            }
            let mut values = merged.clone();
            programmer.apply(&mut values);
            for (slot, expected) in merged.iter().enumerate() {
                let touched = touched.get(slot).copied().flatten();
                prop_assert_eq!(
                    values.get(slot).copied(),
                    Some(merge_programmer(*expected, touched))
                );
            }
        }

        /// Applying twice is applying once: the layer is a function of its
        /// contents and has no memory of what it overwrote.
        #[test]
        fn applying_twice_gives_the_same_answer(
            merged in proptest::collection::vec(any::<u16>(), 6),
            slot in 0usize..6,
            value in any::<u16>(),
        ) {
            let plan = plan();
            let mut programmer = ProgrammerLayer::new(&plan);
            programmer.set(slot, value);
            let mut once = merged.clone();
            programmer.apply(&mut once);
            let mut twice = once.clone();
            programmer.apply(&mut twice);
            prop_assert_eq!(once, twice);
        }

        /// Whatever sequence of touches and clears it is put through, the layer
        /// holds exactly the values a plain map would - and its touched list
        /// stays consistent with its contents.
        #[test]
        fn touches_and_clears_leave_the_layer_agreeing_with_a_plain_map(
            operations in proptest::collection::vec((0usize..6, proptest::option::of(any::<u16>())), 0..40),
        ) {
            let plan = plan();
            let mut programmer = ProgrammerLayer::new(&plan);
            let mut expected: std::collections::BTreeMap<usize, u16> = std::collections::BTreeMap::new();
            for (slot, value) in operations {
                match value {
                    Some(value) => {
                        programmer.set(slot, value);
                        expected.insert(slot, value);
                    }
                    None => {
                        programmer.clear(slot);
                        expected.remove(&slot);
                    }
                }
            }
            prop_assert_eq!(programmer.len(), expected.len());
            for slot in 0..plan.slot_count() {
                prop_assert_eq!(programmer.get(slot), expected.get(&slot).copied());
            }
            let listed: std::collections::BTreeMap<usize, u16> = programmer.contributions().collect();
            prop_assert_eq!(listed, expected);
        }
    }
}
