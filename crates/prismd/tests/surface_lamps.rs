//! **A key's lamp follows what the key is bound to** — S59's first exit
//! criterion, asserted against the bytes that reach the surface.
//!
//! # Why this target exists at all
//!
//! Until S59 the surface had two lamps and both hung on the **hardware
//! position**: a strip's Select key, lit off that strip's cue list, and the Save
//! key, lit off the unsaved-changes flag. Neither could survive a desk whose
//! keys are bound, because a Save lamp on a key that has been given to `Store`
//! is a lamp that lies — and it lies in the dark, which is where an operator
//! reads it.
//!
//! So the lamp is a property of the **action** now (`prismd::lamp`), and the
//! question it answers is the owner's: *would pressing this now lead anywhere?*
//! Two halves — the command line would accept what the key writes, and what it
//! writes would do something.
//!
//! # Nothing here touches a device, and nothing asks the profile
//!
//! `CLAUDE.md`'s rule and S20's method rule, the same two `tests/controls.rs`
//! keeps: the surface is a [`MockSurfacePort`], and every note number is written
//! out **by hand from `docs/MCU_MAPPING.md` §2.1**. Asking the profile which
//! note carries F1 would be asking the code under test what to look at.
//!
//! # And it reads bytes rather than a model
//!
//! S8's finding, which this project has repeated in four places since: a test
//! that asserted `controller.led(...)` would pass for a desk that computed the
//! right answer and never sent it. What is asserted here is the **Note On**
//! message the daemon actually put on the wire — velocity `0x7F` for lit and
//! `0x00` for dark, which is `docs/MCU_MAPPING.md` §2.2.

// Every test holds `common::one_daemon_at_a_time` across its awaits, for
// `wiring.rs`'s reason: a daemon owns a real-time tick thread, and several in
// one process measure each other rather than the daemon.
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::time::Duration;

use prism_core::{SessionState, Show, ShowFile, ShowStore};
use prism_domain::{
    AttributeDef, AttributeType, BoundControl, Command, ExecutorId, Fixture, FixtureId,
    FixtureType, GlobalButton, MachineChange, SelectionMode, SurfaceAction, UniverseId, Vec3,
    ViewId,
};
use prismd::cli::{Listen, Options};
use prismd::daemon::Daemon;
use prismd::surface::{MockSurfaceHandle, MockSurfacePort};

mod common;

/// Note On, MIDI channel 1 — §2.1's "all note and CC numbers are on MIDI
/// channel 1", which is status `0x90`.
const NOTE_ON: u8 = 0x90;

/// F1, note 54 — §2.1's "Function" row: F1–F8 = 54–61.
const F1: u8 = 54;

/// F2, note 55.
const F2: u8 = 55;

/// F3, note 56.
const F3: u8 = 56;

/// Velocity that lights a button LED — §2.2.
const LIT: u8 = 0x7F;

/// Velocity that puts it out.
const DARK: u8 = 0x00;

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

/// One channel of one attribute, at an offset.
fn attribute(attribute: AttributeType, coarse_offset: u16) -> AttributeDef {
    AttributeDef {
        switched: None,
        attribute,
        label: None,
        occurrence: 0,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value: 0,
        merge_mode: attribute.default_merge_mode(),
        ranges: Vec::new(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

/// A show with **one dimmer patched**, which is all the programmer needs to
/// hold something: Store's lamp is about whether there are values, not about
/// what they are on.
fn write_show(path: &Path) {
    let mut show = Show::new();
    show.embed_fixture_type(FixtureType {
        id: "generic.dimmer.dark".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "1ch".to_owned(),
        footprint: 1,
        attributes: vec![attribute(AttributeType::Dimmer, 0)],
        // S61: a profile written by hand describes channels and not a device.
        physical: None,
    })
    .expect("a one-channel dimmer is a fixture type");
    show.patch_fixture(Fixture {
        software_dimmer: true,
        id: FixtureId::new(1),
        name: "Dimmer 1".to_owned(),
        type_id: "generic.dimmer.dark".to_owned(),
        universe: UniverseId::new(1),
        address: 1,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        invert_pan: false,
        invert_tilt: false,
    })
    .expect("the address is free");

    let mut file = ShowFile::new();
    file.show = show;
    let mut session = SessionState::new();
    session.store_view(ViewId::new(2), "Programming").unwrap();
    session.select_executor(Some(ExecutorId::new(0))).unwrap();
    file.session = session;
    let mut store = ShowStore::open(path).unwrap();
    store.save(&mut file).unwrap();
}

/// Puts a value in the programmer: the patched dimmer, selected, at half.
fn hold_a_value(daemon: &Daemon) {
    daemon
        .desk()
        .core()
        .apply(&Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        })
        .expect("the selection was refused");
    daemon
        .desk()
        .core()
        .apply(&Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            occurrence: 0,
            value: 32_768,
            relative: false,
        })
        .expect("the value was refused");
}

/// Makes the show dirty, which is the Save lamp's whole condition.
fn edit_the_show(daemon: &Daemon, id: u32, address: u16) {
    daemon
        .desk()
        .core()
        .apply(&Command::PatchFixture {
            software_dimmer: true,
            id: FixtureId::new(id),
            name: String::new(),
            type_id: "generic.dimmer.dark".to_owned(),
            universe: UniverseId::new(1),
            address,
        })
        .expect("the fixture was refused");
}

/// A started daemon with a mock surface attached.
async fn desk(dir: &Path) -> (Daemon, MockSurfaceHandle) {
    write_show(&dir.join("aula.prism"));
    let mut daemon = Daemon::start(&options(dir)).await.unwrap();
    let (port, handle) = MockSurfacePort::new();
    daemon.attach_surface(Box::new(port));
    (daemon, handle)
}

/// Says what one control does, over the protocol.
fn bind(daemon: &Daemon, control: BoundControl, action: Option<SurfaceAction>) {
    daemon
        .desk()
        .core()
        .apply(&Command::ConfigureMachine {
            change: MachineChange::SurfaceBinding { control, action },
        })
        .expect("the binding was refused");
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

/// The **last** velocity the surface was sent for this note, or nothing.
///
/// The last one rather than any one, because the shadow model sends only what
/// changed (§2.2): a lamp that went on and then off has two messages in the
/// log, and the one that describes the desk an operator is looking at is the
/// second. Searching for *a* lit message would pass for a lamp that flickered.
fn lamp(handle: &MockSurfaceHandle, note: u8) -> Option<u8> {
    handle
        .received()
        .iter()
        .filter(|message| message.len() == 3 && message[0] == NOTE_ON && message[1] == note)
        .map(|message| message[2])
        .next_back()
}

/// Runs until the lamp on `note` reaches `want`, and says so by name if it does
/// not.
async fn run_until_lamp(daemon: &mut Daemon, handle: &MockSurfaceHandle, note: u8, want: u8) {
    let what = format!("note {note} to read {want:#04x}");
    run_until(daemon, &what, || lamp(handle, note) == Some(want)).await;
}

/// **Store is dark with an empty programmer and lit with a value in it.**
///
/// The owner's first condition, and the one that shows the whole change: F1 is
/// not the Store key on any profile, it is a function key that has been *given*
/// Store — so a lamp that followed the hardware would have nothing to say about
/// it, and before S59 nothing did.
#[tokio::test]
async fn a_bound_store_key_lights_when_the_programmer_holds_something() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    bind(
        &daemon,
        BoundControl::Global {
            button: GlobalButton::F1,
        },
        Some(SurfaceAction::ConsoleWord {
            word: "Store".to_owned(),
        }),
    );
    run_until_lamp(&mut daemon, &handle, F1, DARK).await;

    // Something selected with a value on it: now Store would store something.
    hold_a_value(&daemon);
    run_until_lamp(&mut daemon, &handle, F1, LIT).await;
}

/// **The Save lamp follows the Save *action*, not the Save *key*.**
///
/// §4.1 has said *Save | LED lit while unsaved changes exist* since S22, and the
/// daemon lit `GlobalButton::Save` by name to satisfy it. This is the same
/// sentence with the key moved: F2 is given `SaveShow` and reports the show's
/// dirty flag, which is only true if the lamp is reading the binding.
#[tokio::test]
async fn the_save_lamp_moves_with_the_binding() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    bind(
        &daemon,
        BoundControl::Global {
            button: GlobalButton::F2,
        },
        Some(SurfaceAction::SaveShow),
    );
    run_until_lamp(&mut daemon, &handle, F2, DARK).await;

    // Any show edit makes it dirty.
    edit_the_show(&daemon, 7, 20);
    run_until_lamp(&mut daemon, &handle, F2, LIT).await;
}

/// **An argument keyword reads the line's *position*, not the show.**
///
/// This is the grammar half on its own (`prism_core::console::accepts_next`),
/// and the transition worth asserting is not *empty versus not empty*: on an
/// **empty** line the completer offers every word it has, so `Cue` is lit there
/// and lit correctly. What it cannot follow is a selection in progress — after
/// `1 ` the only words the grammar will take are `at`, `thru` and `full`, so a
/// bound `Cue` key goes dark; write `store ` instead and it comes back, because
/// *store cue …* is a line.
///
/// A desk that lit every keypad key all the time would pass a test written the
/// other way round. This is the pair that tells them apart.
#[tokio::test]
async fn an_argument_keyword_lights_only_where_the_line_would_take_it() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    bind(
        &daemon,
        BoundControl::Global {
            button: GlobalButton::F3,
        },
        Some(SurfaceAction::ConsoleWord {
            word: "Cue".to_owned(),
        }),
    );

    let write = |daemon: &Daemon, text: &str| {
        daemon
            .desk()
            .core()
            .apply(&Command::CommandLineInput {
                text: text.to_owned(),
                run: false,
                mode: None,
            })
            .expect("the line was refused");
    };

    // A selection being built takes a level or a range, and nothing else.
    write(&daemon, "1 ");
    run_until_lamp(&mut daemon, &handle, F3, DARK).await;

    // A verb waiting for the thing it acts on takes an object word.
    write(&daemon, "store ");
    run_until_lamp(&mut daemon, &handle, F3, LIT).await;
}

/// **Clear follows the stage, which is the owner's correction of 2026-09-20.**
///
/// *Ich möchte aber das clear nicht auf den Programmer schaut, sondern auf die
/// clear stufe.* The two agree most of the time and part company at the end: the
/// press that takes the values leaves a programmer with none, while the stage
/// still has the encoder bank and the page to take (`ClearStage::All`). A lamp
/// reading the values would go dark one press early and an operator would stop
/// pressing one press early.
///
/// So this walks the machine down: something selected with a value on it, then
/// Clear, Clear, Clear, and the lamp is out only after the last one.
#[tokio::test]
async fn the_clear_lamp_follows_the_stage_and_not_the_values() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    bind(
        &daemon,
        BoundControl::Global {
            button: GlobalButton::F1,
        },
        Some(SurfaceAction::ConsoleWord {
            word: "Clear".to_owned(),
        }),
    );
    run_until_lamp(&mut daemon, &handle, F1, DARK).await;

    hold_a_value(&daemon);
    run_until_lamp(&mut daemon, &handle, F1, LIT).await;

    // Three presses of the machine, and the lamp stays lit through the first
    // two. The third is what puts it out.
    for press in 1..=2 {
        daemon
            .desk()
            .core()
            .apply(&Command::ClearProgrammer)
            .expect("clear was refused");
        run_until_lamp(&mut daemon, &handle, F1, LIT).await;
        assert_eq!(
            lamp(&handle, F1),
            Some(LIT),
            "the lamp went out after clear {press}, one press early"
        );
    }
    daemon
        .desk()
        .core()
        .apply(&Command::ClearProgrammer)
        .expect("clear was refused");
    run_until_lamp(&mut daemon, &handle, F1, DARK).await;
}

/// **Full follows the selection, and Update follows the open cue.**
///
/// Two of the nine conditions, together because they are the pair that shows
/// they are separate: selecting fixtures lights `Full` and leaves `Update`
/// dark, because there is something to set to full and nothing to store back
/// into. `Update` is the one the daemon would otherwise refuse with
/// `NothingIsBeingEdited` — this is that refusal said before the press.
#[tokio::test]
async fn full_follows_the_selection_and_update_follows_the_open_cue() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    for (button, word) in [(GlobalButton::F1, "Full"), (GlobalButton::F2, "Update")] {
        bind(
            &daemon,
            BoundControl::Global { button },
            Some(SurfaceAction::ConsoleWord {
                word: word.to_owned(),
            }),
        );
    }
    run_until_lamp(&mut daemon, &handle, F1, DARK).await;
    run_until_lamp(&mut daemon, &handle, F2, DARK).await;

    hold_a_value(&daemon);
    run_until_lamp(&mut daemon, &handle, F1, LIT).await;
    // And Update is **still** dark: a selection is not a cue being edited, and
    // a lamp that confused the two would invite a press the desk then refuses.
    assert_eq!(
        lamp(&handle, F2),
        Some(DARK),
        "Update lit with nothing open to update"
    );
}

/// **Oops and Redo follow the journal, and Oops the line as well.**
///
/// B58 gave the Undo key two jobs — take a word off a standing line, or take an
/// edit back on an empty one — and the lamp has to mean both, or it would be
/// dark at exactly the moment an operator reaches for it to correct a typo.
///
/// Redo has the simpler half and is asserted with it, because the pair is what
/// shows the lamp is reading the journal rather than *something happened*: an
/// edit lights Oops and leaves Redo dark, and taking it back swaps them.
#[tokio::test]
async fn oops_and_redo_follow_the_journal_and_oops_the_line_too() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    bind(
        &daemon,
        BoundControl::Global {
            button: GlobalButton::F1,
        },
        Some(SurfaceAction::Oops),
    );
    bind(
        &daemon,
        BoundControl::Global {
            button: GlobalButton::F2,
        },
        Some(SurfaceAction::Redo),
    );
    run_until_lamp(&mut daemon, &handle, F1, DARK).await;
    run_until_lamp(&mut daemon, &handle, F2, DARK).await;

    // An undoable edit lights Oops and leaves Redo dark.
    edit_the_show(&daemon, 4, 40);
    run_until_lamp(&mut daemon, &handle, F1, LIT).await;
    assert_eq!(
        lamp(&handle, F2),
        Some(DARK),
        "Redo lit with nothing to redo"
    );

    // Taking it back swaps them.
    daemon
        .desk()
        .core()
        .apply(&Command::Oops)
        .expect("the undo was refused");
    run_until_lamp(&mut daemon, &handle, F2, LIT).await;

    // And a word standing on the line lights Oops whatever the journal says —
    // B58's other half.
    daemon
        .desk()
        .core()
        .apply(&Command::Redo)
        .expect("the redo was refused");
    daemon
        .desk()
        .core()
        .apply(&Command::CommandLineInput {
            text: "store ".to_owned(),
            run: false,
            mode: None,
        })
        .expect("the line was refused");
    run_until_lamp(&mut daemon, &handle, F1, LIT).await;
}

/// **The playback row follows whether the executor has a cue list.**
///
/// An operator pressing Go on an empty executor has done nothing, and the lamp
/// is what says so before the press rather than after. The selected executor
/// starts with no sequence on it, so this is the one condition that can be
/// turned on by assigning rather than by programming.
#[tokio::test]
async fn the_playback_keys_follow_the_selected_executor() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    bind(
        &daemon,
        BoundControl::Global {
            button: GlobalButton::F1,
        },
        Some(SurfaceAction::ExecutorGo {
            target: prism_domain::ExecutorTarget::Selected,
            direction: prism_domain::GoDirection::Next,
        }),
    );
    run_until_lamp(&mut daemon, &handle, F1, DARK).await;

    // A cue list, put on the executor the session has selected. It is **stored
    // into existence** rather than made with `New`, which takes a view and
    // nothing else (`console.rs`'s grammar): a sequence comes into being when
    // something is stored into it.
    hold_a_value(&daemon);
    daemon
        .desk()
        .core()
        .apply(&Command::CommandLineInput {
            text: "Store Sequence 1".to_owned(),
            run: true,
            mode: None,
        })
        .expect("the store was refused");
    daemon
        .desk()
        .core()
        .apply(&Command::CommandLineInput {
            text: "Assign Sequence 1 Executor 0".to_owned(),
            run: true,
            mode: None,
        })
        .expect("the assignment was refused");

    run_until_lamp(&mut daemon, &handle, F1, LIT).await;
}

/// **A page change leaves a half-typed line exactly where it is** — S59.
///
/// The owner named this as a precaution rather than a sighting on 2026-09-20:
/// *page braucht die gleiche Änderung wie view*, after B56 (GitHub #24) found
/// that a view change ran `View 2` as a line and the daemon clears a line it has
/// run. Changing the executor page never went through the line — the bar sends
/// `SetExecutorPage` and the X-Touch's `Bank ◀▶` always has — so this is the
/// test that holds it rather than a fix.
///
/// It is here rather than beside the view test because the press is the point:
/// a **key on the desk**, while a line stands in a session a browser is typing
/// into.
#[tokio::test]
async fn a_page_change_from_the_desk_leaves_the_line_standing() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    bind(
        &daemon,
        BoundControl::Global {
            button: GlobalButton::F1,
        },
        Some(SurfaceAction::ExecutorPage { delta: 1 }),
    );
    // The desk handle is cloned so that the two readings below borrow it rather
    // than the daemon — the daemon has to stay mutable to be run.
    let desk_handle = daemon.desk().clone();
    let line = || {
        desk_handle
            .core()
            .file
            .session
            .session()
            .command_line
            .clone()
    };
    let page = || desk_handle.core().file.session.session().executor_page;

    daemon
        .desk()
        .core()
        .apply(&Command::CommandLineInput {
            text: "Fixture 1 thru ".to_owned(),
            run: false,
            mode: None,
        })
        .expect("the line was refused");
    let before = page();

    handle.press(NOTE_ON, F1);
    run_until(&mut daemon, "the page to change", || page() != before).await;
    assert_eq!(
        line(),
        "Fixture 1 thru ",
        "the page change took the line with it"
    );
}

/// **An unbound key is dark**, and it is dark because nothing is on it rather
/// than because nothing looked.
///
/// The surface never lights its own lamps (`prism_surface::feedback`), so a
/// panel the daemon ignored would also be dark — which is why this asserts the
/// *transition*: F1 is lit through a binding, the binding is taken away, and the
/// lamp goes out.
#[tokio::test]
async fn unbinding_a_key_puts_its_lamp_out() {
    let _one = common::one_daemon_at_a_time();
    let dir = tempfile::tempdir().unwrap();
    let (mut daemon, handle) = desk(dir.path()).await;

    let f1 = BoundControl::Global {
        button: GlobalButton::F1,
    };
    // Oops on an empty journal and an empty line is dark, so the show is made
    // dirty first and Save is what is bound — one condition, already asserted
    // above, used here only to get a lamp lit.
    bind(&daemon, f1, Some(SurfaceAction::SaveShow));
    edit_the_show(&daemon, 3, 30);
    run_until_lamp(&mut daemon, &handle, F1, LIT).await;

    bind(&daemon, f1, None);
    run_until_lamp(&mut daemon, &handle, F1, DARK).await;
}
