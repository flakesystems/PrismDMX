/**
 * One picture of the rig, as data: S30's exit criteria asserted on what was
 * drawn rather than on what was meant.
 *
 * - *Every patched fixture appears with correct position and orientation*:
 *   each body is drawn where its position projects to, and a tipped fixture's
 *   beam lands where its rotation sends it.
 * - *Beams reflect live output*: the recorded frame from a real daemon lights
 *   exactly the head the programmer opened, and a level written into it moves
 *   a beam or puts it out.
 */

import { describe, expect, it } from "vitest";

import { TELEMETRY_HEADER_BYTES, TelemetryFrameView } from "../telemetry/frame";
import { RecordingView } from "../testing/recording-view";
import { finalShow, litFrame, litPayload } from "../testing/viewer-recording";
import type { Camera } from "./camera";
import { lensOf, look, newCamera, project } from "./camera";
import type { RigFixture } from "./rig";
import { rigOf } from "./rig";
import { INK, LABEL_LIMIT, Scene, convexHull } from "./scene";
import type { V3 } from "./space";
import { v3 } from "./space";

const rig = rigOf(finalShow());

function frontCamera(): Camera {
  const camera = newCamera();
  camera.target = v3(0, 3, 1);
  camera.distance = 20;
  look(camera, "front");
  return camera;
}

/** The recorded frame with `levels` written into its first universe. */
function frameWith(levels: Readonly<Record<number, number>>): TelemetryFrameView {
  const bytes = litPayload().slice();
  for (const [address, level] of Object.entries(levels)) {
    bytes[TELEMETRY_HEADER_BYTES + 2 + Number(address) - 1] = level;
  }
  const view = new TelemetryFrameView();
  expect(view.read(bytes)).toBeNull();
  return view;
}

function projected(camera: Camera, surface: RecordingView, point: V3): [number, number] {
  const out = [0, 0];
  expect(project(lensOf(camera, surface.width, surface.height), point, out)).toBe(true);
  return [out[0] ?? 0, out[1] ?? 0];
}

function centroid(points: readonly number[]): [number, number] {
  let x = 0;
  let y = 0;
  const count = points.length / 2;
  for (let index = 0; index < count; index += 1) {
    x += points[index * 2] ?? 0;
    y += points[index * 2 + 1] ?? 0;
  }
  return [x / count, y / count];
}

describe("the recorded rig, lit by the recorded frame", () => {
  it("draws every fixture, counts the one the programmer opened, and gives it one beam", () => {
    const surface = new RecordingView();
    const stats = new Scene().draw(surface, rig, litFrame(), frontCamera(), new Set());
    expect(stats).toEqual({ fixtures: 3, lit: 1, unplaced: 0, absent: 0, beams: 1 });
    expect(surface.beams).toHaveLength(1);
    expect(surface.beams[0]?.rgb).toBe("255,255,255");
    // Every fixture is numbered, which is how an operator reads a rig.
    expect(surface.labels.map((label) => label.text).sort()).toEqual(["1", "10", "2"]);
    expect(surface.lines).toBeGreaterThan(10);
  });

  it("draws each body where its position projects to", () => {
    const camera = frontCamera();
    const surface = new RecordingView();
    new Scene().draw(surface, rig, litFrame(), camera, new Set());
    const bodies = surface.polygons.filter((polygon) => polygon.blend === "over" && polygon.points.length === 8);
    for (const fixture of rig) {
      const [x, y] = projected(camera, surface, fixture.position);
      const near = bodies.some((face) => {
        const [cx, cy] = centroid(face.points);
        return Math.hypot(cx - x, cy - y) < 40;
      });
      expect(near, `fixture ${String(fixture.id)}`).toBe(true);
    }
  });

  it("drops the open head's beam straight down to a pool on the floor under it", () => {
    const camera = frontCamera();
    const surface = new RecordingView();
    new Scene().draw(surface, rig, litFrame(), camera, new Set());
    const beam = surface.beams[0];
    // The beam leaves 0.2 m under the head and lands under it.
    const [lensX, lensY] = projected(camera, surface, v3(-2, 5.8, 1));
    expect(beam?.from[0]).toBeCloseTo(lensX, 2);
    expect(beam?.from[1]).toBeCloseTo(lensY, 2);
    const [floorX, floorY] = projected(camera, surface, v3(-2, 0, 1));
    expect(Math.hypot((beam?.to[0] ?? 0) - floorX, (beam?.to[1] ?? 0) - floorY)).toBeLessThan(6);
    const pools = surface.polygons.filter((polygon) => polygon.blend === "add" && polygon.points.length === 32);
    expect(pools).toHaveLength(1);
  });

  it("lands a tipped fixture's beam downstage of it — the rotation, drawn", () => {
    const camera = frontCamera();
    const surface = new RecordingView();
    // Fixture 2 is hung tipped 20° towards the audience; open it.
    const stats = new Scene().draw(surface, rig, frameWith({ 15: 255 }), camera, new Set());
    expect(stats.lit).toBe(2);
    const second = surface.beams.find((beam) => beam.from[0] > surface.width / 2);
    // Its lens is 0.2 m below it along its own tipped axis, and its beam
    // leaves along that axis until it meets the floor.
    const tip = (20 * Math.PI) / 180;
    const height = 6 - 0.2 * Math.cos(tip);
    const reach = height / Math.cos(tip);
    const landing = v3(2, 0, 1 - 0.2 * Math.sin(tip) - reach * Math.sin(tip));
    const [x, y] = projected(camera, surface, landing);
    expect(Math.hypot((second?.to[0] ?? 0) - x, (second?.to[1] ?? 0) - y)).toBeLessThan(12);
  });

  it("turns a beam with pan and tilt, and puts it out with the dimmer", () => {
    const camera = frontCamera();
    const straight = new RecordingView();
    new Scene().draw(straight, rig, litFrame(), camera, new Set());
    const tilted = new RecordingView();
    // Tilt to full: +135°, far past horizontal, so the beam no longer lands.
    new Scene().draw(tilted, rig, frameWith({ 3: 255, 4: 255 }), camera, new Set());
    expect(tilted.beams[0]?.to).not.toEqual(straight.beams[0]?.to);
    expect(tilted.beams[0]?.alphaTo).toBeCloseTo(0.02, 6);

    const dark = new RecordingView();
    const stats = new Scene().draw(dark, rig, frameWith({ 5: 0 }), camera, new Set());
    expect(stats.lit).toBe(0);
    expect(dark.beams).toHaveLength(0);
  });

  it("points the floor PAR up and upstage, and it lands nowhere", () => {
    const surface = new RecordingView();
    new Scene().draw(surface, rig, frameWith({ 101: 255 }), frontCamera(), new Set());
    const up = surface.beams.find((beam) => beam.rgb === "255,0,0");
    expect(up).toBeDefined();
    // Above its lens on the screen: it points up.
    expect(up?.to[1] ?? 0).toBeLessThan(up?.from[1] ?? 0);
    expect(up?.alphaTo).toBeCloseTo(0.02, 6);
  });
});

describe("what is not lit, not placed or not on the cable", () => {
  it("is dark before any frame, and every fixture is still drawn", () => {
    const surface = new RecordingView();
    const stats = new Scene().draw(surface, rig, null, frontCamera(), new Set());
    expect(stats).toEqual({ fixtures: 3, lit: 0, unplaced: 0, absent: 0, beams: 0 });
    expect(surface.polygons.some((polygon) => polygon.fill === INK.lens)).toBe(true);
  });

  it("counts fixtures still at the origin, and fixtures on a universe the frame has not got", () => {
    const moved: RigFixture[] = rig.map((fixture) =>
      fixture.id === 10 ? { ...fixture, universe: 7 } : { ...fixture, unplaced: fixture.id === 2 },
    );
    const stats = new Scene().draw(new RecordingView(), moved, litFrame(), frontCamera(), new Set());
    expect(stats.unplaced).toBe(1);
    expect(stats.absent).toBe(1);
  });
});

describe("selection and picking", () => {
  it("outlines a selected fixture and numbers it in its own colour", () => {
    const surface = new RecordingView();
    new Scene().draw(surface, rig, litFrame(), frontCamera(), new Set([2]));
    expect(surface.polygons.some((polygon) => polygon.outline === INK.selected)).toBe(true);
    expect(surface.labels.find((label) => label.text === "2")?.colour).toBe(INK.selected);
  });

  it("picks the fixture drawn nearest a point, and nothing far from every fixture", () => {
    const camera = frontCamera();
    const surface = new RecordingView();
    const scene = new Scene();
    new Scene().pick(0, 0, 10);
    scene.draw(surface, rig, litFrame(), camera, new Set());
    const [x, y] = projected(camera, surface, rig[1]?.position ?? v3(0, 0, 0));
    expect(scene.pick(x + 3, y - 2, 18)).toBe(2);
    expect(scene.pick(1, 1, 18)).toBeNull();
  });

  it("numbers only the selection on a rig too big to read numbers off", () => {
    const big: RigFixture[] = [];
    const first = rig[0];
    if (first === undefined) {
      throw new Error("the recording has a head");
    }
    for (let id = 1; id <= LABEL_LIMIT + 4; id += 1) {
      big.push({ ...first, id, position: v3((id % 20) - 10, 6, Math.floor(id / 20)) });
    }
    const surface = new RecordingView();
    new Scene().draw(surface, big, null, frontCamera(), new Set([7]));
    expect(surface.labels.map((label) => label.text)).toEqual(["7"]);
  });
});

describe("a camera in the wrong place", () => {
  it("draws nothing it is inside or behind, and does not throw", () => {
    const camera = newCamera();
    camera.target = v3(-2, 6, 1);
    camera.distance = 0.05;
    look(camera, "front");
    const surface = new RecordingView();
    const stats = new Scene().draw(surface, rig, litFrame(), camera, new Set());
    expect(stats.fixtures).toBe(3);
    expect(surface.clears).toBe(1);
  });
});

describe("the hull a beam is drawn as", () => {
  it("is the outside of the points, in order, without the one inside", () => {
    const points = new Float64Array([0, 0, 10, 0, 10, 10, 0, 10, 5, 5]);
    const out = new Float64Array(12);
    const count = convexHull(points, 5, out, new Int32Array(6));
    expect(count).toBe(4);
    const corners = new Set<string>();
    for (let index = 0; index < count; index += 1) {
      corners.add(`${String(out[index * 2])},${String(out[index * 2 + 1])}`);
    }
    expect(corners).toEqual(new Set(["0,0", "10,0", "10,10", "0,10"]));
  });
});
