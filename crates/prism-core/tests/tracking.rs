//! **What a cue asserts, and what it inherits** — S48.
//!
//! The engine's `tests/cue_tracking.rs` asserts the *output*: the same cue
//! reached two ways puts out the same frames. This target asserts the two halves
//! that live in the daemon's own model instead — the edit an operator makes
//! (`Command::SetCueTracking`) and the reading a cue sheet is given
//! (`Query::CueTracking`) — and the one claim that says the state is **derived**
//! rather than stored: editing cue 2 changes what cue 5 outputs, without cue 5
//! being touched.

mod common;

use common::{cue, populated_show, sequence};
use prism_core::{Show, ShowError, ShowFile};
use prism_domain::{
    AttributeType, Command, Cue, CuePart, CueTracking, CueTrackingMode, FixtureId, SequenceId,
};

/// The show as bytes, so "nothing changed" can be asserted rather than claimed.
fn bytes(show: &Show) -> Vec<u8> {
    rmp_serde::to_vec_named(show).expect("a show encodes")
}

/// A cue naming several fixtures at once, all of them tracking.
fn look(number: &str, values: &[(u32, u16)]) -> Cue {
    Cue {
        parts: values
            .iter()
            .map(|&(fixture, value)| CuePart {
                fixture: FixtureId::new(fixture),
                attribute: AttributeType::Red,
                value,
                preset_ref: None,
                tracking: CueTracking::Track,
            })
            .collect(),
        ..cue(number, 1, AttributeType::Red, 0)
    }
}

/// The worked example: cue 1 sets fixture 1, cue 3 mentions neither it nor
/// anything cue 5 touches, cue 5 sets fixture 1 again.
fn tracked_show() -> Show {
    let mut show = populated_show();
    show.store_sequence(sequence(
        1,
        vec![
            look("1", &[(1, 32_768)]),
            look("2", &[(2, 65_535)]),
            look("3", &[(3, 20_000)]),
            look("5", &[(1, 65_535)]),
        ],
    ))
    .expect("a sequence the populated show already has");
    show
}

fn row(show: &Show, number: &str) -> prism_domain::CueTrackingRow {
    show.cue_tracking(SequenceId::new(1))
        .into_iter()
        .find(|row| row.number == number)
        .unwrap_or_else(|| panic!("no row for cue {number}"))
}

/* -------------------------------------------------------------------------- */
/* The reading                                                                */
/* -------------------------------------------------------------------------- */

#[test]
fn a_cue_that_names_nothing_new_inherits_everything_above_it() {
    let show = tracked_show();
    let three = row(&show, "3");
    assert!(!three.blocks);
    let inherited: Vec<(u32, u16)> = three
        .inherited
        .iter()
        .map(|value| (value.fixture.get(), value.value))
        .collect();
    assert_eq!(
        inherited,
        vec![(1, 32_768), (2, 65_535)],
        "cue 3 does not inherit what cues 1 and 2 asserted"
    );
    // And what it asserts is not repeated back: the client already has it.
    assert!(
        !three.inherited.iter().any(|value| value.fixture.get() == 3),
        "the answer repeated the cue's own values back at the client"
    );
}

#[test]
fn the_first_cue_of_a_list_inherits_nothing_and_therefore_blocks() {
    // There is nothing above it, so it asserts everything by construction —
    // which is worth drawing rather than hiding, because it is what makes the
    // top of a list a rehearsable place.
    let show = tracked_show();
    let one = row(&show, "1");
    assert!(one.blocks);
    assert!(one.inherited.is_empty());
}

#[test]
fn the_answer_follows_playback_order_and_names_the_list_it_is_about() {
    let show = tracked_show();
    let numbers: Vec<String> = show
        .cue_tracking(SequenceId::new(1))
        .into_iter()
        .map(|row| row.number)
        .collect();
    assert_eq!(numbers, ["1", "2", "3", "5"]);
    // A list nobody has is no rows rather than an error: a window asking about a
    // list somebody has just deleted is a race, and `Query` has no refusal
    // shape (`docs/IPC_PROTOCOL.md` §5.2).
    assert!(show.cue_tracking(SequenceId::new(99)).is_empty());
}

/* -------------------------------------------------------------------------- */
/* Derived rather than stored                                                 */
/* -------------------------------------------------------------------------- */

#[test]
fn editing_cue_two_changes_what_cue_three_inherits_without_cue_three_being_touched() {
    // **The claim that says the state is derived.** Nothing writes a resolved
    // state into the show, so correcting an earlier cue corrects every cue after
    // it — which is exactly what a file that stored the resolved state could not
    // do.
    let mut show = tracked_show();
    let before = row(&show, "3");
    let cue_three_before = show.cue(SequenceId::new(1), "3").cloned();

    let mut sequence = show
        .sequence(SequenceId::new(1))
        .cloned()
        .expect("sequence 1");
    sequence.cues[1].parts[0].value = 111;
    show.store_sequence(sequence).expect("the list is there");

    let after = row(&show, "3");
    assert_ne!(before.inherited, after.inherited);
    assert_eq!(
        after
            .inherited
            .iter()
            .find(|value| value.fixture.get() == 2)
            .map(|value| value.value),
        Some(111)
    );
    assert_eq!(
        show.cue(SequenceId::new(1), "3").cloned(),
        cue_three_before,
        "cue 3 itself was rewritten, so the state is stored rather than derived"
    );
}

/* -------------------------------------------------------------------------- */
/* The edit                                                                   */
/* -------------------------------------------------------------------------- */

#[test]
fn cue_only_marks_every_value_of_the_cue_and_track_puts_it_back() {
    let mut show = tracked_show();
    show.set_cue_tracking(SequenceId::new(1), "3", CueTrackingMode::CueOnly)
        .expect("cue 3 is there");
    let three = show.cue(SequenceId::new(1), "3").expect("cue 3");
    assert!(
        three
            .parts
            .iter()
            .all(|part| part.tracking == CueTracking::CueOnly)
    );

    show.set_cue_tracking(SequenceId::new(1), "3", CueTrackingMode::Track)
        .expect("cue 3 is there");
    let three = show.cue(SequenceId::new(1), "3").expect("cue 3");
    assert!(
        three
            .parts
            .iter()
            .all(|part| part.tracking == CueTracking::Track)
    );
    // And no values were added or taken away by either: the two modes say what
    // the cue's values *do*, not what they are.
    assert_eq!(three.parts.len(), 1);
}

#[test]
fn blocking_writes_the_inherited_values_into_the_cue() {
    let mut show = tracked_show();
    assert!(!row(&show, "3").blocks);

    show.set_cue_tracking(SequenceId::new(1), "3", CueTrackingMode::Block)
        .expect("cue 3 is there");

    let three = show.cue(SequenceId::new(1), "3").expect("cue 3").clone();
    assert_eq!(
        three.parts.len(),
        3,
        "the inherited values were not written"
    );
    let mut written: Vec<(u32, u16)> = three
        .parts
        .iter()
        .map(|part| (part.fixture.get(), part.value))
        .collect();
    written.sort_unstable();
    assert_eq!(written, vec![(1, 32_768), (2, 65_535), (3, 20_000)]);
    // Every one of them tracks, because a value taken back at the end is not an
    // assertion and a blocking cue asserts everything.
    assert!(
        three
            .parts
            .iter()
            .all(|part| part.tracking == CueTracking::Track)
    );
    // A written-in value carries **no preset link**: it is the result of
    // somebody else's edit, and copying the link would make a later
    // `StorePreset` rewrite a cue nobody stored into.
    assert!(three.parts.iter().all(|part| part.preset_ref.is_none()));

    assert!(row(&show, "3").blocks);
    assert!(row(&show, "3").inherited.is_empty());
}

#[test]
fn nothing_reaches_past_a_blocking_cue() {
    // The point of blocking, said as the thing that stops happening: with cue 3
    // blocked, correcting cue 2 no longer changes what cue 3 or anything after
    // it inherits.
    let mut show = tracked_show();
    show.set_cue_tracking(SequenceId::new(1), "3", CueTrackingMode::Block)
        .expect("cue 3 is there");
    let five_before = row(&show, "5").inherited;

    let mut sequence = show
        .sequence(SequenceId::new(1))
        .cloned()
        .expect("sequence 1");
    sequence.cues[1].parts[0].value = 111;
    show.store_sequence(sequence).expect("the list is there");

    assert_eq!(
        row(&show, "5").inherited,
        five_before,
        "an edit above the block reached past it"
    );
    assert!(row(&show, "3").blocks);
}

#[test]
fn blocking_a_cue_that_already_asserts_everything_changes_nothing() {
    // The first cue of a list inherits nothing, so there is nothing to write —
    // and a command that writes nothing produces no operations at all, which is
    // `Show::set_cue_property`'s rule and an Oops step an operator would
    // otherwise press and watch do nothing.
    let mut show = tracked_show();
    let before = bytes(&show);
    let ops = show
        .set_cue_tracking(SequenceId::new(1), "1", CueTrackingMode::Block)
        .expect("cue 1 is there");
    assert!(ops.is_empty());
    assert_eq!(bytes(&show), before);
}

#[test]
fn a_cue_only_cue_hands_nothing_on_and_a_block_over_it_writes_what_is_underneath() {
    // The two halves meeting: cue 2 holds fixture 2 cue-only, so cue 3 does not
    // inherit it, and blocking cue 3 writes only what actually reaches it.
    let mut show = tracked_show();
    show.set_cue_tracking(SequenceId::new(1), "2", CueTrackingMode::CueOnly)
        .expect("cue 2 is there");
    let three = row(&show, "3");
    assert_eq!(
        three
            .inherited
            .iter()
            .map(|value| value.fixture.get())
            .collect::<Vec<_>>(),
        vec![1],
        "a cue-only value was handed on to the next cue"
    );

    show.set_cue_tracking(SequenceId::new(1), "3", CueTrackingMode::Block)
        .expect("cue 3 is there");
    let mut written: Vec<u32> = show
        .cue(SequenceId::new(1), "3")
        .expect("cue 3")
        .parts
        .iter()
        .map(|part| part.fixture.get())
        .collect();
    written.sort_unstable();
    assert_eq!(written, vec![1, 3]);
}

#[test]
fn a_show_saved_and_reloaded_resolves_to_the_same_thing() {
    // The state is derived, so nothing about it is written — which means the
    // claim to check is that the **edits** survive: a cue-only mark, and the
    // values a block wrote. Round-tripped through the encoding a `.prism` file
    // keeps each sequence in (S15).
    let mut show = tracked_show();
    show.set_cue_tracking(SequenceId::new(1), "2", CueTrackingMode::CueOnly)
        .expect("cue 2 is there");
    show.set_cue_tracking(SequenceId::new(1), "5", CueTrackingMode::Block)
        .expect("cue 5 is there");
    let before = show.cue_tracking(SequenceId::new(1));

    let packed = rmp_serde::to_vec_named(&show).expect("a show encodes");
    let reopened: Show = rmp_serde::from_slice(&packed).expect("and decodes");
    assert_eq!(
        reopened.cue_tracking(SequenceId::new(1)),
        before,
        "the show resolves differently after a save and a reload"
    );
    assert!(
        reopened
            .cue(SequenceId::new(1), "2")
            .expect("cue 2")
            .parts
            .iter()
            .all(|part| part.tracking == CueTracking::CueOnly)
    );
}

/* -------------------------------------------------------------------------- */
/* Refusals                                                                   */
/* -------------------------------------------------------------------------- */

#[test]
fn a_cue_or_a_list_that_is_not_there_is_refused_and_changes_nothing() {
    let mut show = tracked_show();
    let before = bytes(&show);
    assert_eq!(
        show.set_cue_tracking(SequenceId::new(99), "1", CueTrackingMode::Block),
        Err(ShowError::UnknownSequence(SequenceId::new(99)))
    );
    assert_eq!(
        show.set_cue_tracking(SequenceId::new(1), "4", CueTrackingMode::CueOnly),
        Err(ShowError::UnknownCue {
            sequence: SequenceId::new(1),
            number: "4".to_owned(),
        })
    );
    assert_eq!(bytes(&show), before);
}

#[test]
fn the_cue_number_is_trimmed_because_an_operator_typed_it() {
    let mut show = tracked_show();
    show.set_cue_tracking(SequenceId::new(1), "  3 ", CueTrackingMode::CueOnly)
        .expect("the number is the same cue with or without the spaces");
    assert!(
        show.cue(SequenceId::new(1), "3")
            .expect("cue 3")
            .parts
            .iter()
            .all(|part| part.tracking == CueTracking::CueOnly)
    );
}

/* -------------------------------------------------------------------------- */
/* Through the command path                                                   */
/* -------------------------------------------------------------------------- */

#[test]
fn the_command_is_a_show_edit_and_an_oops_puts_it_back() {
    let mut file = ShowFile {
        show: tracked_show(),
        ..ShowFile::new()
    };
    let before = bytes(&file.show);

    file.apply(&Command::SetCueTracking {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "3".to_owned(),
        tracking: CueTrackingMode::Block,
    })
    .expect("cue 3 is there");
    assert_eq!(
        file.show
            .cue(SequenceId::new(1), "3")
            .expect("cue 3")
            .parts
            .len(),
        3
    );

    file.apply(&Command::Oops).expect("a show edit is undoable");
    assert_eq!(
        bytes(&file.show),
        before,
        "an Oops did not put the cue back as it was"
    );

    file.apply(&Command::Redo).expect("and forward again");
    assert_eq!(
        file.show
            .cue(SequenceId::new(1), "3")
            .expect("cue 3")
            .parts
            .len(),
        3
    );
}

#[test]
fn a_command_that_changes_nothing_leaves_the_show_byte_identical() {
    let mut file = ShowFile {
        show: tracked_show(),
        ..ShowFile::new()
    };
    let before = bytes(&file.show);
    file.apply(&Command::SetCueTracking {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "3".to_owned(),
        tracking: CueTrackingMode::Track,
    })
    .expect("cue 3 is there and already tracks");
    assert_eq!(bytes(&file.show), before);
}
