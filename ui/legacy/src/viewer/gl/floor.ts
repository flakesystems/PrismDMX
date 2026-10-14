/**
 * The floor, and every beam's light on it — **S30b**.
 *
 * # Why not three.js's spot lights
 *
 * A spot light throws a picture (`SpotLight.map`) only while it casts a
 * shadow, so each one costs a picture **and** a shadow map in every material
 * it lights — two texture units of the sixteen WebGL guarantees. Six lit
 * beams was the ceiling, and the first real rig (four Robin T1s with 24
 * lights in the pool) failed to compile its materials at all.
 *
 * So the floor is lit the way visualisers do it: every beam's mask
 * (`./mask.ts`) is copied into one **atlas**, and the floor's own shader
 * projects each beam onto itself — where the fragment is in the beam's frame,
 * whether that is inside the cone, what the mask says there — and adds the
 * beam's colour. One texture however many beams, and the gobo, the iris, the
 * blades and the softness land on the floor exactly as they are drawn in the
 * haze, because they are read from the same picture.
 */

import {
  CanvasTexture,
  Color,
  LinearFilter,
  Mesh,
  PlaneGeometry,
  ShaderMaterial,
  Vector3,
} from "three";

/** One beam as the floor sees it. */
export interface Projector {
  /** Where the light leaves, world. */
  readonly position: Vector3;
  /** Which way it points, world, unit. */
  readonly direction: Vector3;
  /** The mask's "up", world, unit, at right angles to `direction` — a spinning gobo turns it. */
  readonly up: Vector3;
  /** Tangent of the half angle. */
  readonly spread: number;
  /** Colour times light. */
  readonly color: readonly [number, number, number];
  /** Its mask. */
  readonly mask: HTMLCanvasElement;
  /** Changes whenever the mask is redrawn. */
  readonly version: number;
}

const VERTEX = /* glsl */ `
varying vec3 vWorld;
void main() {
  vec4 world = modelMatrix * vec4(position, 1.0);
  vWorld = world.xyz;
  gl_Position = projectionMatrix * viewMatrix * world;
}
`;

function fragment(count: number): string {
  return /* glsl */ `
#define COUNT ${String(count)}
uniform sampler2D uAtlas;
uniform float uColumns;
uniform int uUsed;
uniform vec3 uPos[COUNT];
uniform vec3 uDir[COUNT];
uniform vec3 uUp[COUNT];
uniform float uK[COUNT];
uniform vec3 uColor[COUNT];
uniform vec3 uBase;
varying vec3 vWorld;

void main() {
  vec3 light = uBase;
  for (int i = 0; i < COUNT; i++) {
    if (i >= uUsed) break;
    vec3 v = vWorld - uPos[i];
    float z = dot(v, uDir[i]);
    if (z <= 0.0) continue;
    vec3 right = normalize(cross(uDir[i], uUp[i]));
    float r = z * uK[i];
    vec2 p = vec2(dot(v, right), dot(v, uUp[i])) / r;
    if (dot(p, p) > 1.0) continue;
    float slot = float(i);
    vec2 cell = vec2(mod(slot, uColumns), floor(slot / uColumns));
    vec2 uv = (cell + (p * 0.5 + 0.5) * 0.98 + 0.01) / uColumns;
    float mask = texture2D(uAtlas, uv).r;
    // Lambert on a floor facing up, and the light spread over the area the
    // cone covers at this distance.
    float facing = max(0.0, -uDir[i].y);
    float spread = 1.0 / (1.0 + r * r * 2.2);
    light += uColor[i] * mask * facing * spread * 2.6;
  }
  gl_FragColor = vec4(light, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;
}

/** The floor mesh and the atlas its projectors are read from. */
export class Floor {
  readonly mesh: Mesh<PlaneGeometry, ShaderMaterial>;
  readonly #atlas: HTMLCanvasElement;
  readonly #texture: CanvasTexture;
  readonly #count: number;
  readonly #cell: number;
  readonly #columns: number;
  readonly #held: { mask: HTMLCanvasElement | null; version: number }[] = [];
  readonly #pos: Vector3[];
  readonly #dir: Vector3[];
  readonly #up: Vector3[];
  readonly #k: number[];
  readonly #color: Vector3[];
  readonly #used = { value: 0 };

  /** `count` projectors at most, each mask copied at `cell` pixels. */
  constructor(count: number, cell: number) {
    this.#count = Math.max(1, count);
    this.#cell = cell;
    this.#columns = Math.ceil(Math.sqrt(this.#count));
    this.#atlas = document.createElement("canvas");
    this.#atlas.width = this.#columns * cell;
    this.#atlas.height = this.#columns * cell;
    this.#texture = new CanvasTexture(this.#atlas);
    this.#texture.minFilter = LinearFilter;
    this.#texture.magFilter = LinearFilter;
    this.#texture.generateMipmaps = false;
    // Cell 0 is the canvas's top left, and the shader counts rows from the
    // top: a flipped upload would have every projector read an empty cell.
    this.#texture.flipY = false;
    for (let index = 0; index < this.#count; index += 1) {
      this.#held.push({ mask: null, version: -1 });
    }
    const vectors = (): Vector3[] => Array.from({ length: this.#count }, () => new Vector3());
    this.#pos = vectors();
    this.#dir = vectors();
    this.#up = vectors();
    this.#k = new Array<number>(this.#count).fill(0.2);
    this.#color = vectors();
    const material = new ShaderMaterial({
      vertexShader: VERTEX,
      fragmentShader: fragment(this.#count),
      uniforms: {
        uAtlas: { value: this.#texture },
        uColumns: { value: this.#columns },
        uUsed: this.#used,
        uPos: { value: this.#pos },
        uDir: { value: this.#dir },
        uUp: { value: this.#up },
        uK: { value: this.#k },
        uColor: { value: this.#color },
        uBase: { value: new Color(0.018, 0.02, 0.024) },
      },
    });
    this.mesh = new Mesh(new PlaneGeometry(200, 200), material);
    this.mesh.rotation.x = -Math.PI / 2;
    this.mesh.name = "floor";
  }

  /** Lights the floor with these projectors, brightest first; the rest are dropped. */
  set(projectors: readonly Projector[]): void {
    const used = Math.min(this.#count, projectors.length);
    const context = this.#atlas.getContext("2d");
    let dirty = false;
    for (let index = 0; index < used; index += 1) {
      const projector = projectors[index];
      if (projector === undefined) {
        continue;
      }
      const held = this.#held[index];
      if (context !== null && held !== undefined && (held.mask !== projector.mask || held.version !== projector.version)) {
        const x = (index % this.#columns) * this.#cell;
        const y = Math.floor(index / this.#columns) * this.#cell;
        context.drawImage(projector.mask, x, y, this.#cell, this.#cell);
        held.mask = projector.mask;
        held.version = projector.version;
        dirty = true;
      }
      this.#pos[index]?.copy(projector.position);
      this.#dir[index]?.copy(projector.direction);
      this.#up[index]?.copy(projector.up);
      this.#k[index] = projector.spread;
      this.#color[index]?.set(projector.color[0], projector.color[1], projector.color[2]);
    }
    this.#used.value = used;
    if (dirty) {
      this.#texture.needsUpdate = true;
    }
  }

  dispose(): void {
    this.#texture.dispose();
    this.mesh.geometry.dispose();
    this.mesh.material.dispose();
  }
}
