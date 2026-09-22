/**
 * The three coordinate systems — **S30b**. Each conversion is tested by what it
 * must preserve rather than by its entries: a turned point is the same point
 * whichever space it is turned in, and a device built in GDTF space stands up
 * in the viewer's.
 */

import { Vector3 } from "three";
import { describe, expect, it } from "vitest";

import type { DeviceNode } from "../device";
import { orientation, turn, v3 } from "../space";
import { GDTF_TO_THREE, GLTF_TO_GDTF, gdtfMatrix, gdtfSize, showRotationToThree, showToThree } from "./coords";

function node(extra: Partial<DeviceNode>): DeviceNode {
  return {
    index: 0,
    name: "Node",
    parent: null,
    kind: "Geometry",
    model: null,
    primitive: null,
    size: v3(0, 0, 0),
    position: v3(0, 0, 0),
    xAxis: v3(1, 0, 0),
    yAxis: v3(0, 1, 0),
    zAxis: v3(0, 0, 1),
    beam: null,
    ...extra,
  };
}

function close(actual: Vector3, expected: readonly [number, number, number]): void {
  expect(actual.x).toBeCloseTo(expected[0], 9);
  expect(actual.y).toBeCloseTo(expected[1], 9);
  expect(actual.z).toBeCloseTo(expected[2], 9);
}

describe("show space in three.js", () => {
  it("flips z, so upstage is three's −Z", () => {
    expect(showToThree(v3(1, 2, 3))).toEqual([1, 2, -3]);
  });

  it("turns a point the way the show's own rotation turns it", () => {
    for (const rotation of [v3(0, 0, 0), v3(20, 0, 0), v3(-150, 35, 90), v3(12, -80, 200)]) {
      const matrix = orientation(rotation);
      const point = v3(0.3, -1.2, 2.5);
      const turned = new Vector3(...showToThree(point)).applyMatrix4(showRotationToThree(matrix));
      close(turned, showToThree(turn(matrix, point)));
    }
  });
});

describe("GDTF space in three.js", () => {
  it("stands GDTF's Z up as three's Y, and GDTF's −Z (a beam) as down", () => {
    close(new Vector3(0, 0, 1).applyMatrix4(GDTF_TO_THREE), [0, 1, 0]);
    close(new Vector3(0, 0, -1).applyMatrix4(GDTF_TO_THREE), [0, -1, 0]);
    // GDTF's Y is away from the operator: upstage, three's −Z.
    close(new Vector3(0, 1, 0).applyMatrix4(GDTF_TO_THREE), [0, 0, -1]);
  });

  it("stands a Y-up glTF model up in its geometry", () => {
    // The Robin T1's yoke runs along its model's Y; in GDTF that is Z, and in
    // the viewer it is up again.
    const up = new Vector3(0, 1, 0).applyMatrix4(GLTF_TO_GDTF).applyMatrix4(GDTF_TO_THREE);
    close(up, [0, 1, 0]);
  });

  it("rebuilds a node's own GDTF matrix from the axes the domain carries in show space", () => {
    // A beam 0.2 m below its parent: GDTF translation (0, 0, −0.2), carried in
    // show axes as y = −0.2.
    const beam = node({ position: v3(0, -0.2, 0) });
    close(new Vector3(0, 0, 0).applyMatrix4(gdtfMatrix(beam)), [0, 0, -0.2]);
    // A node turned a quarter about GDTF's Z: GDTF X goes to GDTF Y, which the
    // domain carries as show's z.
    const turned = node({ xAxis: v3(0, 0, 1), zAxis: v3(-1, 0, 0) });
    close(new Vector3(1, 0, 0).applyMatrix4(gdtfMatrix(turned)), [0, 1, 0]);
    close(new Vector3(0, 1, 0).applyMatrix4(gdtfMatrix(turned)), [-1, 0, 0]);
  });

  it("reads a model's box as length, width and height", () => {
    // Show axes: across 0.3, height 0.5, depth 0.2.
    expect(gdtfSize(node({ size: v3(0.3, 0.5, 0.2) }))).toEqual([0.3, 0.2, 0.5]);
  });
});
