//! Where the trackers are - the latest position of each, and nothing else.
//!
//! `ARCHITECTURE_SPEC.md` §8: trackers send at their own rate, typically thirty
//! to sixty times a second, **asynchronously to the tick**. A receiver thread
//! writes here; the tick reads **only the latest** value of each tracker. A
//! stale position is worthless, so nothing is queued, nothing is counted
//! against a budget and nothing can fall behind: a tracker that sends a hundred
//! times between two ticks has one position at the second tick, the newest.
//!
//! # One word per tracker, and why that is the whole design
//!
//! A position is three numbers and a lock-free reader of three words can see
//! half of one update and half of the next. A seqlock repairs that and costs a
//! retry loop on the one thread that must not loop. So a position is **one
//! atomic word**: three signed 21-bit counts of **millimetres**, packed. That is
//! `±1048 m` on every axis - the desk refuses a place further than a
//! kilometre from the origin (`prism_domain::MAX_REACH`), so nothing a stage can
//! hold is out of range - and a resolution of a millimetre, which at ten metres
//! is a hundredth of a degree of aim: finer than a motor.
//!
//! The coordinates are **show space** (metres, Y up, `z` upstage - see
//! `prism_domain::placement`) and already mapped: how a tracking system's axes
//! line up with the stage is the receiver's configuration, applied before the
//! write, so this table and the tick know one frame and nothing about PSN.
//!
//! # What it does not do
//!
//! It has no clock and no timeout. A tracker that goes quiet **keeps its last
//! position here**, which is exactly what the specification asks the desk to
//! do: hold, and say so. *Saying so* is the daemon's, which compares
//! [`TrackerTable::count`] with its own clock - the tick has no use for the
//! answer, and keeping wall-clock time out of it is what keeps a run of it
//! reproducible from the tick index alone.

use prism_domain::{MAX_TRACKER, Vec3};

use crate::sync::{AtomicU64, Ordering};

/// How many trackers the table holds: `0..=`[`MAX_TRACKER`].
pub const TRACKERS: usize = MAX_TRACKER as usize + 1;

/// Bits one coordinate takes in the packed word.
const BITS: u32 = 21;
/// The mask of one coordinate.
const MASK: u64 = (1 << BITS) - 1;
/// The largest count of millimetres one coordinate holds, `2^20 - 1`.
const LIMIT: i64 = (1 << (BITS - 1)) - 1;

/// One tracker's cell.
#[derive(Debug)]
struct Cell {
    /// The three coordinates, packed. Meaningful once [`Self::count`] is not 0.
    position: AtomicU64,
    /// How many positions this tracker has published. Never `0` once heard.
    count: AtomicU64,
}

/// The latest position of every tracker, shared between the receiver thread
/// that writes it and the tick that reads it.
///
/// Exactly one thread should [`publish`](Self::publish) - the receiver - and any
/// number may read. It is built once, at its full size, and never grows.
#[derive(Debug)]
pub struct TrackerTable {
    cells: Box<[Cell]>,
}

impl TrackerTable {
    /// A table in which no tracker has been heard.
    #[must_use]
    pub fn new() -> Self {
        let cells = (0..TRACKERS)
            .map(|_| Cell {
                position: AtomicU64::new(0),
                count: AtomicU64::new(0),
            })
            .collect();
        Self { cells }
    }

    /// Records where a tracker is, in metres of show space.
    ///
    /// **A position the table cannot hold is not recorded** and the call says
    /// so: a coordinate that is not a number, or further from the origin than
    /// the table's `±1048 m`, is a tracking system that is wrong rather than a
    /// stage that is large, and storing a clamped one would put a head on a
    /// point nobody is standing on. A tracker number past
    /// [`MAX_TRACKER`] is likewise refused.
    ///
    /// Wait-free: two stores.
    pub fn publish(&self, tracker: u16, position: Vec3) -> bool {
        let Some(cell) = self.cells.get(usize::from(tracker)) else {
            return false;
        };
        let Some(word) = pack(position) else {
            return false;
        };
        // The position first and the count second, so a reader that sees a new
        // count is certain to see a position at least that new.
        cell.position.store(word, Ordering::Release);
        cell.count.fetch_add(1, Ordering::Release);
        true
    }

    /// The latest position of a tracker, or `None` if none has been heard.
    ///
    /// Wait-free: two loads.
    #[must_use]
    pub fn read(&self, tracker: u16) -> Option<Vec3> {
        let cell = self.cells.get(usize::from(tracker))?;
        // The count first: `0` is *never heard* and the position behind it is
        // not a position.
        if cell.count.load(Ordering::Acquire) == 0 {
            return None;
        }
        Some(unpack(cell.position.load(Ordering::Acquire)))
    }

    /// How many positions a tracker has published, `0` if it never has.
    ///
    /// What a daemon watches to tell a tracker that has gone quiet from one that
    /// is standing still: a standing performer still sends, so the count moves
    /// and the position does not.
    #[must_use]
    pub fn count(&self, tracker: u16) -> u64 {
        self.cells
            .get(usize::from(tracker))
            .map_or(0, |cell| cell.count.load(Ordering::Acquire))
    }

    /// Forgets every tracker, as a restart of the receiver does.
    ///
    /// The counts go back to nought so that *never heard* is true again - a
    /// new source is not the old one holding its last position for ever.
    pub fn clear(&self) {
        for cell in &self.cells {
            cell.count.store(0, Ordering::Release);
            cell.position.store(0, Ordering::Release);
        }
    }
}

impl Default for TrackerTable {
    fn default() -> Self {
        Self::new()
    }
}

/// One coordinate in metres as a signed 21-bit count of millimetres, or `None`
/// for one that is not a number or does not fit.
fn millimetres(metres: f64) -> Option<u64> {
    if !metres.is_finite() {
        return None;
    }
    let mm = (metres * 1000.0).round();
    if mm.abs() > LIMIT as f64 {
        return None;
    }
    // Two's complement in `BITS` bits.
    Some((mm as i64 as u64) & MASK)
}

/// A coordinate back to metres, sign-extending the 21 bits.
fn metres(bits: u64) -> f64 {
    let bits = bits & MASK;
    let signed = if bits >> (BITS - 1) == 1 {
        bits as i64 - (1 << BITS)
    } else {
        bits as i64
    };
    signed as f64 / 1000.0
}

fn pack(position: Vec3) -> Option<u64> {
    let x = millimetres(position.x)?;
    let y = millimetres(position.y)?;
    let z = millimetres(position.z)?;
    Some(x | (y << BITS) | (z << (2 * BITS)))
}

fn unpack(word: u64) -> Vec3 {
    Vec3 {
        x: metres(word),
        y: metres(word >> BITS),
        z: metres(word >> (2 * BITS)),
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::{TRACKERS, TrackerTable};
    use prism_domain::{MAX_TRACKER, Vec3};
    use proptest::prelude::*;

    fn at(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    #[test]
    fn a_tracker_nobody_has_heard_has_no_position() {
        let table = TrackerTable::new();
        assert_eq!(table.read(0), None);
        assert_eq!(table.count(0), 0);
        assert_eq!(table.read(MAX_TRACKER), None);
    }

    #[test]
    fn the_latest_position_is_the_one_read() {
        let table = TrackerTable::new();
        assert!(table.publish(7, at(1.0, 2.0, 3.0)));
        assert!(table.publish(7, at(-4.5, 0.25, 12.0)));
        assert_eq!(table.read(7), Some(at(-4.5, 0.25, 12.0)));
        assert_eq!(table.count(7), 2);
        assert_eq!(table.count(8), 0, "another tracker is not touched");
    }

    #[test]
    fn a_position_the_table_cannot_hold_is_refused_and_leaves_the_last_one() {
        let table = TrackerTable::new();
        assert!(table.publish(1, at(1.0, 1.0, 1.0)));
        assert!(!table.publish(1, at(f64::NAN, 0.0, 0.0)));
        assert!(!table.publish(1, at(0.0, f64::INFINITY, 0.0)));
        assert!(!table.publish(1, at(0.0, 0.0, 1049.0)));
        assert!(!table.publish(1, at(-1049.0, 0.0, 0.0)));
        assert!(!table.publish(MAX_TRACKER + 1, at(0.0, 0.0, 0.0)));
        assert_eq!(table.read(1), Some(at(1.0, 1.0, 1.0)));
        assert_eq!(table.count(1), 1, "a refusal is not a position");
    }

    #[test]
    fn the_whole_range_of_the_stage_fits() {
        let table = TrackerTable::new();
        let far = at(1000.0, -1000.0, 1000.0);
        assert!(table.publish(2, far));
        assert_eq!(table.read(2), Some(far));
    }

    #[test]
    fn clearing_makes_every_tracker_unheard_again() {
        let table = TrackerTable::new();
        table.publish(3, at(1.0, 2.0, 3.0));
        table.clear();
        assert_eq!(table.read(3), None);
        assert_eq!(table.count(3), 0);
    }

    #[test]
    fn the_table_is_as_big_as_the_range_says() {
        assert_eq!(TRACKERS, usize::from(MAX_TRACKER) + 1);
    }

    proptest! {
        /// A position comes back to the millimetre, whatever its signs - the
        /// property a hand-rolled bit pack is only trusted by.
        #[test]
        fn a_position_comes_back_to_the_millimetre(
            x in -1000.0_f64..1000.0,
            y in -1000.0_f64..1000.0,
            z in -1000.0_f64..1000.0,
        ) {
            let table = TrackerTable::new();
            prop_assert!(table.publish(0, at(x, y, z)));
            let back = table.read(0).expect("it was published");
            prop_assert!((back.x - x).abs() <= 0.0005 + 1e-9);
            prop_assert!((back.y - y).abs() <= 0.0005 + 1e-9);
            prop_assert!((back.z - z).abs() <= 0.0005 + 1e-9);
        }
    }
}
