//! The MIDI wire layer: bytes in, whole messages out.
//!
//! This module knows nothing about Mackie Control. It knows what a status byte
//! is, that a compliant sender may leave one out, that a SysEx arrives in
//! whatever pieces the transport felt like, and that none of it is trustworthy.
//! `docs/MCU_MAPPING.md` §2.4 is the whole specification:
//!
//! - **running status** must be handled;
//! - **malformed or truncated messages are discarded silently and counted**;
//! - **SysEx reassembles across packet boundaries**, with a maximum buffer size
//!   and a timeout that drops an unterminated message;
//! - **nothing allocates per message.**
//!
//! # Why the counters are part of the interface
//!
//! `CLAUDE.md` requires that an invalid MIDI packet never propagate a failure,
//! and S10 and S11 both wrote down why silently discarding is the worse half of
//! that: what nobody counts, nobody can look for. A desk whose Select button
//! has stopped working and a desk whose cable is dropping bytes look identical
//! from the front. [`DecodeCounters`] is what tells them apart, so it is a
//! public value rather than a debug aid.
//!
//! # Time is an argument, not a clock
//!
//! The SysEx timeout needs to know what time it is, and this decoder is handed
//! the answer ([`MidiDecoder::push`] takes `now`) rather than holding a
//! [`Clock`](prism_engine::Clock) of its own. Three reasons, and the third is
//! the one that settles it: the arrival time of a packet is a *property of the
//! packet*, so a decoder that reads a clock is guessing at something its caller
//! knows exactly; a test then needs no simulated clock at all, only arithmetic;
//! and a `Clock` carries `sleep_until`, which a codec must never call. See
//! `PROGRESS.md`'s decision log.
//!
//! # What is bounded
//!
//! [`MAX_SYSEX_BYTES`] bounds the reassembly buffer, which is the only state in
//! this crate a sender can make grow. It is a fixed array inside the decoder,
//! so a hostile stream of unterminated SysEx costs a counter and nothing else —
//! measured in `tests/codec_allocations.rs` rather than asserted.

use core::fmt;
use std::time::Duration;

/// The largest SysEx payload this decoder will reassemble, in bytes between
/// `F0` and `F7` exclusive.
///
/// The longest message the MCU protocol defines is the scribble strip buffer:
/// five header bytes, an offset and up to 112 characters. 128 leaves room for a
/// device that answers a query with more than we expect and still bounds what a
/// stream can make this process hold.
pub const MAX_SYSEX_BYTES: usize = 128;

/// The longest byte sequence [`MidiMessage::encode_into`] can produce: a
/// maximum-length SysEx with its `F0` and `F7`.
pub const MAX_MESSAGE_BYTES: usize = MAX_SYSEX_BYTES + 2;

/// How long a half-arrived SysEx is held before it is dropped.
///
/// A whole scribble strip message is 64 bytes; over USB MIDI that is a
/// handful of packets and microseconds. A quarter of a second is therefore
/// three orders of magnitude more than any real message needs, and still far
/// below the point where a stalled reassembly could swallow a later message an
/// operator is waiting for.
pub const DEFAULT_SYSEX_TIMEOUT: Duration = Duration::from_millis(250);

/// One complete MIDI message.
///
/// All seven channel voice messages are here, not only the four the MCU uses.
/// A decoder that did not know how many data bytes a Program Change takes would
/// lose its place in the stream the moment one arrived — and deciding that a
/// message is not interesting is [`crate::ControlEvent`]'s job, one layer up,
/// where it can be counted as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiMessage<'a> {
    /// Note Off. The MCU sends releases as a Note On with velocity 0, but a
    /// device is entitled to send this instead.
    NoteOff {
        /// Zero-based MIDI channel.
        channel: u8,
        /// Note number.
        note: u8,
        /// Release velocity.
        velocity: u8,
    },
    /// Note On. Velocity 0 is a release.
    NoteOn {
        /// Zero-based MIDI channel.
        channel: u8,
        /// Note number.
        note: u8,
        /// Velocity.
        velocity: u8,
    },
    /// Polyphonic key pressure. Not used by the MCU.
    PolyPressure {
        /// Zero-based MIDI channel.
        channel: u8,
        /// Note number.
        note: u8,
        /// Pressure.
        pressure: u8,
    },
    /// Control Change: the V-Pots, the jog wheel, the ring LEDs and the
    /// 7-segment display.
    ControlChange {
        /// Zero-based MIDI channel.
        channel: u8,
        /// Controller number.
        controller: u8,
        /// Controller value.
        value: u8,
    },
    /// Program Change. Not used by the MCU.
    ProgramChange {
        /// Zero-based MIDI channel.
        channel: u8,
        /// Program number.
        program: u8,
    },
    /// Channel pressure: the level meters.
    ChannelPressure {
        /// Zero-based MIDI channel.
        channel: u8,
        /// Pressure — for the MCU, a strip and a meter level packed together.
        pressure: u8,
    },
    /// Pitch Bend: the fader positions, 14-bit, **LSB first on the wire**.
    PitchBend {
        /// Zero-based MIDI channel.
        channel: u8,
        /// 0…16383, already assembled from the two data bytes.
        value: u16,
    },
    /// A complete System Exclusive message, **without** its `F0` and `F7`.
    ///
    /// Borrowed from the decoder's reassembly buffer, which is what keeps a
    /// message that arrived in six packets from costing an allocation.
    SysEx(&'a [u8]),
}

/// Which kind of channel voice message a status byte introduces.
///
/// A separate type from the status byte so that turning data bytes into a
/// [`MidiMessage`] is a total match over seven cases rather than a match with
/// an unreachable arm — the codec is denied `panic!` in production code, and an
/// arm that cannot be reached is also an arm that cannot be covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChannelKind {
    NoteOff,
    NoteOn,
    PolyPressure,
    ControlChange,
    ProgramChange,
    ChannelPressure,
    PitchBend,
}

impl ChannelKind {
    /// The kind a status byte introduces, or `None` if it is not a channel
    /// voice status.
    const fn from_status(status: u8) -> Option<Self> {
        match status & 0xF0 {
            0x80 => Some(Self::NoteOff),
            0x90 => Some(Self::NoteOn),
            0xA0 => Some(Self::PolyPressure),
            0xB0 => Some(Self::ControlChange),
            0xC0 => Some(Self::ProgramChange),
            0xD0 => Some(Self::ChannelPressure),
            0xE0 => Some(Self::PitchBend),
            _ => None,
        }
    }

    /// The high nibble of this kind's status byte.
    const fn status_nibble(self) -> u8 {
        match self {
            Self::NoteOff => 0x80,
            Self::NoteOn => 0x90,
            Self::PolyPressure => 0xA0,
            Self::ControlChange => 0xB0,
            Self::ProgramChange => 0xC0,
            Self::ChannelPressure => 0xD0,
            Self::PitchBend => 0xE0,
        }
    }

    /// How many data bytes follow.
    const fn data_bytes(self) -> u8 {
        match self {
            Self::ProgramChange | Self::ChannelPressure => 1,
            _ => 2,
        }
    }

    /// The message these data bytes make.
    const fn message(self, channel: u8, data: [u8; 2]) -> MidiMessage<'static> {
        let [first, second] = data;
        match self {
            Self::NoteOff => MidiMessage::NoteOff {
                channel,
                note: first,
                velocity: second,
            },
            Self::NoteOn => MidiMessage::NoteOn {
                channel,
                note: first,
                velocity: second,
            },
            Self::PolyPressure => MidiMessage::PolyPressure {
                channel,
                note: first,
                pressure: second,
            },
            Self::ControlChange => MidiMessage::ControlChange {
                channel,
                controller: first,
                value: second,
            },
            Self::ProgramChange => MidiMessage::ProgramChange {
                channel,
                program: first,
            },
            Self::ChannelPressure => MidiMessage::ChannelPressure {
                channel,
                pressure: first,
            },
            // 14-bit, LSB first: the first data byte is the low seven bits.
            Self::PitchBend => MidiMessage::PitchBend {
                channel,
                value: (first as u16) | ((second as u16) << 7),
            },
        }
    }
}

/// Why a message could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The caller's buffer is shorter than the message.
    BufferTooSmall {
        /// Bytes the message needs.
        needed: usize,
        /// Bytes the caller offered.
        offered: usize,
    },
    /// A field does not fit the width MIDI gives it — a channel above 15, a
    /// data byte above 127, a pitch bend above 16383.
    OutOfRange {
        /// The field, as a caller would name it in a log line.
        field: &'static str,
        /// What it held.
        value: u32,
    },
    /// The control does not exist on this surface: a ninth strip, a button the
    /// profile has no note for, a 7-segment digit past the twelfth.
    NoSuchControl,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { needed, offered } => {
                write!(f, "need {needed} bytes, offered {offered}")
            }
            Self::OutOfRange { field, value } => write!(f, "{field} is out of range: {value}"),
            Self::NoSuchControl => f.write_str("no such control on this surface"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Writes `bytes` into `buf`, or says why it could not.
pub(crate) fn write(buf: &mut [u8], bytes: &[u8]) -> Result<usize, EncodeError> {
    let Some(target) = buf.get_mut(..bytes.len()) else {
        return Err(EncodeError::BufferTooSmall {
            needed: bytes.len(),
            offered: buf.len(),
        });
    };
    target.copy_from_slice(bytes);
    Ok(bytes.len())
}

/// Checks that a value fits a seven-bit data byte.
pub(crate) const fn data_byte(field: &'static str, value: u8) -> Result<u8, EncodeError> {
    if value > 0x7F {
        return Err(EncodeError::OutOfRange {
            field,
            value: value as u32,
        });
    }
    Ok(value)
}

/// Checks that a value fits a four-bit channel number.
const fn channel_nibble(value: u8) -> Result<u8, EncodeError> {
    if value > 0x0F {
        return Err(EncodeError::OutOfRange {
            field: "channel",
            value: value as u32,
        });
    }
    Ok(value)
}

impl MidiMessage<'_> {
    /// Writes this message into a caller-provided buffer and returns how many
    /// bytes it used.
    ///
    /// A release is written the way the MCU writes it — that decision belongs
    /// to [`crate::ControlEvent`], not here; this function writes exactly the
    /// message it is given.
    ///
    /// # Errors
    ///
    /// [`EncodeError::BufferTooSmall`] if the buffer is shorter than the
    /// message, [`EncodeError::OutOfRange`] if a field is wider than the wire
    /// allows.
    pub fn encode_into(&self, buf: &mut [u8]) -> Result<usize, EncodeError> {
        let (kind, channel, data) = match *self {
            Self::NoteOff {
                channel,
                note,
                velocity,
            } => (ChannelKind::NoteOff, channel, [note, velocity]),
            Self::NoteOn {
                channel,
                note,
                velocity,
            } => (ChannelKind::NoteOn, channel, [note, velocity]),
            Self::PolyPressure {
                channel,
                note,
                pressure,
            } => (ChannelKind::PolyPressure, channel, [note, pressure]),
            Self::ControlChange {
                channel,
                controller,
                value,
            } => (ChannelKind::ControlChange, channel, [controller, value]),
            Self::ProgramChange { channel, program } => {
                (ChannelKind::ProgramChange, channel, [program, 0])
            }
            Self::ChannelPressure { channel, pressure } => {
                (ChannelKind::ChannelPressure, channel, [pressure, 0])
            }
            Self::PitchBend { channel, value } => {
                if value > 0x3FFF {
                    return Err(EncodeError::OutOfRange {
                        field: "pitch bend",
                        value: u32::from(value),
                    });
                }
                // LSB first, and it is the half of this that a round trip
                // asserting only "it reads back" would never notice.
                let lsb = (value & 0x7F) as u8;
                let msb = (value >> 7) as u8;
                (ChannelKind::PitchBend, channel, [lsb, msb])
            }
            Self::SysEx(payload) => return write_sysex(buf, payload),
        };

        let channel = channel_nibble(channel)?;
        let [first, second] = data;
        let status = kind.status_nibble() | channel;
        if kind.data_bytes() == 1 {
            data_byte("data", first)?;
            return write(buf, &[status, first]);
        }
        data_byte("data", first)?;
        data_byte("data", second)?;
        write(buf, &[status, first, second])
    }
}

/// Writes `F0`, a payload and `F7`.
fn write_sysex(buf: &mut [u8], payload: &[u8]) -> Result<usize, EncodeError> {
    if payload.len() > MAX_SYSEX_BYTES {
        return Err(EncodeError::OutOfRange {
            field: "sysex length",
            value: payload.len() as u32,
        });
    }
    for byte in payload {
        data_byte("sysex payload", *byte)?;
    }
    let needed = payload.len() + 2;
    let Some(target) = buf.get_mut(..needed) else {
        return Err(EncodeError::BufferTooSmall {
            needed,
            offered: buf.len(),
        });
    };
    let (head, rest) = target.split_at_mut(1);
    let (body, tail) = rest.split_at_mut(payload.len());
    head.fill(0xF0);
    body.copy_from_slice(payload);
    tail.fill(0xF7);
    Ok(needed)
}

/// What the decoder has seen and thrown away.
///
/// Every field is a count of *bytes or messages that did not become a
/// [`MidiMessage`]*, except [`bytes`](Self::bytes) and
/// [`messages`](Self::messages), which are what did. A surface that is
/// misbehaving shows up here before anybody can describe the symptom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DecodeCounters {
    /// Bytes fed to the decoder.
    pub bytes: u64,
    /// Complete messages emitted.
    pub messages: u64,
    /// Data bytes that arrived with no status byte and no running status to
    /// borrow — the first bytes after a connection is opened mid-stream, or a
    /// cable dropping the status byte.
    pub orphan_data: u64,
    /// Messages cut short by a new status byte before their data was complete.
    pub truncated: u64,
    /// SysEx messages abandoned because another status byte arrived first.
    pub sysex_interrupted: u64,
    /// SysEx messages longer than [`MAX_SYSEX_BYTES`].
    pub sysex_overflow: u64,
    /// SysEx messages that never ended and aged out.
    pub sysex_timeout: u64,
    /// `F7` bytes with no SysEx open.
    pub stray_end: u64,
    /// System common messages (`F1`…`F6`), which the MCU does not use. Their
    /// data bytes are swallowed so the decoder does not lose its place.
    pub system_common: u64,
    /// System real-time bytes (`F8`…`FF`), which may appear anywhere including
    /// inside a SysEx and disturb nothing.
    pub realtime: u64,
}

impl DecodeCounters {
    /// Everything discarded, as one number for a status panel.
    #[must_use]
    pub const fn discarded(&self) -> u64 {
        self.orphan_data
            + self.truncated
            + self.sysex_interrupted
            + self.sysex_overflow
            + self.sysex_timeout
            + self.stray_end
    }
}

/// What the decoder is in the middle of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    /// Nothing: a data byte now is an orphan.
    None,
    /// A channel voice message, which doubles as the running status: after a
    /// complete message the decoder stays here, so the next data byte starts
    /// another one of the same kind.
    Channel {
        kind: ChannelKind,
        channel: u8,
        expected: u8,
    },
    /// A system common message whose data bytes are being swallowed.
    Common { expected: u8 },
    /// A SysEx being reassembled.
    SysEx,
}

/// Turns a stream of MIDI bytes into whole messages.
///
/// Holds no clock, no thread and no allocation. Feed it whatever a port hands
/// over — one byte, a USB packet, a whole burst — and it calls the sink once
/// per complete message.
#[derive(Debug)]
pub struct MidiDecoder {
    pending: Pending,
    data: [u8; 2],
    data_len: u8,
    sysex: [u8; MAX_SYSEX_BYTES],
    sysex_len: usize,
    sysex_overflowed: bool,
    sysex_started: Duration,
    timeout: Duration,
    counters: DecodeCounters,
}

impl Default for MidiDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiDecoder {
    /// A decoder with the default SysEx timeout.
    #[must_use]
    pub const fn new() -> Self {
        Self::with_timeout(DEFAULT_SYSEX_TIMEOUT)
    }

    /// A decoder that drops an unterminated SysEx after `timeout`.
    #[must_use]
    pub const fn with_timeout(timeout: Duration) -> Self {
        Self {
            pending: Pending::None,
            data: [0; 2],
            data_len: 0,
            sysex: [0; MAX_SYSEX_BYTES],
            sysex_len: 0,
            sysex_overflowed: false,
            sysex_started: Duration::ZERO,
            timeout,
            counters: DecodeCounters {
                bytes: 0,
                messages: 0,
                orphan_data: 0,
                truncated: 0,
                sysex_interrupted: 0,
                sysex_overflow: 0,
                sysex_timeout: 0,
                stray_end: 0,
                system_common: 0,
                realtime: 0,
            },
        }
    }

    /// What has been decoded and what has been thrown away.
    #[must_use]
    pub const fn counters(&self) -> DecodeCounters {
        self.counters
    }

    /// How long an unterminated SysEx is held.
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Whether a SysEx is half-arrived.
    ///
    /// Public because it is the difference between *the surface said nothing*
    /// and *the surface is mid-sentence*, and a reconnect decision upstairs may
    /// want to know.
    #[must_use]
    pub const fn is_reassembling(&self) -> bool {
        matches!(self.pending, Pending::SysEx)
    }

    /// Whether the decoder is holding a running status.
    #[must_use]
    pub const fn has_running_status(&self) -> bool {
        matches!(self.pending, Pending::Channel { .. })
    }

    /// Drops a half-arrived SysEx that has aged past the timeout.
    ///
    /// Called for you by [`push`](Self::push), and worth calling on an idle
    /// port as well: a stream that goes silent in the middle of a SysEx never
    /// pushes another byte, so nothing else would ever notice. Returns whether
    /// a message was dropped.
    pub fn poll(&mut self, now: Duration) -> bool {
        if !self.is_reassembling() {
            return false;
        }
        if now.saturating_sub(self.sysex_started) < self.timeout {
            return false;
        }
        self.counters.sysex_timeout += 1;
        self.reset_sysex();
        self.pending = Pending::None;
        true
    }

    /// Feeds bytes to the decoder, calling `sink` once per complete message.
    ///
    /// `now` is the arrival time of this packet on any monotonic scale, and is
    /// used for nothing but the SysEx timeout.
    pub fn push<F>(&mut self, bytes: &[u8], now: Duration, mut sink: F)
    where
        F: FnMut(MidiMessage<'_>),
    {
        self.poll(now);
        self.counters.bytes += bytes.len() as u64;
        for byte in bytes {
            let byte = *byte;
            if byte < 0x80 {
                self.on_data(byte, &mut sink);
            } else if byte >= 0xF8 {
                // System real-time. It may appear between any two bytes,
                // including inside a SysEx, and disturbs nothing.
                self.counters.realtime += 1;
            } else {
                self.on_status(byte, now, &mut sink);
            }
        }
    }

    /// A data byte.
    fn on_data<F>(&mut self, byte: u8, sink: &mut F)
    where
        F: FnMut(MidiMessage<'_>),
    {
        match self.pending {
            Pending::SysEx => {
                if let Some(slot) = self.sysex.get_mut(self.sysex_len) {
                    *slot = byte;
                    self.sysex_len += 1;
                } else if !self.sysex_overflowed {
                    // Counted once per message rather than once per byte: a
                    // truncated 4 KiB burst is one fault, not four thousand.
                    self.sysex_overflowed = true;
                    self.counters.sysex_overflow += 1;
                }
            }
            Pending::Channel {
                kind,
                channel,
                expected,
            } => {
                if let Some(slot) = self.data.get_mut(usize::from(self.data_len)) {
                    *slot = byte;
                    self.data_len += 1;
                }
                if self.data_len >= expected {
                    self.data_len = 0;
                    self.counters.messages += 1;
                    sink(kind.message(channel, self.data));
                }
            }
            Pending::Common { expected } => {
                self.data_len += 1;
                if self.data_len >= expected {
                    self.data_len = 0;
                    self.pending = Pending::None;
                }
            }
            Pending::None => self.counters.orphan_data += 1,
        }
    }

    /// A status byte below `F8`.
    fn on_status<F>(&mut self, status: u8, now: Duration, sink: &mut F)
    where
        F: FnMut(MidiMessage<'_>),
    {
        if status == 0xF7 {
            if self.is_reassembling() {
                self.finish_sysex(sink);
            } else {
                // A `F7` that ends nothing is still a status byte, so it
                // cancels running status like any other.
                self.abandon_pending();
                self.counters.stray_end += 1;
            }
            return;
        }

        self.abandon_pending();

        if let Some(kind) = ChannelKind::from_status(status) {
            self.pending = Pending::Channel {
                kind,
                channel: status & 0x0F,
                expected: kind.data_bytes(),
            };
            self.data_len = 0;
            return;
        }

        if status == 0xF0 {
            self.reset_sysex();
            self.sysex_started = now;
            self.pending = Pending::SysEx;
            return;
        }

        // F1…F6: system common. The MCU uses none of them, but their data
        // bytes have to be swallowed or the decoder loses its place in the
        // stream and reports the rest of the burst as orphans.
        self.counters.system_common += 1;
        self.data_len = 0;
        let expected = common_data_bytes(status);
        self.pending = if expected == 0 {
            Pending::None
        } else {
            Pending::Common { expected }
        };
    }

    /// Gives up on whatever was half-arrived, counting it.
    fn abandon_pending(&mut self) {
        match self.pending {
            Pending::SysEx => {
                self.counters.sysex_interrupted += 1;
                self.reset_sysex();
            }
            Pending::Channel { .. } | Pending::Common { .. } => {
                if self.data_len > 0 {
                    self.counters.truncated += 1;
                }
            }
            Pending::None => {}
        }
        self.data_len = 0;
        self.pending = Pending::None;
    }

    /// An `F7` that ends a SysEx this decoder was reassembling.
    fn finish_sysex<F>(&mut self, sink: &mut F)
    where
        F: FnMut(MidiMessage<'_>),
    {
        if !self.sysex_overflowed {
            // The borrow of `sysex` and the writes to the other fields are
            // disjoint, which is what lets the payload be handed out by
            // reference instead of copied.
            if let Some(payload) = self.sysex.get(..self.sysex_len) {
                self.counters.messages += 1;
                sink(MidiMessage::SysEx(payload));
            }
        }
        self.reset_sysex();
        // A SysEx cancels running status, per the MIDI specification.
        self.pending = Pending::None;
    }

    /// Empties the reassembly buffer's bookkeeping. The array itself is left
    /// alone: nothing reads past `sysex_len`, and clearing 128 bytes per
    /// message would be work done for a tidiness nobody can observe.
    fn reset_sysex(&mut self) {
        self.sysex_len = 0;
        self.sysex_overflowed = false;
    }
}

/// How many data bytes a system common status takes.
const fn common_data_bytes(status: u8) -> u8 {
    match status {
        // MIDI Time Code quarter frame, Song Select.
        0xF1 | 0xF3 => 1,
        // Song Position Pointer.
        0xF2 => 2,
        // F4 and F5 are undefined, F6 is Tune Request.
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_SYSEX_TIMEOUT, EncodeError, MAX_MESSAGE_BYTES, MAX_SYSEX_BYTES, MidiDecoder,
        MidiMessage,
    };
    use std::time::Duration;

    /// Decodes a whole stream at one instant and collects what came out.
    ///
    /// SysEx payloads are copied here because the borrow ends with the call —
    /// which is the point of handing them out by reference in the first place.
    fn decode(decoder: &mut MidiDecoder, bytes: &[u8]) -> Vec<Owned> {
        decode_at(decoder, bytes, Duration::from_millis(1))
    }

    fn decode_at(decoder: &mut MidiDecoder, bytes: &[u8], now: Duration) -> Vec<Owned> {
        let mut out = Vec::new();
        decoder.push(bytes, now, |message| out.push(Owned::from(message)));
        out
    }

    /// A [`MidiMessage`] that owns its SysEx payload, so a test can keep it.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Owned {
        Channel(MidiMessage<'static>),
        SysEx(Vec<u8>),
    }

    impl From<MidiMessage<'_>> for Owned {
        fn from(message: MidiMessage<'_>) -> Self {
            match message {
                MidiMessage::SysEx(payload) => Self::SysEx(payload.to_vec()),
                MidiMessage::NoteOff {
                    channel,
                    note,
                    velocity,
                } => Self::Channel(MidiMessage::NoteOff {
                    channel,
                    note,
                    velocity,
                }),
                MidiMessage::NoteOn {
                    channel,
                    note,
                    velocity,
                } => Self::Channel(MidiMessage::NoteOn {
                    channel,
                    note,
                    velocity,
                }),
                MidiMessage::PolyPressure {
                    channel,
                    note,
                    pressure,
                } => Self::Channel(MidiMessage::PolyPressure {
                    channel,
                    note,
                    pressure,
                }),
                MidiMessage::ControlChange {
                    channel,
                    controller,
                    value,
                } => Self::Channel(MidiMessage::ControlChange {
                    channel,
                    controller,
                    value,
                }),
                MidiMessage::ProgramChange { channel, program } => {
                    Self::Channel(MidiMessage::ProgramChange { channel, program })
                }
                MidiMessage::ChannelPressure { channel, pressure } => {
                    Self::Channel(MidiMessage::ChannelPressure { channel, pressure })
                }
                MidiMessage::PitchBend { channel, value } => {
                    Self::Channel(MidiMessage::PitchBend { channel, value })
                }
            }
        }
    }

    fn channel(message: MidiMessage<'static>) -> Owned {
        Owned::Channel(message)
    }

    #[test]
    fn every_channel_voice_message_decodes_with_the_right_data_length() {
        // Not because the MCU sends all seven, but because a decoder that
        // guessed two data bytes for a Program Change would swallow the status
        // byte of whatever came next.
        let mut decoder = MidiDecoder::new();
        let stream = [
            0x83, 0x40, 0x20, // Note Off, channel 4
            0x94, 0x41, 0x7F, // Note On, channel 5
            0xA5, 0x42, 0x21, // Poly pressure, channel 6
            0xB6, 0x10, 0x41, // CC, channel 7
            0xC7, 0x22, // Program change, channel 8
            0xD8, 0x23, // Channel pressure, channel 9
            0xE9, 0x0C, 0x63, // Pitch bend, channel 10
        ];
        let messages = decode(&mut decoder, &stream);
        assert_eq!(
            messages,
            vec![
                channel(MidiMessage::NoteOff {
                    channel: 3,
                    note: 0x40,
                    velocity: 0x20
                }),
                channel(MidiMessage::NoteOn {
                    channel: 4,
                    note: 0x41,
                    velocity: 0x7F
                }),
                channel(MidiMessage::PolyPressure {
                    channel: 5,
                    note: 0x42,
                    pressure: 0x21
                }),
                channel(MidiMessage::ControlChange {
                    channel: 6,
                    controller: 0x10,
                    value: 0x41
                }),
                channel(MidiMessage::ProgramChange {
                    channel: 7,
                    program: 0x22
                }),
                channel(MidiMessage::ChannelPressure {
                    channel: 8,
                    pressure: 0x23
                }),
                channel(MidiMessage::PitchBend {
                    channel: 9,
                    value: 0x0C | (0x63 << 7)
                }),
            ]
        );
        assert_eq!(decoder.counters().messages, 7);
        assert_eq!(decoder.counters().discarded(), 0);
    }

    #[test]
    fn pitch_bend_is_fourteen_bits_with_the_low_seven_first() {
        // The half of the fader path that a round trip asserting only "it reads
        // back" cannot see: swapping the two bytes is symmetrical, so it
        // survives an encode/decode pair and puts the fader in the wrong place
        // on a real desk. So the bytes are named.
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0xE2, 0x7F, 0x00]);
        assert_eq!(
            messages,
            vec![channel(MidiMessage::PitchBend {
                channel: 2,
                value: 127
            })]
        );
        let messages = decode(&mut decoder, &[0xE2, 0x00, 0x01]);
        assert_eq!(
            messages,
            vec![channel(MidiMessage::PitchBend {
                channel: 2,
                value: 128
            })]
        );
        let mut buf = [0u8; 3];
        let written = MidiMessage::PitchBend {
            channel: 2,
            value: 128,
        }
        .encode_into(&mut buf)
        .expect("three bytes is enough");
        assert_eq!(&buf[..written], &[0xE2, 0x00, 0x01]);
    }

    #[test]
    fn running_status_decodes_identically_to_an_explicit_status_byte() {
        // IMPLEMENTATION_PLAN.md S19: "Running status decoded identically to
        // explicit status". Two streams, one assertion - and the streams are
        // built to differ in length so a test that compared them to themselves
        // could not pass.
        let explicit = [
            0x90, 0x18, 0x7F, 0x90, 0x19, 0x00, 0x90, 0x1A, 0x7F, 0x90, 0x1B, 0x40,
        ];
        let running = [0x90, 0x18, 0x7F, 0x19, 0x00, 0x1A, 0x7F, 0x1B, 0x40];
        assert!(running.len() < explicit.len());

        let mut one = MidiDecoder::new();
        let mut two = MidiDecoder::new();
        assert_eq!(decode(&mut one, &explicit), decode(&mut two, &running));
        assert_eq!(one.counters().messages, 4);
        assert_eq!(two.counters().messages, 4);
        assert_eq!(two.counters().discarded(), 0);
        assert!(two.has_running_status());
    }

    #[test]
    fn running_status_survives_a_real_time_byte_and_is_cancelled_by_system_common() {
        // The MIDI rule, and the one a naive parser gets wrong: F8-FF may
        // appear between any two bytes and change nothing; F1-F6 cancel running
        // status.
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0x90, 0x18, 0x7F, 0xFE, 0x19, 0x7F]);
        assert_eq!(messages.len(), 2);
        assert_eq!(decoder.counters().realtime, 1);

        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0x90, 0x18, 0x7F, 0xF6, 0x19, 0x7F]);
        assert_eq!(messages.len(), 1);
        assert_eq!(decoder.counters().system_common, 1);
        assert_eq!(decoder.counters().orphan_data, 2);
        assert!(!decoder.has_running_status());
    }

    #[test]
    fn a_system_common_message_takes_its_data_bytes_with_it() {
        // A Song Position Pointer is three bytes. A decoder that dropped only
        // the status byte would report its two data bytes as orphans, and the
        // counter that is supposed to mean "the cable is dropping bytes" would
        // be measuring a device saying something ordinary.
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0xF2, 0x11, 0x22, 0x90, 0x18, 0x7F]);
        assert_eq!(
            messages,
            vec![channel(MidiMessage::NoteOn {
                channel: 0,
                note: 0x18,
                velocity: 0x7F
            })]
        );
        assert_eq!(decoder.counters().system_common, 1);
        assert_eq!(decoder.counters().orphan_data, 0);
    }

    #[test]
    fn a_data_byte_with_no_status_is_counted_rather_than_guessed_at() {
        // Attaching to a stream already in flight is the ordinary way to see
        // this, and inventing a status byte for it would put a fader move on
        // whatever channel was fashionable.
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0x40, 0x7F, 0x90, 0x18, 0x7F]);
        assert_eq!(messages.len(), 1);
        assert_eq!(decoder.counters().orphan_data, 2);
        assert_eq!(decoder.counters().messages, 1);
    }

    #[test]
    fn a_message_cut_short_by_a_new_status_is_discarded_and_counted() {
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0x90, 0x18, 0xB0, 0x10, 0x01]);
        assert_eq!(
            messages,
            vec![channel(MidiMessage::ControlChange {
                channel: 0,
                controller: 0x10,
                value: 0x01
            })]
        );
        assert_eq!(decoder.counters().truncated, 1);
        assert_eq!(decoder.counters().messages, 1);
    }

    #[test]
    fn a_status_byte_that_repeats_before_its_data_is_not_a_truncation() {
        // A sender restating the status byte it is already in is legal and
        // common. Counting it as a fault would fill the diagnostics with noise
        // and hide the real one.
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0x90, 0x90, 0x18, 0x7F]);
        assert_eq!(messages.len(), 1);
        assert_eq!(decoder.counters().truncated, 0);
    }

    #[test]
    fn a_sysex_split_across_packets_reassembles_into_one_message() {
        // IMPLEMENTATION_PLAN.md S19. The packets are deliberately uneven and
        // one of them is a single byte, because a decoder that only worked on
        // whole messages would pass a test that split them in halves.
        let mut decoder = MidiDecoder::new();
        let mut out = Vec::new();
        for (index, packet) in [
            &[0xF0, 0x00, 0x00][..],
            &[0x66][..],
            &[0x14, 0x12, 0x07][..],
            &b"Prism"[..],
            &[0xF7][..],
        ]
        .into_iter()
        .enumerate()
        {
            let now = Duration::from_millis(index as u64);
            decoder.push(packet, now, |message| out.push(Owned::from(message)));
        }
        assert_eq!(
            out,
            vec![Owned::SysEx(vec![
                0x00, 0x00, 0x66, 0x14, 0x12, 0x07, b'P', b'r', b'i', b's', b'm'
            ])]
        );
        assert_eq!(decoder.counters().messages, 1);
        assert_eq!(decoder.counters().discarded(), 0);
        assert!(!decoder.is_reassembling());
    }

    #[test]
    fn a_real_time_byte_inside_a_sysex_does_not_join_the_payload() {
        // Active sensing arrives every 300 ms on some devices and lands wherever
        // it lands. A payload with an FE in it is a scribble strip full of
        // rubbish.
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0xF0, 0x11, 0xFE, 0x22, 0xF7]);
        assert_eq!(messages, vec![Owned::SysEx(vec![0x11, 0x22])]);
        assert_eq!(decoder.counters().realtime, 1);
    }

    #[test]
    fn an_unterminated_sysex_times_out_and_is_dropped() {
        // IMPLEMENTATION_PLAN.md S19. The wait has a deadline because the time
        // is an argument: no sleeping, and the boundary is asserted from both
        // sides rather than from the far side only.
        let mut decoder = MidiDecoder::new();
        decode_at(&mut decoder, &[0xF0, 0x00, 0x00, 0x66], Duration::ZERO);
        assert!(decoder.is_reassembling());

        // One tick before the deadline it is still being held.
        assert!(!decoder.poll(DEFAULT_SYSEX_TIMEOUT - Duration::from_millis(1)));
        assert!(decoder.is_reassembling());
        assert_eq!(decoder.counters().sysex_timeout, 0);

        assert!(decoder.poll(DEFAULT_SYSEX_TIMEOUT));
        assert!(!decoder.is_reassembling());
        assert_eq!(decoder.counters().sysex_timeout, 1);
        assert!(!decoder.poll(Duration::from_secs(60)));

        // And the F7 that eventually turns up belongs to nothing.
        let messages = decode_at(&mut decoder, &[0xF7], Duration::from_secs(61));
        assert!(messages.is_empty());
        assert_eq!(decoder.counters().stray_end, 1);
    }

    #[test]
    fn a_timed_out_sysex_does_not_swallow_the_message_after_it() {
        // The timeout is checked when the next packet arrives too, not only by
        // an explicit poll - otherwise a port that goes quiet and then speaks
        // again loses the first thing it says.
        let mut decoder = MidiDecoder::with_timeout(Duration::from_millis(10));
        decode_at(&mut decoder, &[0xF0, 0x00], Duration::ZERO);
        let messages = decode_at(&mut decoder, &[0x90, 0x18, 0x7F], Duration::from_millis(50));
        assert_eq!(
            messages,
            vec![channel(MidiMessage::NoteOn {
                channel: 0,
                note: 0x18,
                velocity: 0x7F
            })]
        );
        assert_eq!(decoder.counters().sysex_timeout, 1);
    }

    #[test]
    fn a_sysex_interrupted_by_another_status_is_abandoned() {
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0xF0, 0x00, 0x00, 0x90, 0x18, 0x7F]);
        assert_eq!(messages.len(), 1);
        assert_eq!(decoder.counters().sysex_interrupted, 1);

        // And a second F0 restarts rather than nesting.
        let mut decoder = MidiDecoder::new();
        let messages = decode(&mut decoder, &[0xF0, 0x11, 0xF0, 0x22, 0xF7]);
        assert_eq!(messages, vec![Owned::SysEx(vec![0x22])]);
        assert_eq!(decoder.counters().sysex_interrupted, 1);
    }

    #[test]
    fn a_sysex_longer_than_the_buffer_is_discarded_whole_and_counted_once() {
        // Not truncated to fit: a scribble strip line built from the first 128
        // bytes of a 200-byte message is a display full of plausible nonsense,
        // which is worse than a display that did not change.
        let mut decoder = MidiDecoder::new();
        let mut stream = vec![0xF0];
        stream.extend(std::iter::repeat_n(0x01, MAX_SYSEX_BYTES + 40));
        stream.push(0xF7);
        let messages = decode(&mut decoder, &stream);
        assert!(messages.is_empty());
        assert_eq!(decoder.counters().sysex_overflow, 1);
        assert_eq!(decoder.counters().messages, 0);

        // The decoder is usable immediately afterwards.
        let messages = decode(&mut decoder, &[0xF0, 0x42, 0xF7]);
        assert_eq!(messages, vec![Owned::SysEx(vec![0x42])]);
    }

    #[test]
    fn a_sysex_of_exactly_the_buffer_size_still_arrives() {
        // The off-by-one on the other side of the limit, and it is the one that
        // would quietly refuse the longest legal scribble strip message.
        let mut decoder = MidiDecoder::new();
        let mut stream = vec![0xF0];
        stream.extend(std::iter::repeat_n(0x7F, MAX_SYSEX_BYTES));
        stream.push(0xF7);
        let messages = decode(&mut decoder, &stream);
        assert_eq!(messages, vec![Owned::SysEx(vec![0x7F; MAX_SYSEX_BYTES])]);
        assert_eq!(decoder.counters().sysex_overflow, 0);
    }

    #[test]
    fn an_empty_sysex_is_a_message_rather_than_nothing() {
        let mut decoder = MidiDecoder::new();
        assert_eq!(
            decode(&mut decoder, &[0xF0, 0xF7]),
            vec![Owned::SysEx(Vec::new())]
        );
        assert_eq!(decoder.counters().messages, 1);
    }

    #[test]
    fn a_sysex_cancels_running_status() {
        let mut decoder = MidiDecoder::new();
        let messages = decode(
            &mut decoder,
            &[0x90, 0x18, 0x7F, 0xF0, 0x11, 0xF7, 0x19, 0x7F],
        );
        assert_eq!(messages.len(), 2);
        assert_eq!(decoder.counters().orphan_data, 2);
        assert!(!decoder.has_running_status());
    }

    #[test]
    fn the_counters_start_at_zero_and_count_every_byte() {
        let decoder = MidiDecoder::default();
        assert_eq!(decoder.counters(), Default::default());
        assert_eq!(decoder.counters().discarded(), 0);
        assert_eq!(decoder.timeout(), DEFAULT_SYSEX_TIMEOUT);

        let mut decoder = MidiDecoder::new();
        decode(&mut decoder, &[0x90, 0x18, 0x7F, 0xFE]);
        assert_eq!(decoder.counters().bytes, 4);
    }

    #[test]
    fn a_message_writes_the_bytes_it_was_read_from() {
        let mut buf = [0u8; MAX_MESSAGE_BYTES];
        let cases: [(MidiMessage<'_>, &[u8]); 8] = [
            (
                MidiMessage::NoteOff {
                    channel: 3,
                    note: 0x40,
                    velocity: 0x20,
                },
                &[0x83, 0x40, 0x20],
            ),
            (
                MidiMessage::NoteOn {
                    channel: 4,
                    note: 0x41,
                    velocity: 0x7F,
                },
                &[0x94, 0x41, 0x7F],
            ),
            (
                MidiMessage::PolyPressure {
                    channel: 5,
                    note: 0x42,
                    pressure: 0x21,
                },
                &[0xA5, 0x42, 0x21],
            ),
            (
                MidiMessage::ControlChange {
                    channel: 6,
                    controller: 0x10,
                    value: 0x41,
                },
                &[0xB6, 0x10, 0x41],
            ),
            (
                MidiMessage::ProgramChange {
                    channel: 7,
                    program: 0x22,
                },
                &[0xC7, 0x22],
            ),
            (
                MidiMessage::ChannelPressure {
                    channel: 8,
                    pressure: 0x23,
                },
                &[0xD8, 0x23],
            ),
            (
                MidiMessage::PitchBend {
                    channel: 9,
                    value: 0x31EC,
                },
                &[0xE9, 0x6C, 0x63],
            ),
            (
                MidiMessage::SysEx(&[0x00, 0x00, 0x66, 0x14]),
                &[0xF0, 0x00, 0x00, 0x66, 0x14, 0xF7],
            ),
        ];
        for (message, expected) in cases {
            let written = message.encode_into(&mut buf).expect("buffer is big enough");
            assert_eq!(&buf[..written], expected, "{message:?}");
        }
    }

    #[test]
    fn a_field_wider_than_the_wire_is_refused_rather_than_masked() {
        // Masking would put a Select press on the wrong strip and look like it
        // worked, which is the failure mode this exists to prevent.
        let mut buf = [0u8; MAX_MESSAGE_BYTES];
        assert_eq!(
            MidiMessage::NoteOn {
                channel: 16,
                note: 1,
                velocity: 1
            }
            .encode_into(&mut buf),
            Err(EncodeError::OutOfRange {
                field: "channel",
                value: 16
            })
        );
        assert_eq!(
            MidiMessage::NoteOn {
                channel: 0,
                note: 0x80,
                velocity: 1
            }
            .encode_into(&mut buf),
            Err(EncodeError::OutOfRange {
                field: "data",
                value: 0x80
            })
        );
        assert_eq!(
            MidiMessage::NoteOn {
                channel: 0,
                note: 1,
                velocity: 0xFF
            }
            .encode_into(&mut buf),
            Err(EncodeError::OutOfRange {
                field: "data",
                value: 0xFF
            })
        );
        assert_eq!(
            MidiMessage::ProgramChange {
                channel: 0,
                program: 0x99
            }
            .encode_into(&mut buf),
            Err(EncodeError::OutOfRange {
                field: "data",
                value: 0x99
            })
        );
        assert_eq!(
            MidiMessage::PitchBend {
                channel: 0,
                value: 0x4000
            }
            .encode_into(&mut buf),
            Err(EncodeError::OutOfRange {
                field: "pitch bend",
                value: 0x4000
            })
        );
        assert_eq!(
            MidiMessage::SysEx(&[0x80]).encode_into(&mut buf),
            Err(EncodeError::OutOfRange {
                field: "sysex payload",
                value: 0x80
            })
        );
        assert_eq!(
            MidiMessage::SysEx(&[0x01; MAX_SYSEX_BYTES + 1]).encode_into(&mut buf),
            Err(EncodeError::OutOfRange {
                field: "sysex length",
                value: (MAX_SYSEX_BYTES + 1) as u32
            })
        );
    }

    #[test]
    fn a_buffer_that_is_too_small_says_how_much_was_needed() {
        let mut buf = [0u8; 2];
        assert_eq!(
            MidiMessage::NoteOn {
                channel: 0,
                note: 1,
                velocity: 1
            }
            .encode_into(&mut buf),
            Err(EncodeError::BufferTooSmall {
                needed: 3,
                offered: 2
            })
        );
        assert_eq!(
            MidiMessage::SysEx(&[0x01, 0x02]).encode_into(&mut buf),
            Err(EncodeError::BufferTooSmall {
                needed: 4,
                offered: 2
            })
        );
        let mut small = [0u8; 1];
        assert_eq!(
            MidiMessage::ProgramChange {
                channel: 0,
                program: 1
            }
            .encode_into(&mut small),
            Err(EncodeError::BufferTooSmall {
                needed: 2,
                offered: 1
            })
        );
    }

    #[test]
    fn an_encode_error_explains_itself_in_a_log_line() {
        assert_eq!(
            EncodeError::BufferTooSmall {
                needed: 3,
                offered: 2
            }
            .to_string(),
            "need 3 bytes, offered 2"
        );
        assert_eq!(
            EncodeError::OutOfRange {
                field: "channel",
                value: 16
            }
            .to_string(),
            "channel is out of range: 16"
        );
        assert_eq!(
            EncodeError::NoSuchControl.to_string(),
            "no such control on this surface"
        );
    }
}
