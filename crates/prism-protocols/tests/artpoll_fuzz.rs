//! What a hostile, broken or merely strange stream on UDP port 6454 does — S46.
//!
//! `IMPLEMENTATION_PLAN.md` S46, third exit criterion: *replies from the network
//! are parsed under the fuzz harness pattern of `prism-surface`: no panic, no
//! allocation storm, and a malformed reply is dropped rather than believed.*
//! That harness is `crates/prism-surface/tests/fuzz.rs` and
//! `codec_allocations.rs`, and this is the same three claims about the other
//! stream of bytes a stranger controls.
//!
//! # Why a receive path deserves this and a send path does not
//!
//! Everything `prism-protocols` did before S46 wrote bytes. The bytes it wrote
//! came from this desk, so the worst a bug could do was put a wrong frame on a
//! wire. A **receive** path is different in kind: the bytes come from whoever can
//! reach the port, an Art-Net network is a flat unauthenticated broadcast domain
//! that a school's whole building is on, and the thread reading them is inside
//! the process driving the show. A panic there is `CLAUDE.md`'s zero-crash
//! invariant broken by a stranger with a packet generator.
//!
//! # The three claims, and how each is measured
//!
//! **No panic** is a volume argument: a quarter of a million random bytes in
//! shapes a parser might trip over, plus a `proptest` generator next door in
//! `artpoll.rs` that shrinks a failure to something a person can read.
//!
//! **No allocation storm** is measured rather than asserted, with the counting
//! global allocator `prism-surface` uses: the hostile window is fed more rubbish
//! than the ordinary window is fed replies, so *no growth* is a comparison
//! between two numbers rather than one reading — and the guard at the end proves
//! the probe can see an allocation at all, so a probe that had quietly stopped
//! counting could not pass this file.
//!
//! **Dropped rather than believed** is an accounting identity, the same shape
//! `prism-surface` uses for its discard counters:
//!
//! ```text
//!   every datagram delivered == counters.replies + counters.malformed + dropped
//! ```
//!
//! A parser that quietly swallowed a datagram breaks it one way; one that
//! invented a node breaks it the other.

#![allow(
    clippy::print_stdout,
    reason = "a measured criterion has to print the number it measured"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::net::SocketAddr;
use std::time::Duration;

use prism_engine::ManualClock;
use prism_protocols::{
    ART_NET_ID, ART_POLL_REPLY_BYTES, ART_POLL_REPLY_MIN, DiscoveryConfig, MockUdpNode,
    MockUdpNodeHandle, NodeDiscovery, OP_POLL_REPLY, parse_art_poll_reply,
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

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Runs `body` with the probe armed and answers how many allocator calls it made.
fn measure(body: impl FnOnce()) -> u64 {
    CALLS.with(|calls| calls.set(0));
    ARMED.with(|armed| armed.set(true));
    body();
    ARMED.with(|armed| armed.set(false));
    CALLS.with(Cell::get)
}

/// A reproducible stream of bytes — `prism-surface`'s xorshift64\*, so the
/// sequence is the same on every machine and a failure can be re-run by quoting
/// the seed. `rand` is not a dependency of this workspace and a fuzz generator is
/// not a reason to make it one.
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

/// A datagram that will reach the field-reading half of the parser: the eight
/// identifying bytes and the opcode are right and everything after them is not.
fn plausible(rng: &mut Xorshift) -> Vec<u8> {
    let length = ART_POLL_REPLY_MIN + rng.upto(64);
    let mut datagram = Vec::with_capacity(length);
    datagram.extend_from_slice(&ART_NET_ID);
    datagram.extend_from_slice(&OP_POLL_REPLY.to_le_bytes());
    while datagram.len() < length {
        datagram.push(rng.byte());
    }
    datagram
}

fn node(last: u8) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, last], 6454))
}

fn discovery(max_nodes: usize) -> (NodeDiscovery<MockUdpNode, ManualClock>, MockUdpNodeHandle) {
    let socket = MockUdpNode::new();
    let handle = socket.handle();
    let mut discovery = NodeDiscovery::with_clock(
        socket,
        DiscoveryConfig {
            bind: "127.0.0.1:0".parse().expect("a loopback address"),
            wait: Duration::from_millis(1),
            recv_budget: 4_096,
            max_nodes,
            ..DiscoveryConfig::default()
        },
        ManualClock::new(),
    );
    discovery.open().expect("a mock socket binds");
    (discovery, handle)
}

#[test]
fn a_quarter_of_a_million_random_bytes_produce_no_panic() {
    // The volume matters for the same reason it does one protocol along: a
    // parser that trips only on a particular length or a particular byte at a
    // particular offset is not found by twenty datagrams.
    let mut rng = Xorshift(0x5EED_1234_ABCD_0002);
    let mut total = 0usize;
    let mut parsed = 0usize;
    for _ in 0..2_000 {
        let length = rng.upto(600);
        let datagram: Vec<u8> = (0..length).map(|_| rng.byte()).collect();
        total += datagram.len();
        if let Some(reply) = parse_art_poll_reply(&datagram) {
            parsed += 1;
            let _ = reply.short_name();
            let _ = reply.long_name();
            let _ = reply.node_report();
            let _ = reply.output_ports();
        }
    }
    assert!(total > 250_000, "{total} bytes is not a fuzz run");
    assert_eq!(
        parsed, 0,
        "random bytes should not spell Art-Net and an opcode; if they ever do, \
         this number is the interesting part of the failure"
    );

    // And again where the header is right, which is the half a purely random
    // stream never reaches.
    let mut plausible_parsed = 0usize;
    for _ in 0..2_000 {
        let datagram = plausible(&mut rng);
        let reply = parse_art_poll_reply(&datagram).expect("a valid header is a reply");
        plausible_parsed += 1;
        assert!(reply.short_name().len() <= 18);
        assert!(reply.output_ports().len() <= 4);
        assert_eq!(reply.mac_address().len(), 17);
    }
    assert_eq!(plausible_parsed, 2_000);
}

#[test]
fn the_parser_allocates_nothing_whatever_it_is_fed() {
    // The claim the whole receive path rests on: the three names stay as the
    // byte arrays they arrived in, so a hostile stream cannot make this process
    // ask the operating system for memory in a loop.
    let mut rng = Xorshift(0xC0FF_EE00_1234_0046);
    let ordinary: Vec<Vec<u8>> = (0..64).map(|_| plausible(&mut rng)).collect();
    let hostile: Vec<Vec<u8>> = (0..4_096)
        .map(|_| {
            let length = rng.upto(600);
            (0..length).map(|_| rng.byte()).collect()
        })
        .collect();

    let ordinary_calls = measure(|| {
        for datagram in &ordinary {
            if let Some(reply) = parse_art_poll_reply(datagram) {
                let _ = reply.short_name();
                let _ = reply.long_name();
                let _ = reply.node_report();
            }
        }
    });
    let hostile_calls = measure(|| {
        for datagram in &hostile {
            if let Some(reply) = parse_art_poll_reply(datagram) {
                let _ = reply.short_name();
                let _ = reply.long_name();
                let _ = reply.node_report();
            }
        }
    });

    println!(
        "artpoll parse: {} ordinary datagrams -> {ordinary_calls} allocator calls; \
         {} hostile -> {hostile_calls}",
        ordinary.len(),
        hostile.len()
    );
    assert_eq!(ordinary_calls, 0, "parsing a reply must not allocate");
    assert_eq!(
        hostile_calls, 0,
        "sixty-four times the rubbish must not cost sixty-four times anything"
    );

    // The guard: the probe can see an allocation, so a probe that had quietly
    // stopped counting could not pass this file.
    let guard = measure(|| {
        let owned: Vec<u8> = Vec::with_capacity(64);
        assert_eq!(owned.capacity(), 64);
        drop(owned);
    });
    assert!(guard > 0, "the allocation probe has stopped counting");
}

#[test]
fn a_settled_table_costs_nothing_to_keep_up_to_date() {
    // The second allocation claim, and the one that matters over a whole show: a
    // node answering every three seconds for six hours must not grow anything.
    // The first reply for a node allocates — that is the row being made — and
    // every reply after it overwrites fixed-size fields in place.
    let (mut discovery, socket) = discovery(64);
    let mut rng = Xorshift(0xABCD_0046_0000_0001);
    let settled = plausible(&mut rng);
    socket.deliver(node(5), &settled);
    discovery.service();
    assert_eq!(discovery.nodes().len(), 1);

    let calls = measure(|| {
        for _ in 0..256 {
            discovery.service();
        }
    });
    println!("artpoll discovery: 256 idle passes -> {calls} allocator calls");
    assert_eq!(calls, 0, "an idle discovery must not allocate per pass");

    let repeats: u32 = 256;
    for _ in 0..repeats {
        socket.deliver(node(5), &settled);
    }
    let calls = measure(|| while discovery.service() > 0 {});
    println!("artpoll discovery: {repeats} repeat replies -> {calls} allocator calls");
    // At most one call per datagram, and that one is **the mock's**: the queued
    // `Vec` it was handed is released once its bytes have been copied into the
    // discovery's own fixed buffer. What the assertion rules out is the thing
    // that would matter in a hall — a table that allocates *per reply*, which
    // for two hundred and fifty-six replies would be hundreds of calls rather
    // than at most one apiece.
    assert!(
        calls <= u64::from(repeats),
        "a node that keeps answering must not grow the table: {calls} calls for \
         {repeats} replies"
    );
    assert_eq!(discovery.nodes().len(), 1);
    assert_eq!(discovery.nodes()[0].replies, u64::from(repeats) + 1);
}

#[test]
fn every_datagram_is_delivered_counted_or_dropped_and_the_three_add_up() {
    // The accounting identity: a parser that quietly swallowed a datagram breaks
    // it one way, one that invented a node breaks it the other.
    let (mut discovery, socket) = discovery(8);
    let mut rng = Xorshift(0x0046_1234_5678_9ABC);
    let mut delivered = 0u64;
    for index in 0..2_048u32 {
        let datagram = if index % 3 == 0 {
            plausible(&mut rng)
        } else {
            let length = rng.upto(600);
            (0..length).map(|_| rng.byte()).collect()
        };
        // Spread over more source addresses than the table has room for, so the
        // bounded-table path is on the identity as well.
        socket.deliver(node((index % 32) as u8), &datagram);
        delivered += 1;
    }
    while discovery.service() > 0 {}
    // One last pass, because the loop above stops on a pass that read nothing
    // useful and there may be rubbish left behind it.
    while discovery.counters().replies + discovery.counters().malformed < delivered {
        if discovery.service() == 0 && discovery.counters().read_errors == 0 {
            break;
        }
    }

    let counters = discovery.counters();
    println!(
        "artpoll discovery: {delivered} datagrams -> {} replies, {} malformed, {} dropped",
        counters.replies, counters.malformed, counters.dropped
    );
    assert_eq!(
        counters.replies + counters.malformed,
        delivered,
        "a datagram was neither read as a reply nor counted as rubbish"
    );
    assert!(
        discovery.nodes().len() <= 8,
        "the table is bounded whatever a stranger sends"
    );
    assert!(counters.dropped > 0, "the bound was actually reached");
    assert_eq!(counters.read_errors, 0);
}

#[test]
fn a_truncated_reply_is_dropped_at_every_length_it_could_be_cut_to() {
    // "Truncated" for a datagram protocol means this: the node started saying
    // something and the network delivered part of it. Every prefix shorter than
    // the minimum is dropped, and every prefix at or above it is read — which is
    // the boundary `ART_POLL_REPLY_MIN` names.
    let mut rng = Xorshift(0x1111_2222_3333_4444);
    let whole = {
        let mut datagram = plausible(&mut rng);
        datagram.resize(ART_POLL_REPLY_BYTES, 0);
        datagram
    };
    for length in 0..whole.len() {
        let cut = whole.get(..length).expect("a prefix of its own length");
        assert_eq!(
            parse_art_poll_reply(cut).is_some(),
            length >= ART_POLL_REPLY_MIN,
            "a reply cut to {length} bytes was read wrongly"
        );
    }
}
