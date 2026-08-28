//! `Cue → Fade → Frame`, end to end, on a simulated clock.
//!
//! The unit tests check the fade arithmetic and the cue state machine
//! separately. This target runs them through the whole engine — a Go arriving
//! over the command queue, 44 Hz of absolute deadlines, the merge, the encoder —
//! and asserts the **bytes** a fixture would receive.
//!
//! That is where the second of this session's exit criteria lives. "Fades
//! interpolate in 16 bit even on 8-bit patched channels" is a claim about what
//! comes out of the encoder, not about what the merge holds internally, and the
//! only way to check it is to watch the wire.

#![cfg(not(loom))]

use std::sync::Arc;
use std::time::Duration;

use prism_domain::{
    AttributeDef, AttributeType, Cue, CuePart, CueTrigger, ExecutorId, Fixture, FixtureId,
    FixtureType, GoDirection, Sequence, SequenceId, UniverseId, Vec3,
};
use prism_engine::{
    Clock, Engine, FrameLayout, FramePublisher, FrameSubscriber, ManualClock, MergeBody,
    TICK_PERIOD, TickCommand, command_queue,
};

/// A dimmer patched 8-bit: one channel, dark at home.
fn dimmer_8() -> FixtureType {
    fixture_type("test.dim8", 1, vec![attribute(0, None)])
}

/// The same dimmer patched 16-bit: coarse and fine, dark at home.
fn dimmer_16() -> FixtureType {
    fixture_type("test.dim16", 2, vec![attribute(0, Some(1))])
}

fn attribute(coarse_offset: u16, fine_offset: Option<u16>) -> AttributeDef {
    AttributeDef {
        attribute: AttributeType::Dimmer,
        feature_group: AttributeType::Dimmer.feature_group(),
        coarse_offset,
        fine_offset,
        default_value: 0,
        merge_mode: AttributeType::Dimmer.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

fn fixture_type(id: &str, footprint: u16, attributes: Vec<AttributeDef>) -> FixtureType {
    FixtureType {
        id: id.to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Test".to_owned(),
        mode: "test".to_owned(),
        footprint,
        attributes,
    }
}

fn fixture(type_id: &str) -> Fixture {
    Fixture {
        software_dimmer: true,
        id: FixtureId::new(1),
        name: "Dimmer".to_owned(),
        type_id: type_id.to_owned(),
        universe: UniverseId::MIN,
        address: 1,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        invert_pan: false,
        invert_tilt: false,
    }
}

/// One cue taking the dimmer of fixture 1 to `value` over `fade` seconds.
fn cue(number: &str, value: u16, fade: f64) -> Cue {
    Cue {
        number: number.to_owned(),
        name: format!("Cue {number}"),
        fade_in: fade,
        fade_out: fade,
        delay: 0.0,
        trigger: CueTrigger::Go,
        trigger_time: None,
        parts: vec![CuePart {
            fixture: FixtureId::new(1),
            attribute: AttributeType::Dimmer,
            value,
            preset_ref: None,
        }],
    }
}

fn sequence(cues: Vec<Cue>) -> Sequence {
    Sequence {
        id: SequenceId::new(1),
        name: "Main".to_owned(),
        color: None,
        cues,
        looping: false,
        is_active: false,
        current_cue_index: None,
    }
}

struct Rig {
    engine: Engine<MergeBody>,
    producer: prism_engine::Producer<TickCommand>,
    subscriber: FrameSubscriber,
    clock: ManualClock,
}

impl Rig {
    /// One dimmer of the given type, on executor 1, running `cues`.
    fn new(fixture_type: &FixtureType, cues: Vec<Cue>) -> Self {
        let layout = FrameLayout::new([UniverseId::MIN]).unwrap();
        let patched = fixture(&fixture_type.id);
        let mut body =
            MergeBody::for_patch(&layout, [(&patched, fixture_type)], [ExecutorId::new(1)])
                .unwrap();
        body.load_sequence(ExecutorId::new(1), &sequence(cues))
            .unwrap();

        let mut publisher = FramePublisher::new(Arc::new(layout));
        let subscriber = publisher.subscribe();
        let (producer, consumer) = command_queue(16);
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
                executor: ExecutorId::new(1).into(),
                direction: GoDirection::Next,
            })
            .unwrap();
    }

    /// Runs one tick and returns the dimmer's channels as they reach a driver.
    fn tick(&mut self) -> (u8, u8) {
        self.engine.run_ticks(&self.clock, 1);
        assert!(self.subscriber.refresh());
        let frame = self.subscriber.frame();
        (
            frame.channel(0, 1).unwrap_or_default(),
            frame.channel(0, 2).unwrap_or_default(),
        )
    }
}

#[test]
fn a_ten_second_fade_on_an_eight_bit_channel_never_steps_by_more_than_one_byte() {
    // The criterion, on the wire. A fade computed in 8 bits would move the
    // channel on one tick in three and sit still on the other two, which is the
    // stepping docs/DMX_MERGE.md 5 is about. Computed in 16 bits and quantised
    // at the write, the byte climbs by exactly one at a time and passes through
    // every one of the 256 values on the way.
    let mut rig = Rig::new(&dimmer_8(), vec![cue("1", 65_535, 10.0)]);
    rig.go();

    let mut seen = [false; 256];
    let mut previous = 0u8;
    // Tick 0 starts the fade; 440 ticks is ten seconds.
    for index in 0..=440u32 {
        let (coarse, _) = rig.tick();
        assert!(
            coarse >= previous && coarse - previous <= 1,
            "tick {index} stepped from {previous} to {coarse}"
        );
        seen[usize::from(coarse)] = true;
        previous = coarse;
    }

    assert_eq!(previous, 255, "the fade did not arrive");
    assert!(
        seen.iter().all(|hit| *hit),
        "the fade skipped a byte value, so it stepped"
    );
    assert!(rig.clock.now().abs_diff(Duration::from_secs(10)) < TICK_PERIOD);
}

#[test]
fn the_same_fade_is_the_same_fade_whatever_resolution_it_is_patched_at() {
    // The internal value is 16-bit regardless of the patch, so the coarse byte
    // of an 8-bit channel and the coarse byte of a 16-bit one are the same byte
    // at every instant. If the fade were computed against the patched
    // resolution, these would diverge.
    let mut eight = Rig::new(&dimmer_8(), vec![cue("1", 65_535, 10.0)]);
    let mut sixteen = Rig::new(&dimmer_16(), vec![cue("1", 65_535, 10.0)]);
    eight.go();
    sixteen.go();

    let mut fine_values = 0u32;
    for index in 0..=440u32 {
        let (coarse, _) = eight.tick();
        let (coarse_16, fine_16) = sixteen.tick();
        assert_eq!(
            coarse, coarse_16,
            "the two patches disagreed at tick {index}"
        );
        if fine_16 != 0 {
            fine_values += 1;
        }
    }
    // And the 16-bit patch really is carrying the extra bits rather than
    // repeating the coarse byte.
    assert!(
        fine_values > 300,
        "only {fine_values} ticks had a fine byte"
    );
}

#[test]
fn a_go_arriving_over_the_command_queue_runs_the_cue_list_to_the_wire() {
    // Patch, cue list, command queue, tick, merge, encoder, triple buffer: the
    // whole chain, with nothing written into the engine by hand.
    let mut rig = Rig::new(
        &dimmer_8(),
        vec![cue("1", 65_535, 0.0), cue("2", 32_768, 0.0)],
    );

    // Nothing has been fired, so the rig sits at its home value.
    assert_eq!(rig.tick(), (0x00, 0x00));

    rig.go();
    assert_eq!(rig.tick(), (0xFF, 0x00));

    rig.go();
    assert_eq!(rig.tick(), (0x80, 0x00));

    // **A Go at the end comes round to the first cue**, on a list that does not
    // loop as much as on one that does: an operator at the bottom of a list who
    // presses Go wants the top of it, and a desk that held would look broken.
    // Nothing goes to black on the way — cue 1 is entered with its own fade,
    // exactly as it was the first time.
    rig.go();
    assert_eq!(rig.tick(), (0xFF, 0x00));

    // Switching the executor off releases it, and the channel falls back to the
    // home value the patch gives it.
    rig.producer
        .push(TickCommand::SetExecutorActive {
            executor: ExecutorId::new(1).into(),
            on: false,
        })
        .unwrap();
    assert_eq!(rig.tick(), (0x00, 0x00));
    assert_eq!(rig.engine.stats().panics, 0);
    assert_eq!(rig.engine.stats().commands, 4);
}

#[test]
fn a_go_part_way_through_a_fade_carries_on_from_where_it_had_got_to() {
    // The determinism criterion, on the wire: the byte at the instant of the Go
    // is the byte the previous fade had reached, and the next fade starts there.
    let mut rig = Rig::new(&dimmer_8(), vec![cue("1", 65_535, 10.0), cue("2", 0, 5.0)]);
    rig.go();
    for _ in 0..220 {
        rig.tick();
    }
    // Five seconds into a ten-second fade: half of 65535 is 32767, coarse 0x7F.
    let (halfway, _) = rig.tick();
    assert_eq!(halfway, 0x7F);

    rig.go();
    let (at_the_go, _) = rig.tick();
    assert_eq!(at_the_go, 0x7F, "the Go stepped the channel");

    // And down again over five seconds, one byte at a time.
    let mut previous = at_the_go;
    for index in 0..220u32 {
        let (coarse, _) = rig.tick();
        assert!(
            coarse <= previous && previous - coarse <= 1,
            "tick {index} stepped from {previous} to {coarse}"
        );
        previous = coarse;
    }
    assert_eq!(previous, 0x00);
}
