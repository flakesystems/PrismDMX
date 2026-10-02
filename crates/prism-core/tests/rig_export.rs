//! The MVR export, against the Open Fixture Library as a whole — **B65**.
//!
//! The owner's condition for the export was that **the Open Fixture Library's
//! fixtures are not lost on the way out**: an `.mvr` carries a GDTF for every
//! profile, and the Library has none. So the desk writes one, and what this
//! target holds is that **every profile in the installed library survives being
//! written and read back** — the same footprint, the same attributes in the same
//! places — rather than the three or four a unit test would choose.
//!
//! # Why this target skips itself
//!
//! For `fixture_library.rs`'s reason: the corpus is downloaded at install time
//! and not committed, so a fresh clone must still pass. CI installs it.
//!
//! Beside it, the cases that need no corpus: where the manufacturer's own file
//! is copied, and where it is not.

// This target prints a report for a person to read.
#![allow(clippy::print_stdout)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use prism_core::library::gdtf;
use prism_core::{FixtureLibrary, ShowFile};
use prism_domain::{Fixture, FixtureId, FixtureType, UniverseId, Vec3};

fn library_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("profiles/fixtures")
}

fn installed() -> Option<FixtureLibrary> {
    let root = library_root();
    if !(root.join("ofl/manufacturers.json").is_file() || root.join("manufacturers.json").is_file())
    {
        println!(
            "skipping: no Open Fixture Library corpus under {} — run \
             tools/fetch-fixtures/fetch-ofl.sh",
            root.display()
        );
        return None;
    }
    let mut library = FixtureLibrary::default();
    library.read_installed_tree(&root);
    Some(library)
}

/// What a channel is, for comparing two profiles: its kind, which of the kind,
/// where it sits.
type Place = (String, u8, u16, Option<u16>);

fn places(profile: &FixtureType) -> Vec<Place> {
    let mut all: Vec<Place> = profile
        .attributes
        .iter()
        .map(|a| {
            (
                format!("{:?}", a.attribute),
                a.occurrence,
                a.coarse_offset,
                a.fine_offset,
            )
        })
        .collect();
    all.sort();
    all
}

/// **Every profile of the library is a GDTF the desk reads back as itself.**
#[test]
fn every_open_fixture_library_profile_survives_being_written_as_gdtf() {
    let Some(library) = installed() else {
        return;
    };
    // The library as one profile per mode; a GDTF is one per fixture.
    let mut devices: BTreeMap<(String, String), Vec<FixtureType>> = BTreeMap::new();
    for entry in library.entries() {
        if entry.gdtf {
            continue;
        }
        let Some(profile) = library.profile(&entry.id) else {
            continue;
        };
        devices
            .entry((profile.manufacturer.clone(), profile.name.clone()))
            .or_default()
            .push(profile);
    }
    assert!(
        devices.len() > 100,
        "the corpus is there but holds {} fixtures",
        devices.len()
    );

    let mut modes = 0_usize;
    let mut lost: Vec<String> = Vec::new();
    let mut range_drift = 0_usize;
    let mut default_drift = 0_usize;
    for ((manufacturer, name), profiles) in &devices {
        let refs: Vec<&FixtureType> = profiles.iter().collect();
        let Some(bytes) = gdtf::write::write_archive(&refs) else {
            lost.push(format!("{manufacturer} {name}: not written"));
            continue;
        };
        let (built, counts) = gdtf::read_archive(&bytes, true);
        if counts.files_rejected != 0 {
            lost.push(format!("{manufacturer} {name}: rejected by the reader"));
            continue;
        }
        // Two modes of one fixture with one name are one mode in a GDTF; the
        // writer keeps the first, so compare against the first of each name.
        let mut seen: Vec<&str> = Vec::new();
        for profile in &profiles[..] {
            if seen.contains(&profile.mode.as_str()) {
                continue;
            }
            seen.push(&profile.mode);
            modes += 1;
            let Some((_, read)) = built.iter().find(|(_, read)| read.mode == profile.mode) else {
                lost.push(format!(
                    "{manufacturer} {name} / {}: mode missing",
                    profile.mode
                ));
                continue;
            };
            if read.footprint != profile.footprint {
                lost.push(format!(
                    "{manufacturer} {name} / {}: footprint {} became {}",
                    profile.mode, profile.footprint, read.footprint
                ));
            }
            if places(read) != places(profile) {
                let (a, b) = (places(profile), places(read));
                let first = a.iter().zip(&b).find(|(x, y)| x != y);
                lost.push(format!(
                    "{manufacturer} {name} / {}: channels differ ({} vs {}; first {:?})",
                    profile.mode,
                    a.len(),
                    b.len(),
                    first
                ));
                continue;
            }
            let mut was_sorted: Vec<_> = profile.attributes.iter().collect();
            was_sorted.sort_by_key(|a| a.coarse_offset);
            let mut now_sorted: Vec<_> = read.attributes.iter().collect();
            now_sorted.sort_by_key(|a| a.coarse_offset);
            for (was, now) in was_sorted.iter().zip(&now_sorted) {
                if was.default_value.abs_diff(now.default_value) > 257 && was.fine_offset.is_none()
                {
                    default_drift += 1;
                }
                if was.ranges.len() != now.ranges.len()
                    || was
                        .ranges
                        .iter()
                        .zip(&now.ranges)
                        .any(|(a, b)| a.name != b.name)
                {
                    range_drift += 1;
                    if range_drift <= 6 {
                        println!(
                            "  ranges {manufacturer} {name} / {}: {:?} vs {:?}",
                            profile.mode,
                            was.ranges
                                .iter()
                                .map(|r| (r.name.as_str(), r.from, r.to))
                                .collect::<Vec<_>>(),
                            now.ranges
                                .iter()
                                .map(|r| (r.name.as_str(), r.from, r.to))
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    }
    println!(
        "{} OFL fixtures, {modes} modes written as GDTF and read back; {} lost, {range_drift} \
         channels whose named ranges differ, {default_drift} whose home value moved",
        devices.len(),
        lost.len()
    );
    for line in lost.iter().take(20) {
        println!("  {line}");
    }
    assert!(
        lost.is_empty(),
        "{} modes did not survive; the first: {:?}",
        lost.len(),
        lost.first()
    );
}

fn patched(profile: &FixtureType) -> ShowFile {
    let mut file = ShowFile::default();
    file.show.embed_fixture_type(profile.clone()).unwrap();
    file.show
        .patch_fixture(Fixture {
            id: FixtureId::new(1),
            name: "One".to_owned(),
            type_id: profile.id.clone(),
            universe: UniverseId::new(1),
            address: 1,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
            software_dimmer: true,
        })
        .unwrap();
    file
}

/// A small `.gdtf` on disk, which is what a venue's folder holds.
fn gdtf_on_disk(dir: &Path, footprint: u16) -> PathBuf {
    let channels: String = (1..=footprint)
        .map(|offset| {
            format!(
                r#"<DMXChannel DMXBreak="1" Offset="{offset}"><LogicalChannel Attribute="Dimmer">
                     <ChannelFunction Name="D" Attribute="Dimmer" PhysicalFrom="0" PhysicalTo="1"/>
                   </LogicalChannel></DMXChannel>"#
            )
        })
        .collect();
    // A second `Dimmer` is renumbered by the reader, which is all this needs:
    // what matters here is only the footprint.
    let description = format!(
        r#"<GDTF DataVersion="1.2"><FixtureType Name="Spot" Manufacturer="Acme" FixtureTypeID="">
             <DMXModes><DMXMode Name="Standard"><DMXChannels>{channels}</DMXChannels></DMXMode>
             </DMXModes></FixtureType></GDTF>"#
    );
    // A ZIP holding the description and a stand-in model, which the export has
    // to carry as it is.
    let mut writer = prism_core::library::zip::Writer::new();
    writer.add_deflated("description.xml", description.as_bytes());
    writer.add_stored("models/gltf/body.glb", b"glTF-bytes");
    let path = dir.join("Acme@Spot.gdtf");
    std::fs::write(&path, writer.finish().unwrap()).unwrap();
    path
}

/// The manufacturer's own file goes in **as it was published** when the show
/// holds exactly what that file says — and the plan names it.
#[test]
fn a_profile_the_library_holds_a_gdtf_of_is_exported_as_that_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = gdtf_on_disk(dir.path(), 1);
    let (built, _) = gdtf::read_archive_file(&path, true);
    assert_eq!(built.len(), 1);
    let (_, profile) = built.first().cloned().unwrap();

    let mut file = patched(&profile);
    file.library.file_imported(built, &path);

    let exported = file.export_rig().expect("a rig");
    assert_eq!(exported.report.published, 1);
    assert_eq!(exported.report.written, 0);

    let on_disk = std::fs::read(&path).unwrap();
    let plan = prism_core::library::zip::Archive::read(&exported.bytes).unwrap();
    assert_eq!(
        plan.file("Acme@Spot.gdtf").as_deref(),
        Some(on_disk.as_slice()),
        "byte for byte: its models and pictures are not the desk's to drop"
    );
}

/// **A show patched against an older revision is not exported with a newer
/// file**: the addresses written beside it were laid out for the footprint the
/// show embedded, and the library's file has since changed.
#[test]
fn a_show_that_embedded_an_older_revision_is_exported_from_what_it_embedded() {
    let dir = tempfile::tempdir().unwrap();
    let path = gdtf_on_disk(dir.path(), 1);
    let (built, _) = gdtf::read_archive_file(&path, true);
    let (_, profile) = built.first().cloned().unwrap();
    let mut file = patched(&profile);

    // The venue's file is revised afterwards: the mode is now two channels wide.
    let revised = gdtf_on_disk(dir.path(), 2);
    let (revised_built, _) = gdtf::read_archive_file(&revised, true);
    file.library.file_imported(revised_built, &revised);

    let exported = file.export_rig().expect("a rig");
    assert_eq!(exported.report.published, 0, "the file no longer agrees");
    assert_eq!(exported.report.written, 1);
    let (read, _, _) = prism_core::library::mvr::read_archive(&exported.bytes, true);
    assert_eq!(
        read[0].1.footprint, 1,
        "what the show patched is what the plan carries"
    );
}

/// And a profile the library holds **no** GDTF of — one that came from the Open
/// Fixture Library, or that a venue wrote by hand — is written.
#[test]
fn a_profile_with_no_gdtf_anywhere_is_written_and_none_is_dropped() {
    let profile = FixtureLibrary::generic()
        .entries()
        .first()
        .and_then(|entry| FixtureLibrary::generic().profile(&entry.id))
        .expect("the built-in generics are there");
    let file = patched(&profile);
    let exported = file.export_rig().expect("a rig");
    assert_eq!(exported.report.written, 1);
    assert_eq!(exported.report.published, 0);
    let (read, counts, rig) = prism_core::library::mvr::read_archive(&exported.bytes, true);
    assert_eq!(counts.profiles, 1);
    assert_eq!(counts.fixtures_without_profile, 0);
    assert_eq!(read[0].1.footprint, profile.footprint);
    assert_eq!(rig.fixtures.len(), 1);
}
