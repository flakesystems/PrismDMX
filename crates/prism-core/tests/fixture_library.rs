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
         \x20 mode entries that are null:             {}\n\
         \x20 fine bytes with no coarse channel:      {}\n\
         \x20 switching aliases resolved:             {}\n\
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
        counts.channels_unused,
        counts.channels_orphan_fine,
        counts.channels_switched,
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
    // **S52.** It was 2 679 — a second colour wheel, a per-pixel red on a tube.
    assert_eq!(
        counts.channels_duplicate, 0,
        "{} channels of the installed library are dropped for being a second of their kind",
        counts.channels_duplicate
    );
    // **And no mode is skipped for a matrix insert.** It was 90 profiles and
    // 724 inserts. What is left in this counter is a layout that depends on
    // another channel's *value* — a switching channel — which is a different
    // question and is `docs/ISSUES.md`'s.
    assert_eq!(
        counts.modes_with_inserts, 0,
        "{} modes still have an insert this reader cannot write out",
        counts.modes_with_inserts
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

/// **A lamp with a warm white and a cold white keeps both** — S52.
///
/// The owner's own fault report, and the shortest statement of what an
/// occurrence is for. The Open Fixture Library names thirteen emitter colours;
/// this model had eleven of them, because `Warm White` and `Cold White` were
/// both read as `White`. On a lamp with one of them that is a wrong label. On a
/// lamp with **both** the second one collided with the first and was dropped,
/// so half the fixture did not respond at all.
///
/// Asserted over the corpus rather than over one profile, because *this reader
/// keeps both* is a claim about the library. Six profiles of this revision have
/// both — `generic/cw-ww-fader` is the plainest of them.
#[test]
fn a_fixture_with_a_warm_and_a_cold_white_keeps_both() {
    use prism_domain::AttributeType::{ColdWhite, WarmWhite};
    let Some(library) = installed() else { return };
    let mut with_both = 0_usize;
    for entry in library.entries() {
        let Some(profile) = library.profile(&entry.id) else {
            continue;
        };
        let has = |attribute| {
            profile
                .attributes
                .iter()
                .any(|def| def.attribute == attribute)
        };
        if !(has(WarmWhite) && has(ColdWhite)) {
            continue;
        }
        with_both += 1;
        // The two are separate channels, so they are separate offsets — which
        // is the whole of the fault: one of them used to be dropped and the
        // operator had one knob for two lamps.
        let offsets: Vec<u16> = profile
            .attributes
            .iter()
            .filter(|def| def.attribute == WarmWhite || def.attribute == ColdWhite)
            .map(|def| def.coarse_offset)
            .collect();
        assert_eq!(offsets.len(), 2, "{} has {offsets:?}", entry.id);
        assert_ne!(
            offsets[0], offsets[1],
            "{} folds them onto one channel",
            entry.id
        );
    }
    assert!(
        with_both >= 3,
        "only {with_both} profiles have both a warm and a cold white — this library has more"
    );
    println!("\nS52: {with_both} profiles carry a warm white and a cold white\n");
}

/// **A wheel slot is called what the manufacturer calls it** — S52.
///
/// S51 read a channel's capabilities and named each range from the capability
/// itself: `comment`, `effectName`, `shutterEffect`. A wheel capability rarely
/// has one of those. What it has is `wheel` and `slotNumber`, and the name lives
/// in the fixture's top-level `wheels` block, which the reader had never opened
/// — so **3 440 of the corpus's 4 497 wheel capabilities read *Slot 3***, and
/// the steps window this session builds would have offered nine gobo wheels in
/// ten as a list of numbers.
///
/// What is asserted is that hardly any range still reads `Slot <n>`. Not *none*:
/// a file whose wheel has fewer slots than its channel has capabilities leaves
/// one genuinely unnamed, and inventing a name for it is the thing this session
/// is under instruction not to do.
#[test]
fn a_wheel_slot_is_named_out_of_the_wheels_the_file_declares() {
    let Some(library) = installed() else { return };
    let mut named = 0_usize;
    let mut numbered = 0_usize;
    for entry in library.entries() {
        let Some(profile) = library.profile(&entry.id) else {
            continue;
        };
        for def in &profile.attributes {
            for range in &def.ranges {
                if range.name.starts_with("Slot ")
                    && range.name["Slot ".len()..]
                        .chars()
                        .all(|c| c.is_ascii_digit())
                {
                    numbered += 1;
                } else {
                    named += 1;
                }
            }
        }
    }
    assert!(named > 10_000, "only {named} ranges have a name at all");
    assert!(
        numbered * 100 < named,
        "{numbered} of {} ranges still read \"Slot <n>\"",
        named + numbered
    );
    println!("\nS52: {named} ranges named, {numbered} still a bare slot number\n");
}

/// **A profile with a matrix insert patches** — S52.
///
/// An Open Fixture Library mode may say *repeat these template channels once
/// per pixel* rather than writing its channels out. Until S52 such a mode was
/// skipped whole, because a footprint may not depend on state: **90 of the 634
/// installed profiles** could not be patched at all, and the owner's LED tube
/// is one of them. Resolving them is what an occurrence made possible — an
/// eight-pixel tube is a fixture with eight reds.
///
/// The claim is over the corpus: modes with an insert exist, they now produce
/// profiles, and the repeats are numbered from nought without a gap. The gap is
/// the property worth asserting — an occurrence that skipped a number would be
/// a knob the band draws and no channel answers.
#[test]
fn a_matrix_insert_becomes_the_channels_it_stands_for() {
    let Some(library) = installed() else { return };
    let mut repeated = 0_usize;
    let mut deepest = 0_u8;
    for entry in library.entries() {
        let Some(profile) = library.profile(&entry.id) else {
            continue;
        };
        let mut by_kind: std::collections::BTreeMap<prism_domain::AttributeType, Vec<u8>> =
            std::collections::BTreeMap::new();
        for def in &profile.attributes {
            by_kind
                .entry(def.attribute)
                .or_default()
                .push(def.occurrence);
        }
        for (attribute, mut occurrences) in by_kind {
            if occurrences.len() == 1 {
                continue;
            }
            repeated += 1;
            deepest = deepest.max(u8::try_from(occurrences.len() - 1).unwrap_or(u8::MAX));
            occurrences.sort_unstable();
            // **Nought upwards, one at a time.** The occurrence is an index into
            // what the fixture has, not a label somebody chose, so a fixture
            // with three colour wheels has 0, 1, 2 and nothing else.
            assert_eq!(
                occurrences,
                (0..u8::try_from(occurrences.len()).unwrap_or(u8::MAX)).collect::<Vec<_>>(),
                "{} numbers its {attribute:?} channels oddly",
                entry.id
            );
        }
    }
    assert!(
        repeated > 100,
        "only {repeated} repeated parameters in the whole library — the inserts are not resolving"
    );
    println!("\nS52: {repeated} repeated parameters, the deepest {deepest} occurrences\n");
}

/// **The discriminators the format defines are read** — S53, over the corpus.
///
/// S51's table read a capability's **type** and nothing else, and that is lossy
/// in a way no counter could show: `channels_unmapped` stayed at nought while
/// channels arrived under the wrong knob. What this asserts is that the four
/// attributes the discriminators produce are actually *reached* by the library
/// — an attribute nothing in two thousand profiles maps to would be a row
/// somebody added and nothing exercises.
///
/// The colour wheel is the one that matters: **115 channels** of the corpus
/// that an operator pressing *Colour* could not find, because they were filed
/// under *Gobo* — 110 of them a colour wheel, the rest an iris, a prism or a
/// frost wheel that had been called a gobo for want of reading its slots.
///
/// **`ColorWheelRotation` is deliberately not asserted to be reached**, because
/// nothing in the corpus reaches it and that is the reader being right. Every
/// colour wheel here puts its scroll as a *range at the top of the select
/// channel* — slot 1 … slot 8, then rotate CW — and such a channel is **one
/// knob**, which the reader gets by taking the channel's first mapped
/// capability. The row exists because the format defines the distinction and a
/// fixture that gives the scroll a channel of its own will land on it; the rule
/// itself is held by `a_wheel_goes_to_the_bank_its_slots_say_it_belongs_on`,
/// which is where a rule belongs. Asserting a number here that the corpus does
/// not contain would make this test a wish.
#[test]
fn the_capability_discriminators_reach_the_attributes_they_name() {
    use prism_domain::AttributeType::{
        BladeRotation, BladeSystem, ColorWheel, ColorWheelRotation, Gobo, Haze,
    };
    let Some(library) = installed() else { return };
    let mut count = std::collections::BTreeMap::<prism_domain::AttributeType, usize>::new();
    for entry in library.entries() {
        let Some(profile) = library.profile(&entry.id) else {
            continue;
        };
        for def in &profile.attributes {
            *count.entry(def.attribute).or_default() += 1;
        }
    }
    let at = |attribute| count.get(&attribute).copied().unwrap_or_default();
    println!(
        "\nS53: {} colour wheels, {} colour-wheel rotations, {} gobo wheels, \
         {} blade rotations, {} frame rotations, {} haze\n",
        at(ColorWheel),
        at(ColorWheelRotation),
        at(Gobo),
        at(BladeRotation),
        at(BladeSystem),
        at(Haze),
    );
    // **Each discriminator the corpus exercises is reached**, which is what
    // says they are being read at all. A row nothing in 2 871 profiles maps to
    // would be one somebody added and nothing exercises — see the note above
    // for why the colour wheel's *rotation* is not one of those rows.
    assert!(
        at(ColorWheel) > 100,
        "only {} colour wheels",
        at(ColorWheel)
    );
    assert_eq!(
        at(ColorWheelRotation),
        0,
        "a channel now reaches the colour wheel's rotation; if that is a real          dedicated scroll channel this assertion should become `> 0`, and if it          is a select channel whose top range rotates then the reader has begun          splitting one knob in two"
    );
    assert!(at(BladeRotation) > 0, "no blade rotation is reached");
    assert!(at(BladeSystem) > 0, "no frame rotation is reached");
    assert!(at(Haze) > 0, "no haze channel is reached");
    // **And the gobo bank still has its gobo wheels.** A change that moved
    // every wheel to the colour bank would satisfy every assertion above, so
    // this is the half that says the split is a split and not a landslide.
    assert!(at(Gobo) > 100, "only {} gobo wheels", at(Gobo));
}

/// **Every channel of the library carries the name its manufacturer gave it** —
/// S53.
///
/// `AttributeDef::label` is what the encoder reads in place of this desk's own
/// word for the attribute. It is a label and never a key — two fixtures whose
/// reds are called different things still share `AttributeType::Red`, which is
/// what lets one line reach a rig from three manufacturers.
#[test]
fn every_channel_of_the_installed_library_carries_its_own_name() {
    let Some(library) = installed() else { return };
    let mut named = 0_usize;
    let mut nameless = 0_usize;
    for entry in library.entries() {
        let Some(profile) = library.profile(&entry.id) else {
            continue;
        };
        for def in &profile.attributes {
            if def.label.as_deref().is_some_and(|name| !name.is_empty()) {
                named += 1;
            } else {
                nameless += 1;
            }
        }
    }
    println!("\nS53: {named} channels carry their own name, {nameless} do not\n");
    assert_eq!(
        nameless, 0,
        "{nameless} channels of the installed library have no name of their own"
    );
    assert!(named > 30_000, "only {named} channels were looked at");
}

/// **No slot of any profile in the installed library is out of reach** — S54.
///
/// The strongest thing this file says, and the one that took the longest to be
/// sayable. Every counter before it asks whether a *channel* reached an
/// attribute; this asks whether every **DMX slot an operator patches** has a
/// knob on it — which is the question an operator actually has, and the one
/// nothing was asking.
///
/// It was **707 slots in 337 of the 2 871 profiles** before S54, and no counter
/// noticed: `channels_unmapped` sat at nought the whole time because every
/// channel it *understood* reached an attribute. The ones it did not understand
/// kept their place in the footprint, were driven to nought for ever and had no
/// knob — 34 of the 35 channels of a `glp/knv-cube`, 9 of the 10 of a
/// `jb-systems/twin-effect-laser`. Those two fixtures were unusable and the
/// library reported itself lossless.
///
/// Four things reach the floor, and `the_conversion_reports_what_it_could_not_use`
/// prints how many of each: a **switching alias** whose positions disagree
/// (289), a mode entry that is **`null`** (210), a channel the file says does
/// **nothing** (99), and a **fine byte** whose coarse channel is not here (14).
/// A fifth would be a capability type a later version of the format adds, which
/// is the reason this is a floor and not four fixes.
#[test]
fn no_slot_of_any_profile_is_out_of_reach() {
    let Some(library) = installed() else { return };
    let mut slots = 0_usize;
    let mut reached = 0_usize;
    let mut worst: Vec<(usize, u16, String)> = Vec::new();
    for entry in library.entries() {
        let Some(profile) = library.profile(&entry.id) else {
            continue;
        };
        let footprint = profile.footprint as usize;
        let mut covered = vec![false; footprint];
        for def in &profile.attributes {
            for offset in [Some(def.coarse_offset), def.fine_offset]
                .into_iter()
                .flatten()
            {
                if let Some(slot) = covered.get_mut(offset as usize) {
                    *slot = true;
                }
            }
        }
        let hit = covered.iter().filter(|seen| **seen).count();
        slots += footprint;
        reached += hit;
        if hit < footprint {
            worst.push((footprint - hit, profile.footprint, entry.id.clone()));
        }
    }
    worst.sort_by_key(|(holes, _, _)| std::cmp::Reverse(*holes));
    println!(
        "
S54: {reached} of {slots} slots reachable, {} out of reach
",
        slots - reached
    );
    assert!(slots > 40_000, "only {slots} slots were looked at");
    assert_eq!(
        slots,
        reached,
        "{} slots have no knob on them, worst first: {:?}",
        slots - reached,
        worst.iter().take(8).collect::<Vec<_>>()
    );
}

/// **A fixture is reached by its own channels, not by a supplied dimmer** — the
/// other half of S54's floor.
///
/// A floor that gave every slot a knob but called them all `Raw` would satisfy
/// the assertion above and be useless, so this holds the **shape** of the
/// answer: the overwhelming majority of the library's slots are still reached
/// by a parameter this desk has a word for, and the raw knob is the exception it
/// was built to be.
#[test]
fn the_raw_channel_is_the_exception_and_not_the_rule() {
    use prism_domain::AttributeType::Raw;
    let Some(library) = installed() else { return };
    let mut raw = 0_usize;
    let mut named = 0_usize;
    for entry in library.entries() {
        let Some(profile) = library.profile(&entry.id) else {
            continue;
        };
        for def in &profile.attributes {
            if def.attribute == Raw {
                raw += 1;
                // Every raw knob says something, because a knob nobody can name
                // is one nobody can use: the manufacturer's word where the file
                // has one, and `Ch 7` where it does not.
                assert!(
                    def.label.as_deref().is_some_and(|name| !name.is_empty()),
                    "a raw channel of {} has no name at all",
                    entry.id
                );
            } else {
                named += 1;
            }
        }
    }
    println!(
        "
S54: {raw} raw channels, {named} the desk has a word for
"
    );
    assert!(raw > 0, "no raw channel at all, so the floor is untested");
    assert!(
        raw * 20 < named,
        "{raw} of {} attributes are raw, which is not an exception any more",
        raw + named
    );
}
