//! The show model reaching the engine, which is the only reason it exists.
//!
//! `prism-core` depends on `prism-engine` **only here**, as a development
//! dependency. The show model must not be able to reach into the engine at run
//! time — that wiring is the daemon's (S17) — but the two do share one rule,
//! "what is a legal patch", and two copies of a rule drift apart. This target
//! is what holds them together: every patch the show accepts is one
//! `MergeBody::for_patch` accepts, asserted over arbitrary patches rather than
//! argued for in a comment.
//!
//! It also pins the three doors S17 will use: `FrameLayout` from
//! [`Show::universes`], `for_patch` from [`Show::patched`], and `load_groups` /
//! `load_sequence` from the show's own pools.

mod common;

use common::{cue, dimmer_type, fixture, group, par_type, sequence};
use prism_core::Show;
use prism_domain::{AttributeType, FixtureId, GroupId, SequenceId, UniverseId};
use prism_engine::{DmxFrame, FrameLayout, MergeBody};
use proptest::prelude::*;

/// Everything the daemon does to turn a show into a running engine.
fn body(show: &Show, playbacks: u32) -> Result<MergeBody, prism_engine::PatchError> {
    let layout = FrameLayout::new(show.universes()).unwrap();
    MergeBody::for_patch(&layout, show.patched(), (0..playbacks).map(SequenceId::new))
}

/// The frame a body at rest writes.
fn frame_of(show: &Show, body: &MergeBody) -> DmxFrame {
    let layout = FrameLayout::new(show.universes()).unwrap();
    let mut frame = DmxFrame::new(&layout);
    body.channels().encode(body.values(), &mut frame);
    frame
}

#[test]
fn a_show_built_through_the_model_becomes_a_running_engine() {
    let mut show = Show::new();
    show.embed_fixture_type(par_type()).unwrap();
    show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
        .unwrap();
    show.patch_fixture(fixture(2, "generic.rgbw.par", 2, 5))
        .unwrap();
    show.store_group(group(1, &[1, 2])).unwrap();
    show.store_sequence(sequence(1, vec![cue("1", 1, AttributeType::Red, 65535)]))
        .unwrap();

    let mut body = body(&show, 4).unwrap();
    // One slot per fixture and attribute: two PARs of four attributes each, and
    // **an intensity apiece that the desk supplies** — S43. A PAR's profile has
    // no dimmer channel, so `Fixture::has_software_dimmer` is true for both and
    // `MergePlan::build` gives each one a `Dimmer` slot with no channel behind
    // it. Ten, and this assertion is one of the places that would notice if the
    // two crates stopped agreeing about which fixtures get one.
    assert_eq!(body.plan().slot_count(), 10);

    // The three set-up doors, driven from the show's own pools.
    body.load_groups(&show.groups().cloned().collect::<Vec<_>>());
    body.load_sequence(
        SequenceId::new(1),
        show.sequence(SequenceId::new(1)).unwrap(),
    )
    .unwrap();

    let frame = frame_of(&show, &body);
    assert_eq!(frame.universe_count(), 2);
}

#[test]
fn a_universe_nothing_is_patched_into_is_not_in_the_layout() {
    let mut show = Show::new();
    show.embed_fixture_type(par_type()).unwrap();
    show.patch_fixture(fixture(1, "generic.rgbw.par", 7, 1))
        .unwrap();
    let layout = FrameLayout::new(show.universes()).unwrap();
    assert_eq!(layout.universes(), [UniverseId::new(7)]);
    assert_eq!(layout.index_of(UniverseId::new(1)), None);
}

#[test]
fn the_higher_fixture_number_wins_a_shared_channel() {
    // The show reports the overlap rather than refusing it; the engine makes
    // the outcome deterministic. This asserts they agree about which one wins.
    let mut show = Show::new();
    show.embed_fixture_type(dimmer_type("dim.low", 0)).unwrap();
    show.embed_fixture_type(dimmer_type("dim.high", 65535))
        .unwrap();
    show.patch_fixture(fixture(1, "dim.low", 1, 5)).unwrap();
    show.patch_fixture(fixture(2, "dim.high", 1, 5)).unwrap();

    let conflicts = show.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].second, FixtureId::new(2));

    let low_wins = body(&show, 1).unwrap();
    assert_eq!(frame_of(&show, &low_wins).channel(0, 5), Some(255));

    // And the other way round, so the assertion is about the rule and not about
    // this pair of home values.
    let mut swapped = Show::new();
    swapped
        .embed_fixture_type(dimmer_type("dim.low", 0))
        .unwrap();
    swapped
        .embed_fixture_type(dimmer_type("dim.high", 65535))
        .unwrap();
    swapped.patch_fixture(fixture(1, "dim.high", 1, 5)).unwrap();
    swapped.patch_fixture(fixture(2, "dim.low", 1, 5)).unwrap();
    let high_loses = body(&swapped, 1).unwrap();
    assert_eq!(frame_of(&swapped, &high_loses).channel(0, 5), Some(0));
}

#[test]
fn a_repatch_moves_the_revision_the_daemon_watches() {
    // The programmer is addressed by merge-plan slot, so a repatch invalidates
    // every queued programmer command (S6). The show model's job is to make
    // that visible; catching it is S17's.
    let mut show = Show::new();
    show.embed_fixture_type(par_type()).unwrap();
    show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
        .unwrap();
    let before = show.patch_revision();
    let slots_before = body(&show, 1).unwrap().plan().slot_count();

    show.patch_fixture(fixture(2, "generic.rgbw.par", 1, 5))
        .unwrap();
    assert!(show.patch_revision() > before);
    assert_ne!(body(&show, 1).unwrap().plan().slot_count(), slots_before);

    // Editing something that is not the patch does not move it.
    let steady = show.patch_revision();
    show.store_group(group(1, &[1])).unwrap();
    show.store_executor(common::executor(0, None)).unwrap();
    show.store_sequence(sequence(1, vec![cue("1", 1, AttributeType::Red, 1)]))
        .unwrap();
    show.set_sequence_master(SequenceId::new(1), 1).unwrap();
    assert_eq!(show.patch_revision(), steady);
}

#[test]
fn a_group_the_show_holds_is_a_group_master_the_engine_holds() {
    let mut show = Show::new();
    show.embed_fixture_type(dimmer_type("generic.dimmer", 65535))
        .unwrap();
    show.patch_fixture(fixture(1, "generic.dimmer", 1, 1))
        .unwrap();
    show.store_group(group(3, &[1])).unwrap();

    let mut body = body(&show, 1).unwrap();
    body.load_groups(&show.groups().cloned().collect::<Vec<_>>());
    body.masters_mut().set_group_level(GroupId::new(3), 0);
    body.resolve();
    assert_eq!(frame_of(&show, &body).channel(0, 1), Some(0));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Every patch the show model accepts is one the engine accepts.
    ///
    /// The two validate separately — the show against `Fixture::last_address`
    /// and its own profile checks, the engine when it builds the channel plan —
    /// and this is what says they agree. A show that could be saved and could
    /// not then be played would be the worst of the two failures.
    #[test]
    fn what_the_show_accepts_the_engine_accepts(
        fixtures in proptest::collection::vec((1u32..6, 1u32..4, 1u16..500), 1..6),
    ) {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.embed_fixture_type(dimmer_type("generic.dimmer", 0)).unwrap();
        for (index, (id, universe, address)) in fixtures.into_iter().enumerate() {
            let type_id = if index % 2 == 0 { "generic.rgbw.par" } else { "generic.dimmer" };
            // Whatever the show refuses is not part of the claim; what it
            // accepts is.
            let _ = show.patch_fixture(fixture(id, type_id, universe, address));
        }
        // Not a claim about the empty patch: the address space is wide enough
        // that at least one of these always lands.
        prop_assert!(show.fixtures().count() > 0);
        prop_assert!(body(&show, 2).is_ok(), "the engine refused a patch the show accepted");
    }
}
