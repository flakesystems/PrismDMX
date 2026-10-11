//! Stage marks: points of the room whose place is known - **B70**.
//!
//! A moving head is calibrated by driving its beam onto things whose place in
//! show space is already known and saying *there* (`crate::calibrate`). The
//! things are **marks**: the spike tape on the floor, the corner of a platform,
//! a cleat on the truss. They belong to the **show**, not to a calibration in
//! progress and not to a fixture: they are facts about the room as this
//! production uses it, every head is calibrated against the same ones, and a
//! show carried to another hall keeps them (and is told by the calibration, when
//! it is repeated, that they have to be found again there).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{MAX_REACH, MarkId, Vec3};

/// How many marks a show may hold. A stage has tens; the limit is a typing error
/// stopped, not a rig refused.
pub const MAX_MARKS: usize = 512;

/// The longest a mark's name may be, in characters.
pub const MAX_MARK_NAME: usize = 64;

/// One reference point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Mark {
    /// Its number - what the pool is keyed by.
    pub id: MarkId,
    /// What the crew calls it: `DS centre`, `Spike 3`. May be empty.
    pub name: String,
    /// Where it is, in metres of show space (`crate::placement`): `x` across the
    /// stage, `y` up from the floor, `z` upstage.
    pub position: Vec3,
}

impl Mark {
    /// Whether this is a mark a show can hold: a name that fits, and a place
    /// that is a number within [`MAX_REACH`] of the origin on every axis.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let reachable = |v: f64| v.is_finite() && v.abs() <= MAX_REACH;
        self.name.chars().count() <= MAX_MARK_NAME
            && reachable(self.position.x)
            && reachable(self.position.y)
            && reachable(self.position.z)
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_MARK_NAME, Mark};
    use crate::{MarkId, Vec3};

    fn mark(name: &str, position: Vec3) -> Mark {
        Mark {
            id: MarkId::new(1),
            name: name.to_owned(),
            position,
        }
    }

    #[test]
    fn a_name_that_fits_and_a_place_in_reach_is_a_mark() {
        assert!(mark("DS centre", Vec3::new(0.0, 0.0, -3.0)).is_valid());
        assert!(mark("", Vec3::ZERO).is_valid());
    }

    #[test]
    fn a_long_name_or_a_wild_place_is_not() {
        assert!(!mark(&"x".repeat(MAX_MARK_NAME + 1), Vec3::ZERO).is_valid());
        assert!(mark(&"x".repeat(MAX_MARK_NAME), Vec3::ZERO).is_valid());
        assert!(!mark("far", Vec3::new(1_000.5, 0.0, 0.0)).is_valid());
        assert!(!mark("nan", Vec3::new(f64::NAN, 0.0, 0.0)).is_valid());
    }
}
