//! A non-recursive walk over a MessagePack payload, run before `serde` sees it.
//!
//! # Why this exists
//!
//! `prism_domain::JsonValue` is recursive, and `serde` buffers the content of an
//! internally tagged enum before any domain code runs — so by the time a
//! `Delta::ShowPatch` is being constructed, the nesting has already been walked
//! by the deserialiser's own stack. `serde_json` guards this with a depth limit
//! of 128 levels. MessagePack has no such limit, and a payload of a few kilobytes
//! can nest a hundred thousand deep.
//!
//! A stack overflow is not a recoverable error in Rust: it aborts the process.
//! In `prismd` that means the DMX output goes with it, which is the one thing
//! `CLAUDE.md` says must never happen. Twelve bytes from an unauthenticated
//! socket must not be able to do that.
//!
//! # Why a scan rather than a limited deserialiser
//!
//! The obvious alternative is a `Deserializer` wrapper that counts levels as it
//! goes. It does not work here: the recursion that has to be stopped happens
//! *inside* `serde`'s own buffering of an internally tagged enum, one layer below
//! anywhere a wrapper could count.
//!
//! So the depth is established before decoding starts, by walking the bytes with
//! an explicit stack — which cannot itself overflow, whatever it is given. The
//! walk is linear over a payload that is already bounded by
//! [`crate::MAX_FRAME_BYTES`], and it doubles as a structural check: a truncated
//! frame, a reserved byte and trailing rubbish are all found here rather than
//! halfway through building a value.

use core::fmt;

/// What was wrong with the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanFault {
    /// The payload ends in the middle of a value.
    UnexpectedEnd,
    /// `0xc1`, which the MessagePack specification never assigns.
    ReservedByte,
    /// A complete value, and then more bytes. One frame carries one message.
    TrailingBytes,
    /// A declared string, binary or container length that cannot be addressed
    /// on this machine.
    LengthOverflow,
}

impl fmt::Display for ScanFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => f.write_str("the payload ends inside a value"),
            Self::ReservedByte => f.write_str("the payload contains the reserved byte 0xc1"),
            Self::TrailingBytes => f.write_str("the payload holds more than one value"),
            Self::LengthOverflow => f.write_str("the payload declares an unrepresentable length"),
        }
    }
}

impl core::error::Error for ScanFault {}

/// The outcome of one value header: how many bytes to skip, or how many values
/// the container that just opened holds.
enum Head {
    /// A scalar, plus the number of payload bytes that follow its header.
    Scalar(usize),
    /// An array or map, and the number of *values* inside it. A map of `n`
    /// pairs holds `2n` values.
    Container(usize),
}

/// A read cursor that never reads past the end.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn byte(&mut self) -> Result<u8, ScanFault> {
        let byte = *self.bytes.get(self.pos).ok_or(ScanFault::UnexpectedEnd)?;
        self.pos += 1;
        Ok(byte)
    }

    /// A big-endian unsigned integer of `width` bytes, as MessagePack writes
    /// every length.
    fn length(&mut self, width: usize) -> Result<usize, ScanFault> {
        let mut value: u64 = 0;
        for _ in 0..width {
            value = (value << 8) | u64::from(self.byte()?);
        }
        usize::try_from(value).map_err(|_| ScanFault::LengthOverflow)
    }

    fn skip(&mut self, count: usize) -> Result<(), ScanFault> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or(ScanFault::LengthOverflow)?;
        if end > self.bytes.len() {
            return Err(ScanFault::UnexpectedEnd);
        }
        self.pos = end;
        Ok(())
    }

    /// Reads one value header and reports what it opened.
    fn head(&mut self) -> Result<Head, ScanFault> {
        let byte = self.byte()?;
        let head = match byte {
            // Positive and negative fixint, nil, false, true: the header is the
            // whole value.
            0x00..=0x7f | 0xc0 | 0xc2 | 0xc3 | 0xe0..=0xff => Head::Scalar(0),
            0x80..=0x8f => Head::Container(pairs(usize::from(byte & 0x0f))?),
            0x90..=0x9f => Head::Container(usize::from(byte & 0x0f)),
            0xa0..=0xbf => Head::Scalar(usize::from(byte & 0x1f)),
            0xc1 => return Err(ScanFault::ReservedByte),
            0xc4 => Head::Scalar(self.length(1)?),
            0xc5 => Head::Scalar(self.length(2)?),
            0xc6 => Head::Scalar(self.length(4)?),
            // The extension types carry a one-byte type tag ahead of the data.
            0xc7 => Head::Scalar(self.length(1)?.saturating_add(1)),
            0xc8 => Head::Scalar(self.length(2)?.saturating_add(1)),
            0xc9 => Head::Scalar(self.length(4)?.saturating_add(1)),
            0xca => Head::Scalar(4),
            0xcb => Head::Scalar(8),
            0xcc | 0xd0 => Head::Scalar(1),
            0xcd | 0xd1 => Head::Scalar(2),
            0xce | 0xd2 => Head::Scalar(4),
            0xcf | 0xd3 => Head::Scalar(8),
            0xd4 => Head::Scalar(2),
            0xd5 => Head::Scalar(3),
            0xd6 => Head::Scalar(5),
            0xd7 => Head::Scalar(9),
            0xd8 => Head::Scalar(17),
            0xd9 => Head::Scalar(self.length(1)?),
            0xda => Head::Scalar(self.length(2)?),
            0xdb => Head::Scalar(self.length(4)?),
            0xdc => Head::Container(self.length(2)?),
            0xdd => Head::Container(self.length(4)?),
            0xde => Head::Container(pairs(self.length(2)?)?),
            0xdf => Head::Container(pairs(self.length(4)?)?),
        };
        Ok(head)
    }
}

/// A map of `n` pairs holds `2n` values.
fn pairs(count: usize) -> Result<usize, ScanFault> {
    count.checked_mul(2).ok_or(ScanFault::LengthOverflow)
}

/// Walks `payload` and reports the deepest container nesting it reaches.
///
/// Returns [`ScanFault`] if the bytes are not exactly one well-formed
/// MessagePack value. The walk uses an explicit stack, so it is bounded by the
/// payload's own length and cannot overflow the machine stack however deeply the
/// payload nests — which is the whole point of doing it here rather than in a
/// deserialiser.
///
/// `limit` bounds the stack this function keeps: a payload deeper than the limit
/// is refused as soon as the limit is crossed, so a hostile frame is rejected
/// after a few bytes of work rather than after walking all of it.
pub fn depth_of(payload: &[u8], limit: usize) -> Result<usize, TooDeepOr> {
    let mut cursor = Cursor::new(payload);
    // One entry per open container, holding how many values are still expected
    // in the level above it.
    let mut stack: Vec<usize> = Vec::new();
    let mut remaining: usize = 1;
    let mut deepest: usize = 0;

    loop {
        while remaining == 0 {
            match stack.pop() {
                Some(outer) => remaining = outer,
                None => {
                    return if cursor.pos == payload.len() {
                        Ok(deepest)
                    } else {
                        Err(TooDeepOr::Fault(ScanFault::TrailingBytes))
                    };
                }
            }
        }
        remaining -= 1;

        match cursor.head().map_err(TooDeepOr::Fault)? {
            Head::Scalar(skip) => cursor.skip(skip).map_err(TooDeepOr::Fault)?,
            Head::Container(count) => {
                if stack.len() + 1 > limit {
                    return Err(TooDeepOr::TooDeep);
                }
                stack.push(remaining);
                remaining = count;
                deepest = deepest.max(stack.len());
            }
        }
    }
}

/// Either the payload was malformed, or it nested past the limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooDeepOr {
    /// Nesting exceeded the limit the scan was given.
    TooDeep,
    /// The bytes are not one well-formed MessagePack value.
    Fault(ScanFault),
}

#[cfg(test)]
mod tests {
    use super::{ScanFault, TooDeepOr, depth_of};

    /// Encoded the way the wire does it, so the scanner is exercised against
    /// real output rather than bytes written by hand.
    fn packed<T: serde::Serialize>(value: &T) -> Vec<u8> {
        rmp_serde::to_vec_named(value).unwrap()
    }

    #[test]
    fn a_scalar_has_no_depth() {
        assert_eq!(depth_of(&packed(&7_u8), 8), Ok(0));
        assert_eq!(depth_of(&packed(&"hello"), 8), Ok(0));
        assert_eq!(depth_of(&packed(&()), 8), Ok(0));
        assert_eq!(depth_of(&packed(&core::f64::consts::PI), 8), Ok(0));
    }

    #[test]
    fn a_container_is_one_level() {
        assert_eq!(depth_of(&packed(&vec![1_u8, 2, 3]), 8), Ok(1));
        assert_eq!(depth_of(&packed(&vec![vec![1_u8]]), 8), Ok(2));
        assert_eq!(depth_of(&packed(&Vec::<u8>::new()), 8), Ok(1));
    }

    #[test]
    fn the_deepest_branch_is_the_depth() {
        // A shallow branch beside a deep one must not lower the answer.
        let value = (vec![1_u8], vec![vec![vec![1_u8]]]);
        assert_eq!(depth_of(&packed(&value), 8), Ok(4));
    }

    #[test]
    fn nesting_past_the_limit_is_refused() {
        let deep = nest(200);
        assert_eq!(depth_of(&deep, 128), Err(TooDeepOr::TooDeep));
        assert_eq!(depth_of(&deep, 200), Ok(200));
        assert_eq!(depth_of(&nest(128), 128), Ok(128));
        assert_eq!(depth_of(&nest(129), 128), Err(TooDeepOr::TooDeep));
    }

    /// The payload this whole module exists for: cheap to write, and deep enough
    /// that decoding it would exhaust the stack.
    #[test]
    fn a_hundred_thousand_levels_cost_a_hundred_thousand_bytes() {
        let deep = nest(100_000);
        assert_eq!(deep.len(), 100_001);
        assert_eq!(depth_of(&deep, 128), Err(TooDeepOr::TooDeep));
    }

    #[test]
    fn a_truncated_value_is_not_a_depth_question() {
        assert_eq!(
            depth_of(&[], 8),
            Err(TooDeepOr::Fault(ScanFault::UnexpectedEnd))
        );
        // A fixstr of five bytes, with three of them missing.
        assert_eq!(
            depth_of(&[0xa5, b'a', b'b'], 8),
            Err(TooDeepOr::Fault(ScanFault::UnexpectedEnd))
        );
        // An array announcing two elements and carrying one.
        assert_eq!(
            depth_of(&[0x92, 0x01], 8),
            Err(TooDeepOr::Fault(ScanFault::UnexpectedEnd))
        );
        // A map announcing one pair and carrying only its key.
        assert_eq!(
            depth_of(&[0x81, 0x01], 8),
            Err(TooDeepOr::Fault(ScanFault::UnexpectedEnd))
        );
    }

    #[test]
    fn one_frame_carries_one_value() {
        assert_eq!(
            depth_of(&[0x01, 0x02], 8),
            Err(TooDeepOr::Fault(ScanFault::TrailingBytes))
        );
    }

    #[test]
    fn the_reserved_byte_is_refused() {
        assert_eq!(
            depth_of(&[0xc1], 8),
            Err(TooDeepOr::Fault(ScanFault::ReservedByte))
        );
    }

    #[test]
    fn a_declared_length_is_not_believed() {
        // str32 announcing four gigabytes, in a five-byte payload.
        assert_eq!(
            depth_of(&[0xdb, 0xff, 0xff, 0xff, 0xff], 8),
            Err(TooDeepOr::Fault(ScanFault::UnexpectedEnd))
        );
        // array32 announcing four billion elements: the walk runs out of bytes
        // rather than out of memory, because nothing is allocated per element.
        assert_eq!(
            depth_of(&[0xdd, 0xff, 0xff, 0xff, 0xff], 8),
            Err(TooDeepOr::Fault(ScanFault::UnexpectedEnd))
        );
    }

    /// Every format family, so a byte that is skipped by the wrong width shows
    /// up as trailing rubbish rather than passing quietly.
    #[test]
    fn every_scalar_width_is_skipped_exactly() {
        let cases: &[&[u8]] = &[
            &[0x7f],                                                       // positive fixint
            &[0xe0],                                                       // negative fixint
            &[0xc0],                                                       // nil
            &[0xc2],                                                       // false
            &[0xc3],                                                       // true
            &[0xcc, 0x01],                                                 // uint 8
            &[0xcd, 0, 1],                                                 // uint 16
            &[0xce, 0, 0, 0, 1],                                           // uint 32
            &[0xcf, 0, 0, 0, 0, 0, 0, 0, 1],                               // uint 64
            &[0xd0, 0x01],                                                 // int 8
            &[0xd1, 0, 1],                                                 // int 16
            &[0xd2, 0, 0, 0, 1],                                           // int 32
            &[0xd3, 0, 0, 0, 0, 0, 0, 0, 1],                               // int 64
            &[0xca, 0, 0, 0, 0],                                           // float 32
            &[0xcb, 0, 0, 0, 0, 0, 0, 0, 0],                               // float 64
            &[0xa1, b'x'],                                                 // fixstr
            &[0xd9, 0x01, b'x'],                                           // str 8
            &[0xda, 0, 1, b'x'],                                           // str 16
            &[0xdb, 0, 0, 0, 1, b'x'],                                     // str 32
            &[0xc4, 0x01, 0x00],                                           // bin 8
            &[0xc5, 0, 1, 0x00],                                           // bin 16
            &[0xc6, 0, 0, 0, 1, 0x00],                                     // bin 32
            &[0xd4, 0x00, 0x00],                                           // fixext 1
            &[0xd5, 0x00, 0, 0],                                           // fixext 2
            &[0xd6, 0x00, 0, 0, 0, 0],                                     // fixext 4
            &[0xd7, 0x00, 0, 0, 0, 0, 0, 0, 0, 0],                         // fixext 8
            &[0xd8, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // fixext 16
            &[0xc7, 0x01, 0x00, 0x00],                                     // ext 8
            &[0xc8, 0, 1, 0x00, 0x00],                                     // ext 16
            &[0xc9, 0, 0, 0, 1, 0x00, 0x00],                               // ext 32
        ];
        for bytes in cases {
            assert_eq!(depth_of(bytes, 8), Ok(0), "{bytes:02x?} was not one value");
        }
    }

    #[test]
    fn the_wide_container_headers_are_read_as_containers() {
        // array16, array32, map16, map32, each holding one element or pair.
        assert_eq!(depth_of(&[0xdc, 0, 1, 0x01], 8), Ok(1));
        assert_eq!(depth_of(&[0xdd, 0, 0, 0, 1, 0x01], 8), Ok(1));
        assert_eq!(depth_of(&[0xde, 0, 1, 0x01, 0x02], 8), Ok(1));
        assert_eq!(depth_of(&[0xdf, 0, 0, 0, 1, 0x01, 0x02], 8), Ok(1));
    }

    /// `n` nested single-element arrays, innermost value `nil`.
    fn nest(levels: usize) -> Vec<u8> {
        let mut bytes = vec![0x91; levels];
        bytes.push(0xc0);
        bytes
    }
}
