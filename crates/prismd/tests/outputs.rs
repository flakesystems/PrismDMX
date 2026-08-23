//! The output patch, against a running daemon — S33.
//!
//! `tests/wiring.rs` proves one output reaches one wire. This target is the
//! *rig*: several outputs, of several kinds, each carrying the universes it was
//! actually given, reconfigured while the show runs.
//!
//! # No device is touched and nothing goes on the network
//!
//! `CLAUDE.md` requires every test to run deterministically with nothing
//! plugged in, and this machine has a real SH-RS09B attached that the suite must
//! not open. So the rig here is the **real** configuration — two Open DMX
//! adapters, an sACN gateway, two Art-Net nodes — opened through
//! `--mock-devices`, which replaces the last inch of every driver with a
//! recording double. What is asserted is therefore the *routing*, which is what
//! S33 is about; that each driver puts the right bytes on its own wire is
//! `prism-protocols`' claim and is asserted there, packet by packet.
//!
//! # Why the frames are asserted rather than the configuration
//!
//! A rig that is configured correctly and delivers nothing is the failure this
//! session exists to prevent. Every claim here is read off
//! `MockOutputHandle::timeline` — the frames a driver was actually handed, with
//! the times they arrived — which is the shape S18 established for the D2 gate
//! and for the same reason: a list of frames with no times in it cannot tell a
//! driver's own cadence from a stage that stopped.

// Every test holds `common::one_daemon_at_a_time` across its awaits, for
// `wiring.rs`'s reason: a daemon owns a real-time tick thread, and several in
// one process measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::{ShowFile, ShowStore};
use prism_domain::{
    ArtNetPort, Command, OutputChange, OutputId, OutputInstance, OutputKind, UniverseId,
};
use prism_protocols::MockOutputHandle;
use prismd::cli::Options;
use prismd::daemon::Daemon;

mod common;

/// The twelve universes of the worked example, one dimmer patched into each.
///
/// Patched rather than merely configured, because `prismd` builds its frame
/// layout from the desk's whole range but a *universe with nothing in it* is not
/// what an installer is wiring for — and because the dark-universe report is
/// about the universes a show **uses**.
fn write_twelve_universe_show(path: &Path) {
    let mut file = ShowFile::new();
    file.show
        .embed_fixture_type(common::dimmer_type("generic.dimmer", 65_535))
        .unwrap();
    for universe in 1..=12u32 {
        file.show
            .patch_fixture(common::fixture(universe, "generic.dimmer", universe, 1))
            .unwrap();
    }
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: Some(12),
        // No `--mock-output`: the rig comes out of this machine's configuration,
        // which is what a venue runs and what the four commands edit.
        outputs: Vec::new(),
        // …but every driver is a double, so no cable is opened and no datagram
        // leaves this process.
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

fn universes(list: &[u32]) -> Vec<UniverseId> {
    list.iter().copied().map(UniverseId::new).collect()
}

/// **S33's worked example**, written out the way an installer would read it off
/// the back of the rack.
fn worked_example() -> Vec<OutputInstance> {
    vec![
        OutputInstance::new(
            OutputId::new(1),
            "Stage left dimmers",
            OutputKind::OpenDmx {
                serial: Some("B0037HIY".to_owned()),
            },
            universes(&[1]),
        ),
        OutputInstance::new(
            OutputId::new(2),
            "Stage right dimmers",
            OutputKind::OpenDmx {
                serial: Some("A50285BI".to_owned()),
            },
            universes(&[2]),
        ),
        OutputInstance::new(
            OutputId::new(3),
            "Hall gateway",
            OutputKind::Sacn {
                receivers: Vec::new(),
                ttl: 1,
                ports: Vec::new(),
            },
            universes(&[3, 4]),
        ),
        OutputInstance::new(
            OutputId::new(4),
            "Bridge node",
            OutputKind::ArtNet {
                nodes: vec!["127.0.0.1:6454".parse().unwrap()],
                sync: false,
                ports: vec![
                    ArtNetPort {
                        universe: UniverseId::new(5),
                        net: 0,
                        sub_net: 0,
                        port: 0,
                    },
                    ArtNetPort {
                        universe: UniverseId::new(6),
                        net: 0,
                        sub_net: 0,
                        port: 1,
                    },
                    ArtNetPort {
                        universe: UniverseId::new(7),
                        net: 0,
                        sub_net: 0,
                        port: 2,
                    },
                    ArtNetPort {
                        universe: UniverseId::new(8),
                        net: 0,
                        sub_net: 0,
                        port: 3,
                    },
                ],
            },
            universes(&[5, 6, 7, 8]),
        ),
        OutputInstance::new(
            OutputId::new(5),
            "Gallery node",
            OutputKind::ArtNet {
                nodes: vec!["127.0.0.1:6455".parse().unwrap()],
                sync: false,
                ports: Vec::new(),
            },
            universes(&[9, 10, 11, 12]),
        ),
    ]
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

/// The universes one driver was given, once each and in order.
fn universes_seen(handle: &MockOutputHandle) -> Vec<u32> {
    let mut seen: Vec<u32> = handle
        .frames()
        .into_iter()
        .map(|(universe, _)| universe.get())
        .collect();
    seen.sort_unstable();
    seen.dedup();
    seen
}

/// **The exit criterion, configured and asserted.**
///
/// Twelve universes across five outputs of three kinds, built entirely from
/// commands — which is also the claim that the rig is *data*: nothing here is a
/// command-line flag, and the same lines a settings window will send are what
/// build it.
#[tokio::test]
async fn twelve_universes_across_five_outputs_each_get_exactly_their_own() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_twelve_universe_show(&dir.path().join("aula.prism"));

    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    assert!(
        daemon.recorded_outputs().is_empty(),
        "a desk with no rig configured yet has no outputs, which is a legitimate state"
    );

    for output in worked_example() {
        daemon
            .desk()
            .core()
            .apply(&Command::AddOutput { output })
            .unwrap_or_else(|error| panic!("the rig was refused: {error}"));
    }

    let expected: [(u32, Vec<u32>); 5] = [
        (1, vec![1]),
        (2, vec![2]),
        (3, vec![3, 4]),
        (4, vec![5, 6, 7, 8]),
        (5, vec![9, 10, 11, 12]),
    ];
    for (id, _) in &expected {
        let handle = daemon
            .recorded_output(OutputId::new(*id))
            .unwrap_or_else(|| panic!("output {id} is not running"));
        until(&format!("output {id} to send"), || handle.frame_count() > 0).await;
    }

    for (id, wanted) in expected {
        let handle = daemon.recorded_output(OutputId::new(id)).unwrap();
        assert_eq!(
            universes_seen(&handle),
            wanted,
            "output {id} was given universes it was not configured for, or missed its own"
        );
    }

    daemon.shutdown().await;
}

/// One universe may go to several outputs, and it arrives **byte-identical** at
/// both — which is the other half of the routing claim: the frame is fanned out
/// rather than shared, so a driver that mutated one would be visible here.
#[tokio::test]
async fn a_universe_sent_to_two_outputs_arrives_byte_identical_at_both() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_twelve_universe_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    for id in [1u32, 2] {
        daemon
            .desk()
            .core()
            .apply(&Command::AddOutput {
                output: OutputInstance::new(
                    OutputId::new(id),
                    format!("Node {id}"),
                    OutputKind::Mock,
                    universes(&[3]),
                ),
            })
            .unwrap();
    }

    let first = daemon.recorded_output(OutputId::new(1)).unwrap();
    let second = daemon.recorded_output(OutputId::new(2)).unwrap();
    until("both to have sent several frames", || {
        first.frame_count() > 3 && second.frame_count() > 3
    })
    .await;

    let one = common::last_frame_of(&first, UniverseId::new(3)).unwrap();
    let two = common::last_frame_of(&second, UniverseId::new(3)).unwrap();
    assert_eq!(one.len(), 512);
    assert_eq!(one[0], 255, "the dimmer is at home, which is full");
    assert_eq!(one, two, "the same universe is the same 512 bytes");

    daemon.shutdown().await;
}

/// **Neither an output arriving nor one leaving costs a tick**, asserted on the
/// captured frame sequence the way S18 asserts the D2 gate.
///
/// Two claims, because either alone is passed by a daemon broken in the other
/// way: the output that did not change has **no silence** across the whole
/// reconfiguration, and the one that arrived starts sending within a refresh
/// interval while the one that left stops.
#[tokio::test]
async fn adding_and_removing_an_output_mid_show_costs_the_others_nothing() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_twelve_universe_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    let core = || daemon.desk().core();
    core()
        .apply(&Command::AddOutput {
            output: OutputInstance::new(
                OutputId::new(1),
                "Standing",
                OutputKind::Mock,
                universes(&[1]),
            ),
        })
        .unwrap();
    let standing = daemon.recorded_output(OutputId::new(1)).unwrap();
    until("the standing output to be running", || {
        standing.frame_count() > 5
    })
    .await;

    // One arrives while the show runs.
    core()
        .apply(&Command::AddOutput {
            output: OutputInstance::new(
                OutputId::new(2),
                "Arriving",
                OutputKind::Mock,
                universes(&[2]),
            ),
        })
        .unwrap();
    let arriving = daemon.recorded_output(OutputId::new(2)).unwrap();
    until(
        "the arriving output to send within a refresh interval",
        || arriving.frame_count() > 0,
    )
    .await;

    // …and one leaves.
    core()
        .apply(&Command::RemoveOutput {
            id: OutputId::new(2),
        })
        .unwrap();
    assert!(
        daemon.recorded_output(OutputId::new(2)).is_none(),
        "a removed output is not a running one"
    );
    let after_removal = arriving.frame_count();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        arriving.frame_count(),
        after_removal,
        "a removed output goes on sending"
    );

    // The claim the whole design is for: the output that did not change never
    // stopped. 250 ms is S18's threshold and one output cadence is 23 ms, so a
    // second's worth of frames spans the whole reconfiguration several times
    // over.
    until(
        "a second's worth of frames across the reconfiguration",
        || standing.frame_count() > 44,
    )
    .await;
    let timeline = standing.timeline();
    assert!(timeline.len() > 44, "{} frames", timeline.len());
    for pair in timeline.windows(2) {
        let gap = pair[1].at.duration_since(pair[0].at);
        assert!(
            gap < Duration::from_millis(250),
            "the standing output was silent for {gap:?} while the rig was reconfigured"
        );
    }

    daemon.shutdown().await;
}

/// Re-addressing an output while the show runs moves *that* output's universes
/// and leaves every other one alone — and a **rename costs nothing at all**,
/// because an operator who typed a better name should not watch their rig blink.
#[tokio::test]
async fn re_addressing_an_output_moves_only_that_output() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_twelve_universe_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    for (id, universe) in [(1u32, 1u32), (2, 2)] {
        daemon
            .desk()
            .core()
            .apply(&Command::AddOutput {
                output: OutputInstance::new(
                    OutputId::new(id),
                    format!("Output {id}"),
                    OutputKind::Mock,
                    universes(&[universe]),
                ),
            })
            .unwrap();
    }
    let standing = daemon.recorded_output(OutputId::new(1)).unwrap();
    until("both outputs running", || {
        standing.frame_count() > 5
            && daemon
                .recorded_output(OutputId::new(2))
                .is_some_and(|handle| handle.frame_count() > 5)
    })
    .await;

    // A rename: the same driver goes on sending, which is what
    // `OutputInstance::needs_restart` is narrow for.
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureOutput {
            id: OutputId::new(1),
            change: OutputChange::Name {
                name: "Hall dimmers".to_owned(),
            },
        })
        .unwrap();
    let same_driver = daemon.recorded_output(OutputId::new(1)).unwrap();
    assert!(
        same_driver.frame_count() >= standing.frame_count(),
        "a rename replaced the driver and lost its recording"
    );
    assert_eq!(
        daemon
            .desk()
            .output_snapshots()
            .iter()
            .find(|snapshot| snapshot.id == OutputId::new(1))
            .map(|snapshot| snapshot.name.clone()),
        Some("Hall dimmers".to_owned())
    );

    // A re-addressing: a new driver on the new universe, and the recording of
    // the old one stops where it stopped.
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureOutput {
            id: OutputId::new(2),
            change: OutputChange::Universes {
                universes: universes(&[7]),
            },
        })
        .unwrap();
    let readdressed = daemon.recorded_output(OutputId::new(2)).unwrap();
    until(
        "the re-addressed output to send on its new universe",
        || !universes_seen(&readdressed).is_empty(),
    )
    .await;
    assert_eq!(universes_seen(&readdressed), vec![7]);

    // And the one that did not change never stopped.
    for pair in standing.timeline().windows(2) {
        assert!(
            pair[1].at.duration_since(pair[0].at) < Duration::from_millis(250),
            "the output that was not touched went silent"
        );
    }

    daemon.shutdown().await;
}

/// **An output whose device disappears degrades alone.**
///
/// The other four keep sending and the engine never hears about it, which is
/// `CLAUDE.md`'s zero-crash invariant at the one place it is really about: a
/// driver that panics on every frame must cost frames on its own line and
/// nothing anywhere else.
#[tokio::test]
async fn an_output_whose_device_disappears_degrades_alone() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_twelve_universe_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    for output in worked_example() {
        daemon
            .desk()
            .core()
            .apply(&Command::AddOutput { output })
            .unwrap();
    }
    let handles: Vec<(u32, MockOutputHandle)> = (1..=5)
        .map(|id| (id, daemon.recorded_output(OutputId::new(id)).unwrap()))
        .collect();
    for (id, handle) in &handles {
        until(&format!("output {id} to send"), || handle.frame_count() > 3).await;
    }

    // The cable is pulled — and then panics on the way back, which is the worse
    // half: a driver that unwinds must not take the process with it.
    let ticks_before = daemon.desk().core().engine().health().ticks();
    // Taken here rather than compared against zero, because `missed()` counts
    // from start-up and this test is about **the fault**. A cold two-core CI
    // runner can perfectly well drop one 22.7 ms deadline while a
    // twelve-universe show and five driver threads are still coming up, and a
    // test that called that a regression would be measuring the runner. What
    // must not move is the count across the panic, which is what the message
    // below has always claimed. (Loosened in S36, after run 32579984199 read 1
    // where every run before it had read 0.)
    let missed_before = daemon.desk().core().engine().health().missed();
    let failing = &handles[2].1;
    failing.fail_send(20, prism_protocols::OutputError::Disconnected);
    failing.panic_on_send(3);

    let counts: Vec<usize> = handles
        .iter()
        .map(|(_, handle)| handle.frame_count())
        .collect();
    tokio::time::sleep(Duration::from_millis(300)).await;

    for ((id, handle), before) in handles.iter().zip(counts) {
        if *id == 3 {
            continue;
        }
        assert!(
            handle.frame_count() > before,
            "output {id} stopped when output 3 lost its device"
        );
    }
    let ticks_after = daemon.desk().core().engine().health().ticks();
    assert!(
        ticks_after > ticks_before,
        "the engine noticed a driver's fault"
    );
    assert_eq!(
        daemon.desk().core().engine().health().missed(),
        missed_before,
        "and it did not miss a tick over it"
    );

    daemon.shutdown().await;
}

/// A patched universe that no output carries is **reported**, not dropped.
#[tokio::test]
async fn a_universe_with_nowhere_to_go_is_reported() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_twelve_universe_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();

    // Nothing is wired at all: all twelve are named.
    assert_eq!(daemon.desk().core().dark_universes().len(), 12);

    for output in worked_example() {
        daemon
            .desk()
            .core()
            .apply(&Command::AddOutput { output })
            .unwrap();
    }
    assert!(daemon.desk().core().dark_universes().is_empty());

    // Switching a node off puts its universes back in the dark, and the
    // operator is told so with the command rather than at the next start.
    let deltas = daemon
        .desk()
        .core()
        .apply(&Command::SetOutputEnabled {
            id: OutputId::new(3),
            enabled: false,
        })
        .unwrap();
    let notices: Vec<String> = deltas
        .iter()
        .filter_map(|delta| match delta {
            prism_domain::Delta::Notice { message, .. } => Some(message.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(notices.len(), 2, "{notices:?}");
    assert!(notices[0].contains("universe 3"), "{notices:?}");
    assert!(notices[1].contains("universe 4"), "{notices:?}");
    assert_eq!(
        daemon
            .desk()
            .core()
            .dark_universes()
            .iter()
            .map(ToString::to_string)
            .count(),
        2
    );

    daemon.shutdown().await;
}

/// **The configuration survives a restart**, and a show copied to a second
/// machine arrives with no output configuration in it — the two halves of where
/// the rig lives, asserted the way `desk_id_is_not_show_content` asserts the
/// identity.
#[tokio::test]
async fn the_rig_survives_a_restart_and_never_travels_with_the_show() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let show_path = dir.path().join("aula.prism");
    write_twelve_universe_show(&show_path);

    {
        let daemon = Daemon::start(&options(dir.path())).await.unwrap();
        for output in worked_example() {
            daemon
                .desk()
                .core()
                .apply(&Command::AddOutput { output })
                .unwrap();
        }
        daemon.desk().core().save().unwrap();
        daemon.shutdown().await;
    }

    // The show file, byte for byte: not one mention of the hall's cabling.
    let show_bytes = std::fs::read(&show_path).unwrap();
    let show_text = String::from_utf8_lossy(&show_bytes);
    for word in [
        "Stage left dimmers",
        "Hall gateway",
        "Bridge node",
        "127.0.0.1",
        "B0037HIY",
    ] {
        assert!(
            !show_text.contains(word),
            "the show file carries the hall's cabling: {word}"
        );
    }

    // And the machine configuration beside it has all of it.
    let machine = std::fs::read_to_string(dir.path().join("machine.json")).unwrap();
    assert!(machine.contains("Bridge node"), "{machine}");
    assert!(machine.contains("127.0.0.1:6454"), "{machine}");

    // A second start finds the rig where it left it.
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let snapshots = daemon.desk().output_snapshots();
    assert_eq!(snapshots.len(), 5);
    assert_eq!(snapshots[3].name, "Bridge node");
    assert_eq!(
        snapshots[3]
            .output
            .as_ref()
            .map(|output| output.universes.clone()),
        Some(universes(&[5, 6, 7, 8]))
    );
    for (id, _) in [(1u32, ()), (2, ()), (3, ()), (4, ()), (5, ())] {
        assert!(
            daemon.recorded_output(OutputId::new(id)).is_some(),
            "output {id} did not come back"
        );
    }
    daemon.shutdown().await;
}

/// A daemon whose rig came off its command line refuses to change it — and says
/// why, rather than appearing to work and forgetting at the next restart.
#[tokio::test]
async fn a_rig_named_on_the_command_line_is_not_the_machines_to_change() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_twelve_universe_show(&dir.path().join("aula.prism"));

    let mut options = options(dir.path());
    options.outputs = vec![prismd::cli::mock_output(1)];
    let daemon = Daemon::start(&options).await.unwrap();

    // It is running: the flag's output carries the show's twelve universes.
    let handle = daemon.recorded_output(OutputId::new(1)).unwrap();
    until("the flag's output to send", || handle.frame_count() > 12).await;
    assert_eq!(universes_seen(&handle), (1..=12).collect::<Vec<_>>());

    let error = daemon
        .desk()
        .core()
        .apply(&Command::RemoveOutput {
            id: OutputId::new(1),
        })
        .unwrap_err();
    assert!(error.to_string().contains("command line"), "{error}");

    // And the machine configuration on disk is untouched: a test's rig must not
    // become a venue's.
    let machine = std::fs::read_to_string(dir.path().join("machine.json")).unwrap();
    assert!(!machine.contains("Mock output"), "{machine}");
    assert!(machine.contains("deskId"), "{machine}");

    daemon.shutdown().await;
}

/// A row that is not usable is reported and skipped, and the rest of the rig
/// starts — a hand-edited `machine.json` with one bad hop limit in it must not
/// be a desk that will not start half an hour before a show.
#[tokio::test]
async fn one_unusable_row_does_not_stop_the_rest_of_the_rig() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_twelve_universe_show(&dir.path().join("aula.prism"));

    // Written by hand, the way a technician would: one good row and one whose
    // hop limit leaves no machine at all.
    let machine = serde_json::json!({
        "deskId": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
        "outputs": [
            {
                "id": 1,
                "name": "Hall",
                "kind": { "t": "Mock" },
                "universes": [1],
                "enabled": true
            },
            {
                "id": 2,
                "name": "Broken gateway",
                "kind": { "t": "Sacn", "receivers": [], "ttl": 0, "ports": [] },
                "universes": [2],
                "enabled": true
            }
        ]
    });
    std::fs::write(
        dir.path().join("machine.json"),
        serde_json::to_string_pretty(&machine).unwrap(),
    )
    .unwrap();

    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let handle = daemon.recorded_output(OutputId::new(1)).unwrap();
    until("the usable output to send", || handle.frame_count() > 0).await;
    assert!(
        daemon.recorded_output(OutputId::new(2)).is_none(),
        "a row nothing can open must not reach a driver"
    );
    // It is still a row an operator can see, and it is red.
    let snapshots = daemon.desk().output_snapshots();
    assert_eq!(snapshots.len(), 2, "the broken row is shown, not hidden");
    assert_eq!(
        snapshots[1].health,
        prism_domain::OutputHealth::Disconnected
    );

    daemon.shutdown().await;
}
