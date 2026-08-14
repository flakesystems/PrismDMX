//! The Open Fixture Library, read as a whole — when it is there.
//!
//! # Why this target skips itself
//!
//! The library is **downloaded at install time and not committed**
//! (`profiles/fixtures/SOURCE.md`), so a fresh clone does not have it and
//! `cargo test` on that clone must still pass. Every test here therefore checks
//! for the directory first and returns if it is absent — printing why, so a
//! silent skip cannot be mistaken for a pass.
//!
//! CI installs it (`tools/fetch-fixtures`), so **these run there on every
//! commit**. What they are for is the thing unit tests over hand-written JSON
//! cannot give: the whole corpus, with every shape a real library has in it.
//!
//! # What is asserted, and what is only reported
//!
//! Asserted: that every profile the reader produces is one the *show model*
//! accepts, that keys are unique, that no attribute lands outside its footprint,
//! and that a named fixture converts to exactly the channels its manufacturer's
//! manual gives it.
//!
//! Reported: the conversion counts. They are printed rather than pinned to
//! numbers, because the library moves upstream and a test that failed whenever
//! somebody added a fixture would teach people to raise the number without
//! reading it. What is pinned is the *shape* — that most modes convert, and that
//! the losses stay in proportion.

// This target prints a report for a person to read.
#![allow(clippy::print_stdout)]

use std::path::{Path, PathBuf};

use prism_core::{FixtureLibrary, Show};

/// Where the installer puts it, relative to this crate.
fn library_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("profiles/fixtures")
}

/// The installed library, or `None` with a reason printed.
fn installed() -> Option<FixtureLibrary> {
    let root = library_root();
    if !root.join("manufacturers.json").is_file() {
        println!(
            "skipping: no fixture library at {} — run tools/fetch-fixtures/fetch-fixtures.sh",
            root.display()
        );
        return None;
    }
    let mut library = FixtureLibrary::default();
    library.read_ofl_tree(&root);
    Some(library)
}

/// **Every profile the reader produces is one the show model accepts.**
///
/// The strongest thing that can be said about a converter whose output is fed
/// to a validator: run the whole corpus through `embed_fixture_type`, which is
/// the same door `Command::EmbedFixtureType` goes through. An attribute outside
/// its footprint, a duplicate, an empty footprint — any of them, in any of two
/// thousand profiles, fails here.
#[test]
fn every_profile_in_the_installed_library_is_one_a_show_accepts() {
    let Some(library) = installed() else { return };
    assert!(library.len() > 1000, "only {} profiles", library.len());

    let mut show = Show::new();
    for entry in library.entries() {
        let profile = library
            .profile(&entry.id)
            .unwrap_or_else(|| panic!("{} is in the entries and not in the profiles", entry.id));
        show.embed_fixture_type(profile.clone())
            .unwrap_or_else(|error| panic!("{}: {error}", entry.id));
    }
    assert_eq!(show.fixture_types().count(), library.len());

    // And every attribute is inside its own footprint, coarse and fine — which
    // `embed_fixture_type` checks, and which is worth saying separately because
    // it is the one way a converter can be wrong that a patch cannot survive.
    for entry in library.entries() {
        let profile = library.profile(&entry.id).expect("it is there");
        assert_eq!(profile.footprint, entry.footprint);
        for def in &profile.attributes {
            for offset in [Some(def.coarse_offset), def.fine_offset]
                .into_iter()
                .flatten()
            {
                assert!(
                    offset < profile.footprint,
                    "{} puts {:?} at {offset}, outside {}",
                    entry.id,
                    def.attribute,
                    profile.footprint
                );
            }
        }
    }
}

/// A key names one profile, so `EmbedFixtureType` is never ambiguous.
#[test]
fn every_key_in_the_installed_library_is_its_own() {
    let Some(library) = installed() else { return };
    let mut keys: Vec<&str> = library
        .entries()
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    let total = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), total, "a key names two profiles");
}

/// **A named fixture converts to the channels its manual gives it.**
///
/// The one assertion in this file that is about a *specific* fixture, and it is
/// the one that would catch a reader that had drifted: the Stage Right wash is
/// the fixture S44 was asked for, its 9-channel mode is written out in the OFL
/// file as Pan, Tilt, Dimmer/Strobe, Red, Green, Blue, White, Pan/Tilt Speed,
/// Reset — and that is what the desk has to put on the wire.
#[test]
fn a_known_fixture_lands_on_the_channels_its_manual_gives_it() {
    let Some(library) = installed() else { return };
    let key = "stage-right/stage-wash-7x10w-led-moving-head/9ch";
    let Some(profile) = library.profile(key) else {
        // Upstream may rename or retire a fixture. That is not this desk's
        // defect, and failing here would make an upstream edit look like one.
        println!("skipping: {key} is not in this revision of the library");
        return;
    };
    assert_eq!(profile.footprint, 9);
    assert_eq!(profile.manufacturer, "Stage Right");
    let at = |attribute: prism_domain::AttributeType| {
        profile
            .attributes
            .iter()
            .find(|def| def.attribute == attribute)
            .unwrap_or_else(|| panic!("{attribute:?} is missing from {key}"))
            .coarse_offset
    };
    use prism_domain::AttributeType::{Blue, Dimmer, Green, Pan, Red, Tilt, White};
    assert_eq!(
        [
            at(Pan),
            at(Tilt),
            at(Dimmer),
            at(Red),
            at(Green),
            at(Blue),
            at(White)
        ],
        [0, 1, 2, 3, 4, 5, 6],
        "the manufacturer's channel order"
    );
    // Pan turns 540° on this head, which the file states and the desk carries
    // centred on zero.
    let pan = profile
        .attributes
        .iter()
        .find(|def| def.attribute == Pan)
        .expect("it pans");
    assert_eq!((pan.physical_from, pan.physical_to), (-270.0, 270.0));
}

/// What the conversion costs, printed for a person and shaped for a test.
///
/// The numbers move when the library does, so what is asserted is the shape: the
/// great majority of modes convert, and the great majority of those control
/// something. A re-import that broke the reader would fail this; one that merely
/// added fixtures would not.
#[test]
fn the_conversion_reports_what_it_could_not_use() {
    let Some(library) = installed() else { return };
    let counts = library.conversion();
    println!(
        "\nOpen Fixture Library: {} fixtures, {} modes converted, {} profiles\n\
         \x20 skipped (matrix or switching channels): {}\n\
         \x20 modes that control nothing:             {}\n\
         \x20 attributes mapped:                      {}\n\
         \x20 channels with no attribute here:        {}\n\
         \x20 channels dropped as duplicates:         {}\n\
         \x20 channel names never defined:            {}\n\
         \x20 files rejected:                         {}\n\
         \x20 redirects followed:                     {}\n",
        counts.fixtures,
        counts.modes,
        library.len(),
        counts.modes_with_inserts,
        counts.modes_without_attributes,
        counts.attributes,
        counts.channels_unmapped,
        counts.channels_duplicate,
        counts.channels_undefined,
        counts.files_rejected,
        counts.redirects,
    );

    assert!(counts.fixtures > 500, "only {} fixtures", counts.fixtures);
    assert_eq!(
        counts.files_rejected, 0,
        "a file upstream ships did not parse"
    );
    let offered = counts.modes + counts.modes_with_inserts;
    assert!(
        counts.modes * 100 / offered >= 65,
        "only {}% of modes convert",
        counts.modes * 100 / offered
    );
    assert!(
        counts.modes_without_attributes * 100 / counts.modes <= 20,
        "{} of {} modes control nothing",
        counts.modes_without_attributes,
        counts.modes
    );
    // A channel name that resolves to nothing is the reader's failure rather
    // than the library's, so this one is held tight: it was 2 254 before
    // template channels were resolved and is a fraction of that now.
    assert!(
        counts.channels_undefined < counts.attributes / 10,
        "{} channel names resolved to nothing",
        counts.channels_undefined
    );
}

/// **A search over the whole corpus answers, and answers small.**
///
/// The reason the library left the snapshot (S44): an answer has to fit in a
/// 1 MiB frame. Asserted on the encoded bytes rather than on the count.
#[test]
fn a_search_over_the_whole_library_fits_in_a_frame() {
    let Some(library) = installed() else { return };
    for text in ["", "robe", "mac", "led", "a", "wash 600", "e"] {
        let matches = library.search(text, prism_core::MAX_SEARCH_LIMIT);
        assert!(
            matches.len() <= prism_core::MAX_SEARCH_LIMIT,
            "{text} answered with {}",
            matches.len()
        );
        let encoded = rmp_serde::to_vec_named(&matches).expect("it encodes");
        assert!(
            encoded.len() < 256 * 1024,
            "{text} answered with {} bytes, which is a large share of a frame",
            encoded.len()
        );
    }
    // And every match really does contain what was asked for.
    for entry in library.search("robe", prism_core::DEFAULT_SEARCH_LIMIT) {
        let haystack = format!(
            "{} {} {} {}",
            entry.manufacturer, entry.name, entry.mode, entry.id
        )
        .to_lowercase();
        assert!(haystack.contains("robe"), "{} does not match", entry.id);
    }
}
