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

/// The daemon, as the protocol sees it.
///
/// Cheap to share: the IPC server, the autosave timer and the telemetry task
/// all hold one, and every one of them reaches the show through the same lock.
pub struct Desk {
    core: Mutex<Core>,
    outputs: Vec<OutputEntry>,
}

impl Desk {
    /// A desk over a wired-up core.
    #[must_use]
    pub fn new(core: Core, outputs: Vec<OutputEntry>) -> Self {
        Self {
            core: Mutex::new(core),
            outputs,
        }
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

    /// The configured outputs and their health.
    #[must_use]
    pub fn outputs(&self) -> &[OutputEntry] {
        &self.outputs
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
            show,
            session,
            programmer: core.file.programmer.state().clone(),
            outputs: self.output_snapshots(),
            health: self.health(&core),
            // How many profiles this desk can embed — a number, not the
            // profiles. S44 made the library two thousand of them, so a client
            // asks (`Query::SearchLibrary`) rather than being sent a menu that
            // would not fit in a frame.
            fixture_library: u32::try_from(core.file.library.len()).unwrap_or(u32::MAX),
        }
    }

    /// One entry per configured output.
    #[must_use]
    pub fn output_snapshots(&self) -> Vec<OutputSnapshot> {
        self.outputs
            .iter()
            .map(|output| OutputSnapshot {
                id: output.id,
                name: output.name.clone(),
                health: output.status.health(),
            })
            .collect()
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
            } => Answer::PatchPreview {
                preview: core
                    .file
                    .show
                    .preview_patch(*id, type_id, *universe, *address),
            },
            // The library never changes while the daemon runs, so this is a
            // search over a table rather than anything that touches the show —
            // and the limit is clamped here, because a client that asked for
            // two thousand would otherwise get an answer no frame can carry.
            // What a store would do, which needs the show, the programmer and
            // the store mode this build has — all three of which live on the
            // one `ShowFile`. See `prism_core::ShowFile::preview_store`.
            Query::StorePreview { target } => Answer::StorePreview {
                preview: core.file.preview_store(target),
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

#[cfg(test)]
mod tests {
    use super::{Desk, OutputEntry};
    use crate::core::Core;
    use crate::engine::EngineThread;
    use crate::testkit::show_file;
    use prism_core::{JsonMirror, ShowStore};
    use prism_domain::{
        Answer, AttributeType, Command, Delta, ExecutorId, FixtureId, GoDirection, OutputHealth,
        OutputId, Query, SelectionMode,
    };
    use prism_engine::FramePublisher;
    use prism_ipc::CommandOutcome;
    use prism_protocols::{MockOutput, OutputStatus, OutputThread, RunnerConfig, spawn};
    use std::sync::Arc;

    fn desk(dir: &std::path::Path) -> (Arc<Desk>, OutputThread) {
        let file = show_file();
        let store = ShowStore::open(dir.join("test.prism")).unwrap();
        let layout = Arc::new(crate::engine::frame_layout(4).unwrap());
        let report = Arc::new(prism_engine::PlaybackReport::new(8));
        let body = crate::core::build_body(&layout, &file, &report).unwrap();

        let mut publisher = FramePublisher::new(Arc::clone(&layout));
        let subscriber = publisher.subscribe();
        let output = MockOutput::new(OutputId::new(1), [prism_domain::UniverseId::new(1)]);
        let driver = spawn("out-mock", output, subscriber, RunnerConfig::default()).unwrap();
        let engine = EngineThread::start(body, publisher).unwrap();
        let core = Core::new(file, store, engine, layout, report).unwrap();

        let outputs = vec![OutputEntry {
            id: OutputId::new(1),
            name: "Mock".to_owned(),
            status: Arc::clone(driver.status()),
        }];
        (Arc::new(Desk::new(core, outputs)), driver)
    }

    /// The exit criterion: the handshake serves a snapshot carrying the show
    /// **and** the session.
    #[test]
    fn a_snapshot_carries_the_show_the_session_and_the_programmer() {
        let dir = tempfile::tempdir().unwrap();
        let (desk, driver) = desk(dir.path());
        // Something in the programmer, so an empty one would be visible.
        desk.command(Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        });
        desk.command(Command::SetAttribute {
            attribute: AttributeType::Dimmer,
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
                .value(FixtureId::new(1), AttributeType::Dimmer)
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

        driver.stop();
    }

    #[test]
    fn a_command_is_applied_and_a_refused_one_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let (desk, driver) = desk(dir.path());
        let outcome = desk.command(Command::ExecutorGo {
            executor_id: ExecutorId::new(0),
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
                .any(|delta| matches!(delta, Delta::ExecutorState { .. })),
            "{reported:?}"
        );

        let outcome = desk.command(Command::ExecutorGo {
            executor_id: ExecutorId::new(9),
            direction: GoDirection::Next,
        });
        let CommandOutcome::Refused { message } = outcome else {
            panic!("an executor that does not exist is refused, not {outcome:?}");
        };
        assert!(message.contains('9'), "{message}");

        driver.stop();
    }

    #[test]
    fn the_snapshot_says_what_the_save_lamp_shows() {
        let dir = tempfile::tempdir().unwrap();
        let (desk, driver) = desk(dir.path());
        assert!(!desk.snapshot().health.unsaved_changes);

        desk.command(Command::PatchFixture {
            id: FixtureId::new(3),
            name: "Fixture 3".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: prism_domain::UniverseId::new(1),
            address: 200,
        });
        assert!(desk.snapshot().health.unsaved_changes);

        desk.command(Command::SaveShow);
        assert!(!desk.snapshot().health.unsaved_changes);

        driver.stop();
    }

    #[test]
    fn an_output_reports_its_health_into_the_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let (desk, driver) = desk(dir.path());
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

        driver.stop();
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
        let (desk, driver) = desk(dir.path());
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

        driver.stop();
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
        let (desk, driver) = desk(dir.path());
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
        driver.stop();
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
