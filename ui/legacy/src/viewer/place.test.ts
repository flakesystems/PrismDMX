/**
 * The two gestures that hang a rig: what each sends, fixture by fixture.
 */

import { describe, expect, it } from "vitest";

import { finalShow } from "../testing/viewer-recording";
import {
  BLANK,
  DEFAULT_SPACING,
  NO_TRACKER,
  fieldsAreNumbers,
  fieldsOf,
  mirrorChoice,
  placeSet,
  placeSpread,
  readField,
  selectedFixtures,
  trackerIsValid,
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

describe("a head whose motor runs the other way - S32", () => {
  const flipped = (pan: boolean, tilt: boolean) =>
    rig.map((fixture) => (fixture.id === 1 ? { ...fixture, mirror: { pan, tilt } } : fixture));

  it("is not sent by a place that mirrors nothing", () => {
    for (const place of placeSet(selectedFixtures(rig, [1, 2]), BLANK)) {
      expect(place).not.toHaveProperty("mirror");
    }
  });

  it("starts the form from what the fixture is, and from what a selection agrees about", () => {
    const [first] = selectedFixtures(flipped(true, false), [1]);
    expect(fieldsOf(first)).toMatchObject({ mirrorPan: "on", mirrorTilt: "off" });
    const both = selectedFixtures(flipped(true, false), [1, 2]);
    // Fixture 1 mirrors pan and fixture 2 does not: they disagree, so the box
    // says *keep each its own* by being blank.
    expect(mirrorChoice(both, "pan")).toBe("");
    expect(mirrorChoice(both, "tilt")).toBe("off");
    expect(mirrorChoice([], "pan")).toBe("");
  });

  it("is sent with the choice, and a blank choice keeps what each fixture has", () => {
    const both = selectedFixtures(flipped(true, false), [1, 2]);
    // Blank: fixture 1 keeps its mirrored pan and fixture 2 stays plain.
    const kept = placeSet(both, BLANK);
    expect(kept[0]?.mirror).toEqual({ pan: true, tilt: false });
    expect(kept[1]).not.toHaveProperty("mirror");
    // On: every selected fixture, whatever it was.
    const on = placeSet(both, { ...BLANK, mirrorTilt: "on" });
    expect(on.map((place) => place.mirror)).toEqual([
      { pan: true, tilt: true },
      { pan: false, tilt: true },
    ]);
    // Off takes it away, and a place with neither says nothing at all.
    const off = placeSet(both, { ...BLANK, mirrorPan: "off" });
    expect(off[0]).not.toHaveProperty("mirror");
  });

  it("is not mistaken for a number by the form's check", () => {
    expect(fieldsAreNumbers({ ...BLANK, mirrorPan: "on", mirrorTilt: "off" })).toBe(true);
  });
});

describe("a tracker to follow - S32", () => {
  const chosen = () => selectedFixtures(rig, [1, 2]);

  it("is left alone where the field is blank", () => {
    // Neither fixture follows anything, and nothing was typed.
    for (const place of placeSet(chosen(), BLANK)) {
      expect(place).not.toHaveProperty("follow");
    }
  });

  it("is given with the typed number and the offset that was typed over zero", () => {
    const placed = placeSet(chosen(), { ...BLANK, tracker: "5", oy: "0,4" });
    expect(placed.map((place) => place.follow)).toEqual([
      { tracker: 5, offset: { x: 0, y: 0.4, z: 0 } },
      { tracker: 5, offset: { x: 0, y: 0.4, z: 0 } },
    ]);
  });

  it("is kept by a gesture that does not mention it, and an offset keeps the others", () => {
    const following = rig.map((fixture) =>
      fixture.id === 1
        ? { ...fixture, follow: { tracker: 7, offset: { x: 0.1, y: 0.2, z: 0.3 } } }
        : fixture,
    );
    const [first] = selectedFixtures(following, [1]);
    // A drag moves the place and says nothing about the tracker: it stays.
    expect(placeSet([first!], { ...BLANK, y: "9" })[0]?.follow).toEqual({
      tracker: 7,
      offset: { x: 0.1, y: 0.2, z: 0.3 },
    });
    // Typing one component of the offset changes that one.
    expect(placeSet([first!], { ...BLANK, oy: "1" })[0]?.follow).toEqual({
      tracker: 7,
      offset: { x: 0.1, y: 1, z: 0.3 },
    });
    // And the tracker alone moves it to another performer, offset kept.
    expect(placeSet([first!], { ...BLANK, tracker: "8" })[0]?.follow).toEqual({
      tracker: 8,
      offset: { x: 0.1, y: 0.2, z: 0.3 },
    });
  });

  it("is taken away by a dash, and an offset alone does not give a fixture a tracker", () => {
    const following = rig.map((fixture) => ({
      ...fixture,
      follow: { tracker: 3, offset: { x: 0, y: 0, z: 0 } },
    }));
    const [first] = selectedFixtures(following, [1]);
    expect(placeSet([first!], { ...BLANK, tracker: NO_TRACKER })[0]).not.toHaveProperty("follow");
    // Nobody follows anything, so there is no performer for an offset to be part of.
    expect(placeSet(chosen(), { ...BLANK, oy: "1" })[0]).not.toHaveProperty("follow");
  });

  it("is a whole number from nought to the last tracker, or blank, or a dash", () => {
    for (const good of ["", " ", "0", "1023", "-"]) {
      expect(trackerIsValid(good), good).toBe(true);
    }
    for (const bad of ["1024", "-1", "1.5", "anna", "3 4"]) {
      expect(trackerIsValid(bad), bad).toBe(false);
    }
    expect(fieldsAreNumbers({ ...BLANK, tracker: "9999" })).toBe(false);
    expect(fieldsAreNumbers({ ...BLANK, tracker: "12", oy: "0.4" })).toBe(true);
  });

  it("starts the form from the tracker the first fixture has", () => {
    const following = rig.map((fixture) =>
      fixture.id === 2
        ? { ...fixture, follow: { tracker: 4, offset: { x: 0, y: 0.5, z: 0 } } }
        : fixture,
    );
    const fields = fieldsOf(following.find((fixture) => fixture.id === 2));
    expect(fields).toMatchObject({ tracker: "4", ox: "0", oy: "0.5", oz: "0" });
    expect(fieldsOf(following.find((fixture) => fixture.id === 1))).toMatchObject({
      tracker: "",
      oy: "",
    });
  });
});
