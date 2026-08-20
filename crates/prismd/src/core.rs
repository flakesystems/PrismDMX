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
use std::time::Duration;

use prism_core::{Applied, Autosave, Effect, ShowFile, ShowFileError, ShowStore};
use prism_domain::{
    AttributeType, Command, Delta, ExecutorId, FixtureId, GroupId, NoticeLevel, ProgrammerState,
    UniverseId,
};
use prism_engine::{FrameLayout, MergeBody, MergePlan, PatchError, PlaybackReport, TickCommand};

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
    /// What the tick says its playbacks are doing — the channel S26 recorded as
    /// missing and S34 built. Written by the tick, read here; see
    /// `prism_engine::PlaybackReport` for why it is a table of atomics and not a
    /// queue.
    report: Arc<PlaybackReport>,
    /// What was last broadcast about each executor, so a poll that found nothing
    /// new says nothing. A `Delta::ExecutorState` per tick would move the show
    /// document at playback rate, and every view that watches the show would
    /// re-ask its questions with it.
    reported: BTreeMap<ExecutorId, (bool, Option<u32>)>,
    autosave: Autosave,
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
        report: Arc<PlaybackReport>,
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
            report,
            reported: BTreeMap::new(),
            autosave: Autosave::new(),
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
        // The **engine's** clock, not this type's. `TickHealth` counts ticks
        // from the moment the tick thread started, so the time it is divided by
        // has to start there too — wiring the daemon up takes a while, and S44
        // made it take longer by reading a fixture library. Measured from here,
        // the reported rate read 97 Hz for an engine ticking at 44.
        self.engine.uptime()
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
                // **Playback state is the tick's to report, and only the
                // tick's** (S34). Until the readback existed the daemon wrote
                // `is_active` here on the way past, because nothing else could;
                // now that would be a second author racing the first, and the
                // loser is whichever arrives second. What that looked like: the
                // strip lights on the command, goes dark on the next poll
                // because the tick had not run yet, and lights again a tick
                // later. `Core::poll_playback` is the one author.
                Effect::ExecutorGo {
                    executor,
                    direction,
                } => self.send(TickCommand::Go {
                    executor,
                    direction,
                }),
                Effect::ExecutorOff { executor } => self.send(TickCommand::SetExecutorActive {
                    executor,
                    on: false,
                }),
                Effect::ExecutorOn { executor } => {
                    self.send(TickCommand::SetExecutorActive { executor, on: true });
                }
                // A flash does **not** touch the stored master and does not
                // record `is_active` either: it is a momentary gesture, and the
                // readback is what tells the desk the strip is lit. Writing
                // playback state here as well would be two authors for one
                // field, one of them a guess.
                Effect::ExecutorFlash { executor, on } => {
                    self.send(TickCommand::SetExecutorFlash { executor, on });
                }
                Effect::ExecutorSpeed { executor, speed } => {
                    self.send(TickCommand::SetExecutorSpeed { executor, speed });
                }
                Effect::ExecutorTapSpeed { executor } => {
                    self.send(TickCommand::TapExecutorSpeed { executor });
                }
                Effect::ExecutorXFade { executor, position } => {
                    self.send(TickCommand::SetExecutorXFade { executor, position });
                }
                Effect::SetExecutorMaster { executor, level } => {
                    self.send(TickCommand::SetExecutorLevel { executor, level });
                }
                // Carried out by `ShowFile::apply` and never handed on (S13,
                // S14, and S44's `EmbedProfile`, which needs the desk's
                // library and gets it there). Named rather than caught by a
                // wildcard, so an effect added later is a compile error here.
                Effect::Programmer | Effect::Undo | Effect::Redo | Effect::EmbedProfile => {}
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
        let mut body = match build_body(&self.layout, &self.file, &self.report) {
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

    /// Reads what the tick says its playbacks are doing and answers with what
    /// has changed since the last poll.
    ///
    /// **The channel S26 recorded as missing.** `Executor::current_cue_index`
    /// has been in the domain since S1 and on the wire since S11 with nothing
    /// filling it, because what cue a playback is on lives on the tick thread.
    /// It is polled rather than pushed for the reason
    /// `prism_engine::PlaybackReport` gives: the tick may not allocate, lock or
    /// block, so it publishes the current state and whoever cares samples it.
    ///
    /// **Only differences are broadcast**, and that is what keeps the show
    /// document still during a fade: a cue index moves when a cue changes, not
    /// when a level does. S28 left the warning — `Query::StorePreview` is asked
    /// once per delta — and a delta per tick would have turned it into a
    /// question per frame.
    pub fn poll_playback(&mut self) -> Vec<Delta> {
        let mut deltas = Vec::new();
        let mut seen: BTreeMap<ExecutorId, (bool, Option<u32>)> = BTreeMap::new();
        for state in self.report.states() {
            seen.insert(state.executor, (state.is_active, state.cue_index));
            if self.reported.get(&state.executor) == Some(&(state.is_active, state.cue_index)) {
                continue;
            }
            // The show is asked as well, because the desk's own record is what a
            // fresh client's snapshot carries and it may already agree — a Go
            // wrote `is_active` on the way past, and the cue index is what
            // arrives late.
            // Nothing to broadcast when the show already agreed, and nothing at
            // all for an executor the show no longer has — the tick is one body
            // behind after a rebuild, and that is ordinary.
            if let Ok(true) = self.file.show.record_executor_state(
                state.executor,
                state.is_active,
                state.cue_index,
            ) {
                deltas.push(Delta::ExecutorState {
                    executor_id: state.executor,
                    is_active: state.is_active,
                    cue_index: state.cue_index,
                });
            }
        }
        self.reported = seen;
        deltas
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
        // The executors' own masters and speeds are put on by `build_body`,
        // which is where show state belongs: this type holds only what the
        // *daemon* has moved and the show does not carry.
        let _ = file;
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
pub fn build_body(
    layout: &FrameLayout,
    file: &ShowFile,
    report: &Arc<PlaybackReport>,
) -> Result<MergeBody, PatchError> {
    let executors: Vec<ExecutorId> = file.show.executors().map(|executor| executor.id).collect();
    let mut body = MergeBody::for_patch(layout, file.show.patched(), executors)?;
    body.report_into(Arc::clone(report));

    let groups: Vec<prism_domain::Group> = file.show.groups().cloned().collect();
    body.load_groups(&groups);
    // An executor's master and its speed are **show** state (S14 relied on the
    // first being so, S34 made the second), so a freshly built body reads them
    // out of the show rather than starting at its constructor's defaults. Doing
    // it here rather than in `Masters::apply_to` is what gives the *first* body
    // — the one `daemon` builds before the tick starts — the levels a saved show
    // was saved with.
    for executor in file.show.executors() {
        body.layer_mut()
            .set_master(executor.id, executor.master_level);
        if let Some(player) = body.cues_mut().player_mut(executor.id) {
            player.set_speed(executor.speed);
        }
    }
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
        desk_with(dir, show_file())
    }

    /// The same, over a show a test built for itself.
    fn desk_with(
        dir: &std::path::Path,
        file: prism_core::ShowFile,
    ) -> (Core, MockOutputHandle, OutputThread) {
        let store = ShowStore::open(dir.join("test.prism")).unwrap();
        let layout = Arc::new(crate::engine::frame_layout(4).unwrap());
        let report = Arc::new(prism_engine::PlaybackReport::new(prism_engine::MAX_SOURCES));
        let body = super::build_body(&layout, &file, &report).unwrap();

        let mut publisher = FramePublisher::new(Arc::clone(&layout));
        let subscriber = publisher.subscribe();
        let output = MockOutput::new(prism_domain::OutputId::new(1), [UniverseId::new(1)]);
        let frames = output.handle();
        let driver = spawn("out-mock", output, subscriber, RunnerConfig::default()).unwrap();
        let engine = EngineThread::start(body, publisher).unwrap();

        let core = Core::new(file, store, engine, layout, report).unwrap();
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

        // The command itself says nothing about the playback: since S34 what an
        // executor is doing comes back from the **tick**, which is the only
        // thing that knows. A daemon that also wrote it here would be a second
        // author racing the first.
        let deltas = core
            .apply(&Command::ExecutorGo {
                executor_id: ExecutorId::new(0),
                direction: GoDirection::Next,
            })
            .unwrap();
        assert!(deltas.is_empty(), "{deltas:?}");

        // The cue raises the dark dimmer on channel 5. Not channel 1: the
        // playbacks merge HTP against the home layer, so a cue can only ever be
        // seen where home is below it.
        until("the cue", || {
            channel(&frames, 5).is_some_and(|level| level > 0)
        });
        let mut told = Vec::new();
        until("the client to be told", || {
            told.extend(core.poll_playback());
            told.contains(&Delta::ExecutorState {
                executor_id: ExecutorId::new(0),
                is_active: true,
                cue_index: Some(0),
            })
        });

        core.apply(&Command::ExecutorOff {
            executor_id: ExecutorId::new(0),
        })
        .unwrap();
        let mut stopped = Vec::new();
        until("the client to be told it stopped", || {
            stopped.extend(core.poll_playback());
            stopped.iter().any(|delta| {
                matches!(
                    delta,
                    Delta::ExecutorState {
                        is_active: false,
                        cue_index: None,
                        ..
                    }
                )
            })
        });

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
        core.apply(&Command::ExecutorGo {
            executor_id: ExecutorId::new(0),
            direction: GoDirection::Next,
        })
        .unwrap();
        let mut first = Vec::new();
        until("the first Go to be reported", || {
            first.extend(core.poll_playback());
            !first.is_empty()
        });
        assert!(
            first
                .iter()
                .any(|delta| matches!(delta, Delta::ExecutorState { .. }))
        );

        // Already running, and this sequence has one cue: the Go steps to the
        // cue it is already on, so nothing about the executor moved and the
        // readback has nothing to say. An LED cannot be lit twice (S11), and
        // since S34 the *readback* is what would say so.
        core.apply(&Command::ExecutorGo {
            executor_id: ExecutorId::new(0),
            direction: GoDirection::Next,
        })
        .unwrap();
        let started = Instant::now();
        let mut again = Vec::new();
        while started.elapsed() < Duration::from_millis(200) {
            again.extend(core.poll_playback());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(again.is_empty(), "{again:?}");
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

    // -- S34: the eight button functions, asserted on frames ----------------

    /// A rig with **one** executor whose four buttons and fader are set by the
    /// test, playing a two-cue list on the dark dimmer at channel 5.
    ///
    /// Channel 5 is dark at home, so a cue raising it is visible; channel 1 sits
    /// at full whatever happens, which is what says the rig is alive.
    fn desk_for_buttons(
        dir: &std::path::Path,
        buttons: Vec<prism_domain::ExecutorButtonFunction>,
        fader: prism_domain::ExecutorFaderFunction,
    ) -> (Core, MockOutputHandle, OutputThread) {
        use crate::testkit::{cue, executor, sequence};
        let mut file = show_file();
        file.show
            .store_sequence(sequence(
                7,
                vec![
                    cue("1", 4, AttributeType::Dimmer, 65_535),
                    cue("2", 4, AttributeType::Dimmer, 20_000),
                ],
            ))
            .unwrap();
        let mut slot = executor(3, Some(7));
        slot.button_functions = buttons;
        slot.fader_function = fader;
        slot.master_level = u16::MAX;
        file.show.store_executor(slot).unwrap();
        file.show.mark_saved();
        desk_with(dir, file)
    }

    fn press(core: &mut Core, index: u8, pressed: bool) -> Vec<Delta> {
        core.apply(&Command::ExecutorButton {
            executor_id: ExecutorId::new(3),
            button: prism_domain::ExecutorButtonRef::Slot { index },
            pressed,
        })
        .expect("the executor plays a sequence")
    }

    /// **Exit criterion.** Every one of the eight `ExecutorButtonFunction`
    /// values does what its name says — checked on the **frames** the mock
    /// output receives rather than on `isActive`, because a test that read the
    /// flag the command sets would be asking the code under test what it did.
    #[test]
    fn each_of_the_eight_button_functions_does_what_its_name_says_on_the_wire() {
        use prism_domain::ExecutorButtonFunction as Fn;
        let dir = tempfile::tempdir().unwrap();
        // Rec = On, Solo = Off, Mute = Go+, Select = Go-.
        let (mut core, frames, driver) = desk_for_buttons(
            dir.path(),
            vec![Fn::On, Fn::Off, Fn::GoForward, Fn::GoBack],
            prism_domain::ExecutorFaderFunction::Master,
        );
        until("the rig at home", || channel(&frames, 1) == Some(255));
        assert_eq!(channel(&frames, 5), Some(0), "channel 5 is dark at home");

        // `On` — the sequence starts at its first cue, which is full.
        press(&mut core, 0, true);
        until("On to start the list", || channel(&frames, 5) == Some(255));

        // `On` again does **not** restart the list under the operator. Step to
        // cue 2 first, then press On and watch cue 2 stay.
        press(&mut core, 2, true);
        until("Go+ to reach cue 2", || channel(&frames, 5) == Some(78));
        press(&mut core, 0, true);
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(
            channel(&frames, 5),
            Some(78),
            "a second On restarted the list"
        );

        // `Go-` — back to cue 1.
        press(&mut core, 3, true);
        until("Go- to reach cue 1", || channel(&frames, 5) == Some(255));

        // `Off` — the list stops and the light goes with it.
        press(&mut core, 1, true);
        until("Off to stop the list", || channel(&frames, 5) == Some(0));

        // A release of any of these four is not a second press.
        press(&mut core, 0, true);
        until("On again", || channel(&frames, 5) == Some(255));
        let quiet = press(&mut core, 0, false);
        assert!(quiet.is_empty(), "a release said something: {quiet:?}");
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(channel(&frames, 5), Some(255));

        driver.stop();
    }

    /// The other four functions, on the same rig and the same frames.
    #[test]
    fn flash_toggle_learn_speed_and_empty_do_what_their_names_say_on_the_wire() {
        use prism_domain::ExecutorButtonFunction as Fn;
        let dir = tempfile::tempdir().unwrap();
        // Rec = Flash, Solo = Toggle, Mute = LearnSpeed, Select = Empty.
        let (mut core, frames, driver) = desk_for_buttons(
            dir.path(),
            vec![Fn::Flash, Fn::Toggle, Fn::LearnSpeed, Fn::Empty],
            prism_domain::ExecutorFaderFunction::Master,
        );
        until("the rig at home", || channel(&frames, 1) == Some(255));

        // `Flash` — held, the list runs at full; released, it stops.
        press(&mut core, 0, true);
        until("the flash", || channel(&frames, 5) == Some(255));
        press(&mut core, 0, false);
        until("the release", || channel(&frames, 5) == Some(0));

        // `Toggle` — on, then off, from the same button.
        press(&mut core, 1, true);
        until("the toggle on", || channel(&frames, 5) == Some(255));
        core.poll_playback();
        press(&mut core, 1, true);
        until("the toggle off", || channel(&frames, 5) == Some(0));

        // `LearnSpeed` — a tap is accepted and changes no value. Two of them
        // change the rate, which `prism_engine`'s own tests measure; here the
        // claim is that it reaches the engine and moves nothing.
        press(&mut core, 2, true);
        press(&mut core, 2, true);
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(channel(&frames, 5), Some(0));
        assert_eq!(channel(&frames, 1), Some(255));

        // `Empty` — a key with nothing on it. Not a refusal: the executor says
        // so, and nothing is broadcast.
        let nothing = press(&mut core, 3, true);
        assert!(nothing.is_empty(), "{nothing:?}");
        // And a position the executor has no button for at all is the same.
        let beyond = press(&mut core, 9, true);
        assert!(beyond.is_empty(), "{beyond:?}");

        driver.stop();
    }

    /// **Exit criterion.** A `Flash` pressed and released leaves the stored
    /// master byte-identical, and one held across a `SetExecutorMaster` does not
    /// lose the new value.
    ///
    /// The stored master is read out of the **show**, which is what a client's
    /// snapshot carries and what a reload would restore; the light is read off
    /// the frames. Two claims, because a flash that wrote into the master would
    /// satisfy the second and fail the first.
    #[test]
    fn a_flash_leaves_the_stored_master_alone_and_does_not_swallow_a_fader_move() {
        use prism_domain::ExecutorButtonFunction as Fn;
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk_for_buttons(
            dir.path(),
            vec![Fn::Flash, Fn::On, Fn::Off, Fn::Empty],
            prism_domain::ExecutorFaderFunction::Master,
        );
        until("the rig at home", || channel(&frames, 1) == Some(255));

        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(3),
            level: 16_383,
        })
        .unwrap();
        press(&mut core, 1, true);
        until("a quarter of the cue", || channel(&frames, 5) == Some(63));
        let stored = core.file.show.executor(ExecutorId::new(3)).unwrap().clone();

        press(&mut core, 0, true);
        until("the flash", || channel(&frames, 5) == Some(255));
        assert_eq!(
            core.file.show.executor(ExecutorId::new(3)),
            Some(&stored),
            "the flash changed the executor the show holds"
        );

        // The fader moves while the flash is held. The light does not — the
        // flash is on top — and the new level is what the release restores.
        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(3),
            level: 49_151,
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(channel(&frames, 5), Some(255), "the flash lost its grip");

        press(&mut core, 0, false);
        until("the level that arrived during the flash", || {
            channel(&frames, 5) == Some(191)
        });
        assert_eq!(
            core.file
                .show
                .executor(ExecutorId::new(3))
                .unwrap()
                .master_level,
            49_151
        );

        driver.stop();
    }

    /// **Exit criterion.** `Toggle` on an executor a *second client* has just
    /// started stops it — one desk, one answer.
    ///
    /// The second client is modelled the way the protocol makes it real: a
    /// separate `Command::ExecutorGo`, applied through the same `Core`, which is
    /// exactly what a second WebSocket connection produces. What is being
    /// checked is that the toggle consults `is_active` **on the daemon** rather
    /// than a client's own idea of it.
    #[test]
    fn a_toggle_stops_an_executor_a_second_client_started() {
        use prism_domain::ExecutorButtonFunction as Fn;
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk_for_buttons(
            dir.path(),
            vec![Fn::Toggle, Fn::Empty, Fn::Empty, Fn::Empty],
            prism_domain::ExecutorFaderFunction::Master,
        );
        until("the rig at home", || channel(&frames, 1) == Some(255));

        // The other client starts it. Nobody pressed the toggle.
        core.apply(&Command::ExecutorGo {
            executor_id: ExecutorId::new(3),
            direction: GoDirection::Next,
        })
        .unwrap();
        until("the other client's Go", || channel(&frames, 5) == Some(255));
        // The readback is what tells the desk it is running, and a toggle is
        // resolved against that. In the daemon this poll runs every 25 ms; here
        // it is called by hand, and the wait is the honest statement of what a
        // toggle depends on.
        until("the desk to hear that it is running", || {
            core.poll_playback();
            core.file
                .show
                .executor(ExecutorId::new(3))
                .is_some_and(|executor| executor.is_active)
        });

        // The first press of the toggle therefore **stops** it. A client that
        // resolved `Toggle` for itself would have sent a start, because it had
        // never pressed anything.
        press(&mut core, 0, true);
        until("the toggle to stop it", || channel(&frames, 5) == Some(0));

        driver.stop();
    }

    /// **Exit criterion.** `currentCueIndex` is filled while a sequence runs,
    /// and it comes back from the tick rather than from a guess.
    #[test]
    fn the_cue_index_comes_back_from_the_tick_and_goes_away_again() {
        use prism_domain::ExecutorButtonFunction as Fn;
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk_for_buttons(
            dir.path(),
            vec![Fn::On, Fn::Off, Fn::GoForward, Fn::Empty],
            prism_domain::ExecutorFaderFunction::Master,
        );
        until("the rig at home", || channel(&frames, 1) == Some(255));
        assert_eq!(
            core.file
                .show
                .executor(ExecutorId::new(3))
                .unwrap()
                .current_cue_index,
            None,
            "a stopped executor is on no cue"
        );

        press(&mut core, 0, true);
        until("the cue index", || {
            !core.poll_playback().is_empty()
                || core
                    .file
                    .show
                    .executor(ExecutorId::new(3))
                    .is_some_and(|executor| executor.current_cue_index == Some(0))
        });
        assert_eq!(
            core.file
                .show
                .executor(ExecutorId::new(3))
                .unwrap()
                .current_cue_index,
            Some(0)
        );

        press(&mut core, 2, true);
        until("the second cue", || {
            core.poll_playback();
            core.file
                .show
                .executor(ExecutorId::new(3))
                .is_some_and(|executor| executor.current_cue_index == Some(1))
        });

        press(&mut core, 1, true);
        until("the cue index to go away", || {
            core.poll_playback();
            core.file
                .show
                .executor(ExecutorId::new(3))
                .is_some_and(|executor| executor.current_cue_index.is_none())
        });

        driver.stop();
    }

    /// The readback is **silent when nothing moved**, which is what keeps the
    /// show document still during a fade.
    ///
    /// S28 left the warning: `Query::StorePreview` is asked once per delta, so a
    /// `Delta::ExecutorState` per tick would have become a question per frame.
    #[test]
    fn the_readback_says_nothing_while_a_fade_runs() {
        use crate::testkit::{cue, executor, sequence};
        let dir = tempfile::tempdir().unwrap();
        let mut file = show_file();
        // A twenty-second fade, so the whole of this test happens inside one
        // cue and the *only* thing that could produce a delta is a value moving.
        let mut slow = cue("1", 4, AttributeType::Dimmer, 65_535);
        slow.fade_in = 20.0;
        file.show.store_sequence(sequence(7, vec![slow])).unwrap();
        file.show.store_executor(executor(3, Some(7))).unwrap();
        file.show.mark_saved();
        let (mut core, frames, driver) = desk_with(dir.path(), file);
        until("the rig at home", || channel(&frames, 1) == Some(255));

        core.apply(&Command::ExecutorGo {
            executor_id: ExecutorId::new(3),
            direction: GoDirection::Next,
        })
        .unwrap();
        until("the cue index to arrive", || {
            !core.poll_playback().is_empty()
                || core
                    .file
                    .show
                    .executor(ExecutorId::new(3))
                    .is_some_and(|executor| executor.current_cue_index == Some(0))
        });

        // Now the fade runs for a while, and the readback has nothing to say
        // about it at all.
        let started = Instant::now();
        let mut said = Vec::new();
        while started.elapsed() < Duration::from_millis(300) {
            said.extend(core.poll_playback());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            said.is_empty(),
            "the readback spoke {} times during one fade: {said:?}",
            said.len()
        );
        // And the fade really was running, so this is not a measurement of a
        // rig at rest.
        let level = channel(&frames, 5).unwrap_or(0);
        assert!(level > 0 && level < 255, "the fade was not moving: {level}");

        driver.stop();
    }

    /// **The fader is the executor's, not the protocol's.** One
    /// `SetExecutorMaster` and four meanings, chosen by `faderFunction`.
    #[test]
    fn what_the_fader_does_is_the_executors_own_setting() {
        use prism_domain::ExecutorButtonFunction as Fn;
        use prism_domain::ExecutorFaderFunction as Fader;
        let dir = tempfile::tempdir().unwrap();

        // `Speed`: the show's `speed` moves and the master does not.
        {
            let (mut core, _frames, driver) = desk_for_buttons(
                dir.path(),
                vec![Fn::On, Fn::Empty, Fn::Empty, Fn::Empty],
                Fader::Speed,
            );
            core.apply(&Command::SetExecutorMaster {
                executor_id: ExecutorId::new(3),
                level: 2_048,
            })
            .unwrap();
            let executor = core.file.show.executor(ExecutorId::new(3)).unwrap();
            assert_eq!(executor.speed, 2_048);
            assert_eq!(executor.master_level, u16::MAX, "the master moved");
            driver.stop();
        }

        // `XFade`: no show state at all — a crossfade in progress is a gesture,
        // and a show file that remembered one would reload holding half a cue.
        {
            let dir = tempfile::tempdir().unwrap();
            let (mut core, _frames, driver) = desk_for_buttons(
                dir.path(),
                vec![Fn::On, Fn::Empty, Fn::Empty, Fn::Empty],
                Fader::XFade,
            );
            let before = core.file.show.executor(ExecutorId::new(3)).unwrap().clone();
            let deltas = core
                .apply(&Command::SetExecutorMaster {
                    executor_id: ExecutorId::new(3),
                    level: 30_000,
                })
                .unwrap();
            assert!(deltas.is_empty(), "{deltas:?}");
            assert_eq!(core.file.show.executor(ExecutorId::new(3)), Some(&before));
            driver.stop();
        }

        // `Empty`: a fader with nothing on it. Accepted and ignored, because the
        // executor says so — not refused, which would put a message on a screen
        // for a fader an operator can see is dead.
        {
            let dir = tempfile::tempdir().unwrap();
            let (mut core, _frames, driver) = desk_for_buttons(
                dir.path(),
                vec![Fn::On, Fn::Empty, Fn::Empty, Fn::Empty],
                Fader::Empty,
            );
            let before = core.file.show.executor(ExecutorId::new(3)).unwrap().clone();
            assert!(
                core.apply(&Command::SetExecutorMaster {
                    executor_id: ExecutorId::new(3),
                    level: 1,
                })
                .unwrap()
                .is_empty()
            );
            assert_eq!(core.file.show.executor(ExecutorId::new(3)), Some(&before));
            driver.stop();
        }
    }
}
