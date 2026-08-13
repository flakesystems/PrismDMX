//! What a broken cable, a half-connected surface and a hostile stream do.
//!
//! `IMPLEMENTATION_PLAN.md` S19, third exit criterion, and
//! `docs/MCU_MAPPING.md` §6: *fuzz with truncated and out-of-range messages;
//! assert no panic, no allocation growth, correct discard counters*.
//!
//! The **no allocation growth** half is measured next door in
//! `codec_allocations.rs`, because it needs a global allocator and a global
//! allocator affects every test in its binary. This file is the other two.
//!
//! # "Correct discard counters" is an identity, not a number
//!
//! For a random byte stream nobody can say what the counters *should* read.
//! What can be said is that they add up, and that is what is asserted here:
//!
//! ```text
//!   every byte pushed          == counters.wire.bytes
//!   every complete message     == events + unmapped + sysex_ignored
//! ```
//!
//! A codec that quietly swallowed a message would break the second identity; a
//! codec that invented one would break it the other way. Both are what
//! `CLAUDE.md` means by *an invalid MIDI packet must never propagate a
//! failure* — the packet is not merely survived, its fate is written down.
//!
//! # Where the bytes come from
//!
//! Two sources, because they find different things. A `proptest` generator
//! explores the shape of a stream and shrinks a failure to something a person
//! can read. A deterministic xorshift walks a **large** volume of bytes — a
//! quarter of a million per run — which is where a state machine that only
//! wedges after a particular sequence turns up. Neither needs a dependency
//! beyond the one `prism-engine` and `prism-ipc` already use.

use std::time::Duration;

use proptest::prelude::*;

use prism_surface::{ControlEvent, McuCodec, MidiDecoder, X_TOUCH};

/// A moment for the decoder's SysEx timeout. Held fixed so a fuzz run measures
/// the parser and not the clock.
const NOW: Duration = Duration::from_millis(3);

/// A reproducible stream of bytes.
///
/// xorshift64*, so the sequence is the same on every machine and a failure can
/// be re-run by quoting the seed. `rand` is not a dependency of this workspace
/// and a fuzz generator is not a reason to make it one.
struct Xorshift(u64);

impl Xorshift {
    fn next_u64(&mut self) -> u64 {
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.0 = state;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 24) as u8
    }

    fn upto(&mut self, limit: usize) -> usize {
        (self.next_u64() % limit as u64) as usize
    }
}

/// Runs a stream through the codec and returns what came out and what was
/// counted, checking the accounting identity on the way.
fn run(packets: &[Vec<u8>]) -> (Vec<ControlEvent>, prism_surface::CodecCounters) {
    let mut codec = McuCodec::new(X_TOUCH);
    let mut events = Vec::new();
    let mut pushed = 0u64;
    for packet in packets {
        codec.push(packet, NOW, |event| events.push(event));
        pushed += packet.len() as u64;
    }
    let counters = codec.counters();
    assert_eq!(counters.wire.bytes, pushed, "a byte went uncounted");
    assert_eq!(
        counters.wire.messages,
        events.len() as u64 + counters.unmapped + counters.sysex_ignored,
        "a complete message was neither delivered nor counted"
    );
    (events, counters)
}

#[test]
fn a_quarter_of_a_million_random_bytes_produce_no_panic_and_add_up() {
    // The volume matters: a parser that wedges only after a particular sequence
    // of a status byte, a truncation and a SysEx is not going to be found by
    // twenty bytes.
    let mut rng = Xorshift(0x5EED_1234_ABCD_0001);
    let mut total = 0usize;
    let mut counters = prism_surface::CodecCounters::default();
    for _ in 0..1_000 {
        let length = 1 + rng.upto(512);
        let packet: Vec<u8> = (0..length).map(|_| rng.byte()).collect();
        total += packet.len();
        let (_, seen) = run(&[packet]);
        counters = seen;
    }
    assert!(total > 250_000, "{total} bytes is not a fuzz run");
    // The last run's counters are a spot check that the fuzz is producing
    // something rather than a stream of one repeated byte.
    assert!(counters.wire.bytes > 0);
}

#[test]
fn a_random_stream_decodes_the_same_however_it_is_cut_into_packets() {
    // The property a MIDI transport forces on a parser: a USB packet boundary
    // is not a message boundary, and where the driver chose to cut must not
    // change a single event. This is the strongest single claim in the file -
    // it fails for any parser that keeps state on the stack of `push`.
    let mut rng = Xorshift(0xC0FF_EE00_1234_5678);
    for _ in 0..200 {
        let length = 1 + rng.upto(300);
        let stream: Vec<u8> = (0..length).map(|_| rng.byte()).collect();

        let (whole, whole_counters) = run(std::slice::from_ref(&stream));
        let one_at_a_time: Vec<Vec<u8>> = stream.iter().map(|byte| vec![*byte]).collect();
        let (dribbled, dribbled_counters) = run(&one_at_a_time);
        assert_eq!(whole, dribbled, "cut into single bytes: {stream:02X?}");
        assert_eq!(whole_counters, dribbled_counters);

        // And at an arbitrary interior point, which is what a real driver does.
        let cut = rng.upto(stream.len());
        let (split_first, split_second) = stream.split_at(cut);
        let (split, split_counters) = run(&[split_first.to_vec(), split_second.to_vec()]);
        assert_eq!(whole, split, "cut at {cut}: {stream:02X?}");
        assert_eq!(whole_counters, split_counters);
    }
}

#[test]
fn every_valid_message_cut_short_is_discarded_and_counted() {
    // "Truncated" in the criterion means this: the surface started saying
    // something and stopped. Each prefix of each message is tried, and the
    // message that follows must still arrive - a parser that lost its place
    // would swallow the next press instead.
    let messages: [&[u8]; 6] = [
        &[0x90, 26, 0x7F],                                 // strip 3 Select
        &[0xE4, 0x6C, 0x63],                               // strip 5 fader
        &[0xB0, 19, 0x41],                                 // strip 4 V-Pot
        &[0xD0, 0x3C],                                     // a meter
        &[0xF0, 0x00, 0x00, 0x66, 0x14, 0x21, 0x00, 0xF7], // meter mode
        &[0x90, 112, 0x7F],                                // main fader touched
    ];
    let follows: &[u8] = &[0x90, 94, 0x7F];

    for message in messages {
        for cut in 1..message.len() {
            let mut stream = message[..cut].to_vec();
            stream.extend_from_slice(follows);
            let (events, counters) = run(std::slice::from_ref(&stream));
            assert_eq!(
                events.len(),
                1,
                "{message:02X?} cut at {cut} produced {events:?}"
            );
            // A lone status byte replaced by the next one is *not* a fault: a
            // sender restating its status is legal and common, and counting it
            // would fill the diagnostics with noise and bury the real thing.
            // A `F0` on its own is different — it opened a SysEx.
            let started_saying_something = cut > 1 || message.first() == Some(&0xF0);
            assert_eq!(
                counters.discarded() > 0,
                started_saying_something,
                "{message:02X?} cut at {cut} counted {counters:?}"
            );
        }
    }
}

#[test]
fn a_note_or_a_controller_outside_every_table_is_unmapped_rather_than_guessed() {
    // "Out-of-range" in the criterion. A MIDI data byte cannot be out of range
    // - it is seven bits by construction - so what is out of range is the
    // *control*: notes 113-127, the CCs between the blocks, and every channel
    // but the one this surface is on.
    let mut stream = Vec::new();
    let mut expected = 0u64;
    for note in 113..=127u8 {
        stream.extend_from_slice(&[0x90, note, 0x7F]);
        expected += 1;
    }
    for controller in [24u8, 40, 47, 59, 61, 63, 76, 100, 127] {
        stream.extend_from_slice(&[0xB0, controller, 0x01]);
        expected += 1;
    }
    for channel in 1..=15u8 {
        stream.extend_from_slice(&[0x90 | channel, 26, 0x7F]);
        expected += 1;
    }
    let (events, counters) = run(&[stream]);
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(counters.unmapped, expected);
    assert_eq!(counters.wire.discarded(), 0, "none of this is malformed");
}

#[test]
fn a_stream_of_unterminated_sysex_costs_a_counter_and_nothing_else() {
    // The one buffer a sender can fill. Each message is abandoned by the next
    // F0, so the buffer is reused rather than grown - which the allocator test
    // measures, and which this one states in terms of the counters.
    let mut codec = McuCodec::new(X_TOUCH);
    let mut events = 0usize;
    for index in 0..5_000u64 {
        let mut packet = vec![0xF0];
        packet.extend(std::iter::repeat_n(0x7F, 200));
        codec.push(&packet, Duration::from_millis(index), |_| events += 1);
    }
    // ...and then the surface says something real.
    codec.push(
        &[0xF7, 0x90, 94, 0x7F],
        Duration::from_millis(6_000),
        |_| events += 1,
    );
    let counters = codec.counters();
    assert_eq!(events, 1, "the button press after the flood must arrive");
    assert_eq!(counters.wire.sysex_overflow, 5_000);
    assert_eq!(counters.wire.sysex_interrupted, 4_999);
    assert_eq!(counters.wire.messages, 1);
}

#[test]
fn a_surface_that_goes_quiet_mid_message_is_not_waited_for_for_ever() {
    // The timeout, at the codec's own front door. Everything in this suite is
    // on a supplied clock, so no test sleeps - `docs/MCU_MAPPING.md` §6's
    // "assert timeout drops an unterminated message" costs microseconds.
    let mut codec = McuCodec::with_timeout(X_TOUCH, Duration::from_millis(50));
    codec.push(&[0xF0, 0x00, 0x00, 0x66], Duration::ZERO, |event| {
        panic!("half a message is not an event: {event:?}")
    });
    assert!(!codec.poll(Duration::from_millis(49)));
    assert!(codec.poll(Duration::from_millis(50)));
    assert_eq!(codec.counters().wire.sysex_timeout, 1);
    assert!(!codec.poll(Duration::from_secs(10)), "dropped only once");
}

#[test]
fn the_decoder_never_holds_a_message_open_across_a_status_byte() {
    // A liveness claim rather than a safety one: after any stream that ends on
    // a status byte other than F0, the decoder is not reassembling - so a
    // surface that is unplugged mid-SysEx and comes back cannot leave the codec
    // waiting on a message from the last time it was on.
    let mut rng = Xorshift(0x0BAD_F00D_0000_0001);
    for _ in 0..500 {
        let mut decoder = MidiDecoder::new();
        let length = 1 + rng.upto(64);
        let mut stream: Vec<u8> = (0..length).map(|_| rng.byte()).collect();
        stream.push(0x90);
        decoder.push(&stream, NOW, |_| {});
        assert!(!decoder.is_reassembling(), "{stream:02X?}");
        assert!(decoder.has_running_status());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn no_byte_stream_stops_the_next_press_from_arriving(
        packets in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..64), 0..24),
    ) {
        // `CLAUDE.md`'s invariant stated as the thing an operator would notice:
        // not merely that nothing panicked, but that after *any* rubbish - a
        // half-finished SysEx, a message cut short, a stream of orphans - the
        // Play button still works. Empty packets are in the generator on
        // purpose: a port returning zero bytes is an ordinary thing for a port
        // to do.
        let (_, counters) = run(&packets);
        let mut codec = McuCodec::new(X_TOUCH);
        for packet in &packets {
            codec.push(packet, NOW, |_| {});
        }
        let mut got = Vec::new();
        codec.push(&[0x90, 94, 0x7F], NOW, |event| got.push(event));
        prop_assert_eq!(got.len(), 1, "after {} bytes of rubbish", counters.wire.bytes);
    }

    #[test]
    fn a_valid_message_hidden_in_rubbish_still_arrives(
        before in prop::collection::vec(0x00..=0x7Fu8, 0..32),
        after in prop::collection::vec(0x00..=0x7Fu8, 0..32),
    ) {
        // Data bytes on either side, which is the shape of a stream joined
        // half way through. They are orphans; the message between them is not.
        let mut stream = before.clone();
        stream.extend_from_slice(&[0x90, 26, 0x7F, 0xF7]);
        stream.extend_from_slice(&after);
        let (events, counters) = run(&[stream]);
        prop_assert_eq!(events.len(), 1);
        prop_assert_eq!(
            counters.wire.orphan_data,
            (before.len() + after.len()) as u64
        );
        prop_assert_eq!(counters.wire.stray_end, 1);
    }
}
