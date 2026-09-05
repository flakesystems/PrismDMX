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

use prism_core::{
    Applied, Autosave, Effect, MachineConfig, MachineError, ShowFile, ShowFileError, ShowStore,
};
use prism_domain::{
    AttributeType, Command, Delta, FixtureId, GroupId, NoticeLevel, PlaybackId, ProgrammerState,
    UniverseId,
};
use prism_engine::{FrameLayout, MergeBody, MergePlan, PatchError, PlaybackReport, TickCommand};

use crate::engine::EngineThread;
use crate::log;
use crate::outputs::{Machine, OutputSupervisor};

/// Why a command did not happen.
#[derive(Debug)]
pub enum CoreError {
    /// One of the three models refused it, and nothing changed (§5).
    Refused(ShowFileError),
    /// The **machine** refused it — S33's fourth applier, and a refusal about
    /// the building rather than about the show. Kept apart from
    /// [`Self::Refused`] so an operator is not told *the show refused it* for a
    /// mistyped hop limit.
    Machine(MachineError),
    /// It was applied and the disk would not take it.
    Store(prism_core::StoreError),
}

impl core::fmt::Display for CoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Refused(error) => error.fmt(f),
            Self::Machine(error) => error.fmt(f),
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
    /// **This machine**, as opposed to what it is playing: the desk identity,
    /// the output patch, where it is written and the driver threads keeping step
    /// with it (S33). Held here rather than beside the show because it is what
    /// S33's four commands edit and what has to be written back when they do —
    /// and never inside a `.prism` file, which is the whole of
    /// `prism_core::outputs`' argument.
    machine: Machine,
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
    reported: BTreeMap<PlaybackId, (bool, Option<u32>)>,
    autosave: Autosave,
    /// The patch revision the current plan was built from (S11).
    patch_revision: u64,
    /// A surface port change nobody has acted on yet — S36.
    ///
    /// The port is the **daemon's**, not the core's: `Daemon` owns the
    /// `SurfaceLink` because a port has a thread's worth of state and a cable
    /// that can come out, and `Core` is what a client's command reaches. So a
    /// `SetSurfacePort` leaves the new name here and the run loop picks it up on
    /// its next surface tick — which is at most a millisecond later
    /// (`surface::SURFACE_PERIOD`).
    ///
    /// `Some(None)` and `None` are different things and both are needed:
    /// *change it to no surface at all*, and *nothing to do*.
    surface_change: Option<Option<String>>,
    /// Whether a recovery copy is standing beside the show — S37.
    ///
    /// Tracked here rather than asked of the file system, because a settings
    /// panel wants it and the alternative is a `stat` twice a second for a fact
    /// that changes when the autosave writes one or a save removes one. Both of
    /// those go through this type, so both can say so.
    recovery: bool,
    /// A setting change nobody has acted on yet — S37.
    ///
    /// [`Self::surface_change`]'s shape and its reason: the *binding profile* is
    /// the daemon's, because `Daemon` owns the `SurfaceLink` and the table it is
    /// drawing with. `Some(None)` is *go back to the built-in table*.
    profile_change: Option<Option<std::path::PathBuf>>,
    /// A new exit action nobody has acted on yet — S37, and the same shape one
    /// field along: what the stage does when the daemon stops is `Daemon`'s.
    exit_change: Option<prism_domain::ExitAction>,
    /// The binding table **in force** — S38.
    ///
    /// It lives here rather than in `Daemon`, and moving it was S38's first
    /// structural decision. A table read once at start-up could live wherever
    /// the port did; a table a **command** edits has to live where a command
    /// arrives, which is this type. `Daemon` still owns the `SurfaceLink` and
    /// still gets the table handed to it — see [`Self::take_binding_change`] —
    /// but it is no longer the thing that holds it, so two clients editing
    /// cannot produce two tables.
    bindings: prism_surface::Bindings,
    /// How many times the table has moved since this daemon started — S38.
    ///
    /// The change token `Delta::SurfaceBindingsChanged` carries and
    /// `Answer::SurfaceBindings` echoes. Not persisted: it answers *has the
    /// table moved under me*, which is a question about this run.
    binding_revision: u32,
    /// Whether learn is armed — S38.
    ///
    /// One desk, one learn, and it is **not** written down anywhere: a desk that
    /// restarted into learn mode would be a desk whose keys do nothing. One
    /// shot, so the first control the surface reports clears it.
    learning: bool,
    /// A binding table the surface has not been given yet — S38.
    ///
    /// [`Self::profile_change`]'s shape and its reason: the `SurfaceLink` that
    /// draws with it is `Daemon`'s. `None` is *nothing to do*.
    binding_change: Option<prism_surface::Bindings>,
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
        machine: Machine,
        store: ShowStore,
        engine: EngineThread,
        layout: Arc<FrameLayout>,
        report: Arc<PlaybackReport>,
        bindings: prism_surface::Bindings,
    ) -> Result<Self, PatchError> {
        let plan = MergePlan::build(file.show.patched().map(|(fixture, fixture_type)| {
            // **The patch's own answer** — S43: a fixture whose profile has no
            // intensity gets one from the desk unless the operator switched it
            // off in the patch window.
            (
                fixture.id,
                fixture_type,
                fixture.has_software_dimmer(fixture_type),
            )
        }))
        .map_err(PatchError::Plan)?;
        let patch_revision = file.show.patch_revision();
        let engine_programmer = file.programmer.state().clone();
        Ok(Self {
            file,
            machine,
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
            surface_change: None,
            recovery: false,
            profile_change: None,
            exit_change: None,
            bindings,
            binding_revision: 0,
            learning: false,
            binding_change: None,
        })
    }

    /// The show file on disk.
    #[must_use]
    pub const fn store(&self) -> &ShowStore {
        &self.store
    }

    /// This machine: the desk identity and the rig — S33.
    #[must_use]
    pub const fn machine(&self) -> &MachineConfig {
        &self.machine.config
    }

    /// The driver threads.
    #[must_use]
    pub const fn outputs(&self) -> &OutputSupervisor {
        &self.machine.outputs
    }

    /// The universes this show patches that the rig does not carry — S33.
    ///
    /// Reported rather than refused (`prism_core::ShowIssue::UniverseNotOutput`):
    /// an operator whose universe 7 goes nowhere has to be able to read that
    /// before the show rather than discover it when the light does not come up.
    #[must_use]
    pub fn dark_universes(&self) -> Vec<prism_core::ShowIssue> {
        // The **running** rig rather than the configured one: a row that was
        // refused, or whose thread would not start, is configuration that is
        // not carrying anything — and *where does the light actually go* is the
        // question this answers.
        prism_core::dark_universes(&self.file.show, &self.machine.outputs.carried_universes())
    }

    /// Stops every driver thread. The shutdown path.
    pub fn stop_outputs(&mut self) {
        self.machine.outputs.stop_all();
    }

    /// Gives this machine an Art-Net discovery — S46.
    ///
    /// The daemon gives the supervisor its own before this `Core` exists
    /// (`prismd::daemon`), because the rig is reconciled on the way up and the
    /// poll targets come out of that same rig. This is how a **test** gives one
    /// to a daemon that is already running, which is what lets punch-list B6 be
    /// asserted over the protocol — `Query::OutputStatus` on a configured node
    /// that never answers — rather than only against a supervisor.
    pub fn adopt_discovery(&mut self, discovery: crate::discovery::Discovery) {
        self.machine.outputs.adopt_discovery(discovery);
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
        // **Three appliers, routed on two predicates** (S33). The rig belongs to
        // the building rather than to the show, so it is neither
        // `ShowFile::apply`'s nor a second match over the command list here —
        // `Command::is_machine_command` is the door, exactly as
        // `is_session_command` is one level down.
        let applied = if command.is_machine_command() {
            if self.machine.path.is_none() {
                return Err(CoreError::Machine(MachineError::ConfiguredOnTheCommandLine));
            }
            // S36's flag, and it is asked separately because it is a separate
            // fact: a daemon may take its rig from `machine.json` and its
            // surface from `--surface` at the same time, and an operator told
            // the wrong flag would go looking in the wrong place.
            if matches!(command, Command::SetSurfacePort { .. })
                && self.machine.surface_on_command_line
            {
                return Err(CoreError::Machine(MachineError::SurfaceOnTheCommandLine));
            }
            // S38's, and it is a third question for S36's reason: the *table* and
            // the *port* are named by two different flags, a daemon may be given
            // one and not the other, and an operator told the wrong one would go
            // looking in the wrong place. Learn is deliberately **not** under
            // it, because it writes nothing down: a command line holding the
            // table has nothing to say about arming it.
            if matches!(
                command,
                Command::ConfigureMachine {
                    change: prism_domain::MachineChange::SurfaceBinding { .. }
                }
            ) && self.machine.profile_on_command_line
            {
                return Err(CoreError::Machine(MachineError::BindingsOnTheCommandLine));
            }
            self.machine
                .config
                .apply(command)
                .map_err(CoreError::Machine)?
        } else {
            self.file.apply(command).map_err(CoreError::Refused)?
        };
        self.carry_out(applied)
    }

    /// Brings the driver threads into line with the rig and writes it down.
    ///
    /// Answered here rather than in `prism-core` for [`Effect::Save`]'s reason,
    /// one level along: a cable has a thread, a socket and a failure mode, and
    /// the model that decides what a rig *is* holds none of the three.
    ///
    /// A configuration that cannot be written is a **notice, not a refusal**.
    /// The change has already happened — the threads are running, the light is
    /// on the stage — and taking it back because a disk is full would be
    /// undoing something an operator can see working. What they need instead is
    /// to be told it will not survive a restart.
    fn carry_out_outputs(&mut self) -> Vec<Delta> {
        let Machine {
            config,
            path,
            outputs,
            ..
        } = &mut self.machine;
        let mut deltas = outputs.reconcile(config.outputs());
        let Some(path) = path else {
            return deltas;
        };
        if let Err(error) = crate::machine::write(path, config) {
            log::error(
                "output",
                &format!("the output patch could not be written: {error}"),
            );
            deltas.push(Delta::Notice {
                level: NoticeLevel::Warn,
                message: format!(
                    "the outputs were changed but could not be saved to {}: {error}",
                    path.display()
                ),
            });
        }
        // A universe that now goes nowhere is said out loud, once, when the rig
        // changes — `ShowIssue::UniverseNotOutput` is the same fact as data.
        for issue in self.dark_universes() {
            log::warn("output", &issue.to_string());
            deltas.push(Delta::Notice {
                level: NoticeLevel::Warn,
                message: issue.to_string(),
            });
        }
        deltas
    }

    /// Leaves the new port for the run loop and writes the configuration down
    /// — S36.
    ///
    /// [`Self::carry_out_outputs`] for the other device this machine owns, and
    /// the same two rules: the model that decided the *name* does not open
    /// anything, and a configuration that cannot be written is a **notice, not a
    /// refusal** — the operator has already chosen, and what they need to be
    /// told is that the choice will not survive a restart.
    ///
    /// Opening the port is deliberately **not** done here. `Daemon` owns the
    /// `SurfaceLink`; this leaves the name behind and the next surface tick
    /// takes it, at most a millisecond later.
    fn carry_out_surface(&mut self) -> Vec<Delta> {
        self.surface_change = Some(self.machine.config.surface_port().map(str::to_owned));
        let Some(path) = &self.machine.path else {
            return Vec::new();
        };
        let Err(error) = crate::machine::write(path, &self.machine.config) else {
            return Vec::new();
        };
        log::error(
            "surface",
            &format!("the surface port could not be written: {error}"),
        );
        vec![Delta::Notice {
            level: NoticeLevel::Warn,
            message: format!(
                "the control surface was changed but could not be saved to {}: {error}",
                path.display()
            ),
        }]
    }

    /// The port a `SetSurfacePort` asked for, once — S36.
    ///
    /// Taken rather than read, so the run loop opens a port on the poll after
    /// the command and not on every poll after it.
    pub fn take_surface_change(&mut self) -> Option<Option<String>> {
        self.surface_change.take()
    }

    /// The MIDI port this machine's control surface is configured on — S36.
    #[must_use]
    pub fn surface_port(&self) -> Option<&str> {
        self.machine.config.surface_port()
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
            match effect {
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
                    executor: *executor,
                    direction: *direction,
                }),
                // S40's `Goto`. The cue number became an index in
                // `Show::cue_index_of`, because the tick resolves nothing.
                Effect::Goto {
                    executor,
                    cue_index,
                } => self.send(TickCommand::GotoCue {
                    executor: *executor,
                    cue_index: *cue_index,
                }),
                Effect::ExecutorOff { executor } => self.send(TickCommand::SetExecutorActive {
                    executor: *executor,
                    on: false,
                }),
                Effect::ExecutorOn { executor } => self.send(TickCommand::SetExecutorActive {
                    executor: *executor,
                    on: true,
                }),
                // A flash does **not** touch the stored master and does not
                // record `is_active` either: it is a momentary gesture, and the
                // readback is what tells the desk the strip is lit. Writing
                // playback state here as well would be two authors for one
                // field, one of them a guess.
                Effect::ExecutorFlash { executor, on } => {
                    self.send(TickCommand::SetExecutorFlash {
                        executor: *executor,
                        on: *on,
                    });
                }
                Effect::ExecutorSpeed { executor, speed } => {
                    self.send(TickCommand::SetExecutorSpeed {
                        executor: *executor,
                        speed: *speed,
                    });
                }
                Effect::ExecutorTapSpeed { executor } => {
                    self.send(TickCommand::TapExecutorSpeed {
                        executor: *executor,
                    });
                }
                Effect::ExecutorCrossfade {
                    executor,
                    mode,
                    position,
                } => {
                    self.send(TickCommand::SetExecutorCrossfade {
                        executor: *executor,
                        mode: *mode,
                        position: *position,
                    });
                }
                Effect::SetExecutorMaster { executor, level } => {
                    self.send(TickCommand::SetExecutorLevel {
                        executor: *executor,
                        level: *level,
                    });
                }
                // Carried out by `ShowFile::apply` and never handed on (S13,
                // S14, and S44's `EmbedProfile`, which needs the desk's
                // library and gets it there). Named rather than caught by a
                // wildcard, so an effect added later is a compile error here.
                Effect::Programmer | Effect::Undo | Effect::Redo | Effect::EmbedProfile => {}
                // S33: the rig changed, so the driver threads have to catch up
                // and the machine configuration has to be written down.
                Effect::Outputs => deltas.extend(self.carry_out_outputs()),
                // S36: the surface is on a different port, so the daemon has to
                // put the old one down and pick the new one up — and the
                // machine configuration has to be written down, exactly as a
                // rig change is.
                Effect::Surface => deltas.extend(self.carry_out_surface()),
                Effect::Save => deltas.extend(self.save()?),
                // S37's five file commands and its three machine effects. Each
                // is named rather than caught by a wildcard, for the reason the
                // rest of this match is: an effect added later is a compile
                // error here.
                Effect::SaveShowAs(path) => deltas.extend(self.save_show_as(path.clone())?),
                Effect::OpenShow(path) => deltas.extend(self.open_show(path.clone())?),
                Effect::NewShow(path) => deltas.extend(self.new_show(path.clone())?),
                Effect::ExportShow(path) => deltas.extend(self.export_show(path)?),
                Effect::ImportShow(path) => deltas.extend(self.import_show(path.clone())?),
                Effect::NewDeskIdentity => deltas.extend(self.new_desk_identity()),
                Effect::NewToken => deltas.extend(self.new_token()),
                Effect::Machine => deltas.extend(self.carry_out_machine()),
                // S38's two. The first is the half `prism-core` could not do -
                // it needs `docs/MCU_MAPPING.md` section 4.1's built-in table,
                // which lives in a MIDI codec the show model may not depend on -
                // and the second is a mode that is deliberately written down
                // nowhere.
                Effect::SurfaceBinding { control, action } => {
                    deltas.extend(self.carry_out_binding(*control, action.clone()));
                }
                Effect::SurfaceLearn(on) => deltas.push(self.set_learning(*on)),
                // **S45's custom row, fired.** A key with a line on it is a
                // line, so it travels the way every other line does: the same
                // `CommandLineInput { run: true }` a keyboard's Enter and a
                // bound X-Touch key send, read and carried out by
                // `prism_core::ShowFile` (S49). Before S49 this wrote the line
                // and bumped a counter for a client to notice, which is the
                // stop-gap that session removed.
                Effect::CommandLine(line) => {
                    deltas.extend(self.apply(&Command::CommandLineInput {
                        text: line.clone(),
                        run: true,
                        mode: None,
                    })?);
                }
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

    // -- the show file, and this machine (S37) --------------------------------

    /// The `.prism` file this daemon has open, the ones before it and what the
    /// autosave is doing — S37.
    #[must_use]
    pub fn show_file_info(&self) -> prism_domain::ShowFileInfo {
        let path = self.store.path().display().to_string();
        prism_domain::ShowFileInfo {
            recent: self.machine.config.shows().without(&path),
            path,
            unsaved_changes: self.file.is_dirty(),
            recovery: self.recovery,
            autosave_seconds: u32::try_from(Autosave::INTERVAL.as_secs()).unwrap_or(u32::MAX),
        }
    }

    /// What this machine is set to, and what this run is actually doing — S37.
    ///
    /// Half of it is the configuration and half of it is the run: where the data
    /// directory is, what is really listening, and which settings a command line
    /// is holding. That is why the delta is built here rather than in
    /// `prism_core::MachineConfig::apply`.
    #[must_use]
    pub fn machine_settings(&self) -> prism_domain::MachineSettings {
        let settings = self.machine.config.settings();
        prism_domain::MachineSettings {
            desk_id: self.machine.config.desk_id().to_string(),
            data_dir: self.machine.data_dir.display().to_string(),
            local: settings.local,
            websocket: settings.websocket,
            websocket_open: self.machine.websocket_open,
            token: settings.token.clone(),
            log_level: settings.log_level,
            universes: settings.universes,
            exit_action: settings.exit_action,
            autostart: settings.autostart,
            fixture_library: settings.fixture_library.clone(),
            surface_profile: settings.surface_profile.clone(),
            overrides: self.machine.overrides.clone(),
        }
    }

    /// Records where the WebSocket listener actually bound — S37.
    ///
    /// Written by `Daemon::start` and by nobody else, and only when the bind
    /// succeeded: a listener that could not bind leaves this `None`, which is
    /// what makes *configured here, listening nowhere* a state a panel can draw.
    pub const fn set_websocket_open(&mut self, address: Option<std::net::SocketAddr>) {
        self.machine.websocket_open = address;
    }

    /// Writes down which show this desk has open, so it starts here next time —
    /// S37.
    pub fn remember_show(&mut self) {
        let path = self.store.path().display().to_string();
        self.machine.config.remember_show(&path);
        drop(self.write_machine());
    }

    /// The binding profile a `ConfigureMachine` asked for, once — S37.
    ///
    /// `Core::take_surface_change`'s shape: the table belongs to `Daemon`, which
    /// owns the `SurfaceLink` drawing with it.
    pub fn take_profile_change(&mut self) -> Option<Option<std::path::PathBuf>> {
        self.profile_change.take()
    }

    /// The exit action a `ConfigureMachine` asked for, once — S37.
    pub fn take_exit_change(&mut self) -> Option<prism_domain::ExitAction> {
        self.exit_change.take()
    }

    /// Answers `Effect::Machine`: writes the configuration down and says what
    /// this machine is now — S37.
    ///
    /// A configuration that cannot be written is a **notice, not a refusal**,
    /// which is `carry_out_outputs`' rule and its reason: the change has already
    /// happened, and what an operator needs to be told is that it will not
    /// survive a restart.
    fn carry_out_machine(&mut self) -> Vec<Delta> {
        // Two of the settings are the daemon's to act on rather than merely to
        // store, and both are left for the run loop for `surface_change`'s
        // reason: the thing they change belongs to `Daemon`.
        self.profile_change = Some(
            self.machine
                .config
                .settings()
                .surface_profile
                .as_deref()
                .map(std::path::PathBuf::from),
        );
        self.exit_change = Some(self.machine.config.settings().exit_action);
        // …and one takes effect on the spot, because a log level is a switch.
        log::set_level(log::Level::from(self.machine.config.settings().log_level));

        let mut deltas = vec![Delta::MachineChanged {
            settings: self.machine_settings(),
        }];
        deltas.extend(self.write_machine());
        deltas
    }

    /// The binding table in force - S38.
    #[must_use]
    pub const fn bindings(&self) -> &prism_surface::Bindings {
        &self.bindings
    }

    /// How many times the table has moved since this daemon started - S38.
    #[must_use]
    pub const fn binding_revision(&self) -> u32 {
        self.binding_revision
    }

    /// Whether learn is armed - S38.
    #[must_use]
    pub const fn is_learning(&self) -> bool {
        self.learning
    }

    /// A table the surface has not been given yet, once - S38.
    ///
    /// [`Self::take_profile_change`]'s shape: the `SurfaceLink` drawing with it
    /// is `Daemon`'s, because a port has a thread's worth of state and a cable
    /// that can come out.
    pub const fn take_binding_change(&mut self) -> Option<prism_surface::Bindings> {
        self.binding_change.take()
    }

    /// Arms or disarms learn, and says so - S38.
    fn set_learning(&mut self, learning: bool) -> Delta {
        self.learning = learning;
        log::debug(
            "surface",
            if learning {
                "learn is armed: the next control is named rather than obeyed"
            } else {
                "learn is off"
            },
        );
        Delta::SurfaceLearnChanged {
            learning,
            control: None,
        }
    }

    /// Names the control an operator has just touched, and disarms - S38.
    ///
    /// **One shot**, which is what stops a client that went away mid-learn
    /// leaving a desk whose keys do nothing. Called by the surface poll rather
    /// than by a command, because the whole point is that it comes from the desk.
    pub fn learned(&mut self, control: prism_domain::BoundControl) -> Vec<Delta> {
        if !self.learning {
            return Vec::new();
        }
        self.learning = false;
        log::info("surface", &format!("learn named {control}"));
        vec![Delta::SurfaceLearnChanged {
            learning: false,
            control: Some(control),
        }]
    }

    /// Answers `Effect::SurfaceBinding`: puts the row on the table in force,
    /// writes the whole table down, and hands it to the surface - S38.
    ///
    /// The refusal has already happened (`prism_core::MachineConfig::configure`
    /// refuses the reserved control before anything is written), so a failure
    /// here would be a table and a validator that disagree. It is still not a
    /// panic - `CLAUDE.md`'s zero-crash invariant does not make exceptions for
    /// lines that cannot happen - it is a notice, and the table is left where it
    /// was.
    fn carry_out_binding(
        &mut self,
        control: prism_domain::BoundControl,
        action: Option<prism_domain::SurfaceAction>,
    ) -> Vec<Delta> {
        if let Err(error) = self.bindings.bind(control, action, &prism_surface::X_TOUCH) {
            log::warn("surface", &error.to_string());
            return vec![Delta::Notice {
                level: NoticeLevel::Warn,
                message: error.to_string(),
            }];
        }
        self.record_bindings()
    }

    /// Replaces the whole table - S38, and this is what reading a profile does.
    ///
    /// **A file is an import rather than a live source.** Naming a profile reads
    /// it into this machine's own configuration; from then on that is the table
    /// and the path is only the record of where it came from. The alternative -
    /// the file winning at every start - would mean an operator who rebound a key
    /// at the desk found it back the way it was the next morning.
    ///
    /// A table that could not be read leaves the one in force **exactly where it
    /// was**, which is S22's rule met from a new direction: S22 said a malformed
    /// profile falls back to the built-in defaults, and it said so about a desk
    /// starting up with nothing else to fall back on. A desk that has a table of
    /// its own has something better to fall back on than the defaults, and
    /// overwriting an operator's edits because of a typo in a file would be the
    /// one outcome worse than ignoring the file.
    pub fn replace_bindings(&mut self, bindings: prism_surface::Bindings) -> Vec<Delta> {
        self.bindings = bindings;
        self.record_bindings()
    }

    /// Writes down the table this daemon **started** with, changing nothing —
    /// S38.
    ///
    /// [`Self::replace_bindings`] without the two things that make it a
    /// *change*: no revision, and **no hand-off to the surface**. A desk that
    /// has never been told adopts what it started with so that the first edit is
    /// a change to a table rather than the creation of one — but nothing has
    /// moved, so a client has nothing to be told and the surface has nothing to
    /// redraw.
    ///
    /// Handing it over would be worse than pointless. Swapping a table rebuilds
    /// the `SurfaceLink` round its port, which throws the shadow model away on
    /// purpose — and with it the feedback counters a settings panel draws.
    /// `surface_gate.rs::pressing_smpte_beats_does_nothing_at_all_and_is_counted`
    /// is the test that noticed: it read a `reserved` count of 0 where it had
    /// pressed the button twice, because the housekeeping tick had rebuilt the
    /// surface underneath it.
    pub fn adopt_bindings(&mut self, bindings: prism_surface::Bindings) -> Vec<Delta> {
        self.bindings = bindings;
        self.machine
            .config
            .set_surface_bindings(self.bindings.rows());
        self.write_machine()
    }

    /// Writes the table down, counts the change and hands it to the surface.
    fn record_bindings(&mut self) -> Vec<Delta> {
        self.machine
            .config
            .set_surface_bindings(self.bindings.rows());
        self.binding_revision = self.binding_revision.saturating_add(1);
        // The surface is `Daemon`'s, so the new table is left here for the run
        // loop exactly as a profile change is.
        self.binding_change = Some(self.bindings.clone());
        let mut deltas = vec![Delta::SurfaceBindingsChanged {
            revision: self.binding_revision,
        }];
        deltas.extend(self.write_machine());
        deltas
    }

    /// Writes `machine.json`, or says why it could not.
    fn write_machine(&mut self) -> Vec<Delta> {
        let Some(path) = &self.machine.path else {
            return Vec::new();
        };
        let Err(error) = crate::machine::write(path, &self.machine.config) else {
            return Vec::new();
        };
        log::error(
            "machine",
            &format!("the settings could not be written: {error}"),
        );
        vec![Delta::Notice {
            level: NoticeLevel::Warn,
            message: format!(
                "the settings were changed but could not be saved to {}: {error}",
                path.display()
            ),
        }]
    }

    /// Gives this desk a new sACN identity — S37's `MachineChange::NewIdentity`.
    ///
    /// **It takes effect at the next start**, and that is the decision rather
    /// than a limitation: the outputs were built with the old CID, and an sACN
    /// source that changed its identity mid-show would be a *new* source
    /// fighting the old one until its 2.5 s network-data-loss timeout expires
    /// (`prism_core::desk`). So the number is written down and the operator is
    /// told, which is what `MachineChange::needs_restart` says of it.
    fn new_desk_identity(&mut self) -> Vec<Delta> {
        match crate::machine::generate_desk_id() {
            Ok(id) => {
                self.machine.config.set_desk_id(id);
                log::info("desk", &format!("this desk has a new identity: {id}"));
                vec![Delta::Notice {
                    level: NoticeLevel::Info,
                    message: format!(
                        "this desk's identity is now {id}. The sACN outputs keep the old one until the daemon is restarted"
                    ),
                }]
            }
            // A machine with no entropy is a **notice rather than a refusal**:
            // the rest of the command has been applied, and refusing here would
            // leave half a change behind.
            Err(error) => vec![Delta::Notice {
                level: NoticeLevel::Error,
                message: format!("this machine has no entropy for a new identity: {error}"),
            }],
        }
    }

    /// Makes a §2.1 token — S37's `MachineChange::NewToken`.
    ///
    /// [`Self::new_desk_identity`]'s shape and its reason: a client that chose
    /// the token would be choosing this desk's password.
    fn new_token(&mut self) -> Vec<Delta> {
        match crate::machine::generate_token() {
            Ok(token) => {
                self.machine.config.set_token(&token);
                Vec::new()
            }
            Err(error) => vec![Delta::Notice {
                level: NoticeLevel::Error,
                message: format!("this machine has no entropy for an access token: {error}"),
            }],
        }
    }

    /// Writes the show to a different file and opens that one from then on —
    /// S37's `SaveShowAs`.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the file cannot be opened or written, in which
    /// case the show that is open is untouched and still the one that is open.
    fn save_show_as(&mut self, path: std::path::PathBuf) -> Result<Vec<Delta>, CoreError> {
        let path = self.resolve(path);
        let mut store = ShowStore::open(&path).map_err(CoreError::Store)?;
        // **The new file first, the switch afterwards.** A daemon that adopted
        // the path and then failed to write it would be holding a show whose
        // file does not exist, and the Save lamp would be lit over a name
        // nothing is behind.
        let mut deltas = store.save(&mut self.file).map_err(CoreError::Store)?;
        self.store = store;
        self.recovery = false;
        log::info("show", &format!("saved as {}", self.store.path().display()));
        deltas.extend(self.show_file_changed());
        Ok(deltas)
    }

    /// Opens a `.prism` file over the running show — S37's `OpenShow`.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the file is not there or will not read, and the
    /// running show is untouched.
    fn open_show(&mut self, path: std::path::PathBuf) -> Result<Vec<Delta>, CoreError> {
        let path = self.resolve(path);
        // **An open is not a create**, which is the whole difference between
        // this and `NewShow`: `ShowStore::open` makes a file that is not there,
        // so an operator's typo would otherwise silently become an empty show
        // with the file they meant still on the disk beside it.
        if !path.is_file() {
            return Err(CoreError::Store(prism_core::StoreError::Io(format!(
                "there is no show at {}",
                path.display()
            ))));
        }
        let store = ShowStore::open(&path).map_err(CoreError::Store)?;
        self.adopt(store)
    }

    /// Makes an empty show and opens it — S37's `NewShow`.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if something is already there, or if the file cannot
    /// be made. **Refused rather than overwritten**: a *new* show that replaced
    /// an existing one would be the most destructive command in
    /// `docs/IPC_PROTOCOL.md` §5 and would look like the least.
    fn new_show(&mut self, path: std::path::PathBuf) -> Result<Vec<Delta>, CoreError> {
        let path = self.resolve(path);
        if path.exists() {
            return Err(CoreError::Store(prism_core::StoreError::Io(format!(
                "{} is already there; open it, or choose another name",
                path.display()
            ))));
        }
        let mut store = ShowStore::open(&path).map_err(CoreError::Store)?;
        let mut empty = ShowFile::new();
        store.save(&mut empty).map_err(CoreError::Store)?;
        self.adopt(store)
    }

    /// Writes the show out as JSON — S37's `ExportShow`, over S15's
    /// `export_json`.
    ///
    /// Changes nothing at all, the open file included: an export is a copy in a
    /// second format, for a diff, a backup or a bug report. `prism_core::store`
    /// is where it says out loud that JSON is not bit-exact for floats and the
    /// `.prism` file is the authoritative one.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the show cannot be encoded or the file written.
    fn export_show(&mut self, path: &std::path::Path) -> Result<Vec<Delta>, CoreError> {
        let path = self.resolve(path.to_path_buf());
        let text = prism_core::export_json(&self.file).map_err(CoreError::Store)?;
        std::fs::write(&path, text)
            .map_err(|error| CoreError::Store(prism_core::StoreError::Io(error.to_string())))?;
        log::info("show", &format!("exported to {}", path.display()));
        Ok(vec![Delta::Notice {
            level: NoticeLevel::Info,
            message: format!("the show was exported to {}", path.display()),
        }])
    }

    /// Reads a JSON export back over the running show — S37's `ImportShow`.
    ///
    /// The show is replaced and **not** written to disk, so the Save lamp is lit
    /// afterwards: an import an operator did not mean to do must be one they can
    /// walk away from.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the file cannot be read or is not an export, and
    /// the running show is untouched.
    fn import_show(&mut self, path: std::path::PathBuf) -> Result<Vec<Delta>, CoreError> {
        let path = self.resolve(path);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| CoreError::Store(prism_core::StoreError::Io(error.to_string())))?;
        let imported = prism_core::import_json(&text).map_err(CoreError::Store)?;
        self.file.show = imported.show;
        self.file.session = imported.session;
        self.file.programmer = prism_core::Programmer::new();
        self.file.journal.clear();
        // An import is **not** a save: what is in memory is not what is in the
        // `.prism` file, and the lamp has to say so. `Show::mark_dirty` exists
        // for this one case, because a document read out of an export arrives
        // clean — it has just been deserialised.
        self.file.show.mark_dirty();
        self.masters = Masters::default();
        self.engine_programmer = ProgrammerState::default();
        let mut deltas = self.carry_out(Applied {
            deltas: Vec::new(),
            effects: vec![Effect::Repatch, Effect::ReloadGroups],
        })?;
        log::info("show", &format!("imported {}", path.display()));
        deltas.push(Delta::DirtyFlag {
            unsaved_changes: self.file.is_dirty(),
        });
        deltas.extend(self.show_file_changed());
        Ok(deltas)
    }

    /// Takes up a different `ShowStore` and loads what is in it.
    fn adopt(&mut self, store: ShowStore) -> Result<Vec<Delta>, CoreError> {
        self.store = store;
        self.recovery = self.store.has_recovery();
        let mut deltas = self.load()?;
        log::info("show", &format!("opened {}", self.store.path().display()));
        // A freshly loaded show has no unsaved edits, and the lamp has to say
        // so: the one that was open may well have had some.
        deltas.push(Delta::DirtyFlag {
            unsaved_changes: self.file.is_dirty(),
        });
        deltas.extend(self.show_file_changed());
        Ok(deltas)
    }

    /// Says which show is open, and writes it down so the next start finds it.
    fn show_file_changed(&mut self) -> Vec<Delta> {
        let path = self.store.path().display().to_string();
        self.machine.config.remember_show(&path);
        let mut deltas = self.write_machine();
        deltas.push(Delta::ShowFileChanged {
            file: self.show_file_info(),
        });
        deltas
    }

    /// Resolves a relative path against the data directory — S37.
    ///
    /// A client and a daemon do not share a working directory, so a bare
    /// `aula.prism` has to mean somewhere in particular, and the data directory
    /// is where a desk's own show already lives. An absolute path is taken as it
    /// stands, which is how a show on a stick is opened.
    fn resolve(&self, path: std::path::PathBuf) -> std::path::PathBuf {
        if path.is_absolute() {
            path
        } else {
            self.machine.data_dir.join(path)
        }
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
        let mut seen: BTreeMap<PlaybackId, (bool, Option<u32>)> = BTreeMap::new();
        for state in self.report.states() {
            seen.insert(state.playback, (state.is_active, state.cue_index));
            if self.reported.get(&state.playback) == Some(&(state.is_active, state.cue_index)) {
                continue;
            }
            // The show is asked as well, because the desk's own record is what a
            // fresh client's snapshot carries and it may already agree — a Go
            // wrote `is_active` on the way past, and the cue index is what
            // arrives late.
            // Nothing to broadcast when the show already agreed, and nothing at
            // all for a playback the show no longer has — the tick is one body
            // behind after a rebuild, and that is ordinary.
            if let Ok(true) = self.file.show.record_playback_state(
                state.playback,
                state.is_active,
                state.cue_index,
            ) {
                deltas.push(Delta::PlaybackState {
                    playback: state.playback,
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
    // **One playback per cue list, and that is the whole of it** (S45). It was
    // *every executor, plus the sequences nothing plays* until then, and
    // punch-list entry B18 is what that cost: two executors on one list were two
    // players of it, each with its own cue pointer and its own fade, fighting
    // over the same slots in the merge with neither of them wrong.
    //
    // Nothing about which fader holds what appears here any more, which is why
    // `Command::AssignExecutor` no longer asks for a rebuild: putting a list on
    // a slot moves a handle, and the set below does not mention slots.
    let playbacks: Vec<PlaybackId> = file
        .show
        .sequences()
        .map(|sequence| PlaybackId::of_sequence(sequence.id))
        .collect();
    let mut body = MergeBody::for_patch(layout, file.show.patched(), playbacks)?;
    body.report_into(Arc::clone(report));

    let groups: Vec<prism_domain::Group> = file.show.groups().cloned().collect();
    body.load_groups(&groups);
    // A cue list's master and its rate are **show** state (S14 relied on the
    // first being so, S34 made the second, S45 moved both off the executor), so
    // a freshly built body reads them out of the show rather than starting at
    // its constructor's defaults. Doing it here rather than in
    // `Masters::apply_to` is what gives the *first* body — the one `daemon`
    // builds before the tick starts — the levels a saved show was saved with.
    for sequence in file.show.sequences() {
        let playback = PlaybackId::of_sequence(sequence.id);
        body.layer_mut().set_master(playback, sequence.master_level);
        if let Some(player) = body.cues_mut().player_mut(playback) {
            player.set_speed(sequence.speed);
        }
        if let Err(error) = body.load_sequence(playback, sequence) {
            // A sequence the engine will not compile is one cue list that does
            // nothing, and the rest of the rig is unaffected — which is why it
            // is reported rather than refused.
            log::warn(
                "engine",
                &format!("sequence {} could not be loaded: {error}", sequence.id),
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
        AttributeType, Command, Delta, ExecutorId, FixtureId, GoDirection, GroupId, PlaybackId,
        PlaybackTarget, SelectionMode, SequenceId, UniverseId,
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
        let enrolment = publisher.enrolment();
        let subscriber = publisher.subscribe();
        let output = MockOutput::new(prism_domain::OutputId::new(1), [UniverseId::new(1)]);
        let frames = output.handle();
        let driver = spawn("out-mock", output, subscriber, RunnerConfig::default()).unwrap();
        let engine = EngineThread::start(body, publisher).unwrap();

        let core = Core::new(
            file,
            crate::outputs::Machine {
                config: prism_core::MachineConfig::default(),
                path: None,
                outputs: crate::outputs::OutputSupervisor::new(
                    enrolment,
                    Box::new(crate::outputs::MockDevices::default()),
                    crate::outputs::OutputContext {
                        cid: prism_protocols::Cid::from_u128(1),
                        source_name: "PrismDMX test".to_owned(),
                    },
                ),
                surface_on_command_line: false,
                profile_on_command_line: false,
                data_dir: dir.to_path_buf(),
                overrides: Vec::new(),
                websocket_open: None,
            },
            store,
            engine,
            layout,
            report,
            prism_surface::Bindings::defaults(),
        )
        .unwrap();
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

        // **Two presses since S51** (B37): the first takes the selection and
        // the second the values, and it is the values the wire is holding.
        core.apply(&Command::ClearProgrammer).unwrap();
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
            software_dimmer: true,
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
            software_dimmer: true,
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
            software_dimmer: true,
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
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
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
            told.contains(&Delta::PlaybackState {
                playback: PlaybackId::of_sequence(SequenceId::new(1)),
                is_active: true,
                cue_index: Some(0),
            })
        });

        core.apply(&Command::ExecutorOff {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
        })
        .unwrap();
        let mut stopped = Vec::new();
        until("the client to be told it stopped", || {
            stopped.extend(core.poll_playback());
            stopped.iter().any(|delta| {
                matches!(
                    delta,
                    Delta::PlaybackState {
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
            software_dimmer: true,
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
                target: PlaybackTarget::of_executor(ExecutorId::new(9)),
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
            software_dimmer: true,
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
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
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
            software_dimmer: true,
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
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
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
                .any(|delta| matches!(delta, Delta::PlaybackState { .. }))
        );

        // Already running, and this sequence has one cue: the Go steps to the
        // cue it is already on, so nothing about the executor moved and the
        // readback has nothing to say. An LED cannot be lit twice (S11), and
        // since S34 the *readback* is what would say so.
        core.apply(&Command::ExecutorGo {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
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
            software_dimmer: true,
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
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "2".to_owned(),
            mode: prism_domain::StoreMode::Merge,
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

    // -- S45: one sequence, one playback, asserted on frames -----------------

    /// A rig with **two** executors on one cue list, each with a fader function
    /// the test chooses, playing a two-cue list on the dark dimmer at channel 5.
    ///
    /// Executor 3 is the first handle and executor 4 is the second. Channel 5 is
    /// dark at home, so what a cue and a master do to it is visible; channel 1
    /// sits at full whatever happens, which is what says the rig is alive.
    fn desk_for_two_handles(
        dir: &std::path::Path,
        first: prism_domain::ExecutorFaderFunction,
        second: prism_domain::ExecutorFaderFunction,
    ) -> (Core, MockOutputHandle, OutputThread) {
        use crate::testkit::{cue, executor, sequence};
        use prism_domain::ExecutorButtonFunction as Fn;
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
        for (id, fader) in [(3, first), (4, second)] {
            let mut slot = executor(id, Some(7));
            slot.fader_function = fader;
            slot.button_functions = vec![Fn::On, Fn::Off, Fn::GoForward, Fn::Empty];
            file.show.store_executor(slot).unwrap();
        }
        file.show
            .set_sequence_master(SequenceId::new(7), u16::MAX)
            .unwrap();
        file.show.mark_saved();
        desk_with(dir, file)
    }

    /// Presses one of executor `id`'s four keys.
    fn press_on(core: &mut Core, id: u32, index: u8) {
        core.apply(&Command::ExecutorButton {
            executor_id: ExecutorId::new(id),
            button: prism_domain::ExecutorButtonRef::Slot { index },
            pressed: true,
        })
        .expect("the executor plays a cue list");
    }

    /// **Exit criterion, punch-list B18.** Two executors on one cue list with
    /// the same fader function move together *at the DMX output*.
    ///
    /// The frame is the only place the difference is real: a screen showing one
    /// number twice is a screen, and what the entry is about is that the light
    /// followed one fader and not the other.
    #[test]
    fn two_master_faders_on_one_cue_list_move_one_light() {
        use prism_domain::ExecutorFaderFunction as Fader;
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) =
            desk_for_two_handles(dir.path(), Fader::Master, Fader::Master);
        until("the rig at home", || channel(&frames, 1) == Some(255));

        // Executor 3 switches the list on, and its cue takes channel 5 to full.
        press_on(&mut core, 3, 0);
        until("the cue", || channel(&frames, 5) == Some(255));

        // Executor 4's fader is a `Master` on the same list, so it is the same
        // number: pulling it halves the light executor 3's Go put up.
        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(4),
            level: 32_768,
        })
        .unwrap();
        until("the second fader to move the same light", || {
            channel(&frames, 5) == Some(128)
        });
        // And it is one number rather than two that happen to agree.
        assert_eq!(
            core.file
                .show
                .sequence(SequenceId::new(7))
                .unwrap()
                .master_level,
            32_768
        );

        // Back up from the *first* fader, which is the other direction of the
        // same claim.
        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(3),
            level: u16::MAX,
        })
        .unwrap();
        until("the first fader to move it back", || {
            channel(&frames, 5) == Some(255)
        });

        driver.stop();
    }

    /// **The other half of B18**: a `Master` and an `XFade` on one cue list are
    /// two different handles and stay independent.
    ///
    /// *Ist ein Fader XFade und ein Fader Master, sollten beide unabhängig
    /// voneinander funktionieren* — the entry, in the owner's words. What
    /// "independent" means on a frame is that each one moves the light in its
    /// own way and neither writes the other's number: the crossfade drives the
    /// transition's clock (`docs/DMX_MERGE.md` §4.2) and the master scales what
    /// comes out of it, so the two compose rather than fight.
    #[test]
    fn a_master_and_a_crossfade_on_one_cue_list_are_two_handles() {
        use prism_domain::ExecutorFaderFunction as Fader;
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) =
            desk_for_two_handles(dir.path(), Fader::Master, Fader::XFade);
        until("the rig at home", || channel(&frames, 1) == Some(255));

        press_on(&mut core, 3, 0);
        until("the cue", || channel(&frames, 5) == Some(255));

        // Executor 4 is a crossfade: moving it takes the transition's clock off
        // the engine and puts it on the fader, so the light moves.
        //
        // **Engaging one moves nothing, and the movement is what starts it** —
        // S51, B36. It used to head for the far end the moment it was engaged;
        // now the fader is put where the operator's hand is and the first
        // *movement* arms the stroke, which is what makes a fader switched to a
        // crossfade mid-show change nothing at all.
        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(4),
            level: 0,
        })
        .unwrap();
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            channel(&frames, 5),
            Some(255),
            "engaging a crossfade moved the light"
        );

        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(4),
            level: 20_000,
        })
        .unwrap();
        until("the crossfade to take the transition over", || {
            channel(&frames, 5) != Some(255)
        });
        // And it wrote **nothing** into the show: where a crossfade fader stands
        // is a gesture in progress, so executor 3's master is exactly where the
        // operator left it. A second `Master` would have written this number.
        assert_eq!(
            core.file
                .show
                .sequence(SequenceId::new(7))
                .unwrap()
                .master_level,
            u16::MAX,
            "the crossfade wrote the master"
        );

        // Driving the crossfade to its far end completes the cue, which lands
        // the light on cue 2's own level.
        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(4),
            level: u16::MAX,
        })
        .unwrap();
        let arrived = 20_000_u16 >> 8;
        until("the crossfade to finish the cue", || {
            channel(&frames, 5) == Some(u8::try_from(arrived).unwrap_or(0))
        });

        // The master still scales it, from its own handle and by its own factor
        // — which is the two of them composing rather than one of them being
        // the other.
        core.apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(3),
            level: 32_768,
        })
        .unwrap();
        until("the master to scale what the crossfade left", || {
            channel(&frames, 5) == Some(u8::try_from(arrived / 2).unwrap_or(0))
        });

        driver.stop();
    }

    /// **Exit criterion, B18's deeper half.** `Go` on either of two executors
    /// carrying one cue list advances **one** cue pointer.
    ///
    /// Before S45 each executor was a playback of its own, so the second Go
    /// started a second player at cue 1 while the first stood on cue 2 — two
    /// contributors to the same slots, and `docs/DMX_MERGE.md` did what it was
    /// told with both.
    #[test]
    fn a_go_on_either_handle_advances_one_cue_pointer() {
        use prism_domain::ExecutorFaderFunction as Fader;
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) =
            desk_for_two_handles(dir.path(), Fader::Master, Fader::Master);
        until("the rig at home", || channel(&frames, 1) == Some(255));

        // On, from the first handle: cue 1 takes channel 5 to full.
        press_on(&mut core, 3, 0);
        until("the first cue", || {
            core.poll_playback();
            core.file
                .show
                .sequence(SequenceId::new(7))
                .is_some_and(|sequence| sequence.current_cue_index == Some(0))
        });

        // Go, from the **second** handle: the same pointer moves to cue 2, and
        // the light goes with it. A second playback would have entered cue 1
        // again and held channel 5 at full.
        press_on(&mut core, 4, 2);
        until("the second cue", || {
            core.poll_playback();
            core.file
                .show
                .sequence(SequenceId::new(7))
                .is_some_and(|sequence| sequence.current_cue_index == Some(1))
        });
        until("the second cue's level", || channel(&frames, 5) == Some(78));

        // Both rows say the same thing, because there is one row and both
        // executors read it — which is the half of the entry an operator sees.
        for id in [3, 4] {
            let sequence = core
                .file
                .show
                .executor(ExecutorId::new(id))
                .and_then(|executor| executor.sequence_id)
                .and_then(|id| core.file.show.sequence(id))
                .expect("both executors carry cue list 7");
            assert_eq!(sequence.current_cue_index, Some(1), "executor {id}");
            assert!(sequence.is_active, "executor {id}");
        }

        driver.stop();
    }

    /// **Exit criterion, and the whole of S49 in one test.** A key with a line
    /// on it fires the line, and there is **no client attached at all**.
    ///
    /// Before S49 this test could only assert the line and a counter: the parser
    /// was in the interface, so the daemon wrote `Session::command_line`, bumped
    /// `Session::command_line_run` and waited for whichever browser held the
    /// keyboard focus to turn it into commands. A desk with nobody watching a
    /// screen did nothing; a desk with two watching did it twice, and a doubled
    /// Go is the fault the session exists to remove.
    ///
    /// So the assertion is on the **frames** now, which is the only place that
    /// can tell the two arrangements apart.
    #[test]
    fn a_key_with_a_line_on_it_fires_the_line() {
        use prism_domain::ExecutorButtonFunction as Fn;
        let dir = tempfile::tempdir().unwrap();
        let (mut core, frames, driver) = desk_for_buttons(
            dir.path(),
            vec![Fn::Empty, Fn::Empty, Fn::Empty, Fn::Empty],
            prism_domain::ExecutorFaderFunction::Master,
        );
        until("the rig at home", || channel(&frames, 1) == Some(255));

        core.apply(&Command::ConfigureExecutor {
            executor_id: ExecutorId::new(3),
            change: prism_domain::ExecutorChange::Button {
                index: 1,
                function: Fn::CommandLine {
                    line: "On Sequence 7".to_owned(),
                },
            },
        })
        .unwrap();

        press(&mut core, 1, true);
        until("the bound line to put light on the rig", || {
            channel(&frames, 5) == Some(255)
        });
        // And the line is cleared, exactly as it is when Enter runs one: a line
        // that has been run is not a line an operator is still writing.
        assert_eq!(core.file.session.session().command_line, "");

        // Typing the same line by hand leaves the desk in the same place, which
        // is the whole of "exactly what typing the line produces" — and it is
        // now a claim about the *show* rather than about a counter.
        let typed = {
            let dir = tempfile::tempdir().unwrap();
            let (mut other, other_frames, driver) = desk_for_buttons(
                dir.path(),
                vec![Fn::Empty, Fn::Empty, Fn::Empty, Fn::Empty],
                prism_domain::ExecutorFaderFunction::Master,
            );
            other
                .apply(&Command::CommandLineInput {
                    text: "On Sequence 7".to_owned(),
                    run: true,
                    mode: None,
                })
                .unwrap();
            until("the typed line to put light on the rig", || {
                channel(&other_frames, 5) == Some(255)
            });
            let session = other.file.session.session().clone();
            let sequence = other
                .file
                .show
                .sequence(SequenceId::new(7))
                .unwrap()
                .clone();
            driver.stop();
            (session, sequence)
        };
        let sequence = core.file.show.sequence(SequenceId::new(7)).unwrap();
        assert_eq!(typed.0.command_line, "");
        assert_eq!(typed.1.is_active, sequence.is_active);
        assert_eq!(typed.1.current_cue_index, sequence.current_cue_index);

        driver.stop();
    }

    /// **A line that is not one is written and left standing** — S49.
    ///
    /// The other half of running a line at the daemon, and the half a pointer
    /// depends on: `Store` typed, a fixture tile clicked, and the candidate
    /// `Store Fixture 5` is not a command. It must not vanish and it must not be
    /// carried out — the operator sees what they built, with the daemon's own
    /// complaint under it (`Query::CommandLineReading`), and corrects it.
    #[test]
    fn a_line_that_is_not_a_command_is_written_and_left_standing() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, _frames, driver) = desk_for_buttons(
            dir.path(),
            vec![prism_domain::ExecutorButtonFunction::Empty; 4],
            prism_domain::ExecutorFaderFunction::Master,
        );

        core.apply(&Command::CommandLineInput {
            text: "Store Fixture 5".to_owned(),
            run: true,
            mode: None,
        })
        .unwrap();
        assert_eq!(core.file.session.session().command_line, "Store Fixture 5");
        // Nothing was stored, which is what *left standing* has to mean.
        assert!(core.file.show.sequence(SequenceId::new(7)).is_some());

        driver.stop();
    }

    /// **A refusal stops the rest of the line** — S49, and it is a change from
    /// what the client-side loop did.
    ///
    /// `1 thru 4 at 50` on a rig whose fixture 4 is not patched is one sentence
    /// whose first half cannot be carried out. Before S49 a client sent both
    /// commands and the second set a level on whatever happened to be selected;
    /// now the line stops, and the refusal travels as a notice.
    #[test]
    fn a_refusal_stops_the_rest_of_the_line() {
        let dir = tempfile::tempdir().unwrap();
        let (mut core, _frames, driver) = desk_for_buttons(
            dir.path(),
            vec![prism_domain::ExecutorButtonFunction::Empty; 4],
            prism_domain::ExecutorFaderFunction::Master,
        );
        let before = core.file.programmer.state().clone();

        let deltas = core
            .apply(&Command::CommandLineInput {
                text: "404 at 50".to_owned(),
                run: true,
                mode: None,
            })
            .unwrap();

        assert!(
            deltas.iter().any(|delta| matches!(
                delta,
                Delta::Notice {
                    level: prism_domain::NoticeLevel::Error,
                    ..
                }
            )),
            "the refusal is said out loud: {deltas:?}"
        );
        assert_eq!(
            &before,
            core.file.programmer.state(),
            "the second half of the line was carried out"
        );

        driver.stop();
    }

    /// **Exit criterion**: a custom row survives a save and a restart.
    ///
    /// The `.prism` file keeps each executor as an opaque MessagePack document
    /// (S15), and the eight fixed functions are bare strings in it — so the
    /// ninth, which carries a line, is the one that had to be checked rather
    /// than assumed.
    #[test]
    fn a_custom_row_survives_a_save_and_a_restart() {
        use prism_domain::ExecutorButtonFunction as Fn;
        let dir = tempfile::tempdir().unwrap();
        let (mut core, _frames, driver) = desk_for_buttons(
            dir.path(),
            vec![Fn::On, Fn::Empty, Fn::Empty, Fn::Empty],
            prism_domain::ExecutorFaderFunction::Speed,
        );
        core.apply(&Command::ConfigureExecutor {
            executor_id: ExecutorId::new(3),
            change: prism_domain::ExecutorChange::Button {
                index: 3,
                function: Fn::CommandLine {
                    line: "Go+ Sequence 7".to_owned(),
                },
            },
        })
        .unwrap();
        core.apply(&Command::SaveShow).unwrap();
        let read = core.store().read().unwrap();
        driver.stop();

        let executor = read.show.executor(ExecutorId::new(3)).expect("executor 3");
        assert_eq!(
            executor.button_functions[3],
            Fn::CommandLine {
                line: "Go+ Sequence 7".to_owned()
            }
        );
        assert_eq!(executor.button_functions[0], Fn::On);
        assert_eq!(
            executor.fader_function,
            prism_domain::ExecutorFaderFunction::Speed
        );
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
        file.show.store_executor(slot).unwrap();
        file.show
            .set_sequence_master(SequenceId::new(7), u16::MAX)
            .unwrap();
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
                .sequence(SequenceId::new(7))
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
            target: PlaybackTarget::of_executor(ExecutorId::new(3)),
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
                .sequence(SequenceId::new(7))
                .is_some_and(|sequence| sequence.is_active)
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
                .sequence(SequenceId::new(7))
                .unwrap()
                .current_cue_index,
            None,
            "a stopped playback is on no cue"
        );

        press(&mut core, 0, true);
        until("the cue index", || {
            !core.poll_playback().is_empty()
                || core
                    .file
                    .show
                    .sequence(SequenceId::new(7))
                    .is_some_and(|sequence| sequence.current_cue_index == Some(0))
        });
        assert_eq!(
            core.file
                .show
                .sequence(SequenceId::new(7))
                .unwrap()
                .current_cue_index,
            Some(0)
        );

        press(&mut core, 2, true);
        until("the second cue", || {
            core.poll_playback();
            core.file
                .show
                .sequence(SequenceId::new(7))
                .is_some_and(|sequence| sequence.current_cue_index == Some(1))
        });

        press(&mut core, 1, true);
        until("the cue index to go away", || {
            core.poll_playback();
            core.file
                .show
                .sequence(SequenceId::new(7))
                .is_some_and(|sequence| sequence.current_cue_index.is_none())
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
            target: PlaybackTarget::of_executor(ExecutorId::new(3)),
            direction: GoDirection::Next,
        })
        .unwrap();
        until("the cue index to arrive", || {
            !core.poll_playback().is_empty()
                || core
                    .file
                    .show
                    .sequence(SequenceId::new(7))
                    .is_some_and(|sequence| sequence.current_cue_index == Some(0))
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
            // The rate and the master are the cue list's since S45.
            let sequence = core.file.show.sequence(SequenceId::new(7)).unwrap();
            assert_eq!(sequence.speed, 2_048);
            assert_eq!(sequence.master_level, u16::MAX, "the master moved");
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
            let before = core.file.show.sequence(SequenceId::new(7)).unwrap().clone();
            let deltas = core
                .apply(&Command::SetExecutorMaster {
                    executor_id: ExecutorId::new(3),
                    level: 30_000,
                })
                .unwrap();
            assert!(deltas.is_empty(), "{deltas:?}");
            assert_eq!(core.file.show.sequence(SequenceId::new(7)), Some(&before));
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
            let before = core.file.show.sequence(SequenceId::new(7)).unwrap().clone();
            assert!(
                core.apply(&Command::SetExecutorMaster {
                    executor_id: ExecutorId::new(3),
                    level: 1,
                })
                .unwrap()
                .is_empty()
            );
            assert_eq!(core.file.show.sequence(SequenceId::new(7)), Some(&before));
            driver.stop();
        }
    }
}
