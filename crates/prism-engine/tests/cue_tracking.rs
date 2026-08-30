//! **Tracking, asserted on frames** — S48.
//!
//! The whole session in one sentence: *the output at a cue must not depend on
//! how the operator got there.* Until S48 it did. Cue 1 puts a wash at 50 %, cue
//! 5 puts it at 100 %, cue 3 mentions neither — and walking 1→2→3 left the wash
//! at 50 % while jumping 7→3 left it at 100 %. The same cue, two outputs, and
//! which one you got depended on where you had been. An operator cannot rehearse
//! cue 3 like that, and a show cannot be handed to anybody else.
//!
//! Every claim here is made on the **bytes a driver would receive**, because
//! that is the only place the difference is real: a player holding the right
//! numbers internally and writing the wrong ones is a bug this target would still
//! catch, and one holding the wrong numbers that happen to encode the same is not
//! a bug at all.
//!
//! What is asserted, and how the *byte-identical frame sequence* criterion is met
//! literally rather than approximately:
//!
//! - With **snap** cues the two routes produce byte-identical frames from the
//!   very first tick after the cue is taken, for the whole run.
//! - With **fades** the transition into cue 3 legitimately differs — one route
//!   arrives from cue 2's look and the other from cue 7's, and an attribute has
//!   to fade from where it *is*. So the comparison starts at the tick the
//!   transition completes and runs on through cues 4, 5 and 6: from the moment
//!   the list is settled at cue 3, the two are the same show.

#![cfg(not(loom))]

use std::sync::Arc;

use prism_domain::{
    AttributeDef, AttributeType, Cue, CuePart, CueTracking, CueTrigger, Fixture, FixtureId,
    FixtureType, GoDirection, Sequence, SequenceId, UniverseId, Vec3,
};
use prism_engine::{
    Engine, FrameLayout, FramePublisher, FrameSubscriber, ManualClock, MergeBody, TickCommand,
    command_queue,
};
use proptest::prelude::*;

/// How many dimmers the rig carries. Enough that a cue can name some and leave
/// others, which is the whole subject.
const FIXTURES: u32 = 6;

/// A one-channel dimmer, dark at home.
fn dimmer() -> FixtureType {
    FixtureType {
        id: "test.dimmer".to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "test".to_owned(),
        footprint: 1,
        attributes: vec![AttributeDef {
            attribute: AttributeType::Dimmer,
            feature_group: AttributeType::Dimmer.feature_group(),
            coarse_offset: 0,
            fine_offset: None,
            default_value: 0,
            merge_mode: AttributeType::Dimmer.default_merge_mode(),
            invert: false,
            physical_from: 0.0,
            physical_to: 100.0,
        }],
    }
}

fn fixture(id: u32) -> Fixture {
    Fixture {
        id: FixtureId::new(id),
        name: format!("Dim {id}"),
        type_id: "test.dimmer".to_owned(),
        universe: UniverseId::MIN,
        address: u16::try_from(id).unwrap_or(1),
        position: Vec3::default(),
        rotation: Vec3::default(),
        invert_pan: false,
        invert_tilt: false,
        software_dimmer: true,
    }
}

fn part(fixture: u32, value: u16, tracking: CueTracking) -> CuePart {
    CuePart {
        fixture: FixtureId::new(fixture),
        attribute: AttributeType::Dimmer,
        value,
        preset_ref: None,
        tracking,
    }
}

fn cue(number: &str, fade: f64, parts: Vec<CuePart>) -> Cue {
    Cue {
        number: number.to_owned(),
        name: format!("Cue {number}"),
        fade_in: fade,
        fade_out: fade,
        delay: 0.0,
        trigger: CueTrigger::Go,
        trigger_time: None,
        parts,
    }
}

fn sequence(cues: Vec<Cue>) -> Sequence {
    Sequence {
        id: SequenceId::new(1),
        name: "Main".to_owned(),
        color: None,
        cues,
        looping: false,
        master_level: u16::MAX,
        speed: prism_domain::SPEED_UNITY,
        is_active: false,
        current_cue_index: None,
    }
}

/// One engine over one cue list, driven a tick at a time.
struct Rig {
    engine: Engine<MergeBody>,
    producer: prism_engine::Producer<TickCommand>,
    subscriber: FrameSubscriber,
    clock: ManualClock,
}

impl Rig {
    fn new(cues: Vec<Cue>) -> Self {
        let layout = FrameLayout::new([UniverseId::MIN]).unwrap();
        let profile = dimmer();
        let patched: Vec<Fixture> = (1..=FIXTURES).map(fixture).collect();
        let mut body = MergeBody::for_patch(
            &layout,
            patched.iter().map(|fixture| (fixture, &profile)),
            [SequenceId::new(1)],
        )
        .unwrap();
        body.load_sequence(SequenceId::new(1), &sequence(cues))
            .unwrap();

        let mut publisher = FramePublisher::new(Arc::new(layout));
        let subscriber = publisher.subscribe();
        let (producer, consumer) = command_queue(64);
        Self {
            engine: Engine::new(body, consumer, publisher),
            producer,
            subscriber,
            clock: ManualClock::new(),
        }
    }

    fn go(&mut self) {
        self.producer
            .push(TickCommand::Go {
                executor: SequenceId::new(1).into(),
                direction: GoDirection::Next,
            })
            .unwrap();
    }

    fn goto(&mut self, index: u16) {
        self.producer
            .push(TickCommand::GotoCue {
                executor: SequenceId::new(1).into(),
                cue_index: index,
            })
            .unwrap();
    }

    /// One tick, and the channels of the patched universe as a driver sees them.
    fn tick(&mut self) -> Vec<u8> {
        self.engine.run_ticks(&self.clock, 1);
        assert!(self.subscriber.refresh());
        self.subscriber.frame().channels()[..FIXTURES as usize].to_vec()
    }

    /// `count` ticks, every frame kept.
    fn run(&mut self, count: usize) -> Vec<Vec<u8>> {
        (0..count).map(|_| self.tick()).collect()
    }

    /// `count` ticks, only the last frame kept.
    fn settle(&mut self, count: usize) -> Vec<u8> {
        self.run(count).pop().unwrap_or_default()
    }
}

/// The list from the session's own statement of the fault.
///
/// Cue 1 puts fixture 1 at 50 %, cue 5 puts it at 100 %, and cue 3 names neither
/// it nor anything cue 5 touches. Six cues so that 7 → 3 is a real jump backwards
/// over the cue that moved the wash.
fn worked_example(fade: f64) -> Vec<Cue> {
    vec![
        cue("1", fade, vec![part(1, 32_768, CueTracking::Track)]),
        cue("2", fade, vec![part(2, 65_535, CueTracking::Track)]),
        cue("3", fade, vec![part(3, 20_000, CueTracking::Track)]),
        cue("4", fade, vec![part(4, 30_000, CueTracking::Track)]),
        cue("5", fade, vec![part(1, 65_535, CueTracking::Track)]),
        cue("6", fade, vec![part(5, 40_000, CueTracking::Track)]),
        cue("7", fade, vec![part(6, 50_000, CueTracking::Track)]),
    ]
}

/// Ticks to let a snap cue land: one is enough, three is honest.
const SNAP: usize = 3;

/// Ticks for a one-second fade to finish, with a little over.
const FADED: usize = 60;

#[test]
fn walking_to_a_cue_and_jumping_to_it_produce_the_same_frames() {
    // **The headline, with snap cues, so *byte-identical frame sequence* is
    // literal.** No fade means no transition to argue about: from the tick cue 3
    // is taken, the two routes are the same show frame for frame.
    let mut walked = Rig::new(worked_example(0.0));
    walked.go(); // cue 1
    walked.settle(SNAP);
    walked.go(); // cue 2
    walked.settle(SNAP);
    walked.go(); // cue 3
    let by_walking = walked.run(120);

    let mut jumped = Rig::new(worked_example(0.0));
    jumped.goto(6); // cue 7, over the top of cue 5
    jumped.settle(SNAP);
    jumped.goto(2); // cue 3
    let by_jumping = jumped.run(120);

    assert_eq!(
        by_jumping, by_walking,
        "the frame sequence at cue 3 depends on how cue 3 was reached"
    );
    // And the measurement was of something: fixture 1 is at 50 %, which is what
    // cue 1 said and what cue 5 would have overwritten.
    assert_eq!(by_walking[0][0], 128, "cue 1's wash is not what came out");
}

#[test]
fn a_jump_backwards_restores_what_the_walk_would_have_left() {
    // The fault this session removes, stated as the thing that *would* have
    // happened: cue 5 puts fixture 1 at full, so a jump back to cue 3 that kept
    // whatever the playback was holding would leave it at 255. It does not.
    let mut jumped = Rig::new(worked_example(0.0));
    jumped.goto(6);
    jumped.settle(SNAP);
    jumped.goto(2);
    let frame = jumped.settle(SNAP);
    assert_eq!(frame[0], 128, "cue 5's value tracked backwards into cue 3");
    // Cue 7's own value is gone as well, because cue 3 is above it in the list
    // and nothing below a cue reaches it.
    assert_eq!(frame[5], 0, "cue 7's value survived a jump above it");
    // And what cue 2 asserted is still standing, because it is above cue 3.
    assert_eq!(frame[1], 255, "cue 2's value did not track into cue 3");
}

#[test]
fn a_go_from_a_jumped_to_cue_carries_on_as_if_the_list_had_been_walked() {
    // The other half of the deliverable, and the one that makes fades honest:
    // the transition *into* cue 3 differs between the two routes, because an
    // attribute fades from where it is and the two routes come from different
    // looks. From the moment the list is settled at cue 3, everything after it
    // is byte for byte the same show.
    let mut walked = Rig::new(worked_example(1.0));
    walked.go();
    walked.settle(FADED);
    walked.go();
    walked.settle(FADED);
    walked.go();
    walked.settle(FADED);

    let mut jumped = Rig::new(worked_example(1.0));
    jumped.goto(6);
    jumped.settle(FADED);
    jumped.goto(2);
    jumped.settle(FADED);

    // Settled at cue 3 by two different routes, then four Gos onwards.
    let mut by_walking = Vec::new();
    let mut by_jumping = Vec::new();
    for _ in 0..4 {
        walked.go();
        jumped.go();
        by_walking.extend(walked.run(FADED));
        by_jumping.extend(jumped.run(FADED));
    }
    assert_eq!(
        by_jumping, by_walking,
        "the list runs differently after a jump than after a walk"
    );
    assert!(
        by_walking.iter().any(|frame| frame[0] == 255),
        "cue 5 never raised the wash, so the comparison proves nothing"
    );
}

#[test]
fn editing_cue_two_changes_what_cue_five_outputs_without_cue_five_being_touched() {
    // **The exit criterion that says the state is derived rather than stored**,
    // on the frames rather than on the model. Cue 5 names fixture 1 and nothing
    // else; what it puts out on fixture 2 comes from cue 2, four cues above it.
    // Correct cue 2 and cue 5's output follows, with cue 5 itself byte for byte
    // the cue it was.
    let mut cues = worked_example(0.0);
    let cue_five = cues[4].clone();

    let mut before = Rig::new(cues.clone());
    before.goto(4);
    let was = before.settle(SNAP);
    assert_eq!(was[1], 255, "cue 2's value is not what reaches cue 5");

    // One value of one part of cue 2. Nothing else in the list is touched.
    cues[1].parts[0].value = 20_000;
    let mut after = Rig::new(cues.clone());
    after.goto(4);
    let now = after.settle(SNAP);
    assert_eq!(now[1], 78, "the edit to cue 2 did not reach cue 5");
    assert_eq!(
        now[0], was[0],
        "cue 5's own value moved, so this measured the wrong thing"
    );
    assert_eq!(cues[4], cue_five, "cue 5 was rewritten");
}

#[test]
fn a_cue_only_value_is_taken_back_when_the_list_leaves_the_cue() {
    // The one-off. Cue 2 slams fixture 1 to full and hands it back at cue 3 -
    // back to the 50 % cue 1 put there, and *not* to nought, because cue-only
    // means *undo my edit* rather than *turn it off*.
    let cues = vec![
        cue("1", 0.0, vec![part(1, 32_768, CueTracking::Track)]),
        cue("2", 0.0, vec![part(1, 65_535, CueTracking::CueOnly)]),
        cue("3", 0.0, vec![part(2, 10_000, CueTracking::Track)]),
    ];
    let mut rig = Rig::new(cues);
    rig.go();
    assert_eq!(rig.settle(SNAP)[0], 128);
    rig.go();
    assert_eq!(
        rig.settle(SNAP)[0],
        255,
        "the cue-only value never went out"
    );
    rig.go();
    assert_eq!(
        rig.settle(SNAP)[0],
        128,
        "the cue-only value was still there at cue 3"
    );
}

#[test]
fn a_cue_only_value_with_nothing_under_it_leaves_the_merge_rather_than_going_to_zero() {
    // The distinction the tracking table's `None` carries: an attribute the
    // playback has stopped holding falls through to whatever is below it in the
    // merge, and with nothing below it that is the *home* value. Home is 128
    // here, so a playback that had written 0 and a playback that had let go
    // would look different — which is the point.
    let mut profile = dimmer();
    profile.attributes[0].default_value = 32_768;
    let layout = FrameLayout::new([UniverseId::MIN]).unwrap();
    let patched: Vec<Fixture> = (1..=FIXTURES).map(fixture).collect();
    let mut body = MergeBody::for_patch(
        &layout,
        patched.iter().map(|fixture| (fixture, &profile)),
        [SequenceId::new(1)],
    )
    .unwrap();
    body.load_sequence(
        SequenceId::new(1),
        &sequence(vec![
            cue("1", 0.0, vec![part(1, 65_535, CueTracking::CueOnly)]),
            cue("2", 0.0, vec![part(2, 10_000, CueTracking::Track)]),
        ]),
    )
    .unwrap();
    let mut publisher = FramePublisher::new(Arc::new(layout));
    let mut subscriber = publisher.subscribe();
    let (mut producer, consumer) = command_queue(16);
    let mut engine = Engine::new(body, consumer, publisher);
    let clock = ManualClock::new();

    let mut step = |engine: &mut Engine<MergeBody>, count: usize| {
        engine.run_ticks(&clock, count as u64);
        assert!(subscriber.refresh());
        subscriber.frame().channels()[0]
    };

    producer
        .push(TickCommand::Go {
            executor: SequenceId::new(1).into(),
            direction: GoDirection::Next,
        })
        .unwrap();
    assert_eq!(step(&mut engine, SNAP), 255);
    producer
        .push(TickCommand::Go {
            executor: SequenceId::new(1).into(),
            direction: GoDirection::Next,
        })
        .unwrap();
    assert_eq!(
        step(&mut engine, SNAP),
        128,
        "the released attribute was written as nought instead of let go"
    );
}

#[test]
fn an_inherited_attribute_fades_from_where_it_is_and_not_from_where_it_was_asserted() {
    // The fade across the boundary. Fixture 1 is asserted at 0 % in cue 1, sits
    // there through three cues that never mention it, and is asserted at 100 %
    // in cue 5 over a one-second fade. It has to start from 0 - where it is -
    // rather than jumping anywhere first, and it has to arrive.
    let cues = vec![
        cue("1", 0.0, vec![part(1, 0, CueTracking::Track)]),
        cue("2", 0.0, vec![part(2, 10_000, CueTracking::Track)]),
        cue("3", 0.0, vec![part(3, 20_000, CueTracking::Track)]),
        cue("4", 0.0, vec![part(4, 30_000, CueTracking::Track)]),
        cue("5", 1.0, vec![part(1, 65_535, CueTracking::Track)]),
    ];
    let mut rig = Rig::new(cues);
    for _ in 0..4 {
        rig.go();
        rig.settle(SNAP);
    }
    rig.go();
    let frames = rig.run(FADED);
    assert_eq!(
        frames[0][0], 0,
        "the fade did not start where the light was"
    );
    let mut previous = 0u8;
    for frame in &frames {
        assert!(
            frame[0] >= previous,
            "the fade went backwards: {previous} then {}",
            frame[0]
        );
        previous = frame[0];
    }
    assert_eq!(previous, 255, "the fade never arrived");
}

/// A cue list a property test can generate: `count` cues over the six dimmers,
/// each naming a few of them, some of the values cue-only.
fn generated_cues() -> impl Strategy<Value = Vec<Cue>> {
    proptest::collection::vec(
        proptest::collection::vec(
            (1..=FIXTURES, any::<u16>(), any::<bool>()).prop_map(|(fixture, value, one_off)| {
                part(
                    fixture,
                    value,
                    if one_off {
                        CueTracking::CueOnly
                    } else {
                        CueTracking::Track
                    },
                )
            }),
            0..4,
        ),
        1..8,
    )
    .prop_map(|cues| {
        cues.into_iter()
            .enumerate()
            .map(|(index, parts)| cue(&(index + 1).to_string(), 0.0, parts))
            .collect()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// **Every cue of a generated list, reached both ways, gives one output.**
    ///
    /// The worked example above is one list somebody chose; this is the claim
    /// over lists nobody chose, which is the difference between a fixed bug and
    /// a rule. Snap cues, so the frame at every tick after the cue is taken is
    /// the settled one and the comparison needs no window.
    #[test]
    fn every_cue_reached_both_ways_gives_the_same_frame(cues in generated_cues()) {
        let count = cues.len();
        for target in 0..count {
            let mut walked = Rig::new(cues.clone());
            for _ in 0..=target {
                walked.go();
                walked.settle(SNAP);
            }
            let by_walking = walked.settle(SNAP);

            // From the *end* of the list, which is the worst case: everything
            // below the target has been asserted and has to be taken back.
            let mut jumped = Rig::new(cues.clone());
            jumped.goto(u16::try_from(count - 1).unwrap_or(0));
            jumped.settle(SNAP);
            jumped.goto(u16::try_from(target).unwrap_or(0));
            let by_jumping = jumped.settle(SNAP);

            prop_assert_eq!(
                &by_jumping,
                &by_walking,
                "cue {} differs between walking and jumping",
                target + 1
            );
        }
    }
}
