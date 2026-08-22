//! **The D11 gate**, and the rest of what attaching a surface to a daemon has
//! to be true of.
//!
//! `ARCHITECTURE_SPEC.md` §12 states the gate in one line: *mock MIDI sends
//! `Channel ▶` and F1 **with no UI client connected**; a client then connects
//! and finds both the new view and the opened window in its `Snapshot`.* That is
//! decision D11 in its entirety — the console does not remote-control the
//! interface, it changes state in the daemon, and the interface finds that state
//! when it arrives.
//!
//! The order is the whole point, so it is enforced rather than assumed: the
//! test asserts the daemon has **zero** clients at the moment the buttons are
//! pressed, and only then connects one.
//!
//! # Nothing here touches a device
//!
//! `CLAUDE.md`'s rule, and this file is where it would be tempting to break it.
//! The surface is a [`MockSurfacePort`]; the bytes pushed into it are written
//! out **by hand from `docs/MCU_MAPPING.md` §2.1** — `Channel ▶` is note 49 and
//! F1 is note 54, on channel 1, with velocity 127 for a press. Asking the
//! profile for those numbers would be asking the code under test what to press,
//! which is S19's finding and S20's method rule in one.

// Every test here holds `common::one_daemon_at_a_time` across its awaits — a
// daemon owns a tick thread at real-time priority, and two of those in one
// process measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::{SessionState, ShowStore};
use prism_domain::{Executor, ExecutorEncoderFunction, ExecutorFaderFunction, ExecutorId, ViewId};
use prism_ipc::{Client, ClientEvent, ClientKind, Hello};
use prism_surface::Bindings;
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;
use prismd::surface::MockSurfacePort;

mod common;

/// Note On, MIDI channel 1 — `docs/MCU_MAPPING.md` §2.1's "all note and CC
/// numbers are on MIDI channel 1", which is status `0x90`.
const NOTE_ON: u8 = 0x90;

/// `Channel ▶`, note 49 (§2.1, "Fader banks" row). D8 makes it `SelectView`.
const CHANNEL_RIGHT: u8 = 49;

/// F1, note 54 (§2.1, "Function" row: F1–F8 = 54–61).
const F1: u8 = 54;

/// SMPTE/Beats, note 53 — the button that must never be bound (§4.3).
const SMPTE_BEATS: u8 = 53;

fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: 2,
        outputs: vec![mock_output(1)],
        local: false,
        log_level: prismd::log::Level::Warn,
        ..Options::default()
    }
}

/// The rig of `common::show_file`, with a console-shaped session: page 0, so a
/// strip addresses the executor of the same number; executor 0 selected; and
/// **two** stored views, because `Channel ▶` means *the next one* and a desk
/// with one view has no next one.
fn write_console_show(path: &Path) {
    let mut file = common::show_file();
    file.show
        .store_executor(Executor {
            id: ExecutorId::new(1),
            sequence_id: None,
            fader_function: ExecutorFaderFunction::Master,
            button_functions: Vec::new(),
            encoder_function: ExecutorEncoderFunction::Empty,
            master_level: 12_345,
            speed: prism_domain::SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
        })
        .unwrap();
    let mut session = SessionState::new();
    session.store_view(ViewId::new(2), "Programming").unwrap();
    session.select_executor(Some(ExecutorId::new(0))).unwrap();
    file.session = session;
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

/// Runs the daemon in slices until `condition` holds, or fails by name.
///
/// In slices because the surface is polled inside `Daemon::run` — which is the
/// arrangement under test — and a condition can only be looked at between them.
/// Every wait has a deadline, per `tests/daemon.rs`: a named failure in seconds
/// is worth more than a job that stops.
async fn run_until(daemon: &mut Daemon, what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        daemon
            .run(Some(Duration::from_millis(5)), std::future::pending())
            .await;
    }
    panic!("timed out waiting for {what}");
}

/// **The D11 gate.** A console operates the interface while there is no
/// interface.
#[tokio::test]
async fn the_console_changes_the_session_with_no_client_connected_and_a_client_finds_it_afterwards()
{
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_console_show(&dir.path().join("aula.prism"));

    let mut options = options(dir.path());
    options.local = true;
    let mut daemon = Daemon::start(&options).await.unwrap();
    let (port, surface) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    let desk = daemon.desk().clone();

    // Nobody is watching. This is the precondition of the gate, so it is
    // asserted rather than described.
    assert_eq!(daemon.server().client_count().await, 0);
    assert_eq!(
        desk.core().file.session.session().active_view_id,
        ViewId::new(1),
        "the show opened on view 1"
    );
    assert!(desk.core().file.session.session().open_windows.is_empty());

    // Two presses on a desk with no interface attached to it.
    surface.press(NOTE_ON, CHANNEL_RIGHT);
    surface.press(NOTE_ON, F1);

    run_until(&mut daemon, "the console to change the session", || {
        let core = desk.core();
        let session = core.file.session.session();
        session.active_view_id == ViewId::new(2) && !session.open_windows.is_empty()
    })
    .await;
    assert_eq!(
        daemon.server().client_count().await,
        0,
        "and it happened with nobody connected"
    );

    // Now the interface arrives, and asks for the world.
    let address = common::local_address(dir.path());
    let connecting = tokio::spawn(async move {
        let wire = prism_ipc::local::connect(&address).await.unwrap();
        Client::handshake(wire, Hello::new(ClientKind::Desktop))
            .await
            .unwrap()
    });
    let (_client, snapshot) = tokio::select! {
        result = connecting => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("the client never connected")
        }
    };

    // **Both**, in the snapshot, because the session was already there when the
    // client asked for it.
    let session = prism_core::JsonMirror::new(snapshot.session.clone());
    assert_eq!(
        session.get("/session/activeViewId").unwrap(),
        &prism_domain::JsonValue::Int(2),
        "the view the console selected"
    );
    assert_eq!(
        session.get("/session/openWindows/0/type").unwrap(),
        &prism_domain::JsonValue::String("FixtureSheet".to_owned()),
        "the window the console opened"
    );

    daemon.shutdown().await;
}

/// **The other half of D11**, and the half S25 owes: a client is *connected*,
/// the console presses a button, and the client is told — without asking.
///
/// The gate above is about a console operating a desk with nobody watching.
/// This is about the interface following one that is: the delta the press
/// produced arrives on the same wire every other delta arrives on, because the
/// console and a client go through the same door. The browser's end of this is
/// `ui/e2e/session.spec.ts`, which watches the same thing happen in Chromium.
///
/// The surface here is `--mock-surface`: a file, opened by `Daemon::start` from
/// the command line, so what is under test is the arrangement an operator gets
/// rather than one a test wired up.
#[tokio::test]
async fn a_console_press_reaches_a_connected_client_as_a_delta() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_console_show(&dir.path().join("aula.prism"));
    let keys = dir.path().join("console.midi");

    let mut options = options(dir.path());
    options.local = true;
    options.mock_surface = Some(keys.clone());
    let mut daemon = Daemon::start(&options).await.unwrap();
    assert!(
        daemon.surface().is_some(),
        "--mock-surface attaches a surface at start-up, without a device"
    );

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
    // What the client believes before the press, from its own snapshot.
    let session = prism_core::JsonMirror::new(snapshot.session.clone());
    assert_eq!(
        session.get("/session/activeViewId").unwrap(),
        &prism_domain::JsonValue::Int(1)
    );

    // `Channel ▶` — the same three bytes the desk sends, appended to the file a
    // separate process would append them to.
    {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&keys)
            .expect("the daemon created the file when it opened it");
        file.write_all(&[NOTE_ON, CHANNEL_RIGHT, 127]).unwrap();
        file.write_all(&[NOTE_ON, CHANNEL_RIGHT, 0]).unwrap();
        file.flush().unwrap();
    }

    // The client is *told*. It asked for nothing; the delta arrived because the
    // daemon broadcasts what the console changed.
    let listening = tokio::spawn(async move {
        loop {
            match client.next_event().await {
                Some(Ok(ClientEvent::Delta(prism_domain::Delta::SessionPatch { ops }))) => {
                    if let Some(op) = ops.into_iter().next() {
                        return op;
                    }
                }
                Some(Ok(_)) => {}
                other => panic!("the connection ended: {other:?}"),
            }
        }
    });
    let op = tokio::select! {
        result = listening => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("the console press never reached the client")
        }
    };
    assert_eq!(
        op,
        prism_domain::JsonPatchOp::Replace {
            path: "/session/activeViewId".to_owned(),
            value: prism_domain::JsonValue::Int(2),
        }
    );

    daemon.shutdown().await;
}

/// A profile that will not parse never stops a daemon starting — the second
/// exit criterion, against a real file and a real process.
#[tokio::test]
async fn a_malformed_profile_leaves_the_desk_working_on_the_built_in_bindings() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_console_show(&dir.path().join("aula.prism"));
    let profile = dir.path().join("xtouch.json");
    // Not merely broken JSON: a profile that parses and then binds the one
    // button §4.3 reserves, which is the failure the loader exists to name.
    std::fs::write(
        &profile,
        r#"{"profileVersion":1,"device":"behringer-x-touch",
            "bindings":[{"control":"Global.SmpteBeats","action":{"t":"SaveShow"}}]}"#,
    )
    .unwrap();

    let mut options = options(dir.path());
    options.surface_profile = Some(profile);
    let mut daemon = Daemon::start(&options).await.unwrap();
    assert_eq!(
        daemon.bindings(),
        &Bindings::defaults(),
        "a refused profile leaves the built-in table in force"
    );

    let (port, surface) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    let desk = daemon.desk().clone();

    // And the desk works: the defaults are a working table, not an empty one.
    surface.press(NOTE_ON, F1);
    run_until(&mut daemon, "the default bindings to be in force", || {
        !desk.core().file.session.session().open_windows.is_empty()
    })
    .await;

    daemon.shutdown().await;
}

/// The reserved button reaches nothing, and is counted where somebody can see
/// it.
#[tokio::test]
async fn pressing_smpte_beats_does_nothing_at_all_and_is_counted() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_console_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let (port, surface) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    let desk = daemon.desk().clone();

    // The reserved button, and then an ordinary one. Waiting for the *second*
    // press to land is what makes this deterministic without a sleep: both
    // arrive through the same port in the same order, so a desk that has acted
    // on F1 has already seen SMPTE/Beats.
    surface.press(NOTE_ON, SMPTE_BEATS);
    surface.press(NOTE_ON, F1);
    run_until(&mut daemon, "the presses to be read", || {
        !desk.core().file.session.session().open_windows.is_empty()
    })
    .await;

    assert_eq!(
        daemon_reserved(&daemon),
        2,
        "both halves of the press are dropped where a status panel can see it"
    );
    // Nothing the reserved button could have done, happened. It is bound to
    // nothing and layer 2 drops it before layer 3 is asked at all.
    assert_eq!(
        desk.core().file.session.session().active_view_id,
        ViewId::new(1)
    );
    assert_eq!(desk.core().file.session.session().open_windows.len(), 1);

    daemon.shutdown().await;
}

/// A press the show refuses is an ordinary evening, not a fault.
#[tokio::test]
async fn a_press_the_show_refuses_changes_nothing_and_the_desk_carries_on() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_console_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let (port, surface) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    let desk = daemon.desk().clone();

    // Strip 8's Select is note 31 (§2.1: Select is notes 24-31), which on page 0
    // is executor 7 — an executor this show has not got. The desk refuses it,
    // and an operator pressing Go on an empty strip must not cost anything.
    surface.press(NOTE_ON, 31);
    surface.press(NOTE_ON, F1);
    run_until(&mut daemon, "the presses to be read", || {
        !desk.core().file.session.session().open_windows.is_empty()
    })
    .await;

    assert!(
        desk.core().file.show.executor(ExecutorId::new(7)).is_none(),
        "the strip that was pressed has no executor"
    );
    let output = daemon.desk().outputs()[0].status.clone();
    let frames = output.frames_sent();
    daemon
        .run(Some(Duration::from_millis(100)), std::future::pending())
        .await;
    assert!(output.frames_sent() > frames, "and the rig is still driven");

    daemon.shutdown().await;
}

/// How many reserved presses the attached surface has dropped.
fn daemon_reserved(daemon: &Daemon) -> u64 {
    daemon
        .surface()
        .expect("a surface is attached")
        .counters()
        .reserved
}

/// The picture reaches the desk: what the show holds is on the faders and the
/// scribble strips, in §5.2's order.
#[tokio::test]
async fn the_show_is_drawn_onto_the_surface_faders_first() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_console_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let (port, surface) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));

    // The scribble strips are third in §5.2's order, so waiting for one of them
    // is waiting for everything before it as well. A resync burst is 156
    // messages at a millisecond apiece (S21), so this is about a sixth of a
    // second rather than an unbounded wait.
    run_until(&mut daemon, "the whole surface to be drawn", || {
        surface
            .received()
            .iter()
            .any(|message| message.first() == Some(&0xF0))
    })
    .await;
    let sent = surface.received();

    // §5.2's order, end to end: the first messages out are the nine motor
    // faders, before a single LED. A message is classified by its status byte,
    // transcribed from §2.2 by hand — 0xE0-0xE8 is pitch bend, 0x90 a note.
    let faders = sent
        .iter()
        .take(9)
        .filter(|message| matches!(message.first(), Some(status) if (0xE0..=0xE8).contains(status)))
        .count();
    assert_eq!(faders, 9, "the nine faders come first: {sent:?}");

    // Executor 0's master is 65535 and executor 1's is 12345, so strip 0 and
    // strip 1 are parked in different places — a picture built from defaults
    // could not tell *drawn* from *not drawn*.
    let strip0 = sent
        .iter()
        .find(|message| message.first() == Some(&0xE0))
        .expect("strip 0's fader");
    let strip1 = sent
        .iter()
        .find(|message| message.first() == Some(&0xE1))
        .expect("strip 1's fader");
    assert_ne!(strip0.get(2), strip1.get(2), "{strip0:?} {strip1:?}");
    // 65535 of 65535 against a top of travel of 16380 is 16380, which is
    // 127 * 128 + 124 — worked out by hand, LSB first (§2.1).
    assert_eq!(strip0.as_slice(), &[0xE0, 124, 127]);

    // And the executor's name is on its scribble strip, folded to upper case
    // the way the hardware folds it. "Sequence 1" truncated to seven
    // characters is "SEQUENC".
    assert!(
        sent.iter().any(|message| {
            message.first() == Some(&0xF0) && message.windows(7).any(|window| window == b"SEQUENC")
        }),
        "the sequence name should be on the strip: {sent:?}"
    );

    daemon.shutdown().await;
}

/// A cable pulled out is reported to the operator and changes nothing else.
/// A cable pulled mid-show, and put back.
///
/// **S36 added the second half.** Until then a surface that went away stayed
/// away for the life of the daemon, because nothing looked again; now the port
/// is asked once per poll and a desk that comes back is redrawn whole. Neither
/// edge reaches the engine, which is the §5.3 rule and is asserted rather than
/// described: the tick count moves and no tick is missed across both of them.
#[tokio::test]
async fn a_surface_that_goes_away_is_reported_and_the_show_carries_on() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_console_show(&dir.path().join("aula.prism"));

    let mut options = options(dir.path());
    options.local = true;
    let mut daemon = Daemon::start(&options).await.unwrap();
    let (port, surface) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));

    let address = common::local_address(dir.path());
    let connecting = tokio::spawn(async move {
        let wire = prism_ipc::local::connect(&address).await.unwrap();
        Client::handshake(wire, Hello::new(ClientKind::Desktop))
            .await
            .unwrap()
    });
    let (mut client, _snapshot) = tokio::select! {
        result = connecting => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("the client never connected")
        }
    };

    run_until(&mut daemon, "the surface to be drawn", || {
        surface.received().len() > 20
    })
    .await;
    surface.unplug();

    // The operator is told, on the same channel every other fault uses.
    let listening = tokio::spawn(async move {
        loop {
            match client.next_event().await {
                Some(Ok(ClientEvent::Delta(prism_domain::Delta::Notice { message, .. }))) => {
                    return message;
                }
                Some(Ok(_)) => {}
                other => panic!("the connection ended: {other:?}"),
            }
        }
    });
    let message = tokio::select! {
        result = listening => result.unwrap(),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("the operator was never told the surface had gone")
        }
    };
    assert!(message.contains("no surface"), "{message}");

    // §5.3: the engine is unaffected, and nothing more is written to a port
    // that is not there.
    let output = daemon.desk().outputs()[0].status.clone();
    let frames = output.frames_sent();
    surface.clear_received();
    daemon
        .run(Some(Duration::from_millis(200)), std::future::pending())
        .await;
    assert!(surface.received().is_empty(), "nothing goes to a dead port");
    assert!(
        output.frames_sent() > frames,
        "and the rig is still being driven"
    );

    // **And it comes back** — S36. `docs/MCU_MAPPING.md` §5.3: a reconnect
    // invalidates the shadow model, so the whole picture is transmitted once as
    // the ordinary diff rather than as a special path. The engine hears about
    // neither edge, which is asserted the same way it was above: the tick count
    // moves and no tick is missed.
    let missed = daemon.desk().core().engine().health().missed();
    surface.replug();
    run_until(&mut daemon, "the desk to be redrawn", || {
        surface.received().len() > 100
    })
    .await;
    assert_eq!(
        daemon.desk().core().engine().health().missed(),
        missed,
        "the tick never noticed a cable"
    );
    assert!(
        output.frames_sent() > frames,
        "and the rig never stopped for one"
    );

    daemon.shutdown().await;
}
