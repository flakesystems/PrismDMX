//! The merge as a [`crate::TickBody`]: the seam S2 left open.
//!
//! [`MergeBody`] owns a [`crate::MergePlan`], the [`crate::PlaybackLayer`] over
//! it and the buffers the resolve needs, and runs the merge once per tick.
//! Everything it touches is sized when the body is built, so the tick itself
//! never reaches the allocator — `crates/prism-engine/tests/tick_allocations.rs`
//! counts that rather than asserting it.
//!
//! # What it does not do yet
//!
//! `render` resolves attribute *values*. It does not write the frame: turning a
//! 16-bit attribute value into DMX bytes — the coarse/fine split, the
//! attribute and per-fixture inverts, patch-time address validation — is S4,
//! and half of an encoder would be worse than none. The grand master, group
//! masters and the programmer state machine are S6. The commands for those
//! arrive here already and are deliberately ignored, which the tests state
//! outright so that the gap is a recorded decision rather than a surprise.

use prism_domain::ExecutorId;

use crate::command::TickCommand;
use crate::frame::DmxFrame;
use crate::plan::{MergeError, MergePlan};
use crate::playback::{MergeScratch, PlaybackLayer};
use crate::tick::{TickBody, TickInfo};

/// The HTP/LTP merge, wired into the tick.
#[derive(Debug, Clone)]
pub struct MergeBody {
    plan: MergePlan,
    layer: PlaybackLayer,
    scratch: MergeScratch,
    values: Box<[u16]>,
}

impl MergeBody {
    /// Builds the merge for a patch and a set of executors.
    ///
    /// Every buffer the tick will use is allocated here, once.
    ///
    /// # Errors
    ///
    /// [`MergeError::TooManySources`] if there are more executors than
    /// [`crate::MAX_SOURCES`].
    pub fn new(
        plan: MergePlan,
        executors: impl IntoIterator<Item = ExecutorId>,
    ) -> Result<Self, MergeError> {
        let layer = PlaybackLayer::new(&plan, executors)?;
        let scratch = MergeScratch::new(&plan);
        let values = vec![0; plan.slot_count()].into_boxed_slice();
        let mut body = Self {
            plan,
            layer,
            scratch,
            values,
        };
        // Start at the home layer rather than at zero, so a body that is read
        // before its first tick describes a rig at home rather than a blackout.
        body.resolve();
        Ok(body)
    }

    /// The patch this body merges over.
    #[must_use]
    pub const fn plan(&self) -> &MergePlan {
        &self.plan
    }

    /// The playback sources.
    #[must_use]
    pub const fn layer(&self) -> &PlaybackLayer {
        &self.layer
    }

    /// The playback sources, mutably — how S5 will feed cue values in.
    pub const fn layer_mut(&mut self) -> &mut PlaybackLayer {
        &mut self.layer
    }

    /// The merged attribute values, one per slot of [`Self::plan`].
    ///
    /// `0..=65535` regardless of the resolution the attribute is patched at:
    /// working in 16 bits until the final write is what keeps a fade over an
    /// 8-bit channel smooth (`docs/DMX_MERGE.md` §5). S4 turns these into bytes.
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
        } = self;
        layer.resolve(plan, scratch, values);
    }
}

impl TickBody for MergeBody {
    fn apply(&mut self, command: TickCommand) {
        match command {
            TickCommand::SetExecutorLevel { executor, level } => {
                self.layer.set_master(executor, level);
            }
            TickCommand::SetExecutorActive { executor, on } => {
                if on {
                    self.layer.activate(executor);
                } else {
                    self.layer.deactivate(executor);
                }
            }
            // Cue traversal is S5; the grand master and blackout are masters,
            // which `docs/DMX_MERGE.md` §4 applies after the merge — S6.
            TickCommand::Go { .. }
            | TickCommand::SetGrandMaster(_)
            | TickCommand::SetBlackout(_) => {}
        }
    }

    fn render(&mut self, _tick: &TickInfo, _frame: &mut DmxFrame) {
        self.resolve();
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::body::MergeBody;
    use crate::merge::{FULL, merge_programmer};
    use crate::plan::MergePlan;
    use crate::testkit::moving_head;
    use crate::{
        DmxFrame, Engine, FrameLayout, ManualClock, TickBody, TickCommand, TickInfo, command_queue,
    };
    use prism_domain::{AttributeType, ExecutorId, FixtureId, GoDirection, UniverseId};
    use std::sync::Arc;

    fn body(fixtures: u32, executors: u32) -> MergeBody {
        let head = moving_head();
        let plan = MergePlan::build((1..=fixtures).map(|id| (FixtureId::new(id), &head))).unwrap();
        MergeBody::new(plan, (1..=executors).map(ExecutorId::new)).unwrap()
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
        // Go is S5, the grand master and blackout are S6. Recorded as a test so
        // the gap is visible: a body that silently swallowed a blackout would
        // look like it worked.
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
            TickCommand::Go {
                executor: ExecutorId::new(1),
                direction: GoDirection::Next,
            },
        ] {
            body.apply(command);
        }
        body.resolve();
        assert_eq!(body.values().get(dimmer).copied(), Some(FULL));
    }

    #[test]
    fn rendering_resolves_the_merge_and_leaves_the_frame_to_s4() {
        let mut body = body(1, 1);
        let dimmer = slot(&body, 1, AttributeType::Dimmer);
        body.layer_mut()
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, 12_345);
        body.layer_mut().activate(ExecutorId::new(1));

        let layout = FrameLayout::new([UniverseId::MIN]).unwrap();
        let mut frame = DmxFrame::new(&layout);
        body.render(&tick(0), &mut frame);
        assert_eq!(body.values().get(dimmer).copied(), Some(12_345));
        // The encoder is S4; until it exists the frame stays as it was.
        assert!(frame.channels().iter().all(|&channel| channel == 0));
    }

    #[test]
    fn the_merge_runs_on_the_tick_through_the_command_queue() {
        let head = moving_head();
        let plan = MergePlan::build([(FixtureId::new(1), &head)]).unwrap();
        let mut body = MergeBody::new(plan, [ExecutorId::new(1)]).unwrap();
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
        let head = moving_head();
        let plan = MergePlan::build([(FixtureId::new(1), &head)]).unwrap();
        let mut body = MergeBody::new(plan, [ExecutorId::new(3), ExecutorId::new(5)]).unwrap();
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
        // "Written as coarse 0x7F, fine 0xFF" - the split itself is S4, but the
        // number the specification quotes is checked here all the same.
        assert_eq!((merged_dimmer >> 8) as u8, 0x7F);
        assert_eq!((merged_dimmer & 0xFF) as u8, 0xFF);

        // Pan is LTP: executor 5 has the higher activation counter and would
        // win with 45000 - but the programmer holds 50000, which overrides
        // everything. Result 50000, coarse 0xC3, fine 0x50.
        let merged_pan = body.values().get(pan).copied().unwrap();
        assert_eq!(merged_pan, 45_000);
        let with_programmer = merge_programmer(merged_pan, Some(50_000));
        assert_eq!(with_programmer, 50_000);
        assert_eq!((with_programmer >> 8) as u8, 0xC3);
        assert_eq!((with_programmer & 0xFF) as u8, 0x50);

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
