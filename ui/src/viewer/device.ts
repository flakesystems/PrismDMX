/**
 * A GDTF device as the visualiser reads it — **S30b**: the geometry tree, every
 * function of every channel, and every wheel, out of `FixtureType::physical`
 * (`prism_domain::device`).
 *
 * Read out of the show document like the rest of the rig: a profile is embedded
 * in the show, so this is control state and is rebuilt on a render, never per
 * frame. A profile that carries none of it — an Open Fixture Library profile,
 * a generic, a show from before S30b — reads as `null`, and the viewer draws
 * it from its attribute list instead (`./state.ts`).
 */

import type { JsonValue } from "../bindings";
import { isArray, isObject } from "../mirror/patch";
import type { V3 } from "./space";
import { ORIGIN, v3 } from "./space";

/** A beam geometry's optics. */
export interface DeviceBeam {
  readonly type: string;
  /** Full angle at half intensity, degrees. */
  readonly beamAngle: number;
  /** Full angle at a tenth, degrees. */
  readonly fieldAngle: number;
  /** Lens radius, metres. */
  readonly radius: number;
  readonly flux: number;
  readonly kelvin: number;
  /** Width over height; 1 for round. */
  readonly ratio: number;
}

/** One node of the geometry tree, its transform relative to its parent in show axes. */
export interface DeviceNode {
  readonly index: number;
  readonly name: string;
  readonly parent: number | null;
  readonly kind: string;
  readonly model: string | null;
  readonly primitive: string | null;
  /** Across, height, depth — show axes, metres. */
  readonly size: V3;
  readonly position: V3;
  readonly xAxis: V3;
  readonly yAxis: V3;
  readonly zAxis: V3;
  readonly beam: DeviceBeam | null;
}

/** A channel set inside a function. */
export interface DeviceSet {
  readonly name: string;
  readonly from: number;
  readonly to: number;
  /** One-based wheel slot. */
  readonly slot: number | null;
}

/** One function of a channel. */
export interface DeviceFunction {
  readonly attribute: string;
  readonly from: number;
  readonly to: number;
  readonly physicalFrom: number;
  readonly physicalTo: number;
  readonly wheel: string | null;
  readonly sets: readonly DeviceSet[];
  readonly modeMaster: string | null;
  readonly modeFrom: number;
  readonly modeTo: number;
}

/** One patched channel. */
export interface DeviceChannel {
  /** Coarse byte, from nought. */
  readonly offset: number;
  readonly fine: number | null;
  readonly geometry: string | null;
  readonly attribute: string;
  readonly functions: readonly DeviceFunction[];
}

/** A prism facet's push. */
export interface DeviceFacet {
  readonly x: number;
  readonly y: number;
}

/** One slot of a wheel. */
export interface DeviceSlot {
  readonly name: string;
  /** sRGB `0..=255`, or `null` for white. */
  readonly color: readonly [number, number, number] | null;
  readonly transmission: number;
  readonly media: string | null;
  readonly facets: readonly DeviceFacet[];
}

/** One wheel. */
export interface DeviceWheel {
  readonly name: string;
  readonly slots: readonly DeviceSlot[];
}

/** Everything the visualiser knows about a GDTF device. */
export interface Device {
  /** The GDTF GUID; empty when the file stated none. */
  readonly guid: string;
  readonly nodes: readonly DeviceNode[];
  readonly channels: readonly DeviceChannel[];
  readonly wheels: ReadonlyMap<string, DeviceWheel>;
}

function num(value: JsonValue | undefined, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function str(value: JsonValue | undefined): string | null {
  return typeof value === "string" && value !== "" ? value : null;
}

function vec(value: JsonValue | undefined, fallback: V3): V3 {
  if (value === undefined || value === null || !isObject(value)) {
    return fallback;
  }
  return v3(num(value.x, fallback.x), num(value.y, fallback.y), num(value.z, fallback.z));
}

function list(value: JsonValue | undefined): readonly JsonValue[] {
  return value !== undefined && isArray(value) ? value : [];
}

function readBeam(value: JsonValue | undefined): DeviceBeam | null {
  if (value === undefined || value === null || !isObject(value)) {
    return null;
  }
  return {
    type: str(value.beamType) ?? "Wash",
    beamAngle: num(value.beamAngle, 0),
    fieldAngle: num(value.fieldAngle, 0),
    radius: num(value.beamRadius, 0),
    flux: num(value.luminousFlux, 0),
    kelvin: num(value.colorTemperature, 0),
    ratio: num(value.rectangleRatio, 1),
  };
}

function readNodes(value: JsonValue | undefined): DeviceNode[] {
  const nodes: DeviceNode[] = [];
  for (const entry of list(value)) {
    if (!isObject(entry)) {
      continue;
    }
    const index = nodes.length;
    const parent = entry.parent;
    nodes.push({
      index,
      name: str(entry.name) ?? `Geometry ${String(index + 1)}`,
      // A parent that is not an earlier node is no parent: a tree is read in
      // document order and a forward reference would be a loop.
      parent: typeof parent === "number" && parent >= 0 && parent < index ? parent : null,
      kind: str(entry.kind) ?? "Geometry",
      model: str(entry.model),
      primitive: str(entry.primitive),
      size: vec(entry.size, ORIGIN),
      position: vec(entry.position, ORIGIN),
      xAxis: vec(entry.xAxis, v3(1, 0, 0)),
      yAxis: vec(entry.yAxis, v3(0, 1, 0)),
      zAxis: vec(entry.zAxis, v3(0, 0, 1)),
      beam: readBeam(entry.beam),
    });
  }
  return nodes;
}

function readFunctions(value: JsonValue | undefined): DeviceFunction[] {
  const functions: DeviceFunction[] = [];
  for (const entry of list(value)) {
    if (!isObject(entry)) {
      continue;
    }
    const sets: DeviceSet[] = [];
    for (const set of list(entry.sets)) {
      if (!isObject(set)) {
        continue;
      }
      sets.push({
        name: str(set.name) ?? "",
        from: num(set.from, 0),
        to: num(set.to, 65535),
        slot: typeof set.slot === "number" ? set.slot : null,
      });
    }
    functions.push({
      attribute: str(entry.attribute) ?? "",
      from: num(entry.from, 0),
      to: num(entry.to, 65535),
      physicalFrom: num(entry.physicalFrom, 0),
      physicalTo: num(entry.physicalTo, 1),
      wheel: str(entry.wheel),
      sets,
      modeMaster: str(entry.modeMaster),
      modeFrom: num(entry.modeFrom, 0),
      modeTo: num(entry.modeTo, 65535),
    });
  }
  return functions;
}

function readChannels(value: JsonValue | undefined): DeviceChannel[] {
  const channels: DeviceChannel[] = [];
  for (const entry of list(value)) {
    if (!isObject(entry)) {
      continue;
    }
    channels.push({
      offset: num(entry.offset, 0),
      fine: typeof entry.fine === "number" ? entry.fine : null,
      geometry: str(entry.geometry),
      attribute: str(entry.attribute) ?? "",
      functions: readFunctions(entry.functions),
    });
  }
  return channels;
}

function readColor(value: JsonValue | undefined): readonly [number, number, number] | null {
  if (value === undefined || value === null || !isObject(value)) {
    return null;
  }
  return [num(value.r, 255), num(value.g, 255), num(value.b, 255)];
}

function readWheels(value: JsonValue | undefined): Map<string, DeviceWheel> {
  const wheels = new Map<string, DeviceWheel>();
  for (const entry of list(value)) {
    if (!isObject(entry)) {
      continue;
    }
    const name = str(entry.name);
    if (name === null) {
      continue;
    }
    const slots: DeviceSlot[] = [];
    for (const slot of list(entry.slots)) {
      if (!isObject(slot)) {
        continue;
      }
      const facets: DeviceFacet[] = [];
      for (const facet of list(slot.facets)) {
        if (isObject(facet)) {
          facets.push({ x: num(facet.x, 0), y: num(facet.y, 0) });
        }
      }
      slots.push({
        name: str(slot.name) ?? "",
        color: readColor(slot.color),
        transmission: num(slot.transmission, 1),
        media: str(slot.media),
        facets,
      });
    }
    wheels.set(name, { name, slots });
  }
  return wheels;
}

/**
 * The device a profile's `physical` describes, or `null` for a profile that
 * has no geometry tree — the viewer draws that one from its attribute list.
 */
export function deviceOf(physical: JsonValue | null): Device | null {
  if (physical === null || !isObject(physical)) {
    return null;
  }
  const nodes = readNodes(physical.geometries);
  if (nodes.length === 0) {
    return null;
  }
  return {
    guid: str(physical.fixtureTypeId) ?? "",
    nodes,
    channels: readChannels(physical.channels),
    wheels: readWheels(physical.wheels),
  };
}

/** Whether node `index` is `ancestor` or hangs somewhere under it. */
export function isUnder(nodes: readonly DeviceNode[], index: number, ancestor: number): boolean {
  let at: number | null = index;
  let guard = 0;
  while (at !== null && guard < 64) {
    if (at === ancestor) {
      return true;
    }
    at = nodes[at]?.parent ?? null;
    guard += 1;
  }
  return false;
}
