/**
 * Bodies for everything that has no model file — **S30b**.
 *
 * GDTF names a primitive for a geometry whose `Model` has no file: `Cube`,
 * `Cylinder`, `Sphere`, and the three a moving head is made of — `Base`,
 * `Yoke`, `Head` — plus `Scanner`, `Conventional` and `Pigtail`. Each is drawn
 * here as a shape of that kind, fitted to the model's box, **in GDTF space**
 * (Z up) like the file's own models.
 *
 * And a profile with no geometry at all — an Open Fixture Library profile, a
 * generic — is drawn as the device it is from its channels: a moving head
 * (base, yoke, head) when it has pan or tilt, a PAR can when it has not.
 * Never a box: a box is what S30's first viewer drew, and the owner was right
 * that it said nothing.
 *
 * # One mesh a part, built once
 *
 * A PAR can is a bracket, a can and a rim, and drawn as three meshes a rig of
 * five hundred is fifteen hundred draw calls before a beam is lit — most of a
 * frame on a machine that draws WebGL in software. So every part is **merged**
 * into one geometry with its colours in its vertices, drawn with one shared
 * material, and **built once** per shape and size: every PAR of a rig draws
 * the same geometry from its own place.
 */

import {
  BoxGeometry,
  BufferGeometry,
  Color,
  CylinderGeometry,
  Float32BufferAttribute,
  Group,
  Mesh,
  MeshStandardMaterial,
  Object3D,
  SphereGeometry,
  TorusGeometry,
} from "three";
import type { Material } from "three";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";

/** The housing: dark, slightly metallic, like every stage fixture. */
export const HOUSING = new MeshStandardMaterial({ color: 0x33373f, metalness: 0.3, roughness: 0.5 });

/** A lighter trim, for a yoke's arms and a can's rim. */
export const TRIM = new MeshStandardMaterial({ color: 0x454a54, metalness: 0.45, roughness: 0.4 });

/** A cable's black. */
const CABLE = new MeshStandardMaterial({ color: 0x0b0c0e, roughness: 0.9 });

/** What every merged body is drawn with: its colours are in its vertices. */
const BODY = new MeshStandardMaterial({ vertexColors: true, metalness: 0.35, roughness: 0.48 });

/** Every merged body built so far, by shape and size. */
const BUILT = new Map<string, BufferGeometry>();

/**
 * One geometry out of every mesh under `root`, in `root`'s frame, each mesh's
 * material colour written into its vertices.
 */
function merged(root: Object3D): BufferGeometry {
  root.updateMatrixWorld(true);
  const parts: BufferGeometry[] = [];
  const colour = new Color();
  root.traverse((object) => {
    if (!(object instanceof Mesh)) {
      return;
    }
    const source = object.geometry as BufferGeometry;
    const flat = (source.index === null ? source.clone() : source.toNonIndexed()).applyMatrix4(object.matrixWorld);
    const part = new BufferGeometry();
    const position = flat.getAttribute("position");
    const normal = flat.getAttribute("normal");
    if (position === undefined || normal === undefined) {
      return;
    }
    part.setAttribute("position", position);
    part.setAttribute("normal", normal);
    const material = object.material as Material;
    colour.copy(material instanceof MeshStandardMaterial ? material.color : new Color(0x33373f));
    const colours = new Float32Array(position.count * 3);
    for (let index = 0; index < position.count; index += 1) {
      colours[index * 3] = colour.r;
      colours[index * 3 + 1] = colour.g;
      colours[index * 3 + 2] = colour.b;
    }
    part.setAttribute("color", new Float32BufferAttribute(colours, 3));
    parts.push(part);
  });
  return (parts.length === 0 ? null : mergeGeometries(parts, false)) ?? new BufferGeometry();
}

/** A mesh of the merged body `key` names, built by `build` the first time. */
function solid(key: string, name: string, build: () => Object3D): Mesh {
  let geometry = BUILT.get(key);
  if (geometry === undefined) {
    geometry = merged(build());
    BUILT.set(key, geometry);
  }
  const made = new Mesh(geometry, BODY);
  made.name = name;
  return made;
}

/** A mesh that casts and takes shadows. */
function mesh(geometry: BufferGeometry, material: Material): Mesh {
  const made = new Mesh(geometry, material);
  made.castShadow = true;
  made.receiveShadow = true;
  return made;
}

/** A cylinder along GDTF's Z (three's cylinders run along their own Y). */
function zCylinder(radiusTop: number, radiusBottom: number, height: number, segments: number, material: Material): Mesh {
  const made = mesh(new CylinderGeometry(radiusTop, radiusBottom, height, segments, 1), material);
  made.rotation.x = Math.PI / 2;
  return made;
}

/**
 * A primitive of GDTF's kind fitted to `size` (length X, width Y, height Z),
 * centred — one mesh, its geometry shared with every other of the same kind
 * and size.
 */
export function primitive(kind: string, size: readonly [number, number, number], segments = 32): Object3D {
  const key = `primitive:${kind}:${size.map((edge) => edge.toFixed(4)).join("x")}:${String(segments)}`;
  return solid(key, `primitive:${kind}`, () => buildPrimitive(kind, size, segments));
}

/** The primitive as the meshes it is made of, before they are merged. */
function buildPrimitive(kind: string, size: readonly [number, number, number], segments: number): Object3D {
  const [length, width, height] = size.map((edge) => Math.max(edge, 0.01)) as [number, number, number];
  const group = new Group();
  switch (kind.replace(/\d.*$/, "")) {
    case "Cylinder":
    case "Conventional":
    case "Beam": {
      const radius = Math.min(length, width) / 2;
      group.add(zCylinder(radius, radius, height, segments, HOUSING));
      break;
    }
    case "Sphere": {
      const sphere = mesh(new SphereGeometry(0.5, segments, Math.max(8, segments / 2)), HOUSING);
      sphere.scale.set(length, width, height);
      group.add(sphere);
      break;
    }
    case "Base": {
      // A plinth with a turntable on it.
      const plinth = mesh(new BoxGeometry(length, width, height * 0.8), HOUSING);
      plinth.position.z = -height * 0.1;
      const table = zCylinder(Math.min(length, width) * 0.32, Math.min(length, width) * 0.36, height * 0.2, segments, TRIM);
      table.position.z = height * 0.4;
      group.add(plinth, table);
      break;
    }
    case "Yoke": {
      // A U: a bridge on top and two arms down.
      const arm = Math.max(length * 0.12, 0.02);
      const bridge = mesh(new BoxGeometry(length, width, arm), TRIM);
      bridge.position.z = height / 2 - arm / 2;
      const left = mesh(new BoxGeometry(arm, width, height), TRIM);
      left.position.x = -length / 2 + arm / 2;
      const right = left.clone();
      right.position.x = length / 2 - arm / 2;
      group.add(bridge, left, right);
      break;
    }
    case "Head":
    case "Scanner": {
      // A barrel with a bezel at its front — the front is GDTF's −Z, where
      // the beam leaves.
      const radius = Math.min(length, width) / 2;
      const barrel = zCylinder(radius * 0.92, radius, height, segments, HOUSING);
      const bezel = mesh(new TorusGeometry(radius * 0.9, radius * 0.08, 8, segments), TRIM);
      bezel.position.z = -height / 2;
      group.add(barrel, bezel);
      break;
    }
    case "Pigtail": {
      group.add(mesh(new BoxGeometry(length, width, height), CABLE));
      break;
    }
    default:
      group.add(mesh(new BoxGeometry(length, width, height), HOUSING));
  }
  return group;
}

/** The parts of a device drawn from its channels rather than its geometry. */
export interface StandIn {
  /** The whole thing, in GDTF space, origin at its hanging point. */
  readonly root: Object3D;
  /** What pan turns, about its Z. `null` for a fixture that does not move. */
  readonly yoke: Object3D | null;
  /** What tilt turns, about its X. */
  readonly head: Object3D | null;
  /** Where the light leaves, beam along its −Z. */
  readonly lens: Object3D;
  /** The lens's radius, metres. */
  readonly radius: number;
}

/**
 * A device for a profile with no geometry tree: a moving head when it moves,
 * a PAR can when it does not. Sized like the small fixtures of a small venue.
 */
export function standIn(moves: boolean, segments = 32): StandIn {
  const root = new Group();
  root.name = moves ? "stand-in:moving-head" : "stand-in:par";
  if (!moves) {
    // A PAR can hung from a bracket: the can points down, the lens at its foot.
    const body = solid(`par:${String(segments)}`, "stand-in:par-body", () => {
      const parts = new Group();
      const bracket = mesh(new TorusGeometry(0.13, 0.012, 6, segments, Math.PI), TRIM);
      bracket.rotation.x = Math.PI / 2;
      bracket.position.z = -0.05;
      const can = zCylinder(0.1, 0.11, 0.28, segments, HOUSING);
      can.position.z = -0.19;
      const rim = mesh(new TorusGeometry(0.108, 0.01, 6, segments), TRIM);
      rim.position.z = -0.33;
      parts.add(bracket, can, rim);
      return parts;
    });
    const lens = new Group();
    lens.position.z = -0.335;
    root.add(body, lens);
    return { root, yoke: null, head: null, lens, radius: 0.095 };
  }
  // Base on top (it hangs), a yoke under it that pans, a head in the yoke
  // that tilts — the same tree a GDTF moving head has.
  const base = primitive("Base", [0.34, 0.24, 0.1], segments);
  root.add(base);
  const yoke = new Group();
  yoke.name = "stand-in:yoke";
  yoke.position.z = -0.06;
  const arms = primitive("Yoke", [0.34, 0.1, 0.3], segments);
  arms.rotation.x = Math.PI;
  arms.position.z = -0.15;
  yoke.add(arms);
  root.add(yoke);
  const head = new Group();
  head.name = "stand-in:head";
  head.position.z = -0.28;
  const barrel = primitive("Head", [0.24, 0.24, 0.34], segments);
  head.add(barrel);
  const lens = new Group();
  lens.position.z = -0.17;
  head.add(lens);
  yoke.add(head);
  return { root, yoke, head, lens, radius: 0.1 };
}
