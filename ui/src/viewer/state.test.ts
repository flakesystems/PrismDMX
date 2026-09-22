/**
 * What every beam and axis is doing, read off the **cable** — **S30b** — held
 * to the daemon's bytes: the lit frame `crates/prismd/tests/ui_viewer.rs`
 * recorded has the first head open, panned to +135° and its dots gobo in.
 * Where a case needs levels the script did not set — a closed shutter, a
 * strobe, the star — the recorded frame is copied and the levels written into
 * the copy, as `look.test.ts` does, so the framing is still the daemon's.
 *
 * The function rules that a two-channel test head cannot show — a speed a
 * profile does not state, a frost band, a colour temperature — are tested on
 * functions shaped like the Robin T1's, which is where each rule came from.
 */

import { describe, expect, it } from "vitest";

import { TelemetryFrameView, TELEMETRY_HEADER_BYTES } from "../telemetry/frame";
import { finalShow, litFrame, litPayload } from "../testing/viewer-recording";
import type { DeviceFunction } from "./device";
import { wheelColour } from "./look";
import type { RigFixture } from "./rig";
import { rigOf } from "./rig";
import type { BeamState } from "./state";
import { bandOf, darkBeam, emptyState, family, kelvinColour, readState, speedOf, strobeLevel } from "./state";

function fixture(id: number): RigFixture {
  const found = rigOf(finalShow()).find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error(`fixture ${String(id)} is not in the rig`);
  }
  return found;
}

/** The recorded frame with `levels` written into its first universe, by DMX address. */
function frameWith(levels: Readonly<Record<number, number>>): TelemetryFrameView {
  const bytes = litPayload().slice();
  for (const [address, level] of Object.entries(levels)) {
    bytes[TELEMETRY_HEADER_BYTES + 2 + Number(address) - 1] = level;
  }
  const view = new TelemetryFrameView();
  expect(view.read(bytes)).toBeNull();
  return view;
}

/** The head's one beam — geometry 3. */
function beamOf(id: number, frame: TelemetryFrameView): BeamState {
  const beam = readState(fixture(id), frame, emptyState()).beams.get(3);
  if (beam === undefined) {
    throw new Error("the head has no beam at geometry 3");
  }
  return beam;
}

/** The head's channels, by DMX address at address 1: gobo 7, shutter 8. */
const GOBO = 7;
const SHUTTER = 8;

function fn(extra: Partial<DeviceFunction>): DeviceFunction {
  return {
    attribute: "Gobo1PosRotate",
    from: 0,
    to: 65535,
    physicalFrom: 0,
    physicalTo: 1,
    wheel: null,
    sets: [],
    modeMaster: null,
    modeFrom: 0,
    modeTo: 65535,
    ...extra,
  };
}

describe("the recorded lit frame, read as a device", () => {
  it("has the first head open, its yoke panned to +135° and the dots gobo in", () => {
    const state = readState(fixture(1), litFrame(), emptyState());
    expect(state.present).toBe(true);
    expect(state.axes.get(1)?.pan).toBeCloseTo(135, 1);
    expect(state.axes.get(2)?.tilt).toBeCloseTo(0, 1);
    const beam = state.beams.get(3);
    expect(beam?.intensity).toBe(1);
    expect(beam?.strobe).toBe("none");
    expect(beam?.gobos).toEqual([{ wheel: "Gobo1", media: "gobo-dots", rotation: 0, spin: 0 }]);
    // Zoom at nought: the narrow end of 8…40°.
    expect(beam?.angle).toBeCloseTo(8, 6);
    // No colour channel: the colour of the lamp the file states, 6500 K.
    expect(beam?.color).toEqual(kelvinColour(6500));
  });

  it("has the second head dark", () => {
    expect(beamOf(2, litFrame()).intensity).toBe(0);
  });

  it("is absent with no frame, and every beam dark", () => {
    const state = readState(fixture(1), litFrame(), emptyState());
    readState(fixture(1), null, state);
    expect(state.present).toBe(false);
    expect(state.axes.size).toBe(0);
    expect(state.beams.get(3)?.intensity).toBe(0);
  });
});

describe("a GDTF shutter", () => {
  it("is dark in its Closed set and open in its Open one", () => {
    expect(beamOf(1, frameWith({ [SHUTTER]: 0 })).intensity).toBe(0);
    expect(beamOf(1, frameWith({ [SHUTTER]: 40 })).intensity).toBe(1);
  });

  it("strobes at the rate its strobe function's physical range says", () => {
    const beam = beamOf(1, frameWith({ [SHUTTER]: 100 }));
    expect(beam.strobe).toBe("strobe");
    const from = Math.floor((64 * 65535) / 255);
    expect(beam.hz).toBeCloseTo(1 + (19 * (100 * 257 - from)) / (65535 - from), 6);
  });
});

describe("a GDTF gobo wheel", () => {
  it("puts the slot its set names in the beam, and nothing for Open", () => {
    expect(beamOf(1, frameWith({ [GOBO]: 25 })).gobos.map((gobo) => gobo.media)).toEqual(["gobo-star"]);
    expect(beamOf(1, frameWith({ [GOBO]: 5 })).gobos).toEqual([]);
  });
});

describe("a fixture with no device", () => {
  it("is read from its attribute list into its one beam", () => {
    const state = readState(fixture(10), litFrame(), emptyState());
    expect(state.present).toBe(true);
    expect(state.beams.size).toBe(0);
    expect(state.single).not.toBeNull();
  });
});

describe("family", () => {
  it("names an attribute's family and its first number", () => {
    expect(family("Gobo2PosRotate")).toEqual(["GobonPosRotate", 2]);
    expect(family("Blade3A")).toEqual(["BladenA", 3]);
    expect(family("ColorAdd_R")).toEqual(["ColorAdd_R", 0]);
    expect(family("AnimationWheel1Pos")).toEqual(["AnimationWheelnPos", 1]);
  });
});

describe("speedOf", () => {
  it("is the physical range where the profile states a speed", () => {
    const gobo = fn({ physicalFrom: -757.895, physicalTo: 757.895 });
    expect(speedOf(gobo, 65535)).toBeCloseTo(757.895, 3);
    expect(speedOf(gobo, 0)).toBeCloseTo(-757.895, 3);
  });

  it("is still in a set named for it, whatever the range says", () => {
    const prism = fn({
      physicalFrom: 1,
      physicalTo: -1,
      sets: [
        { name: "Fast to slow 7/7", from: 0, to: 32767, slot: null },
        { name: "No rotation middle", from: 32768, to: 33023, slot: null },
      ],
    });
    expect(speedOf(prism, 32896)).toBe(0);
  });

  it("turns steadily, by the half of the range, where no speed is stated", () => {
    // The Robin T1's animation wheel: 0…1, which is no speed at all.
    const wheel = fn({});
    expect(speedOf(wheel, 1000)).toBeLessThan(0);
    expect(speedOf(wheel, 60000)).toBeGreaterThan(0);
    expect(Math.abs(speedOf(wheel, 60000))).toBe(45);
  });
});

describe("bandOf", () => {
  // The Robin T1's first frost: Open, a ramp, full, then a pulse.
  const frost = fn({
    attribute: "Frost1",
    to: 21_588,
    sets: [
      { name: "Open", from: 0, to: 256, slot: null },
      { name: "Light Frost from 0% to 100%", from: 257, to: 13_106, slot: null },
      { name: "100% Light frost", from: 13_107, to: 13_877, slot: null },
      { name: "Pulse closing from slow to fast", from: 13_878, to: 21_588, slot: null },
    ],
  });

  it("reads how far into its effect a value is from the set it stands in", () => {
    expect(bandOf(frost, 100)).toBe(0);
    expect(bandOf(frost, 257 + (13_106 - 257) / 2)).toBeCloseTo(0.5, 6);
    expect(bandOf(frost, 13_500)).toBe(1);
    expect(bandOf(frost, 20_000)).toBe(0.5);
  });

  it("is the function's own fraction where the sets say nothing", () => {
    expect(bandOf(fn({ attribute: "Frost1" }), 65535 / 4)).toBeCloseTo(0.25, 6);
  });
});

describe("strobeLevel", () => {
  it("is 1 for a beam that is not strobing", () => {
    expect(strobeLevel(darkBeam(), 0.37, 1)).toBe(1);
  });

  it("flashes, ramps and flickers by its kind", () => {
    const beam = { ...darkBeam(), strobe: "strobe" as const, hz: 2 };
    expect(strobeLevel(beam, 0.05, 0)).toBe(1);
    expect(strobeLevel(beam, 0.3, 0)).toBe(0);
    expect(strobeLevel({ ...beam, strobe: "pulseOpen" }, 0.125, 0)).toBeCloseTo(0.25, 6);
    expect(strobeLevel({ ...beam, strobe: "pulseClose" }, 0.125, 0)).toBeCloseTo(0.75, 6);
    const random = { ...beam, strobe: "random" as const };
    const flashes = Array.from({ length: 40 }, (_, tick) => strobeLevel(random, tick / 2, 3));
    expect(new Set(flashes)).toEqual(new Set([0, 1]));
    // The same fixture flickers the same way at the same moment.
    expect(strobeLevel(random, 7.5, 3)).toBe(strobeLevel(random, 7.5, 3));
  });
});

describe("colour", () => {
  it("is the colour of a temperature, warm to cold", () => {
    const warm = kelvinColour(2700);
    const cold = kelvinColour(8000);
    expect(Math.max(...warm)).toBe(1);
    expect(warm[0]).toBe(1);
    expect(warm[2]).toBeLessThan(0.5);
    expect(cold[2]).toBe(1);
    expect(cold[0]).toBeLessThan(1);
  });

  it("reads cold and warm whites and temperatures from a wheel slot's name", () => {
    expect(wheelColour("Cold White")).toEqual(kelvinColour(7500));
    expect(wheelColour("Kaltweiß")).toEqual(kelvinColour(7500));
    expect(wheelColour("Warm White")).toEqual(kelvinColour(3000));
    expect(wheelColour("CTO 3200K")).toEqual(kelvinColour(3200));
    expect(wheelColour("5600 K")).toEqual(kelvinColour(5600));
    expect(wheelColour("Deep Red")).toEqual([1, 0.1, 0.1]);
    expect(wheelColour("Gobo 3")).toBeUndefined();
  });
});
