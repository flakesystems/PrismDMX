/**
 * What one encoder reads, in the cases a recorded script does not reach.
 *
 * The script in `desk-recording.json` is a desk being operated and it is what
 * the readers are *held to* (`session.test.ts`). This file is the awkward
 * corners beside it: a selection holding two different values, a fixture whose
 * profile has not got the attribute, a show that has been edited by hand. All
 * of them are ordinary states on a real desk and none of them is worth a
 * recorded daemon script of its own.
 */

import { describe, expect, it } from "vitest";

import type { JsonValue, ProgrammerState } from "../bindings";
import { bankParameters, bankReadings, groupOf, touchedBanks, valueText } from "./programmer";

/** A show with two dimmers and one head, in the shape `prism-core` serialises. */
const SHOW: JsonValue = {
  fixtures: {
    "1": { typeId: "dim" },
    "2": { typeId: "dim" },
    "5": { typeId: "head" },
  },
  fixtureTypes: {
    dim: { attributes: [{ attribute: "Dimmer", featureGroup: "Dimmer" }] },
    head: {
      attributes: [
        { attribute: "Pan", featureGroup: "Position" },
        { attribute: "Tilt", featureGroup: "Position" },
        // A profile that files its **dimmer** on the colour bank. Odd, legal,
        // and the reason the bank an attribute is on is the profile's answer
        // rather than the attribute name's.
        { attribute: "Dimmer", featureGroup: "Color" },
      ],
    },
  },
};

/** A programmer holding the given values, in the flat wire shape. */
function programmer(
  selection: number[],
  values: [number, string, number][] = [],
): ProgrammerState {
  return {
    selection,
    activeFeatureGroup: "Dimmer",
    clearStage: 0,
    values: values.map(([fixture, attribute, value]) => ({
      fixture,
      // The recording's own vocabulary; a test file may know what it built.
      attribute: attribute === "Pan" ? "Pan" : attribute === "Tilt" ? "Tilt" : "Dimmer",
      value: { value, source: "Manual", presetRef: null },
    })),
  };
}

describe("what an encoder reads", () => {
  it("says nothing at all for an attribute nobody has touched", () => {
    const [dimmer] = bankReadings(programmer([1, 2]), SHOW, "Dimmer");
    expect(dimmer?.level).toBeNull();
    expect(dimmer?.mixed).toBe(false);
    expect(dimmer?.held).toBe(0);
    expect(dimmer?.available).toBe(2);
    expect(valueText(dimmer ?? emptyReading())).toBe("—");
  });

  it("shows the value when the whole selection agrees", () => {
    const state = programmer(
      [1, 2],
      [
        [1, "Dimmer", 32767],
        [2, "Dimmer", 32767],
      ],
    );
    const [dimmer] = bankReadings(state, SHOW, "Dimmer");
    expect(dimmer?.level).toBe(32767);
    expect(dimmer?.held).toBe(2);
    expect(valueText(dimmer ?? emptyReading())).toBe("50%");
  });

  it("says `mixed` rather than averaging two values nobody set", () => {
    const state = programmer(
      [1, 2],
      [
        [1, "Dimmer", 0],
        [2, "Dimmer", 65535],
      ],
    );
    const [dimmer] = bankReadings(state, SHOW, "Dimmer");
    expect(dimmer?.mixed).toBe(true);
    expect(dimmer?.level).toBeNull();
    expect(dimmer?.held).toBe(2);
    expect(valueText(dimmer ?? emptyReading())).toBe("mixed");
  });

  it("counts what the selection has, not what it holds", () => {
    // Fixture 1 is a dimmer with no pan, so a Position bank over a mixed
    // selection has one fixture that can be panned and one that cannot.
    const readings = bankReadings(programmer([1, 5], [[5, "Pan", 16383]]), SHOW, "Position");
    expect(readings.map((reading) => reading.attribute)).toEqual(["Pan", "Tilt"]);
    expect(readings[0]?.available).toBe(1);
    expect(readings[0]?.held).toBe(1);
    expect(readings[1]?.available).toBe(1);
    expect(readings[1]?.held).toBe(0);
  });

  it("gives every bank its parameters in the generated order", () => {
    expect(bankParameters("Dimmer")).toEqual(["Dimmer"]);
    expect(bankParameters("Position")).toEqual(["Pan", "Tilt"]);
    expect(bankParameters("Focus")).toEqual(["Focus"]);
    expect(bankReadings(null, null, "Beam").map((reading) => reading.index)).toEqual([
      0, 1, 2, 3, 4, 5,
    ]);
  });
});

describe("which bank an attribute is on", () => {
  it("is the profile's answer and not the attribute's name", () => {
    // The head files its dimmer under Colour, so a value on it marks *that*
    // bank — which is what `prism_core::Programmer::feature_groups` does.
    expect(groupOf(SHOW, 5, "Dimmer")).toBe("Color");
    expect(groupOf(SHOW, 1, "Dimmer")).toBe("Dimmer");
    expect(touchedBanks(programmer([5], [[5, "Dimmer", 100]]), SHOW)).toEqual(["Color"]);
  });

  it("marks the banks in bank order, however the values were set", () => {
    const state = programmer(
      [1, 5],
      [
        [5, "Pan", 1],
        [1, "Dimmer", 1],
      ],
    );
    expect(touchedBanks(state, SHOW)).toEqual(["Dimmer", "Position"]);
  });

  it("answers nothing for what the show cannot resolve", () => {
    // Each of these is an ordinary state: an unpatched fixture, a profile that
    // has gone, a mode without that attribute, and a hand-edited file.
    expect(groupOf(SHOW, 99, "Dimmer")).toBeNull();
    expect(groupOf({ fixtures: { "1": { typeId: "gone" } } }, 1, "Dimmer")).toBeNull();
    expect(groupOf(SHOW, 1, "Pan")).toBeNull();
    expect(groupOf({ fixtures: { "1": { typeId: "x" } }, fixtureTypes: { x: 7 } }, 1, "Dimmer")).toBeNull();
    expect(
      groupOf(
        {
          fixtures: { "1": { typeId: "x" } },
          fixtureTypes: { x: { attributes: [4, { attribute: "Dimmer" }] } },
        },
        1,
        "Dimmer",
      ),
    ).toBeNull();
    expect(
      groupOf(
        {
          fixtures: { "1": { typeId: "x" } },
          fixtureTypes: { x: { attributes: [{ attribute: "Dimmer", featureGroup: "Ultra" }] } },
        },
        1,
        "Dimmer",
      ),
    ).toBeNull();
    expect(groupOf(null, 1, "Dimmer")).toBeNull();
    // A value the show cannot resolve marks no bank at all — S6 drops it on the
    // way into the engine, and a knob that did nothing would be worse.
    expect(touchedBanks(programmer([99], [[99, "Dimmer", 1]]), SHOW)).toEqual([]);
  });
});

/** A reading of nothing, for the cases above that index into an array. */
function emptyReading() {
  return { attribute: "Dimmer", index: 0, level: null, mixed: false, held: 0, available: 0 } as const;
}
