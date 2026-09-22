/**
 * Three coordinate systems meet in the visualiser, and this is the one place
 * they are converted — **S30b**.
 *
 * - **Show space** (`prism_domain::placement`): metres, Y up, `z` growing
 *   upstage. Left-handed. Where a fixture *hangs* is stated in it.
 * - **GDTF space**: metres, Z up, Y away from the operator. Right-handed. What
 *   a device *is* — its geometry tree — is stated in it; the domain carries it
 *   converted to show axes, and {@link gdtfMatrix} converts it back, because a
 *   device is built most simply in the frame its file was written in.
 * - **three.js space**: Y up, the camera's default view along −Z. Right-handed.
 *   Show's upstage is three's −Z; GDTF's Z-up is a quarter turn about X.
 *
 * A glTF model inside a GDTF is Y-up, as glTF requires; the Robin T1's
 * `yoke.glb` runs from −394 to 0 along its Y, which is its height. So a model
 * is turned a quarter about X to stand in its geometry — {@link GLTF_TO_GDTF}.
 */

import { Matrix4 } from "three";

import type { DeviceNode } from "../device";
import type { Mat3, V3 } from "../space";

/** Show space to three.js: `z` flips. */
export function showToThree(point: V3): [number, number, number] {
  return [point.x, point.y, -point.z];
}

/** A show-space rotation as a three.js matrix: `Q R Q` with `Q` the `z` flip. */
export function showRotationToThree(rotation: Mat3): Matrix4 {
  const [r0, r1, r2] = rotation;
  // Q R Q negates the entries where exactly one of row and column is `z`.
  return new Matrix4().set(
    r0[0], r0[1], -r0[2], 0,
    r1[0], r1[1], -r1[2], 0,
    -r2[0], -r2[1], r2[2], 0,
    0, 0, 0, 1,
  );
}

/** GDTF space to three.js: a quarter turn about X, so GDTF's Z is three's Y. */
export const GDTF_TO_THREE = new Matrix4().makeRotationX(-Math.PI / 2);

/** A glTF model into GDTF space: its Y is GDTF's Z. */
export const GLTF_TO_GDTF = new Matrix4().makeRotationX(Math.PI / 2);

/** Show axes back to GDTF's: swap the last two. */
function gdtf(vector: V3): [number, number, number] {
  return [vector.x, vector.z, vector.y];
}

/**
 * A node's transform relative to its parent, in GDTF space — the file's own
 * matrix. The domain carries the axes in show space as columns: GDTF's X
 * column is show's X axis converted, GDTF's Y column is show's **Z** axis
 * converted and GDTF's Z column show's **Y** (`P R P` undone).
 */
export function gdtfMatrix(node: DeviceNode): Matrix4 {
  const x = gdtf(node.xAxis);
  const y = gdtf(node.zAxis);
  const z = gdtf(node.yAxis);
  const t = gdtf(node.position);
  return new Matrix4().set(
    x[0], y[0], z[0], t[0],
    x[1], y[1], z[1], t[1],
    x[2], y[2], z[2], t[2],
    0, 0, 0, 1,
  );
}

/** A model's box in GDTF axes: length (X), width (Y), height (Z). */
export function gdtfSize(node: DeviceNode): [number, number, number] {
  return gdtf(node.size);
}
