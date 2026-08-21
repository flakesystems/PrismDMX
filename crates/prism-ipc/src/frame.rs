//! Length-prefixed MessagePack — `docs/IPC_PROTOCOL.md` §3.
//!
//! ```text
//! ┌──────────────┬───────────────────────────┐
//! │ u32 LE length│ MessagePack payload       │
//! └──────────────┴───────────────────────────┘
//! ```
//!
//! # Three refusals, and the order they happen in
//!
//! 1. **Too large.** The length prefix is read first and checked against
//!    [`MAX_FRAME_BYTES`] *before* a buffer for the body exists. A peer
//!    announcing four gigabytes is disconnected after four bytes have been
//!    read, having cost four bytes of memory — see [`payload_length`], and
//!    `tests/oversized_frame.rs`, which counts what the allocator was asked for.
//! 2. **Too deep.** [`crate::scan::depth_of`] walks the payload with an explicit
//!    stack before `serde` is allowed near it. The reason is written out in
//!    `scan`'s module documentation: a stack overflow aborts the process, and
//!    this process is holding the DMX output.
//! 3. **Not the message it claims to be.** Only then is the payload deserialised.
//!
//! # Encoding is fallible, and that is a decision rather than an accident
//!
//! S1 guards every `f64` in the domain against non-finite values in *both*
//! directions, so `to_vec_named` really can fail — an infinite fade time is
//! refused at the wire rather than travelling and producing a cue that never
//! completes. Everything here therefore returns a `Result`, including the
//! encode side.
//!
//! `to_vec_named` and not `to_vec`: `Command` and `Delta` are internally tagged
//! (`#[serde(tag = "t")]`), and MessagePack's compact array encoding of a struct
//! has nowhere to put the tag.

use core::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::scan::{ScanFault, TooDeepOr, depth_of};

/// Bytes of length prefix ahead of every payload.
pub const LENGTH_PREFIX_BYTES: usize = 4;

/// The largest payload either side will read or write, in bytes.
///
/// A megabyte is far more than any message the protocol defines: the largest is
/// a `Snapshot` of a whole show, which is a few hundred kilobytes for a school's
/// rig (`PROGRESS.md`, S15). The limit exists to bound what an unauthenticated
/// socket can make the daemon allocate, so it is set where a legitimate message
/// will never reach it and a hostile one always will.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// The deepest container nesting a payload may contain.
///
/// The same limit `serde_json` applies, for the same reason and against the same
/// attack. Every message the protocol defines nests under ten levels; the
/// headroom is for `JsonValue`, which is the only unbounded shape on the wire.
pub const MAX_NESTING_DEPTH: usize = 128;

/// Why a frame could not be produced or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// The value could not be serialised. In practice a non-finite `f64`, which
    /// the domain refuses in both directions (S1).
    Encode(String),
    /// Well-formed MessagePack that is not the message it was read as.
    Decode(String),
    /// The frame is larger than [`MAX_FRAME_BYTES`]. Carries what was announced,
    /// so a log can say how far out of range it was — nothing was allocated for
    /// it.
    TooLarge {
        /// The payload length the header announced, in bytes.
        announced: u64,
        /// The limit it was measured against.
        limit: usize,
    },
    /// The payload nests deeper than [`MAX_NESTING_DEPTH`].
    TooDeep {
        /// The limit it was measured against.
        limit: usize,
    },
    /// The payload is not one well-formed MessagePack value.
    Malformed(ScanFault),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(why) => write!(f, "the message could not be encoded: {why}"),
            Self::Decode(why) => write!(f, "the frame is not the message it claims to be: {why}"),
            Self::TooLarge { announced, limit } => write!(
                f,
                "the frame announces {announced} bytes, and the limit is {limit}"
            ),
            Self::TooDeep { limit } => {
                write!(f, "the payload nests deeper than {limit} levels")
            }
            Self::Malformed(fault) => write!(f, "malformed payload: {fault}"),
        }
    }
}

impl core::error::Error for FrameError {}

impl FrameError {
    /// Whether the reader has lost track of where the next frame begins.
    ///
    /// This is the line between "close the connection" and "tell the peer and
    /// carry on", and it is about the *framing* rather than about how bad the
    /// message was. An oversized frame is read from the length prefix, so the
    /// bytes that follow it are of unknown length and the connection is over —
    /// §3's "closes the connection rather than allocating". Everything else here
    /// concerns a payload the framing already delimited: the next frame starts
    /// exactly where it always would, so the peer is answered with a `Reject`
    /// and the connection carries on. A hostile client gets a refusal per
    /// message rather than a way to be disconnected on purpose.
    #[must_use]
    pub const fn loses_the_frame_boundary(&self) -> bool {
        match self {
            Self::TooLarge { .. } => true,
            Self::TooDeep { .. } | Self::Malformed(_) | Self::Encode(_) | Self::Decode(_) => false,
        }
    }
}

/// Serialises `value` into a MessagePack payload, without the length prefix.
///
/// # Errors
///
/// [`FrameError::Encode`] if the value cannot be serialised, and
/// [`FrameError::TooLarge`] if the result exceeds [`MAX_FRAME_BYTES`] — the
/// limit is enforced on both sides, so this end never puts a frame on the wire
/// that the other end is obliged to hang up on.
pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, FrameError> {
    let payload =
        rmp_serde::to_vec_named(value).map_err(|why| FrameError::Encode(why.to_string()))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge {
            announced: payload.len() as u64,
            limit: MAX_FRAME_BYTES,
        });
    }
    Ok(payload)
}

/// Serialises `value` into a complete frame: length prefix followed by payload.
///
/// # Errors
///
/// As [`encode`].
pub fn encode_frame<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, FrameError> {
    let payload = encode(value)?;
    let mut frame = Vec::with_capacity(LENGTH_PREFIX_BYTES + payload.len());
    // The cast is sound: `encode` has already refused anything above
    // MAX_FRAME_BYTES, which is far below u32::MAX.
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Reads the announced payload length from a frame header.
///
/// This is the guard the "oversized frame closes the connection **without a
/// large allocation**" criterion is about: a reader calls it with the four bytes
/// it has, and only allocates a body buffer if it answers `Ok`.
///
/// # Errors
///
/// [`FrameError::TooLarge`] if the header announces more than
/// [`MAX_FRAME_BYTES`].
pub fn payload_length(header: [u8; LENGTH_PREFIX_BYTES]) -> Result<usize, FrameError> {
    let announced = u32::from_le_bytes(header);
    if announced as usize > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge {
            announced: u64::from(announced),
            limit: MAX_FRAME_BYTES,
        });
    }
    Ok(announced as usize)
}

/// Decodes one payload, after establishing that it is safe to decode.
///
/// # Errors
///
/// [`FrameError::Malformed`] if the bytes are not one well-formed MessagePack
/// value, [`FrameError::TooDeep`] if they nest past [`MAX_NESTING_DEPTH`], and
/// [`FrameError::Decode`] if they are well-formed but not this message.
pub fn decode<T: DeserializeOwned>(payload: &[u8]) -> Result<T, FrameError> {
    match depth_of(payload, MAX_NESTING_DEPTH) {
        Ok(_) => {}
        Err(TooDeepOr::TooDeep) => {
            return Err(FrameError::TooDeep {
                limit: MAX_NESTING_DEPTH,
            });
        }
        Err(TooDeepOr::Fault(fault)) => return Err(FrameError::Malformed(fault)),
    }
    rmp_serde::from_slice(payload).map_err(|why| FrameError::Decode(why.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        FrameError, LENGTH_PREFIX_BYTES, MAX_FRAME_BYTES, MAX_NESTING_DEPTH, decode, encode,
        encode_frame, payload_length,
    };
    use crate::scan::ScanFault;
    use prism_domain::{
        Command, Delta, JsonPatchOp, JsonValue, NoticeLevel, PlaybackTarget, SelectionMode,
    };

    fn notice() -> Delta {
        Delta::Notice {
            level: NoticeLevel::Warn,
            message: "the recovery copy outlived its show".to_owned(),
        }
    }

    #[test]
    fn a_frame_is_a_little_endian_length_and_then_the_payload() {
        let payload = encode(&notice()).unwrap();
        let frame = encode_frame(&notice()).unwrap();

        assert_eq!(frame.len(), LENGTH_PREFIX_BYTES + payload.len());
        assert_eq!(
            frame[..LENGTH_PREFIX_BYTES],
            (payload.len() as u32).to_le_bytes()
        );
        assert_eq!(&frame[LENGTH_PREFIX_BYTES..], &payload[..]);

        let header: [u8; LENGTH_PREFIX_BYTES] = frame[..LENGTH_PREFIX_BYTES].try_into().unwrap();
        assert_eq!(payload_length(header), Ok(payload.len()));
    }

    /// The S1 finding, on the wire: internal tagging needs named fields.
    ///
    /// **Asserted on the bytes, and it has to be.** `rmp-serde`'s deserialiser
    /// accepts the compact array encoding as well and reads it back correctly,
    /// so a Rust round trip cannot tell the two apart — a test that encoded
    /// compactly and decoded happily would pass while the UI could not read a
    /// single message. The other end of this wire is TypeScript reading
    /// `ui/src/bindings/`, where `Command` is an object with a `t`.
    #[test]
    fn the_payload_is_a_map_carrying_the_tag_and_the_field_names() {
        let command = Command::ExecutorOff {
            target: PlaybackTarget::of_executor(prism_domain::ExecutorId::new(3)),
        };
        let payload = encode(&command).unwrap();

        // fixmap of two pairs: "t" and "executorId".
        assert_eq!(payload[0], 0x82, "the payload is not a two-entry map");
        assert!(contains(&payload, b"t"));
        assert!(contains(&payload, b"executorId"));
        assert_eq!(decode::<Command>(&payload).unwrap(), command);

        // What `to_vec` would have written instead: an array of two values, with
        // no key for the tag and no name for the field. Rust reads it back;
        // nothing else can.
        let compact = rmp_serde::to_vec(&command).unwrap();
        assert_eq!(compact[0], 0x92, "the compact encoding is an array");
        assert!(!contains(&compact, b"executorId"));
    }

    /// Whether `needle` appears in `haystack`.
    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn a_header_announcing_more_than_the_limit_is_refused() {
        let header = (MAX_FRAME_BYTES as u32 + 1).to_le_bytes();
        assert_eq!(
            payload_length(header),
            Err(FrameError::TooLarge {
                announced: MAX_FRAME_BYTES as u64 + 1,
                limit: MAX_FRAME_BYTES,
            })
        );
        assert_eq!(
            payload_length(u32::MAX.to_le_bytes()),
            Err(FrameError::TooLarge {
                announced: u64::from(u32::MAX),
                limit: MAX_FRAME_BYTES,
            })
        );
        // Exactly at the limit is legal. The limit is a maximum, not a barrier.
        assert_eq!(
            payload_length((MAX_FRAME_BYTES as u32).to_le_bytes()),
            Ok(MAX_FRAME_BYTES)
        );
        assert_eq!(payload_length([0, 0, 0, 0]), Ok(0));
    }

    #[test]
    fn a_message_larger_than_the_limit_is_not_written_either() {
        // The limit is enforced on both sides, so a daemon with an enormous show
        // finds out here rather than by being hung up on.
        let huge = Delta::Notice {
            level: NoticeLevel::Info,
            message: "x".repeat(MAX_FRAME_BYTES + 1),
        };
        assert!(matches!(
            encode(&huge),
            Err(FrameError::TooLarge { limit, .. }) if limit == MAX_FRAME_BYTES
        ));
        assert!(encode_frame(&huge).is_err());
    }

    #[test]
    fn a_payload_nesting_past_the_limit_is_refused_before_serde_sees_it() {
        let mut deep = vec![0x91_u8; MAX_NESTING_DEPTH + 1];
        deep.push(0xc0);
        assert_eq!(
            decode::<JsonValue>(&deep),
            Err(FrameError::TooDeep {
                limit: MAX_NESTING_DEPTH
            })
        );

        // One level shallower is a value, and decodes.
        let mut allowed = vec![0x91_u8; MAX_NESTING_DEPTH];
        allowed.push(0xc0);
        assert!(decode::<JsonValue>(&allowed).is_ok());
    }

    /// The payload the depth limit exists for, in the shape it would really
    /// arrive in: a delta carrying a `JsonValue`.
    #[test]
    fn a_hostile_delta_is_refused_rather_than_decoded() {
        // {"t": "ShowPatch", "ops": [{"op": "add", "path": "/x", "value": <deep>}]}
        let mut deep = vec![0x91_u8; 50_000];
        deep.push(0xc0);
        let mut payload = rmp_serde::to_vec_named(&Delta::ShowPatch {
            ops: vec![JsonPatchOp::Add {
                path: "/x".to_owned(),
                value: JsonValue::Null,
            }],
        })
        .unwrap();
        // Replace the trailing `nil` that encodes JsonValue::Null with the deep
        // value, so the frame is exactly the message with a hostile leaf.
        assert_eq!(payload.pop(), Some(0xc0));
        payload.extend_from_slice(&deep);

        assert_eq!(
            decode::<Delta>(&payload),
            Err(FrameError::TooDeep {
                limit: MAX_NESTING_DEPTH
            })
        );
    }

    #[test]
    fn a_malformed_payload_names_what_was_wrong_with_it() {
        assert_eq!(
            decode::<Command>(&[]),
            Err(FrameError::Malformed(ScanFault::UnexpectedEnd))
        );
        assert_eq!(
            decode::<Command>(&[0xc1]),
            Err(FrameError::Malformed(ScanFault::ReservedByte))
        );
        let mut two = encode(&Command::ClearProgrammer).unwrap();
        two.extend_from_slice(&encode(&Command::ClearProgrammer).unwrap());
        assert_eq!(
            decode::<Command>(&two),
            Err(FrameError::Malformed(ScanFault::TrailingBytes))
        );
    }

    /// The line between closing the connection and answering the peer, and it
    /// is about the framing rather than about how bad the message was.
    #[test]
    fn only_an_oversized_frame_loses_the_frame_boundary() {
        let payload = encode(&Command::ClearProgrammer).unwrap();
        let wrong = decode::<Delta>(&payload).unwrap_err();
        assert!(matches!(wrong, FrameError::Decode(_)));
        assert!(!wrong.loses_the_frame_boundary());

        assert!(
            FrameError::TooLarge {
                announced: 1,
                limit: 0
            }
            .loses_the_frame_boundary()
        );
        for intact in [
            FrameError::TooDeep { limit: 1 },
            FrameError::Malformed(ScanFault::TrailingBytes),
            FrameError::Encode(String::new()),
        ] {
            assert!(
                !intact.loses_the_frame_boundary(),
                "{intact:?} concerns a payload the framing already delimited"
            );
        }
    }

    /// S1: serialisation is fallible, and the reason is a value the domain
    /// refuses rather than a codec that ran out of memory.
    #[test]
    fn a_non_finite_float_is_refused_at_the_wire() {
        let broken = JsonValue::Float(f64::INFINITY);
        assert!(matches!(encode(&broken), Err(FrameError::Encode(_))));
        assert!(matches!(
            encode(&JsonValue::Float(f64::NAN)),
            Err(FrameError::Encode(_))
        ));
        assert!(encode(&JsonValue::Float(1.5)).is_ok());
    }

    #[test]
    fn every_error_says_something_a_person_could_act_on() {
        let messages = [
            FrameError::Encode("nan".to_owned()).to_string(),
            FrameError::Decode("missing field".to_owned()).to_string(),
            FrameError::TooLarge {
                announced: 4_294_967_295,
                limit: MAX_FRAME_BYTES,
            }
            .to_string(),
            FrameError::TooDeep { limit: 128 }.to_string(),
            FrameError::Malformed(ScanFault::UnexpectedEnd).to_string(),
            FrameError::Malformed(ScanFault::ReservedByte).to_string(),
            FrameError::Malformed(ScanFault::TrailingBytes).to_string(),
            FrameError::Malformed(ScanFault::LengthOverflow).to_string(),
        ];
        for message in &messages {
            assert!(message.len() > 20, "{message:?} is not an explanation");
        }
    }

    #[test]
    fn a_command_survives_the_frame_it_travels_in() {
        let command = Command::SelectFixtures {
            ids: vec![prism_domain::FixtureId::new(7)],
            mode: SelectionMode::Toggle,
        };
        let frame = encode_frame(&command).unwrap();
        let header: [u8; LENGTH_PREFIX_BYTES] = frame[..LENGTH_PREFIX_BYTES].try_into().unwrap();
        let length = payload_length(header).unwrap();
        let decoded: Command = decode(&frame[LENGTH_PREFIX_BYTES..][..length]).unwrap();
        assert_eq!(decoded, command);
    }
}
