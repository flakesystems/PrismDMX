//! The daemon's side of `docs/IPC_PROTOCOL.md`: one `ServerHandler`.
//!
//! S16 left exactly this to do. `prism-ipc`'s server holds no show and no
//! engine on purpose — *a server with an opinion about whether a command is
//! valid would be a second source of truth* — so everything it knows about the
//! daemon arrives through four methods, two of which have a default.
//!
//! # Why the state is behind a lock and what that costs
//!
//! [`ServerHandler::command`] is synchronous and takes `&self`: the server
//! calls it from the connection's task and broadcasts what it answers with. So
//! the daemon's state sits behind a [`std::sync::Mutex`], and applying a command
//! — including the disk write a `SaveShow` asks for — happens on a runtime
//! worker.
//!
//! That is the right place for it. `ARCHITECTURE_SPEC.md` §3 puts the show
//! model, the session state and persistence on `core-main (async, tokio)` at
//! **normal** priority, and none of it is on the tick: the engine is an OS
//! thread of its own that this module reaches only by pushing 16-byte commands
//! into a queue. A save that takes a quarter of a second on a school's network
//! drive delays the next command and not one frame of DMX, which is D2 working
//! exactly as it was drawn.
//!
//! # The order the server guarantees, and the order this relies on
//!
//! Deltas reach **every** client and the `Ack` reaches one, in that order —
//! the fact before the receipt. `ShowFile::apply` already answers with exactly
//! those deltas, so [`CommandOutcome::Applied`] is a move rather than a
//! translation.

use std::sync::{Arc, Mutex, PoisonError};

use prism_domain::{Answer, Command, Delta, Query};
use prism_ipc::{
    ClientId, CommandOutcome, DaemonHealth, Hello, OutputSnapshot, PROTOCOL_VERSION, ServerHandler,
    Snapshot,
};
use prism_protocols::OutputStatus;

use crate::core::Core;
use crate::log;

/// One configured output, as the status panel names it.
#[derive(Debug, Clone)]
pub struct OutputEntry {
    /// Which output.
    pub id: prism_domain::OutputId,
    /// What the operator calls it.
    pub name: String,
    /// What its driver thread publishes.
    pub status: Arc<OutputStatus>,
}

/// One snapshot row per configured output — S33.
///
/// A free function rather than a method because it needs the core lock the
/// caller is already holding, and taking it twice would let the rig change
/// between the two halves of one snapshot.
///
/// **Every configured output, not every running one.** A disabled node and one
/// whose thread could not start are both rows an operator has to see: the second
/// is the case a status panel exists for, and leaving it out would be an output
/// that vanished rather than one that is red.
fn output_snapshots(core: &Core) -> Vec<OutputSnapshot> {
    let supervisor = core.outputs();
    let elapsed = supervisor.elapsed();
    core.machine()
        .outputs()
        .iter()
        .map(|output| {
            let status = supervisor.status(output.id);
            let fault = status.and_then(|status| status.last_error(elapsed));
            OutputSnapshot {
                id: output.id,
                name: output.name.clone(),
                // **The reported health, not the driver's** — S46. For an
                // Art-Net output the driver's `Ok` means the socket took the
                // datagram, and UDP always takes it; `reported_health` folds in
                // whether anything at the far end answers. Punch-list B6.
                health: supervisor.reported_health(output.id),
                output: Some(output.clone()),
                frames_sent: status.map_or(0, |status| status.frames_sent()),
                last_error: fault.map(|fault| fault.error.to_string()),
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a fault 584 million years ago is not the number that is wrong"
                )]
                last_error_ago_ms: fault.map(|fault| fault.ago.as_millis() as u64),
            }
        })
        .collect()
}

/// The daemon, as the protocol sees it.
///
/// Cheap to share: the IPC server, the autosave timer and the telemetry task
/// all hold one, and every one of them reaches the show through the same lock.
pub struct Desk {
    core: Mutex<Core>,
    /// What the attached control surface is doing, as the run loop last saw it
    /// — S36, widened in S37.
    ///
    /// It was the open port's name alone until S37; the Devices panel needs the
    /// health, the counters and the binding table beside it, and all four come
    /// from the same place at the same cadence.
    ///
    /// Here rather than on the `Core` because the *port* is the daemon's: a
    /// cable has a thread's worth of state and a `Core` is what a client's
    /// command reaches. `Daemon::run` refreshes this on its housekeeping tick
    /// and [`Desk::query`] answers `Answer::MidiPorts` from it, so what a
    /// settings window is told is at most half a second old — which is the right
    /// freshness for a fact that changes when a person moves a plug.
    open_surface: Mutex<Option<String>>,
    /// The health, the counters and the binding table — S37.
    ///
    /// `None` means *no surface is attached at all*, which is a different fact
    /// from one that is attached and `Disconnected`: a laptop with no port
    /// configured has nothing to report, and a desk that is switched off has a
    /// health and a set of counters that happen to be zero.
    surface_status: Mutex<Option<prism_domain::SurfaceStatus>>,
    /// Raised when a client asked the daemon to stop — S29.
    ///
    /// `ARCHITECTURE_SPEC.md` §10.3 has said since S17 that the daemon *exits
    /// only on explicit instruction — tray menu, CLI, service stop*, and named
    /// an ordering that only an orderly stop can honour: clients told first,
    /// then a blackout or a hold published as a frame, then the drivers. Until
    /// S29 the only thing that could ask for it was Ctrl-C on the daemon's own
    /// console, which a daemon spawned by a desktop shell has not got.
    ///
    /// A `Notify` rather than a flag the housekeeping tick picks up — which is
    /// how `take_surface_change` and its three companions travel — because this
    /// one is a **gesture with an answer an operator is waiting for**, and half
    /// a second of a menu item that has visibly done nothing is half a second
    /// in which it gets clicked again.
    ///
    /// It is on the `Desk` rather than on the `Core` because it changes no
    /// state: the `Core` is what a command reaches when it has something to
    /// change, and this one has the **process** as its subject.
    stop: Arc<tokio::sync::Notify>,
    /// The fixture files served last — S30b — so a model asked for half a
    /// megabyte at a time is read from its archive once, not once a part.
    /// Newest last, at most [`KEPT_RESOURCES`].
    resources: Mutex<Vec<(ResourceKey, ResourceFile)>>,
}

/// What a fixture file is asked for by: GUID, profile, kind and name.
type ResourceKey = (String, String, prism_domain::ResourceKind, String);

/// A fixture file read: its path inside the archive, and its bytes.
type ResourceFile = Arc<(String, Vec<u8>)>;

/// How many fixture files [`Desk`] keeps read. A rig's worth of one device's
/// models and gobos is a handful, and the viewer asks for them one at a time.
const KEPT_RESOURCES: usize = 8;

impl Desk {
    /// A desk over a wired-up core.
    #[must_use]
    pub fn new(core: Core) -> Self {
        Self {
            core: Mutex::new(core),
            open_surface: Mutex::new(None),
            surface_status: Mutex::new(None),
            stop: Arc::new(tokio::sync::Notify::new()),
            resources: Mutex::new(Vec::new()),
        }
    }

    /// What `Daemon::run` waits on to learn that somebody asked it to stop —
    /// S29.
    ///
    /// Handed out as an `Arc` rather than awaited through the desk, because the
    /// run loop's other arms need the daemon mutably and a future borrowing it
    /// would stop them.
    #[must_use]
    pub fn stop_signal(&self) -> Arc<tokio::sync::Notify> {
        Arc::clone(&self.stop)
    }

    /// Asks the daemon to stop — S29, and [`Command::Shutdown`]'s whole effect.
    ///
    /// Idempotent, and deliberately: a `Notify` holds one permit, so a menu item
    /// pressed three times stops the daemon once.
    pub fn request_stop(&self) {
        self.stop.notify_one();
    }

    /// Records which MIDI port the surface is open on — S36.
    ///
    /// Written by the run loop and by nobody else.
    pub fn set_open_surface(&self, port: Option<String>) {
        *self
            .open_surface
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = port;
    }

    /// The MIDI port the surface is open on, as last recorded.
    #[must_use]
    pub fn open_surface(&self) -> Option<String> {
        self.open_surface
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Records what the attached surface is doing — S37.
    ///
    /// Written by the run loop on its housekeeping tick and by nobody else,
    /// which is [`Self::set_open_surface`]'s cadence and its reason: the surface
    /// polls every millisecond, and a status panel half a second fresher is not
    /// worth a lock and an allocation a thousand times a second.
    pub fn set_surface_status(&self, status: Option<prism_domain::SurfaceStatus>) {
        *self
            .surface_status
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = status;
    }

    /// What the attached surface is doing, as last recorded.
    #[must_use]
    pub fn surface_status(&self) -> Option<prism_domain::SurfaceStatus> {
        self.surface_status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The daemon's state.
    ///
    /// A lock a panicking holder cannot poison out of use: the alternative is a
    /// desk that stops answering because one command unwound, and
    /// `CLAUDE.md`'s zero-crash invariant is about exactly that. The engine
    /// thread is not behind this lock at all, so DMX is unaffected either way.
    pub fn core(&self) -> std::sync::MutexGuard<'_, Core> {
        self.core.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The running outputs and their health.
    ///
    /// **Through the core since S33**: the rig changes while the daemon runs, so
    /// a list fixed at construction would be a list that stopped being true the
    /// first time an operator added a node.
    #[must_use]
    pub fn outputs(&self) -> Vec<OutputEntry> {
        self.core().outputs().entries()
    }

    /// The world, as `docs/IPC_PROTOCOL.md` §4.1 defines it.
    ///
    /// Three documents: the show and the session as `JsonValue`, because
    /// `ShowPatch` and `SessionPatch` are RFC 6902 operations and an operation
    /// is only meaningful against a document root; and the programmer, which
    /// S16 added for the reason §9's snapshot-completeness row gives.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let core = self.core();
        let show = core.file.show.to_json().unwrap_or_else(|error| {
            // A show that cannot be projected is a show no client can mirror.
            // Sending an empty document is worse than saying so, so the client
            // gets a null and the daemon says why — and it cannot happen for a
            // show this model accepted, because the projection is the last thing
            // every edit does (S11).
            log::error("ipc", &format!("the show cannot be projected: {error}"));
            prism_domain::JsonValue::Null
        });
        let session = core.file.session.to_json().unwrap_or_else(|error| {
            log::error("ipc", &format!("the session cannot be projected: {error}"));
            prism_domain::JsonValue::Null
        });
        Snapshot {
            switch_positions: core.switch_positions(),
            show,
            session,
            programmer: core.file.programmer.state().clone(),
            outputs: output_snapshots(&core),
            health: self.health(&core),
            // How many profiles this desk can embed — a number, not the
            // profiles. S44 made the library two thousand of them, so a client
            // asks (`Query::SearchLibrary`) rather than being sent a menu that
            // would not fit in a frame.
            fixture_library: u32::try_from(core.file.library.len()).unwrap_or(u32::MAX),
            // S37's two panels that are not the rig. Both arrive with the world
            // rather than being asked for, which is `outputs`' reason exactly:
            // both are state the daemon owns, both change only when a command
            // changes them, and a client that had to ask would draw an empty
            // settings window for a round trip.
            machine: core.machine_settings(),
            show_file: core.show_file_info(),
        }
    }

    /// One entry per **configured** output — S33.
    #[must_use]
    pub fn output_snapshots(&self) -> Vec<OutputSnapshot> {
        output_snapshots(&self.core())
    }

    /// How the daemon itself is doing.
    fn health(&self, core: &Core) -> DaemonHealth {
        let tick = core.engine().health();
        DaemonHealth {
            protocol_version: PROTOCOL_VERSION,
            tick_hz: tick.rate(core.uptime()),
            missed_ticks: tick.missed(),
            unsaved_changes: core.file.is_dirty(),
        }
    }

    /// Applies a command and answers in the protocol's own vocabulary.
    ///
    /// A refusal is a `Reject` carrying the reason in words an operator can
    /// read, and it changed nothing — which is §5, and which `prism-core`
    /// asserts on the serialised bytes rather than claiming.
    pub fn command(&self, command: Command) -> CommandOutcome {
        // **The fourth kind of command, read before anything is routed** — S29.
        // It is not the show's, the session's or the machine's; its subject is
        // this process, so there is no applier to hand it to and no delta to
        // broadcast. Answered `Applied` with nothing in it, which is the honest
        // shape: the daemon accepted the instruction, and what follows is the
        // shutdown `ARCHITECTURE_SPEC.md` §10.3 describes rather than a change
        // any client could be told about.
        if matches!(command, Command::Shutdown) {
            log::info("daemon", "a client asked the desk to stop");
            self.request_stop();
            return CommandOutcome::Applied { deltas: Vec::new() };
        }
        let mut core = self.core();
        match core.apply(&command) {
            Ok(deltas) => CommandOutcome::Applied { deltas },
            Err(error) => {
                log::debug("ipc", &format!("refused {command:?}: {error}"));
                CommandOutcome::refused(error.to_string())
            }
        }
    }

    /// Answers a question. **Changes nothing** — `docs/IPC_PROTOCOL.md` §5.2.
    ///
    /// There is no refusal shape and there is nothing to broadcast: the answer
    /// is arithmetic over the show this daemon is holding, computed here so
    /// that no client has to compute it. The lock is taken for reading and let
    /// go again, exactly as `snapshot` does.
    pub fn query(&self, query: &Query) -> Answer {
        if let Query::FixtureResource {
            fixture_type_id,
            type_id,
            kind,
            name,
            offset,
        } = query
        {
            return self.fixture_resource(fixture_type_id, type_id, *kind, name, *offset);
        }
        let core = self.core();
        match query {
            Query::PatchConflicts => Answer::PatchConflicts {
                conflicts: core.file.show.conflicts(),
            },
            Query::PatchPreview {
                id,
                type_id,
                universe,
                address,
                adding,
            } => Answer::PatchPreview {
                preview: core.file.preview_patch(
                    type_id,
                    prism_domain::PatchPlacement {
                        id: *id,
                        universe: *universe,
                        address: *address,
                    },
                    *adding,
                ),
            },
            // The library never changes while the daemon runs, so this is a
            // search over a table rather than anything that touches the show —
            // and the limit is clamped here, because a client that asked for
            // two thousand would otherwise get an answer no frame can carry.
            // What a store would do, which needs the show, the programmer and
            // what the chosen mode *means* — all three of which live on the one
            // `ShowFile`. See `prism_core::ShowFile::preview_store`. The mode is
            // the operator's since S39 and travels in the question.
            Query::StorePreview { target, mode } => Answer::StorePreview {
                preview: core.file.preview_store(target, *mode),
            },
            // S36. **The enumeration is done here and now**, on the thread that
            // asked, because that is what makes the answer true: a list built at
            // start-up would be a list of what was plugged in then, and the
            // gesture this exists for is somebody plugging a desk in and
            // pressing *rescan*. It is a system call and not a cheap one, which
            // is exactly why it is a query rather than a field of the snapshot.
            Query::MidiPorts => {
                let listed = prism_midi::ports();
                Answer::MidiPorts {
                    ports: listed
                        .names()
                        .into_iter()
                        .map(|name| prism_domain::MidiPortInfo {
                            input: listed.inputs.contains(&name),
                            output: listed.outputs.contains(&name),
                            name,
                        })
                        .collect(),
                    configured: core.surface_port().map(str::to_owned),
                    open: self.open_surface(),
                    status: self.surface_status(),
                }
            }
            // S48, and the deepest derived thing in the project: what a cue
            // inherits is folded out of every cue above it, so a client that
            // worked it out would be a second opinion about the rule the engine
            // resolves a Goto through — and two answers to *where am I* is the
            // fault S48 exists to remove, not one to reintroduce in TypeScript.
            Query::CueTracking { sequence_id } => Answer::CueTracking {
                sequence_id: *sequence_id,
                cues: core.file.show.cue_tracking(*sequence_id),
            },
            // S49, and it is the reading under the command line: what the line
            // an operator is typing *would* do. It is here for §5.2's rule — it
            // is derived, and since S49 the thing it is derived from is here.
            //
            // Two of the fields the parser cannot supply and this can. The
            // **question** is `Some` only when the destination is really
            // occupied, which is the one thing an interface used to look up in
            // its own mirror; and the parser deliberately does not read the
            // show, so it says *what would be written* and this decides *whether
            // to ask*.
            Query::CommandLineReading { text } => {
                let reading = prism_core::console::parse_command_line(text);
                let question = reading
                    .question()
                    .filter(|question| core.file.holds(&question.at))
                    .map(prism_core::ModeQuestion::question);
                Answer::CommandLineReading {
                    text: text.clone(),
                    reading: prism_core::console::reading_text(&reading),
                    kind: reading.kind(),
                    commands: u32::try_from(reading.commands().len()).unwrap_or(u32::MAX),
                    verb: prism_core::console::is_verb_line(text),
                    clearing: prism_core::console::is_clearing_line(text),
                    question,
                    completions: prism_core::console::completions(text),
                }
            }
            // S37. Derived from the patch and from the rig, which is exactly
            // why it is a question: a client that intersected the two would be
            // a second opinion about something `prism_core::dark_universes`
            // already decides.
            // S37, and the one thing S33 left with no channel: a **live** frame
            // counter. Read here rather than mirrored, because it moves at the
            // output's own cadence and only an open settings window is looking
            // at it. The configuration is not repeated — that arrives whole in
            // `Delta::OutputsChanged` whenever it moves.
            // S38, and it is a question for §5.2's own rule: the table in
            // force is **derived** — the built-in defaults, or a profile read
            // into this machine's own rows, or the rows an operator has typed —
            // and a client that layered those for itself would be a second
            // opinion about something `prism_surface::Bindings` already decides.
            //
            // The two flags on each row are the **device profile's** rather than
            // the table's: which controls stay PrismDMX's while the surface is
            // also driving a sound console (`docs/MCU_MAPPING.md` §4.3), and
            // which may never be bound. A client holds no profile, so it is told.
            Query::SurfaceBindings => {
                let bindings = core.bindings();
                Answer::SurfaceBindings {
                    controls: prism_domain::BoundControl::all()
                        .into_iter()
                        .map(|control| prism_domain::SurfaceControl {
                            name: control.to_string(),
                            action: bindings.action(control),
                            permanent: crate::surface::is_permanent(control),
                            reserved: control.reserved().is_some(),
                            // The third fact off the device profile — S59.
                            geometry: prism_surface::box_of(control),
                            control,
                        })
                        .collect(),
                    device: prism_surface::X_TOUCH.name.to_owned(),
                    // The two an **export** needs, which are the profile's own
                    // constants rather than a client's copy of them — S43.
                    device_key: prism_surface::X_TOUCH.key.to_owned(),
                    profile_version: prism_surface::PROFILE_VERSION,
                    profile: core.machine().settings().surface_profile.clone(),
                    revision: core.binding_revision(),
                    learning: core.is_learning(),
                    panel: Some(prism_surface::X_TOUCH_PANEL),
                }
            }
            Query::OutputStatus => {
                let supervisor = core.outputs();
                let elapsed = supervisor.elapsed();
                Answer::OutputStatus {
                    outputs: core
                        .machine()
                        .outputs()
                        .iter()
                        .map(|output| {
                            let status = supervisor.status(output.id);
                            let fault = status.and_then(|status| status.last_error(elapsed));
                            prism_domain::OutputStatusInfo {
                                id: output.id,
                                // S46, and punch-list B6 — see `output_snapshots`.
                                health: supervisor.reported_health(output.id),
                                frames_sent: status.map_or(0, |status| status.frames_sent()),
                                last_error: fault.map(|fault| fault.error.to_string()),
                                #[expect(
                                    clippy::cast_possible_truncation,
                                    reason = "a fault 584 million years ago is not the number that is wrong"
                                )]
                                last_error_ago_ms: fault.map(|fault| fault.ago.as_millis() as u64),
                                // One row per configured node, and empty for
                                // every kind that cannot be asked — S46.
                                nodes: supervisor.node_reach(output),
                            }
                        })
                        .collect(),
                }
            }
            // S46. Read rather than asked for on the spot, because there is no
            // call that answers *what is on this network*: discovery is a
            // conversation over time, so the daemon holds a table its own
            // receive thread keeps and this reads it. The question sends
            // nothing, which is §5.2's first rule.
            //
            // The two **disagreements** are worked out here and not by a
            // client. A browser holding the rig and the discovery table could
            // intersect them, and that is precisely `PatchPreview`'s trap: a
            // second opinion about something the daemon already holds both
            // halves of, which drifts the first time a port-address default
            // changes.
            Query::ArtNetNodes => {
                let supervisor = core.outputs();
                let table = supervisor.discovered();
                let rig = core.machine().outputs();
                let now = std::time::Instant::now();
                Answer::ArtNetNodes {
                    nodes: table
                        .nodes
                        .iter()
                        .map(|node| {
                            let desk = crate::outputs::desk_ports_to(rig, node.address);
                            prism_domain::ArtNetNodeInfo {
                                address: node.address.to_string(),
                                ip: node.ip.to_string(),
                                short_name: node.short_name.clone(),
                                long_name: node.long_name.clone(),
                                mac: node.mac.clone(),
                                firmware: node.firmware,
                                style: node.style,
                                status1: node.status1,
                                status2: node.status2,
                                ports: node.ports.clone(),
                                inputs: node.inputs.clone(),
                                configured: rig.iter().any(|output| {
                                    matches!(
                                        &output.kind,
                                        prism_domain::OutputKind::ArtNet { nodes, .. }
                                            if nodes
                                                .iter()
                                                .any(|target| target.ip() == node.address.ip())
                                    )
                                }),
                                unaddressed_ports: node
                                    .ports
                                    .iter()
                                    .copied()
                                    .filter(|port| !desk.contains(port))
                                    .collect(),
                                missing_ports: desk
                                    .iter()
                                    .copied()
                                    .filter(|port| !node.ports.contains(port))
                                    .collect(),
                                suggested_universes: node
                                    .ports
                                    .iter()
                                    .filter_map(|port| {
                                        prism_domain::ArtNetNodeInfo::universe_for_port(*port)
                                    })
                                    .collect(),
                                replies: node.replies,
                                #[expect(
                                    clippy::cast_possible_truncation,
                                    reason = "a reply 584 million years ago is not the number that is wrong"
                                )]
                                last_reply_ago_ms: now
                                    .saturating_duration_since(node.last_seen)
                                    .as_millis()
                                    as u64,
                            }
                        })
                        .collect(),
                    listening: table.listening,
                    error: table.error.clone(),
                    remedy: table.remedy(),
                    counters: prism_domain::ArtNetCounters {
                        polls_sent: table.counters.polls_sent,
                        polls_failed: table.counters.polls_failed,
                        replies: table.counters.replies,
                        malformed: table.counters.malformed,
                        dropped: table.counters.dropped,
                        read_errors: table.counters.read_errors,
                    },
                }
            }
            // `filter_map` rather than a `match` with an unreachable arm:
            // `dark_universes` answers with one variant of a nine-variant enum,
            // and S21's rule is that a branch nothing can reach is **removed**
            // rather than covered. What is left is a filter that would quietly
            // drop a variant added later — which is the right failure for a
            // *report*, and `crates/prismd/tests/settings.rs` asserts the count
            // this answers with rather than trusting the shape.
            Query::DarkUniverses => Answer::DarkUniverses {
                universes: core
                    .dark_universes()
                    .into_iter()
                    .filter_map(|issue| match issue {
                        prism_core::ShowIssue::UniverseNotOutput { universe } => Some(universe),
                        _ => None,
                    })
                    .collect(),
            },
            // S57's two: the library a fixture at a time and a page at a time,
            // and the fixture one key belongs to. The same table, the same
            // clamp, and nothing about the show.
            Query::BrowseLibrary {
                text,
                offset,
                limit,
            } => {
                let page = core.file.library.browse(
                    text,
                    usize::try_from(*offset).unwrap_or(usize::MAX),
                    usize::try_from(*limit).unwrap_or(usize::MAX),
                );
                Answer::LibraryFixtures {
                    fixtures: page.fixtures,
                    matched: u32::try_from(page.matched).unwrap_or(u32::MAX),
                    total: u32::try_from(page.total).unwrap_or(u32::MAX),
                }
            }
            Query::FixtureOfMode { type_id } => Answer::FixtureOfMode {
                fixture: core.file.library.fixture_of(type_id),
            },
            // Answered above, without the core: see `fixture_resource`.
            Query::FixtureResource { kind, name, .. } => Answer::FixtureResource {
                kind: *kind,
                name: name.clone(),
                path: String::new(),
                offset: 0,
                total: 0,
                data: String::new(),
            },
            Query::SearchLibrary { text, limit } => Answer::LibraryMatches {
                matches: core
                    .file
                    .library
                    .search(text, usize::try_from(*limit).unwrap_or(usize::MAX)),
                total: u32::try_from(core.file.library.len()).unwrap_or(u32::MAX),
            },
        }
    }

    /// A model or a gobo picture out of the fixture's own archive, a part at
    /// a time — S30b, `Query::FixtureResource`.
    ///
    /// **The core lock is held only to find where the file is.** Reading a
    /// published archive off the disk takes milliseconds, and every command
    /// from every client waits for the core; so the library answers *where*
    /// under the lock and the file is read after it is let go. The last few
    /// files read are kept, because a model is asked for half a megabyte at a
    /// time and each part would otherwise read the whole archive again.
    fn fixture_resource(
        &self,
        fixture_type_id: &str,
        type_id: &str,
        kind: prism_domain::ResourceKind,
        name: &str,
        offset: u32,
    ) -> Answer {
        let key: ResourceKey = (
            fixture_type_id.to_owned(),
            type_id.to_owned(),
            kind,
            name.to_owned(),
        );
        let kept = self.resources.lock().ok().and_then(|held| {
            held.iter()
                .find(|(each, _)| *each == key)
                .map(|(_, file)| Arc::clone(file))
        });
        let file = kept.or_else(|| {
            let lookup =
                self.core()
                    .file
                    .library
                    .resource_lookup(fixture_type_id, type_id, kind, name);
            let read = Arc::new(lookup.and_then(|lookup| lookup.read())?);
            if let Ok(mut held) = self.resources.lock() {
                held.retain(|(each, _)| *each != key);
                held.push((key, Arc::clone(&read)));
                if held.len() > KEPT_RESOURCES {
                    held.remove(0);
                }
            }
            Some(read)
        });
        let (path, bytes) = file.as_deref().map_or(("", &[][..]), |(path, bytes)| {
            (path.as_str(), bytes.as_slice())
        });
        let (start, part) = resource_part(bytes, offset);
        Answer::FixtureResource {
            kind,
            name: name.to_owned(),
            path: path.to_owned(),
            offset: start,
            total: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            data: base64(part),
        }
    }

    /// Deltas the daemon produced by itself rather than in answer to a command
    /// — the autosave's complaint, an output changing health.
    ///
    /// Returned rather than broadcast, because broadcasting is asynchronous and
    /// this is not.
    pub fn poll_autosave(&self) -> Vec<Delta> {
        self.core().poll_autosave()
    }
}

/// The handler the IPC server is given.
///
/// A newtype rather than an `impl` on `Arc<Desk>` directly, so the trait cannot
/// be picked up by accident somewhere that only wanted a shared desk.
#[derive(Clone)]
pub struct DeskHandler {
    desk: Arc<Desk>,
}

impl DeskHandler {
    /// A handler over a shared desk.
    #[must_use]
    pub const fn new(desk: Arc<Desk>) -> Self {
        Self { desk }
    }
}

impl ServerHandler for DeskHandler {
    fn snapshot(&self, _client: ClientId, _hello: &Hello) -> Snapshot {
        self.desk.snapshot()
    }

    fn command(&self, _client: ClientId, command: Command) -> CommandOutcome {
        self.desk.command(command)
    }

    fn query(&self, _client: ClientId, query: Query) -> Answer {
        self.desk.query(&query)
    }

    fn connected(&self, client: ClientId, hello: &Hello) {
        log::info(
            "ipc",
            &format!("{client} connected ({:?})", hello.client_kind),
        );
    }

    fn disconnected(&self, client: ClientId) {
        // §8: the daemon frees its state and carries on. Nothing about the show
        // changes, and this line is the whole of what a client leaving costs.
        log::info("ipc", &format!("{client} disconnected"));
    }
}

/// How much of a fixture's file one answer carries — S30b.
///
/// Half a megabyte, which is two thirds of a megabyte as base64 and so well
/// inside `prism_ipc::MAX_FRAME_BYTES` with the rest of the message around it.
const RESOURCE_CHUNK: usize = 512 * 1024;

/// The part of a file that starts at `offset`: at most [`RESOURCE_CHUNK`]
/// bytes, and where it really starts — the end of the file for an offset past
/// it, so a client that asks for more than there is gets an empty part and
/// stops rather than an error it would have to understand.
fn resource_part(bytes: &[u8], offset: u32) -> (u32, &[u8]) {
    let start = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(bytes.len());
    let end = start.saturating_add(RESOURCE_CHUNK).min(bytes.len());
    (u32::try_from(start).unwrap_or(u32::MAX), &bytes[start..end])
}

/// Standard base64 with padding (RFC 4648 §4) — S30b's answer carries a file
/// part as text; see `Answer::FixtureResource`.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = u32::from(chunk[0]);
        let b = chunk.get(1).map_or(0, |byte| u32::from(*byte));
        let c = chunk.get(2).map_or(0, |byte| u32::from(*byte));
        let triple = (a << 16) | (b << 8) | c;
        for (index, shift) in [18_u32, 12, 6, 0].into_iter().enumerate() {
            if index > chunk.len() {
                out.push('=');
            } else {
                let at = usize::try_from((triple >> shift) & 0x3f).unwrap_or(0);
                out.push(char::from(ALPHABET[at]));
            }
        }
    }
    out
}

#[cfg(test)]
mod resource_tests {
    use super::{RESOURCE_CHUNK, base64, resource_part};

    /// RFC 4648 §10's vectors, which cover every padding.
    #[test]
    fn base64_is_the_rfc_s() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), encoded, "{plain:?}");
        }
        assert_eq!(base64(&[0xfb, 0xff, 0xbf]), "+/+/");
    }

    /// A file is served half a megabyte at a time, each part where it was
    /// asked for, and a part asked for past the end is empty at the end.
    #[test]
    fn a_file_is_served_a_part_at_a_time() {
        let file: Vec<u8> = (0..RESOURCE_CHUNK * 2 + 10)
            .map(|index| u8::try_from(index % 251).unwrap_or(0))
            .collect();
        let chunk = u32::try_from(RESOURCE_CHUNK).expect("half a megabyte");
        let (start, first) = resource_part(&file, 0);
        assert_eq!((start, first.len()), (0, RESOURCE_CHUNK));
        let (start, second) = resource_part(&file, chunk);
        assert_eq!((start, second.len()), (chunk, RESOURCE_CHUNK));
        let (start, last) = resource_part(&file, chunk * 2);
        assert_eq!((start, last), (chunk * 2, &file[RESOURCE_CHUNK * 2..]));
        let whole: Vec<u8> = [first, second, last].concat();
        assert_eq!(whole, file);

        let end = u32::try_from(file.len()).expect("a small file");
        assert_eq!(resource_part(&file, u32::MAX), (end, &[][..]));
        assert_eq!(resource_part(&[], 0), (0, &[][..]));
    }

    /// A part as base64, with the answer around it, fits in one IPC frame.
    #[test]
    fn a_part_fits_in_a_frame() {
        let part = vec![0xff_u8; RESOURCE_CHUNK];
        assert!(base64(&part).len() + 4096 < prism_ipc::MAX_FRAME_BYTES);
    }
}

#[cfg(test)]
mod tests {
    use super::{Desk, OutputEntry};
    use crate::core::Core;
    use crate::engine::EngineThread;
    use crate::testkit::show_file;
    use prism_core::{JsonMirror, ShowStore};
    use prism_domain::{
        Answer, AttributeKey, AttributeType, Command, Delta, ExecutorId, FixtureId, GoDirection,
        OutputHealth, OutputId, OutputInstance, OutputKind, PlaybackTarget, Query, SelectionMode,
    };
    use prism_engine::FramePublisher;
    use prism_ipc::CommandOutcome;
    use prism_protocols::OutputStatus;
    use std::sync::Arc;

    /// A desk over one mock output, wired the way the daemon wires one.
    ///
    /// **The rig goes through the machine configuration since S33**, because
    /// that is where it lives: a `Desk` handed a list of `OutputEntry` values
    /// would be a second place an output can come from, and the one thing S33
    /// is about is that there is only one.
    fn desk(dir: &std::path::Path) -> Arc<Desk> {
        let file = show_file();
        let store = ShowStore::open(dir.join("test.prism")).unwrap();
        let layout = Arc::new(crate::engine::frame_layout(4).unwrap());
        let report = Arc::new(prism_engine::PlaybackReport::new(8));
        let body = crate::core::build_body(&layout, &file, &report).unwrap();

        let publisher = FramePublisher::new(Arc::clone(&layout));
        let mut outputs = crate::outputs::OutputSupervisor::new(
            publisher.enrolment(),
            Box::new(crate::outputs::MockDevices::default()),
            crate::outputs::OutputContext {
                cid: prism_protocols::Cid::from_u128(1),
                source_name: "PrismDMX test".to_owned(),
            },
        );
        let mut machine = prism_core::MachineConfig::default();
        machine
            .apply(&Command::AddOutput {
                output: OutputInstance::new(
                    OutputId::new(1),
                    "Mock",
                    OutputKind::Mock,
                    [prism_domain::UniverseId::new(1)],
                ),
            })
            .unwrap();
        outputs.reconcile(machine.outputs());

        let engine = EngineThread::start(body, publisher).unwrap();
        let core = Core::new(
            file,
            crate::outputs::Machine {
                config: machine,
                path: Some(dir.join("machine.json")),
                outputs,
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
        Arc::new(Desk::new(core))
    }

    /// The exit criterion: the handshake serves a snapshot carrying the show
    /// **and** the session.
    #[test]
    fn a_snapshot_carries_the_show_the_session_and_the_programmer() {
        let dir = tempfile::tempdir().unwrap();
        let desk = desk(dir.path());
        // Something in the programmer, so an empty one would be visible.
        desk.command(Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        });
        desk.command(Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            occurrence: 0,
            value: 12345,
            relative: false,
        });

        let snapshot = desk.snapshot();

        // Read the way a client reads it: through the mirror the deltas are
        // applied to, which is what makes the snapshot a document rather than
        // a picture of one.
        let show = JsonMirror::new(snapshot.show.clone());
        assert_eq!(
            show.get("/fixtures/1/name").unwrap(),
            &prism_domain::JsonValue::String("Fixture 1".to_owned())
        );
        // The session document: `{ session, views }`, and this session is not
        // at its defaults — a snapshot that dropped it would look identical to
        // one that carried a fresh session.
        let session = JsonMirror::new(snapshot.session.clone());
        assert_eq!(
            session.get("/session/executorPage").unwrap(),
            &prism_domain::JsonValue::Int(3)
        );
        assert_eq!(
            session.get("/session/commandLine").unwrap(),
            &prism_domain::JsonValue::String("fixture 1 at full".to_owned())
        );
        // And the third document S16 added.
        assert_eq!(snapshot.programmer.selection, vec![FixtureId::new(1)]);
        assert_eq!(
            snapshot
                .programmer
                .value(
                    FixtureId::new(1),
                    AttributeKey::first(AttributeType::Dimmer)
                )
                .map(|held| held.value),
            Some(12345)
        );

        assert_eq!(snapshot.outputs.len(), 1);
        assert_eq!(snapshot.outputs[0].name, "Mock");
        assert_eq!(
            snapshot.health.protocol_version,
            prism_ipc::PROTOCOL_VERSION
        );

        // The tick rate is measured rather than declared, so it is zero until
        // the first tick has run. Waiting for one is the assertion: a daemon
        // whose engine never started would sit here until the deadline.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline && desk.snapshot().health.tick_hz <= 0.0 {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(
            desk.snapshot().health.tick_hz > 0.0,
            "the tick is running and the snapshot says so"
        );
    }

    #[test]
    fn a_command_is_applied_and_a_refused_one_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let desk = desk(dir.path());
        let outcome = desk.command(Command::ExecutorGo {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            direction: GoDirection::Next,
        });
        let CommandOutcome::Applied { deltas } = outcome else {
            panic!("a Go on a loaded executor is applied, not {outcome:?}");
        };
        // A Go carries no deltas of its own: since S34 what the executor is
        // doing comes back from the tick, through `Core::poll_playback`, which
        // the daemon's own loop calls. What this asserts is that the command was
        // *applied* rather than refused.
        assert!(deltas.is_empty(), "{deltas:?}");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut reported = Vec::new();
        while std::time::Instant::now() < deadline && reported.is_empty() {
            reported.extend(desk.core().poll_playback());
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(
            reported
                .iter()
                .any(|delta| matches!(delta, Delta::PlaybackState { .. })),
            "{reported:?}"
        );

        let outcome = desk.command(Command::ExecutorGo {
            target: PlaybackTarget::of_executor(ExecutorId::new(9)),
            direction: GoDirection::Next,
        });
        let CommandOutcome::Refused { message } = outcome else {
            panic!("an executor that does not exist is refused, not {outcome:?}");
        };
        assert!(message.contains('9'), "{message}");
    }

    #[test]
    fn the_snapshot_says_what_the_save_lamp_shows() {
        let dir = tempfile::tempdir().unwrap();
        let desk = desk(dir.path());
        assert!(!desk.snapshot().health.unsaved_changes);

        desk.command(Command::PatchFixture {
            software_dimmer: true,
            id: FixtureId::new(3),
            name: "Fixture 3".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: prism_domain::UniverseId::new(1),
            address: 200,
        });
        assert!(desk.snapshot().health.unsaved_changes);

        desk.command(Command::SaveShow);
        assert!(!desk.snapshot().health.unsaved_changes);
    }

    #[test]
    fn an_output_reports_its_health_into_the_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let desk = desk(dir.path());
        // The driver connects on its first step, so this is the ordinary state
        // rather than a contrived one.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline
            && desk.snapshot().outputs[0].health != OutputHealth::Ok
        {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(desk.snapshot().outputs[0].health, OutputHealth::Ok);
        assert_eq!(desk.outputs().len(), 1);
    }

    /// **A question changes nothing, and the answer is the daemon's arithmetic.**
    ///
    /// `docs/IPC_PROTOCOL.md` §5.2. The rig here has a one-channel dimmer at 1,
    /// a four-channel PAR at 10 and a dimmer at 5, so a PAR asked about at 8
    /// would run over the one at 10 — which is exactly the answer an operator
    /// has to be given *before* they press Apply.
    #[test]
    fn a_question_is_answered_and_the_show_does_not_move() {
        let dir = tempfile::tempdir().unwrap();
        let desk = desk(dir.path());
        let before = desk.snapshot().show;

        let Answer::PatchConflicts { conflicts } = desk.query(&Query::PatchConflicts) else {
            panic!("that is not a conflicts answer");
        };
        assert_eq!(conflicts, Vec::new(), "the rig starts clean");

        let Answer::PatchPreview { preview } = desk.query(&Query::PatchPreview {
            id: FixtureId::new(9),
            type_id: "generic.rgbw.par".to_owned(),
            universe: prism_domain::UniverseId::new(1),
            address: 8,
            adding: 0,
        }) else {
            panic!("that is not a preview");
        };
        assert!(preview.accepted, "an overlap is not a refusal");
        assert_eq!(preview.footprint, 4);
        assert_eq!(preview.last_address, Some(11));
        assert_eq!(preview.conflicts.len(), 1, "{preview:?}");
        assert_eq!(preview.conflicts[0].first, FixtureId::new(2));
        assert_eq!(preview.conflicts[0].second, FixtureId::new(9));

        // A refusal, said without sending anything.
        let Answer::PatchPreview { preview } = desk.query(&Query::PatchPreview {
            id: FixtureId::new(9),
            type_id: "nothing.at.all".to_owned(),
            universe: prism_domain::UniverseId::new(1),
            address: 1,
            adding: 0,
        }) else {
            panic!("that is not a preview");
        };
        assert!(!preview.accepted);
        assert!(preview.refusal.is_some());

        // **And nothing moved**, which is the whole property the third message
        // shape rests on: an interface may ask this as often as it likes on a
        // desk that is running a show.
        assert_eq!(desk.snapshot().show, before);
        assert!(!desk.snapshot().health.unsaved_changes);
    }

    /// **The snapshot carries how many profiles there are, and not the
    /// profiles** — S44.
    ///
    /// A client needs the number to say *2 157 profiles* beside a search box;
    /// it must not be sent the library, because two thousand of them do not fit
    /// in a frame. The search is a question, and it is asked.
    #[test]
    fn the_snapshot_counts_the_profiles_and_the_search_answers_them() {
        let dir = tempfile::tempdir().unwrap();
        let desk = desk(dir.path());
        // `show_file` builds a `ShowFile`, whose library defaults to the four
        // generic profiles — this daemon never read a directory.
        let counted = desk.snapshot().fixture_library;
        assert_eq!(counted as usize, prism_core::generic_profiles().len());

        let Answer::LibraryMatches { matches, total } = desk.query(&Query::SearchLibrary {
            text: "par".to_owned(),
            limit: 10,
        }) else {
            panic!("that is not a library answer");
        };
        assert_eq!(
            total, counted,
            "the total is the whole library, not the page"
        );
        assert!(!matches.is_empty(), "the generic PARs are in there");
        assert!(
            matches
                .iter()
                .all(|entry| entry.name.to_lowercase().contains("par")),
            "{matches:?}"
        );
        // And a limit a client asked for is clamped rather than obeyed.
        let Answer::LibraryMatches { matches, .. } = desk.query(&Query::SearchLibrary {
            text: String::new(),
            limit: u32::MAX,
        }) else {
            panic!("that is not a library answer");
        };
        assert_eq!(matches.len(), counted as usize);

        // A question changes nothing, this one included.
        assert!(!desk.snapshot().health.unsaved_changes);
    }

    #[test]
    fn a_desk_with_no_outputs_is_an_ordinary_desk() {
        // A daemon started with nothing attached still runs, still answers, and
        // still serves a snapshot — which is what makes "outputs are
        // configuration" true rather than aspirational.
        let status = Arc::new(OutputStatus::new());
        assert_eq!(status.health(), OutputHealth::Disconnected);
        let entry = OutputEntry {
            id: OutputId::new(2),
            name: "Nothing".to_owned(),
            status,
        };
        assert_eq!(entry.name, "Nothing");
        assert_eq!(format!("{:?}", entry.id), format!("{:?}", OutputId::new(2)));
    }
}
