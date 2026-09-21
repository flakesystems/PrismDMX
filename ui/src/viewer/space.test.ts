/**
 * Show space in TypeScript, held to **Rust's** answers rather than to its own.
 *
 * `prism_domain::orientation` is the definition. The recording carries its
 * matrix for every rotation the script hangs a fixture at, and this file's
 * matrix has to match it to the last digit that matters — a viewer with the
 * other Euler order would draw every rig somewhere else, and would pass every
 * test that only checked it against itself.
 */

import { describe, expect, it } from "vitest";

import { viewerRecording } from "../testing/viewer-recording";
import type { Mat3, V3 } from "./space";
import {
  DOWN,
  IDENTITY,
  around,
  cross,
  dot,
  length,
  multiply,
  normalize,
  orientation,
  sub,
  turn,
  v3,
  yoke,
} from "./space";

function close(actual: V3, expected: V3): void {
  expect(actual.x).toBeCloseTo(expected.x, 9);
  expect(actual.y).toBeCloseTo(expected.y, 9);
  expect(actual.z).toBeCloseTo(expected.z, 9);
}

function sameMatrix(actual: Mat3, expected: readonly (readonly number[])[]): void {
  for (let row = 0; row < 3; row += 1) {
    for (let column = 0; column < 3; column += 1) {
      expect(actual[row as 0 | 1 | 2][column as 0 | 1 | 2]).toBeCloseTo(
        expected[row]?.[column] ?? Number.NaN,
        12,
      );
    }
  }
}

describe("a fixture's rotation", () => {
  it("is the matrix Rust computes, for every rotation the recording hangs", () => {
    expect(viewerRecording.orientations.length).toBeGreaterThanOrEqual(3);
    for (const { rotation, matrix } of viewerRecording.orientations) {
      sameMatrix(orientation(rotation), matrix);
    }
  });

  it("points a hanging beam where the Rust module's table says", () => {
    close(turn(orientation(v3(0, 0, 0)), DOWN), DOWN);
    close(turn(orientation(v3(90, 0, 0)), DOWN), v3(0, 0, -1));
    close(turn(orientation(v3(-90, 0, 0)), DOWN), v3(0, 0, 1));
    close(turn(orientation(v3(180, 0, 0)), DOWN), v3(0, 1, 0));
    close(turn(orientation(v3(0, 0, 90)), DOWN), v3(1, 0, 0));
  });

  it("applies the heading last", () => {
    close(turn(orientation(v3(90, 90, 0)), DOWN), v3(-1, 0, 0));
  });
});

describe("a yoke", () => {
  it("is nothing at pan and tilt nought", () => {
    sameMatrix(yoke(0, 0), IDENTITY);
  });

  it("tips a hanging beam downstage with positive tilt, and pan swings the tip round", () => {
    const tipped = turn(yoke(0, 30), DOWN);
    expect(tipped.z).toBeLessThan(0);
    expect(tipped.y).toBeCloseTo(-Math.cos(Math.PI / 6), 9);
    // Pan a quarter turn, then the same tilt: the tip is now across the stage.
    const swung = turn(yoke(90, 30), DOWN);
    expect(Math.abs(swung.x)).toBeCloseTo(0.5, 9);
    expect(swung.z).toBeCloseTo(0, 9);
  });
});

describe("the arithmetic", () => {
  it("multiplies in the order it is written", () => {
    const a = orientation(v3(10, 20, 30));
    const b = orientation(v3(-40, 5, 70));
    const product = multiply(a, b);
    const point = v3(0.3, -1.2, 2);
    close(turn(product, point), turn(a, turn(b, point)));
    sameMatrix(multiply(IDENTITY, a), a);
  });

  it("never answers a direction of no length", () => {
    close(normalize(v3(0, 0, 0)), DOWN);
    close(normalize(v3(Number.NaN, 0, 0), v3(1, 0, 0)), v3(1, 0, 0));
    expect(length(normalize(v3(3, 4, 12)))).toBeCloseTo(1, 12);
  });

  it("draws a circle round any axis, including straight down", () => {
    for (const axis of [DOWN, v3(0, 1, 0), normalize(v3(1, -2, 0.5)), v3(1, 0, 0)]) {
      const [u, w] = around(axis);
      expect(dot(u, axis)).toBeCloseTo(0, 12);
      expect(dot(w, axis)).toBeCloseTo(0, 12);
      expect(dot(u, w)).toBeCloseTo(0, 12);
      expect(length(u)).toBeCloseTo(1, 12);
      expect(length(w)).toBeCloseTo(1, 12);
    }
    close(cross(v3(1, 0, 0), v3(0, 1, 0)), v3(0, 0, 1));
    close(sub(v3(1, 2, 3), v3(1, 1, 1)), v3(0, 1, 2));
  });
});
