//! Proof that layer 2 never calls the allocator either.
//!
//! S19 measured this of the codec and `docs/MCU_MAPPING.md` §2.4 required it.
//! Nothing in §5 requires it of the shadow model — and that is exactly why it is
//! worth measuring, because the obvious implementation of a diff is a `Vec` of
//! messages built every frame, thirty times a second, on the thread next to a
//! 44 Hz tick.
//!
//! The two pictures and the frame's queue are fixed-size arrays inside
//! [`SurfaceController`], so the whole outbound path is arithmetic over memory
//! that already exists. This file is what says so, the way S16's finding says it
//! has to be said: **a functionally correct implementation can be the wrong
//! one, and only the counter sees the difference.**

#![allow(
    clippy::print_stdout,
    reason = "a measured criterion has to print the number it measured"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Duration;

use prism_domain::RgbColor;
use prism_surface::{
    ButtonId, DisplayLine, Fader, GlobalButton, LedState, MeterSignal, RingMode, StripButton,
    SurfaceController, SurfaceEvent, X_TOUCH,
};

/// Counts allocator calls made by whichever thread has armed the probe.
struct CountingAllocator;

thread_local! {
    /// Whether this thread is inside a measured window.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    /// Allocator calls made by this thread while armed.
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

/// A minute of a running show, as the state changes a desk would be given.
fn a_running_show(controller: &mut SurfaceController, step: u16) {
    for strip in 0..8u8 {
        controller.set_fader(Fader::Strip(strip), step.wrapping_mul(211));
        controller.set_meter(strip, MeterSignal::Level((step % 13) as u8));
        controller.set_ring(strip, RingMode::Wrap, (step % 12) as u8);
        controller.set_text(strip, DisplayLine::Lower, "100%");
        controller.set_color_rgb(
            strip,
            RgbColor {
                r: step as u8,
                g: 200,
                b: 12,
            },
        );
        for button in StripButton::ALL {
            controller.set_led(
                ButtonId::Strip { strip, button },
                if step.is_multiple_of(2) {
                    LedState::On
                } else {
                    LedState::Off
                },
            );
        }
    }
    controller.set_fader(Fader::Main, step);
    controller.set_led(
        ButtonId::Global(GlobalButton::Save),
        if step.is_multiple_of(3) {
            LedState::Flashing
        } else {
            LedState::Off
        },
    );
    controller.set_segment_text("PAGE-1");
}

#[test]
fn a_resync_burst_and_a_running_show_cost_no_allocation() {
    // The whole outbound path: connect, draw the entire surface, then change
    // every control on it thirty times a second for a simulated minute.
    let mut controller = SurfaceController::new(X_TOUCH);
    let mut sent = 0u64;
    let mut bytes = 0u64;

    // Warm up outside the window - the first pass through a code path can touch
    // lazily initialised machinery that has nothing to do with this crate.
    controller.connected(Duration::ZERO);
    controller.pump(Duration::from_millis(40), |_| sent += 1);

    let calls = allocator_calls(|| {
        let mut now = Duration::from_millis(41);
        let mut step = 0u16;
        while now < Duration::from_secs(60) {
            if now.subsec_millis().is_multiple_of(33) {
                step = step.wrapping_add(1);
                a_running_show(&mut controller, step);
            }
            controller.pump(now, |feedback| {
                sent += 1;
                // Touch the message so a sink that did nothing cannot be the
                // reason nothing was allocated.
                bytes += u64::from(matches!(
                    feedback,
                    prism_surface::Feedback::DisplayText { .. }
                ));
            });
            now = now.saturating_add(Duration::from_millis(1));
        }
    });
    println!("prism-surface: {sent} messages ({bytes} of them text), {calls} allocator calls");
    assert!(sent > 5_000, "the fixture is not a running show: {sent}");
    assert_eq!(calls, 0, "the outbound path allocated {calls} times");
}

#[test]
fn interpreting_a_busy_desk_costs_no_allocation() {
    // The inbound path: bytes through layer 1, then the scaling, the two
    // acceleration curves and the touch bookkeeping this layer adds.
    let mut stream = Vec::new();
    for step in 0..200u8 {
        stream.extend_from_slice(&[0x90, 104, 0x7F]);
        stream.extend_from_slice(&[0xE0, step & 0x7C, step & 0x7F]);
        stream.extend_from_slice(&[0x90, 104, 0x00]);
        stream.extend_from_slice(&[0xB0, 16, step & 0x0F]);
        stream.extend_from_slice(&[0xB0, 60, 0x01]);
        stream.extend_from_slice(&[0x90, 24, 0x7F, 0x90, 24, 0x00]);
        // The reserved button, which is dropped and counted rather than passed
        // on, and a message no profile describes.
        stream.extend_from_slice(&[0x90, 53, 0x7F, 0x90, 120, 0x7F]);
    }

    let mut controller = SurfaceController::new(X_TOUCH);
    controller.connected(Duration::ZERO);
    let mut events = 0u64;
    let mut steps = 0i64;
    controller.push(&stream, Duration::from_millis(1), |_| events += 1);
    let warm = events;
    events = 0;

    let calls = allocator_calls(|| {
        for round in 2..22u64 {
            controller.push(&stream, Duration::from_millis(round), |event| {
                events += 1;
                if let SurfaceEvent::Encoder { steps: turned, .. }
                | SurfaceEvent::Jog { steps: turned } = event
                {
                    steps += i64::from(turned);
                }
            });
        }
    });
    println!("prism-surface: {events} events, {steps} parameter steps, {calls} allocator calls");
    assert!(warm > 1_000, "the fixture is not a busy desk: {warm}");
    assert_eq!(events, warm * 20);
    assert_eq!(controller.counters().reserved, 200 * 21);
    assert_eq!(calls, 0, "the inbound path allocated {calls} times");
}

#[test]
fn losing_and_finding_the_surface_costs_no_allocation() {
    // Reconnecting throws the whole shadow model away and rebuilds it, which is
    // the one place a `Vec` would be easiest to reach for.
    let mut controller = SurfaceController::new(X_TOUCH);
    controller.connected(Duration::ZERO);
    controller.pump(Duration::from_millis(40), |_| {});

    let mut sent = 0u64;
    let calls = allocator_calls(|| {
        let mut now = Duration::from_millis(41);
        for _ in 0..20 {
            controller.disconnected();
            now = now.saturating_add(Duration::from_millis(100));
            controller.connected(now);
            for _ in 0..200 {
                controller.pump(now, |_| sent += 1);
                now = now.saturating_add(Duration::from_millis(1));
            }
        }
    });
    println!("prism-surface: 20 reconnections, {sent} messages, {calls} allocator calls");
    assert_eq!(
        sent,
        20 * 156,
        "each reconnection redraws the whole surface"
    );
    assert_eq!(calls, 0, "reconnecting allocated {calls} times");
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
