//! S40's four verbs, over the six numbered things a desk has.
//!
//! `Delete`, `Copy`, `Move` and `Label` are one command each rather than
//! twenty-four (`prism_domain::ObjectRef` says why), so this is where *what each
//! of them means, pool by pool* is held to the show rather than to a comment.
//! Every assertion below reads the thing back out of the show; a test that
//! checked the command had been accepted would pass for a command that did the
//! wrong thing.
//!
//! # The two rules it also holds
//!
//! - **A refusal changes nothing**, taken literally: the show is serialised with
//!   `rmp_serde::to_vec_named` before and after and the two `Vec<u8>` compared,
//!   which is the standard `command_application.rs` set in S11.
//! - **A move brings the references along.** Moving a sequence repoints every
//!   executor that played it and moving a preset rewrites every cue part that
//!   linked to it. Both would be silent faults: nothing looks different until
//!   somebody presses Go, or edits the preset and watches the cue not follow.

mod common;

use common::{cue, executor, group, populated_show, preset, sequence};
use prism_core::{Show, ShowError, ShowFile};
use prism_domain::{
    AttributeType, Command, ExecutorId, GroupId, ObjectRef, OverwriteMode, PresetId, SequenceId,
};

/// The show as bytes, for "a refusal changes nothing".
fn bytes(show: &Show) -> Vec<u8> {
    rmp_serde::to_vec_named(show).unwrap()
}

fn seq(id: u32) -> ObjectRef {
    ObjectRef::Sequence {
        sequence_id: SequenceId::new(id),
    }
}

fn cue_ref(sequence: u32, number: &str) -> ObjectRef {
    ObjectRef::Cue {
        sequence_id: Some(SequenceId::new(sequence)),
        cue_number: number.to_owned(),
    }
}

fn group_ref(id: u32) -> ObjectRef {
    ObjectRef::Group {
        group_id: GroupId::new(id),
    }
}

fn preset_ref(id: u32) -> ObjectRef {
    ObjectRef::Preset {
        preset_id: PresetId::new(id),
    }
}

fn executor_ref(id: u32) -> ObjectRef {
    ObjectRef::Executor {
        executor_id: ExecutorId::new(id),
    }
}

/* -------------------------------------------------------------------------- */
/* Delete                                                                     */
/* -------------------------------------------------------------------------- */

#[test]
fn delete_empties_each_of_the_five_show_pools() {
    let mut show = populated_show();
    show.apply(&Command::Delete {
        target: cue_ref(1, "2"),
    })
    .expect("cue 2 is there");
    assert_eq!(
        show.sequence(SequenceId::new(1)).unwrap().cues.len(),
        1,
        "the cue is still in the list"
    );
    // **And the cues after it keep their numbers.** A cue number is what an
    // operator has written on a running order and what a `Goto` names.
    assert_eq!(
        show.sequence(SequenceId::new(1)).unwrap().cues[0].number,
        "1"
    );

    show.apply(&Command::Delete {
        target: group_ref(1),
    })
    .expect("group 1 is there");
    assert!(show.group(GroupId::new(1)).is_none());

    show.apply(&Command::Delete {
        target: preset_ref(4),
    })
    .expect("preset 4 is there");
    assert!(show.preset(PresetId::new(4)).is_none());

    show.apply(&Command::Delete {
        target: executor_ref(0),
    })
    .expect("executor 0 is there");
    assert!(show.executor(ExecutorId::new(0)).is_none());

    show.apply(&Command::Delete { target: seq(1) })
        .expect("sequence 1 is there");
    assert!(show.sequence(SequenceId::new(1)).is_none());
}

/// **A deleted executor leaves its place behind.**
///
/// The row goes out of the show and executor 1 is still executor 1 on the bar,
/// because the eight strips of a page are `page * 8 + slot` arithmetic (**D7**)
/// rather than rows. That is the requirement read literally, and it is also the
/// exact inverse of the `AssignExecutor` that made the row — which is what lets
/// an Oops put the grid back as it was rather than leaving a slot behind with a
/// deleted executor's master and buttons still on it.
#[test]
fn a_deleted_executor_leaves_the_place_and_takes_the_row() {
    let mut show = populated_show();
    let before = show.executors().count();

    show.apply(&Command::Delete {
        target: executor_ref(0),
    })
    .unwrap();

    assert_eq!(show.executors().count(), before - 1);
    assert!(show.executor(ExecutorId::new(0)).is_none());
    // The sequence it played is untouched: deleting a fader is not deleting a
    // show.
    assert!(show.sequence(SequenceId::new(1)).is_some());
}

#[test]
fn deleting_something_that_is_not_there_changes_nothing_at_all() {
    let mut show = populated_show();
    let before = bytes(&show);
    for target in [
        seq(404),
        cue_ref(1, "404"),
        group_ref(404),
        preset_ref(404),
        executor_ref(404),
    ] {
        let refusal = show.apply(&Command::Delete {
            target: target.clone(),
        });
        assert!(refusal.is_err(), "{target:?} was accepted");
        assert_eq!(bytes(&show), before, "{target:?} changed the show");
    }
}

/* -------------------------------------------------------------------------- */
/* Label                                                                      */
/* -------------------------------------------------------------------------- */

#[test]
fn label_names_each_pool_and_an_executor_names_its_cue_list() {
    let mut show = populated_show();
    for (target, read) in [
        (seq(1), "sequence"),
        (group_ref(1), "group"),
        (preset_ref(4), "preset"),
    ] {
        show.apply(&Command::Label {
            target: target.clone(),
            name: "Named".to_owned(),
        })
        .unwrap_or_else(|error| panic!("{read}: {error}"));
    }
    assert_eq!(show.sequence(SequenceId::new(1)).unwrap().name, "Named");
    assert_eq!(show.group(GroupId::new(1)).unwrap().name, "Named");
    assert_eq!(show.preset(PresetId::new(4)).unwrap().name, "Named");

    show.apply(&Command::Label {
        target: cue_ref(1, "1"),
        name: "Blackout".to_owned(),
    })
    .unwrap();
    assert_eq!(
        show.sequence(SequenceId::new(1)).unwrap().cues[0].name,
        "Blackout"
    );

    // **An executor has no name of its own**, so this names the cue list on it —
    // which is what the scribble strip shows, and what an operator means.
    show.apply(&Command::Label {
        target: executor_ref(0),
        name: "On the fader".to_owned(),
    })
    .unwrap();
    assert_eq!(
        show.sequence(SequenceId::new(1)).unwrap().name,
        "On the fader"
    );

    // And an empty slot is refused rather than silently doing nothing.
    let refusal = show.apply(&Command::Label {
        target: executor_ref(1),
        name: "Nowhere".to_owned(),
    });
    assert!(matches!(refusal, Err(ShowError::ExecutorHasNoSequence(_))));
}

#[test]
fn a_label_that_changes_nothing_says_nothing() {
    let mut show = populated_show();
    let name = show.group(GroupId::new(1)).unwrap().name.clone();
    let applied = show
        .apply(&Command::Label {
            target: group_ref(1),
            name,
        })
        .unwrap();
    // An empty `ShowPatch` is a broadcast to every client that says nothing —
    // S27's rule for `RenumberFixture`, one pool along.
    assert!(applied.deltas.is_empty(), "{applied:?}");
}

/* -------------------------------------------------------------------------- */
/* Copy                                                                       */
/* -------------------------------------------------------------------------- */

#[test]
fn copy_leaves_the_source_alone_in_every_pool() {
    let mut show = populated_show();
    for (from, to) in [
        (seq(1), seq(9)),
        (group_ref(1), group_ref(9)),
        (preset_ref(4), preset_ref(9)),
    ] {
        show.apply(&Command::Copy {
            from: from.clone(),
            to: to.clone(),
            mode: OverwriteMode::Merge,
        })
        .unwrap_or_else(|error| panic!("{from:?}: {error}"));
    }
    assert_eq!(
        show.sequence(SequenceId::new(9)).unwrap().cues.len(),
        show.sequence(SequenceId::new(1)).unwrap().cues.len()
    );
    assert_eq!(
        show.group(GroupId::new(9)).unwrap().fixtures,
        show.group(GroupId::new(1)).unwrap().fixtures
    );
    assert_eq!(
        show.preset(PresetId::new(9)).unwrap().values.len(),
        show.preset(PresetId::new(4)).unwrap().values.len()
    );
    // A copy onto a free number takes the source's name, because there is no
    // name there to keep.
    assert_eq!(
        show.sequence(SequenceId::new(9)).unwrap().name,
        show.sequence(SequenceId::new(1)).unwrap().name
    );
}

/// **A copy onto something that exists keeps its name**, because a copy is not
/// a rename — that is `Label`'s.
#[test]
fn a_copy_onto_a_named_thing_leaves_the_name_alone() {
    let mut show = populated_show();
    show.store_group(group(2, &[4])).unwrap();
    show.apply(&Command::Label {
        target: group_ref(2),
        name: "Mine".to_owned(),
    })
    .unwrap();

    show.apply(&Command::Copy {
        from: group_ref(1),
        to: group_ref(2),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    assert_eq!(show.group(GroupId::new(2)).unwrap().name, "Mine");
    // Merge is a union, in the destination's order first.
    let members: Vec<u32> = show
        .group(GroupId::new(2))
        .unwrap()
        .fixtures
        .iter()
        .map(|id| id.get())
        .collect();
    assert_eq!(members, vec![4, 1, 2, 3]);
}

#[test]
fn override_replaces_where_merge_writes_into() {
    let mut show = populated_show();
    show.store_group(group(2, &[4])).unwrap();

    show.apply(&Command::Copy {
        from: group_ref(1),
        to: group_ref(2),
        mode: OverwriteMode::Override,
    })
    .unwrap();

    let members: Vec<u32> = show
        .group(GroupId::new(2))
        .unwrap()
        .fixtures
        .iter()
        .map(|id| id.get())
        .collect();
    assert_eq!(members, vec![1, 2, 3], "an override kept what it replaced");
}

#[test]
fn a_pair_that_is_not_the_same_kind_of_thing_is_refused() {
    let mut show = populated_show();
    let before = bytes(&show);
    let refusal = show.apply(&Command::Copy {
        from: cue_ref(1, "1"),
        to: group_ref(1),
        mode: OverwriteMode::Merge,
    });
    assert!(matches!(refusal, Err(ShowError::MismatchedObjects { .. })));
    assert_eq!(bytes(&show), before);
}

/// A copy or a move onto **itself** is refused rather than treated as a no-op:
/// an operator who typed it meant something else, and silence is the wrong
/// answer to a line that cannot have been meant.
#[test]
fn copying_something_onto_itself_is_refused() {
    let mut show = populated_show();
    let before = bytes(&show);
    for command in [
        Command::Copy {
            from: seq(1),
            to: seq(1),
            mode: OverwriteMode::Merge,
        },
        Command::Move {
            from: executor_ref(0),
            to: executor_ref(0),
            mode: OverwriteMode::Merge,
        },
    ] {
        assert!(
            matches!(show.apply(&command), Err(ShowError::SameObject(_))),
            "{command:?}"
        );
        assert_eq!(bytes(&show), before);
    }
}

/* -------------------------------------------------------------------------- */
/* Move                                                                       */
/* -------------------------------------------------------------------------- */

#[test]
fn a_move_takes_the_source_away_and_carries_its_name() {
    let mut show = populated_show();
    let name = show.group(GroupId::new(1)).unwrap().name.clone();

    show.apply(&Command::Move {
        from: group_ref(1),
        to: group_ref(9),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    assert!(show.group(GroupId::new(1)).is_none(), "the source stayed");
    assert_eq!(show.group(GroupId::new(9)).unwrap().name, name);
}

/// **An executor swaps.** A desk's faders are places, and an operator
/// rearranging them is not throwing half of them away.
#[test]
fn moving_an_executor_onto_an_occupied_slot_swaps_the_two() {
    let mut show = populated_show();
    show.store_sequence(sequence(2, vec![cue("1", 1, AttributeType::Blue, 100)]))
        .unwrap();
    show.store_executor(executor(3, Some(2))).unwrap();

    show.apply(&Command::Move {
        from: executor_ref(0),
        to: executor_ref(3),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    assert_eq!(
        show.executor(ExecutorId::new(3)).unwrap().sequence_id,
        Some(SequenceId::new(1))
    );
    assert_eq!(
        show.executor(ExecutorId::new(0)).unwrap().sequence_id,
        Some(SequenceId::new(2)),
        "the slot that was moved onto did not come back the other way"
    );
}

/// And onto an **empty** slot it is a move rather than a swap: the place the
/// operator moved out of is left empty, which is `Delete Executor`'s answer.
#[test]
fn moving_an_executor_onto_an_empty_slot_leaves_that_slot_empty() {
    let mut show = populated_show();
    show.apply(&Command::Move {
        from: executor_ref(0),
        to: executor_ref(7),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    assert_eq!(
        show.executor(ExecutorId::new(7)).unwrap().sequence_id,
        Some(SequenceId::new(1))
    );
    assert!(show.executor(ExecutorId::new(0)).is_none());
}

/// **Moving a sequence brings every executor that played it along.**
///
/// The alternative is a fader pointing at a cue list that is gone, which
/// `Show::issues` would report and nobody would read until the Go did nothing.
#[test]
fn moving_a_sequence_repoints_the_executors_that_played_it() {
    let mut show = populated_show();
    assert_eq!(
        show.executor(ExecutorId::new(0)).unwrap().sequence_id,
        Some(SequenceId::new(1))
    );

    show.apply(&Command::Move {
        from: seq(1),
        to: seq(9),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    assert!(show.sequence(SequenceId::new(1)).is_none());
    assert_eq!(
        show.executor(ExecutorId::new(0)).unwrap().sequence_id,
        Some(SequenceId::new(9)),
        "the fader is pointing at a cue list that is gone"
    );
}

/// **Moving a preset rewrites every cue part that linked to it.**
///
/// Invisibly otherwise: nothing looks different until somebody edits the preset
/// and watches the cue not follow — which is the fault S28's `relink` exists to
/// prevent, one verb along.
#[test]
fn moving_a_preset_repoints_every_cue_that_linked_to_it() {
    let mut show = populated_show();
    // A cue part linked to preset 4.
    let mut linked = show.sequence(SequenceId::new(1)).unwrap().clone();
    linked.cues[0].parts[0].preset_ref = Some(PresetId::new(4));
    show.store_sequence(linked).unwrap();

    show.apply(&Command::Move {
        from: preset_ref(4),
        to: preset_ref(9),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    assert!(show.preset(PresetId::new(4)).is_none());
    assert_eq!(
        show.sequence(SequenceId::new(1)).unwrap().cues[0].parts[0].preset_ref,
        Some(PresetId::new(9)),
        "the cue is still linked to a preset that has gone"
    );
}

/// A move of a cue is a **renumber**, and onto a taken number the mode decides.
#[test]
fn moving_a_cue_is_a_renumber_and_the_mode_decides_a_collision() {
    let mut show = populated_show();
    show.apply(&Command::Move {
        from: cue_ref(1, "1"),
        to: cue_ref(1, "1.5"),
        mode: OverwriteMode::Merge,
    })
    .unwrap();
    let numbers: Vec<String> = show
        .sequence(SequenceId::new(1))
        .unwrap()
        .cues
        .iter()
        .map(|cue| cue.number.clone())
        .collect();
    assert_eq!(numbers, vec!["1.5".to_owned(), "2".to_owned()]);

    // Onto a number that is taken, in Override: the destination becomes the
    // source and the source is gone.
    show.apply(&Command::Move {
        from: cue_ref(1, "1.5"),
        to: cue_ref(1, "2"),
        mode: OverwriteMode::Override,
    })
    .unwrap();
    let cues = &show.sequence(SequenceId::new(1)).unwrap().cues;
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].number, "2");
    assert_eq!(cues[0].parts[0].attribute, AttributeType::Red);
}

/// **A cue copied into another cue list**, which is the case that makes `Copy`
/// worth having: an operator lifts a look out of one running order into another.
#[test]
fn a_cue_copies_into_another_cue_list() {
    let mut show = populated_show();
    show.store_sequence(sequence(2, vec![cue("1", 4, AttributeType::Dimmer, 100)]))
        .unwrap();

    show.apply(&Command::Copy {
        from: cue_ref(1, "1"),
        to: cue_ref(2, "1"),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    // Merged: cue 1 of sequence 2 keeps its own value and gains the source's.
    let merged = &show.sequence(SequenceId::new(2)).unwrap().cues[0];
    assert_eq!(merged.parts.len(), 2);
    // And the source is untouched, which is the difference from a move.
    assert_eq!(show.sequence(SequenceId::new(1)).unwrap().cues.len(), 2);
}

/// A copy onto a **free** cue number takes the source's name; onto one that
/// exists in Merge mode the destination keeps its own.
#[test]
fn a_copied_cue_takes_a_name_only_where_there_is_none() {
    let mut show = populated_show();
    show.apply(&Command::Copy {
        from: cue_ref(1, "1"),
        to: cue_ref(1, "7"),
        mode: OverwriteMode::Merge,
    })
    .unwrap();
    let cues = &show.sequence(SequenceId::new(1)).unwrap().cues;
    let seven = cues.iter().find(|cue| cue.number == "7").expect("cue 7");
    assert_eq!(seven.name, cues[0].name);

    // Onto cue 2, which has a name of its own.
    show.apply(&Command::Copy {
        from: cue_ref(1, "1"),
        to: cue_ref(1, "2"),
        mode: OverwriteMode::Merge,
    })
    .unwrap();
    let two = show
        .sequence(SequenceId::new(1))
        .unwrap()
        .cues
        .iter()
        .find(|cue| cue.number == "2")
        .expect("cue 2")
        .clone();
    assert_eq!(two.name, "Cue 2");
    assert_eq!(two.parts.len(), 2, "a merge kept both values");
}

/// A preset copied onto one that exists **merges its values and keeps the
/// destination's pool**, because a copy into preset 6 is a copy into preset 6
/// rather than a way of moving it between pools.
#[test]
fn a_copied_preset_merges_and_keeps_the_destinations_pool() {
    let mut show = populated_show();
    let mut other = preset(6, 2, AttributeType::Green, 100);
    other.pool = prism_domain::FeatureGroup::Beam;
    show.store_preset(other).unwrap();

    show.apply(&Command::Copy {
        from: preset_ref(4),
        to: preset_ref(6),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    let six = show.preset(PresetId::new(6)).unwrap();
    assert_eq!(six.pool, prism_domain::FeatureGroup::Beam);
    assert_eq!(six.values.len(), 2);

    // A **move** carries the pool along, because the preset itself moved.
    show.apply(&Command::Move {
        from: preset_ref(4),
        to: preset_ref(6),
        mode: OverwriteMode::Override,
    })
    .unwrap();
    assert_eq!(
        show.preset(PresetId::new(6)).unwrap().pool,
        show_pool_of_four()
    );
}

/// The pool `common::preset` files preset 4 under, named once.
fn show_pool_of_four() -> prism_domain::FeatureGroup {
    prism_domain::FeatureGroup::Color
}

/// **An executor copies everything but its number and its playback state.**
///
/// A copy that carried `isActive` would claim a slot was running because another
/// one was, and that state has one author — S34's tick.
#[test]
fn a_copied_executor_takes_the_settings_and_not_the_playback() {
    let mut show = populated_show();
    show.record_playback_state(ExecutorId::new(0).into(), true, Some(0))
        .unwrap();

    show.apply(&Command::Copy {
        from: executor_ref(0),
        to: executor_ref(5),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    let copy = show.executor(ExecutorId::new(5)).unwrap();
    assert_eq!(copy.sequence_id, Some(SequenceId::new(1)));
    assert_eq!(
        copy.master_level,
        show.executor(ExecutorId::new(0)).unwrap().master_level
    );
    assert!(!copy.is_active, "the copy claims to be running");
    assert_eq!(copy.current_cue_index, None);
}

/// Labelling a preset, which is the one pool `label_object` reaches through the
/// preset map rather than through a sequence.
#[test]
fn a_preset_takes_a_name_and_says_nothing_when_it_already_has_it() {
    let mut show = populated_show();
    show.apply(&Command::Label {
        target: preset_ref(4),
        name: "Deep blue".to_owned(),
    })
    .unwrap();
    assert_eq!(show.preset(PresetId::new(4)).unwrap().name, "Deep blue");

    let applied = show
        .apply(&Command::Label {
            target: preset_ref(4),
            name: "Deep blue".to_owned(),
        })
        .unwrap();
    assert!(applied.deltas.is_empty(), "{applied:?}");
}

/// Labelling a cue that is not there, and one in a cue list that is not there.
#[test]
fn labelling_something_that_is_not_there_is_refused() {
    let mut show = populated_show();
    let before = bytes(&show);
    for target in [
        seq(404),
        cue_ref(1, "404"),
        cue_ref(404, "1"),
        preset_ref(404),
    ] {
        let refusal = show.apply(&Command::Label {
            target: target.clone(),
            name: "Nowhere".to_owned(),
        });
        assert!(refusal.is_err(), "{target:?} was accepted");
        assert_eq!(bytes(&show), before);
    }
}

/// A copy whose **source** is not there is refused, in every pool, and writes
/// nothing — including at the destination, which a two-step edit could easily
/// have half-written.
#[test]
fn a_copy_from_something_that_is_not_there_writes_nothing() {
    let mut show = populated_show();
    let before = bytes(&show);
    for (from, to) in [
        (seq(404), seq(9)),
        (cue_ref(1, "404"), cue_ref(1, "9")),
        (cue_ref(404, "1"), cue_ref(1, "9")),
        (group_ref(404), group_ref(9)),
        (preset_ref(404), preset_ref(9)),
        (executor_ref(404), executor_ref(9)),
    ] {
        for command in [
            Command::Copy {
                from: from.clone(),
                to: to.clone(),
                mode: OverwriteMode::Merge,
            },
            Command::Move {
                from: from.clone(),
                to: to.clone(),
                mode: OverwriteMode::Merge,
            },
        ] {
            assert!(show.apply(&command).is_err(), "{command:?} was accepted");
            assert_eq!(bytes(&show), before, "{command:?} changed the show");
        }
    }
}

/// A cue moved onto an **empty** number is refused rather than making a cue
/// with no number: a cue number is what an operator types to reach it.
#[test]
fn a_cue_cannot_be_moved_onto_a_blank_number() {
    let mut show = populated_show();
    let before = bytes(&show);
    let refusal = show.apply(&Command::Move {
        from: cue_ref(1, "1"),
        to: cue_ref(1, "   "),
        mode: OverwriteMode::Merge,
    });
    assert!(
        matches!(refusal, Err(ShowError::EmptyCueNumber)),
        "{refusal:?}"
    );
    assert_eq!(bytes(&show), before);
}

/// A `Copy` or a `Move` naming a **view** never reaches the show applier, and
/// says so rather than doing something odd.
#[test]
fn a_view_pair_is_not_the_shows_to_copy() {
    let mut show = populated_show();
    let view = |id: u32| ObjectRef::View {
        view_id: prism_domain::ViewId::new(id),
    };
    for command in [
        Command::Copy {
            from: view(1),
            to: view(2),
            mode: OverwriteMode::Merge,
        },
        Command::Move {
            from: view(1),
            to: view(2),
            mode: OverwriteMode::Merge,
        },
        Command::Label {
            target: view(1),
            name: String::new(),
        },
    ] {
        assert!(
            matches!(show.apply(&command), Err(ShowError::NotAShowCommand)),
            "{command:?}"
        );
    }
}

/* -------------------------------------------------------------------------- */
/* Undo                                                                       */
/* -------------------------------------------------------------------------- */

/// **Every one of the four is undoable, and the Oops puts everything back.**
///
/// `ARCHITECTURE_SPEC.md` §6.1: a show edit is undoable and a playback action is
/// not, and these four are show edits whichever pool they name. Held on the
/// show's **bytes**, which is the standard S11 set: a scope that missed half of
/// what a move touched would show up here and nowhere else.
#[test]
fn every_generic_verb_is_taken_back_whole() {
    for command in [
        Command::Delete {
            target: cue_ref(1, "2"),
        },
        Command::Label {
            target: group_ref(1),
            name: "Renamed".to_owned(),
        },
        Command::Copy {
            from: seq(1),
            to: seq(9),
            mode: OverwriteMode::Merge,
        },
        Command::Move {
            from: seq(1),
            to: seq(9),
            mode: OverwriteMode::Merge,
        },
        Command::Move {
            from: preset_ref(4),
            to: preset_ref(9),
            mode: OverwriteMode::Merge,
        },
        Command::Move {
            from: executor_ref(0),
            to: executor_ref(7),
            mode: OverwriteMode::Merge,
        },
    ] {
        let mut file = ShowFile::new();
        file.show = populated_show();
        // A linked cue part, so the preset move's *references* are in the scope
        // this checks rather than only its two ends.
        let mut linked = file.show.sequence(SequenceId::new(1)).unwrap().clone();
        linked.cues[0].parts[0].preset_ref = Some(PresetId::new(4));
        file.show.store_sequence(linked).unwrap();
        let before = bytes(&file.show);

        file.apply(&command)
            .unwrap_or_else(|error| panic!("{command:?}: {error}"));
        assert_ne!(bytes(&file.show), before, "{command:?} changed nothing");

        file.apply(&Command::Oops)
            .expect("the step is on the journal");
        assert_eq!(
            bytes(&file.show),
            before,
            "{command:?} was not taken back whole"
        );
    }
}

/// A verb naming a **view** is the session's, and the show applier says so
/// rather than doing something odd with it.
#[test]
fn a_view_is_not_the_shows_to_delete() {
    let mut show = populated_show();
    let before = bytes(&show);
    let refusal = show.apply(&Command::Delete {
        target: ObjectRef::View {
            view_id: prism_domain::ViewId::new(1),
        },
    });
    assert!(matches!(refusal, Err(ShowError::NotAShowCommand)));
    assert_eq!(bytes(&show), before);
}

/// A cue that names no sequence never reaches the show: `ShowFile::apply`
/// resolves it against `Session::selectedSequence` first, and a bare `Show` has
/// no session to ask.
#[test]
fn a_cue_that_names_no_sequence_is_refused_by_a_bare_show() {
    let mut show = populated_show();
    let before = bytes(&show);
    let refusal = show.apply(&Command::Delete {
        target: ObjectRef::Cue {
            sequence_id: None,
            cue_number: "1".to_owned(),
        },
    });
    assert!(matches!(refusal, Err(ShowError::NoSelectedSequence)));
    assert_eq!(bytes(&show), before);
}

/// And through the **file**, with a sequence selected, the same line lands.
#[test]
fn the_selected_sequence_is_what_a_cue_with_no_number_means() {
    let mut file = ShowFile::new();
    file.show = populated_show();
    file.apply(&Command::SelectSequence {
        sequence_id: SequenceId::new(1),
    })
    .unwrap();

    file.apply(&Command::Delete {
        target: ObjectRef::Cue {
            sequence_id: None,
            cue_number: "2".to_owned(),
        },
    })
    .expect("the selected cue list is sequence 1");

    assert_eq!(
        file.show.sequence(SequenceId::new(1)).unwrap().cues.len(),
        1
    );
}

/// With **nothing** selected it is a message rather than a silence.
#[test]
fn a_line_that_needs_the_selection_says_so_when_there_is_none() {
    let mut file = ShowFile::new();
    file.show = populated_show();
    let refusal = file.apply(&Command::Delete {
        target: ObjectRef::Cue {
            sequence_id: None,
            cue_number: "2".to_owned(),
        },
    });
    assert!(refusal.is_err(), "{refusal:?}");
    assert!(
        refusal.unwrap_err().to_string().contains("no cue list"),
        "the refusal does not say what is wrong"
    );
    let _ = preset(1, 1, AttributeType::Red, 0);
}

/* -------------------------------------------------------------------------- */
/* What a merge means, pool by pool                                           */
/* -------------------------------------------------------------------------- */

/// **A cue list merged into another matches cues up by number**, keeps the ones
/// the destination had to itself, and appends the ones it did not.
///
/// Matching by number rather than by position is the only reading an operator
/// can predict: cue 5 of the source becomes cue 5 of the destination whatever
/// order either list happens to be in.
#[test]
fn merging_a_cue_list_writes_over_cues_of_the_same_number_and_keeps_the_rest() {
    let mut show = populated_show();
    show.store_sequence(sequence(
        2,
        vec![
            cue("1", 4, AttributeType::Dimmer, 100),
            cue("5", 4, AttributeType::Dimmer, 200),
        ],
    ))
    .unwrap();

    show.apply(&Command::Copy {
        from: seq(1),
        to: seq(2),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    let cues = &show.sequence(SequenceId::new(2)).unwrap().cues;
    let numbers: Vec<&str> = cues.iter().map(|cue| cue.number.as_str()).collect();
    // Sorted by number, because that is how a cue list is stored (S13).
    assert_eq!(numbers, ["1", "2", "5"], "{cues:?}");
    // Cue 1 is the source's now, whole — a cue is merged by number, and its
    // parts are the source's.
    assert_eq!(cues[0].parts[0].fixture, prism_domain::FixtureId::new(1));
    // Cue 5 was the destination's alone and is untouched.
    assert_eq!(cues[2].parts[0].value, 200);
}

/// A group merged into another **takes the fixtures it does not already have**,
/// each once, and in the destination's order — a group is a set with an order an
/// operator chose, not a bag.
#[test]
fn merging_a_group_takes_the_fixtures_it_does_not_have() {
    let mut show = populated_show();
    show.store_group(group(2, &[3, 4])).unwrap();

    show.apply(&Command::Copy {
        from: group_ref(1),
        to: group_ref(2),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    let fixtures: Vec<u32> = show
        .group(GroupId::new(2))
        .unwrap()
        .fixtures
        .iter()
        .map(|fixture| fixture.get())
        .collect();
    assert_eq!(fixtures, [3, 4, 1, 2], "fixture 3 came in twice");
}

/// Inside a merged **cue**, a part is matched by fixture *and* attribute, so the
/// source's value replaces the destination's rather than sitting beside it — two
/// values for one attribute would be a cue that means two things.
#[test]
fn a_merged_cue_writes_over_a_part_with_the_same_fixture_and_attribute() {
    let mut show = populated_show();
    show.store_sequence(sequence(2, vec![cue("1", 1, AttributeType::Red, 100)]))
        .unwrap();

    show.apply(&Command::Copy {
        from: cue_ref(1, "1"),
        to: cue_ref(2, "1"),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    let parts = &show.sequence(SequenceId::new(2)).unwrap().cues[0].parts;
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].value, 65535);
}

/// The same rule one pool along, in a **preset**.
#[test]
fn a_merged_preset_writes_over_a_value_with_the_same_fixture_and_attribute() {
    let mut show = populated_show();
    show.store_preset(preset(7, 1, AttributeType::Red, 100))
        .unwrap();

    show.apply(&Command::Copy {
        from: preset_ref(4),
        to: preset_ref(7),
        mode: OverwriteMode::Merge,
    })
    .unwrap();

    let values = &show.preset(PresetId::new(7)).unwrap().values;
    assert_eq!(values.len(), 1, "{values:?}");
    assert_eq!(values[0].value, 65535);
}

/// **Labelling something the name it already has writes nothing.**
///
/// Not an optimisation: a delta that changed nothing would still reach every
/// client, redraw a pool and land in the journal as an undoable step, so an
/// operator would have to press Oops for a rename they did not make.
#[test]
fn labelling_something_the_name_it_has_says_nothing() {
    let mut show = populated_show();
    for (target, name) in [
        (seq(1), "Sequence 1"),
        (cue_ref(1, "1"), "Cue 1"),
        (group_ref(1), "Group 1"),
    ] {
        let applied = show
            .apply(&Command::Label {
                target: target.clone(),
                name: name.to_owned(),
            })
            .expect("the object is there");
        assert!(applied.deltas.is_empty(), "{target:?} produced {applied:?}");
    }
}
