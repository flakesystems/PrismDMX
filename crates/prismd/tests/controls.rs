//! **S38's exit criteria**: the binding table of `docs/MCU_MAPPING.md` §4 read,
//! written and learned over the protocol, against a running daemon.
//!
//! # Nothing here touches a device
//!
//! `CLAUDE.md`'s rule, and this is the file where it would be tempting to break
//! it. The surface is a [`MockSurfacePort`] and every note number pressed is
//! written out **by hand from §2.1** — F1 is 54, Play is 94, Solo on strip 0 is
//! note 8, the jog wheel is CC 60. Asking the profile which note to send would
//! be asking the code under test what to press, which is S19's finding and S20's
//! method rule in one; it matters twice as much here, because the thing under
//! test *is* the table a profile would have been asked.
//!
//! # The daemon is the one a venue runs
//!
//! No output flags and `--mock-devices`, which is S36's finding and S37's rule:
//! a daemon whose outputs came off its command line refuses **every** machine
//! command, so a test about a machine command must not use `--mock-output`.
//! Every driver is still a double, so nothing is opened and no datagram leaves
//! this machine.

// Every test holds `common::one_daemon_at_a_time` across its awaits, for
// `wiring.rs`'s reason: a daemon owns a real-time tick thread, and several in
// one process measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::{SessionState, ShowFile, ShowStore};
use prism_domain::{
    Answer, BoundControl, Command, Delta, ExecutorId, GlobalButton, MachineChange, Query,
    StripButton, SurfaceAction, ViewId, WindowType,
};
use prism_ipc::{Client, ClientEvent, ClientKind, Hello};
use prism_surface::Bindings;
use prismd::cli::{Listen, Options};
use prismd::daemon::Daemon;
use prismd::surface::{MockSurfaceHandle, MockSurfacePort};

mod common;

/// Note On, MIDI channel 1 — §2.1's "all note and CC numbers are on MIDI
/// channel 1", which is status `0x90`.
const NOTE_ON: u8 = 0x90;

/// Control Change, MIDI channel 1.
const CONTROL_CHANGE: u8 = 0xB0;

/// F1, note 54 — §2.1's "Function" row: F1–F8 = 54–61.
const F1: u8 = 54;

/// Play, note 94 — §2.1's "Transport" row, and one of the five that reach
/// PrismDMX permanently in the shared mode (§4.3).
const PLAY: u8 = 94;

/// Solo on the leftmost strip, note 8 — §2.1's "Channel strips" table.
const SOLO_0: u8 = 8;

/// The V-Pot of the leftmost strip, CC 16 — §2.1's "V-Pot rotation" row.
const VPOT_0: u8 = 16;

/// The jog wheel, CC 60 — §2.1's last row.
const JOG: u8 = 60;

/// SMPTE/Beats, note 53 — the button that must never be bound (§4.3).
const SMPTE_BEATS: u8 = 53;

/// The daemon a venue runs: its rig and its surface are this machine's, and
/// nothing about the machine is held by a flag.
fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: None,
        outputs: Vec::new(),
        mock_devices: true,
        local: Some(false),
        // S37: the listener is a setting and the setting is on, so a target that
        // said nothing would bind 127.0.0.1:7373 — and several run at once.
        websocket: Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
        ..Options::default()
    }
}

/// A show with a session a console can be seen operating: two views, so
/// `Channel ▶` has somewhere to go, and executor 0 selected.
fn write_show(path: &Path) {
    let mut file = ShowFile::new();
    let mut session = SessionState::new();
    session.store_view(ViewId::new(2), "Programming").unwrap();
    session.select_executor(Some(ExecutorId::new(0))).unwrap();
    file.session = session;
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

/// A started daemon with a mock surface attached.
async fn desk(dir: &Path) -> (Daemon, MockSurfaceHandle) {
    write_show(&dir.join("aula.prism"));
    let mut daemon = Daemon::start(&options(dir)).await.unwrap();
    let (port, handle) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    (daemon, handle)
}

/// Runs the daemon in slices until `condition` holds, or fails by name.
///
/// `surface_gate.rs`'s helper and its reason: the surface is polled **inside**
/// `Daemon::run`, which is the arrangement under test, so a condition can only
/// be looked at between slices. Every wait has a deadline — a named failure in
/// seconds is worth more than a job that stops.
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

/// The table the daemon says is in force, over the protocol.
fn ask_table(daemon: &Daemon) -> (Vec<prism_domain::SurfaceControl>, u32, bool) {
    match daemon.desk().query(&Query::SurfaceBindings) {
        Answer::SurfaceBindings {
            controls,
            revision,
            learning,
            ..
        } => (controls, revision, learning),
        other => panic!("asked for the table and got {other:?}"),
    }
}

/// Says what one control does, over the protocol.
fn bind(daemon: &Daemon, control: BoundControl, action: Option<SurfaceAction>) -> Vec<Delta> {
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceBinding { control, action },
        })
        .expect("the binding was refused")
}

// ---------------------------------------------------------------- exit 1 & 2

/// **A binding changed in the interface takes effect without restarting the
/// daemon** — S38's first exit criterion, observed through a mock surface.
///
/// The claim is not *the table changed*, which a getter would satisfy for a
/// daemon that never told the desk. It is *the key does something else now*, so
/// it is asserted at both ends: F1 opens a Fixture Sheet before and a Patch
/// window after, and the same three bytes are what cause both.
#[tokio::test]
async fn a_binding_changed_over_the_protocol_takes_effect_without_a_restart() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, surface) = desk(dir.path()).await;
    let desk_handle = daemon.desk().clone();

    // What F1 does out of the box — §4.1, and asserted here rather than assumed
    // so that a change of defaults cannot make this test pass by accident.
    surface.press(NOTE_ON, F1);
    run_until(&mut daemon, "F1 to open its window", || {
        !desk_handle
            .core()
            .file
            .session
            .session()
            .open_windows
            .is_empty()
    })
    .await;
    assert_eq!(
        desk_handle.core().file.session.session().open_windows[0].window_type,
        WindowType::FixtureSheet,
        "F1 opens a Fixture Sheet out of the box"
    );

    // One control, one command. Nothing is restarted and nothing is reopened.
    let f1 = BoundControl::Global {
        button: GlobalButton::F1,
    };
    let deltas = bind(
        &daemon,
        f1,
        Some(SurfaceAction::OpenWindow {
            window: WindowType::Patch,
        }),
    );
    assert!(
        deltas
            .iter()
            .any(|delta| matches!(delta, Delta::SurfaceBindingsChanged { .. })),
        "the table moving is broadcast: {deltas:?}"
    );

    // The run loop is what hands the new table to the surface. Read off the
    // `Core`, because that is where the table lives since S38 and the daemon is
    // borrowed for the driving.
    run_until(&mut daemon, "the desk to draw with the new table", || {
        desk_handle.core().bindings().action(f1)
            == Some(SurfaceAction::OpenWindow {
                window: WindowType::Patch,
            })
    })
    .await;

    // And the same three bytes now do the other thing.
    surface.press(NOTE_ON, F1);
    run_until(&mut daemon, "F1 to open the window it now names", || {
        desk_handle
            .core()
            .file
            .session
            .session()
            .open_windows
            .iter()
            .any(|window| window.window_type == WindowType::Patch)
    })
    .await;
}

/// **The shipped profile still *is* the built-in defaults after a round trip
/// through the editor** — S38's second exit criterion.
///
/// S22 asserted that `profiles/surface/xtouch.json` reproduces
/// `Bindings::defaults()`, and that assertion is untouched in
/// `prism-surface/tests/bindings.rs`. What this adds is the round trip S38
/// introduced: every row read out of the daemon, written back through the
/// command one at a time, and the table compared with the one it started as.
///
/// It is worth having as well as the crate-level property because it goes
/// through the **whole** path — the query builds the rows, the command
/// validates each one, `MachineConfig` stores them, and the daemon rebuilds a
/// table from what it stored — and any of those four could lose a row on its
/// own.
#[tokio::test]
async fn the_shipped_defaults_survive_a_round_trip_through_the_editor() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (daemon, _surface) = desk(dir.path()).await;

    let before = *daemon.bindings();
    assert_eq!(before, Bindings::defaults(), "a fresh desk starts on §4.1");

    let (controls, _, _) = ask_table(&daemon);
    assert_eq!(controls.len(), BoundControl::all().len());
    for row in &controls {
        // Every row, including the ones that are deliberately nothing: the
        // strip encoder and F5–F8 are `None` in §4.1, and a round trip that
        // skipped them would not be one.
        bind(&daemon, row.control, row.action);
    }

    assert_eq!(
        *daemon.desk().core().bindings(),
        Bindings::defaults(),
        "every row written back is the table it came from"
    );
    // And the stored form agrees, because that is what the next start reads.
    let stored = daemon
        .desk()
        .core()
        .machine()
        .surface_bindings()
        .expect("the desk wrote its table down")
        .to_vec();
    let (rebuilt, problem) = Bindings::from_rows(&stored, &prism_surface::X_TOUCH);
    assert_eq!(problem, None);
    assert_eq!(rebuilt, Bindings::defaults());
}

// ------------------------------------------------------------------- exit 3

/// **A table binding the reserved control cannot be saved, and the message
/// names the button and says why** — S38's third exit criterion.
///
/// Asserted on the **wording** rather than on the variant, as S22 did for a
/// profile file: the refusal exists for the person who believes they have bound
/// it, so what it says is the feature.
#[tokio::test]
async fn the_reserved_control_is_refused_by_name_before_anything_is_written() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (daemon, surface) = desk(dir.path()).await;

    let smpte = BoundControl::Global {
        button: GlobalButton::SmpteBeats,
    };
    let before = *daemon.bindings();
    let refusal = daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceBinding {
                control: smpte,
                action: Some(SurfaceAction::Oops),
            },
        })
        .expect_err("SMPTE/Beats must never be bindable")
        .to_string();

    assert!(refusal.contains("SmpteBeats"), "{refusal}");
    assert!(refusal.contains("Xctl+MC"), "{refusal}");
    assert!(
        refusal.contains("switches the desk between the two hosts"),
        "{refusal}"
    );
    assert!(refusal.contains("Leave it unbound"), "{refusal}");
    assert_eq!(*daemon.bindings(), before, "and nothing was written");

    // Belt and braces, exactly as S22 left it: layer 2 drops the press and
    // counts it, so even a table that somehow held it could not fire.
    surface.press(NOTE_ON, SMPTE_BEATS);

    // Clearing it is **not** refused — unbound is the state §4.3 wants it in,
    // and a rule that refused that would make the one safe state unreachable.
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceBinding {
                control: smpte,
                action: None,
            },
        })
        .expect("clearing the reserved control is allowed");
}

// ------------------------------------------------------------------- exit 4

/// **Learn names the right control for every group of §2.1** — S38's fourth
/// exit criterion, driven from a mock surface.
///
/// One control from each shape the surface has: a panel button, a strip button,
/// a strip encoder and the jog wheel. Every note and CC is written out by hand
/// from §2.1, which is the point — a test that asked the profile which note a
/// control sends would be asking the code under test what to press.
///
/// The name is read off the **delta**, through a connected client, because that
/// is where an editor reads it: learn is broadcast rather than answered to the
/// one client that asked, since there is one desk and two editors must not both
/// believe they have armed it.
#[tokio::test]
async fn learn_names_the_control_that_was_pressed_for_every_group_of_section_2_1() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let mut options = options(dir.path());
    options.local = Some(true);
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

    for (what, bytes, expected) in [
        (
            "a panel button",
            vec![NOTE_ON, PLAY, 127],
            BoundControl::Global {
                button: GlobalButton::Play,
            },
        ),
        (
            "a strip button",
            vec![NOTE_ON, SOLO_0, 127],
            BoundControl::StripButton {
                button: StripButton::Solo,
            },
        ),
        (
            "a strip encoder",
            vec![CONTROL_CHANGE, VPOT_0, 0x01],
            BoundControl::StripEncoder,
        ),
        (
            "the jog wheel",
            vec![CONTROL_CHANGE, JOG, 0x01],
            BoundControl::Jog,
        ),
    ] {
        arm(&daemon);
        surface.send(&bytes);
        // The release goes in with the press, for the buttons: learn is one shot,
        // so the release arrives with learn already off and has to be swallowed
        // rather than fired — `SurfaceAction::is_momentary` forwards both edges
        // of an `ExecutorButton`, and Play is bound to one.
        if bytes[0] == NOTE_ON {
            surface.send(&[NOTE_ON, bytes[1], 0]);
        }

        let named = next_learned(&mut client, &mut daemon, what).await;
        assert_eq!(named, Some(expected), "learn named {what}");
        assert!(
            !daemon.desk().core().is_learning(),
            "learn disarms itself after naming {what}"
        );
        let (_, _, learning) = ask_table(&daemon);
        assert!(!learning, "and says so when it is asked");
    }

    // **And the presses did nothing.** Play is `On` on the selected executor and
    // Solo is that strip executor's second key; a desk that fired them while
    // learning would have started a sequence to find out what a key is called.
    assert!(
        daemon
            .desk()
            .core()
            .file
            .session
            .session()
            .open_windows
            .is_empty(),
        "learning a control must not fire it"
    );

    daemon.shutdown().await;
}

/// The control the next `Delta::SurfaceLearnChanged` names, driving the daemon
/// meanwhile.
async fn next_learned(
    client: &mut Client,
    daemon: &mut Daemon,
    what: &str,
) -> Option<BoundControl> {
    let listening = async {
        loop {
            match client.next_event().await {
                Some(Ok(ClientEvent::Delta(Delta::SurfaceLearnChanged {
                    control: Some(control),
                    ..
                }))) => return control,
                Some(Ok(_)) => {}
                other => panic!("the connection ended: {other:?}"),
            }
        }
    };
    tokio::select! {
        control = listening => Some(control),
        () = daemon.run(Some(Duration::from_secs(10)), std::future::pending()) => {
            panic!("learn never named {what}")
        }
    }
}

/// Arms learn over the protocol.
fn arm(daemon: &Daemon) {
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceLearn { learning: true },
        })
        .expect("arming learn");
}

// ------------------------------------------------------------------- exit 5

/// **Two clients with the editor open do not produce two tables** — S38's fifth
/// exit criterion.
///
/// Structural rather than raced: every edit is **one control**, so two operators
/// changing two different keys cannot undo each other, and both are told the
/// same revision. The alternative shape — a command carrying the whole table —
/// is what would have made this a race, and `MachineChange`'s documentation says
/// so.
#[tokio::test]
async fn two_editors_change_two_controls_and_there_is_one_table() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (daemon, _surface) = desk(dir.path()).await;

    let (_, first, _) = ask_table(&daemon);

    // Two clients, two different controls, in the order two people would.
    let f5 = BoundControl::Global {
        button: GlobalButton::F5,
    };
    let f6 = BoundControl::Global {
        button: GlobalButton::F6,
    };
    bind(&daemon, f5, Some(SurfaceAction::SaveShow));
    bind(&daemon, f6, Some(SurfaceAction::Oops));

    let (controls, second, _) = ask_table(&daemon);
    assert_eq!(second, first + 2, "each edit moves the revision once");

    let action_of = |wanted: BoundControl| {
        controls
            .iter()
            .find(|row| row.control == wanted)
            .and_then(|row| row.action)
    };
    // **Neither edit undid the other**, which is the claim.
    assert_eq!(action_of(f5), Some(SurfaceAction::SaveShow));
    assert_eq!(action_of(f6), Some(SurfaceAction::Oops));
    // And F1 is still what §4.1 says, so nothing else moved either.
    assert_eq!(
        action_of(BoundControl::Global {
            button: GlobalButton::F1
        }),
        Some(SurfaceAction::OpenWindow {
            window: WindowType::FixtureSheet
        })
    );

    // One table, and the stored one is it: a second daemon over this data
    // directory would read exactly these rows.
    let stored = daemon
        .desk()
        .core()
        .machine()
        .surface_bindings()
        .expect("the table is written down")
        .to_vec();
    let (rebuilt, problem) = Bindings::from_rows(&stored, &prism_surface::X_TOUCH);
    assert_eq!(problem, None);
    assert_eq!(&rebuilt, daemon.desk().core().bindings());
}

// ------------------------------------------------- ownership, and the profile

/// **Ownership is shown, and it is the device profile's answer** — S38's fifth
/// deliverable.
///
/// §4.3's permanently-ours set is the transport section and the jog wheel, and
/// it is transcribed **by hand** here from that section rather than read out of
/// `McuProfile::permanent`: a test that asked the constant would pass for any
/// constant at all, including one that claimed the whole panel is always in
/// reach. That is S22's rule for the §4.1 transcription, applied to §4.3.
#[tokio::test]
async fn every_control_says_whether_it_is_ours_in_the_shared_mode() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (daemon, _surface) = desk(dir.path()).await;

    let (controls, _, _) = ask_table(&daemon);
    // §4.3's table, written out: "The whole transport section — Rewind,
    // Forward, Stop, Play, Record" and "The jog wheel".
    let always = [
        BoundControl::Global {
            button: GlobalButton::Rewind,
        },
        BoundControl::Global {
            button: GlobalButton::FastForward,
        },
        BoundControl::Global {
            button: GlobalButton::Stop,
        },
        BoundControl::Global {
            button: GlobalButton::Play,
        },
        BoundControl::Global {
            button: GlobalButton::Record,
        },
        BoundControl::Jog,
    ];
    for row in &controls {
        let expected = always.contains(&row.control);
        assert_eq!(
            row.permanent,
            expected,
            "{} is {} in the shared mode",
            row.name,
            if expected { "ours" } else { "not ours" }
        );
    }

    // And the reserved control is marked, so an editor can grey the row rather
    // than let an operator find out by being told no.
    let reserved: Vec<&str> = controls
        .iter()
        .filter(|row| row.reserved)
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(reserved, ["Global.SmpteBeats"]);
}

/// **A profile file is an import, and this desk's own table outlives it** —
/// S38's storage decision, asserted end to end.
///
/// A desk that has been edited and then restarted starts with the keys it was
/// left with. The alternative — the profile file winning at every start — would
/// mean an operator who rebound a key found it back the way it was the next
/// morning, and it is the reason the table is `MachineConfig`'s rather than a
/// file's.
#[tokio::test]
async fn a_desk_starts_with_the_keys_it_was_left_with() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let f5 = BoundControl::Global {
        button: GlobalButton::F5,
    };
    {
        let (daemon, _surface) = desk(dir.path()).await;
        assert_eq!(daemon.bindings().action(f5), None, "F5 is free in §4.1");
        bind(&daemon, f5, Some(SurfaceAction::SaveShow));
    }
    // A second daemon over the same data directory — which is how a desk comes
    // back in the morning.
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    assert_eq!(
        daemon.bindings().action(f5),
        Some(SurfaceAction::SaveShow),
        "the desk starts with the keys it was left with"
    );
}
