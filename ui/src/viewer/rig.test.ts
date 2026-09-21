/**
 * The rig the viewer draws, read out of show documents a real daemon built —
 * `crates/prismd/tests/ui_viewer.rs`, decoded through `decodeServerMessage`.
 */

import { describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";
import { finalShow, showsAfterEachStep, viewerRecording } from "../testing/viewer-recording";
import { DEFAULT_BEAM_ANGLE, DEFAULT_SIZE, rigOf } from "./rig";
import type { RigFixture } from "./rig";

function fixture(show: JsonValue, id: number): RigFixture {
  const found = rigOf(show).find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error(`fixture ${String(id)} is not in the rig`);
  }
  return found;
}

describe("the rig, out of the recorded show", () => {
  it("is every patched fixture, in number order, where the hang put it", () => {
    const rig = rigOf(finalShow());
    expect(rig.map((each) => each.id)).toEqual([1, 2, 10]);
    expect(fixture(finalShow(), 1).position).toEqual({ x: -2, y: 6, z: 1 });
    expect(fixture(finalShow(), 2).rotation).toEqual({ x: 20, y: 0, z: 0 });
    expect(fixture(finalShow(), 10).position).toEqual({ x: 0, y: 0.2, z: 4 });
    expect(rig.every((each) => !each.unplaced)).toBe(true);
  });

  it("follows the script: patched at the origin, hung, refused, taken back, hung again", () => {
    const shows = showsAfterEachStep();
    expect(shows.length).toBe(viewerRecording.steps.length);
    const unplaced = (step: number): number =>
      rigOf(shows[step] ?? null).filter((each) => each.unplaced).length;
    expect(unplaced(1)).toBe(3);
    expect(unplaced(2)).toBe(0);
    // The refused spread moved nothing.
    expect(viewerRecording.steps[3]?.refused).toBe(true);
    expect(fixture(shows[3] ?? null, 1).position).toEqual({ x: -2, y: 6, z: 1 });
    // One Oops took the whole hang back, and Redo hung it again.
    expect(unplaced(4)).toBe(3);
    expect(unplaced(5)).toBe(0);
  });

  it("gives a GDTF head the beam its archive states, and a generic PAR the default box", () => {
    const head = fixture(finalShow(), 1);
    expect(head.described).toBe(true);
    expect(head.size).toEqual({ x: 0.3, y: 0.5, z: 0.3 });
    expect(head.beams).toHaveLength(1);
    const beam = head.beams[0];
    expect(beam?.position.y).toBeCloseTo(-0.2, 9);
    // Straight down: GDTF's −Z, in show space. Compared by value, because the
    // negation that makes it leaves a signed nought in the other two.
    expect(beam?.direction.x).toBeCloseTo(0, 12);
    expect(beam?.direction.y).toBe(-1);
    expect(beam?.direction.z).toBeCloseTo(0, 12);
    expect(beam?.angle).toBe(18);
    expect(head.channels.Pan).toMatchObject({ coarse: 0, fine: 1, from: -270, to: 270 });
    expect(head.channels.Dimmer?.coarse).toBe(4);
    expect(head.channels.Zoom).toMatchObject({ from: 8, to: 40 });

    const par = fixture(finalShow(), 10);
    expect(par.described).toBe(false);
    expect(par.size).toEqual(DEFAULT_SIZE);
    expect(par.beams[0]?.angle).toBe(DEFAULT_BEAM_ANGLE);
    expect(Object.keys(par.channels).sort()).toEqual(["Blue", "Green", "Red", "White"]);
  });
});

describe("a show the viewer cannot make sense of", () => {
  it("is an empty rig, not a throw", () => {
    expect(rigOf(null)).toEqual([]);
    expect(rigOf({})).toEqual([]);
    expect(rigOf({ fixtures: [] })).toEqual([]);
  });

  it("skips what is not a fixture and fills in what a fixture does not say", () => {
    const rig = rigOf({
      fixtures: {
        seven: { typeId: "x" },
        "3": "not an object",
        "4": { typeId: "odd", position: { x: "far", y: 2 } },
      },
      fixtureTypes: {
        odd: {
          physical: { size: { x: 0, y: 0, z: 0 }, beams: [3, { beamAngle: 0, direction: { x: 0, y: 0, z: 0 } }] },
          attributes: [
            "junk",
            { attribute: "Dimmer", occurrence: 1, coarseOffset: 3 },
            { attribute: "Dimmer", coarseOffset: 0, ranges: [7, { from: 1 }] },
            { attribute: "Dimmer", coarseOffset: 9 },
            { attribute: "Focus", coarseOffset: 2 },
          ],
        },
      },
    });
    expect(rig).toHaveLength(1);
    const odd = rig[0];
    expect(odd?.position).toEqual({ x: 0, y: 2, z: 0 });
    expect(odd?.unplaced).toBe(false);
    expect(odd?.size).toEqual(DEFAULT_SIZE);
    // A beam with no angle and no direction is a beam at the default angle,
    // pointing down — never a `NaN` travelling into every point drawn from it.
    expect(odd?.beams).toEqual([
      { position: { x: 0, y: 0, z: 0 }, direction: { x: 0, y: -1, z: 0 }, angle: DEFAULT_BEAM_ANGLE },
    ]);
    // The first dimmer of occurrence nought, and nothing the viewer does not read.
    expect(odd?.channels.Dimmer).toEqual({
      coarse: 0,
      fine: null,
      invert: false,
      from: 0,
      to: 100,
      ranges: [{ name: "", from: 1, to: 0 }],
    });
    expect(Object.keys(odd?.channels ?? {})).toEqual(["Dimmer"]);
  });

  it("gives a described fixture with no beams one out of the bottom of its body", () => {
    const [only] = rigOf({
      fixtures: { "1": { typeId: "t" } },
      fixtureTypes: { t: { physical: { size: { x: 1, y: 2, z: 1 }, beams: [] } } },
    });
    expect(only?.size).toEqual({ x: 1, y: 2, z: 1 });
    expect(only?.beams[0]?.position).toEqual({ x: 0, y: -1, z: 0 });
  });
});
