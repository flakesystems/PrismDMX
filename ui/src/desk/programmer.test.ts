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
import {
  ENCODERS_PER_PAGE,
  bankParameters,
  bankReadings,
  encoderPage,
  groupOf,
  sourceText,
  touchedBanks,
  valueText,
} from "./programmer";

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

/**
 * The page arithmetic on its own, for the cases a bank cannot produce.
 *
 * Every generated feature group has at least one attribute, so an *empty* bank
 * and a fractional page number are only reachable from a document — and a
 * document is not a promise.
 */
describe("paging a bank", () => {
  /** `n` readings, distinguishable by index. */
  const readings = (n: number) =>
    Array.from({ length: n }, (_, index) => ({ ...emptyReading(), index }));

  it("is one page for a bank that fits, and never nought pages", () => {
    expect(encoderPage(readings(1), 0)).toMatchObject({ page: 0, pages: 1 });
    expect(encoderPage(readings(ENCODERS_PER_PAGE), 0)).toMatchObject({ page: 0, pages: 1 });
    // An empty bank is one empty page, not no page at all: `pages: 0` would
    // make `pages - 1` negative and disable nothing.
    expect(encoderPage([], 0)).toEqual({ readings: [], page: 0, pages: 1 });
  });

  it("splits a bank into pages of four, last page short", () => {
    const six = encoderPage(readings(6), 0);
    expect(six.pages).toBe(2);
    expect(six.readings.map((reading) => reading.index)).toEqual([0, 1, 2, 3]);
    expect(encoderPage(readings(6), 1).readings.map((reading) => reading.index)).toEqual([4, 5]);
    // Exactly two full pages, rather than three with an empty one.
    expect(encoderPage(readings(8), 0).pages).toBe(2);
    expect(encoderPage(readings(9), 0).pages).toBe(3);
  });

  it("clamps a page number the bank cannot honour", () => {
    expect(encoderPage(readings(6), 99).page).toBe(1);
    expect(encoderPage(readings(6), -3).page).toBe(0);
    expect(encoderPage(readings(6), 1.9).page).toBe(1);
  });
});

/**
 * Where a value came from, which the encoder bar got the room to show when S35
 * put the two bars in one band.
 *
 * It matters before a store rather than after one: `presetRef` is what keeps a
 * preset link alive through `StoreCue` (S13), so *this encoder is on a preset*
 * is the thing an operator would otherwise only discover once the cue was
 * written.
 */
describe("where a value came from", () => {
  it("says nothing for an encoder holding nothing", () => {
    const [dimmer] = bankReadings(programmer([1, 2]), SHOW, "Dimmer");
    expect(dimmer?.source).toBeNull();
    expect(sourceText(dimmer ?? emptyReading())).toBe("");
  });

  it("names the source when the whole selection agrees", () => {
    const [dimmer] = bankReadings(
      programmer([1, 2], [[1, "Dimmer", 32767], [2, "Dimmer", 32767]]),
      SHOW,
      "Dimmer",
    );
    // `programmer()` builds manual values, which is what an encoder turn makes.
    expect(dimmer?.source).toBe("Manual");
    expect(sourceText(dimmer ?? emptyReading())).toBe("man");
  });

  it("says `~` when the selection holds values from different places", () => {
    // Fixture 1's dimmer came from an encoder and fixture 2's from a preset.
    // Naming one of them would be picking a winner, which is `valueText`'s rule
    // for the value itself.
    const state = programmer([1, 2], [[1, "Dimmer", 32767]]);
    const mixed = {
      ...state,
      values: [
        ...state.values,
        {
          fixture: 2,
          attribute: "Dimmer" as const,
          value: { value: 32767, source: "Preset" as const, presetRef: 4 },
        },
      ],
    };
    const [dimmer] = bankReadings(mixed, SHOW, "Dimmer");
    expect(dimmer?.held).toBe(2);
    expect(dimmer?.source).toBeNull();
    expect(sourceText(dimmer ?? emptyReading())).toBe("~");
  });

  it("names each source with a word of its own", () => {
    for (const [source, text] of [
      ["Manual", "man"],
      ["Preset", "preset"],
      ["Recalled", "cue"],
      [null, "~"],
    ] as const) {
      expect(sourceText({ ...emptyReading(), held: 1, source })).toBe(text);
    }
  });
});

/** A reading of nothing, for the cases above that index into an array. */
function emptyReading() {
  return {
    attribute: "Dimmer",
    index: 0,
    level: null,
    mixed: false,
    held: 0,
    available: 0,
    source: null,
  } as const;
}
