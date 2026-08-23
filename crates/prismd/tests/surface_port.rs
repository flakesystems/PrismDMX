//! The control surface's MIDI port, against a running daemon — **S36**.
//!
//! `tests/surface_gate.rs` is about what a surface *does* once it is attached.
//! This target is about **which** surface, and it is the other half of S33: the
//! desk in the rack, like the cabling, belongs to the building and is
//! configured as data rather than as a command-line flag.
//!
//! # Nothing here opens a device
//!
//! `CLAUDE.md`'s rule, and this is the file where breaking it would be easiest:
//! a real X-Touch is attached to the machine this was written on, firmware
//! V1.25, serial `0156406` (S20). So every port named here is one **nothing can
//! be called** — a name with an emoji in it — and what is asserted is the
//! behaviour a configured port that is absent produces, which is the same on a
//! build server as it is here. That the daemon can drive a port that *is* there
//! is a 🔌 row in `ARCHITECTURE_SPEC.md` §14 and cannot be a test.

// Every test holds `common::one_daemon_at_a_time` across its awaits — a daemon
// owns a tick thread at real-time priority, and two of those in one process
// measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_domain::{Answer, Command, Query};
use prism_ipc::CommandOutcome;
use prismd::cli::Options;
use prismd::daemon::Daemon;

mod common;

/// A MIDI port name nothing can be called.
///
/// The emoji is deliberate rather than cute: every rule in `prism_midi::selects`
/// is a comparison over names, and a name no operating system produces cannot
/// match a real device by accident on any machine the suite ever runs on.
const NO_SUCH_PORT: &str = "no such desk \u{1F50C}";

/// A second one, for the case where the port changes while the show is running.
const NOR_THIS_ONE: &str = "nor this desk \u{1F50C}";

/// A daemon whose machine configuration is **its own to change**.
///
/// No output flag, which matters and is S33's rule rather than S36's: outputs
/// named on a command line are the whole rig for that run, the machine
/// configuration is then neither read nor written, and **every** machine
/// command is refused — the surface port among them, because the reason is that
/// nothing is written down at all. So the rig here is built with `AddOutput`
/// against `--mock-devices`, which is how `tests/outputs.rs` does it and how
/// S37's settings window will start its daemon.
fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: Some(2),
        mock_devices: true,
        local: Some(false),
        // **No listener of any kind** — S37. Since the WebSocket listener is a
        // *setting* and the setting is on, a test that said nothing would bind
        // 127.0.0.1:7373, and the several daemon targets `cargo test` runs at
        // once would each be asking for it. A suite must not open a socket it
        // does not use.
        websocket: prismd::cli::Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
        ..Options::default()
    }
}

/// One recording output on universe 1, added the way a settings window would.
fn add_an_output(daemon: &Daemon) {
    apply(
        daemon,
        Command::AddOutput {
            output: prism_domain::OutputInstance::new(
                prism_domain::OutputId::new(1),
                "Hall",
                prism_domain::OutputKind::Mock,
                [prism_domain::UniverseId::new(1)],
            ),
        },
    );
}

/// **The exit criterion.** A configured port that is not there is a warning and
/// a daemon that starts — never a daemon that will not.
///
/// It is the rule S33 put an unusable output row under, one device along, and it
/// matters for the same reason: a desk switched off half an hour before a show
/// must not be why the show cannot be run.
#[tokio::test]
async fn a_configured_port_that_is_not_there_is_a_warning_and_a_daemon_that_starts() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    add_an_output(&daemon);
    // No surface yet: nothing is configured and nothing was asked for.
    assert!(daemon.surface().is_none());
    apply(
        &daemon,
        Command::SetSurfacePort {
            port: Some(NO_SUCH_PORT.to_owned()),
        },
    );

    // The daemon takes it up within a poll, and it is attached and absent at
    // the same time — which is exactly the state a settings window has to draw.
    run_until(&mut daemon, "the surface to be taken up", |daemon| {
        daemon.surface().is_some()
    })
    .await;
    let surface = daemon.surface().expect("a surface is attached");
    assert_eq!(surface.health(), prism_surface::SurfaceHealth::Disconnected);
    assert_eq!(surface.open_name(), None);
    assert!(
        surface.describe().contains(NO_SUCH_PORT),
        "{}",
        surface.describe()
    );

    // And the rig is being driven throughout, because a surface has nothing to
    // do with light reaching a stage.
    let output = daemon.desk().outputs()[0].status.clone();
    let frames = output.frames_sent();
    daemon
        .run(Some(Duration::from_millis(200)), std::future::pending())
        .await;
    assert!(output.frames_sent() > frames, "the show carried on");
    daemon.shutdown().await;

    // The same daemon started **again** over the same data directory: the port
    // is in `machine.json`, so this time it is configured before anything runs,
    // and it still starts.
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    assert_eq!(
        daemon.desk().core().surface_port(),
        Some(NO_SUCH_PORT),
        "the port a settings window wrote survives a restart"
    );
    let surface = daemon
        .surface()
        .expect("a configured port is attached at start-up, present or not");
    assert_eq!(surface.health(), prism_surface::SurfaceHealth::Disconnected);
    daemon.shutdown().await;
}

/// The list a settings window offers, and the mark against the chosen one.
///
/// The other exit criterion: **enumeration answers on a machine with no MIDI
/// device, with an empty list rather than an error**. It cannot assert the list
/// is empty — this machine has an X-Touch attached and a build server has
/// nothing — so it asserts what is true either way, which is that it answers,
/// that every name is a name, and that nothing was opened.
#[tokio::test]
async fn the_ports_are_enumerated_and_the_configured_one_is_marked() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let Answer::MidiPorts {
        ports,
        configured,
        open,
        status: _,
    } = daemon.desk().query(&Query::MidiPorts)
    else {
        panic!("Query::MidiPorts has to be answered with MidiPorts");
    };
    assert_eq!(configured, None, "nothing is configured yet");
    assert_eq!(open, None, "and nothing is open");
    for port in &ports {
        assert!(!port.name.is_empty(), "{ports:?}");
        assert!(port.input || port.output, "a port goes one way at least");
    }

    // Choose one that is not there. The list is unchanged — enumerating is the
    // operating system's answer and not the daemon's — and the mark moves.
    apply(
        &daemon,
        Command::SetSurfacePort {
            port: Some(NO_SUCH_PORT.to_owned()),
        },
    );
    let Answer::MidiPorts {
        ports: again,
        configured,
        open,
        status: _,
    } = daemon.desk().query(&Query::MidiPorts)
    else {
        panic!("Query::MidiPorts has to be answered with MidiPorts");
    };
    assert_eq!(again, ports, "choosing a port does not change what exists");
    assert_eq!(configured, Some(NO_SUCH_PORT.to_owned()));
    assert_eq!(
        open, None,
        "a port that is named and absent is exactly the state a panel has to draw"
    );
    daemon.shutdown().await;
}

/// A `SetSurfacePort` answers every client, so two settings windows cannot
/// disagree about which desk this is.
#[tokio::test]
async fn choosing_a_port_is_broadcast_and_is_not_undoable() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let CommandOutcome::Applied { deltas } = daemon.desk().command(Command::SetSurfacePort {
        port: Some(NO_SUCH_PORT.to_owned()),
    }) else {
        panic!("a port that is not there is still a port that can be chosen");
    };
    assert!(
        deltas.contains(&prism_domain::Delta::SurfaceChanged {
            port: Some(NO_SUCH_PORT.to_owned())
        }),
        "{deltas:?}"
    );

    // Not undoable, and for the reason the four output commands are not: the
    // Oops journal is the **show's**, and this is not show content.
    assert!(
        !Command::SetSurfacePort {
            port: Some(NO_SUCH_PORT.to_owned())
        }
        .is_undoable()
    );
    let CommandOutcome::Refused { message } = daemon.desk().command(Command::Oops) else {
        panic!("there is nothing to undo, so an Oops has to be refused");
    };
    assert!(!message.is_empty());
    assert_eq!(
        daemon.desk().core().surface_port(),
        Some(NO_SUCH_PORT),
        "and the desk is still the desk"
    );
    daemon.shutdown().await;
}

/// S33's rule, one device along: **a daemon whose surface was named on its
/// command line refuses to change it.**
///
/// The alternative is a `SetSurfacePort` that appears to work and vanishes at
/// the next restart, because nothing writes it down.
#[tokio::test]
async fn a_surface_named_on_the_command_line_is_not_the_configurations_to_change() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));

    let mut options = options(dir.path());
    options.surface = Some(NO_SUCH_PORT.to_owned());
    let daemon = Daemon::start(&options).await.unwrap();
    assert!(
        daemon.surface().is_some(),
        "--surface attaches one, there or not"
    );

    let CommandOutcome::Refused { message } = daemon.desk().command(Command::SetSurfacePort {
        port: Some(NOR_THIS_ONE.to_owned()),
    }) else {
        panic!("a surface from the command line is not the configuration's to change");
    };
    assert!(message.contains("--surface"), "{message}");
    assert!(message.contains("command line"), "{message}");

    // And nothing was written: the flag is for the run, not for the building.
    assert_eq!(daemon.desk().core().surface_port(), None);
    let written = std::fs::read_to_string(prismd::paths::machine_config_path(dir.path())).unwrap();
    assert!(!written.contains(NOR_THIS_ONE), "{written}");
    assert!(!written.contains(NO_SUCH_PORT), "{written}");

    // **The rig is still the configuration's**, because the two are separate
    // facts and only one of them came off a command line. A daemon told which
    // desk to use has not been told anything about its cabling, and an operator
    // refused an `AddOutput` here would be refused for a reason that is not
    // true.
    add_an_output(&daemon);
    assert_eq!(daemon.desk().core().outputs().entries().len(), 1);
    daemon.shutdown().await;
}

/// The port name is written where the cabling is, and nowhere else — asserted
/// on the **bytes**, the way `desk_id_is_not_show_content` is.
#[tokio::test]
async fn the_port_is_in_the_machine_configuration_and_not_in_the_show() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let show = dir.path().join("aula.prism");
    common::write_show(&show);
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    apply(
        &daemon,
        Command::SetSurfacePort {
            port: Some(NO_SUCH_PORT.to_owned()),
        },
    );
    assert!(matches!(
        daemon.desk().command(Command::SaveShow),
        CommandOutcome::Applied { .. }
    ));
    daemon.shutdown().await;

    let machine = std::fs::read_to_string(prismd::paths::machine_config_path(dir.path())).unwrap();
    assert!(machine.contains(NO_SUCH_PORT), "{machine}");
    let saved = std::fs::read(&show).unwrap();
    assert!(
        !contains(&saved, NO_SUCH_PORT.as_bytes()),
        "a show carried to another hall must not name this hall's desk"
    );
    assert!(
        !contains(&saved, b"surfacePort"),
        "and it must not have a place to put one either"
    );
}

/// Changing the port mid-show costs the rig nothing.
///
/// The same claim S33 makes about reconfiguring an output, for the other device
/// this machine owns: the engine never hears about a surface, so putting one
/// down and taking another up must not cost a frame or a tick.
#[tokio::test]
async fn changing_the_port_while_the_show_runs_costs_the_rig_nothing() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    common::write_show(&dir.path().join("aula.prism"));
    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    add_an_output(&daemon);

    apply(
        &daemon,
        Command::SetSurfacePort {
            port: Some(NO_SUCH_PORT.to_owned()),
        },
    );
    run_until(&mut daemon, "the first port to be taken up", |daemon| {
        daemon.surface().is_some()
    })
    .await;

    let output = daemon.desk().outputs()[0].status.clone();
    let frames = output.frames_sent();
    let missed = daemon.desk().core().engine().health().missed();

    apply(
        &daemon,
        Command::SetSurfacePort {
            port: Some(NOR_THIS_ONE.to_owned()),
        },
    );
    run_until(&mut daemon, "the second port to be taken up", |daemon| {
        daemon
            .surface()
            .is_some_and(|surface| surface.describe().contains(NOR_THIS_ONE))
    })
    .await;
    daemon
        .run(Some(Duration::from_millis(200)), std::future::pending())
        .await;
    assert!(output.frames_sent() > frames, "the rig never stopped");
    assert_eq!(
        daemon.desk().core().engine().health().missed(),
        missed,
        "and the tick never noticed"
    );

    // Taking the surface away is a change like any other, and leaves the same
    // daemon running without one.
    apply(&daemon, Command::SetSurfacePort { port: None });
    run_until(&mut daemon, "the surface to be let go", |daemon| {
        daemon.surface().is_none()
    })
    .await;
    assert_eq!(daemon.desk().core().surface_port(), None);
    daemon.shutdown().await;
}

/// Applies a command that must be applied, and says which one if it is not.
fn apply(daemon: &Daemon, command: Command) {
    match daemon.desk().command(command.clone()) {
        CommandOutcome::Applied { .. } => {}
        CommandOutcome::Refused { message } => panic!("{command:?} was refused: {message}"),
    }
}

/// Runs the daemon in slices until `condition` holds, or fails by name.
///
/// `surface_gate.rs`'s helper, and here for the same reason: the surface is
/// taken up and put down **inside** `Daemon::run`, which is the arrangement
/// under test, so a condition can only be looked at between slices. Every wait
/// has a deadline — a named failure in seconds is worth more than a job that
/// stops.
async fn run_until(daemon: &mut Daemon, what: &str, mut condition: impl FnMut(&Daemon) -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        if condition(daemon) {
            return;
        }
        daemon
            .run(Some(Duration::from_millis(5)), std::future::pending())
            .await;
    }
    panic!("timed out waiting for {what}");
}

/// Whether `haystack` holds `needle`, for an assertion about a binary file.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
