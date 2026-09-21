/**
 * The rig, as the 3D viewer needs it: where each fixture hangs, how big it is,
 * where its beams leave it, and which channels decide what those beams do.
 *
 * Read out of the **show document**, like every other window in this
 * interface: the patch and the embedded profiles are control state, they
 * arrive as `ShowPatch` deltas, and a change to them is a React render — rare,
 * and the right place to rebuild this list. What changes thirty times a second
 * is the cable, and that is `./look.ts`'s and never React's.
 *
 * # What a profile without a body gets
 *
 * `FixtureType::physical` is `None` for an Open Fixture Library profile, for
 * the four generics and for every show embedded before S61. S61 wrote down what
 * that means, and it is what this does: **a box with one beam out of the
 * bottom**, sized like a small lantern, at the angle a PAR throws. It is not an
 * error and nothing says it is.
 */

import type { JsonValue } from "../bindings";
import { isArray, isObject } from "../mirror/patch";
import { pointerToken, valueAt } from "../mirror/select";
import type { Mat3, V3 } from "./space";
import { DOWN, ORIGIN, normalize, orientation, v3 } from "./space";

/** The body a fixture with no physical description is drawn with, in metres. */
export const DEFAULT_SIZE: V3 = { x: 0.3, y: 0.3, z: 0.3 };

/** The angle a fixture throws when nothing says otherwise — a PAR's, roughly. */
export const DEFAULT_BEAM_ANGLE = 25;

/** One beam of a fixture, in the fixture's own frame. */
export interface RigBeam {
  /** Where it leaves the body, relative to the fixture's origin, in metres. */
  readonly position: V3;
  /** Which way it points with pan and tilt at nought, as a unit vector. */
  readonly direction: V3;
  /** Its full angle, in degrees, before any zoom. */
  readonly angle: number;
}

/** One channel of a fixture, as the viewer reads it off the cable. */
export interface RigChannel {
  /** Offset of the coarse byte within the fixture's footprint, from nought. */
  readonly coarse: number;
  /** Offset of the fine byte, or `null` for an 8-bit channel. */
  readonly fine: number | null;
  /** Whether the profile says the device reads this channel backwards. */
  readonly invert: boolean;
  /** The physical value at nought. */
  readonly from: number;
  /** The physical value at full. */
  readonly to: number;
  /** The channel's named ranges, lowest first. */
  readonly ranges: readonly RigRange[];
}

/** One named range of a channel. */
export interface RigRange {
  /** The manufacturer's name for it. */
  readonly name: string;
  /** Lowest value, `0..=65535`. */
  readonly from: number;
  /** Highest value, `0..=65535`. */
  readonly to: number;
}

/**
 * The channels that change what a beam looks like, by this desk's attribute
 * names — the first of each kind. A second colour wheel changes a beam as well,
 * and drawing it is a refinement for the day a viewer draws gobos.
 */
export type RigChannels = {
  readonly [K in LookAttribute]?: RigChannel;
};

/** The attributes the viewer reads. */
export const LOOK_ATTRIBUTES = [
  "Dimmer",
  "Pan",
  "Tilt",
  "Zoom",
  "Shutter",
  "ColorWheel",
  "Red",
  "Green",
  "Blue",
  "White",
  "Amber",
  "WarmWhite",
  "ColdWhite",
  "Lime",
  "Uv",
  "Indigo",
  "Cyan",
  "Magenta",
  "Yellow",
] as const;

/** One of {@link LOOK_ATTRIBUTES}. */
export type LookAttribute = (typeof LOOK_ATTRIBUTES)[number];

/** One patched fixture, ready to draw. */
export interface RigFixture {
  /** The fixture number. */
  readonly id: number;
  /** What it is called. */
  readonly name: string;
  /** Its universe. */
  readonly universe: number;
  /** Its start address, `1..=512`. */
  readonly address: number;
  /** Where it hangs, in metres. */
  readonly position: V3;
  /** Which way it faces, in degrees — `prism_domain::placement`. */
  readonly rotation: V3;
  /** {@link rotation} as a matrix, worked out once per render rather than per frame. */
  readonly orientation: Mat3;
  /** Whether it is still where a patch leaves it: at the origin, turned by nothing. */
  readonly unplaced: boolean;
  /** Width, height and depth of its body, in metres. */
  readonly size: V3;
  /** Every beam it has — at least one. */
  readonly beams: readonly RigBeam[];
  /** The channels the viewer reads. */
  readonly channels: RigChannels;
  /** Whether its profile came with a physical description (S61). */
  readonly described: boolean;
}

/** A number, or `fallback` for anything that is not a finite one. */
function finite(value: JsonValue | null, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/** A vector out of a `{ x, y, z }` object, or `fallback`. */
function vectorOf(value: JsonValue | null, fallback: V3): V3 {
  if (value === null || !isObject(value)) {
    return fallback;
  }
  return v3(
    finite(value.x ?? null, fallback.x),
    finite(value.y ?? null, fallback.y),
    finite(value.z ?? null, fallback.z),
  );
}

/** The named ranges of one attribute definition. */
function rangesOf(value: JsonValue | undefined): readonly RigRange[] {
  if (value === undefined || !isArray(value)) {
    return [];
  }
  const ranges: RigRange[] = [];
  for (const range of value) {
    if (!isObject(range)) {
      continue;
    }
    const name = range.name;
    ranges.push({
      name: typeof name === "string" ? name : "",
      from: finite(range.from ?? null, 0),
      to: finite(range.to ?? null, 0),
    });
  }
  return ranges;
}

/** The channels of one profile the viewer reads, first of each kind. */
function channelsOf(attributes: JsonValue | null): RigChannels {
  const channels: { [K in LookAttribute]?: RigChannel } = {};
  if (attributes === null || !isArray(attributes)) {
    return channels;
  }
  const wanted = new Set<string>(LOOK_ATTRIBUTES);
  for (const def of attributes) {
    if (!isObject(def)) {
      continue;
    }
    const attribute = def.attribute;
    if (typeof attribute !== "string" || !wanted.has(attribute)) {
      continue;
    }
    // The first of its kind: a definition that states no occurrence is the
    // first (S52's `#[serde(default)]`), and a second one is skipped.
    if (finite(def.occurrence ?? null, 0) !== 0) {
      continue;
    }
    const key = attribute as LookAttribute;
    if (channels[key] !== undefined) {
      continue;
    }
    const fine = def.fineOffset;
    channels[key] = {
      coarse: finite(def.coarseOffset ?? null, 0),
      fine: typeof fine === "number" && Number.isFinite(fine) ? fine : null,
      invert: def.invert === true,
      from: finite(def.physicalFrom ?? null, 0),
      to: finite(def.physicalTo ?? null, 100),
      ranges: rangesOf(def.ranges),
    };
  }
  return channels;
}

/** The body and beams a profile describes, or the default box. */
function bodyOf(physical: JsonValue | null): Pick<RigFixture, "size" | "beams" | "described"> {
  if (physical === null || !isObject(physical)) {
    return {
      size: DEFAULT_SIZE,
      beams: [
        {
          position: v3(0, -DEFAULT_SIZE.y / 2, 0),
          direction: DOWN,
          angle: DEFAULT_BEAM_ANGLE,
        },
      ],
      described: false,
    };
  }
  const stated = vectorOf(physical.size ?? null, ORIGIN);
  // A size of nought is *the file gave no model for the body* (S61), which is
  // a fixture the viewer sizes for itself rather than one of no size.
  const size =
    stated.x > 0 && stated.y > 0 && stated.z > 0 ? stated : DEFAULT_SIZE;
  const beams: RigBeam[] = [];
  const listed = physical.beams;
  if (listed !== undefined && isArray(listed)) {
    for (const beam of listed) {
      if (!isObject(beam)) {
        continue;
      }
      const angle = finite(beam.beamAngle ?? null, 0);
      beams.push({
        position: vectorOf(beam.position ?? null, ORIGIN),
        direction: normalize(vectorOf(beam.direction ?? null, DOWN)),
        angle: angle > 0 ? angle : DEFAULT_BEAM_ANGLE,
      });
    }
  }
  if (beams.length === 0) {
    beams.push({ position: v3(0, -size.y / 2, 0), direction: DOWN, angle: DEFAULT_BEAM_ANGLE });
  }
  return { size, beams, described: true };
}

/**
 * Every patched fixture, in number order, ready to draw.
 *
 * Profiles are read once each however many fixtures share them, so a rig of
 * two hundred of one head is one profile's work.
 */
export function rigOf(show: JsonValue | null): readonly RigFixture[] {
  const fixtures = valueAt(show, "/fixtures");
  if (fixtures === null || !isObject(fixtures)) {
    return [];
  }
  const profiles = new Map<string, Pick<RigFixture, "size" | "beams" | "described" | "channels">>();
  const rig: RigFixture[] = [];
  for (const [key, entry] of Object.entries(fixtures)) {
    const id = Number(key);
    if (!Number.isInteger(id) || !isObject(entry)) {
      continue;
    }
    const typeId = typeof entry.typeId === "string" ? entry.typeId : "";
    let profile = profiles.get(typeId);
    if (profile === undefined) {
      const base = `/fixtureTypes/${pointerToken(typeId)}`;
      profile = {
        ...bodyOf(valueAt(show, `${base}/physical`)),
        channels: channelsOf(valueAt(show, `${base}/attributes`)),
      };
      profiles.set(typeId, profile);
    }
    const position = vectorOf(entry.position ?? null, ORIGIN);
    const rotation = vectorOf(entry.rotation ?? null, ORIGIN);
    rig.push({
      id,
      name: typeof entry.name === "string" ? entry.name : "",
      universe: finite(entry.universe ?? null, 0),
      address: finite(entry.address ?? null, 0),
      position,
      rotation,
      orientation: orientation(rotation),
      unplaced: isNought(position) && isNought(rotation),
      ...profile,
    });
  }
  return rig.sort((left, right) => left.id - right.id);
}

/** Whether every component is nought. */
function isNought(value: V3): boolean {
  return value.x === 0 && value.y === 0 && value.z === 0;
}

