//! Starting a daemon, running it, and stopping it on purpose.
//!
//! The order matters and every step of it was decided by an earlier session:
//!
//! 1. **The lock first** (`ARCHITECTURE_SPEC.md` §10.3). Two daemons driving one
//!    rig is the worst failure this project has, so nothing else is opened until
//!    it is certain there is only one.
//! 2. **The desk identity next** (S10, S11). An sACN output holding the nil CID
//!    refuses to connect, so the identity is generated and written *before* any
//!    output exists.
//! 3. **The show, then the engine, then the outputs — in that order and not
//!    another.** `FramePublisher::subscribe` allocates (S2), so every output and
//!    the telemetry channel are attached before the tick starts.
//! 4. **The listeners last**, and their addresses go into the lock file only
//!    once they are bound: an endpoint published before its listener exists is
//!    an address clients fail to reach.
//!
//! # Stopping
//!
//! `ARCHITECTURE_SPEC.md` §10.3: the daemon exits only on explicit instruction,
//! *sending sACN termination packets and applying a configurable
//! blackout-or-hold*. Both halves are here, and the blackout half is a **frame,
//! not a flag** (S10): the sACN driver ends its streams carrying the last look,
//! so a daemon that wants a dark stage publishes a blackout and waits for the
//! outputs to send it before stopping them. Telling a driver to "shut down dark"
//! would have put the decision one level too low.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use prism_core::{MachineConfig, ShowFile, ShowStore};
use prism_domain::{Delta, OutputHealth, OutputId, OutputInstance, UniverseId};
use prism_engine::{FrameLayout, FramePublisher, FrameSubscriber, PlaybackReport};
use prism_ipc::{
    Server, ServerConfig, TelemetryFrame, local::LocalListener, websocket::WebSocketListener,
};
use prism_protocols::MockOutputHandle;

use crate::cli::{Exit, Options};
use crate::core::{Core, build_body};
use crate::engine::EngineThread;
use crate::lock::{DaemonLock, LockError};
use crate::log;
use crate::outputs::{
    Machine, MockDevices, OutputContext, OutputFactory, OutputSupervisor, SystemOutputs,
};
use crate::paths;
use crate::server::{Desk, DeskHandler};
use crate::surface::{SURFACE_PERIOD, SurfaceLink, SurfacePort};
use prism_surface::Bindings;

/// How often the daemon looks at its own housekeeping: the autosave policy and
/// the outputs' health.
const HOUSEKEEPING: Duration = Duration::from_millis(500);

/// `docs/IPC_PROTOCOL.md` §7: 25 to 30 Hz, independent of the 44 Hz tick.
const TELEMETRY_PERIOD: Duration = Duration::from_millis(33);

/// How often the daemon reads what the tick says its playbacks are doing (S34).
///
/// One engine tick and a bit, so a cue that changes is on the screen within a
/// frame or two of changing — a cue number that lagged a Go by half a second
/// would be worse than no number. It costs a walk over the executor grid and
/// **nothing is broadcast unless something moved**, which is what keeps the
/// show document still during a fade: a cue index changes when a cue changes.
const PLAYBACK_PERIOD: Duration = Duration::from_millis(25);

/// How long the daemon gives a blackout frame to reach the fixtures before it
/// stops the outputs.
///
/// Three engine ticks and an output cadence: long enough for the frame to be
/// rendered, published, taken by a driver on its own cadence and put on the
/// wire, and short enough that nobody waiting for the process to end notices.
const BLACKOUT_SETTLE: Duration = Duration::from_millis(150);

/// Why the daemon could not start.
#[derive(Debug)]
pub enum StartError {
    /// Another daemon is running, or the lock could not be taken.
    Lock(LockError),
    /// The data directory could not be worked out from the environment.
    NoDataDirectory(paths::NoDataDirectory),
    /// A file could not be read or written.
    Io(std::io::Error),
    /// The show file could not be opened.
    Store(prism_core::StoreError),
    /// The show cannot be turned into a running engine.
    Patch(prism_engine::PatchError),
    /// The frame layout is not a layout.
    Layout(prism_engine::LayoutError),
}

impl core::fmt::Display for StartError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Lock(error) => error.fmt(f),
            Self::NoDataDirectory(error) => error.fmt(f),
            Self::Io(error) => write!(f, "the daemon could not start: {error}"),
            Self::Store(error) => write!(f, "the show could not be opened: {error}"),
            Self::Patch(error) => write!(f, "the show could not be played: {error}"),
            Self::Layout(error) => write!(f, "the frame layout is wrong: {error}"),
        }
    }
}

impl core::error::Error for StartError {}

impl From<LockError> for StartError {
    fn from(error: LockError) -> Self {
        Self::Lock(error)
    }
}

impl From<std::io::Error> for StartError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<prism_core::StoreError> for StartError {
    fn from(error: prism_core::StoreError) -> Self {
        Self::Store(error)
    }
}

/// A running daemon, and everything that has to be shut down in order.
///
/// `Debug` shows what it is rather than what it holds: an engine thread, a set
/// of driver threads and a lock have no useful debug form between them, and the
/// two things a person wants at a breakpoint are where clients reach it and how
/// many outputs it opened.
pub struct Daemon {
    desk: Arc<Desk>,
    server: Server,
    telemetry: FrameSubscriber,
    /// Whether the engine has published its first frame — see [`Self::published`].
    telemetry_started: bool,
    layout: Arc<FrameLayout>,
    lock: DaemonLock,
    exit: Exit,
    listeners: Vec<tokio::task::JoinHandle<()>>,
    /// The binding table a surface attached to this daemon will use, read at
    /// startup so a broken profile is reported once rather than per surface.
    bindings: Bindings,
    /// Where that table was read from, or `None` for the built-in one — S37.
    ///
    /// Kept beside the table rather than derived from it, because a profile
    /// that was **not readable** produces the built-in table and a path that is
    /// still worth showing: *this file is named and this is what is in force*
    /// is exactly the pair the Devices panel has to draw, and it is S36's
    /// `configured` and `open` one layer along.
    profile: Option<PathBuf>,
    /// The control surface, once one has been attached. `None` is the ordinary
    /// state: **D2** says the daemon runs with no client, and it runs with no
    /// console just as happily.
    surface: Option<SurfaceLink>,
}

impl core::fmt::Debug for Daemon {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Daemon")
            .field("endpoints", &self.endpoints())
            .field("outputs", &self.desk.core().outputs().entries().len())
            .field("exit", &self.exit)
            .finish_non_exhaustive()
    }
}

impl Daemon {
    /// Opens everything, in the order the module documentation gives.
    ///
    /// # Errors
    ///
    /// [`StartError`] — and the common one is [`StartError::Lock`] carrying
    /// `LockError::AlreadyRunning`, which is not a fault: it is the shell being
    /// told to attach to the daemon that is already there (D9).
    pub async fn start(options: &Options) -> Result<Self, StartError> {
        // Whatever the command line said, before anything else — so that a
        // `--log-level debug` is in force for the lines below it. The
        // *configured* level is picked up a few lines further down, once the
        // machine configuration has been read, which is the earliest it can be.
        if let Some(level) = options.log_level {
            log::set_level(level);
        }

        // 1. The lock.
        let data_dir = match options.data_dir.clone() {
            Some(directory) => directory,
            None => paths::system_data_dir().map_err(StartError::NoDataDirectory)?,
        };
        let mut lock = DaemonLock::acquire(&data_dir)?;
        if lock.took_over_a_stale_lock() {
            log::warn(
                "lock",
                "a lock file was left behind by a daemon that is no longer running; taking it over",
            );
        }
        log::info(
            "lock",
            &format!(
                "process {} owns {}",
                lock.document().pid,
                lock.path().display()
            ),
        );

        // 2. The desk identity, before any output exists.
        let (machine, created) =
            crate::machine::load_or_create(&paths::machine_config_path(&data_dir))?;
        if created {
            log::info(
                "desk",
                &format!(
                    "this desk had no identity, so one was made: {}",
                    machine.desk_id()
                ),
            );
        }

        // 2a. What this run is actually set to — S37. The command line wins
        // where it said anything, the machine configuration answers everywhere
        // else, and `Resolved::overrides` is the list a settings window greys
        // rows out from.
        let settings = crate::cli::resolve(
            options,
            machine.settings(),
            !options.outputs.is_empty(),
            options.surface.is_some(),
        );
        log::set_level(settings.log_level);

        // 3. The show, the engine, the outputs.
        //
        // Which show: the flag, then the one this desk had open when it was last
        // stopped (S37), then the default in the data directory. The middle one
        // is what makes a desk start where it was left.
        let show_path = options
            .show
            .clone()
            .or_else(|| {
                machine
                    .shows()
                    .paths
                    .first()
                    .map(PathBuf::from)
                    .filter(|path| path.is_file())
            })
            .unwrap_or_else(|| paths::default_show_path(&data_dir));
        let (file, store) = open_show(&show_path)?;
        // **Off the critical path.** Reading the Open Fixture Library is 634
        // files and about 8.5 MB — measured at 395 ms in a release build and
        // 1.5 s in a debug one — and nothing between here and the first DMX
        // frame needs it. So it is read on a thread of its own while the engine
        // and the outputs come up, and joined below, before the listeners bind.
        // A desk's first duty is to put light on stage; a profile menu can wait
        // for the one client that might ask about it, and by then it has not
        // had to.
        let loading = spawn_library_load(&data_dir, settings.fixtures.clone());
        let layout =
            Arc::new(crate::engine::frame_layout(settings.universes).map_err(StartError::Layout)?);
        // The channel back out of the tick (S34), built once and shared: a
        // rebuilt body inherits it, so the desk never loses sight of what its
        // playbacks are doing. Sized at `MAX_SOURCES`, which is the limit
        // `PlaybackLayer::new` already refuses a show for exceeding, so it can
        // never be too small — and nothing may resize it, because that would
        // allocate inside the tick.
        let report = Arc::new(PlaybackReport::new(prism_engine::MAX_SOURCES));
        let body = build_body(&layout, &file, &report).map_err(StartError::Patch)?;

        let mut publisher = FramePublisher::new(Arc::clone(&layout));
        let source_name = source_name(&show_path);
        // The telemetry channel is a subscriber like any driver, so what a
        // client sees is the frame the fixtures got rather than a second
        // calculation of it. Taken **before** the outputs and directly off the
        // publisher, because it is the one subscriber that is never
        // reconfigured — every other one now joins and leaves through the
        // enrolment (S33).
        let telemetry = crate::engine::telemetry_subscriber(&mut publisher);

        // Which rig this run uses, and whether it is this machine's to change.
        // See `rig_for`: outputs named on the command line are the whole rig for
        // the run and are not written back, so a daemon started with
        // `--mock-output` neither inherits a venue's cabling nor overwrites it.
        let (rig, machine_path) = rig_for(options, &machine, &file, &data_dir);
        let factory: Box<dyn OutputFactory> = if options.mock_devices {
            log::info(
                "output",
                "every output is opened with a mock driver (--mock-devices):                  no device is touched and nothing goes on the network",
            );
            Box::new(MockDevices::default())
        } else {
            Box::new(SystemOutputs::default())
        };
        let mut outputs = OutputSupervisor::new(
            publisher.enrolment(),
            factory,
            OutputContext {
                cid: crate::machine::cid_of(&machine),
                source_name: source_name.clone(),
            },
        );
        // **The one place a listening socket is opened.** Not under
        // `--mock-devices`, where nothing may touch a device or a network; not
        // under `--no-artnet-discovery`, which is how a second desk on one
        // machine gives the port to the first; and not at all until the rig has
        // an Art-Net row — `Discovery::retarget` decides that, out of the rig
        // `reconcile` is about to be given. S46.
        if options.artnet_discovery && !options.mock_devices {
            outputs.adopt_discovery(crate::discovery::Discovery::system());
        } else {
            // Told not to listen, which is a **reason** and not an absence: a
            // panel that said *configure an Art-Net output and I will listen*
            // would be telling an operator to do something that would not help.
            outputs.adopt_discovery(crate::discovery::Discovery::disabled(
                if options.mock_devices {
                    "node discovery is off for this run (--mock-devices)"
                } else {
                    "node discovery is off for this run (--no-artnet-discovery)"
                },
            ));
        }
        outputs.reconcile(&rig);
        let output_count = outputs.entries().len();
        // Said once on the way up, before anything is on stage: a universe the
        // patch uses and no cable carries is a legitimate state and never a
        // silent one (`prism_core::ShowIssue::UniverseNotOutput`).
        for issue in prism_core::dark_universes(&file.show, &outputs.carried_universes()) {
            log::warn("output", &issue.to_string());
        }

        let engine = EngineThread::start(body, publisher)?;

        // Joined **after** the engine is running and **before** anything can
        // ask: a client's first `Snapshot` carries how many profiles this desk
        // has, and a number that was right a moment later would be worse than a
        // handshake that waited. By now the load has had the whole of the
        // engine and output start-up to run in.
        let mut file = file;
        file.library = join_library_load(loading);
        // The rig this run is using, so that a snapshot shows what is actually
        // there. When the command line supplied it, `machine_path` is `None`,
        // the stored configuration is left alone and the four output commands
        // are refused.
        let machine = if machine_path.is_some() {
            machine
        } else {
            MachineConfig::with_outputs(machine.desk_id(), rig.clone())
        };
        // What this desk's keys do, in the order S38 decided - see
        // `starting_bindings`. The flag first, then this machine's own table,
        // then the profile it names, then the built-in defaults.
        let (bindings, adopt) =
            starting_bindings(options, &machine, settings.surface_profile.as_deref());
        let mut core = Core::new(
            file,
            Machine {
                config: machine,
                path: machine_path,
                outputs,
                surface_on_command_line: options.surface.is_some(),
                profile_on_command_line: options.surface_profile.is_some(),
                data_dir: data_dir.clone(),
                overrides: settings.overrides.clone(),
                websocket_open: None,
            },
            store,
            engine,
            Arc::clone(&layout),
            report,
            bindings.clone(),
        )
        .map_err(StartError::Patch)?;
        // A desk that has never been told writes down what it started with, so
        // that the first edit is a change to a table rather than the creation of
        // one - and so that `machine.json` says what the keys do even before
        // anybody has touched them. Not done when a flag is holding the table:
        // that run neither reads the stored one nor writes it.
        if adopt {
            drop(core.adopt_bindings(bindings.clone()));
        }
        // A desk that starts where it was left has to write down where that is
        // — S37. Recorded here rather than in `Core::open_show`, because the
        // show a daemon starts with does not arrive through a command.
        core.remember_show();
        let desk = Arc::new(Desk::new(core));

        // 4. The listeners, and only then their addresses.
        let server = Server::with_config(
            DeskHandler::new(Arc::clone(&desk)),
            ServerConfig {
                token: settings.token.clone(),
                ..ServerConfig::default()
            },
        );
        let mut listeners = Vec::new();
        if settings.local {
            let address = prism_ipc::local::daemon_address(&label_for(&data_dir));
            let mut listener = LocalListener::bind(&address)?;
            let endpoint = listener.endpoint();
            lock.publish(&endpoint)?;
            log::info("ipc", &format!("listening on {endpoint}"));
            let accepting = server.clone();
            listeners.push(tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok(wire) => drop(accepting.spawn(wire)),
                        Err(error) => {
                            log::warn("ipc", &format!("a client could not be accepted: {error}"));
                        }
                    }
                }
            }));
        }
        // **A listener that cannot bind is a warning and a daemon that starts**
        // — S37, and it is the rule S36 wrote for a MIDI port that is not there
        // rather than a new one. The WebSocket listener is *on by default* now,
        // so the address is one two daemons on one machine will both ask for;
        // refusing to start over it would mean one stray process makes a desk
        // unstartable half an hour before a show. What the operator gets instead
        // is a settings panel that says *configured here, listening nowhere*,
        // which is the state `MachineSettings::websocket_open` exists to draw.
        if let Some(address) = settings.websocket {
            match WebSocketListener::bind(address).await {
                Ok(mut listener) => {
                    let endpoint = listener.endpoint();
                    lock.publish(&endpoint)?;
                    if let Some(token) = &settings.token {
                        lock.publish_token(token)?;
                    }
                    log::info("ipc", &format!("listening on {endpoint}"));
                    desk.core().set_websocket_open(Some(address));
                    let accepting = server.clone();
                    listeners.push(tokio::spawn(async move {
                        while let Some(wire) = listener.accept().await {
                            drop(accepting.spawn(wire));
                        }
                    }));
                }
                Err(error) => log::warn(
                    "ipc",
                    &format!(
                        "the WebSocket listener could not bind {address} ({error});                          the daemon is running without it"
                    ),
                ),
            }
        }

        log::info(
            "daemon",
            &format!(
                "prismd {} is running: {} at {} Hz, {} output(s), show {}",
                env!("CARGO_PKG_VERSION"),
                layout.universe_count(),
                prism_engine::TICK_HZ,
                output_count,
                show_path.display()
            ),
        );

        let mut daemon = Self {
            desk,
            server,
            telemetry,
            telemetry_started: false,
            layout,
            lock,
            exit: settings.exit,
            listeners,
            bindings,
            profile: settings.surface_profile.clone(),
            surface: None,
        };

        // Which surface this run has, in order — **S36**.
        //
        // `--mock-surface` first, because it is the one a test asks for and a
        // test must never reach a device; then `--surface`, which is the port
        // for the run and is why `SetSurfacePort` is refused; then the machine
        // configuration, which is what a desk in a rack actually starts with.
        // Nothing is a legitimate outcome: a laptop has no X-Touch.
        if let Some(path) = &options.mock_surface {
            match crate::surface::FileSurfacePort::open(path) {
                Ok(port) => daemon.attach_surface(Box::new(port)),
                Err(error) => log::warn(
                    "surface",
                    &format!(
                        "{} could not be opened ({error}); no surface is attached",
                        path.display()
                    ),
                ),
            }
        } else if let Some(port) = options
            .surface
            .clone()
            .or_else(|| daemon.desk.core().surface_port().map(str::to_owned))
        {
            daemon.attach_midi_surface(&port);
        }
        // Said once here as well as every half second in the run loop, so a
        // client that asks `Query::MidiPorts` before the first housekeeping
        // tick is told what is open rather than *nothing*.
        daemon
            .desk
            .set_open_surface(daemon.surface.as_ref().and_then(SurfaceLink::open_name));
        Ok(daemon)
    }

    /// Opens a real MIDI port and attaches it — **S36**.
    ///
    /// **A port that is not there is a warning and a daemon that starts**, which
    /// is the exit criterion and not a kindness: a desk switched off half an
    /// hour before a show must not be the reason the show cannot be run, and the
    /// same daemon picks the desk up the moment somebody plugs it in, without a
    /// restart. That is `crate::surface::SurfaceLink::follow_the_cable`, and
    /// `prism_midi` does the retrying underneath it.
    pub fn attach_midi_surface(&mut self, port: &str) {
        let port = crate::surface::RealSurfacePort::attach(port);
        if let Some(why) = port.why() {
            log::warn(
                "surface",
                &format!(
                    "the configured MIDI port {:?} is not open: {why}.                      The daemon is running and will take it when it appears",
                    port.configured()
                ),
            );
        }
        self.attach_surface(Box::new(port));
    }

    /// Puts the current surface down and takes up whatever the configuration now
    /// names — **S36**.
    ///
    /// Called from the run loop when a `SetSurfacePort` has been applied. A
    /// `--mock-surface` is left alone, because a run that asked for one asked
    /// for exactly it.
    fn follow_surface_change(&mut self, port: Option<String>) {
        match port {
            Some(port) => {
                log::info(
                    "surface",
                    &format!("the surface moves to MIDI port {port:?}"),
                );
                self.attach_midi_surface(&port);
            }
            None => {
                log::info(
                    "surface",
                    "no MIDI port is configured; the surface is detached",
                );
                self.surface = None;
            }
        }
    }

    /// Re-reads the X-Touch binding table and redraws the surface with it —
    /// S37.
    ///
    /// **Naming the profile again *is* the reload**, which is what the Devices
    /// panel's *reload* button sends: a table edited beside a running daemon is
    /// picked up by pointing at it a second time. There is no separate reload
    /// command, because a second way of saying one thing is a second thing to
    /// keep in step.
    ///
    /// A profile that is missing or malformed is reported and the **built-in
    /// table stands** — S22's rule, and the reason this cannot fail: a broken
    /// JSON file must never be the reason a desk stops answering its keys.
    fn follow_profile_change(&mut self, path: Option<&Path>) {
        self.profile = path.map(Path::to_path_buf);
        let table = match path {
            Some(path) => {
                log::info(
                    "surface",
                    &format!("reading the binding table from {}", path.display()),
                );
                // **A file that will not parse leaves the table in force exactly
                // where it was** - S38, and it is S22's rule met from a new
                // direction. S22 said a malformed profile falls back to the
                // built-in defaults, about a desk starting up with nothing else
                // to fall back on; a desk that has a table of its own has
                // something better than the defaults to keep, and throwing an
                // operator's edits away over a typo in a file would be worse
                // than ignoring the file. `load_profile` still cannot fail and
                // still warns.
                match crate::surface::read_profile(path) {
                    Some(table) => table,
                    None => return,
                }
            }
            None => {
                log::info("surface", "the built-in binding table is in force");
                Bindings::defaults()
            }
        };
        // Through the `Core`, because the table lives there since S38: an import
        // is a change to this machine's own table and is written down like any
        // other. The deltas are dropped rather than broadcast because this
        // arrives *from* a command whose deltas the caller is already sending.
        drop(self.desk.core().replace_bindings(table.clone()));
        self.bindings = table;
        // The table the attached surface is drawing with, replaced in place. The
        // whole picture follows, because a new table can mean a different
        // scribble strip on every one of the eight.
        if let Some(surface) = self.surface.take() {
            let port = surface.into_port();
            self.attach_surface(port);
        }
    }

    /// Puts a table the `Core` has just accepted under the attached surface - S38.
    ///
    /// [`Self::follow_profile_change`]'s last two steps on their own, for the
    /// ordinary case: an operator changed one control in the editor, the `Core`
    /// applied it, and the desk has to draw with it. The port is kept and the
    /// shadow model is thrown away, which is `into_port`'s whole argument.
    fn follow_binding_change(&mut self, bindings: Bindings) {
        self.bindings = bindings;
        if let Some(surface) = self.surface.take() {
            let port = surface.into_port();
            self.attach_surface(port);
        }
    }

    /// Attaches a control surface.
    ///
    /// Separate from [`start`](Self::start) because a port is a thing rather
    /// than a setting: `Options` is `Clone` and comparable, and a MIDI port is
    /// neither. It is also the seam `CLAUDE.md` asks for — the D11 gate attaches
    /// a mock and nothing in the suite touches a device.
    ///
    /// The surface is drawn once as soon as the daemon starts polling: attaching
    /// invalidates the shadow model, which is §5.3's resync burst and is paced
    /// like everything else.
    pub fn attach_surface(&mut self, port: Box<dyn SurfacePort>) {
        log::info(
            "surface",
            &format!(
                "a control surface is attached: {} controls bound",
                self.bindings.bound()
            ),
        );
        self.surface = Some(SurfaceLink::attach(port, self.bindings.clone()));
    }

    /// The attached surface, for a status panel or a test.
    #[must_use]
    pub const fn surface(&self) -> Option<&SurfaceLink> {
        self.surface.as_ref()
    }

    /// The binding table in force.
    #[must_use]
    pub const fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// The desk, for a test that wants to look at what the daemon holds.
    #[must_use]
    pub fn desk(&self) -> &Arc<Desk> {
        &self.desk
    }

    /// What every mock output has been given, in order.
    ///
    /// `ARCHITECTURE_SPEC.md` §12 runs the end-to-end tests "against a daemon in
    /// mock-output mode", and a mode nothing can read is not one. **S18's D2
    /// gate is what this exists for**: *assert the output frame sequence has no
    /// gap across the whole run, on captured frames rather than by watching*.
    #[must_use]
    pub fn recorded_outputs(&self) -> Vec<MockOutputHandle> {
        self.desk.core().outputs().recordings()
    }

    /// What **one** mock output has been given, by output number — S33.
    ///
    /// The worked example asks each driver what it was handed, so it has to be
    /// able to name one: `recorded_outputs` in a rig of five is five recordings
    /// in output-number order and no way to say *the sACN node*.
    #[must_use]
    pub fn recorded_output(&self, id: OutputId) -> Option<MockOutputHandle> {
        self.desk.core().outputs().recording(id)
    }

    /// The IPC server, for a caller that wants to know about the connections.
    ///
    /// Cheap to clone, and every clone is a handle onto the same set of clients
    /// — which is what lets S18's backpressure gate watch the queue counters of
    /// a client while the daemon it belongs to is running, and what a status
    /// panel (S27) will read one row per connection from. It is deliberately
    /// not a way to *serve* anything: the accept loops are the daemon's.
    #[must_use]
    pub const fn server(&self) -> &Server {
        &self.server
    }

    /// Where clients are told to find this daemon.
    #[must_use]
    pub fn endpoints(&self) -> String {
        self.lock.document().endpoints()
    }

    /// Runs until `stop` resolves, or until `run_for` has passed.
    ///
    /// Everything periodic is here rather than in tasks of its own: the
    /// autosave, the output health and the telemetry channel all read the same
    /// desk, and a loop that names its own cadences is easier to reason about
    /// than three timers that do not know about each other.
    pub async fn run(&mut self, run_for: Option<Duration>, stop: impl Future<Output = ()>) {
        let mut housekeeping = tokio::time::interval(HOUSEKEEPING);
        let mut telemetry = tokio::time::interval(TELEMETRY_PERIOD);
        // The surface is polled at the pacing floor rather than at a rate of its
        // own: the send pause is enforced against this loop's clock, so polling
        // less often would send less rather than the same amount later
        // (`docs/MCU_MAPPING.md` §5.2 and §2.7).
        let mut surface = tokio::time::interval(SURFACE_PERIOD);
        let mut playback = tokio::time::interval(PLAYBACK_PERIOD);
        let mut sequence = 0u64;
        let mut buffer = Vec::new();
        // Keyed by output number rather than by position, because the rig can
        // change under this loop now (S33): a `Vec` in step with a list that
        // grew and shrank would compare one output's health against another's.
        let mut health: std::collections::BTreeMap<OutputId, OutputHealth> = self
            .desk
            .core()
            .outputs()
            .entries()
            .into_iter()
            .map(|output| (output.id, output.status.health()))
            .collect();

        let deadline = run_for.map(|duration| tokio::time::Instant::now() + duration);
        // S29: the third way this loop ends, and the one §10.3 names first.
        // Taken out of the desk before the loop so the arm's future borrows an
        // `Arc` rather than `self`, which the other arms need mutably.
        let asked_to_stop = self.desk.stop_signal();
        tokio::pin!(stop);
        loop {
            tokio::select! {
                () = &mut stop => {
                    log::info("daemon", "stopping on request");
                    return;
                }
                // `Command::Shutdown` — the tray menu of `ARCHITECTURE_SPEC.md`
                // §10.3, and the only explicit instruction a daemon with no
                // console of its own could not be given until S29. What happens
                // next is `Daemon::shutdown`, which is the same path Ctrl-C
                // takes: clients told first, then the exit action, then the
                // drivers.
                () = asked_to_stop.notified() => {
                    log::info("daemon", "stopping because a client asked it to");
                    return;
                }
                () = async {
                    match deadline {
                        Some(deadline) => tokio::time::sleep_until(deadline).await,
                        // Never: a daemon without a deadline runs until it is
                        // told to stop, which is §10.3 in one line.
                        None => std::future::pending().await,
                    }
                } => {
                    log::info("daemon", "the time it was asked to run for has passed");
                    return;
                }
                _ = housekeeping.tick() => {
                    // **One lock for all three**, and that is a cost decision
                    // rather than tidiness: this arm runs on the same runtime as
                    // the surface's millisecond poll, on machines with two cores
                    // (a CI runner) as well as on a desk. Three acquisitions of
                    // the `Core` mutex to learn that nothing has changed is
                    // twice as much contention as one, for no answer.
                    let (change, profile, exit, table) = {
                        let mut core = self.desk.core();
                        (
                            // S36's other door, and it is here because the
                            // surface tick only runs when there **is** a
                            // surface: a desk with none has to be able to
                            // acquire one. Half a second rather than a
                            // millisecond is the right cost for a gesture
                            // nobody makes twice.
                            if self.surface.is_none() {
                                core.take_surface_change()
                            } else {
                                None
                            },
                            // S37's two settings that this loop rather than the
                            // core has to act on: the binding table a surface
                            // draws with, and what the stage does when the
                            // daemon stops. Both are picked up here for
                            // `surface_change`'s reason exactly — the thing they
                            // change is the daemon's.
                            core.take_profile_change(),
                            core.take_exit_change(),
                            // S38's, and it is here rather than on the surface
                            // tick because a binding is a gesture nobody makes
                            // twice a second - the same cost decision the three
                            // above it record.
                            core.take_binding_change(),
                        )
                    };
                    if let Some(port) = change {
                        self.follow_surface_change(port);
                    }
                    if let Some(path) = profile {
                        self.follow_profile_change(path.as_deref());
                    }
                    if let Some(action) = exit {
                        self.exit = Exit::from(action);
                    }
                    if let Some(table) = table {
                        self.follow_binding_change(table);
                    }
                    // What a settings window is told about the desk: the port
                    // that is actually open, refreshed on this cadence rather
                    // than on the surface's. Half a second is the right
                    // freshness for a fact that changes when somebody moves a
                    // plug, and it keeps the millisecond path free of a string.
                    self.desk
                        .set_open_surface(self.surface.as_ref().and_then(SurfaceLink::open_name));
                    // …and what it is *doing*, which is S37's Devices panel:
                    // the health, the counters, the reconnection count and the
                    // binding table, on the same cadence and for the same
                    // reason. `None` is *no surface at all*, which a panel draws
                    // differently from one that is attached and disconnected.
                    let profile = self.profile.clone();
                    self.desk.set_surface_status(
                        self.surface
                            .as_ref()
                            .map(|surface| surface.status(profile.as_deref())),
                    );
                    for delta in self.desk.poll_autosave() {
                        self.server.broadcast(delta).await;
                    }
                    for delta in self.output_health_changes(&mut health) {
                        self.server.broadcast(delta).await;
                    }
                }
                _ = surface.tick(), if self.surface.is_some() => {
                    // S36: a `SetSurfacePort` left a name behind. Taken before
                    // the poll, so the very next poll is the new port's and the
                    // resync burst starts a millisecond after the command
                    // rather than a frame after it.
                    let (change, table) = {
                        let mut core = self.desk.core();
                        (core.take_surface_change(), core.take_binding_change())
                    };
                    if let Some(port) = change {
                        self.follow_surface_change(port);
                    }
                    // S38, and it is on **this** tick rather than only on the
                    // housekeeping one for the port's reason exactly: a key
                    // rebound in the editor should do the new thing when the
                    // operator presses it, not up to half a second later. The
                    // housekeeping tick takes it too, because a daemon with no
                    // surface attached never runs this arm at all.
                    if let Some(table) = table {
                        self.follow_binding_change(table);
                    }
                    // A press becomes a command, the command reaches the
                    // daemon's own state, and the delta goes to whoever is
                    // attached — which may be nobody. That is D11.
                    let deltas = self
                        .surface
                        .as_mut()
                        .map(|surface| surface.poll(&self.desk))
                        .unwrap_or_default();
                    for delta in deltas {
                        self.server.broadcast(delta).await;
                    }
                }
                _ = playback.tick() => {
                    // The tick's own answer about what is running and which cue
                    // it is on — the channel S26 recorded as missing. Polled
                    // rather than pushed because the tick may not allocate,
                    // lock or block (`ARCHITECTURE_SPEC.md` §3.1), and silent
                    // unless something changed.
                    let deltas = self.desk.core().poll_playback();
                    for delta in deltas {
                        self.server.broadcast(delta).await;
                    }
                }
                _ = telemetry.tick() => {
                    // Built only when somebody is listening: 32 KiB thirty times
                    // a second for nobody is the one cost a droppable channel
                    // has no excuse for.
                    //
                    // And **not before the engine has published a frame**. A
                    // triple buffer starts blank, so a client that connects in
                    // the moment between the listeners binding and the first
                    // tick would otherwise be shown a picture of a dark rig
                    // that is in fact lit — telemetry may be dropped (§7), but
                    // it may not be wrong. `published` latches on the first
                    // frame and is never cleared: after that the subscriber
                    // holds the last real one, which is what a driver on its
                    // own cadence re-sends rather than going dark.
                    if self.server.client_count().await > 0 && self.published() {
                        sequence = sequence.wrapping_add(1);
                        self.telemetry_frame(sequence).encode_into(&mut buffer);
                        self.server.telemetry(buffer.clone()).await;
                    }
                }
            }
        }
    }

    /// Whether the engine has ever published a frame into the telemetry
    /// subscriber.
    ///
    /// Latches: `FrameSubscriber::refresh` answers *was there a new one since
    /// last time*, which is false for a rig that has settled as well as for one
    /// that has not started, and those are opposite facts.
    fn published(&mut self) -> bool {
        if self.telemetry.refresh() {
            self.telemetry_started = true;
        }
        self.telemetry_started
    }

    /// The current levels of the universes the show patches.
    fn telemetry_frame(&mut self, sequence: u64) -> TelemetryFrame {
        self.telemetry.refresh();
        let frame = self.telemetry.frame();
        let universes = self
            .desk
            .core()
            .patched_universes()
            .into_iter()
            .filter_map(|universe| {
                let position = self.layout.index_of(universe)?;
                let levels = frame.universe(position)?;
                Some(prism_ipc::telemetry::UniverseLevels {
                    universe,
                    levels: <[u8; 512]>::try_from(levels).ok()?,
                })
            })
            .collect();
        TelemetryFrame {
            sequence,
            universes,
        }
    }

    /// Outputs whose status light has changed since the last look.
    ///
    /// **Keyed by output number since S33**, because the rig changes while the
    /// daemon runs: an output that has gone is dropped from the record, one that
    /// has arrived is reported the first time it says anything, and a position
    /// in a list is no longer a stable name for an interface.
    fn output_health_changes(
        &self,
        seen: &mut std::collections::BTreeMap<OutputId, OutputHealth>,
    ) -> Vec<Delta> {
        let mut deltas = Vec::new();
        let core = self.desk.core();
        let entries = core.outputs().entries();
        for output in &entries {
            // **The reported health, not the driver's** — S46. An Art-Net
            // output whose nodes have stopped answering is `Degraded` here as
            // well as in the settings panel, because a delta and an answer
            // saying different things about one output is worse than either of
            // them being wrong.
            let health = core.outputs().reported_health(output.id);
            if seen.insert(output.id, health) != Some(health) {
                log::info("output", &format!("{} is {health:?}", output.name));
                deltas.push(Delta::OutputHealth {
                    output_id: output.id,
                    health,
                });
            }
        }
        // An output that is no longer there stops being reported about. It is
        // not a health change — a client learns it has gone from
        // `Delta::OutputsChanged`, which is the fact rather than a symptom.
        seen.retain(|id, _| entries.iter().any(|output| output.id == *id));
        drop(core);
        deltas
    }

    /// Stops everything, in the order §10.3 asks for.
    pub async fn shutdown(mut self) {
        // Clients are told first, so they show *the daemon stopped* rather than
        // *the connection broke*.
        self.server.shutdown().await;
        for listener in self.listeners.drain(..) {
            listener.abort();
        }

        if self.exit == Exit::Blackout {
            // A frame, not a flag: the drivers end their streams carrying the
            // last look, so the last look has to be a blackout before they are
            // asked to stop.
            log::info("daemon", "blacking the stage out before stopping");
            self.desk.core().set_blackout(true);
            tokio::time::sleep(BLACKOUT_SETTLE).await;
        }

        // `OutputThread::stop` runs `DmxOutput::shutdown`, which is where sACN
        // ends its streams with three `Stream_Terminated` packets per universe
        // (S10). Before the engine, so the last frame is one the outputs sent.
        self.desk.core().stop_outputs();
        log::info("daemon", "stopped");
        // The lock goes last, as it was taken first: it is dropped with `self`,
        // which removes the discovery file and releases the guard.
    }
}

/// Starts reading the profile directories on a thread of its own.
///
/// See [`Daemon::start`] for why: the read is hundreds of milliseconds and
/// nothing before the first DMX frame needs it. `std::thread` rather than a
/// runtime task because it is a blocking file walk — `ARCHITECTURE_SPEC.md` §3
/// keeps `core-main`'s executor for work that yields.
fn spawn_library_load(
    data_dir: &Path,
    configured: Option<PathBuf>,
) -> std::thread::JoinHandle<prism_core::FixtureLibrary> {
    let data_dir = data_dir.to_path_buf();
    std::thread::spawn(move || load_library(&data_dir, configured.as_deref()))
}

/// The library that thread read.
///
/// A thread that panicked leaves this desk with the built-in profiles rather
/// than stopping it: `CLAUDE.md`'s zero-crash invariant applies to a fixture
/// menu as much as to anything else, and a desk with four profiles is one an
/// operator can still patch a dimmer into.
fn join_library_load(
    loading: std::thread::JoinHandle<prism_core::FixtureLibrary>,
) -> prism_core::FixtureLibrary {
    loading.join().unwrap_or_else(|_| {
        log::error(
            "library",
            "reading the fixture library panicked - the built-in profiles are all that is offered",
        );
        let mut library = prism_core::FixtureLibrary::default();
        for profile in prism_core::generic_profiles() {
            library.insert_profile(profile);
        }
        library
    })
}

/// The profiles this desk can embed — S44.
///
/// Three sources, and the **order is the whole of the override rule**: the
/// operator's own folder first, then the installed Open Fixture Library, then
/// the four built-in generics. `FixtureLibrary` keeps the first profile it is
/// given for a key, so a file in the data directory wins over one the installer
/// wrote, and both win over a generic.
///
/// Nothing here fails. A library that is not installed is an ordinary state —
/// the desk starts with four profiles and says so — because a lighting desk
/// that would not start over a missing directory is a worse answer than one
/// with four profiles in it.
///
/// Public since S51 so that `tests/fixture_install.rs` can put the two
/// directories side by side, run the real installer over one of them and read
/// the library the daemon would have got — which is punch-list entry B43's
/// exit criterion, and not something a test of `FixtureLibrary` alone can say.
pub fn load_library(data_dir: &Path, configured: Option<&Path>) -> prism_core::FixtureLibrary {
    let mut library = prism_core::FixtureLibrary::default();

    // **The venue's own, first** — punch-list entry B43. First because
    // `FixtureLibrary` keeps the first profile it is given for a key, which is
    // what makes a file here a *correction* of a vendored one rather than a
    // second entry beside it.
    let own = paths::fixtures_dir(data_dir);
    if own.is_dir() {
        library.read_own_tree(&own);
    }

    let installed = configured
        .map(Path::to_path_buf)
        .or_else(paths::installed_library_dir);
    match installed {
        Some(root) => {
            library.read_ofl_tree(&root);
            let counts = library.conversion();
            log::info(
                "library",
                &format!(
                    "{} profiles from {} ({} fixtures, {} modes skipped, {} redirects)",
                    library.len(),
                    root.display(),
                    counts.fixtures,
                    counts.modes_with_inserts,
                    counts.redirects,
                ),
            );
        }
        None => log::warn(
            "library",
            "no fixture library is installed - run tools/fetch-fixtures to install the              Open Fixture Library; the built-in generic profiles are all that is offered",
        ),
    }

    // Last, so a library profile keyed `generic.dimmer` would win over the
    // built-in one rather than the other way round.
    for profile in prism_core::generic_profiles() {
        library.insert_profile(profile);
    }
    log::info("library", &format!("{} profiles offered", library.len()));
    library
}

/// Opens the show file, creating an empty show if there is not one there.
///
/// A file rather than a show held in memory, because a daemon that cannot save
/// what an operator built is a daemon that loses it.
fn open_show(path: &Path) -> Result<(ShowFile, ShowStore), StartError> {
    let store = ShowStore::open(path)?;
    let mut file = ShowFile::new();
    let effects = store.load(&mut file)?;
    if effects.is_empty() {
        log::info("show", &format!("{} is a new show", path.display()));
    } else {
        log::info(
            "show",
            &format!(
                "opened {} - {} fixtures, {} sequences",
                path.display(),
                file.show.fixtures().count(),
                file.show.sequences().count()
            ),
        );
    }
    if store.has_recovery() {
        // An autosave copy that outlived its show is unsaved work somebody
        // should be offered rather than something to clear away quietly (S15).
        log::warn(
            "show",
            &format!(
                "a recovery copy from an earlier run is beside this show: {}",
                store.recovery_path().display()
            ),
        );
    }
    Ok((file, store))
}

/// What an sACN receiver shows beside the source: the show's own file name.
fn source_name(show: &Path) -> String {
    show.file_stem().map_or_else(
        || "PrismDMX".to_owned(),
        |stem| format!("PrismDMX {}", stem.to_string_lossy()),
    )
}

/// A label that distinguishes this daemon's endpoint from another user's on the
/// same machine.
///
/// Derived from the data directory, so a client that knows where the daemon
/// keeps its files can work out the address without reading anything — and so
/// that two accounts on one machine do not ask for the same pipe name. A short
/// hash rather than the path itself, because a pipe name is not a path and a
/// Unix socket path is length-limited.
fn label_for(data_dir: &Path) -> String {
    // FNV-1a. Not a cryptographic question: this distinguishes two directories
    // on one machine, and a collision costs one of two daemons a startup error
    // rather than anything silent.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in data_dir.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The binding table this run starts with, and whether to write it down - S38.
///
/// Four sources in order, and the order **is** S38's storage decision:
///
/// 1. **`--surface-profile <path>`**, which is the table for that run. S33's
///    rule for `--mock-output` and S36's for `--surface`: a flag names the value,
///    the stored one is neither read nor written, and `SurfaceBinding` is refused
///    with that flag named.
/// 2. **This machine's own table**, out of `machine.json`. A desk that has been
///    edited starts as it was left, which is the whole point of an editor.
/// 3. **The profile file the settings name**, for a desk that has a path and has
///    never been edited - the state every desk is in the first time it runs.
/// 4. **The built-in defaults** of `docs/MCU_MAPPING.md` section 4.1.
///
/// Two and three are in that order and not the other way round, and that is the
/// decision worth arguing. A file that won at every start would mean an operator
/// who rebound a key at the desk found it back the way it was the next morning -
/// so a file is an **import**: naming one reads it into this machine's table
/// (`Core::replace_bindings`) and `Settings::surface_profile` is the record of
/// where the table came from rather than where it lives.
///
/// The second half of the answer is whether to write the table down as it
/// stands. A desk that has never been told adopts what it started with, so that
/// the first edit is a change to a table rather than the creation of one, and so
/// that `machine.json` says what the keys do before anybody has touched them.
fn starting_bindings(
    options: &Options,
    machine: &MachineConfig,
    profile: Option<&Path>,
) -> (Bindings, bool) {
    if let Some(path) = &options.surface_profile {
        return (crate::surface::load_profile(path), false);
    }
    if let Some(rows) = machine.surface_bindings() {
        let (table, problem) = Bindings::from_rows(rows, &prism_surface::X_TOUCH);
        if let Some(error) = problem {
            log::warn(
                "surface",
                &format!(
                    "this desk's stored binding table was not used: {error}.                      The built-in bindings are in force"
                ),
            );
        }
        return (table, false);
    }
    match profile {
        Some(path) => (crate::surface::load_profile(path), true),
        None => (Bindings::defaults(), true),
    }
}

/// Which rig this run uses, and whether it is this machine's to change — S33.
///
/// Two answers, and they go together:
///
/// - **Nothing on the command line**: the rig is `MachineConfig::outputs`, the
///   configuration is this desk's, and `AddOutput` and its three companions edit
///   it and write it back. This is what a venue runs.
/// - **Outputs on the command line**: those are the whole rig for this run, the
///   stored configuration is not used and not touched, and the four commands are
///   refused (`MachineError::ConfiguredOnTheCommandLine`).
///
/// **Why not merge the two.** A flag is what a test and a bring-up use — every
/// target in this repository starts a daemon with `--mock-output` — and a run
/// like that must neither inherit a hall's cabling nor write a mock output into
/// it. Adding to the stored rig would do the first; writing back would do the
/// second; and letting the commands edit a rig nothing saves would be an
/// `AddOutput` that appeared to work and vanished at the next restart.
///
/// The universes are filled in here because a flag has nowhere to put a list:
/// `--open-dmx 3` names its own and everything else carries the show's. See
/// [`crate::cli::fill_universes`].
///
/// Every row is validated on the way in, whichever side it came from, and one
/// that is refused is **reported and skipped** rather than stopping the daemon —
/// a hand-edited `machine.json` with one bad hop limit in it must not be a desk
/// that will not start half an hour before a show.
fn rig_for(
    options: &Options,
    machine: &MachineConfig,
    file: &ShowFile,
    data_dir: &Path,
) -> (Vec<OutputInstance>, Option<PathBuf>) {
    if options.outputs.is_empty() {
        let rig = validated(machine.outputs().to_vec());
        return (rig, Some(paths::machine_config_path(data_dir)));
    }
    let rig = crate::cli::fill_universes(&options.outputs, &show_universes(file));
    log::info(
        "output",
        &format!(
            "{} output(s) named on the command line: this machine's own configuration \
             is neither read nor written this run",
            rig.len()
        ),
    );
    (validated(rig), None)
}

/// Keeps the rows that are usable and says why about the ones that are not.
fn validated(rig: Vec<OutputInstance>) -> Vec<OutputInstance> {
    rig.into_iter()
        .filter(|output| match prism_core::validate_output(output) {
            Ok(()) => true,
            Err(error) => {
                log::error(
                    "output",
                    &format!("output {} is not usable: {error}", output.id),
                );
                false
            }
        })
        .collect()
}

/// The universes an output carries: the ones the show patches, or universe 1 if
/// the show patches nothing.
///
/// A fresh show has no fixtures, and an output carrying no universes would send
/// nothing at all — which looks exactly like a broken cable to whoever is
/// setting the rig up.
fn show_universes(file: &ShowFile) -> Vec<UniverseId> {
    let universes = file.show.universes();
    if universes.is_empty() {
        return vec![UniverseId::MIN];
    }
    universes
}

/// Where the daemon keeps its files, for a caller that wants to report it.
#[must_use]
pub fn data_dir_of(options: &Options) -> Option<PathBuf> {
    options
        .data_dir
        .clone()
        .or_else(|| paths::system_data_dir().ok())
}

#[cfg(test)]
mod tests {
    use super::{
        Daemon, StartError, data_dir_of, label_for, open_show, show_universes, source_name,
    };
    use crate::testkit::show_file;
    use prism_domain::UniverseId;
    use std::path::Path;

    #[test]
    fn a_label_distinguishes_two_data_directories_and_is_stable() {
        assert_eq!(
            label_for(Path::new("/home/a/.local/share/PrismDMX")),
            label_for(Path::new("/home/a/.local/share/PrismDMX"))
        );
        assert_ne!(
            label_for(Path::new("/home/a/.local/share/PrismDMX")),
            label_for(Path::new("/home/b/.local/share/PrismDMX"))
        );
        assert_eq!(label_for(Path::new("/a")).len(), 16, "hexadecimal, padded");
    }

    #[test]
    fn the_sacn_source_name_is_the_shows_own_name() {
        assert_eq!(source_name(Path::new("/shows/aula.prism")), "PrismDMX aula");
        assert_eq!(source_name(Path::new("")), "PrismDMX");
    }

    #[test]
    fn an_output_on_a_show_with_nothing_patched_still_carries_a_universe() {
        assert_eq!(
            show_universes(&show_file()),
            vec![UniverseId::new(1)],
            "the test rig is all in universe 1"
        );
        assert_eq!(
            show_universes(&prism_core::ShowFile::new()),
            vec![UniverseId::MIN],
            "a fresh show would otherwise put a driver on no universe at all"
        );
    }

    #[tokio::test]
    async fn a_daemon_says_what_it_is_at_a_breakpoint() {
        let dir = tempfile::tempdir().unwrap();
        let daemon = Daemon::start(&crate::cli::Options {
            data_dir: Some(dir.path().to_path_buf()),
            universes: Some(1),
            outputs: vec![crate::cli::mock_output(1)],
            local: Some(false),
            log_level: Some(crate::log::Level::Warn),
            ..crate::cli::Options::default()
        })
        .await
        .unwrap();
        let described = format!("{daemon:?}");
        assert!(described.contains("Daemon"), "{described}");
        assert!(described.contains("outputs: 1"), "{described}");
        assert!(described.contains("Hold"), "{described}");
        daemon.shutdown().await;
    }

    #[test]
    fn a_show_file_that_is_not_there_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("aula.prism");
        let (file, store) = open_show(&path).unwrap();
        assert!(path.is_file());
        assert_eq!(file.show.fixtures().count(), 0);
        assert!(!store.has_recovery());
    }

    #[test]
    fn a_show_that_is_there_is_opened_with_everything_in_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("aula.prism");
        {
            let mut written = show_file();
            let mut store = prism_core::ShowStore::open(&path).unwrap();
            store.save(&mut written).unwrap();
        }
        let (file, _store) = open_show(&path).unwrap();
        assert_eq!(file.show.fixtures().count(), 3);
        assert_eq!(file.show.sequences().count(), 1);
        // S12's criterion, one layer up: reopening a show restores the console.
        assert_eq!(file.session.session().executor_page, 3);
        assert_eq!(file.session.session().command_line, "fixture 1 at full");
    }

    #[test]
    fn every_reason_a_daemon_cannot_start_says_what_happened() {
        // The one message somebody starting a daemon by hand sees, so each
        // variant has to name its own cause rather than "an error occurred".
        let reasons: Vec<(StartError, &str)> = vec![
            (
                StartError::from(std::io::Error::other("no room")),
                "no room",
            ),
            (
                StartError::NoDataDirectory(crate::paths::NoDataDirectory),
                "data directory",
            ),
            (
                StartError::Store(prism_core::StoreError::NotAShowFile { application_id: 0 }),
                "could not be opened",
            ),
            (
                StartError::Patch(prism_engine::PatchError::PlanMismatch {
                    plan: 1,
                    channels: 2,
                }),
                "could not be played",
            ),
            (
                StartError::Layout(prism_engine::LayoutError::Empty),
                "frame layout",
            ),
            (
                StartError::from(crate::lock::LockError::AlreadyRunning(Box::default())),
                "already running",
            ),
        ];
        for (error, expected) in reasons {
            let message = error.to_string();
            assert!(message.contains(expected), "{message}");
            let as_error: &dyn core::error::Error = &error;
            assert!(!as_error.to_string().is_empty());
        }
    }

    #[test]
    fn a_daemon_without_a_named_directory_asks_the_machine_where_to_keep_its_files() {
        use crate::cli::Options;

        let named = Options {
            data_dir: Some(std::path::PathBuf::from("/srv/prism")),
            ..Options::default()
        };
        assert_eq!(
            data_dir_of(&named),
            Some(std::path::PathBuf::from("/srv/prism"))
        );
        // And with none named, whatever this machine says — which is a path
        // rather than a particular one, because a Windows runner and a Linux
        // one answer differently and both are right.
        assert!(data_dir_of(&Options::default()).is_some());
    }

    #[test]
    fn a_show_that_is_not_a_show_is_refused_rather_than_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-show.prism");
        std::fs::write(&path, b"this is not a database").unwrap();
        let error = open_show(&path).unwrap_err();
        assert!(!error.to_string().is_empty());
        assert!(path.is_file(), "and the operator's file is still there");
    }
}
