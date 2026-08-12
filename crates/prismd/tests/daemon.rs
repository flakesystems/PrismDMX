//! The exit criteria for S17, asserted against a daemon that is actually
//! running.
//!
//! Four of them, and each is a claim about a process rather than about a
//! function:
//!
//! - the daemon starts, loads a show and outputs DMX **with no client ever
//!   connecting** — which is the whole of D2 stated as a test;
//! - a second instance detects the first and refuses to start;
//! - a lock file left by a killed process is detected and taken over;
//! - the handshake serves a `Snapshot` carrying the show **and** the session.
//!
//! Every wait here has a deadline. S16 pushed a test that blocked on a message
//! a two-core runner never delivered and it cost forty minutes of a CI job; a
//! named failure in seconds is worth more than a job that stops.

// Every test here holds `common::one_daemon_at_a_time` across its awaits, which
// is what the guard is for: a daemon owns a real-time tick thread, and several
// of those in one process measure each other rather than the daemon. The lint
// exists for a lock that another task on the same runtime might want — nothing
// else in this process wants this one, and each test's guard is dropped when
// its daemon is.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::ShowStore;
use prism_domain::{AttributeType, Command, FixtureId, SelectionMode, UniverseId};
use prism_ipc::{Client, ClientEvent, ClientKind, Hello};
use prismd::cli::{Options, OutputSpec};
use prismd::daemon::Daemon;

mod common;

/// A daemon in the shape the criteria describe: its own data directory, a show
/// written before it starts, and a mock output.
fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: 2,
        outputs: vec![OutputSpec::Mock],
        // The daemon under test opens the local transport; the tests that do
        // not need a client switch it off, because a pipe name is global to the
        // machine and two of these run at once under `cargo test`.
        local: false,
        log_level: prismd::log::Level::Warn,
        ..Options::default()
    }
}

/// Waits for `condition`, or fails by name.
async fn until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}");
}

/// **The exit criterion.** A daemon starts, loads a show, and drives DMX with
/// nobody watching.
#[tokio::test]
async fn a_daemon_loads_a_show_and_drives_dmx_with_no_client_connected() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let desk = daemon.desk().clone();

    // The show is the one on disk, not an empty one.
    assert_eq!(desk.core().file.show.fixtures().count(), 3);

    // And it reaches the wire. The mock output's frames are the assertion: a
    // daemon that held a show and published nothing would pass every other test
    // in this file.
    let output = desk.outputs()[0].status.clone();
    until("frames to reach the output", || output.frames_sent() > 10).await;
    assert_eq!(
        prism_domain::OutputHealth::Ok,
        output.health(),
        "the output is up"
    );

    // Let it run for a second under its own timers, with nobody connected —
    // which is the point of the criterion — and then ask what the tick managed.
    // A second rather than the fifty milliseconds it takes to see a frame:
    // `tick_hz` is ticks divided by uptime, and over a short window the
    // milliseconds between the engine starting and the clock starting are a
    // measurable share of it. This is S10's rule about percentiles applied to a
    // mean: a number is a gate only where the window supports it.
    assert_eq!(daemon.desk().outputs().len(), 1);
    daemon
        .run(Some(Duration::from_secs(1)), std::future::pending())
        .await;

    let health = desk.snapshot().health;
    assert!(
        health.tick_hz > 30.0 && health.tick_hz < 55.0,
        "the tick should be near 44 Hz, and reads {} Hz",
        health.tick_hz
    );
    assert!(
        output.frames_sent() > 30,
        "and it should have driven the rig the whole time, not just at first"
    );

    daemon.shutdown().await;
}

/// **The exit criterion.** A second instance detects the first and refuses.
#[tokio::test]
async fn a_second_daemon_refuses_to_start_and_says_where_the_first_one_is() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let first = Daemon::start(&options(dir.path())).await.unwrap();
    let refusal = Daemon::start(&options(dir.path())).await.unwrap_err();
    let message = refusal.to_string();
    assert!(
        message.contains("already running"),
        "a second daemon must be told why: {message}"
    );
    assert!(
        message.contains(&std::process::id().to_string()),
        "and which process has it: {message}"
    );

    // Two daemons driving one rig is the failure this prevents, so the first
    // one has to be unaffected by the attempt.
    let output = first.desk().outputs()[0].status.clone();
    let before = output.frames_sent();
    until("the first daemon to carry on", || {
        output.frames_sent() > before + 5
    })
    .await;

    first.shutdown().await;

    // And once it has finished, the address is free.
    let third = Daemon::start(&options(dir.path())).await.unwrap();
    third.shutdown().await;
}

/// **The exit criterion.** A lock file left by a killed process is taken over.
///
/// The kill is simulated the only way it can be from inside one process: the
/// file system is put in exactly the state the operating system leaves it in
/// when it reaps a daemon — the discovery document still there, naming a
/// process that is gone, and nothing holding the guard.
#[tokio::test]
async fn a_lock_file_from_a_killed_daemon_is_taken_over() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let corpse = serde_json::json!({
        "pid": 4711,
        "local": dir.path().join("prismd.sock").to_string_lossy(),
        "websocket": "127.0.0.1:7373",
    });
    let lock_path = dir.path().join("prismd.lock");
    std::fs::write(&lock_path, serde_json::to_vec_pretty(&corpse).unwrap()).unwrap();
    // The socket file a Unix daemon leaves behind, which `LocalListener::bind`
    // deliberately does not sweep up (S16) because deciding a daemon is dead is
    // a liveness question and belongs to this session.
    std::fs::write(dir.path().join("prismd.sock"), b"stale").unwrap();

    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&lock_path).unwrap()).unwrap();
    assert_eq!(
        document["pid"].as_u64(),
        Some(u64::from(std::process::id())),
        "the lock file names the daemon that is running now"
    );
    assert!(
        !dir.path().join("prismd.sock").exists(),
        "the dead daemon's socket file has to go, or the next bind fails"
    );

    daemon.shutdown().await;
    assert!(
        !lock_path.exists(),
        "a daemon that stopped cleanly leaves nothing for a client to find"
    );
}

/// **The exit criterion.** The handshake serves a `Snapshot` with the show and
/// the session.
///
/// Over a real transport and a real client, because the handshake is the one
/// thing in this session that only `prism-ipc` can perform.
#[tokio::test]
async fn a_client_connects_and_is_served_the_show_and_the_session() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut options = options(dir.path());
    options.local = true;
    let mut daemon = Daemon::start(&options).await.unwrap();

    // The daemon's own accept loop runs on the runtime, so the daemon has to be
    // running while the client connects.
    let address = common::local_address(dir.path());
    let connecting = tokio::spawn(async move {
        let wire = prism_ipc::local::connect(&address).await.unwrap();
        Client::handshake(wire, Hello::new(ClientKind::Desktop))
            .await
            .unwrap()
    });
    let (mut client, snapshot) = tokio::select! {
        result = connecting => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("the client never connected")
        }
    };

    // The show document, read the way a client reads it.
    let show = prism_core::JsonMirror::new(snapshot.show.clone());
    assert_eq!(
        show.get("/fixtures/1/name").unwrap(),
        &prism_domain::JsonValue::String("Fixture 1".to_owned())
    );
    // The session document, and this session was saved off its defaults so that
    // a snapshot which dropped it would not look identical to one that carried
    // a fresh session (S14's rule about fixtures made of default values).
    let session = prism_core::JsonMirror::new(snapshot.session.clone());
    assert_eq!(
        session.get("/session/executorPage").unwrap(),
        &prism_domain::JsonValue::Int(3)
    );
    assert_eq!(
        session.get("/session/commandLine").unwrap(),
        &prism_domain::JsonValue::String("fixture 1 at full".to_owned())
    );
    assert_eq!(snapshot.outputs.len(), 1);
    assert_eq!(
        snapshot.health.protocol_version,
        prism_ipc::PROTOCOL_VERSION
    );
    // Deliberately no assertion about the tick rate here. It is *measured*, so
    // it is zero until the first tick has run, and how soon that is depends on
    // how busy the machine is rather than on anything this test is about —
    // which is precisely S13's rule that a red timing assertion is a question
    // about the machine first. The rate is asserted where it can be waited for,
    // in `a_daemon_loads_a_show_and_drives_dmx_with_no_client_connected`.

    // And the connection is a working one: a command reaches the show, the
    // delta comes back before the acknowledgement, and the rig follows.
    let running = tokio::spawn(async move {
        client
            .send(Command::SelectFixtures {
                ids: vec![FixtureId::new(1)],
                mode: SelectionMode::Set,
            })
            .await
            .unwrap();
        let seq = client
            .send(Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                value: 0,
                relative: false,
            })
            .await
            .unwrap();
        let mut acknowledged = false;
        let mut programmer_changed = false;
        while !acknowledged {
            match client.next_event().await {
                Some(Ok(ClientEvent::Delta(prism_domain::Delta::ProgrammerChanged { .. }))) => {
                    programmer_changed = true;
                }
                Some(Ok(ClientEvent::Ack { seq: acked })) if acked == seq => acknowledged = true,
                Some(Ok(_)) => {}
                other => panic!("the connection ended: {other:?}"),
            }
        }
        assert!(
            programmer_changed,
            "the fact travels before the receipt (docs/IPC_PROTOCOL.md section 5)"
        );
        client
    });
    let client = tokio::select! {
        client = running => client.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("the command was never answered")
        }
    };

    client.disconnect().await;
    // §8: the daemon frees the client's state and carries on. The rig is still
    // being driven, which is the property D2 exists for.
    let output = daemon.desk().outputs()[0].status.clone();
    let before = output.frames_sent();
    until("the daemon to carry on without its client", || {
        output.frames_sent() > before + 5
    })
    .await;

    daemon.shutdown().await;
}

/// The other half of §10.3's shutdown: *a configurable blackout-or-hold*.
///
/// Blackout is a frame rather than a flag (S10), so the assertion is on the
/// bytes the output was given last — not on a flag somewhere.
#[tokio::test]
async fn a_daemon_told_to_black_out_publishes_a_blackout_before_it_stops() {
    for (exit, expected) in [
        (prismd::cli::Exit::Hold, 255),
        (prismd::cli::Exit::Blackout, 0),
    ] {
        let _turn = common::one_daemon_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        common::write_show(&dir.path().join("aula.prism"));
        let mut options = options(dir.path());
        options.exit = exit;

        // The mock output's recording, which outlives the daemon: what matters
        // is the frame that went out **last**, and that can only be read after
        // the shutdown has finished.
        let daemon = Daemon::start(&options).await.unwrap();
        let frames = daemon.recorded_outputs()[0].clone();
        until("the rig at home", || {
            common::last_frame_of(&frames, UniverseId::new(1)).is_some_and(|data| data[0] == 255)
        })
        .await;

        daemon.shutdown().await;
        let data = common::last_frame_of(&frames, UniverseId::new(1)).unwrap();
        assert_eq!(
            data[0], expected,
            "with {exit:?} the last frame on the wire should be {expected}"
        );
    }
}

/// A daemon that has to make an identity makes exactly one, and the next start
/// reads it back.
///
/// S10's failure mode: a desk with a fresh CID at every start is a *new source*
/// every start, and the previous one holds the universe for two and a half
/// seconds while the two fight.
#[tokio::test]
async fn a_desk_identity_is_made_once_and_then_kept() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let first = Daemon::start(&options(dir.path())).await.unwrap();
    first.shutdown().await;
    let after_first = std::fs::read_to_string(dir.path().join("machine.json")).unwrap();

    let second = Daemon::start(&options(dir.path())).await.unwrap();
    second.shutdown().await;
    assert_eq!(
        std::fs::read_to_string(dir.path().join("machine.json")).unwrap(),
        after_first,
        "a second start must not invent a second identity"
    );
    assert!(after_first.contains("deskId"));
    assert!(
        !after_first.contains("00000000-0000-0000-0000-000000000000"),
        "the nil CID is not an identity and an sACN output refuses it"
    );
}

/// A daemon started against a file that is not there makes one, which is what
/// lets a school open the box and turn it on.
#[tokio::test]
async fn a_daemon_with_no_show_makes_one_and_still_drives_dmx() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    assert!(dir.path().join("aula.prism").is_file());
    assert_eq!(daemon.desk().core().file.show.fixtures().count(), 0);

    // Nothing is patched, so the frame is all zeroes — and it is still being
    // published, which is what tells a rig the desk is alive.
    let output = daemon.desk().outputs()[0].status.clone();
    until("frames from an empty show", || output.frames_sent() > 5).await;
    daemon.shutdown().await;

    // And the file it made is a show file the store will open again.
    let store = ShowStore::open(dir.path().join("aula.prism")).unwrap();
    store.integrity_check().unwrap();
}
