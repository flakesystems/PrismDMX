//! The masters: the top of the stack, and the only layer that scales rather
//! than selects.
//!
//! `docs/DMX_MERGE.md` §4, applied to the merged result in this order:
//!
//! 1. **Group masters** — scale intensity for the fixtures of a group
//! 2. **Grand master** — scales all intensity globally
//! 3. Speed masters — playback *rate*, not values; step 2 of the tick, not here
//!
//! Two constraints make this layer what it is.
//!
//! **Masters scale intensity only.** A grand master that dimmed colour values
//! would desaturate the rig on the way down instead of dimming it, and one that
//! scaled pan would swing every head to the left as it came down. Which
//! attributes count as intensity is decided by the attribute *definition* —
//! [`crate::AttributeSlot::is_intensity`] — not by a name and not by a merge
//! mode.
//!
//! **A master scales, it never selects.** [`crate::apply_master`] at full scale
//! is exactly the identity, so a desk with every master up produces precisely
//! what the merge produced. That is asserted rather than assumed, because it is
//! the property an operator relies on without ever thinking about it.
//!
//! # A fixture in two groups
//!
//! Group memberships overlap: the same head is usually in "all heads" and in
//! "stage left". Two rules were available — multiply the masters together, or
//! take the lowest — and this layer takes the **lowest**. A group master is an
//! inhibitive master, an answer to "how much of this fixture's light may pass",
//! and when two constraints apply the tighter one binds. Multiplying would give
//! a quarter of the light for two faders at half, which is a number no operator
//! predicted from either fader, and would make the result depend on how many
//! groups a fixture happens to belong to.

use std::collections::BTreeMap;

use prism_domain::{FixtureId, Group, GroupId};

use crate::merge::{FULL, apply_master};
use crate::plan::MergePlan;

/// The grand master, the blackout and the group masters, over one patch.
///
/// Every table is sized from the plan (and from the group list) up front, so
/// [`Self::apply`] neither allocates nor searches: it walks the intensity slots
/// and the groups each one belongs to.
#[derive(Debug, Clone)]
pub struct MasterLayer {
    /// Grand master level, `0..=65535`.
    grand: u16,
    /// Whether blackout is engaged.
    blackout: bool,
    /// The slots the masters may touch, ascending. Everything else is invisible
    /// to this layer.
    intensity: Box<[u32]>,
    /// Group numbers, sorted, so a command can find one by binary search.
    groups: Box<[GroupId]>,
    /// One level per entry of `groups`.
    levels: Box<[u16]>,
    /// Where each intensity slot's group list starts in `members`. One longer
    /// than `intensity`.
    member_offsets: Box<[u32]>,
    /// Indices into `groups`, grouped by intensity slot.
    members: Box<[u32]>,
}

impl MasterLayer {
    /// Masters over `plan` with no groups: grand master at full, blackout off.
    ///
    /// A desk in this state is exactly transparent, which is what makes it a
    /// safe default for a body that is built before any master is known.
    #[must_use]
    pub fn new(plan: &MergePlan) -> Self {
        let intensity: Vec<u32> = plan
            .slots()
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.is_intensity())
            .map(|(index, _)| index as u32)
            .collect();
        let member_offsets = vec![0; intensity.len() + 1].into_boxed_slice();
        Self {
            grand: FULL,
            blackout: false,
            intensity: intensity.into_boxed_slice(),
            groups: Box::new([]),
            levels: Box::new([]),
            member_offsets,
            members: Box::new([]),
        }
    }

    /// Masters over `plan` with the show's groups on them, every group at full.
    #[must_use]
    pub fn for_groups(plan: &MergePlan, groups: &[Group]) -> Self {
        let mut layer = Self::new(plan);
        layer.set_groups(plan, groups);
        layer
    }

    /// Replaces the group table, keeping the grand master and the blackout.
    ///
    /// Allocates: this is set-up work, done when the show's groups change, not
    /// something to do while the tick runs. Every group starts at full, so a
    /// reload does not darken the stage.
    ///
    /// A group naming a fixture this patch does not have contributes nothing;
    /// the group itself still exists and still has a master, so an operator who
    /// repatches does not lose a fader.
    pub fn set_groups(&mut self, plan: &MergePlan, groups: &[Group]) {
        let mut ids: Vec<GroupId> = groups.iter().map(|group| group.id).collect();
        ids.sort_unstable();
        ids.dedup();

        // Which intensity slots each fixture owns, once, so a group of a
        // thousand heads is a thousand lookups rather than a walk of the patch
        // per member.
        let mut by_fixture: BTreeMap<FixtureId, Vec<u32>> = BTreeMap::new();
        for (position, slot) in self.intensity.iter().enumerate() {
            if let Some(definition) = plan.slot(*slot as usize) {
                by_fixture
                    .entry(definition.fixture)
                    .or_default()
                    .push(position as u32);
            }
        }

        // Group-major over the deduplicated numbers rather than over the list:
        // asking each number which entries carry it cannot fail, where looking
        // each entry's number up would have a branch nothing can reach.
        let mut per_slot: Vec<Vec<u32>> = vec![Vec::new(); self.intensity.len()];
        for (index, id) in ids.iter().enumerate() {
            let positions = groups
                .iter()
                .filter(|group| group.id == *id)
                .flat_map(|group| group.fixtures.iter())
                .filter_map(|fixture| by_fixture.get(fixture))
                .flatten();
            for position in positions {
                // A fixture listed twice in one group, or two entries sharing a
                // group number, must not scale it twice over.
                if let Some(entry) = per_slot.get_mut(*position as usize)
                    && !entry.contains(&(index as u32))
                {
                    entry.push(index as u32);
                }
            }
        }

        let mut member_offsets = Vec::with_capacity(per_slot.len() + 1);
        let mut members = Vec::new();
        member_offsets.push(0);
        for entry in &per_slot {
            members.extend_from_slice(entry);
            member_offsets.push(members.len() as u32);
        }

        self.levels = vec![FULL; ids.len()].into_boxed_slice();
        self.groups = ids.into_boxed_slice();
        self.member_offsets = member_offsets.into_boxed_slice();
        self.members = members.into_boxed_slice();
    }

    /// The grand master level, `0..=65535`.
    #[must_use]
    pub const fn grand(&self) -> u16 {
        self.grand
    }

    /// Moves the grand master.
    pub const fn set_grand(&mut self, level: u16) {
        self.grand = level;
    }

    /// Whether blackout is engaged.
    #[must_use]
    pub const fn blackout(&self) -> bool {
        self.blackout
    }

    /// Engages or releases blackout.
    ///
    /// Blackout is not a grand master at zero that has to be put back: it is a
    /// separate switch, and releasing it restores whatever the grand master was
    /// left at. An operator who blacks out and comes back finds the fader where
    /// they left it.
    pub const fn set_blackout(&mut self, on: bool) {
        self.blackout = on;
    }

    /// The level the masters actually multiply by — the grand master, or zero
    /// while blackout is engaged.
    #[must_use]
    pub const fn effective_grand(&self) -> u16 {
        if self.blackout { 0 } else { self.grand }
    }

    /// How many groups have a master.
    #[must_use]
    pub const fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Every group with a master, in ascending order.
    #[must_use]
    pub const fn groups(&self) -> &[GroupId] {
        &self.groups
    }

    /// One group's master level, if the layer knows that group.
    #[must_use]
    pub fn group_level(&self, group: GroupId) -> Option<u16> {
        self.levels.get(self.index_of(group)?).copied()
    }

    /// Moves one group's master. Returns `false` if the group is unknown — a
    /// stale command must not stop the tick.
    pub fn set_group_level(&mut self, group: GroupId, level: u16) -> bool {
        let index = self.index_of(group);
        let Some(cell) = index.and_then(|index| self.levels.get_mut(index)) else {
            return false;
        };
        *cell = level;
        true
    }

    fn index_of(&self, group: GroupId) -> Option<usize> {
        self.groups.binary_search(&group).ok()
    }

    /// The slots the masters may touch, ascending.
    #[must_use]
    pub const fn intensity_slots(&self) -> &[u32] {
        &self.intensity
    }

    /// The lowest group master applying to the intensity slot at `position` in
    /// [`Self::intensity_slots`], or [`FULL`] if it is in no group.
    fn group_ceiling(&self, position: usize) -> u16 {
        // One expression rather than three statements: the offsets and the
        // membership list are built together and cannot disagree, so an early
        // return per lookup would only add branches no input can reach. A slot
        // in no group has an empty range, which is the `unwrap_or` and is the
        // ordinary case for most of a rig.
        self.member_offsets
            .get(position)
            .zip(self.member_offsets.get(position + 1))
            .and_then(|(start, end)| self.members.get(*start as usize..*end as usize))
            .map_or(FULL, |memberships| {
                memberships
                    .iter()
                    .filter_map(|index| self.levels.get(*index as usize).copied())
                    .min()
                    .unwrap_or(FULL)
            })
    }

    /// Scales every intensity value and leaves every other value alone.
    ///
    /// `ARCHITECTURE_SPEC.md` §5 step 6, after the programmer: a grand master at
    /// zero blacks out an intensity the operator is holding in the programmer
    /// too, which is what makes it a grand master rather than a playback fader.
    ///
    /// Allocation-free and lock-free: this runs on the tick. It costs one pass
    /// over the intensity slots, and nothing at all for the rest of the patch.
    pub fn apply(&self, values: &mut [u16]) {
        let grand = self.effective_grand();
        for (position, slot) in self.intensity.iter().enumerate() {
            let Some(value) = values.get_mut(*slot as usize) else {
                continue;
            };
            // Groups first, then the grand master - `docs/DMX_MERGE.md` §4.
            let grouped = apply_master(*value, self.group_ceiling(position));
            *value = apply_master(grouped, grand);
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::master::MasterLayer;
    use crate::merge::{FULL, apply_master};
    use crate::plan::MergePlan;
    use crate::testkit::{attribute_def, fixture_type, moving_head};
    use prism_domain::AttributeKey;
    use prism_domain::{AttributeDef, AttributeType, FeatureGroup, FixtureId, Group, GroupId};
    use proptest::prelude::*;

    /// Three moving heads: six slots, alternating HTP dimmer and LTP pan.
    fn plan() -> MergePlan {
        let head = moving_head();
        MergePlan::build((1..=3).map(|id| (FixtureId::new(id), &head, false))).unwrap()
    }

    fn slot(plan: &MergePlan, fixture: u32, attribute: AttributeType) -> usize {
        plan.index_of(FixtureId::new(fixture), AttributeKey::first(attribute))
            .unwrap()
    }

    fn group(id: u32, fixtures: &[u32]) -> Group {
        Group {
            id: GroupId::new(id),
            name: format!("Group {id}"),
            fixtures: fixtures.iter().map(|id| FixtureId::new(*id)).collect(),
        }
    }

    /// The six slots at full, so any scaling shows up as a change.
    fn full() -> Vec<u16> {
        vec![FULL; 6]
    }

    #[test]
    fn a_fresh_master_layer_is_exactly_transparent() {
        // A master scales, it never selects: with the grand master up and no
        // groups, the desk hands through precisely what the merge produced.
        let plan = plan();
        let masters = MasterLayer::new(&plan);
        assert_eq!(masters.grand(), FULL);
        assert!(!masters.blackout());
        assert_eq!(masters.group_count(), 0);

        let mut values = vec![1u16, 2, 3, 4, 5, 6];
        masters.apply(&mut values);
        assert_eq!(values, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn only_the_intensity_slots_are_visible_to_the_masters() {
        let plan = plan();
        let masters = MasterLayer::new(&plan);
        let expected: Vec<u32> = (1..=3)
            .map(|fixture| slot(&plan, fixture, AttributeType::Dimmer) as u32)
            .collect();
        assert_eq!(masters.intensity_slots(), expected.as_slice());
    }

    #[test]
    fn the_grand_master_at_zero_zeroes_intensity_and_leaves_everything_else_alone() {
        // docs/DMX_MERGE.md 6.3, and the session's own exit criterion. A grand
        // master that dimmed colour would desaturate the rig on the way down;
        // one that scaled pan would swing every head as it came down.
        let plan = plan();
        let mut masters = MasterLayer::new(&plan);
        masters.set_grand(0);

        let mut values = full();
        masters.apply(&mut values);
        for (index, value) in values.iter().enumerate() {
            let intensity = plan.slot(index).unwrap().is_intensity();
            let expected = if intensity { 0 } else { FULL };
            assert_eq!(*value, expected, "slot {index}");
        }
    }

    #[test]
    fn the_grand_master_at_half_halves_intensity_only() {
        let plan = plan();
        let mut masters = MasterLayer::new(&plan);
        masters.set_grand(32_767);

        let mut values = vec![40_000u16; 6];
        masters.apply(&mut values);
        for (index, value) in values.iter().enumerate() {
            let expected = if plan.slot(index).unwrap().is_intensity() {
                apply_master(40_000, 32_767)
            } else {
                40_000
            };
            assert_eq!(*value, expected, "slot {index}");
        }
    }

    #[test]
    fn blackout_zeroes_intensity_and_releasing_it_gives_the_fader_back() {
        // Blackout is a switch, not a fader taken to zero: an operator who
        // blacks out mid-show and comes back must find the grand master where
        // they left it, not at zero and not at full.
        let plan = plan();
        let mut masters = MasterLayer::new(&plan);
        masters.set_grand(32_767);
        masters.set_blackout(true);
        assert!(masters.blackout());
        assert_eq!(masters.effective_grand(), 0);

        let dimmer = slot(&plan, 1, AttributeType::Dimmer);
        let pan = slot(&plan, 1, AttributeType::Pan);
        let mut values = full();
        masters.apply(&mut values);
        assert_eq!(values.get(dimmer).copied(), Some(0));
        assert_eq!(values.get(pan).copied(), Some(FULL), "blackout moved a pan");

        masters.set_blackout(false);
        assert_eq!(masters.grand(), 32_767);
        let mut values = full();
        masters.apply(&mut values);
        assert_eq!(
            values.get(dimmer).copied(),
            Some(apply_master(FULL, 32_767))
        );
    }

    #[test]
    fn a_group_master_scales_only_the_fixtures_in_that_group() {
        let plan = plan();
        let mut masters = MasterLayer::for_groups(&plan, &[group(4, &[1, 3])]);
        assert_eq!(masters.group_count(), 1);
        assert_eq!(masters.groups(), [GroupId::new(4)]);
        assert_eq!(masters.group_level(GroupId::new(4)), Some(FULL));
        assert!(masters.set_group_level(GroupId::new(4), 32_767));

        let mut values = full();
        masters.apply(&mut values);
        let halved = apply_master(FULL, 32_767);
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Dimmer)).copied(),
            Some(halved)
        );
        assert_eq!(
            values.get(slot(&plan, 3, AttributeType::Dimmer)).copied(),
            Some(halved)
        );
        // Fixture 2 is in no group, and no pan is touched at all.
        assert_eq!(
            values.get(slot(&plan, 2, AttributeType::Dimmer)).copied(),
            Some(FULL)
        );
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Pan)).copied(),
            Some(FULL)
        );
    }

    #[test]
    fn a_fixture_in_two_groups_is_held_down_by_the_lower_master() {
        // The rule this layer chose: a group master is inhibitive, so where two
        // apply the tighter one binds. Multiplying would give a quarter of the
        // light for two faders at half - a number neither fader predicts.
        let plan = plan();
        let mut masters =
            MasterLayer::for_groups(&plan, &[group(1, &[1, 2]), group(2, &[1]), group(3, &[])]);
        assert_eq!(masters.group_count(), 3);
        masters.set_group_level(GroupId::new(1), 32_767);
        masters.set_group_level(GroupId::new(2), 16_000);

        let mut values = full();
        masters.apply(&mut values);
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Dimmer)).copied(),
            Some(apply_master(FULL, 16_000)),
            "the lower of the two masters did not win"
        );
        assert_eq!(
            values.get(slot(&plan, 2, AttributeType::Dimmer)).copied(),
            Some(apply_master(FULL, 32_767))
        );
        assert_eq!(
            values.get(slot(&plan, 3, AttributeType::Dimmer)).copied(),
            Some(FULL)
        );
    }

    #[test]
    fn the_group_masters_run_before_the_grand_master() {
        // docs/DMX_MERGE.md 4 gives the order. Both are scalings so the product
        // is the same either way round, but the truncation is not, and the
        // number on the wire has to be the one the specification describes.
        let plan = plan();
        let mut masters = MasterLayer::for_groups(&plan, &[group(1, &[1])]);
        masters.set_group_level(GroupId::new(1), 30_000);
        masters.set_grand(40_000);

        let mut values = full();
        masters.apply(&mut values);
        let expected = apply_master(apply_master(FULL, 30_000), 40_000);
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Dimmer)).copied(),
            Some(expected)
        );
    }

    #[test]
    fn a_group_naming_an_unpatched_fixture_still_has_a_master() {
        // A show outlives its rig. The group keeps its fader; the fixture that
        // is no longer there simply is not scaled by it.
        let plan = plan();
        let masters = MasterLayer::for_groups(&plan, &[group(7, &[9])]);
        assert_eq!(masters.group_level(GroupId::new(7)), Some(FULL));
        let mut values = full();
        masters.apply(&mut values);
        assert_eq!(values, full());
    }

    #[test]
    fn a_command_for_a_group_this_layer_does_not_have_is_ignored() {
        let plan = plan();
        let mut masters = MasterLayer::for_groups(&plan, &[group(1, &[1])]);
        assert!(!masters.set_group_level(GroupId::new(99), 0));
        assert_eq!(masters.group_level(GroupId::new(99)), None);
        let mut values = full();
        masters.apply(&mut values);
        assert_eq!(values, full());
    }

    #[test]
    fn the_same_group_number_twice_is_one_master_over_both_memberships() {
        // Two entries with the same number are one group as far as its fader is
        // concerned; the alternative is a second master nothing can reach.
        let plan = plan();
        let mut masters = MasterLayer::for_groups(&plan, &[group(1, &[1]), group(1, &[2])]);
        assert_eq!(masters.group_count(), 1);
        masters.set_group_level(GroupId::new(1), 0);
        let mut values = full();
        masters.apply(&mut values);
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Dimmer)).copied(),
            Some(0)
        );
        assert_eq!(
            values.get(slot(&plan, 2, AttributeType::Dimmer)).copied(),
            Some(0)
        );
    }

    #[test]
    fn a_fixture_listed_twice_in_one_group_is_scaled_once() {
        let plan = plan();
        let mut masters = MasterLayer::for_groups(&plan, &[group(1, &[1, 1])]);
        masters.set_group_level(GroupId::new(1), 32_767);
        let mut values = full();
        masters.apply(&mut values);
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Dimmer)).copied(),
            Some(apply_master(FULL, 32_767))
        );
    }

    #[test]
    fn reloading_the_groups_keeps_the_grand_master_and_starts_every_group_at_full() {
        let plan = plan();
        let mut masters = MasterLayer::for_groups(&plan, &[group(1, &[1])]);
        masters.set_grand(20_000);
        masters.set_group_level(GroupId::new(1), 0);
        masters.set_groups(&plan, &[group(1, &[1]), group(2, &[2])]);

        assert_eq!(masters.grand(), 20_000);
        assert_eq!(masters.group_count(), 2);
        assert_eq!(masters.group_level(GroupId::new(1)), Some(FULL));
        // A reload must not black the stage out, which a group table rebuilt at
        // zero would do.
        let mut values = full();
        masters.apply(&mut values);
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Dimmer)).copied(),
            Some(apply_master(FULL, 20_000))
        );
    }

    #[test]
    fn the_masters_scale_what_the_definition_calls_intensity_not_what_is_called_dimmer() {
        // The criterion is `AttributeDef.featureGroup`, which a profile sets per
        // attribute - not the attribute's name. Asserted both ways round, so
        // neither a rename nor a shortcut through `AttributeType` can pass.
        let odd = fixture_type(
            "test.odd",
            vec![
                AttributeDef {
                    feature_group: FeatureGroup::Beam,
                    ..attribute_def(AttributeType::Dimmer, 0)
                },
                AttributeDef {
                    feature_group: FeatureGroup::Dimmer,
                    ..attribute_def(AttributeType::Shutter, 0)
                },
            ],
        );
        let plan = MergePlan::build([(FixtureId::new(1), &odd, false)]).unwrap();
        let mut masters = MasterLayer::new(&plan);
        masters.set_grand(0);

        let mut values = vec![FULL; 2];
        masters.apply(&mut values);
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Dimmer)).copied(),
            Some(FULL),
            "the grand master dimmed an attribute filed under Beam"
        );
        assert_eq!(
            values.get(slot(&plan, 1, AttributeType::Shutter)).copied(),
            Some(0),
            "the grand master missed an attribute filed under Dimmer"
        );
    }

    #[test]
    fn applying_to_a_shorter_buffer_fills_what_it_can() {
        let plan = plan();
        let mut masters = MasterLayer::new(&plan);
        masters.set_grand(0);
        let mut values = vec![FULL; 2];
        masters.apply(&mut values);
        assert_eq!(values, [0, FULL]);
    }

    #[test]
    fn a_patch_with_nothing_in_it_has_no_intensity_slots() {
        let plan = MergePlan::build(std::iter::empty()).unwrap();
        let masters = MasterLayer::for_groups(&plan, &[group(1, &[1])]);
        assert!(masters.intensity_slots().is_empty());
        let mut values: Vec<u16> = Vec::new();
        masters.apply(&mut values);
        assert!(values.is_empty());
    }

    proptest! {
        /// `docs/DMX_MERGE.md` §6.3 — the grand master at full is a no-op,
        /// whatever the values and whatever the groups are doing, as long as
        /// they are up too.
        #[test]
        fn a_full_desk_changes_nothing(values in proptest::collection::vec(any::<u16>(), 6)) {
            let plan = plan();
            let masters = MasterLayer::for_groups(&plan, &[group(1, &[1, 2, 3])]);
            let mut scaled = values.clone();
            masters.apply(&mut scaled);
            prop_assert_eq!(scaled, values);
        }

        /// §6.3 — the grand master at zero forces **every** intensity to zero
        /// and leaves **every** other attribute exactly as it was.
        #[test]
        fn a_grand_master_at_zero_zeroes_intensity_and_only_intensity(
            values in proptest::collection::vec(any::<u16>(), 6),
            blackout in any::<bool>(),
        ) {
            let plan = plan();
            let mut masters = MasterLayer::new(&plan);
            if blackout {
                masters.set_blackout(true);
            } else {
                masters.set_grand(0);
            }
            let mut scaled = values.clone();
            masters.apply(&mut scaled);
            for (index, before) in values.iter().enumerate() {
                let expected = if plan.slot(index).unwrap().is_intensity() { 0 } else { *before };
                prop_assert_eq!(scaled.get(index).copied(), Some(expected));
            }
        }

        /// A master only ever attenuates: no setting of any fader can make a
        /// value larger than the merge produced.
        #[test]
        fn the_masters_never_raise_a_value(
            values in proptest::collection::vec(any::<u16>(), 6),
            grand in any::<u16>(),
            first in any::<u16>(),
            second in any::<u16>(),
        ) {
            let plan = plan();
            let mut masters = MasterLayer::for_groups(&plan, &[group(1, &[1, 2]), group(2, &[1])]);
            masters.set_grand(grand);
            masters.set_group_level(GroupId::new(1), first);
            masters.set_group_level(GroupId::new(2), second);
            let mut scaled = values.clone();
            masters.apply(&mut scaled);
            for (index, before) in values.iter().enumerate() {
                prop_assert!(scaled.get(index).copied().unwrap_or(0) <= *before);
            }
        }

        /// A fixture in several groups is scaled by the lowest of them, and by
        /// nothing else — asserted against the arithmetic rather than against
        /// another implementation of the same walk.
        #[test]
        fn the_lowest_group_master_is_the_one_that_applies(
            levels in proptest::collection::vec(any::<u16>(), 3),
            grand in any::<u16>(),
            value in any::<u16>(),
        ) {
            let plan = plan();
            let mut masters = MasterLayer::for_groups(
                &plan,
                &[group(1, &[1]), group(2, &[1]), group(3, &[1])],
            );
            masters.set_grand(grand);
            for (index, level) in levels.iter().enumerate() {
                masters.set_group_level(GroupId::new(index as u32 + 1), *level);
            }
            let mut values = vec![value; 6];
            masters.apply(&mut values);
            let lowest = levels.iter().copied().min().unwrap_or(FULL);
            let expected = apply_master(apply_master(value, lowest), grand);
            prop_assert_eq!(
                values.get(slot(&plan, 1, AttributeType::Dimmer)).copied(),
                Some(expected)
            );
        }

        /// Applying twice is applying once only when the desk is transparent —
        /// but applying the *same* desk to the same input always gives the same
        /// answer. Determinism at the level of this layer.
        #[test]
        fn applying_the_same_desk_twice_to_the_same_input_gives_the_same_answer(
            values in proptest::collection::vec(any::<u16>(), 6),
            grand in any::<u16>(),
            level in any::<u16>(),
        ) {
            let plan = plan();
            let mut masters = MasterLayer::for_groups(&plan, &[group(1, &[1, 2, 3])]);
            masters.set_grand(grand);
            masters.set_group_level(GroupId::new(1), level);
            let mut once = values.clone();
            masters.apply(&mut once);
            let mut again = values;
            masters.apply(&mut again);
            prop_assert_eq!(once, again);
        }
    }
}
