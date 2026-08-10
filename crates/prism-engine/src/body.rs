//! The merge as a [`crate::TickBody`]: the seam S2 left open.
//!
//! [`MergeBody`] owns a [`crate::MergePlan`], the [`crate::PlaybackLayer`] over
//! it and the buffers the resolve needs, and runs the merge once per tick.
//! Everything it touches is sized when the body is built, so the tick itself
//! never reaches the allocator — `crates/prism-engine/tests/tick_allocations.rs`
//! counts that rather than asserting it.
//!
//! The tick runs `ARCHITECTURE_SPEC.md` §5 in order: the playbacks are advanced
//! and evaluated (steps 2 and 3, `crate::player`), the result is merged (step 4,
//! `crate::playback`) and encoded into the frame (step 7, `crate::encode`).
//!
//! # What it does not do yet
//!
//! The grand master, group masters and the programmer state machine are S6. The
//! commands for those arrive here already and are deliberately ignored, which
//! the tests state outright so that the gap is a recorded decision rather than a
//! surprise.

use prism_domain::{ExecutorId, Fixture, FixtureType, Sequence};

use crate::command::TickCommand;
use crate::cue::{CueError, SequencePlan};
use crate::encode::{ChannelPlan, PatchError};
use crate::frame::{DmxFrame, FrameLayout};
use crate::plan::MergePlan;
use crate::playback::{MergeScratch, PlaybackLayer};
use crate::player::CueLayer;
use crate::tick::{TickBody, TickInfo};

/// The playbacks, the HTP/LTP merge and the DMX encoding, wired into the tick.
#[derive(Debug, Clone)]
pub struct MergeBody {
    plan: MergePlan,
    channels: ChannelPlan,
    layer: PlaybackLayer,
    cues: CueLayer,
    scratch: MergeScratch,
    values: Box<[u16]>,
}

impl MergeBody {
    /// Builds the merge and the encoder for a patch and a set of executors.
    ///
    /// Every buffer the tick will use is allocated here, once. The two plans
    /// must describe the same patch — build them with [`Self::for_patch`] unless
    /// there is a reason not to.
    ///
    /// # Errors
    ///
    /// [`PatchError::PlanMismatch`] if the plans were built against different
    /// patches, or [`PatchError::Plan`] carrying
    /// [`crate::MergeError::TooManySources`] if there are more executors than
    /// [`crate::MAX_SOURCES`].
    pub fn new(
        plan: MergePlan,
        channels: ChannelPlan,
        executors: impl IntoIterator<Item = ExecutorId>,
    ) -> Result<Self, PatchError> {
        if channels.slot_count() != plan.slot_count() {
            return Err(PatchError::PlanMismatch {
                plan: plan.slot_count(),
                channels: channels.slot_count(),
            });
        }
        let layer = PlaybackLayer::new(&plan, executors)?;
        let cues = CueLayer::for_layer(&layer);
        let scratch = MergeScratch::new(&plan);
        let values = vec![0; plan.slot_count()].into_boxed_slice();
        let mut body = Self {
            plan,
            channels,
            layer,
            cues,
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
        executors: impl IntoIterator<Item = ExecutorId>,
    ) -> Result<Self, PatchError>
    where
        I: IntoIterator<Item = (&'a Fixture, &'a FixtureType)>,
    {
        let fixtures: Vec<(&Fixture, &FixtureType)> = fixtures.into_iter().collect();
        let (plan, channels) = plans(layout, &fixtures)?;
        Self::new(plan, channels, executors)
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

    /// Compiles a cue list against this body's patch and puts it on an executor.
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
    /// [`CueError::UnknownExecutor`] if this body has no such executor, or
    /// [`CueError::TooManyCues`] / [`CueError::TooManyParts`] if the sequence is
    /// implausibly large.
    pub fn load_sequence(
        &mut self,
        executor: ExecutorId,
        sequence: &Sequence,
    ) -> Result<(), CueError> {
        if self.cues.player(executor).is_none() {
            return Err(CueError::UnknownExecutor(executor));
        }
        let compiled = SequencePlan::build(&self.plan, sequence)?;
        if let Some(player) = self.cues.player_mut(executor) {
            player.load(compiled);
        }
        Ok(())
    }

    /// The merged attribute values, one per slot of [`Self::plan`].
    ///
    /// `0..=65535` regardless of the resolution the attribute is patched at:
    /// working in 16 bits until the final write is what keeps a fade over an
    /// 8-bit channel smooth (`docs/DMX_MERGE.md` §5). [`Self::channels`] turns
    /// these into bytes.
    #[must_use]
    pub const fn values(&self) -> &[u16] {
        &self.values
    }

    /// Runs the merge. Allocation-free — this is the tick's work.
    pub fn resolve(&mut self) {
        let Self {
            plan,
            layer,
            scratch,
            values,
            ..
        } = self;
        layer.resolve(plan, scratch, values);
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
            TickCommand::Go {
                executor,
                direction,
            } => {
                if let Some(player) = self.cues.player_mut(executor) {
                    player.go(direction);
                }
            }
            // The grand master and blackout are masters, which
            // `docs/DMX_MERGE.md` §4 applies after the merge — S6.
            TickCommand::SetGrandMaster(_) | TickCommand::SetBlackout(_) => {}
        }
    }

    fn render(&mut self, tick: &TickInfo, frame: &mut DmxFrame) {
        // `ARCHITECTURE_SPEC.md` §5 in order: advance the fades and evaluate the
        // executors, merge, encode.
        self.cues.advance(tick.index, &mut self.layer);
        self.resolve();
        self.channels.encode(&self.values, frame);
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::body::MergeBody;
    use crate::cue::CueError;
    use crate::encode::{ChannelPlan, PatchError, coarse_byte, fine_byte};
    use crate::merge::{FULL, merge_programmer};
    use crate::plan::{MergeError, MergePlan};
    use crate::testkit::{cue, cue_part, fixture, moving_head, moving_head_16, sequence};
    use crate::{
        Clock, DmxFrame, Engine, FrameLayout, ManualClock, TickBody, TickCommand, TickInfo,
        command_queue,
    };
    use prism_domain::{AttributeType, ExecutorId, Fixture, FixtureId, GoDirection, UniverseId};
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
            executor: ExecutorId::new(1),
            on: true,
        });
        body.apply(TickCommand::SetExecutorLevel {
            executor: ExecutorId::new(1),
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
                executor: ExecutorId::new(executor),
                on: true,
            });
        }
        body.resolve();
        assert_eq!(body.values().get(pan).copied(), Some(20_000));

        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(2),
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
            executor: ExecutorId::new(77),
            on: true,
        });
        body.apply(TickCommand::SetExecutorLevel {
            executor: ExecutorId::new(77),
            level: 1,
        });
        body.resolve();
        assert_eq!(body.values(), [0, 32_768]);
    }

    #[test]
    fn the_commands_this_session_does_not_own_are_ignored_rather_than_half_handled() {
        // The grand master and blackout are S6. Recorded as a test so the gap is
        // visible: a body that silently swallowed a blackout would look like it
        // worked.
        let mut body = body(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, FULL);
        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1),
            on: true,
        });
        for command in [
            TickCommand::SetBlackout(true),
            TickCommand::SetGrandMaster(0),
        ] {
            body.apply(command);
        }
        body.resolve();
        assert_eq!(body.values().get(dimmer).copied(), Some(FULL));
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
            executor: ExecutorId::new(1),
            on: true,
        });
        body.apply(TickCommand::Go {
            executor: ExecutorId::new(1),
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
                executor: ExecutorId::new(1),
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
            executor: ExecutorId::new(1),
            on: true,
        });
        body.render(&tick(0), &mut frame);
        assert_eq!(body.values().get(dimmer).copied(), Some(40_000));

        body.apply(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1),
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
    fn a_sequence_can_only_be_loaded_onto_an_executor_this_body_has() {
        let mut body = body(1, 2);
        assert_eq!(
            body.load_sequence(ExecutorId::new(77), &sequence(Vec::new(), false))
                .unwrap_err(),
            CueError::UnknownExecutor(ExecutorId::new(77))
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
                executor: ExecutorId::new(1),
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
            MergeBody::for_patch(&layout(), [(&patched, &head)], []).unwrap_err(),
            PatchError::UniverseNotPatched {
                fixture: FixtureId::new(1),
                universe: UniverseId::new(9),
            }
        );
        let twice = fixture(1, "test.movinghead", 1, 1);
        assert_eq!(
            MergeBody::for_patch(&layout(), [(&twice, &head), (&twice, &head)], []).unwrap_err(),
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
                executor: ExecutorId::new(1),
                on: true,
            })
            .unwrap();
        producer
            .push(TickCommand::SetExecutorLevel {
                executor: ExecutorId::new(1),
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
        // everything. Result 50000, coarse 0xC3, fine 0x50.
        let merged_pan = body.values().get(pan).copied().unwrap();
        assert_eq!(merged_pan, 45_000);
        let with_programmer = merge_programmer(merged_pan, Some(50_000));
        assert_eq!(with_programmer, 50_000);
        // The programmer layer is S6, so this value does not reach the frame
        // yet; the bytes the specification quotes are checked all the same.
        assert_eq!(coarse_byte(with_programmer), 0xC3);
        assert_eq!(fine_byte(with_programmer), 0x50);

        // "Turning executor 5 off changes nothing about pan while the
        // programmer holds it."
        body.layer_mut().deactivate(ExecutorId::new(5));
        body.resolve();
        let merged_pan = body.values().get(pan).copied().unwrap();
        assert_eq!(merge_programmer(merged_pan, Some(50_000)), 50_000);

        // "Clearing the programmer drops pan to 45000 only if executor 5 is
        // still active; otherwise it falls back to executor 3's 20000 ..."
        assert_eq!(merge_programmer(merged_pan, None), 20_000);
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
}
