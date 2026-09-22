/**
 * What a fixture is doing, read off the **cable** — the half of S30's exit
 * criteria that says *beams reflect live output*.
 *
 * # Why the cable, and not the programmer or the playbacks
 *
 * Live output is what the telemetry channel carries: the frame the daemon gave
 * the outputs, after the merge, the masters, the software dimmer and every
 * invert (`docs/IPC_PROTOCOL.md` §7). Reading it here means the viewer shows
 * what the rig is being told, which is the question an operator looks at a
 * visualiser to answer — and it means the viewer asks the daemon nothing and
 * the tick does nothing new for it.
 *
 * It also means this file is a **reading of bytes**, and it is tested that
 * way: against a frame `crates/prismd/tests/ui_viewer.rs` recorded off a real
 * daemon, decoded by `decodeServerMessage` and `TelemetryFrameView`.
 *
 * # What is read, and what each is taken to mean
 *
 * - **Intensity** is the dimmer. A fixture with no dimmer channel — an RGBW PAR,
 *   whose software dimmer (S43) is already folded into the colour it sends — is
 *   as bright as its brightest emitter. A fixture with neither makes no light.
 * - **Colour** is the sum of its emitters, each at the colour it is, then
 *   filtered by cyan, magenta and yellow and by the colour wheel slot's name.
 * - **A closed shutter** is dark: a shutter channel standing in a range named
 *   *closed*, *blackout* or *off*.
 * - **Pan and tilt** are degrees, off the channel's physical range. The
 *   generic profiles' placeholder `0…100` is not a range of degrees and is read
 *   as the ±270° / ±135° a typical head has — the same numbers `prism_core`
 *   gives an OFL head that states none.
 * - **Zoom** is degrees where the range is one, and a percentage of 10°…40°
 *   where it is the placeholder.
 *
 * `invert` on a channel is applied because it is the *device* reading that
 * channel backwards. `Fixture::invertPan` is not: it is an operator's
 * convention, already on the cable, and the head moves as the cable says.
 */

import type { TelemetryFrameView } from "../telemetry/frame";
import type { RigChannel, RigFixture } from "./rig";

/** What a fixture's beams are doing. */
export interface Look {
  /** `0..=1`. */
  intensity: number;
  /** Red, green and blue at `0..=1`, the largest at 1 where there is any colour. */
  red: number;
  green: number;
  blue: number;
  /** Degrees. */
  pan: number;
  /** Degrees. */
  tilt: number;
  /** The beam's full angle in degrees, or `null` for the profile's own. */
  angle: number | null;
  /** Whether its universe is on the cable at all. */
  present: boolean;
}

/** A dark, absent look — what a fixture is before a frame has arrived. */
export function darkLook(): Look {
  return { intensity: 0, red: 1, green: 1, blue: 1, pan: 0, tilt: 0, angle: null, present: false };
}

/** Below this, a beam is not drawn and a fixture is not counted as lit. */
export const LIT = 0.02;

/** What each emitter adds, in red, green and blue. */
const EMITTERS = [
  ["Red", 1, 0, 0],
  ["Green", 0, 1, 0],
  ["Blue", 0, 0, 1],
  ["White", 1, 1, 1],
  ["Amber", 1, 0.62, 0],
  ["WarmWhite", 1, 0.8, 0.55],
  ["ColdWhite", 0.8, 0.9, 1],
  ["Lime", 0.7, 1, 0],
  ["Uv", 0.25, 0, 0.6],
  ["Indigo", 0.3, 0, 1],
] as const;

/**
 * The colour a wheel slot's name stands for.
 *
 * GDTF gives a slot a CIE colour and this desk does not carry it yet; OFL gives
 * a name and a hex that S51 did not keep. So the name is what there is, and a
 * name that is not a colour — *Open*, *Gobo 3*, *Rainbow* — is white.
 */
const WHEEL_COLOURS: readonly (readonly [RegExp, number, number, number])[] = [
  [/deep red|congo|\bred\b/i, 1, 0.1, 0.1],
  [/orange/i, 1, 0.5, 0.1],
  [/amber|ctoi?\b|\bcto\b/i, 1, 0.7, 0.35],
  [/yellow/i, 1, 0.95, 0.2],
  [/lime|light green/i, 0.7, 1, 0.3],
  [/green/i, 0.15, 1, 0.25],
  [/cyan|turquoise/i, 0.2, 1, 1],
  [/light blue|ctb/i, 0.6, 0.8, 1],
  [/blue/i, 0.15, 0.3, 1],
  [/lavender|violet|purple|uv/i, 0.6, 0.25, 1],
  [/magenta/i, 1, 0.2, 0.9],
  [/pink/i, 1, 0.55, 0.75],
];

/** An sRGB `0..=255` component as linear light. */
export function linear(component: number): number {
  const value = component / 255;
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

/**
 * The colour of a black body at `kelvin`, as linear RGB with its largest
 * component 1 — Tanner Helland's fit, which is what every visualiser uses and
 * is within a few per cent of the Planckian locus from 1 000 to 40 000 K.
 */
export function kelvinColour(kelvin: number): [number, number, number] {
  const t = Math.min(40000, Math.max(1000, kelvin)) / 100;
  const red = t <= 66 ? 255 : 329.698727446 * (t - 60) ** -0.1332047592;
  const green = t <= 66 ? 99.4708025861 * Math.log(t) - 161.1195681661 : 288.1221695283 * (t - 60) ** -0.0755148492;
  const blue = t >= 66 ? 255 : t <= 19 ? 0 : 138.5177312231 * Math.log(t - 10) - 305.0447927307;
  const rgb: [number, number, number] = [
    linear(Math.min(255, Math.max(0, red))),
    linear(Math.min(255, Math.max(0, green))),
    linear(Math.min(255, Math.max(0, blue))),
  ];
  const top = Math.max(...rgb, 1e-6);
  return [rgb[0] / top, rgb[1] / top, rgb[2] / top];
}

/**
 * The colour a wheel slot's **name** says, or `undefined` when the name is
 * not a colour. A colour temperature — *5600K*, *CTB 8000 K* — is the colour
 * of that temperature; *cold* and *warm white* (and the German *kalt* and
 * *warm*) are 7500 K and 3000 K. Those come first, because *Cold White* is not
 * white and *CTO 3200K* is not amber.
 */
export function wheelColour(name: string): readonly [number, number, number] | undefined {
  const kelvin = /(\d{4,5})\s*k\b/i.exec(name);
  if (kelvin !== null) {
    return kelvinColour(Number(kelvin[1]));
  }
  if (/cold|cool|kalt/i.test(name)) {
    return kelvinColour(7500);
  }
  if (/warm/i.test(name)) {
    return kelvinColour(3000);
  }
  const found = WHEEL_COLOURS.find(([pattern]) => pattern.test(name));
  return found === undefined ? undefined : [found[1], found[2], found[3]];
}

/** A shutter range that is dark. */
const CLOSED = /clos|blackout|\boff\b/i;

/** Where a universe sits in a frame, or `null` when the frame has not got it. */
export function universeIndex(frame: TelemetryFrameView, universe: number): number | null {
  for (let index = 0; index < frame.count; index += 1) {
    if (frame.universeAt(index) === universe) {
      return index;
    }
  }
  return null;
}

/** A channel's value, `0..=65535`, with the device's own invert applied. */
export function channelValue(
  frame: TelemetryFrameView,
  index: number,
  address: number,
  channel: RigChannel,
): number {
  const base = address - 1;
  const coarse = frame.levelAt(index, base + channel.coarse);
  const value =
    channel.fine === null ? coarse * 257 : coarse * 256 + frame.levelAt(index, base + channel.fine);
  return channel.invert ? 65535 - value : value;
}

/** The physical value a channel stands at, off its range. */
function physical(value: number, from: number, to: number): number {
  return from + ((to - from) * value) / 65535;
}

/** Whether a range is the `0…100` placeholder a profile uses when it states none. */
function placeholder(channel: RigChannel): boolean {
  return channel.from === 0 && channel.to === 100;
}

/** The named range a value stands in, or `null`. */
function rangeAt(channel: RigChannel, value: number): string | null {
  for (const range of channel.ranges) {
    if (range.from <= value && value <= range.to) {
      return range.name;
    }
  }
  return null;
}

/**
 * Reads what `fixture` is doing out of `frame` into `into`, and answers it.
 *
 * `into` is reused rather than returned new: this runs for every fixture on
 * every frame, and a picture of the rig thirty times a second is not a reason
 * to feed the collector four hundred objects a frame.
 */
export function readLook(
  fixture: RigFixture,
  frame: TelemetryFrameView,
  into: Look = darkLook(),
): Look {
  const index = universeIndex(frame, fixture.universe);
  if (index === null || fixture.address < 1) {
    into.intensity = 0;
    into.red = 1;
    into.green = 1;
    into.blue = 1;
    into.pan = 0;
    into.tilt = 0;
    into.angle = null;
    into.present = false;
    return into;
  }
  const { channels } = fixture;
  const fraction = (channel: RigChannel | undefined): number | null =>
    channel === undefined ? null : channelValue(frame, index, fixture.address, channel) / 65535;

  // The emitters, summed.
  let red = 0;
  let green = 0;
  let blue = 0;
  let emitters = false;
  for (const [attribute, r, g, b] of EMITTERS) {
    const level = fraction(channels[attribute]);
    if (level === null) {
      continue;
    }
    emitters = true;
    red += level * r;
    green += level * g;
    blue += level * b;
  }
  const brightest = Math.max(red, green, blue);

  const dimmer = fraction(channels.Dimmer);
  let intensity: number;
  if (emitters) {
    // A head whose red, green and blue are all at nought is dark whatever its
    // dimmer says — and a PAR with no dimmer is as bright as its emitters.
    intensity = Math.min(1, brightest) * (dimmer ?? 1);
  } else {
    intensity = dimmer ?? 0;
  }
  if (emitters && brightest > 0) {
    red /= brightest;
    green /= brightest;
    blue /= brightest;
  } else {
    red = 1;
    green = 1;
    blue = 1;
  }

  // Subtractive, on top of whatever the emitters made.
  const cyan = fraction(channels.Cyan);
  const magenta = fraction(channels.Magenta);
  const yellow = fraction(channels.Yellow);
  if (cyan !== null) {
    red *= 1 - cyan;
  }
  if (magenta !== null) {
    green *= 1 - magenta;
  }
  if (yellow !== null) {
    blue *= 1 - yellow;
  }

  const wheel = channels.ColorWheel;
  if (wheel !== undefined) {
    const slot = rangeAt(wheel, channelValue(frame, index, fixture.address, wheel));
    const colour = slot === null ? undefined : wheelColour(slot);
    if (colour !== undefined) {
      red *= colour[0];
      green *= colour[1];
      blue *= colour[2];
    }
  }

  const shutter = channels.Shutter;
  if (shutter !== undefined) {
    const slot = rangeAt(shutter, channelValue(frame, index, fixture.address, shutter));
    if (slot !== null && CLOSED.test(slot)) {
      intensity = 0;
    }
  }

  const pan = channels.Pan;
  const tilt = channels.Tilt;
  const zoom = channels.Zoom;
  into.intensity = intensity;
  into.red = red;
  into.green = green;
  into.blue = blue;
  into.pan =
    pan === undefined
      ? 0
      : placeholder(pan)
        ? physical(channelValue(frame, index, fixture.address, pan), -270, 270)
        : physical(channelValue(frame, index, fixture.address, pan), pan.from, pan.to);
  into.tilt =
    tilt === undefined
      ? 0
      : placeholder(tilt)
        ? physical(channelValue(frame, index, fixture.address, tilt), -135, 135)
        : physical(channelValue(frame, index, fixture.address, tilt), tilt.from, tilt.to);
  into.angle = zoomAngle(zoom, zoom === undefined ? 0 : channelValue(frame, index, fixture.address, zoom));
  into.present = true;
  return into;
}

/** The beam angle a zoom channel stands at, or `null` for the profile's own. */
function zoomAngle(zoom: RigChannel | undefined, value: number): number | null {
  if (zoom === undefined) {
    return null;
  }
  if (placeholder(zoom)) {
    return physical(value, 10, 40);
  }
  const degrees = physical(value, zoom.from, zoom.to);
  // A range that is not one of degrees is not guessed at.
  return degrees > 0 && degrees < 180 ? degrees : null;
}
