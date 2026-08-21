//! S28: sequences, cues and presets — what a store does, and what it says it
//! will do first.
//!
//! Four claims, and each of them is one the interface leans on:
//!
//! - **a preset link stays alive** — editing a preset changes the cues that
//!   reference it, which `prism_domain::preset` has claimed in its first
//!   paragraph since S1 and nothing did until now;
//! - **a preview is what the store after it does** — S27's rule for the patch,
//!   applied to a store, and the reason `Query::StorePreview` exists at all;
//! - **the store is a merge, and the counts say what that costs** — the numbers
//!   an operator reads before pressing the button;
//! - **a cue can be corrected in place**, number included, without a client ever
//!   sending a whole cue.

mod common;

use common::{cue, executor, populated_show, preset, sequence};
use prism_core::{Effect, Show, ShowError, ShowFile, ShowFileError};
use prism_domain::{
    AttributeType, Command, CueProperty, CueTrigger, ExecutorButtonFunction, ExecutorFaderFunction,
    ExecutorId, FeatureGroup, FixtureId, GoDirection, ObjectRef, OverwriteMode, PlaybackTarget,
    PresetId, RgbColor, SelectionMode, SequenceId, SequenceStoreMode, StoreMode, StoreTarget,
};

/// The show as bytes, so "nothing changed" can be asserted rather than claimed.
fn bytes(show: &Show) -> Vec<u8> {
    rmp_serde::to_vec_named(show).expect("a show encodes")
}

/// A file with the populated show and a programmer holding nothing yet.
fn file() -> ShowFile {
    ShowFile {
        show: populated_show(),
        ..ShowFile::new()
    }
}

/// Selects fixtures and sets one attribute on them, through the commands.
fn dial(file: &mut ShowFile, fixtures: &[u32], attribute: AttributeType, value: u16) {
    file.apply(&Command::SelectFixtures {
        ids: fixtures.iter().copied().map(FixtureId::new).collect(),
        mode: SelectionMode::Set,
    })
    .expect("those fixtures are patched");
    file.apply(&Command::SetAttribute {
        attribute,
        value: i32::from(value),
        relative: false,
    })
    .expect("an absolute value inside the range");
}

/// The part of a stored cue that carries an attribute.
fn part_of(file: &ShowFile, number: &str, attribute: AttributeType) -> prism_domain::CuePart {
    file.show
        .sequence(SequenceId::new(1))
        .expect("sequence 1")
        .cues
        .iter()
        .find(|cue| cue.number == number)
        .unwrap_or_else(|| panic!("no cue {number}"))
        .parts
        .iter()
        .find(|part| part.attribute == attribute)
        .cloned()
        .unwrap_or_else(|| panic!("cue {number} has no {attribute:?}"))
}

/* -------------------------------------------------------------------------- */
/* Preset links                                                               */
/* -------------------------------------------------------------------------- */

/// **The claim `prism_domain::preset` has made since S1, finally true.**
///
/// A cue part carrying a `presetRef` follows later edits of the preset — *change
/// the blue everywhere* — and until S28 it followed nothing: the cue kept the
/// value the preset had when it was stored, and editing the preset moved the
/// pool and left every cue behind.
///
/// Asserted on the **stored cue** rather than on the command being accepted,
/// which is the shape S39's criteria ask for too.
#[test]
fn editing_a_preset_changes_the_cues_that_reference_it() {
    let mut file = file();
    // Fixture 1 at full red, applied from preset 4 — which already exists in
    // the populated show, holding exactly that — so the programmer's value
    // carries the link, and then stored into a cue.
    dial(&mut file, &[1], AttributeType::Red, 65535);
    file.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    })
    .expect("preset 4 exists");
    file.apply(&Command::StoreCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "7".to_owned(),
        mode: StoreMode::Merge,
    })
    .expect("sequence 1 exists");

    assert_eq!(
        part_of(&file, "7", AttributeType::Red).preset_ref,
        Some(PresetId::new(4))
    );
    assert_eq!(part_of(&file, "7", AttributeType::Red).value, 65535);

    // Now edit the preset: a lower red, stored over the same number out of a
    // fresh programmer.
    file.apply(&Command::ClearProgrammer).expect("it clears");
    dial(&mut file, &[1], AttributeType::Red, 30000);
    let applied = file
        .apply(&Command::StorePreset {
            preset_id: PresetId::new(4),
            pool: Some(FeatureGroup::Color),
            name: "Half red".to_owned(),
            color: Some(RgbColor { r: 128, g: 0, b: 0 }),
            mode: StoreMode::Merge,
        })
        .expect("the store is accepted");

    // The cue moved with it, and the link is still there.
    assert_eq!(part_of(&file, "7", AttributeType::Red).value, 30000);
    assert_eq!(
        part_of(&file, "7", AttributeType::Red).preset_ref,
        Some(PresetId::new(4))
    );
    // And the sequence the change reached is named, so the engine reloads it —
    // a cue whose value changed in the document and not in the merge body would
    // go on playing the old look until the daemon happened to restart.
    assert!(
        applied
            .effects
            .contains(&Effect::ReloadSequence(SequenceId::new(1))),
        "{:?}",
        applied.effects
    );
}

/// A part that is **not** linked is left exactly where it was.
///
/// The other half of the same claim: a preset edit reaches the parts that
/// reference the preset and nothing else, so an operator who stored a colour by
/// hand does not find it moving when somebody edits a preset of the same colour.
#[test]
fn editing_a_preset_leaves_the_values_that_are_not_linked_to_it() {
    let mut file = file();
    // Cue 1 of sequence 1 holds fixture 1 red at full with **no** link.
    let before = file
        .show
        .sequence(SequenceId::new(1))
        .expect("sequence 1")
        .clone();
    assert!(
        before.cues[0]
            .parts
            .iter()
            .all(|part| part.preset_ref.is_none())
    );

    dial(&mut file, &[1], AttributeType::Red, 111);
    file.apply(&Command::StorePreset {
        preset_id: PresetId::new(4),
        pool: Some(FeatureGroup::Color),
        name: "Dim red".to_owned(),
        color: None,
        mode: StoreMode::Merge,
    })
    .expect("the store is accepted");

    assert_eq!(
        file.show.sequence(SequenceId::new(1)).expect("sequence 1"),
        &before,
        "a preset edit reached a cue part that never referenced it"
    );
}

/// A link to a value the preset no longer carries keeps **both** its value and
/// its link.
///
/// Dropping the value would change light nobody asked to change; dropping the
/// link would mean a preset that regained the value could never reach the cue
/// again. `Show::relink` says so and this is what says it is true.
#[test]
fn a_link_to_a_value_the_preset_does_not_carry_survives_untouched() {
    let mut file = file();
    dial(&mut file, &[1], AttributeType::Red, 65535);
    file.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    })
    .expect("preset 4 exists");
    file.apply(&Command::StoreCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "7".to_owned(),
        mode: StoreMode::Merge,
    })
    .expect("sequence 1 exists");

    // A store that says nothing about fixture 1's red: it is fixture 2's green
    // that goes in, so the linked part is one the new preset does not mention.
    file.apply(&Command::ClearProgrammer).expect("it clears");
    dial(&mut file, &[2], AttributeType::Green, 200);
    file.apply(&Command::StorePreset {
        preset_id: PresetId::new(4),
        pool: Some(FeatureGroup::Color),
        name: "Preset 4".to_owned(),
        color: None,
        mode: StoreMode::Merge,
    })
    .expect("the store is accepted");

    let part = part_of(&file, "7", AttributeType::Red);
    assert_eq!(part.value, 65535);
    assert_eq!(part.preset_ref, Some(PresetId::new(4)));
}

/// Taking a preset edit back takes it out of the cues as well.
///
/// The reason `Image::Preset` images the sequences beside the preset: restoring
/// the pool alone would leave the edit standing in every cue that referenced it.
#[test]
fn an_oops_over_a_preset_edit_puts_the_cues_back_too() {
    let mut file = file();
    dial(&mut file, &[1], AttributeType::Red, 65535);
    file.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    })
    .expect("preset 4 exists");
    file.apply(&Command::StoreCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "7".to_owned(),
        mode: StoreMode::Merge,
    })
    .expect("sequence 1 exists");
    let before = bytes(&file.show);

    file.apply(&Command::ClearProgrammer).expect("it clears");
    dial(&mut file, &[1], AttributeType::Red, 1000);
    file.apply(&Command::StorePreset {
        preset_id: PresetId::new(4),
        pool: Some(FeatureGroup::Color),
        name: "Nearly off".to_owned(),
        color: None,
        mode: StoreMode::Merge,
    })
    .expect("the store is accepted");
    assert_ne!(bytes(&file.show), before, "the store changed nothing");

    file.apply(&Command::Oops).expect("there is a step to undo");
    assert_eq!(
        bytes(&file.show),
        before,
        "the Oops left the preset edit standing somewhere"
    );
}

/// A preset takes the values of **its pool** and leaves the rest.
///
/// Which pool an attribute belongs to is the profile's answer rather than the
/// attribute name's, which is why the daemon decides it and no client does.
#[test]
fn a_preset_stores_the_values_of_its_own_pool_only() {
    let mut file = file();
    dial(&mut file, &[1], AttributeType::Red, 65535);
    // Fixture 4 is a dimmer, so this value is on the Dimmer bank.
    dial(&mut file, &[4], AttributeType::Dimmer, 500);

    file.apply(&Command::StorePreset {
        preset_id: PresetId::new(9),
        pool: Some(FeatureGroup::Color),
        name: "Reds".to_owned(),
        color: None,
        mode: StoreMode::Merge,
    })
    .expect("the store is accepted");

    let stored = file.show.preset(PresetId::new(9)).expect("preset 9");
    assert_eq!(stored.values.len(), 1);
    assert_eq!(stored.values[0].attribute, AttributeType::Red);
    assert_eq!(stored.pool, FeatureGroup::Color);
}

/// A store into a pool the programmer has nothing for is refused when the
/// preset does not exist, and accepted when it does — because
/// `Command::StorePreset` carries the name, so it is then a relabel.
#[test]
fn an_empty_store_creates_nothing_and_relabels_what_is_there() {
    let mut file = file();
    let before = bytes(&file.show);
    let refusal = file.apply(&Command::StorePreset {
        preset_id: PresetId::new(9),
        pool: Some(FeatureGroup::Position),
        name: "Nowhere".to_owned(),
        color: None,
        mode: StoreMode::Merge,
    });
    assert!(
        matches!(refusal, Err(ShowFileError::Programmer(_))),
        "{refusal:?}"
    );
    assert_eq!(bytes(&file.show), before, "a refusal changed the show");

    // Preset 4 exists, so the same empty programmer is a rename.
    file.apply(&Command::StorePreset {
        preset_id: PresetId::new(4),
        pool: Some(FeatureGroup::Color),
        name: "Renamed".to_owned(),
        color: Some(RgbColor { r: 1, g: 2, b: 3 }),
        mode: StoreMode::Merge,
    })
    .expect("a relabel is accepted");
    let stored = file.show.preset(PresetId::new(4)).expect("preset 4");
    assert_eq!(stored.name, "Renamed");
    assert_eq!(stored.color, Some(RgbColor { r: 1, g: 2, b: 3 }));
    assert_eq!(stored.values.len(), 1, "the relabel changed the values");
}

/* -------------------------------------------------------------------------- */
/* The preview                                                                */
/* -------------------------------------------------------------------------- */

/// **A preview is what the store that follows it does.**
///
/// S27's rule for a patch, applied to a store. The three counts are compared
/// against the cue the store actually wrote, so a preview that counted
/// differently from `Programmer::cue` fails here rather than misleading an
/// operator on a stage.
#[test]
fn a_store_preview_says_what_the_store_that_follows_it_does() {
    let mut file = file();
    // Cue 1 holds one part: fixture 1 red. The programmer holds fixture 1 red
    // (a replacement) and fixture 2 red (an addition).
    dial(&mut file, &[1, 2], AttributeType::Red, 4321);

    let preview = file.preview_store(
        &StoreTarget::Cue {
            sequence_id: SequenceId::new(1),
            cue_number: "1".to_owned(),
        },
        StoreMode::Merge,
    );
    assert!(preview.accepted);
    assert!(preview.exists, "cue 1 is already there");
    assert_eq!(preview.name, "Cue 1");
    assert_eq!(preview.mode, StoreMode::Merge);
    assert_eq!((preview.added, preview.replaced, preview.kept), (1, 1, 0));

    let before = bytes(&file.show);
    file.apply(&Command::StoreCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
        mode: StoreMode::Merge,
    })
    .expect("the store is accepted");
    assert_ne!(bytes(&file.show), before);

    let stored = file
        .show
        .sequence(SequenceId::new(1))
        .expect("sequence 1")
        .cues
        .iter()
        .find(|cue| cue.number == "1")
        .expect("cue 1")
        .clone();
    // added + replaced + kept is the cue afterwards, which is the arithmetic the
    // three numbers are only worth anything if they satisfy.
    assert_eq!(
        stored.parts.len(),
        usize::try_from(preview.added + preview.replaced + preview.kept).expect("a small count")
    );
    // And a store leaves the name and the times alone: it is about the look.
    assert_eq!(stored.name, "Cue 1");
    assert!((stored.fade_in - 3.0).abs() < f64::EPSILON);
}

/// A preview of a cue that is not there says so, and *kept* is zero.
#[test]
fn a_preview_of_a_cue_that_does_not_exist_is_a_create() {
    let mut file = file();
    dial(&mut file, &[1, 2], AttributeType::Red, 100);
    let preview = file.preview_store(
        &StoreTarget::Cue {
            sequence_id: SequenceId::new(1),
            cue_number: "88".to_owned(),
        },
        StoreMode::Merge,
    );
    assert!(preview.accepted);
    assert!(!preview.exists);
    assert_eq!(preview.name, "");
    assert_eq!((preview.added, preview.replaced, preview.kept), (2, 0, 0));
}

/// A preview says *what the refusal will be*, in the daemon's own words.
#[test]
fn a_preview_carries_the_refusal_the_store_would_answer_with() {
    let file = file();
    let empty = file.preview_store(
        &StoreTarget::Cue {
            sequence_id: SequenceId::new(1),
            cue_number: "1".to_owned(),
        },
        StoreMode::Merge,
    );
    assert!(!empty.accepted);
    assert!(empty.refusal.is_some());
    // Even refused, it still says what is there — an operator looking at cue 1
    // is looking at *Cue 1*, whether or not they can store into it now.
    assert!(empty.exists);
    assert_eq!(empty.name, "Cue 1");

    let missing = file.preview_store(
        &StoreTarget::Cue {
            sequence_id: SequenceId::new(404),
            cue_number: "1".to_owned(),
        },
        StoreMode::Merge,
    );
    assert!(!missing.accepted);
    assert!(!missing.exists);
    assert!(
        missing
            .refusal
            .as_deref()
            .is_some_and(|why| why.contains("404")),
        "{:?}",
        missing.refusal
    );
}

/// **A question changes nothing.** Asserted on the bytes, as every other
/// "changes nothing" in this crate is.
#[test]
fn a_store_preview_writes_nothing_at_all() {
    let mut file = file();
    dial(&mut file, &[1, 2], AttributeType::Red, 4321);
    let before = bytes(&file.show);
    let programmer = file.programmer.state().clone();
    for target in [
        StoreTarget::Cue {
            sequence_id: SequenceId::new(1),
            cue_number: "1".to_owned(),
        },
        StoreTarget::Cue {
            sequence_id: SequenceId::new(404),
            cue_number: String::new(),
        },
        StoreTarget::Preset {
            preset_id: PresetId::new(4),
            pool: FeatureGroup::Color,
        },
        StoreTarget::Preset {
            preset_id: PresetId::new(77),
            pool: FeatureGroup::Beam,
        },
    ] {
        let _ = file.preview_store(&target, StoreMode::Merge);
    }
    assert_eq!(bytes(&file.show), before);
    assert_eq!(file.programmer.state(), &programmer);
}

/// A preset preview counts what the pool filter leaves, not what the programmer
/// holds.
#[test]
fn a_preset_preview_counts_only_the_values_of_its_pool() {
    let mut file = file();
    dial(&mut file, &[1], AttributeType::Red, 65535);
    dial(&mut file, &[4], AttributeType::Dimmer, 500);

    let colour = file.preview_store(
        &StoreTarget::Preset {
            preset_id: PresetId::new(4),
            pool: FeatureGroup::Color,
        },
        StoreMode::Merge,
    );
    // Preset 4 holds fixture 1 red already, so this replaces one and adds none.
    assert_eq!((colour.added, colour.replaced, colour.kept), (0, 1, 0));

    let dimmer = file.preview_store(
        &StoreTarget::Preset {
            preset_id: PresetId::new(4),
            pool: FeatureGroup::Dimmer,
        },
        StoreMode::Merge,
    );
    // The dimmer value is the only one of that pool, and preset 4's one stored
    // value is **kept**, which is exactly what Merge means.
    assert_eq!((dimmer.added, dimmer.replaced, dimmer.kept), (1, 0, 1));
}

/* -------------------------------------------------------------------------- */
/* Cues, edited                                                               */
/* -------------------------------------------------------------------------- */

/// Every field of a cue can be corrected, one command per field.
#[test]
fn a_cue_is_corrected_one_field_at_a_time() {
    let mut file = file();
    let set = |file: &mut ShowFile, property: CueProperty| {
        file.apply(&Command::SetCueProperty {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
            property,
        })
        .expect("the edit is accepted");
    };
    // The name is `Command::Label`'s since S40, and it is applied through the
    // same file so the round trip below covers it too.
    file.apply(&Command::Label {
        target: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
        },
        name: "Blackout".to_owned(),
    })
    .expect("the label is accepted");
    set(&mut file, CueProperty::FadeIn { seconds: 0.5 });
    set(&mut file, CueProperty::FadeOut { seconds: 12.0 });
    set(&mut file, CueProperty::Delay { seconds: 2.0 });
    set(
        &mut file,
        CueProperty::Trigger {
            trigger: CueTrigger::Time,
            trigger_time: Some(4.0),
        },
    );

    let cue = file
        .show
        .sequence(SequenceId::new(1))
        .expect("sequence 1")
        .cues[0]
        .clone();
    assert_eq!(cue.name, "Blackout");
    assert!((cue.fade_in - 0.5).abs() < f64::EPSILON);
    assert!((cue.fade_out - 12.0).abs() < f64::EPSILON);
    assert!((cue.delay - 2.0).abs() < f64::EPSILON);
    assert_eq!(cue.trigger, CueTrigger::Time);
    assert_eq!(cue.trigger_time, Some(4.0));
    // The look is untouched: a property edit is not a store.
    assert_eq!(cue.parts.len(), 1);
}

/// A renumber moves the cue into its new place in playback order.
#[test]
fn renumbering_a_cue_reorders_the_list() {
    let mut file = file();
    file.apply(&Command::Move {
        from: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
        },
        to: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "3".to_owned(),
        },
        mode: OverwriteMode::Override,
    })
    .expect("3 is free");
    let numbers: Vec<String> = file
        .show
        .sequence(SequenceId::new(1))
        .expect("sequence 1")
        .cues
        .iter()
        .map(|cue| cue.number.clone())
        .collect();
    assert_eq!(numbers, vec!["2".to_owned(), "3".to_owned()]);
}

/// **A renumber onto a number that is taken is a question, not a refusal**
/// (S40).
///
/// S28 refused it, for `RenumberFixture`'s reason: the number is the key, and
/// replacing the other cue deletes a look nobody asked to delete. S40 keeps the
/// *protection* and moves it one layer out — `Move Cue 3 Cue 8` onto a cue that
/// exists asks **merge, override or cancel**, and cancel is the operator not
/// sending the command. So the refusal became a mode, and what this test holds
/// is that each mode does what its name says.
#[test]
fn a_cue_moved_onto_one_that_exists_does_what_the_mode_says() {
    let moved = |mode| {
        let mut file = file();
        file.apply(&Command::Move {
            from: ObjectRef::Cue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
            },
            to: ObjectRef::Cue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "2".to_owned(),
            },
            mode,
        })
        .expect("the move is accepted");
        file.show
            .sequence(SequenceId::new(1))
            .expect("sequence 1")
            .clone()
    };

    // Override: cue 2 *becomes* cue 1, name and all, and cue 1 is gone. One cue
    // is left and it is the one that moved.
    let overridden = moved(OverwriteMode::Override);
    assert_eq!(
        overridden
            .cues
            .iter()
            .map(|cue| cue.number.clone())
            .collect::<Vec<_>>(),
        vec!["2".to_owned()]
    );
    assert_eq!(overridden.cues[0].name, "Cue 1");
    assert_eq!(overridden.cues[0].parts.len(), 1);
    assert_eq!(overridden.cues[0].parts[0].attribute, AttributeType::Red);

    // Merge: cue 2 keeps its own name and gains cue 1's values beside its own.
    let merged = moved(OverwriteMode::Merge);
    assert_eq!(
        merged
            .cues
            .iter()
            .map(|cue| cue.number.clone())
            .collect::<Vec<_>>(),
        vec!["2".to_owned()]
    );
    assert_eq!(merged.cues[0].name, "Cue 2");
    assert_eq!(merged.cues[0].parts.len(), 2);
}

/// A cue moved onto a **free** number is an ordinary renumber, and the mode is
/// not read at all.
#[test]
fn a_cue_moved_onto_a_free_number_is_a_renumber() {
    let mut file = file();
    file.apply(&Command::Move {
        from: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
        },
        to: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1.5".to_owned(),
        },
        mode: OverwriteMode::Merge,
    })
    .expect("1.5 is free");
    let numbers: Vec<String> = file
        .show
        .sequence(SequenceId::new(1))
        .expect("sequence 1")
        .cues
        .iter()
        .map(|cue| cue.number.clone())
        .collect();
    assert_eq!(numbers, vec!["1.5".to_owned(), "2".to_owned()]);
}

/// A time that runs backwards is refused, and it says what was wrong with it.
#[test]
fn a_negative_time_is_refused_and_names_itself() {
    let mut file = file();
    let refusal = file.apply(&Command::SetCueProperty {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
        property: CueProperty::FadeIn { seconds: -2.0 },
    });
    let Err(ShowFileError::Show(error)) = refusal else {
        panic!("a negative fade was accepted: {refusal:?}");
    };
    assert!(error.to_string().contains("-2"), "{error}");
}

/// **An edit that changes nothing is not a delta and not a step.**
///
/// The case S27 met with `RenumberFixture`: an operator who pressed Oops after
/// one of these would otherwise watch nothing happen and press it again, losing
/// the edit they meant to take back.
#[test]
fn setting_a_field_to_what_it_already_holds_is_not_a_step() {
    let mut file = file();
    let applied = file
        .apply(&Command::Label {
            target: ObjectRef::Cue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
            },
            name: "Cue 1".to_owned(),
        })
        .expect("it is accepted");
    assert!(applied.deltas.is_empty(), "{:?}", applied.deltas);
    assert!(!file.show.is_dirty());
    assert!(
        matches!(file.apply(&Command::Oops), Err(ShowFileError::Journal(_))),
        "the edit that changed nothing was filed as a step"
    );
}

/// A deleted cue takes nothing else with it, and the numbers do not close up.
#[test]
fn deleting_a_cue_leaves_the_other_numbers_where_they_are() {
    let mut file = file();
    file.show
        .store_sequence(sequence(
            2,
            vec![
                cue("1", 1, AttributeType::Red, 1),
                cue("2", 1, AttributeType::Red, 2),
                cue("3", 1, AttributeType::Red, 3),
            ],
        ))
        .expect("a sequence stores");
    file.apply(&Command::Delete {
        target: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(2)),
            cue_number: "2".to_owned(),
        },
    })
    .expect("cue 2 is there");
    let numbers: Vec<String> = file
        .show
        .sequence(SequenceId::new(2))
        .expect("sequence 2")
        .cues
        .iter()
        .map(|cue| cue.number.clone())
        .collect();
    assert_eq!(numbers, vec!["1".to_owned(), "3".to_owned()]);
}

/// A cue that is not there is a refusal that names it.
#[test]
fn an_edit_to_a_cue_that_is_not_there_is_refused() {
    let mut file = file();
    for command in [
        Command::SetCueProperty {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "404".to_owned(),
            property: CueProperty::Delay { seconds: 1.0 },
        },
        Command::Delete {
            target: ObjectRef::Cue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "404".to_owned(),
            },
        },
    ] {
        let refusal = file.apply(&command);
        assert!(
            matches!(
                refusal,
                Err(ShowFileError::Show(ShowError::UnknownCue { .. }))
            ),
            "{command:?} answered {refusal:?}"
        );
    }
}

/* -------------------------------------------------------------------------- */
/* Sequences and executors                                                    */
/* -------------------------------------------------------------------------- */

/// **A store into a free number makes the cue list, and one into a taken number
/// never empties it by accident** (S40).
///
/// S39 had two commands here — `CreateSequence`, refused when the number was
/// taken, and `StoreSequence`, refused when it was free — and S40 has one,
/// because `Store Sequence 4` typed on the command line cannot know which it is
/// (the parser does not read the show, S26). What survived is the protection
/// rather than the refusal: an empty programmer on a **free** number makes an
/// empty cue list, which is what `CreateSequence` did, and an empty programmer
/// on a **taken** one is refused and writes nothing, so a store can never empty
/// a playback that is on stage without a look and a mode behind it.
#[test]
fn a_sequence_is_created_empty_and_never_replaces_one() {
    let mut file = file();
    file.apply(&Command::StoreSequence {
        sequence_id: SequenceId::new(9),
        name: "Act 2".to_owned(),
        mode: SequenceStoreMode::Append,
    })
    .expect("9 is free");
    let made = file.show.sequence(SequenceId::new(9)).expect("sequence 9");
    assert_eq!(made.name, "Act 2");
    assert!(made.cues.is_empty());
    assert!(!made.looping);

    let before = bytes(&file.show);
    let refusal = file.apply(&Command::StoreSequence {
        sequence_id: SequenceId::new(1),
        name: "Over the top".to_owned(),
        mode: SequenceStoreMode::Append,
    });
    assert!(
        matches!(
            refusal,
            Err(ShowFileError::Programmer(
                prism_core::ProgrammerError::NothingToStore
            ))
        ),
        "{refusal:?}"
    );
    assert_eq!(bytes(&file.show), before);
    // And the name it carried did not land either: a store names a cue list
    // only when it makes one.
    assert_eq!(
        file.show.sequence(SequenceId::new(1)).unwrap().name,
        "Sequence 1"
    );
}

/// **A cue list can be put on an executor, and then it can be fired.**
///
/// The whole of *cues can be fired from the interface*: before S28 an executor
/// could only be given a sequence by a show file somebody else had written, so
/// `ExecutorGo` had nothing to reach.
#[test]
fn assigning_a_sequence_to_an_empty_slot_makes_it_playable() {
    let mut file = file();
    let refusal = file.apply(&Command::ExecutorGo {
        target: PlaybackTarget::of_executor(ExecutorId::new(5)),
        direction: GoDirection::Next,
    });
    assert!(
        matches!(
            refusal,
            Err(ShowFileError::Show(ShowError::UnknownExecutor(_)))
        ),
        "{refusal:?}"
    );

    file.apply(&Command::AssignExecutor {
        executor_id: ExecutorId::new(5),
        sequence_id: Some(SequenceId::new(1)),
    })
    .expect("sequence 1 exists");

    let made = file.show.executor(ExecutorId::new(5)).expect("executor 5");
    assert_eq!(made.sequence_id, Some(SequenceId::new(1)));
    assert_eq!(made.fader_function, ExecutorFaderFunction::Master);
    assert_eq!(made.master_level, u16::MAX);
    // The three the protocol can actually press, and a fourth left empty
    // because the five that have no command are S34's.
    assert_eq!(
        made.button_functions,
        vec![
            ExecutorButtonFunction::GoForward,
            ExecutorButtonFunction::GoBack,
            ExecutorButtonFunction::Off,
            ExecutorButtonFunction::Empty,
        ]
    );

    let applied = file
        .apply(&Command::ExecutorGo {
            target: PlaybackTarget::of_executor(ExecutorId::new(5)),
            direction: GoDirection::Next,
        })
        .expect("it can be fired now");
    assert!(
        applied.effects.contains(&Effect::ExecutorGo {
            executor: ExecutorId::new(5).into(),
            direction: GoDirection::Next,
        }),
        "{:?}",
        applied.effects
    );
}

/// Taking a sequence off keeps the slot; taking the *assignment* back removes
/// it, because that is the state the show was in.
#[test]
fn clearing_a_slot_keeps_it_and_an_oops_over_a_new_one_removes_it() {
    let mut file = file();
    // Executor 0 already exists with sequence 1 on it and a master an operator
    // could have moved.
    file.apply(&Command::AssignExecutor {
        executor_id: ExecutorId::new(0),
        sequence_id: None,
    })
    .expect("it is accepted");
    let kept = file.show.executor(ExecutorId::new(0)).expect("executor 0");
    assert_eq!(kept.sequence_id, None);
    assert_eq!(kept.master_level, 65535, "the slot lost its master");

    // A slot that never existed is created and then taken back to *not there*.
    let before = bytes(&file.show);
    file.apply(&Command::AssignExecutor {
        executor_id: ExecutorId::new(6),
        sequence_id: Some(SequenceId::new(1)),
    })
    .expect("sequence 1 exists");
    file.apply(&Command::Oops).expect("there is a step");
    assert_eq!(bytes(&file.show), before, "the empty slot was left behind");
}

/// Clearing a slot that has nothing on it changes nothing at all.
#[test]
fn clearing_an_empty_slot_is_not_an_edit() {
    let mut file = file();
    let before = bytes(&file.show);
    let applied = file
        .apply(&Command::AssignExecutor {
            executor_id: ExecutorId::new(7),
            sequence_id: None,
        })
        .expect("it is accepted");
    assert!(applied.deltas.is_empty(), "{:?}", applied.deltas);
    assert_eq!(bytes(&file.show), before);
}

/// A sequence that is not there cannot be put on an executor.
#[test]
fn an_executor_cannot_be_given_a_sequence_that_does_not_exist() {
    let mut file = file();
    let before = bytes(&file.show);
    let refusal = file.apply(&Command::AssignExecutor {
        executor_id: ExecutorId::new(3),
        sequence_id: Some(SequenceId::new(404)),
    });
    assert!(
        matches!(
            refusal,
            Err(ShowFileError::Show(ShowError::UnknownSequence(_)))
        ),
        "{refusal:?}"
    );
    assert_eq!(bytes(&file.show), before);
}

/// The scenery these tests lean on, asserted rather than assumed.
#[test]
fn the_populated_show_is_the_one_these_tests_describe() {
    let show = populated_show();
    assert_eq!(
        show.preset(PresetId::new(4))
            .expect("preset 4")
            .values
            .len(),
        1
    );
    assert_eq!(
        show.sequence(SequenceId::new(1))
            .expect("sequence 1")
            .cues
            .len(),
        2
    );
    assert!(
        show.executor(ExecutorId::new(5)).is_none(),
        "slot 5 is empty"
    );
    assert_eq!(
        show.executor(ExecutorId::new(0))
            .expect("executor 0")
            .sequence_id,
        Some(SequenceId::new(1))
    );
    // And the two helpers this file uses beside the show.
    assert_eq!(executor(9, None).sequence_id, None);
    assert_eq!(preset(3, 1, AttributeType::Blue, 7).values[0].value, 7);
}
