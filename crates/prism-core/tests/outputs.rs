//! The output patch as a **fourth model with a third applier** — S33.
//!
//! `src/outputs.rs` holds the rules. This target holds the two claims that are
//! about how the rig sits beside the other three models, and both are asserted
//! on **bytes** rather than on the models:
//!
//! - a rig never reaches a `.prism` file, however it is edited and however often
//!   the show is saved;
//! - a machine command reaching `ShowFile::apply` is refused, changes nothing
//!   and journals nothing — so an Oops can never take back a change to the
//!   venue's cabling.
//!
//! `crates/prism-core/src/desk.rs` makes the first claim structurally
//! (`outputs_are_not_show_content`); this makes it through the door a daemon
//! actually uses.

mod common;

use common::{machine_commands, populated_show};
use prism_core::{MachineConfig, ShowError, ShowFile, ShowStore};
use prism_domain::{
    Command, FixtureId, OutputId, OutputInstance, OutputKind, SelectionMode, UniverseId,
};

fn rig() -> Vec<OutputInstance> {
    vec![
        OutputInstance::new(
            OutputId::new(1),
            "Hall dimmers",
            OutputKind::OpenDmx {
                serial: Some("B0037HIY".to_owned()),
            },
            [UniverseId::new(1)],
        ),
        OutputInstance::new(
            OutputId::new(2),
            "Stage left node",
            OutputKind::ArtNet {
                nodes: vec!["192.168.1.50:6454".parse().unwrap()],
                sync: false,
                ports: Vec::new(),
            },
            [UniverseId::new(5), UniverseId::new(6)],
        ),
    ]
}

/// A show saved on a desk with a rig on it opens on a desk with none — because
/// there is nothing of the rig in the file to open.
#[test]
fn a_rig_is_never_written_into_a_show_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aula.prism");

    let mut machine = MachineConfig::default();
    for output in rig() {
        machine.apply(&Command::AddOutput { output }).unwrap();
    }
    assert_eq!(machine.outputs().len(), 2);

    let mut file = ShowFile {
        show: populated_show(),
        ..ShowFile::new()
    };
    let mut store = ShowStore::open(&path).unwrap();
    store.save(&mut file).unwrap();
    // Saved twice, because "the second save is the one that leaks" has been the
    // shape of this class of defect before.
    file.apply(&Command::ClearProgrammer).unwrap();
    store.save(&mut file).unwrap();

    let bytes = std::fs::read(&path).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    for word in [
        "Hall dimmers",
        "Stage left node",
        "192.168.1.50",
        "B0037HIY",
        "OpenDmx",
        "ArtNet",
    ] {
        assert!(!text.contains(word), "the show file carries {word:?}");
    }

    // And the show that comes back out has no idea a rig ever existed.
    let mut reopened = ShowFile::new();
    ShowStore::open(&path).unwrap().load(&mut reopened).unwrap();
    let reopened_bytes = rmp_serde::to_vec_named(&reopened).unwrap();
    assert!(
        !String::from_utf8_lossy(&reopened_bytes).contains("Hall dimmers"),
        "the reopened show learned the first hall's cabling"
    );
}

/// A machine command that reaches the show file is refused, and refused
/// **without a trace**: the show is byte-identical, the session is
/// byte-identical, and the journal has nothing in it for an Oops to find.
#[test]
fn a_machine_command_at_the_show_files_door_is_refused_and_journals_nothing() {
    for command in machine_commands() {
        let mut file = ShowFile {
            show: populated_show(),
            ..ShowFile::new()
        };
        // Something undoable first, so that "the journal is empty" is a claim
        // about *this* command rather than about a journal that was never used.
        //
        // A selection rather than a Clear since **S43**: a Clear with nothing to
        // clear is no longer an edit at all (B2), so it would journal nothing
        // and this test would be asserting about an empty journal either way.
        file.apply(&Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        })
        .unwrap();
        let undoable_before = file.journal.len();
        assert!(undoable_before > 0);

        let before = rmp_serde::to_vec_named(&file).unwrap();
        let error = file.apply(&command).unwrap_err();
        assert_eq!(
            error,
            prism_core::ShowFileError::Show(ShowError::NotAShowCommand),
            "{command:?}"
        );
        assert_eq!(
            rmp_serde::to_vec_named(&file).unwrap(),
            before,
            "{command:?} changed the file on its way to being refused"
        );
        // The journal is exactly where it was: nothing filed, nothing taken
        // away. `Command::is_undoable` says the same thing in the domain.
        assert!(!command.is_undoable(), "{command:?}");
        assert_eq!(file.journal.len(), undoable_before);
        assert_eq!(file.journal.redo_len(), 0);
    }
}
