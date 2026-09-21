//! What an `.mvr` becomes — **S62**.
//!
//! Every archive here is built **byte by byte**, a ZIP holding ZIPs, for the
//! reason the GDTF tests build theirs: there is no corpus to download, and a
//! test that shared a library's opinion of the bytes would be agreeing with the
//! reader rather than checking it.

use super::{RigAddress, read_archive};
use crate::library::zip::testkit::Builder;

/// An archive of stored members — the shape every test here needs, built by
/// `zip::testkit` so nothing is produced by a ZIP library.
fn stored_archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    files
        .iter()
        .fold(Builder::new(), |builder, (name, body)| {
            builder.stored(name, body)
        })
        .build()
}

/// The same, deflated, which is what a real exporter writes.
fn deflated_archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    files
        .iter()
        .fold(Builder::new(), |builder, (name, body)| {
            builder.deflated(name, body)
        })
        .build()
}

/// A `description.xml` for a fixture with one mode of `footprint` dimmers.
fn gdtf(manufacturer: &str, name: &str, footprint: u16) -> String {
    let channels: String = (1..=footprint)
        .map(|offset| {
            format!(
                r#"<DMXChannel Offset="{offset}">
                     <LogicalChannel Attribute="Dimmer">
                       <ChannelFunction Attribute="Dimmer"/>
                     </LogicalChannel>
                   </DMXChannel>"#
            )
        })
        .collect();
    format!(
        r#"<GDTF DataVersion="1.2">
             <FixtureType Name="{name}" Manufacturer="{manufacturer}" FixtureTypeID="GUID">
               <DMXModes>
                 <DMXMode Name="Mode 1">
                   <DMXChannels>{channels}</DMXChannels>
                 </DMXMode>
               </DMXModes>
             </FixtureType>
           </GDTF>"#
    )
}

/// A whole `GeneralSceneDescription.xml` around a list of fixtures.
fn scene(fixtures: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
           <GeneralSceneDescription verMajor="1" verMinor="6" provider="Test">
             <Scene>
               <Layers>
                 <Layer uuid="L1" name="Stage">
                   <ChildList>{fixtures}</ChildList>
                 </Layer>
               </Layers>
             </Scene>
           </GeneralSceneDescription>"#
    )
}

/// One `<Fixture>`, as an exporter writes one.
fn fixture(name: &str, spec: &str, mode: &str, id: u32, address: u32) -> String {
    format!(
        r#"<Fixture uuid="F-{id}" name="{name}">
             <GDTFSpec>{spec}</GDTFSpec>
             <GDTFMode>{mode}</GDTFMode>
             <FixtureID>{id}</FixtureID>
             <Addresses><Address break="1">{address}</Address></Addresses>
             <Matrix>{{1,0,0}}{{0,1,0}}{{0,0,1}}{{1500,4000,6200}}</Matrix>
           </Fixture>"#
    )
}

/// **The whole point of the module**: a plan's profiles land in the library.
#[test]
fn an_mvr_yields_the_profiles_of_every_fixture_in_the_plan() {
    let document = scene(&format!(
        "{}{}",
        fixture("Wash 1", "Robe@Wash.gdtf", "Mode 1", 1, 1),
        fixture("Wash 2", "Robe@Wash.gdtf", "Mode 1", 2, 5),
    ));
    let archive = stored_archive(&[
        ("GeneralSceneDescription.xml", document.as_bytes()),
        (
            "Robe@Wash.gdtf",
            &stored_archive(&[("description.xml", gdtf("Robe", "Wash", 4).as_bytes())]),
        ),
    ]);

    let (built, counts, rig) = read_archive(&archive, true);
    assert_eq!(counts.profiles, 1, "one profile, used by two fixtures");
    assert_eq!(counts.profiles_rejected, 0);
    assert_eq!(counts.fixtures, 2);
    assert_eq!(counts.fixtures_without_profile, 0);

    assert_eq!(built.len(), 1, "one mode of one fixture type");
    let (entry, profile) = &built[0];
    assert_eq!(
        entry.id, "robe/wash/mode-1",
        "the key comes out of the profile"
    );
    assert!(entry.own, "an imported plan is the venue's own");
    assert!(entry.gdtf);
    assert_eq!(profile.footprint, 4);

    // And the plan is read, without anything being done about it.
    assert_eq!(rig.fixtures.len(), 2);
    assert_eq!(rig.fixtures[0].name, "Wash 1");
    assert_eq!(rig.fixtures[0].spec, "Robe@Wash.gdtf");
    assert_eq!(rig.fixtures[0].mode, "Mode 1");
    assert_eq!(rig.fixtures[0].fixture_id, Some(1));
    assert_eq!(
        rig.fixtures[0].addresses,
        [RigAddress {
            break_number: 1,
            absolute: 1
        }]
    );
}

/// **A deflated archive**, which is what every real exporter writes.
#[test]
fn a_deflated_plan_reads_the_same_as_a_stored_one() {
    let document = scene(&fixture("Head", "Maker@Head.gdtf", "Mode 1", 1, 513));
    let archive = deflated_archive(&[
        ("GeneralSceneDescription.xml", document.as_bytes()),
        (
            "Maker@Head.gdtf",
            &deflated_archive(&[("description.xml", gdtf("Maker", "Head", 2).as_bytes())]),
        ),
    ]);

    let (built, counts, rig) = read_archive(&archive, true);
    assert_eq!(counts.profiles, 1);
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].0.id, "maker/head/mode-1");
    // 513 is the first address of the second universe, and this is the one
    // place that arithmetic happens.
    assert_eq!(
        rig.fixtures[0].addresses[0].universe_and_address(),
        Some((2, 1))
    );
}

/// An address is one absolute number, and a desk patches a universe and a slot.
#[test]
fn an_absolute_address_becomes_a_universe_and_an_address() {
    let at = |absolute| {
        RigAddress {
            break_number: 1,
            absolute,
        }
        .universe_and_address()
    };
    assert_eq!(at(1), Some((1, 1)));
    assert_eq!(at(512), Some((1, 512)));
    assert_eq!(at(513), Some((2, 1)));
    assert_eq!(at(1024), Some((2, 512)));
    assert_eq!(at(1025), Some((3, 1)));
    // Nought is not a patch — an exporter writes it for a fixture nobody has
    // addressed yet, and a desk that read it as 1.1 would invent a patch.
    assert_eq!(at(0), None);
}

/// **A plan may name a profile it does not carry**, and that is not a fault.
///
/// The format allows an exporter to leave out what it expects the other end to
/// have. What must not happen is that the archive is thrown away over it.
#[test]
fn a_fixture_whose_profile_is_missing_is_counted_and_the_rest_still_reads() {
    let document = scene(&format!(
        "{}{}",
        fixture("Present", "There@Yes.gdtf", "Mode 1", 1, 1),
        fixture("Absent", "Nowhere@No.gdtf", "Mode 1", 2, 20),
    ));
    let archive = stored_archive(&[
        ("GeneralSceneDescription.xml", document.as_bytes()),
        (
            "There@Yes.gdtf",
            &stored_archive(&[("description.xml", gdtf("There", "Yes", 1).as_bytes())]),
        ),
    ]);

    let (built, counts, rig) = read_archive(&archive, true);
    assert_eq!(counts.fixtures, 2);
    assert_eq!(counts.fixtures_without_profile, 1);
    assert_eq!(built.len(), 1, "the one that is there still arrives");
    assert_eq!(rig.fixtures.len(), 2, "the plan still says what it says");
}

/// Fixtures nested in groups are fixtures.
#[test]
fn fixtures_are_found_however_deeply_the_plan_nested_them() {
    let inner = fixture("Deep", "Maker@Head.gdtf", "Mode 1", 7, 1);
    let document = scene(&format!(
        r#"<GroupObject uuid="G1" name="Truss">
             <ChildList>
               <GroupObject uuid="G2" name="Bar"><ChildList>{inner}</ChildList></GroupObject>
             </ChildList>
           </GroupObject>"#
    ));
    let archive = stored_archive(&[
        ("GeneralSceneDescription.xml", document.as_bytes()),
        (
            "Maker@Head.gdtf",
            &stored_archive(&[("description.xml", gdtf("Maker", "Head", 1).as_bytes())]),
        ),
    ]);

    let (_, counts, rig) = read_archive(&archive, true);
    assert_eq!(counts.fixtures, 1);
    assert_eq!(rig.fixtures[0].name, "Deep");
    assert_eq!(rig.fixtures[0].fixture_id, Some(7));
}

/// A profile in a subdirectory is the profile its `GDTFSpec` names.
///
/// Exporters differ: some write the profiles at the root, some under a folder.
/// A `GDTFSpec` names the file, so the match is on the file name and not the
/// path — otherwise every fixture of an archive laid out the second way would
/// count as missing its profile.
#[test]
fn a_profile_in_a_subdirectory_still_matches_its_spec() {
    let document = scene(&fixture("Head", "Maker@Head.gdtf", "Mode 1", 1, 1));
    let archive = stored_archive(&[
        ("GeneralSceneDescription.xml", document.as_bytes()),
        (
            "gdtf/Maker@Head.gdtf",
            &stored_archive(&[("description.xml", gdtf("Maker", "Head", 1).as_bytes())]),
        ),
    ]);

    let (built, counts, _) = read_archive(&archive, true);
    assert_eq!(built.len(), 1);
    assert_eq!(
        counts.fixtures_without_profile, 0,
        "the path is not the name"
    );
}

/// Where a fixture hangs, in this desk's axes.
///
/// MVR states the translation in **millimetres** and is **Z-up**; this desk is
/// Y-up and metres. `{1500,4000,6200}` is 1.5 m across, 4 m upstage and 6.2 m
/// up — so it arrives as x 1.5, y 6.2, z 4.
#[test]
fn a_position_arrives_in_metres_and_in_this_desks_axes() {
    let document = scene(&fixture("Head", "Maker@Head.gdtf", "Mode 1", 1, 1));
    let archive = stored_archive(&[
        ("GeneralSceneDescription.xml", document.as_bytes()),
        (
            "Maker@Head.gdtf",
            &stored_archive(&[("description.xml", gdtf("Maker", "Head", 1).as_bytes())]),
        ),
    ]);

    let (_, _, rig) = read_archive(&archive, true);
    let position = rig.fixtures[0].position.expect("the plan states one");
    assert!((position.x - 1.5).abs() < 1e-9, "{position:?}");
    assert!((position.y - 6.2).abs() < 1e-9, "{position:?}");
    assert!((position.z - 4.0).abs() < 1e-9, "{position:?}");
}

/// A file that is not an archive is nothing, and does not panic.
#[test]
fn something_that_is_not_an_archive_yields_nothing() {
    let (built, counts, rig) = read_archive(b"not a zip at all", true);
    assert!(built.is_empty());
    assert_eq!(counts, super::Conversion::default());
    assert!(rig.fixtures.is_empty());
}

/// An archive with no scene document still yields its profiles.
///
/// A library is the point; the plan is the extra. An exporter that wrote a bare
/// bag of profiles should still fill a library.
#[test]
fn an_archive_without_a_scene_still_yields_its_profiles() {
    let archive = stored_archive(&[(
        "Maker@Head.gdtf",
        &stored_archive(&[("description.xml", gdtf("Maker", "Head", 3).as_bytes())]),
    )]);
    let (built, counts, rig) = read_archive(&archive, true);
    assert_eq!(built.len(), 1);
    assert_eq!(counts.profiles, 1);
    assert_eq!(counts.fixtures, 0);
    assert!(rig.fixtures.is_empty());
}

/// A member that ends in `.gdtf` and is not one is counted and left.
#[test]
fn a_member_that_is_not_a_profile_is_counted_and_left() {
    let archive = stored_archive(&[
        ("broken.gdtf", b"PK and then nothing"),
        (
            "good.gdtf",
            &stored_archive(&[("description.xml", gdtf("Maker", "Head", 1).as_bytes())]),
        ),
    ]);
    let (built, counts, _) = read_archive(&archive, true);
    assert_eq!(counts.profiles, 2);
    assert_eq!(counts.profiles_rejected, 1);
    assert_eq!(built.len(), 1);
}
