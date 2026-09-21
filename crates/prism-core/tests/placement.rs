//! Where a fixture hangs — `Command::PlaceFixtures`, **S30**.
//!
//! Four claims, and each is a property the 3D viewer leans on:
//!
//! - a place is written as two `replace`s and **nothing** else, so a client's
//!   mirror moves exactly the two fields and the patch revision does not move;
//! - the engine is not told: no `Effect::Repatch`, forward **or** backward,
//!   because a head moved in the view must cost the tick nothing;
//! - a refusal refuses the whole gesture — half a spread is a rig nobody asked
//!   for;
//! - one gesture is one Oops, and the Oops restores the bytes.

mod common;

use common::populated_show;
use prism_core::{Effect, ShowError, ShowFile, ShowFileError, UndoScope};
use prism_domain::{Command, Delta, FixtureId, FixturePlace, JsonPatchOp, MAX_REACH, Vec3};

fn file() -> ShowFile {
    ShowFile {
        show: populated_show(),
        ..ShowFile::new()
    }
}

fn place(id: u32, position: Vec3, rotation: Vec3) -> FixturePlace {
    FixturePlace {
        id: FixtureId::new(id),
        position,
        rotation,
    }
}

fn spread() -> Command {
    Command::PlaceFixtures {
        placements: vec![
            place(1, Vec3::new(-1.0, 6.0, 2.0), Vec3::new(20.0, 0.0, 0.0)),
            place(2, Vec3::new(1.0, 6.0, 2.0), Vec3::new(20.0, 0.0, 0.0)),
        ],
    }
}

/// The show's bytes, which is what an Oops has to put back.
fn bytes(file: &ShowFile) -> Vec<u8> {
    rmp_serde::to_vec_named(&file.show).unwrap()
}

#[test]
fn a_place_is_two_replaces_and_moves_nothing_the_engine_reads() {
    let mut file = file();
    let revision = file.show.patch_revision();
    let applied = file.apply(&spread()).unwrap();

    assert!(
        !applied.effects.contains(&Effect::Repatch),
        "a placement repatched: {:?}",
        applied.effects
    );
    assert_eq!(
        file.show.patch_revision(),
        revision,
        "the patch revision moved"
    );

    let ops: Vec<&JsonPatchOp> = applied
        .deltas
        .iter()
        .flat_map(|delta| match delta {
            Delta::ShowPatch { ops } => ops.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect();
    let paths: Vec<&str> = ops
        .iter()
        .map(|op| match op {
            JsonPatchOp::Replace { path, .. } => path.as_str(),
            other => panic!("a placement wrote {other:?}"),
        })
        .collect();
    assert_eq!(
        paths,
        [
            "/fixtures/1/position",
            "/fixtures/1/rotation",
            "/fixtures/2/position",
            "/fixtures/2/rotation",
        ]
    );

    let hung = file.show.fixture(FixtureId::new(2)).unwrap();
    assert_eq!(hung.position, Vec3::new(1.0, 6.0, 2.0));
    assert_eq!(hung.rotation, Vec3::new(20.0, 0.0, 0.0));
    assert!(file.is_dirty(), "a moved rig is a show to save");
}

#[test]
fn only_what_changed_is_written() {
    let mut file = file();
    file.apply(&spread()).unwrap();
    let applied = file
        .apply(&Command::PlaceFixtures {
            placements: vec![place(
                1,
                Vec3::new(-1.0, 6.0, 2.0),
                Vec3::new(20.0, 90.0, 0.0),
            )],
        })
        .unwrap();
    let Some(Delta::ShowPatch { ops }) = applied.deltas.first() else {
        panic!("a turn is a delta: {applied:?}");
    };
    assert_eq!(ops.len(), 1, "{ops:?}");
    assert!(matches!(&ops[0], JsonPatchOp::Replace { path, .. } if path == "/fixtures/1/rotation"));
}

#[test]
fn a_place_it_already_has_is_not_a_step() {
    let mut file = file();
    file.apply(&spread()).unwrap();
    let steps = file.journal.len();
    let applied = file.apply(&spread()).unwrap();
    assert!(applied.deltas.is_empty(), "{applied:?}");
    assert_eq!(
        file.journal.len(),
        steps,
        "nothing moved, and Oops has nothing to take back"
    );
}

#[test]
fn a_fixture_that_is_not_patched_refuses_the_whole_gesture() {
    let mut file = file();
    let before = bytes(&file);
    let refused = file.apply(&Command::PlaceFixtures {
        placements: vec![
            place(1, Vec3::new(0.0, 5.0, 0.0), Vec3::ZERO),
            place(999, Vec3::new(0.0, 5.0, 0.0), Vec3::ZERO),
        ],
    });
    assert!(
        matches!(
            refused,
            Err(ShowFileError::Show(ShowError::UnknownFixture(id))) if id == FixtureId::new(999)
        ),
        "{refused:?}"
    );
    assert_eq!(
        bytes(&file),
        before,
        "fixture 1 moved although the gesture was refused"
    );
}

#[test]
fn a_place_off_the_stage_is_refused_by_name() {
    let mut file = file();
    let before = bytes(&file);
    for position in [
        Vec3::new(0.0, MAX_REACH * 6.0, 0.0),
        Vec3::new(f64::NAN, 0.0, 0.0),
    ] {
        let refused = file.apply(&Command::PlaceFixtures {
            placements: vec![
                place(1, Vec3::new(0.0, 5.0, 0.0), Vec3::ZERO),
                place(2, position, Vec3::ZERO),
            ],
        });
        assert!(
            matches!(
                refused,
                Err(ShowFileError::Show(ShowError::FixtureOutOfReach(id))) if id == FixtureId::new(2)
            ),
            "{refused:?}"
        );
    }
    assert_eq!(bytes(&file), before);
}

#[test]
fn one_gesture_is_one_oops_and_neither_way_repatches() {
    let mut file = file();
    let before = bytes(&file);
    file.apply(&spread()).unwrap();
    let after = bytes(&file);

    let record = file.journal.undoable().expect("a placement is a step");
    assert_eq!(
        record.scope(),
        vec![
            UndoScope::Place(FixtureId::new(1)),
            UndoScope::Place(FixtureId::new(2)),
        ]
    );

    let undone = file.apply(&Command::Oops).unwrap();
    assert_eq!(bytes(&file), before, "the Oops did not put the rig back");
    assert!(
        !undone.effects.contains(&Effect::Repatch),
        "an Oops over a placement repatched: {:?}",
        undone.effects
    );

    let redone = file.apply(&Command::Redo).unwrap();
    assert_eq!(bytes(&file), after);
    assert!(!redone.effects.contains(&Effect::Repatch));
}

#[test]
fn a_repatch_keeps_the_place() {
    // S11's rule, which S30 is the first session to be able to see: a
    // corrected address must not move a hung head back to the origin.
    let mut file = file();
    file.apply(&spread()).unwrap();
    file.apply(&common::patch_command(1, 1, 101)).unwrap();
    let fixture = file.show.fixture(FixtureId::new(1)).unwrap();
    assert_eq!(fixture.address, 101);
    assert_eq!(fixture.position, Vec3::new(-1.0, 6.0, 2.0));
    assert_eq!(fixture.rotation, Vec3::new(20.0, 0.0, 0.0));
}
