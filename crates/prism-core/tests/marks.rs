//! Stage marks - `Command::EditMarks`, **B70**.
//!
//! The reference points a head is calibrated against are the show's, so they
//! are held to what a show's pools are held to:
//!
//! - an edit is written as `add`, `replace` and `remove` operations on
//!   `/marks/N` and nothing else, so a client's mirror moves exactly those;
//! - the engine is never told, and the patch does not move;
//! - a refusal refuses the whole gesture;
//! - one gesture is one Oops, in both directions;
//! - a mark survives the file, exactly.

mod common;

use common::populated_show;
use prism_core::{Effect, ShowError, ShowFile, ShowFileError, ShowStore, UndoScope};
use prism_domain::{Command, Delta, JsonPatchOp, MAX_MARK_NAME, MAX_MARKS, Mark, MarkId, Vec3};

fn file() -> ShowFile {
    ShowFile {
        show: populated_show(),
        ..ShowFile::new()
    }
}

fn mark(id: u32, name: &str, position: Vec3) -> Mark {
    Mark {
        id: MarkId::new(id),
        name: name.to_owned(),
        position,
    }
}

fn edit(set: Vec<Mark>, remove: Vec<u32>) -> Command {
    Command::EditMarks {
        set,
        remove: remove.into_iter().map(MarkId::new).collect(),
    }
}

fn bytes(file: &ShowFile) -> Vec<u8> {
    rmp_serde::to_vec_named(&file.show).unwrap()
}

fn ops(applied: &prism_core::Applied) -> Vec<&JsonPatchOp> {
    applied
        .deltas
        .iter()
        .flat_map(|delta| match delta {
            Delta::ShowPatch { ops } => ops.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn describe(op: &JsonPatchOp) -> String {
    match op {
        JsonPatchOp::Add { path, .. } => format!("add {path}"),
        JsonPatchOp::Replace { path, .. } => format!("replace {path}"),
        JsonPatchOp::Remove { path } => format!("remove {path}"),
        other => format!("{other:?}"),
    }
}

/// The three operations a mark edit can produce, applied to a JSON value the
/// way the interface applies them.
fn apply_patch(document: &mut serde_json::Value, op: &JsonPatchOp) {
    let (path, value) = match op {
        JsonPatchOp::Add { path, value } | JsonPatchOp::Replace { path, value } => {
            (path, Some(serde_json::to_value(value).unwrap()))
        }
        JsonPatchOp::Remove { path } => (path, None),
        other => panic!("a mark edit wrote {other:?}"),
    };
    let key = path.strip_prefix("/marks/").expect("a mark path");
    let marks = document
        .get_mut("marks")
        .and_then(serde_json::Value::as_object_mut)
        .expect("the document has a marks object");
    match (op, value) {
        (JsonPatchOp::Replace { .. }, Some(value)) => {
            assert!(
                marks.contains_key(key),
                "replace of a member that is not there: {path}"
            );
            marks.insert(key.to_owned(), value);
        }
        (_, Some(value)) => {
            marks.insert(key.to_owned(), value);
        }
        (_, None) => {
            assert!(
                marks.remove(key).is_some(),
                "remove of a member that is not there: {path}"
            );
        }
    }
}

#[test]
fn a_mark_is_added_changed_and_removed_as_one_member_of_the_marks_object() {
    let mut file = file();
    let revision = file.show.patch_revision();

    let applied = file
        .apply(&edit(
            vec![
                mark(1, "DS centre", Vec3::new(0.0, 0.0, -3.0)),
                mark(2, "", Vec3::new(2.0, 0.0, 1.0)),
            ],
            vec![],
        ))
        .unwrap();
    assert_eq!(
        ops(&applied).into_iter().map(describe).collect::<Vec<_>>(),
        ["add /marks/1", "add /marks/2"]
    );
    assert!(applied.effects.is_empty(), "{:?}", applied.effects);
    assert_eq!(
        file.show.patch_revision(),
        revision,
        "a mark is not a patch"
    );
    assert!(file.is_dirty());

    let applied = file
        .apply(&edit(
            vec![mark(1, "DS centre", Vec3::new(0.0, 0.0, -3.5))],
            vec![2],
        ))
        .unwrap();
    assert_eq!(
        ops(&applied).into_iter().map(describe).collect::<Vec<_>>(),
        ["remove /marks/2", "replace /marks/1"]
    );
    assert_eq!(file.show.stage_marks().count(), 1);
    assert_eq!(
        file.show.stage_mark(MarkId::new(1)).unwrap().position,
        Vec3::new(0.0, 0.0, -3.5)
    );
}

#[test]
fn the_document_a_client_holds_ends_up_as_the_show() {
    let mut file = file();
    let mut document = serde_json::to_value(&file.show).unwrap();
    for command in [
        edit(
            vec![
                mark(3, "Spike 3", Vec3::new(1.0, 0.0, 1.0)),
                mark(4, "Spike 4", Vec3::new(-1.0, 0.0, 1.0)),
            ],
            vec![],
        ),
        edit(vec![mark(3, "Spike 3b", Vec3::new(1.5, 0.0, 1.0))], vec![4]),
        // The same number removed and set in one gesture.
        edit(vec![mark(3, "Spike 3c", Vec3::new(1.5, 0.0, 2.0))], vec![3]),
        // And a number in both that was never there.
        edit(vec![mark(9, "nine", Vec3::ZERO)], vec![9]),
    ] {
        let applied = file.apply(&command).unwrap();
        for op in ops(&applied) {
            apply_patch(&mut document, op);
        }
    }
    assert_eq!(document, serde_json::to_value(&file.show).unwrap());
}

#[test]
fn a_mark_set_to_what_it_is_and_a_removal_of_nothing_are_not_steps() {
    let mut file = file();
    let here = mark(1, "X", Vec3::new(1.0, 0.0, 1.0));
    file.apply(&edit(vec![here.clone()], vec![])).unwrap();
    let steps = file.journal.len();
    let applied = file.apply(&edit(vec![here], vec![99])).unwrap();
    assert!(applied.deltas.is_empty(), "{applied:?}");
    assert_eq!(file.journal.len(), steps);
}

#[test]
fn a_mark_that_is_not_valid_refuses_the_whole_gesture() {
    let mut file = file();
    file.apply(&edit(vec![mark(1, "keep", Vec3::ZERO)], vec![]))
        .unwrap();
    let before = bytes(&file);
    for bad in [
        mark(2, &"n".repeat(MAX_MARK_NAME + 1), Vec3::ZERO),
        mark(2, "far", Vec3::new(0.0, 0.0, 5_000.0)),
        mark(2, "nan", Vec3::new(f64::NAN, 0.0, 0.0)),
    ] {
        let refused = file.apply(&edit(vec![mark(3, "fine", Vec3::ZERO), bad], vec![1]));
        assert!(
            matches!(
                refused,
                Err(ShowFileError::Show(ShowError::InvalidMark(id))) if id == MarkId::new(2)
            ),
            "{refused:?}"
        );
        assert_eq!(bytes(&file), before, "a refused gesture changed the show");
    }
}

#[test]
fn a_show_holds_no_more_marks_than_the_limit() {
    let mut file = file();
    let many: Vec<Mark> = (1..=u32::try_from(MAX_MARKS).unwrap())
        .map(|id| mark(id, "", Vec3::ZERO))
        .collect();
    file.apply(&edit(many, vec![])).unwrap();
    let before = bytes(&file);
    let refused = file.apply(&edit(
        vec![mark(100_000, "one too many", Vec3::ZERO)],
        vec![],
    ));
    assert!(
        matches!(refused, Err(ShowFileError::Show(ShowError::TooManyMarks))),
        "{refused:?}"
    );
    assert_eq!(bytes(&file), before);
    // But the same gesture that takes one away may add one.
    file.apply(&edit(vec![mark(100_000, "swap", Vec3::ZERO)], vec![1]))
        .unwrap();
}

#[test]
fn one_gesture_is_one_oops_and_neither_way_repatches() {
    let mut file = file();
    file.apply(&edit(
        vec![
            mark(1, "a", Vec3::ZERO),
            mark(2, "b", Vec3::new(1.0, 0.0, 0.0)),
        ],
        vec![],
    ))
    .unwrap();
    let before = bytes(&file);
    file.apply(&edit(vec![mark(1, "a", Vec3::new(0.0, 0.0, 4.0))], vec![2]))
        .unwrap();
    let after = bytes(&file);

    let record = file.journal.undoable().expect("a mark edit is a step");
    assert_eq!(
        record.scope(),
        vec![
            UndoScope::Mark(MarkId::new(1)),
            UndoScope::Mark(MarkId::new(2))
        ]
    );

    let undone = file.apply(&Command::Oops).unwrap();
    assert_eq!(bytes(&file), before, "the Oops did not put the marks back");
    assert!(
        !undone.effects.contains(&Effect::Repatch),
        "{:?}",
        undone.effects
    );

    file.apply(&Command::Redo).unwrap();
    assert_eq!(bytes(&file), after, "the Redo did not do it again");
}

#[test]
fn oops_takes_the_first_mark_back_out_of_the_document() {
    let mut file = file();
    let mut document = serde_json::to_value(&file.show).unwrap();
    for command in [
        edit(vec![mark(7, "seven", Vec3::ZERO)], vec![]),
        Command::Oops,
        Command::Redo,
        Command::Oops,
    ] {
        let applied = file.apply(&command).unwrap();
        for op in ops(&applied) {
            apply_patch(&mut document, op);
        }
    }
    assert_eq!(file.show.stage_marks().count(), 0);
    assert_eq!(document, serde_json::to_value(&file.show).unwrap());
}

#[test]
fn a_mark_survives_the_file_to_the_last_bit() {
    let directory = tempfile::tempdir().unwrap();
    let mut file = file();
    let place = Vec3::new(0.1 + 0.2, -1.0 / 3.0, 2.0_f64.sqrt());
    file.apply(&edit(vec![mark(5, "Fünf", place)], vec![]))
        .unwrap();
    let mut store = ShowStore::open(directory.path().join("marks.prism")).unwrap();
    store.save(&mut file).unwrap();

    let back = ShowStore::open(directory.path().join("marks.prism"))
        .unwrap()
        .read()
        .unwrap();
    let held = back.show.stage_mark(MarkId::new(5)).unwrap();
    assert_eq!(held.name, "Fünf");
    assert_eq!(held.position, place, "a float came back a unit away");
}

#[test]
fn a_mark_survives_the_json_export() {
    let mut file = file();
    file.apply(&edit(
        vec![mark(5, "Five", Vec3::new(1.5, 0.0, -2.25))],
        vec![],
    ))
    .unwrap();
    let text = prism_core::export_json(&file).unwrap();
    let back = prism_core::import_json(&text).unwrap();
    assert_eq!(back.show.stage_marks().count(), 1);
    // And an export from before there were marks still imports.
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["show"].as_object_mut().unwrap().remove("marks");
    let old = prism_core::import_json(&value.to_string()).unwrap();
    assert_eq!(old.show.stage_marks().count(), 0);
}
