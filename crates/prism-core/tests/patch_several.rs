//! The patch window's arithmetic — S57, punch-list **B60** (GitHub #28).
//!
//! Three things the owner asked of the patch window are answers only the daemon
//! can give, because only the daemon holds the patch: **where the next free
//! address is** for a whole footprint, **where each of several new fixtures
//! would go**, and **that patching them is one step** an Oops takes back. The
//! browser draws what these say and works none of it out (D3), so the rules are
//! asserted here — over gaps, the ends of universes and footprints that do not
//! fit — and not in a browser.

use prism_core::{ShowError, ShowFile, ShowFileError};
use prism_domain::{Command, FixtureId, PatchAddress, PatchPlacement, PatchPreview, UniverseId};

/// A PAR: four channels, out of the desk's built-in library.
const PAR: &str = "generic.rgbw.par";
/// A moving head: thirteen channels.
const HEAD: &str = "generic.movinghead";

fn at(universe: u32, address: u16) -> PatchAddress {
    PatchAddress {
        universe: UniverseId::new(universe),
        address,
    }
}

fn placed(id: u32, universe: u32, address: u16) -> PatchPlacement {
    PatchPlacement {
        id: FixtureId::new(id),
        universe: UniverseId::new(universe),
        address,
    }
}

/// Patches one fixture the way S27's form does.
fn patch(file: &mut ShowFile, id: u32, type_id: &str, universe: u32, address: u16) {
    file.apply(&Command::EmbedFixtureType {
        type_id: type_id.to_owned(),
    })
    .unwrap();
    file.apply(&Command::PatchFixture {
        id: FixtureId::new(id),
        name: format!("Fixture {id}"),
        type_id: type_id.to_owned(),
        universe: UniverseId::new(universe),
        address,
        software_dimmer: true,
    })
    .unwrap();
}

/// What a new fixture at this place would do, asked as the form asks it.
fn new_at(
    file: &ShowFile,
    type_id: &str,
    id: u32,
    universe: u32,
    address: u16,
    adding: u16,
) -> PatchPreview {
    file.preview_patch(type_id, placed(id, universe, address), adding)
}

// -- the next free address ---------------------------------------------------

#[test]
fn an_empty_universe_is_free_from_where_it_was_asked() {
    let file = ShowFile::new();
    let preview = new_at(&file, PAR, 1, 1, 1, 1);
    assert!(preview.accepted, "{preview:?}");
    assert_eq!(
        preview.next_free,
        Some(at(1, 1)),
        "free is where it was asked"
    );
    let preview = new_at(&file, PAR, 1, 3, 77, 1);
    assert_eq!(preview.next_free, Some(at(3, 77)));
}

#[test]
fn the_next_free_address_fits_the_whole_footprint_and_skips_a_gap_too_small() {
    let mut file = ShowFile::new();
    // 1-4, then a gap of three channels, then 8-11.
    patch(&mut file, 1, PAR, 1, 1);
    patch(&mut file, 2, PAR, 1, 8);
    // A PAR asked for at 1 overlaps fixture 1, and the three-channel gap at 5-7
    // is **not** somewhere four channels go: the next free place is 12.
    let preview = new_at(&file, PAR, 3, 1, 1, 1);
    assert_eq!(preview.conflicts.len(), 1, "{preview:?}");
    assert_eq!(preview.next_free, Some(at(1, 12)));
    // A dimmer, one channel wide, fits in that gap.
    let preview = new_at(&file, "generic.dimmer", 3, 1, 1, 1);
    assert_eq!(preview.next_free, Some(at(1, 5)));
}

#[test]
fn the_next_free_address_runs_on_into_the_next_universe() {
    let mut file = ShowFile::new();
    // Universe 1 is full to 505, so seven channels are left at its end.
    for index in 0..38_u16 {
        patch(&mut file, u32::from(index) + 1, HEAD, 1, 1 + index * 13);
    }
    assert_eq!(38 * 13, 494);
    // A head does not fit in 495-512 twice, but once: 495-507.
    assert_eq!(
        new_at(&file, HEAD, 100, 1, 1, 1).next_free,
        Some(at(1, 495))
    );
    patch(&mut file, 39, HEAD, 1, 495);
    // Now five channels are left, and a head needs thirteen: universe 2, at 1.
    assert_eq!(new_at(&file, HEAD, 100, 1, 1, 1).next_free, Some(at(2, 1)));
    // A PAR still fits in what is left.
    assert_eq!(new_at(&file, PAR, 100, 1, 1, 1).next_free, Some(at(1, 508)));
}

#[test]
fn nothing_is_free_past_the_last_universe() {
    let mut file = ShowFile::new();
    patch(&mut file, 1, HEAD, 64, 500);
    // Asked for at the end of universe 64: nothing after it, and 500-512 is
    // taken, so a head has nowhere to go.
    assert_eq!(new_at(&file, HEAD, 2, 64, 500, 1).next_free, None);
    // And a key nobody carries cannot be measured at all.
    let preview = new_at(&file, "nothing.at.all", 2, 1, 1, 1);
    assert!(!preview.accepted);
    assert_eq!(preview.next_free, None);
}

#[test]
fn a_repatched_fixture_is_not_in_its_own_way() {
    let mut file = ShowFile::new();
    patch(&mut file, 1, PAR, 1, 1);
    // Fixture 1 asked about where it already is: free, and free right there.
    let preview = file.preview_patch(PAR, placed(1, 1, 1), 0);
    assert!(preview.conflicts.is_empty());
    assert_eq!(preview.next_free, Some(at(1, 1)));
    assert!(
        preview.placements.is_empty(),
        "a repatch places nothing new"
    );
}

// -- several at once -----------------------------------------------------------

#[test]
fn ten_new_fixtures_are_placed_one_after_the_other_around_what_is_there() {
    let mut file = ShowFile::new();
    patch(&mut file, 3, PAR, 1, 9);
    patch(&mut file, 5, HEAD, 1, 20);
    let preview = new_at(&file, PAR, 1, 1, 1, 10);
    assert!(preview.accepted, "{preview:?}");
    assert_eq!(
        preview.placements,
        vec![
            placed(1, 1, 1),
            placed(2, 1, 5),
            // 9-12 is fixture 3, 13-16 is free, 17-19 is too small.
            placed(4, 1, 13),
            // 20-32 is the head.
            placed(6, 1, 33),
            placed(7, 1, 37),
            placed(8, 1, 41),
            placed(9, 1, 45),
            placed(10, 1, 49),
            placed(11, 1, 53),
            placed(12, 1, 57),
        ],
        "numbers skip 3 and 5, places skip what they are patched at"
    );
}

#[test]
fn several_that_do_not_all_fit_are_placed_nowhere() {
    let file = ShowFile::new();
    // Three heads from 64.490: the first as asked, then universe 64 is full.
    let preview = new_at(&file, HEAD, 1, 64, 490, 3);
    assert!(preview.accepted);
    assert_eq!(
        preview.placements,
        Vec::new(),
        "seven of ten is not what was asked"
    );
    // One head from there is fine.
    assert_eq!(
        new_at(&file, HEAD, 1, 64, 490, 1).placements,
        vec![placed(1, 64, 490)]
    );
}

#[test]
fn more_than_a_gesture_may_carry_is_refused_before_it_is_placed() {
    let file = ShowFile::new();
    let preview = new_at(&file, "generic.dimmer", 1, 1, 1, 513);
    assert!(!preview.accepted);
    assert!(preview.placements.is_empty());
    assert!(new_at(&file, "generic.dimmer", 1, 1, 1, 512).accepted);
}

#[test]
fn a_new_fixture_may_not_take_a_number_that_is_patched() {
    let mut file = ShowFile::new();
    patch(&mut file, 1, PAR, 1, 1);
    let preview = new_at(&file, PAR, 1, 1, 100, 1);
    assert!(!preview.accepted);
    assert!(preview.placements.is_empty());
    assert!(
        preview
            .refusal
            .as_deref()
            .is_some_and(|why| why.contains('1')),
        "{preview:?}"
    );
}

// -- one gesture, one step ---------------------------------------------------

#[test]
fn patching_ten_of_one_fixture_is_one_undo_step_and_ten_that_do_not_overlap() {
    let mut file = ShowFile::new();
    let before = rmp_serde::to_vec_named(&file).unwrap();
    let placements = new_at(&file, PAR, 1, 1, 1, 10).placements;
    assert_eq!(placements.len(), 10);

    file.apply(&Command::PatchFixtures {
        type_id: PAR.to_owned(),
        name: String::new(),
        software_dimmer: true,
        placements,
    })
    .unwrap();
    assert_eq!(file.show.fixtures().count(), 10);
    assert!(
        file.show.conflicts().is_empty(),
        "{:?}",
        file.show.conflicts()
    );
    assert!(
        file.show.fixture_type(PAR).is_some(),
        "the profile came with them"
    );
    // Named after the type, and told apart by their place in the gesture.
    let names: Vec<String> = file
        .show
        .fixtures()
        .map(|fixture| fixture.name.clone())
        .collect();
    assert_eq!(names[0], "RGBW PAR 1");
    assert_eq!(names[9], "RGBW PAR 10");

    // **One** Oops takes back all ten and the profile they stand on.
    file.apply(&Command::Oops).unwrap();
    assert_eq!(file.show.fixtures().count(), 0);
    assert!(file.show.fixture_type(PAR).is_none());
    assert_eq!(rmp_serde::to_vec_named(&file).unwrap(), before);
    // And there was only the one step to take.
    assert!(file.apply(&Command::Oops).is_err());

    // Redo puts the profile back before the fixtures that stand on it.
    file.apply(&Command::Redo).unwrap();
    assert_eq!(file.show.fixtures().count(), 10);
    assert!(file.show.conflicts().is_empty());
}

#[test]
fn a_refused_gesture_writes_nothing() {
    let mut file = ShowFile::new();
    patch(&mut file, 2, PAR, 1, 1);
    let before = rmp_serde::to_vec_named(&file).unwrap();

    // The second of three is on a number that is patched: nothing is written,
    // not even the first, and not even the profile.
    let error = file
        .apply(&Command::PatchFixtures {
            type_id: HEAD.to_owned(),
            name: "Spot".to_owned(),
            software_dimmer: true,
            placements: vec![placed(1, 2, 1), placed(2, 2, 20), placed(3, 2, 40)],
        })
        .unwrap_err();
    assert_eq!(
        error,
        ShowFileError::Show(ShowError::FixtureNumberInUse(FixtureId::new(2)))
    );
    assert_eq!(rmp_serde::to_vec_named(&file).unwrap(), before);

    // Two with one number, one that runs past the end, none, and more than a
    // gesture may carry: refused alike.
    let too_many: Vec<PatchPlacement> = (1..=513).map(|id| placed(100 + id, 3, 1)).collect();
    for placements in [
        vec![placed(7, 2, 1), placed(7, 2, 20)],
        vec![placed(7, 2, 505)],
        Vec::new(),
        too_many,
    ] {
        assert!(
            file.apply(&Command::PatchFixtures {
                type_id: HEAD.to_owned(),
                name: String::new(),
                software_dimmer: true,
                placements,
            })
            .is_err()
        );
        assert_eq!(rmp_serde::to_vec_named(&file).unwrap(), before);
    }
}

#[test]
fn a_typed_name_is_kept_and_numbered_only_when_there_are_several() {
    let mut file = ShowFile::new();
    file.apply(&Command::PatchFixtures {
        type_id: PAR.to_owned(),
        name: "Front".to_owned(),
        software_dimmer: true,
        placements: vec![placed(1, 1, 1)],
    })
    .unwrap();
    file.apply(&Command::PatchFixtures {
        type_id: PAR.to_owned(),
        name: "Back".to_owned(),
        software_dimmer: true,
        placements: vec![placed(2, 1, 5), placed(3, 1, 9)],
    })
    .unwrap();
    let names: Vec<&str> = file
        .show
        .fixtures()
        .map(|fixture| fixture.name.as_str())
        .collect();
    assert_eq!(names, vec!["Front", "Back 1", "Back 2"]);
}

#[test]
fn a_fixture_patched_with_no_name_is_named_after_its_type() {
    let mut file = ShowFile::new();
    file.apply(&Command::EmbedFixtureType {
        type_id: HEAD.to_owned(),
    })
    .unwrap();
    file.apply(&Command::PatchFixture {
        id: FixtureId::new(4),
        name: "  ".to_owned(),
        type_id: HEAD.to_owned(),
        universe: UniverseId::new(1),
        address: 1,
        software_dimmer: true,
    })
    .unwrap();
    assert_eq!(
        file.show.fixture(FixtureId::new(4)).unwrap().name,
        "Moving Head"
    );
}

#[test]
fn a_new_fixture_is_previewed_from_the_library_without_embedding_it() {
    // What lets the library be browsed without leaving profiles behind: the
    // show has none, and the preview measures the library's copy.
    let file = ShowFile::new();
    let preview = new_at(&file, HEAD, 1, 1, 1, 1);
    assert!(preview.accepted);
    assert_eq!(preview.footprint, 13);
    assert!(
        file.show.fixture_type(HEAD).is_none(),
        "asking embedded nothing"
    );
}
