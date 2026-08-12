//! What the daemon wires together, exercised through a running daemon.
//!
//! `tests/daemon.rs` asserts the four exit criteria. This target is the rest of
//! the wiring — the WebSocket listener, the telemetry channel, the outputs and
//! the two failure paths a daemon has to survive on the way up — and it exists
//! for the reason every session since S11 has recorded: **the uncovered line is
//! where the defect is**, and "it is only an error path" has now been wrong
//! eight times running.
//!
//! # No test here sends multicast or broadcast
//!
//! S10 made that a rule rather than an omission: a suite that put lighting data
//! on the network it runs on is the fault `ARCHITECTURE_SPEC.md` §7.2 exists to
//! prevent. So the sACN output under test is **unicast to a loopback socket** —
//! `--sacn-to`, which is not a test hook but the configuration §7.2 names for a
//! venue whose network forbids multicast — and the Art-Net one is unicast to
//! loopback, which is its default anyway.

// Every test here holds `common::one_daemon_at_a_time` across its awaits, which
// is what the guard is for: a daemon owns a real-time tick thread, and several
// of those in one process measure each other rather than the daemon. The lint
// exists for a lock that another task on the same runtime might want — nothing
// else in this process wants this one, and each test's guard is dropped when
// its daemon is.
#![allow(clippy::await_holding_lock)]

use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::time::Duration;

use prism_domain::{OutputHealth, UniverseId};
use prism_ipc::{Client, ClientEvent, ClientKind, Hello};
use prism_protocols::OutputError;
use prismd::cli::{Options, OutputSpec};
use prismd::daemon::Daemon;

mod common;

fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: 2,
        outputs: vec![OutputSpec::Mock],
        local: false,
        log_level: prismd::log::Level::Warn,
        ..Options::default()
    }
}

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

/// The lock document, read the way a client reads it (§2.2).
fn lock_document(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("prismd.lock")).unwrap()).unwrap()
}

/// A socket that receives whatever an output is pointed at, and its address.
fn receiver() -> (UdpSocket, SocketAddr) {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let address = socket.local_addr().unwrap();
    (socket, address)
}

/// The Web Remote's transport, end to end, and the token §2.1 asks for.
#[tokio::test]
async fn a_websocket_client_is_served_the_world_and_finds_the_daemon_by_its_lock_file() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut options = options(dir.path());
    // Port 0: the operating system chooses, and the lock file is how anybody
    // finds out which — which is exactly what §2.2 is for.
    options.websocket = Some("127.0.0.1:0".parse().unwrap());
    options.token = Some("hunter2".to_owned());
    let mut daemon = Daemon::start(&options).await.unwrap();

    let document = lock_document(dir.path());
    let address = document["websocket"].as_str().unwrap().to_owned();
    assert_ne!(
        address, "127.0.0.1:0",
        "the port actually bound is published"
    );
    assert_eq!(document["token"].as_str(), Some("hunter2"));
    assert!(daemon.endpoints().contains("ws://"));

    let url = format!("ws://{address}/ipc");
    let connecting = tokio::spawn({
        let url = url.clone();
        async move {
            let wire = prism_ipc::websocket::connect(&url).await.unwrap();
            Client::handshake(
                wire,
                Hello::new(ClientKind::WebRemote).with_token("hunter2"),
            )
            .await
        }
    });
    let handshake = tokio::select! {
        result = connecting => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("the client never connected")
        }
    };
    let (client, snapshot) = handshake.unwrap();
    let show = prism_core::JsonMirror::new(snapshot.show.clone());
    assert_eq!(
        show.get("/fixtures/1/name").unwrap(),
        &prism_domain::JsonValue::String("Fixture 1".to_owned())
    );
    client.disconnect().await;

    // And the same listener without the token turns a client away in words
    // rather than accepting it.
    let refused = tokio::spawn(async move {
        let wire = prism_ipc::websocket::connect(&url).await.unwrap();
        Client::handshake(wire, Hello::new(ClientKind::WebRemote)).await
    });
    let refusal = tokio::select! {
        result = refused => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("the second client was never answered")
        }
    };
    assert!(
        refusal.is_err(),
        "§2.1: a listener that is not on loopback needs the token"
    );

    daemon.shutdown().await;
}

/// §7's channel, carrying what the fixtures are actually being given.
#[tokio::test]
async fn telemetry_reaches_a_connected_client_and_carries_the_patched_universes() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));
    let mut options = options(dir.path());
    options.local = true;
    let mut daemon = Daemon::start(&options).await.unwrap();

    let address = common::local_address(dir.path());
    let listening = tokio::spawn(async move {
        let wire = prism_ipc::local::connect(&address).await.unwrap();
        let (mut client, _snapshot) = Client::handshake(wire, Hello::new(ClientKind::Desktop))
            .await
            .unwrap();
        loop {
            match client.next_event().await {
                Some(Ok(ClientEvent::Telemetry(frame))) => return frame,
                Some(Ok(_)) => {}
                other => panic!("the connection ended before any telemetry: {other:?}"),
            }
        }
    });
    let frame = tokio::select! {
        result = listening => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("no telemetry frame arrived")
        }
    };

    // The show patches universes 1 and 2, and the frame carries those and not
    // the sixty-four the layout has room for — a client on a school network
    // should not be sent a megabyte a second of nothing.
    assert_eq!(
        frame
            .universes
            .iter()
            .map(|levels| levels.universe)
            .collect::<Vec<_>>(),
        vec![UniverseId::new(1), UniverseId::new(2)]
    );
    let first = &frame.universes[0];
    assert_eq!(
        first.levels[0], 255,
        "the picture a client is shown is the frame the fixtures got"
    );

    daemon.shutdown().await;
}

/// §7's other half: a status light per interface, and it changes when the
/// interface does.
#[tokio::test]
async fn an_output_that_falls_over_is_reported_to_every_client() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));
    let mut options = options(dir.path());
    options.local = true;
    let mut daemon = Daemon::start(&options).await.unwrap();

    let address = common::local_address(dir.path());
    let watching = tokio::spawn(async move {
        let wire = prism_ipc::local::connect(&address).await.unwrap();
        let (mut client, _snapshot) = Client::handshake(wire, Hello::new(ClientKind::Desktop))
            .await
            .unwrap();
        loop {
            match client.next_event().await {
                Some(Ok(ClientEvent::Delta(prism_domain::Delta::OutputHealth {
                    health, ..
                }))) if health != OutputHealth::Ok => return health,
                Some(Ok(_)) => {}
                other => panic!("the connection ended: {other:?}"),
            }
        }
    });

    // The cable comes out and stays out, and it does so **while the daemon is
    // running**. The failed send is what takes the interface down; the failed
    // reconnections are what keep it down for longer than the housekeeping
    // interval.
    //
    // **Two conditions have to hold before it can come out, and a fixed 200 ms
    // sleep was guessing at both.** S18 reproduced the guess failing by running
    // this file under six CPU burners:
    //
    // - *the client has to be listening.* A delta goes to the clients connected
    //   when it happens and is never replayed to one that arrives afterwards.
    // - *the cable has to have been in long enough to be noticed.* The daemon
    //   learns an output's health by **polling** it every housekeeping interval
    //   (500 ms), so a state that appears and disappears between two polls was
    //   never there as far as any client is concerned. On a loaded runner the
    //   driver reported connected and the cable came out in the same interval:
    //   the daemon's before and after were both `Disconnected`, there was no
    //   edge, and the test sat until its deadline having proved nothing. That
    //   is a fact about a poll rather than a defect — the light always
    //   converges on the current health — but it is a fact a test has to
    //   respect.
    let cable = daemon.recorded_outputs()[0].clone();
    let server = daemon.server().clone();
    let status = daemon.desk().outputs()[0].status.clone();
    tokio::spawn(async move {
        /// Longer than two housekeeping intervals, so the daemon cannot have
        /// missed the interface coming up.
        const CONNECTED_FOR: Duration = Duration::from_millis(1_200);

        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let mut up_since = None;
        loop {
            match (status.health(), up_since) {
                (OutputHealth::Ok, None) => up_since = Some(tokio::time::Instant::now()),
                (OutputHealth::Ok, Some(_)) => {}
                _ => up_since = None,
            }
            let settled = up_since.is_some_and(|since| since.elapsed() >= CONNECTED_FOR);
            if (settled && server.client_count().await > 0)
                || tokio::time::Instant::now() >= deadline
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        cable.fail_connect(64, OutputError::Disconnected);
        cable.fail_send(1, OutputError::Disconnected);
    });

    // Thirty seconds rather than ten, and it is a deadline rather than a wait:
    // the cable now stays in for over a second before it comes out, and on a
    // machine slow enough for that to matter the daemon then has a poll and a
    // broadcast to do. A budget that is only just enough is a test that fails
    // for the wrong reason.
    let health = tokio::select! {
        result = watching => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(30)), std::future::pending()) => {
            panic!("nobody was told the output had gone")
        }
    };
    assert_eq!(health, OutputHealth::Disconnected);

    daemon.shutdown().await;
}

/// The Art-Net output, opened by the daemon and pointed at a loopback socket.
#[tokio::test]
async fn an_artnet_output_puts_the_shows_look_on_the_wire() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));
    let (socket, address) = receiver();

    let mut options = options(dir.path());
    options.outputs = vec![OutputSpec::ArtNet { target: address }];
    let daemon = Daemon::start(&options).await.unwrap();

    let received = tokio::task::spawn_blocking(move || {
        let mut datagram = vec![0u8; 1024];
        let (length, _) = socket.recv_from(&mut datagram).unwrap();
        datagram.truncate(length);
        datagram
    })
    .await
    .unwrap();

    assert_eq!(&received[..8], prism_protocols::ART_NET_ID);
    assert_eq!(
        received.len(),
        prism_protocols::ART_DMX_BYTES,
        "an ArtDmx packet is 530 bytes"
    );
    // Channel 1 of universe 1 is the dimmer at home, and the data begins after
    // the eighteen-byte header.
    let data = &received[prism_protocols::ART_DMX_HEADER..];
    assert!(
        data[0] == 255 || data[0] == 0,
        "universe 1 is full and universe 2 is dark, and either may arrive first"
    );

    daemon.shutdown().await;
}

/// The sACN output, unicast — which is a real configuration (§7.2) and the only
/// one a test suite is allowed to use.
#[tokio::test]
async fn an_sacn_output_unicast_carries_this_desks_identity() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));
    let (socket, address) = receiver();

    let mut options = options(dir.path());
    options.outputs = vec![OutputSpec::Sacn {
        unicast: Some(address),
    }];
    let daemon = Daemon::start(&options).await.unwrap();

    // The identity the daemon made on this start is the CID on the wire, which
    // is the whole of S10's decision arriving where it was going.
    let machine: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("machine.json")).unwrap())
            .unwrap();
    let desk_id = prism_core::DeskId::parse(machine["deskId"].as_str().unwrap()).unwrap();

    let received = tokio::task::spawn_blocking(move || {
        let mut datagram = vec![0u8; 1024];
        let (length, _) = socket.recv_from(&mut datagram).unwrap();
        datagram.truncate(length);
        datagram
    })
    .await
    .unwrap();

    assert_eq!(received.len(), prism_protocols::E131_DATA_BYTES);
    assert_eq!(&received[4..16], prism_protocols::ACN_PACKET_IDENTIFIER);
    assert_eq!(
        &received[22..38],
        &desk_id.into_bytes(),
        "the CID is this desk's identity and not a fresh one per start"
    );
    // The source name is the show's own file name (§7.2).
    let name = String::from_utf8_lossy(&received[44..108]);
    assert!(name.starts_with("PrismDMX aula"), "{name}");

    daemon.shutdown().await;
}

// There is deliberately **no test here that opens the Open DMX USB adapter.**
// `CLAUDE.md` requires every test to run deterministically with no device
// attached, and a test that asked the daemon for an `--open-dmx` output would
// put a frame on a real cable on any machine that had one plugged in — this
// one included, which is where S8 measured the adapter. The arm that builds it
// is four lines and is exercised as far as it can be without hardware by
// `cli.rs`'s parser test; `crates/prism-protocols/tests/hardware.rs` is where
// the device itself is driven, behind `#[ignore]`.

/// A daemon that cannot open its show says so **and lets go of the lock**.
///
/// The second half is the one that matters: a daemon that failed on the way up
/// and left the guard held would make the machine unstartable until it was
/// rebooted.
#[tokio::test]
async fn a_daemon_that_cannot_open_its_show_releases_the_lock() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.show = Some(dir.path().join("not-a-show.prism"));
    std::fs::write(options.show.as_ref().unwrap(), b"this is not a database").unwrap();

    let error = Daemon::start(&options).await.unwrap_err();
    assert!(error.to_string().contains("could not be opened"), "{error}");

    // The next daemon starts, which it could not do if the guard were still
    // held or the discovery file still there.
    let mut good = options.clone();
    good.show = Some(dir.path().join("aula.prism"));
    let daemon = Daemon::start(&good).await.unwrap();
    daemon.shutdown().await;
}

/// A recovery copy from an earlier run is unsaved work somebody should be
/// offered, not something to clear away quietly (S15).
#[tokio::test]
async fn a_recovery_copy_left_by_an_earlier_run_is_found() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let show = dir.path().join("aula.prism");
    common::write_show(&show);
    // What a killed daemon leaves: the autosave copy beside the show.
    let file = common::show_file();
    let store = prism_core::ShowStore::open(&show).unwrap();
    store.write_recovery(&file).unwrap();
    assert!(store.has_recovery());
    drop(store);

    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    assert!(
        daemon.desk().core().store().has_recovery(),
        "the daemon must not have thrown the operator's unsaved work away"
    );
    daemon.shutdown().await;
}

/// A daemon with no listener at all is a daemon: `ARCHITECTURE_SPEC.md` §10.2's
/// Raspberry Pi lighting server, before anybody has attached a Web Remote.
#[tokio::test]
async fn a_daemon_with_no_listener_still_runs_and_still_publishes_its_pid() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    assert_eq!(daemon.endpoints(), "no endpoint");
    assert_eq!(
        lock_document(dir.path())["pid"].as_u64(),
        Some(u64::from(std::process::id())),
        "there is still a process somebody has to be able to find"
    );
    let output = daemon.desk().outputs()[0].status.clone();
    until("the rig to be driven anyway", || output.frames_sent() > 5).await;

    daemon.shutdown().await;
}
