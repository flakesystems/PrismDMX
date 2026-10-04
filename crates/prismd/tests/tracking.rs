//! A tracker moves a light - **S32**, asserted on a running daemon.
//!
//! The chain this target holds end to end:
//!
//! ```text
//!   a PSN datagram ─▶ TrackerReceiver ─▶ TrackerTable ─▶ the tick's follow layer
//!        (mock socket)   (mapped)          (one word)      (aim, mixed into pan/tilt)
//!                                                                  │
//!   the byte on the wire  ◀───────────────────────────────────────┘
//! ```
//!
//! Everything below the datagram is the real thing: the real receiver thread,
//! the real table, the real tick. What is a double is the **socket** - a queue a
//! test feeds - because `CLAUDE.md` forbids a test from touching a network, and
//! S10's sharper rule forbids one from putting a multicast datagram on the one it
//! runs on.
//!
//! # What it asserts
//!
//! - a tracker's position becomes a head's pan and tilt on the cable;
//! - **the programmer beats the follow** (`docs/DMX_MERGE.md` §3), and clearing it
//!   hands the head back to the tracker;
//! - a tracker that goes quiet leaves the head **where it was**, with no jump, and
//!   the operator is told once - and told once when it comes back;
//! - giving a head a tracker **does not stop a running cue list**: the body is not
//!   rebuilt (`TickHealth::swaps`), only the follow layer is handed over;
//! - Oops takes a tracker back, and the head goes back to the cues.

// Every test holds `common::one_daemon_at_a_time` across its awaits, for
// `wiring.rs`'s reason.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use prism_core::{ShowFile, ShowStore};
use prism_domain::{
    AttributeDef, AttributeType, Command, FixtureId, FixturePlace, FixtureType, FollowTarget,
    MachineChange, SelectionMode, TrackerChange, UniverseId, Vec3,
};
use prism_protocols::psn::{encode_data, encode_info};
use prism_protocols::{MockUdpNode, MockUdpNodeHandle, UdpNode};
use prismd::cli::Options;
use prismd::daemon::Daemon;
use prismd::discovery::SocketSource;
use prismd::tracking::Tracking;

mod common;

/// A socket source handing out one mock socket a test feeds.
struct MockSockets(Mutex<Option<MockUdpNode>>);

impl MockSockets {
    fn new() -> (Arc<Self>, MockUdpNodeHandle) {
        let socket = MockUdpNode::new();
        let handle = socket.handle();
        (Arc::new(Self(Mutex::new(Some(socket)))), handle)
    }
}

impl SocketSource for MockSockets {
    fn open(&self) -> Box<dyn UdpNode> {
        self.0
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
            .map_or_else(
                || Box::new(MockUdpNode::new()) as Box<dyn UdpNode>,
                |socket| Box::new(socket) as Box<dyn UdpNode>,
            )
    }
}

fn from() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([10, 0, 0, 9], 56_565))
}

/// A moving head: a dimmer, a pan of +-270 degrees and a tilt of +-135, both
/// resting centred. Three 8-bit channels.
fn head_type() -> FixtureType {
    let channel =
        |attribute: AttributeType, offset: u16, home: u16, from: f64, to: f64| AttributeDef {
            switched: None,
            attribute,
            label: None,
            occurrence: 0,
            feature_group: attribute.feature_group(),
            coarse_offset: offset,
            fine_offset: None,
            default_value: home,
            merge_mode: attribute.default_merge_mode(),
            invert: false,
            physical_from: from,
            physical_to: to,
            ranges: Vec::new(),
        };
    FixtureType {
        id: "test.head".to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Head".to_owned(),
        mode: "3ch".to_owned(),
        footprint: 3,
        attributes: vec![
            channel(AttributeType::Dimmer, 0, 0, 0.0, 100.0),
            channel(AttributeType::Pan, 1, 32_768, -270.0, 270.0),
            channel(AttributeType::Tilt, 2, 32_768, -135.0, 135.0),
        ],
        physical: None,
    }
}

/// One head at `(0, 6, 0)` hanging at nought, and a dimmer that a cue list can
/// raise - so a running playback is something the follow can be shown not to
/// stop.
fn write_show(path: &Path) {
    let mut file = ShowFile::new();
    file.show.embed_fixture_type(head_type()).unwrap();
    file.show
        .embed_fixture_type(common::dimmer_type("generic.dimmer", 0))
        .unwrap();
    let mut head = common::fixture(1, "test.head", 1, 1);
    head.position = Vec3 {
        x: 0.0,
        y: 6.0,
        z: 0.0,
    };
    file.show.patch_fixture(head).unwrap();
    file.show
        .patch_fixture(common::fixture(2, "generic.dimmer", 1, 10))
        .unwrap();
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

/// **Every driver is a double and the rig is not on the command line**: a daemon
/// whose outputs came off its command line refuses every machine command
/// (`MachineError::ConfiguredOnTheCommandLine`), and a test of a *machine
/// setting* has to be able to make one. The output is added with the same
/// command an operator's panel sends.
fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: Some(1),
        outputs: Vec::new(),
        mock_devices: true,
        local: Some(false),
        websocket: prismd::cli::Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
        ..Options::default()
    }
}

/// A daemon with one mock output carrying universe 1, and what that output has
/// been handed.
async fn start(dir: &Path) -> (Daemon, prism_protocols::MockOutputHandle) {
    let daemon = Daemon::start(&options(dir)).await.unwrap();
    daemon
        .desk()
        .core()
        .apply(&Command::AddOutput {
            output: prism_domain::OutputInstance::new(
                prism_domain::OutputId::new(1),
                "Cable",
                prism_domain::OutputKind::Mock,
                [UniverseId::new(1)],
            ),
        })
        .unwrap();
    let frames = daemon
        .recorded_output(prism_domain::OutputId::new(1))
        .expect("the output has a recording");
    (daemon, frames)
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

/// The byte at `channel` (1-based) of the last frame of universe 1.
fn byte(frames: &prism_protocols::MockOutputHandle, channel: usize) -> Option<u8> {
    common::last_frame_of(frames, UniverseId::new(1))
        .and_then(|data| data.get(channel - 1).copied())
}

/// Switches the receiver on, over a mock socket, the way an operator would: the
/// machine setting, not a back door.
fn listen(daemon: &Daemon) -> MockUdpNodeHandle {
    listen_with(daemon, &[])
}

/// [`listen`], with other settings made **first**: a setting changed while the
/// receiver is running restarts it on a new socket, which is the right thing for
/// a desk and the wrong one for a test that holds the first socket's handle.
fn listen_with(daemon: &Daemon, first: &[TrackerChange]) -> MockUdpNodeHandle {
    let (source, socket) = MockSockets::new();
    daemon
        .desk()
        .core()
        .adopt_tracking(Tracking::with_source(source));
    for change in first
        .iter()
        .cloned()
        .chain([TrackerChange::Enabled { enabled: true }])
    {
        daemon
            .desk()
            .core()
            .apply(&Command::ConfigureMachine {
                change: MachineChange::Tracker { change },
            })
            .unwrap();
    }
    socket
}

/// Gives head 1 tracker `tracker`, aimed at the performer's position.
fn follow(daemon: &Daemon, tracker: Option<u16>) {
    daemon
        .desk()
        .core()
        .apply(&Command::PlaceFixtures {
            placements: vec![FixturePlace {
                id: FixtureId::new(1),
                position: Vec3 {
                    x: 0.0,
                    y: 6.0,
                    z: 0.0,
                },
                rotation: Vec3::ZERO,
                follow: tracker.map(|tracker| FollowTarget {
                    tracker,
                    offset: Vec3::ZERO,
                }),
                mirror: prism_domain::Mirror::default(),
            }],
        })
        .unwrap();
}

/// The operator puts head 1 on its tracker, all the way, from the programmer.
fn follow_all_the_way(daemon: &Daemon, amount: i32) {
    let mut core = daemon.desk().core();
    core.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(1)],
        mode: SelectionMode::Set,
    })
    .unwrap();
    core.apply(&Command::SetAttribute {
        attribute: AttributeType::Follow,
        occurrence: 0,
        value: amount,
        relative: false,
    })
    .unwrap();
}

/// A position as the tracking system sends it: with the default mapping its `y`
/// is show space's depth and its `z` is height, so `[0, -6, 0]` is a point
/// downstage on the floor - 45 degrees of tilt from a head six metres up.
const DOWNSTAGE: [f32; 3] = [0.0, -6.0, 0.0];

/// 45 degrees of a tilt of +-135 degrees, as an 8-bit value: `180 / 270 * 255`.
const TILT_45: u8 = 170;

#[tokio::test]
async fn a_tracker_moves_a_head_that_follows_it() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;

    let socket = listen(&daemon);
    follow(&daemon, Some(5));
    follow_all_the_way(&daemon, 65_535);
    // At home, until the tracker says anything: a head cannot be aimed at a
    // tracker nobody has heard.
    until("the head at home", || byte(&frames, 3) == Some(128)).await;

    socket.deliver(from(), &encode_data(0, 0, &[(5, DOWNSTAGE)]));
    until("the head to tilt towards the performer", || {
        byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(TILT_45) <= 1)
    })
    .await;
    assert_eq!(
        byte(&frames, 2),
        Some(128),
        "downstage is straight ahead: pan nought"
    );

    // And across the stage: show space's `x` is the tracker's.
    socket.deliver(from(), &encode_data(0, 0, &[(5, [6.0, 0.0, 0.0])]));
    until("the head to swing across", || {
        byte(&frames, 2).is_some_and(|pan| pan != 128)
    })
    .await;
    daemon.shutdown().await;
}

/// **A head whose motor runs the other way is aimed the other way** - the
/// operator's own report, on a head standing on the floor. Flipping the tilt on
/// the running desk swaps the follow layer alone and the same performer now puts
/// the head at the mirror of the angle, which is what makes the real head, whose
/// motor runs backwards, point at them.
#[tokio::test]
async fn a_mirrored_tilt_aims_the_head_the_other_way_on_the_running_desk() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;
    let socket = listen(&daemon);
    follow(&daemon, Some(5));
    follow_all_the_way(&daemon, 65_535);
    socket.deliver(from(), &encode_data(0, 0, &[(5, DOWNSTAGE)]));
    until("the head to tilt towards the performer", || {
        byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(TILT_45) <= 1)
    })
    .await;

    daemon
        .desk()
        .core()
        .apply(&Command::PlaceFixtures {
            placements: vec![FixturePlace {
                id: FixtureId::new(1),
                position: Vec3 {
                    x: 0.0,
                    y: 6.0,
                    z: 0.0,
                },
                rotation: Vec3::ZERO,
                follow: Some(FollowTarget {
                    tracker: 5,
                    offset: Vec3::ZERO,
                }),
                mirror: prism_domain::Mirror {
                    pan: false,
                    tilt: true,
                },
            }],
        })
        .unwrap();
    // 255 - 170: the mirror of the value, because the travel is symmetric.
    until("the head to tilt the other way", || {
        byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(255 - TILT_45) <= 1)
    })
    .await;
    assert_eq!(byte(&frames, 2), Some(128), "pan was not mirrored");
    daemon.shutdown().await;
}

/// **The vocabulary**: `fixture 1 follow at 100` is a line the desk reads, and
/// puts the head on its tracker - the same value the Position bank's fourth knob
/// writes.
#[tokio::test]
async fn the_command_line_puts_a_head_on_its_tracker() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;
    let socket = listen(&daemon);
    follow(&daemon, Some(5));
    socket.deliver(from(), &encode_data(0, 0, &[(5, DOWNSTAGE)]));
    until("the position to arrive", || {
        daemon.desk().core().tracking().table().read(5).is_some()
    })
    .await;
    daemon
        .desk()
        .core()
        .apply(&Command::CommandLineInput {
            text: "fixture 1 follow at 100".to_owned(),
            run: true,
            mode: None,
        })
        .unwrap();
    until("the head to be put on the performer", || {
        byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(TILT_45) <= 1)
    })
    .await;
    daemon
        .desk()
        .core()
        .apply(&Command::CommandLineInput {
            text: "fixture 1 follow at 0".to_owned(),
            run: true,
            mode: None,
        })
        .unwrap();
    until("the head to be handed back to the cues", || {
        byte(&frames, 3) == Some(128)
    })
    .await;
    daemon.shutdown().await;
}

#[tokio::test]
async fn follow_at_nought_leaves_the_head_to_the_cues() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;
    let socket = listen(&daemon);
    follow(&daemon, Some(5));
    socket.deliver(from(), &encode_data(0, 0, &[(5, DOWNSTAGE)]));
    until("the position to arrive", || {
        daemon.desk().core().tracking().table().read(5).is_some()
    })
    .await;
    // Nothing says follow, so the tracker is heard and the head does not move.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(byte(&frames, 3), Some(128));
    daemon.shutdown().await;
}

#[tokio::test]
async fn the_programmer_beats_the_follow_and_clear_hands_the_head_back() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;
    let socket = listen(&daemon);
    follow(&daemon, Some(5));
    follow_all_the_way(&daemon, 65_535);
    socket.deliver(from(), &encode_data(0, 0, &[(5, DOWNSTAGE)]));
    until("the head on the tracker", || {
        byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(TILT_45) <= 1)
    })
    .await;

    // The operator takes the tilt: what they hold is theirs, whatever follows.
    daemon
        .desk()
        .core()
        .apply(&Command::SetAttribute {
            attribute: AttributeType::Tilt,
            occurrence: 0,
            value: 0,
            relative: false,
        })
        .unwrap();
    until("the operator's tilt", || byte(&frames, 3) == Some(0)).await;
    assert!(
        byte(&frames, 2).is_some(),
        "and the pan, which they did not touch, is still the head's own"
    );
    daemon.shutdown().await;
}

#[tokio::test]
async fn a_tracker_that_goes_quiet_leaves_the_head_where_it_was_and_the_operator_is_told_once() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;
    let socket = listen_with(&daemon, &[TrackerChange::Timeout { milliseconds: 100 }]);
    follow(&daemon, Some(5));
    follow_all_the_way(&daemon, 65_535);
    socket.deliver(from(), &encode_info(0, 0, "Sim", &[(5, "Anna")]));
    socket.deliver(from(), &encode_data(0, 0, &[(5, DOWNSTAGE)]));
    until("the head on the tracker", || {
        byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(TILT_45) <= 1)
    })
    .await;
    assert!(
        daemon.desk().poll_trackers().is_empty(),
        "a tracker that is being heard is not a warning"
    );

    // Silence: the head holds, bit for bit, and the desk says so - once.
    let held = common::last_frame_of(&frames, UniverseId::new(1)).unwrap();
    tokio::time::sleep(Duration::from_millis(350)).await;
    let notices = daemon.desk().poll_trackers();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(
        format!("{notices:?}").contains("Anna"),
        "it names the tracker: {notices:?}"
    );
    assert!(
        daemon.desk().poll_trackers().is_empty(),
        "once, not every poll"
    );
    let after = common::last_frame_of(&frames, UniverseId::new(1)).unwrap();
    assert_eq!(
        &after[..3],
        &held[..3],
        "nothing jumps while the tracker is quiet"
    );

    // And back: the head moves to where the performer now is, and the operator
    // is told that too, once.
    socket.deliver(from(), &encode_data(0, 0, &[(5, [6.0, 0.0, 0.0])]));
    until("the tracker to be heard again", || {
        let answer = daemon.desk().query(&prism_domain::Query::Trackers);
        matches!(&answer, prism_domain::Answer::Trackers { trackers, .. }
            if trackers.iter().any(|t| t.id == 5 && t.health == prism_domain::TrackerHealth::Live))
    })
    .await;
    let notices = daemon.desk().poll_trackers();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(daemon.desk().poll_trackers().is_empty());
    daemon.shutdown().await;
}

/// **Giving a head a tracker is not a rebuild.** A show is running; the operator
/// assigns a tracker; the cue list carries on, because only the follow layer was
/// handed to the tick and the body was not.
#[tokio::test]
async fn assigning_a_tracker_does_not_stop_a_running_cue_list() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;
    let _socket = listen(&daemon);

    // A programmer value on the dimmer stands in for a running look: it is state
    // a body rebuild would also keep, so what is measured is the swap counter.
    let mut core = daemon.desk().core();
    core.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(2)],
        mode: SelectionMode::Set,
    })
    .unwrap();
    core.apply(&Command::SetAttribute {
        attribute: AttributeType::Dimmer,
        occurrence: 0,
        value: 65_535,
        relative: false,
    })
    .unwrap();
    drop(core);
    until("the look to be lit", || byte(&frames, 10) == Some(255)).await;

    let swaps = daemon.desk().core().engine().health().swaps();
    follow(&daemon, Some(5));
    follow(&daemon, Some(6));
    follow(&daemon, None);
    until("the layer to be taken", || {
        !daemon.desk().core().engine().follow_pending()
    })
    .await;
    assert_eq!(
        daemon.desk().core().engine().health().swaps(),
        swaps,
        "no body was handed over: nothing about the patch changed"
    );
    assert_eq!(byte(&frames, 10), Some(255), "and the look is still lit");
    daemon.shutdown().await;
}

#[tokio::test]
async fn oops_takes_a_tracker_back_and_the_head_goes_back_to_the_cues() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;
    let socket = listen(&daemon);
    follow_all_the_way(&daemon, 65_535);
    socket.deliver(from(), &encode_data(0, 0, &[(5, DOWNSTAGE)]));
    until("the position to arrive", || {
        daemon.desk().core().tracking().table().read(5).is_some()
    })
    .await;
    // Not following anything yet: the head is at home.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(byte(&frames, 3), Some(128));

    follow(&daemon, Some(5));
    until("the head on the tracker", || {
        byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(TILT_45) <= 1)
    })
    .await;

    daemon.desk().core().apply(&Command::Oops).unwrap();
    until("the head back at home", || byte(&frames, 3) == Some(128)).await;
    assert!(
        daemon
            .desk()
            .core()
            .file
            .show
            .fixture(FixtureId::new(1))
            .unwrap()
            .follow
            .is_none()
    );
    daemon.shutdown().await;
}

#[tokio::test]
async fn the_panel_is_answered_with_what_is_out_there_and_how_many_heads_follow_it() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, _frames) = start(dir.path()).await;
    let socket = listen(&daemon);
    follow(&daemon, Some(5));
    socket.deliver(
        from(),
        &encode_info(0, 0, "Sim", &[(5, "Anna"), (7, "Ben")]),
    );
    socket.deliver(from(), &encode_data(0, 0, &[(5, [1.0, 2.0, 3.0])]));
    until("tracker 5 to be heard", || {
        matches!(
            daemon.desk().query(&prism_domain::Query::Trackers),
            prism_domain::Answer::Trackers { trackers, .. }
                if trackers.iter().any(|t| t.id == 5 && t.age_ms < u64::MAX)
        )
    })
    .await;
    let prism_domain::Answer::Trackers {
        trackers,
        listening,
        error,
        rejected,
        datagrams,
        from,
        ..
    } = daemon.desk().query(&prism_domain::Query::Trackers)
    else {
        panic!("a question about trackers was answered with something else")
    };
    assert!(listening, "{error:?}");
    assert_eq!(rejected, 0);
    assert_eq!(datagrams, 2, "the info packet and the data packet");
    assert_eq!(
        from.as_deref(),
        Some("10.0.0.9:56565"),
        "the sender is named"
    );
    let anna = trackers.iter().find(|t| t.id == 5).unwrap();
    assert_eq!(anna.name.as_deref(), Some("Anna"));
    assert_eq!(anna.followers, 1);
    // In show space: the default mapping swaps y and z.
    assert_eq!(
        anna.position,
        Vec3 {
            x: 1.0,
            y: 3.0,
            z: 2.0
        }
    );
    let ben = trackers.iter().find(|t| t.id == 7).unwrap();
    assert_eq!(
        (ben.followers, ben.age_ms),
        (0, u64::MAX),
        "named, never heard"
    );
    daemon.shutdown().await;
}

/// A tracker some head follows and the receiver has never heard is the row an
/// installer is looking for.
#[tokio::test]
async fn a_followed_tracker_nobody_has_heard_is_in_the_list_all_the_same() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, _frames) = start(dir.path()).await;
    let _socket = listen(&daemon);
    follow(&daemon, Some(9));
    let prism_domain::Answer::Trackers { trackers, .. } =
        daemon.desk().query(&prism_domain::Query::Trackers)
    else {
        panic!("a question about trackers was answered with something else")
    };
    let nine = trackers.iter().find(|t| t.id == 9).unwrap();
    assert_eq!(nine.followers, 1);
    assert_eq!(nine.health, prism_domain::TrackerHealth::Quiet);
    daemon.shutdown().await;
}

/// A setting that is not a setting is refused whole, and the receiver is not
/// disturbed by it.
#[tokio::test]
async fn a_tracker_setting_that_is_wrong_is_refused_and_nothing_changes() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let daemon = Daemon::start(&options(dir.path())).await.unwrap();
    for change in [
        TrackerChange::Group {
            group: "not-an-address".to_owned(),
        },
        TrackerChange::Interface {
            address: Some("wifi".to_owned()),
        },
        TrackerChange::Port { port: 0 },
        TrackerChange::Scale { scale: 0.0 },
        TrackerChange::Scale { scale: -1.0 },
        TrackerChange::Offset {
            offset: Vec3 {
                x: 0.0,
                y: 99_999.0,
                z: 0.0,
            },
        },
    ] {
        let refused = daemon.desk().core().apply(&Command::ConfigureMachine {
            change: MachineChange::Tracker {
                change: change.clone(),
            },
        });
        assert!(refused.is_err(), "{change:?} should have been refused");
    }
    let settings = daemon.desk().core().machine_settings().trackers;
    assert_eq!(settings, prism_domain::TrackerSettings::default());
    // And the one that is clamped rather than refused.
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::Tracker {
                change: TrackerChange::Timeout { milliseconds: 1 },
            },
        })
        .unwrap();
    assert_eq!(
        daemon.desk().core().machine_settings().trackers.timeout_ms,
        100,
        "a warning threshold has a nearest sensible value"
    );
    daemon.shutdown().await;
}

/// **A cue stores following, like a value.** Cue 1 says *this head is on its
/// tracker from here* (`Follow` at full); cue 2 gives it a fixed pan and tilt
/// and says nothing about following - which is how a head is let go. The head goes where the performer is, and then where the
/// cue puts it - and never anywhere in between that nobody asked for.
#[tokio::test]
async fn a_cue_turns_following_on_and_a_cue_that_stores_a_place_lets_the_head_go() {
    use prism_domain::{
        Cue, CuePart, CueTracking, CueTrigger, Executor, ExecutorEncoderFunction,
        ExecutorFaderFunction, ExecutorId, GoDirection, PlaybackTarget, Sequence, SequenceId,
    };

    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aula.prism");
    {
        let mut file = ShowFile::new();
        file.show.embed_fixture_type(head_type()).unwrap();
        let mut head = common::fixture(1, "test.head", 1, 1);
        head.position = Vec3 {
            x: 0.0,
            y: 6.0,
            z: 0.0,
        };
        head.follow = Some(FollowTarget {
            tracker: 5,
            offset: Vec3::ZERO,
        });
        file.show.patch_fixture(head).unwrap();
        let part = |attribute: AttributeType, value: u16| CuePart {
            fixture: FixtureId::new(1),
            attribute,
            occurrence: 0,
            value,
            preset_ref: None,
            tracking: CueTracking::Track,
        };
        let cue = |number: &str, parts: Vec<CuePart>| Cue {
            number: number.to_owned(),
            name: format!("Cue {number}"),
            fade_in: 0.0,
            fade_out: 0.0,
            delay: 0.0,
            trigger: CueTrigger::Go,
            trigger_time: None,
            parts,
        };
        file.show
            .store_sequence(Sequence {
                id: SequenceId::new(1),
                name: "Follow spot".to_owned(),
                color: None,
                cues: vec![
                    cue("1", vec![part(AttributeType::Follow, u16::MAX)]),
                    // **No `Follow` part** - cue 2 says only where the head goes,
                    // and storing a position lets a following head go
                    // (`prism_domain::CueTrack`).
                    cue(
                        "2",
                        vec![
                            part(AttributeType::Pan, 49_152),
                            part(AttributeType::Tilt, 16_384),
                        ],
                    ),
                ],
                looping: false,
                master_level: u16::MAX,
                speed: prism_domain::SPEED_UNITY,
                is_active: false,
                current_cue_index: None,
                crossfade_position: 0,
            })
            .unwrap();
        file.show
            .store_executor(Executor {
                id: ExecutorId::new(0),
                sequence_id: Some(SequenceId::new(1)),
                fader_function: ExecutorFaderFunction::Master,
                button_functions: Vec::new(),
                encoder_function: ExecutorEncoderFunction::Empty,
                encoder_executor: None,
            })
            .unwrap();
        let mut store = ShowStore::open(&path).unwrap();
        store.save(&mut file).unwrap();
    }
    let (daemon, frames) = start(dir.path()).await;
    let socket = listen(&daemon);
    socket.deliver(from(), &encode_data(0, 0, &[(5, DOWNSTAGE)]));
    until("the position to arrive", || {
        daemon.desk().core().tracking().table().read(5).is_some()
    })
    .await;
    // No cue yet: the head is at home, whatever the tracker says.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(byte(&frames, 3), Some(128));

    let go = || Command::ExecutorGo {
        target: PlaybackTarget::of_executor(ExecutorId::new(0)),
        direction: GoDirection::Next,
    };
    daemon.desk().core().apply(&go()).unwrap();
    until("cue 1 to put the head on the performer", || {
        byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(TILT_45) <= 1)
    })
    .await;

    // Cue 2: following off, and a fixed place. Pan 49152 is 0.75 of +-270 (135
    // degrees round); tilt 16384 is a quarter of +-135 (-67.5 degrees).
    daemon.desk().core().apply(&go()).unwrap();
    until("cue 2 to put the head where it says", || {
        byte(&frames, 2) == Some(192) && byte(&frames, 3) == Some(64)
    })
    .await;
    // And the tracker moving now changes nothing: this head is not following.
    socket.deliver(from(), &encode_data(0, 0, &[(5, [6.0, 0.0, 0.0])]));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!((byte(&frames, 2), byte(&frames, 3)), (Some(192), Some(64)));
    daemon.shutdown().await;
}

/// **Sources and mappings survive a restart** - S32's third exit criterion. The
/// daemon here writes its machine configuration to its data directory (it is
/// given no rig on the command line, which is what holds a daemon back from
/// doing so), is stopped, and a second one reads it back.
#[tokio::test]
async fn the_tracker_settings_survive_a_restart() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let mut persistent = options(dir.path());
    persistent.outputs = Vec::new();
    persistent.mock_devices = true;

    let wanted = {
        let daemon = Daemon::start(&persistent).await.unwrap();
        for change in [
            TrackerChange::Enabled { enabled: true },
            TrackerChange::Group {
                group: "236.10.10.77".to_owned(),
            },
            TrackerChange::Port { port: 6_000 },
            TrackerChange::Interface {
                address: Some("10.1.2.3".to_owned()),
            },
            TrackerChange::Axis {
                axis: prism_domain::ShowAxis::X,
                from: prism_domain::SourceAxis::Y,
                invert: true,
            },
            TrackerChange::Scale { scale: 0.001 },
            TrackerChange::Offset {
                offset: Vec3 {
                    x: 2.0,
                    y: 0.0,
                    z: -1.0,
                },
            },
            TrackerChange::Timeout { milliseconds: 900 },
        ] {
            daemon
                .desk()
                .core()
                .apply(&Command::ConfigureMachine {
                    change: MachineChange::Tracker { change },
                })
                .unwrap();
        }
        let wanted = daemon.desk().core().machine_settings().trackers;
        daemon.shutdown().await;
        wanted
    };
    assert!(wanted.enabled);
    assert_eq!(wanted.group, "236.10.10.77");

    let daemon = Daemon::start(&persistent).await.unwrap();
    assert_eq!(daemon.desk().core().machine_settings().trackers, wanted);
    daemon.shutdown().await;
}
