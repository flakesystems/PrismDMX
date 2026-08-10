//! Concurrent read/write stress on the frame hand-off.
//!
//! The `loom` model in `src/triple_buffer.rs` proves the ownership protocol is
//! correct for every legal interleaving of a two-frame exchange; it cannot run a
//! realistic frame, because the state space of 512 channels is unbounded. This
//! target is the other half: real threads, real frame sizes, millions of
//! exchanges, on whatever memory model the machine actually has.
//!
//! Tearing is detectable because every channel of a frame carries the same value
//! and that value is derived from the frame's own sequence number. A frame made
//! of two publishes therefore fails the check no matter where the seam falls.

#![cfg(not(loom))]
#![allow(
    clippy::print_stdout,
    reason = "a measured criterion has to print the number it measured"
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use prism_domain::UniverseId;
use prism_engine::{FrameLayout, FramePublisher, FrameSubscriber};

/// The value every channel of frame `sequence` carries. Coprime with the frame
/// size so a shifted or partially copied frame cannot alias a valid one.
fn pattern(sequence: u64) -> u8 {
    (sequence % 251) as u8
}

/// What one reader saw.
struct Seen {
    frames: u64,
    last_sequence: u64,
}

/// Reads until it has seen `target`, checking every frame it is given.
///
/// The last published frame stays marked fresh until it is read, so this always
/// terminates once the writer has finished.
fn read_until(mut subscriber: FrameSubscriber, target: u64) -> Seen {
    let mut frames = 0;
    let mut last_sequence = 0;
    while last_sequence < target {
        if subscriber.refresh() {
            let frame = subscriber.frame();
            let sequence = frame.sequence();
            assert!(
                sequence > last_sequence,
                "sequence went backwards: {last_sequence} then {sequence}"
            );
            let expected = pattern(sequence);
            if let Some(position) = frame.channels().iter().position(|&byte| byte != expected) {
                panic!(
                    "torn frame {sequence}: channel {position} was {} not {expected}",
                    frame.channels().get(position).copied().unwrap_or_default()
                );
            }
            last_sequence = sequence;
            frames += 1;
        } else {
            std::hint::spin_loop();
        }
    }
    Seen {
        frames,
        last_sequence,
    }
}

/// Publishes `frames` stamped frames to `readers` threads and checks all of them.
fn stress(universes: u32, readers: usize, frames: u64) -> Vec<Seen> {
    let layout = Arc::new(FrameLayout::new((1..=universes).map(UniverseId::new)).unwrap());
    let mut publisher = FramePublisher::new(layout);
    let subscribers: Vec<FrameSubscriber> = (0..readers).map(|_| publisher.subscribe()).collect();

    thread::scope(|scope| {
        let handles: Vec<_> = subscribers
            .into_iter()
            .map(|subscriber| scope.spawn(move || read_until(subscriber, frames)))
            .collect();

        for sequence in 1..=frames {
            publisher.frame_mut().fill(pattern(sequence));
            publisher.publish();
        }

        handles
            .into_iter()
            .map(|handle| handle.join().expect("reader thread failed"))
            .collect()
    })
}

#[test]
fn no_reader_ever_sees_a_frame_made_of_two_publishes() {
    let frames = 50_000;
    let seen = stress(2, 4, frames);
    for reader in &seen {
        assert_eq!(reader.last_sequence, frames);
        assert!(reader.frames <= frames);
        assert!(reader.frames > 0);
    }
    let received: Vec<u64> = seen.iter().map(|reader| reader.frames).collect();
    println!("50 000 frames, 2 universes, 4 readers - frames taken: {received:?}");
}

#[test]
fn a_reader_that_cannot_keep_up_loses_frames_rather_than_holding_the_writer_back() {
    // The writer publishes as fast as it can while one reader deliberately
    // dawdles. The point is not the exact count but that the slow reader ends
    // up with far fewer frames than were published and the run still finishes:
    // that is a driver falling behind, which ARCHITECTURE_SPEC.md 3.2 requires.
    let frames = 20_000;
    let layout = Arc::new(FrameLayout::new([UniverseId::new(1)]).unwrap());
    let mut publisher = FramePublisher::new(layout);
    let subscriber = publisher.subscribe();

    let done = AtomicBool::new(false);
    let (taken, elapsed) = thread::scope(|scope| {
        let finished = &done;
        let reader = scope.spawn(move || {
            let mut subscriber = subscriber;
            let mut taken = 0;
            let mut last = 0;
            let mut check = |subscriber: &FrameSubscriber| {
                let frame = subscriber.frame();
                assert!(frame.sequence() > last);
                last = frame.sequence();
                let expected = pattern(last);
                assert!(frame.channels().iter().all(|&byte| byte == expected));
                taken += 1;
            };
            loop {
                if subscriber.refresh() {
                    check(&subscriber);
                    // Deliberately slow: give the writer every chance to lap us.
                    thread::yield_now();
                    continue;
                }
                // `done` is released after the last publish, so once it is
                // visible one more refresh is all that can be outstanding.
                if finished.load(Ordering::Acquire) {
                    if subscriber.refresh() {
                        check(&subscriber);
                    }
                    break;
                }
                thread::yield_now();
            }
            taken
        });

        let started = Instant::now();
        for sequence in 1..=frames {
            publisher.frame_mut().fill(pattern(sequence));
            publisher.publish();
        }
        let elapsed = started.elapsed();
        done.store(true, Ordering::Release);
        (reader.join().expect("reader thread failed"), elapsed)
    });

    println!("writer published {frames} frames in {elapsed:?}, slow reader took {taken}");
    assert!(taken > 0);
    assert!(taken <= frames);
    // Nowhere near a tick period per frame: the writer was never blocked.
    assert!(
        elapsed < Duration::from_secs(10),
        "publishing was held up by the reader: {elapsed:?}"
    );
}

#[test]
#[ignore = "long-running: the full-size stress run, see the crate documentation"]
fn a_full_size_frame_survives_millions_of_exchanges() {
    // 64 universes is the design target from CLAUDE.md, and a full frame is
    // 32 KB - large enough that a torn copy has somewhere to hide.
    let frames = 1_000_000;
    let started = Instant::now();
    let seen = stress(64, 4, frames);
    let elapsed = started.elapsed();
    for reader in &seen {
        assert_eq!(reader.last_sequence, frames);
    }
    let received: Vec<u64> = seen.iter().map(|reader| reader.frames).collect();
    println!("{frames} frames of 64 universes to 4 readers in {elapsed:?}, taken: {received:?}");
}
