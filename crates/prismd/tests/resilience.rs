//! **The D2 gate.** `ARCHITECTURE_SPEC.md` §12: *start the daemon, kill the
//! client, reconnect — assert output ran without a gap*.
//!
//! S17 asserted that a daemon runs with no client. This target asserts the
//! sentence the whole two-process design exists for: that a client dying in the
//! middle of a show costs the rig nothing. Four claims, each an exit criterion:
//!
//! - a client is **killed mid-show** and the frames the rig was given have no
//!   gap across the whole run;
//! - it comes back and re-snapshots onto a state identical to the daemon's;
//! - a protocol version mismatch is an explicit `Reject` and never undefined
//!   behaviour;
//! - a deliberately slow client loses telemetry, loses no command, and does not
//!   affect anybody else.
//!
//! # What "no gap" means here, and why it is not simply a frame count
//!
//! `MockOutputHandle` records every frame in order, and a *missing* entry in
//! that list would say very little on its own: a driver sends on its own
//! cadence (`prism_protocols::RunnerConfig`), so one frame more or fewer is the
//! ordinary jitter of an output thread rather than a stage going dark. The two
//! claims that are worth making are therefore about **time** and about
//! **content**, and this file makes both:
//!
//! 1. **No silence.** Consecutive frames for one universe are never more than
//!    [`MAX_GAP`] apart, over the whole run, with the kill inside the window.
//! 2. **No unbidden darkness.** From the moment the show's look is on the wire,
//!    every later frame still carries it — including every frame after the
//!    client that set it was killed.
//!
//! A test that asserted only the first would pass on a daemon that blacked the
//! stage out and went on publishing zeroes at 44 Hz. One that asserted only the
//! second would pass on a daemon that froze holding a good frame. Together they
//! are the claim an operator would make: *the light did not change and did not
//! stop*.
//!
//! # A killed client is not a client that said goodbye
//!
//! `Client::disconnect` flushes and shuts the socket down in order. That is the
//! polite case and it is **not** what D2 is about. Here the client's task is
//! aborted while it is waiting — the connection is dropped mid-conversation,
//! with messages already queued for it that it will never read, which is the
//! state a client is in when the machine it runs on is switched off. S16 paid
//! for the difference once: a reader that stopped only at end of stream leaked a
//! Windows named pipe, and the daemon kept the dead client for ever.

// Every test here holds `common::one_daemon_at_a_time` across its awaits, which
// is what the guard is for: a daemon owns a real-time tick thread, and several
// of those in one process measure each other rather than the daemon. Each
// test's guard is dropped when its daemon is.
#![allow(clippy::await_holding_lock)]
// These tests *measure* — the longest silence, the round-trip time, how much
// telemetry was lost — and a measurement nobody can read is not one. The same
// allowance `prism-engine`'s timing tests and `prism-core`'s persistence tests
// carry, for the same reason: `--nocapture` is how a figure recorded in
// PROGRESS.md is reproduced.
#![allow(clippy::print_stdout)]

use std::path::Path;
use std::time::{Duration, Instant};

use prism_core::{SessionMirror, ShowMirror};
use prism_domain::{
    AttributeType, Command, Delta, ExecutorId, FixtureId, GoDirection, JsonPatchOp, JsonValue,
    PlaybackTarget, ProgrammerState, SelectionMode, UniverseId,
};
use prism_ipc::{Client, ClientError, ClientEvent, ClientKind, Hello, RejectReason};
use prism_protocols::FrameRecord;
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;

mod common;

/// The longest silence one universe may have between two frames before this
/// suite calls it a gap.
///
/// Eleven cadences. A mock output runs at the engine's own tick period
/// (`RunnerConfig::default`), so about 22.7 ms is the ordinary spacing and a
/// quarter of a second is eleven missed ones in a row — far more than the
/// jitter of a normal-priority driver thread on a busy machine, and far less
/// than the second a DMX receiver holds its last look for. The failure this
/// number is set against is not a late frame: it is an output coupled to a
/// client, which stalls for as long as the client takes to die.
const MAX_GAP: Duration = Duration::from_millis(250);

/// How long any wait in this file may take before it is a named failure.
///
/// Every wait has one. S16 pushed a test that waited for a message nothing had
/// promised to send, and the CI job stood for forty minutes with no output.
const PATIENCE: Duration = Duration::from_secs(20);

/// A daemon in the shape these criteria need: its own data directory, a show
/// written before it starts, a mock output, and the local transport open —
/// because every criterion here is about a client.
fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: Some(2),
        outputs: vec![mock_output(1)],
        local: Some(true),
        // No WebSocket listener: since S37 the *setting* opens one, and a
        // recording target that said nothing would bind 127.0.0.1:7373 for the
        // length of the run.
        websocket: prismd::cli::Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
        ..Options::default()
    }
}

/// Waits for `condition`, or fails by name.
async fn until(what: &str, mut condition: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        if condition().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}");
}

/// The next thing the daemon says to this client, or `None` if nothing came
/// within the deadline.
async fn next_event(client: &mut Client) -> Option<ClientEvent> {
    match tokio::time::timeout(Duration::from_secs(5), client.next_event()).await {
        Ok(Some(Ok(event))) => Some(event),
        Ok(Some(Err(error))) => panic!("the connection reported {error}"),
        Ok(None) | Err(_) => None,
    }
}

/// Reads whatever is already waiting for this client, and nothing more.
///
/// **A "fast client" is one that is being read**, and a client nobody is
/// polling is indistinguishable from one that has stopped reading — which is
/// the whole subject of the backpressure gate, and which the first version of
/// that test got wrong: while it waited for the *slow* client to fall behind, it
/// was not reading the fast one either, so on a CI runner both had dropped
/// exactly 28 telemetry frames and the comparison between them said nothing.
/// The deadline is short because "already waiting" is the question; a client
/// with nothing to read answers in a millisecond.
async fn drain_ready(client: &mut Client) -> usize {
    let mut read = 0;
    while read < 64 {
        match tokio::time::timeout(Duration::from_millis(1), client.next_event()).await {
            Ok(Some(Ok(_))) => read += 1,
            Ok(Some(Err(error))) => panic!("the connection reported {error}"),
            Ok(None) | Err(_) => break,
        }
    }
    read
}

/// Sends a command and waits for its `Ack`, applying everything that arrives on
/// the way to `apply`.
///
/// The deltas come **before** the acknowledgement (`docs/IPC_PROTOCOL.md` §5),
/// so a client that has its `Ack` has already been told everything the command
/// did — which is what makes "accumulated deltas" a well-defined state to
/// compare a fresh snapshot against.
async fn apply_and_wait(client: &mut Client, command: Command, mut apply: impl FnMut(&Delta)) {
    let seq = client.send(command).await.expect("the connection is open");
    loop {
        match next_event(client).await {
            Some(ClientEvent::Delta(delta)) => apply(&delta),
            Some(ClientEvent::Ack { seq: acked }) if acked == seq => return,
            Some(ClientEvent::Ack { .. } | ClientEvent::Telemetry(_)) => {}
            other => panic!("the command was not applied: {other:?}"),
        }
    }
}

/// Everything one universe was given, in order.
fn one_universe(timeline: &[FrameRecord], universe: UniverseId) -> Vec<&FrameRecord> {
    timeline
        .iter()
        .filter(|record| record.universe == universe)
        .collect()
}

/// The longest silence between two consecutive frames, and when it started.
fn longest_gap(frames: &[&FrameRecord]) -> (Duration, Option<Instant>) {
    let mut longest = Duration::ZERO;
    let mut began = None;
    for pair in frames.windows(2) {
        let gap = pair[1].at.duration_since(pair[0].at);
        if gap > longest {
            longest = gap;
            began = Some(pair[0].at);
        }
    }
    (longest, began)
}

/// **The exit criterion.** A client is killed in the middle of a show and the
/// rig does not notice.
///
/// The show is genuinely running when the client dies: the client's own `Go`
/// started the sequence on executor 0, which brings fixture 2 up. So what the
/// frames after the kill have to show is not merely *some* light but *the light
/// the dead client asked for* — a daemon that reset anything on a disconnection
/// would be visible here rather than arguable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_killed_mid_show_costs_the_rig_nothing() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let frames = daemon.recorded_outputs()[0].clone();
    let server = daemon.server().clone();
    let address = common::local_address(dir.path());

    // The daemon runs under its own timers for the whole scenario, rather than
    // being driven a step at a time: the claim is about a process that is
    // running, and its telemetry and housekeeping are part of what a dying
    // client interacts with.
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let running = tokio::spawn(async move {
        daemon
            .run(None, async {
                let _ = stopped.await;
            })
            .await;
        daemon
    });

    // A client connects, starts the show, and then stops reading. Everything
    // the daemon says from here queues up behind it — which is the state a
    // client is in the moment before its machine is switched off.
    let client = tokio::spawn({
        let address = address.clone();
        async move {
            let wire = prism_ipc::local::connect(&address).await.unwrap();
            let (mut client, _snapshot) = Client::handshake(wire, Hello::new(ClientKind::Desktop))
                .await
                .unwrap();
            client
                .send(Command::ExecutorGo {
                    target: PlaybackTarget::of_executor(ExecutorId::new(0)),
                    direction: GoDirection::Next,
                })
                .await
                .unwrap();
            std::future::pending::<()>().await;
        }
    });

    until("the client to be served", async || {
        server.client_count().await == 1
    })
    .await;
    // Cue 1 brings fixture 2 (address 5) up, and the rig is where the assertion
    // reads it from: waiting on the frames rather than on an acknowledgement is
    // what makes this "the show is running" rather than "a command was heard".
    until("the cue the client started to reach the rig", async || {
        common::last_frame_of(&frames, UniverseId::new(1)).is_some_and(|data| data[4] == 255)
    })
    .await;

    // The kill. Not `disconnect`: the task is aborted where it stands, so the
    // socket is dropped mid-conversation with messages queued for it.
    let killed_at = Instant::now();
    client.abort();
    until("the daemon to notice the client has gone", async || {
        server.client_count().await == 0
    })
    .await;

    // And then a run of its own with nobody attached, which is the half of the
    // criterion that says *across the entire run*. The count is over both
    // universes this output carries, so it is twice the number of frames the
    // assertions below look at.
    let before = frames.frame_count();
    until("the rig to be driven on after the kill", async || {
        frames.frame_count() > before + 150
    })
    .await;

    // Read before the shutdown, so the recording is of a daemon that is running
    // rather than of one that is stopping.
    let timeline = frames.timeline();
    let _ = stop.send(());
    let daemon = tokio::time::timeout(PATIENCE, running)
        .await
        .expect("the daemon must stop when it is told to")
        .unwrap();
    daemon.shutdown().await;

    let universe = one_universe(&timeline, UniverseId::new(1));
    assert!(
        universe.len() > 60,
        "there is nothing to say about {} frames",
        universe.len()
    );

    // The kill has to be *inside* the recording, or every assertion below is
    // about a run that never met the failure it is testing for.
    let first = universe.first().unwrap().at;
    let last = universe.last().unwrap().at;
    assert!(
        first < killed_at && killed_at < last,
        "the client was killed outside the recorded run, so this proves nothing"
    );
    let after_the_kill = universe
        .iter()
        .filter(|record| record.at > killed_at)
        .count();
    assert!(
        after_the_kill > 50,
        "only {after_the_kill} frames were recorded after the kill"
    );

    // 1. No silence.
    let (longest, began) = longest_gap(&universe);
    let offset = began.map(|at| at.duration_since(first));
    println!(
        "D2 gate: {} frames over {:?}, longest gap {longest:?} at {offset:?}, \
         {after_the_kill} of them after the kill",
        universe.len(),
        last.duration_since(first),
    );
    assert!(
        longest < MAX_GAP,
        "the rig was silent for {longest:?} at {offset:?}, and the client died at {:?}",
        killed_at.duration_since(first)
    );

    // 2. No unbidden darkness. The look is fixture 1 at home and fixture 2
    // raised by the cue the client started; from the frame it is first complete
    // on, every later frame still carries it.
    let up = universe
        .iter()
        .position(|record| record.data[0] == 255 && record.data[4] == 255)
        .expect("the show's look never reached the rig at all");
    assert!(
        universe[up].at < killed_at,
        "the look was not up before the client was killed"
    );
    for record in &universe[up..] {
        assert_eq!(
            (record.data[0], record.data[4]),
            (255, 255),
            "the look changed by itself {:?} after it came up",
            record.at.duration_since(universe[up].at)
        );
    }
}

/// **The exit criterion.** The client comes back and re-snapshots onto exactly
/// what the daemon holds.
///
/// This is also `docs/IPC_PROTOCOL.md` §9's *snapshot completeness* row, and the
/// two are the same claim from opposite ends: a fresh client's snapshot must
/// equal the state an existing client reached by accumulating deltas. If it does
/// not, one of the two is wrong — either a delta is missing, in which case every
/// long-lived client is silently drifting, or the snapshot is, in which case
/// every reconnection is.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_killed_client_comes_back_to_the_state_it_had_accumulated() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let frames = daemon.recorded_outputs()[0].clone();
    let server = daemon.server().clone();
    let address = common::local_address(dir.path());

    let (stop, stopped) = tokio::sync::oneshot::channel();
    let running = tokio::spawn(async move {
        daemon
            .run(None, async {
                let _ = stopped.await;
            })
            .await;
        daemon
    });

    let wire = prism_ipc::local::connect(&address).await.unwrap();
    let (mut client, snapshot) = Client::handshake(wire, Hello::new(ClientKind::Desktop))
        .await
        .unwrap();

    // The three documents the snapshot carries, mirrored the way a client keeps
    // them: by deltas alone, from here on.
    let mut show = ShowMirror::new(snapshot.show.clone());
    let mut session = SessionMirror::new(snapshot.session.clone());
    let mut programmer = snapshot.programmer.clone();

    // A command sequence that touches all three documents and every kind of
    // change: the programmer, the session, an executor's show state, a running
    // playback, and a repatch — which is the one that rebuilds the engine while
    // a client is watching.
    let script = vec![
        Command::SelectFixtures {
            ids: vec![FixtureId::new(1), FixtureId::new(2)],
            mode: SelectionMode::Set,
        },
        Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            value: 40000,
            relative: false,
        },
        Command::SetExecutorPage { page: 7 },
        Command::CommandLineInput {
            text: "fixture 2 at 50".to_owned(),
            run: false,
        },
        Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: 30000,
        },
        Command::ExecutorGo {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            direction: GoDirection::Next,
        },
        Command::PatchFixture {
            software_dimmer: true,
            id: FixtureId::new(4),
            name: "Fixture 4".to_owned(),
            type_id: "generic.dimmer".to_owned(),
            universe: UniverseId::new(1),
            address: 20,
        },
        Command::SelectFixtures {
            ids: vec![FixtureId::new(4)],
            mode: SelectionMode::Set,
        },
        Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            value: 20000,
            relative: false,
        },
    ];
    for command in script {
        apply_and_wait(&mut client, command, |delta| {
            show.apply_delta(delta).expect("a delta must fit the show");
            session
                .apply_delta(delta)
                .expect("a delta must fit the session");
            if let Delta::ProgrammerChanged { state } = delta {
                programmer = state.clone();
            }
        })
        .await;
    }

    // The client dies without saying anything, holding that state.
    drop(client);
    until("the daemon to notice the client has gone", async || {
        server.client_count().await == 0
    })
    .await;

    // And a fresh one connects, which is §8's *reconnect and re-snapshot*.
    let wire = prism_ipc::local::connect(&address).await.unwrap();
    let (fresh, again) = Client::handshake(wire, Hello::new(ClientKind::Desktop))
        .await
        .unwrap();

    assert_eq!(
        show.value(),
        &again.show,
        "the show a client accumulated is not the show the daemon holds"
    );
    assert_eq!(
        session.value(),
        &again.session,
        "the session a client accumulated is not the session the daemon holds"
    );
    assert_eq!(
        programmer, again.programmer,
        "the programmer a client accumulated is not the one the daemon holds"
    );
    // And the fixture built out of default values could not have told us that:
    // the script moved every one of these off its starting value.
    assert_eq!(
        session.get("/session/executorPage").unwrap(),
        &JsonValue::Int(7)
    );
    assert_eq!(
        show.get("/fixtures/4/name").unwrap(),
        &JsonValue::String("Fixture 4".to_owned())
    );
    assert_eq!(
        programmer
            .value(FixtureId::new(4), AttributeType::Dimmer)
            .map(|held| held.value),
        Some(20000)
    );

    // The rig ran through all of it, repatch included.
    let before = frames.frame_count();
    until("the rig to be driven after the reconnection", async || {
        frames.frame_count() > before + 10
    })
    .await;

    fresh.disconnect().await;
    let _ = stop.send(());
    let daemon = tokio::time::timeout(PATIENCE, running)
        .await
        .expect("the daemon must stop when it is told to")
        .unwrap();
    daemon.shutdown().await;
}

/// **The exit criterion.** A protocol version mismatch is an explicit `Reject`,
/// never undefined behaviour.
///
/// `docs/IPC_PROTOCOL.md` §4.2 says why in one sentence: *undefined behaviour
/// from a silent mismatch is unacceptable in software that controls a show*. So
/// the refusal has to name both versions — an operator whose interface will not
/// connect needs to be told which half to update — and the daemon has to be
/// entirely unaffected by having turned somebody away.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_of_the_wrong_version_is_refused_in_words_and_the_show_goes_on() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let frames = daemon.recorded_outputs()[0].clone();
    let server = daemon.server().clone();
    let address = common::local_address(dir.path());

    let (stop, stopped) = tokio::sync::oneshot::channel();
    let running = tokio::spawn(async move {
        daemon
            .run(None, async {
                let _ = stopped.await;
            })
            .await;
        daemon
    });

    // Both directions: an interface built against a newer engine and one built
    // against an older. Neither is a hostile client — both are the ordinary
    // case of a half-updated installation.
    for version in [prism_ipc::PROTOCOL_VERSION + 1, 0] {
        let wire = prism_ipc::local::connect(&address).await.unwrap();
        let mut hello = Hello::new(ClientKind::Desktop);
        hello.protocol_version = version;
        let refusal = Client::handshake(wire, hello).await.unwrap_err();

        let ClientError::Rejected { reason, message } = refusal else {
            panic!("a version mismatch must be a rejection, not {refusal:?}");
        };
        assert_eq!(reason, RejectReason::ProtocolVersion);
        assert!(
            message.contains(&version.to_string())
                && message.contains(&prism_ipc::PROTOCOL_VERSION.to_string()),
            "the refusal has to name both versions: {message}"
        );
        assert!(reason.closes_the_connection());
    }

    // Nobody was ever connected, and the rig never knew.
    assert_eq!(server.client_count().await, 0);
    let before = frames.frame_count();
    until("the rig to be driven after the refusals", async || {
        frames.frame_count() > before + 10
    })
    .await;

    // And a client of the right version is served straight afterwards, which is
    // what makes the refusal a refusal rather than a broken listener.
    let wire = prism_ipc::local::connect(&address).await.unwrap();
    let (client, snapshot) = Client::handshake(wire, Hello::new(ClientKind::Desktop))
        .await
        .unwrap();
    assert_eq!(
        snapshot.health.protocol_version,
        prism_ipc::PROTOCOL_VERSION
    );
    client.disconnect().await;

    let _ = stop.send(());
    let daemon = tokio::time::timeout(PATIENCE, running)
        .await
        .expect("the daemon must stop when it is told to")
        .unwrap();
    daemon.shutdown().await;
}

/// **The exit criterion.** A deliberately slow client loses telemetry, loses no
/// command, and does not affect anybody else.
///
/// S16 built the policy and asserted it through a socket with a 512-byte buffer.
/// What it could not assert is this: that a real daemon, sending real telemetry
/// at 30 Hz to a client that has stopped reading, drops the pictures and keeps
/// every delta — and that the client beside it never notices. The three claims
/// are measured separately, because they fail separately.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_client_loses_telemetry_and_no_commands_and_nobody_else_notices() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    // A rig across many universes, so §7's channel has something to carry: a
    // telemetry frame is 514 bytes per patched universe, and a client that is
    // not reading has to fall behind a socket buffer before any of this is
    // about backpressure at all.
    common::write_wide_show(&dir.path().join("aula.prism"), common::WIDE_UNIVERSES);

    let mut options = options(dir.path());
    options.universes = Some(common::WIDE_UNIVERSES);
    let mut daemon = Daemon::start(&options).await.unwrap();
    let frames = daemon.recorded_outputs()[0].clone();
    let server = daemon.server().clone();
    let address = common::local_address(dir.path());

    let (stop, stopped) = tokio::sync::oneshot::channel();
    let running = tokio::spawn(async move {
        daemon
            .run(None, async {
                let _ = stopped.await;
            })
            .await;
        daemon
    });

    // The slow one reads its snapshot — which the handshake requires — and then
    // nothing at all.
    let wire = prism_ipc::local::connect(&address).await.unwrap();
    let (mut slow, _snapshot) = Client::handshake(wire, Hello::new(ClientKind::WebRemote))
        .await
        .unwrap();
    until("the slow client to be served", async || {
        server.client_count().await == 1
    })
    .await;

    // The one that keeps up, connected second so the ids are known.
    let wire = prism_ipc::local::connect(&address).await.unwrap();
    let (mut fast, _snapshot) = Client::handshake(wire, Hello::new(ClientKind::Desktop))
        .await
        .unwrap();
    until("both clients to be served", async || {
        server.client_count().await == 2
    })
    .await;
    let clients = server.clients().await;
    assert_eq!(clients.len(), 2);
    let (slow_id, fast_id) = (clients[0], clients[1]);

    // Telemetry is what gets dropped, and it is dropped without anybody being
    // disconnected over it (§7: droppable by definition).
    //
    // *More* frames dropped than delivered, rather than merely one: a socket
    // buffer absorbs the first second of a channel nobody is reading, so a
    // client that had lost a single frame would not yet be a client this test
    // has anything to say about. Scale-free on purpose — it is a statement
    // about the client being behind, not about how big a pipe buffer is on
    // whichever operating system this is running on.
    until("the slow client to fall behind the telemetry", async || {
        // The fast client is read on every turn of this wait, because that is
        // the only thing that makes it the fast one. See `drain_ready`.
        drain_ready(&mut fast).await;
        server.stats(slow_id).await.is_some_and(|stats| {
            stats.telemetry_dropped > stats.telemetry_sent && stats.telemetry_dropped >= 20
        })
    })
    .await;
    assert_eq!(
        server.client_count().await,
        2,
        "telemetry must never be a reason to disconnect anybody"
    );

    // Now the commands, from the client that is reading. Each one is a session
    // change with a distinguishable value in it, so what the slow client
    // receives can be checked for order and for completeness rather than only
    // for a count.
    let pages: Vec<u32> = (10..10 + common::SLOW_CLIENT_COMMANDS).collect();
    let began = Instant::now();
    for &page in &pages {
        apply_and_wait(&mut fast, Command::SetExecutorPage { page }, |_| {}).await;
    }
    let round_trips = began.elapsed();

    // *…and does not affect other clients.* The fast client's commands were
    // answered while the slow one sat there not reading a byte.
    println!(
        "backpressure: {} command round trips in {round_trips:?} beside a client that reads nothing",
        pages.len()
    );
    assert!(
        round_trips < Duration::from_secs(5),
        "a slow client held up a fast one for {round_trips:?}"
    );

    let slow_stats = server.stats(slow_id).await.expect("still connected");
    let fast_stats = server.stats(fast_id).await.expect("still connected");
    // A ratio rather than "fewer": the claim is that the fast client is not
    // behind *in the way the slow one is*, and one frame lost to a scheduler
    // hiccup on a busy runner is not that. With the slow client at twenty or
    // more, this leaves the fast one at most four.
    assert!(
        fast_stats.telemetry_dropped * 4 < slow_stats.telemetry_dropped,
        "the fast client dropped {} telemetry frames and the slow one {}",
        fast_stats.telemetry_dropped,
        slow_stats.telemetry_dropped
    );
    assert!(
        fast_stats.control_high_water < slow_stats.control_high_water,
        "the fast client was {} control messages behind and the slow one {}",
        fast_stats.control_high_water,
        slow_stats.control_high_water
    );
    assert_eq!(
        server.client_count().await,
        2,
        "neither client has been given up on"
    );

    // And then the slow one starts reading again. Every command's delta is
    // there, in order and complete: §8 drops telemetry and never a control
    // message, and a client that missed one would hold a state that silently
    // disagreed with the daemon's.
    let mut seen = Vec::new();
    let mut telemetry = 0_u32;
    while seen.len() < pages.len() {
        match next_event(&mut slow).await {
            Some(ClientEvent::Delta(Delta::SessionPatch { ops })) => {
                seen.extend(ops.iter().filter_map(executor_page_in));
            }
            Some(ClientEvent::Telemetry(_)) => telemetry += 1,
            Some(ClientEvent::Delta(_) | ClientEvent::Ack { .. }) => {}
            other => panic!(
                "the slow client had {} of {} deltas when the connection gave up: {other:?}",
                seen.len(),
                pages.len()
            ),
        }
    }
    assert_eq!(seen, pages, "a control message was dropped or reordered");
    println!(
        "backpressure: the slow client was given {} of {} telemetry frames and dropped {}; \
         the fast client dropped {}",
        telemetry,
        slow_stats.telemetry_sent + slow_stats.telemetry_dropped,
        slow_stats.telemetry_dropped,
        fast_stats.telemetry_dropped,
    );

    // The rig ran through the whole of it.
    let before = frames.frame_count();
    until("the rig to be driven throughout", async || {
        frames.frame_count() > before + 10
    })
    .await;

    fast.disconnect().await;
    slow.disconnect().await;
    let _ = stop.send(());
    let daemon = tokio::time::timeout(PATIENCE, running)
        .await
        .expect("the daemon must stop when it is told to")
        .unwrap();
    daemon.shutdown().await;
}

/// The executor page a session operation set, if that is what it is.
fn executor_page_in(op: &JsonPatchOp) -> Option<u32> {
    match op {
        JsonPatchOp::Replace { path, value } | JsonPatchOp::Add { path, value }
            if path.ends_with("executorPage") =>
        {
            match value {
                JsonValue::Int(page) => u32::try_from(*page).ok(),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A programmer state is a value, and this file compares two of them — so the
/// comparison has to be one an unchanged programmer would pass and a changed one
/// would not.
#[test]
fn a_programmer_state_that_differs_compares_unequal() {
    let mut changed = ProgrammerState::default();
    assert_eq!(changed, ProgrammerState::default());
    changed.selection.push(FixtureId::new(1));
    assert_ne!(changed, ProgrammerState::default());
}

/// The gap analysis is the whole of the D2 gate's first claim, so it is checked
/// against a recording whose answer is known rather than trusted.
#[test]
fn the_gap_analysis_finds_a_silence_and_ignores_a_steady_stream() {
    let start = Instant::now();
    let record = |offset: u64, universe: u32| FrameRecord {
        at: start + Duration::from_millis(offset),
        universe: UniverseId::new(universe),
        data: vec![0; 512],
    };
    // Universe 1 is steady at 20 ms; universe 2 stops for 400 ms in the middle,
    // and the two are interleaved the way one output carrying both records them.
    let timeline = vec![
        record(0, 1),
        record(0, 2),
        record(20, 1),
        record(20, 2),
        record(40, 1),
        record(440, 2),
        record(60, 1),
    ];

    let steady = one_universe(&timeline, UniverseId::new(1));
    assert_eq!(steady.len(), 4);
    assert_eq!(longest_gap(&steady).0, Duration::from_millis(20));

    let stalled = one_universe(&timeline, UniverseId::new(2));
    assert_eq!(longest_gap(&stalled).0, Duration::from_millis(420));
    assert!(longest_gap(&stalled).0 > MAX_GAP);
    assert_eq!(
        longest_gap(&stalled).1,
        Some(start + Duration::from_millis(20))
    );

    // And a recording of one frame has no gap to report rather than a panic.
    assert_eq!(longest_gap(&[]).0, Duration::ZERO);
    assert_eq!(longest_gap(&steady[..1]).1, None);
}
