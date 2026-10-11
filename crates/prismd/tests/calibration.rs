//! Calibrating a head, asked of a running daemon — **B70**.
//!
//! The solver is pure (`prism-domain`) and the reading of a head's two axes is
//! the show's (`prism-core/tests/calibration.rs`). What only a daemon can say is
//! whether **the three readings of one axis are one reading**: the programmer
//! value the calibration asks the operator to drive a head to, the byte the
//! follow layer puts on the cable for a tracker standing on the same mark, and
//! the angle the viewer draws. If the first two disagreed, calibrating a head
//! and then following a tracker with it would aim it somewhere else.

// Every test holds `common::one_daemon_at_a_time` across its awaits, for
// `wiring.rs`'s reason.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::{ShowFile, ShowStore};
use prism_domain::{
    Answer, AttributeDef, AttributeType, CalibrationAnswer, CalibrationSighting, Command,
    FixtureId, FixturePlace, FixtureType, Mirror, Query, SelectionMode, UniverseId, Vec3,
};
use prismd::cli::Options;
use prismd::daemon::Daemon;

mod common;

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

fn write_show(path: &Path) {
    let mut file = ShowFile::new();
    file.show.embed_fixture_type(head_type()).unwrap();
    let mut head = common::fixture(1, "test.head", 1, 1);
    head.position = Vec3::new(0.0, 6.0, 0.0);
    file.show.patch_fixture(head).unwrap();
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

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

fn byte(frames: &prism_protocols::MockOutputHandle, channel: usize) -> Option<u8> {
    common::last_frame_of(frames, UniverseId::new(1))
        .and_then(|data| data.get(channel - 1).copied())
}

fn place(daemon: &Daemon, position: Vec3, rotation: Vec3, mirror: Mirror) {
    daemon
        .desk()
        .core()
        .apply(&Command::PlaceFixtures {
            placements: vec![FixturePlace {
                id: FixtureId::new(1),
                position,
                rotation,
                follow: None,
                mirror,
            }],
        })
        .unwrap();
}

fn drive(daemon: &Daemon, pan: u16, tilt: u16) {
    let mut core = daemon.desk().core();
    core.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(1)],
        mode: SelectionMode::Set,
    })
    .unwrap();
    for (attribute, value) in [(AttributeType::Pan, pan), (AttributeType::Tilt, tilt)] {
        core.apply(&Command::SetAttribute {
            attribute,
            occurrence: 0,
            value: i32::from(value),
            relative: false,
        })
        .unwrap();
    }
}

fn aim(daemon: &Daemon, target: Vec3, position: Vec3, rotation: Vec3) -> prism_domain::AimedAt {
    match daemon.desk().query(&Query::CalibrationAim {
        fixture: FixtureId::new(1),
        target,
        position,
        rotation,
        near: None,
    }) {
        Answer::Calibration {
            answer: CalibrationAnswer::Aimed { aim: Some(aimed) },
            ..
        } => aimed,
        other => panic!("not an aim: {other:?}"),
    }
}

/// The operator drives the head to where the desk says a mark is, and the cable
/// then carries what the desk's own aim put there: the value the daemon answers
/// with **is** the byte the tick writes, for a head hung tipped and turned, with
/// its tilt mirrored.
#[tokio::test]
async fn the_value_the_calibration_asks_for_is_the_byte_the_cable_carries() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, frames) = start(dir.path()).await;

    let position = Vec3::new(0.0, 6.0, 0.0);
    let rotation = Vec3::new(0.0, 30.0, 0.0);
    place(&daemon, position, rotation, Mirror::default());
    let aimed = aim(&daemon, Vec3::new(3.0, 0.0, 4.0), position, rotation);
    assert!(aimed.reached);
    drive(&daemon, aimed.pan, aimed.tilt);
    // An 8-bit channel carries the sixteen-bit value divided by 257.
    let narrow = |value: u16| u8::try_from((u32::from(value) + 128) / 257).unwrap();
    until("the head to be driven", || {
        byte(&frames, 2).is_some_and(|pan| pan.abs_diff(narrow(aimed.pan)) <= 1)
            && byte(&frames, 3).is_some_and(|tilt| tilt.abs_diff(narrow(aimed.tilt)) <= 1)
    })
    .await;
    daemon.shutdown().await;
}

/// The whole round trip through the daemon, with the mirror on: a head hung
/// somewhere other than the show says, five sightings through
/// `Query::CalibrationSolve`, and the pose that comes back is the real one.
#[tokio::test]
async fn a_solve_asked_of_the_daemon_finds_the_pose_the_sightings_were_taken_from() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    write_show(&dir.path().join("aula.prism"));
    let (daemon, _frames) = start(dir.path()).await;

    // The head really hangs here; the show says otherwise.
    let real = (Vec3::new(1.5, 6.5, -1.0), Vec3::new(4.0, 25.0, -3.0));
    place(
        &daemon,
        Vec3::new(1.0, 6.0, 0.0),
        Vec3::ZERO,
        Mirror {
            pan: false,
            tilt: true,
        },
    );
    let marks = [
        Vec3::new(-2.0, 0.0, 3.0),
        Vec3::new(3.0, 0.0, 2.0),
        Vec3::new(0.0, 0.0, -3.0),
        Vec3::new(4.0, 0.0, -2.0),
        Vec3::new(-3.0, 0.0, -1.0),
    ];
    let sightings: Vec<CalibrationSighting> = marks
        .iter()
        .map(|&target| {
            // The operator drives the head; the daemon is asked where the head
            // *really* would have to be driven, from the real pose.
            let aimed = aim(&daemon, target, real.0, real.1);
            CalibrationSighting {
                target,
                pan: aimed.pan,
                tilt: aimed.tilt,
            }
        })
        .collect();

    let answer = daemon.desk().query(&Query::CalibrationSolve {
        fixture: FixtureId::new(1),
        position: Vec3::new(1.0, 6.0, 0.0),
        rotation: Vec3::ZERO,
        hold_position: false,
        sightings,
    });
    let Answer::Calibration {
        answer:
            CalibrationAnswer::Solved {
                position,
                rms,
                fits,
                spare,
                ..
            },
        ..
    } = answer
    else {
        panic!("not solved: {answer:?}");
    };
    // 16 bits of a 540-degree pan is 0.008 degrees a step.
    assert!(rms < 0.05, "rms {rms}");
    assert!(
        (position.x - real.0.x).abs() < 0.15
            && (position.y - real.0.y).abs() < 0.15
            && (position.z - real.0.z).abs() < 0.15,
        "{position:?}"
    );
    assert_eq!(fits.len(), 5);
    assert_eq!(spare, 4);
    assert!(fits.iter().all(|fit| fit.distance > 3.0));

    // And it changed nothing: the head is still where the show had it.
    assert_eq!(
        daemon
            .desk()
            .core()
            .file
            .show
            .fixture(FixtureId::new(1))
            .unwrap()
            .position,
        Vec3::new(1.0, 6.0, 0.0)
    );
    daemon.shutdown().await;
}
