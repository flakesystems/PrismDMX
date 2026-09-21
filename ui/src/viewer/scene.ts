/**
 * One picture of the rig: the floor, every beam and every fixture body, drawn
 * onto a {@link ViewSurface} from a camera — **S30**.
 *
 * # The order is the depth buffer
 *
 * There is no depth buffer on a 2D canvas, so the order things are drawn in is
 * what decides what hides what:
 *
 * 1. **The floor grid**, which is under everything.
 * 2. **Every beam**, as light *added* to what is under it — so two beams
 *    crossing are brighter where they cross, and the order among beams does
 *    not matter at all. Where a beam lands on the floor it leaves a pool.
 * 3. **Every body**, far to near, each face lit by one fixed light and the
 *    faces turned away not drawn. A body in front of a beam hides it; a beam in
 *    front of a body is under it — the one thing the painter's order gets
 *    wrong, and it is a small thing next to a head drawn inside its own beam.
 * 4. **The lens** of each fixture, in the colour it is putting out, so a head
 *    whose beam leaves the picture still says it is lit.
 * 5. **The numbers**, which are what an operator reads a rig by.
 *
 * # What a fixture is at
 *
 * `rig.ts` says where it hangs and what it is; `look.ts` says what the cable
 * is telling it. A beam leaves the body at the profile's place, turned by the
 * fixture's orientation and then by its **yoke** — pan and tilt,
 * `space.ts::yoke` — and throws at the zoom's angle or the profile's own.
 */

import type { TelemetryFrameView } from "../telemetry/frame";
import type { Camera, Lens } from "./camera";
import { clipToNear, depthOf, lensOf, project } from "./camera";
import type { Look } from "./look";
import { LIT, darkLook, readLook } from "./look";
import type { RigFixture } from "./rig";
import type { V3 } from "./space";
import { add, around, dot, multiply, normalize, scale, sub, turn, v3, yoke } from "./space";
import type { ViewSurface } from "./surface";

/** The colours. Dark, because a lighting desk is used in a dark room. */
export const INK = {
  background: "#07090d",
  grid: "#161d27",
  axis: "#26313e",
  body: [74, 82, 95] as const,
  outline: "#0b0e13",
  selected: "#ffd479",
  lens: "#1b222c",
  label: "#8b98a8",
} as const;

/** How many rays a beam's cone is drawn with. */
const RAYS = 16;

/** How far a beam that lands nowhere is drawn, in metres. */
export const BEAM_REACH = 18;

/** The furthest a beam is followed to the floor, in metres. */
const FLOOR_REACH = 80;

/** Above this many fixtures only the selected ones are numbered. */
export const LABEL_LIMIT = 96;

/** Where the light that shades the bodies comes from. */
const LIGHT = normalize(v3(0.4, 1, -0.7));

/** The six faces of a box: its corners by bit (x = 1, y = 2, z = 4) and its outward normal. */
const FACES: readonly (readonly [readonly [number, number, number, number], V3])[] = [
  [[0, 2, 6, 4], v3(-1, 0, 0)],
  [[1, 5, 7, 3], v3(1, 0, 0)],
  [[0, 4, 5, 1], v3(0, -1, 0)],
  [[2, 3, 7, 6], v3(0, 1, 0)],
  [[0, 1, 3, 2], v3(0, 0, -1)],
  [[4, 6, 7, 5], v3(0, 0, 1)],
];

/** What one picture contained. Written into the readout and read by the tests. */
export interface SceneStats {
  /** Fixtures in the rig. */
  fixtures: number;
  /** Fixtures putting out light. */
  lit: number;
  /** Fixtures still where a patch leaves them. */
  unplaced: number;
  /** Fixtures whose universe the frame does not carry. */
  absent: number;
  /** Beams drawn. */
  beams: number;
}

/** A fresh, empty set of statistics. */
export function noStats(): SceneStats {
  return { fixtures: 0, lit: 0, unplaced: 0, absent: 0, beams: 0 };
}

/**
 * The picture, and the buffers it is drawn through — kept between frames, so
 * a picture thirty times a second is not thirty sets of arrays for the
 * collector.
 */
export class Scene {
  #points = new Float64Array(2 * (RAYS + 2));
  #hull = new Float64Array(2 * (RAYS + 2));
  #order = new Int32Array(RAYS + 2);
  #ends: V3[] = [];
  #screen = new Float64Array(2 * 8);
  #looks: Look[] = [];
  #centres = new Float64Array(0);
  #ids: number[] = [];
  #depth: number[] = [];
  #sorted: number[] = [];

  /**
   * Draws `rig` as `frame` has it, from `camera`, and says what was drawn.
   *
   * `frame` is `null` before any telemetry has arrived, and every fixture is
   * then dark: a rig nobody has told anything is a rig at home.
   */
  draw(
    surface: ViewSurface,
    rig: readonly RigFixture[],
    frame: TelemetryFrameView | null,
    camera: Camera,
    selection: ReadonlySet<number>,
    pixelScale = 1,
  ): SceneStats {
    const stats = noStats();
    const lens = lensOf(camera, surface.width, surface.height);
    surface.clear(INK.background);
    this.#floor(surface, lens, rig, pixelScale);

    while (this.#looks.length < rig.length) {
      this.#looks.push(darkLook());
    }
    if (this.#centres.length < rig.length * 2) {
      this.#centres = new Float64Array(rig.length * 2);
    }
    this.#ids.length = rig.length;

    stats.fixtures = rig.length;
    for (let index = 0; index < rig.length; index += 1) {
      const fixture = rig[index];
      const look = this.#looks[index];
      if (fixture === undefined || look === undefined) {
        continue;
      }
      if (frame === null || !frame.hasFrame) {
        const dark = darkLook();
        Object.assign(look, dark);
      } else {
        readLook(fixture, frame, look);
        if (!look.present) {
          stats.absent += 1;
        }
      }
      if (fixture.unplaced) {
        stats.unplaced += 1;
      }
      if (look.intensity >= LIT) {
        stats.lit += 1;
        stats.beams += this.#beams(surface, lens, fixture, look);
      }
    }

    // Far to near.
    this.#depth.length = rig.length;
    this.#sorted.length = rig.length;
    for (let index = 0; index < rig.length; index += 1) {
      const fixture = rig[index];
      this.#depth[index] = fixture === undefined ? 0 : depthOf(lens, fixture.position);
      this.#sorted[index] = index;
    }
    const depth = this.#depth;
    this.#sorted.sort((left, right) => (depth[right] ?? 0) - (depth[left] ?? 0));

    const numbered = rig.length <= LABEL_LIMIT;
    for (const index of this.#sorted) {
      const fixture = rig[index];
      const look = this.#looks[index];
      if (fixture === undefined || look === undefined) {
        continue;
      }
      const selected = selection.has(fixture.id);
      this.#body(surface, lens, fixture, look, selected);
      this.#ids[index] = fixture.id;
      const at = project(lens, fixture.position, this.#centres, index * 2);
      if (!at) {
        this.#centres[index * 2] = Number.NaN;
        this.#centres[index * 2 + 1] = Number.NaN;
      }
      if (at && (numbered || selected)) {
        const top = add(fixture.position, v3(0, fixture.size.y / 2 + 0.18, 0));
        if (project(lens, top, this.#screen, 0)) {
          surface.label(
            String(fixture.id),
            this.#screen[0] ?? 0,
            this.#screen[1] ?? 0,
            selected ? INK.selected : INK.label,
            Math.round(11 * pixelScale),
          );
        }
      }
    }
    return stats;
  }

  /**
   * The fixture drawn nearest to `x`, `y` on the surface, within `radius`
   * pixels, or `null`. Reads the last picture, so it answers what the operator
   * is looking at.
   */
  pick(x: number, y: number, radius: number): number | null {
    let best: number | null = null;
    let bestDistance = radius * radius;
    for (let index = 0; index < this.#ids.length; index += 1) {
      const px = this.#centres[index * 2] ?? Number.NaN;
      const py = this.#centres[index * 2 + 1] ?? Number.NaN;
      if (Number.isNaN(px) || Number.isNaN(py)) {
        continue;
      }
      const distance = (px - x) * (px - x) + (py - y) * (py - y);
      if (distance <= bestDistance) {
        bestDistance = distance;
        best = this.#ids[index] ?? null;
      }
    }
    return best;
  }

  /** The grid on the floor, a metre apart, round the rig and at least a small stage. */
  #floor(surface: ViewSurface, lens: Lens, rig: readonly RigFixture[], pixelScale: number): void {
    let minX = -8;
    let maxX = 8;
    let minZ = -6;
    let maxZ = 8;
    for (const fixture of rig) {
      minX = Math.min(minX, fixture.position.x - 4);
      maxX = Math.max(maxX, fixture.position.x + 4);
      minZ = Math.min(minZ, fixture.position.z - 4);
      maxZ = Math.max(maxZ, fixture.position.z + 4);
    }
    const span = Math.max(maxX - minX, maxZ - minZ);
    const step = span <= 60 ? 1 : Math.ceil(span / 60);
    minX = Math.floor(minX / step) * step;
    minZ = Math.floor(minZ / step) * step;
    const width = Math.max(1, pixelScale);
    for (let x = minX; x <= maxX; x += step) {
      this.#segment(surface, lens, v3(x, 0, minZ), v3(x, 0, maxZ), x === 0 ? INK.axis : INK.grid, width);
    }
    for (let z = minZ; z <= maxZ; z += step) {
      this.#segment(surface, lens, v3(minX, 0, z), v3(maxX, 0, z), z === 0 ? INK.axis : INK.grid, width);
    }
  }

  /** A line in space, cut where it passes behind the camera. */
  #segment(surface: ViewSurface, lens: Lens, a: V3, b: V3, colour: string, width: number): void {
    const inA = depthOf(lens, a) >= 0.05;
    const inB = depthOf(lens, b) >= 0.05;
    if (!inA && !inB) {
      return;
    }
    const from = inA ? a : clipToNear(lens, a, b);
    const to = inB ? b : clipToNear(lens, b, a);
    const out = this.#screen;
    if (project(lens, from, out, 0) && project(lens, to, out, 2)) {
      surface.line(out[0] ?? 0, out[1] ?? 0, out[2] ?? 0, out[3] ?? 0, colour, width);
    }
  }

  /** Every beam of one lit fixture. Answers how many were drawn. */
  #beams(surface: ViewSurface, lens: Lens, fixture: RigFixture, look: Look): number {
    const head = multiply(fixture.orientation, yoke(look.pan, look.tilt));
    const rgb = `${String(Math.round(look.red * 255))},${String(Math.round(look.green * 255))},${String(Math.round(look.blue * 255))}`;
    let drawn = 0;
    for (const beam of fixture.beams) {
      const origin = add(fixture.position, turn(head, beam.position));
      const direction = normalize(turn(head, beam.direction));
      if (this.#beam(surface, lens, origin, direction, look.angle ?? beam.angle, look.intensity, rgb)) {
        drawn += 1;
      }
    }
    return drawn;
  }

  /** One cone of light, and its pool where it lands. */
  #beam(
    surface: ViewSurface,
    lens: Lens,
    origin: V3,
    direction: V3,
    angle: number,
    intensity: number,
    rgb: string,
  ): boolean {
    const points = this.#points;
    if (!project(lens, origin, points, 0)) {
      return false;
    }
    const [u, w] = around(direction);
    const spread = Math.tan((Math.min(Math.max(angle, 1), 160) * Math.PI) / 360);
    const ends = this.#ends;
    ends.length = RAYS;
    let landed = true;
    for (let ray = 0; ray < RAYS; ray += 1) {
      const theta = (ray / RAYS) * Math.PI * 2;
      const edge = normalize(
        add(direction, scale(add(scale(u, Math.cos(theta)), scale(w, Math.sin(theta))), spread)),
      );
      let reach = BEAM_REACH;
      if (edge.y < -1e-3 && origin.y > 0) {
        const toFloor = -origin.y / edge.y;
        if (toFloor <= FLOOR_REACH) {
          reach = toFloor;
        } else {
          landed = false;
        }
      } else {
        landed = false;
      }
      ends[ray] = add(origin, scale(edge, reach));
    }

    // The cone: the lens and the far ends, as the hull of what they project to.
    let count = 1;
    let sumX = 0;
    let sumY = 0;
    let inFront = true;
    for (const end of ends) {
      const visible = depthOf(lens, end) >= 0.05 ? end : clipToNear(lens, end, origin);
      if (visible !== end) {
        inFront = false;
      }
      if (project(lens, visible, points, count * 2)) {
        sumX += points[count * 2] ?? 0;
        sumY += points[count * 2 + 1] ?? 0;
        count += 1;
      }
    }
    if (count < 3) {
      return false;
    }
    const hull = convexHull(points, count, this.#hull, this.#order);
    const toX = sumX / (count - 1);
    const toY = sumY / (count - 1);
    surface.beam(
      this.#hull,
      hull,
      points[0] ?? 0,
      points[1] ?? 0,
      toX,
      toY,
      rgb,
      0.42 * intensity,
      (landed ? 0.1 : 0.02) * intensity,
    );

    // The pool, where every ray reached the floor in front of the camera.
    if (landed && inFront) {
      for (let ray = 0; ray < RAYS; ray += 1) {
        const end = ends[ray];
        if (end === undefined || !project(lens, end, points, ray * 2)) {
          return true;
        }
      }
      surface.polygon(points, RAYS, `rgb(${rgb})`, 0.3 * intensity, "add");
    }
    return true;
  }

  /** One fixture's body, and its lens. */
  #body(surface: ViewSurface, lens: Lens, fixture: RigFixture, look: Look, selected: boolean): void {
    const half = scale(fixture.size, 0.5);
    const corners: V3[] = [];
    for (let bit = 0; bit < 8; bit += 1) {
      const local = v3(bit & 1 ? half.x : -half.x, bit & 2 ? half.y : -half.y, bit & 4 ? half.z : -half.z);
      const corner = add(fixture.position, turn(fixture.orientation, local));
      if (depthOf(lens, corner) < 0.05) {
        // A body the camera is inside, or behind: nothing sensible to draw.
        return;
      }
      corners.push(corner);
    }
    const face = this.#points;
    for (const [indices, normal] of FACES) {
      const outward = turn(fixture.orientation, normal);
      const centre = add(fixture.position, turn(fixture.orientation, v3(normal.x * half.x, normal.y * half.y, normal.z * half.z)));
      if (dot(outward, sub(centre, lens.eye)) >= 0) {
        continue;
      }
      let ok = true;
      for (let corner = 0; corner < 4; corner += 1) {
        const point = corners[indices[corner] ?? 0];
        if (point === undefined || !project(lens, point, face, corner * 2)) {
          ok = false;
        }
      }
      if (!ok) {
        continue;
      }
      const shade = 0.4 + 0.55 * Math.max(0, dot(outward, LIGHT));
      const [r, g, b] = INK.body;
      surface.polygon(
        face,
        4,
        `rgb(${String(Math.round(r * shade))},${String(Math.round(g * shade))},${String(Math.round(b * shade))})`,
        1,
        "over",
        selected ? INK.selected : INK.outline,
      );
    }

    // The lens: a disc where each beam leaves, facing the way it points, in
    // the colour on the cable — drawn only where the camera can see its face.
    const head = multiply(fixture.orientation, yoke(look.pan, look.tilt));
    for (const beam of fixture.beams) {
      const origin = add(fixture.position, turn(head, beam.position));
      const direction = normalize(turn(head, beam.direction));
      if (dot(direction, sub(lens.eye, origin)) <= 0) {
        continue;
      }
      const [u, w] = around(direction);
      const radius = Math.min(fixture.size.x, fixture.size.z) * 0.32;
      let ok = true;
      for (let step = 0; step < 8; step += 1) {
        const theta = (step / 8) * Math.PI * 2;
        const rim = add(origin, add(scale(u, Math.cos(theta) * radius), scale(w, Math.sin(theta) * radius)));
        if (!project(lens, rim, face, step * 2)) {
          ok = false;
        }
      }
      if (!ok) {
        continue;
      }
      if (look.intensity >= LIT) {
        surface.polygon(
          face,
          8,
          `rgb(${String(Math.round(look.red * 255))},${String(Math.round(look.green * 255))},${String(Math.round(look.blue * 255))})`,
          Math.min(1, 0.35 + look.intensity),
          "add",
        );
      } else {
        surface.polygon(face, 8, INK.lens, 1, "over");
      }
    }
  }
}

/**
 * The convex hull of `count` points in `points` (`x, y` pairs), written into
 * `out` in order, answering how many it has. Andrew's monotone chain; `order`
 * is scratch space for the sort.
 */
export function convexHull(
  points: ArrayLike<number>,
  count: number,
  out: Float64Array,
  order: Int32Array,
): number {
  for (let index = 0; index < count; index += 1) {
    order[index] = index;
  }
  // Insertion sort by x then y: seventeen points, and no allocation.
  for (let index = 1; index < count; index += 1) {
    const held = order[index] ?? 0;
    const hx = points[held * 2] ?? 0;
    const hy = points[held * 2 + 1] ?? 0;
    let at = index - 1;
    while (at >= 0) {
      const other = order[at] ?? 0;
      const ox = points[other * 2] ?? 0;
      const oy = points[other * 2 + 1] ?? 0;
      if (ox < hx || (ox === hx && oy <= hy)) {
        break;
      }
      order[at + 1] = other;
      at -= 1;
    }
    order[at + 1] = held;
  }
  const x = (slot: number): number => points[(order[slot] ?? 0) * 2] ?? 0;
  const y = (slot: number): number => points[(order[slot] ?? 0) * 2 + 1] ?? 0;
  const turnOf = (ax: number, ay: number, bx: number, by: number, cx: number, cy: number): number =>
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);

  let size = 0;
  const push = (slot: number): void => {
    out[size * 2] = x(slot);
    out[size * 2 + 1] = y(slot);
    size += 1;
  };
  const lastTurn = (slot: number): number =>
    turnOf(out[(size - 2) * 2] ?? 0, out[(size - 2) * 2 + 1] ?? 0, out[(size - 1) * 2] ?? 0, out[(size - 1) * 2 + 1] ?? 0, x(slot), y(slot));

  for (let slot = 0; slot < count; slot += 1) {
    while (size >= 2 && lastTurn(slot) <= 0) {
      size -= 1;
    }
    push(slot);
  }
  const lower = size + 1;
  for (let slot = count - 2; slot >= 0; slot -= 1) {
    while (size >= lower && lastTurn(slot) <= 0) {
      size -= 1;
    }
    push(slot);
  }
  // The last point is the first again.
  return Math.max(0, size - 1);
}
