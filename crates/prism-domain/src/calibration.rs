//! Where a head really hangs, worked out from where it really points — **B70**.
//!
//! The 3D viewer and the follow layer both trust [`crate::FixturePlace`]: a head
//! that is hung two metres from where the show says, or turned ten degrees from
//! it, is drawn wrongly and — which is the cost that matters — is aimed at a
//! tracker wrongly. This module is the **calibration**: the operator drives a
//! head's beam onto points whose place in the room is known (a spike mark on the
//! floor, the corner of a truss), says *there*, and the position and rotation
//! that make every one of those sightings true are the head's real place.
//!
//! # The model is the viewer's, backwards twice
//!
//! A head's beam leaves along [`beam`] — `(−sin p · sin t, −cos t, −cos p · sin t)`
//! in the fixture's own frame, which is the inverse of [`crate::aim`] — turned by
//! the fixture's [`crate::orientation`]. A sighting says: *with pan `p` and tilt
//! `t`, the beam went through the point `M`*. So the fixture at `P` with
//! orientation `R` satisfies `R · beam(p, t) ∥ (M − P)`, and a pose is right when
//! that holds for every sighting. The solver finds the `P` and the `R` that
//! make the directions agree as well as they can (least squares on the angle),
//! starting from the place the show already has — a rigger's plan is almost
//! always close, and the answer is the solution **nearest** to it.
//!
//! The angles are the **viewer's**: the caller has already undone a profile's
//! inverted channel and a fixture's [`crate::Mirror`] (`AxisModel`), so this
//! module never learns that a motor runs the other way.
//!
//! # What it needs, and what it says about its answer
//!
//! A sighting is two numbers, so a pose of six unknowns needs three sightings
//! and one with the position held (a tape measure's word) needs two. Three is
//! *exactly* enough: it can be solved, and nothing in it can disagree with
//! anything else, so a mistyped mark would pass unnoticed. [`Calibrated::spare`]
//! says how many equations are left over to check the answer against, and the
//! residual of every sighting says which one is wrong when they disagree.
//! Marks that lie on one straight line through the head, or sightings that do
//! not pin one of the unknowns, are [`CalibrationError::Underdetermined`] rather
//! than an answer that depends on the starting guess.
//!
//! The solver is Levenberg–Marquardt with an analytic Jacobian, started from the
//! given heading and from three more turned a quarter round it, so a rig whose
//! rotation was never entered still finds its heading. It runs on the core
//! thread when somebody asks and **never on the tick**.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{MAX_REACH, Orientation, Vec3, orientation, rotation_of, turn};

/// The pointing noise assumed when the sightings agree better than a person can
/// aim a beam, in degrees: the floor under the uncertainty that is reported.
const NOISE_FLOOR: f64 = 0.1;
/// Most iterations of one start.
const MAX_ITERATIONS: u32 = 200;
/// How near zero a scaled pivot may be before the problem is called flat.
const SINGULAR: f64 = 1e-10;

/// One sighting: the beam, at this pan and tilt, was seen to go through `target`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sighting {
    /// A point of show space the operator confirmed the beam is on, in metres.
    pub target: Vec3,
    /// Pan in degrees, **the viewer's** — see the module documentation.
    pub pan: f64,
    /// Tilt in degrees, **the viewer's**.
    pub tilt: f64,
}

/// What a calibration may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Position and rotation are both solved for.
    Nothing,
    /// The position stays where it is — it came from a plan or a tape measure —
    /// and only the rotation is solved for.
    Position,
}

impl Hold {
    /// How many unknowns are left to solve for.
    #[must_use]
    pub const fn unknowns(self) -> usize {
        match self {
            Self::Nothing => 6,
            Self::Position => 3,
        }
    }
}

/// Why no pose could be answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationError {
    /// Fewer sightings than the unknowns need.
    TooFew {
        /// How many were given.
        have: usize,
        /// How many the chosen [`Hold`] needs.
        need: usize,
    },
    /// A number that is not one, or a place beyond [`MAX_REACH`].
    NotReachable,
    /// A mark that is where the head is, which has no direction.
    OnTheFixture,
    /// The sightings do not decide the pose: marks in a line, the same mark
    /// twice, or a head that was never moved between them.
    Underdetermined,
}

impl core::fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooFew { have, need } => write!(
                f,
                "{have} sightings are not enough: this needs at least {need}"
            ),
            Self::NotReachable => write!(f, "a position or an angle is not a usable number"),
            Self::OnTheFixture => write!(f, "a reference point is where the fixture is"),
            Self::Underdetermined => write!(
                f,
                "these sightings do not decide where the fixture is: use marks that are not in a \
                 line, or on different sides of it"
            ),
        }
    }
}

impl core::error::Error for CalibrationError {}

/// The pose that fits the sightings, and how well it does.
#[derive(Debug, Clone, PartialEq)]
pub struct Calibrated {
    /// Where the head hangs, in metres.
    pub position: Vec3,
    /// How it is turned, in degrees — [`crate::orientation`]'s triple.
    pub rotation: Vec3,
    /// For each sighting, in the order given: the angle in degrees between where
    /// the beam went and where the pose says the mark is. **The one that is
    /// large is the one that was wrong.**
    pub residuals: Vec<f64>,
    /// The root of the mean square of [`Self::residuals`], in degrees.
    pub rms: f64,
    /// The largest of them, in degrees.
    pub worst: f64,
    /// One standard deviation of the position, in metres, from how the sightings
    /// are spread and how well they agree — `0` when the position was held.
    pub position_sigma: f64,
    /// One standard deviation of the rotation, in degrees.
    pub rotation_sigma: f64,
    /// Equations beyond the unknowns: `2 · sightings − unknowns`. **`0` means
    /// nothing was checked.**
    pub spare: usize,
    /// How many iterations the winning start took.
    pub iterations: u32,
}

/// The direction a beam leaves a fixture along, in the fixture's own frame.
///
/// The way back from [`crate::aim`]: pan `0` and tilt `0` point straight down.
#[must_use]
pub fn beam(pan: f64, tilt: f64) -> Vec3 {
    let (sp, cp) = pan.to_radians().sin_cos();
    let (st, ct) = tilt.to_radians().sin_cos();
    Vec3 {
        x: -sp * st,
        y: -ct,
        z: -cp * st,
    }
}

/// Finds the pose that puts every sighting's beam through its mark.
///
/// `position` and `rotation` are the place the show has now and are where the
/// search begins; with [`Hold::Position`] the position is returned untouched.
///
/// # Errors
///
/// See [`CalibrationError`].
pub fn calibrate(
    position: Vec3,
    rotation: Vec3,
    sightings: &[Sighting],
    hold: Hold,
) -> Result<Calibrated, CalibrationError> {
    let need = match hold {
        Hold::Nothing => 3,
        Hold::Position => 2,
    };
    if sightings.len() < need {
        return Err(CalibrationError::TooFew {
            have: sightings.len(),
            need,
        });
    }
    let usable = |v: Vec3| {
        v.x.is_finite()
            && v.y.is_finite()
            && v.z.is_finite()
            && v.x.abs() <= MAX_REACH
            && v.y.abs() <= MAX_REACH
            && v.z.abs() <= MAX_REACH
    };
    if !usable(position)
        || !rotation.x.is_finite()
        || !rotation.y.is_finite()
        || !rotation.z.is_finite()
        || sightings.iter().any(|s| {
            !usable(s.target)
                || !s.pan.is_finite()
                || !s.tilt.is_finite()
                || s.pan.abs() > 1e6
                || s.tilt.abs() > 1e6
        })
    {
        return Err(CalibrationError::NotReachable);
    }
    // The same mark seen at the same angles twice says one thing, and says it
    // twice: it must not count towards the number that are needed.
    let beams: Vec<Vec3> = sightings.iter().map(|s| beam(s.pan, s.tilt)).collect();

    let start = orientation(rotation);
    // A quarter turn about the vertical is the only guess a rigger gets wrong
    // by a whole number of: which way the stage faces.
    let quarters = [0.0_f64, 90.0, 180.0, 270.0];
    let mut best: Option<Fit> = None;
    for quarter in quarters {
        let turned = compose(&about_y(quarter), &start);
        match fit(position, turned, sightings, &beams, hold) {
            Ok(candidate) => {
                if best.as_ref().is_none_or(|held| candidate.cost < held.cost) {
                    best = Some(candidate);
                }
            }
            Err(CalibrationError::OnTheFixture) => return Err(CalibrationError::OnTheFixture),
            Err(_) => {}
        }
    }
    let Some(best) = best else {
        return Err(CalibrationError::Underdetermined);
    };

    let residuals: Vec<f64> = sightings
        .iter()
        .zip(&beams)
        .map(|(s, b)| angle_between(turn(&best.rotation, *b), subtract(s.target, best.position)))
        .collect();
    let rms = (residuals.iter().map(|r| r * r).sum::<f64>() / residuals.len() as f64).sqrt();
    let worst = residuals.iter().copied().fold(0.0, f64::max);
    let unknowns = hold.unknowns();
    let spare = (2 * sightings.len()).saturating_sub(unknowns);
    // The scatter the sightings show is the best estimate of how well a beam
    // can be aimed — but never trust it to be smaller than a person can manage.
    let sigma = if spare > 0 {
        let from_data = (residuals.iter().map(|r| r * r).sum::<f64>() / spare as f64).sqrt();
        from_data.max(NOISE_FLOOR)
    } else {
        NOISE_FLOOR
    }
    .to_radians();
    Ok(Calibrated {
        position: best.position,
        rotation: rotation_of(&best.rotation),
        residuals,
        rms,
        worst,
        position_sigma: best.position_variance.sqrt() * sigma,
        rotation_sigma: (best.rotation_variance.sqrt() * sigma).to_degrees(),
        spare,
        iterations: best.iterations,
    })
}

/// What one start ends at.
struct Fit {
    position: Vec3,
    rotation: Orientation,
    /// Sum of the squared residuals, in radians squared.
    cost: f64,
    /// Sum of the variances of the position's three axes per unit of pointing
    /// variance, and the same for the rotation.
    position_variance: f64,
    rotation_variance: f64,
    iterations: u32,
}

/// Levenberg–Marquardt from one start.
fn fit(
    start_position: Vec3,
    start_rotation: Orientation,
    sightings: &[Sighting],
    beams: &[Vec3],
    hold: Hold,
) -> Result<Fit, CalibrationError> {
    let free_position = hold == Hold::Nothing;
    let unknowns = hold.unknowns();
    let mut position = start_position;
    let mut rotation = start_rotation;
    let (mut cost, _) = evaluate(position, &rotation, sightings, beams)?;
    let mut lambda = 1e-3;
    let mut iterations = 0;
    for _ in 0..MAX_ITERATIONS {
        iterations += 1;
        let (jacobian, residual) = linearise(position, &rotation, sightings, beams, free_position)?;
        let (normal, gradient) = normal_equations(&jacobian, &residual, unknowns);
        let mut improved = false;
        for _ in 0..12 {
            let mut damped = normal;
            for k in 0..unknowns {
                damped[k][k] += lambda * normal[k][k].max(1e-12);
            }
            let Some(step) = solve(&damped, &gradient.map(|g| -g), unknowns) else {
                lambda *= 10.0;
                continue;
            };
            let (next_position, next_rotation) =
                apply_step(position, &rotation, &step, free_position);
            // A search that leaves the building has found a direction the
            // sightings do not bound, not a place.
            if next_position.x.abs() > MAX_REACH
                || next_position.y.abs() > MAX_REACH
                || next_position.z.abs() > MAX_REACH
            {
                lambda *= 10.0;
                continue;
            }
            let Ok((next_cost, _)) = evaluate(next_position, &next_rotation, sightings, beams)
            else {
                lambda *= 10.0;
                continue;
            };
            if next_cost.is_finite() && next_cost <= cost {
                let gain = cost - next_cost;
                position = next_position;
                rotation = next_rotation;
                let done = gain <= cost * 1e-14 + 1e-30
                    || step.iter().take(unknowns).all(|s| s.abs() < 1e-12);
                cost = next_cost;
                lambda = (lambda * 0.1).max(1e-12);
                improved = true;
                if done {
                    iterations = iterations.max(1);
                    return finish(position, rotation, cost, iterations, sightings, beams, hold);
                }
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
    finish(position, rotation, cost, iterations, sightings, beams, hold)
}

/// Turns an end point into a [`Fit`] — or refuses it when it cannot be told
/// from any other.
fn finish(
    position: Vec3,
    rotation: Orientation,
    cost: f64,
    iterations: u32,
    sightings: &[Sighting],
    beams: &[Vec3],
    hold: Hold,
) -> Result<Fit, CalibrationError> {
    let free_position = hold == Hold::Nothing;
    let unknowns = hold.unknowns();
    // A search that ends on top of a mark, or at the edge of the reachable
    // world, has found a place the sightings do not bound - not a pose.
    let at_the_edge = |v: f64| v.abs() >= MAX_REACH * 0.99;
    if free_position
        && (at_the_edge(position.x)
            || at_the_edge(position.y)
            || at_the_edge(position.z)
            || sightings
                .iter()
                .any(|s| norm(subtract(s.target, position)) < SETTLED_ON_A_MARK))
    {
        return Err(CalibrationError::Underdetermined);
    }
    let (jacobian, residual) = linearise(position, &rotation, sightings, beams, free_position)?;
    let (normal, _) = normal_equations(&jacobian, &residual, unknowns);
    let Some(inverse) = invert(&normal, unknowns) else {
        return Err(CalibrationError::Underdetermined);
    };
    let (position_variance, rotation_variance) = if free_position {
        (
            inverse[0][0] + inverse[1][1] + inverse[2][2],
            inverse[3][3] + inverse[4][4] + inverse[5][5],
        )
    } else {
        (0.0, inverse[0][0] + inverse[1][1] + inverse[2][2])
    };
    if !position_variance.is_finite()
        || !rotation_variance.is_finite()
        || position_variance < 0.0
        || rotation_variance < 0.0
    {
        return Err(CalibrationError::Underdetermined);
    }
    Ok(Fit {
        position,
        rotation,
        cost,
        position_variance,
        rotation_variance,
        iterations,
    })
}

type Row = [f64; 6];

/// The sum of squares of the residual, and the residual itself.
fn evaluate(
    position: Vec3,
    rotation: &Orientation,
    sightings: &[Sighting],
    beams: &[Vec3],
) -> Result<(f64, Vec<[f64; 3]>), CalibrationError> {
    let mut cost = 0.0;
    let mut residual = Vec::with_capacity(sightings.len());
    for (sighting, beam) in sightings.iter().zip(beams) {
        let to = subtract(sighting.target, position);
        let unit = normalise(to).ok_or(CalibrationError::OnTheFixture)?;
        let seen = turn(rotation, *beam);
        let r = [seen.x - unit.x, seen.y - unit.y, seen.z - unit.z];
        cost += r[0] * r[0] + r[1] * r[1] + r[2] * r[2];
        residual.push(r);
    }
    Ok((cost, residual))
}

/// The Jacobian of every residual component (three a sighting) with respect to
/// the unknowns — position first, then a small turn about the world's axes —
/// and the residual.
fn linearise(
    position: Vec3,
    rotation: &Orientation,
    sightings: &[Sighting],
    beams: &[Vec3],
    free_position: bool,
) -> Result<(Vec<Row>, Vec<[f64; 3]>), CalibrationError> {
    let (_, residual) = evaluate(position, rotation, sightings, beams)?;
    let mut rows = Vec::with_capacity(sightings.len() * 3);
    for (sighting, beam) in sightings.iter().zip(beams) {
        let to = subtract(sighting.target, position);
        let length = norm(to);
        let unit = normalise(to).ok_or(CalibrationError::OnTheFixture)?;
        let seen = turn(rotation, *beam);
        // d(R'b)/dω = −[R'b]×, and d(u)/dP = −(I − u uᵀ) / |M − P|.
        let skew = [
            [0.0, seen.z, -seen.y],
            [-seen.z, 0.0, seen.x],
            [seen.y, -seen.x, 0.0],
        ];
        let u = [unit.x, unit.y, unit.z];
        for component in 0..3 {
            let mut row: Row = [0.0; 6];
            if free_position {
                for (axis, slot) in row.iter_mut().take(3).enumerate() {
                    let identity = if component == axis { 1.0 } else { 0.0 };
                    *slot = (identity - u[component] * u[axis]) / length;
                }
                for (axis, slot) in row.iter_mut().skip(3).enumerate() {
                    *slot = skew[component][axis];
                }
            } else {
                for (axis, slot) in row.iter_mut().take(3).enumerate() {
                    *slot = skew[component][axis];
                }
            }
            rows.push(row);
        }
    }
    Ok((rows, residual))
}

/// `JᵀJ` and `Jᵀr`.
fn normal_equations(
    jacobian: &[Row],
    residual: &[[f64; 3]],
    unknowns: usize,
) -> ([[f64; 6]; 6], [f64; 6]) {
    let mut normal = [[0.0; 6]; 6];
    let mut gradient = [0.0; 6];
    for (index, row) in jacobian.iter().enumerate() {
        let r = residual[index / 3][index % 3];
        for a in 0..unknowns {
            gradient[a] += row[a] * r;
            for b in 0..unknowns {
                normal[a][b] += row[a] * row[b];
            }
        }
    }
    (normal, gradient)
}

/// Gaussian elimination with partial pivoting on a matrix that is first scaled
/// to unit diagonal, so a column in metres and one in radians are weighed alike.
fn solve(matrix: &[[f64; 6]; 6], right: &[f64; 6], n: usize) -> Option<[f64; 6]> {
    let scale: Vec<f64> = (0..n)
        .map(|k| matrix[k][k].max(1e-300).sqrt().recip())
        .collect();
    let mut a = [[0.0; 7]; 6];
    for i in 0..n {
        for j in 0..n {
            a[i][j] = matrix[i][j] * scale[i] * scale[j];
        }
        a[i][n] = right[i] * scale[i];
    }
    for column in 0..n {
        let pivot =
            (column..n).max_by(|&p, &q| a[p][column].abs().total_cmp(&a[q][column].abs()))?;
        if a[pivot][column].abs() < SINGULAR {
            return None;
        }
        a.swap(column, pivot);
        for row in column + 1..n {
            let factor = a[row][column] / a[column][column];
            let pivot_row = a[column];
            for (cell, held) in a[row].iter_mut().zip(pivot_row).take(n + 1).skip(column) {
                *cell -= factor * held;
            }
        }
    }
    let mut x = [0.0; 6];
    for row in (0..n).rev() {
        let mut sum = a[row][n];
        for k in row + 1..n {
            sum -= a[row][k] * x[k];
        }
        x[row] = sum / a[row][row];
    }
    let mut out = [0.0; 6];
    for k in 0..n {
        out[k] = x[k] * scale[k];
    }
    out.iter().all(|v| v.is_finite()).then_some(out)
}

/// The inverse of the normal matrix, or `None` if it is flat in any direction.
fn invert(matrix: &[[f64; 6]; 6], n: usize) -> Option<[[f64; 6]; 6]> {
    let mut inverse = [[0.0; 6]; 6];
    for column in 0..n {
        let mut unit = [0.0; 6];
        unit[column] = 1.0;
        let x = solve(matrix, &unit, n)?;
        for row in 0..n {
            inverse[row][column] = x[row];
        }
    }
    Some(inverse)
}

/// One step of the search: the position moves, and the rotation is turned by a
/// small rotation vector in the world frame.
fn apply_step(
    position: Vec3,
    rotation: &Orientation,
    step: &[f64; 6],
    free_position: bool,
) -> (Vec3, Orientation) {
    let (moved, turn_by) = if free_position {
        (
            Vec3 {
                x: position.x + step[0],
                y: position.y + step[1],
                z: position.z + step[2],
            },
            [step[3], step[4], step[5]],
        )
    } else {
        (position, [step[0], step[1], step[2]])
    };
    (moved, compose(&exponential(turn_by), rotation))
}

/// The rotation about the vector `w` by `|w|` radians (Rodrigues).
fn exponential(w: [f64; 3]) -> Orientation {
    let angle = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    if angle < 1e-15 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let (x, y, z) = (w[0] / angle, w[1] / angle, w[2] / angle);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

/// `left · right`.
fn compose(left: &Orientation, right: &Orientation) -> Orientation {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| left[i][k] * right[k][j]).sum();
        }
    }
    out
}

/// A turn of `degrees` about the vertical.
fn about_y(degrees: f64) -> Orientation {
    exponential([0.0, degrees.to_radians(), 0.0])
}

fn subtract(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn norm(v: Vec3) -> f64 {
    (v.x * v.x + v.y * v.y + v.z * v.z).sqrt()
}

/// A mark nearer than a centimetre to the fixture has no direction worth the
/// name — and a search that wandered onto one would "fit" by making its
/// direction anything at all.
const TOO_CLOSE: f64 = 0.01;
/// How near a mark a *solved* position may end up, in metres. A head half a
/// metre from the mark it was aimed at is not a rig, it is the search having
/// found the one place where that mark says nothing.
const SETTLED_ON_A_MARK: f64 = 0.5;

fn normalise(v: Vec3) -> Option<Vec3> {
    let length = norm(v);
    (length.is_finite() && length > TOO_CLOSE).then(|| Vec3 {
        x: v.x / length,
        y: v.y / length,
        z: v.z / length,
    })
}

/// The angle between two directions, in degrees, good to the last place even
/// when they are nearly the same.
fn angle_between(a: Vec3, b: Vec3) -> f64 {
    let cross = Vec3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    };
    let dot = a.x * b.x + a.y * b.y + a.z * b.z;
    norm(cross).atan2(dot).to_degrees()
}

// -- on the wire --------------------------------------------------------------

/// One sighting as a client sends it: the mark, and the **programmer values** of
/// pan and tilt the head had when the operator said *there* (`0..=65535`, the
/// merge's own, before the cable inverts them).
///
/// Values and not angles, because only the daemon knows a profile's travel, an
/// inverted channel and a mirrored axis ([`crate::AxisModel`]): a client that
/// turned them into degrees would be a second opinion about what a pan of 40 %
/// is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct CalibrationSighting {
    /// Where the mark is, in metres of show space.
    pub target: Vec3,
    /// Pan, as the programmer holds it.
    pub pan: u16,
    /// Tilt, as the programmer holds it.
    pub tilt: u16,
}

/// How one sighting sits in the solved pose.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct SightingFit {
    /// Degrees between where the beam went and where the pose says the mark is.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub residual: f64,
    /// Metres from the solved head to the mark - a mark close to the head says
    /// little about where it is.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub distance: f64,
}

/// Where a head must be driven to hit a point - `Answer::CalibrationAim`'s row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct AimedAt {
    /// Pan, as a programmer value.
    pub pan: u16,
    /// Tilt, as a programmer value.
    pub tilt: u16,
    /// Whether the head's travel reaches the point at all.
    pub reached: bool,
}

/// What the two calibration queries answer with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum CalibrationAnswer {
    /// A pose that fits.
    Solved {
        /// Where the head hangs, in metres.
        position: Vec3,
        /// How it is turned, in degrees.
        rotation: Vec3,
        /// One row per sighting, in the order they were sent.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        fits: Vec<SightingFit>,
        /// Root mean square of the residuals, in degrees.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        rms: f64,
        /// The largest residual, in degrees.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        worst: f64,
        /// One standard deviation of the position, in metres; nought when it
        /// was held.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        position_sigma: f64,
        /// One standard deviation of the rotation, in degrees.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        rotation_sigma: f64,
        /// Equations beyond the unknowns. **Nought means nothing was checked.**
        spare: u32,
        /// How many iterations the search took.
        iterations: u32,
    },
    /// No pose: the daemon's own words for why.
    Refused {
        /// What was wrong and what to do about it.
        reason: String,
    },
    /// Where to drive a head to hit a point - what `Query::CalibrationAim`
    /// answers.
    Aimed {
        /// The values, or `None` for a fixture with no pan and tilt to drive and
        /// for a point that is on the fixture itself.
        aim: Option<AimedAt>,
    },
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{CalibrationError, Hold, Sighting, beam, calibrate};
    use crate::{Vec3, orientation, turn};

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    /// What the world looks like to a head hung at `position` and `rotation`
    /// when it is told to point at `pan` and `tilt` and the beam lands `reach`
    /// metres out: the point that is **true**.
    fn land(position: Vec3, rotation: Vec3, pan: f64, tilt: f64, reach: f64) -> Vec3 {
        let direction = turn(&orientation(rotation), beam(pan, tilt));
        v(
            position.x + direction.x * reach,
            position.y + direction.y * reach,
            position.z + direction.z * reach,
        )
    }

    /// A spread of sightings that pins a pose well: pans round the compass,
    /// tilts in the range a head throws light at a floor.
    fn spread(position: Vec3, rotation: Vec3, count: usize) -> Vec<Sighting> {
        (0..count)
            .map(|k| {
                let pan = -150.0 + 300.0 * k as f64 / count.max(2) as f64 - 10.0;
                let tilt = 25.0 + 12.0 * ((k * 7) % 5) as f64;
                let reach = 5.0 + ((k * 3) % 4) as f64 * 1.7;
                Sighting {
                    target: land(position, rotation, pan, tilt, reach),
                    pan,
                    tilt,
                }
            })
            .collect()
    }

    fn near(a: Vec3, b: Vec3, tolerance: f64) -> bool {
        (a.x - b.x).abs() < tolerance
            && (a.y - b.y).abs() < tolerance
            && (a.z - b.z).abs() < tolerance
    }

    #[test]
    fn the_beam_is_the_viewers_yoke() {
        // Straight down at nought; tipped towards the front at ninety.
        assert!((beam(0.0, 0.0).y + 1.0).abs() < 1e-12);
        let tipped = beam(0.0, 90.0);
        assert!((tipped.z + 1.0).abs() < 1e-12 && tipped.y.abs() < 1e-12);
        // And it is `aim`'s inverse: whatever `aim` answers for a point puts
        // the beam through it.
        let position = v(1.0, 6.0, -2.0);
        let rotation = v(12.0, 33.0, -5.0);
        let target = v(-3.0, 0.0, 4.0);
        let travel = crate::Travel {
            from: -270.0,
            to: 270.0,
        };
        let tilt_travel = crate::Travel {
            from: -135.0,
            to: 135.0,
        };
        let aimed = crate::aim(
            position,
            &orientation(rotation),
            target,
            travel,
            tilt_travel,
            (0.0, 0.0),
        )
        .unwrap();
        let along = turn(&orientation(rotation), beam(aimed.pan, aimed.tilt));
        let wanted = v(
            target.x - position.x,
            target.y - position.y,
            target.z - position.z,
        );
        let length = (wanted.x * wanted.x + wanted.y * wanted.y + wanted.z * wanted.z).sqrt();
        assert!(near(
            along,
            v(wanted.x / length, wanted.y / length, wanted.z / length),
            1e-9
        ));
    }

    #[test]
    fn a_pose_is_found_from_a_start_that_is_not_far_off() {
        let truth = (v(2.0, 6.5, -1.5), v(8.0, 25.0, -4.0));
        let sightings = spread(truth.0, truth.1, 5);
        let got = calibrate(
            v(1.6, 6.0, -1.0),
            v(0.0, 15.0, 0.0),
            &sightings,
            Hold::Nothing,
        )
        .unwrap();
        assert!(near(got.position, truth.0, 1e-5), "{:?}", got.position);
        assert!(near(got.rotation, truth.1, 1e-4), "{:?}", got.rotation);
        assert!(got.rms < 1e-6, "rms {}", got.rms);
        assert_eq!(got.spare, 4);
    }

    #[test]
    fn a_rig_whose_rotation_was_never_entered_finds_its_heading() {
        // The plan has the head at its measured place and no rotation; it is
        // really turned 180° about the vertical.
        let truth = (v(-1.0, 7.0, 3.0), v(0.0, 180.0, 0.0));
        let sightings = spread(truth.0, truth.1, 4);
        let got = calibrate(truth.0, Vec3::ZERO, &sightings, Hold::Position).unwrap();
        assert_eq!(got.position, truth.0, "held position moved");
        assert!(got.rms < 1e-6, "rms {}", got.rms);
        let again = turn(&orientation(got.rotation), beam(0.0, 30.0));
        let real = turn(&orientation(truth.1), beam(0.0, 30.0));
        assert!(near(again, real, 1e-6), "{again:?} vs {real:?}");
    }

    #[test]
    fn a_held_position_is_returned_untouched_even_when_it_is_wrong() {
        let truth = (v(2.0, 6.0, 0.0), v(0.0, 0.0, 0.0));
        let sightings = spread(truth.0, truth.1, 5);
        let wrong = v(2.5, 6.0, 0.0);
        let got = calibrate(wrong, truth.1, &sightings, Hold::Position).unwrap();
        assert_eq!(got.position, wrong);
        assert!(got.rms > 0.5, "the disagreement must show: {}", got.rms);
        assert!(got.position_sigma.abs() < f64::EPSILON);
    }

    #[test]
    fn a_mistyped_mark_is_the_one_with_the_big_residual() {
        let truth = (v(0.0, 6.0, 0.0), v(0.0, 0.0, 0.0));
        let mut sightings = spread(truth.0, truth.1, 7);
        sightings[3].target.x += 1.5;
        let got = calibrate(truth.0, truth.1, &sightings, Hold::Nothing).unwrap();
        let worst = got
            .residuals
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert_eq!(worst.0, 3, "{:?}", got.residuals);
        assert!(got.worst > 2.0 * got.rms, "{got:?}");
    }

    #[test]
    fn noise_in_the_aim_costs_a_little_position() {
        // A deterministic scatter of ±0.15° on every angle.
        let truth = (v(1.0, 6.0, -2.0), v(5.0, 40.0, 3.0));
        let mut sightings = spread(truth.0, truth.1, 8);
        let mut state = 0x9E37_79B9_u32;
        let mut wobble = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (f64::from(state >> 8) / f64::from(1_u32 << 24) - 0.5) * 0.3
        };
        for s in &mut sightings {
            s.pan += wobble();
            s.tilt += wobble();
        }
        let got = calibrate(v(0.0, 5.0, -1.0), Vec3::ZERO, &sightings, Hold::Nothing).unwrap();
        assert!(near(got.position, truth.0, 0.3), "{:?}", got.position);
        assert!(got.rms < 0.3, "rms {}", got.rms);
        assert!(
            got.position_sigma > 0.0 && got.position_sigma < 0.5,
            "{}",
            got.position_sigma
        );
    }

    #[test]
    fn too_few_sightings_are_refused_with_the_number_that_is_needed() {
        let s = spread(v(0.0, 6.0, 0.0), Vec3::ZERO, 2);
        assert_eq!(
            calibrate(Vec3::ZERO, Vec3::ZERO, &s, Hold::Nothing),
            Err(CalibrationError::TooFew { have: 2, need: 3 })
        );
        assert_eq!(
            calibrate(Vec3::ZERO, Vec3::ZERO, &s[..1], Hold::Position),
            Err(CalibrationError::TooFew { have: 1, need: 2 })
        );
    }

    #[test]
    fn the_same_sighting_three_times_decides_nothing() {
        let one = spread(v(0.0, 6.0, 0.0), Vec3::ZERO, 1)[0];
        let result = calibrate(
            v(0.0, 6.0, 0.0),
            Vec3::ZERO,
            &[one, one, one],
            Hold::Nothing,
        );
        assert_eq!(result, Err(CalibrationError::Underdetermined));
        // Nor does it fix a rotation, which one beam leaves free about itself.
        let result = calibrate(v(0.0, 6.0, 0.0), Vec3::ZERO, &[one, one], Hold::Position);
        assert_eq!(result, Err(CalibrationError::Underdetermined));
    }

    #[test]
    fn marks_in_one_line_decide_nothing_about_the_position() {
        // Three marks on a line through the head's own foot: the head can be
        // anywhere along the perpendicular and the sightings agree.
        let truth = v(0.0, 6.0, 0.0);
        let marks = [v(-2.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(4.0, 0.0, 0.0)];
        let sightings: Vec<Sighting> = marks
            .iter()
            .map(|&target| {
                let down = v(target.x - truth.x, target.y - truth.y, target.z - truth.z);
                let tilt = (down.x.hypot(down.z) / -down.y).atan().to_degrees();
                Sighting {
                    target,
                    // Positive pan swings the beam towards -x.
                    pan: if target.x < truth.x { 90.0 } else { -90.0 },
                    tilt,
                }
            })
            .collect();
        let result = calibrate(truth, Vec3::ZERO, &sightings, Hold::Nothing);
        assert!(
            matches!(result, Err(CalibrationError::Underdetermined)),
            "{result:?}"
        );
    }

    #[test]
    fn a_mark_on_the_fixture_has_no_direction() {
        let at = v(0.0, 6.0, 0.0);
        let mut s = spread(at, Vec3::ZERO, 3);
        s[1].target = at;
        assert_eq!(
            calibrate(at, Vec3::ZERO, &s, Hold::Nothing),
            Err(CalibrationError::OnTheFixture)
        );
    }

    #[test]
    fn numbers_that_are_not_numbers_are_refused() {
        let mut s = spread(v(0.0, 6.0, 0.0), Vec3::ZERO, 3);
        s[0].pan = f64::NAN;
        assert_eq!(
            calibrate(Vec3::ZERO, Vec3::ZERO, &s, Hold::Nothing),
            Err(CalibrationError::NotReachable)
        );
        let s = spread(v(0.0, 6.0, 0.0), Vec3::ZERO, 3);
        assert_eq!(
            calibrate(v(5_000.0, 0.0, 0.0), Vec3::ZERO, &s, Hold::Nothing),
            Err(CalibrationError::NotReachable)
        );
    }

    proptest! {
        /// Whatever the pose and however the plan is out, five sightings of a
        /// well-spread set of marks bring back the pose — to the accuracy of
        /// the sightings, which here are exact.
        #[test]
        fn a_pose_survives_the_round_trip(
            px in -6.0_f64..6.0, py in 4.0_f64..9.0, pz in -6.0_f64..6.0,
            rx in -30.0_f64..30.0, ry in -179.0_f64..179.0, rz in -30.0_f64..30.0,
            ex in -0.6_f64..0.6, ey in -0.6_f64..0.6, ez in -0.6_f64..0.6,
            er in -8.0_f64..8.0,
        ) {
            let position = v(px, py, pz);
            let rotation = v(rx, ry, rz);
            let sightings = spread(position, rotation, 6);
            let start = v(px + ex, py + ey, pz + ez);
            let start_rotation = v(rx + er, ry + er, rz - er);
            let got = calibrate(start, start_rotation, &sightings, Hold::Nothing).unwrap();
            prop_assert!(got.rms < 1e-4, "rms {} for {:?} {:?}", got.rms, position, rotation);
            prop_assert!(near(got.position, position, 1e-3), "{:?} vs {:?}", got.position, position);
            // Rotation is checked as a direction, which has one answer where
            // the Euler triple has several.
            let probe = beam(40.0, 50.0);
            let a = turn(&orientation(got.rotation), probe);
            let b = turn(&orientation(rotation), probe);
            prop_assert!(near(a, b, 1e-4), "{a:?} vs {b:?}");
        }

        /// A held position is never moved, and with it held and the plan
        /// right, the rotation comes back from two sightings.
        #[test]
        fn a_held_position_never_moves(
            px in -6.0_f64..6.0, py in 4.0_f64..9.0, pz in -6.0_f64..6.0,
            ry in -179.0_f64..179.0, rx in -20.0_f64..20.0,
        ) {
            let position = v(px, py, pz);
            let rotation = v(rx, ry, 0.0);
            let sightings = spread(position, rotation, 3);
            let got = calibrate(position, Vec3::ZERO, &sightings, Hold::Position).unwrap();
            prop_assert_eq!(got.position, position);
            prop_assert!(got.rms < 1e-4, "rms {}", got.rms);
        }
    }
}
