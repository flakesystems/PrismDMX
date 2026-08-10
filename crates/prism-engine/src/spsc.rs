//! The bounded, lock-free queue that carries commands into the tick.
//!
//! Single producer (the MIDI thread, or the daemon's core thread), single
//! consumer (the tick). Both `push` and `pop` are a bounded number of atomic
//! operations with no retry loop, so both are wait-free, and neither allocates:
//! the ring is sized once, at construction.
//!
//! # Why the payload is a fixed-size byte array
//!
//! Sharing a slot between two threads needs interior mutability, and
//! `unsafe_code` is denied workspace-wide, so the slots are [`AtomicU8`] arrays
//! and anything travelling through them has to encode into a fixed number of
//! bytes. That is a constraint worth having rather than one to work around:
//! `prism_domain::Command` owns `String`s and `Vec`s, and draining one of those
//! in the tick would *free* memory there — an allocator call, which
//! `ARCHITECTURE_SPEC.md` §3.1 forbids just as firmly as it forbids allocating.
//! The daemon translates a `Command` into a flat [`crate::TickCommand`] on the
//! core thread; only the flat form crosses into the engine.

use core::marker::PhantomData;

use crate::sync::{Arc, AtomicU8, AtomicUsize, Ordering};

/// Bytes one queued value may occupy.
///
/// Sixteen is comfortably more than [`crate::TickCommand`] needs today; the
/// headroom is for the variants S3-S5 add. A slot costs this much whatever it
/// holds, and a 1024-slot queue is 16 KB, so there is no reason to be stingy.
pub const PAYLOAD_BYTES: usize = 16;

/// A value that can travel into the tick.
///
/// Fixed size, no owned fields, no destructor: encoding is what keeps the
/// allocator out of the tick. See the module documentation for why that matters
/// more than the ergonomics cost.
pub trait TickPayload: Copy {
    /// Writes `self` into `out`. Unused bytes are already zero.
    fn encode(self, out: &mut [u8; PAYLOAD_BYTES]);

    /// Reads a value back. `None` for bytes that do not describe one, which the
    /// consumer counts and discards rather than acting on.
    fn decode(bytes: &[u8; PAYLOAD_BYTES]) -> Option<Self>
    where
        Self: Sized;
}

/// The shared ring. `head` and `tail` only ever increase; the slot index is the
/// counter taken modulo the capacity, so "full" and "empty" cannot be confused.
struct Ring {
    bytes: Box<[AtomicU8]>,
    capacity: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
    rejected: AtomicUsize,
}

impl Ring {
    fn slot(&self, counter: usize) -> Option<&[AtomicU8]> {
        self.bytes
            .chunks_exact(PAYLOAD_BYTES)
            .nth(counter % self.capacity)
    }
}

/// The producing end. Exactly one thread may hold it.
pub struct Producer<T> {
    ring: Arc<Ring>,
    payload: PhantomData<fn(T)>,
}

/// The consuming end — the tick. Exactly one thread may hold it.
pub struct Consumer<T> {
    ring: Arc<Ring>,
    payload: PhantomData<fn() -> T>,
}

/// Creates a bounded queue. A capacity of zero is rounded up to one.
#[must_use]
pub fn command_queue<T: TickPayload>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let capacity = capacity.max(1);
    let bytes = (0..capacity * PAYLOAD_BYTES)
        .map(|_| AtomicU8::new(0))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let ring = Arc::new(Ring {
        bytes,
        capacity,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
        rejected: AtomicUsize::new(0),
    });
    (
        Producer {
            ring: Arc::clone(&ring),
            payload: PhantomData,
        },
        Consumer {
            ring,
            payload: PhantomData,
        },
    )
}

impl<T: TickPayload> Producer<T> {
    /// Queues a value.
    ///
    /// # Errors
    ///
    /// Hands the value back when the queue is full. The producer is never the
    /// tick, so it is free to decide what that means — drop the command, count
    /// the overrun, or retry.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        let tail = self.ring.tail.load(Ordering::Relaxed);
        let head = self.ring.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) >= self.ring.capacity {
            return Err(value);
        }
        let Some(slot) = self.ring.slot(tail) else {
            return Err(value);
        };
        let mut bytes = [0u8; PAYLOAD_BYTES];
        value.encode(&mut bytes);
        for (cell, byte) in slot.iter().zip(bytes) {
            cell.store(byte, Ordering::Relaxed);
        }
        self.ring
            .tail
            .store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// How many values are waiting to be drained.
    #[must_use]
    pub fn len(&self) -> usize {
        let tail = self.ring.tail.load(Ordering::Relaxed);
        tail.wrapping_sub(self.ring.head.load(Ordering::Acquire))
    }

    /// Whether nothing is waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Slots in the ring.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.ring.capacity
    }
}

impl<T: TickPayload> Consumer<T> {
    /// Reads one slot. Outer `None` means the queue is empty; inner `None` means
    /// the slot did not decode and has been counted as rejected.
    fn take(&mut self) -> Option<Option<T>> {
        let head = self.ring.head.load(Ordering::Relaxed);
        let tail = self.ring.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let mut bytes = [0u8; PAYLOAD_BYTES];
        if let Some(slot) = self.ring.slot(head) {
            for (cell, byte) in slot.iter().zip(bytes.iter_mut()) {
                *byte = cell.load(Ordering::Relaxed);
            }
        }
        self.ring
            .head
            .store(head.wrapping_add(1), Ordering::Release);
        match T::decode(&bytes) {
            Some(value) => Some(Some(value)),
            None => {
                self.ring.rejected.fetch_add(1, Ordering::Relaxed);
                Some(None)
            }
        }
    }

    /// Takes the next decodable value, skipping any that do not decode.
    pub fn pop(&mut self) -> Option<T> {
        loop {
            if let Some(value) = self.take()? {
                return Some(value);
            }
        }
    }

    /// Hands every value that is waiting *now* to `visit`, and returns how many
    /// there were.
    ///
    /// Bounded by the queue's length at entry, so a producer that keeps pushing
    /// cannot hold the tick inside this call.
    pub fn drain(&mut self, mut visit: impl FnMut(T)) -> usize {
        let pending = self.len();
        let mut delivered = 0;
        for _ in 0..pending {
            match self.take() {
                Some(Some(value)) => {
                    visit(value);
                    delivered += 1;
                }
                Some(None) => {}
                None => break,
            }
        }
        delivered
    }

    /// How many values are waiting.
    #[must_use]
    pub fn len(&self) -> usize {
        let tail = self.ring.tail.load(Ordering::Acquire);
        tail.wrapping_sub(self.ring.head.load(Ordering::Relaxed))
    }

    /// Whether nothing is waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Slots in the ring.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.ring.capacity
    }

    /// How many slots have failed to decode over the queue's lifetime. A number
    /// above zero means a bug on the producing side; the tick's response is to
    /// keep running and let it be seen in telemetry.
    #[must_use]
    pub fn rejected(&self) -> usize {
        self.ring.rejected.load(Ordering::Relaxed)
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::{PAYLOAD_BYTES, TickPayload, command_queue};

    /// A payload with no meaning beyond being distinguishable.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Marker(u32);

    impl TickPayload for Marker {
        fn encode(self, out: &mut [u8; PAYLOAD_BYTES]) {
            out[..4].copy_from_slice(&self.0.to_le_bytes());
        }

        fn decode(bytes: &[u8; PAYLOAD_BYTES]) -> Option<Self> {
            let mut raw = [0u8; 4];
            raw.copy_from_slice(&bytes[..4]);
            let value = u32::from_le_bytes(raw);
            (value != u32::MAX).then_some(Self(value))
        }
    }

    #[test]
    fn a_queue_delivers_in_order() {
        let (mut producer, mut consumer) = command_queue::<Marker>(8);
        for n in 0..5 {
            producer.push(Marker(n)).unwrap();
        }
        let drained: Vec<Marker> = std::iter::from_fn(|| consumer.pop()).collect();
        assert_eq!(drained, (0..5).map(Marker).collect::<Vec<_>>());
    }

    #[test]
    fn an_empty_queue_yields_nothing_rather_than_waiting() {
        let (_producer, mut consumer) = command_queue::<Marker>(4);
        assert_eq!(consumer.pop(), None);
        assert_eq!(consumer.len(), 0);
        assert!(consumer.is_empty());
    }

    #[test]
    fn a_full_queue_rejects_and_hands_the_value_back() {
        let (mut producer, mut consumer) = command_queue::<Marker>(2);
        producer.push(Marker(1)).unwrap();
        producer.push(Marker(2)).unwrap();
        // The producer is not the tick, so refusing is the safe answer: the
        // caller decides whether to drop the command or report the overrun.
        assert_eq!(producer.push(Marker(3)), Err(Marker(3)));
        assert_eq!(producer.len(), 2);
        assert_eq!(consumer.pop(), Some(Marker(1)));
        producer.push(Marker(3)).unwrap();
        assert_eq!(consumer.pop(), Some(Marker(2)));
        assert_eq!(consumer.pop(), Some(Marker(3)));
    }

    #[test]
    fn indices_wrap_without_losing_a_slot() {
        let (mut producer, mut consumer) = command_queue::<Marker>(3);
        for n in 0..100 {
            producer.push(Marker(n)).unwrap();
            assert_eq!(consumer.pop(), Some(Marker(n)));
        }
        assert!(consumer.is_empty());
    }

    #[test]
    fn draining_visits_every_pending_value_once() {
        let (mut producer, mut consumer) = command_queue::<Marker>(8);
        for n in 0..4 {
            producer.push(Marker(n)).unwrap();
        }
        let mut seen = Vec::new();
        let count = consumer.drain(|value| seen.push(value));
        assert_eq!(count, 4);
        assert_eq!(seen, (0..4).map(Marker).collect::<Vec<_>>());
        assert_eq!(consumer.drain(|_| unreachable!()), 0);
    }

    #[test]
    fn a_payload_that_cannot_be_decoded_is_dropped_not_panicked_on() {
        // Corruption here means a bug, but the tick's answer to a bug is to
        // keep emitting frames.
        let (mut producer, mut consumer) = command_queue::<Marker>(4);
        producer.push(Marker(u32::MAX)).unwrap();
        producer.push(Marker(1)).unwrap();
        let mut seen = Vec::new();
        assert_eq!(consumer.drain(|value| seen.push(value)), 1);
        assert_eq!(seen, [Marker(1)]);
        assert_eq!(consumer.rejected(), 1);
    }

    #[test]
    fn both_ends_agree_on_how_much_is_waiting() {
        let (mut producer, mut consumer) = command_queue::<Marker>(4);
        assert!(producer.is_empty());
        assert!(consumer.is_empty());
        producer.push(Marker(1)).unwrap();
        producer.push(Marker(2)).unwrap();
        assert_eq!(producer.len(), 2);
        assert_eq!(consumer.len(), 2);
        assert!(!producer.is_empty());
        assert!(!consumer.is_empty());
        assert_eq!(consumer.rejected(), 0);
        consumer.pop();
        assert_eq!(producer.len(), 1);
        assert_eq!(consumer.len(), 1);
    }

    #[test]
    fn capacity_is_what_was_asked_for() {
        let (producer, consumer) = command_queue::<Marker>(5);
        assert_eq!(producer.capacity(), 5);
        assert_eq!(consumer.capacity(), 5);
    }

    #[test]
    fn a_zero_capacity_queue_is_rounded_up_to_one() {
        let (mut producer, mut consumer) = command_queue::<Marker>(0);
        assert_eq!(producer.capacity(), 1);
        producer.push(Marker(1)).unwrap();
        assert_eq!(producer.push(Marker(2)), Err(Marker(2)));
        assert_eq!(consumer.pop(), Some(Marker(1)));
    }
}

#[cfg(all(loom, test))]
mod loom_tests {
    use super::{PAYLOAD_BYTES, TickPayload, command_queue};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Marker(u8);

    impl TickPayload for Marker {
        fn encode(self, out: &mut [u8; PAYLOAD_BYTES]) {
            out[0] = self.0;
        }

        fn decode(bytes: &[u8; PAYLOAD_BYTES]) -> Option<Self> {
            Some(Self(bytes[0]))
        }
    }

    /// The producer and the consumer run on different threads and share only the
    /// two index words. Whatever order their operations interleave in, the
    /// consumer must see values in order and never a slot it does not own.
    #[test]
    fn values_cross_the_thread_boundary_in_order() {
        loom::model(|| {
            let (mut producer, mut consumer) = command_queue::<Marker>(2);

            let sender = loom::thread::spawn(move || {
                for n in 1..=2u8 {
                    while producer.push(Marker(n)).is_err() {
                        loom::thread::yield_now();
                    }
                }
            });

            let receiver = loom::thread::spawn(move || {
                let mut expected = 1u8;
                while expected <= 2 {
                    if let Some(Marker(value)) = consumer.pop() {
                        assert_eq!(value, expected);
                        expected += 1;
                    } else {
                        loom::thread::yield_now();
                    }
                }
            });

            sender.join().unwrap();
            receiver.join().unwrap();
        });
    }
}
