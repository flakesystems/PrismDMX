/**
 * Where the viewer looks from — and it is **client-local**.
 *
 * `ARCHITECTURE_SPEC.md` §4.2 names the 3D viewer's camera beside scroll
 * position and zoom as state that is legitimately different per screen: two
 * operators looking at one rig from two angles is the normal case, and a
 * camera the console could move would be one it moved under somebody else. So
 * this is never a command, never a delta and never React state: the window
 * holds one in a ref, the pointer moves it, and the loop reads it.
 *
 * # The model
 *
 * An orbit: a **target** on the stage, a **distance** from it, a **heading**
 * round it and an **elevation** above it. Heading nought is standing downstage
 * — in the audience — looking upstage, which is the view a lighting designer
 * sits in. The projection is a plain perspective one with the vertical field
 * of view fixed; nothing here needs more.
 */

import type { V3 } from "./space";
import { add, cross, dot, normalize, scale, sub, v3 } from "./space";

/** The vertical field of view, in degrees. */
export const FIELD_OF_VIEW = 50;

/** How close to a point the projection still draws it, in metres. */
export const NEAR = 0.05;

/** The steepest the camera may look, in degrees — straight down is a singularity. */
export const STEEPEST = 89;

/** The furthest the camera may stand, in metres. */
export const FURTHEST = 400;

/** The nearest the camera may stand, in metres. */
export const NEAREST = 1;

/** An orbit camera. Mutated in place by the pointer; read by the loop. */
export interface Camera {
  /** What it looks at. */
  target: V3;
  /** How far from it, in metres. */
  distance: number;
  /** Degrees round the target; nought is downstage looking upstage. */
  heading: number;
  /** Degrees above the stage's horizontal. */
  elevation: number;
  /** Bumped on every change, so a loop can tell it moved without comparing. */
  version: number;
}

/** The four views a toolbar offers, by what an operator calls them. */
export const VIEWS = {
  /** From the audience, a little above head height. */
  front: { heading: 0, elevation: 12 },
  /** From above, which is a plot. */
  top: { heading: 0, elevation: STEEPEST },
  /** From stage right. */
  side: { heading: -90, elevation: 8 },
  /** From the front corner, which shows depth and height at once. */
  perspective: { heading: -35, elevation: 28 },
} as const;

/** The name of one of {@link VIEWS}. */
export type ViewName = keyof typeof VIEWS;

/** A camera looking at a stage the size of a small venue, from the front corner. */
export function newCamera(): Camera {
  return {
    target: v3(0, 2, 0),
    distance: 16,
    heading: VIEWS.perspective.heading,
    elevation: VIEWS.perspective.elevation,
    version: 0,
  };
}

/** Where the camera stands. */
export function eyeOf(camera: Camera): V3 {
  const heading = (camera.heading * Math.PI) / 180;
  const elevation = (camera.elevation * Math.PI) / 180;
  const flat = Math.cos(elevation);
  // Heading nought stands at −z (downstage) and looks towards +z.
  const offset = v3(
    Math.sin(heading) * flat * camera.distance,
    Math.sin(elevation) * camera.distance,
    -Math.cos(heading) * flat * camera.distance,
  );
  return add(camera.target, offset);
}

/** A camera's frame, worked out once per paint. */
export interface Lens {
  /** Where it stands. */
  readonly eye: V3;
  /** Which way is right on the screen. */
  readonly right: V3;
  /** Which way is up on the screen. */
  readonly up: V3;
  /** Which way it looks. */
  readonly forward: V3;
  /** Pixels per unit of `x / depth`. */
  readonly focal: number;
  /** The middle of the surface. */
  readonly cx: number;
  /** The middle of the surface. */
  readonly cy: number;
}

/** The camera's frame for a surface `width` × `height` pixels. */
export function lensOf(camera: Camera, width: number, height: number): Lens {
  const eye = eyeOf(camera);
  const forward = normalize(sub(camera.target, eye), v3(0, 0, 1));
  // In this left-handed frame, *up × forward* is right.
  const right = normalize(cross(v3(0, 1, 0), forward), v3(1, 0, 0));
  const up = cross(forward, right);
  const focal = height / 2 / Math.tan((FIELD_OF_VIEW * Math.PI) / 360);
  return { eye, right, up, forward, focal, cx: width / 2, cy: height / 2 };
}

/** How far in front of the camera a point is, in metres. */
export function depthOf(lens: Lens, point: V3): number {
  return dot(sub(point, lens.eye), lens.forward);
}

/**
 * Writes `point`'s place on the surface into `out[offset]`, `out[offset + 1]`,
 * and answers whether it is in front of the camera. A point behind it is not
 * written — there is nowhere on the screen it could go.
 */
export function project(lens: Lens, point: V3, out: Float64Array | number[], offset = 0): boolean {
  const relative = sub(point, lens.eye);
  const depth = dot(relative, lens.forward);
  if (depth < NEAR) {
    return false;
  }
  out[offset] = lens.cx + (dot(relative, lens.right) / depth) * lens.focal;
  out[offset + 1] = lens.cy - (dot(relative, lens.up) / depth) * lens.focal;
  return true;
}

/**
 * The point where the segment from `a` to `b` crosses the near plane, for a
 * segment with one end behind the camera.
 */
export function clipToNear(lens: Lens, a: V3, b: V3): V3 {
  const da = depthOf(lens, a);
  const db = depthOf(lens, b);
  const t = (NEAR * 1.0001 - da) / (db - da);
  return add(a, scale(sub(b, a), t));
}

/** Turns the camera to one of the named views, keeping where it looks. */
export function look(camera: Camera, view: ViewName): void {
  camera.heading = VIEWS[view].heading;
  camera.elevation = VIEWS[view].elevation;
  camera.version += 1;
}

/** Orbits by a drag of `dx`, `dy` pixels. */
export function orbit(camera: Camera, dx: number, dy: number): void {
  camera.heading = wrap(camera.heading - dx * 0.4);
  camera.elevation = clamp(camera.elevation + dy * 0.3, -STEEPEST, STEEPEST);
  camera.version += 1;
}

/** Moves the target across the screen by a drag of `dx`, `dy` pixels. */
export function slide(camera: Camera, dx: number, dy: number, height: number): void {
  const lens = lensOf(camera, 1, Math.max(1, height));
  const metresPerPixel = camera.distance / Math.max(1, lens.focal);
  camera.target = add(
    camera.target,
    add(scale(lens.right, -dx * metresPerPixel), scale(lens.up, dy * metresPerPixel)),
  );
  camera.version += 1;
}

/** Moves closer (`steps` < 0) or further away, a tenth at a step. */
export function zoom(camera: Camera, steps: number): void {
  camera.distance = clamp(camera.distance * Math.pow(1.1, steps), NEAREST, FURTHEST);
  camera.version += 1;
}

/**
 * Aims at `points` and stands back far enough to see them all, keeping the
 * heading and the elevation. An empty rig is framed as a small stage.
 */
export function frameAll(camera: Camera, points: readonly V3[]): void {
  if (points.length === 0) {
    camera.target = v3(0, 2, 0);
    camera.distance = 16;
    camera.version += 1;
    return;
  }
  let minX = Infinity;
  let minY = Infinity;
  let minZ = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  let maxZ = -Infinity;
  for (const point of points) {
    minX = Math.min(minX, point.x);
    minY = Math.min(minY, point.y, 0);
    minZ = Math.min(minZ, point.z);
    maxX = Math.max(maxX, point.x);
    maxY = Math.max(maxY, point.y);
    maxZ = Math.max(maxZ, point.z);
  }
  camera.target = v3((minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2);
  // The floor under the rig is part of the picture, so the stage is framed
  // with room round it rather than to the fixtures' own bounding box.
  const radius = Math.max(3, Math.hypot(maxX - minX, maxY - minY, maxZ - minZ) / 2 + 2);
  const half = (FIELD_OF_VIEW * Math.PI) / 360;
  camera.distance = clamp(radius / Math.sin(half), NEAREST, FURTHEST);
  camera.version += 1;
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

function wrap(degrees: number): number {
  let value = degrees % 360;
  if (value <= -180) {
    value += 360;
  }
  if (value > 180) {
    value -= 360;
  }
  return value;
}
