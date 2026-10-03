//! PosiStageNet - the packets a tracking system sends, read and written - **S32**.
//!
//! PSN (VYV and MA Lighting, version 2.03) streams the position of tracked
//! objects over UDP multicast, by default to `236.10.10.10:56565`, at whatever
//! rate the tracker has. This module is the **codec and nothing else**: bytes in,
//! events out, and the reverse for the tests and for anybody who wants to be a
//! tracker. Sockets are [`TrackerReceiver`](crate::TrackerReceiver)'s, and what a position *means* is the
//! tick's.
//!
//! # The format, as this reads it
//!
//! Little-endian throughout, and built of **chunks**. A chunk is a sixteen-bit
//! id, then sixteen bits whose low fifteen are the length of its data and whose
//! top bit says the data is itself a run of chunks:
//!
//! ```text
//! 0x6755 PSN_DATA_PACKET
//!   0x0000 header      u64 timestamp, u8 version high, u8 version low,
//!                      u8 frame id, u8 packet count of this frame
//!   0x0001 tracker list
//!     <tracker id>     one chunk per tracker, its id being the tracker's number
//!       0x0000 position       f32 x, y, z
//!       0x0001 speed          f32 x, y, z
//!       0x0002 orientation    f32 x, y, z
//!       0x0003 status         f32
//!       0x0004 acceleration   f32 x, y, z
//!       0x0005 target         f32 x, y, z
//!       0x0006 timestamp      u64
//!
//! 0x6756 PSN_INFO_PACKET
//!   0x0000 header      as above
//!   0x0001 system name  text
//!   0x0002 tracker list
//!     <tracker id>
//!       0x0000 name     text
//! ```
//!
//! A frame that does not fit in one datagram is several, each a valid packet
//! with the same frame id; this reads each as it comes and has no use for the
//! count. Chunks this does not know are **skipped by their length** - that is
//! what the length is for, and what lets a later version of the format add
//! chunks without a desk that predates it dropping the packet.
//!
//! # It never panics and it never allocates
//!
//! The bytes are a network's: another program's, on a multicast group anyone can
//! write to. [`decode`] reads with bounds-checked accessors only, takes a
//! closure for what it finds rather than building a list, and answers with
//! [`Malformed`] for a packet it cannot make sense of - which the receiver
//! counts and goes on from. `tests/psn_fuzz.rs` throws random and
//! structure-aware garbage at it and measures the allocator.
//!
//! # Units, and axes
//!
//! The specification says *position* and does not say *metres*, or which way is
//! up: tracking systems differ, and MA's own consoles ask the operator to map
//! the axes. So what comes out of here is **raw**, exactly as sent, and
//! [`prism_domain::TrackerMapping`] is where an installer says what their system
//! means.

use std::net::Ipv4Addr;

/// The multicast group a PSN system sends to unless told otherwise.
pub const GROUP: Ipv4Addr = Ipv4Addr::new(236, 10, 10, 10);

/// The UDP port a PSN system sends to unless told otherwise.
pub const PORT: u16 = 56_565;

/// The datagram size a receiver should be ready for.
///
/// A PSN packet is built to fit a standard Ethernet frame (1 500 bytes less the
/// IP and UDP headers), and the reference implementations read into 1 500.
pub const MAX_PACKET: usize = 1_500;

/// Root chunk of a data packet.
pub const DATA_PACKET: u16 = 0x6755;
/// Root chunk of an info packet.
pub const INFO_PACKET: u16 = 0x6756;

const HEADER: u16 = 0x0000;
const DATA_TRACKER_LIST: u16 = 0x0001;
const INFO_SYSTEM_NAME: u16 = 0x0001;
const INFO_TRACKER_LIST: u16 = 0x0002;
const TRACKER_POSITION: u16 = 0x0000;
const TRACKER_NAME: u16 = 0x0000;

/// Why a packet could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Malformed {
    /// The first chunk is not a PSN root. Anything else on the group - or PSN
    /// version 1, which has other roots and which this does not read.
    NotPsn,
    /// A chunk claims more bytes than the packet has left.
    Truncated,
}

/// One thing a packet says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event<'a> {
    /// Where a tracker is, **raw**: three numbers in the system's own units and
    /// axes.
    Position {
        /// The tracker's number.
        tracker: u16,
        /// `x`, `y`, `z`, as sent.
        position: [f32; 3],
    },
    /// What the sending system calls itself.
    System(&'a str),
    /// What a tracker is called. Lossy where the bytes are not UTF-8: a name is
    /// for a person to read, and a replacement character is a better answer to
    /// a badly encoded one than a refusal.
    Name {
        /// The tracker's number.
        tracker: u16,
        /// Its name.
        name: &'a str,
    },
}

/// One chunk: its id and the bytes of its data, and whatever follows it.
///
/// A chunk that claims more than the packet has left is **clamped to what is
/// there** and `damaged` is set, so everything the packet said *before* the
/// damage can still be read - a position that arrived whole is not made worse by
/// the neighbour that did not. `None` when fewer than a header's four bytes are
/// left, which is damage unless there was nothing left at all.
fn chunk<'a>(bytes: &'a [u8], damaged: &mut bool) -> Option<(u16, &'a [u8], &'a [u8])> {
    if bytes.is_empty() {
        return None;
    }
    let (Some(id), Some(field)) = (u16_at(bytes, 0), u16_at(bytes, 2)) else {
        *damaged = true;
        return None;
    };
    let length = usize::from(field & 0x7FFF);
    let body = bytes.get(4..)?;
    if length > body.len() {
        *damaged = true;
        return Some((id, body, &[]));
    }
    Some((id, body.get(..length)?, body.get(length..)?))
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let pair = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([*pair.first()?, *pair.get(1)?]))
}

fn f32_at(bytes: &[u8], at: usize) -> Option<f32> {
    let quad = bytes.get(at..at.checked_add(4)?)?;
    Some(f32::from_le_bytes([
        *quad.first()?,
        *quad.get(1)?,
        *quad.get(2)?,
        *quad.get(3)?,
    ]))
}

/// Reads one datagram, calling `found` for each position, name or system name
/// in it.
///
/// A **data packet** yields [`Event::Position`] for every tracker that has a
/// position chunk; an **info packet** yields [`Event::System`] and an
/// [`Event::Name`] for each named tracker. Anything else in a packet - speed,
/// orientation, acceleration, a status word, a version this does not know - is
/// skipped.
///
/// A packet that is damaged part of the way through has still said whatever came
/// before the damage, and `found` has been called for it: the error is the
/// *rest*.
///
/// # Errors
///
/// [`Malformed::NotPsn`] for a datagram whose root is not one of PSN's - decided
/// from the two bytes of the id alone, so a text line or another protocol is
/// *not PSN* whatever its next two bytes happen to claim - and
/// [`Malformed::Truncated`] for one whose chunks run past its end.
pub fn decode<'a>(datagram: &'a [u8], found: &mut impl FnMut(Event<'a>)) -> Result<(), Malformed> {
    let root = u16_at(datagram, 0).ok_or(Malformed::Truncated)?;
    if root != DATA_PACKET && root != INFO_PACKET {
        return Err(Malformed::NotPsn);
    }
    let mut damaged = false;
    let Some((_, body, _)) = chunk(datagram, &mut damaged) else {
        return Err(Malformed::Truncated);
    };
    if root == DATA_PACKET {
        decode_data(body, &mut damaged, found);
    } else {
        decode_info(body, &mut damaged, found);
    }
    if damaged {
        Err(Malformed::Truncated)
    } else {
        Ok(())
    }
}

fn decode_data<'a>(mut body: &'a [u8], damaged: &mut bool, found: &mut impl FnMut(Event<'a>)) {
    while let Some((id, data, rest)) = chunk(body, damaged) {
        body = rest;
        if id != DATA_TRACKER_LIST {
            continue;
        }
        let mut trackers = data;
        while let Some((tracker, fields, after)) = chunk(trackers, damaged) {
            trackers = after;
            let mut fields = fields;
            while let Some((field, value, after)) = chunk(fields, damaged) {
                fields = after;
                if field == TRACKER_POSITION
                    && let (Some(x), Some(y), Some(z)) =
                        (f32_at(value, 0), f32_at(value, 4), f32_at(value, 8))
                {
                    found(Event::Position {
                        tracker,
                        position: [x, y, z],
                    });
                }
            }
        }
    }
}

fn decode_info<'a>(mut body: &'a [u8], damaged: &mut bool, found: &mut impl FnMut(Event<'a>)) {
    while let Some((id, data, rest)) = chunk(body, damaged) {
        body = rest;
        match id {
            INFO_SYSTEM_NAME => {
                if let Ok(name) = core::str::from_utf8(data) {
                    found(Event::System(name));
                }
            }
            INFO_TRACKER_LIST => {
                let mut trackers = data;
                while let Some((tracker, fields, after)) = chunk(trackers, damaged) {
                    trackers = after;
                    let mut fields = fields;
                    while let Some((field, value, after)) = chunk(fields, damaged) {
                        fields = after;
                        if field == TRACKER_NAME
                            && let Ok(name) = core::str::from_utf8(value)
                        {
                            found(Event::Name { tracker, name });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// ---- writing -----------------------------------------------------------------

/// A chunk header: the id, then the length with the sub-chunk flag.
fn header(out: &mut Vec<u8>, id: u16, length: usize, subchunks: bool) {
    out.extend_from_slice(&id.to_le_bytes());
    // Only fifteen bits of length exist; a packet that needs more is not one
    // datagram, and nothing here writes one.
    let field = (u16::try_from(length).unwrap_or(0x7FFF) & 0x7FFF) | (u16::from(subchunks) << 15);
    out.extend_from_slice(&field.to_le_bytes());
}

fn frame_header(out: &mut Vec<u8>, timestamp: u64, frame: u8) {
    header(out, HEADER, 12, false);
    out.extend_from_slice(&timestamp.to_le_bytes());
    // Version 2.03, one packet in this frame.
    out.extend_from_slice(&[2, 3, frame, 1]);
}

/// Writes a data packet with one position chunk per tracker.
///
/// For the tests - a tracker to point the receiver at - and for anybody who
/// wants to move a light with a script. Speed, orientation and the rest are not
/// written: a receiver that needs them is not this one.
#[must_use]
pub fn encode_data(timestamp: u64, frame: u8, trackers: &[(u16, [f32; 3])]) -> Vec<u8> {
    // Each tracker: its chunk header (4) + the position chunk header (4) + 12.
    const TRACKER: usize = 4 + 4 + 12;
    let list = trackers.len() * TRACKER;
    let mut out = Vec::with_capacity(4 + 16 + 4 + list);
    header(&mut out, DATA_PACKET, 16 + 4 + list, true);
    frame_header(&mut out, timestamp, frame);
    header(&mut out, DATA_TRACKER_LIST, list, true);
    for &(tracker, [x, y, z]) in trackers {
        header(&mut out, tracker, 4 + 12, true);
        header(&mut out, TRACKER_POSITION, 12, false);
        for value in [x, y, z] {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out
}

/// Writes an info packet naming the system and its trackers.
#[must_use]
pub fn encode_info(timestamp: u64, frame: u8, system: &str, trackers: &[(u16, &str)]) -> Vec<u8> {
    let list: usize = trackers.iter().map(|(_, name)| 4 + 4 + name.len()).sum();
    let mut out = Vec::new();
    header(
        &mut out,
        INFO_PACKET,
        16 + 4 + system.len() + 4 + list,
        true,
    );
    frame_header(&mut out, timestamp, frame);
    header(&mut out, INFO_SYSTEM_NAME, system.len(), false);
    out.extend_from_slice(system.as_bytes());
    header(&mut out, INFO_TRACKER_LIST, list, true);
    for &(tracker, name) in trackers {
        header(&mut out, tracker, 4 + name.len(), true);
        header(&mut out, TRACKER_NAME, name.len(), false);
        out.extend_from_slice(name.as_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        DATA_PACKET, Event, GROUP, INFO_PACKET, Malformed, PORT, decode, encode_data, encode_info,
    };

    fn events(datagram: &[u8]) -> (Vec<String>, Result<(), Malformed>) {
        let mut seen = Vec::new();
        let result = decode(datagram, &mut |event| seen.push(format!("{event:?}")));
        (seen, result)
    }

    #[test]
    fn the_defaults_are_the_published_ones() {
        assert_eq!(GROUP.octets(), [236, 10, 10, 10]);
        assert_eq!(PORT, 56_565);
    }

    #[test]
    fn a_data_packet_gives_each_trackers_position() {
        let packet = encode_data(7, 1, &[(0, [1.0, 2.0, 3.0]), (5, [-4.5, 0.25, 9.0])]);
        let mut found = Vec::new();
        decode(&packet, &mut |event| found.push(event)).unwrap();
        assert_eq!(
            found,
            vec![
                Event::Position {
                    tracker: 0,
                    position: [1.0, 2.0, 3.0]
                },
                Event::Position {
                    tracker: 5,
                    position: [-4.5, 0.25, 9.0]
                },
            ]
        );
    }

    #[test]
    fn an_info_packet_gives_the_system_and_the_names() {
        let packet = encode_info(1, 0, "OpenFollow", &[(1, "Anna"), (2, "Ben")]);
        let mut found = Vec::new();
        decode(&packet, &mut |event| found.push(event)).unwrap();
        assert_eq!(
            found,
            vec![
                Event::System("OpenFollow"),
                Event::Name {
                    tracker: 1,
                    name: "Anna"
                },
                Event::Name {
                    tracker: 2,
                    name: "Ben"
                },
            ]
        );
    }

    /// The layout a real tracker sends, written out byte by byte from the
    /// specification and not through `encode_data` - so the encoder and the
    /// decoder cannot be wrong in the same direction.
    #[test]
    fn a_packet_laid_out_by_hand_reads() {
        let mut packet: Vec<u8> = Vec::new();
        // Root: a data packet whose chunks are 16 + 24 = 40 bytes, and which
        // holds chunks (the top bit of the length field).
        packet.extend([0x55, 0x67, 40, 0x80]);
        // Header chunk: id 0, 12 bytes of data.
        packet.extend([0, 0, 12, 0x00]);
        packet.extend(0x0102_0304_0506_0708_u64.to_le_bytes());
        packet.extend([2, 3, 9, 1]);
        // Tracker list: id 1, holding one tracker chunk of 4 + 16 = 20 bytes.
        packet.extend([1, 0, 20, 0x80]);
        // Tracker 0x0102, holding one position chunk of 4 + 12 = 16 bytes.
        packet.extend([0x02, 0x01, 16, 0x80]);
        packet.extend([0, 0, 12, 0x00]);
        for value in [10.0_f32, 20.0, 30.0] {
            packet.extend(value.to_le_bytes());
        }
        let (seen, result) = events(&packet);
        assert_eq!(result, Ok(()));
        assert_eq!(
            seen,
            vec!["Position { tracker: 258, position: [10.0, 20.0, 30.0] }"]
        );
        assert_eq!(packet.get(..2), Some(&DATA_PACKET.to_le_bytes()[..]));
        // And the encoder writes the very same bytes.
        assert_eq!(
            encode_data(0x0102_0304_0506_0708, 9, &[(0x0102, [10.0, 20.0, 30.0])]),
            packet
        );
    }

    #[test]
    fn chunks_it_does_not_know_are_skipped_by_their_length() {
        let mut packet = encode_data(0, 0, &[(3, [1.0, 1.0, 1.0])]);
        // Splice an unknown chunk (id 0x00AA, four bytes) in front of the
        // tracker list and grow the root's length by eight.
        let at = 4 + 16;
        let unknown = [0xAA, 0x00, 0x04, 0x00, 1, 2, 3, 4];
        packet.splice(at..at, unknown);
        let grown = u16::from_le_bytes([packet[2], packet[3] & 0x7F]) + 8;
        packet[2] = (grown & 0xFF) as u8;
        packet[3] = ((grown >> 8) as u8 & 0x7F) | 0x80;
        let (seen, result) = events(&packet);
        assert_eq!(result, Ok(()));
        assert_eq!(seen.len(), 1);
    }

    #[test]
    fn something_that_is_not_psn_is_said_so() {
        assert_eq!(events(b"hello, lighting network").1, Err(Malformed::NotPsn));
        // PSN version 1's roots are not read.
        assert_eq!(
            events(&[0x54, 0x67, 0, 0]).1,
            Err(Malformed::NotPsn),
            "v1 data"
        );
    }

    #[test]
    fn a_packet_that_ends_early_is_truncated_and_keeps_what_came_first() {
        let packet = encode_data(0, 0, &[(1, [1.0, 2.0, 3.0]), (2, [4.0, 5.0, 6.0])]);
        // Cut the second tracker's position in half.
        let cut = &packet[..packet.len() - 6];
        let mut found = Vec::new();
        let result = decode(cut, &mut |event| found.push(event));
        assert_eq!(result, Err(Malformed::Truncated));
        assert_eq!(found.len(), 1, "the first tracker arrived whole");
    }

    #[test]
    fn nothing_at_all_is_truncated() {
        assert_eq!(events(&[]).1, Err(Malformed::Truncated));
        assert_eq!(events(&[0x55]).1, Err(Malformed::Truncated));
        assert_eq!(
            events(&[0x55, 0x67, 0xFF, 0xFF]).1,
            Err(Malformed::Truncated)
        );
    }

    #[test]
    fn a_position_chunk_that_is_too_short_is_not_a_position() {
        // One tracker whose position chunk holds eight bytes: two floats.
        let mut packet = Vec::new();
        packet.extend([0x55, 0x67, 20, 0x80]);
        packet.extend([1, 0, 16, 0x80]);
        packet.extend([7, 0, 12, 0x80]);
        packet.extend([0, 0, 8, 0]);
        packet.extend([0u8; 8]);
        let (seen, result) = events(&packet);
        assert_eq!(result, Ok(()));
        assert!(seen.is_empty(), "{seen:?}");
    }

    #[test]
    fn the_info_root_is_the_published_number() {
        assert_eq!(INFO_PACKET, 0x6756);
    }
}
