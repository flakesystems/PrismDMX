/**
 * The small parts of the 3D stage that need no WebGL to be checked — **S30b**:
 * the primitives a device without models is drawn with, the stand-ins for a
 * profile with no device, the beam's cone and the uniforms it is shaped by,
 * the mask's key, and the floor's book of projectors.
 *
 * What the shaders then *draw* is the end-to-end suite's, in a real browser.
 */

import { Box3, Mesh, PerspectiveCamera, Plane, Texture, Vector3, Vector4 } from "three";
import { describe, expect, it } from "vitest";

import { darkBeam } from "../state";
import { BeamVolume, beamAnchor, lensDisc, unitCone } from "./beam3d";
import { Floor } from "./floor";
import type { Projector } from "./floor";
import { maskKey } from "./mask";
import { primitive, standIn } from "./primitives";

function sizeOf(object: ReturnType<typeof primitive>): Vector3 {
  object.updateMatrixWorld(true);
  return new Box3().setFromObject(object).getSize(new Vector3());
}

describe("primitives", () => {
  it("draws every kind GDTF names, fitted to its box", () => {
    for (const kind of ["Cube", "Cylinder", "Conventional1_1", "Sphere", "Base", "Yoke", "Head", "Scanner", "Pigtail", "Beam", "Other"]) {
      const made = primitive(kind, [0.3, 0.2, 0.5], 16);
      expect(made.name).toBe(`primitive:${kind}`);
      let meshes = 0;
      made.traverse((object) => {
        if (object instanceof Mesh) {
          meshes += 1;
        }
      });
      expect(meshes, kind).toBeGreaterThan(0);
      const size = sizeOf(made);
      // Nothing it draws is bigger than the box it was given, by more than a
      // rounding of its curves.
      expect(size.x, kind).toBeLessThanOrEqual(0.31);
      expect(size.y, kind).toBeLessThanOrEqual(0.31);
      expect(size.z, kind).toBeLessThanOrEqual(0.51);
    }
  });

  it("draws a box it was given no size for as a small one rather than nothing", () => {
    expect(sizeOf(primitive("Cube", [0, 0, 0])).x).toBeGreaterThan(0);
  });

  it("stands in a moving head with a yoke and a head, and a PAR with neither", () => {
    const head = standIn(true, 16);
    expect(head.yoke).not.toBeNull();
    expect(head.head).not.toBeNull();
    expect(head.radius).toBeGreaterThan(0);
    const par = standIn(false, 16);
    expect(par.yoke).toBeNull();
    expect(par.head).toBeNull();
    expect(par.lens.parent).not.toBeNull();
  });
});

describe("the beam", () => {
  it("is a cone of the segments asked for, with both caps", () => {
    const cone = unitCone(12);
    // Two triangles of side and one of each cap per segment, three points each.
    expect(cone.getAttribute("position").count).toBe(12 * 4 * 3);
  });

  it("shapes its cone by the angle, the length and where the camera is", () => {
    const volume = new BeamVolume(unitCone(8), 12, new Texture(), 0.05);
    const camera = new PerspectiveCamera();
    camera.position.set(0, 0, 5);
    camera.updateMatrixWorld(true);
    volume.mesh.updateMatrixWorld(true);
    volume.set({ light: 0.8, color: [1, 0.5, 0.25], angle: 90, length: 7, rotation: 0.3 }, camera, new Plane(new Vector3(0, 1, 0), 2));
    const uniforms = volume.material.uniforms;
    expect(uniforms.uK?.value).toBeCloseTo(1, 9);
    expect(uniforms.uL?.value).toBe(7);
    expect(uniforms.uA?.value).toBeCloseTo(0.05, 9);
    expect(uniforms.uLight?.value).toBe(0.8);
    expect(uniforms.uRot?.value).toBe(0.3);
    expect((uniforms.uCam?.value as Vector3 | undefined)?.z).toBeCloseTo(5, 9);
    expect((uniforms.uFloor?.value as Vector4 | undefined)?.w).toBeCloseTo(2, 9);
    expect(volume.material.defines).toEqual({ STEPS: 12 });
    expect(volume.mesh.visible).toBe(true);

    // Dark is not drawn at all.
    volume.set({ light: 0, color: [1, 1, 1], angle: 20, length: 7, rotation: 0 }, camera, new Plane());
    expect(volume.mesh.visible).toBe(false);
    volume.dispose();
  });

  it("is a flat shell at the lowest detail", () => {
    const shell = new BeamVolume(unitCone(8), 0, new Texture(), 0.05);
    expect(shell.material.defines).toEqual({});
    expect(shell.material.fragmentShader).not.toContain("STEPS");
  });

  it("has a lens facing down it and an anchor to hang it from", () => {
    const lens = lensDisc(0.06);
    expect(lens.rotation.x).toBeCloseTo(Math.PI, 9);
    expect(beamAnchor().name).toBe("beam");
  });
});

describe("maskKey", () => {
  const none = () => null;

  it("is the same for the same beam, and moves with anything drawn into the mask", () => {
    const open = darkBeam();
    expect(maskKey(open, none)).toBe(maskKey(darkBeam(), none));
    const gobo = { ...darkBeam(), gobos: [{ wheel: "Gobo1", media: "g", rotation: 0, spin: 0 }] };
    expect(maskKey(gobo, none)).not.toBe(maskKey(open, none));
    expect(maskKey({ ...darkBeam(), iris: 0.4 }, none)).not.toBe(maskKey(open, none));
    expect(maskKey({ ...darkBeam(), blades: [{ a: 0.2, b: 0.2, rotation: 0 }] }, none)).not.toBe(maskKey(open, none));
    expect(maskKey({ ...darkBeam(), frost: 0.5 }, none)).not.toBe(maskKey(open, none));
  });

  it("moves when a gobo's picture arrives, so the mask is drawn again with it", () => {
    const gobo = { ...darkBeam(), gobos: [{ wheel: "Gobo1", media: "g", rotation: 0, spin: 0 }] };
    const arrived = () => ({ width: 1, height: 1 }) as unknown as HTMLCanvasElement;
    expect(maskKey(gobo, arrived)).not.toBe(maskKey(gobo, none));
  });

  it("does not move with a gobo's spin, which the beam turns itself", () => {
    const still = { ...darkBeam(), gobos: [{ wheel: "Gobo1", media: "g", rotation: 0, spin: 0 }] };
    const spinning = { ...darkBeam(), gobos: [{ wheel: "Gobo1", media: "g", rotation: 0, spin: 90 }] };
    expect(maskKey(spinning, none)).toBe(maskKey(still, none));
  });
});

describe("the floor", () => {
  function projector(brightness: number): Projector {
    return {
      position: new Vector3(0, 6, 0),
      direction: new Vector3(0, -1, 0),
      up: new Vector3(0, 0, -1),
      spread: 0.2,
      color: [brightness, brightness, brightness],
      mask: document.createElement("canvas"),
      version: 1,
    };
  }

  it("takes as many projectors as it has room for, and drops the rest", () => {
    const floor = new Floor(4, 32);
    floor.set([projector(1), projector(0.8), projector(0.6), projector(0.4), projector(0.2), projector(0.1)]);
    const uniforms = floor.mesh.material.uniforms;
    expect(uniforms.uUsed?.value).toBe(4);
    expect((uniforms.uPos?.value as Vector3[] | undefined)?.[0]?.y).toBe(6);
    expect((uniforms.uColor?.value as Vector3[] | undefined)?.[3]?.x).toBeCloseTo(0.4, 9);
    expect((uniforms.uK?.value as number[] | undefined)?.[2]).toBe(0.2);
    expect(floor.mesh.material.fragmentShader).toContain("#define COUNT 4");

    floor.set([]);
    expect(uniforms.uUsed?.value).toBe(0);
    floor.dispose();
  });
});
