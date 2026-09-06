//! The playback layer: the source set, and resolving it.
//!
//! `docs/DMX_MERGE.md` §2. Every executor that can contribute to the output is
//! a [`PlaybackSource`]: a master level, an activation stamp, and a sparse set
//! of attribute values. [`PlaybackLayer::resolve`] turns the whole set into one
//! value per slot, applying HTP or LTP as each slot's merge mode says.
//!
//! Resolving takes `&self`. That is the point of the module: the output is a
//! function of the plan and the source set, with no hidden time in it. The
//! working memory it needs is passed in as a [`MergeScratch`] rather than
//! carried as state, so nothing about one tick can leak into the next.
//!
//! # Why the resolve is source-major
//!
//! The obvious shape — for each slot, ask every source what it has — costs
//! `slots × sources` every tick whether or not anything is running. A source
//! is instead asked for the slots it actually touches, and the per-slot winner
//! is accumulated in the scratch. A rig with sixty-four executors and eight
//! thousand attributes then costs what the *active cues* contain rather than
//! what the patch could theoretically hold.
//!
//! The price is that the accumulator has to exist. It is a buffer sized from
//! the plan, reset at the start of every resolve, so it is memory rather than
//! state — and `resolving_twice_gives_the_same_answer` in this module's tests
//! is what holds that claim up.

use prism_domain::{MergeMode, PlaybackId};

use crate::merge::{FULL, apply_master};
use crate::plan::{MergeError, MergePlan};

/// Upper bound on playback sources in one layer.
///
/// Each source carries a dense per-slot array, so the layer costs
/// `sources × slots`; the limit is what stops a corrupt show from turning that
/// product into a refusal to start rather than a huge allocation.
pub const MAX_SOURCES: usize = 1_024;

/// One playback as the merge sees it: a master level, an activation stamp and
/// the attribute values it is currently providing.
///
/// **It was one per executor until S40**, and is now one per
/// [`PlaybackId`] — a cue list playing on no fader is a source like any other,
/// with its master at full, which is the whole of what S40 needed from this
/// module. Nothing else about the merge moved: `docs/DMX_MERGE.md` §2 is written
/// about *sources*, and an executor is still one.
///
/// The values are sparse — a cue touches a handful of fixtures, not the whole
/// rig — but stored densely with a list of the slots that have been written.
/// Setting a value is therefore O(1) and cannot create a duplicate, while
/// iterating costs what the source actually contains.
#[derive(Debug, Clone)]
pub struct PlaybackSource {
    id: PlaybackId,
    activation: Option<u64>,
    master: u16,
    /// The level a held `Flash` is contributing at, if one is held.
    ///
    /// A **layer over** the master rather than a write into it
    /// (`docs/DMX_MERGE.md` §2.1 applies the master before the maximum, and this
    /// is what is applied). Releasing a flash therefore restores exactly the
    /// master that was stored, including one that arrived *while* the flash was
    /// held — which `SetExecutorMaster` writing into `master` gets right and a
    /// save-and-restore in the flash would lose.
    flash: Option<u16>,
    values: Box<[u16]>,
    present: Box<[bool]>,
    touched: Box<[u32]>,
    count: usize,
}

impl PlaybackSource {
    fn new(id: PlaybackId, slots: usize) -> Self {
        Self {
            id,
            activation: None,
            // A fader nobody has touched reads full: an executor switched on
            // before its master is moved has to produce light.
            master: FULL,
            flash: None,
            values: vec![0; slots].into_boxed_slice(),
            present: vec![false; slots].into_boxed_slice(),
            touched: vec![0; slots].into_boxed_slice(),
            count: 0,
        }
    }

    /// Which playback this source is.
    #[must_use]
    pub const fn id(&self) -> PlaybackId {
        self.id
    }

    /// Whether this source takes part in the merge at all.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.activation.is_some()
    }

    /// When this source was switched on, if it is on. Higher is more recent;
    /// this is the whole of LTP ordering (`docs/DMX_MERGE.md` §2.2).
    #[must_use]
    pub const fn activation(&self) -> Option<u64> {
        self.activation
    }

    /// The playback's **stored** master level, `0..=65535`.
    ///
    /// What the show holds and what a rebuild puts back. A held flash does not
    /// appear here — see [`Self::flash`] and [`Self::effective_master`].
    #[must_use]
    pub const fn master(&self) -> u16 {
        self.master
    }

    /// The level a held flash is contributing at, if one is held.
    #[must_use]
    pub const fn flash(&self) -> Option<u16> {
        self.flash
    }

    /// The master the merge actually applies: the flash while one is held, and
    /// the stored master otherwise.
    #[must_use]
    pub const fn effective_master(&self) -> u16 {
        match self.flash {
            Some(level) => level,
            None => self.master,
        }
    }

    /// How many attributes this source is providing.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Whether this source is providing nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Provides a value for one slot, replacing whatever was there.
    ///
    /// Returns `false` if the slot does not exist in the plan this source was
    /// built for — the tick's answer to an impossible write is to ignore it,
    /// never to panic.
    pub fn set(&mut self, slot: usize, value: u16) -> bool {
        let (Some(cell), Some(present)) = (self.values.get_mut(slot), self.present.get_mut(slot))
        else {
            return false;
        };
        *cell = value;
        if !*present {
            *present = true;
            if let Some(entry) = self.touched.get_mut(self.count) {
                *entry = slot as u32;
                self.count += 1;
            }
        }
        true
    }

    /// The value this source provides for a slot, if it provides one.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<u16> {
        match self.present.get(slot) {
            Some(true) => self.values.get(slot).copied(),
            _ => None,
        }
    }

    /// Every value this source provides, in the order they were first set.
    pub fn contributions(&self) -> impl Iterator<Item = (usize, u16)> + '_ {
        self.touched.iter().take(self.count).filter_map(|slot| {
            let slot = *slot as usize;
            Some((slot, self.values.get(slot).copied()?))
        })
    }

    /// Drops every value. S5 clears and refills a source as its cue is
    /// evaluated, so a value surviving this would be a light that never goes
    /// out.
    pub fn clear(&mut self) {
        for slot in self.touched.iter().take(self.count) {
            if let Some(present) = self.present.get_mut(*slot as usize) {
                *present = false;
            }
        }
        self.count = 0;
    }
}

/// Working memory for one resolve.
///
/// Sized from the plan and reused every tick. It is passed in rather than held
/// by the layer so that [`PlaybackLayer::resolve`] can take `&self`: the merge
/// is a function of the plan and the source set, and the type signature is
/// where that is easiest to keep honest.
#[derive(Debug, Clone)]
pub struct MergeScratch {
    accumulators: Box<[Accumulator]>,
}

impl MergeScratch {
    /// Working memory for the given plan.
    #[must_use]
    pub fn new(plan: &MergePlan) -> Self {
        Self {
            accumulators: vec![Accumulator::EMPTY; plan.slot_count()].into_boxed_slice(),
        }
    }
}

/// The winner so far for one slot.
#[derive(Debug, Clone, Copy)]
struct Accumulator {
    /// LTP ordering key of the winning source: `(activation, playback)`.
    order: (u64, u64),
    /// The winning value — an LTP source's value, or the running HTP maximum.
    value: u16,
    /// Whether any source has contributed. Distinguishes "the merge resolved to
    /// zero" from "nothing is active", which is the difference between a dark
    /// fixture and a fixture at home.
    covered: bool,
}

impl Accumulator {
    const EMPTY: Self = Self {
        order: (0, 0),
        value: 0,
        covered: false,
    };
}

/// Every playback source in the show, and the merge over them.
#[derive(Debug, Clone)]
pub struct PlaybackLayer {
    /// Sorted by executor number, so lookups are a binary search.
    sources: Box<[PlaybackSource]>,
    next_activation: u64,
}

impl PlaybackLayer {
    /// One source per executor, none of them active.
    ///
    /// Duplicated executor numbers collapse into one source.
    ///
    /// # Errors
    ///
    /// [`MergeError::TooManySources`] if there are more than [`MAX_SOURCES`].
    pub fn new(
        plan: &MergePlan,
        playbacks: impl IntoIterator<Item: Into<PlaybackId>>,
    ) -> Result<Self, MergeError> {
        let mut ids: Vec<PlaybackId> = playbacks.into_iter().map(Into::into).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() > MAX_SOURCES {
            return Err(MergeError::TooManySources(ids.len()));
        }
        let sources = ids
            .into_iter()
            .map(|id| PlaybackSource::new(id, plan.slot_count()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            sources,
            next_activation: 0,
        })
    }

    /// How many sources this layer holds.
    #[must_use]
    pub const fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Every source, in playback order.
    #[must_use]
    pub const fn sources(&self) -> &[PlaybackSource] {
        &self.sources
    }

    fn index_of(&self, id: PlaybackId) -> Option<usize> {
        self.sources
            .binary_search_by_key(&id, PlaybackSource::id)
            .ok()
    }

    // The five lookups below take `impl Into<PlaybackId>` so that every caller
    // that names an executor - which is nearly all of them - reads as it did
    // before S40. See `prism_domain::PlaybackId`'s `From` implementations.

    /// One source by playback.
    #[must_use]
    pub fn source(&self, id: impl Into<PlaybackId>) -> Option<&PlaybackSource> {
        self.sources.get(self.index_of(id.into())?)
    }

    /// One source by playback, for writing values into.
    pub fn source_mut(&mut self, id: impl Into<PlaybackId>) -> Option<&mut PlaybackSource> {
        let index = self.index_of(id.into())?;
        self.sources.get_mut(index)
    }

    /// Switches a source on and stamps it with the next activation counter.
    ///
    /// Returns `false` if the playback is unknown **or already active**: one
    /// that is already on has not been turned on again, so pressing its button a
    /// second time must not reorder the rig under the operator.
    /// `docs/DMX_MERGE.md` §2.2 orders by when a source "goes active".
    pub fn activate(&mut self, id: impl Into<PlaybackId>) -> bool {
        let stamp = self.next_activation;
        let Some(source) = self.source_mut(id) else {
            return false;
        };
        if source.activation.is_some() {
            return false;
        }
        source.activation = Some(stamp);
        // Saturating rather than wrapping: at one activation per tick this runs
        // for four hundred million years, and a wrapped counter would silently
        // reverse the LTP order.
        self.next_activation = stamp.saturating_add(1);
        true
    }

    /// Switches a source off. Returns `false` if it was unknown or already off.
    pub fn deactivate(&mut self, id: impl Into<PlaybackId>) -> bool {
        match self.source_mut(id) {
            Some(source) if source.activation.is_some() => {
                source.activation = None;
                true
            }
            _ => false,
        }
    }

    /// Sets a playback's stored master level. Returns `false` if it is unknown.
    ///
    /// Writes the stored level even while a flash is held: the flash goes on
    /// overriding it until it is released, and then the level that arrived
    /// meanwhile is the one that stands. A flash that had saved the old value
    /// and put it back would silently throw that command away.
    pub fn set_master(&mut self, id: impl Into<PlaybackId>, level: u16) -> bool {
        match self.source_mut(id) {
            Some(source) => {
                source.master = level;
                true
            }
            None => false,
        }
    }

    /// Holds or releases a flash over a playback's master.
    ///
    /// Returns `false` if the playback is unknown. `Some(level)` holds the
    /// flash at that level, `None` releases it and the stored master takes over
    /// again — byte for byte, because it was never written to.
    pub fn set_flash(&mut self, id: impl Into<PlaybackId>, level: Option<u16>) -> bool {
        match self.source_mut(id) {
            Some(source) => {
                source.flash = level;
                true
            }
            None => false,
        }
    }

    /// Resolves every slot in the plan into `out`.
    ///
    /// `docs/DMX_MERGE.md` §2, entire: HTP slots take the maximum of the
    /// mastered contributions, LTP slots take the value of the most recently
    /// activated source that provides them, and a slot no active source
    /// provides falls back to its home value.
    ///
    /// The result does not depend on the order of the sources, and calling this
    /// twice with the same source set gives the same answer twice. `scratch`
    /// must have been built from the same plan; `out` may be shorter, in which
    /// case it is filled as far as it goes rather than panicking — a tick that
    /// panics costs a frame (`CLAUDE.md`).
    ///
    /// Allocation-free, lock-free, and does no I/O: this runs on the tick.
    pub fn resolve(&self, plan: &MergePlan, scratch: &mut MergeScratch, out: &mut [u16]) {
        for accumulator in &mut scratch.accumulators {
            *accumulator = Accumulator::EMPTY;
        }

        for source in &self.sources {
            let Some(activation) = source.activation else {
                continue;
            };
            // The tie-break when two sources went active on the same tick.
            // `PlaybackId::key` puts every executor before every sequence
            // playback, so a desk's own faders win it - see that type.
            let order = (activation, source.id.key());
            for (slot, value) in source.contributions() {
                let (Some(definition), Some(accumulator)) =
                    (plan.slot(slot), scratch.accumulators.get_mut(slot))
                else {
                    continue;
                };
                match definition.merge_mode {
                    MergeMode::Htp => {
                        // The master is applied here, before the maximum, not to
                        // the winner afterwards - `docs/DMX_MERGE.md` §2.1.
                        let mastered = apply_master(value, source.effective_master());
                        if !accumulator.covered || mastered > accumulator.value {
                            accumulator.value = mastered;
                        }
                    }
                    MergeMode::Ltp => {
                        // No master: half a pan position is not a position (§2.3).
                        if !accumulator.covered || order > accumulator.order {
                            accumulator.order = order;
                            accumulator.value = value;
                        }
                    }
                }
                accumulator.covered = true;
            }
        }

        for ((definition, accumulator), slot) in plan
            .slots()
            .iter()
            .zip(scratch.accumulators.iter())
            .zip(out.iter_mut())
        {
            *slot = if accumulator.covered {
                accumulator.value
            } else {
                definition.home
            };
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::merge::{FULL, SourceValue, merge_playbacks};
    use crate::plan::{MergeError, MergePlan};
    use crate::playback::{MAX_SOURCES, MergeScratch, PlaybackLayer};
    use crate::testkit::{attribute_def, fixture_type, moving_head};
    use prism_domain::AttributeKey;
    use prism_domain::{AttributeType, FixtureId, MergeMode, SequenceId};
    use proptest::prelude::*;

    /// Three moving heads: six slots, alternating HTP dimmer and LTP pan.
    fn plan() -> MergePlan {
        let head = moving_head();
        MergePlan::build((1..=3).map(|id| (FixtureId::new(id), &head, false))).unwrap()
    }

    fn layer(plan: &MergePlan, sources: u32) -> PlaybackLayer {
        PlaybackLayer::new(plan, (1..=sources).map(SequenceId::new)).unwrap()
    }

    fn resolved(plan: &MergePlan, layer: &PlaybackLayer) -> Vec<u16> {
        let mut scratch = MergeScratch::new(plan);
        let mut out = vec![0u16; plan.slot_count()];
        layer.resolve(plan, &mut scratch, &mut out);
        out
    }

    fn slot(plan: &MergePlan, fixture: u32, attribute: AttributeType) -> usize {
        plan.index_of(FixtureId::new(fixture), AttributeKey::first(attribute))
            .unwrap()
    }

    #[test]
    fn nothing_active_resolves_to_the_home_layer() {
        // docs/DMX_MERGE.md 1: the bottom layer is never empty.
        let plan = plan();
        let layer = layer(&plan, 4);
        let values = resolved(&plan, &layer);
        let homes: Vec<u16> = plan.slots().iter().map(|slot| slot.home).collect();
        assert_eq!(values, homes);
        assert_eq!(values.len(), 6);
    }

    #[test]
    fn a_source_that_is_not_active_contributes_nothing_however_full_its_values_are() {
        let plan = plan();
        let mut layer = layer(&plan, 2);
        let pan = slot(&plan, 1, AttributeType::Pan);
        assert!(
            layer
                .source_mut(SequenceId::new(1))
                .unwrap()
                .set(pan, 60_000)
        );
        assert_eq!(resolved(&plan, &layer)[pan], 32_768);
        layer.activate(SequenceId::new(1));
        assert_eq!(resolved(&plan, &layer)[pan], 60_000);
    }

    #[test]
    fn activation_stamps_increase_and_an_already_active_source_keeps_its_place() {
        // "The last thing you turned on wins" is about turning something on. An
        // executor that was already on has not been turned on again, so pressing
        // its button a second time must not reorder the rig underneath the
        // operator.
        let plan = plan();
        let mut layer = layer(&plan, 3);
        assert!(layer.activate(SequenceId::new(1)));
        assert!(layer.activate(SequenceId::new(2)));
        let first = layer.source(SequenceId::new(1)).unwrap().activation();
        let second = layer.source(SequenceId::new(2)).unwrap().activation();
        assert!(first < second);

        // Already active: no new stamp, and the answer is false.
        assert!(!layer.activate(SequenceId::new(1)));
        assert_eq!(
            layer.source(SequenceId::new(1)).unwrap().activation(),
            first
        );
    }

    #[test]
    fn switching_a_source_off_and_on_again_moves_it_to_the_front() {
        let plan = plan();
        let mut layer = layer(&plan, 2);
        let pan = slot(&plan, 2, AttributeType::Pan);
        layer.source_mut(SequenceId::new(1)).unwrap().set(pan, 100);
        layer.source_mut(SequenceId::new(2)).unwrap().set(pan, 200);
        layer.activate(SequenceId::new(1));
        layer.activate(SequenceId::new(2));
        assert_eq!(resolved(&plan, &layer)[pan], 200);

        assert!(layer.deactivate(SequenceId::new(1)));
        assert!(layer.activate(SequenceId::new(1)));
        assert_eq!(resolved(&plan, &layer)[pan], 100);
    }

    #[test]
    fn deactivating_a_source_that_is_already_off_changes_nothing() {
        let plan = plan();
        let mut layer = layer(&plan, 1);
        assert!(!layer.deactivate(SequenceId::new(1)));
        assert!(!layer.source(SequenceId::new(1)).unwrap().is_active());
        assert_eq!(layer.source(SequenceId::new(1)).unwrap().activation(), None);
    }

    #[test]
    fn the_fallback_chain_runs_all_the_way_down_to_home() {
        // docs/DMX_MERGE.md 6.2, and the last paragraph of 7.
        let plan = plan();
        let mut layer = layer(&plan, 3);
        let pan = slot(&plan, 1, AttributeType::Pan);
        for (executor, value) in [(1u32, 10_000u16), (2, 20_000), (3, 30_000)] {
            layer
                .source_mut(SequenceId::new(executor))
                .unwrap()
                .set(pan, value);
            layer.activate(SequenceId::new(executor));
        }
        assert_eq!(resolved(&plan, &layer)[pan], 30_000);
        layer.deactivate(SequenceId::new(3));
        assert_eq!(resolved(&plan, &layer)[pan], 20_000);
        layer.deactivate(SequenceId::new(2));
        assert_eq!(resolved(&plan, &layer)[pan], 10_000);
        layer.deactivate(SequenceId::new(1));
        assert_eq!(resolved(&plan, &layer)[pan], 32_768);
    }

    #[test]
    fn the_master_scales_an_htp_slot_and_leaves_an_ltp_slot_alone() {
        // docs/DMX_MERGE.md 2.1 and 2.3 in one scenario.
        let plan = plan();
        let mut layer = layer(&plan, 1);
        let dimmer = slot(&plan, 1, AttributeType::Dimmer);
        let pan = slot(&plan, 1, AttributeType::Pan);
        {
            let source = layer.source_mut(SequenceId::new(1)).unwrap();
            source.set(dimmer, FULL);
            source.set(pan, 45_000);
        }
        layer.activate(SequenceId::new(1));
        assert!(layer.set_master(SequenceId::new(1), 32_767));

        let values = resolved(&plan, &layer);
        assert_eq!(values[dimmer], 32_767);
        assert_eq!(values[pan], 45_000);
    }

    #[test]
    fn the_master_is_applied_before_the_maximum_not_after() {
        // The one arithmetic ordering docs/DMX_MERGE.md 2.1 is explicit about.
        // Applied after the maximum, the answer here would be 32767; applied
        // before it, the second executor's unmastered 40000 wins.
        let plan = plan();
        let mut layer = layer(&plan, 2);
        let dimmer = slot(&plan, 1, AttributeType::Dimmer);
        layer
            .source_mut(SequenceId::new(1))
            .unwrap()
            .set(dimmer, FULL);
        layer
            .source_mut(SequenceId::new(2))
            .unwrap()
            .set(dimmer, 40_000);
        layer.activate(SequenceId::new(1));
        layer.activate(SequenceId::new(2));
        layer.set_master(SequenceId::new(1), 32_767);
        assert_eq!(resolved(&plan, &layer)[dimmer], 40_000);
    }

    #[test]
    fn an_executor_fading_out_cannot_darken_what_another_one_is_holding_up() {
        // The operational reason HTP exists at all, per docs/DMX_MERGE.md 2.
        let plan = plan();
        let mut layer = layer(&plan, 2);
        let dimmer = slot(&plan, 1, AttributeType::Dimmer);
        layer
            .source_mut(SequenceId::new(1))
            .unwrap()
            .set(dimmer, FULL);
        layer
            .source_mut(SequenceId::new(2))
            .unwrap()
            .set(dimmer, FULL);
        layer.activate(SequenceId::new(1));
        layer.activate(SequenceId::new(2));
        for level in [FULL, 40_000, 20_000, 0] {
            layer.set_master(SequenceId::new(2), level);
            assert_eq!(resolved(&plan, &layer)[dimmer], FULL, "master {level}");
        }
    }

    #[test]
    fn contributions_are_sparse_and_a_second_write_replaces_the_first() {
        let plan = plan();
        let mut layer = layer(&plan, 1);
        let pan = slot(&plan, 1, AttributeType::Pan);
        let source = layer.source_mut(SequenceId::new(1)).unwrap();
        assert!(source.is_empty());
        assert!(source.set(pan, 111));
        assert!(source.set(pan, 222));
        assert_eq!(source.len(), 1);
        assert_eq!(source.get(pan), Some(222));
        assert_eq!(source.get(slot(&plan, 2, AttributeType::Pan)), None);
        assert_eq!(source.contributions().collect::<Vec<_>>(), [(pan, 222)]);
    }

    #[test]
    fn clearing_a_source_removes_every_contribution_it_had() {
        // S5 clears and refills a source each tick as its cue is evaluated, so
        // a stale value surviving a clear would be a light that never goes out.
        let plan = plan();
        let mut layer = layer(&plan, 1);
        let pan = slot(&plan, 1, AttributeType::Pan);
        let source = layer.source_mut(SequenceId::new(1)).unwrap();
        source.set(pan, 60_000);
        source.set(slot(&plan, 2, AttributeType::Pan), 60_000);
        source.clear();
        assert!(source.is_empty());
        assert_eq!(source.get(pan), None);
        layer.activate(SequenceId::new(1));
        assert_eq!(resolved(&plan, &layer)[pan], 32_768);
    }

    #[test]
    fn a_slot_outside_the_plan_is_refused_rather_than_written() {
        let plan = plan();
        let mut layer = layer(&plan, 1);
        let source = layer.source_mut(SequenceId::new(1)).unwrap();
        assert!(!source.set(plan.slot_count(), 1));
        assert!(!source.set(usize::MAX, 1));
        assert!(source.is_empty());
        assert_eq!(source.get(plan.slot_count()), None);
    }

    #[test]
    fn an_executor_the_layer_does_not_know_about_is_ignored() {
        let plan = plan();
        let mut layer = layer(&plan, 2);
        assert!(layer.source(SequenceId::new(9)).is_none());
        assert!(layer.source_mut(SequenceId::new(9)).is_none());
        assert!(!layer.activate(SequenceId::new(9)));
        assert!(!layer.deactivate(SequenceId::new(9)));
        assert!(!layer.set_master(SequenceId::new(9), 100));
    }

    #[test]
    fn an_executor_listed_twice_becomes_one_source() {
        let plan = plan();
        let ids = [1u32, 5, 1, 5, 5].map(SequenceId::new);
        let layer = PlaybackLayer::new(&plan, ids).unwrap();
        assert_eq!(layer.source_count(), 2);
        assert!(layer.source(SequenceId::new(1)).is_some());
        assert!(layer.source(SequenceId::new(5)).is_some());
    }

    #[test]
    fn a_new_source_starts_off_at_full_master() {
        // A fader that has never been touched reads full, not zero: an executor
        // switched on before anyone moves its fader must produce light.
        let plan = plan();
        let layer = layer(&plan, 1);
        let source = layer.source(SequenceId::new(1)).unwrap();
        assert_eq!(source.master(), FULL);
        assert!(!source.is_active());
        assert_eq!(source.id(), SequenceId::new(1).into());
    }

    #[test]
    fn an_absurd_number_of_sources_is_rejected_rather_than_allocated() {
        let plan = plan();
        let ids = (0..=MAX_SOURCES as u32).map(SequenceId::new);
        assert_eq!(
            PlaybackLayer::new(&plan, ids).unwrap_err(),
            MergeError::TooManySources(MAX_SOURCES + 1)
        );
    }

    #[test]
    fn a_layer_over_an_empty_plan_resolves_to_nothing() {
        let plan = MergePlan::build(std::iter::empty()).unwrap();
        let mut layer = PlaybackLayer::new(&plan, [SequenceId::new(1)]).unwrap();
        layer.activate(SequenceId::new(1));
        assert!(resolved(&plan, &layer).is_empty());
    }

    #[test]
    fn resolving_into_a_short_buffer_fills_what_it_can_and_does_not_panic() {
        // The tick must not be able to panic on a mis-sized buffer: CLAUDE.md's
        // zero-crash invariant costs a frame for every panic.
        let plan = plan();
        let layer = layer(&plan, 1);
        let mut scratch = MergeScratch::new(&plan);
        let mut out = [0u16; 2];
        layer.resolve(&plan, &mut scratch, &mut out);
        assert_eq!(out, [0, 32_768]);
    }

    #[test]
    fn a_scratch_built_for_a_smaller_plan_degrades_instead_of_panicking() {
        // The scratch is documented as belonging to the plan it was built from.
        // If a caller ever gets that wrong, the tick must lose values, not the
        // show: `CLAUDE.md`'s zero-crash invariant does not stop at the parts
        // that are easy to get right.
        let plan = plan();
        let smaller = MergePlan::build([(FixtureId::new(1), &moving_head(), false)]).unwrap();
        let mut layer = layer(&plan, 1);
        let last = slot(&plan, 3, AttributeType::Pan);
        layer.source_mut(SequenceId::new(1)).unwrap().set(last, 999);
        layer.activate(SequenceId::new(1));

        let mut scratch = MergeScratch::new(&smaller);
        let mut out = vec![7u16; plan.slot_count()];
        layer.resolve(&plan, &mut scratch, &mut out);
        // Two slots resolved, the rest left as they were found.
        assert_eq!(out, [0, 32_768, 7, 7, 7, 7]);
    }

    #[test]
    fn resolving_twice_gives_the_same_answer() {
        // The scratch buffer is working memory, not state. If anything survived
        // a call, the second answer would differ from the first.
        let plan = plan();
        let mut layer = layer(&plan, 2);
        let dimmer = slot(&plan, 1, AttributeType::Dimmer);
        layer
            .source_mut(SequenceId::new(1))
            .unwrap()
            .set(dimmer, 40_000);
        layer.activate(SequenceId::new(1));
        let mut scratch = MergeScratch::new(&plan);
        let mut first = vec![0u16; plan.slot_count()];
        let mut second = vec![0u16; plan.slot_count()];
        layer.resolve(&plan, &mut scratch, &mut first);
        layer.resolve(&plan, &mut scratch, &mut second);
        assert_eq!(first, second);
        assert_eq!(first[dimmer], 40_000);
    }

    /// One generated source: whether it is active, its master, and a value for
    /// each slot or none.
    type SourceSpec = (bool, u16, Vec<Option<u16>>);

    fn source_specs(sources: usize, slots: usize) -> impl Strategy<Value = Vec<SourceSpec>> {
        proptest::collection::vec(
            (
                any::<bool>(),
                any::<u16>(),
                proptest::collection::vec(proptest::option::of(any::<u16>()), slots),
            ),
            0..sources,
        )
    }

    /// Loads the generated sources into a layer, activating them in list order
    /// so the activation stamps follow the list.
    fn load(plan: &MergePlan, specs: &[SourceSpec]) -> PlaybackLayer {
        let mut layer = PlaybackLayer::new(
            plan,
            (0..specs.len() as u32).map(|index| SequenceId::new(index + 1)),
        )
        .unwrap();
        for (index, (active, master, values)) in specs.iter().enumerate() {
            let executor = SequenceId::new(index as u32 + 1);
            layer.set_master(executor, *master);
            let source = layer.source_mut(executor).unwrap();
            for (slot, value) in values.iter().enumerate() {
                if let Some(value) = value {
                    source.set(slot, *value);
                }
            }
            if *active {
                layer.activate(executor);
            }
        }
        layer
    }

    proptest! {
        /// The resolver and the pure function in [`crate::merge`] must agree on
        /// every slot. The resolver exists only because a per-slot fold over
        /// every source would waste the tick; if it ever stops agreeing with the
        /// specification's arithmetic, it is the resolver that is wrong.
        #[test]
        fn the_resolver_agrees_with_the_pure_merge_on_every_slot(
            specs in source_specs(6, 6),
        ) {
            let plan = plan();
            let layer = load(&plan, &specs);
            let values = resolved(&plan, &layer);

            for (index, slot) in plan.slots().iter().enumerate() {
                let sources: Vec<SourceValue> = layer
                    .sources()
                    .iter()
                    .filter_map(|source| {
                        Some(SourceValue {
                            activation: source.activation()?,
                            executor: source.id(),
                            master: source.master(),
                            value: source.get(index)?,
                        })
                    })
                    .collect();
                let expected = merge_playbacks(slot.merge_mode, slot.home, &sources);
                prop_assert_eq!(values.get(index).copied(), Some(expected));
            }
        }

        /// Identical input, identical output — `docs/DMX_MERGE.md` §6.4. The
        /// layer is rebuilt from scratch each time, so nothing can carry over.
        #[test]
        fn the_same_source_set_always_produces_the_same_values(
            specs in source_specs(5, 6),
        ) {
            let plan = plan();
            let first = resolved(&plan, &load(&plan, &specs));
            let second = resolved(&plan, &load(&plan, &specs));
            prop_assert_eq!(first, second);
        }

        /// §6.1 monotonicity, at the layer rather than the operation: raising
        /// one source's contribution to an HTP slot can never lower the output.
        #[test]
        fn raising_one_source_never_lowers_an_htp_slot(
            specs in source_specs(5, 6),
            raise in any::<u16>(),
            which in 0usize..5,
        ) {
            let plan = plan();
            let before = resolved(&plan, &load(&plan, &specs));
            let mut raised = specs.clone();
            if let Some((active, _, values)) = raised.get_mut(which) {
                *active = true;
                for value in values.iter_mut() {
                    let current = value.unwrap_or(0);
                    *value = Some(current.saturating_add(raise));
                }
            }
            let after = resolved(&plan, &load(&plan, &raised));
            for (index, slot) in plan.slots().iter().enumerate() {
                if slot.merge_mode == MergeMode::Htp {
                    prop_assert!(
                        after.get(index) >= before.get(index),
                        "slot {} fell from {:?} to {:?}",
                        index,
                        before.get(index),
                        after.get(index)
                    );
                }
            }
        }

        /// §6.2, at the layer: the LTP winner is the most recently activated
        /// source that provides the slot, and deactivating it hands the slot to
        /// the next one down, all the way to home.
        #[test]
        fn deactivating_the_ltp_winner_falls_back_to_the_next_source(
            values in proptest::collection::vec(any::<u16>(), 1..6),
        ) {
            let plan = plan();
            let pan = slot(&plan, 1, AttributeType::Pan);
            let mut layer = PlaybackLayer::new(
                &plan,
                (0..values.len() as u32).map(|index| SequenceId::new(index + 1)),
            ).unwrap();
            for (index, value) in values.iter().enumerate() {
                let executor = SequenceId::new(index as u32 + 1);
                if let Some(source) = layer.source_mut(executor) {
                    source.set(pan, *value);
                }
                layer.activate(executor);
            }
            for (index, value) in values.iter().enumerate().rev() {
                prop_assert_eq!(resolved(&plan, &layer).get(pan).copied(), Some(*value));
                layer.deactivate(SequenceId::new(index as u32 + 1));
            }
            prop_assert_eq!(resolved(&plan, &layer).get(pan).copied(), Some(32_768));
        }

        /// A fixture type with an unusual merge mode still merges by the mode in
        /// its definition, not by the attribute's default: `AttributeDef.mergeMode`
        /// is the authority, per `docs/DMX_MERGE.md` §2.
        #[test]
        fn the_mode_comes_from_the_attribute_definition(
            a in any::<u16>(),
            b in any::<u16>(),
        ) {
            let mut def = attribute_def(AttributeType::Pan, 0);
            def.merge_mode = MergeMode::Htp;
            let odd = fixture_type("test.htp-pan", vec![def]);
            let plan = MergePlan::build([(FixtureId::new(1), &odd, false)]).unwrap();
            let mut layer = PlaybackLayer::new(
                &plan,
                [SequenceId::new(1), SequenceId::new(2)],
            ).unwrap();
            layer.source_mut(SequenceId::new(1)).unwrap().set(0, a);
            layer.source_mut(SequenceId::new(2)).unwrap().set(0, b);
            layer.activate(SequenceId::new(1));
            layer.activate(SequenceId::new(2));
            prop_assert_eq!(resolved(&plan, &layer).first().copied(), Some(a.max(b)));
        }
    }
}
