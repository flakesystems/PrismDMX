/**
 * One fixture in the scene — **S30b**.
 *
 * ```text
 * place            where it hangs (show space, converted — ./coords.ts)
 *   device         a quarter turn: GDTF's Z-up into three's Y-up
 *     node         one per GDTF geometry, its matrix the file's own
 *       motion     for an Axis: the turn pan (about Z) or tilt (about X) adds
 *         model    the geometry's glTF or 3DS, or its primitive
 *         beam     for a Beam: the lens, the volume, the prism's copies
 *         …children
 * ```
 *
 * A profile with no geometry tree gets a stand-in (`./primitives.ts`) with
 * the same shape: a yoke that pans, a head that tilts, a lens.
 *
 * Models arrive asynchronously (`../resources.ts`) and replace the primitive
 * drawn in their place until then; a model the desk does not have is never
 * replaced, and the primitive is what the device looks like.
 */

import {
  Box3,
  BoxGeometry,
  EdgesGeometry,
  Group,
  LineSegments,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  Object3D,
  Vector3,
} from "three";
import type { BufferGeometry, Camera, Material, Plane, Texture } from "three";

import type { Device, DeviceNode } from "../device";
import type { RigFixture } from "../rig";
import { orientation } from "../space";
import type { BeamState, FixtureState } from "../state";
import { strobeLevel } from "../state";
import { BeamVolume, beamAnchor, lensDisc } from "./beam3d";
import { GDTF_TO_THREE, gdtfMatrix, gdtfSize, showRotationToThree, showToThree } from "./coords";
import type { Detail } from "./detail";
import type { Projector } from "./floor";
import type { Pictures } from "./mask";
import { BeamMask } from "./mask";
import { primitive, standIn } from "./primitives";

/**
 * How much a beam lights the haze at full, per unit of haze. Tuned by eye at
 * the default detail, where there is no glow to help: at 2.2 a Robin T1's
 * shafts at the default haze were barely there against the floor.
 */
const HAZE_GAIN = 3.5;

/** Loads a model for a device, or `null` when the desk has none. */
export type ModelSource = (device: { guid: string; typeId: string }, name: string) => Promise<Object3D | null>;

/** One beam of a fixture, and everything that draws it. */
export class FixtureBeam {
  readonly anchor: Group;
  readonly mask: BeamMask;
  readonly volumes: BeamVolume[] = [];
  readonly lens: Mesh<BufferGeometry, MeshBasicMaterial>;
  /** Degrees, the geometry's own angle before zoom. */
  readonly angle: number;
  /** Where the prism copies hang, one per facet. */
  readonly facets: Group[] = [];
  readonly radius: number;
  /** Set each frame for the light manager. */
  light = 0;
  color: [number, number, number] = [1, 1, 1];
  spread = 20;
  soft = 0;
  length = 10;
  /** Radians the mask is turned this frame — a spinning gobo. */
  #turn = 0;
  /** The facets' deflection this frame, for the floor. */
  #copies = 0;
  readonly #projectors: { position: Vector3; direction: Vector3; up: Vector3 }[] = [];
  #geometry: BufferGeometry;
  #steps: number;

  constructor(radius: number, angle: number, detail: Detail, geometry: BufferGeometry) {
    this.anchor = beamAnchor();
    this.radius = radius;
    this.angle = angle > 0.5 ? angle : 25;
    this.mask = new BeamMask(detail.mask);
    this.#geometry = geometry;
    this.#steps = detail.steps;
    const main = new BeamVolume(geometry, detail.steps, this.mask.texture, radius);
    this.volumes.push(main);
    this.anchor.add(main.mesh);
    this.lens = lensDisc(radius);
    this.anchor.add(this.lens);
  }

  /** The prism copies: one volume per facet, each tilted by its facet. */
  #facetsFor(count: number): void {
    while (this.facets.length < count) {
      const holder = new Group();
      const volume = new BeamVolume(this.#geometry, this.#steps, this.mask.texture, this.radius);
      holder.add(volume.mesh);
      this.volumes.push(volume);
      this.facets.push(holder);
      this.anchor.add(holder);
    }
    this.facets.forEach((holder, index) => {
      holder.visible = index < count;
    });
  }

  /** Draws this beam as `state` has it at `seconds`. */
  update(
    state: BeamState,
    seconds: number,
    seed: number,
    haze: number,
    pictures: Pictures,
    camera: Camera,
    floor: Plane,
  ): void {
    if (this.mask.update(state, pictures)) {
      // The open mask is shared; a beam with something in it has its own.
      for (const volume of this.volumes) {
        volume.setMask(this.mask.texture);
      }
    }
    const flash = strobeLevel(state, seconds, seed);
    const output = state.intensity * flash;
    const angle = Math.max(0.5, (state.angle ?? this.angle) + state.frost * 20);
    // Where it lands: along its axis to the floor, or a long way off.
    this.anchor.updateWorldMatrix(true, false);
    const origin = WORLD_ORIGIN.setFromMatrixPosition(this.anchor.matrixWorld);
    const axis = WORLD_AXIS.set(0, 0, -1).transformDirection(this.anchor.matrixWorld);
    const toFloor = axis.y < -0.02 && origin.y > 0 ? origin.y / -axis.y : Number.POSITIVE_INFINITY;
    const length = Math.min(60, toFloor * 1.05 + 0.3, Number.isFinite(toFloor) ? 60 : 25);
    const spin = state.gobos.reduce((turn, gobo) => turn + gobo.spin, 0);
    const rotation = ((spin * seconds) % 360) * (Math.PI / 180);

    const prism = state.prism;
    const copies = prism?.facets.length ?? 0;
    this.#facetsFor(copies);
    // A prism splits the light: the centre beam is gone and each facet takes
    // its share, the whole turned by the prism's rotation.
    const main = this.volumes[0];
    if (main !== undefined) {
      main.set(
        { light: copies > 0 ? 0 : output * haze * HAZE_GAIN, color: state.color, angle, length, rotation },
        camera,
        floor,
      );
    }
    if (prism !== null && copies > 0) {
      const turn = ((prism.rotation + prism.spin * seconds) * Math.PI) / 180;
      prism.facets.forEach((facet, index) => {
        const holder = this.facets[index];
        const volume = this.volumes[index + 1];
        if (holder === undefined || volume === undefined) {
          return;
        }
        const x = facet.x * Math.cos(turn) - facet.y * Math.sin(turn);
        const y = facet.x * Math.sin(turn) + facet.y * Math.cos(turn);
        // The facet pushes the beam to `(x, y)` a unit along it; capped at
        // a deflection a real prism gives.
        const deflection = Math.min(Math.atan(Math.hypot(x, y)) * 0.6, (22 * Math.PI) / 180);
        holder.rotation.set(0, 0, 0);
        holder.rotateOnAxis(TILT_AXIS.set(-y, x, 0).normalize(), deflection);
        volume.set(
          { light: (output * haze * HAZE_GAIN) / Math.sqrt(copies), color: state.color, angle, length, rotation },
          camera,
          floor,
        );
      });
    }

    const lens = this.lens.material;
    const glow = Math.min(1, output * 1.4);
    lens.color.setRGB(
      0.07 + state.color[0] * glow * 3,
      0.08 + state.color[1] * glow * 3,
      0.09 + state.color[2] * glow * 3,
    );

    this.light = output;
    this.#turn = rotation;
    this.#copies = copies;
    this.color = state.color;
    this.spread = angle;
    this.soft = Math.min(1, state.blur * 0.6 + state.frost);
    this.length = length;
  }

  /**
   * What this beam throws onto the floor: one projector, or one per prism
   * facet, each carrying its share of the light. The mask's "up" is the
   * beam's own Y turned by the gobo's spin, which is the turn the volume
   * applies (`uRot`) — so the shafts and the pool turn together.
   *
   * A projector throws from a point, and the volume leaves the **whole lens**
   * — at a distance `d` it is `radius + d · k` wide. So each projector stands
   * at the cone's apex, `radius / k` behind the lens, where a point source
   * lights exactly the circle the volume draws: the shaft ends flush with its
   * pool. From the lens itself the pool was a lens radius too small.
   */
  projectors(into: Projector[]): void {
    if (this.light <= 0.02) {
      return;
    }
    const sources: Object3D[] = this.#copies > 0 ? this.facets.slice(0, this.#copies) : [this.anchor];
    const share = this.#copies > 0 ? 1 / Math.sqrt(this.#copies) : 1;
    const spread = Math.tan((Math.min(170, Math.max(0.5, this.spread)) * Math.PI) / 360);
    const sin = Math.sin(this.#turn);
    const cos = Math.cos(this.#turn);
    sources.forEach((source, index) => {
      let held = this.#projectors[index];
      if (held === undefined) {
        held = { position: new Vector3(), direction: new Vector3(), up: new Vector3() };
        this.#projectors.push(held);
      }
      source.updateWorldMatrix(true, false);
      held.direction.set(0, 0, -1).transformDirection(source.matrixWorld);
      held.position
        .setFromMatrixPosition(source.matrixWorld)
        .addScaledVector(held.direction, -Math.max(0.005, this.radius) / spread);
      held.up.set(sin, cos, 0).transformDirection(source.matrixWorld);
      into.push({
        position: held.position,
        direction: held.direction,
        up: held.up,
        spread,
        color: [
          this.color[0] * this.light * share,
          this.color[1] * this.light * share,
          this.color[2] * this.light * share,
        ],
        mask: this.mask.canvas,
        version: this.mask.version,
      });
    });
  }

  /** Takes this beam's shafts out of the haze for this frame — see `Detail.volumes`. */
  hideVolumes(): void {
    for (const volume of this.volumes) {
      volume.mesh.visible = false;
    }
  }

  /** Where this beam's light leaves, and where it points, in the world. */
  worldPose(position: Vector3, target: Vector3): void {
    position.setFromMatrixPosition(this.anchor.matrixWorld);
    target.set(0, 0, -1).transformDirection(this.anchor.matrixWorld).multiplyScalar(4).add(position);
  }

  get texture(): Texture {
    return this.mask.texture;
  }

  dispose(): void {
    this.mask.dispose();
    for (const volume of this.volumes) {
      volume.dispose();
    }
    this.lens.geometry.dispose();
    this.lens.material.dispose();
  }
}

const WORLD_ORIGIN = new Vector3();
const WORLD_AXIS = new Vector3();
const TILT_AXIS = new Vector3();

/** One fixture's scene graph. */
export class Fixture3D {
  readonly id: number;
  readonly place = new Group();
  readonly beams = new Map<number, FixtureBeam>();
  /** Every mesh a click may land on, for picking. */
  readonly bodies: Object3D[] = [];
  readonly #motion = new Map<number, Group>();
  readonly #device: Device | null;
  #standIn: { yoke: Object3D | null; head: Object3D | null } | null = null;
  #selected = false;
  #outline: LineSegments | null = null;
  #disposed = false;

  constructor(
    fixture: RigFixture,
    detail: Detail,
    cone: BufferGeometry,
    models: ModelSource | null,
    outline: Material,
  ) {
    this.id = fixture.id;
    this.#device = fixture.device;
    this.place.name = `fixture:${String(fixture.id)}`;
    this.place.userData.fixture = fixture.id;
    const root = new Group();
    root.matrixAutoUpdate = false;
    root.matrix.copy(GDTF_TO_THREE);
    this.place.add(root);

    if (fixture.device !== null) {
      this.#build(fixture, fixture.device, root, detail, cone, models);
    } else {
      const moves = fixture.channels.Pan !== undefined || fixture.channels.Tilt !== undefined;
      const stand = standIn(moves, detail.segments);
      root.add(stand.root);
      this.#standIn = { yoke: stand.yoke, head: stand.head };
      // The body is what a click lands on and what the selection box goes
      // round — collected before the beam is hung in it, or the cone of
      // light, metres long, would be both.
      stand.root.traverse((object) => {
        if (object instanceof Mesh) {
          this.bodies.push(object);
        }
      });
      const angle = fixture.beams[0]?.angle ?? 25;
      const beam = new FixtureBeam(stand.radius, angle, detail, cone);
      stand.lens.add(beam.anchor);
      this.beams.set(0, beam);
    }
    this.place.traverse((object) => {
      object.userData.fixture = fixture.id;
    });
    this.#outline = new LineSegments(OUTLINE_BOX, outline);
    this.#outline.visible = false;
    this.place.add(this.#outline);
    this.setPlace(fixture);
  }

  /** Builds a GDTF device's tree under `root`. */
  #build(
    fixture: RigFixture,
    device: Device,
    root: Group,
    detail: Detail,
    cone: BufferGeometry,
    models: ModelSource | null,
  ): void {
    const holders: Object3D[] = [];
    for (const node of device.nodes) {
      const holder = new Group();
      holder.name = node.name;
      holder.matrixAutoUpdate = false;
      holder.matrix.copy(gdtfMatrix(node));
      const parent = node.parent === null ? root : (this.#motion.get(node.parent) ?? holders[node.parent] ?? root);
      parent.add(holder);
      holders.push(holder);
      // Everything under a node hangs from its motion group, so a pan of the
      // yoke carries the head and a tilt of the head carries the lens.
      const motion = new Group();
      motion.name = `${node.name}:motion`;
      holder.add(motion);
      this.#motion.set(node.index, motion);
      this.#dress(fixture, node, motion, detail, models);
      if (node.beam !== null) {
        const radius = node.beam.radius > 0 ? node.beam.radius : Math.max(0.02, Math.min(gdtfSize(node)[0], gdtfSize(node)[1]) / 2);
        const beam = new FixtureBeam(radius, node.beam.beamAngle, detail, cone);
        motion.add(beam.anchor);
        this.beams.set(node.index, beam);
      }
    }
  }

  /** A node's body: its model when it arrives, its primitive until then. */
  #dress(fixture: RigFixture, node: DeviceNode, into: Group, detail: Detail, models: ModelSource | null): void {
    if (node.kind === "Beam" && node.model === null) {
      // A lens is drawn by its beam; its `Cylinder` primitive would hide it.
      return;
    }
    const size = gdtfSize(node);
    const drawable = node.model !== null || node.primitive !== null;
    if (!drawable || size.every((edge) => edge <= 0)) {
      return;
    }
    const stand = primitive(node.primitive ?? node.kind, size, detail.segments);
    into.add(stand);
    stand.traverse((object) => {
      if (object instanceof Mesh) {
        this.bodies.push(object);
      }
    });
    const device = this.#device;
    if (!detail.models || node.model === null || models === null || device === null) {
      return;
    }
    void models({ guid: device.guid, typeId: fixture.typeId }, node.model).then((model) => {
      if (model === null || this.#disposed) {
        return;
      }
      fitTo(model, size);
      into.remove(stand);
      into.add(model);
      model.traverse((object) => {
        object.userData.fixture = fixture.id;
        if (object instanceof Mesh) {
          object.castShadow = true;
          object.receiveShadow = true;
          this.bodies.push(object);
        }
      });
    });
  }

  /** Moves the fixture to where the show hangs it. */
  setPlace(fixture: RigFixture): void {
    const [x, y, z] = showToThree(fixture.position);
    this.place.position.set(x, y, z);
    this.place.quaternion.setFromRotationMatrix(showRotationToThree(orientation(fixture.rotation)));
    this.place.updateMatrixWorld(true);
    this.#fitOutline();
  }

  /** Turns the axes and draws the beams as `state` has them. */
  update(
    state: FixtureState,
    seconds: number,
    haze: number,
    pictures: (guid: string) => Pictures,
    camera: Camera,
    floor: Plane,
  ): void {
    if (this.#device !== null) {
      for (const [index, motion] of this.#motion) {
        const axis = state.axes.get(index);
        motion.rotation.set(axis === undefined ? 0 : (axis.tilt * Math.PI) / 180, 0, axis === undefined ? 0 : (axis.pan * Math.PI) / 180);
      }
    } else if (this.#standIn !== null) {
      this.#standIn.yoke?.rotation.set(0, 0, (state.pan * Math.PI) / 180);
      this.#standIn.head?.rotation.set((state.tilt * Math.PI) / 180, 0, 0);
    }
    this.place.updateMatrixWorld(true);
    const guid = this.#device?.guid ?? "";
    for (const [index, beam] of this.beams) {
      const beamState = this.#device === null ? state.single : state.beams.get(index);
      if (beamState === undefined || beamState === null) {
        continue;
      }
      beam.update(beamState, seconds, this.id + index, haze, pictures(guid), camera, floor);
    }
  }

  set selected(selected: boolean) {
    this.#selected = selected;
    if (this.#outline !== null) {
      this.#outline.visible = selected;
    }
  }

  get selected(): boolean {
    return this.#selected;
  }

  /** The outline box round the fixture's bodies. */
  #fitOutline(): void {
    if (this.#outline === null) {
      return;
    }
    // Measured in the fixture's own frame: a box taken square to the world
    // and turned back is the box of a box, and on a fixture hung at an angle
    // it came out twice the size of the fixture.
    const box = new Box3();
    const inverse = new Matrix4().copy(this.place.matrixWorld).invert();
    const relative = new Matrix4();
    for (const body of this.bodies) {
      if (!(body instanceof Mesh)) {
        continue;
      }
      body.updateWorldMatrix(true, false);
      const geometry = body.geometry as BufferGeometry;
      geometry.computeBoundingBox();
      if (geometry.boundingBox === null) {
        continue;
      }
      relative.multiplyMatrices(inverse, body.matrixWorld);
      box.union(geometry.boundingBox.clone().applyMatrix4(relative));
    }
    if (box.isEmpty()) {
      box.set(new Vector3(-0.15, -0.15, -0.15), new Vector3(0.15, 0.15, 0.15));
    }
    const size = box.getSize(new Vector3()).addScalar(0.06);
    this.#outline.scale.copy(size);
    box.getCenter(this.#outline.position);
  }

  dispose(): void {
    this.#disposed = true;
    for (const beam of this.beams.values()) {
      beam.dispose();
    }
  }
}

/** A unit box of edges, for the selection outline. */
const OUTLINE_BOX = new EdgesGeometry(new BoxGeometry(1, 1, 1));

/**
 * A loaded model, fitted to its geometry: a model whose box is a thousand
 * times its geometry's is in millimetres (3DS files commonly are) and is
 * scaled; anything else is left as the file states it — a GDTF's glTF is in
 * metres by its node's own scale.
 */
export function fitTo(model: Object3D, size: readonly [number, number, number]): void {
  const box = new Box3().setFromObject(model);
  const extent = box.getSize(new Vector3());
  const wanted = Math.max(...size, 0.001);
  const has = Math.max(extent.x, extent.y, extent.z, 0.000001);
  if (has > wanted * 100) {
    model.scale.multiplyScalar(0.001);
  }
}
