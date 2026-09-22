/**
 * One beam of light, drawn — **S30b**.
 *
 * # The shaft in the haze
 *
 * A beam is a truncated cone from its lens to where it lands, and it is drawn
 * as a **volume**: for every pixel the cone covers, the ray from the camera is
 * intersected with the cone, and the light is summed along the part of the ray
 * inside it — at each sample the beam's mask (`./mask.ts`) is read at that
 * point's place across the beam, so a gobo is a set of shafts, a blade cuts a
 * flat face through the haze, and an iris narrows the whole shaft. The light
 * falls off with distance from the lens as the beam spreads, and nothing
 * below the floor is lit. How many samples is the detail level's; nought
 * draws a flat cone, which is the cheapest thing that still reads as a beam.
 *
 * # The cone is computed, not rebuilt
 *
 * The geometry is a unit cone — rings at `t = 0` (the lens) and `t = 1` (the
 * far end), caps at both — and the vertex shader places it from three
 * uniforms: the lens radius, the tangent of the half angle, the length. Zoom,
 * iris and the throw to the floor change every frame and never rebuild a
 * buffer.
 */

import {
  AdditiveBlending,
  BackSide,
  BufferGeometry,
  CircleGeometry,
  Color,
  Float32BufferAttribute,
  FrontSide,
  Group,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  Plane,
  ShaderMaterial,
  Vector3,
  Vector4,
} from "three";
import type { Camera, Texture } from "three";

/** The unit cone: `x, y` round the axis, `z` from nought (lens) to one (far end). */
export function unitCone(segments: number): BufferGeometry {
  const positions: number[] = [];
  const ring = (t: number): number[] => {
    const points: number[] = [];
    for (let segment = 0; segment < segments; segment += 1) {
      const angle = (segment / segments) * Math.PI * 2;
      points.push(Math.cos(angle), Math.sin(angle), t);
    }
    return points;
  };
  const near = ring(0);
  const far = ring(1);
  const at = (points: number[], index: number): [number, number, number] => {
    const i = (index % segments) * 3;
    return [points[i] ?? 0, points[i + 1] ?? 0, points[i + 2] ?? 0];
  };
  for (let segment = 0; segment < segments; segment += 1) {
    const a = at(near, segment);
    const b = at(near, segment + 1);
    const c = at(far, segment);
    const d = at(far, segment + 1);
    positions.push(...a, ...c, ...b, ...b, ...c, ...d);
    // The caps, so a ray looking up the beam still has a face to start from.
    // Wound so each cap's front faces out of the cone: the lens cap towards
    // +z (the fixture), the far cap towards −z.
    positions.push(0, 0, 0, ...a, ...b);
    positions.push(0, 0, 1, ...d, ...c);
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  return geometry;
}

const VERTEX = /* glsl */ `
uniform float uR0;
uniform float uK;
uniform float uL;
varying vec3 vLocal;
void main() {
  float d = position.z * uL;
  float r = uR0 + d * uK;
  vec3 local = vec3(position.xy * r, -d);
  vLocal = local;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(local, 1.0);
}
`;

/** The volume: a ray march through the cone. `STEPS` is a define. */
const VOLUME = /* glsl */ `
uniform vec3 uCam;
uniform float uR0;
uniform float uK;
uniform float uL;
uniform float uA;
uniform float uRot;
uniform sampler2D uMask;
uniform vec3 uColor;
uniform float uLight;
uniform vec4 uFloor;
varying vec3 vLocal;

float maskAt(vec3 q) {
  float d = -q.z;
  if (d < 0.0 || d > uL) return 0.0;
  if (dot(uFloor.xyz, q) + uFloor.w < 0.0) return 0.0;
  float r = uR0 + d * uK;
  vec2 p = q.xy / r;
  if (dot(p, p) > 1.0) return 0.0;
  float c = cos(uRot);
  float s = sin(uRot);
  p = vec2(c * p.x - s * p.y, s * p.x + c * p.y);
  float fall = pow((uA + 0.3) / (uA + d + 0.3), 1.35);
  return texture2D(uMask, p * 0.5 + 0.5).r * fall;
}

void main() {
  vec3 dir = vLocal - uCam;
  float len = length(dir);
  dir /= len;
  float t0 = 0.0;
  float t1 = len;
  if (abs(dir.z) > 1e-5) {
    float ta = (0.0 - uCam.z) / dir.z;
    float tb = (-uL - uCam.z) / dir.z;
    t0 = max(t0, min(ta, tb));
    t1 = min(t1, max(ta, tb));
  }
  vec3 o = vec3(uCam.xy, uA - uCam.z);
  vec3 dd = vec3(dir.xy, -dir.z);
  float k2 = uK * uK;
  float qa = dd.x * dd.x + dd.y * dd.y - k2 * dd.z * dd.z;
  float qb = 2.0 * (o.x * dd.x + o.y * dd.y - k2 * o.z * dd.z);
  float qc = o.x * o.x + o.y * o.y - k2 * o.z * o.z;
  if (qa > 1e-6) {
    float disc = qb * qb - 4.0 * qa * qc;
    if (disc <= 0.0) discard;
    float root = sqrt(disc);
    t0 = max(t0, (-qb - root) / (2.0 * qa));
    t1 = min(t1, (-qb + root) / (2.0 * qa));
  }
  if (t1 <= t0) discard;
  float stepLength = (t1 - t0) / float(STEPS);
  float sum = 0.0;
  for (int i = 0; i < STEPS; i++) {
    float t = t0 + (float(i) + 0.5) * stepLength;
    sum += maskAt(uCam + dir * t);
  }
  float light = sum * stepLength * uLight;
  gl_FragColor = vec4(uColor * light, 1.0);
}
`;

/** The flat cone, for the lowest detail. */
const SHELL = /* glsl */ `
uniform float uL;
uniform vec3 uColor;
uniform float uLight;
uniform vec4 uFloor;
varying vec3 vLocal;
void main() {
  if (dot(uFloor.xyz, vLocal) + uFloor.w < 0.0) discard;
  float d = -vLocal.z / max(uL, 0.001);
  float light = uLight * 0.25 * pow(1.0 - clamp(d, 0.0, 1.0), 1.5);
  gl_FragColor = vec4(uColor * light, 1.0);
}
`;

/** What one beam is drawn with this frame. */
export interface BeamLook {
  /** Light leaving it, `0..` — intensity, strobe and haze together. */
  light: number;
  /** Linear colour. */
  color: [number, number, number];
  /** Full angle, degrees. */
  angle: number;
  /** How far it is drawn, metres. */
  length: number;
  /** Radians the mask is turned — a spinning gobo. */
  rotation: number;
}

/** One cone of light: a volume and the uniforms that shape it. */
export class BeamVolume {
  readonly mesh: Mesh;
  readonly material: ShaderMaterial;
  #radius: number;

  constructor(geometry: BufferGeometry, steps: number, mask: Texture, radius: number) {
    this.#radius = Math.max(0.005, radius);
    this.material = new ShaderMaterial({
      vertexShader: VERTEX,
      fragmentShader: steps > 0 ? VOLUME : SHELL,
      defines: steps > 0 ? { STEPS: steps } : {},
      uniforms: {
        uR0: { value: this.#radius },
        uK: { value: Math.tan((20 * Math.PI) / 360) },
        uL: { value: 10 },
        uA: { value: 1 },
        uCam: { value: new Vector3() },
        uRot: { value: 0 },
        uMask: { value: mask },
        uColor: { value: new Color(1, 1, 1) },
        uLight: { value: 0 },
        uFloor: { value: new Vector4(0, 1, 0, 0) },
      },
      transparent: true,
      depthWrite: false,
      blending: AdditiveBlending,
      side: steps > 0 ? BackSide : FrontSide,
    });
    this.mesh = new Mesh(geometry, this.material);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = 10;
  }

  /** Samples a different mask — a beam's own once something is in it. */
  setMask(mask: Texture): void {
    const uniform = this.material.uniforms.uMask;
    if (uniform !== undefined && uniform.value !== mask) {
      uniform.value = mask;
    }
  }

  /** Shapes the cone for this frame. `camera` is read for its place. */
  set(look: BeamLook, camera: Camera, floor: Plane): void {
    const uniforms = this.material.uniforms;
    const k = Math.tan((Math.min(Math.max(look.angle, 0.5), 170) * Math.PI) / 360);
    setValue(uniforms.uK, k);
    setValue(uniforms.uL, look.length);
    setValue(uniforms.uA, this.#radius / k);
    setValue(uniforms.uLight, look.light);
    setValue(uniforms.uRot, look.rotation);
    const colour = uniforms.uColor?.value;
    if (colour instanceof Color) {
      colour.setRGB(look.color[0], look.color[1], look.color[2]);
    }
    this.mesh.visible = look.light > 0.0005;
    if (!this.mesh.visible) {
      return;
    }
    const inverse = SCRATCH_MATRIX.copy(this.mesh.matrixWorld).invert();
    const cam = uniforms.uCam?.value;
    if (cam instanceof Vector3) {
      cam.setFromMatrixPosition(camera.matrixWorld).applyMatrix4(inverse);
    }
    const plane = SCRATCH_PLANE.copy(floor).applyMatrix4(inverse);
    const target = uniforms.uFloor?.value;
    if (target instanceof Vector4) {
      target.set(plane.normal.x, plane.normal.y, plane.normal.z, plane.constant);
    }
  }

  dispose(): void {
    this.material.dispose();
  }
}

const SCRATCH_MATRIX = new Matrix4();
const SCRATCH_PLANE = new Plane();

function setValue(uniform: { value: unknown } | undefined, value: number): void {
  if (uniform !== undefined) {
    uniform.value = value;
  }
}

/** The disc every lens is scaled from. */
const UNIT_DISC = new CircleGeometry(1, 32);

/** The lens: a disc in the beam's colour, bright enough to bloom. */
export function lensDisc(radius: number): Mesh<CircleGeometry, MeshBasicMaterial> {
  // One unit disc for every lens of a rig, scaled to each: the colour is each
  // beam's own, so the material is not shared.
  const lens = new Mesh(UNIT_DISC, new MeshBasicMaterial({ color: 0x111418, toneMapped: false }));
  const scale = Math.max(0.005, radius);
  lens.scale.set(scale, scale, 1);
  // It faces down the beam: its normal is the beam's −Z.
  lens.rotation.x = Math.PI;
  lens.position.z = -0.002;
  return lens;
}

/** A beam's place in its geometry: a group whose −Z is the beam. */
export function beamAnchor(): Group {
  const anchor = new Group();
  anchor.name = "beam";
  return anchor;
}
