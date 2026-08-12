//! Everything the daemon is authoritative about, and the effects that follow.
//!
//! `prism-core` decides what a command *means* and answers with deltas and
//! `Effect`s; `prism-engine` runs the lighting; `prism-ipc` carries the result
//! outwards. This module is the only place that holds all three, which is what
//! S11 to S16 kept saying it would be:
//!
//! - **The daemon dispatches through `ShowFile::apply`** and nowhere else (S12,
//!   S14). Not `Show::apply`, not a second match over the command list, and not
//!   a journal of its own: `Command::is_session_command` routes, and `Oops` and
//!   `Redo` are carried out behind that door.
//! - **`Effect::Save` is the one effect that reaches here** (S15). A show file
//!   has a path, a disk and a failure mode, and the model that decides what a
//!   show *is* holds none of the three. [`Core::apply`] answers it with
//!   `ShowStore::save`, which returns the deltas so the `DirtyFlag` transition
//!   travels exactly once.
//! - **The programmer is diffed against what the engine was last told** (S13).
//!   `Delta::ProgrammerChanged` carries the whole state and the engine is
//!   addressed per `MergePlan` slot, so the two shapes do not match and
//!   something has to do the work. `MergeBody::load_programmer` allocates and is
//!   therefore set-up, not something to do per encoder turn.
//! - **`Show::patch_revision()` is the number to watch** (S11). When it moves,
//!   every slot index the daemon holds is stale, so the body is rebuilt and the
//!   plan is replaced before another programmer command is translated.
//!
//! # Why the engine's state is mirrored here
//!
//! The merge body lives on the tick thread and the core thread may not touch
//! it. So everything the daemon has ever *told* the engine — the grand master,
//! the blackout, the group levels — is kept here as well, and a rebuilt body is
//! set to it before it is handed over. A daemon that rebuilt without doing that
//! would answer a repatch by quietly putting the grand master back to full.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use prism_core::{Applied, Autosave, Effect, ShowFile, ShowFileError, ShowStore};
use prism_domain::{
    AttributeType, Command, Delta, ExecutorId, FixtureId, GroupId, NoticeLevel, ProgrammerState,
    UniverseId,
};
use prism_engine::{FrameLayout, MergeBody, MergePlan, PatchError, TickCommand};

use crate::engine::EngineThread;
use crate::log;

/// Why a command did not happen.
#[derive(Debug)]
pub enum CoreError {
    /// One of the three models refused it, and nothing changed (§5).
    Refused(ShowFileError),
    /// It was applied and the disk would not take it.
    Store(prism_core::StoreError),
}

impl core::fmt::Display for CoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Refused(error) => error.fmt(f),
            Self::Store(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for CoreError {}

/// The masters the daemon has moved, so a rebuilt body starts where the last
/// one was rather than at its constructor's defaults.
#[derive(Debug, Clone)]
struct Masters {
    grand: u16,
    blackout: bool,
    groups: BTreeMap<GroupId, u16>,
}

impl Default for Masters {
    /// Exactly transparent, which is what `MasterLayer::new` builds and what a
    /// desk nobody has touched should be.
    fn default() -> Self {
        Self {
            grand: prism_engine::FULL,
            blackout: false,
            groups: BTreeMap::new(),
        }
    }
}

/// The daemon's state: the show, the disk, and the engine it is driving.
pub struct Core {
    /// The show, the session, the programmer and the Oops journal.
    pub file: ShowFile,
    store: ShowStore,
    engine: EngineThread,
    layout: Arc<FrameLayout>,
    /// The plan the body on the tick thread is running. A copy, because the
    /// programmer is addressed by slot and the tick thread cannot be asked.
    plan: MergePlan,
    /// What the engine has been told the programmer holds.
    engine_programmer: ProgrammerState,
    masters: Masters,
    autosave: Autosave,
    started: Instant,
    /// The patch revision the current plan was built from (S11).
    patch_revision: u64,
}

impl Core {
    /// Wires a loaded show to a started engine.
    ///
    /// The engine is already running: it was started with a body built from
    /// this show, because `FramePublisher::subscribe` allocates and every output
    /// has to be attached before the tick begins (S2).
    ///
    /// # Errors
    ///
    /// [`PatchError`] if the show cannot be turned into a merge body, which
    /// after `MergeBody::for_patch` succeeded once cannot happen — the plan is
    /// rebuilt here so the daemon holds the same one the tick does.
    pub fn new(
        file: ShowFile,
        store: ShowStore,
        engine: EngineThread,
        layout: Arc<FrameLayout>,
    ) -> Result<Self, PatchError> {
        let plan = MergePlan::build(
            file.show
                .patched()
                .map(|(fixture, fixture_type)| (fixture.id, fixture_type)),
        )
        .map_err(PatchError::Plan)?;
        let patch_revision = file.show.patch_revision();
        let engine_programmer = file.programmer.state().clone();
        Ok(Self {
            file,
            store,
            engine,
            layout,
            plan,
            engine_programmer,
            masters: Masters::default(),
            autosave: Autosave::new(),
            started: Instant::now(),
            patch_revision,
        })
    }

    /// The show file on disk.
    #[must_use]
    pub const fn store(&self) -> &ShowStore {
        &self.store
    }

    /// The engine, for the daemon's own timers and its shutdown.
    #[must_use]
    pub const fn engine(&self) -> &EngineThread {
        &self.engine
    }

    /// How long the daemon has been running — the clock `Autosave` is polled on.
    #[must_use]
    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// The universes the show actually patches, for the telemetry channel.
    ///
    /// Not the frame layout: the layout is the desk's whole range so that
    /// patching a fixture into a new universe needs no restart, and sending 64
    /// universes of nothing to every client thirty times a second would be a
    /// megabyte a second of it.
    #[must_use]
    pub fn patched_universes(&self) -> Vec<UniverseId> {
        self.file.show.universes()
    }

    /// Applies a command through `ShowFile::apply` and carries out what it
    /// answered with.
    ///
    /// Returns everything to broadcast, in order: what the models produced,
    /// then what the effects produced. A refusal changes nothing and is
    /// reported as [`CoreError::Refused`].
    ///
    /// # Errors
    ///
    /// [`CoreError`] if the command was refused, or if it was applied and the
    /// save it asked for failed.
    pub fn apply(&mut self, command: &Command) -> Result<Vec<Delta>, CoreError> {
        let applied = self.file.apply(command).map_err(CoreError::Refused)?;
        self.carry_out(applied)
    }

    /// Carries out the effects of an [`Applied`] and returns everything to
    /// broadcast.
    fn carry_out(&mut self, applied: Applied) -> Result<Vec<Delta>, CoreError> {
        let Applied {
            mut deltas,
            effects,
        } = applied;

        // The patch is checked first and once: `Repatch` and `ReloadSequence`
        // in one answer are one rebuild, not two, and every slot index the
        // daemon holds is stale until it has happened.
        let rebuild = effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Repatch | Effect::ReloadGroups | Effect::ReloadSequence(_)
            )
        }) || self.patch_revision != self.file.show.patch_revision();

        for effect in &effects {
            match *effect {
                // Answered by the rebuild below.
                Effect::Repatch | Effect::ReloadGroups | Effect::ReloadSequence(_) => {}
                Effect::ExecutorGo {
                    executor,
                    direction,
                } => {
                    self.send(TickCommand::Go {
                        executor,
                        direction,
                    });
                    deltas.extend(self.record_executor(executor, true));
                }
                Effect::ExecutorOff { executor } => {
                    self.send(TickCommand::SetExecutorActive {
                        executor,
                        on: false,
                    });
                    deltas.extend(self.record_executor(executor, false));
                }
                Effect::SetExecutorMaster { executor, level } => {
                    self.send(TickCommand::SetExecutorLevel { executor, level });
                }
                // Carried out by `ShowFile::apply` and never handed on (S13,
                // S14). Named rather than caught by a wildcard, so an effect
                // added later is a compile error here.
                Effect::Programmer | Effect::Undo | Effect::Redo => {}
                Effect::Save => deltas.extend(self.save()?),
            }
        }

        if rebuild {
            self.rebuild()?;
        }
        // After the rebuild, so a programmer command that arrived with a
        // repatch is translated against the plan it belongs to.
        self.push_programmer();
        Ok(deltas)
    }

    /// Writes the show, and answers with the `DirtyFlag` transition.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the write failed — in which case nothing was
    /// saved and the Save LED stays lit, which is the truth (S15).
    pub fn save(&mut self) -> Result<Vec<Delta>, CoreError> {
        let deltas = self.store.save(&mut self.file).map_err(CoreError::Store)?;
        log::info("show", &format!("saved {}", self.store.path().display()));
        Ok(deltas)
    }

    /// Polls the autosave policy and writes the recovery copy if it is due.
    ///
    /// Answers with a `Delta::Notice` if the copy could not be written. The
    /// operator's file is untouched either way: an autosave is not a save, and
    /// `write_recovery` deliberately does not clear the dirty flag (S15).
    pub fn poll_autosave(&mut self) -> Vec<Delta> {
        if !self.autosave.poll(self.uptime(), self.file.is_dirty()) {
            return Vec::new();
        }
        match self.store.write_recovery(&self.file) {
            Ok(()) => {
                log::debug(
                    "show",
                    &format!(
                        "recovery copy written to {}",
                        self.store.recovery_path().display()
                    ),
                );
                Vec::new()
            }
            Err(error) => {
                let message = format!("the recovery copy could not be written: {error}");
                log::warn("show", &message);
                vec![Delta::Notice {
                    level: NoticeLevel::Warn,
                    message,
                }]
            }
        }
    }

    /// Loads a show file over the top of the running one.
    ///
    /// **A load is not a replay** (S15): `ShowStore::load` answers with
    /// `Repatch`, `ReloadGroups` and one `ReloadSequence` per sequence, which is
    /// the to-do list this carries out. The programmer and the journal are
    /// emptied by the loader, because both describe the show that was open a
    /// moment ago.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the file cannot be read, in which case the
    /// running show is untouched.
    pub fn load(&mut self) -> Result<Vec<Delta>, CoreError> {
        let effects = self.store.load(&mut self.file).map_err(CoreError::Store)?;
        // Everything about the desk is now about a different show, so the
        // masters go back to transparent rather than carrying the last show's
        // grand master into this one.
        self.masters = Masters::default();
        self.engine_programmer = ProgrammerState::default();
        self.carry_out(Applied {
            deltas: Vec::new(),
            effects,
        })
    }

    /// Rebuilds the merge body from the show and hands it to the tick.
    ///
    /// Everything [`build_body`] does allocates, which is exactly why it is
    /// here: the tick thread may not (`ARCHITECTURE_SPEC.md` §3.1).
    fn rebuild(&mut self) -> Result<(), CoreError> {
        let mut body = match build_body(&self.layout, &self.file) {
            Ok(body) => body,
            Err(error) => {
                // The show model and the engine agree about what a legal patch
                // is — `prism-core`'s `tests/show_to_engine.rs` asserts it over
                // arbitrary patches — so this is a defect rather than an
                // operator's mistake. The old body goes on running, which is
                // the least bad thing a desk can do.
                log::error(
                    "engine",
                    &format!("the patch could not be rebuilt and the rig is unchanged: {error}"),
                );
                return Ok(());
            }
        };
        self.masters.apply_to(&mut body, &self.file);

        self.plan = body.plan().clone();
        self.patch_revision = self.file.show.patch_revision();
        // The engine now holds exactly the programmer that was loaded into the
        // new body, so the next diff starts from there rather than from what
        // the old plan's slots meant.
        self.engine_programmer = self.file.programmer.state().clone();
        self.engine.install(body);
        self.engine.collect_retired();
        Ok(())
    }

    /// Sends the engine the difference between the programmer it was told about
    /// and the one the show file now holds.
    ///
    /// S13's requirement, and the reason it is a diff: `Delta::ProgrammerChanged`
    /// carries the whole state because that is what a client wants, and the
    /// engine takes one slot at a time because that is what fits through a
    /// queue with no allocation in it.
    fn push_programmer(&mut self) {
        let current = self.file.programmer.state().clone();
        if current.values == self.engine_programmer.values {
            self.engine_programmer = current;
            return;
        }
        if current.values.is_empty() {
            // The ordinary case after a Clear, and one command instead of one
            // per value.
            self.send(TickCommand::ClearProgrammer);
            self.engine_programmer = current;
            return;
        }
        for (slot, value) in self.programmer_changes(&current) {
            match value {
                Some(value) => self.send(TickCommand::SetProgrammerValue { slot, value }),
                None => self.send(TickCommand::ClearProgrammerValue { slot }),
            }
        }
        self.engine_programmer = current;
    }

    /// Which merge-plan slots changed, and to what. `None` means the slot is no
    /// longer held and the playbacks below decide again.
    fn programmer_changes(&self, current: &ProgrammerState) -> Vec<(u32, Option<u16>)> {
        let mut changes = Vec::new();
        let mut visit = |fixture: FixtureId, attribute: AttributeType, value: Option<u16>| {
            if let Some(slot) = self.plan.index_of(fixture, attribute) {
                changes.push((u32::try_from(slot).unwrap_or(u32::MAX), value));
            }
        };
        for (&fixture, attributes) in &current.values {
            for (&attribute, value) in attributes {
                let before = self
                    .engine_programmer
                    .value(fixture, attribute)
                    .map(|held| held.value);
                if before != Some(value.value) {
                    visit(fixture, attribute, Some(value.value));
                }
            }
        }
        for (&fixture, attributes) in &self.engine_programmer.values {
            for &attribute in attributes.keys() {
                if current.value(fixture, attribute).is_none() {
                    visit(fixture, attribute, None);
                }
            }
        }
        changes
    }

    /// Records that an executor started or stopped, and answers with the delta.
    ///
    /// The cue index is left as the show has it: what cue a playback is on is
    /// the tick thread's, and reading it back is the feedback channel S18 and
    /// S26 need rather than something to invent here.
    fn record_executor(&mut self, executor: ExecutorId, is_active: bool) -> Vec<Delta> {
        let cue_index = self
            .file
            .show
            .executor(executor)
            .and_then(|executor| executor.current_cue_index);
        match self
            .file
            .show
            .record_executor_state(executor, is_active, cue_index)
        {
            Ok(true) => vec![Delta::ExecutorState {
                executor_id: executor,
                is_active,
                cue_index,
            }],
            // Nothing changed, or the executor has gone — either way there is
            // nothing to tell anybody, and the command that produced this was
            // already validated against the show.
            Ok(false) | Err(_) => Vec::new(),
        }
    }

    /// Writes the recovery copy now, whatever the interval says.
    ///
    /// What an operator's *Save recovery copy* would call, and what a test uses
    /// instead of waiting thirty seconds for the policy to come round.
    pub fn force_autosave(&mut self) -> Vec<Delta> {
        self.autosave = Autosave::every(Duration::ZERO);
        let deltas = self.poll_autosave();
        self.autosave = Autosave::new();
        deltas
    }

    /// Moves the grand master. Held here as well as in the engine, so a rebuild
    /// does not put it back to full.
    pub fn set_grand_master(&mut self, level: u16) {
        self.masters.grand = level;
        self.send(TickCommand::SetGrandMaster(level));
    }

    /// Switches blackout on or off.
    ///
    /// **Blackout is a frame, not a flag** (S10): a daemon that wants a dark
    /// stage publishes one and lets the outputs send it, rather than telling
    /// the drivers to stop. That is what makes the shutdown option in
    /// [`crate::daemon`] work at all.
    pub fn set_blackout(&mut self, on: bool) {
        self.masters.blackout = on;
        self.send(TickCommand::SetBlackout(on));
    }

    /// Moves a group master.
    pub fn set_group_master(&mut self, group: GroupId, level: u16) {
        self.masters.groups.insert(group, level);
        self.send(TickCommand::SetGroupMaster { group, level });
    }

    fn send(&mut self, command: TickCommand) {
        if !self.engine.send(command) {
            // The queue is a thousand deep and drained every 23 ms, so a full
            // one means the tick has stopped. Saying so is all that can be
            // done from here.
            log::error(
                "engine",
                "the tick command queue is full: the engine is not draining it",
            );
        }
    }
}

impl Masters {
    /// Puts this desk's masters onto a freshly built body.
    fn apply_to(&self, body: &mut MergeBody, file: &ShowFile) {
        body.masters_mut().set_grand(self.grand);
        body.masters_mut().set_blackout(self.blackout);
        for (&group, &level) in &self.groups {
            body.masters_mut().set_group_level(group, level);
        }
        // An executor's master is show state (S14 relied on it being so), which
        // means the show is where a rebuild reads it from rather than a second
        // copy that could disagree.
        for executor in file.show.executors() {
            body.layer_mut()
                .set_master(executor.id, executor.master_level);
        }
        body.resolve();
    }
}

/// Everything a show has to say to the engine, in one merge body.
///
/// The four set-up doors `prism-core`'s `tests/show_to_engine.rs` pinned:
/// `for_patch` from `Show::patched`, `load_groups` and `load_sequence` from the
/// show's own pools, and `load_programmer` from the operator's live edit. All
/// four allocate, which is why this is a function the *core* thread calls and
/// never something the tick does.
///
/// The masters are not here: they are the daemon's own state rather than the
/// show's, and [`Core`] applies them on top.
///
/// # Errors
///
/// [`PatchError`] if the patch cannot be merged or encoded. A show the model
/// accepted is one the engine accepts, so this is a defect rather than an
/// operator's mistake.
pub fn build_body(layout: &FrameLayout, file: &ShowFile) -> Result<MergeBody, PatchError> {
    let executors: Vec<ExecutorId> = file.show.executors().map(|executor| executor.id).collect();
    let mut body = MergeBody::for_patch(layout, file.show.patched(), executors)?;

    let groups: Vec<prism_domain::Group> = file.show.groups().cloned().collect();
    body.load_groups(&groups);
    for executor in file.show.executors() {
        let Some(sequence_id) = executor.sequence_id else {
            continue;
        };
        if let Some(sequence) = file.show.sequence(sequence_id)
            && let Err(error) = body.load_sequence(executor.id, sequence)
        {
            // A sequence the engine will not compile is one executor that does
            // nothing, and the rest of the rig is unaffected — which is why it
            // is reported rather than refused.
            log::warn(
                "engine",
                &format!("executor {} could not be loaded: {error}", executor.id),
            );
        }
    }
    let unresolved = body.load_programmer(file.programmer.state());
    if unresolved > 0 {
        log::debug(
            "engine",
            &format!("{unresolved} programmer values name something this patch does not have"),
        );
    }
    body.resolve();
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::{Core, CoreError};
    use crate::engine::EngineThread;
    use crate::testkit::{cue, dimmer_type, fixture, sequence, show_file};
    use prism_core::ShowStore;
    use prism_domain::{
        AttributeType, Command, Delta, ExecutorId, FixtureId, GoDirection, GroupId, SelectionMode,
        SequenceId, UniverseId,
    };
    use prism_engine::FramePublisher;
    use prism_protocols::{MockOutput, MockOutputHandle, OutputThread, RunnerConfig, spawn};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// A daemon core with one mock output on universe 1, and the handle that
    /// says what reached the wire.
    fn desk(dir: &std::path::Path) -> (Core, MockOutputHandle, OutputThread) {
        let file = show_file();
        let store = ShowStore::open(dir.join("test.prism")).unwrap();
        let layout = Arc::new(crate::engine::frame_layout(4).unwrap());
        let body = super::build_body(&layout, &file).unwrap();

        let mut publisher = FramePublisher::new(Arc::clone(&layout));
        let subscriber = publisher.subscribe();
        let output = MockOutput::new(prism_domain::OutputId::new(1), [UniverseId::new(1)]);
        let frames = output.handle();
        let driver = spawn("out-mock", output, subscriber, RunnerConfig::default()).unwrap();
        let engine = EngineThread::start(body, publisher).unwrap();

        let core = Core::new(file, store, engine, layout).unwrap();
        (core, frames, driver)
    }

    fn until(what: &str, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if condition() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("timed out waiting for {what}");
    }

    fn channel(frames: &MockOutputHandle, channel: usize) -> Option<u8> {
        frames
            .last_frame()
            .and_then(|(_, data)| data.get(channel - 1).copied())
    }

    #[test]
    fn a_programmer_value_reaches_the_wire_and_a_clear_takes_it_back() {
        // The whole of S13's requirement in one run: the delta carries the
        // state, the engine is addressed per slot, and the daemon is what turns
        // one into the other.
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        until("the rig at home", || channel(&frames, 1) == Some(255));

        core.apply(&Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        })
        .unwrap();
        let deltas = core
            .apply(&Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                value: 0,
                relative: false,
            })
            .unwrap();
        assert!(
            deltas
                .iter()
                .any(|delta| matches!(delta, Delta::ProgrammerChanged { .. })),
            "a client has to be told, {deltas:?}"
        );
        until("the programmer value on the wire", || {
            channel(&frames, 1) == Some(0)
        });

        core.apply(&Command::ClearProgrammer).unwrap();
        until("the playbacks to decide again", || {
            channel(&frames, 1) == Some(255)
        });

        driver.stop();
    }

    #[test]
    fn a_repatch_rebuilds_the_engine_and_the_old_rig_goes_dark() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        until("the rig at home", || channel(&frames, 1) == Some(255));

        // Move the dimmer to channel 100. Channel 1 is then patched by nothing,
        // and S11's second half of `Effect::Repatch` is what has to blank it.
        let swaps = core.engine().health().swaps();
        core.apply(&Command::PatchFixture {
            id: FixtureId::new(1),
            name: "Fixture 1".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: UniverseId::new(1),
            address: 100,
        })
        .unwrap();

        until("the rebuilt rig", || {
            channel(&frames, 100) == Some(255) && channel(&frames, 1) == Some(0)
        });
        assert!(core.engine().health().swaps() > swaps);

        driver.stop();
    }

    #[test]
    fn a_programmer_value_survives_a_repatch_and_lands_on_the_new_slot() {
        // The trap S11 named: the programmer is addressed by merge-plan slot,
        // and a repatch renumbers every slot. A daemon that kept the old index
        // would put the operator's value on a different fixture.
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        core.apply(&Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        })
        .unwrap();
        core.apply(&Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            value: 32768,
            relative: false,
        })
        .unwrap();
        until("the held value", || channel(&frames, 1) == Some(128));

        // Patching a *second* fixture in front of it moves the dimmer's slot.
        core.apply(&Command::PatchFixture {
            id: FixtureId::new(3),
            name: "Fixture 3".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: UniverseId::new(1),
            address: 200,
        })
        .unwrap();
        until("the held value on the new plan", || {
            channel(&frames, 1) == Some(128) && channel(&frames, 200) == Some(255)
        });

        driver.stop();
    }

    #[test]
    fn a_master_survives_a_rebuild() {
        // A repatch that quietly put the grand master back to full would be a
        // stage that lights up in the middle of a show.
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        until("the rig at home", || channel(&frames, 1) == Some(255));

        core.set_grand_master(32768);
        until("the grand master", || channel(&frames, 1) == Some(128));

        core.apply(&Command::PatchFixture {
            id: FixtureId::new(3),
            name: "Fixture 3".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: UniverseId::new(1),
            address: 200,
        })
        .unwrap();
        until("the new fixture", || channel(&frames, 200) == Some(128));
        assert_eq!(
            channel(&frames, 1),
            Some(128),
            "the grand master must not come back to full behind a repatch"
        );

        // And the group master and the blackout take the same road.
        core.set_group_master(GroupId::new(1), 0);
        until("the group master", || channel(&frames, 1) == Some(0));
        core.set_group_master(GroupId::new(1), 65535);
        core.set_blackout(true);
        until("the blackout", || channel(&frames, 1) == Some(0));

        driver.stop();
    }

    #[test]
    fn a_go_reaches_the_playback_and_the_client_is_told() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        until("the rig at home", || channel(&frames, 1) == Some(255));

        let deltas = core
            .apply(&Command::ExecutorGo {
                executor_id: ExecutorId::new(0),
                direction: GoDirection::Next,
            })
            .unwrap();
        assert!(
            deltas.contains(&Delta::ExecutorState {
                executor_id: ExecutorId::new(0),
                is_active: true,
                cue_index: None,
            }),
            "{deltas:?}"
        );
        // The cue raises the dark dimmer on channel 5. Not channel 1: the
        // playbacks merge HTP against the home layer, so a cue can only ever be
        // seen where home is below it.
        until("the cue", || {
            channel(&frames, 5).is_some_and(|level| level > 0)
        });

        let deltas = core
            .apply(&Command::ExecutorOff {
                executor_id: ExecutorId::new(0),
            })
            .unwrap();
        assert!(
            deltas.iter().any(|delta| matches!(
                delta,
                Delta::ExecutorState {
                    is_active: false,
                    ..
                }
            )),
            "{deltas:?}"
        );

        driver.stop();
    }

    #[test]
    fn a_save_is_the_one_effect_that_reaches_the_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, _frames, driver) = desk(dir.path());

        core.apply(&Command::PatchFixture {
            id: FixtureId::new(3),
            name: "Fixture 3".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: UniverseId::new(1),
            address: 200,
        })
        .unwrap();
        assert!(core.file.is_dirty());

        let deltas = core.apply(&Command::SaveShow).unwrap();
        assert!(
            deltas.contains(&Delta::DirtyFlag {
                unsaved_changes: false
            }),
            "the Save LED goes out exactly once, {deltas:?}"
        );
        assert!(!core.file.is_dirty());

        // And what is on the platter is what was in memory.
        let read = core.store().read().unwrap();
        assert!(read.show.fixture(FixtureId::new(3)).is_some());

        driver.stop();
    }

    #[test]
    fn a_refused_command_changes_nothing_and_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, _frames, driver) = desk(dir.path());
        let before = serde_json::to_vec(&core.file.show).unwrap();

        let error = core
            .apply(&Command::ExecutorGo {
                executor_id: ExecutorId::new(9),
                direction: GoDirection::Next,
            })
            .unwrap_err();
        assert!(matches!(error, CoreError::Refused(_)));
        assert!(error.to_string().contains('9'), "{error}");
        assert_eq!(
            serde_json::to_vec(&core.file.show).unwrap(),
            before,
            "a refusal leaves the show byte-identical"
        );

        driver.stop();
    }

    #[test]
    fn a_load_is_a_to_do_list_and_the_new_rig_reaches_the_wire() {
        // S15: `ShowStore::load` answers with `Repatch`, `ReloadGroups` and one
        // `ReloadSequence` per sequence, and this is what carries them out.
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        until("the first rig", || channel(&frames, 1) == Some(255));

        // Write a different show into the same file, behind the daemon's back.
        {
            let mut other = show_file();
            other.show = prism_core::Show::new();
            other
                .show
                .embed_fixture_type(dimmer_type("generic.dimmer", 0))
                .unwrap();
            other
                .show
                .patch_fixture(fixture(7, "generic.dimmer", 1, 42))
                .unwrap();
            other
                .show
                .store_sequence(sequence(2, vec![cue("1", 7, AttributeType::Dimmer, 65535)]))
                .unwrap();
            let mut store = ShowStore::open(dir.path().join("test.prism")).unwrap();
            store.save(&mut other).unwrap();
        }

        core.load().unwrap();
        until("the loaded rig", || {
            channel(&frames, 42) == Some(0) && channel(&frames, 1) == Some(0)
        });
        assert!(core.file.show.fixture(FixtureId::new(7)).is_some());
        assert!(core.file.programmer.state().is_empty(), "a load empties it");
        assert_eq!(core.file.journal.len(), 0, "and the journal with it");

        driver.stop();
    }

    #[test]
    fn an_autosave_writes_a_recovery_copy_and_only_while_there_is_something_to_recover() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, _frames, driver) = desk(dir.path());
        // Nothing unsaved: the policy answers no however long the daemon has
        // been up.
        assert!(core.poll_autosave().is_empty());
        assert!(!core.store().has_recovery());

        core.apply(&Command::PatchFixture {
            id: FixtureId::new(3),
            name: "Fixture 3".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: UniverseId::new(1),
            address: 200,
        })
        .unwrap();
        // The interval runs from the edit, not from the start, so the copy is
        // not due yet — and driving the clock is the daemon's job, so the test
        // asks the policy directly rather than waiting thirty seconds.
        assert!(core.poll_autosave().is_empty());
        core.force_autosave();
        assert!(core.store().has_recovery());

        // And a save clears it away again.
        core.apply(&Command::SaveShow).unwrap();
        assert!(!core.store().has_recovery());

        driver.stop();
    }

    #[test]
    fn an_executor_master_reaches_the_engine_and_survives_a_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        until("the rig at home", || channel(&frames, 1) == Some(255));

        // The cue raises channel 5; the master scales what it raises it to.
        core.apply(&Command::ExecutorGo {
            executor_id: ExecutorId::new(0),
            direction: GoDirection::Next,
        })
        .unwrap();
        until("the cue", || {
            channel(&frames, 5).is_some_and(|level| level > 0)
        });
        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: 0,
        })
        .unwrap();
        until("the master to take it back down", || {
            channel(&frames, 5) == Some(0)
        });

        // And a rebuild reads it back out of the show rather than from a second
        // copy that could disagree (S14: an executor master *is* show state).
        core.apply(&Command::PatchFixture {
            id: FixtureId::new(3),
            name: "Fixture 3".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: UniverseId::new(1),
            address: 200,
        })
        .unwrap();
        until("the rebuilt rig", || channel(&frames, 200) == Some(255));
        assert_eq!(
            channel(&frames, 5),
            Some(0),
            "the executor master must not come back to full behind a repatch"
        );

        driver.stop();
    }

    /// The Oops journal, reached the way S14 said it must be — through
    /// `ShowFile::apply`, with no second journal in the daemon — and the one
    /// path that produces a `ClearProgrammerValue` for a single slot.
    #[test]
    fn an_oops_takes_one_programmer_value_back_off_the_rig() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        until("the rig at home", || channel(&frames, 1) == Some(255));

        core.apply(&Command::SelectFixtures {
            ids: vec![FixtureId::new(1), FixtureId::new(2)],
            mode: SelectionMode::Set,
        })
        .unwrap();
        core.apply(&Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            value: 0,
            relative: false,
        })
        .unwrap();
        core.apply(&Command::SetAttribute {
            attribute: AttributeType::Red,
            value: 65535,
            relative: false,
        })
        .unwrap();
        until("both values on the wire", || {
            channel(&frames, 1) == Some(0) && channel(&frames, 10) == Some(255)
        });

        // One command back. The red value goes and the dimmer stays, which is
        // the case that produces a per-slot clear rather than a whole-programmer
        // one — and there is no way to reach it except through the journal.
        core.apply(&Command::Oops).unwrap();
        until("the red value to be taken back", || {
            channel(&frames, 10) == Some(0) && channel(&frames, 1) == Some(0)
        });
        assert!(
            core.file
                .programmer
                .state()
                .value(FixtureId::new(1), AttributeType::Dimmer)
                .is_some(),
            "the dimmer was set by an earlier command and stays"
        );

        // And forward again.
        core.apply(&Command::Redo).unwrap();
        until("the red value to come back", || {
            channel(&frames, 10) == Some(255)
        });

        driver.stop();
    }

    #[test]
    fn a_go_that_changes_nothing_tells_nobody() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, _frames, driver) = desk(dir.path());
        let first = core
            .apply(&Command::ExecutorGo {
                executor_id: ExecutorId::new(0),
                direction: GoDirection::Next,
            })
            .unwrap();
        assert!(
            first
                .iter()
                .any(|delta| matches!(delta, Delta::ExecutorState { .. }))
        );
        // Already running: the executor moves to the next cue, which is show
        // state the daemon does not know, so there is nothing to report about
        // it being active — an LED cannot be lit twice (S11).
        let again = core
            .apply(&Command::ExecutorGo {
                executor_id: ExecutorId::new(0),
                direction: GoDirection::Next,
            })
            .unwrap();
        assert!(
            !again
                .iter()
                .any(|delta| matches!(delta, Delta::ExecutorState { .. })),
            "{again:?}"
        );
        driver.stop();
    }

    /// A recovery copy that cannot be written is a warning, not a failure: the
    /// operator's own file is untouched either way, and an autosave that took
    /// the daemon down would be the cure being worse than the disease.
    #[test]
    fn an_autosave_that_cannot_be_written_is_reported_and_the_daemon_carries_on() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, _frames, driver) = desk(dir.path());
        // A directory where the recovery copy wants to be — which is what a
        // permission or a file somebody has open looks like from here.
        std::fs::create_dir(core.store().recovery_path()).unwrap();

        core.apply(&Command::PatchFixture {
            id: FixtureId::new(3),
            name: "Fixture 3".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: UniverseId::new(1),
            address: 200,
        })
        .unwrap();
        let deltas = core.force_autosave();
        assert!(
            deltas.iter().any(|delta| matches!(
                delta,
                Delta::Notice {
                    level: prism_domain::NoticeLevel::Warn,
                    ..
                }
            )),
            "the operator has to be told their unsaved work is not being kept: {deltas:?}"
        );
        assert!(core.file.is_dirty(), "and nothing was saved");

        driver.stop();
    }

    #[test]
    fn a_sequence_stored_into_reaches_the_engine() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk(dir.path());
        until("the rig at home", || channel(&frames, 1) == Some(255));

        core.apply(&Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        })
        .unwrap();
        core.apply(&Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            value: 16384,
            relative: false,
        })
        .unwrap();
        let swaps = core.engine().health().swaps();
        core.apply(&Command::StoreCue {
            sequence_id: SequenceId::new(1),
            cue_number: "2".to_owned(),
        })
        .unwrap();
        until("the reloaded cue list", || {
            core.engine().health().swaps() > swaps
        });
        assert_eq!(
            core.file
                .show
                .sequence(SequenceId::new(1))
                .unwrap()
                .cues
                .len(),
            2
        );

        driver.stop();
    }
}
