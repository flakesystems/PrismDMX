/**
 * Show space, as the 3D viewer reads it — **S30**.
 *
 * `prism_domain::placement` is the definition and this is its second reader.
 * The two have to agree to the last digit, because a rig drawn with the other
 * Euler order is a rig drawn somewhere else; `space.test.ts` holds this file to
 * the matrices `crates/prismd/tests/ui_viewer.rs` recorded out of the Rust
 * function, so they are held to each other rather than to their comments.
 *
 * # The frame
 *
 * Metres, **Y up**: `x` across the stage, `y` height, `z` depth growing
 * **upstage**, away from the audience. A left-handed frame — a camera standing
 * downstage and looking upstage sees `+x` on its right.
 *
 * # A rotation
 *
 * Three angles in degrees, applied **Z, then X, then Y**: {@link orientation}
 * is `Ry · Rx · Rz`. At nought a fixture hangs as its profile describes it,
 * which is beam straight down.
 *
 * Nothing in here allocates in a loop that runs per frame except where it says
 * so; the painter builds on these and runs thirty times a second.
 */

/** A point or a direction. */
export interface V3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/** A 3×3 matrix by rows, the shape `prism_domain::Orientation` has. */
export type Mat3 = readonly [
  readonly [number, number, number],
  readonly [number, number, number],
  readonly [number, number, number],
];

/** The origin. */
export const ORIGIN: V3 = { x: 0, y: 0, z: 0 };

/** Straight down, which is where a hanging beam points. */
export const DOWN: V3 = { x: 0, y: -1, z: 0 };

/** The identity. */
export const IDENTITY: Mat3 = [
  [1, 0, 0],
  [0, 1, 0],
  [0, 0, 1],
];

/** A vector. */
export function v3(x: number, y: number, z: number): V3 {
  return { x, y, z };
}

/** `a + b`. */
export function add(a: V3, b: V3): V3 {
  return { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z };
}

/** `a − b`. */
export function sub(a: V3, b: V3): V3 {
  return { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z };
}

/** `a · k`. */
export function scale(a: V3, k: number): V3 {
  return { x: a.x * k, y: a.y * k, z: a.z * k };
}

/** The dot product. */
export function dot(a: V3, b: V3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

/** The cross product, by the formula — which way it points is the frame's business. */
export function cross(a: V3, b: V3): V3 {
  return {
    x: a.y * b.z - a.z * b.y,
    y: a.z * b.x - a.x * b.z,
    z: a.x * b.y - a.y * b.x,
  };
}

/** How long it is. */
export function length(a: V3): number {
  return Math.sqrt(dot(a, a));
}

/**
 * The same direction at length one, or `fallback` for a vector too short to
 * have a direction — never a `NaN`, which would travel into every point drawn
 * from it.
 */
export function normalize(a: V3, fallback: V3 = DOWN): V3 {
  const size = length(a);
  if (!Number.isFinite(size) || size < 1e-12) {
    return fallback;
  }
  return scale(a, 1 / size);
}

/** Degrees as radians. */
function radians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

/**
 * The rotation a fixture's `rotation` stands for — `Ry · Rx · Rz`, exactly as
 * `prism_domain::orientation` writes it, term for term.
 */
export function orientation(rotation: V3): Mat3 {
  const sa = Math.sin(radians(rotation.x));
  const ca = Math.cos(radians(rotation.x));
  const sb = Math.sin(radians(rotation.y));
  const cb = Math.cos(radians(rotation.y));
  const sc = Math.sin(radians(rotation.z));
  const cc = Math.cos(radians(rotation.z));
  return [
    [cb * cc + sb * sa * sc, -cb * sc + sb * sa * cc, sb * ca],
    [ca * sc, ca * cc, -sa],
    [-sb * cc + cb * sa * sc, sb * sc + cb * sa * cc, cb * ca],
  ];
}

/** `m · a`. */
export function turn(m: Mat3, a: V3): V3 {
  return {
    x: m[0][0] * a.x + m[0][1] * a.y + m[0][2] * a.z,
    y: m[1][0] * a.x + m[1][1] * a.y + m[1][2] * a.z,
    z: m[2][0] * a.x + m[2][1] * a.y + m[2][2] * a.z,
  };
}

/** `a · b`. */
export function multiply(a: Mat3, b: Mat3): Mat3 {
  const row = (r: Mat3[number]): Mat3[number] => [
    r[0] * b[0][0] + r[1] * b[1][0] + r[2] * b[2][0],
    r[0] * b[0][1] + r[1] * b[1][1] + r[2] * b[2][1],
    r[0] * b[0][2] + r[1] * b[1][2] + r[2] * b[2][2],
  ];
  return [row(a[0]), row(a[1]), row(a[2])];
}

/**
 * Where a moving head's beam is turned by its pan and tilt: **pan about the
 * fixture's own vertical, then tilt about the yoke's across axis** —
 * `Ry(pan) · Rx(tilt)`, applied inside the fixture's orientation.
 *
 * The same order `orientation` uses for heading and tip, and for the same
 * reason: a yoke turns first and the head tips in it. Positive pan turns the
 * way a positive heading does, and positive tilt tips a hanging beam towards
 * where its front faces at pan nought — downstage for a fixture hung at
 * nought. A profile whose motor runs the other way has `invert` on the channel,
 * and that is applied before this sees a number.
 */
export function yoke(pan: number, tilt: number): Mat3 {
  return orientation({ x: tilt, y: pan, z: 0 });
}

/**
 * Two unit vectors at right angles to `axis` and to each other, for drawing a
 * circle around it.
 */
export function around(axis: V3): readonly [V3, V3] {
  // Whichever world axis is furthest from the beam is crossed with it, so a
  // beam pointing straight down does not cross itself into nothing.
  const helper = Math.abs(axis.y) < 0.9 ? v3(0, 1, 0) : v3(1, 0, 0);
  const first = normalize(cross(axis, helper), v3(1, 0, 0));
  const second = normalize(cross(axis, first), v3(0, 0, 1));
  return [first, second];
}
