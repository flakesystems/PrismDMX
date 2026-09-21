/**
 * What the viewer reads off the cable, tested against **the daemon's bytes**.
 *
 * The lit frame is `prism_ipc::TelemetryFrame::encode`'s, recorded off a real
 * `prismd` with the first head opened and panned by the programmer, and
 * decoded here by `decodeServerMessage` and `TelemetryFrameView`. Where a case
 * needs levels the script did not set — a colour wheel, a closed shutter — the
 * recorded frame is copied and the levels are written into the copy, so the
 * framing is still the daemon's and only the numbers are the test's.
 */

import { describe, expect, it } from "vitest";

import { TelemetryFrameView, TELEMETRY_HEADER_BYTES } from "../telemetry/frame";
import { finalShow, litFrame, litPayload, viewerRecording } from "../testing/viewer-recording";
import { LIT, channelValue, darkLook, readLook, universeIndex } from "./look";
import type { RigChannel, RigFixture } from "./rig";
import { rigOf } from "./rig";

function fixture(id: number): RigFixture {
  const found = rigOf(finalShow()).find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error(`fixture ${String(id)} is not in the rig`);
  }
  return found;
}

/** The recorded frame with `levels` written into its first universe. */
function frameWith(levels: Readonly<Record<number, number>>): TelemetryFrameView {
  const bytes = litPayload().slice();
  for (const [address, level] of Object.entries(levels)) {
    // Header, then the section's two bytes of universe number, then the levels.
    bytes[TELEMETRY_HEADER_BYTES + 2 + Number(address) - 1] = level;
  }
  const view = new TelemetryFrameView();
  expect(view.read(bytes)).toBeNull();
  return view;
}

/** A fixture at address 1 of universe 1 with exactly these channels. */
function made(channels: RigFixture["channels"]): RigFixture {
  const base = fixture(10);
  return { ...base, address: 1, universe: 1, channels };
}

function channel(coarse: number, extra: Partial<RigChannel> = {}): RigChannel {
  return { coarse, fine: null, invert: false, from: 0, to: 100, ranges: [], ...extra };
}

describe("the recorded lit frame", () => {
  it("is the frame the recording says it is", () => {
    const frame = litFrame();
    const index = universeIndex(frame, viewerRecording.lit.universe);
    expect(index).not.toBeNull();
    for (const [offset, level] of viewerRecording.lit.firstHead.entries()) {
      expect(frame.levelAt(index ?? 0, offset)).toBe(level);
    }
  });

  it("has the first head open and panned to +135°, off its sixteen-bit channel", () => {
    const look = readLook(fixture(1), litFrame());
    expect(look.present).toBe(true);
    expect(look.intensity).toBe(1);
    expect(look.pan).toBeCloseTo(135, 1);
    // Tilt stands at its home, the middle of −135…+135.
    expect(look.tilt).toBeCloseTo(0, 1);
    // Zoom at nought is the narrow end of the profile's 8…40°.
    expect(look.angle).toBeCloseTo(8, 6);
    expect([look.red, look.green, look.blue]).toEqual([1, 1, 1]);
  });

  it("has the second head and the PAR dark", () => {
    expect(readLook(fixture(2), litFrame()).intensity).toBe(0);
    expect(readLook(fixture(10), litFrame()).intensity).toBeLessThan(LIT);
  });
});

describe("a colour-only fixture", () => {
  it("is as bright as its brightest emitter, in the colour they make", () => {
    // The PAR at 101: red half, blue full, no dimmer channel — its software
    // dimmer is already on the cable.
    const look = readLook(fixture(10), frameWith({ 101: 128, 103: 255 }));
    expect(look.intensity).toBeCloseTo(1, 6);
    expect(look.blue).toBe(1);
    expect(look.red).toBeCloseTo(128 / 255, 3);
    expect(look.green).toBe(0);
  });

  it("scaled by a dimmer where it has one", () => {
    const look = readLook(
      made({ Dimmer: channel(0), Red: channel(1), Green: channel(2) }),
      frameWith({ 1: 51, 2: 255, 3: 255 }),
    );
    expect(look.intensity).toBeCloseTo(0.2, 3);
    expect([look.red, look.green, look.blue]).toEqual([1, 1, 0]);
  });
});

describe("a head with no emitters", () => {
  it("filters white through cyan, magenta and yellow", () => {
    const look = readLook(
      made({ Dimmer: channel(0), Cyan: channel(1), Magenta: channel(2), Yellow: channel(3) }),
      frameWith({ 1: 255, 2: 255, 3: 0, 4: 0 }),
    );
    expect(look.intensity).toBe(1);
    expect(look.red).toBe(0);
    expect(look.green).toBe(1);
    expect(look.blue).toBe(1);
  });

  it("takes a colour wheel slot's colour from its name, and white from a name that is none", () => {
    const wheel = channel(1, {
      ranges: [
        { name: "Open", from: 0, to: 9999 },
        { name: "Deep Red", from: 10000, to: 19999 },
        { name: "Light Blue", from: 20000, to: 65535 },
      ],
    });
    const fixtureWithWheel = made({ Dimmer: channel(0), ColorWheel: wheel });
    const open = readLook(fixtureWithWheel, frameWith({ 1: 255, 2: 0 }));
    expect([open.red, open.green, open.blue]).toEqual([1, 1, 1]);
    const red = readLook(fixtureWithWheel, frameWith({ 1: 255, 2: 60 }));
    expect(red.red).toBe(1);
    expect(red.blue).toBeLessThan(0.2);
    const blue = readLook(fixtureWithWheel, frameWith({ 1: 255, 2: 200 }));
    expect(blue.blue).toBe(1);
  });

  it("is dark with its shutter in a range called closed", () => {
    const shutter = channel(1, {
      ranges: [
        { name: "Closed", from: 0, to: 2000 },
        { name: "Open", from: 2001, to: 65535 },
      ],
    });
    const head = made({ Dimmer: channel(0), Shutter: shutter });
    expect(readLook(head, frameWith({ 1: 255, 2: 0 })).intensity).toBe(0);
    expect(readLook(head, frameWith({ 1: 255, 2: 255 })).intensity).toBe(1);
  });

  it("makes no light with neither a dimmer nor an emitter", () => {
    expect(readLook(made({ Pan: channel(0) }), frameWith({ 1: 255 })).intensity).toBe(0);
  });
});

describe("position and zoom", () => {
  it("reads the placeholder range as a typical head's travel", () => {
    const look = readLook(made({ Pan: channel(0), Tilt: channel(1) }), frameWith({ 1: 255, 2: 0 }));
    expect(look.pan).toBe(270);
    expect(look.tilt).toBe(-135);
  });

  it("applies the device's own invert, and reads the fine byte after the coarse", () => {
    const frame = frameWith({ 1: 0x12, 2: 0x34 });
    expect(channelValue(frame, 0, 1, channel(0, { fine: 1 }))).toBe(0x1234);
    expect(channelValue(frame, 0, 1, channel(0, { fine: 1, invert: true }))).toBe(65535 - 0x1234);
    expect(channelValue(frame, 0, 1, channel(0))).toBe(0x12 * 257);
  });

  it("reads a zoom placeholder as 10…40°, and does not guess at a range that is not degrees", () => {
    const placeholderZoom = readLook(made({ Dimmer: channel(0), Zoom: channel(1) }), frameWith({ 1: 255, 2: 255 }));
    expect(placeholderZoom.angle).toBe(40);
    const odd = readLook(
      made({ Dimmer: channel(0), Zoom: channel(1, { from: 0, to: 500 }) }),
      frameWith({ 1: 255, 2: 255 }),
    );
    expect(odd.angle).toBeNull();
    expect(readLook(made({ Dimmer: channel(0) }), frameWith({ 1: 255 })).angle).toBeNull();
  });
});

describe("a fixture the frame does not carry", () => {
  it("is absent and dark, and the look it was given is reused", () => {
    const into = darkLook();
    into.intensity = 1;
    const elsewhere = { ...fixture(1), universe: 9 };
    const look = readLook(elsewhere, litFrame(), into);
    expect(look).toBe(into);
    expect(look.present).toBe(false);
    expect(look.intensity).toBe(0);
    expect(universeIndex(litFrame(), 9)).toBeNull();
  });
});
