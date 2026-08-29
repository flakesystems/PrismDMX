//! S15 exit criteria: a show survives the platter **byte-identically**, a
//! process killed mid-write leaves a file that still opens and holds the last
//! committed state, a version-1 file migrates to version 2, and the dirty flag
//! drives the `DirtyFlag` deltas that light the X-Touch's Save LED.
//!
//! "Byte-identical" is taken literally, as it has been since S11: the whole
//! [`ShowFile`] is serialised with `rmp_serde::to_vec_named` before the save and
//! again after the load, and the two `Vec<u8>` are compared.
//!
//! **The fixture in this file is deliberately free of default values.** S14
//! recorded the reason after nearly missing a defect because of it: a fixture
//! built out of zeros cannot tell "restored correctly" from "never touched",
//! and for a save/load round trip that is the central trap — a loader that
//! silently dropped a table would pass against a show whose tables were empty
//! anyway.
#![allow(clippy::print_stdout)]

mod common;

use std::io::{BufRead as _, BufReader};
use std::process::{Command as OsCommand, Stdio};
use std::time::Duration;

use common::{attribute, dimmer_type, par_type};
use prism_core::{
    Autosave, Effect, ShowFile, ShowStore, StoreError, export_json, import_json, show_patch_ops,
};
use prism_domain::{
    AttributeType, Command, Cue, CuePart, CueTrigger, Delta, Executor, ExecutorButtonFunction,
    ExecutorEncoderFunction, ExecutorFaderFunction, ExecutorId, FeatureGroup, Fixture, FixtureId,
    FixtureType, Group, GroupId, JsonValue, ParamDirection, Preset, PresetId, PresetPool,
    PresetValue, RgbColor, SelectionMode, Sequence, SequenceId, UniverseId, Vec3, ViewId,
    WindowInstanceId, WindowType,
};
use proptest::prelude::*;

/// The file as bytes — the show and the session, which is what a `.prism` file
/// holds. The programmer and the journal are `#[serde(skip)]` and are asserted
/// separately, because their absence is the point.
fn bytes(file: &ShowFile) -> Vec<u8> {
    rmp_serde::to_vec_named(file).unwrap()
}

/// A moving head with a 16-bit pan, so a profile in the file is more than a
/// list of one-byte channels.
fn head_type() -> FixtureType {
    FixtureType {
        id: "generic.head".to_owned(),
        manufacturer: "Aula".to_owned(),
        name: "Wash Head".to_owned(),
        mode: "6ch".to_owned(),
        footprint: 6,
        attributes: vec![
            prism_domain::AttributeDef {
                attribute: AttributeType::Pan,
                feature_group: FeatureGroup::Position,
                coarse_offset: 0,
                fine_offset: Some(1),
                default_value: 32768,
                merge_mode: prism_domain::MergeMode::Ltp,
                invert: true,
                physical_from: -270.0,
                physical_to: 270.0,
            },
            attribute(AttributeType::Tilt, 2, 32768),
            attribute(AttributeType::Dimmer, 3, 0),
            attribute(AttributeType::Zoom, 4, 16384),
            attribute(AttributeType::Gobo, 5, 0),
        ],
    }
}

/// A fixture that is hung somewhere, pointed somewhere and possibly inverted —
/// none of which is a default.
fn hung(id: u32, type_id: &str, universe: u32, address: u16, x: f64) -> Fixture {
    Fixture {
        software_dimmer: true,
        id: FixtureId::new(id),
        name: format!("Head {id}"),
        type_id: type_id.to_owned(),
        universe: UniverseId::new(universe),
        address,
        // Exactly representable in binary, which is what S1's finding about
        // `serde_json`'s parser requires of anything compared for equality
        // across the JSON export as well as the file.
        position: Vec3 {
            x,
            y: 4.5,
            z: -2.25,
        },
        rotation: Vec3 {
            x: 0.0,
            y: 180.0,
            z: 90.5,
        },
        invert_pan: id.is_multiple_of(2),
        invert_tilt: id.is_multiple_of(3),
    }
}

/// A show and a session in which nothing is at its default — see the module
/// documentation for why that is a requirement rather than thoroughness.
fn saveable_file() -> ShowFile {
    let mut file = ShowFile::new();
    file.show.embed_fixture_type(par_type()).unwrap();
    file.show.embed_fixture_type(head_type()).unwrap();
    file.show
        .embed_fixture_type(dimmer_type("generic.dimmer", 0))
        .unwrap();
    file.show
        .patch_fixture(hung(1, "generic.head", 1, 1, 1.5))
        .unwrap();
    file.show
        .patch_fixture(hung(2, "generic.head", 1, 7, -3.75))
        .unwrap();
    file.show
        .patch_fixture(hung(3, "generic.rgbw.par", 2, 21, 0.125))
        .unwrap();
    file.show
        .patch_fixture(hung(4, "generic.dimmer", 64, 512, 8.0))
        .unwrap();
    file.show
        .store_group(Group {
            id: GroupId::new(7),
            name: "Front Wash".to_owned(),
            fixtures: vec![FixtureId::new(1), FixtureId::new(3)],
        })
        .unwrap();
    file.show
        .store_preset(Preset {
            id: PresetId::new(12),
            pool: PresetPool::Color,
            name: "Aula Warm".to_owned(),
            color: Some(RgbColor {
                r: 255,
                g: 176,
                b: 64,
            }),
            values: vec![
                PresetValue {
                    fixture: FixtureId::new(3),
                    attribute: AttributeType::Red,
                    value: 65535,
                },
                PresetValue {
                    fixture: FixtureId::new(3),
                    attribute: AttributeType::Green,
                    value: 45000,
                },
            ],
        })
        .unwrap();
    file.show
        .store_sequence(Sequence {
            id: SequenceId::new(5),
            name: "Act One".to_owned(),
            // A colour on the strip is show content and travels in the file,
            // like the name beside it.
            color: Some(RgbColor {
                r: 255,
                g: 140,
                b: 0,
            }),
            looping: true,
            master_level: u16::MAX,
            speed: prism_domain::SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
            cues: vec![
                Cue {
                    number: "1".to_owned(),
                    name: "Preset".to_owned(),
                    fade_in: 3.5,
                    fade_out: 2.0,
                    delay: 0.25,
                    trigger: CueTrigger::Go,
                    trigger_time: None,
                    parts: vec![CuePart {
                        fixture: FixtureId::new(3),
                        attribute: AttributeType::Red,
                        value: 65535,
                        preset_ref: Some(PresetId::new(12)),
                    }],
                },
                Cue {
                    number: "2.5".to_owned(),
                    name: "Wash out".to_owned(),
                    fade_in: 12.0,
                    fade_out: 12.0,
                    delay: 0.0,
                    trigger: CueTrigger::Time,
                    trigger_time: Some(8.75),
                    parts: vec![
                        CuePart {
                            fixture: FixtureId::new(1),
                            attribute: AttributeType::Pan,
                            value: 12345,
                            preset_ref: None,
                        },
                        CuePart {
                            fixture: FixtureId::new(2),
                            attribute: AttributeType::Dimmer,
                            value: 65535,
                            preset_ref: None,
                        },
                    ],
                },
            ],
        })
        .unwrap();
    file.show
        .store_executor(Executor {
            id: ExecutorId::new(17),
            sequence_id: Some(SequenceId::new(5)),
            fader_function: ExecutorFaderFunction::Master,
            button_functions: vec![
                ExecutorButtonFunction::GoForward,
                ExecutorButtonFunction::Off,
                ExecutorButtonFunction::Flash,
                ExecutorButtonFunction::Toggle,
            ],
            encoder_function: ExecutorEncoderFunction::Speed,
        })
        .unwrap();
    // The level and the rate are the **cue list's** since S45, and neither is
    // its default: a saved and reloaded show has to carry what an operator set
    // rather than what the type would have said.
    file.show
        .set_sequence_master(SequenceId::new(5), 40000)
        .unwrap();
    file.show
        .set_sequence_speed(SequenceId::new(5), prism_domain::SPEED_UNITY * 2)
        .unwrap();

    // The session half, driven through the commands an operator would use.
    for command in [
        Command::OpenWindow {
            window: WindowType::FixtureSheet,
            params: None,
        },
        Command::OpenWindow {
            window: WindowType::PresetPool,
            params: Some(std::collections::BTreeMap::from([(
                "pool".to_owned(),
                JsonValue::String("Color".to_owned()),
            )])),
        },
        Command::StoreView {
            view_id: ViewId::new(4),
            name: "Programming".to_owned(),
        },
        Command::OpenWindow {
            window: WindowType::Viewer3D,
            params: None,
        },
        Command::SelectView {
            view_id: ViewId::new(4),
        },
        Command::SetExecutorPage { page: 2 },
        Command::SelectExecutor {
            executor_id: ExecutorId::new(17),
        },
        Command::SetEncoderBank {
            group: FeatureGroup::Beam,
        },
        Command::SetProgrammerPage { page: 3 },
        Command::SelectProgrammerParam {
            direction: ParamDirection::Next,
        },
        Command::CommandLineInput {
            text: "1 thru 4 at full".to_owned(),
            run: false,
        },
    ] {
        file.apply(&command).unwrap();
    }
    // Fractional coordinates, so the file has to carry them exactly — and a
    // place that does not bury the second window, which **S43** made a rule
    // (`prism_core::layout`, punch-list B10): the daemon opens the two side by
    // side now, so a move back over the neighbour changes nothing at all.
    file.session
        .place_window(WindowInstanceId::new(1), 120.0, 500.0, 800.5, 512.25)
        .unwrap();
    file
}

/// A second show, different from [`saveable_file`] in **every** table — which
/// is what makes "the file is one of the two and never a mixture" a real
/// assertion in the crash test.
fn second_file() -> ShowFile {
    let mut file = ShowFile::new();
    file.show.embed_fixture_type(par_type()).unwrap();
    for id in 1..=40u32 {
        let address = u16::try_from((id - 1) * 4 + 1).unwrap();
        file.show
            .patch_fixture(hung(id, "generic.rgbw.par", 1, address, f64::from(id)))
            .unwrap();
    }
    file.show
        .store_group(Group {
            id: GroupId::new(1),
            name: "Everything".to_owned(),
            fixtures: (1..=40).map(FixtureId::new).collect(),
        })
        .unwrap();
    file.show
        .store_sequence(Sequence {
            id: SequenceId::new(1),
            name: "Chase".to_owned(),
            color: None,
            looping: false,
            master_level: u16::MAX,
            speed: prism_domain::SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
            cues: (1..=40)
                .map(|id| Cue {
                    number: id.to_string(),
                    name: format!("Step {id}"),
                    fade_in: 0.5,
                    fade_out: 0.5,
                    delay: 0.0,
                    trigger: CueTrigger::Follow,
                    trigger_time: None,
                    parts: vec![CuePart {
                        fixture: FixtureId::new(id),
                        attribute: AttributeType::Red,
                        value: 65535,
                        preset_ref: None,
                    }],
                })
                .collect(),
        })
        .unwrap();
    file.show
        .store_executor(common::executor(3, Some(1)))
        .unwrap();
    file.session
        .open_window(WindowType::SequenceSheet, None)
        .unwrap();
    file
}

// -- the round trip -------------------------------------------------------

#[test]
fn a_saved_show_comes_back_byte_identical() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");

    let mut saved = saveable_file();
    let before = bytes(&saved);
    // The guard the module documentation asks for: this fixture is not the
    // empty show, so a loader that dropped a table would be caught.
    assert_ne!(before, bytes(&ShowFile::new()));

    let mut store = ShowStore::open(&path).unwrap();
    store.save(&mut saved).unwrap();
    drop(store);

    let store = ShowStore::open(&path).unwrap();
    let mut reopened = ShowFile::new();
    store.load(&mut reopened).unwrap();
    assert_eq!(bytes(&reopened), before);

    // And field by field, so a round trip that was equal for the wrong reason
    // cannot pass.
    let show = &reopened.show;
    assert_eq!(show.fixture_types().count(), 3);
    assert_eq!(
        show.fixture_type("generic.head").unwrap().attributes[0].fine_offset,
        Some(1)
    );
    let head = show.fixture(FixtureId::new(2)).unwrap();
    assert_eq!(
        head.position,
        Vec3 {
            x: -3.75,
            y: 4.5,
            z: -2.25
        }
    );
    assert_eq!(head.rotation.z, 90.5);
    assert!(head.invert_pan);
    assert_eq!(
        show.fixture(FixtureId::new(4)).unwrap().universe,
        UniverseId::new(64)
    );
    assert_eq!(show.group(GroupId::new(7)).unwrap().fixtures.len(), 2);
    let preset = show.preset(PresetId::new(12)).unwrap();
    assert_eq!(
        preset.color,
        Some(RgbColor {
            r: 255,
            g: 176,
            b: 64
        })
    );
    assert_eq!(preset.values.len(), 2);
    let sequence = show.sequence(SequenceId::new(5)).unwrap();
    assert!(sequence.looping);
    assert_eq!(sequence.cues[1].number, "2.5");
    assert_eq!(sequence.cues[1].trigger_time, Some(8.75));
    assert_eq!(
        sequence.cues[0].parts[0].preset_ref,
        Some(PresetId::new(12))
    );
    // The level and the rate travel on the cue list since S45, and the
    // assignment on the executor.
    assert_eq!(sequence.master_level, 40000);
    assert_eq!(sequence.speed, prism_domain::SPEED_UNITY * 2);
    let executor = show.executor(ExecutorId::new(17)).unwrap();
    assert_eq!(executor.button_functions.len(), 4);
    assert_eq!(executor.sequence_id, Some(SequenceId::new(5)));

    // The session is in the file too — the exit criterion says "sessions
    // included", and S12's reason for putting the views in this half is that a
    // show must not carry another operator's layouts.
    let session = reopened.session.session();
    assert_eq!(session.active_view_id, ViewId::new(4));
    assert_eq!(session.open_windows.len(), 2);
    assert_eq!(session.executor_page, 2);
    assert_eq!(session.selected_executor, Some(ExecutorId::new(17)));
    assert_eq!(session.encoder_bank, FeatureGroup::Beam);
    assert_eq!(session.programmer_page, 3);
    assert_eq!(session.programmer_param_index, 1);
    assert_eq!(session.command_line, "1 thru 4 at full");
    assert_eq!(reopened.session.views().count(), 2);
    let placed = reopened.session.window(WindowInstanceId::new(1)).unwrap();
    assert_eq!(
        (placed.x, placed.y, placed.w, placed.h),
        (120.0, 500.0, 800.5, 512.25)
    );
}

#[test]
fn a_reopened_show_is_clean_and_has_nothing_to_undo() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");
    let mut saved = saveable_file();
    let mut store = ShowStore::open(&path).unwrap();
    store.save(&mut saved).unwrap();

    // A file being loaded into a daemon that already had a show open: the
    // journal describes *the other show*, and the programmer is an absolute
    // override over a rig that has just changed underneath it.
    let mut file = saveable_file();
    file.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(1)],
        mode: SelectionMode::Set,
    })
    .unwrap();
    file.apply(&Command::SetAttribute {
        attribute: AttributeType::Dimmer,
        value: 65535,
        relative: false,
    })
    .unwrap();
    assert!(!file.journal.is_empty());
    assert!(!file.programmer.state().values.is_empty());

    store.load(&mut file).unwrap();
    assert!(
        file.journal.is_empty(),
        "the journal describes a show that is no longer open"
    );
    assert_eq!(file.journal.redo_len(), 0);
    assert!(
        file.programmer.state().values.is_empty(),
        "a programmer left over from another show is an override nobody asked for"
    );
    assert!(file.programmer.state().selection.is_empty());
    assert!(
        !file.is_dirty(),
        "a file that has just been read is not an unsaved change"
    );
    assert!(file.apply(&Command::Oops).is_err());
}

#[test]
fn a_load_tells_the_engine_to_rebuild_everything() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");
    let mut store = ShowStore::open(&path).unwrap();
    store.save(&mut saveable_file()).unwrap();

    let mut file = ShowFile::new();
    let effects = store.load(&mut file).unwrap();
    assert!(effects.contains(&Effect::Repatch));
    assert!(effects.contains(&Effect::ReloadGroups));
    assert!(effects.contains(&Effect::ReloadSequence(SequenceId::new(5))));
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, Effect::ReloadSequence(_)))
            .count(),
        file.show.sequences().count()
    );
}

// -- the dirty flag, which is the Save LED --------------------------------

#[test]
fn the_lamp_goes_out_when_the_write_succeeds_and_only_then() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");
    let mut store = ShowStore::open(&path).unwrap();

    let mut file = saveable_file();
    assert!(
        file.is_dirty(),
        "the fixture was built by editing a new show"
    );

    let deltas = store.save(&mut file).unwrap();
    assert_eq!(
        deltas,
        vec![Delta::DirtyFlag {
            unsaved_changes: false
        }],
        "the lamp goes out, and the client is told exactly once"
    );
    assert!(!file.is_dirty());

    // Saving again changes nothing, so it says nothing: an LED cannot be
    // turned off twice.
    assert_eq!(store.save(&mut file).unwrap(), Vec::new());

    // An edit lights it again, and `SaveShow` is still the command that asks
    // for the write — the file has no store, so the effect names the layer
    // that does (S11).
    let applied = file.apply(&common::patch_command(9, 3, 1)).unwrap();
    assert!(applied.deltas.contains(&Delta::DirtyFlag {
        unsaved_changes: true
    }));
    assert_eq!(
        file.apply(&Command::SaveShow).unwrap().effects,
        vec![Effect::Save]
    );
    assert!(
        file.is_dirty(),
        "asking for a save is not the same as having saved"
    );
    assert_eq!(
        store.save(&mut file).unwrap(),
        vec![Delta::DirtyFlag {
            unsaved_changes: false
        }]
    );

    // And what was written is what the file now holds, patch included.
    let mut reopened = ShowFile::new();
    ShowStore::open(&path).unwrap().load(&mut reopened).unwrap();
    assert!(reopened.show.fixture(FixtureId::new(9)).is_some());
}

#[test]
fn an_undo_back_to_the_saved_state_still_leaves_the_lamp_lit() {
    // S14 decided this and S15 must not quietly change it: the file on disk is
    // not the state in memory just because the two happen to be equal again.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");
    let mut store = ShowStore::open(&path).unwrap();
    let mut file = saveable_file();
    store.save(&mut file).unwrap();

    file.apply(&common::patch_command(9, 3, 1)).unwrap();
    file.apply(&Command::Oops).unwrap();
    assert!(file.is_dirty());
    assert!(file.show.fixture(FixtureId::new(9)).is_none());
}

// -- autosave and the recovery copy ---------------------------------------

#[test]
fn an_autosave_is_due_thirty_seconds_after_the_file_became_dirty() {
    let mut autosave = Autosave::new();
    assert_eq!(Autosave::INTERVAL, Duration::from_secs(30));

    // A clean file is never due, however long the daemon has been running.
    assert!(!autosave.poll(Duration::from_secs(600), false));
    // The clock starts when the first edit arrives, not when the daemon did.
    assert!(!autosave.poll(Duration::from_secs(600), true));
    assert!(!autosave.poll(Duration::from_secs(629), true));
    assert!(autosave.poll(Duration::from_secs(630), true));
    // And then every thirty seconds for as long as it stays dirty.
    assert!(!autosave.poll(Duration::from_secs(659), true));
    assert!(autosave.poll(Duration::from_secs(660), true));
    // A save resets it: the next autosave is thirty seconds after the *next*
    // edit, not thirty seconds after this one.
    assert!(!autosave.poll(Duration::from_secs(661), false));
    assert!(!autosave.poll(Duration::from_secs(680), true));
    assert!(autosave.poll(Duration::from_secs(710), true));
}

#[test]
fn a_recovery_copy_holds_what_has_not_been_saved_and_a_save_discards_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");
    let mut store = ShowStore::open(&path).unwrap();
    let mut file = saveable_file();
    store.save(&mut file).unwrap();

    // The edit the operator has not saved.
    file.apply(&common::patch_command(9, 3, 1)).unwrap();
    let unsaved = bytes(&file);
    store.write_recovery(&file).unwrap();

    // An autosave is not a save: the lamp stays lit, because the file the
    // operator named still does not have this edit in it.
    assert!(file.is_dirty());
    assert!(store.has_recovery());
    let mut on_disk = ShowFile::new();
    ShowStore::open(store.path())
        .unwrap()
        .load(&mut on_disk)
        .unwrap();
    assert!(on_disk.show.fixture(FixtureId::new(9)).is_none());

    // The recovery copy is a `.prism` file like any other, which is what makes
    // it openable by the daemon that finds it after a crash.
    let recovery = ShowStore::open(store.recovery_path()).unwrap();
    let mut recovered = ShowFile::new();
    recovery.load(&mut recovered).unwrap();
    assert_eq!(bytes(&recovered), unsaved);
    drop(recovery);

    // A successful save makes it meaningless, so it goes.
    store.save(&mut file).unwrap();
    assert!(!store.has_recovery());
    assert!(!store.recovery_path().exists());
}

// -- crash safety ---------------------------------------------------------

/// The child half of [`a_process_killed_mid_write_leaves_the_last_committed_state`].
///
/// `#[ignore]`d because it never returns: it saves the same show over and over
/// until somebody kills it, which is the point. The parent re-invokes this
/// binary by name — see there.
#[test]
#[ignore = "the child half of the crash test; the parent re-invokes this binary"]
fn a_writer_that_saves_until_it_is_killed() {
    use std::io::Write as _;

    let path = std::env::var("PRISM_CRASH_SHOW").expect("PRISM_CRASH_SHOW");
    let mark = std::env::var("PRISM_CRASH_MARK").expect("PRISM_CRASH_MARK");
    let mut store = ShowStore::open(&path).unwrap();
    let mut file = second_file();
    // The parent waits for this line before it starts its stopwatch, so the
    // kill lands during a write rather than during a start-up.
    println!("ready");
    std::io::stdout().flush().unwrap();
    loop {
        // What the parent reads off the corpse: whether this process was
        // inside a save when it was killed. A plain write is enough — the
        // process dies, the machine does not, so what is in the page cache is
        // what the next reader sees.
        std::fs::write(&mark, INSIDE_A_SAVE).unwrap();
        store.save(&mut file).unwrap();
        std::fs::write(&mark, "between saves").unwrap();
    }
}

/// What the child writes into its marker file while a save is in flight.
const INSIDE_A_SAVE: &str = "inside a save";

#[test]
fn a_process_killed_mid_write_leaves_the_last_committed_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");

    let mut first = saveable_file();
    let mut store = ShowStore::open(&path).unwrap();
    store.save(&mut first).unwrap();
    // Dropping the connection is not part of the test — it is so that the
    // child is the only writer, as a second daemon would never be.
    drop(store);

    let first = bytes(&first);
    let second = bytes(&second_file());
    assert_ne!(first, second);

    let mark = directory.path().join("mark.txt");
    let mut wrote_the_second = 0;
    let mut died_writing = 0;
    let mut largest_log = 0;
    let mut kills = 0;
    // **The window this is aiming at is narrow, so there are a lot of shots.**
    // A save of this show takes a fraction of a millisecond and the marker file
    // is written on either side of it, so a kill lands *inside* one only some
    // of the time — and how often depends entirely on how fast the machine's
    // disk is. Six delays were enough here for five sessions and then a CI
    // runner took all six between saves, at which point the test asserted that
    // it had proved nothing, which is exactly what it should do.
    //
    // So: twenty-four delays rather than six, spread finely, and the loop
    // **stops as soon as it has seen all three things it is looking for** —
    // usually after three or four kills. Running the whole list is the unlucky
    // case rather than the ordinary one, and twenty-four consecutive misses is
    // a real finding about the store rather than about the runner.
    let delays = [
        17, 43, 71, 113, 149, 211, 3, 29, 57, 91, 131, 179, 7, 23, 37, 61, 83, 101, 127, 163, 197,
        233, 13, 47,
    ];
    for delay in delays {
        kills += 1;
        let mut child = OsCommand::new(std::env::current_exe().unwrap())
            .args([
                "a_writer_that_saves_until_it_is_killed",
                "--exact",
                "--ignored",
                "--nocapture",
            ])
            .env("PRISM_CRASH_SHOW", &path)
            .env("PRISM_CRASH_MARK", &mark)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        for line in BufReader::new(child.stdout.take().unwrap()).lines() {
            if line.unwrap().trim() == "ready" {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(delay));
        // No unwinding, no destructors, no `sqlite3_close`: this is what
        // pulling the power out looks like to the file.
        child.kill().unwrap();
        child.wait().unwrap();

        // What the child was doing when it died, and how much it had written
        // that no reader will ever see: the two pieces of evidence that this
        // is a crash during a write rather than a tidy shutdown.
        died_writing +=
            usize::from(std::fs::read_to_string(&mark).is_ok_and(|state| state == INSIDE_A_SAVE));
        let log = path.with_file_name("aula.prism-wal");
        largest_log = largest_log.max(log.metadata().map(|meta| meta.len()).unwrap_or(0));

        let store = ShowStore::open(&path).unwrap();
        store.integrity_check().unwrap();
        let mut reopened = ShowFile::new();
        store.load(&mut reopened).unwrap();
        let found = bytes(&reopened);
        assert!(
            found == first || found == second,
            "the file is neither show but a mixture of the two, \
             which is what a write without a transaction produces"
        );
        wrote_the_second += usize::from(found == second);
        drop(store);

        // Everything this test is about has been seen at least once. Going on
        // would only cost a second per kill.
        if died_writing > 0 && wrote_the_second > 0 && largest_log > 0 {
            break;
        }
    }

    println!(
        "crash test: {kills} kills, {died_writing} inside a save, {wrote_the_second} \
         after a commit, largest write-ahead log {largest_log} bytes"
    );
    assert!(
        died_writing > 0,
        "none of {kills} kills landed inside a save, so nothing about a crash \
         was tested"
    );
    assert!(
        wrote_the_second > 0,
        "no kill landed after a commit, so nothing about committed state was tested"
    );
    assert!(
        largest_log > 0,
        "no write-ahead log survived a kill, so nothing was in flight"
    );
}

// -- migration ------------------------------------------------------------

#[test]
fn a_version_one_file_migrates_to_version_two() {
    // The fixture is checked in and frozen: a migration tested against a file
    // the current code wrote is a migration tested against itself. It is
    // copied first, because opening it is what migrates it and the file in the
    // repository has to stay a version-1 file.
    let original = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("version-1.prism");
    let before = std::fs::read(&original).unwrap();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("migrated.prism");
    std::fs::copy(&original, &path).unwrap();

    let store = ShowStore::open(&path).unwrap();
    assert_eq!(store.version(), ShowStore::FORMAT_VERSION);
    assert_eq!(ShowStore::FORMAT_VERSION, 2);

    let mut file = ShowFile::new();
    store.load(&mut file).unwrap();
    // The show is the one the version-1 file was written with, in full.
    assert_eq!(file.show.fixtures().count(), 4);
    assert_eq!(file.show.fixture_types().count(), 3);
    assert_eq!(
        file.show.fixture(FixtureId::new(2)).unwrap().position,
        Vec3 {
            x: -3.75,
            y: 4.5,
            z: -2.25
        }
    );
    assert_eq!(
        file.show.sequence(SequenceId::new(5)).unwrap().cues.len(),
        2
    );
    // The level a version-1 file wrote on its executor is the cue list's now —
    // `ShowStore::carry_levels_onto_their_cue_lists`, which is the half a
    // `serde(default)` on the sequence could not do.
    assert_eq!(
        file.show.sequence(SequenceId::new(5)).unwrap().master_level,
        40000
    );
    // And the half version 1 had nowhere to put is the session a fresh desk
    // starts in, rather than a file that will not open.
    assert_eq!(file.session, prism_core::SessionState::new());
    assert!(!file.is_dirty());

    // It is a version-2 file now, and stays one.
    drop(store);
    let store = ShowStore::open(&path).unwrap();
    assert_eq!(store.version(), 2);
    assert_eq!(
        std::fs::read(&original).unwrap(),
        before,
        "the fixture was migrated in place"
    );
}

#[test]
fn a_file_that_is_not_a_show_is_not_opened_as_one() {
    // The two other ways this goes wrong — a database that is somebody else's
    // file, and one written by a newer PrismDMX — need the file format itself
    // and are asserted in `store.rs`'s own tests.
    let directory = tempfile::tempdir().unwrap();
    let text = directory.path().join("notes.prism");
    std::fs::write(&text, b"this is not a database, it is a note to self").unwrap();
    assert!(matches!(
        ShowStore::open(&text),
        Err(StoreError::Database(_))
    ));
}

// -- JSON export and import -----------------------------------------------

#[test]
fn an_exported_show_can_be_imported_again() {
    let file = saveable_file();
    let text = export_json(&file).unwrap();

    // It is text a person can read and a version control system can diff —
    // which is the whole reason this format exists beside the file.
    assert!(text.contains("\"generic.head\""));
    assert!(text.contains("\"Aula Warm\""));
    assert!(text.contains(&format!("\"version\": {}", ShowStore::FORMAT_VERSION)));

    let imported = import_json(&text).unwrap();
    assert_eq!(bytes(&imported), bytes(&file));
    // An import is a new file: there is nothing to undo and nothing in the
    // programmer, and it has never been saved anywhere.
    assert!(imported.journal.is_empty());
    assert!(imported.programmer.state().values.is_empty());
}

#[test]
fn an_export_is_not_the_authoritative_format_and_says_so() {
    // S1's finding: `serde_json` writes the shortest text that round-trips
    // through a *correctly rounded* parser and does not have one, so a float
    // can come back a unit in the last place away. The `.prism` file is
    // MessagePack for that reason, and this is the assertion that the two
    // formats are not interchangeable claims.
    let mut file = saveable_file();
    let mut fixture = file.show.fixture(FixtureId::new(1)).unwrap().clone();
    fixture.position.x = 0.1 + 0.2;
    file.show.patch_fixture(fixture).unwrap();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");
    let mut store = ShowStore::open(&path).unwrap();
    let expected = bytes(&file);
    store.save(&mut file).unwrap();
    let mut reopened = ShowFile::new();
    store.load(&mut reopened).unwrap();
    assert_eq!(bytes(&reopened), expected, "the file is exact");

    let through_json = import_json(&export_json(&file).unwrap()).unwrap();
    let x = through_json
        .show
        .fixture(FixtureId::new(1))
        .unwrap()
        .position
        .x;
    // Not asserted to be *unequal* — that would be an assertion about a bug in
    // somebody else's parser. What is asserted is the promise this crate
    // actually makes about the export: the value is within one ULP.
    assert!(
        (x - (0.1 + 0.2)).abs() <= f64::EPSILON,
        "the export moved a position by more than a unit in the last place"
    );
}

#[test]
fn a_version_one_export_imports_with_a_fresh_session() {
    let file = saveable_file();
    let text = export_json(&file).unwrap();
    let mut document: serde_json::Value = serde_json::from_str(&text).unwrap();
    document["version"] = serde_json::json!(1);
    document.as_object_mut().unwrap().remove("session");

    let imported = import_json(&document.to_string()).unwrap();
    assert_eq!(imported.session, prism_core::SessionState::new());
    assert_eq!(
        rmp_serde::to_vec_named(&imported.show).unwrap(),
        rmp_serde::to_vec_named(&file.show).unwrap()
    );
}

#[test]
fn an_import_refuses_what_it_cannot_vouch_for() {
    // Not an export at all.
    assert!(matches!(import_json("{}"), Err(StoreError::Damaged(_))));
    assert!(matches!(
        import_json("nonsense"),
        Err(StoreError::Damaged(_))
    ));

    let text = export_json(&saveable_file()).unwrap();
    let mut document: serde_json::Value = serde_json::from_str(&text).unwrap();
    document["version"] = serde_json::json!(99);
    assert_eq!(
        import_json(&document.to_string()),
        Err(StoreError::FutureVersion {
            found: 99,
            supported: ShowStore::FORMAT_VERSION,
        })
    );

    // A float that is not a number is refused on the way in, exactly as S1
    // asked: `prism-domain` guards every `f64` in both directions. It has to be
    // written as text, because there is no `serde_json::Value` that holds an
    // infinity — which is S1's finding from the other side.
    let mut file = saveable_file();
    let mut fixture = file.show.fixture(FixtureId::new(1)).unwrap().clone();
    fixture.position.x = 123_456.75;
    file.show.patch_fixture(fixture).unwrap();
    let tampered = export_json(&file).unwrap().replace("123456.75", "1e999");
    assert!(tampered.contains("1e999"));
    assert!(matches!(
        import_json(&tampered),
        Err(StoreError::Damaged(_))
    ));
}

// -- the broad net --------------------------------------------------------

proptest! {
    /// Whatever the show model accepts, the platter gives back unchanged.
    ///
    /// The hand-written round trip above asserts one carefully non-default
    /// show; this asserts the shape of the claim over arbitrary content —
    /// names with newlines and quotation marks in them, positions anywhere in
    /// the finite float range, cue lists of every length. Only the fields that
    /// make a show *legal* are fixed: the profile a fixture instantiates, the
    /// address it sits at, and the fixtures a cue part may name.
    #[test]
    fn any_show_the_model_accepts_survives_the_platter(
        fixtures in prop::collection::vec(any::<Fixture>(), 1..4),
        sequence in any::<Sequence>(),
        line in ".*",
    ) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("property.prism");

        let mut file = ShowFile::new();
        file.show.embed_fixture_type(par_type()).unwrap();
        for (index, fixture) in fixtures.into_iter().enumerate() {
            let mut fixture = fixture;
            let index = u16::try_from(index).unwrap();
            fixture.id = FixtureId::new(u32::from(index) + 1);
            fixture.type_id = "generic.rgbw.par".to_owned();
            fixture.universe = UniverseId::new(1);
            fixture.address = index * 4 + 1;
            file.show.patch_fixture(fixture).unwrap();
        }
        let patched = u32::try_from(file.show.fixtures().count()).unwrap();
        let mut sequence = sequence;
        for (index, cue) in sequence.cues.iter_mut().enumerate() {
            cue.number = (index + 1).to_string();
            for part in &mut cue.parts {
                part.fixture = FixtureId::new(part.fixture.get() % patched + 1);
                part.preset_ref = None;
            }
        }
        file.show.store_sequence(sequence).unwrap();
        file.session.set_command_line(&line, false).unwrap();

        let expected = bytes(&file);
        let mut store = ShowStore::open(&path).unwrap();
        store.save(&mut file).unwrap();
        let mut reopened = ShowFile::new();
        store.load(&mut reopened).unwrap();
        prop_assert_eq!(bytes(&reopened), expected);
    }
}

/// The show half of a save is the same show the mirror would have built, which
/// is the check that persistence and delta generation describe one model.
#[test]
fn what_is_written_is_what_a_client_would_have_mirrored() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("aula.prism");

    let mut file = ShowFile::new();
    let mut mirror = prism_core::ShowMirror::new(file.show.to_json().unwrap());
    let mut deltas = file
        .show
        .embed_fixture_type(par_type())
        .map(|ops| vec![Delta::ShowPatch { ops }])
        .unwrap();
    for id in 1..=3u32 {
        let address = u16::try_from((id - 1) * 4 + 1).unwrap();
        deltas.extend(
            file.apply(&common::patch_command(id, 1, address))
                .unwrap()
                .deltas,
        );
    }
    mirror.apply_all(&show_patch_ops(&deltas)).unwrap();

    let mut store = ShowStore::open(&path).unwrap();
    store.save(&mut file).unwrap();
    let mut reopened = ShowFile::new();
    store.load(&mut reopened).unwrap();

    assert_eq!(reopened.show.to_json().unwrap(), mirror.into_value());
}
