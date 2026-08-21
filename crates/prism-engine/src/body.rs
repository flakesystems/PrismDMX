//! The merge as a [`crate::TickBody`]: the seam S2 left open.
//!
//! [`MergeBody`] owns a [`crate::MergePlan`], the [`crate::PlaybackLayer`] over
//! it and the buffers the resolve needs, and runs the merge once per tick.
//! Everything it touches is sized when the body is built, so the tick itself
//! never reaches the allocator — `crates/prism-engine/tests/tick_allocations.rs`
//! counts that rather than asserting it.
//!
//! The tick runs `ARCHITECTURE_SPEC.md` §5 in order, and the order is the whole
//! point of this module:
//!
//! 1. the playbacks are advanced and evaluated — steps 2 and 3, `crate::player`
//! 2. the source set is merged — step 4, `crate::playback`
//! 3. the programmer overrides it — step 5, `crate::programmer`
//! 4. the masters scale what is left — step 6, `crate::master`
//! 5. the result is encoded into the frame — step 7, `crate::encode`
//!
//! Steps 3 and 4 are not interchangeable. The programmer sits *below* the
//! masters, so a grand master at zero blacks out an intensity the operator is
//! holding in the programmer as surely as one a cue is holding — which is what
//! makes it a grand master rather than one more playback fader. The other way
//! round, blackout would be a suggestion an operator could lose to their own
//! programmer.
//!
//! # What lives elsewhere
//!
//! The programmer *state machine* — the three-stage Clear, what a store does,
//! the selection — is `prism-core` (S13). What reaches the tick is values, one
//! slot at a time, over [`TickCommand`]. Speed masters are playback rate rather
//! than value, so they belong to step 2 and not to `crate::master`.

use prism_domain::{Fixture, FixtureType, Group, PlaybackId, ProgrammerState, Sequence};

use crate::command::TickCommand;
use crate::cue::{CueError, SequencePlan};
use crate::encode::{ChannelPlan, PatchError};
use crate::frame::{DmxFrame, FrameLayout};
use crate::master::MasterLayer;
use crate::plan::MergePlan;
use crate::playback::{MergeScratch, PlaybackLayer, PlaybackSource};
use crate::player::CueLayer;
use crate::programmer::ProgrammerLayer;
use crate::readback::{PlaybackReport, PlaybackState};
use crate::sync::Arc;
use crate::tick::{TickBody, TickInfo};

/// The whole pipeline — playbacks, merge, programmer, masters and the DMX
/// encoding — wired into the tick.
#[derive(Debug, Clone)]
pub struct MergeBody {
    plan: MergePlan,
    /// Where the tick publishes what its playbacks are doing, if anybody asked
    /// for it. `None` costs one branch a tick and nothing else.
    report: Option<Arc<PlaybackReport>>,
    channels: ChannelPlan,
    layer: PlaybackLayer,
    cues: CueLayer,
    programmer: ProgrammerLayer,
    masters: MasterLayer,
    scratch: MergeScratch,
    values: Box<[u16]>,
}

/// A body with nothing to play, for the cases that are about the patch alone.
///
/// The type annotation an empty array needs, given once: since S40 the
/// constructors take `impl Into<PlaybackId>` and `[]` on its own no longer says
/// which type it is empty of.
pub const NO_PLAYBACKS: [PlaybackId; 0] = [];

impl MergeBody {
    /// Builds the merge and the encoder for a patch and a set of playbacks.
    ///
    /// Every buffer the tick will use is allocated here, once. The two plans
    /// must describe the same patch — build them with [`Self::for_patch`] unless
    /// there is a reason not to.
    ///
    /// # Errors
    ///
    /// [`PatchError::PlanMismatch`] if the plans were built against different
    /// patches, or [`PatchError::Plan`] carrying
    /// [`crate::MergeError::TooManySources`] if there are more playbacks than
    /// [`crate::MAX_SOURCES`].
    pub fn new(
        plan: MergePlan,
        channels: ChannelPlan,
        playbacks: impl IntoIterator<Item: Into<PlaybackId>>,
    ) -> Result<Self, PatchError> {
        if channels.slot_count() != plan.slot_count() {
            return Err(PatchError::PlanMismatch {
                plan: plan.slot_count(),
                channels: channels.slot_count(),
            });
        }
        let layer = PlaybackLayer::new(&plan, playbacks)?;
        let cues = CueLayer::for_layer(&layer);
        let programmer = ProgrammerLayer::new(&plan);
        let masters = MasterLayer::new(&plan);
        let scratch = MergeScratch::new(&plan);
        let values = vec![0; plan.slot_count()].into_boxed_slice();
        let mut body = Self {
            plan,
            report: None,
            channels,
            layer,
            cues,
            programmer,
            masters,
            scratch,
            values,
        };
        // Start at the home layer rather than at zero, so a body that is read
        // before its first tick describes a rig at home rather than a blackout.
        body.resolve();
        Ok(body)
    }

    /// Builds both plans from one patch: `Patch → Merge → Frame` in a call.
    ///
    /// Each entry is a patched fixture and the type it instantiates. This is the
    /// constructor a daemon wants — the plans cannot disagree because they come
    /// from the same list.
    ///
    /// # Errors
    ///
    /// [`PatchError`] if the patch cannot be merged (a fixture patched twice, a
    /// type naming an attribute twice) or cannot be encoded (an address that
    /// does not fit, a universe the layout does not carry).
    pub fn for_patch<'a, I>(
        layout: &FrameLayout,
        fixtures: I,
        playbacks: impl IntoIterator<Item: Into<PlaybackId>>,
    ) -> Result<Self, PatchError>
    where
        I: IntoIterator<Item = (&'a Fixture, &'a FixtureType)>,
    {
        let fixtures: Vec<(&Fixture, &FixtureType)> = fixtures.into_iter().collect();
        let (plan, channels) = plans(layout, &fixtures)?;
        Self::new(plan, channels, playbacks)
    }

    /// The patch this body merges over.
    #[must_use]
    pub const fn plan(&self) -> &MergePlan {
        &self.plan
    }

    /// The channels this body writes, and where.
    #[must_use]
    pub const fn channels(&self) -> &ChannelPlan {
        &self.channels
    }

    /// The playback sources.
    #[must_use]
    pub const fn layer(&self) -> &PlaybackLayer {
        &self.layer
    }

    /// The playback sources, mutably. An executor with a sequence loaded is
    /// driven by its player and writing into its source by hand will not last
    /// past the next tick.
    pub const fn layer_mut(&mut self) -> &mut PlaybackLayer {
        &mut self.layer
    }

    /// The cue players, one per executor.
    #[must_use]
    pub const fn cues(&self) -> &CueLayer {
        &self.cues
    }

    /// The cue players, mutably.
    pub const fn cues_mut(&mut self) -> &mut CueLayer {
        &mut self.cues
    }

    /// The programmer: the operator's absolute override, above every playback.
    #[must_use]
    pub const fn programmer(&self) -> &ProgrammerLayer {
        &self.programmer
    }

    /// The programmer, mutably. On the tick it is driven by
    /// [`TickCommand::SetProgrammerValue`] and its two companions; this is how a
    /// host sets one up beforehand.
    pub const fn programmer_mut(&mut self) -> &mut ProgrammerLayer {
        &mut self.programmer
    }

    /// The grand master, the blackout and the group masters.
    #[must_use]
    pub const fn masters(&self) -> &MasterLayer {
        &self.masters
    }

    /// The masters, mutably.
    pub const fn masters_mut(&mut self) -> &mut MasterLayer {
        &mut self.masters
    }

    /// Publishes what the playbacks are doing into `report`, every tick.
    ///
    /// The channel back out of the tick (S34). Set it up before the body is
    /// installed: sharing the handle is what the `Arc` is for, and nothing about
    /// it allocates once it is in place. Passing a second report replaces the
    /// first, which is what a rebuilt body inherits.
    ///
    /// See [`PlaybackReport`] for what a reader may conclude from a sample —
    /// the short version is *one entry is consistent, two entries are not
    /// necessarily from the same tick*, which is the price of never making the
    /// tick wait.
    pub fn report_into(&mut self, report: Arc<PlaybackReport>) {
        self.report = Some(report);
        self.publish_playbacks();
    }

    /// The report this body publishes into, if it has one.
    #[must_use]
    pub fn report(&self) -> Option<&Arc<PlaybackReport>> {
        self.report.as_ref()
    }

    /// What one playback is doing, as the report describes it.
    ///
    /// The same answer the tick publishes, computed the same way in one place so
    /// a reader and the readback cannot disagree.
    #[must_use]
    pub fn playback_state(&self, playback: impl Into<PlaybackId>) -> Option<PlaybackState> {
        let player = self.cues.player(playback.into())?;
        Some(state_of(player, &self.layer))
    }

    /// Writes every playback into the report. Called at the end of every tick,
    /// and allocation-free: the table was sized when the report was built.
    ///
    /// **The length moves on whichever side of the entries keeps the window
    /// narrow.** A body rebuilt with *fewer* executors would otherwise leave a
    /// reader one poll in which the old length still covers an entry this tick
    /// no longer wrote; a body with *more* would leave one in which the new
    /// length covers an entry this tick has not written yet. Neither would be a
    /// fault — a sample may be one tick out of date — but both would be a
    /// reading of a playback that is not there, and this costs one comparison.
    fn publish_playbacks(&self) {
        let Some(report) = self.report.as_ref() else {
            return;
        };
        let published = self.cues.player_count();
        if published < report.len() {
            report.publish_len(published);
        }
        for (index, player) in self.cues.players().iter().enumerate() {
            report.publish(index, state_of(player, &self.layer));
        }
        report.publish_len(published);
    }

    /// Puts the show's groups onto the group masters.
    ///
    /// Allocates, so this is set-up work like [`Self::load_sequence`], not
    /// something to do while the tick runs. Every group arrives at full: a
    /// reload must not black the stage out.
    pub fn load_groups(&mut self, groups: &[Group]) {
        self.masters.set_groups(&self.plan, groups);
    }

    /// Resolves an operator-facing programmer state against this body's own
    /// patch and installs it, returning how many of its entries named something
    /// this patch does not have.
    ///
    /// The same reasoning as [`Self::load_sequence`] compiling its own sequence:
    /// a state resolved against a different rig would put the operator's values
    /// on the wrong lights, and every one of them would still look plausible.
    ///
    /// Set-up work. `prism_domain::ProgrammerState` owns maps and vectors, so it
    /// cannot cross into the tick at all — the tick receives single values over
    /// the command queue.
    pub fn load_programmer(&mut self, state: &ProgrammerState) -> usize {
        self.programmer.load(&self.plan, state)
    }

    /// Compiles a cue list against this body's patch and puts it on a playback.
    ///
    /// The body compiles the sequence itself rather than taking a compiled one,
    /// for the same reason [`Self::for_patch`] builds both plans: a sequence
    /// compiled against a different rig would resolve its parts to the wrong
    /// slots, and every value would still look plausible.
    ///
    /// Allocates, so this is a set-up operation — not something to do while the
    /// tick is running.
    ///
    /// # Errors
    ///
    /// [`CueError::UnknownExecutor`] if this body has no such playback, or
    /// [`CueError::TooManyCues`] / [`CueError::TooManyParts`] if the sequence is
    /// implausibly large.
    pub fn load_sequence(
        &mut self,
        playback: impl Into<PlaybackId>,
        sequence: &Sequence,
    ) -> Result<(), CueError> {
        let playback = playback.into();
        if self.cues.player(playback).is_none() {
            return Err(CueError::UnknownPlayback(playback));
        }
        let compiled = SequencePlan::build(&self.plan, sequence)?;
        if let Some(player) = self.cues.player_mut(playback) {
            player.load(compiled);
        }
        Ok(())
    }

    /// The fully resolved attribute values, one per slot of [`Self::plan`].
    ///
    /// The output of the whole stack — playbacks, programmer and masters — not
    /// of the playback merge alone. `0..=65535` regardless of the resolution the
    /// attribute is patched at: working in 16 bits until the final write is what
    /// keeps a fade over an 8-bit channel smooth (`docs/DMX_MERGE.md` §5).
    /// [`Self::channels`] turns these into bytes.
    #[must_use]
    pub const fn values(&self) -> &[u16] {
        &self.values
    }

    /// Runs the stack: merge the playbacks, override with the programmer, scale
    /// by the masters.
    ///
    /// `ARCHITECTURE_SPEC.md` §5 steps 4 to 6. Allocation-free — this is the
    /// tick's work, and the reason `render` is little more than a call to it.
    pub fn resolve(&mut self) {
        let Self {
            plan,
            layer,
            programmer,
            masters,
            scratch,
            values,
            ..
        } = self;
        layer.resolve(plan, scratch, values);
        programmer.apply(values);
        masters.apply(values);
    }
}

/// Both plans for one patch, validated against each other by construction.
///
/// Not generic, and called from a generic wrapper, so building a patch is one
/// copy of this code rather than one per caller's iterator type.
fn plans(
    layout: &FrameLayout,
    fixtures: &[(&Fixture, &FixtureType)],
) -> Result<(MergePlan, ChannelPlan), PatchError> {
    let plan = MergePlan::build(
        fixtures
            .iter()
            .map(|(fixture, fixture_type)| (fixture.id, *fixture_type)),
    )?;
    let channels = ChannelPlan::build(&plan, layout, fixtures.iter().copied())?;
    Ok((plan, channels))
}

/// What one player is doing, in the form the readback publishes.
///
/// **`is_active` is the playback rather than the merge.** A player that is
/// releasing has no cue in force and is still contributing light for the length
/// of its fade-out; the desk's *running* lamp is about the cue list, so a
/// release reads as stopped and the light goes on fading. An executor with no
/// cue list has no cue to be on and answers with whether the merge holds it,
/// which is the only sense *active* has for one.
fn state_of(player: &crate::player::CuePlayer, layer: &PlaybackLayer) -> PlaybackState {
    let cue_index = player
        .current_cue()
        .and_then(|index| u32::try_from(index).ok());
    PlaybackState {
        playback: player.playback(),
        is_active: if player.is_loaded() {
            cue_index.is_some()
        } else {
            layer
                .source(player.playback())
                .is_some_and(PlaybackSource::is_active)
        },
        cue_index,
    }
}

impl TickBody for MergeBody {
    fn apply(&mut self, command: TickCommand) {
        match command {
            TickCommand::SetExecutorLevel { executor, level } => {
                self.layer.set_master(executor, level);
            }
            TickCommand::SetExecutorActive { executor, on } => {
                // On an executor with a cue list, "active" means the sequence is
                // playing: `ExecutorButtonFunction::On` and `Off`. On one
                // without, it is the raw activation S3 defined, which is how a
                // host drives an executor it is holding values in by hand.
                match self.cues.player_mut(executor) {
                    Some(player) if player.is_loaded() => {
                        if on {
                            player.on();
                        } else {
                            player.off();
                        }
                    }
                    _ => {
                        if on {
                            self.layer.activate(executor);
                        } else {
                            self.layer.deactivate(executor);
                        }
                    }
                }
            }
            // A flash is a *layer* over the master and a temporary start, never
            // a `SetExecutorLevel`: `docs/DMX_MERGE.md` §2.1 applies the master
            // before the maximum, and this replaces the master that is applied
            // while leaving the stored one where it was. Releasing therefore
            // restores it byte for byte, including one that arrived while the
            // flash was held.
            TickCommand::SetExecutorFlash { executor, on } => {
                self.layer
                    .set_flash(executor, on.then_some(crate::merge::FULL));
                match self.cues.player_mut(executor) {
                    Some(player) if player.is_loaded() => {
                        if on {
                            player.flash_on();
                        } else {
                            player.flash_off();
                        }
                    }
                    // An executor with no cue list is one a host is holding
                    // values in by hand, so a flash puts it into the merge for
                    // as long as it is held — and takes back out only what it
                    // put in.
                    Some(player) => {
                        if on {
                            if self.layer.activate(executor) {
                                player.mark_flashed(true);
                            }
                        } else if player.is_flashed() {
                            player.mark_flashed(false);
                            self.layer.deactivate(executor);
                        }
                    }
                    None => {}
                }
            }
            TickCommand::SetExecutorSpeed { executor, speed } => {
                if let Some(player) = self.cues.player_mut(executor) {
                    player.set_speed(speed);
                }
            }
            TickCommand::TapExecutorSpeed { executor } => {
                if let Some(player) = self.cues.player_mut(executor) {
                    player.tap();
                }
            }
            TickCommand::SetExecutorXFade { executor, position } => {
                if let Some(player) = self.cues.player_mut(executor) {
                    player.set_crossfade(position);
                }
            }
            TickCommand::Go {
                executor,
                direction,
            } => {
                if let Some(player) = self.cues.player_mut(executor) {
                    player.go(direction);
                }
            }
            // S40's `Goto`. The index was resolved from the cue number on the
            // core thread; a playback with fewer cues than that answers `false`
            // and nothing moves, which is the tick's answer to an impossible
            // instruction everywhere else in this file too.
            TickCommand::GotoCue {
                executor,
                cue_index,
            } => {
                if let Some(player) = self.cues.player_mut(executor) {
                    player.goto(cue_index as usize);
                }
            }
            // `docs/DMX_MERGE.md` §4: the masters are applied after the merge,
            // so a command only moves a fader here and the arithmetic happens in
            // `resolve`.
            TickCommand::SetGrandMaster(level) => self.masters.set_grand(level),
            TickCommand::SetBlackout(on) => self.masters.set_blackout(on),
            TickCommand::SetGroupMaster { group, level } => {
                self.masters.set_group_level(group, level);
            }
            // The programmer is addressed by merge-plan slot: the core thread
            // resolved the fixture and attribute before pushing this.
            TickCommand::SetProgrammerValue { slot, value } => {
                self.programmer.set(slot as usize, value);
            }
            TickCommand::ClearProgrammerValue { slot } => {
                self.programmer.clear(slot as usize);
            }
            TickCommand::ClearProgrammer => self.programmer.clear_all(),
        }
    }

    fn render(&mut self, tick: &TickInfo, frame: &mut DmxFrame) {
        // `ARCHITECTURE_SPEC.md` §5 in order: advance the fades and evaluate the
        // executors, merge, override with the programmer, scale by the masters,
        // encode.
        self.cues.advance(tick.index, &mut self.layer);
        self.resolve();
        self.channels.encode(&self.values, frame);
        // And say what the playbacks did — after the frame, because the frame is
        // what has a deadline and this is feedback for a screen.
        self.publish_playbacks();
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::body::MergeBody;
    use crate::cue::CueError;
    use crate::encode::{ChannelPlan, PatchError, coarse_byte, fine_byte};
    use crate::merge::{FULL, apply_master, merge_programmer};
    use crate::plan::{MergeError, MergePlan};
    use crate::testkit::{cue, cue_part, fixture, moving_head, moving_head_16, sequence};
    use crate::{
        Clock, DmxFrame, Engine, FrameLayout, ManualClock, TickBody, TickCommand, TickInfo,
        command_queue,
    };
    use prism_domain::{
        AttributeType, ExecutorId, Fixture, FixtureId, GoDirection, Group, GroupId,
        ProgrammerState, ProgrammerValue, ProgrammerValueSource, UniverseId,
    };
    use proptest::prelude::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn layout() -> FrameLayout {
        FrameLayout::new([UniverseId::MIN]).unwrap()
    }

    /// `fixtures` moving heads, patched back to back from address 1.
    fn patch(fixtures: u32) -> Vec<Fixture> {
        (1..=fixtures)
            .map(|id| fixture(id, "test.movinghead", 1, (id as u16 - 1) * 2 + 1))
            .collect()
    }

    fn body(fixtures: u32, executors: u32) -> MergeBody {
        let head = moving_head();
        let patched = patch(fixtures);
        MergeBody::for_patch(
            &layout(),
            patched.iter().map(|fixture| (fixture, &head)),
            (1..=executors).map(ExecutorId::new),
        )
        .unwrap()
    }

    fn slot(body: &MergeBody, fixture: u32, attribute: AttributeType) -> usize {
        body.plan()
            .index_of(FixtureId::new(fixture), attribute)
            .unwrap()
    }

    fn tick(index: u64) -> TickInfo {
        TickInfo {
            index,
            deadline: crate::TICK_PERIOD,
            started: crate::TICK_PERIOD,
            missed: 0,
        }
    }

    #[test]
    fn a_fresh_body_holds_the_home_layer() {
        // Before its first tick, not merely after it: a host that reads the
        // body between building it and starting the engine must see a rig at
        // home rather than a blackout.
        let body = body(2, 4);
        assert_eq!(body.values(), [0, 32_768, 0, 32_768]);
        assert_eq!(body.plan().slot_count(), 4);
        assert_eq!(body.layer().source_count(), 4);
        assert!(body.layer().sources().iter().all(|s| !s.is_active()));
    }

    #[test]
    fn an_executor_level_command_moves_that_executors_master() {
        let mut body = body(1, 2);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, FULL);
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: true,
        });
        body.apply(TickCommand::SetExecutorLevel {
            executor: ExecutorId::new(1).into(),
            level: 32_767,
        });
        body.resolve();
        assert_eq!(body.values().first().copied(), Some(32_767));
    }

    #[test]
    fn activation_commands_drive_the_ltp_order() {
        let mut body = body(1, 2);
        let pan = slot(&body, 1, AttributeType::Pan);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(pan, 10_000);
        body.layer_mut()
            .source_mut(ExecutorId::new(2))
            .unwrap()
            .set(pan, 20_000);
        for executor in [1u32, 2] {
            body.apply(TickCommand::SetExecutorActive {
                executor: ExecutorId::new(executor).into(),
                on: true,
            });
        }
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(20_000));

        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(2).into(),
            on: false,
        });
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(10_000));
    }

    #[test]
    fn a_command_for_an_executor_this_body_does_not_have_is_ignored() {
        // A stale command from before a page change must not panic the tick.
        let mut body = body(1, 1);
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(77).into(),
            on: true,
        });
        body.apply(TickCommand::SetExecutorLevel {
            executor: ExecutorId::new(77).into(),
            level: 1,
        });
        body.resolve();
        assert_eq!(body.values(), [0, 32_768]);
    }

    /// One executor at full on fixture 1's dimmer, with its pan swung off home,
    /// switched on. The state most of the tests below start from.
    fn lit(fixtures: u32, executors: u32) -> MergeBody {
        let mut body = body(fixtures, executors);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        let pan = slot(&body, 1, AttributeType::Pan);
        {
            let source = body.layer_mut().source_mut(ExecutorId::new(1)).unwrap();
            source.set(dimmer, FULL);
            source.set(pan, 20_000);
        }
        body.layer_mut().activate(ExecutorId::new(1));
        body.resolve();
        body
    }

    fn programmer_value(value: u16) -> ProgrammerValue {
        ProgrammerValue {
            value,
            source: ProgrammerValueSource::Manual,
            preset_ref: None,
        }
    }

    #[test]
    fn the_grand_master_and_the_blackout_now_reach_the_merge() {
        // Until S6 these two were ignored and a test said so outright, because a
        // body that silently swallowed a blackout would pass every other test in
        // the crate. This is that test, turned round: they arrive, they scale
        // intensity, and they leave the position alone.
        let mut body = lit(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        let pan = slot(&body, 1, AttributeType::Pan);
        assert_eq!(body.values().get(dimmer).copied(), Some(FULL));

        body.apply(TickCommand::SetGrandMaster(32_767));
        body.resolve();
        assert_eq!(
            body.values().get(dimmer).copied(),
            Some(apply_master(FULL, 32_767))
        );
        assert_eq!(body.values().get(pan).copied(), Some(20_000));

        body.apply(TickCommand::SetBlackout(true));
        body.resolve();
        assert_eq!(body.values().get(dimmer).copied(), Some(0));
        assert_eq!(
            body.values().get(pan).copied(),
            Some(20_000),
            "blackout moved a position"
        );

        // And releasing blackout gives the fader back where it was, rather than
        // at full or at zero.
        body.apply(TickCommand::SetBlackout(false));
        body.resolve();
        assert_eq!(
            body.values().get(dimmer).copied(),
            Some(apply_master(FULL, 32_767))
        );
    }

    #[test]
    fn the_programmer_overrides_a_running_playback_and_clearing_hands_it_back() {
        // docs/DMX_MERGE.md 3: what the operator grabs is what the rig does,
        // whatever any playback is holding - and letting go gives the playback
        // its attribute back rather than leaving the stage where the programmer
        // left it.
        let mut body = lit(1, 1);
        let pan = slot(&body, 1, AttributeType::Pan);
        let slot_index = pan as u32;

        body.apply(TickCommand::SetProgrammerValue {
            slot: slot_index,
            value: 50_000,
        });
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(50_000));
        assert_eq!(body.programmer().len(), 1);

        body.apply(TickCommand::ClearProgrammerValue { slot: slot_index });
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(20_000));

        // And the whole-programmer clear, which is the first stage of the
        // operator's Clear button.
        body.apply(TickCommand::SetProgrammerValue {
            slot: slot_index,
            value: 50_000,
        });
        body.apply(TickCommand::ClearProgrammer);
        body.resolve();
        assert!(body.programmer().is_empty());
        assert_eq!(body.values().get(pan).copied(), Some(20_000));
    }

    #[test]
    fn a_programmer_value_of_zero_darkens_a_fixture_a_cue_is_holding_at_full() {
        // The sparse layer's whole point, at body level: zero is a value, not an
        // absence, so grabbing a dimmer and pulling it down beats an executor
        // holding it up. A layer that treated zero as "untouched" would leave
        // the light on and the operator baffled.
        let mut body = lit(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.apply(TickCommand::SetProgrammerValue {
            slot: dimmer as u32,
            value: 0,
        });
        body.resolve();
        assert_eq!(body.values().get(dimmer).copied(), Some(0));
    }

    #[test]
    fn the_masters_scale_the_programmer_as_well_as_the_playbacks() {
        // ARCHITECTURE_SPEC.md 5: the programmer is step 5 and the masters are
        // step 6. The other way round, a blackout would be a suggestion the
        // operator could lose to their own programmer.
        let mut body = lit(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        let pan = slot(&body, 1, AttributeType::Pan);
        body.programmer_mut().set(dimmer, FULL);
        body.programmer_mut().set(pan, 50_000);
        body.apply(TickCommand::SetGrandMaster(32_767));
        body.resolve();
        assert_eq!(
            body.values().get(dimmer).copied(),
            Some(apply_master(FULL, 32_767))
        );
        assert_eq!(body.values().get(pan).copied(), Some(50_000));

        body.apply(TickCommand::SetBlackout(true));
        body.resolve();
        assert_eq!(
            body.values().get(dimmer).copied(),
            Some(0),
            "the programmer survived a blackout"
        );
        assert_eq!(body.values().get(pan).copied(), Some(50_000));
    }

    #[test]
    fn a_group_master_reaches_the_merge_and_scales_only_its_own_fixtures() {
        let mut body = body(2, 1);
        let first = slot(&body, 1, AttributeType::Dimmer);
        let second = slot(&body, 2, AttributeType::Dimmer);
        {
            let source = body.layer_mut().source_mut(ExecutorId::new(1)).unwrap();
            source.set(first, FULL);
            source.set(second, FULL);
        }
        body.layer_mut().activate(ExecutorId::new(1));
        body.load_groups(&[Group {
            id: GroupId::new(3),
            name: "Left".to_owned(),
            fixtures: vec![FixtureId::new(1)],
        }]);

        body.apply(TickCommand::SetGroupMaster {
            group: GroupId::new(3),
            level: 32_767,
        });
        body.resolve();
        assert_eq!(
            body.values().get(first).copied(),
            Some(apply_master(FULL, 32_767))
        );
        assert_eq!(body.values().get(second).copied(), Some(FULL));

        // A command for a group this body does not have is ignored, like a
        // command for an executor it does not have.
        body.apply(TickCommand::SetGroupMaster {
            group: GroupId::new(99),
            level: 0,
        });
        body.resolve();
        assert_eq!(body.values().get(second).copied(), Some(FULL));
    }

    #[test]
    fn a_programmer_command_for_a_slot_this_patch_does_not_have_is_ignored() {
        // A stale command from before a repatch. The tick's answer is to drop
        // it, never to panic: a panic costs a frame.
        let mut body = lit(1, 1);
        for command in [
            TickCommand::SetProgrammerValue {
                slot: 99,
                value: 500,
            },
            TickCommand::ClearProgrammerValue { slot: 99 },
            TickCommand::SetProgrammerValue {
                slot: u32::MAX,
                value: 500,
            },
        ] {
            body.apply(command);
        }
        body.resolve();
        assert!(body.programmer().is_empty());
        assert_eq!(
            body.values()
                .get(slot(&body, 1, AttributeType::Dimmer))
                .copied(),
            Some(FULL)
        );
    }

    #[test]
    fn an_operator_facing_programmer_state_can_be_installed_against_this_bodys_patch() {
        // `prism_domain::ProgrammerState` owns maps and vectors, so it cannot
        // cross into the tick; a host resolves it against the body's own plan
        // beforehand, and an entry naming a light that is no longer patched is
        // counted rather than refused.
        let mut body = lit(1, 2);
        let pan = slot(&body, 1, AttributeType::Pan);
        let mut state = ProgrammerState::default();
        state.set_value(
            FixtureId::new(1),
            AttributeType::Pan,
            programmer_value(50_000),
        );
        state.set_value(FixtureId::new(9), AttributeType::Pan, programmer_value(1));

        assert_eq!(body.load_programmer(&state), 1);
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(50_000));

        assert_eq!(body.load_programmer(&ProgrammerState::default()), 0);
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(20_000));
    }

    #[test]
    fn the_whole_stack_runs_on_the_tick_through_the_command_queue() {
        // Every layer S6 added, driven the way the daemon will drive it: over
        // the queue, on the engine's own tick, and out onto the wire.
        let head = moving_head_16();
        let patched = fixture(1, "test.movinghead16", 1, 1);
        let mut body =
            MergeBody::for_patch(&layout(), [(&patched, &head)], [ExecutorId::new(1)]).unwrap();
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, FULL);
        body.load_groups(&[Group {
            id: GroupId::new(1),
            name: "All".to_owned(),
            fixtures: vec![FixtureId::new(1)],
        }]);

        let layout = Arc::new(FrameLayout::new([UniverseId::MIN]).unwrap());
        let mut publisher = crate::FramePublisher::new(layout);
        let mut subscriber = publisher.subscribe();
        let (mut producer, consumer) = command_queue(16);
        let mut engine = Engine::new(body, consumer, publisher);

        for command in [
            TickCommand::SetExecutorActive {
                executor: ExecutorId::new(1).into(),
                on: true,
            },
            TickCommand::SetProgrammerValue {
                slot: dimmer as u32,
                value: 40_000,
            },
            TickCommand::SetGroupMaster {
                group: GroupId::new(1),
                level: 32_767,
            },
            TickCommand::SetGrandMaster(32_767),
        ] {
            producer.push(command).unwrap();
        }
        engine.run_ticks(&ManualClock::new(), 1);
        assert_eq!(engine.stats().commands, 4);
        assert_eq!(engine.stats().panics, 0);

        // The programmer wins the merge at 40000, the group master halves it,
        // the grand master halves what is left, and the encoder splits it.
        let expected = apply_master(apply_master(40_000, 32_767), 32_767);
        assert_eq!(engine.body().values().get(dimmer).copied(), Some(expected));
        assert!(subscriber.refresh());
        assert_eq!(
            subscriber.frame().channel(0, 1),
            Some(coarse_byte(expected))
        );
        assert_eq!(subscriber.frame().channel(0, 2), Some(fine_byte(expected)));
    }

    #[test]
    fn a_go_for_an_executor_with_no_sequence_does_nothing_at_all() {
        // An executor with nothing on it is the ordinary state of most of a
        // page. A Go on one must not disturb what a host has written into it.
        let mut body = body(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, FULL);
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: true,
        });
        body.apply(TickCommand::Go {
            executor: ExecutorId::new(1).into(),
            direction: GoDirection::Next,
        });
        body.render(&tick(0), &mut DmxFrame::new(&layout()));
        assert_eq!(body.values().get(dimmer).copied(), Some(FULL));
    }

    #[test]
    fn a_go_command_walks_the_executor_through_its_cue_list() {
        let mut body = body(1, 2);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.load_sequence(
            ExecutorId::new(1),
            &sequence(
                vec![
                    cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 10_000)]),
                    cue("2", 0.0, vec![cue_part(1, AttributeType::Dimmer, 20_000)]),
                ],
                false,
            ),
        )
        .unwrap();

        let mut frame = DmxFrame::new(&layout());
        for (index, expected) in [(0u64, 10_000u16), (1, 20_000)] {
            body.apply(TickCommand::Go {
                executor: ExecutorId::new(1).into(),
                direction: GoDirection::Next,
            });
            body.render(&tick(index), &mut frame);
            assert_eq!(body.values().get(dimmer).copied(), Some(expected));
        }
        assert_eq!(
            body.cues()
                .player(ExecutorId::new(1))
                .unwrap()
                .current_cue(),
            Some(1)
        );
    }

    #[test]
    fn switching_a_loaded_executor_on_and_off_runs_and_releases_its_sequence() {
        // On a loaded executor, `SetExecutorActive` is "start the sequence" and
        // "stop it", not a raw activation: an executor with a cue list on it is
        // played, not poked.
        let mut body = body(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.load_sequence(
            ExecutorId::new(1),
            &sequence(
                vec![cue(
                    "1",
                    0.0,
                    vec![cue_part(1, AttributeType::Dimmer, 40_000)],
                )],
                false,
            ),
        )
        .unwrap();

        let mut frame = DmxFrame::new(&layout());
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: true,
        });
        body.render(&tick(0), &mut frame);
        assert_eq!(body.values().get(dimmer).copied(), Some(40_000));

        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: false,
        });
        body.render(&tick(1), &mut frame);
        assert_eq!(body.values().get(dimmer).copied(), Some(0));
        assert!(!body.layer().source(ExecutorId::new(1)).unwrap().is_active());

        // And a host can reach the same playback directly, which is how a
        // daemon takes a cue list back off an executor.
        body.cues_mut()
            .player_mut(ExecutorId::new(1))
            .unwrap()
            .unload();
        body.render(&tick(2), &mut frame);
        assert!(!body.cues().player(ExecutorId::new(1)).unwrap().is_loaded());
    }

    #[test]
    fn a_sequence_can_only_be_loaded_onto_a_playback_this_body_has() {
        let mut body = body(1, 2);
        assert_eq!(
            body.load_sequence(ExecutorId::new(77), &sequence(Vec::new(), false))
                .unwrap_err(),
            CueError::UnknownPlayback(ExecutorId::new(77).into())
        );
    }

    #[test]
    fn a_sequence_is_compiled_against_this_bodys_own_patch() {
        // The lesson of `PlanMismatch`, applied once more: a caller cannot hand
        // in a sequence compiled against a different rig, because it hands in
        // the sequence and the body compiles it.
        let mut body = body(1, 1);
        body.load_sequence(
            ExecutorId::new(1),
            &sequence(
                vec![cue(
                    "1",
                    0.0,
                    vec![
                        cue_part(9, AttributeType::Dimmer, 100),
                        cue_part(1, AttributeType::Dimmer, 100),
                    ],
                )],
                false,
            ),
        )
        .unwrap();
        let player = body.cues().player(ExecutorId::new(1)).unwrap();
        let compiled = player.sequence().unwrap();
        assert_eq!(compiled.unresolved(), 1);
        assert_eq!(compiled.slot_count(), 1);
        assert_eq!(
            compiled.slot(0).unwrap().slot,
            slot(&body, 1, AttributeType::Dimmer)
        );
    }

    #[test]
    fn a_ten_second_fade_reaches_half_after_five_seconds_of_engine_ticks() {
        // IMPLEMENTATION_PLAN.md S5, on the engine's own clock rather than on a
        // tick index handed in by a test: 44 Hz, absolute deadlines, five
        // seconds of them.
        let mut body = body(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.load_sequence(
            ExecutorId::new(1),
            &sequence(
                vec![cue(
                    "1",
                    10.0,
                    vec![cue_part(1, AttributeType::Dimmer, 65_535)],
                )],
                false,
            ),
        )
        .unwrap();

        let layout = Arc::new(FrameLayout::new([UniverseId::MIN]).unwrap());
        let publisher = crate::FramePublisher::new(layout);
        let (mut producer, consumer) = command_queue(8);
        let mut engine = Engine::new(body, consumer, publisher);
        let clock = ManualClock::new();

        producer
            .push(TickCommand::Go {
                executor: ExecutorId::new(1).into(),
                direction: GoDirection::Next,
            })
            .unwrap();
        // Tick 0 starts the fade, and ticks 1..=220 carry it to five seconds.
        engine.run_ticks(&clock, 221);
        assert_eq!(engine.last_index(), 220);
        assert!(clock.now().abs_diff(Duration::from_secs(5)) < crate::TICK_PERIOD);
        assert_eq!(
            engine.body().values().get(dimmer).copied(),
            Some(32_767),
            "a ten-second fade was not at half after five seconds"
        );
    }

    #[test]
    fn rendering_resolves_the_merge_and_encodes_it_into_the_frame() {
        // The chain closes here: the merge produces 12345, the encoder splits it
        // over the two channels of the 16-bit dimmer this head is patched with.
        let head = moving_head_16();
        let patched = fixture(1, "test.movinghead16", 1, 1);
        let mut body =
            MergeBody::for_patch(&layout(), [(&patched, &head)], [ExecutorId::new(1)]).unwrap();
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, 12_345);
        body.layer_mut().activate(ExecutorId::new(1));

        let mut frame = DmxFrame::new(&layout());
        body.render(&tick(0), &mut frame);
        assert_eq!(body.values().get(dimmer).copied(), Some(12_345));
        assert_eq!(frame.channel(0, 1), Some(0x30));
        assert_eq!(frame.channel(0, 2), Some(0x39));
        // Pan is at home, centred, and written as well: every patched channel is
        // written every tick, whether a source touched it or not.
        assert_eq!(frame.channel(0, 3), Some(0x80));
        assert_eq!(frame.channel(0, 4), Some(0x00));
    }

    #[test]
    fn a_channel_plan_built_against_a_different_merge_plan_is_rejected() {
        // The two plans must describe one patch. Pairing them by hand is how a
        // host would get that wrong, so the constructor says no.
        let head = moving_head();
        let one = patch(1);
        let two = patch(2);
        let plan = MergePlan::build(two.iter().map(|fixture| (fixture.id, &head))).unwrap();
        let narrow = MergePlan::build(one.iter().map(|fixture| (fixture.id, &head))).unwrap();
        let channels = ChannelPlan::build(
            &narrow,
            &layout(),
            one.iter().map(|fixture| (fixture, &head)),
        )
        .unwrap();
        assert_eq!(
            MergeBody::new(plan, channels, [ExecutorId::new(1)]).unwrap_err(),
            PatchError::PlanMismatch {
                plan: 4,
                channels: 2,
            }
        );
    }

    #[test]
    fn a_patch_neither_plan_accepts_never_becomes_a_body() {
        // `for_patch` builds both plans, so it can fail either way round: the
        // merge rejects a fixture patched twice, the encoder rejects a universe
        // with no frame to write into.
        let head = moving_head();
        let patched = fixture(1, "test.movinghead", 9, 1);
        assert_eq!(
            MergeBody::for_patch(&layout(), [(&patched, &head)], crate::NO_PLAYBACKS).unwrap_err(),
            PatchError::UniverseNotPatched {
                fixture: FixtureId::new(1),
                universe: UniverseId::new(9),
            }
        );
        let twice = fixture(1, "test.movinghead", 1, 1);
        assert_eq!(
            MergeBody::for_patch(
                &layout(),
                [(&twice, &head), (&twice, &head)],
                crate::NO_PLAYBACKS
            )
            .unwrap_err(),
            PatchError::Plan(MergeError::DuplicateFixture(FixtureId::new(1)))
        );
    }

    #[test]
    fn the_merge_runs_on_the_tick_through_the_command_queue() {
        let mut body = body(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, FULL);

        let layout = Arc::new(FrameLayout::new([UniverseId::MIN]).unwrap());
        let publisher = crate::FramePublisher::new(layout);
        let (mut producer, consumer) = command_queue(8);
        let mut engine = Engine::new(body, consumer, publisher);

        producer
            .push(TickCommand::SetExecutorActive {
                executor: ExecutorId::new(1).into(),
                on: true,
            })
            .unwrap();
        producer
            .push(TickCommand::SetExecutorLevel {
                executor: ExecutorId::new(1).into(),
                level: 32_767,
            })
            .unwrap();
        engine.run_ticks(&ManualClock::new(), 1);

        assert_eq!(engine.stats().commands, 2);
        assert_eq!(engine.stats().panics, 0);
        assert_eq!(engine.body().values().first().copied(), Some(32_767));
    }

    #[test]
    fn the_worked_example_from_the_specification() {
        // docs/DMX_MERGE.md 7, number for number.
        //
        //   Source      | Activation | Dimmer | Pan   | Master
        //   Home        | -          | 0      | 32768 | -
        //   Executor 3  | counter 7  | 65535  | 20000 | 50 %
        //   Executor 5  | counter 9  | 30000  | 45000 | 100 %
        //   Programmer  | -          | -      | 50000 | -
        //
        // The stamps 7 and 9 are the specification's; what the merge uses is
        // their order, so executor 3 is switched on first here.
        // "a moving head with a 16-bit dimmer and 16-bit pan", patched at 1.
        let head = moving_head_16();
        let patched = fixture(1, "test.movinghead16", 1, 1);
        let mut body = MergeBody::for_patch(
            &layout(),
            [(&patched, &head)],
            [ExecutorId::new(3), ExecutorId::new(5)],
        )
        .unwrap();
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        let pan = slot(&body, 1, AttributeType::Pan);

        {
            let source = body.layer_mut().source_mut(ExecutorId::new(3)).unwrap();
            source.set(dimmer, 65_535);
            source.set(pan, 20_000);
        }
        {
            let source = body.layer_mut().source_mut(ExecutorId::new(5)).unwrap();
            source.set(dimmer, 30_000);
            source.set(pan, 45_000);
        }
        body.layer_mut().activate(ExecutorId::new(3));
        body.layer_mut().activate(ExecutorId::new(5));
        // 50 % of full scale. 65535 is odd, so half of it is 32767 - which is
        // exactly the number the specification's "65535 x 0.5 = 32767" gives.
        body.layer_mut().set_master(ExecutorId::new(3), 32_767);
        body.layer_mut().set_master(ExecutorId::new(5), FULL);
        body.resolve();

        // Dimmer is HTP: executor 3 contributes 65535 x 0.5 = 32767, executor 5
        // contributes 30000 x 1.0 = 30000. The maximum is 32767, there is no
        // programmer value for it, and a grand master at full is a no-op.
        let merged_dimmer = body.values().get(dimmer).copied().unwrap();
        assert_eq!(merged_dimmer, 32_767);
        assert_eq!(merge_programmer(merged_dimmer, None), 32_767);
        // "Written as coarse 0x7F, fine 0xFF" - now on the wire, not as
        // arithmetic beside it.
        let mut frame = DmxFrame::new(&layout());
        body.render(&tick(0), &mut frame);
        assert_eq!(frame.channel(0, 1), Some(0x7F));
        assert_eq!(frame.channel(0, 2), Some(0xFF));

        // Pan is LTP: executor 5 has the higher activation counter and would
        // win with 45000 - but the programmer holds 50000, which overrides
        // everything. Result 50000, coarse 0xC3, fine 0x50 - on the wire now,
        // where in S3 this was arithmetic beside the frame.
        assert_eq!(body.values().get(pan).copied(), Some(45_000));
        assert_eq!(merge_programmer(45_000, Some(50_000)), 50_000);
        body.programmer_mut().set(pan, 50_000);
        body.render(&tick(1), &mut frame);
        assert_eq!(body.values().get(pan).copied(), Some(50_000));
        assert_eq!(frame.channel(0, 3), Some(0xC3));
        assert_eq!(frame.channel(0, 4), Some(0x50));
        assert_eq!(coarse_byte(50_000), 0xC3);
        assert_eq!(fine_byte(50_000), 0x50);

        // "Turning executor 5 off changes nothing about pan while the
        // programmer holds it."
        body.layer_mut().deactivate(ExecutorId::new(5));
        body.render(&tick(2), &mut frame);
        assert_eq!(body.values().get(pan).copied(), Some(50_000));
        assert_eq!(frame.channel(0, 3), Some(0xC3));
        assert_eq!(frame.channel(0, 4), Some(0x50));

        // "Clearing the programmer drops pan to 45000 only if executor 5 is
        // still active; otherwise it falls back to executor 3's 20000 ..."
        body.programmer_mut().clear(pan);
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(20_000));
        body.layer_mut().activate(ExecutorId::new(5));
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(45_000));

        // "... and with both off, to home at 32768."
        body.layer_mut().deactivate(ExecutorId::new(3));
        body.layer_mut().deactivate(ExecutorId::new(5));
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(32_768));
        // And the dimmer falls back to its own home, which is dark.
        assert_eq!(body.values().get(dimmer).copied(), Some(0));
    }

    /// One thing an operator can do to a playback source.
    #[derive(Debug, Clone, Copy)]
    struct Action {
        executor: u32,
        slot: usize,
        value: u16,
        master: u16,
        active: bool,
    }

    /// Arbitrary playback state over the four slots of `body(2, 4)`: values from
    /// several executors, masters part way up, some of them switched on.
    fn actions() -> impl Strategy<Value = Vec<Action>> {
        proptest::collection::vec(
            (
                1u32..=4,
                0usize..4,
                any::<u16>(),
                any::<u16>(),
                any::<bool>(),
            ),
            0..12,
        )
        .prop_map(|raw| {
            raw.into_iter()
                .map(|(executor, slot, value, master, active)| Action {
                    executor,
                    slot,
                    value,
                    master,
                    active,
                })
                .collect()
        })
    }

    /// A body of two moving heads with that playback state applied.
    fn played(actions: &[Action]) -> MergeBody {
        let mut body = body(2, 4);
        for action in actions {
            let executor = ExecutorId::new(action.executor);
            if let Some(source) = body.layer_mut().source_mut(executor) {
                source.set(action.slot, action.value);
            }
            body.layer_mut().set_master(executor, action.master);
            if action.active {
                body.layer_mut().activate(executor);
            } else {
                body.layer_mut().deactivate(executor);
            }
        }
        body
    }

    proptest! {
        /// `docs/DMX_MERGE.md` §6.3 — a programmer value always appears in the
        /// output, whatever the playbacks are doing. This is the property the
        /// operator relies on without ever thinking about it: what you grab is
        /// what you see.
        #[test]
        fn a_programmer_value_always_reaches_the_output(
            actions in actions(),
            slot in 0usize..4,
            value in any::<u16>(),
        ) {
            let mut body = played(&actions);
            body.programmer_mut().set(slot, value);
            body.resolve();
            prop_assert_eq!(body.values().get(slot).copied(), Some(value));
        }

        /// §6.3 — with no active playback and an empty programmer, every
        /// patched attribute is at its home value. The bottom of the stack is
        /// never empty, so a rig with nothing running sits at home rather than
        /// at zero.
        #[test]
        fn a_desk_with_nothing_running_resolves_to_home(actions in actions()) {
            let mut body = played(&actions);
            for executor in 1..=4 {
                body.layer_mut().deactivate(ExecutorId::new(executor));
            }
            body.resolve();
            let homes: Vec<u16> = body.plan().slots().iter().map(|slot| slot.home).collect();
            prop_assert_eq!(body.values(), homes.as_slice());
        }

        /// §6.3 — the grand master at zero forces every intensity attribute to
        /// zero and leaves every other attribute untouched, whatever the
        /// playbacks and the programmer hold.
        #[test]
        fn a_grand_master_at_zero_zeroes_intensity_and_nothing_else(
            actions in actions(),
            programmer in proptest::collection::vec(proptest::option::of(any::<u16>()), 4),
        ) {
            let mut body = played(&actions);
            for (slot, value) in programmer.iter().enumerate() {
                if let Some(value) = value {
                    body.programmer_mut().set(slot, *value);
                }
            }
            body.resolve();
            let before = body.values().to_vec();

            body.apply(TickCommand::SetGrandMaster(0));
            body.resolve();
            for (index, was) in before.iter().enumerate() {
                let intensity = body.plan().slot(index).unwrap().is_intensity();
                let expected = if intensity { 0 } else { *was };
                prop_assert_eq!(body.values().get(index).copied(), Some(expected));
            }
        }

        /// §6.3 — the grand master at full is a no-op. A master scales, it never
        /// selects, so a desk with everything up produces exactly the merge.
        #[test]
        fn a_grand_master_at_full_changes_nothing(
            actions in actions(),
            programmer in proptest::collection::vec(proptest::option::of(any::<u16>()), 4),
        ) {
            let mut body = played(&actions);
            for (slot, value) in programmer.iter().enumerate() {
                if let Some(value) = value {
                    body.programmer_mut().set(slot, *value);
                }
            }
            body.resolve();
            let before = body.values().to_vec();
            body.apply(TickCommand::SetGrandMaster(FULL));
            body.resolve();
            prop_assert_eq!(body.values(), before.as_slice());
        }

        /// Resolving the same body twice gives the same answer twice: the stack
        /// is a function of its state, with nothing carried between ticks. This
        /// is determinism at the level of one body; the frame-level criterion is
        /// in `tests/pipeline.rs`.
        #[test]
        fn resolving_twice_gives_the_same_values(
            actions in actions(),
            grand in any::<u16>(),
            slot in 0usize..4,
            value in any::<u16>(),
        ) {
            let mut body = played(&actions);
            body.programmer_mut().set(slot, value);
            body.masters_mut().set_grand(grand);
            body.resolve();
            let once = body.values().to_vec();
            body.resolve();
            prop_assert_eq!(body.values(), once.as_slice());
        }
    }

    // -- S34: the flash layer and the channel back out of the tick -----------

    /// **Exit criterion, on the wire.** A flash raises what reaches the fixture
    /// and leaves the stored master byte-identical; a `SetExecutorLevel` that
    /// arrives *while* the flash is held is the level that stands when it is
    /// released.
    ///
    /// The stored master is compared as the number the show holds and the light
    /// as the byte the fixture gets, because those are two different claims and
    /// a flash that wrote into the master would pass one of them.
    #[test]
    fn a_flash_raises_the_light_and_leaves_the_stored_master_untouched() {
        let head = moving_head_16();
        let patched = fixture(1, "test.movinghead16", 1, 1);
        let mut body =
            MergeBody::for_patch(&layout(), [(&patched, &head)], [ExecutorId::new(1)]).unwrap();
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, FULL);

        let layout = Arc::new(FrameLayout::new([UniverseId::MIN]).unwrap());
        let mut publisher = crate::FramePublisher::new(layout);
        let mut subscriber = publisher.subscribe();
        let (mut producer, consumer) = command_queue(16);
        let mut engine = Engine::new(body, consumer, publisher);
        let clock = ManualClock::new();

        // A quarter master on a running executor: a quarter of the light.
        for command in [
            TickCommand::SetExecutorActive {
                executor: ExecutorId::new(1).into(),
                on: true,
            },
            TickCommand::SetExecutorLevel {
                executor: ExecutorId::new(1).into(),
                level: 16_383,
            },
        ] {
            producer.push(command).unwrap();
        }
        engine.run_ticks(&clock, 1);
        subscriber.refresh();
        let quarter = subscriber.frame().channels()[0];
        assert_eq!(quarter, 63, "a quarter master is a quarter of the light");
        assert_eq!(
            engine
                .body()
                .layer()
                .source(ExecutorId::new(1))
                .unwrap()
                .master(),
            16_383
        );

        // Held: full light, and the stored master has not moved.
        producer
            .push(TickCommand::SetExecutorFlash {
                executor: ExecutorId::new(1).into(),
                on: true,
            })
            .unwrap();
        engine.run_ticks(&clock, 1);
        subscriber.refresh();
        assert_eq!(subscriber.frame().channels()[0], 255);
        assert_eq!(
            engine
                .body()
                .layer()
                .source(ExecutorId::new(1))
                .unwrap()
                .master(),
            16_383,
            "the flash wrote into the stored master"
        );

        // A fader move *during* the flash: the light does not change, because
        // the flash is on top — and the new level is what the release restores.
        producer
            .push(TickCommand::SetExecutorLevel {
                executor: ExecutorId::new(1).into(),
                level: 49_151,
            })
            .unwrap();
        engine.run_ticks(&clock, 1);
        subscriber.refresh();
        assert_eq!(subscriber.frame().channels()[0], 255);

        producer
            .push(TickCommand::SetExecutorFlash {
                executor: ExecutorId::new(1).into(),
                on: false,
            })
            .unwrap();
        engine.run_ticks(&clock, 1);
        subscriber.refresh();
        assert_eq!(
            subscriber.frame().channels()[0],
            191,
            "the release lost the level that arrived while it was held"
        );
        assert_eq!(
            engine
                .body()
                .layer()
                .source(ExecutorId::new(1))
                .unwrap()
                .master(),
            49_151
        );
        assert_eq!(engine.stats().panics, 0);
    }

    /// A flash on an executor with no cue list puts it into the merge for as
    /// long as it is held, and takes back out only what it put in.
    #[test]
    fn a_flash_activates_an_executor_that_has_no_cue_list() {
        let mut body = body(1, 2);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, FULL);
        assert!(!body.layer().source(ExecutorId::new(1)).unwrap().is_active());

        body.apply(TickCommand::SetExecutorFlash {
            executor: ExecutorId::new(1).into(),
            on: true,
        });
        body.resolve();
        assert!(body.layer().source(ExecutorId::new(1)).unwrap().is_active());
        assert_eq!(body.values()[dimmer], FULL);

        body.apply(TickCommand::SetExecutorFlash {
            executor: ExecutorId::new(1).into(),
            on: false,
        });
        body.resolve();
        assert!(!body.layer().source(ExecutorId::new(1)).unwrap().is_active());
        assert_eq!(body.values()[dimmer], 0, "home is dark for this rig");

        // And an executor that was already on stays on: the release stops what
        // the flash started and nothing else.
        body.layer_mut().activate(ExecutorId::new(2));
        body.apply(TickCommand::SetExecutorFlash {
            executor: ExecutorId::new(2).into(),
            on: true,
        });
        body.apply(TickCommand::SetExecutorFlash {
            executor: ExecutorId::new(2).into(),
            on: false,
        });
        assert!(body.layer().source(ExecutorId::new(2)).unwrap().is_active());
    }

    /// **The channel S26 recorded as missing.** The tick publishes which cue
    /// each playback is on, and a reader that never touches the engine can read
    /// it.
    #[test]
    fn the_tick_publishes_which_cue_each_playback_is_on() {
        let head = moving_head();
        let patched = patch(1);
        let mut body = MergeBody::for_patch(
            &layout(),
            patched.iter().map(|fixture| (fixture, &head)),
            [ExecutorId::new(1), ExecutorId::new(2)],
        )
        .unwrap();
        body.load_sequence(
            ExecutorId::new(1),
            &sequence(
                vec![
                    cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 40_000)]),
                    cue("2", 0.0, vec![cue_part(1, AttributeType::Dimmer, 20_000)]),
                ],
                false,
            ),
        )
        .unwrap();
        let report = Arc::new(crate::PlaybackReport::new(crate::MAX_SOURCES));
        body.report_into(Arc::clone(&report));

        // Before anything runs: two executors, both stopped, no cue on either.
        assert_eq!(report.len(), 2);
        let stopped: Vec<_> = report.states().collect();
        assert!(stopped.iter().all(|state| !state.is_active));
        assert!(stopped.iter().all(|state| state.cue_index.is_none()));

        let mut frame = DmxFrame::new(&layout());
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: true,
        });
        body.render(&tick(1), &mut frame);
        assert_eq!(report.get(0).unwrap().cue_index, Some(0));
        assert!(report.get(0).unwrap().is_active);
        assert_eq!(report.get(1).unwrap().cue_index, None);

        body.apply(TickCommand::Go {
            executor: ExecutorId::new(1).into(),
            direction: GoDirection::Next,
        });
        body.render(&tick(2), &mut frame);
        assert_eq!(report.get(0).unwrap().cue_index, Some(1));

        // Stopped: the cue index goes away with it, rather than being left
        // pointing at the cue that used to be running.
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: false,
        });
        body.render(&tick(3), &mut frame);
        assert_eq!(report.get(0).unwrap().cue_index, None);
        assert!(!report.get(0).unwrap().is_active);

        // The reader sees an executor number with each one, so a rebuilt body
        // with a different grid cannot be read as the old one.
        assert_eq!(
            report
                .states()
                .map(|state| state.playback)
                .collect::<Vec<_>>(),
            vec![
                prism_domain::PlaybackId::from(ExecutorId::new(1)),
                prism_domain::PlaybackId::from(ExecutorId::new(2))
            ]
        );
    }

    /// A body built without a report ticks exactly as it did, and one given a
    /// report answers the same thing its own reader does.
    #[test]
    fn the_report_and_the_bodys_own_reader_cannot_disagree() {
        let mut body = body(1, 2);
        assert!(body.report().is_none());
        assert_eq!(body.playback_state(ExecutorId::new(9)), None);

        let report = Arc::new(crate::PlaybackReport::new(4));
        body.report_into(Arc::clone(&report));
        assert!(body.report().is_some());
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(2).into(),
            on: true,
        });
        let mut frame = DmxFrame::new(&layout());
        body.render(&tick(1), &mut frame);
        for state in report.states() {
            assert_eq!(body.playback_state(state.playback), Some(state));
        }
        assert!(
            report
                .states()
                .any(|state| state.playback == ExecutorId::new(2).into() && state.is_active)
        );
    }

    /// A speed of zero freezes a playback and the frame stops moving with it.
    #[test]
    fn an_executor_speed_command_changes_the_rate_and_no_value() {
        let head = moving_head();
        let patched = patch(1);
        let mut body = MergeBody::for_patch(
            &layout(),
            patched.iter().map(|fixture| (fixture, &head)),
            [ExecutorId::new(1)],
        )
        .unwrap();
        body.load_sequence(
            ExecutorId::new(1),
            &sequence(
                vec![cue(
                    "1",
                    4.0,
                    vec![cue_part(1, AttributeType::Dimmer, 65_535)],
                )],
                false,
            ),
        )
        .unwrap();
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        let mut frame = DmxFrame::new(&layout());

        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: true,
        });
        for index in 1..=40 {
            body.render(&tick(index), &mut frame);
        }
        let moving = body.values()[dimmer];
        assert!(moving > 0 && moving < 65_535, "{moving}");

        body.apply(TickCommand::SetExecutorSpeed {
            executor: ExecutorId::new(1).into(),
            speed: 0,
        });
        for index in 41..=200 {
            body.render(&tick(index), &mut frame);
        }
        assert_eq!(body.values()[dimmer], moving, "a frozen fade moved");
        // And it is frozen rather than finished: the cue is still the one that
        // was running.
        assert_eq!(
            body.playback_state(ExecutorId::new(1)).unwrap().cue_index,
            Some(0)
        );

        // A tap and a crossfade on an executor that has neither loaded change
        // nothing and do not panic — the tick's answer to an impossible command
        // is to ignore it.
        body.apply(TickCommand::TapExecutorSpeed {
            executor: ExecutorId::new(99).into(),
        });
        body.apply(TickCommand::SetExecutorXFade {
            executor: ExecutorId::new(99).into(),
            position: 4,
        });
        body.apply(TickCommand::SetExecutorFlash {
            executor: ExecutorId::new(99).into(),
            on: true,
        });
        body.render(&tick(201), &mut frame);
        assert_eq!(body.values()[dimmer], moving);
    }

    /// The crossfade reaches the frame through the queue, which is the path the
    /// daemon uses.
    #[test]
    fn a_crossfade_command_drives_the_transition_from_the_fader() {
        let head = moving_head();
        let patched = patch(1);
        let mut body = MergeBody::for_patch(
            &layout(),
            patched.iter().map(|fixture| (fixture, &head)),
            [ExecutorId::new(1)],
        )
        .unwrap();
        body.load_sequence(
            ExecutorId::new(1),
            &sequence(
                vec![
                    cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 0)]),
                    cue("2", 600.0, vec![cue_part(1, AttributeType::Dimmer, 65_535)]),
                ],
                false,
            ),
        )
        .unwrap();
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        let mut frame = DmxFrame::new(&layout());

        body.apply(TickCommand::SetExecutorXFade {
            executor: ExecutorId::new(1).into(),
            position: 0,
        });
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: true,
        });
        body.render(&tick(1), &mut frame);
        body.apply(TickCommand::Go {
            executor: ExecutorId::new(1).into(),
            direction: GoDirection::Next,
        });
        body.render(&tick(2), &mut frame);
        assert_eq!(body.values()[dimmer], 0);

        body.apply(TickCommand::SetExecutorXFade {
            executor: ExecutorId::new(1).into(),
            position: 65_535,
        });
        body.render(&tick(3), &mut frame);
        // A ten-minute fade, arrived at in one tick, because the fader is the
        // clock.
        assert_eq!(body.values()[dimmer], 65_535);
    }
}
