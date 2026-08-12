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

use prism_core::{ShowFile, ShowStore};
use prism_domain::{Delta, OutputHealth, OutputId, UniverseId};
use prism_engine::{FrameLayout, FramePublisher, FrameSubscriber};
use prism_ipc::{
    Server, ServerConfig, TelemetryFrame, local::LocalListener, websocket::WebSocketListener,
};
use prism_protocols::{
    ArtNetConfig, ArtNetOutput, Destination, DmxOutput, MockOutput, MockOutputHandle, OpenDmxUsb,
    OutputThread, RunnerConfig, SacnConfig, SacnDestination, SacnOutput, SystemUdp, spawn,
    system_backend,
};

use crate::cli::{Exit, Options, OutputSpec};
use crate::core::{Core, build_body};
use crate::engine::EngineThread;
use crate::lock::{DaemonLock, LockError};
use crate::log;
use crate::paths;
use crate::server::{Desk, DeskHandler, OutputEntry};

/// How often the daemon looks at its own housekeeping: the autosave policy and
/// the outputs' health.
const HOUSEKEEPING: Duration = Duration::from_millis(500);

/// `docs/IPC_PROTOCOL.md` §7: 25 to 30 Hz, independent of the 44 Hz tick.
const TELEMETRY_PERIOD: Duration = Duration::from_millis(33);

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
    outputs: Vec<OutputThread>,
    recordings: Vec<MockOutputHandle>,
    telemetry: FrameSubscriber,
    layout: Arc<FrameLayout>,
    lock: DaemonLock,
    exit: Exit,
    listeners: Vec<tokio::task::JoinHandle<()>>,
}

impl core::fmt::Debug for Daemon {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Daemon")
            .field("endpoints", &self.endpoints())
            .field("outputs", &self.outputs.len())
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
        log::set_level(options.log_level);

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

        // 3. The show, the engine, the outputs.
        let show_path = options
            .show
            .clone()
            .unwrap_or_else(|| paths::default_show_path(&data_dir));
        let (file, store) = open_show(&show_path)?;
        let layout =
            Arc::new(crate::engine::frame_layout(options.universes).map_err(StartError::Layout)?);
        let body = build_body(&layout, &file).map_err(StartError::Patch)?;

        let mut publisher = FramePublisher::new(Arc::clone(&layout));
        let source_name = source_name(&show_path);
        let opened = open_outputs(
            &options.outputs,
            &mut publisher,
            &machine,
            &source_name,
            &file,
        );
        // The telemetry channel is a subscriber like any driver, so what a
        // client sees is the frame the fixtures got rather than a second
        // calculation of it. Subscribed last, and still before the tick starts.
        let telemetry = crate::engine::telemetry_subscriber(&mut publisher);

        let output_count = opened.threads.len();
        let entries = opened.entries.clone();
        let engine = EngineThread::start(body, publisher)?;
        let core =
            Core::new(file, store, engine, Arc::clone(&layout)).map_err(StartError::Patch)?;
        let desk = Arc::new(Desk::new(core, entries));

        // 4. The listeners, and only then their addresses.
        let server = Server::with_config(
            DeskHandler::new(Arc::clone(&desk)),
            ServerConfig {
                token: options.token.clone(),
                ..ServerConfig::default()
            },
        );
        let mut listeners = Vec::new();
        if options.local {
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
        if let Some(address) = options.websocket {
            let mut listener = WebSocketListener::bind(address).await?;
            let endpoint = listener.endpoint();
            lock.publish(&endpoint)?;
            if let Some(token) = &options.token {
                lock.publish_token(token)?;
            }
            log::info("ipc", &format!("listening on {endpoint}"));
            let accepting = server.clone();
            listeners.push(tokio::spawn(async move {
                while let Some(wire) = listener.accept().await {
                    drop(accepting.spawn(wire));
                }
            }));
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

        Ok(Self {
            desk,
            server,
            outputs: opened.threads,
            recordings: opened.recordings,
            telemetry,
            layout,
            lock,
            exit: options.exit,
            listeners,
        })
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
    pub fn recorded_outputs(&self) -> &[MockOutputHandle] {
        &self.recordings
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
        let mut sequence = 0u64;
        let mut buffer = Vec::new();
        let mut health: Vec<OutputHealth> = self
            .desk
            .outputs()
            .iter()
            .map(|output| output.status.health())
            .collect();

        let deadline = run_for.map(|duration| tokio::time::Instant::now() + duration);
        tokio::pin!(stop);
        loop {
            tokio::select! {
                () = &mut stop => {
                    log::info("daemon", "stopping on request");
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
                    for delta in self.desk.poll_autosave() {
                        self.server.broadcast(delta).await;
                    }
                    for delta in self.output_health_changes(&mut health) {
                        self.server.broadcast(delta).await;
                    }
                }
                _ = telemetry.tick() => {
                    // Built only when somebody is listening: 32 KiB thirty times
                    // a second for nobody is the one cost a droppable channel
                    // has no excuse for.
                    if self.server.client_count().await > 0 {
                        sequence = sequence.wrapping_add(1);
                        self.telemetry_frame(sequence).encode_into(&mut buffer);
                        self.server.telemetry(buffer.clone()).await;
                    }
                }
            }
        }
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
    fn output_health_changes(&self, seen: &mut [OutputHealth]) -> Vec<Delta> {
        let mut deltas = Vec::new();
        for (output, seen) in self.desk.outputs().iter().zip(seen.iter_mut()) {
            let health = output.status.health();
            if health != *seen {
                *seen = health;
                log::info("output", &format!("{} is {health:?}", output.name));
                deltas.push(Delta::OutputHealth {
                    output_id: output.id,
                    health,
                });
            }
        }
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
        for output in self.outputs.drain(..) {
            output.stop();
        }
        log::info("daemon", "stopped");
        // The lock goes last, as it was taken first: it is dropped with `self`,
        // which removes the discovery file and releases the guard.
    }
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

/// Opens every output the command line asked for and puts each on its own
/// thread.
///
/// An output that cannot be built is reported and skipped rather than stopping
/// the daemon: a missing USB adapter must not take the Art-Net rig with it, and
/// `OutputRunner` is written so that a driver that cannot connect retries with
/// backoff for as long as the show runs.
fn open_outputs(
    specs: &[OutputSpec],
    publisher: &mut FramePublisher,
    machine: &prism_core::MachineConfig,
    source_name: &str,
    file: &ShowFile,
) -> OpenOutputs {
    let mut open = OpenOutputs::default();
    let universes = show_universes(file);
    for (index, spec) in specs.iter().enumerate() {
        let id = OutputId::new(u32::try_from(index + 1).unwrap_or(u32::MAX));
        match spec {
            OutputSpec::Mock => {
                let output = MockOutput::new(id, universes.clone());
                open.recordings.push(output.handle());
                open.attach(
                    publisher,
                    id,
                    "Mock output".to_owned(),
                    output,
                    RunnerConfig::default(),
                );
            }
            OutputSpec::ArtNet { target } => open.attach(
                publisher,
                id,
                format!("Art-Net to {target}"),
                ArtNetOutput::new(
                    id,
                    universes.clone(),
                    SystemUdp::new(),
                    ArtNetConfig {
                        destination: Destination::Unicast(vec![*target]),
                        ..ArtNetConfig::default()
                    },
                ),
                RunnerConfig::default(),
            ),
            OutputSpec::Sacn { unicast } => open.attach(
                publisher,
                id,
                unicast.map_or_else(
                    || "sACN".to_owned(),
                    |target| format!("sACN unicast to {target}"),
                ),
                SacnOutput::new(
                    id,
                    universes.clone(),
                    SystemUdp::new(),
                    SacnConfig {
                        cid: crate::machine::cid_of(machine),
                        source_name: source_name.to_owned(),
                        destination: unicast.map_or(SacnDestination::Multicast, |target| {
                            SacnDestination::Unicast(vec![target])
                        }),
                        ..SacnConfig::default()
                    },
                ),
                RunnerConfig::default(),
            ),
            OutputSpec::OpenDmx { universe } => open.attach(
                publisher,
                id,
                format!("Open DMX USB on universe {universe}"),
                OpenDmxUsb::new(id, *universe, system_backend()),
                RunnerConfig::for_profile(&prism_protocols::SH_RS09B),
            ),
        }
    }
    open
}

/// The outputs opened so far.
///
/// A type rather than two locals because [`OpenOutputs::attach`] is generic over
/// the driver: `DmxOutput` is object-safe but a `Box<dyn DmxOutput>` is not
/// itself one, and `OutputRunner` wants the concrete type so that its
/// `catch_unwind` sits directly around the driver's own code.
#[derive(Default)]
struct OpenOutputs {
    entries: Vec<OutputEntry>,
    threads: Vec<OutputThread>,
    /// The recordings of the mock outputs among them — see
    /// [`Daemon::recorded_outputs`].
    recordings: Vec<MockOutputHandle>,
}

impl OpenOutputs {
    fn attach<O: DmxOutput + 'static>(
        &mut self,
        publisher: &mut FramePublisher,
        id: OutputId,
        name: String,
        output: O,
        config: RunnerConfig,
    ) {
        // Before the tick starts, always: `subscribe` allocates (S2).
        let subscriber = publisher.subscribe();
        match spawn(&format!("out-{}", id.get()), output, subscriber, config) {
            Ok(thread) => {
                log::info("output", &format!("{name} started"));
                self.entries.push(OutputEntry {
                    id,
                    name,
                    status: Arc::clone(thread.status()),
                });
                self.threads.push(thread);
            }
            Err(error) => log::error("output", &format!("{name} could not be started: {error}")),
        }
    }
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
            universes: 1,
            outputs: vec![crate::cli::OutputSpec::Mock],
            local: false,
            log_level: crate::log::Level::Warn,
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
