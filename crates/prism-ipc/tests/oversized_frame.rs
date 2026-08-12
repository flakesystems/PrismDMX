//! Proof that refusing an oversized frame costs no large allocation.
//!
//! `IMPLEMENTATION_PLAN.md` S16, second exit criterion, and
//! `docs/IPC_PROTOCOL.md` §3: *an oversized frame closes the connection rather
//! than allocating*.
//!
//! # Why this needs an allocator and not an assertion
//!
//! The functional half — the connection does close, with the right error — is
//! `transport::stream`'s own test, and it passes for both the correct
//! implementation and the wrong one. The wrong one reads the length, calls
//! `vec![0; length]`, and *then* notices the length is absurd; it returns exactly
//! the same error, and it has just asked the operating system for four gigabytes
//! in a process that is holding the DMX output up.
//!
//! So the claim is about memory, and it is measured the way `prism-engine`
//! measures the tick: an allocator that counts what the thread under test asked
//! for. What matters here is not the number of calls but the **size** of the
//! largest one, so the probe records that too.
//!
//! # The measurement is a comparison
//!
//! An absolute number would be a claim about the runtime's own bookkeeping as
//! much as about this crate's. So the same probe runs twice: once refusing a
//! frame that announces four gigabytes, and once reading a legitimate frame of
//! exactly [`prism_ipc::MAX_FRAME_BYTES`]. The second **must** allocate a
//! megabyte — that is what reading a megabyte means — and the first must not
//! come anywhere near it. A guard test at the end proves the probe can see a
//! large allocation at all, so a probe that had quietly stopped counting could
//! not pass this file.

#![allow(
    clippy::print_stdout,
    reason = "a measured criterion has to print the number it measured"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use prism_ipc::transport::stream;
use prism_ipc::{FrameError, MAX_FRAME_BYTES, WireError};
use tokio::io::AsyncWriteExt;

/// Records what the arming thread asks the allocator for.
struct CountingAllocator;

thread_local! {
    /// Whether this thread is inside a measured window. `const` initialised, so
    /// touching it cannot itself allocate.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    /// Allocator calls made while armed.
    static CALLS: Cell<u64> = const { Cell::new(0) };
    /// Bytes asked for while armed.
    static BYTES: Cell<u64> = const { Cell::new(0) };
    /// The largest single request made while armed. This is the number the
    /// criterion is about.
    static PEAK: Cell<usize> = const { Cell::new(0) };
}

fn record(size: usize) {
    // `try_with`, not `with`: during thread teardown the local is gone, and a
    // panic from inside the allocator would be unrecoverable.
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = CALLS.try_with(|calls| calls.set(calls.get() + 1));
            let _ = BYTES.try_with(|bytes| bytes.set(bytes.get() + size as u64));
            let _ = PEAK.try_with(|peak| peak.set(peak.get().max(size)));
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
        record(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record(0);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// What one measured window cost.
#[derive(Debug, Clone, Copy)]
struct Probe {
    calls: u64,
    bytes: u64,
    peak: usize,
}

fn arm() {
    CALLS.with(|calls| calls.set(0));
    BYTES.with(|bytes| bytes.set(0));
    PEAK.with(|peak| peak.set(0));
    ARMED.with(|armed| armed.set(true));
}

fn disarm() -> Probe {
    ARMED.with(|armed| armed.set(false));
    Probe {
        calls: CALLS.with(Cell::get),
        bytes: BYTES.with(Cell::get),
        peak: PEAK.with(Cell::get),
    }
}

/// Reads one frame off a stream that has already been written to, measuring what
/// the reader asks the allocator for.
///
/// The runtime is the default `#[tokio::test]` one — current thread — so the
/// reading task runs on the thread that armed the probe. On a worker thread the
/// probe would see nothing and the test would pass by measuring nothing at all,
/// which is what `the_probe_can_see_a_large_allocation` exists to rule out.
async fn read_one(
    prepared: &[u8],
    duplex_capacity: usize,
) -> (Probe, Option<Result<Vec<u8>, WireError>>) {
    let (here, mut there) = tokio::io::duplex(duplex_capacity);
    let mut wire = stream::spawn(here);

    // Written before the window opens: what the *writer* costs is not the
    // question.
    there
        .write_all(prepared)
        .await
        .expect("the duplex was sized to hold this");

    arm();
    let received = wire.recv().await;
    let probe = disarm();
    (probe, received)
}

/// The criterion.
#[tokio::test]
async fn refusing_an_oversized_frame_costs_no_large_allocation() {
    // A header announcing four gigabytes, and nothing behind it.
    let (refused, received) = read_one(&u32::MAX.to_le_bytes(), 64 * 1024).await;
    assert_eq!(
        received,
        Some(Err(WireError::Frame(FrameError::TooLarge {
            announced: u64::from(u32::MAX),
            limit: MAX_FRAME_BYTES,
        }))),
        "the connection must be refused, not merely survived"
    );

    // The same reader, reading a frame that really is a megabyte.
    let mut legitimate = (MAX_FRAME_BYTES as u32).to_le_bytes().to_vec();
    legitimate.extend_from_slice(&vec![7_u8; MAX_FRAME_BYTES]);
    let (accepted, received) = read_one(&legitimate, 4 * MAX_FRAME_BYTES).await;
    assert_eq!(
        received.map(|result| result.map(|payload| payload.len())),
        Some(Ok(MAX_FRAME_BYTES)),
        "a frame at the limit is legal and must arrive"
    );

    println!(
        "oversized frame: {} calls, {} bytes, largest single request {} bytes",
        refused.calls, refused.bytes, refused.peak
    );
    println!(
        "one-megabyte frame: {} calls, {} bytes, largest single request {} bytes",
        accepted.calls, accepted.bytes, accepted.peak
    );

    // The comparison: reading a megabyte costs a megabyte, and refusing four
    // gigabytes costs nothing of the kind.
    assert!(
        accepted.peak >= MAX_FRAME_BYTES,
        "reading a megabyte should have allocated one; the probe measured {} bytes, \
         which means it is not measuring the reader",
        accepted.peak
    );

    // A generous ceiling, because the point is the order of magnitude: four
    // gigabytes announced, and not one kilobyte of it believed. The wrong
    // implementation would show a peak of 4 294 967 295 here.
    const CEILING: usize = 4096;
    assert!(
        refused.peak <= CEILING,
        "refusing an oversized frame asked for {} bytes in a single allocation, \
         which means the length was believed before it was checked",
        refused.peak
    );
    assert!(
        refused.bytes <= CEILING as u64,
        "refusing an oversized frame cost {} bytes in total",
        refused.bytes
    );
}

/// A probe that cannot see a large allocation would pass the test above by
/// measuring nothing. This is the guard against that, and it is the reason the
/// runtime above has to be the current-thread one.
#[tokio::test]
async fn the_probe_can_see_a_large_allocation() {
    arm();
    let deliberate = vec![0_u8; 4 * MAX_FRAME_BYTES];
    let probe = disarm();
    assert_eq!(deliberate.len(), 4 * MAX_FRAME_BYTES);

    assert!(
        probe.peak >= 4 * MAX_FRAME_BYTES,
        "the probe saw {} bytes for a four-megabyte allocation",
        probe.peak
    );
    assert!(probe.calls >= 1);
    println!(
        "guard: a four-megabyte allocation showed as {} bytes in {} calls",
        probe.peak, probe.calls
    );
}

/// And the other half of the guard: an armed window with nothing in it must not
/// report a large allocation, or the ceiling above would be meaningless.
#[test]
fn an_empty_window_costs_nothing() {
    arm();
    let probe = disarm();
    assert_eq!(probe.peak, 0);
    assert_eq!(probe.bytes, 0);
}
