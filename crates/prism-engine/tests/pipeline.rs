//! `ARCHITECTURE_SPEC.md` §5, end to end, and the determinism criterion.
//!
//! The unit tests check each layer against its own specification. This target
//! checks the one thing none of them can: that the layers are wired together in
//! the order the specification gives, and that the bytes which leave the engine
//! are a function of the input and of nothing else.
//!
//! ```text
//!   patch → cue → fade → merge → programmer → masters → DMX bytes
//! ```
//!
//! Every assertion here is on the frame a driver would receive, not on the
//! engine's internal values: an output driver is what the rig is actually wired
//! to.

#![cfg(not(loom))]

use std::sync::Arc;

use prism_domain::{
    AttributeDef, AttributeType, Cue, CuePart, CueTrigger, ExecutorId, FeatureGroup, Fixture,
    FixtureId, FixtureType, GoDirection, Group, GroupId, MergeMode, ProgrammerState,
    ProgrammerValue, ProgrammerValueSource, Sequence, SequenceId, UniverseId, Vec3,
};
use prism_engine::{
    Engine, FrameLayout, FramePublisher, FrameSubscriber, ManualClock, MergeBody, TickCommand,
    apply_master, coarse_byte, command_queue, fine_byte,
};

/// A moving head with a 16-bit dimmer (home dark) and a 16-bit pan (home
/// centred), four channels — the fixture of `docs/DMX_MERGE.md` §7.
fn head() -> FixtureType {
    FixtureType {
        id: "test.head".to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Test".to_owned(),
        mode: "4ch".to_owned(),
        footprint: 4,
        attributes: vec![
            attribute(
                AttributeType::Dimmer,
                FeatureGroup::Dimmer,
                MergeMode::Htp,
                0,
                0,
            ),
            attribute(
                AttributeType::Pan,
                FeatureGroup::Position,
                MergeMode::Ltp,
                2,
                32_768,
            ),
        ],
    }
}

fn attribute(
    attribute: AttributeType,
    feature_group: FeatureGroup,
    merge_mode: MergeMode,
    coarse_offset: u16,
    default_value: u16,
) -> AttributeDef {
    AttributeDef {
        attribute,
        feature_group,
        coarse_offset,
        fine_offset: Some(coarse_offset + 1),
        default_value,
        merge_mode,
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

/// Two heads at addresses 1 and 5 of universe 1.
fn patch() -> Vec<Fixture> {
    (1..=2u32)
        .map(|id| Fixture {
            id: FixtureId::new(id),
            name: format!("Head {id}"),
            type_id: "test.head".to_owned(),
            universe: UniverseId::MIN,
            address: (id as u16 - 1) * 4 + 1,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
        })
        .collect()
}

/// One cue that fades both heads up and swings head 1's pan, over `fade`
/// seconds.
fn sequence(fade: f64) -> Sequence {
    Sequence {
        id: SequenceId::new(1),
        name: "Show".to_owned(),
        cues: vec![Cue {
            number: "1".to_owned(),
            name: "Up".to_owned(),
            fade_in: fade,
            fade_out: fade,
            delay: 0.0,
            trigger: CueTrigger::Go,
            trigger_time: None,
            parts: vec![
                part(1, AttributeType::Dimmer, 65_535),
                part(1, AttributeType::Pan, 20_000),
                part(2, AttributeType::Dimmer, 40_000),
            ],
        }],
        looping: false,
        is_active: false,
        current_cue_index: None,
    }
}

fn part(fixture: u32, attribute: AttributeType, value: u16) -> CuePart {
    CuePart {
        fixture: FixtureId::new(fixture),
        attribute,
        value,
        preset_ref: None,
    }
}

fn group(id: u32, fixtures: &[u32]) -> Group {
    Group {
        id: GroupId::new(id),
        name: format!("Group {id}"),
        fixtures: fixtures.iter().map(|id| FixtureId::new(*id)).collect(),
    }
}

/// A body with the patch, the cue list and one group on it.
fn body(fade: f64) -> MergeBody {
    let head = head();
    let patched = patch();
    let mut body = MergeBody::for_patch(
        &FrameLayout::new([UniverseId::MIN]).unwrap(),
        patched.iter().map(|fixture| (fixture, &head)),
        [ExecutorId::new(1), ExecutorId::new(2)],
    )
    .unwrap();
    body.load_sequence(ExecutorId::new(1), &sequence(fade))
        .unwrap();
    body.load_groups(&[group(1, &[1])]);
    body
}

/// The engine, its command queue and one output driver's view of it.
struct Rig {
    engine: Engine<MergeBody>,
    producer: prism_engine::Producer<TickCommand>,
    driver: FrameSubscriber,
    clock: ManualClock,
}

fn rig(fade: f64) -> Rig {
    let layout = Arc::new(FrameLayout::new([UniverseId::MIN]).unwrap());
    let mut publisher = FramePublisher::new(layout);
    let driver = publisher.subscribe();
    let (producer, consumer) = command_queue(64);
    Rig {
        engine: Engine::new(body(fade), consumer, publisher),
        producer,
        driver,
        clock: ManualClock::new(),
    }
}

impl Rig {
    /// Runs one tick and returns the bytes the driver receives.
    fn tick(&mut self) -> Vec<u8> {
        self.engine.run_ticks(&self.clock, 1);
        self.driver.refresh();
        self.driver.frame().channels().to_vec()
    }

    fn push(&mut self, command: TickCommand) {
        self.producer.push(command).unwrap();
    }

    /// The slot the programmer and the cue compiler both address this attribute
    /// by.
    fn slot(&self, fixture: u32, attribute: AttributeType) -> usize {
        self.engine
            .body()
            .plan()
            .index_of(FixtureId::new(fixture), attribute)
            .unwrap()
    }
}

/// The four channels of head `fixture`, as (dimmer coarse, fine, pan coarse,
/// fine).
fn head_channels(frame: &[u8], fixture: usize) -> (u8, u8, u8, u8) {
    let base = (fixture - 1) * 4;
    (
        frame[base],
        frame[base + 1],
        frame[base + 2],
        frame[base + 3],
    )
}

#[test]
fn the_priority_stack_reaches_the_wire_one_layer_at_a_time() {
    // `docs/DMX_MERGE.md` §1, from the bottom up, asserted on the bytes a
    // driver receives after each layer is added.
    let mut rig = rig(0.0);

    // Home: the bottom layer is never empty. Dark, and centred rather than at
    // pan 0.
    let frame = rig.tick();
    assert_eq!(head_channels(&frame, 1), (0x00, 0x00, 0x80, 0x00));
    assert_eq!(head_channels(&frame, 2), (0x00, 0x00, 0x80, 0x00));

    // Playbacks: the cue runs, in zero time, and both heads take its values.
    rig.push(TickCommand::Go {
        executor: ExecutorId::new(1).into(),
        direction: GoDirection::Next,
    });
    let frame = rig.tick();
    assert_eq!(head_channels(&frame, 1), (0xFF, 0xFF, 0x4E, 0x20));
    assert_eq!(head_channels(&frame, 2), (0x9C, 0x40, 0x80, 0x00));

    // Programmer: absolute override of head 1's pan, whatever the cue holds.
    let pan = rig.slot(1, AttributeType::Pan) as u32;
    rig.push(TickCommand::SetProgrammerValue {
        slot: pan,
        value: 50_000,
    });
    let frame = rig.tick();
    assert_eq!(head_channels(&frame, 1), (0xFF, 0xFF, 0xC3, 0x50));
    assert_eq!(head_channels(&frame, 2), (0x9C, 0x40, 0x80, 0x00));

    // Group master: head 1 only, intensity only. The programmer's pan is
    // untouched, which is the constraint that keeps a grand master from
    // swinging the rig as it comes down.
    rig.push(TickCommand::SetGroupMaster {
        group: GroupId::new(1),
        level: 32_767,
    });
    let frame = rig.tick();
    let halved = apply_master(65_535, 32_767);
    assert_eq!(
        head_channels(&frame, 1),
        (coarse_byte(halved), fine_byte(halved), 0xC3, 0x50)
    );
    assert_eq!(head_channels(&frame, 2), (0x9C, 0x40, 0x80, 0x00));

    // Grand master: every intensity in the rig, on top of the group master.
    rig.push(TickCommand::SetGrandMaster(32_767));
    let frame = rig.tick();
    let twice = apply_master(halved, 32_767);
    let head_two = apply_master(40_000, 32_767);
    assert_eq!(
        head_channels(&frame, 1),
        (coarse_byte(twice), fine_byte(twice), 0xC3, 0x50)
    );
    assert_eq!(
        head_channels(&frame, 2),
        (coarse_byte(head_two), fine_byte(head_two), 0x80, 0x00)
    );

    // Blackout: every intensity to zero, every position exactly where it was.
    rig.push(TickCommand::SetBlackout(true));
    let frame = rig.tick();
    assert_eq!(head_channels(&frame, 1), (0x00, 0x00, 0xC3, 0x50));
    assert_eq!(head_channels(&frame, 2), (0x00, 0x00, 0x80, 0x00));

    // And releasing it puts the rig back exactly as it was, rather than at full.
    rig.push(TickCommand::SetBlackout(false));
    let frame = rig.tick();
    assert_eq!(
        head_channels(&frame, 1),
        (coarse_byte(twice), fine_byte(twice), 0xC3, 0x50)
    );
}

#[test]
fn clearing_the_programmer_hands_the_attribute_back_to_the_playback_below_it() {
    // The other half of "the programmer always wins": letting go must give the
    // playback its attribute back, not leave the stage where the programmer put
    // it. On the wire, because that is where an operator would see it.
    let mut rig = rig(0.0);
    let pan = rig.slot(1, AttributeType::Pan) as u32;
    rig.push(TickCommand::Go {
        executor: ExecutorId::new(1).into(),
        direction: GoDirection::Next,
    });
    rig.push(TickCommand::SetProgrammerValue {
        slot: pan,
        value: 50_000,
    });
    let frame = rig.tick();
    assert_eq!(head_channels(&frame, 1).2, 0xC3);

    rig.push(TickCommand::ClearProgrammer);
    let frame = rig.tick();
    assert_eq!(head_channels(&frame, 1), (0xFF, 0xFF, 0x4E, 0x20));
}

#[test]
fn an_operator_facing_programmer_state_drives_the_same_channels() {
    // The route a daemon takes when it installs a whole programmer at once -
    // reloading a show, or applying a preset to a selection - rather than one
    // value at a time over the queue.
    let mut body = body(0.0);
    let mut state = ProgrammerState::default();
    state.set_value(
        FixtureId::new(2),
        AttributeType::Dimmer,
        ProgrammerValue {
            value: 65_535,
            source: ProgrammerValueSource::Preset,
            preset_ref: None,
        },
    );
    assert_eq!(body.load_programmer(&state), 0);

    let layout = Arc::new(FrameLayout::new([UniverseId::MIN]).unwrap());
    let mut publisher = FramePublisher::new(layout);
    let mut driver = publisher.subscribe();
    let (_producer, consumer) = command_queue(8);
    let mut engine = Engine::new(body, consumer, publisher);
    engine.run_ticks(&ManualClock::new(), 1);
    driver.refresh();

    let frame = driver.frame().channels();
    assert_eq!(head_channels(frame, 2), (0xFF, 0xFF, 0x80, 0x00));
    // Head 1 has nothing on it and is still at home.
    assert_eq!(head_channels(frame, 1), (0x00, 0x00, 0x80, 0x00));
}

/// One run of a scripted show: the same commands at the same ticks, and the
/// bytes that left the engine on every one of them.
fn scripted_run(ticks: u64) -> Vec<Vec<u8>> {
    let mut rig = rig(10.0);
    let dimmer = rig.slot(2, AttributeType::Dimmer) as u32;
    let pan = rig.slot(1, AttributeType::Pan) as u32;
    let mut frames = Vec::with_capacity(ticks as usize);
    for index in 0..ticks {
        match index {
            0 => rig.push(TickCommand::Go {
                executor: ExecutorId::new(1).into(),
                direction: GoDirection::Next,
            }),
            10 => rig.push(TickCommand::SetProgrammerValue {
                slot: pan,
                value: 50_000,
            }),
            20 => rig.push(TickCommand::SetGroupMaster {
                group: GroupId::new(1),
                level: 40_000,
            }),
            30 => rig.push(TickCommand::SetGrandMaster(30_000)),
            40 => rig.push(TickCommand::SetProgrammerValue {
                slot: dimmer,
                value: 12_345,
            }),
            50 => rig.push(TickCommand::ClearProgrammerValue { slot: pan }),
            60 => rig.push(TickCommand::SetBlackout(true)),
            70 => rig.push(TickCommand::SetBlackout(false)),
            _ => {}
        }
        frames.push(rig.tick());
    }
    frames
}

#[test]
fn identical_input_produces_byte_identical_frames_across_runs() {
    // `docs/DMX_MERGE.md` §6.4. The engine holds scratch buffers, an
    // accumulator, activation counters and a programmer whose touched list is
    // ordered by when values arrived - every one of them a place where state
    // could leak from one run into the look of the next. Three runs of the same
    // hundred ticks must produce the same three hundred frames, byte for byte.
    let ticks = 100;
    let first = scripted_run(ticks);
    let second = scripted_run(ticks);
    let third = scripted_run(ticks);

    assert_eq!(first.len(), ticks as usize);
    for (index, ((one, two), three)) in first
        .iter()
        .zip(second.iter())
        .zip(third.iter())
        .enumerate()
    {
        assert_eq!(one, two, "runs 1 and 2 differ at tick {index}");
        assert_eq!(one, three, "runs 1 and 3 differ at tick {index}");
    }

    // And the show was really running: the frames are not all the same frame,
    // and the fade moved a channel one step at a time rather than jumping.
    let distinct: std::collections::BTreeSet<&Vec<u8>> = first.iter().collect();
    assert!(
        distinct.len() > 20,
        "only {} distinct frames in {ticks} ticks, so the run was static",
        distinct.len()
    );
}

#[test]
fn a_fade_still_interpolates_underneath_the_masters() {
    // The masters scale the merged value, they do not quantise it: a ten-second
    // fade under a grand master at 30000 must still move the coarse byte in
    // single steps rather than in jumps, which is what "no visible stepping"
    // means once the whole stack is in the way.
    let mut rig = rig(10.0);
    rig.push(TickCommand::SetGrandMaster(30_000));
    rig.push(TickCommand::Go {
        executor: ExecutorId::new(1).into(),
        direction: GoDirection::Next,
    });

    let mut previous = 0u8;
    let mut moved = 0;
    for _ in 0..441 {
        let frame = rig.tick();
        let coarse = head_channels(&frame, 1).0;
        assert!(
            coarse.abs_diff(previous) <= 1,
            "the coarse byte stepped from {previous} to {coarse}"
        );
        if coarse != previous {
            moved += 1;
        }
        previous = coarse;
    }
    assert!(
        moved > 100,
        "the fade only moved {moved} times in ten seconds"
    );
    assert_eq!(previous, coarse_byte(apply_master(65_535, 30_000)));
}
