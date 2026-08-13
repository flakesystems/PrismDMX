//! Proof that the codec never calls the allocator, whatever it is fed.
//!
//! `docs/MCU_MAPPING.md` §2.4: *the codec allocates nothing per message; it
//! writes into caller-provided buffers*, and `IMPLEMENTATION_PLAN.md` S19:
//! *fuzz with truncated and out-of-range messages — no panic, **no allocation
//! growth**, correct discard counters*.
//!
//! # Why this needs an allocator and not an assertion
//!
//! S16 is the reason this file exists at all. A functionally correct
//! implementation can be the wrong one, and only the counter sees the
//! difference: a decoder that collected each SysEx into a `Vec` would pass
//! every other test in this crate — same events, same counters, same bytes —
//! while putting a heap allocation on a path that runs on the `midi-in` thread
//! next to a 44 Hz tick, and handing a hostile sender a way to make this
//! process ask the operating system for memory in a loop.
//!
//! So the claim is measured the way `prism-engine` measures the tick and
//! `prism-ipc` measures an oversized frame: a global allocator that tallies
//! what the arming thread asks for.
//!
//! # Three windows and a guard
//!
//! The hostile window is fed **more** rubbish than the ordinary window is fed
//! messages, so "no growth" is a comparison between two numbers rather than a
//! single reading — and the guard at the end proves the probe can see an
//! allocation at all, so a probe that had quietly stopped counting could not
//! pass this file.

#![allow(
    clippy::print_stdout,
    reason = "a measured criterion has to print the number it measured"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Duration;

use prism_surface::{
    ButtonId, ControlEvent, Fader, Feedback, GlobalButton, LedState, MAX_MESSAGE_BYTES, McuCodec,
    MeterSignal, RingMode, SCRIBBLE_STRIP_COLORS, SegmentChar, StripButton, StripColor, VPotRing,
    X_TOUCH,
};

/// Counts allocator calls made by whichever thread has armed the probe.
struct CountingAllocator;

thread_local! {
    /// Whether this thread is inside a measured window. `const` initialised, so
    /// touching it cannot itself allocate.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    /// Allocator calls made by this thread while armed.
    static CALLS: Cell<u64> = const { Cell::new(0) };
}

fn record() {
    // `try_with`, not `with`: during thread teardown the local is gone, and a
    // panic from inside the allocator would be unrecoverable.
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = CALLS.try_with(|calls| calls.set(calls.get() + 1));
        }
    });
}

#[allow(
    unsafe_code,
    reason = "GlobalAlloc cannot be implemented safely; the unsafety is confined to \
              this test harness and never enters the library"
)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Runs `work` with the probe armed and returns how many allocator calls it
/// made on this thread.
fn allocator_calls(work: impl FnOnce()) -> u64 {
    CALLS.with(|calls| calls.set(0));
    ARMED.with(|armed| armed.set(true));
    work();
    ARMED.with(|armed| armed.set(false));
    CALLS.with(Cell::get)
}

/// A minute of a busy desk: presses, releases, fader moves, encoder turns and
/// the scribble strip traffic that answers them.
fn a_busy_minute() -> Vec<u8> {
    let mut stream = Vec::new();
    let mut buf = [0u8; MAX_MESSAGE_BYTES];
    for round in 0..200u16 {
        let strip = (round % 8) as u8;
        let events = [
            ControlEvent::Touch {
                fader: Fader::Strip(strip),
                touched: true,
            },
            ControlEvent::Move {
                fader: Fader::Strip(strip),
                position: round.wrapping_mul(53) & 0x3FFF,
            },
            ControlEvent::Touch {
                fader: Fader::Strip(strip),
                touched: false,
            },
            ControlEvent::Button {
                button: ButtonId::Strip {
                    strip,
                    button: StripButton::Select,
                },
                pressed: true,
            },
            ControlEvent::Button {
                button: ButtonId::Strip {
                    strip,
                    button: StripButton::Select,
                },
                pressed: false,
            },
            ControlEvent::VPot {
                strip,
                steps: if round % 2 == 0 { 1 } else { -1 },
            },
            ControlEvent::Jog { steps: 3 },
            ControlEvent::Button {
                button: ButtonId::Global(GlobalButton::Play),
                pressed: true,
            },
        ];
        for event in events {
            let written = event
                .encode_into(&X_TOUCH, &mut buf)
                .expect("a control the surface has");
            stream.extend_from_slice(&buf[..written]);
        }
        // And a whole scribble strip line, which is the longest message the
        // protocol has and the one that needs reassembling.
        let text = b"SPOT 1 SPOT 2 SPOT 3 SPOT 4 SPOT 5 SPOT 6 SPOT 7 SPOT 8";
        let written = Feedback::DisplayText {
            offset: 0,
            text: text.as_slice(),
        }
        .encode_into(&X_TOUCH, &mut buf)
        .expect("the line fits");
        stream.extend_from_slice(&buf[..written]);
    }
    stream
}

/// How many times [`a_hostile_stream`] repeats itself.
const HOSTILE_ROUNDS: u64 = 400;

/// Everything a broken cable or a hostile sender can do, repeated.
///
/// Deterministic and countable on purpose: the exact number of each kind of
/// fault is asserted, which is a stronger statement than "some were counted"
/// and is what turns the discard counters into an interface rather than a
/// debug aid.
fn a_hostile_stream() -> Vec<u8> {
    let mut stream = Vec::new();
    for round in 0..HOSTILE_ROUNDS {
        // An `F7` that ends the previous round's unterminated SysEx — and on
        // the first round ends nothing, which is a stray of its own. It is
        // here rather than at the end because it also clears running status,
        // so the data bytes that follow really are orphaned.
        stream.push(0xF7);
        // Orphan data bytes: a stream joined half way through.
        stream.extend_from_slice(&[0x11, 0x22, 0x33]);
        // A message cut short by the system common that follows it.
        stream.extend_from_slice(&[0x90, 26]);
        // A system common message the MCU never uses, with its data bytes.
        stream.extend_from_slice(&[0xF2, 0x11, 0x22]);
        // An `F7` that ends nothing at all.
        stream.push(0xF7);
        // A note and a controller nothing maps, and real-time noise.
        stream.extend_from_slice(&[0x90, 120, 0x7F, 0xB0, 100, 0x01, 0xFE]);
        // A SysEx far longer than the buffer, never terminated.
        stream.push(0xF0);
        stream.extend(std::iter::repeat_n(0x7F, 300));
        // Another one, so the first is abandoned rather than finished.
        stream.push(0xF0);
        stream.extend(std::iter::repeat_n((round % 128) as u8, 300));
    }
    stream
}

#[test]
fn decoding_a_busy_desk_costs_no_allocation() {
    let stream = a_busy_minute();
    let mut codec = McuCodec::new(X_TOUCH);
    let mut events = 0u64;

    // Warm up outside the window: the first push through a code path can touch
    // lazily initialised machinery that has nothing to do with the codec.
    codec.push(&stream, Duration::ZERO, |_| events += 1);
    let warm = events;
    events = 0;

    let calls = allocator_calls(|| {
        for round in 0..20u64 {
            codec.push(&stream, Duration::from_millis(round), |_| events += 1);
        }
    });
    println!(
        "prism-surface: {} bytes x 20, {events} events, {calls} allocator calls",
        stream.len()
    );
    assert!(
        warm > 1_000,
        "the fixture is not a busy desk: {warm} events"
    );
    assert_eq!(events, warm * 20);
    assert_eq!(calls, 0, "decoding allocated {calls} times");
}

#[test]
fn a_hostile_stream_costs_no_allocation_and_does_not_grow() {
    // "No allocation growth" is a comparison, so the hostile window is fed more
    // bytes than the ordinary one and both readings are taken. A decoder that
    // buffered a SysEx on the heap would report a number here that rose with
    // the number of rounds.
    let hostile = a_hostile_stream();
    let mut codec = McuCodec::new(X_TOUCH);
    codec.push(&hostile, Duration::ZERO, |event| {
        panic!("nothing in a hostile stream is a control: {event:?}")
    });

    // What one pass over the stream contains, to the message. Exact rather
    // than "more than a thousand", because a counter that is out by a factor
    // is a counter nobody can reason from.
    let first = codec.counters();
    assert_eq!(first.wire.orphan_data, 3 * HOSTILE_ROUNDS);
    assert_eq!(first.wire.truncated, HOSTILE_ROUNDS);
    assert_eq!(first.wire.system_common, HOSTILE_ROUNDS);
    assert_eq!(first.wire.realtime, HOSTILE_ROUNDS);
    assert_eq!(first.wire.sysex_interrupted, HOSTILE_ROUNDS);
    assert_eq!(first.wire.sysex_overflow, 2 * HOSTILE_ROUNDS);
    // One per round, plus the leading `F7` of the first round, which had no
    // SysEx of a previous round to close.
    assert_eq!(first.wire.stray_end, HOSTILE_ROUNDS + 1);
    assert_eq!(first.unmapped, 2 * HOSTILE_ROUNDS);
    assert_eq!(first.wire.messages, 2 * HOSTILE_ROUNDS);
    assert_eq!(first.sysex_ignored, 0);

    let short = allocator_calls(|| {
        codec.push(&hostile, Duration::from_millis(1), |_| {});
    });
    let long = allocator_calls(|| {
        for round in 2..12u64 {
            codec.push(&hostile, Duration::from_millis(round), |_| {});
        }
    });
    let counters = codec.counters();
    println!(
        "prism-surface: {} hostile bytes, {} allocator calls once and {} allocator calls ten times",
        hostile.len(),
        short,
        long
    );
    println!("prism-surface: hostile counters {counters:?}");
    // Eleven more passes, so every counter is eleven times the exact figure
    // above bar the one that differed on the first round.
    assert_eq!(counters.wire.sysex_overflow, 24 * HOSTILE_ROUNDS);
    assert_eq!(counters.wire.orphan_data, 36 * HOSTILE_ROUNDS);
    assert_eq!(counters.unmapped, 24 * HOSTILE_ROUNDS);
    assert_eq!(counters.wire.stray_end, 12 * HOSTILE_ROUNDS + 1);
    assert_eq!(short, 0, "one pass allocated {short} times");
    assert_eq!(long, 0, "ten passes allocated {long} times");
}

#[test]
fn writing_every_kind_of_message_costs_no_allocation() {
    // The outbound half: `encode_into` takes the caller's buffer, so a feedback
    // burst at the 30 Hz of §5.2 is free after the buffer exists.
    let mut buf = [0u8; MAX_MESSAGE_BYTES];
    let text = b"SPOT 1 SPOT 2 SPOT 3 SPOT 4 SPOT 5 SPOT 6 SPOT 7 SPOT 8";
    let messages: [Feedback<'_>; 8] = [
        Feedback::Led {
            button: ButtonId::Global(GlobalButton::Save),
            state: LedState::Flashing,
        },
        Feedback::Move {
            fader: Fader::Main,
            position: 12_700,
        },
        Feedback::Ring {
            strip: 3,
            ring: VPotRing {
                mode: RingMode::Wrap,
                position: 7,
                led: true,
            },
        },
        Feedback::Meter {
            strip: 5,
            signal: MeterSignal::Level(9),
        },
        Feedback::Segment {
            digit: 4,
            character: SegmentChar {
                code: 52,
                dot: true,
            },
        },
        Feedback::DisplayText {
            offset: 0,
            text: text.as_slice(),
        },
        Feedback::DisplayColors([StripColor::Cyan; SCRIBBLE_STRIP_COLORS]),
        Feedback::DeviceQuery,
    ];
    let mut written = 0usize;
    for message in &messages {
        written += message.encode_into(&X_TOUCH, &mut buf).expect("it fits");
    }

    let calls = allocator_calls(|| {
        for _ in 0..1_000 {
            for message in &messages {
                let bytes = message.encode_into(&X_TOUCH, &mut buf).unwrap_or(0);
                written += bytes;
            }
        }
    });
    println!("prism-surface: {written} bytes written, {calls} allocator calls");
    assert!(written > 90_000, "{written} bytes");
    assert_eq!(calls, 0, "encoding allocated {calls} times");
}

#[test]
fn the_probe_can_see_an_allocation() {
    // Without this the three tests above pass just as well with a probe that
    // stopped counting - which is the failure mode of every measurement that
    // reports zero.
    let calls = allocator_calls(|| {
        let held: Vec<u8> = Vec::with_capacity(4_096);
        assert_eq!(held.capacity(), 4_096);
    });
    assert!(calls > 0, "the probe counted nothing at all");
}
