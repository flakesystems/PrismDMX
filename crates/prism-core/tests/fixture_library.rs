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

/// **What reading the whole library costs**, which S44's exit criteria ask to be
/// measured rather than assumed.
///
/// Printed rather than asserted against a millisecond figure: it is measured in
/// a *debug* build on whatever machine is running the suite, and a threshold
/// there would fail for the machine rather than for the code. What it is for is
/// the number going into `PROGRESS.md` — and it is what says whether this belongs
/// on the daemon's start-up path at all.
#[test]
fn reading_the_whole_library_is_timed() {
    let root = library_root();
    if !root.join("manufacturers.json").is_file() {
        println!("skipping: no fixture library installed");
        return;
    }
    let started = std::time::Instant::now();
    let mut library = FixtureLibrary::default();
    library.read_ofl_tree(&root);
    let elapsed = started.elapsed();
    println!(
        "
reading {} profiles from {} files took {:?} (debug build)
",
        library.len(),
        library.conversion().fixtures + library.conversion().redirects,
        elapsed
    );
    assert!(library.len() > 1000);
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
         \x20 channels the file says do nothing:      {}\n\
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
        counts.channels_without_function,
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

/// **No channel of the installed library is left without an attribute** —
/// punch-list entry **B38**, and the claim only the corpus can refute.
///
/// This is a number and not a proportion, and that is the change S51 made. It
/// was 5 037 — a third of every channel in the library — and each one of them
/// was a channel an operator could not reach: CMY colour mixing, every colour
/// wheel, every built-in effect, frost, fog, the framing shutters. The table in
/// `prism_core::library::ofl` now has a row for every capability type the
/// format defines, so the honest assertion is **nought**.
///
/// A channel whose every capability is OFL's `NoFunction` is counted
/// separately and is not a gap: the file is saying the channel does nothing.
///
/// # What this deliberately does not claim
///
/// It says nothing about `channels_duplicate`, and that is S51's stated
/// boundary rather than an oversight. A fixture with **two channels of one
/// kind** — a second colour wheel, a per-pixel red on a tube written out
/// longhand — still keeps the lower one and drops the higher, because this
/// model gives one fixture one of each parameter and `MergeError::
/// DuplicateAttribute` is what enforces it. Lifting that means giving an
/// attribute an *occurrence*, which is a change to the key every value in a
/// show is filed under — the programmer, the cue, the preset, the wire and the
/// console grammar — and it is a session of its own rather than a corner of
/// this one. `IMPLEMENTATION_PLAN.md` S52 carries it with the count below.
#[test]
fn every_channel_in_the_installed_library_maps_to_an_attribute() {
    let Some(library) = installed() else { return };
    let counts = library.conversion();
    assert_eq!(
        counts.channels_unmapped, 0,
        "{} channels of the installed library reach no attribute at all — a capability type \
         upstream has that `prism_core::library::ofl` does not",
        counts.channels_unmapped
    );
    // The counter still counts, which is what keeps the nought above meaning
    // something: it is nought because the table is complete, not because
    // nothing is measured. Every channel the library has is accounted for as
    // one of these five.
    let accounted = counts.attributes
        + counts.channels_unmapped
        + counts.channels_without_function
        + counts.channels_duplicate
        + counts.channels_undefined;
    assert!(
        accounted > 10_000,
        "only {accounted} channels were looked at, which is not this library"
    );
    println!(
        "\nB38: {} channels mapped, {} unmapped, {} doing nothing, {} a second of their kind\n",
        counts.attributes,
        counts.channels_unmapped,
        counts.channels_without_function,
        counts.channels_duplicate,
    );
}

/// **Named ranges reach the profiles** — B38's second half, over the corpus.
///
/// A number rather than an example, because *capabilities are read* is a claim
/// about the library and not about the one fixture somebody tried it on. And
/// two properties that a hand-written test would not have caught over two
/// thousand profiles: a range is never inverted, and consecutive ranges of one
/// channel meet with nothing between them — a gap is an encoder position that
/// names nothing while the file says it names something.
#[test]
fn the_installed_library_carries_the_names_of_its_ranges() {
    let Some(library) = installed() else { return };
    let mut with_ranges = 0_usize;
    let mut ranges = 0_usize;
    for entry in library.entries() {
        let profile = library.profile(&entry.id).expect("the library listed it");
        for def in &profile.attributes {
            if def.ranges.is_empty() {
                continue;
            }
            with_ranges += 1;
            ranges += def.ranges.len();
            let mut previous: Option<u16> = None;
            for range in &def.ranges {
                assert!(
                    range.from <= range.to,
                    "{}: {:?} has a range that runs backwards",
                    entry.id,
                    def.attribute
                );
                assert!(
                    !range.name.is_empty(),
                    "{}: {:?} has a range with no name",
                    entry.id,
                    def.attribute
                );
                if let Some(previous) = previous {
                    assert_eq!(
                        range.from,
                        previous.saturating_add(1),
                        "{}: {:?} leaves a gap between two ranges",
                        entry.id,
                        def.attribute
                    );
                }
                previous = Some(range.to);
            }
        }
    }
    assert!(
        with_ranges > 1_000,
        "only {with_ranges} channels came out with named ranges"
    );
    println!("\nB38: {ranges} named ranges over {with_ranges} channels\n");
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

/// **Every colour channel in the installed library rests open** — punch-list B1,
/// second attempt.
///
/// The property behind the fix, over the real tree rather than over one
/// hand-written fixture — and it is what would have caught the fault the first
/// time. `crate::library::colour` was right and the OFL converter was wrong: it
/// believed a stated `defaultValue`, and 387 of the 391 colour channels in this
/// library state nought. No test read a real file, so nothing said so and the
/// owner found it by selecting a lamp.
///
/// Where a *lamp* parks with no DMX and where a *desk* parks a colour channel
/// are two questions. This asserts the second one, which is the desk's to answer.
#[test]
fn no_profile_in_the_installed_library_rests_a_colour_shut() {
    let Some(library) = installed() else {
        return;
    };
    let mut checked = 0_usize;
    let mut subtractive = 0_usize;
    let mut open_filters = 0_usize;
    for entry in library.entries() {
        let profile = library.profile(&entry.id).expect("the library listed it");
        for def in &profile.attributes {
            // **The emitter and not the bank** — S51, B38. The colour bank has
            // cyan, magenta and yellow on it now, and those are *filters*: open
            // is nought and full is opaque. Resting them where an emitter rests
            // would black out every CMY rig, which is why the rule moved off
            // the bank and on to the attribute.
            if def.attribute.is_additive_emitter() {
                assert_eq!(
                    def.default_value,
                    u16::MAX,
                    "{} rests {:?} at {}",
                    entry.id,
                    def.attribute,
                    def.default_value
                );
                checked += 1;
            } else if matches!(
                def.attribute,
                prism_domain::AttributeType::Cyan
                    | prism_domain::AttributeType::Magenta
                    | prism_domain::AttributeType::Yellow
            ) {
                subtractive += 1;
                if def.default_value == 0 {
                    open_filters += 1;
                }
            }
        }
    }
    assert!(checked > 100, "only {checked} colour channels were checked");

    // **And a filter rests out of the beam** — S51, B38, and the other half of
    // the same rule.
    //
    // *Where a desk parks a colour* is a convention of the desk for an emitter,
    // which is what B1 settled. A **filter** has no such convention: which end
    // of a CMY flag is open is a fact about how that head is wired, and the
    // only evidence a desk has about it is the file. So the rule is *nought
    // unless the manufacturer says otherwise*, and the exceptions are exactly
    // the profiles that say otherwise — three of the hundred-odd in this
    // library, all of them heads whose flags are wired the other way up.
    //
    // A proportion rather than a count, because the count moves upstream; what
    // must not happen is a *general* rule that parks filters in the beam, and
    // that would show here as a collapse rather than as a handful.
    assert!(
        subtractive > 50,
        "only {subtractive} subtractive flags were checked — the corpus has CMY heads in it"
    );
    assert!(
        open_filters * 10 >= subtractive * 9,
        "only {open_filters} of {subtractive} subtractive flags rest out of the beam"
    );
    println!("\nB38: {open_filters} of {subtractive} subtractive flags rest open at nought\n");
}
