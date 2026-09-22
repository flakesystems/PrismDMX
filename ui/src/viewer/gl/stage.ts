/**
 * The 3D stage: a WebGL scene the rig hangs in — **S30b**, which replaced
 * S30's 2D canvas once the owner had seen a real fixture drawn in it.
 *
 * # What it draws
 *
 * - **The floor** and a faint grid, lit only by what the rig throws on it.
 * - **Every fixture** (`./fixture3d.ts`): its own models where the desk has
 *   them, its axes turned by pan and tilt, its beams as volumes in the haze.
 * - **Real lights** — a fixed pool of spot lights, handed each frame to the
 *   brightest beams: each throws its beam's mask (gobo, iris, blades) in its
 *   beam's colour onto the floor and the fixtures, softened by focus and frost.
 *   The pool is **fixed** because a scene whose number of lights changes
 *   recompiles every material in it, which is a stall on every cue.
 * - A **glow** around bright light at the higher detail levels.
 *
 * # What it does not do
 *
 * It asks the daemon for nothing but files (`../resources.ts`), sets no React
 * state, and never touches the tick. The frame it draws is read off the
 * telemetry sink by the window's loop, exactly as the DMX Sheet reads its own.
 */

import {
  ACESFilmicToneMapping,
  AmbientLight,
  Color,
  DirectionalLight,
  GridHelper,
  HemisphereLight,
  LineBasicMaterial,
  MeshStandardMaterial,
  Object3D,
  PerspectiveCamera,
  Plane,
  Raycaster,
  SRGBColorSpace,
  Scene,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
import type { BufferGeometry, Mesh } from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { TDSLoader } from "three/examples/jsm/loaders/TDSLoader.js";
import { EffectComposer } from "three/examples/jsm/postprocessing/EffectComposer.js";
import { OutputPass } from "three/examples/jsm/postprocessing/OutputPass.js";
import { RenderPass } from "three/examples/jsm/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";

import type { TelemetryFrameView } from "../../telemetry/frame";
import type { Camera as Orbit } from "../camera";
import { eyeOf } from "../camera";
import type { ResourceCache } from "../resources";
import type { RigFixture } from "../rig";
import type { SceneStats } from "../scene";
import { noStats } from "../scene";
import type { FixtureState } from "../state";
import { emptyState, readState } from "../state";
import { unitCone } from "./beam3d";
import { GLTF_TO_GDTF, showToThree } from "./coords";
import { HOUSING } from "./primitives";
import type { Detail } from "./detail";
import type { FixtureBeam, ModelSource } from "./fixture3d";
import { Fixture3D } from "./fixture3d";
import type { Projector } from "./floor";
import { Floor } from "./floor";
import { graphics } from "./graphics";
import type { Picture, Pictures } from "./mask";

/** A size a stage gave its renderer: CSS pixels and device pixels per one. */
export interface AppliedSize {
  readonly width: number;
  readonly height: number;
  readonly scale: number;
}

/** Whether a stage must size its renderer again — always, the first time. */
export function sizeChanged(applied: AppliedSize | null, width: number, height: number, scale: number): boolean {
  return applied === null || applied.width !== width || applied.height !== height || applied.scale !== scale;
}

/** How much of the resolution a software renderer draws at. */
const SOFTWARE_SCALE = 0.5;

/** Whether this browser can draw WebGL at all — see `./graphics.ts`. */
export function webglAvailable(): boolean {
  return graphics().webgl;
}

/** The stage and everything on it. */
export class Stage {
  readonly renderer: WebGLRenderer;
  readonly scene = new Scene();
  readonly camera = new PerspectiveCamera(50, 1, 0.05, 400);
  readonly floor = new Plane(new Vector3(0, 1, 0), 0);
  #detail: Detail;
  #fixtures = new Map<number, Fixture3D>();
  #states = new Map<number, FixtureState>();
  #rig: readonly RigFixture[] = [];
  #cone: BufferGeometry;
  #floorMesh: Floor;
  #composer: EffectComposer | null = null;
  #models: ModelSource | null;
  #pictures = new Map<string, Picture | "loading" | "none">();
  #resources: ResourceCache | null;
  #outline = new LineBasicMaterial({ color: 0xffd479 });
  /** The size this stage last gave the renderer; nothing until the first. */
  #applied: AppliedSize | null = null;
  /** The fence behind the last frame handed to the GPU, until it is passed. */
  #fence: WebGLSync | null = null;
  #raycaster = new Raycaster();
  #animated = false;
  #changed = () => {};

  constructor(canvas: HTMLCanvasElement, detail: Detail, resources: ResourceCache | null) {
    this.#detail = detail;
    this.#resources = resources;
    // Multisampling is four times the filling, which a graphics card does for
    // free and a software renderer does on the processor the console needs
    // (`./graphics.ts`): there it is left off.
    this.renderer = new WebGLRenderer({ canvas, antialias: !graphics().software, powerPreference: "high-performance" });
    this.renderer.outputColorSpace = SRGBColorSpace;
    this.renderer.toneMapping = ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.1;
    this.scene.background = new Color(0x0a0d12);
    this.#cone = unitCone(detail.segments);
    this.#floorMesh = new Floor(detail.projectors, Math.min(256, detail.mask));
    this.scene.add(this.#floorMesh.mesh);
    const grid = new GridHelper(40, 40, 0x3a4250, 0x1c2129);
    grid.position.y = 0.002;
    this.scene.add(grid);
    // Enough light to see the rig by, and no more: the stage is dark until
    // the fixtures light it.
    this.scene.add(new HemisphereLight(0x9aa8c0, 0x1a1c22, 1.1));
    this.scene.add(new AmbientLight(0xffffff, 0.25));
    // A cool light from the front and above, like the house's working light,
    // so a housing reads against the dark.
    const key = new DirectionalLight(0xb8c4d8, 0.9);
    key.position.set(4, 10, 12);
    this.scene.add(key);

    // Every fixture brings its own world matrices up to date when it moves
    // (`Fixture3D.update`) and nothing else here moves, so three.js is not
    // asked to walk the whole scene again on every render: at five hundred
    // fixtures that walk was a third of a frame.
    this.scene.matrixWorldAutoUpdate = false;
    this.scene.updateMatrixWorld(true);

    this.#models = resources === null ? null : this.#modelSource(resources);
    this.#makeComposer();
  }

  /** Called when something arrives that needs a new picture — a model, a gobo. */
  onChange(changed: () => void): void {
    this.#changed = changed;
  }

  #makeComposer(): void {
    if (!this.#detail.bloom) {
      return;
    }
    const composer = new EffectComposer(this.renderer);
    composer.addPass(new RenderPass(this.scene, this.camera));
    composer.addPass(new UnrealBloomPass(new Vector2(256, 256), 0.55, 0.6, 0.72));
    composer.addPass(new OutputPass());
    this.#composer = composer;
  }

  /** How a model is fetched and turned into a scene object. */
  #modelSource(resources: ResourceCache): ModelSource {
    const loaded = new Map<string, Promise<Object3D | null>>();
    const gltf = new GLTFLoader();
    const tds = new TDSLoader();
    return (device, name) => {
      const key = `${device.guid || device.typeId}|${name}`;
      let held = loaded.get(key);
      if (held === undefined) {
        held = resources.get(device, "Model", name).then(async (file) => {
          if (file === null) {
            return null;
          }
          if (file.path.toLowerCase().endsWith(".3ds")) {
            // 3DS is Z-up like GDTF, so it goes in as it is.
            return tds.parse(file.bytes.buffer, "");
          }
          const parsed = await gltf.parseAsync(file.bytes.buffer, "");
          liftBlack(parsed.scene);
          const holder = new Object3D();
          holder.add(parsed.scene);
          holder.matrixAutoUpdate = false;
          holder.matrix.copy(GLTF_TO_GDTF);
          return holder;
        });
        held.then(() => this.#changed()).catch(() => undefined);
        loaded.set(key, held);
      }
      // Each fixture gets its own copy: one object cannot hang in two places.
      return held.then((model) => (model === null ? null : model.clone(true))).catch(() => null);
    };
  }

  /** The pictures of one device's wheels, as they arrive. */
  picturesOf(guid: string, typeId: string): Pictures {
    return (media) => {
      const key = `${guid || typeId}|${media}`;
      const held = this.#pictures.get(key);
      if (held === undefined && this.#resources !== null) {
        this.#pictures.set(key, "loading");
        void this.#resources.get({ guid, typeId }, "Wheel", media).then(async (file) => {
          if (file === null) {
            this.#pictures.set(key, "none");
            return;
          }
          try {
            const picture = await createImageBitmap(new Blob([file.bytes]));
            this.#pictures.set(key, picture);
            this.#changed();
          } catch {
            this.#pictures.set(key, "none");
          }
        });
        return null;
      }
      return held === undefined || typeof held === "string" ? null : held;
    };
  }

  /** Brings the scene in line with the rig: new fixtures built, gone ones removed, moved ones moved. */
  setRig(rig: readonly RigFixture[]): void {
    if (rig === this.#rig) {
      return;
    }
    const wanted = new Map(rig.map((fixture) => [fixture.id, fixture]));
    const before = new Map(this.#rig.map((fixture) => [fixture.id, fixture]));
    for (const [id, built] of this.#fixtures) {
      const fixture = wanted.get(id);
      const previous = before.get(id);
      // A fixture whose profile changed is rebuilt; one that only moved is moved.
      if (fixture === undefined || previous === undefined || previous.typeId !== fixture.typeId || previous.device !== fixture.device) {
        this.scene.remove(built.place);
        built.dispose();
        this.#fixtures.delete(id);
        this.#states.delete(id);
      } else if (fixture !== previous) {
        built.setPlace(fixture);
      }
    }
    for (const fixture of rig) {
      if (!this.#fixtures.has(fixture.id)) {
        const built = new Fixture3D(fixture, this.#detail, this.#cone, this.#models, this.#outline);
        this.#fixtures.set(fixture.id, built);
        this.#states.set(fixture.id, emptyState());
        this.scene.add(built.place);
      }
    }
    this.#rig = rig;
  }

  setSelection(selection: ReadonlySet<number>): void {
    for (const [id, built] of this.#fixtures) {
      built.selected = selection.has(id);
    }
  }

  /** Sizes the drawing buffer to the canvas. Answers whether it changed. */
  resize(width: number, height: number, ratio: number): boolean {
    // A software renderer fills every pixel on the processor: half the
    // resolution is a quarter of that (`./graphics.ts`).
    const scale = Math.min(ratio, this.#detail.pixelRatio) * (graphics().software ? SOFTWARE_SCALE : 1);
    // Compared with what this stage last applied, never with what the
    // renderer reports: a stage built when the detail changes takes over a
    // canvas the last one already sized, and its renderer reads that size
    // back as its own. Where the old and the new level draw at the same
    // resolution, nothing looked changed and the camera kept its square
    // aspect — the picture was stretched until the window was resized.
    if (!sizeChanged(this.#applied, width, height, scale)) {
      return false;
    }
    this.#applied = { width, height, scale };
    this.renderer.setPixelRatio(scale);
    this.renderer.setSize(width, height, false);
    this.#composer?.setPixelRatio(scale);
    this.#composer?.setSize(width, height);
    this.camera.aspect = width / Math.max(1, height);
    this.camera.updateProjectionMatrix();
    return true;
  }

  /** Whether the last picture had something moving on its own — a strobe, a spinning gobo. */
  get animated(): boolean {
    return this.#animated;
  }

  /** Draws the rig as `frame` has it, from `orbit`, at `seconds`. */
  /**
   * Reads every fixture out of `frame`, moves the rig to match and draws it.
   * With `render` false the rig is read and moved and nothing is drawn — for
   * a window whose picture is hidden, whose readout still has to be true.
   */
  draw(frame: TelemetryFrameView | null, orbit: Orbit, seconds: number, haze: number, render = true): SceneStats {
    const stats = noStats();
    const eye = eyeOf(orbit);
    const [ex, ey, ez] = showToThree(eye);
    const [tx, ty, tz] = showToThree(orbit.target);
    this.camera.position.set(ex, ey, ez);
    this.camera.lookAt(tx, ty, tz);
    this.camera.updateMatrixWorld();

    let animated = false;
    const projectors: Projector[] = [];
    const lit: FixtureBeam[] = [];
    for (const fixture of this.#rig) {
      const built = this.#fixtures.get(fixture.id);
      const state = this.#states.get(fixture.id);
      if (built === undefined || state === undefined) {
        continue;
      }
      readState(fixture, frame, state);
      stats.fixtures += 1;
      if (fixture.unplaced) {
        stats.unplaced += 1;
      }
      if (frame !== null && !state.present) {
        stats.absent += 1;
      }
      built.update(state, seconds, haze, (guid) => this.picturesOf(guid, fixture.typeId), this.camera, this.floor);
      let any = false;
      for (const beam of built.beams.values()) {
        if (beam.light > 0.02) {
          any = true;
          stats.beams += 1;
          beam.projectors(projectors);
          lit.push(beam);
        }
      }
      const beams = [...state.beams.values(), ...(state.single === null ? [] : [state.single])];
      if (beams.some((beam) => beam.intensity > 0.02 && (beam.strobe !== "none" || beam.gobos.some((gobo) => gobo.spin !== 0) || (beam.prism?.spin ?? 0) !== 0))) {
        animated = true;
      }
      if (any) {
        stats.lit += 1;
      }
    }
    this.#animated = animated;
    if (lit.length > this.#detail.volumes) {
      // Only the brightest are drawn in the haze (`Detail.volumes`).
      lit.sort((left, right) => right.light - left.light);
      for (let index = this.#detail.volumes; index < lit.length; index += 1) {
        lit[index]?.hideVolumes();
      }
    }
    projectors.sort((left, right) => sum(right.color) - sum(left.color));
    this.#floorMesh.set(projectors);

    if (!render) {
      return stats;
    }
    if (this.#composer !== null) {
      this.#composer.render();
    } else {
      this.renderer.render(this.scene, this.camera);
    }
    this.#fenceFrame();
    return stats;
  }

  /**
   * Puts a fence behind the frame just handed to the GPU, so
   * {@link Stage.gpuBusy} can tell when the GPU has finished it without
   * waiting for it.
   */
  #fenceFrame(): void {
    const gl = this.renderer.getContext();
    if (!(gl instanceof WebGL2RenderingContext)) {
      return;
    }
    if (this.#fence !== null) {
      gl.deleteSync(this.#fence);
    }
    this.#fence = gl.fenceSync(gl.SYNC_GPU_COMMANDS_COMPLETE, 0);
    gl.flush();
  }

  /**
   * Whether the GPU is still drawing the last frame. Asked without waiting:
   * a status read, never `clientWaitSync` with a timeout. The loop draws
   * nothing new while it is (`./driver3d.ts`), because a frame queued behind
   * another is time taken from every other canvas on the page — on a machine
   * that draws WebGL in software, the DMX Sheet's.
   */
  gpuBusy(): boolean {
    const fence = this.#fence;
    if (fence === null) {
      return false;
    }
    const gl = this.renderer.getContext();
    if (!(gl instanceof WebGL2RenderingContext)) {
      return false;
    }
    if (gl.getSyncParameter(fence, gl.SYNC_STATUS) !== gl.SIGNALED) {
      return true;
    }
    gl.deleteSync(fence);
    this.#fence = null;
    return false;
  }

  /** The fixture drawn under a point of the canvas, in CSS pixels, or `null`. */
  pick(x: number, y: number, width: number, height: number): number | null {
    const pointer = new Vector2((x / width) * 2 - 1, -(y / height) * 2 + 1);
    this.#raycaster.setFromCamera(pointer, this.camera);
    const bodies = [...this.#fixtures.values()].flatMap((fixture) => fixture.bodies);
    const hit = this.#raycaster.intersectObjects(bodies, false)[0];
    const id = hit?.object.userData.fixture;
    return typeof id === "number" ? id : null;
  }

  dispose(): void {
    for (const built of this.#fixtures.values()) {
      built.dispose();
    }
    this.#fixtures.clear();
    this.#composer?.dispose();
    this.#floorMesh.dispose();
    this.renderer.dispose();
  }
}

/** A colour's three components added up — how bright a projector is. */
function sum(color: readonly [number, number, number]): number {
  return color[0] + color[1] + color[2];
}

/**
 * Makes a model's near-black surfaces a dark housing grey. Robe's own models
 * are painted in the paint the real fixture has, a black of 0.003 — which
 * is accurate and, on a dark stage, invisible. A console draws the rig to be
 * *read*, so a body is a shade the lights can show.
 */
function liftBlack(model: Object3D): void {
  model.traverse((object) => {
    const mesh = object as Partial<Mesh>;
    const materials = mesh.material === undefined ? [] : Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    for (const material of materials) {
      if (material instanceof MeshStandardMaterial && material.color.r + material.color.g + material.color.b < 0.12) {
        material.color.copy(HOUSING.color);
        material.metalness = Math.min(material.metalness, 0.35);
        material.roughness = Math.max(material.roughness, 0.5);
      }
    }
  });
}
