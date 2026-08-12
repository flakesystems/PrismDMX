//! The telemetry channel — `docs/IPC_PROTOCOL.md` §7.
//!
//! Telemetry is separate from the control channel because it is high-rate, lossy
//! by nature, and must never delay a command. §7 fixes four things about it: 25
//! to 30 Hz, independent of the 44 Hz tick; DMX output levels and health as its
//! content; **binary, fixed layout — not MessagePack maps** as its encoding; and
//! coalesce-then-drop as its loss policy.
//!
//! # What this session decides, and what it does not
//!
//! This module is the **channel**: the envelope, the header, the versioning and
//! the levels section. What is measured beside the levels — meters, executor
//! fader positions, tick health — is not known until `prismd` exists and has
//! something to measure, so it is deliberately not invented here. The header
//! carries a version and a reserved byte for exactly that reason, and
//! [`TelemetryFrame::decode`] refuses a version it does not know rather than
//! reading a later layout as if it were this one.
//!
//! # Layout
//!
//! ```text
//! ┌────────┬───────┬───────┬────────────┬──────────┐
//! │ "PTLM" │ ver u8│ res u8│ count u16LE│ seq u64LE│   16-byte header
//! └────────┴───────┴───────┴────────────┴──────────┘
//! ┌────────────┬───────────────────────────────────┐
//! │ universe   │ 512 level bytes                   │   × count, in universe
//! │ u16 LE     │                                   │   order
//! └────────────┴───────────────────────────────────┘
//! ```
//!
//! Little-endian throughout, because both ends of this wire are the same
//! machine's byte order in every case that matters and the browser's
//! `DataView` reads it with one flag. Note that this is the *opposite* of DMX's
//! own network protocols, where `prism-protocols` writes big-endian because
//! Art-Net and E1.31 say so. The difference is that those are somebody else's
//! specifications and this one is ours.
//!
//! # Why the levels are one flat block
//!
//! 64 universes is 32 768 bytes and the frame is rebuilt 30 times a second. A
//! per-channel structure would cost more to build than to send, and the client
//! writes the block straight into a canvas renderer without parsing it — that is
//! the whole reason the second channel exists (§7, and `ARCHITECTURE_SPEC.md`
//! D3).

use core::fmt;

use prism_domain::UniverseId;

/// Bytes of fixed header ahead of the universe sections.
pub const TELEMETRY_HEADER_BYTES: usize = 16;

/// The telemetry layout version. Independent of [`crate::PROTOCOL_VERSION`]:
/// this channel is droppable and can grow a section without the control channel
/// breaking.
pub const TELEMETRY_VERSION: u8 = 1;

/// Channels in one DMX universe.
const CHANNELS_PER_UNIVERSE: usize = prism_domain::CHANNELS_PER_UNIVERSE as usize;

/// Bytes of one universe section: the universe number, then its levels.
const UNIVERSE_SECTION_BYTES: usize = 2 + CHANNELS_PER_UNIVERSE;

/// The four bytes every telemetry frame starts with.
const MAGIC: [u8; 4] = *b"PTLM";

/// Why a telemetry frame could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryError {
    /// The bytes do not start with the magic. Something other than telemetry
    /// arrived on the telemetry path.
    NotTelemetry,
    /// A layout this build does not know. The frame is dropped, which is what
    /// the channel does with frames anyway.
    UnknownVersion {
        /// The version the frame announced.
        found: u8,
        /// The version this build reads.
        expected: u8,
    },
    /// The frame is shorter than its own header says it is.
    Truncated {
        /// Bytes the header implies.
        expected: usize,
        /// Bytes there are.
        found: usize,
    },
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTelemetry => f.write_str("the frame does not begin with the telemetry magic"),
            Self::UnknownVersion { found, expected } => write!(
                f,
                "telemetry layout version {found}, and this build reads {expected}"
            ),
            Self::Truncated { expected, found } => write!(
                f,
                "the telemetry frame declares {expected} bytes and carries {found}"
            ),
        }
    }
}

impl core::error::Error for TelemetryError {}

/// One telemetry frame: a sequence number and the current levels per universe.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TelemetryFrame {
    /// Increments per frame, per connection. A client that sees a gap has
    /// dropped frames, which is allowed and worth showing.
    pub sequence: u64,
    /// One entry per universe the daemon is publishing, in universe order.
    pub universes: Vec<UniverseLevels>,
}

/// The 512 levels of one universe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniverseLevels {
    /// Which universe.
    pub universe: UniverseId,
    /// Levels, channel 1 at index 0. Not the DMX start code — that belongs to
    /// the wire, not to a picture of it.
    pub levels: [u8; CHANNELS_PER_UNIVERSE],
}

impl UniverseLevels {
    /// A universe at zero.
    #[must_use]
    pub const fn blackout(universe: UniverseId) -> Self {
        Self {
            universe,
            levels: [0; CHANNELS_PER_UNIVERSE],
        }
    }
}

impl TelemetryFrame {
    /// Bytes this frame will occupy.
    ///
    /// Not `const`: `Vec::len` only became usable in a `const` context in Rust
    /// 1.87, and this workspace supports 1.85.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        TELEMETRY_HEADER_BYTES + self.universes.len() * UNIVERSE_SECTION_BYTES
    }

    /// Writes the frame into `buffer`, clearing whatever was there.
    ///
    /// Takes a buffer rather than returning one because this runs 30 times a
    /// second: the caller keeps one buffer and reuses it, so a telemetry frame
    /// costs no allocation after the first.
    pub fn encode_into(&self, buffer: &mut Vec<u8>) {
        buffer.clear();
        buffer.reserve(self.encoded_len());
        buffer.extend_from_slice(&MAGIC);
        buffer.push(TELEMETRY_VERSION);
        buffer.push(0); // reserved: the room a later section is added in
        // Saturating rather than wrapping: 64 universes is the ceiling
        // (`UniverseId::MAX`), so this can only be reached by a caller that has
        // already gone wrong, and a truncated count is better than a count that
        // has wrapped to something small and plausible.
        let count = u16::try_from(self.universes.len()).unwrap_or(u16::MAX);
        buffer.extend_from_slice(&count.to_le_bytes());
        buffer.extend_from_slice(&self.sequence.to_le_bytes());
        for universe in self.universes.iter().take(usize::from(count)) {
            let number = u16::try_from(universe.universe.get()).unwrap_or(u16::MAX);
            buffer.extend_from_slice(&number.to_le_bytes());
            buffer.extend_from_slice(&universe.levels);
        }
    }

    /// The frame as a fresh byte vector.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        self.encode_into(&mut buffer);
        buffer
    }

    /// Reads a frame.
    ///
    /// # Errors
    ///
    /// [`TelemetryError`] if the bytes are not a telemetry frame of a layout
    /// this build knows, or are shorter than the header claims.
    pub fn decode(bytes: &[u8]) -> Result<Self, TelemetryError> {
        let header = bytes
            .get(..TELEMETRY_HEADER_BYTES)
            .ok_or(TelemetryError::Truncated {
                expected: TELEMETRY_HEADER_BYTES,
                found: bytes.len(),
            })?;
        if header[..4] != MAGIC {
            return Err(TelemetryError::NotTelemetry);
        }
        if header[4] != TELEMETRY_VERSION {
            return Err(TelemetryError::UnknownVersion {
                found: header[4],
                expected: TELEMETRY_VERSION,
            });
        }
        let count = usize::from(u16::from_le_bytes([header[6], header[7]]));
        let sequence = u64::from_le_bytes(
            header[8..16]
                .try_into()
                .unwrap_or_else(|_| unreachable!("the header slice is sixteen bytes")),
        );

        let expected = TELEMETRY_HEADER_BYTES + count * UNIVERSE_SECTION_BYTES;
        if bytes.len() != expected {
            return Err(TelemetryError::Truncated {
                expected,
                found: bytes.len(),
            });
        }

        let mut universes = Vec::with_capacity(count);
        for index in 0..count {
            let start = TELEMETRY_HEADER_BYTES + index * UNIVERSE_SECTION_BYTES;
            let section = &bytes[start..start + UNIVERSE_SECTION_BYTES];
            let number = u16::from_le_bytes([section[0], section[1]]);
            let mut levels = [0_u8; CHANNELS_PER_UNIVERSE];
            levels.copy_from_slice(&section[2..]);
            universes.push(UniverseLevels {
                universe: UniverseId::new(u32::from(number)),
                levels,
            });
        }
        Ok(Self {
            sequence,
            universes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CHANNELS_PER_UNIVERSE, MAGIC, TELEMETRY_HEADER_BYTES, TELEMETRY_VERSION, TelemetryError,
        TelemetryFrame, UNIVERSE_SECTION_BYTES, UniverseLevels,
    };
    use prism_domain::UniverseId;

    /// Not a frame of default values: S14's rule applies here too, and a
    /// blackout on universe zero would pass a decoder that read nothing at all.
    fn frame() -> TelemetryFrame {
        let mut first = UniverseLevels::blackout(UniverseId::new(1));
        first.levels[0] = 255;
        first.levels[511] = 7;
        let mut second = UniverseLevels::blackout(UniverseId::new(64));
        second.levels[255] = 128;
        TelemetryFrame {
            sequence: 4_294_967_296, // past u32, so a narrow field would show
            universes: vec![first, second],
        }
    }

    #[test]
    fn the_header_is_the_sixteen_bytes_the_layout_says_it_is() {
        let bytes = frame().encode();
        assert_eq!(&bytes[..4], &MAGIC);
        assert_eq!(bytes[4], TELEMETRY_VERSION);
        assert_eq!(bytes[5], 0, "the reserved byte is reserved");
        assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), 2);
        assert_eq!(
            u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            4_294_967_296
        );
        assert_eq!(
            bytes.len(),
            TELEMETRY_HEADER_BYTES + 2 * UNIVERSE_SECTION_BYTES
        );
    }

    #[test]
    fn a_frame_survives_the_round_trip_with_its_levels_where_they_were() {
        let decoded = TelemetryFrame::decode(&frame().encode()).unwrap();
        assert_eq!(decoded, frame());
        assert_eq!(decoded.universes[0].universe, UniverseId::new(1));
        assert_eq!(decoded.universes[0].levels[0], 255);
        assert_eq!(decoded.universes[0].levels[511], 7);
        assert_eq!(decoded.universes[1].universe, UniverseId::new(64));
        assert_eq!(decoded.universes[1].levels[255], 128);
    }

    #[test]
    fn the_declared_length_is_what_a_frame_measures() {
        assert_eq!(frame().encoded_len(), frame().encode().len());
        let empty = TelemetryFrame::default();
        assert_eq!(empty.encoded_len(), TELEMETRY_HEADER_BYTES);
        assert_eq!(TelemetryFrame::decode(&empty.encode()).unwrap(), empty);
    }

    /// The reason `encode_into` takes a buffer: 30 frames a second at 64
    /// universes.
    #[test]
    fn a_reused_buffer_is_filled_rather_than_appended_to() {
        let mut buffer = vec![0xff; 9_999];
        frame().encode_into(&mut buffer);
        assert_eq!(buffer, frame().encode());
        frame().encode_into(&mut buffer);
        assert_eq!(buffer, frame().encode());
    }

    #[test]
    fn sixty_four_universes_are_the_size_the_channel_was_designed_for() {
        let full = TelemetryFrame {
            sequence: 1,
            universes: (1..=64)
                .map(|n| UniverseLevels::blackout(UniverseId::new(n)))
                .collect(),
        };
        assert_eq!(full.encoded_len(), 16 + 64 * (2 + 512));
        assert!(
            full.encoded_len() < crate::MAX_FRAME_BYTES,
            "a full telemetry frame must fit one control frame"
        );
        assert_eq!(TelemetryFrame::decode(&full.encode()).unwrap(), full);
    }

    #[test]
    fn something_that_is_not_telemetry_is_not_read_as_telemetry() {
        assert_eq!(
            TelemetryFrame::decode(&[0; TELEMETRY_HEADER_BYTES]),
            Err(TelemetryError::NotTelemetry)
        );
        assert_eq!(
            TelemetryFrame::decode(&[]),
            Err(TelemetryError::Truncated {
                expected: TELEMETRY_HEADER_BYTES,
                found: 0
            })
        );
    }

    /// The reserved byte and the version are the room a later session grows
    /// into, so a frame from a newer daemon is dropped rather than misread.
    #[test]
    fn a_layout_this_build_does_not_know_is_dropped_rather_than_guessed_at() {
        let mut bytes = frame().encode();
        bytes[4] = TELEMETRY_VERSION + 1;
        assert_eq!(
            TelemetryFrame::decode(&bytes),
            Err(TelemetryError::UnknownVersion {
                found: TELEMETRY_VERSION + 1,
                expected: TELEMETRY_VERSION,
            })
        );
    }

    #[test]
    fn a_frame_shorter_than_its_own_header_is_refused() {
        let bytes = frame().encode();
        let expected = bytes.len();
        assert_eq!(
            TelemetryFrame::decode(&bytes[..bytes.len() - 1]),
            Err(TelemetryError::Truncated {
                expected,
                found: expected - 1
            })
        );
        // And longer, which is the case that would otherwise read a short frame
        // and leave rubbish behind it.
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(
            TelemetryFrame::decode(&longer),
            Err(TelemetryError::Truncated {
                expected,
                found: expected + 1
            })
        );
    }

    #[test]
    fn every_error_says_something_a_person_could_act_on() {
        for error in [
            TelemetryError::NotTelemetry,
            TelemetryError::UnknownVersion {
                found: 2,
                expected: 1,
            },
            TelemetryError::Truncated {
                expected: 16,
                found: 4,
            },
        ] {
            assert!(error.to_string().len() > 20, "{error:?}");
        }
    }

    #[test]
    fn a_blackout_is_every_channel_at_zero() {
        let levels = UniverseLevels::blackout(UniverseId::new(3));
        assert_eq!(levels.universe, UniverseId::new(3));
        assert!(levels.levels.iter().all(|&level| level == 0));
        assert_eq!(levels.levels.len(), CHANNELS_PER_UNIVERSE);
    }
}
