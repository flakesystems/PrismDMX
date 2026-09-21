/**
 * The two gestures that hang a rig: what each sends, fixture by fixture.
 */

import { describe, expect, it } from "vitest";

import { finalShow } from "../testing/viewer-recording";
import {
  BLANK,
  DEFAULT_SPACING,
  fieldsAreNumbers,
  fieldsOf,
  placeSet,
  placeSpread,
  readField,
  selectedFixtures,
} from "./place";
import { rigOf } from "./rig";

const rig = rigOf(finalShow());

describe("a typed number", () => {
  it("is blank, a number with either decimal mark, or not a number", () => {
    expect(readField("  ")).toBeNull();
    expect(readField("2,5")).toBe(2.5);
    expect(readField("-6")).toBe(-6);
    expect(readField("six")).toBeNaN();
    expect(fieldsAreNumbers({ ...BLANK, x: "1", y: "" })).toBe(true);
    expect(fieldsAreNumbers({ ...BLANK, z: "far" })).toBe(false);
  });
});

describe("the form", () => {
  it("starts from where the first selected fixture hangs", () => {
    const fields = fieldsOf(rig.find((fixture) => fixture.id === 2));
    expect(fields).toMatchObject({ x: "2", y: "6", z: "1", rx: "20", ry: "0", rz: "0", spacing: "" });
    expect(fieldsOf(undefined)).toEqual(BLANK);
  });

  it("works on the selected fixtures that are patched, in selection order, once each", () => {
    expect(selectedFixtures(rig, [10, 99, 1, 10]).map((fixture) => fixture.id)).toEqual([10, 1]);
  });
});

describe("Set", () => {
  it("puts every fixture at the typed numbers and keeps what is blank", () => {
    const chosen = selectedFixtures(rig, [1, 2]);
    const placed = placeSet(chosen, { ...BLANK, y: "7,5", rx: "45" });
    expect(placed).toEqual([
      { id: 1, position: { x: -2, y: 7.5, z: 1 }, rotation: { x: 45, y: 0, z: 0 } },
      { id: 2, position: { x: 2, y: 7.5, z: 1 }, rotation: { x: 45, y: 0, z: 0 } },
    ]);
  });
});

describe("Spread", () => {
  it("lays them across the stage in selection order, centred on X, a gap apart", () => {
    const chosen = selectedFixtures(rig, [10, 2, 1]);
    const placed = placeSpread(chosen, { ...BLANK, x: "1", y: "5", spacing: "2" });
    expect(placed.map((place) => [place.id, place.position.x, place.position.y])).toEqual([
      [10, -1, 5],
      [2, 1, 5],
      [1, 3, 5],
    ]);
  });

  it("centres on nought a metre apart when nothing is typed", () => {
    const placed = placeSpread(selectedFixtures(rig, [1, 2]), BLANK);
    expect(placed.map((place) => place.position.x)).toEqual([-DEFAULT_SPACING / 2, DEFAULT_SPACING / 2]);
    // Everything else as the fixtures have it.
    expect(placed[0]?.position.y).toBe(6);
    expect(placed[1]?.rotation).toEqual({ x: 20, y: 0, z: 0 });
  });
});
