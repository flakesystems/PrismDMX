//! The settings and the show files, against a running daemon — S37.
//!
//! # The daemon these tests start is the one a venue runs
//!
//! No output flags at all, `--mock-devices`, and the rig built with `AddOutput`
//! — which is what S36 carried out of itself in as many words, because a daemon
//! whose outputs came off its command line refuses *every* machine command and
//! not only the four about the rig. So a target that wants to configure the
//! machine must not use `--mock-output`.
//!
//! **No device is touched and no datagram leaves this process.** This machine
//! has a real SH-RS09B and a real X-Touch attached, and the suite opens neither:
//! every driver is a recording double and no test names a MIDI port.
//!
//! # What is asserted on the file rather than on the claim
//!
//! Every setting is read back out of `machine.json` and out of a **second**
//! daemon started over the same data directory, because the thing S37 is really
//! about is that a setting survives the run that made it. A test that only asked
//! the daemon what it had been told would pass for an implementation that never
//! wrote anything down.

// Every test holds `common::one_daemon_at_a_time` across its awaits, for
// `wiring.rs`'s reason: a daemon owns a real-time tick thread, and several in
// one process measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::{MachineConfig, ShowFile, ShowStore};
use prism_domain::{
    Answer, Command, ExitAction, LogLevel, MachineChange, MachineOverride, OutputId,
    OutputInstance, OutputKind, Query, UniverseId,
};
use prismd::cli::{Listen, Options};
use prismd::daemon::Daemon;

mod common;

/// A show with three dimmers in universe 1, so an open and a save have
/// something to carry.
fn write_show(path: &Path) {
    let mut file = ShowFile::new();
    file.show
        .embed_fixture_type(common::dimmer_type("generic.dimmer", 65_535))
        .unwrap();
    for id in 1..=3u32 {
        file.show
            .patch_fixture(common::fixture(
                id,
                "generic.dimmer",
                1,
                u16::try_from(id).unwrap(),
            ))
            .unwrap();
    }
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

/// The daemon a venue runs: its rig is the configuration's, its surface is the
/// configuration's, and nothing about the machine is held by a flag.
fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        // **Nothing about the machine is held by a flag**, the universe count
        // included: a target that named one would find every settings row it
        // touched greyed out, which is the state S33 and S36 established for a
        // rig and a surface and S37 generalises.
        universes: None,
        // No `--mock-output`: the rig is this machine's, which is what makes
        // every machine command reachable at all.
        outputs: Vec::new(),
        mock_devices: true,
        local: Some(false),
        websocket: Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
        ..Options::default()
    }
}

/// What is on disk, read back as a technician would read it.
fn stored(dir: &Path) -> MachineConfig {
    let text = std::fs::read_to_string(dir.join("machine.json")).expect("machine.json");
    serde_json::from_str(&text).expect("machine.json parses")
}

/// One mock output, so the daemon has a rig to be reconfigured around.
fn add_output(daemon: &Daemon, id: u32) {
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            output: OutputInstance::new(
                OutputId::new(id),
                format!("Output {id}"),
                OutputKind::Mock,
                [UniverseId::new(1)],
            ),
        })
        .unwrap_or_else(|error| panic!("the rig was refused: {error}"));
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

/// **Every setting, written from the interface and read back off the disk** —
/// and then read back again by a daemon that has never heard of this one.
///
/// This is S37's *This machine* panel as a claim rather than as a screen: the
/// command line said nothing about any of it, so every row is the
/// configuration's, and a second start finds all of them where they were left.
#[tokio::test]
async fn every_setting_is_written_from_a_command_and_survives_a_restart() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));

    {
        let daemon = Daemon::start(&options(dir.path())).await.unwrap();
        add_output(&daemon, 1);

        for change in [
            MachineChange::Local { local: false },
            MachineChange::Websocket {
                address: Some("127.0.0.1:7599".parse().unwrap()),
            },
            MachineChange::LogLevel {
                level: LogLevel::Warn,
            },
            MachineChange::Universes { universes: 8 },
            MachineChange::ExitAction {
                action: ExitAction::Blackout,
            },
            MachineChange::Autostart { autostart: true },
            MachineChange::FixtureLibrary {
                path: Some("D:/fixtures".to_owned()),
            },
        ] {
            daemon
                .desk()
                .core()
                .apply(&Command::ConfigureMachine {
                    change: change.clone(),
                })
                .unwrap_or_else(|error| panic!("{change:?} was refused: {error}"));
        }

        // What the settings window is looking at, straight away.
        let snapshot = daemon.desk().snapshot();
        assert!(!snapshot.machine.local);
        assert_eq!(
            snapshot.machine.websocket.map(|a| a.to_string()),
            Some("127.0.0.1:7599".to_owned())
        );
        assert_eq!(snapshot.machine.universes, 8);
        assert!(snapshot.machine.autostart);
        assert_eq!(snapshot.machine.exit_action, ExitAction::Blackout);
        assert_eq!(
            snapshot.machine.fixture_library.as_deref(),
            Some("D:/fixtures")
        );
        // What the command line *did* say is greyed out, and nothing else is:
        // this target turns the listener off and closes the local transport, so
        // those two rows are held and the other eight are the operator's.
        assert!(snapshot.machine.is_overridden(MachineOverride::Websocket));
        assert!(snapshot.machine.is_overridden(MachineOverride::Local));
        assert!(snapshot.machine.is_overridden(MachineOverride::LogLevel));
        assert!(!snapshot.machine.is_overridden(MachineOverride::Universes));
        assert!(!snapshot.machine.is_overridden(MachineOverride::Outputs));
        assert!(!snapshot.machine.is_overridden(MachineOverride::ExitAction));
        // …and a held row is still **written**: the setting is the operator's,
        // the flag only decides what this run does with it. The restart below
        // is what reads it back.
        assert_eq!(stored(dir.path()).settings().log_level, LogLevel::Warn);
        assert_eq!(snapshot.machine.data_dir, dir.path().display().to_string());

        // And on the platter, before the daemon has stopped.
        let config = stored(dir.path());
        assert!(!config.settings().local);
        assert_eq!(config.settings().universes, 8);
        assert!(config.settings().autostart);

        daemon.shutdown().await;
    }

    // A second daemon over the same directory, which has never been told any of
    // this: it finds all of it, and it is running on eight universes.
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let snapshot = daemon.desk().snapshot();
    assert_eq!(snapshot.machine.universes, 8);
    assert!(snapshot.machine.autostart);
    assert_eq!(snapshot.machine.log_level, LogLevel::Warn);
    assert_eq!(snapshot.machine.exit_action, ExitAction::Blackout);
    assert_eq!(
        snapshot.machine.fixture_library.as_deref(),
        Some("D:/fixtures")
    );
    daemon.shutdown().await;
}

/// **A flag is the value for that run, and the interface is told which flag.**
///
/// S33's rule for `--mock-output` and S36's for `--surface`, generalised over
/// every setting: a settings window greys the row out and names the flag, rather
/// than offering a box that would appear to work and vanish at the next restart.
#[tokio::test]
async fn a_command_line_holds_a_setting_and_the_panel_is_told_which_flag() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));

    let mut options = options(dir.path());
    options.universes = Some(4);
    options.log_level = Some(prismd::log::Level::Error);
    let daemon = Daemon::start(&options).await.unwrap();

    let machine = daemon.desk().snapshot().machine;
    assert!(machine.is_overridden(MachineOverride::Universes));
    assert!(machine.is_overridden(MachineOverride::LogLevel));
    assert!(machine.is_overridden(MachineOverride::Local));
    assert!(!machine.is_overridden(MachineOverride::FixtureLibrary));
    // The settings the flags said nothing about are still the operator's.
    assert!(!machine.is_overridden(MachineOverride::ExitAction));
    assert!(!machine.is_overridden(MachineOverride::Token));
    daemon.shutdown().await;
}

/// A listener that cannot bind is a **warning and a daemon that starts** — S37.
///
/// The rule S36 wrote for a MIDI port that is not there, one device along, and
/// it matters more here because the listener is on by default: two daemons on
/// one machine both ask for 7373, and refusing to start over it would mean one
/// stray process makes a desk unstartable half an hour before a show.
///
/// What an operator gets instead is the state a panel can draw: **configured
/// here, listening nowhere.**
#[tokio::test]
async fn a_listener_that_cannot_bind_is_a_warning_and_a_daemon_that_starts() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));

    // Something else is already on the address. A real socket rather than a
    // contrived failure, because *the address is taken* is the case this rule
    // exists for.
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = taken.local_addr().unwrap();

    let mut options = options(dir.path());
    options.websocket = Listen::At(address);
    let daemon = Daemon::start(&options)
        .await
        .expect("a listener that cannot bind must not stop a desk");

    let machine = daemon.desk().snapshot().machine;
    assert_eq!(
        machine.websocket_open, None,
        "it is listening nowhere, which is the state the panel draws"
    );
    assert!(
        machine.is_overridden(MachineOverride::Websocket),
        "and the address it tried is the flag's, which the panel says"
    );
    // The desk is otherwise a desk: the rig runs.
    add_output(&daemon, 1);
    let handle = daemon.recorded_output(OutputId::new(1)).unwrap();
    until("the rig to be driven anyway", || handle.frame_count() > 3).await;

    daemon.shutdown().await;
    drop(taken);
}

/// **A show is saved, saved under a new name and reopened, without a command
/// line** — S37's exit criterion, in Rust.
///
/// `ui/e2e/settings.spec.ts` is the same claim driven from a browser; this is
/// the one that can look at the bytes.
#[tokio::test]
async fn a_show_is_saved_under_a_new_name_and_opened_again() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let panto = dir.path().join("panto.prism");
    daemon
        .desk()
        .core()
        .apply(&Command::SaveShowAs {
            path: panto.display().to_string(),
        })
        .unwrap_or_else(|error| panic!("a save-as was refused: {error}"));
    assert!(panto.is_file(), "the file was not written");
    assert_eq!(
        daemon.desk().snapshot().show_file.path,
        panto.display().to_string(),
        "and it is the show that is open from now on"
    );

    // An edit that only the new file has.
    daemon
        .desk()
        .core()
        .apply(&Command::UnpatchFixture {
            id: prism_domain::FixtureId::new(3),
        })
        .unwrap();
    daemon.desk().core().apply(&Command::SaveShow).unwrap();
    assert_eq!(daemon.desk().core().file.show.fixtures().count(), 2);

    // Back to the first show, which still has three.
    daemon
        .desk()
        .core()
        .apply(&Command::OpenShow {
            path: dir.path().join("aula.prism").display().to_string(),
        })
        .unwrap_or_else(|error| panic!("an open was refused: {error}"));
    assert_eq!(daemon.desk().core().file.show.fixtures().count(), 3);

    let show_file = daemon.desk().snapshot().show_file;
    assert_eq!(
        show_file.path,
        dir.path().join("aula.prism").display().to_string()
    );
    assert!(
        show_file.recent.contains(&panto.display().to_string()),
        "the show that was open is the one that is now recent: {:?}",
        show_file.recent
    );
    assert!(!show_file.unsaved_changes, "a freshly loaded show is clean");

    daemon.shutdown().await;
}

/// A **new** show is made, and one that would replace an existing file is
/// refused.
///
/// The refusal is the point: a *new* show that overwrote an existing one would
/// be the most destructive command in `docs/IPC_PROTOCOL.md` §5 and would look
/// like the least.
#[tokio::test]
async fn a_new_show_is_empty_and_never_replaces_a_file_that_is_there() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    // A relative path is resolved against the data directory, because a client
    // and a daemon do not share a working directory.
    daemon
        .desk()
        .core()
        .apply(&Command::NewShow {
            path: "next-term.prism".to_owned(),
        })
        .unwrap_or_else(|error| panic!("a new show was refused: {error}"));
    assert!(dir.path().join("next-term.prism").is_file());
    assert_eq!(
        daemon.desk().core().file.show.fixtures().count(),
        0,
        "a new show is empty"
    );

    let refusal = daemon
        .desk()
        .core()
        .apply(&Command::NewShow {
            path: "aula.prism".to_owned(),
        })
        .expect_err("a new show must not replace one that is there");
    assert!(
        refusal.to_string().contains("already there"),
        "{refusal} does not say why"
    );
    // And the show that was open is untouched by the refusal.
    assert_eq!(
        daemon.desk().snapshot().show_file.path,
        dir.path().join("next-term.prism").display().to_string()
    );

    // The first show still has its three fixtures, which is what *untouched*
    // means about a file rather than about a claim.
    let store = ShowStore::open(dir.path().join("aula.prism")).unwrap();
    assert_eq!(store.read().unwrap().show.fixtures().count(), 3);

    daemon.shutdown().await;
}

/// An **open** that names nothing is refused rather than quietly making an empty
/// show.
///
/// `ShowStore::open` creates a file that is not there, which is right for a
/// daemon starting up and wrong for an operator who mistyped a name: the show
/// they meant would still be on the disk beside the empty one they got.
#[tokio::test]
async fn opening_a_show_that_is_not_there_is_refused_rather_than_created() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let refusal = daemon
        .desk()
        .core()
        .apply(&Command::OpenShow {
            path: "aulaa.prism".to_owned(),
        })
        .expect_err("an open must not create");
    assert!(refusal.to_string().contains("no show at"), "{refusal}");
    assert!(!dir.path().join("aulaa.prism").exists());
    assert_eq!(daemon.desk().core().file.show.fixtures().count(), 3);

    // And a path that is not a `.prism` at all is refused one layer earlier, by
    // the show model, which is where the string can be judged.
    let refusal = daemon
        .desk()
        .core()
        .apply(&Command::OpenShow {
            path: "aula.txt".to_owned(),
        })
        .expect_err("only a .prism holds a show");
    assert!(refusal.to_string().contains(".prism"), "{refusal}");

    daemon.shutdown().await;
}

/// The JSON export and import S15 built, with commands in front of them at last.
///
/// The import is asserted to leave the Save lamp **lit**: an import an operator
/// did not mean to do must be one they can walk away from, and a daemon that
/// wrote it into the `.prism` on the way past would have taken that away.
#[tokio::test]
async fn a_show_is_exported_to_json_and_read_back_over_the_running_one() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    daemon
        .desk()
        .core()
        .apply(&Command::ExportShow {
            path: "aula.json".to_owned(),
        })
        .unwrap_or_else(|error| panic!("an export was refused: {error}"));
    let exported = dir.path().join("aula.json");
    assert!(exported.is_file());
    assert!(
        std::fs::read_to_string(&exported)
            .unwrap()
            .contains("generic.dimmer"),
        "the export is the show"
    );
    assert_eq!(
        daemon.desk().snapshot().show_file.path,
        dir.path().join("aula.prism").display().to_string(),
        "an export changes nothing, the open file included"
    );

    // Take a fixture out, then read the export back over it.
    daemon
        .desk()
        .core()
        .apply(&Command::UnpatchFixture {
            id: prism_domain::FixtureId::new(3),
        })
        .unwrap();
    assert_eq!(daemon.desk().core().file.show.fixtures().count(), 2);

    daemon
        .desk()
        .core()
        .apply(&Command::ImportShow {
            path: "aula.json".to_owned(),
        })
        .unwrap_or_else(|error| panic!("an import was refused: {error}"));
    assert_eq!(daemon.desk().core().file.show.fixtures().count(), 3);
    assert!(
        daemon.desk().snapshot().health.unsaved_changes,
        "an import is not a save"
    );

    daemon.shutdown().await;
}

/// A desk starts where it was left — S37.
///
/// The show a daemon opens with no `--show` is the one it last had open, so an
/// operator who saved under a new name on Friday finds it on Monday. The flag
/// still wins, which is what makes every other target in this repository
/// unaffected.
#[tokio::test]
async fn a_desk_starts_with_the_show_it_last_had_open() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));

    {
        let daemon = Daemon::start(&options(dir.path())).await.unwrap();
        daemon
            .desk()
            .core()
            .apply(&Command::SaveShowAs {
                path: "panto.prism".to_owned(),
            })
            .unwrap();
        daemon.shutdown().await;
    }
    assert_eq!(
        stored(dir.path()).shows().paths.first().map(String::as_str),
        Some(
            dir.path()
                .join("panto.prism")
                .display()
                .to_string()
                .as_str()
        )
    );

    // No `--show` this time.
    let mut next = options(dir.path());
    next.show = None;
    let daemon = Daemon::start(&next).await.unwrap();
    assert_eq!(
        daemon.desk().snapshot().show_file.path,
        dir.path().join("panto.prism").display().to_string()
    );
    daemon.shutdown().await;
}

/// **A new identity and a new token are made by the daemon**, never sent by a
/// client — S37, and `docs/IPC_PROTOCOL.md` §2.1 for the second of them.
///
/// The identity is the case a school meets: one laptop imaged onto twenty, all
/// of them transmitting sACN under one CID, which to a receiver is one source
/// contradicting itself.
#[tokio::test]
async fn a_desk_can_be_given_a_new_identity_and_a_new_token() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let was = daemon.desk().snapshot().machine.desk_id;

    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::NewIdentity,
        })
        .unwrap();
    let now = daemon.desk().snapshot().machine.desk_id;
    assert_ne!(now, was, "a cloned desk has to be able to stop being one");
    assert_eq!(
        stored(dir.path()).desk_id().to_string(),
        now,
        "and it has to survive the run that made it"
    );

    // §2.1: the daemon makes the token and the interface displays it. A client
    // that chose it would be choosing this desk's password.
    assert_eq!(daemon.desk().snapshot().machine.token, None);
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::NewToken,
        })
        .unwrap();
    let token = daemon
        .desk()
        .snapshot()
        .machine
        .token
        .expect("a token was made");
    assert_eq!(token.len(), 26);
    assert_eq!(
        stored(dir.path()).settings().token.as_deref(),
        Some(token.as_str())
    );

    // And only now can the listener leave loopback.
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::Websocket {
                address: Some("0.0.0.0:7373".parse().unwrap()),
            },
        })
        .unwrap_or_else(|error| panic!("with a token it should be allowed: {error}"));

    daemon.shutdown().await;
}

/// §2.1 as a refusal a running daemon makes, not only as a rule in a module.
#[tokio::test]
async fn a_listener_off_loopback_is_refused_without_a_token() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let refusal = daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::Websocket {
                address: Some("0.0.0.0:7373".parse().unwrap()),
            },
        })
        .expect_err("an unauthenticated console on a school network");
    assert!(refusal.to_string().contains("access token"), "{refusal}");
    // Nothing was written, on the platter as well as in memory.
    assert_eq!(
        stored(dir.path())
            .settings()
            .websocket
            .map(|address| address.to_string()),
        Some("127.0.0.1:7373".to_owned())
    );

    daemon.shutdown().await;
}

/// **The two questions the Outputs panel asks**, answered by a running daemon —
/// S37.
///
/// Both are *derived*: `OutputStatus` from the driver threads, `DarkUniverses`
/// from the patch and the rig together. A client that worked either out would be
/// a second opinion about something the daemon already decides, which is the
/// trap S27 named about address overlaps and D3 forbids.
#[tokio::test]
async fn the_outputs_panel_asks_what_the_drivers_are_doing_and_what_goes_nowhere() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    // Nothing configured: every patched universe goes nowhere, and there are no
    // drivers to ask about.
    let Answer::DarkUniverses { universes } = daemon.desk().query(&Query::DarkUniverses) else {
        panic!("DarkUniverses has to be answered with DarkUniverses");
    };
    assert_eq!(universes, vec![UniverseId::new(1)]);
    let Answer::OutputStatus { outputs } = daemon.desk().query(&Query::OutputStatus) else {
        panic!("OutputStatus has to be answered with OutputStatus");
    };
    assert!(outputs.is_empty());

    // With a cable on universe 1, nothing is dark and the counter moves — which
    // is the whole reason this is a question: `OutputSnapshot` carries a counter
    // that was true when the client connected, and a row added afterwards would
    // read zero for ever.
    add_output(&daemon, 1);
    let Answer::DarkUniverses { universes } = daemon.desk().query(&Query::DarkUniverses) else {
        panic!("DarkUniverses has to be answered with DarkUniverses");
    };
    assert!(universes.is_empty(), "universe 1 has a cable now");

    until("the counter to move", || {
        matches!(
            daemon.desk().query(&Query::OutputStatus),
            Answer::OutputStatus { outputs } if outputs.first().is_some_and(|row| row.frames_sent > 3)
        )
    })
    .await;
    let Answer::OutputStatus { outputs } = daemon.desk().query(&Query::OutputStatus) else {
        panic!("OutputStatus has to be answered with OutputStatus");
    };
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].id, OutputId::new(1));
    assert_eq!(outputs[0].last_error, None);
    assert_eq!(outputs[0].last_error_ago_ms, None);

    daemon.shutdown().await;
}

/// **Naming a binding profile again is the reload** — S37, and the Devices
/// panel's third row.
///
/// A profile that is missing or malformed is reported and **the table in force
/// stands**, which is S22's rule: a broken JSON file beside a daemon must never
/// be the reason a desk stops answering its keys.
///
/// # S38 split two things this test used to be able to say in one sentence
///
/// S22 said *a malformed profile falls back to the built-in defaults*, and it
/// said so about a desk starting up with nothing else to fall back on — so until
/// S38 *the table it had* and *the built-in table* were the same table and this
/// test could assert either. They are not the same once the table is editable: a
/// desk that has a table of its own has something better to keep than the
/// defaults, and throwing an operator's work away over a typo in a file would be
/// the one outcome worse than ignoring the file.
///
/// So the claim is now the stronger one the sentence always made — **the keys it
/// had** — and both readings are asserted: a broken file leaves a desk that has
/// read a good one on the good one, and leaves a desk that has never read one on
/// the built-in table.
#[tokio::test]
async fn a_binding_profile_is_read_by_naming_it_and_a_broken_one_never_stops_the_desk() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    // A surface with no device behind it, so the reload walks the path a desk
    // walks: the link is rebuilt **round the same port**, because a port holds
    // an open connection and a count of how many times the cable has come out,
    // and re-reading a profile must not throw any of that away.
    let (port, _handle) = prismd::surface::MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    let f1 = prism_surface::BoundControl::Global {
        button: prism_surface::GlobalButton::F1,
    };
    let built_in = daemon.bindings().action(f1);
    assert!(built_in.is_some(), "F1 is bound out of the box");

    // A profile is the **whole** table and not an overlay — `Bindings::parse`
    // starts from `Bindings::empty` — which is S22's *a profile is refused whole,
    // not row by row* seen from the other side: a table half of which came from a
    // file would be a desk doing some of what its author intended. So the file
    // below binds F1 and unbinds everything else, and both halves are asserted.
    let profile = dir.path().join("xtouch.json");
    std::fs::write(
        &profile,
        br#"{"profileVersion":1,"device":"behringer-x-touch","bindings":[{"control":"Global.F1","action":{"t":"Oops"}}]}"#,
    )
    .unwrap();
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceProfile {
                path: Some(profile.display().to_string()),
            },
        })
        .unwrap();
    // The run loop is what picks it up, on the housekeeping tick.
    daemon
        .run(Some(Duration::from_millis(700)), std::future::pending())
        .await;
    assert_eq!(
        daemon.bindings().action(f1),
        Some(prism_surface::SurfaceAction::Oops),
        "the named table is in force"
    );
    assert_eq!(
        daemon
            .bindings()
            .action(prism_surface::BoundControl::Global {
                button: prism_surface::GlobalButton::F2
            }),
        None,
        "a profile is the whole table: what it does not name is not bound"
    );

    // A file that is not there: reported, and **the table in force stands**.
    // That is the half S38 changed, and this desk has a table of its own by now,
    // so *the keys it had* is the profile's `Oops` rather than the built-in
    // `OpenWindow`.
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceProfile {
                path: Some(dir.path().join("nothing.json").display().to_string()),
            },
        })
        .unwrap();
    daemon
        .run(Some(Duration::from_millis(700)), std::future::pending())
        .await;
    assert_eq!(
        daemon.bindings().action(f1),
        Some(prism_surface::SurfaceAction::Oops),
        "a broken profile leaves the desk with the keys it had"
    );

    // And one that will not parse, which is the other way a file can be
    // unusable: same answer, and the whole table is still the good one's.
    let broken = dir.path().join("broken.json");
    std::fs::write(&broken, b"{ this is not JSON").unwrap();
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceProfile {
                path: Some(broken.display().to_string()),
            },
        })
        .unwrap();
    daemon
        .run(Some(Duration::from_millis(700)), std::future::pending())
        .await;
    assert_eq!(
        daemon.bindings().action(f1),
        Some(prism_surface::SurfaceAction::Oops),
        "a profile that will not parse leaves the desk with the keys it had"
    );

    // And back to the built-in one by name, which is what a cleared box means.
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceProfile { path: None },
        })
        .unwrap();
    daemon
        .run(Some(Duration::from_millis(700)), std::future::pending())
        .await;
    assert_eq!(daemon.bindings().action(f1), built_in);
    assert_eq!(daemon.desk().snapshot().machine.surface_profile, None);
    // The surface is still attached throughout: a reload is a table swap and
    // not a port being reopened, which is the one thing S20 established cannot
    // recover a desk that has gone quiet.
    assert!(daemon.surface().is_some());

    daemon.shutdown().await;
}

/// The exit action reaches the daemon that acts on it, without a restart — S37.
#[tokio::test]
async fn what_the_stage_does_when_the_daemon_stops_is_changed_while_it_runs() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    add_output(&daemon, 1);

    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::ExitAction {
                action: ExitAction::Blackout,
            },
        })
        .unwrap();
    daemon
        .run(Some(Duration::from_millis(700)), std::future::pending())
        .await;

    // The stage goes dark on the way out, which is what the setting asked for
    // — asserted on the **frame**, because a blackout is a frame and not a flag
    // (S10).
    let handle = daemon.recorded_output(OutputId::new(1)).unwrap();
    daemon.shutdown().await;
    let last = handle
        .frames()
        .into_iter()
        .next_back()
        .expect("the rig was driven");
    assert!(
        last.1.iter().all(|level| *level == 0),
        "the last frame is not a blackout"
    );
}

/// A daemon whose **outputs** came off its command line refuses a setting
/// change too, and says which flag.
///
/// S36 found this in its own tests and it is right rather than incidental: the
/// reason the rule exists is that *nothing is written down this run*, which
/// applies to a log level exactly as it applies to a rig.
#[tokio::test]
async fn a_daemon_configured_on_the_command_line_refuses_a_setting() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));

    let mut options = options(dir.path());
    options.outputs = vec![prismd::cli::mock_output(1)];
    options.mock_devices = false;
    let daemon = Daemon::start(&options).await.unwrap();

    let refusal = daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::Autostart { autostart: true },
        })
        .expect_err("nothing is written down this run");
    assert!(refusal.to_string().contains("command line"), "{refusal}");
    assert!(
        daemon
            .desk()
            .snapshot()
            .machine
            .is_overridden(MachineOverride::Outputs),
        "and the panel is told, so the row is greyed rather than dead"
    );

    daemon.shutdown().await;
}
