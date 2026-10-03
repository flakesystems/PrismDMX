//! What a hostile, broken or merely strange stream on the PSN group does - S32.
//!
//! `tests/artpoll_fuzz.rs`'s three claims about the other stream of bytes a
//! stranger controls. The tracker group is **multicast**: anything on the
//! network can write to it, and the thread reading it is inside the process
//! driving the show, so a panic there is `CLAUDE.md`'s zero-crash invariant
//! broken by whoever has a packet generator.
//!
//! - **No panic** - a quarter of a million datagrams, random and
//!   structure-aware (right roots, wrong lengths; right lengths, wrong
//!   contents), through the codec and through the receiver.
//! - **No allocation** - [`prism_protocols::psn::decode`] calls a closure for
//!   what it finds and builds nothing, and this *counts* that with the same
//!   counting allocator the Art-Net harness uses, with a guard at the end that
//!   proves the probe can see an allocation at all.
//! - **Nothing swallowed** - every datagram the receiver was given is one it
//!   counted, which is the accounting identity that stops a parser from quietly
//!   dropping the packet behind the one it choked on.

#![allow(
    clippy::print_stdout,
    reason = "a measured criterion has to print the number it measured"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use prism_domain::TrackerMapping;
use prism_engine::{ManualClock, TrackerTable};
use prism_protocols::psn::{self, decode, encode_data, encode_info};
use prism_protocols::{Listen, MockUdpNode, TrackerReceiver, TrackingConfig};

struct CountingAllocator;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static CALLS: Cell<u64> = const { Cell::new(0) };
}

fn record() {
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

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measure(body: impl FnOnce()) -> u64 {
    CALLS.with(|calls| calls.set(0));
    ARMED.with(|armed| armed.set(true));
    body();
    ARMED.with(|armed| armed.set(false));
    CALLS.with(Cell::get)
}

/// `prism-surface`'s xorshift64*, so the sequence is the same on every machine.
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

/// A datagram in one of the shapes a parser might trip over.
fn hostile(rng: &mut Xorshift) -> Vec<u8> {
    let length = rng.upto(240);
    let mut bytes: Vec<u8> = (0..length).map(|_| rng.byte()).collect();
    match rng.upto(6) {
        // Pure noise.
        0 => {}
        // A PSN root with a random length field, so the chunk walk starts and
        // runs off wherever it likes.
        1 => {
            let root: [u8; 2] = if rng.upto(2) == 0 {
                psn::DATA_PACKET.to_le_bytes()
            } else {
                psn::INFO_PACKET.to_le_bytes()
            };
            bytes.splice(0..bytes.len().min(2), root);
        }
        // A real packet with a byte flipped.
        2 => {
            let mut packet = encode_data(0, 0, &[(1, [1.0, 2.0, 3.0]), (2, [4.0, 5.0, 6.0])]);
            let at = rng.upto(packet.len());
            packet[at] ^= rng.byte() | 1;
            bytes = packet;
        }
        // A real packet cut short.
        3 => {
            let packet = encode_info(0, 0, "System", &[(1, "A name"), (2, "Another")]);
            let cut = rng.upto(packet.len());
            bytes = packet[..cut].to_vec();
        }
        // Every length field at its maximum.
        4 => {
            let mut packet = encode_data(0, 0, &[(7, [0.0, 0.0, 0.0])]);
            for at in [2, 3, 22, 23, 26, 27, 30, 31] {
                if let Some(byte) = packet.get_mut(at) {
                    *byte = 0xFF;
                }
            }
            bytes = packet;
        }
        // Floats that are not numbers.
        _ => {
            let specials = [
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::MAX,
                f32::MIN,
            ];
            let pick = |rng: &mut Xorshift| specials[rng.upto(specials.len())];
            bytes = encode_data(0, 0, &[(3, [pick(rng), pick(rng), pick(rng)])]);
        }
    }
    bytes
}

#[test]
fn a_quarter_of_a_million_hostile_datagrams_make_the_codec_panic_never() {
    let mut rng = Xorshift(0x05EE_D0F5);
    let mut positions = 0_u64;
    for _ in 0..250_000 {
        let datagram = hostile(&mut rng);
        let _ = decode(&datagram, &mut |_| positions += 1);
    }
    println!("positions found in 250 000 hostile datagrams: {positions}");
}

#[test]
fn reading_a_datagram_allocates_nothing_at_all() {
    // A full frame: a few dozen trackers, and the codec is asked to find every
    // one of them.
    let trackers: Vec<(u16, [f32; 3])> = (0..60).map(|id| (id, [1.0, 2.0, 3.0])).collect();
    let valid = encode_data(1, 2, &trackers);
    let mut rng = Xorshift(0xFEED_5EED);
    let hostile_window: Vec<Vec<u8>> = (0..20_000).map(|_| hostile(&mut rng)).collect();

    let mut found = 0_usize;
    let ordinary = measure(|| {
        for _ in 0..1_000 {
            let _ = decode(&valid, &mut |_| found += 1);
        }
    });
    assert_eq!(found, 60 * 1_000, "the probe is reading real packets");
    let hostile_calls = measure(|| {
        for datagram in &hostile_window {
            let _ = decode(datagram, &mut |_| {});
        }
    });
    println!("allocator calls: {ordinary} on 1 000 real frames, {hostile_calls} on 20 000 hostile");
    assert_eq!(ordinary, 0, "reading a frame called the allocator");
    assert_eq!(hostile_calls, 0, "reading rubbish called the allocator");

    // **The probe can see.** A harness that had quietly stopped counting would
    // pass everything above.
    let seen = measure(|| {
        let v = vec![0_u8; 64];
        std::hint::black_box(&v);
    });
    assert!(
        seen > 0,
        "the counting allocator no longer sees allocations"
    );
}

/// Nothing swallowed: every datagram the receiver is given is one it counts, and
/// the table is never poisoned - a position the table holds is a finite number.
#[test]
fn the_receiver_counts_every_datagram_and_stores_only_numbers() {
    let socket = MockUdpNode::new();
    let handle = socket.handle();
    let table = Arc::new(TrackerTable::new());
    let config = TrackingConfig::new(
        Listen::Unicast(SocketAddr::from(([127, 0, 0, 1], 0))),
        TrackerMapping::default(),
        Duration::from_millis(500),
    );
    let mut receiver =
        TrackerReceiver::with_clock(socket, config, Arc::clone(&table), ManualClock::new());
    receiver.open().unwrap();

    let from = SocketAddr::from(([10, 0, 0, 9], 56_565));
    let mut rng = Xorshift(0x00C0_FFEE);
    let mut delivered = 0_u64;
    for _ in 0..20_000 {
        // Datagrams over the receiver's buffer are refused by the double the way
        // Windows refuses them, and counted all the same.
        handle.deliver(from, &hostile(&mut rng));
        delivered += 1;
        if delivered.is_multiple_of(64) {
            receiver.service();
        }
    }
    for _ in 0..1_000 {
        if receiver.counters().datagrams >= delivered {
            break;
        }
        receiver.service();
    }
    let counters = receiver.counters();
    assert_eq!(
        counters.datagrams, delivered,
        "a datagram was swallowed: {counters:?}"
    );
    for tracker in 0..=prism_domain::MAX_TRACKER {
        if let Some(position) = table.read(tracker) {
            assert!(
                position.x.is_finite() && position.y.is_finite() && position.z.is_finite(),
                "tracker {tracker} holds {position:?}"
            );
        }
    }
    println!("{counters:?}");
}
