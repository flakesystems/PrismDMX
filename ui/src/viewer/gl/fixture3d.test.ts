/**
 * A fixture built as a three.js tree — **S30b** — out of the recorded show,
 * and moved by the recorded frame. No WebGL is needed for any of it: the tree,
 * the materials and the matrices are plain objects, and what is asserted is
 * where the light leaves and which way it goes, in the world, which is what
 * the volume in the haze and the pool on the floor are both drawn from.
 *
 * The recorded head hangs at (−2, 6, 1) in show space with no rotation, its
 * beam 0.2 m below its origin; the frame has it open and its yoke at +135°.
 */

import { BoxGeometry, LineBasicMaterial, Mesh, Object3D, PerspectiveCamera, Plane, Vector3 } from "three";
import { describe, expect, it } from "vitest";

import { finalShow, litFrame } from "../../testing/viewer-recording";
import { rigOf } from "../rig";
import type { RigFixture } from "../rig";
import type { FixtureState } from "../state";
import { emptyState, readState } from "../state";
import { unitCone } from "./beam3d";
import { DETAIL } from "./detail";
import type { ModelSource } from "./fixture3d";
import { Fixture3D, fitTo } from "./fixture3d";
import type { Projector } from "./floor";

function fixture(id: number): RigFixture {
  const found = rigOf(finalShow()).find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error(`fixture ${String(id)} is not in the rig`);
  }
  return found;
}

const camera = new PerspectiveCamera();
camera.position.set(0, 3, 12);
camera.updateMatrixWorld(true);
const floor = new Plane(new Vector3(0, 1, 0), 0);
const noPictures = () => () => null;

function built(id: number, models: ModelSource | null = null): Fixture3D {
  return new Fixture3D(fixture(id), DETAIL.medium, unitCone(24), models, new LineBasicMaterial());
}

function lit(id: number): FixtureState {
  return readState(fixture(id), litFrame(), emptyState());
}

function projectorsOf(built3d: Fixture3D): Projector[] {
  const into: Projector[] = [];
  for (const beam of built3d.beams.values()) {
    beam.projectors(into);
  }
  return into;
}

describe("the recorded head", () => {
  it("hangs where the show hangs it, its light leaving 0.2 m below and pointing down", () => {
    const head = built(1);
    head.update(lit(1), 0, 0.5, noPictures, camera, floor);
    const projectors = projectorsOf(head);
    expect(projectors).toHaveLength(1);
    const [light] = projectors;
    expect(light?.position.x).toBeCloseTo(-2, 4);
    expect(light?.position.y).toBeCloseTo(5.8, 4);
    // Show's z is three's −z.
    expect(light?.position.z).toBeCloseTo(-1, 4);
    expect(light?.direction.y).toBeCloseTo(-1, 6);
    // Its colour is the lamp's 6500 K at full: open, and lit.
    expect(Math.max(...(light?.color ?? [0]))).toBeCloseTo(1, 6);
    // The beam is drawn from the lens to a little past the floor.
    expect(head.beams.get(3)?.length).toBeCloseTo(5.8 * 1.05 + 0.3, 6);
    expect(head.beams.get(3)?.spread).toBeCloseTo(8, 6);
  });

  it("swings its beam with its head's tilt, carried round by its yoke's pan", () => {
    const head = built(1);
    const state = lit(1);
    state.axes.set(2, { pan: 0, tilt: 90 });
    head.update(state, 0, 0.5, noPictures, camera, floor);
    const [light] = projectorsOf(head);
    // Tilted a quarter: level, and the yoke's +135° turns it round the vertical.
    expect(light?.direction.y).toBeCloseTo(0, 6);
    expect(Math.hypot(light?.direction.x ?? 0, light?.direction.z ?? 0)).toBeCloseTo(1, 6);
    const untilted = built(1);
    const turned = lit(1);
    turned.axes.set(1, { pan: 0, tilt: 0 });
    turned.axes.set(2, { pan: 0, tilt: 90 });
    untilted.update(turned, 0, 0.5, noPictures, camera, floor);
    const [straight] = projectorsOf(untilted);
    // The two level beams differ by the pan: 135° apart about the vertical —
    // to the sixteen bits the frame carries it in.
    const a = new Vector3(light?.direction.x, 0, light?.direction.z);
    const b = new Vector3(straight?.direction.x, 0, straight?.direction.z);
    expect((a.angleTo(b) * 180) / Math.PI).toBeCloseTo(135, 2);
  });

  it("throws nothing when it is dark, and one projector per facet through a prism", () => {
    const dark = built(2);
    dark.update(lit(2), 0, 0.5, noPictures, camera, floor);
    expect(projectorsOf(dark)).toEqual([]);

    const head = built(1);
    const state = lit(1);
    const beam = state.beams.get(3);
    if (beam === undefined) {
      throw new Error("no beam");
    }
    beam.prism = {
      facets: [
        { x: 0.3, y: 0 },
        { x: -0.15, y: 0.26 },
        { x: -0.15, y: -0.26 },
      ],
      rotation: 0,
      spin: 0,
    };
    head.update(state, 0, 0.5, noPictures, camera, floor);
    const projectors = projectorsOf(head);
    expect(projectors).toHaveLength(3);
    // Each facet takes its share of the light, and is pushed off the axis.
    expect(Math.max(...(projectors[0]?.color ?? [0]))).toBeCloseTo(1 / Math.sqrt(3), 6);
    for (const projector of projectors) {
      expect(projector.direction.y).toBeLessThan(-0.9);
      expect(projector.direction.y).toBeGreaterThan(-0.9999);
    }
  });

  it("can be taken out of the haze for a frame and still light the floor", () => {
    const head = built(1);
    head.update(lit(1), 0, 0.5, noPictures, camera, floor);
    const beam = head.beams.get(3);
    expect(beam?.volumes[0]?.mesh.visible).toBe(true);
    beam?.hideVolumes();
    expect(beam?.volumes[0]?.mesh.visible).toBe(false);
    expect(projectorsOf(head)).toHaveLength(1);
    // The next frame draws it again.
    head.update(lit(1), 0, 0.5, noPictures, camera, floor);
    expect(beam?.volumes[0]?.mesh.visible).toBe(true);
  });

  it("is outlined while selected", () => {
    const head = built(1);
    expect(head.selected).toBe(false);
    head.selected = true;
    expect(head.selected).toBe(true);
    const outline = head.place.children.find((child) => child.type === "LineSegments");
    expect(outline?.visible).toBe(true);
    head.dispose();
  });

  it("wears its own model once it arrives, in place of the primitive", async () => {
    const asked: string[] = [];
    const model = new Object3D();
    const models: ModelSource = (device, name) => {
      asked.push(`${device.guid}/${name}`);
      return Promise.resolve(model);
    };
    const head = built(1, models);
    expect(asked).toEqual(["5A1E0000-0000-4000-8000-0000000030AA/viewer-head"]);
    await Promise.resolve();
    await Promise.resolve();
    expect(model.parent).not.toBeNull();
    expect(model.userData.fixture).toBe(1);
    head.dispose();
  });
});

describe("a fixture with no device", () => {
  it("stands in as a PAR, its one beam lit by its attribute list", () => {
    const par = built(10);
    expect(par.beams.size).toBe(1);
    expect(par.bodies.length).toBeGreaterThan(0);
    par.update(lit(10), 0, 0.5, noPictures, camera, floor);
    par.setPlace(fixture(10));
    expect(par.place.position.z).toBeCloseTo(-4, 6);
  });
});

describe("fitTo", () => {
  it("scales a model a thousand times its geometry down to metres, and leaves one in metres alone", () => {
    const inMillimetres = new Mesh(new BoxGeometry(300, 300, 500));
    fitTo(inMillimetres, [0.3, 0.3, 0.5]);
    expect(inMillimetres.scale.x).toBeCloseTo(0.001, 9);

    const inMetres = new Mesh(new BoxGeometry(0.3, 0.3, 0.5));
    fitTo(inMetres, [0.3, 0.3, 0.5]);
    expect(inMetres.scale.x).toBe(1);
  });
});
