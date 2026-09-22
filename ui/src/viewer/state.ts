/**
 * What every beam and every moving axis of a fixture is doing, read off the
 * **cable** — **S30b**, and the part of the visualiser the owner found missing:
 * focus, strobe, blades, gobos, prisms, frost, iris, every colour.
 *
 * # Two readers, one answer
 *
 * - A **GDTF device** is read function by function (`./device.ts`): a channel
 *   is several functions over its DMX range, each its own GDTF attribute with
 *   its own physical range — *Shutter1* closed or open, *Shutter1Strobe* at
 *   0.3 to 20 Hz, *Gobo1Pos* an index while *Gobo1* stands in one range and a
 *   rotation in another (`modeMaster`). The function in force is found, its
 *   physical value interpolated, and the attribute's family decides what it
 *   does to the beam. Which beam: a channel acts on the geometry it names and
 *   everything under it, so a pixel bar's cells are lit separately.
 * - An **Open Fixture Library or generic profile** has no functions, only this
 *   desk's attribute types and named ranges. It is read from those: a range
 *   called *strobe* strobes, one called *closed* is dark, a gobo range with no
 *   picture draws a pattern so an operator can see one is in.
 *
 * Both produce the same {@link BeamState}, which is all the renderer reads.
 * Nothing here is React and nothing here allocates per frame beyond the state
 * objects it is handed to fill.
 */

import type { TelemetryFrameView } from "../telemetry/frame";
import type { Device, DeviceChannel, DeviceFunction, DeviceNode } from "./device";
import { isUnder } from "./device";
import type { Look } from "./look";
import { channelValue, darkLook, kelvinColour, linear, readLook, universeIndex } from "./look";
import type { RigChannel, RigFixture } from "./rig";

export { kelvinColour, linear } from "./look";

/** How a beam flashes. */
export type StrobeKind = "none" | "strobe" | "pulseOpen" | "pulseClose" | "random";

/** One wheel image in the beam — a gobo, an animation wheel. */
export interface ImageState {
  /** Which wheel — `Gobo1`, `AnimationWheel1` — so its index and rotate channels find it. */
  wheel: string;
  /** The picture's name in the fixture's archive, or a built-in pattern `"@n"`. */
  media: string;
  /** Degrees, where it stands. */
  rotation: number;
  /** Degrees a second it turns — an index channel's rotate function. */
  spin: number;
}

/** A framing blade. */
export interface BladeState {
  /** How far each end is in, `0..=1` of the beam's radius. */
  a: number;
  b: number;
  /** Degrees it is turned. */
  rotation: number;
}

/** What one beam is doing. */
export interface BeamState {
  /** `0..=1`, dimmer and shutter together. */
  intensity: number;
  strobe: StrobeKind;
  /** Flashes a second. */
  hz: number;
  /** Linear-light colour, largest component 1. */
  color: [number, number, number];
  /** Full beam angle, degrees; `null` for the geometry's own. */
  angle: number | null;
  /** `0..=1` of the beam's radius the iris leaves open. */
  iris: number;
  /** `0..=1`: 0 sharp, 1 as soft as the lens goes. */
  blur: number;
  /** `0..=1`. */
  frost: number;
  gobos: ImageState[];
  prism: { facets: readonly { x: number; y: number }[]; rotation: number; spin: number } | null;
  blades: BladeState[];
  /** Degrees the whole shaper is turned. */
  shaper: number;
}

/** What one fixture is doing: its beams, and every axis's pan and tilt. */
export interface FixtureState {
  /** Whether its universe is on the cable. */
  present: boolean;
  /** By geometry index: degrees about its Z (pan) and X (tilt) axes. */
  axes: Map<number, { pan: number; tilt: number }>;
  /** By geometry index of each beam. */
  beams: Map<number, BeamState>;
  /** For a fixture with no device, the one beam. */
  single: BeamState | null;
  /** Its pan and tilt, for a fixture with no device. */
  pan: number;
  tilt: number;
}

/** A fresh, dark beam. */
export function darkBeam(): BeamState {
  return {
    intensity: 0,
    strobe: "none",
    hz: 0,
    color: [1, 1, 1],
    angle: null,
    iris: 1,
    blur: 0,
    frost: 0,
    gobos: [],
    prism: null,
    blades: [],
    shaper: 0,
  };
}

/** A fresh state for a fixture. */
export function emptyState(): FixtureState {
  return { present: false, axes: new Map(), beams: new Map(), single: null, pan: 0, tilt: 0 };
}

/** What each additive emitter adds, in linear red, green and blue — by GDTF name. */
const EMITTER_COLOURS: Readonly<Record<string, readonly [number, number, number]>> = {
  R: [1, 0, 0],
  G: [0, 1, 0],
  B: [0, 0, 1],
  W: [1, 1, 1],
  WW: [1, 0.72, 0.42],
  CW: [0.78, 0.88, 1],
  A: [1, 0.55, 0],
  UV: [0.28, 0, 0.75],
  C: [0, 1, 1],
  M: [1, 0, 1],
  Y: [1, 1, 0],
  RY: [1, 0.55, 0],
  GY: [0.6, 1, 0],
  BM: [0.35, 0, 1],
  RM: [1, 0, 0.5],
  GC: [0, 1, 0.5],
  BC: [0, 0.5, 1],
};

/** A GDTF attribute's family and its number: `Gobo2PosRotate` is `["GobonPosRotate", 2]`. */
export function family(attribute: string): [string, number] {
  let number = 0;
  let seen = false;
  const name = attribute.replace(/\d+/g, (digits) => {
    if (!seen) {
      number = Number(digits);
      seen = true;
    }
    return "n";
  });
  return [name, number];
}

/** The function of `channel` in force at `value`, given the masters' values. */
function functionAt(
  channel: DeviceChannel,
  value: number,
  masters: ReadonlyMap<string, number>,
): DeviceFunction | null {
  let found: DeviceFunction | null = null;
  for (const candidate of channel.functions) {
    if (value < candidate.from || value > candidate.to) {
      continue;
    }
    if (candidate.modeMaster !== null) {
      const master = masters.get(candidate.modeMaster);
      if (master === undefined || master < candidate.modeFrom || master > candidate.modeTo) {
        continue;
      }
    }
    found = candidate;
  }
  return found;
}

/** The physical value a function stands at. */
function physicalOf(fn: DeviceFunction, value: number): number {
  const span = fn.to - fn.from;
  const fraction = span <= 0 ? 0 : (value - fn.from) / span;
  return fn.physicalFrom + (fn.physicalTo - fn.physicalFrom) * fraction;
}

/** Where a value stands in a function's DMX range, `0..=1`. */
function fractionOf(fn: DeviceFunction, value: number): number {
  const span = fn.to - fn.from;
  return span <= 0 ? 0 : Math.min(1, Math.max(0, (value - fn.from) / span));
}

/** The name of the set `value` stands in, or `""`. */
function setNameAt(fn: DeviceFunction, value: number): string {
  for (const set of fn.sets) {
    if (value >= set.from && value <= set.to) {
      return set.name;
    }
  }
  return "";
}

/** The wheel slot a function's set selects at `value`, one-based, or `null`. */
function slotAt(fn: DeviceFunction, value: number): number | null {
  for (const set of fn.sets) {
    if (value >= set.from && value <= set.to) {
      return set.slot;
    }
  }
  return null;
}

/**
 * How far into its effect a value is, `0..=1`, read from the set it stands in
 * where the set says so.
 *
 * A Robin T1 puts three frosts on one channel, each a band of sets: *Open*,
 * *Light Frost from 0% to 100%*, *100% Light frost*, then pulses and ramps.
 * The function's own fraction is how far into all of that the value is, which
 * put 70 % on the channel at a frost of a tenth. So an *Open* set is none, a
 * *100%* set is all, a *from 0% to 100%* set is how far into that set, and a
 * pulse or a ramp — which moves on its own — is drawn half in. A function
 * whose sets say none of that is its fraction, as before.
 */
export function bandOf(fn: DeviceFunction, value: number): number {
  for (const set of fn.sets) {
    if (value < set.from || value > set.to) {
      continue;
    }
    if (/\bopen\b|no func/i.test(set.name)) {
      return 0;
    }
    if (/from\s*0\s*%|0\s*%\s*(to|-)/i.test(set.name)) {
      return set.to > set.from ? (value - set.from) / (set.to - set.from) : 1;
    }
    if (/100\s*%/.test(set.name)) {
      return 1;
    }
    if (/pulse|ramp|strob/i.test(set.name)) {
      return 0.5;
    }
    break;
  }
  return fractionOf(fn, value);
}

/** The steady turn of a rotation whose profile states no speed, degrees a second. */
const UNSTATED_SPEED = 45;

/**
 * How fast a rotate function turns at `value`, degrees a second.
 *
 * GDTF states a rotation's speed as its physical range, in degrees a second —
 * a Robin T1's gobo wheel runs from −758 to 758. The same file leaves its
 * animation wheel and its prism at the default `0..1`, and a prism at `1..−1`,
 * which is no speed at all: taken as one, the animation wheel turned half a
 * degree a second while it stood still. So a set named *No rotation* or
 * *Stop* (the T1's centre is *No rotation middle*) is still, whatever the
 * range says; a range that states a speed is that speed; and any other turns
 * steadily, in the direction of the half of the range the value is in.
 */
export function speedOf(fn: DeviceFunction, value: number): number {
  if (/no rot|stop|no func|index/i.test(setNameAt(fn, value))) {
    return 0;
  }
  const physical = physicalOf(fn, value);
  if (Math.max(Math.abs(fn.physicalFrom), Math.abs(fn.physicalTo)) > 1.0001) {
    return physical;
  }
  return fractionOf(fn, value) < 0.5 ? -UNSTATED_SPEED : UNSTATED_SPEED;
}

/** The value of every channel of a device on the cable, by offset. */
function channelValues(
  fixture: RigFixture,
  device: Device,
  frame: TelemetryFrameView,
  index: number,
): { values: Map<DeviceChannel, number>; masters: Map<string, number> } {
  const values = new Map<DeviceChannel, number>();
  const masters = new Map<string, number>();
  for (const channel of device.channels) {
    const base = fixture.address - 1;
    const coarse = frame.levelAt(index, base + channel.offset);
    const value =
      channel.fine === null ? coarse * 257 : coarse * 256 + frame.levelAt(index, base + channel.fine);
    values.set(channel, value);
    // A mode master is named `<geometry>_<attribute>`, which is GDTF's
    // default name for a channel.
    masters.set(`${channel.geometry ?? ""}_${channel.attribute}`, value);
  }
  return { values, masters };
}

/** The wheel an index or rotate function belongs to: `GobonPos`, 2 → `Gobo2`. */
function wheelKey(name: string, number: number): string {
  const base = name.startsWith("AnimationWheel") ? "AnimationWheel" : "Gobo";
  return `${base}${String(number)}`;
}

/** Applies one GDTF function at one value to a beam. */
function applyFunction(
  beam: BeamState,
  fn: DeviceFunction,
  value: number,
  device: Device,
  mix: { emitters: [number, number, number]; any: boolean; dimmer: number | null; kelvin: number | null; sub: [number, number, number]; filter: [number, number, number]; transmission: number },
): void {
  const [name, number] = family(fn.attribute);
  const physical = physicalOf(fn, value);
  const fraction = fractionOf(fn, value);
  if (name === "Dimmer") {
    mix.dimmer = fraction;
    return;
  }
  if (name.startsWith("Shuttern")) {
    const kind = name.slice("Shuttern".length);
    if (kind === "") {
      // Open/closed: GDTF writes 1 for open and 0 for closed.
      // The set's own name first — *Closed*, *Open* — and GDTF's 0 for
      // closed and 1 for open where the sets say nothing.
      const said = setNameAt(fn, value);
      if (/clos|blackout/i.test(said) || (!/open/i.test(said) && physical < 0.5)) {
        beam.intensity = -1;
      }
      return;
    }
    beam.hz = Math.max(0.1, physical);
    beam.strobe = kind.includes("PulseOpen")
      ? "pulseOpen"
      : kind.includes("PulseClose")
        ? "pulseClose"
        : kind.includes("Random")
          ? "random"
          : "strobe";
    return;
  }
  if (name.startsWith("ColorAdd_")) {
    const colour = EMITTER_COLOURS[name.slice("ColorAdd_".length)];
    if (colour !== undefined) {
      mix.any = true;
      mix.emitters[0] += fraction * colour[0];
      mix.emitters[1] += fraction * colour[1];
      mix.emitters[2] += fraction * colour[2];
    }
    return;
  }
  if (name.startsWith("ColorSub_")) {
    const which = name.slice("ColorSub_".length);
    if (which === "C") mix.sub[0] = fraction;
    if (which === "M") mix.sub[1] = fraction;
    if (which === "Y") mix.sub[2] = fraction;
    return;
  }
  if (name === "CTO" || name === "CTC" || name === "CTB" || name === "ColorTemperature") {
    if (physical >= 1000) {
      mix.kelvin = physical;
    }
    return;
  }
  if ((name === "Colorn" || name === "ColorMacron") && fn.wheel !== null) {
    const slot = slotAt(fn, value);
    const found = slot === null ? undefined : device.wheels.get(fn.wheel)?.slots[slot - 1];
    if (found !== undefined && found.color !== null) {
      mix.filter[0] *= linear(found.color[0]);
      mix.filter[1] *= linear(found.color[1]);
      mix.filter[2] *= linear(found.color[2]);
      mix.transmission *= Math.max(0.05, found.transmission);
    }
    return;
  }
  if (name === "Zoom") {
    if (physical > 0.5 && physical < 180) {
      beam.angle = physical;
    }
    return;
  }
  if (name === "Focusn" || name === "Focus") {
    // Nought and full are both *somewhere*: a beam at the ends of its focus
    // travel is soft, and sharp in between is where the gobo sits. Without a
    // throw distance to focus at, the middle of the travel is called sharp.
    beam.blur = Math.min(1, Math.abs(fraction - 0.5) * 2);
    return;
  }
  if (name === "Frostn" || name === "Frost") {
    beam.frost = Math.max(beam.frost, Math.min(1, bandOf(fn, value) * (number >= 2 ? 1 : 0.6)));
    return;
  }
  if (name === "Iris") {
    beam.iris = Math.min(1, Math.max(0.03, physical));
    return;
  }
  if ((name.startsWith("Gobon") || name.startsWith("AnimationWheeln")) && fn.wheel !== null) {
    // Any function on a gobo or animation wheel whose set names a slot puts
    // that slot's picture in the beam — the index, the rotating and the
    // shaking functions alike, and the Robin T1 selects its animation wheel
    // through a function it calls `AnimationWheel1Pos`.
    const slot = slotAt(fn, value);
    const found = slot === null ? undefined : device.wheels.get(fn.wheel)?.slots[slot - 1];
    const wheel = wheelKey(name, number);
    if (found?.media !== null && found?.media !== undefined && !beam.gobos.some((gobo) => gobo.wheel === wheel)) {
      beam.gobos.push({ wheel, media: found.media, rotation: 0, spin: 0 });
    }
    if (name === "Gobon" || name === "AnimationWheeln") {
      return;
    }
  }
  if (name === "GobonPos" || name === "AnimationWheelnPos") {
    const image = beam.gobos.find((gobo) => gobo.wheel === wheelKey(name, number));
    if (image !== undefined) {
      image.rotation = physical;
    }
    return;
  }
  if (name === "GobonPosRotate" || name === "AnimationWheelnPosRotate" || name === "AnimationWheelnRotate") {
    const image = beam.gobos.find((gobo) => gobo.wheel === wheelKey(name, number));
    if (image !== undefined) {
      image.spin = speedOf(fn, value);
    }
    return;
  }
  if (name === "Prismn" && fn.wheel !== null) {
    const slot = slotAt(fn, value);
    const found = slot === null ? undefined : device.wheels.get(fn.wheel)?.slots[slot - 1];
    beam.prism = found !== undefined && found.facets.length > 0 ? { facets: found.facets, rotation: 0, spin: 0 } : null;
    return;
  }
  if (name === "PrismnPos" && beam.prism !== null) {
    beam.prism.rotation = physical;
    return;
  }
  if (name === "PrismnPosRotate" && beam.prism !== null) {
    beam.prism.spin = speedOf(fn, value);
    return;
  }
  if (name === "BladenA" || name === "BladenB" || name === "BladenRot") {
    while (beam.blades.length < number) {
      beam.blades.push({ a: 0, b: 0, rotation: 0 });
    }
    const blade = beam.blades[number - 1];
    if (blade === undefined) {
      return;
    }
    // GDTF states a blade's travel as a fraction of the beam; some files
    // stop short of 1, and that is the fixture's own reach.
    const insertion = Math.min(1, Math.max(0, physical));
    if (name === "BladenA") {
      // A blade with no B channel goes in straight; a B channel, read after
      // it, tilts it.
      blade.a = insertion;
      blade.b = insertion;
    } else if (name === "BladenB") {
      blade.b = insertion;
    } else {
      blade.rotation = physical;
    }
    return;
  }
  if (name === "ShaperRot") {
    beam.shaper = physical;
  }
}

/** Reads a GDTF device's state out of `frame`. */
function readDevice(
  fixture: RigFixture,
  device: Device,
  frame: TelemetryFrameView,
  index: number,
  into: FixtureState,
): void {
  const { values, masters } = channelValues(fixture, device, frame, index);
  const nodeIndex = new Map<string, number>();
  for (const node of device.nodes) {
    if (!nodeIndex.has(node.name)) {
      nodeIndex.set(node.name, node.index);
    }
  }

  into.axes.clear();
  for (const channel of device.channels) {
    const value = values.get(channel) ?? 0;
    const fn = functionAt(channel, value, masters);
    if (fn === null) {
      continue;
    }
    const [name] = family(fn.attribute);
    if (name !== "Pan" && name !== "Tilt") {
      continue;
    }
    const at = channel.geometry === null ? undefined : nodeIndex.get(channel.geometry);
    const axis = at ?? firstAxis(device.nodes, name === "Pan" ? 0 : 1);
    if (axis === null) {
      continue;
    }
    const held = into.axes.get(axis) ?? { pan: 0, tilt: 0 };
    if (name === "Pan") {
      held.pan = physicalOf(fn, value);
    } else {
      held.tilt = physicalOf(fn, value);
    }
    into.axes.set(axis, held);
  }

  for (const node of device.nodes) {
    if (node.beam === null) {
      continue;
    }
    const beam = into.beams.get(node.index) ?? darkBeam();
    resetBeam(beam);
    const mix = {
      emitters: [0, 0, 0] as [number, number, number],
      any: false,
      dimmer: null as number | null,
      kelvin: null as number | null,
      sub: [0, 0, 0] as [number, number, number],
      filter: [1, 1, 1] as [number, number, number],
      transmission: 1,
    };
    for (const channel of device.channels) {
      // A channel acts on the geometry it names and everything under it; one
      // that names none acts on the whole device.
      if (channel.geometry !== null) {
        const at = nodeIndex.get(channel.geometry);
        if (at !== undefined && !isUnder(device.nodes, node.index, at)) {
          continue;
        }
      }
      const value = values.get(channel) ?? 0;
      const fn = functionAt(channel, value, masters);
      if (fn !== null) {
        applyFunction(beam, fn, value, device, mix);
      }
    }
    finishBeam(beam, mix, node.beam.kelvin);
    into.beams.set(node.index, beam);
  }
}

/** The first `Axis` node of a device — for a pan (`0`) or the second for a tilt (`1`). */
function firstAxis(nodes: readonly DeviceNode[], which: number): number | null {
  const axes = nodes.filter((node) => node.kind === "Axis");
  return axes[which]?.index ?? axes[0]?.index ?? null;
}

/** Back to dark, keeping the arrays. */
function resetBeam(beam: BeamState): void {
  beam.intensity = 0;
  beam.strobe = "none";
  beam.hz = 0;
  beam.color = [1, 1, 1];
  beam.angle = null;
  beam.iris = 1;
  beam.blur = 0;
  beam.frost = 0;
  beam.gobos.length = 0;
  beam.prism = null;
  beam.blades.length = 0;
  beam.shaper = 0;
}

/** Colour and intensity, out of everything the channels said. */
function finishBeam(
  beam: BeamState,
  mix: { emitters: [number, number, number]; any: boolean; dimmer: number | null; kelvin: number | null; sub: [number, number, number]; filter: [number, number, number]; transmission: number },
  sourceKelvin: number,
): void {
  const closed = beam.intensity < 0;
  let colour: [number, number, number];
  let brightness: number;
  if (mix.any) {
    const top = Math.max(...mix.emitters);
    brightness = Math.min(1, top);
    colour = top > 0 ? [mix.emitters[0] / top, mix.emitters[1] / top, mix.emitters[2] / top] : [1, 1, 1];
  } else {
    brightness = 1;
    const kelvin = mix.kelvin ?? (sourceKelvin > 0 ? sourceKelvin : 6500);
    colour = kelvinColour(kelvin);
  }
  if (mix.any && mix.kelvin !== null) {
    const tint = kelvinColour(mix.kelvin);
    colour = [colour[0] * tint[0], colour[1] * tint[1], colour[2] * tint[2]];
  }
  colour = [
    colour[0] * (1 - mix.sub[0]) * mix.filter[0],
    colour[1] * (1 - mix.sub[1]) * mix.filter[1],
    colour[2] * (1 - mix.sub[2]) * mix.filter[2],
  ];
  const top = Math.max(...colour);
  // What the filters took is light that is gone, not a darker hue: the
  // colour is renormalised and the loss moves into the intensity.
  const filtered = Math.max(0.02, top) * mix.transmission;
  beam.color = top > 0 ? [colour[0] / top, colour[1] / top, colour[2] / top] : [1, 1, 1];
  const dimmer = mix.dimmer ?? (mix.any ? 1 : 0);
  beam.intensity = closed ? 0 : Math.min(1, dimmer * brightness * Math.sqrt(filtered));
}

/** A built-in gobo pattern's name for an OFL gobo slot with no picture. */
function builtInGobo(slot: number): string {
  return `@${String(((slot - 1) % 6) + 1)}`;
}

/** A named range's index among its channel's ranges, one-based, or `null`. */
function rangeIndex(channel: RigChannel, value: number): { index: number; name: string } | null {
  for (let index = 0; index < channel.ranges.length; index += 1) {
    const range = channel.ranges[index];
    if (range !== undefined && range.from <= value && value <= range.to) {
      return { index: index + 1, name: range.name };
    }
  }
  return null;
}

/** Reads a fixture with no device: the attribute list, and named ranges. */
function readPlain(
  fixture: RigFixture,
  frame: TelemetryFrameView,
  index: number,
  look: Look,
  into: FixtureState,
): void {
  readLook(fixture, frame, look);
  const beam = into.single ?? darkBeam();
  resetBeam(beam);
  beam.intensity = look.intensity;
  beam.color = [look.red, look.green, look.blue];
  beam.angle = look.angle;
  into.pan = look.pan;
  into.tilt = look.tilt;
  const { channels } = fixture;
  const value = (channel: RigChannel | undefined): number | null =>
    channel === undefined ? null : channelValue(frame, index, fixture.address, channel);

  const shutter = channels.Shutter;
  const shutterValue = value(shutter);
  if (shutter !== undefined && shutterValue !== null) {
    const range = rangeIndex(shutter, shutterValue);
    if (range !== null && /strob|flash/i.test(range.name)) {
      const found = shutter.ranges[range.index - 1];
      const span = found === undefined ? 1 : Math.max(1, found.to - found.from);
      const fraction = found === undefined ? 0.5 : (shutterValue - found.from) / span;
      beam.strobe = /random/i.test(range.name) ? "random" : "strobe";
      beam.hz = 1 + fraction * 19;
    }
  }
  const focus = value(channels.Focus);
  if (focus !== null) {
    beam.blur = Math.min(1, Math.abs(focus / 65535 - 0.5) * 2);
  }
  const frost = value(channels.Frost);
  if (frost !== null) {
    beam.frost = frost / 65535;
  }
  const iris = value(channels.Iris);
  if (iris !== null) {
    // OFL's iris is *closing* as the value rises.
    beam.iris = Math.max(0.05, 1 - iris / 65535);
  }
  const gobo = channels.Gobo;
  const goboValue = value(gobo);
  if (gobo !== undefined && goboValue !== null) {
    const range = rangeIndex(gobo, goboValue);
    if (range !== null && !/open|none/i.test(range.name)) {
      beam.gobos.push({ wheel: "Gobo1", media: builtInGobo(range.index), rotation: 0, spin: 0 });
    }
  }
  const prism = channels.Prism;
  const prismValue = value(prism);
  if (prism !== undefined && prismValue !== null && prismValue > 2000) {
    const range = rangeIndex(prism, prismValue);
    if (range === null || !/open|none|off/i.test(range.name)) {
      beam.prism = {
        facets: [0, 1, 2].map((facet) => ({
          x: 0.35 * Math.cos((facet * 2 * Math.PI) / 3),
          y: 0.35 * Math.sin((facet * 2 * Math.PI) / 3),
        })),
        rotation: 0,
        spin: 0,
      };
    }
  }
  into.single = beam;
}

const plainLook = darkLook();

/**
 * Reads what `fixture` is doing out of `frame` into `into`. A fixture whose
 * universe the frame does not carry is dark and marked absent.
 */
export function readState(
  fixture: RigFixture,
  frame: TelemetryFrameView | null,
  into: FixtureState,
): FixtureState {
  const index = frame === null || !frame.hasFrame ? null : universeIndex(frame, fixture.universe);
  if (frame === null || index === null || fixture.address < 1) {
    into.present = false;
    into.axes.clear();
    for (const beam of into.beams.values()) {
      resetBeam(beam);
    }
    if (into.single !== null) {
      resetBeam(into.single);
    }
    into.pan = 0;
    into.tilt = 0;
    return into;
  }
  into.present = true;
  if (fixture.device !== null && fixture.device.channels.length > 0) {
    readDevice(fixture, fixture.device, frame, index, into);
  } else {
    readPlain(fixture, frame, index, plainLook, into);
  }
  return into;
}

/**
 * How bright a strobing beam is at time `seconds` — `1` when it is not
 * strobing. `seed` spreads a random strobe so a rig does not flash in step.
 */
export function strobeLevel(beam: BeamState, seconds: number, seed: number): number {
  if (beam.strobe === "none" || beam.hz <= 0) {
    return 1;
  }
  const phase = (seconds * beam.hz) % 1;
  switch (beam.strobe) {
    case "strobe":
      return phase < 0.3 ? 1 : 0;
    case "pulseOpen":
      return phase;
    case "pulseClose":
      return 1 - phase;
    case "random": {
      const tick = Math.floor(seconds * beam.hz);
      const noise = Math.sin((tick + seed * 7.13) * 12.9898) * 43758.5453;
      return noise - Math.floor(noise) < 0.5 ? 1 : 0;
    }
  }
}
