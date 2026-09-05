//! S39: what a store does on top of something that is already there, what
//! *edit cue 3* means, and the state that makes an Update key blink.
//!
//! Five claims, and every one of them is asserted on the **stored cue** rather
//! than on the command being accepted — which is the shape S39's exit criteria
//! ask for in as many words, because a mode the daemon takes and does not honour
//! is exactly what S28 refused to ship:
//!
//! - **each mode does what its name says** against a cue that already exists;
//! - **`StoreSequence` appends at the highest number**, overrides the list, or
//!   merges into every cue of it;
//! - **`EditCue` keeps every `presetRef`**, so an edit cannot quietly break a
//!   preset link;
//! - **a cue loaded and put back unchanged is byte-identical**;
//! - **the update state clears** when the programmer is cleared, when the cue is
//!   deleted, and when another cue is loaded.

mod common;

use common::{cue, populated_show, preset, sequence};
use prism_core::{ProgrammerError, ShowFile, ShowFileError};
use prism_domain::{
    AttributeType, Command, CueEdit, CuePart, CueProperty, FixtureId, ObjectRef, OverwriteMode,
    PresetId, PresetPool, ProgrammerValueSource, SelectionMode, SequenceId, SequenceStoreMode,
    StoreMode, StoreTarget,
};

/// A file with the populated show and an empty programmer.
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

/// A store into cue `number` of sequence 1.
fn store(number: &str, mode: StoreMode) -> Command {
    Command::StoreCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: number.to_owned(),
        mode,
    }
}

/// The cue with that number, as the show holds it.
fn stored(file: &ShowFile, number: &str) -> prism_domain::Cue {
    file.show
        .cue(SequenceId::new(1), number)
        .unwrap_or_else(|| panic!("no cue {number}"))
        .clone()
}

/// What a cue sets, as pairs, so a mode's effect reads as a set rather than as a
/// list of struct literals.
fn keys(cue: &prism_domain::Cue) -> Vec<(u32, AttributeType)> {
    cue.parts
        .iter()
        .map(|part| (part.fixture.get(), part.attribute))
        .collect()
}

/// A show whose cue 1 holds a red on fixture 1 and a green on fixture 2, so a
/// store can be seen to touch one of them and leave the other.
fn two_part_cue() -> ShowFile {
    let mut file = file();
    let mut sequence = sequence(
        1,
        vec![
            cue("1", 1, AttributeType::Red, 1000),
            cue("2", 3, AttributeType::Blue, 7),
        ],
    );
    sequence.cues[0].parts.push(CuePart {
        fixture: FixtureId::new(2),
        attribute: AttributeType::Green,
        value: 2000,
        preset_ref: None,
        tracking: prism_domain::CueTracking::Track,
    });
    sequence.cues[0].name = "Opening".to_owned();
    sequence.cues[0].fade_in = 4.5;
    file.show.store_sequence(sequence).expect("it is valid");
    file.show.mark_saved();
    file
}

/* -------------------------------------------------------------------------- */
/* The three store modes, against a cue that already exists                   */
/* -------------------------------------------------------------------------- */

/// **Merge writes what it was given and leaves everything else standing.**
///
/// The mode S28 shipped, restated here so the three read side by side.
#[test]
fn a_merge_adds_and_replaces_and_takes_nothing_away() {
    let mut file = two_part_cue();
    // Fixture 1's red is a replacement; fixture 1's blue is an addition;
    // fixture 2's green is neither mentioned nor touched.
    dial(&mut file, &[1], AttributeType::Red, 65535);
    dial(&mut file, &[1], AttributeType::Blue, 4444);

    file.apply(&store("1", StoreMode::Merge)).expect("a merge");
    let cue = stored(&file, "1");
    assert_eq!(
        keys(&cue),
        vec![
            (1, AttributeType::Red),
            (1, AttributeType::Blue),
            (2, AttributeType::Green),
        ]
    );
    assert_eq!(cue.parts[0].value, 65535, "the red was replaced");
    assert_eq!(cue.parts[1].value, 4444, "the blue was added");
    assert_eq!(cue.parts[2].value, 2000, "the green was left alone");
    assert_eq!(
        cue.name, "Opening",
        "a store is about the look, not the name"
    );
    assert!((cue.fade_in - 4.5).abs() < f64::EPSILON);
}

/// **Override makes the cue hold exactly what the programmer holds.**
///
/// The values it does not mention are gone — which is what
/// `StorePreview::removed` counts, and the whole reason an operator has to be
/// told before pressing it.
#[test]
fn an_override_leaves_the_cue_holding_exactly_the_programmer() {
    let mut file = two_part_cue();
    dial(&mut file, &[1], AttributeType::Red, 65535);
    dial(&mut file, &[1], AttributeType::Blue, 4444);

    file.apply(&store("1", StoreMode::Override))
        .expect("an override");
    let cue = stored(&file, "1");
    assert_eq!(
        keys(&cue),
        vec![(1, AttributeType::Red), (1, AttributeType::Blue)],
        "fixture 2's green was not in the programmer, so it is not in the cue"
    );
    assert_eq!(cue.parts[0].value, 65535);
    assert_eq!(cue.parts[1].value, 4444);
    // The name and the times survive: they are not the look, and a store that
    // took them with it would make correcting a cue's values cost its label.
    assert_eq!(cue.name, "Opening");
    assert!((cue.fade_in - 4.5).abs() < f64::EPSILON);
    // And the other cue of the list is untouched — a mode is about one cue.
    assert_eq!(keys(&stored(&file, "2")), vec![(3, AttributeType::Blue)]);
}

/// **Remove takes the programmer's values out and uses none of their levels.**
#[test]
fn a_remove_takes_out_what_it_names_and_writes_nothing() {
    let mut file = two_part_cue();
    // A wildly different level on the red, to make the point that a Remove does
    // not use it: the red goes, it is not set to 65535.
    dial(&mut file, &[1], AttributeType::Red, 65535);

    file.apply(&store("1", StoreMode::Remove))
        .expect("a remove");
    let cue = stored(&file, "1");
    assert_eq!(keys(&cue), vec![(2, AttributeType::Green)]);
    assert_eq!(cue.parts[0].value, 2000);
    assert_eq!(cue.name, "Opening");
}

/// A Remove that would empty a cue leaves an **empty cue**, not no cue.
///
/// Deleting a cue is `Command::DeleteCue`'s, and a store that deleted one as a
/// side effect would take a number off a running order that an operator has
/// written down.
#[test]
fn a_remove_that_empties_a_cue_leaves_the_cue_there() {
    let mut file = two_part_cue();
    dial(&mut file, &[1], AttributeType::Red, 1);
    dial(&mut file, &[2], AttributeType::Green, 1);

    file.apply(&store("1", StoreMode::Remove))
        .expect("a remove");
    let cue = stored(&file, "1");
    assert!(cue.parts.is_empty());
    assert_eq!(cue.name, "Opening", "the cue is still the cue");
}

/// A Remove against a cue that is not there, and one that would remove nothing,
/// are both refused — **and the refusal changes nothing**.
#[test]
fn a_remove_with_nothing_to_remove_is_refused_and_writes_nothing() {
    let mut file = two_part_cue();
    dial(&mut file, &[3], AttributeType::Blue, 500);

    let before = rmp_serde::to_vec_named(&file.show).expect("a show encodes");
    let refusal = file.apply(&store("1", StoreMode::Remove)).unwrap_err();
    assert!(
        matches!(
            refusal,
            ShowFileError::Programmer(ProgrammerError::NothingToRemove { .. })
        ),
        "{refusal:?}"
    );
    assert_eq!(
        rmp_serde::to_vec_named(&file.show).expect("a show encodes"),
        before,
        "a refused store wrote into the show"
    );

    let missing = file.apply(&store("404", StoreMode::Remove)).unwrap_err();
    assert!(
        matches!(
            missing,
            ShowFileError::Programmer(ProgrammerError::NothingToRemove { .. })
        ),
        "{missing:?}"
    );
    assert_eq!(
        rmp_serde::to_vec_named(&file.show).expect("a show encodes"),
        before
    );
}

/// **The preview says what each mode would do, and the store then does it.**
///
/// S28's rule — a preview and the store after it may not disagree — extended to
/// the three modes, because a chooser that showed the wrong numbers would be
/// worse than no chooser at all.
#[test]
fn a_preview_of_each_mode_is_the_store_that_follows_it() {
    for mode in [StoreMode::Merge, StoreMode::Override, StoreMode::Remove] {
        let mut file = two_part_cue();
        dial(&mut file, &[1], AttributeType::Red, 65535);
        dial(&mut file, &[1], AttributeType::Blue, 4444);

        let target = StoreTarget::Cue {
            sequence_id: SequenceId::new(1),
            cue_number: "1".to_owned(),
        };
        let preview = file.preview_store(&target, mode);
        assert_eq!(preview.mode, mode);
        assert!(preview.accepted, "{mode:?}: {:?}", preview.refusal);
        assert!(preview.exists);
        assert_eq!(preview.name, "Opening");
        // The four counts account for every value filed under that number...
        assert_eq!(
            preview.kept + preview.replaced + preview.removed,
            2,
            "{mode:?} does not account for the cue that is there"
        );

        file.apply(&store("1", mode)).expect("the preview said yes");
        // ...and what is left is what it said would be.
        assert_eq!(
            stored(&file, "1").parts.len(),
            usize::try_from(preview.added + preview.replaced + preview.kept)
                .expect("a small count"),
            "{mode:?}: the cue is not the sum the preview promised"
        );
    }
}

/// The preset half of the same claim: the three modes reach a pool as well.
///
/// They have to. A preview can be asked about a preset in any of them, and an
/// answer describing an outcome no command can produce is what S28 refused.
#[test]
fn the_modes_reach_a_preset_pool_as_well() {
    let mut file = file();
    file.show
        .store_preset(preset(7, 1, AttributeType::Red, 1000))
        .expect("a preset of one value");
    file.show.mark_saved();
    dial(&mut file, &[2], AttributeType::Red, 65535);

    let stored_preset = |file: &ShowFile| {
        file.show
            .preset(PresetId::new(7))
            .expect("preset 7")
            .values
            .iter()
            .map(|value| value.fixture.get())
            .collect::<Vec<_>>()
    };

    let mut merged = file.clone();
    merged
        .apply(&Command::StorePreset {
            preset_id: PresetId::new(7),
            pool: Some(PresetPool::Color),
            name: "Warm".to_owned(),
            color: None,
            mode: StoreMode::Merge,
        })
        .expect("a merge");
    assert_eq!(
        stored_preset(&merged),
        vec![1, 2],
        "a merge keeps fixture 1"
    );

    let mut overridden = file.clone();
    overridden
        .apply(&Command::StorePreset {
            preset_id: PresetId::new(7),
            pool: Some(PresetPool::Color),
            name: "Warm".to_owned(),
            color: None,
            mode: StoreMode::Override,
        })
        .expect("an override");
    assert_eq!(
        stored_preset(&overridden),
        vec![2],
        "an override leaves the pool holding exactly the programmer"
    );

    // And a Remove of the value that *is* in it.
    let mut file = self::file();
    file.show
        .store_preset(preset(7, 1, AttributeType::Red, 1000))
        .expect("a preset of one value");
    file.show
        .store_preset(prism_domain::Preset {
            values: vec![
                prism_domain::PresetValue {
                    fixture: FixtureId::new(1),
                    attribute: AttributeType::Red,
                    value: 1000,
                },
                prism_domain::PresetValue {
                    fixture: FixtureId::new(2),
                    attribute: AttributeType::Red,
                    value: 2000,
                },
            ],
            ..preset(7, 1, AttributeType::Red, 1000)
        })
        .expect("two values");
    file.show.mark_saved();
    dial(&mut file, &[1], AttributeType::Red, 5);
    file.apply(&Command::StorePreset {
        preset_id: PresetId::new(7),
        pool: Some(PresetPool::Color),
        name: "Warm".to_owned(),
        color: None,
        mode: StoreMode::Remove,
    })
    .expect("a remove");
    assert_eq!(stored_preset(&file), vec![2]);
}

/* -------------------------------------------------------------------------- */
/* StoreSequence                                                              */
/* -------------------------------------------------------------------------- */

/// **Append adds a cue at the highest number** — the criterion in as many words.
#[test]
fn appending_puts_a_new_cue_at_the_highest_number() {
    let mut file = two_part_cue();
    // The list reads 1, 2. A cue at 1.5 makes the point that the next number is
    // one past the highest **whole** number rather than one past the highest.
    file.apply(&Command::Move {
        from: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "2".to_owned(),
        },
        to: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1.5".to_owned(),
        },
        mode: OverwriteMode::Override,
    })
    .expect("a renumber");
    dial(&mut file, &[3], AttributeType::Blue, 900);

    file.apply(&Command::StoreSequence {
        sequence_id: SequenceId::new(1),
        name: String::new(),
        mode: SequenceStoreMode::Append,
    })
    .expect("an append");
    let numbers: Vec<String> = file
        .show
        .sequence(SequenceId::new(1))
        .expect("sequence 1")
        .cues
        .iter()
        .map(|cue| cue.number.clone())
        .collect();
    assert_eq!(numbers, vec!["1", "1.5", "2"]);
    assert_eq!(keys(&stored(&file, "2")), vec![(3, AttributeType::Blue)]);
    // And the cues that were there are untouched.
    assert_eq!(
        keys(&stored(&file, "1")),
        vec![(1, AttributeType::Red), (2, AttributeType::Green)]
    );
}

/// An Append into an **empty** cue list starts at 1.
#[test]
fn appending_into_an_empty_list_starts_at_one() {
    let mut file = file();
    file.show
        .create_sequence(SequenceId::new(9), "Act 2")
        .expect("9 is free");
    file.show.mark_saved();
    dial(&mut file, &[1], AttributeType::Red, 3);

    file.apply(&Command::StoreSequence {
        sequence_id: SequenceId::new(9),
        name: String::new(),
        mode: SequenceStoreMode::Append,
    })
    .expect("an append");
    let cues = &file
        .show
        .sequence(SequenceId::new(9))
        .expect("sequence 9")
        .cues;
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].number, "1");
}

/// **Override makes the sequence be this look**: one cue, and the list that was
/// there is gone.
#[test]
fn overriding_a_sequence_leaves_one_cue() {
    let mut file = two_part_cue();
    dial(&mut file, &[3], AttributeType::Blue, 900);

    file.apply(&Command::StoreSequence {
        sequence_id: SequenceId::new(1),
        name: String::new(),
        mode: SequenceStoreMode::Override,
    })
    .expect("an override");
    let sequence = file.show.sequence(SequenceId::new(1)).expect("sequence 1");
    assert_eq!(sequence.cues.len(), 1);
    assert_eq!(sequence.cues[0].number, "1");
    assert_eq!(
        keys(&sequence.cues[0]),
        vec![(3, AttributeType::Blue)],
        "the sequence holds exactly the programmer"
    );
    assert_eq!(sequence.name, "Sequence 1", "the sequence keeps its name");
}

/// **Merge writes into every cue there is** — the cue-level Merge one level up.
#[test]
fn merging_into_a_sequence_reaches_every_cue() {
    let mut file = two_part_cue();
    dial(&mut file, &[3], AttributeType::Blue, 900);

    file.apply(&Command::StoreSequence {
        sequence_id: SequenceId::new(1),
        name: String::new(),
        mode: SequenceStoreMode::Merge,
    })
    .expect("a merge");
    for number in ["1", "2"] {
        let cue = stored(&file, number);
        assert!(
            cue.parts
                .iter()
                .any(|part| part.fixture == FixtureId::new(3)
                    && part.attribute == AttributeType::Blue
                    && part.value == 900),
            "cue {number} did not get the merged value"
        );
    }
    // Cue 1 kept both of its own values as well.
    assert_eq!(
        keys(&stored(&file, "1")),
        vec![
            (1, AttributeType::Red),
            (2, AttributeType::Green),
            (3, AttributeType::Blue),
        ]
    );
}

/// A Merge into a sequence with no cues is refused, and changes nothing.
///
/// Appending instead would be a different command than the one that was sent.
#[test]
fn merging_into_a_cue_list_with_no_cues_is_refused() {
    let mut file = file();
    file.show
        .create_sequence(SequenceId::new(9), "Act 2")
        .expect("9 is free");
    file.show.mark_saved();
    dial(&mut file, &[1], AttributeType::Red, 3);

    let refusal = file
        .apply(&Command::StoreSequence {
            sequence_id: SequenceId::new(9),
            name: String::new(),
            mode: SequenceStoreMode::Merge,
        })
        .unwrap_err();
    assert!(
        matches!(
            refusal,
            ShowFileError::Programmer(ProgrammerError::NoCuesToMergeInto(_))
        ),
        "{refusal:?}"
    );
    assert!(
        file.show
            .sequence(SequenceId::new(9))
            .expect("sequence 9")
            .cues
            .is_empty()
    );
}

/* -------------------------------------------------------------------------- */
/* EditCue, and the round trip                                                */
/* -------------------------------------------------------------------------- */

/// **`EditCue` loads every attribute of the cue and keeps every `presetRef`.**
///
/// The link is the half that is easy to lose and impossible to see: nothing
/// looks different until somebody edits the preset and the cue does not follow.
#[test]
fn editing_a_cue_loads_every_attribute_and_keeps_every_link() {
    let mut file = file();
    // Fixture 1's red comes from preset 4, which the populated show carries and
    // which holds exactly that — so the programmer's value carries the link.
    dial(&mut file, &[1], AttributeType::Red, 1);
    file.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    })
    .expect("preset 4 exists");
    // And a value with no link beside it, so the test can tell a load that kept
    // every link from one that gave every part the same one.
    dial(&mut file, &[2], AttributeType::Green, 4242);
    file.apply(&store("7", StoreMode::Merge)).expect("a store");
    // **Two presses since S51** (B37): the first takes the selection and the
    // second the values. This test wants an empty programmer to load the cue
    // into, so it presses until there is one rather than counting on a number.
    file.apply(&Command::ClearProgrammer).expect("a clear");
    file.apply(&Command::ClearProgrammer).expect("a clear");
    assert!(file.programmer.state().is_empty());

    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "7".to_owned(),
    })
    .expect("cue 7 is there");

    let state = file.programmer.state();
    assert_eq!(
        state.selection,
        vec![FixtureId::new(1), FixtureId::new(2)],
        "the cue's fixtures are selected, so an encoder reaches them"
    );
    let red = state
        .value(FixtureId::new(1), AttributeType::Red)
        .expect("the red is loaded");
    assert_eq!(red.value, 65535, "preset 4 is fixture 1 at full red");
    assert_eq!(red.preset_ref, Some(PresetId::new(4)), "the link survived");
    assert_eq!(red.source, ProgrammerValueSource::Recalled);
    let green = state
        .value(FixtureId::new(2), AttributeType::Green)
        .expect("the green is loaded");
    assert_eq!(green.value, 4242);
    assert_eq!(green.preset_ref, None, "a part with no link gains none");
    assert_eq!(green.source, ProgrammerValueSource::Recalled);
}

/// **A cue loaded and stored back unchanged is byte-identical.**
///
/// The criterion, taken literally: the cue is serialised before and after and
/// the two `Vec<u8>` are compared, which is a stronger claim than comparing the
/// model and the same one `tests/command_application.rs` makes about refusals.
#[test]
fn a_cue_loaded_and_updated_unchanged_is_byte_identical() {
    let mut file = file();
    dial(&mut file, &[1], AttributeType::Red, 1);
    file.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    })
    .expect("preset 4 exists");
    dial(&mut file, &[2], AttributeType::Green, 4242);
    file.apply(&store("7", StoreMode::Merge)).expect("a store");
    // A name and a fade on it, so the round trip has something to lose besides
    // the values: a store is about the look, and these are not it.
    file.apply(&Command::Label {
        target: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "7".to_owned(),
        },
        name: "Opening".to_owned(),
    })
    .expect("a name");
    file.apply(&Command::SetCueProperty {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "7".to_owned(),
        property: CueProperty::FadeIn { seconds: 6.25 },
    })
    .expect("a fade");
    file.apply(&Command::ClearProgrammer).expect("a clear");

    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "7".to_owned(),
    })
    .expect("cue 7 is there");
    let loaded = rmp_serde::to_vec_named(&stored(&file, "7")).expect("a cue encodes");

    file.apply(&Command::Update).expect("an update");
    assert_eq!(
        rmp_serde::to_vec_named(&stored(&file, "7")).expect("a cue encodes"),
        loaded,
        "a load and an update with nothing changed moved the cue"
    );
}

/// An `EditCue` naming a cue that is not there is refused by the **show**,
/// before the programmer has been touched.
#[test]
fn editing_a_cue_that_is_not_there_is_refused_and_leaves_the_programmer_alone() {
    let mut file = file();
    dial(&mut file, &[1], AttributeType::Red, 4321);
    let before = file.programmer.state().clone();

    let refusal = file
        .apply(&Command::EditCue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "404".to_owned(),
        })
        .unwrap_err();
    assert!(matches!(refusal, ShowFileError::Show(_)), "{refusal:?}");
    assert_eq!(file.programmer.state(), &before);
    assert_eq!(file.session.session().editing_cue, None);
}

/* -------------------------------------------------------------------------- */
/* The update state                                                           */
/* -------------------------------------------------------------------------- */

/// The state an Update key blinks on: which cue, and whether it has moved.
#[test]
fn the_update_state_is_set_by_a_load_and_moved_by_an_edit() {
    let mut file = two_part_cue();
    assert_eq!(file.session.session().editing_cue, None);

    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
    })
    .expect("cue 1 is there");
    assert_eq!(
        file.session.session().editing_cue,
        Some(CueEdit {
            sequence_id: SequenceId::new(1),
            cue_number: "1".to_owned(),
            modified: false,
        })
    );

    // An encoder turned on the loaded cue is what makes the key blink.
    dial(&mut file, &[1], AttributeType::Red, 12345);
    assert_eq!(
        file.session
            .session()
            .editing_cue
            .as_ref()
            .map(|e| e.modified),
        Some(true)
    );

    // And an Update stops it blinking, having put the change back.
    file.apply(&Command::Update).expect("an update");
    assert_eq!(
        file.session
            .session()
            .editing_cue
            .as_ref()
            .map(|e| e.modified),
        Some(false)
    );
    assert_eq!(part_value(&file, "1", 1, AttributeType::Red), 12345);
}

/// The value of one part of one cue.
fn part_value(file: &ShowFile, number: &str, fixture: u32, attribute: AttributeType) -> u16 {
    stored(file, number)
        .parts
        .iter()
        .find(|part| part.fixture == FixtureId::new(fixture) && part.attribute == attribute)
        .unwrap_or_else(|| panic!("cue {number} has no {attribute:?} on {fixture}"))
        .value
}

/// **An `Update` stores in Override mode**, which is the point of it: an Update
/// that merged could never take a value *out* of the cue it is updating, and
/// taking one out is why an operator loads a cue in the first place.
#[test]
fn an_update_overrides_rather_than_merging() {
    let mut file = two_part_cue();
    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
    })
    .expect("cue 1 is there");
    // Take fixture 2's green out of the programmer by clearing and reselecting,
    // which is how an operator drops a fixture from a look.
    file.apply(&Command::ClearProgrammer).expect("a clear");
    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
    })
    .expect("load it again");

    // A second cue is loaded and edited down to one value.
    let mut trimmed = file.programmer.state().clone();
    trimmed.clear_value(FixtureId::new(2), AttributeType::Green);
    file.programmer.restore(trimmed);

    file.apply(&Command::Update).expect("an update");
    assert_eq!(
        keys(&stored(&file, "1")),
        vec![(1, AttributeType::Red)],
        "an update that merged would have left the green standing"
    );
}

/// An `Update` with nothing loaded is refused, and changes nothing.
#[test]
fn an_update_with_no_cue_loaded_is_refused() {
    let mut file = two_part_cue();
    dial(&mut file, &[1], AttributeType::Red, 999);
    let before = rmp_serde::to_vec_named(&file.show).expect("a show encodes");

    let refusal = file.apply(&Command::Update).unwrap_err();
    assert!(
        matches!(
            refusal,
            ShowFileError::Programmer(ProgrammerError::NothingIsBeingEdited)
        ),
        "{refusal:?}"
    );
    assert_eq!(
        rmp_serde::to_vec_named(&file.show).expect("a show encodes"),
        before
    );
}

/// **The three things that clear the update state**, each asserted — S39's exit
/// criterion in one test.
#[test]
fn the_update_state_clears_on_a_clear_a_delete_and_another_load() {
    let load = |file: &mut ShowFile, number: &str| {
        file.apply(&Command::EditCue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: number.to_owned(),
        })
        .expect("that cue is there");
    };

    // 1. The programmer is cleared: the values it was holding are gone, so
    //    there is nothing to put back.
    //
    //    **The update state goes with the values and not with the selection** —
    //    S51, B37. The first press drops the selection and no value moves, so
    //    an operator letting one fixture go to add the next is still editing
    //    the same cue and the Update key stays lit. It is the press that takes
    //    the *values* that ends the edit.
    let mut file = two_part_cue();
    load(&mut file, "1");
    file.apply(&Command::ClearProgrammer).expect("a clear");
    assert_eq!(
        file.session.session().editing_cue,
        Some(CueEdit {
            sequence_id: SequenceId::new(1),
            cue_number: "1".to_owned(),
            modified: false,
        }),
        "dropping the selection ended an edit in which no value moved"
    );
    file.apply(&Command::ClearProgrammer)
        .expect("a second clear");
    assert_eq!(file.session.session().editing_cue, None);

    // 2. The cue is deleted.
    let mut file = two_part_cue();
    load(&mut file, "1");
    file.apply(&Command::Delete {
        target: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
        },
    })
    .expect("a delete");
    assert_eq!(file.session.session().editing_cue, None);
    // ...and deleting a *different* cue leaves it standing.
    let mut file = two_part_cue();
    load(&mut file, "1");
    file.apply(&Command::Delete {
        target: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "2".to_owned(),
        },
    })
    .expect("a delete");
    assert!(file.session.session().editing_cue.is_some());

    // 3. Another cue is loaded.
    let mut file = two_part_cue();
    load(&mut file, "1");
    dial(&mut file, &[1], AttributeType::Red, 7);
    load(&mut file, "2");
    assert_eq!(
        file.session.session().editing_cue,
        Some(CueEdit {
            sequence_id: SequenceId::new(1),
            cue_number: "2".to_owned(),
            modified: false,
        }),
        "loading another cue replaces the edit and starts it unmodified"
    );
}

/// **A renumber of the cue being edited carries the update state with it.**
///
/// The alternative is to clear it, and the reason not to is what an `Update`
/// would do afterwards: it stores in Override mode into the number it remembers,
/// so an operator who corrects `1` to `1.5` and presses Update would end up with
/// both cues.
#[test]
fn renumbering_the_cue_being_edited_follows_it() {
    let mut file = two_part_cue();
    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
    })
    .expect("cue 1 is there");
    file.apply(&Command::Move {
        from: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
        },
        to: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "0.5".to_owned(),
        },
        mode: OverwriteMode::Override,
    })
    .expect("a renumber");
    assert_eq!(
        file.session
            .session()
            .editing_cue
            .as_ref()
            .map(|edit| edit.cue_number.clone()),
        Some("0.5".to_owned())
    );

    file.apply(&Command::Update).expect("an update");
    let numbers: Vec<String> = file
        .show
        .sequence(SequenceId::new(1))
        .expect("sequence 1")
        .cues
        .iter()
        .map(|cue| cue.number.clone())
        .collect();
    assert_eq!(
        numbers,
        vec!["0.5", "2"],
        "the update recreated the cue at the number it used to have"
    );
}

/// **An Override store into the cue being edited stops the key blinking**, and
/// one into any other cue does not.
///
/// `Command::Update` *is* that store with its target taken from the desk, so
/// the rule is one rule rather than two: after it, the cue and the programmer
/// agree, and an Update key still blinking would be asking an operator to press
/// something that would change nothing.
///
/// A **Merge** into the same cue deliberately does not stop it. A merged cue
/// holds everything the programmer holds *and more*, so *nothing has changed
/// since it was loaded* is not true of it — and a key that stopped blinking
/// there would be claiming the two agree when they do not.
#[test]
fn an_override_into_the_cue_being_edited_is_an_update_by_another_name() {
    let mut file = two_part_cue();
    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
    })
    .expect("cue 1 is there");
    dial(&mut file, &[1], AttributeType::Red, 4321);
    let modified = |file: &ShowFile| {
        file.session
            .session()
            .editing_cue
            .as_ref()
            .map(|edit| edit.modified)
    };
    assert_eq!(modified(&file), Some(true));

    // A store into a **different** cue leaves the key blinking: the cue being
    // edited has not been put back.
    file.apply(&store("2", StoreMode::Override))
        .expect("cue 2 exists");
    assert_eq!(modified(&file), Some(true));

    // A Merge into the cue being edited leaves it blinking too.
    file.apply(&store("1", StoreMode::Merge)).expect("a merge");
    assert_eq!(modified(&file), Some(true));

    // And an Override into it stops it.
    file.apply(&store("1", StoreMode::Override))
        .expect("an override");
    assert_eq!(modified(&file), Some(false));
    assert_eq!(part_value(&file, "1", 1, AttributeType::Red), 4321);
}

/// A renumber of a **different** cue leaves the update state exactly as it was.
#[test]
fn renumbering_another_cue_does_not_move_the_update_state() {
    let mut file = two_part_cue();
    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
    })
    .expect("cue 1 is there");
    let before = file.session.session().editing_cue.clone();

    file.apply(&Command::Move {
        from: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "2".to_owned(),
        },
        to: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "9".to_owned(),
        },
        mode: OverwriteMode::Override,
    })
    .expect("a renumber");
    assert_eq!(file.session.session().editing_cue, before);

    // And neither does a rename of the cue that *is* being edited: its number
    // is what the update state is filed under, and a name is not a number.
    file.apply(&Command::Label {
        target: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
        },
        name: "Opening again".to_owned(),
    })
    .expect("a rename");
    assert_eq!(file.session.session().editing_cue, before);
}

/* -------------------------------------------------------------------------- */
/* Oops                                                                       */
/* -------------------------------------------------------------------------- */

/// **Oops takes back a store and an update** — including the update state, so
/// the desk is not left claiming to be editing a cue the programmer no longer
/// holds.
#[test]
fn oops_takes_back_a_store_an_update_and_the_edit_that_started_it() {
    let mut file = two_part_cue();
    dial(&mut file, &[3], AttributeType::Blue, 555);

    let before_store = rmp_serde::to_vec_named(&file.show).expect("a show encodes");
    file.apply(&store("1", StoreMode::Override))
        .expect("an override");
    assert_eq!(keys(&stored(&file, "1")), vec![(3, AttributeType::Blue)]);
    file.apply(&Command::Oops).expect("there is a step");
    assert_eq!(
        rmp_serde::to_vec_named(&file.show).expect("a show encodes"),
        before_store,
        "an Oops over an Override did not put the cue back"
    );

    // And an `EditCue` followed by an `Update`, taken back one at a time.
    let mut file = two_part_cue();
    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
    })
    .expect("cue 1 is there");
    dial(&mut file, &[1], AttributeType::Red, 31000);
    let before_update = rmp_serde::to_vec_named(&file.show).expect("a show encodes");
    file.apply(&Command::Update).expect("an update");
    assert_eq!(part_value(&file, "1", 1, AttributeType::Red), 31000);

    file.apply(&Command::Oops).expect("the update");
    assert_eq!(
        rmp_serde::to_vec_named(&file.show).expect("a show encodes"),
        before_update,
        "an Oops over an Update did not put the cue back"
    );
    file.apply(&Command::Oops).expect("the encoder");
    file.apply(&Command::Oops).expect("the selection");
    file.apply(&Command::Oops).expect("the load itself");
    assert_eq!(
        file.session.session().editing_cue,
        None,
        "an Oops over the load left the desk editing a cue it no longer holds"
    );
    assert!(file.programmer.state().values.is_empty());
}

/// A `DeleteCue` that cleared the update state puts it back when it is undone.
#[test]
fn undoing_a_delete_puts_the_update_state_back() {
    let mut file = two_part_cue();
    file.apply(&Command::EditCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "1".to_owned(),
    })
    .expect("cue 1 is there");
    file.apply(&Command::Delete {
        target: ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
        },
    })
    .expect("a delete");
    assert_eq!(file.session.session().editing_cue, None);

    file.apply(&Command::Oops).expect("the delete");
    assert_eq!(
        file.session.session().editing_cue,
        Some(CueEdit {
            sequence_id: SequenceId::new(1),
            cue_number: "1".to_owned(),
            modified: false,
        }),
        "the cue came back and the desk was left editing nothing"
    );
}

/* -------------------------------------------------------------------------- */
/* The selected sequence                                                      */
/* -------------------------------------------------------------------------- */

/// **The cue list in force is a session field of its own** — S39's decision, and
/// the two things that make it the right one.
#[test]
fn the_selected_sequence_is_its_own_field_and_no_executor_is_needed() {
    let mut file = file();
    assert_eq!(file.session.session().selected_sequence, None);

    // A sequence nobody has put on an executor can be selected.
    file.show
        .create_sequence(SequenceId::new(9), "Act 2")
        .expect("9 is free");
    file.apply(&Command::SelectSequence {
        sequence_id: SequenceId::new(9),
    })
    .expect("a selection");
    assert_eq!(
        file.session.session().selected_sequence,
        Some(SequenceId::new(9))
    );
    assert!(
        file.show
            .executors()
            .all(|executor| executor.sequence_id != Some(SequenceId::new(9))),
        "it was selected without occupying a playback slot"
    );

    // And selecting an executor does **not** move it: an operator programming
    // one cue list while another plays the show is the ordinary case.
    file.apply(&Command::SelectExecutor {
        executor_id: prism_domain::ExecutorId::new(0),
    })
    .expect("executor 0 exists");
    assert_eq!(
        file.session.session().selected_sequence,
        Some(SequenceId::new(9)),
        "selecting an executor moved the cue list in force"
    );
}
