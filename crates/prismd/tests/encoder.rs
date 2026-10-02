//! **An encoder that does something** — S60, and the fault the owner met on the
//! rig on 2026-09-20: *with the fader in crossfade mode and the encoders set to
//! Master, the encoders did nothing.*
//!
//! # What this file asserts, and against what
//!
//! **The byte that reached a mock output**, not the model. A claim about
//! `Sequence::master_level` is a claim about a number the core holds; the claim
//! the owner made is about a light, and a light is what the frame says.
//!
//! The encoder is turned the way a hand turns it: a V-Pot message, CC 16 + the
//! strip, **written out here by hand from `docs/MCU_MAPPING.md` §2.1** — sign in
//! bit 6, the detent count in bits 0–5. Asking the profile what to send would be
//! asking the code under test what to press (S19's finding, S20's method rule).
//!
//! # Nothing here touches a device
//!
//! `CLAUDE.md`'s rule. The surface is a [`MockSurfacePort`] and the output is
//! `--mock-output`.

// Every test here holds `common::one_daemon_at_a_time` across its awaits — a
// daemon owns a tick thread at real-time priority, and two of those in one
// process measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::ShowStore;
use prism_domain::{
    AttributeType, Command, Cue, CuePart, CueTracking, CueTrigger, Executor,
    ExecutorButtonFunction, ExecutorEncoderFunction, ExecutorFaderFunction, ExecutorId, FixtureId,
    OutputId, SPEED_UNITY, Sequence, SequenceId, UniverseId,
};
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;
use prismd::server::Desk;
use prismd::surface::MockSurfacePort;

mod common;

/// The status byte of a Control Change on MIDI channel 1.
const CONTROL_CHANGE: u8 = 0xB0;

/// The V-Pot of strip 0, `docs/MCU_MAPPING.md` §2.1: CC 16 to 23.
const VPOT_STRIP_0: u8 = 16;

/// Eight detents anticlockwise in one message: bit 6 the sign, bits 0–5 the
/// count. §2.7 measured 1…8 as what a fast turn reports.
const EIGHT_DOWN: u8 = 0x40 | 8;

/// **What eight detents are worth, written out by hand.** The V-Pot curve is the
/// triangular numbers in units of one coarse DMX step (`prism_surface::accel`):
/// the eighth entry is 36 of them, and a coarse step is 257 of the 65 535 a
/// master is made of. 65 535 − 36 × 257 = 56 283, which is 219 × 257, so the
/// frame carries **219** and there is no rounding in the claim.
const AFTER_EIGHT_DOWN: u8 = 219;

fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("encoder.prism")),
        universes: Some(1),
        outputs: vec![mock_output(1)],
        local: Some(false),
        websocket: prismd::cli::Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
        ..Options::default()
    }
}

/// A one-cue list on one dimmer, and an executor with a fader function and an
/// encoder function of its own.
///
/// The cue snaps and is full, so the list is *at* 255 the moment it is started
/// and every byte that differs from it afterwards was put there by a hand.
fn show_with(
    fader: ExecutorFaderFunction,
    encoder: ExecutorEncoderFunction,
) -> prism_core::ShowFile {
    let mut file = prism_core::ShowFile::default();
    file.show
        .embed_fixture_type(common::dimmer_type("generic.dimmer", 0))
        .unwrap();
    file.show
        .patch_fixture(common::fixture(1, "generic.dimmer", 1, 1))
        .unwrap();
    file.show
        .store_sequence(Sequence {
            id: SequenceId::new(1),
            name: "Walk".to_owned(),
            color: None,
            cues: vec![Cue {
                number: "1".to_owned(),
                name: String::new(),
                fade_in: 0.0,
                fade_out: 0.0,
                delay: 0.0,
                trigger: CueTrigger::Go,
                trigger_time: None,
                parts: vec![CuePart {
                    fixture: FixtureId::new(1),
                    attribute: AttributeType::Dimmer,
                    occurrence: 0,
                    value: u16::MAX,
                    preset_ref: None,
                    tracking: CueTracking::Track,
                }],
            }],
            looping: false,
            master_level: u16::MAX,
            speed: SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
            crossfade_position: 0,
        })
        .unwrap();
    file.show
        .store_executor(Executor {
            id: ExecutorId::new(0),
            sequence_id: Some(SequenceId::new(1)),
            fader_function: fader,
            button_functions: vec![
                ExecutorButtonFunction::On,
                ExecutorButtonFunction::Off,
                ExecutorButtonFunction::GoForward,
                ExecutorButtonFunction::Empty,
            ],
            encoder_function: encoder,
            encoder_executor: None,
        })
        .unwrap();
    file.show.mark_saved();
    file
}

/// Channel 1 of the last frame the mock output was given.
fn channel_one(frames: &prism_protocols::MockOutputHandle) -> Option<u8> {
    common::last_frame_of(frames, UniverseId::new(1)).and_then(|data| data.first().copied())
}

/// Runs the daemon in slices until `condition` holds, or fails by name.
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

/// Starts the list and waits for the light — the state every test begins from.
async fn started(daemon: &mut Daemon, desk: &Desk, frames: &prism_protocols::MockOutputHandle) {
    desk.command(Command::ExecutorButton {
        executor_id: ExecutorId::new(0),
        button: prism_domain::ExecutorButtonRef::Slot { index: 0 },
        pressed: true,
    });
    run_until(daemon, "the first cue", || channel_one(frames) == Some(255)).await;
}

/// **The fault, on the wire** — and the same turn with the fader on `Master`,
/// because a test that only reproduced the crossfade would say nothing about
/// whether the crossfade was the cause.
///
/// An encoder set to `Master` turns the master of its list **whatever its fader
/// is doing**: the fader's own function belongs to the fader. Before S60 this
/// failed for every one of the four, and it failed the same way — the turn left
/// the daemon as a message and arrived nowhere, because nothing read an
/// executor's `encoder_function` at all.
#[tokio::test]
async fn an_encoder_set_to_master_turns_the_master_whatever_the_fader_does() {
    let _turn = common::one_daemon_at_a_time();
    for fader in [
        ExecutorFaderFunction::Master,
        ExecutorFaderFunction::XFade,
        ExecutorFaderFunction::Fade,
        ExecutorFaderFunction::Speed,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut file = show_with(fader, ExecutorEncoderFunction::Master);
        ShowStore::open(dir.path().join("encoder.prism"))
            .unwrap()
            .save(&mut file)
            .unwrap();

        let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
        let frames = daemon
            .recorded_output(OutputId::new(1))
            .expect("a mock output");
        let desk = daemon.desk().clone();
        let (port, surface) = MockSurfacePort::new();
        daemon.attach_surface(Box::new(port));
        desk.command(Command::SetExecutorPage { page: 0 });
        started(&mut daemon, &desk, &frames).await;

        surface.send(&[CONTROL_CHANGE, VPOT_STRIP_0, EIGHT_DOWN]);
        run_until(&mut daemon, "the encoder to move the light", || {
            channel_one(&frames) != Some(255)
        })
        .await;
        assert_eq!(
            channel_one(&frames),
            Some(AFTER_EIGHT_DOWN),
            "{fader:?}: eight detents down on a Master encoder did not put 219 on the wire"
        );
        daemon.shutdown().await;
    }
}

/// Channel `n` (from one) of the last frame the mock output was given.
fn channel(frames: &prism_protocols::MockOutputHandle, n: usize) -> Option<u8> {
    common::last_frame_of(frames, UniverseId::new(1)).and_then(|data| data.get(n - 1).copied())
}

/// Runs the daemon until the frame has **stopped moving** — S46's rule for a
/// test that reads a number the daemon owns: it waits until the number stops,
/// and does not count slices and hope.
async fn settled(daemon: &mut Daemon, frames: &prism_protocols::MockOutputHandle) {
    let mut last = common::last_frame_of(frames, UniverseId::new(1));
    let mut still = 0;
    for slice in 0..400 {
        daemon
            .run(Some(Duration::from_millis(5)), std::future::pending())
            .await;
        let now = common::last_frame_of(frames, UniverseId::new(1));
        if now == last {
            still += 1;
        } else {
            still = 0;
            last = now;
        }
        if slice >= 20 && still >= 10 {
            return;
        }
    }
}

/// Two one-cue lists on two dimmers — channel 1 and channel 2 — each on an
/// executor of its own. **Executor 0's encoder is `Master`, and is on
/// executor 1**; executor 1's own is `Empty`.
fn two_lists() -> prism_core::ShowFile {
    let mut file = show_with(
        ExecutorFaderFunction::Master,
        ExecutorEncoderFunction::Master,
    );
    file.show
        .patch_fixture(common::fixture(2, "generic.dimmer", 1, 2))
        .unwrap();
    let mut second = file.show.sequence(SequenceId::new(1)).unwrap().clone();
    second.id = SequenceId::new(2);
    second.name = "Other".to_owned();
    second.cues[0].parts[0].fixture = FixtureId::new(2);
    file.show.store_sequence(second).unwrap();
    file.show
        .store_executor(Executor {
            id: ExecutorId::new(1),
            sequence_id: Some(SequenceId::new(2)),
            fader_function: ExecutorFaderFunction::Master,
            button_functions: vec![ExecutorButtonFunction::On],
            encoder_function: ExecutorEncoderFunction::Empty,
            encoder_executor: None,
        })
        .unwrap();
    file.show
        .configure_executor(
            ExecutorId::new(0),
            &prism_domain::ExecutorChange::EncoderExecutor {
                executor_id: Some(ExecutorId::new(1)),
            },
        )
        .unwrap();
    file.show.mark_saved();
    file
}

/// **An encoder on another executor's list, on the wire** — S60's exit
/// criterion in its own words: the byte that reached the mock output.
///
/// Strip 0's encoder turns the list on strip 1: channel 2 comes down to 219 and
/// channel 1, which is the list on strip 0 itself, does not move. Strip 1's own
/// encoder is `Empty` and turning it does nothing — *whose function* and *whose
/// list* are two answers. And taking the encoder back to its own executor
/// moves channel 1 instead.
#[tokio::test]
async fn an_encoder_turns_the_list_of_another_executor_and_the_byte_shows_it() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let mut file = two_lists();
    ShowStore::open(dir.path().join("encoder.prism"))
        .unwrap()
        .save(&mut file)
        .unwrap();

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let frames = daemon
        .recorded_output(OutputId::new(1))
        .expect("a mock output");
    let desk = daemon.desk().clone();
    let (port, surface) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    desk.command(Command::SetExecutorPage { page: 0 });
    for executor_id in [ExecutorId::new(0), ExecutorId::new(1)] {
        desk.command(Command::ExecutorButton {
            executor_id,
            button: prism_domain::ExecutorButtonRef::Slot { index: 0 },
            pressed: true,
        });
    }
    run_until(&mut daemon, "both lists", || {
        channel(&frames, 1) == Some(255) && channel(&frames, 2) == Some(255)
    })
    .await;

    // Strip 0's encoder: the list on strip 1 comes down, strip 0's does not.
    surface.send(&[CONTROL_CHANGE, VPOT_STRIP_0, EIGHT_DOWN]);
    run_until(&mut daemon, "the other list to move", || {
        channel(&frames, 2) != Some(255)
    })
    .await;
    settled(&mut daemon, &frames).await;
    assert_eq!(channel(&frames, 2), Some(AFTER_EIGHT_DOWN));
    assert_eq!(
        channel(&frames, 1),
        Some(255),
        "the encoder moved the list on its own executor"
    );

    // Strip 1's own encoder is `Empty`: nothing.
    surface.send(&[CONTROL_CHANGE, VPOT_STRIP_0 + 1, EIGHT_DOWN]);
    settled(&mut daemon, &frames).await;
    assert_eq!(channel(&frames, 2), Some(AFTER_EIGHT_DOWN));
    assert_eq!(channel(&frames, 1), Some(255));

    // Back to its own executor: the next turn is channel 1's.
    desk.command(Command::ConfigureExecutor {
        executor_id: ExecutorId::new(0),
        change: prism_domain::ExecutorChange::EncoderExecutor { executor_id: None },
    });
    surface.send(&[CONTROL_CHANGE, VPOT_STRIP_0, EIGHT_DOWN]);
    run_until(&mut daemon, "its own list to move", || {
        channel(&frames, 1) != Some(255)
    })
    .await;
    settled(&mut daemon, &frames).await;
    assert_eq!(channel(&frames, 1), Some(AFTER_EIGHT_DOWN));
    assert_eq!(channel(&frames, 2), Some(AFTER_EIGHT_DOWN));
    daemon.shutdown().await;
}
