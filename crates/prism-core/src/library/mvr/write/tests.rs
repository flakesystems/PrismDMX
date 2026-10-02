//! What an exported rig is read back as — **B65**.
//!
//! The reader is the one S62 wrote and was held to bytes; every case here writes
//! a rig and reads the archive with it, so what is asserted is that **the desk's
//! export is the desk's import's input** — the one check a build machine has
//! against a planner it does not own. The matrix convention itself is the MVR
//! specification's, quoted in the module documentation, and is checked against a
//! case worked out by hand rather than against the function under test.

use proptest::prelude::*;

use prism_domain::{
    AttributeDef, AttributeType, Fixture, FixtureId, FixtureType, Orientation, UniverseId, Vec3,
    orientation,
};

use super::{matrix_of, rotation_of_matrix, write_archive};
use crate::library::gdtf;
use crate::library::mvr::read_archive;
use crate::library::zip::Archive;

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3 { x, y, z }
}

fn profile(manufacturer: &str, name: &str, mode: &str, channels: u16) -> FixtureType {
    FixtureType {
        id: format!("{manufacturer}/{name}/{mode}").to_lowercase(),
        manufacturer: manufacturer.to_owned(),
        name: name.to_owned(),
        mode: mode.to_owned(),
        footprint: channels,
        attributes: (0..channels)
            .map(|offset| AttributeDef {
                attribute: if offset == 0 {
                    AttributeType::Dimmer
                } else {
                    AttributeType::Raw
                },
                label: (offset > 0).then(|| format!("Knob {offset}")),
                occurrence: u8::try_from(offset.saturating_sub(1)).unwrap_or(0),
                feature_group: if offset == 0 {
                    AttributeType::Dimmer.feature_group()
                } else {
                    AttributeType::Raw.feature_group()
                },
                coarse_offset: offset,
                fine_offset: None,
                default_value: 0,
                merge_mode: AttributeType::Dimmer.default_merge_mode(),
                invert: false,
                physical_from: 0.0,
                physical_to: 1.0,
                ranges: Vec::new(),
                switched: None,
            })
            .collect(),
        physical: None,
    }
}

fn fixture(id: u32, name: &str, of: &FixtureType, universe: u32, address: u16) -> Fixture {
    Fixture {
        id: FixtureId::new(id),
        name: name.to_owned(),
        type_id: of.id.clone(),
        universe: UniverseId::new(universe),
        address,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        invert_pan: false,
        invert_tilt: false,
        software_dimmer: true,
    }
}

fn nothing_published(_: &[&FixtureType]) -> Option<Vec<u8>> {
    None
}

/// The matrix a planner writes for **a quarter turn about the vertical**, from
/// the specification's own words: `u` is where the fixture's X axis goes. A
/// fixture turned a quarter of the way round with the stage's vertical as its
/// axis sends X to ±Y and leaves Z alone.
///
/// Worked out by hand and not by the function under test: this desk's
/// `rotation.y` turns the *other* way (its frame is left handed), so +90 here is
/// MVR's −90 about Z — which is `u = {0,-1,0}`, `v = {1,0,0}`.
#[test]
fn a_quarter_turn_about_the_vertical_is_the_matrix_the_specification_gives() {
    let text = matrix_of(Vec3::ZERO, v(0.0, 90.0, 0.0));
    let groups = super::groups_of(&text).expect("four groups");
    let close = |group: [f64; 3], expected: [f64; 3]| {
        group
            .iter()
            .zip(expected)
            .all(|(a, b)| (a - b).abs() < 1e-9)
    };
    assert!(close(groups[0], [0.0, -1.0, 0.0]), "u: {text}");
    assert!(close(groups[1], [1.0, 0.0, 0.0]), "v: {text}");
    assert!(close(groups[2], [0.0, 0.0, 1.0]), "w: {text}");
    // And a quarter turn the other way is the mirror of it.
    let back = super::groups_of(&matrix_of(Vec3::ZERO, v(0.0, -90.0, 0.0))).unwrap();
    assert!(close(back[0], [0.0, 1.0, 0.0]));
}

#[test]
fn a_fixture_that_is_not_turned_has_the_identity_and_its_place_in_millimetres() {
    assert_eq!(
        matrix_of(v(1.0, 6.5, -2.25), Vec3::ZERO),
        "{1,0,0}{0,1,0}{0,0,1}{1000,-2250,6500}",
        "x across, the desk's depth is MVR's Y, the desk's height is MVR's Z"
    );
}

fn same(a: &Orientation, b: &Orientation) -> bool {
    a.iter()
        .flatten()
        .zip(b.iter().flatten())
        .all(|(x, y)| (x - y).abs() < 1e-6)
}

proptest! {
    /// The way out and the way in are one convention: whatever the desk hangs a
    /// fixture as, the matrix written for it reads back as the same rotation
    /// (Euler triples are not unique, so the *matrix* is what is compared).
    #[test]
    fn a_rotation_written_is_the_rotation_read(
        x in -180.0..180.0_f64,
        y in -180.0..180.0_f64,
        z in -180.0..180.0_f64,
    ) {
        let rotation = v(x, y, z);
        let text = matrix_of(v(1.0, 2.0, 3.0), rotation);
        let back = rotation_of_matrix(&text).expect("a rotation");
        prop_assert!(same(&orientation(rotation), &orientation(back)), "{text}");
    }
}

#[test]
fn a_matrix_that_is_not_one_is_no_rotation() {
    assert_eq!(rotation_of_matrix(""), None);
    assert_eq!(rotation_of_matrix("{1,0,0}{0,1,0}"), None);
    assert_eq!(rotation_of_matrix("{0,0,0}{0,1,0}{0,0,1}{0,0,0}"), None);
}

#[test]
fn a_scaled_matrix_is_read_for_its_rotation() {
    // A planner that writes a 2x scale in every axis still hangs it the same way.
    let read = rotation_of_matrix("{2,0,0}{0,2,0}{0,0,2}{0,0,0}").expect("a rotation");
    assert!(same(&orientation(read), &orientation(Vec3::ZERO)));
}

/// **The whole round trip**: a rig written as an MVR is read back by S62's
/// reader as the same rig — the same fixtures, numbers, names, addresses, places
/// and facings, with every fixture's profile in the archive and resolved.
#[test]
fn a_rig_comes_back_as_the_rig_it_was() {
    let spot = profile("Acme", "Spot", "Standard", 3);
    let wash = profile("Acme", "Wash", "Basic", 2);
    let mut a = fixture(101, "Front <1>", &spot, 1, 1);
    a.position = v(-2.0, 6.0, 1.5);
    a.rotation = v(30.0, 45.0, -10.0);
    let mut b = fixture(7, "Back & Co", &wash, 3, 17);
    b.position = v(0.5, 5.0, 4.0);
    let mut c = fixture(8, "Back 2", &wash, 3, 19);
    c.rotation = v(0.0, 180.0, 0.0);

    let exported =
        write_archive([(&a, &spot), (&b, &wash), (&c, &wash)], &nothing_published).expect("a rig");
    assert_eq!(exported.report.fixtures, 3);
    assert_eq!(exported.report.profiles, 2, "one file per fixture type");
    assert_eq!(exported.report.written, 2);
    assert_eq!(exported.report.published, 0);

    let (profiles, counts, rig) = read_archive(&exported.bytes, true);
    assert_eq!(counts.profiles, 2);
    assert_eq!(counts.profiles_rejected, 0);
    assert_eq!(counts.fixtures, 3);
    assert_eq!(counts.fixtures_without_profile, 0);
    let ids: Vec<&str> = profiles
        .iter()
        .map(|(entry, _)| entry.id.as_str())
        .collect();
    assert!(ids.contains(&"acme/spot/standard"), "{ids:?}");
    assert!(ids.contains(&"acme/wash/basic"), "{ids:?}");

    assert_eq!(rig.fixtures.len(), 3);
    let first = &rig.fixtures[0];
    assert_eq!(first.name, "Front <1>");
    assert_eq!(first.fixture_id, Some(101));
    assert_eq!(first.mode, "Standard");
    assert_eq!(first.type_id.as_deref(), Some("acme/spot/standard"));
    assert_eq!(first.addresses.len(), 1);
    assert_eq!(first.addresses[0].universe_and_address(), Some((1, 1)));
    let position = first.position.expect("placed");
    assert!((position.x + 2.0).abs() < 1e-6 && (position.y - 6.0).abs() < 1e-6);
    assert!((position.z - 1.5).abs() < 1e-6);
    let rotation = first.rotation.expect("turned");
    assert!(same(&orientation(rotation), &orientation(a.rotation)));

    let second = &rig.fixtures[1];
    assert_eq!(second.name, "Back & Co");
    assert_eq!(second.addresses[0].absolute, 2 * 512 + 17);
    assert_eq!(second.addresses[0].universe_and_address(), Some((3, 17)));
    let third = &rig.fixtures[2];
    assert_eq!(third.addresses[0].universe_and_address(), Some((3, 19)));
    assert!(same(
        &orientation(third.rotation.expect("turned")),
        &orientation(v(0.0, 180.0, 0.0))
    ));
}

/// **The owner's condition**: a profile with no GDTF of its own — which is every
/// Open Fixture Library profile — is in the archive as a GDTF, with the channels
/// it had, so a planner can draw it and a re-import patches it.
#[test]
fn a_profile_with_no_gdtf_of_its_own_is_written_into_the_archive() {
    let mut spot = profile("Acme", "Spot", "Standard", 3);
    spot.attributes[1].ranges = vec![prism_domain::AttributeRange {
        name: "Open".to_owned(),
        from: 0,
        to: 2569,
        media: None,
    }];
    let f = fixture(1, "One", &spot, 1, 1);
    let exported = write_archive([(&f, &spot)], &nothing_published).expect("a rig");
    let (profiles, _, _) = read_archive(&exported.bytes, true);
    assert_eq!(profiles.len(), 1);
    let (_, read) = &profiles[0];
    assert_eq!(read.footprint, 3);
    assert_eq!(read.attributes[1].label.as_deref(), Some("Knob 1"));
    assert_eq!(read.attributes[1].ranges[0].name, "Open");
}

/// Where the library has the manufacturer's file, **that file** goes in, byte for
/// byte — its models and its gobo pictures are not the desk's to throw away.
#[test]
fn a_published_file_is_copied_as_it_was_published() {
    let spot = profile("Acme", "Spot", "Standard", 3);
    let f = fixture(1, "One", &spot, 1, 1);
    // A file with more in it than this desk writes: an extra member.
    let published = {
        let bytes = gdtf::write::write_archive(&[&spot]).expect("a file");
        let inner = Archive::read(&bytes).expect("an archive");
        let mut writer = crate::library::zip::Writer::new();
        writer.add_deflated(
            "description.xml",
            &inner.file("description.xml").expect("a description"),
        );
        writer.add_stored("models/gltf/body.glb", b"glTF-bytes");
        writer.finish().expect("an archive")
    };
    let asked = std::cell::RefCell::new(Vec::new());
    let exported = write_archive([(&f, &spot)], &|modes: &[&FixtureType]| {
        asked
            .borrow_mut()
            .push(modes.iter().map(|m| m.mode.clone()).collect::<Vec<_>>());
        Some(published.clone())
    })
    .expect("a rig");
    assert_eq!(exported.report.published, 1);
    assert_eq!(exported.report.written, 0);
    assert_eq!(asked.borrow().as_slice(), [vec!["Standard".to_owned()]]);

    let plan = Archive::read(&exported.bytes).expect("an archive");
    let member = plan
        .names()
        .find(|name| name.ends_with(".gdtf"))
        .expect("a profile")
        .to_owned();
    assert_eq!(member, "Acme@Spot.gdtf");
    assert_eq!(plan.file(&member).as_deref(), Some(published.as_slice()));
}

#[test]
fn every_mode_of_one_fixture_is_asked_for_once_and_named_in_one_file() {
    let basic = profile("Acme", "Spot", "Basic", 2);
    let full = profile("Acme", "Spot", "Full", 4);
    let a = fixture(1, "A", &basic, 1, 1);
    let b = fixture(2, "B", &full, 1, 10);
    let c = fixture(3, "C", &full, 1, 20);
    let asked = std::cell::Cell::new(0);
    let exported = write_archive(
        [(&a, &basic), (&b, &full), (&c, &full)],
        &|modes: &[&FixtureType]| {
            asked.set(asked.get() + 1);
            assert_eq!(modes.len(), 2, "both modes, once each");
            None
        },
    )
    .expect("a rig");
    assert_eq!(asked.get(), 1);
    assert_eq!(exported.report.profiles, 1);
    let (_, _, rig) = read_archive(&exported.bytes, true);
    let modes: Vec<&str> = rig.fixtures.iter().map(|f| f.mode.as_str()).collect();
    assert_eq!(modes, ["Basic", "Full", "Full"]);
    assert!(rig.fixtures.iter().all(|f| f.type_id.is_some()));
}

#[test]
fn two_fixtures_whose_file_names_collide_get_two_files() {
    let one = profile("A/B", "Spot", "M", 1);
    let two = profile("A_B", "Spot", "M", 1);
    let a = fixture(1, "A", &one, 1, 1);
    let b = fixture(2, "B", &two, 1, 2);
    let exported = write_archive([(&a, &one), (&b, &two)], &nothing_published).expect("a rig");
    let plan = Archive::read(&exported.bytes).expect("an archive");
    let files: Vec<&str> = plan.names().filter(|n| n.ends_with(".gdtf")).collect();
    assert_eq!(files.len(), 2, "{files:?}");
    let (_, counts, rig) = read_archive(&exported.bytes, true);
    assert_eq!(counts.profiles, 2);
    assert!(
        rig.fixtures.iter().all(|f| f.type_id.is_some()),
        "each fixture finds its own file: {:?}",
        rig.fixtures
    );
}

#[test]
fn a_rig_with_nothing_patched_is_no_archive() {
    let none: [(&Fixture, &FixtureType); 0] = [];
    assert!(write_archive(none, &nothing_published).is_none());
}

#[test]
fn the_same_rig_is_the_same_file_twice() {
    let spot = profile("Acme", "Spot", "Standard", 3);
    let f = fixture(1, "One", &spot, 1, 1);
    let build = || {
        write_archive([(&f, &spot)], &nothing_published)
            .expect("a rig")
            .bytes
    };
    assert_eq!(build(), build());
}
