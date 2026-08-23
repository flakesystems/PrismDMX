//! A colour typed at the desk reaches the scribble strips.
//!
//! `Command::Color` writes twenty-four bits onto a cue list; a strip has three
//! lamps. Everything between the two is covered by unit tests already —
//! `prism_surface::color` quantises, `prism_core::objects` stores — so what is
//! left is the claim nobody else makes: **the daemon draws it**, on the strip
//! whose executor holds that cue list, without being asked to redraw.
//!
//! # Nothing here touches a device
//!
//! `CLAUDE.md`'s rule, and `surface_gate.rs`'s method: the surface is a
//! [`MockSurfacePort`] and the bytes are read **against `docs/MCU_MAPPING.md`
//! §2.3 written out by hand** — `F0 00 00 66 14 72 c0…c7 F7`, one additive RGB
//! triple per strip, bit 0 red, bit 1 green, bit 2 blue. Asking
//! `prism_surface` what to expect would be asking the code under test what it
//! does, which is S19's finding and S20's method rule in one.

// Every test here holds `common::one_daemon_at_a_time` across its awaits — a
// daemon owns a tick thread at real-time priority, and two of those in one
// process measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::ShowStore;
use prism_domain::{Command, ExecutorId, ObjectRef, RgbColor, SequenceId};
use prismd::cli::{Options, mock_output};
use prismd::daemon::Daemon;
use prismd::surface::{MockSurfaceHandle, MockSurfacePort};

mod common;

/// The header of the X-Touch's colour message — §2.3's table, byte for byte.
/// `00 00 66` is Mackie's manufacturer ID, `14` the X-Touch in MC mode, `72`
/// the colour command.
const COLOR_HEADER: [u8; 6] = [0xF0, 0x00, 0x00, 0x66, 0x14, 0x72];

/// The colour byte for a strip lit red — bit 0 alone (§2.3).
const RED: u8 = 0b001;

/// The colour byte for a strip lit white — all three lamps.
const WHITE: u8 = 0b111;

fn options(dir: &Path) -> Options {
    Options {
        data_dir: Some(dir.to_path_buf()),
        show: Some(dir.join("aula.prism")),
        universes: Some(2),
        outputs: vec![mock_output(1)],
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

/// The eight colour bytes of the **last** colour message the desk was sent, or
/// `None` when it has not been sent one since the buffer was cleared.
///
/// The last rather than the first: the surface redraws whenever the picture
/// changes, so what matters is what the strips are showing now.
fn strip_colors(surface: &MockSurfaceHandle) -> Option<[u8; 8]> {
    surface
        .received()
        .iter()
        .rev()
        .find(|message| message.starts_with(&COLOR_HEADER) && message.len() == 15)
        .map(|message| {
            let mut colors = [0u8; 8];
            colors.copy_from_slice(&message[6..14]);
            colors
        })
}

/// Runs the daemon in slices until `condition` holds, or fails by name.
///
/// In slices for `surface_gate.rs`'s reason: the surface is polled inside
/// `Daemon::run`, which is the arrangement under test.
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

/// A colour on a cue list lights the strip of the fader holding it — and the
/// seven strips beside it are left alone.
///
/// Executor 0 plays sequence 1 in `common::show_file`, and the session opens on
/// page 0, so strip 0 is that executor.
#[tokio::test]
async fn a_colour_on_a_cue_list_lights_the_strip_of_the_fader_that_holds_it() {
    let _turn = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let mut file = common::show_file();
    ShowStore::open(dir.path().join("aula.prism"))
        .unwrap()
        .save(&mut file)
        .unwrap();

    let mut daemon = Daemon::start(&options(dir.path())).await.unwrap();
    let (port, surface) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    let desk = daemon.desk().clone();
    // Page 0, so strip 0 is executor 0 — the one holding sequence 1. The show
    // this fixture writes opens on another page, and a test that assumed page 0
    // would be asserting about a fader nobody had assigned.
    desk.command(Command::SetExecutorPage { page: 0 });

    // **An uncoloured cue list is white, not dark.** The text on an unlit strip
    // cannot be read (§2.3), so *no colour chosen* is the readable default —
    // and every strip starts there, including the seven with nothing on them.
    run_until(&mut daemon, "the first picture to reach the desk", || {
        strip_colors(&surface).is_some()
    })
    .await;
    assert_eq!(
        strip_colors(&surface),
        Some([WHITE; 8]),
        "a desk with no colours on it is lit white"
    );

    // A pale amber, which is the case `prism_surface::color` exists for: by
    // nearest corner in RGB it is white, and by hue it is the amber it is a
    // pastel of. An operator who put amber on a fader is looking for the amber
    // one.
    surface.clear_received();
    desk.command(Command::Color {
        target: ObjectRef::Sequence {
            sequence_id: SequenceId::new(1),
        },
        color: Some(RgbColor {
            r: 255,
            g: 200,
            b: 180,
        }),
    });
    run_until(&mut daemon, "the strip to be lit", || {
        strip_colors(&surface).is_some_and(|colors| colors[0] == RED)
    })
    .await;
    assert_eq!(
        strip_colors(&surface),
        Some([RED, WHITE, WHITE, WHITE, WHITE, WHITE, WHITE, WHITE]),
        "one strip changed and the other seven did not"
    );

    // And the same line through the **fader** rather than the cue list, which
    // is the indirection an operator means: executor 0 holds sequence 1, so
    // this colours that list.
    surface.clear_received();
    desk.command(Command::Color {
        target: ObjectRef::Executor {
            executor_id: ExecutorId::new(0),
        },
        color: None,
    });
    run_until(&mut daemon, "the strip to go back to white", || {
        strip_colors(&surface).is_some_and(|colors| colors[0] == WHITE)
    })
    .await;
    assert_eq!(
        desk.core()
            .file
            .show
            .sequence(SequenceId::new(1))
            .unwrap()
            .color,
        None,
        "the fader took the colour off the list on it"
    );

    daemon.shutdown().await;
}
