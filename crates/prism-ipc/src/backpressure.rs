//! What happens when a client cannot keep up — `docs/IPC_PROTOCOL.md` §8.
//!
//! > Telemetry is coalesced then dropped. Control messages are **never**
//! > dropped; if the control queue for a client fills, that client is
//! > disconnected with a `Reject` and must reconnect and re-snapshot.
//!
//! That is a policy, not a daemon feature, which is why it lives here: S18
//! measures it as a gate, S17 only has to hold one [`Outbound`] per connection
//! and obey what it says.
//!
//! # Why it is a plain state machine
//!
//! An obvious implementation is two `tokio::mpsc` channels with different
//! capacities. It would work, and it would be untestable in the way that
//! matters: the interesting behaviour is *which message is dropped when*, and a
//! channel answers that question by blocking a task somewhere. [`Outbound`] is a
//! struct with no clock, no channel and no task — the whole policy is decided by
//! [`Outbound::push`] and [`Outbound::take`], and every one of §8's sentences is
//! a unit test rather than a race.
//!
//! # The three rules
//!
//! 1. **Control before telemetry, always.** A command's answer never waits
//!    behind a picture of the lights.
//! 2. **Telemetry coalesces to one.** A client that is behind wants the
//!    *current* levels, never a queue of old ones — the same argument as the
//!    engine's triple buffer, one process boundary out.
//! 3. **A full control queue ends the connection.** Not by dropping a delta:
//!    a client that has missed one delta holds a state that silently disagrees
//!    with the daemon's, and every screen it drives is then lying about the
//!    show. Disconnecting is loud, and the reconnect re-snapshots.

use std::collections::VecDeque;

use crate::message::ServerMessage;

/// How many control messages may queue for one client before the connection is
/// given up on.
///
/// Sized against the burst a real client causes rather than against memory: a
/// show load emits one `ShowPatch` per document plus a `Snapshot`, and an
/// operator running a chase produces a few `ExecutorState` deltas per second. A
/// client that is 256 messages behind is not slow, it is gone.
pub const DEFAULT_CONTROL_QUEUE: usize = 256;

/// The control queue for this client is full, and §8 says the connection ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboundFull {
    /// How many control messages were queued when it filled.
    pub queued: usize,
}

impl core::fmt::Display for OutboundFull {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "the client is {} control messages behind and is not reading",
            self.queued
        )
    }
}

impl core::error::Error for OutboundFull {}

/// What this connection has done so far. Read by the status panel (S27) and by
/// S18's gate, which asserts the shape of these numbers rather than eyeballing
/// a log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutboundStats {
    /// Control messages handed to the transport.
    pub control_sent: u64,
    /// Telemetry frames handed to the transport.
    pub telemetry_sent: u64,
    /// Telemetry frames replaced by a newer one before they were sent. This is
    /// the number that says a client is behind, and it is expected to be
    /// non-zero on a busy desk.
    pub telemetry_dropped: u64,
    /// The most control messages ever queued at once.
    pub control_high_water: usize,
}

/// One client's outbound queue.
#[derive(Debug)]
pub struct Outbound {
    control: VecDeque<ServerMessage>,
    limit: usize,
    /// At most one, always the newest — rule 2.
    telemetry: Option<ServerMessage>,
    stats: OutboundStats,
    closed: bool,
}

impl Default for Outbound {
    fn default() -> Self {
        Self::new(DEFAULT_CONTROL_QUEUE)
    }
}

impl Outbound {
    /// A queue holding at most `limit` control messages.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            control: VecDeque::new(),
            limit,
            telemetry: None,
            stats: OutboundStats::default(),
            closed: false,
        }
    }

    /// Queues a message.
    ///
    /// Telemetry replaces whatever telemetry was waiting. Control messages
    /// queue in order.
    ///
    /// # Errors
    ///
    /// [`OutboundFull`] when the control queue is full. The caller's obligation
    /// then is §8's: [`Self::fail`] with a `Reject`, and close the connection.
    pub fn push(&mut self, message: ServerMessage) -> Result<(), OutboundFull> {
        if self.closed {
            // Everything after the terminal message is moot: the client is going
            // away and will re-snapshot. Silently dropping it here is what keeps
            // the caller from having to know that.
            return Ok(());
        }
        if message.is_droppable() {
            if self.telemetry.replace(message).is_some() {
                self.stats.telemetry_dropped += 1;
            }
            return Ok(());
        }
        if self.control.len() >= self.limit {
            return Err(OutboundFull {
                queued: self.control.len(),
            });
        }
        self.control.push_back(message);
        self.stats.control_high_water = self.stats.control_high_water.max(self.control.len());
        Ok(())
    }

    /// Discards everything queued and leaves exactly `message` to be sent.
    ///
    /// The end of a connection. Whatever was queued describes a state the client
    /// is about to stop tracking, so sending it would only delay the one message
    /// that matters. After this, [`Self::push`] accepts and ignores.
    pub fn fail(&mut self, message: ServerMessage) {
        self.control.clear();
        self.telemetry = None;
        self.control.push_back(message);
        self.closed = true;
    }

    /// The next message to write, control first.
    ///
    /// Not called `next`: this is not an iterator, and a queue that could be
    /// consumed by a `for` loop would be one an accident could drain.
    pub fn take(&mut self) -> Option<ServerMessage> {
        if let Some(message) = self.control.pop_front() {
            self.stats.control_sent += 1;
            return Some(message);
        }
        let message = self.telemetry.take()?;
        self.stats.telemetry_sent += 1;
        Some(message)
    }

    /// Whether anything is waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.control.is_empty() && self.telemetry.is_none()
    }

    /// Control messages waiting.
    #[must_use]
    pub fn control_len(&self) -> usize {
        self.control.len()
    }

    /// Whether a terminal message has been queued, so nothing further will be
    /// accepted.
    #[must_use]
    pub const fn is_closing(&self) -> bool {
        self.closed
    }

    /// The counters.
    #[must_use]
    pub const fn stats(&self) -> OutboundStats {
        self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_CONTROL_QUEUE, Outbound, OutboundFull};
    use crate::message::{RejectReason, ServerMessage};
    use prism_domain::Delta;

    fn delta(n: u64) -> ServerMessage {
        ServerMessage::Ack { seq: n }
    }

    fn telemetry(n: u8) -> ServerMessage {
        ServerMessage::Telemetry { data: vec![n] }
    }

    #[test]
    fn control_messages_keep_their_order() {
        let mut out = Outbound::default();
        for n in 0..5 {
            out.push(delta(n)).unwrap();
        }
        for n in 0..5 {
            assert_eq!(out.take(), Some(delta(n)));
        }
        assert_eq!(out.take(), None);
        assert!(out.is_empty());
    }

    /// Rule 1: a command's answer never waits behind a picture of the lights.
    #[test]
    fn control_goes_out_before_telemetry_whatever_order_it_arrived_in() {
        let mut out = Outbound::default();
        out.push(telemetry(1)).unwrap();
        out.push(delta(7)).unwrap();
        assert_eq!(out.take(), Some(delta(7)));
        assert_eq!(out.take(), Some(telemetry(1)));
    }

    /// Rule 2: the client wants the current levels, not a queue of old ones.
    #[test]
    fn telemetry_coalesces_to_the_newest_frame() {
        let mut out = Outbound::default();
        for n in 0..100 {
            out.push(telemetry(n)).unwrap();
        }
        assert_eq!(out.take(), Some(telemetry(99)));
        assert_eq!(out.take(), None);
        assert_eq!(out.stats().telemetry_dropped, 99);
        assert_eq!(out.stats().telemetry_sent, 1);
    }

    /// Rule 3, and the sentence it turns on: commands are **never** dropped.
    #[test]
    fn a_full_control_queue_is_reported_rather_than_making_room() {
        let mut out = Outbound::new(4);
        for n in 0..4 {
            out.push(delta(n)).unwrap();
        }
        assert_eq!(out.push(delta(4)), Err(OutboundFull { queued: 4 }));
        // Nothing was dropped to make room: the four that were accepted are
        // still there, in order, and the fifth was refused rather than the first
        // being discarded.
        for n in 0..4 {
            assert_eq!(out.take(), Some(delta(n)));
        }
        assert_eq!(out.take(), None);
    }

    /// A slow client's telemetry is dropped and its commands are not — §8 in
    /// one test, and the shape S18 measures.
    #[test]
    fn a_client_that_never_reads_loses_telemetry_and_keeps_every_delta() {
        let mut out = Outbound::new(DEFAULT_CONTROL_QUEUE);
        for n in 0..1_000 {
            out.push(telemetry(u8::try_from(n % 256).unwrap())).unwrap();
        }
        for n in 0..DEFAULT_CONTROL_QUEUE as u64 {
            out.push(delta(n)).unwrap();
        }
        assert_eq!(out.stats().telemetry_dropped, 999);

        let mut seen = Vec::new();
        while let Some(message) = out.take() {
            if let ServerMessage::Ack { seq } = message {
                seen.push(seq);
            }
        }
        assert_eq!(seen, (0..DEFAULT_CONTROL_QUEUE as u64).collect::<Vec<_>>());
    }

    #[test]
    fn the_terminal_message_replaces_everything_that_was_waiting() {
        let mut out = Outbound::new(4);
        out.push(delta(1)).unwrap();
        out.push(telemetry(1)).unwrap();
        let reject = ServerMessage::Reject {
            seq: None,
            reason: RejectReason::Backpressure,
            message: "not reading".to_owned(),
        };
        out.fail(reject.clone());

        assert!(out.is_closing());
        assert_eq!(out.take(), Some(reject));
        assert_eq!(out.take(), None);
    }

    #[test]
    fn nothing_queues_behind_a_terminal_message() {
        let mut out = Outbound::new(4);
        out.fail(ServerMessage::Reject {
            seq: None,
            reason: RejectReason::ShuttingDown,
            message: String::new(),
        });
        // Accepted and ignored: the caller should not have to check.
        out.push(delta(1)).unwrap();
        out.push(telemetry(1)).unwrap();
        assert_eq!(out.control_len(), 1);
        assert!(matches!(out.take(), Some(ServerMessage::Reject { .. })));
        assert_eq!(out.take(), None);
    }

    #[test]
    fn the_counters_count_what_they_say_they_count() {
        let mut out = Outbound::new(8);
        assert_eq!(out.stats(), super::OutboundStats::default());

        out.push(delta(1)).unwrap();
        out.push(delta(2)).unwrap();
        out.push(telemetry(1)).unwrap();
        out.push(telemetry(2)).unwrap();
        assert_eq!(out.control_len(), 2);
        assert_eq!(out.stats().control_high_water, 2);
        assert_eq!(out.stats().telemetry_dropped, 1);
        assert_eq!(out.stats().control_sent, 0);

        while out.take().is_some() {}
        assert_eq!(out.stats().control_sent, 2);
        assert_eq!(out.stats().telemetry_sent, 1);
        // The high-water mark is a maximum, so draining does not lower it.
        assert_eq!(out.stats().control_high_water, 2);
        assert!(out.is_empty());
    }

    #[test]
    fn a_delta_is_a_control_message() {
        // The classification lives on ServerMessage; this pins that Outbound
        // uses it rather than a second opinion of its own.
        let mut out = Outbound::new(1);
        out.push(ServerMessage::Delta {
            delta: Delta::DirtyFlag {
                unsaved_changes: true,
            },
        })
        .unwrap();
        assert_eq!(out.control_len(), 1);
        assert!(
            out.push(ServerMessage::Delta {
                delta: Delta::DirtyFlag {
                    unsaved_changes: false
                }
            })
            .is_err()
        );
        // Telemetry still goes through a full control queue: the two are
        // independent, which is what "separate channel" means.
        out.push(telemetry(1)).unwrap();
    }

    #[test]
    fn a_queue_that_filled_says_how_far_behind_the_client_was() {
        let full = OutboundFull { queued: 256 };
        assert!(full.to_string().contains("256"));
    }
}
