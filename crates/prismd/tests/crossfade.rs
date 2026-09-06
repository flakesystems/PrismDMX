//! **The crossfade an operator can walk a cue list with** — punch-list entry
//! **B36**, asserted where the entry lives: on the frames and on the wire.
//!
//! # Why here and not in the engine
//!
//! `prism_engine::player` has the two modes as unit tests, and those are the
//! ones that say *what a stroke is*. Two of B36's exit criteria cannot be
//! stated there:
//!
//! - **A recorded fader walk produces a byte-identical frame sequence twice.**
//!   A frame is a `prismd` output's, built by the tick out of the merge and the
//!   encoder; a claim about *values* is not a claim about bytes.
//! - **No path in either mode ever writes a fader position back to the
//!   surface.** That is a claim about MIDI leaving the daemon, and it is the
//!   half the entry is actually about: *In keinem Fall soll der Fader nach einer
//!   Bewegung irgendwie zurück bewegt werden.*
//!
//! # Nothing here touches a device
//!
//! `CLAUDE.md`'s rule. The surface is a [`MockSurfacePort`] and the output is
//! `--mock-output`; the bytes are read against `docs/MCU_MAPPING.md` by hand,
//! which is S19's finding and S20's method rule.

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
use prismd::surface::{MockSurfaceHandle, MockSurfacePort};

mod common;

/// The status byte of a pitch-bend message on channel `n` — `docs/MCU_MAPPING.md`
/// §2.2, and the only thing that moves a motor fader.
const PITCH_BEND: u8 = 0xE0;

fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("walk.prism")),
        universes: Some(1),
        outputs: vec![mock_output(1)],
        local: Some(false),
        websocket: prismd::cli::Listen::Off,
        log_level: Some(prismd::log::Level::Warn),
        ..Options::default()
    }
}

/// A three-cue list on one dimmer, with the fader of executor 0 on it.
///
/// Ten-second fades throughout, so **time could not have produced any of the
/// numbers below**: whatever the frames show, the fader put it there.
fn show_with(fader: ExecutorFaderFunction) -> prism_core::ShowFile {
    let mut file = prism_core::ShowFile::default();
    file.show
        .embed_fixture_type(common::dimmer_type("generic.dimmer", 0))
        .unwrap();
    file.show
        .patch_fixture(common::fixture(1, "generic.dimmer", 1, 1))
        .unwrap();
    let cue = |number: &str, value: u16, fade: f64| Cue {
        number: number.to_owned(),
        name: String::new(),
        fade_in: fade,
        fade_out: fade,
        delay: 0.0,
        trigger: CueTrigger::Go,
        trigger_time: None,
        parts: vec![CuePart {
            fixture: FixtureId::new(1),
            attribute: AttributeType::Dimmer,
            occurrence: 0,
            value,
            preset_ref: None,
            tracking: CueTracking::Track,
        }],
    };
    file.show
        .store_sequence(Sequence {
            id: SequenceId::new(1),
            name: "Walk".to_owned(),
            color: None,
            // Cue 1 snaps, so the list is *at* its first look the moment it is
            // started and a test does not have to wait out a fade to begin.
            // The two it walks to are ten seconds each, which is what makes the
            // numbers below impossible for the clock to have produced.
            cues: vec![
                cue("1", 65_535, 0.0),
                cue("2", 20_000, 10.0),
                cue("3", 50_000, 10.0),
            ],
            looping: false,
            master_level: u16::MAX,
            speed: SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
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
            encoder_function: ExecutorEncoderFunction::Empty,
        })
        .unwrap();
    file.show.mark_saved();
    file
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
    let seen = daemon
        .recorded_output(OutputId::new(1))
        .and_then(|handle| common::last_frame_of(&handle, UniverseId::new(1)))
        .map(|data| data[..8.min(data.len())].to_vec());
    panic!("timed out waiting for {what}; last frame starts {seen:?}");
}

/// Lets the daemon run for a fixed slice, whatever happens.
async fn settle(daemon: &mut Daemon) {
    for _ in 0..12 {
        daemon
            .run(Some(Duration::from_millis(5)), std::future::pending())
            .await;
    }
}

/// Runs the daemon until the frame has **stopped moving**, and returns it.
///
/// A fixed number of slices would be a race on a loaded two-core runner: a
/// command that had not yet reached the tick would leave the previous frame in
/// the buffer, and a test whose whole claim is *the same walk twice* would then
/// be comparing one stale read against one fresh one.
///
/// Two conditions, and both are needed. A **minimum** of slices, because the
/// frame is stable before the command lands as well as after it; and then
/// **stability**, because a fader movement is the only thing moving the light —
/// every cue in this list fades over ten seconds and none of them is on the
/// clock while a stroke is armed, so a settled frame is a settled playback.
async fn settled_frame(daemon: &mut Daemon, frames: &prism_protocols::MockOutputHandle) -> Vec<u8> {
    let mut last = frame(frames);
    let mut still = 0;
    for slice in 0..400 {
        daemon
            .run(Some(Duration::from_millis(5)), std::future::pending())
            .await;
        let now = frame(frames);
        if now == last {
            still += 1;
        } else {
            still = 0;
            last = now;
        }
        if slice >= 20 && still >= 10 {
            break;
        }
    }
    last.expect("the output has had a frame")
}

/// Channel 1 of the last frame the mock output was given.
fn channel_one(frames: &prism_protocols::MockOutputHandle) -> Option<u8> {
    frame(frames).and_then(|data| data.first().copied())
}

/// The whole of the last frame, which is what *byte-identical* is about.
fn frame(frames: &prism_protocols::MockOutputHandle) -> Option<Vec<u8>> {
    common::last_frame_of(frames, UniverseId::new(1))
}

/// The positions the desk has been told to put its motor faders at, in order.
///
/// Pitch bend and nothing else: it is the only message that moves a fader
/// (`docs/MCU_MAPPING.md` §2.2), which is what makes an empty list here mean
/// *nothing moved the operator's hand*.
fn fader_writes(surface: &MockSurfaceHandle) -> Vec<(u8, u16)> {
    surface
        .received()
        .iter()
        .filter(|message| message.len() == 3 && message[0] & 0xF0 == PITCH_BEND)
        .map(|message| {
            (
                message[0] & 0x0F,
                u16::from(message[1]) | (u16::from(message[2]) << 7),
            )
        })
        .collect()
}

/// Walks the fader through `positions` and records the frame after each.
async fn walk(
    daemon: &mut Daemon,
    frames: &prism_protocols::MockOutputHandle,
    desk: &Desk,
    positions: &[u16],
) -> Vec<Vec<u8>> {
    let mut seen = Vec::new();
    for position in positions {
        desk.command(Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: *position,
        });
        seen.push(settled_frame(daemon, frames).await);
    }
    seen
}

/// **A recorded fader walk produces the same frames twice** — B36's exit
/// criterion, on the bytes.
///
/// The same daemon, the same list, the same positions, driven twice from the
/// same starting state. Byte-identical, and identical for **both** modes:
/// where a fader is has to be the whole of what the output depends on, or the
/// crossfade is not something a show can be rehearsed with.
#[tokio::test]
async fn a_recorded_fader_walk_produces_the_same_frames_twice() {
    let _turn = common::one_daemon_at_a_time();
    for mode in [ExecutorFaderFunction::XFade, ExecutorFaderFunction::Fade] {
        let dir = tempfile::tempdir().unwrap();
        let mut file = show_with(mode);
        ShowStore::open(dir.path().join("walk.prism"))
            .unwrap()
            .save(&mut file)
            .unwrap();

        let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
        let frames = daemon
            .recorded_output(OutputId::new(1))
            .expect("a mock output");
        let desk = daemon.desk().clone();
        desk.command(Command::SetExecutorPage { page: 0 });

        // The list is started and the fader engaged at the bottom, which is the
        // state both walks begin from.
        let start = |desk: &Desk| {
            desk.command(Command::ExecutorButton {
                executor_id: ExecutorId::new(0),
                button: prism_domain::ExecutorButtonRef::Slot { index: 0 },
                pressed: true,
            });
        };
        start(&desk);
        run_until(&mut daemon, "the first cue", || {
            channel_one(&frames) == Some(255)
        })
        .await;
        desk.command(Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: 0,
        });
        settle(&mut daemon).await;

        let positions = [8_000_u16, 24_000, 48_000, 65_535, 40_000, 12_000, 0];
        let first = walk(&mut daemon, &frames, &desk, &positions).await;

        // Back to the top of the list and the bottom of the fader, and round
        // again. `Goto` rather than a reload, because what has to be the same
        // is the *playback*'s starting state.
        desk.command(Command::Goto {
            target: prism_domain::PlaybackTarget::Executor {
                executor_id: ExecutorId::new(0),
            },
            cue_number: "1".to_owned(),
        });
        settle(&mut daemon).await;
        desk.command(Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: 0,
        });
        settle(&mut daemon).await;
        let second = walk(&mut daemon, &frames, &desk, &positions).await;

        assert_eq!(
            first, second,
            "{mode:?}: the same walk put different bytes on the wire"
        );
        // And the walk moved something, or the claim above is about nothing.
        assert!(
            first.iter().any(|bytes| bytes != &first[0]),
            "{mode:?}: the walk produced one frame over and over"
        );
        daemon.shutdown().await;
    }
}

/// **A walk that stops half way outputs the mixture and holds it** — the state
/// *between two cues*, on the wire.
///
/// Ten-second fades, so the only thing that could move the frame is the clock —
/// and the whole point of the entry is that the clock is not driving this. Two
/// hundred milliseconds is nine ticks; a fade that was running would have moved.
#[tokio::test]
async fn a_walk_stopped_half_way_holds_the_mixture_on_the_wire() {
    let _turn = common::one_daemon_at_a_time();
    for mode in [ExecutorFaderFunction::XFade, ExecutorFaderFunction::Fade] {
        let dir = tempfile::tempdir().unwrap();
        let mut file = show_with(mode);
        ShowStore::open(dir.path().join("walk.prism"))
            .unwrap()
            .save(&mut file)
            .unwrap();

        let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
        let frames = daemon
            .recorded_output(OutputId::new(1))
            .expect("a mock output");
        let desk = daemon.desk().clone();
        desk.command(Command::ExecutorButton {
            executor_id: ExecutorId::new(0),
            button: prism_domain::ExecutorButtonRef::Slot { index: 0 },
            pressed: true,
        });
        run_until(&mut daemon, "the first cue", || {
            channel_one(&frames) == Some(255)
        })
        .await;

        desk.command(Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: 0,
        });
        settle(&mut daemon).await;
        desk.command(Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: 30_000,
        });
        run_until(&mut daemon, "the fader to move the light", || {
            channel_one(&frames) != Some(255)
        })
        .await;

        let held = frame(&frames).expect("a frame");
        assert_ne!(held.first().copied(), Some(255), "{mode:?}: nothing moved");
        for _ in 0..40 {
            settle(&mut daemon).await;
            assert_eq!(
                frame(&frames).as_deref(),
                Some(held.as_slice()),
                "{mode:?}: the half-way mixture did not hold"
            );
        }
        daemon.shutdown().await;
    }
}

/// **The desk never moves a crossfade fader** — B36's other half, asserted
/// against the MIDI the daemon sends.
///
/// This is the fault the entry reports: every repaint used to write the
/// crossfade's *reading* — nought — to the motor, so a fader an operator had
/// pushed up was driven back down a fraction of a second later. Pitch bend is
/// the only message that moves a fader (`docs/MCU_MAPPING.md` §2.2), so the
/// claim is that none is sent for that fader's channel, however much the show
/// around it changes.
///
/// The control test is the one that makes it mean something: a `Master` on the
/// same strip **is** written, so an empty list here is a rule rather than a
/// silent surface.
#[tokio::test]
async fn no_crossfade_fader_is_ever_written_back_to_the_desk() {
    let _turn = common::one_daemon_at_a_time();
    for (mode, written) in [
        (ExecutorFaderFunction::XFade, false),
        (ExecutorFaderFunction::Fade, false),
        (ExecutorFaderFunction::Master, true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut file = show_with(mode);
        ShowStore::open(dir.path().join("walk.prism"))
            .unwrap()
            .save(&mut file)
            .unwrap();

        let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
        let (port, surface) = MockSurfacePort::new();
        daemon.attach_surface(Box::new(port));
        let desk = daemon.desk().clone();
        desk.command(Command::SetExecutorPage { page: 0 });
        // **Selected**, so the main fader follows this executor too. With
        // nothing selected the desk parks the main fader at nought — which is
        // right, and would make this test pass for the wrong reason.
        desk.command(Command::SelectExecutor {
            executor_id: ExecutorId::new(0),
        });
        // The first picture, which every strip gets whatever is on it.
        run_until(&mut daemon, "the desk to be painted", || {
            !surface.received().is_empty()
        })
        .await;

        // Everything the first picture had queued, drained: the outbound path
        // is **paced** (`prism_surface`'s controller), so a message decided
        // before the clear can leave after it.
        for _ in 0..20 {
            settle(&mut daemon).await;
        }
        // From here on, only what the daemon sends *because of the fader*.
        surface.clear_received();
        desk.command(Command::ExecutorButton {
            executor_id: ExecutorId::new(0),
            button: prism_domain::ExecutorButtonRef::Slot { index: 0 },
            pressed: true,
        });
        settle(&mut daemon).await;
        for level in [0_u16, 20_000, 45_000, 65_535, 30_000, 0] {
            desk.command(Command::SetExecutorMaster {
                executor_id: ExecutorId::new(0),
                level,
            });
            settle(&mut daemon).await;
        }
        // And a repaint provoked by something else entirely: a show change is
        // what used to write the fader back even when nobody had touched it.
        desk.command(Command::Label {
            target: prism_domain::ObjectRef::Sequence {
                sequence_id: SequenceId::new(1),
            },
            name: "Walked".to_owned(),
        });
        settle(&mut daemon).await;

        // Strip 0's pitch-bend channel is 0 (`prism_surface::profile`), and the
        // main fader's is 8 — the selected executor is this one too, so both
        // have to stay silent.
        let moved: Vec<(u8, u16)> = fader_writes(&surface)
            .into_iter()
            .filter(|(channel, _)| *channel == 0 || *channel == 8)
            .collect();
        if written {
            assert!(
                !moved.is_empty(),
                "a Master was never written, so this test proves nothing about the other two"
            );
        } else {
            assert!(
                moved.is_empty(),
                "{mode:?}: the desk moved the operator's fader — {moved:?}"
            );
        }
        daemon.shutdown().await;
    }
}
