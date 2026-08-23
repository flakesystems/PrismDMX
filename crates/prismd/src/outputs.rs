//! The rig, and keeping the driver threads in step with it — S33.
//!
//! `prism_core::MachineConfig` says what this building's outputs *are*; this
//! module is what makes them so. [`OutputSupervisor::reconcile`] compares the
//! configured rig against the threads that are running and does the least it
//! can: it stops the ones that are gone or have changed, starts the ones that
//! are new or have changed, and **leaves the rest alone**.
//!
//! # Why the least it can, rather than a restart
//!
//! The exit criterion S33 has to meet is that adding, removing or re-addressing
//! an output while the show runs costs nothing to the outputs that did not
//! change — asserted on the recorded frames, the way S18 asserts the D2 gate.
//! Stopping everything and starting it again would fail that on five outputs to
//! change one, and for sACN it would be five streams terminated and five begun,
//! which a receiver sees as five sources going away.
//!
//! `OutputInstance::needs_restart` is the rule, and it is deliberately narrow:
//! the kind, the universes, the number and whether it is enabled. **A rename
//! costs nothing**, because an operator who typed a better name should not watch
//! their rig blink.
//!
//! # Why an output can join a running tick at all
//!
//! `prism_engine::FrameEnrolment`, which S33 added for this: the subscriber's
//! buffer is allocated here, on the core thread, and left behind one atomic flag
//! for the tick to pick up on its next publish. The tick allocates nothing, locks
//! nothing and waits for nothing — `ARCHITECTURE_SPEC.md` §3.1 — and
//! `prism-engine`'s allocation gate is what says so.
//!
//! # Why the driver is behind a factory
//!
//! `CLAUDE.md`: no test may touch a device, and this machine has a real
//! SH-RS09B attached that the suite must not open. [`OutputFactory`] is the seam
//! — [`SystemOutputs`] opens the real thing, [`MockDevices`] opens a
//! `MockOutput` carrying the same universes — so S33's worked example is
//! configured as two Open DMX adapters, an sACN node and two Art-Net nodes and
//! asserted frame by frame with nothing plugged in and nothing on the network.
//! It is the same seam `FtdiBackend` is one level down, and `--mock-devices` is
//! how an operator asks for it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use prism_domain::{Delta, OutputHealth, OutputId, OutputInstance, OutputKind, UniverseId};
use prism_engine::{FrameEnrolment, SubscriberId};
use prism_protocols::{
    ArtNetConfig, ArtNetOutput, Cid, Destination, DmxOutput, MockOutput, MockOutputHandle,
    OpenDmxUsb, OutputStatus, OutputThread, PortAddress, Priority, RunnerConfig, SacnConfig,
    SacnDestination, SacnOutput, SacnPort, SacnUniverse, SystemUdp, spawn, system_backend,
};

use crate::log;
use crate::server::OutputEntry;

/// **This machine**, as opposed to what it is playing — S33.
///
/// Three things that only ever travel together: what the rig *is*
/// (`prism_core::MachineConfig`, which also holds the desk identity), where that
/// is written down, and the driver threads keeping step with it. A `Core` that
/// took them as three arguments would let a caller hand it a supervisor running
/// one rig and a configuration describing another.
///
/// `path` is `None` for a daemon whose outputs were named on its command line:
/// the stored configuration is neither read nor written that run, and the four
/// output commands are refused rather than edited into a rig nothing saves. See
/// `prismd::daemon::rig_for`.
#[derive(Debug)]
pub struct Machine {
    /// The desk identity and the rig.
    pub config: prism_core::MachineConfig,
    /// Where the configuration is written, or `None` — see above.
    pub path: Option<std::path::PathBuf>,
    /// The driver threads.
    pub outputs: OutputSupervisor,
    /// The control surface's MIDI port was named with `--surface`, so it is not
    /// the machine configuration's to change either — S36.
    ///
    /// A **second** flag rather than a second meaning for `path`, because the
    /// two are genuinely independent: a daemon may perfectly well take its rig
    /// from `machine.json` and its surface from a command line while somebody
    /// is trying a desk out. `MachineError::SurfaceOnTheCommandLine` is what a
    /// `SetSurfacePort` is refused with, and it names the right flag.
    pub surface_on_command_line: bool,
    /// The X-Touch binding table was named with `--surface-profile`, so what its
    /// keys do is not the machine configuration's to change either — S38.
    ///
    /// A **third** flag rather than a third meaning for either of the other two,
    /// and for the reason that split them: `--surface-profile` and `--surface`
    /// are independent — a daemon may be given one and not the other — and an
    /// operator told the wrong flag would go looking in the wrong place.
    /// `MachineError::BindingsOnTheCommandLine` is what a `SurfaceBinding` is
    /// refused with, and it names the right one.
    pub profile_on_command_line: bool,
    /// Where the lock file, the machine configuration and the default show are
    /// — S37.
    ///
    /// Read and never written: a settings window draws it, and it cannot edit
    /// it, because the settings themselves are in this directory. A daemon told
    /// to move it would have to be told somewhere else, and
    /// `ARCHITECTURE_SPEC.md` §10.3 is where that decision is written down.
    pub data_dir: std::path::PathBuf,
    /// Which settings this run's command line is holding — S37.
    ///
    /// `crate::cli::resolve` decides it once, and it travels to a client in
    /// `MachineSettings::overrides` so that a panel can grey out the rows a flag
    /// is holding and name the flag. Fixed for the life of the process: a
    /// command line does not change while it is running.
    pub overrides: Vec<prism_domain::MachineOverride>,
    /// Where the WebSocket listener is **actually** bound, or `None` — S37.
    ///
    /// Configured and absent at the same time is an ordinary state, because a
    /// listener that could not bind is a warning and a daemon that starts. It is
    /// S36's `configured` and `open` for a MIDI port, one device along.
    pub websocket_open: Option<std::net::SocketAddr>,
}

/// What every driver on this machine needs and no row of the rig carries.
///
/// The desk's identity and the show's name are properties of *the daemon*, not
/// of one output: two sACN outputs on one desk are one source (S10), and a rig
/// row that carried its own CID would be a second desk identity waiting to
/// disagree with the first.
#[derive(Debug, Clone)]
pub struct OutputContext {
    /// This desk's sACN CID, from `prism_core::MachineConfig`.
    pub cid: Cid,
    /// What an sACN receiver shows beside the source: the show's file name.
    pub source_name: String,
}

/// Opens the driver for one row of the rig.
///
/// See the module documentation for why this is a trait. The two implementations
/// are [`SystemOutputs`] and [`MockDevices`], and nothing else should need a
/// third: a new *kind* of output is a new arm in both, which is two places
/// rather than one on purpose — the mock has to know what it is standing in for.
pub trait OutputFactory: Send + Sync {
    /// The driver for `output`, and how its thread should be paced.
    fn open(
        &self,
        output: &OutputInstance,
        context: &OutputContext,
    ) -> (Box<dyn DmxOutput>, RunnerConfig);

    /// The recording of a mock driver just opened, or `None` if the last one was
    /// not a mock.
    ///
    /// Answered *after* [`open`](Self::open) and about that same driver, which
    /// is why it is a second method rather than part of the return: a
    /// `Box<dyn DmxOutput>` cannot be asked whether it happens to be a mock.
    /// Required rather than defaulted, because a factory that forgot it would
    /// silently make `--mock-output` a mode nothing can read.
    fn last_recording(&self) -> Option<MockOutputHandle>;
}

/// The last mock driver a factory opened, so its recording can be answered for.
///
/// A slot rather than a return value because [`OutputFactory::open`] answers
/// with a `Box<dyn DmxOutput>`, and a boxed driver cannot be asked whether it
/// happens to be a mock. See [`OutputFactory::last_recording`].
#[derive(Default)]
struct LastRecording(std::sync::Mutex<Option<MockOutputHandle>>);

impl LastRecording {
    fn set(&self, handle: MockOutputHandle) {
        if let Ok(mut last) = self.0.lock() {
            *last = Some(handle);
        }
    }

    fn get(&self) -> Option<MockOutputHandle> {
        self.0.lock().ok().and_then(|last| last.clone())
    }
}

impl core::fmt::Debug for LastRecording {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("LastRecording")
    }
}

/// The real drivers: an FTDI cable, an Art-Net socket, an sACN socket.
///
/// `OutputKind::Mock` is a real driver here as much as any other — it is
/// `--mock-output`, `ARCHITECTURE_SPEC.md` §12's mode, and its recording is what
/// S18's D2 gate is asserted against — so this factory records one too.
#[derive(Debug, Default)]
pub struct SystemOutputs {
    last: LastRecording,
}

/// Every kind opened as a [`MockOutput`] carrying the same universes.
///
/// `--mock-devices`, and `ARCHITECTURE_SPEC.md` §12's "daemon in mock-output
/// mode" one step further along: the rig is the *real* configuration, with the
/// real universes and the real numbers, and only the last inch is a double.
#[derive(Debug, Default)]
pub struct MockDevices {
    last: LastRecording,
}

impl OutputFactory for SystemOutputs {
    fn open(
        &self,
        output: &OutputInstance,
        context: &OutputContext,
    ) -> (Box<dyn DmxOutput>, RunnerConfig) {
        match &output.kind {
            OutputKind::Mock => {
                let mock = MockOutput::new(output.id, output.universes.clone());
                self.last.set(mock.handle());
                (Box::new(mock), RunnerConfig::default())
            }
            OutputKind::OpenDmx { serial } => {
                // §7.1: exactly one universe per cable, which
                // `prism_core::outputs::validate` has already refused anything
                // else for. `MIN` rather than a panic if it ever were empty —
                // `CLAUDE.md`'s zero-crash invariant does not make exceptions.
                let universe = output.universes.first().copied().unwrap_or(UniverseId::MIN);
                let mut profile = prism_protocols::SH_RS09B;
                if let Some(serial) = serial {
                    profile.device = profile.device.with_serial(serial.clone());
                }
                let config = RunnerConfig::for_profile(&profile);
                (
                    Box::new(OpenDmxUsb::with_profile(
                        output.id,
                        universe,
                        system_backend(),
                        profile,
                    )),
                    config,
                )
            }
            OutputKind::ArtNet { nodes, sync, ports } => (
                Box::new(ArtNetOutput::with_ports(
                    output.id,
                    art_net_ports(output, ports),
                    SystemUdp::new(),
                    ArtNetConfig {
                        destination: Destination::Unicast(nodes.clone()),
                        sync: *sync,
                        ..ArtNetConfig::default()
                    },
                )),
                RunnerConfig::default(),
            ),
            OutputKind::Sacn {
                receivers,
                ttl,
                ports,
            } => (
                Box::new(SacnOutput::with_ports(
                    output.id,
                    sacn_ports(output, ports),
                    SystemUdp::new(),
                    SacnConfig {
                        cid: context.cid,
                        source_name: context.source_name.clone(),
                        destination: if receivers.is_empty() {
                            SacnDestination::Multicast
                        } else {
                            SacnDestination::Unicast(receivers.clone())
                        },
                        multicast_ttl: *ttl,
                        ..SacnConfig::default()
                    },
                )),
                RunnerConfig::default(),
            ),
        }
    }

    fn last_recording(&self) -> Option<MockOutputHandle> {
        self.last.get()
    }
}

impl OutputFactory for MockDevices {
    fn open(
        &self,
        output: &OutputInstance,
        _context: &OutputContext,
    ) -> (Box<dyn DmxOutput>, RunnerConfig) {
        let mock = MockOutput::new(output.id, output.universes.clone());
        self.last.set(mock.handle());
        // The engine's own cadence for every kind, deliberately: what a mock
        // stands in for is the *routing*, and an Open DMX adapter's 35 Hz would
        // only make a test wait longer to learn the same thing.
        (Box::new(mock), RunnerConfig::default())
    }

    fn last_recording(&self) -> Option<MockOutputHandle> {
        self.last.get()
    }
}

/// The universe → port-address rows an Art-Net driver is built from.
///
/// Every universe the output carries, in send order, with the row the
/// configuration gives it or `PortAddress::for_universe` where it gives none.
/// The default is stated once, here, rather than in the domain and again in the
/// driver.
fn art_net_ports(
    output: &OutputInstance,
    ports: &[prism_domain::ArtNetPort],
) -> Vec<(UniverseId, PortAddress)> {
    output
        .universes
        .iter()
        .map(|universe| {
            let address = ports
                .iter()
                .find(|port| port.universe == *universe)
                .and_then(|port| PortAddress::new(port.address()))
                .unwrap_or_else(|| PortAddress::for_universe(*universe));
            (*universe, address)
        })
        .collect()
}

/// The same for sACN: the E1.31 universe and the priority per desk universe.
fn sacn_ports(output: &OutputInstance, ports: &[prism_domain::SacnPort]) -> Vec<SacnPort> {
    output
        .universes
        .iter()
        .map(|universe| {
            let configured = ports.iter().find(|port| port.universe == *universe);
            let mut port = SacnPort::new(*universe);
            if let Some(row) = configured {
                if let Some(sacn) = SacnUniverse::new(row.sacn_universe) {
                    port = port.at_universe(sacn);
                }
                if let Some(priority) = Priority::new(row.priority) {
                    port = port.at_priority(priority);
                }
            }
            port
        })
        .collect()
}

/// One output that is currently running.
struct Running {
    /// The row it was built from — what [`OutputInstance::needs_restart`] is
    /// asked about.
    instance: OutputInstance,
    entry: OutputEntry,
    thread: OutputThread,
    subscriber: SubscriberId,
    recording: Option<MockOutputHandle>,
}

/// Keeps the driver threads in step with the configured rig.
pub struct OutputSupervisor {
    enrolment: FrameEnrolment,
    factory: Box<dyn OutputFactory>,
    context: OutputContext,
    running: Vec<Running>,
    /// When this supervisor was made — the origin every `OutputFault` age is
    /// measured against, so that a status panel says *four seconds ago* rather
    /// than a number from a clock the client does not have.
    started: Instant,
}

impl core::fmt::Debug for OutputSupervisor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OutputSupervisor")
            .field("running", &self.running.len())
            .finish_non_exhaustive()
    }
}

impl OutputSupervisor {
    /// A supervisor over a publisher's enrolment, with nothing running yet.
    #[must_use]
    pub fn new(
        enrolment: FrameEnrolment,
        factory: Box<dyn OutputFactory>,
        context: OutputContext,
    ) -> Self {
        Self {
            enrolment,
            factory,
            context,
            running: Vec::new(),
            started: Instant::now(),
        }
    }

    /// Brings the running drivers into line with `rig`.
    ///
    /// Returns what to broadcast: nothing today beyond what the applier already
    /// said, because health is polled rather than pushed (S18) — the return is
    /// there so the daemon's call sites read the same as every other one and so
    /// a later session has somewhere to put a notice.
    ///
    /// **Stops before it starts**, which is what keeps the enrolment's places
    /// within its bound when an output is re-addressed: the place its old
    /// subscriber held is given back before the new one asks for one.
    pub fn reconcile(&mut self, rig: &[OutputInstance]) -> Vec<Delta> {
        // 1. Everything that is gone, or has changed in a way a running driver
        //    cannot be told about. Named first and stopped afterwards, because
        //    stopping one joins a thread and gives an enrolment place back —
        //    neither of which can happen inside a closure holding `self`.
        let leaving: Vec<OutputId> = self
            .running
            .iter()
            .filter(|running| {
                !rig.iter()
                    .any(|wanted| !running.instance.needs_restart(wanted))
            })
            .map(|running| running.instance.id)
            .collect();
        for id in leaving {
            self.stop_one(id);
        }

        // 2. A rename costs nothing: the row moved, the driver did not.
        for wanted in rig {
            if let Some(running) = self
                .running
                .iter_mut()
                .find(|running| running.instance.id == wanted.id)
            {
                running.instance.clone_from(wanted);
                running.entry.name.clone_from(&wanted.name);
            }
        }

        // 3. Everything new, and everything that had to be restarted.
        for wanted in rig {
            if self
                .running
                .iter()
                .any(|running| running.instance.id == wanted.id)
            {
                continue;
            }
            if !wanted.enabled {
                // A disabled output has no thread and no socket. It keeps its
                // row, which is the whole point of the flag.
                continue;
            }
            self.start_one(wanted);
        }

        // 4. The buffers the tick gave up are freed here, on this thread.
        self.enrolment.collect();
        self.running
            .sort_by_key(|running| running.instance.id.get());
        Vec::new()
    }

    /// Starts one output on a thread of its own.
    fn start_one(&mut self, wanted: &OutputInstance) {
        let subscriber = match self.enrolment.subscribe() {
            Ok(subscriber) => subscriber,
            Err(error) => {
                log::error(
                    "output",
                    &format!("{} could not be started: {error}", wanted.name),
                );
                return;
            }
        };
        let id = subscriber.id();
        let (driver, config) = self.factory.open(wanted, &self.context);
        let recording = self.factory.last_recording();
        match spawn(
            &format!("out-{}", wanted.id.get()),
            driver,
            subscriber,
            config,
        ) {
            Ok(thread) => {
                log::info(
                    "output",
                    &format!(
                        "{} started: {} on universe(s) {}",
                        wanted.name,
                        wanted.kind.label(),
                        universe_list(&wanted.universes)
                    ),
                );
                self.running.push(Running {
                    entry: OutputEntry {
                        id: wanted.id,
                        name: wanted.name.clone(),
                        status: Arc::clone(thread.status()),
                    },
                    instance: wanted.clone(),
                    thread,
                    subscriber: id,
                    recording,
                });
            }
            Err(error) => {
                // The place the subscriber took has to go back, or a rig that
                // failed to start a thread would leak one of the sixty-four.
                self.enrolment.unsubscribe(id);
                log::error(
                    "output",
                    &format!("{} could not be started: {error}", wanted.name),
                );
            }
        }
    }

    /// Stops one output and gives its subscriber back.
    fn stop_one(&mut self, id: OutputId) {
        let Some(index) = self
            .running
            .iter()
            .position(|running| running.instance.id == id)
        else {
            return;
        };
        let running = self.running.remove(index);
        log::info("output", &format!("{} stopped", running.entry.name));
        // The enrolment first, so the tick stops copying into a buffer nobody
        // will read while the thread is still winding down; then the thread,
        // which runs `DmxOutput::shutdown` — where sACN ends its streams.
        self.enrolment.unsubscribe(running.subscriber);
        running.thread.stop();
    }

    /// Stops everything, in output-number order. The shutdown path.
    pub fn stop_all(&mut self) {
        for running in self.running.drain(..) {
            self.enrolment.unsubscribe(running.subscriber);
            running.thread.stop();
        }
        self.enrolment.collect();
    }

    /// One entry per running output, for the snapshot and the health poll.
    #[must_use]
    pub fn entries(&self) -> Vec<OutputEntry> {
        self.running
            .iter()
            .map(|running| running.entry.clone())
            .collect()
    }

    /// The universes the **running** drivers carry, once each and in order.
    ///
    /// Not the configured rig's: a row that was refused or whose thread would
    /// not start is configuration that is not carrying anything, and *where does
    /// the light actually go* is the question `ShowIssue::UniverseNotOutput`
    /// answers.
    #[must_use]
    pub fn carried_universes(&self) -> Vec<UniverseId> {
        let mut carried = Vec::new();
        for running in &self.running {
            for universe in &running.instance.universes {
                if !carried.contains(universe) {
                    carried.push(*universe);
                }
            }
        }
        carried.sort_unstable();
        carried
    }

    /// The rows the running drivers were built from.
    #[must_use]
    pub fn instances(&self) -> Vec<OutputInstance> {
        self.running
            .iter()
            .map(|running| running.instance.clone())
            .collect()
    }

    /// The status of one running output.
    #[must_use]
    pub fn status(&self, id: OutputId) -> Option<&Arc<OutputStatus>> {
        self.running
            .iter()
            .find(|running| running.instance.id == id)
            .map(|running| &running.entry.status)
    }

    /// The health of one output as a status light shows it.
    ///
    /// A **disabled** output has no thread, so it is `Disconnected` — which is
    /// the truth: nothing is reaching those fixtures.
    #[must_use]
    pub fn health(&self, id: OutputId) -> OutputHealth {
        self.status(id)
            .map_or(OutputHealth::Disconnected, |status| status.health())
    }

    /// How long this supervisor has been running — what an `OutputFault` age is
    /// measured against.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// The recordings of the mock drivers among the running outputs, in
    /// output-number order.
    ///
    /// `ARCHITECTURE_SPEC.md` §12's mock-output mode, and what S18's D2 gate and
    /// S33's worked example are both asserted against.
    #[must_use]
    pub fn recordings(&self) -> Vec<MockOutputHandle> {
        self.running
            .iter()
            .filter_map(|running| running.recording.clone())
            .collect()
    }

    /// The recording of one output by number — S33's worked example asks each
    /// driver what it was given, so it has to be able to name one.
    #[must_use]
    pub fn recording(&self, id: OutputId) -> Option<MockOutputHandle> {
        self.running
            .iter()
            .find(|running| running.instance.id == id)
            .and_then(|running| running.recording.clone())
    }
}

/// `1, 2, 5` — for a log line an installer reads.
fn universe_list(universes: &[UniverseId]) -> String {
    universes
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::{
        MockDevices, OutputContext, OutputFactory as _, OutputSupervisor, SystemOutputs,
        art_net_ports, sacn_ports, universe_list,
    };
    use prism_domain::{
        ArtNetPort, OutputHealth, OutputId, OutputInstance, OutputKind, SacnPort, UniverseId,
    };
    use prism_engine::{FrameLayout, FramePublisher};
    use prism_protocols::{DmxOutput as _, PortAddress};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn universe(id: u32) -> UniverseId {
        UniverseId::new(id)
    }

    fn context() -> OutputContext {
        OutputContext {
            cid: prism_protocols::Cid::from_u128(1),
            source_name: "PrismDMX test".to_owned(),
        }
    }

    fn mock(id: u32, universes: &[u32]) -> OutputInstance {
        OutputInstance::new(
            OutputId::new(id),
            format!("Output {id}"),
            OutputKind::Mock,
            universes.iter().copied().map(UniverseId::new),
        )
    }

    fn publisher() -> FramePublisher {
        let layout = FrameLayout::new((1..=12).map(UniverseId::new)).unwrap();
        FramePublisher::new(Arc::new(layout))
    }

    fn supervisor(publisher: &FramePublisher) -> OutputSupervisor {
        OutputSupervisor::new(
            publisher.enrolment(),
            Box::new(MockDevices::default()),
            context(),
        )
    }

    /// The two paths a rig meets when it asks for more than there is: the
    /// enrolment's bound, and a number nothing is running under.
    #[test]
    fn an_output_that_cannot_be_given_a_place_is_reported_and_skipped() {
        let publisher = publisher();
        let enrolment = publisher.enrolment();
        let mut supervisor = supervisor(&publisher);

        // Every place taken by something else — the telemetry channel and
        // sixty-three outputs, say.
        let held: Vec<_> = (0..prism_engine::MAX_SUBSCRIBERS)
            .map(|_| enrolment.subscribe().unwrap())
            .collect();
        supervisor.reconcile(&[mock(1, &[1])]);
        assert!(
            supervisor.entries().is_empty(),
            "an output with no place is skipped, not carried as a thread with no frames"
        );
        assert!(supervisor.recording(OutputId::new(1)).is_none());
        assert_eq!(
            supervisor.health(OutputId::new(1)),
            OutputHealth::Disconnected
        );
        drop(held);

        // And stopping one that is not running does nothing at all, which is
        // what makes a reconcile idempotent.
        supervisor.reconcile(&[]);
        supervisor.reconcile(&[]);
        assert!(supervisor.entries().is_empty());
        assert!(supervisor.instances().is_empty());
        assert!(supervisor.carried_universes().is_empty());
        assert!(supervisor.status(OutputId::new(1)).is_none());
        assert!(supervisor.elapsed() >= Duration::ZERO);
        assert!(format!("{:?}", MockDevices::default()).contains("MockDevices"));
        assert!(format!("{:?}", SystemOutputs::default()).contains("LastRecording"));
    }

    /// `--mock-output` is a real output kind, and its recording is what S18's
    /// D2 gate reads — so the **system** factory has to answer for it too.
    #[test]
    fn a_mock_output_is_recorded_whichever_factory_opened_it() {
        for factory in [
            Box::new(SystemOutputs::default()) as Box<dyn super::OutputFactory>,
            Box::new(MockDevices::default()),
        ] {
            let (driver, _) = factory.open(&mock(1, &[1]), &context());
            assert_eq!(driver.universes(), [universe(1)]);
            assert!(
                factory.last_recording().is_some(),
                "a mock output nothing can read is not mock-output mode"
            );
        }
    }

    /// Waits for `condition`, or gives up. Every wait in this file has a
    /// deadline, for S16's reason: a named failure in seconds is worth more
    /// than a job that stops.
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

    #[test]
    fn a_rig_starts_one_thread_per_output_and_stops_them_all() {
        let publisher = publisher();
        let mut supervisor = supervisor(&publisher);
        assert!(supervisor.entries().is_empty());

        supervisor.reconcile(&[mock(1, &[1]), mock(2, &[2])]);
        assert_eq!(supervisor.entries().len(), 2);
        assert_eq!(supervisor.recordings().len(), 2);
        assert!(format!("{supervisor:?}").contains("running: 2"));

        supervisor.stop_all();
        assert!(supervisor.entries().is_empty());
    }

    /// The rule that keeps a rename free — and the rule that makes a
    /// re-addressing a new thread.
    ///
    /// The old status is held as an **`Arc`** rather than as a bare pointer,
    /// and that is not tidiness: a restart drops the original, and an allocator
    /// is entirely free to hand the replacement the address it just freed —
    /// which is what happened on run **32580777886**, where `assert_ne!` on two
    /// pointers compared `0x1e635c4fc90` against itself and failed a rule the
    /// code had kept. Keeping the `Arc` alive across the reconcile makes the
    /// two allocations genuinely distinct, so pointer identity means what the
    /// assertion says it means (S36).
    #[test]
    fn a_rename_keeps_the_thread_and_a_re_addressing_replaces_it() {
        let publisher = publisher();
        let mut supervisor = supervisor(&publisher);
        supervisor.reconcile(&[mock(1, &[1])]);
        let held = Arc::clone(supervisor.status(OutputId::new(1)).unwrap());
        let first = Arc::as_ptr(&held);

        let renamed = OutputInstance {
            name: "Hall dimmers".to_owned(),
            ..mock(1, &[1])
        };
        supervisor.reconcile(&[renamed]);
        assert_eq!(
            Arc::as_ptr(supervisor.status(OutputId::new(1)).unwrap()),
            first,
            "a rename must not blink the rig"
        );
        assert_eq!(supervisor.entries()[0].name, "Hall dimmers");

        supervisor.reconcile(&[mock(1, &[4])]);
        assert_ne!(
            Arc::as_ptr(supervisor.status(OutputId::new(1)).unwrap()),
            first,
            "a universe change is a new driver on a new set of universes"
        );
        assert_eq!(supervisor.instances()[0].universes, vec![universe(4)]);
        // Held to the end: the moment this is dropped, the address is free to
        // be handed out again, and everything above it stops meaning anything.
        drop(held);
        supervisor.stop_all();
    }

    #[test]
    fn a_disabled_output_has_no_thread_and_keeps_its_row() {
        let publisher = publisher();
        let mut supervisor = supervisor(&publisher);
        let disabled = OutputInstance {
            enabled: false,
            ..mock(1, &[1])
        };
        supervisor.reconcile(&[disabled]);
        assert!(supervisor.entries().is_empty());
        assert_eq!(
            supervisor.health(OutputId::new(1)),
            OutputHealth::Disconnected,
            "nothing is reaching those fixtures, and saying otherwise would be a lie"
        );

        supervisor.reconcile(&[mock(1, &[1])]);
        assert_eq!(supervisor.entries().len(), 1);
        supervisor.stop_all();
    }

    /// The enrolment's places are given back when an output stops, and a rig
    /// reconfigured over and over must not run out of them.
    #[test]
    fn the_places_an_output_took_come_back_when_it_goes() {
        let publisher = publisher();
        let enrolment = publisher.enrolment();
        let mut supervisor = supervisor(&publisher);
        for round in 0..80u32 {
            supervisor.reconcile(&[mock(1, &[1 + round % 4])]);
            assert_eq!(enrolment.len(), 1, "round {round}");
        }
        supervisor.stop_all();
        assert_eq!(enrolment.len(), 0);
    }

    /// S33's worked example in miniature, at the level where the routing is
    /// decided: each driver is built carrying **exactly** the universes its row
    /// gave it.
    #[test]
    fn every_driver_is_built_carrying_exactly_the_universes_it_was_given() {
        let factory = MockDevices::default();
        let output = OutputInstance::new(
            OutputId::new(4),
            "Node",
            OutputKind::ArtNet {
                nodes: vec!["10.0.0.9:6454".parse().unwrap()],
                sync: false,
                ports: Vec::new(),
            },
            [universe(5), universe(6), universe(7), universe(8)],
        );
        let (driver, _) = factory.open(&output, &context());
        assert_eq!(
            driver.universes(),
            [universe(5), universe(6), universe(7), universe(8)]
        );
        assert_eq!(driver.id(), OutputId::new(4));
        assert!(factory.last_recording().is_some());
    }

    /// The real factory builds the four kinds and gives the Open DMX adapter the
    /// pacing its own profile asks for (§7.1's 35 Hz), rather than the engine's.
    #[test]
    fn the_system_factory_builds_every_kind_at_its_own_cadence() {
        let factory = SystemOutputs::default();
        let context = context();

        let (driver, config) = factory.open(&mock(1, &[1]), &context);
        assert_eq!(driver.universes(), [universe(1)]);
        assert_eq!(config, prism_protocols::RunnerConfig::default());

        let cable = OutputInstance::new(
            OutputId::new(2),
            "Cable",
            OutputKind::OpenDmx {
                serial: Some("B0037HIY".to_owned()),
            },
            [universe(2)],
        );
        let (driver, config) = factory.open(&cable, &context);
        assert_eq!(driver.universes(), [universe(2)]);
        assert_eq!(
            config,
            prism_protocols::RunnerConfig::for_profile(&prism_protocols::SH_RS09B),
            "an adapter is paced by its own profile, not by the engine"
        );

        let node = OutputInstance::new(
            OutputId::new(3),
            "Node",
            OutputKind::ArtNet {
                nodes: vec!["10.0.0.9:6454".parse().unwrap()],
                sync: true,
                ports: Vec::new(),
            },
            [universe(5), universe(6)],
        );
        let (driver, _) = factory.open(&node, &context);
        assert_eq!(driver.universes(), [universe(5), universe(6)]);

        let gateway = OutputInstance::new(
            OutputId::new(4),
            "Gateway",
            OutputKind::Sacn {
                receivers: Vec::new(),
                ttl: 4,
                ports: Vec::new(),
            },
            [universe(3), universe(4)],
        );
        let (driver, _) = factory.open(&gateway, &context);
        assert_eq!(driver.universes(), [universe(3), universe(4)]);
    }

    /// The four rows an installer reads off the back of a node, turned into the
    /// port addresses that go on the wire — and the default for a universe the
    /// configuration says nothing about.
    #[test]
    fn an_art_net_row_becomes_the_port_address_it_names() {
        let output = OutputInstance::new(
            OutputId::new(1),
            "Node",
            OutputKind::Mock,
            [universe(5), universe(6)],
        );
        let ports = art_net_ports(
            &output,
            &[ArtNetPort {
                universe: universe(5),
                net: 1,
                sub_net: 2,
                port: 3,
            }],
        );
        assert_eq!(
            ports,
            vec![
                (universe(5), PortAddress::from_parts(1, 2, 3).unwrap()),
                (universe(6), PortAddress::for_universe(universe(6))),
            ],
            "a universe with no row keeps the default mapping, N - 1"
        );
    }

    #[test]
    fn an_sacn_row_becomes_the_e131_universe_and_priority_it_names() {
        let output = OutputInstance::new(
            OutputId::new(1),
            "Gateway",
            OutputKind::Mock,
            [universe(3), universe(4)],
        );
        let ports = sacn_ports(
            &output,
            &[SacnPort {
                universe: universe(3),
                sacn_universe: 101,
                priority: 150,
            }],
        );
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].sacn.get(), 101);
        assert_eq!(ports[0].priority.get(), 150);
        assert_eq!(
            ports[1].sacn.get(),
            4,
            "a universe with no row is sent as itself"
        );
        assert_eq!(ports[1].priority.get(), 100);
    }

    #[test]
    fn a_universe_list_reads_as_an_installer_would_write_it() {
        assert_eq!(universe_list(&[]), "");
        assert_eq!(universe_list(&[universe(1)]), "1");
        assert_eq!(
            universe_list(&[universe(5), universe(6), universe(7)]),
            "5, 6, 7"
        );
    }

    /// The frames really do reach the drivers, and each gets only its own.
    #[test]
    fn a_started_output_receives_the_frames_of_its_own_universes() {
        let mut publisher = publisher();
        let mut supervisor = supervisor(&publisher);
        supervisor.reconcile(&[mock(1, &[1]), mock(2, &[2, 3])]);

        publisher.frame_mut().fill(9);
        for _ in 0..4 {
            publisher.publish();
            std::thread::sleep(Duration::from_millis(5));
        }

        let first = supervisor.recording(OutputId::new(1)).unwrap();
        let second = supervisor.recording(OutputId::new(2)).unwrap();
        until("both drivers to have sent", || {
            first.frame_count() > 0 && second.frame_count() > 0
        });

        let mut seen: Vec<u32> = first
            .frames()
            .into_iter()
            .map(|(universe, _)| universe.get())
            .collect();
        seen.dedup();
        assert_eq!(seen, vec![1]);

        let mut seen: Vec<u32> = second
            .frames()
            .into_iter()
            .map(|(universe, _)| universe.get())
            .collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen, vec![2, 3]);
        supervisor.stop_all();
    }
}
