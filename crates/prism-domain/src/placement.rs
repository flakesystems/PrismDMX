//! Where a fixture hangs and which way it faces — **S30**, the 3D viewer.
//!
//! [`crate::Fixture`] has carried `position` and `rotation` since S1, and until
//! S30 nothing read either of them. This module is what they **mean**, written
//! down once, because two readers that disagreed about it would draw one rig in
//! two places: the viewer in the interface, and whatever turns a planner's MVR
//! file into a patch.
//!
//! # Show space
//!
//! Metres, **Y up**: `x` across the stage, `y` height above the floor, `z`
//! depth, **growing upstage** — away from the audience and the desk. That is
//! the frame `prism_core::library::gdtf::geometry` already converts GDTF's Z-up
//! into (`x, y, z` there are GDTF's `x, z, y`), so a beam a profile states and a
//! fixture the operator places are in one space. It is a left-handed frame, the
//! one a game engine calls Y-up, Z-forward; nothing here depends on the word.
//!
//! # A rotation is three angles in degrees, applied Z, then X, then Y
//!
//! `rotation.z` rolls the fixture about the depth axis, `rotation.x` then
//! tips it about the across axis, and `rotation.y` finally turns it about the
//! vertical — [`orientation`] is `Ry · Rx · Rz`. Heading last is the order a
//! rigger thinks in: *hang it, tip it towards the stage, then swing it round*.
//!
//! A fixture at `(0, 0, 0)` hangs as its profile describes it, which for every
//! GDTF head and for the viewer's own box is **beam straight down**. From there:
//!
//! | `rotation` | a hanging beam then points |
//! |---|---|
//! | `x = 90` | downstage, at the audience |
//! | `x = −90` | upstage |
//! | `x = 180` | straight up — a fixture standing on the floor |
//! | `z = 90` | across, towards `+x` |
//!
//! [`rotation_of`] is the way back from a matrix, which is what a planner's file
//! states. It answers **one** of the triples that give that matrix — Euler angles
//! are not unique — and the matrix is the fact: `orientation(rotation_of(m))` is
//! `m` again, and a test holds that over arbitrary rotations.
//!
//! # Placing is not patching
//!
//! A fixture's place moves no channel, so [`crate::Command::PlaceFixtures`] does
//! not repatch and the engine is never told: moving a head in the 3D view costs
//! the tick nothing, and that is asserted rather than hoped.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{FixtureId, Vec3};

/// How far from the origin, in metres, any coordinate of a fixture may be.
///
/// A kilometre is past every stage and every stadium, and a coordinate beyond
/// it is a typing error — `6000` for a trim of six metres entered in
/// millimetres — rather than a rig. Refused rather than clamped, because a
/// fixture silently parked at the limit would be a wrong answer that looks like
/// a right one.
pub const MAX_REACH: f64 = 1_000.0;

/// One fixture's place, as [`crate::Command::PlaceFixtures`] carries it.
///
/// Both halves travel together even when a gesture only means to change one:
/// the interface sends what the fixture **is** after the gesture, which is the
/// rule `PatchFixture` follows for its form, and a command per field would be a
/// second way for the view and the show to disagree.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct FixturePlace {
    /// The fixture number.
    pub id: FixtureId,
    /// Where it hangs, in metres of show space.
    pub position: Vec3,
    /// Which way it faces, in degrees — see the module documentation.
    pub rotation: Vec3,
}

impl FixturePlace {
    /// Whether this place is one a fixture can have.
    ///
    /// Every number finite, and the position within [`MAX_REACH`] of the
    /// origin on every axis. A rotation has no range: `370°` is `10°`, and
    /// refusing it would be refusing a spelling.
    #[must_use]
    pub fn is_reachable(&self) -> bool {
        let finite = |v: Vec3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
        finite(self.position)
            && finite(self.rotation)
            && self.position.x.abs() <= MAX_REACH
            && self.position.y.abs() <= MAX_REACH
            && self.position.z.abs() <= MAX_REACH
    }
}

/// A 3×3 rotation, **by rows**: `matrix[row][column]`.
pub type Orientation = [[f64; 3]; 3];

/// The rotation a fixture's `rotation` stands for: `Ry · Rx · Rz`.
///
/// See the module documentation for the order and the frame. The interface
/// builds the same matrix in `ui/src/viewer/space.ts`, and the recording in
/// `crates/prismd/tests/ui_viewer.rs` carries this function's answer for every
/// fixture it places, so the two are held to one another rather than to their
/// comments.
#[must_use]
pub fn orientation(rotation: Vec3) -> Orientation {
    let (sa, ca) = rotation.x.to_radians().sin_cos();
    let (sb, cb) = rotation.y.to_radians().sin_cos();
    let (sc, cc) = rotation.z.to_radians().sin_cos();
    [
        [cb * cc + sb * sa * sc, -cb * sc + sb * sa * cc, sb * ca],
        [ca * sc, ca * cc, -sa],
        [-sb * cc + cb * sa * sc, sb * sc + cb * sa * cc, cb * ca],
    ]
}

/// A rotation that gives `matrix` — the way back from what a planner's file
/// states.
///
/// Each angle in `(−180, 180]`. Where the fixture is tipped exactly on its side
/// (`x = ±90`) roll and heading turn about the same axis and only their sum is
/// determined; the roll is then nought and the heading carries all of it.
#[must_use]
pub fn rotation_of(matrix: &Orientation) -> Vec3 {
    let sin_x = (-matrix[1][2]).clamp(-1.0, 1.0);
    let x = sin_x.asin();
    let (y, z) = if x.cos().abs() > 1e-9 {
        (
            matrix[0][2].atan2(matrix[2][2]),
            matrix[1][0].atan2(matrix[1][1]),
        )
    } else {
        ((-matrix[2][0]).atan2(matrix[0][0]), 0.0)
    };
    Vec3 {
        x: degrees(x),
        y: degrees(y),
        z: degrees(z),
    }
}

/// `matrix · vector`.
#[must_use]
pub fn turn(matrix: &Orientation, vector: Vec3) -> Vec3 {
    let row = |r: [f64; 3]| r[0] * vector.x + r[1] * vector.y + r[2] * vector.z;
    Vec3 {
        x: row(matrix[0]),
        y: row(matrix[1]),
        z: row(matrix[2]),
    }
}

/// Radians as degrees in `(−180, 180]`, with a negative nought made positive.
fn degrees(radians: f64) -> f64 {
    let mut value = radians.to_degrees();
    if value <= -180.0 {
        value += 360.0;
    }
    if value > 180.0 {
        value -= 360.0;
    }
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{FixturePlace, MAX_REACH, Orientation, orientation, rotation_of, turn};
    use crate::{FixtureId, Vec3};

    const DOWN: Vec3 = Vec3 {
        x: 0.0,
        y: -1.0,
        z: 0.0,
    };

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    fn close(left: Vec3, right: Vec3) -> bool {
        (left.x - right.x).abs() < 1e-9
            && (left.y - right.y).abs() < 1e-9
            && (left.z - right.z).abs() < 1e-9
    }

    fn same(left: &Orientation, right: &Orientation) -> bool {
        left.iter()
            .flatten()
            .zip(right.iter().flatten())
            .all(|(a, b)| (a - b).abs() < 1e-7)
    }

    #[test]
    fn nought_is_the_profile_as_it_hangs() {
        assert!(close(turn(&orientation(Vec3::ZERO), DOWN), DOWN));
    }

    /// The table in the module documentation, row by row.
    #[test]
    fn the_documented_rotations_point_where_the_table_says() {
        let pointed = |rotation: Vec3| turn(&orientation(rotation), DOWN);
        assert!(
            close(pointed(v(90.0, 0.0, 0.0)), v(0.0, 0.0, -1.0)),
            "downstage"
        );
        assert!(
            close(pointed(v(-90.0, 0.0, 0.0)), v(0.0, 0.0, 1.0)),
            "upstage"
        );
        assert!(close(pointed(v(180.0, 0.0, 0.0)), v(0.0, 1.0, 0.0)), "up");
        assert!(
            close(pointed(v(0.0, 0.0, 90.0)), v(1.0, 0.0, 0.0)),
            "across"
        );
    }

    #[test]
    fn the_heading_is_applied_last() {
        // Tipped towards the audience and then swung a quarter turn: the beam
        // ends up pointing across the stage, not tipped about a swung axis.
        let beam = turn(&orientation(v(90.0, 90.0, 0.0)), DOWN);
        assert!(close(beam, v(-1.0, 0.0, 0.0)), "{beam:?}");
    }

    #[test]
    fn a_rotation_comes_back_from_its_matrix() {
        let rotation = v(30.0, -45.0, 10.0);
        let back = rotation_of(&orientation(rotation));
        assert!(close(back, rotation), "{back:?}");
    }

    #[test]
    fn on_its_side_the_heading_carries_the_turn() {
        let back = rotation_of(&orientation(v(90.0, 20.0, 15.0)));
        assert!((back.x - 90.0).abs() < 1e-6, "{back:?}");
        assert!(back.z.abs() < 1e-9, "{back:?}");
        assert!(same(&orientation(back), &orientation(v(90.0, 20.0, 15.0))));
    }

    #[test]
    fn an_angle_comes_back_inside_one_turn() {
        let back = rotation_of(&orientation(v(0.0, 370.0, 0.0)));
        assert!(close(back, v(0.0, 10.0, 0.0)), "{back:?}");
    }

    #[test]
    fn a_place_is_reachable_only_on_a_stage() {
        let at = |position: Vec3, rotation: Vec3| FixturePlace {
            id: FixtureId::new(1),
            position,
            rotation,
        };
        assert!(at(v(4.0, 6.0, -2.0), v(0.0, 720.0, 0.0)).is_reachable());
        assert!(at(v(MAX_REACH, 0.0, 0.0), Vec3::ZERO).is_reachable());
        assert!(!at(v(0.0, MAX_REACH + 1.0, 0.0), Vec3::ZERO).is_reachable());
        assert!(!at(v(f64::NAN, 0.0, 0.0), Vec3::ZERO).is_reachable());
        assert!(!at(Vec3::ZERO, v(0.0, f64::INFINITY, 0.0)).is_reachable());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// **The matrix is the fact.** Whatever triple comes back, it makes
        /// the same rotation — including every triple near a quarter tip.
        #[test]
        fn every_matrix_survives_the_way_back(
            x in -720.0_f64..720.0,
            y in -720.0_f64..720.0,
            z in -720.0_f64..720.0,
        ) {
            let matrix = orientation(v(x, y, z));
            prop_assert!(same(&orientation(rotation_of(&matrix)), &matrix));
        }

        /// A rotation keeps a length, which is what makes it one.
        #[test]
        fn a_turn_keeps_a_length(
            x in -180.0_f64..180.0,
            y in -180.0_f64..180.0,
            z in -180.0_f64..180.0,
        ) {
            let turned = turn(&orientation(v(x, y, z)), v(0.3, -0.4, 1.2));
            let length = (turned.x * turned.x + turned.y * turned.y + turned.z * turned.z).sqrt();
            prop_assert!((length - 1.3).abs() < 1e-9);
        }
    }
}
