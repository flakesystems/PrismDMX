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
//! the tick nothing, and that is asserted rather than hoped. **One exception,
//! and it is a fixture with a tracker** (S32): where a head hangs decides where
//! it must point to meet that tracker, so a place that moves such a fixture is
//! the engine's business too - see [`FollowTarget`].
//!
//! # Aiming is the way back from the yoke - S32
//!
//! A moving head's beam leaves along `Ry(pan) * Rx(tilt)` applied inside the
//! fixture's [`orientation`], with the beam straight down at pan and tilt
//! nought (the viewer's `yoke`). [`aim`] is that, backwards: given a point in
//! show space it says which pan and tilt put the beam through it. A tracker
//! (PSN) gives such a point.

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
    /// The tracker it follows, or `None` for none - **S32**.
    ///
    /// Carried **here** and not in a command of its own, because *where a head
    /// hangs* and *what it is aimed at from there* are one fact about one
    /// fixture, they are undone together, and a gesture in the viewer that moves
    /// a following head changes what it must point at. The interface sends what
    /// the fixture **is** after the gesture, which is this struct's rule, so a
    /// drag carries the follow it found and a tracker panel carries the place it
    /// found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<FollowTarget>,
    /// Which of its axes run the other way from the viewer's - see [`Mirror`].
    ///
    /// Carried here for [`Self::follow`]'s reason: it is a fact about how one
    /// fixture hangs and moves, it is undone with the rest of the gesture, and
    /// a place that leaves it out would quietly un-mirror a head every time it
    /// was dragged.
    #[serde(default, skip_serializing_if = "Mirror::is_none")]
    pub mirror: Mirror,
}

/// **Which of a moving head's axes turn the other way from the viewer's** - S32.
///
/// The viewer draws every head with one convention: positive pan turns the way a
/// positive heading does, positive tilt tips towards the front. A head whose
/// motor runs the other way has the beam going where the picture says it does
/// not, and **no rotation puts that right** - turning a fixture over mirrors
/// pan, but it also sends the beam the wrong way up. So this says so directly.
///
/// It is **how the head is**, and so it is in the show with the fixture. It
/// changes what the viewer draws and what a tracker aims at
/// (`prism_engine::FollowLayer`), and **nothing the cable carries for a value
/// anybody typed**: a pan of 60 % is still 60 %, which is the difference from
/// [`crate::Fixture::invert_pan`], whose whole point is to change the cable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Mirror {
    /// Pan runs the other way.
    #[serde(default)]
    pub pan: bool,
    /// Tilt runs the other way.
    #[serde(default)]
    pub tilt: bool,
}

impl Mirror {
    /// Whether neither axis is mirrored - what every fixture was before this
    /// existed, and what is **not written** to a show file.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        !self.pan && !self.tilt
    }
}

impl FixturePlace {
    /// Whether this place is one a fixture can have.
    ///
    /// Every number finite, and the position within [`MAX_REACH`] of the
    /// origin on every axis. A rotation has no range: `370°` is `10°`, and
    /// refusing it would be refusing a spelling. A tracker to follow must be
    /// one the table holds ([`FollowTarget::is_reachable`]).
    #[must_use]
    pub fn is_reachable(&self) -> bool {
        let finite = |v: Vec3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
        finite(self.position)
            && finite(self.rotation)
            && self.position.x.abs() <= MAX_REACH
            && self.position.y.abs() <= MAX_REACH
            && self.position.z.abs() <= MAX_REACH
            && self.follow.is_none_or(|follow| follow.is_reachable())
    }
}

/// The tracker numbers a desk listens to: `0..=MAX_TRACKER`.
///
/// PSN numbers a tracker with sixteen bits and nothing about a stage has a
/// thousand performers on it. The tick keeps one slot per number
/// (`prism_engine::TrackerTable`), so the range is the size of a table that was
/// allocated once, and a number past it is refused when a head is assigned
/// rather than being heard and ignored for ever.
pub const MAX_TRACKER: u16 = 1_023;

/// Which tracker a head follows, and what part of the performer it lights —
/// **S32**.
///
/// Part of the **show**, on the fixture, because who follows whom is a fact
/// about this production and travels with the file: a show carried to another
/// hall keeps its follow assignments and leaves the first hall's tracking
/// system behind. How loudly each head follows is *not* here — that is the
/// [`crate::AttributeType::Follow`] value, which a cue stores like any other.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct FollowTarget {
    /// The PSN tracker, `0..=`[`MAX_TRACKER`].
    pub tracker: u16,
    /// Added to the tracker's position, in metres of show space.
    ///
    /// A tracker is worn at the belt or on the head and a beam wants the chest
    /// or the face, so this is the height of the part of the performer to
    /// light — `y = 0.4` aims at 40 cm above wherever the tracker says.
    pub offset: Vec3,
}

impl FollowTarget {
    /// Whether this is a target a head can follow: a tracker the table holds
    /// and an offset that is a number of a stage's size.
    #[must_use]
    pub fn is_reachable(&self) -> bool {
        let v = self.offset;
        self.tracker <= MAX_TRACKER
            && v.x.is_finite()
            && v.y.is_finite()
            && v.z.is_finite()
            && v.x.abs() <= MAX_REACH
            && v.y.abs() <= MAX_REACH
            && v.z.abs() <= MAX_REACH
    }
}

/// The travel of one axis, in degrees: what its value `0` and its value `65535`
/// are (`AttributeDef::physical_from` and `physical_to`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Travel {
    /// The angle at the bottom of the axis.
    pub from: f64,
    /// The angle at the top of it.
    pub to: f64,
}

impl Travel {
    /// The lower and the upper end, whichever way round the profile wrote them.
    #[must_use]
    pub fn limits(&self) -> (f64, f64) {
        (self.from.min(self.to), self.from.max(self.to))
    }

    /// The angle a sixteen-bit value stands for.
    #[must_use]
    pub fn degrees(&self, value: u16) -> f64 {
        self.from + (self.to - self.from) * (f64::from(value) / f64::from(u16::MAX))
    }

    /// The sixteen-bit value that stands for an angle, clamped into the travel.
    ///
    /// A travel with no span (`from == to`) has one value and answers the
    /// middle of the axis, which is where a profile that states nothing parks.
    #[must_use]
    pub fn value(&self, degrees: f64) -> u16 {
        let span = self.to - self.from;
        if span.abs() < 1e-9 || !degrees.is_finite() {
            return u16::MAX / 2 + 1;
        }
        let fraction = ((degrees - self.from) / span).clamp(0.0, 1.0);
        // The clamp is what makes the cast exact: `fraction * 65535` is in
        // `0.0..=65535.0`, and rounding keeps a travel's far end at `65535`.
        (fraction * f64::from(u16::MAX)).round() as u16
    }
}

/// Where a head must point, as pan and tilt in degrees — [`aim`]'s answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aim {
    /// Pan, in degrees of the axis's own travel.
    pub pan: f64,
    /// Tilt, in degrees of the axis's own travel.
    pub tilt: f64,
    /// Whether the beam reaches the point at all. `false` means the head is
    /// pointing as close as its travel allows and no closer.
    pub reached: bool,
}

/// The pan and tilt that put a head's beam through `target`.
///
/// `position` and `orientation` are the fixture's place ([`orientation`] of its
/// `rotation`), `target` is a point in show space, and `pan` and `tilt` are the
/// travel of the two axes. `near` is where the head points now, in degrees: a
/// beam can be put through a point two ways (tipped over the top, or the other
/// side with the yoke half a turn round) and a head with a long pan has
/// several turns of it, so the answer is the one **nearest** `near` that the
/// travel allows. Which is what stops a head spinning the long way round
/// because a performer crossed the line behind it.
///
/// `None` only for a target that is *on* the fixture, which has no direction.
/// Straight above or straight below the head the pan is undetermined and
/// `near`'s is kept, because turning the yoke for nothing would be a move with
/// no cause.
#[must_use]
pub fn aim(
    position: Vec3,
    orientation: &Orientation,
    target: Vec3,
    pan: Travel,
    tilt: Travel,
    near: (f64, f64),
) -> Option<Aim> {
    // The direction in the fixture's own frame: the transpose of a rotation is
    // its inverse.
    let transposed: Orientation = [
        [orientation[0][0], orientation[1][0], orientation[2][0]],
        [orientation[0][1], orientation[1][1], orientation[2][1]],
        [orientation[0][2], orientation[1][2], orientation[2][2]],
    ];
    let to = Vec3 {
        x: target.x - position.x,
        y: target.y - position.y,
        z: target.z - position.z,
    };
    let local = turn(&transposed, to);
    let length = (local.x * local.x + local.y * local.y + local.z * local.z).sqrt();
    if !length.is_finite() || length < 1e-9 {
        return None;
    }
    let (dx, dz) = (local.x / length, local.z / length);
    let dy = local.y / length;

    // `beam(pan, tilt) = (−sin p · sin t, −cos t, −cos p · sin t)`, inverted.
    let tipped = (-dy).clamp(-1.0, 1.0).acos(); // `0..=π`
    let side = tipped.sin();
    let heading = if side < 1e-9 {
        near.0.to_radians()
    } else {
        (-dx).atan2(-dz)
    };
    // The second way of pointing at the same place: the yoke half a turn round
    // and the head tipped the other way.
    let candidates = [
        (heading.to_degrees(), tipped.to_degrees()),
        (heading.to_degrees() + 180.0, -tipped.to_degrees()),
    ];

    let (pan_low, pan_high) = pan.limits();
    let (tilt_low, tilt_high) = tilt.limits();
    let mut best: Option<(f64, f64, f64)> = None;
    for (candidate_pan, candidate_tilt) in candidates {
        if candidate_tilt < tilt_low - 1e-9 || candidate_tilt > tilt_high + 1e-9 {
            continue;
        }
        // Every whole turn of the pan the travel holds.
        for turns in -4_i32..=4 {
            let turned = candidate_pan + 360.0 * f64::from(turns);
            if turned < pan_low - 1e-9 || turned > pan_high + 1e-9 {
                continue;
            }
            let cost = (turned - near.0).abs() + (candidate_tilt - near.1).abs();
            if best.is_none_or(|(_, _, held)| cost < held) {
                best = Some((turned, candidate_tilt, cost));
            }
        }
    }
    if let Some((pan, tilt, _)) = best {
        return Some(Aim {
            pan: pan.clamp(pan_low, pan_high),
            tilt: tilt.clamp(tilt_low, tilt_high),
            reached: true,
        });
    }

    // Out of reach: the point the travel allows that is closest to where the
    // beam has to go. The first candidate's tilt is the right one to clamp, and
    // the pan is the whole turn nearest where the head is.
    let (candidate_pan, candidate_tilt) = candidates[0];
    let mut pan_out = candidate_pan.clamp(pan_low, pan_high);
    let mut closest = f64::INFINITY;
    for turns in -4_i32..=4 {
        let turned = candidate_pan + 360.0 * f64::from(turns);
        let clamped = turned.clamp(pan_low, pan_high);
        let cost = (clamped - turned).abs() + (clamped - near.0).abs() * 1e-3;
        if cost < closest {
            closest = cost;
            pan_out = clamped;
        }
    }
    Some(Aim {
        pan: pan_out,
        tilt: candidate_tilt.clamp(tilt_low, tilt_high),
        reached: false,
    })
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

    use super::{
        FixturePlace, MAX_REACH, Orientation, Travel, aim, orientation, rotation_of, turn,
    };
    use crate::{FixtureId, Mirror, Vec3};

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
            follow: None,
            mirror: Mirror::default(),
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

    // ---- S32: aiming ------------------------------------------------------

    /// The yoke the viewer draws, term for term: `Ry(pan) * Rx(tilt)` is
    /// `orientation({ x: tilt, y: pan, z: 0 })`, and the beam is its action on
    /// straight down. This is **the forward half the aim is the way back from**,
    /// written out here so the property below is held against the viewer's own
    /// definition and not against `aim`'s.
    fn beam_of(rotation: Vec3, pan: f64, tilt: f64) -> Vec3 {
        let head = turn(&orientation(v(tilt, pan, 0.0)), DOWN);
        turn(&orientation(rotation), head)
    }

    const PAN: Travel = Travel {
        from: -270.0,
        to: 270.0,
    };
    const TILT: Travel = Travel {
        from: -135.0,
        to: 135.0,
    };

    fn unit(a: Vec3) -> Vec3 {
        let l = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
        v(a.x / l, a.y / l, a.z / l)
    }

    fn towards(from: Vec3, to: Vec3) -> Vec3 {
        unit(v(to.x - from.x, to.y - from.y, to.z - from.z))
    }

    #[test]
    fn a_head_hanging_at_nought_points_down_at_a_point_below_it() {
        let aimed = aim(
            v(1.0, 6.0, 2.0),
            &orientation(Vec3::ZERO),
            v(1.0, 0.0, 2.0),
            PAN,
            TILT,
            (0.0, 0.0),
        )
        .unwrap();
        assert!(aimed.reached);
        assert!(aimed.tilt.abs() < 1e-6, "{aimed:?}");
    }

    #[test]
    fn a_point_downstage_is_a_positive_tilt_at_pan_nought() {
        // `yoke`'s own words: positive tilt tips a hanging beam towards where
        // its front faces at pan nought, which is downstage (-z) for nought.
        let aimed = aim(
            v(0.0, 6.0, 0.0),
            &orientation(Vec3::ZERO),
            v(0.0, 0.0, -6.0),
            PAN,
            TILT,
            (0.0, 0.0),
        )
        .unwrap();
        assert!((aimed.pan).abs() < 1e-6, "{aimed:?}");
        assert!((aimed.tilt - 45.0).abs() < 1e-6, "{aimed:?}");
    }

    #[test]
    fn a_point_across_the_stage_is_a_quarter_turn_of_pan() {
        let aimed = aim(
            v(0.0, 4.0, 0.0),
            &orientation(Vec3::ZERO),
            v(4.0, 0.0, 0.0),
            PAN,
            TILT,
            (0.0, 0.0),
        )
        .unwrap();
        // Positive pan turns the way a positive heading does, so `+x` is
        // `-sin(pan)` negative: the yoke has to go round to -90.
        assert!(
            close(
                beam_of(Vec3::ZERO, aimed.pan, aimed.tilt),
                towards(v(0.0, 4.0, 0.0), v(4.0, 0.0, 0.0))
            ),
            "{aimed:?}"
        );
    }

    #[test]
    fn the_nearer_of_the_two_ways_is_taken() {
        // Over the top of the head: either tipped 100 degrees at pan 0 or
        // tipped -100 degrees at pan 180. A head that is already at pan 170
        // goes the second way and does not spin the long way round.
        let position = v(0.0, 4.0, 0.0);
        let target = v(0.0, 5.0, -1.0);
        let from_front = aim(
            position,
            &orientation(Vec3::ZERO),
            target,
            PAN,
            TILT,
            (0.0, 90.0),
        )
        .unwrap();
        let from_behind = aim(
            position,
            &orientation(Vec3::ZERO),
            target,
            PAN,
            TILT,
            (170.0, -90.0),
        )
        .unwrap();
        assert!(from_front.tilt > 0.0 && from_front.pan.abs() < 1.0);
        assert!(from_behind.tilt < 0.0 && (from_behind.pan - 180.0).abs() < 1.0);
        for aimed in [from_front, from_behind] {
            assert!(close(
                beam_of(Vec3::ZERO, aimed.pan, aimed.tilt),
                towards(position, target)
            ));
        }
    }

    #[test]
    fn a_pan_with_more_than_a_turn_of_travel_takes_the_turn_nearest_the_head() {
        let aimed = aim(
            v(0.0, 4.0, 0.0),
            &orientation(Vec3::ZERO),
            v(0.0, 0.0, 3.0),
            PAN,
            TILT,
            (250.0, 30.0),
        )
        .unwrap();
        // `+z` is upstage, a pan of 180 either way round; the head is at 250.
        assert!((aimed.pan - 180.0).abs() < 1e-6, "{aimed:?}");
    }

    #[test]
    fn straight_below_keeps_the_pan_it_has() {
        let aimed = aim(
            v(0.0, 4.0, 0.0),
            &orientation(Vec3::ZERO),
            v(0.0, 0.0, 0.0),
            PAN,
            TILT,
            (123.0, 7.0),
        )
        .unwrap();
        assert!((aimed.pan - 123.0).abs() < 1e-6, "{aimed:?}");
        assert!(aimed.tilt.abs() < 1e-6, "{aimed:?}");
    }

    #[test]
    fn a_target_on_the_fixture_has_no_direction() {
        let at = v(1.0, 2.0, 3.0);
        assert!(aim(at, &orientation(Vec3::ZERO), at, PAN, TILT, (0.0, 0.0)).is_none());
    }

    #[test]
    fn a_point_the_travel_cannot_reach_is_pointed_at_as_nearly_as_it_can() {
        // A tilt of only +-30 cannot reach a point 45 degrees off the vertical.
        let short = Travel {
            from: -30.0,
            to: 30.0,
        };
        let aimed = aim(
            v(0.0, 6.0, 0.0),
            &orientation(Vec3::ZERO),
            v(0.0, 0.0, -6.0),
            PAN,
            short,
            (0.0, 0.0),
        )
        .unwrap();
        assert!(!aimed.reached);
        assert!((aimed.tilt - 30.0).abs() < 1e-6, "{aimed:?}");
    }

    #[test]
    fn a_travel_converts_between_angles_and_values() {
        assert_eq!(PAN.value(0.0), 32768);
        assert_eq!(PAN.value(-270.0), 0);
        assert_eq!(PAN.value(270.0), 65535);
        assert_eq!(PAN.value(10_000.0), 65535, "clamped, never wrapped");
        assert_eq!(PAN.value(f64::NAN), 32768);
        assert!((PAN.degrees(0) + 270.0).abs() < 1e-9);
        assert!((PAN.degrees(65535) - 270.0).abs() < 1e-9);
        let none = Travel { from: 5.0, to: 5.0 };
        assert_eq!(
            none.value(5.0),
            32768,
            "a travel with no span parks in the middle"
        );
    }

    #[test]
    fn a_follow_target_is_reachable_on_a_stage() {
        use super::{FollowTarget, MAX_TRACKER};
        let target = |tracker: u16, offset: Vec3| FollowTarget { tracker, offset };
        assert!(target(0, v(0.0, 0.4, 0.0)).is_reachable());
        assert!(target(MAX_TRACKER, Vec3::ZERO).is_reachable());
        assert!(!target(MAX_TRACKER + 1, Vec3::ZERO).is_reachable());
        assert!(!target(1, v(f64::NAN, 0.0, 0.0)).is_reachable());
        assert!(!target(1, v(0.0, MAX_REACH + 1.0, 0.0)).is_reachable());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        /// **The beam goes through the point.** Hang a head anywhere, tip and
        /// turn it any way, point it at a point anywhere else, and the viewer's
        /// own forward calculation puts the beam on that point - for every
        /// target that is not straight on the fixture and that a travel of
        /// +-270 and +-135 reaches (`reached`).
        #[test]
        fn aiming_then_drawing_the_beam_goes_through_the_point(
            rx in -180.0_f64..180.0,
            ry in -180.0_f64..180.0,
            rz in -180.0_f64..180.0,
            px in -20.0_f64..20.0, py in 0.0_f64..12.0, pz in -20.0_f64..20.0,
            tx in -20.0_f64..20.0, ty in -2.0_f64..12.0, tz in -20.0_f64..20.0,
            near_pan in -270.0_f64..270.0,
            near_tilt in -135.0_f64..135.0,
        ) {
            let position = v(px, py, pz);
            let target = v(tx, ty, tz);
            prop_assume!(
                ((tx - px).powi(2) + (ty - py).powi(2) + (tz - pz).powi(2)).sqrt() > 0.05
            );
            let rotation = v(rx, ry, rz);
            let aimed = aim(position, &orientation(rotation), target, PAN, TILT, (near_pan, near_tilt))
                .expect("a target off the fixture has a direction");
            prop_assert!(aimed.pan >= -270.0 - 1e-9 && aimed.pan <= 270.0 + 1e-9);
            prop_assert!(aimed.tilt >= -135.0 - 1e-9 && aimed.tilt <= 135.0 + 1e-9);
            if aimed.reached {
                let beam = beam_of(rotation, aimed.pan, aimed.tilt);
                prop_assert!(
                    close(beam, towards(position, target)),
                    "{beam:?} vs {:?}",
                    towards(position, target)
                );
            }
        }

        /// What the travel reaches is exactly the directions within a tilt of
        /// 135 degrees of straight down: a tilt of +-135 and a pan of +-270
        /// cover the sphere together except the cap within 45 degrees of
        /// straight up, and that cap is the one place `reached` is false.
        #[test]
        fn these_travels_reach_everything_but_the_cap_above_the_head(
            dx in -1.0_f64..1.0, dy in -1.0_f64..1.0, dz in -1.0_f64..1.0,
        ) {
            let length = (dx * dx + dy * dy + dz * dz).sqrt();
            prop_assume!(length > 0.1);
            // `cos(135 deg)`, kept off the boundary where rounding decides.
            let up = dy / length;
            prop_assume!((up - core::f64::consts::FRAC_1_SQRT_2).abs() > 1e-6);
            let aimed = aim(
                Vec3::ZERO,
                &orientation(Vec3::ZERO),
                v(dx, dy, dz),
                PAN,
                TILT,
                (0.0, 0.0),
            )
            .unwrap();
            prop_assert_eq!(aimed.reached, up < core::f64::consts::FRAC_1_SQRT_2);
        }
    }
}
