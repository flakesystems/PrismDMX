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

import type { AttributeType, JsonValue, ProgrammerState } from "../bindings";
import type { ParameterKey } from "./programmer";
import {
  ENCODERS_PER_PAGE,
  bankParameters,
  bankReadings,
  bankRepeats,
  encoderPage,
  groupOf,
  homeOf,
  parameterLabel,
  sourceText,
  touchedBanks,
  valueText,
} from "./programmer";

/**
 * One attribute of one fixture — S52.
 *
 * The first of a kind unless a number is given, which is what `occurrence: 0`
 * means everywhere else: the one there has always been.
 */
function key(attribute: AttributeType, occurrence = 0): ParameterKey {
  return { attribute, occurrence };
}

/** A show with two dimmers and one head, in the shape `prism-core` serialises. */
const SHOW: JsonValue = {
  fixtures: {
    "1": { typeId: "dim" },
    "2": { typeId: "dim" },
    "5": { typeId: "head" },
    // **A colour-only PAR** — four channels of colour and no intensity, which
    // is the fixture the desk supplies one for (S43). Fixture 7 has the switch
    // off, so it has none at all.
    "6": { typeId: "par" },
    "7": { typeId: "par", softwareDimmer: false },
  },
  fixtureTypes: {
    // **The resting values are part of the fixture** since S43: a colour rests
    // open (B1) and everything else rests shut, and an encoder reads them when
    // the programmer is holding nothing.
    dim: { attributes: [{ attribute: "Dimmer", featureGroup: "Dimmer", defaultValue: 0 }] },
    head: {
      attributes: [
        { attribute: "Pan", featureGroup: "Position", defaultValue: 32768 },
        { attribute: "Tilt", featureGroup: "Position", defaultValue: 32768 },
        // A profile that files its **dimmer** on the colour bank. Odd, legal,
        // and the reason the bank an attribute is on is the profile's answer
        // rather than the attribute name's.
        { attribute: "Dimmer", featureGroup: "Color", defaultValue: 65535 },
        // A real colour attribute, resting open — B1's rule, and what the
        // colour readings below are read from.
        { attribute: "Red", featureGroup: "Color", defaultValue: 65535 },
      ],
    },
    par: {
      attributes: [
        { attribute: "Red", featureGroup: "Color", defaultValue: 65535 },
        { attribute: "Green", featureGroup: "Color", defaultValue: 65535 },
      ],
    },
    // **S52's two shapes.** A head with two colour wheels is the ordinary
    // repeat: both knobs belong side by side. A tube with a red per pixel is
    // the other one: six pages of things called *Red* is not a bank, so past
    // `INLINE_OCCURRENCES` the band draws one part at a time.
    twowheel: {
      attributes: [
        { attribute: "ColorWheel", featureGroup: "Color", defaultValue: 0 },
        {
          attribute: "ColorWheel",
          occurrence: 1,
          featureGroup: "Color",
          defaultValue: 0,
          ranges: [
            { name: "Open", from: 0, to: 32767 },
            { name: "Deep blue", from: 32768, to: 65535 },
          ],
        },
      ],
    },
    tube: {
      attributes: [
        { attribute: "Red", featureGroup: "Color", defaultValue: 65535 },
        { attribute: "Red", occurrence: 1, featureGroup: "Color", defaultValue: 65535 },
        { attribute: "Red", occurrence: 2, featureGroup: "Color", defaultValue: 65535 },
        { attribute: "Red", occurrence: 3, featureGroup: "Color", defaultValue: 65535 },
      ],
    },
    // A lamp with a dedicated warm and cold white — the owner's own, and what
    // S52's two new attributes are for.
    cwww: {
      attributes: [
        { attribute: "WarmWhite", featureGroup: "Color", defaultValue: 65535 },
        { attribute: "ColdWhite", featureGroup: "Color", defaultValue: 65535 },
      ],
    },
  },
};

/** The fixtures with repeats, patched beside the four above. */
const REPEATS: JsonValue = {
  ...(SHOW as Record<string, JsonValue>),
  fixtures: {
    ...((SHOW as { fixtures: Record<string, JsonValue> }).fixtures),
    "8": { typeId: "twowheel", softwareDimmer: false },
    "9": { typeId: "tube", softwareDimmer: false },
    "10": { typeId: "cwww", softwareDimmer: false },
  },
};

/** A programmer holding the given values, in the flat wire shape. */
function programmer(
  selection: number[],
  values: [number, AttributeType, number, number?][] = [],
): ProgrammerState {
  return {
    selection,
    selectedGroups: [],
    manualSelection: [],
    activeFeatureGroup: "Dimmer",
    clearStage: 0,
    // The attribute is passed through since S51 rather than folded to one of
    // three: the model has thirty-four of them now (B38), and a helper that
    // silently rewrote *Gobo* as *Dimmer* would make a test of the gobo wheel
    // a test of the dimmer.
    values: values.map(([fixture, attribute, value, occurrence]) => ({
      fixture,
      attribute,
      occurrence: occurrence ?? 0,
      value: { value, source: "Manual", presetRef: null },
    })),
  };
}

describe("what an encoder reads", () => {
  /**
   * **What an untouched attribute reads, and S43 changed it** — the owner's
   * third point.
   *
   * It used to be a dash on the argument that *absent is not zero*. The
   * argument still holds; what changed is that the difference is carried by
   * `overriding` now instead of by the number, so the number is free to say
   * something useful: where the lamp **rests**. These two lamps rest shut, so
   * this one still reads 0 % — and it reads it as *not overriding*, which is the
   * half that keeps the old argument true.
   */
  it("reads the resting value when the programmer holds nothing, and says it is not overriding", () => {
    const [dimmer] = bankReadings(programmer([1, 2]), SHOW, "Dimmer", 0);
    expect(dimmer?.level).toBeNull();
    expect(dimmer?.mixed).toBe(false);
    expect(dimmer?.held).toBe(0);
    expect(dimmer?.available).toBe(2);
    expect(dimmer?.overriding).toBe(false);
    expect(dimmer?.home).toBe(0);
    expect(valueText(dimmer ?? emptyReading())).toBe("0%");
  });

  /**
   * **And this is the one the owner actually reported.** A colour rests open
   * (B1), so its encoder reads 100 % before anybody has touched it and mixing is
   * pulling it *down*. Reading a dash there sent an operator looking for the
   * value at the bottom of the range.
   */
  it("reads a colour at full, because that is where a colour rests", () => {
    const red = bankReadings(programmer([5]), SHOW, "Color", 0).find(
      (reading) => reading.attribute === "Red",
    );
    expect(red?.home).toBe(65535);
    expect(red?.overriding).toBe(false);
    expect(valueText(red ?? emptyReading())).toBe("100%");
  });

  it("draws no encoder at all for a parameter nothing selected has", () => {
    // **S52, and the owner's own ask.** This used to be an encoder reading a
    // dash. Two dimmers have no position at all, so the bank is empty and the
    // band says so in a sentence instead of offering three knobs that do
    // nothing.
    expect(bankReadings(programmer([1, 2]), SHOW, "Position", 0)).toEqual([]);
    expect(bankParameters(programmer([1, 2]), SHOW, "Position", 0)).toEqual([]);
    // A dash is still what `valueText` says for a reading with nothing under
    // it; what changed is that the band no longer makes one.
    expect(valueText(emptyReading())).toBe("—");
  });

  it("reads the resting value over a selection only some of which has the attribute", () => {
    // The dimmer has no red at all, so there is one fixture to read a resting
    // value from and no disagreement to report.
    const red = bankReadings(programmer([1, 5]), SHOW, "Color", 0).find(
      (reading) => reading.attribute === "Red",
    );
    expect(red?.available).toBe(1);
    expect(red?.home).toBe(65535);
    expect(red?.overriding).toBe(false);
  });

  it("marks an attribute the programmer holds as overriding", () => {
    // The mark that took over from the dash — S43, the owner's fourth point.
    // *Overriding* means this value goes out whatever the playbacks say, and it
    // is what a click on an encoder turns on without moving anything.
    const state = programmer([1, 2], [[1, "Dimmer", 32767]]);
    const [dimmer] = bankReadings(state, SHOW, "Dimmer", 0);
    expect(dimmer?.overriding).toBe(true);
    expect(dimmer?.held).toBe(1);
    expect(dimmer?.available).toBe(2);
    // Held by one of two, and the number is that one's rather than the resting
    // value: what is asserted wins over what rests.
    expect(valueText(dimmer ?? emptyReading())).toBe("50%");
  });

  it("shows the value when the whole selection agrees", () => {
    const state = programmer(
      [1, 2],
      [
        [1, "Dimmer", 32767],
        [2, "Dimmer", 32767],
      ],
    );
    const [dimmer] = bankReadings(state, SHOW, "Dimmer", 0);
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
    const [dimmer] = bankReadings(state, SHOW, "Dimmer", 0);
    expect(dimmer?.mixed).toBe(true);
    expect(dimmer?.level).toBeNull();
    expect(dimmer?.held).toBe(2);
    expect(valueText(dimmer ?? emptyReading())).toBe("mixed");
  });

  it("counts what the selection has, not what it holds", () => {
    // Fixture 1 is a dimmer with no pan, so a Position bank over a mixed
    // selection has one fixture that can be panned and one that cannot.
    const readings = bankReadings(programmer([1, 5], [[5, "Pan", 16383]]), SHOW, "Position", 0);
    // **Two, not three** — S52. The position bank has a speed knob on it since
    // S51 (B38) and nothing selected has one, so it is not drawn: a bank shows
    // what the fixtures have.
    expect(readings.map((reading) => reading.attribute)).toEqual(["Pan", "Tilt"]);
    expect(readings[0]?.available).toBe(1);
    expect(readings[0]?.held).toBe(1);
    expect(readings[1]?.available).toBe(1);
    expect(readings[1]?.held).toBe(0);
  });

  it("gives a bank the parameters the selection has, in the generated order", () => {
    // **S52.** The order within a bank is still `FEATURE_GROUP_ATTRIBUTES`,
    // generated from `prism_domain::FeatureGroup::attributes`; what is filtered
    // is which of them the selection actually has.
    const of = (selection: number[], bank: Parameters<typeof bankParameters>[2]) =>
      bankParameters(programmer(selection), SHOW, bank, 0).map(parameterLabel);
    expect(of([1], "Dimmer")).toEqual(["Dimmer"]);
    expect(of([5], "Position")).toEqual(["Pan", "Tilt"]);
    // **The head files its dimmer on the colour bank and the knob does not
    // move** — a distinction that predates S52. Which bank a knob is on is
    // `FEATURE_GROUP_ATTRIBUTES`; what a profile's own `featureGroup` decides
    // is whether the masters may scale the channel and which bank key lights up
    // to say the programmer is holding something.
    expect(of([5], "Color")).toEqual(["Red"]);
    expect(of([5], "Dimmer")).toEqual(["Dimmer"]);
    expect(of([6], "Color")).toEqual(["Red", "Green"]);
    // Nothing selected: no bank has anything, which is an empty band and not a
    // row of dashes.
    expect(of([], "Color")).toEqual([]);
    expect(bankReadings(null, null, "Beam", 0)).toEqual([]);
    // And the indices are the ones the jog wheel counts with, from nought.
    expect(bankReadings(programmer([6]), SHOW, "Color", 0).map((r) => r.index)).toEqual([0, 1]);
  });

  /**
   * **A fixture with two of a parameter gets two knobs** — S52, and the
   * owner's own report.
   *
   * Numbered from the second: the first of a kind is unqualified, because a
   * desk where every knob had a `1` after it would have made a rare case
   * everybody's problem.
   */
  it("numbers a repeated parameter instead of dropping it", () => {
    const readings = bankReadings(programmer([8]), REPEATS, "Color", 0);
    expect(readings.map((reading) => reading.label)).toEqual(["ColorWheel", "ColorWheel 2"]);
    expect(readings.map((reading) => reading.occurrence)).toEqual([0, 1]);
    expect(bankRepeats(programmer([8]), REPEATS, "Color")).toBe(2);
    // The two are separate values: the programmer holds one and not the other.
    const held = bankReadings(
      programmer([8], [[8, "ColorWheel", 65535, 1]]),
      REPEATS,
      "Color",
      0,
    );
    expect(held[0]?.overriding).toBe(false);
    expect(held[1]?.overriding).toBe(true);
    expect(held[1]?.level).toBe(65535);
    // And the ranges are the second wheel's own, so its steps are its own.
    expect(held[1]?.range).toBe("Deep blue");
    expect(held[0]?.ranges).toEqual([]);
  });

  /**
   * **Past a few repeats the bank draws one part at a time** — S52.
   *
   * A tube with a red per pixel would otherwise give the colour bank a page of
   * things called *Red* per pixel and bury the four knobs anybody reaches for.
   * `INLINE_OCCURRENCES` is the line, and it is `prism-domain`'s rather than
   * this side's, because the jog wheel has to agree about it.
   */
  it("draws one part at a time once the repeats go deeper than a few", () => {
    const state = programmer([9]);
    expect(bankRepeats(state, REPEATS, "Color")).toBe(4);
    for (const part of [0, 1, 2, 3]) {
      const readings = bankReadings(state, REPEATS, "Color", part);
      expect(readings.map((reading) => reading.occurrence)).toEqual([part]);
      expect(readings.map((reading) => reading.index)).toEqual([0]);
    }
    // A part past the end draws the last one rather than nothing: a band that
    // went blank because a number was too big would be one an operator cannot
    // get back. The correcting command is the band's.
    expect(bankReadings(state, REPEATS, "Color", 99)[0]?.occurrence).toBe(3);
    expect(bankReadings(state, REPEATS, "Color", -1)[0]?.occurrence).toBe(0);
  });

  /**
   * **A warm white and a cold white are two knobs** — S52, the owner's lamp.
   *
   * They were one attribute before, so the second channel of a lamp with both
   * was dropped and half of it did not answer.
   */
  it("gives a warm white and a cold white a knob each", () => {
    expect(bankParameters(programmer([10]), REPEATS, "Color", 0).map(parameterLabel)).toEqual([
      "WarmWhite",
      "ColdWhite",
    ]);
  });
});

describe("which bank an attribute is on", () => {
  it("is the profile's answer and not the attribute's name", () => {
    // The head files its dimmer under Colour, so a value on it marks *that*
    // bank — which is what `prism_core::Programmer::feature_groups` does.
    expect(groupOf(SHOW, 5, key("Dimmer"))).toBe("Color");
    expect(groupOf(SHOW, 1, key("Dimmer"))).toBe("Dimmer");
    expect(touchedBanks(programmer([5], [[5, "Dimmer", 100]]), SHOW)).toEqual(["Color"]);
  });

  /**
   * **The desk supplies an intensity for a fixture whose profile has none** —
   * S43, and this side of it is the *reading*: the merge already has the slot,
   * and the bar has to find it or an operator has a dimmer they cannot turn.
   *
   * It rests at nought, which is what makes a rig of PARs dark at home now that
   * a colour rests open (B1) — and it is on the intensity bank, so it is under
   * the operator's hands where a dimmer belongs.
   */
  it("supplies an intensity for a fixture whose profile has none", () => {
    expect(groupOf(SHOW, 6, key("Dimmer"))).toBe("Dimmer");
    expect(homeOf(SHOW, 6, key("Dimmer"))).toBe(0);
    // The colour beside it is untouched, and open.
    expect(homeOf(SHOW, 6, key("Red"))).toBe(65535);
    // **And there is exactly one of it** — S52. A desk supplies one intensity,
    // never two, so asking for a second is asking for a channel nobody has.
    expect(groupOf(SHOW, 6, key("Dimmer", 1))).toBeNull();
  });

  it("supplies nothing where the operator switched it off, or where there is one already", () => {
    // Switched off in the patch: the fixture has no intensity at all, which is
    // what an operator asks for when the PAR is on a dimmer pack.
    expect(groupOf(SHOW, 7, key("Dimmer"))).toBeNull();
    expect(homeOf(SHOW, 7, key("Dimmer"))).toBeNull();
    // And a profile that *has* a dimmer keeps its own, wherever it files it —
    // the head above files one on the colour bank, and supplying a second would
    // name the same attribute twice.
    expect(groupOf(SHOW, 5, key("Dimmer"))).toBe("Color");
    expect(homeOf(SHOW, 5, key("Dimmer"))).toBe(65535);
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
    expect(groupOf(SHOW, 99, key("Dimmer"))).toBeNull();
    expect(groupOf({ fixtures: { "1": { typeId: "gone" } } }, 1, key("Dimmer"))).toBeNull();
    expect(groupOf(SHOW, 1, key("Pan"))).toBeNull();
    expect(
      groupOf({ fixtures: { "1": { typeId: "x" } }, fixtureTypes: { x: 7 } }, 1, key("Dimmer")),
    ).toBeNull();
    expect(
      groupOf(
        {
          fixtures: { "1": { typeId: "x" } },
          fixtureTypes: { x: { attributes: [4, { attribute: "Dimmer" }] } },
        },
        1,
        key("Dimmer"),
      ),
    ).toBeNull();
    expect(
      groupOf(
        {
          fixtures: { "1": { typeId: "x" } },
          fixtureTypes: { x: { attributes: [{ attribute: "Dimmer", featureGroup: "Ultra" }] } },
        },
        1,
        key("Dimmer"),
      ),
    ).toBeNull();
    expect(groupOf(null, 1, key("Dimmer"))).toBeNull();
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
    const [dimmer] = bankReadings(programmer([1, 2]), SHOW, "Dimmer", 0);
    expect(dimmer?.source).toBeNull();
    expect(sourceText(dimmer ?? emptyReading())).toBe("");
  });

  it("names the source when the whole selection agrees", () => {
    const [dimmer] = bankReadings(
      programmer([1, 2], [[1, "Dimmer", 32767], [2, "Dimmer", 32767]]),
      SHOW,
      "Dimmer",
      0,
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
    const [dimmer] = bankReadings(mixed, SHOW, "Dimmer", 0);
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

/**
 * A reading of nothing, for the cases above that index into an array.
 *
 * **S43** added the two fields at the bottom: `home` is what the attribute rests
 * at when the programmer holds nothing, and `overriding` says whether the
 * programmer holds it — the mark that took over from the dash. A reading of
 * nothing has neither.
 */
function emptyReading() {
  return {
    attribute: "Dimmer",
    occurrence: 0,
    label: "Dimmer",
    // **S53.** A reading of nothing has no manufacturer's word either, so the
    // encoder falls back to the desk's own — which is what `null` means here.
    name: null,
    index: 0,
    level: null,
    mixed: false,
    held: 0,
    available: 0,
    source: null,
    home: null,
    overriding: false,
    ranges: [],
    range: null,
  } as const;
}

/**
 * **The named ranges a channel carries** — S51, punch-list B38.
 *
 * Read out of the show's own embedded profile (S11), like the resting value
 * beside them, so nothing was added to the protocol for it. What is asserted
 * here is the two rules that are not obvious: only when the whole selection
 * agrees, and never guessed at.
 */
describe("a channel's named ranges", () => {
  const wheel = (ranges: unknown): JsonValue =>
    ({
      fixtures: { "1": { typeId: "spot" }, "2": { typeId: "other" } },
      fixtureTypes: {
        spot: {
          attributes: [{ attribute: "Gobo", featureGroup: "Gobo", defaultValue: 0, ranges }],
        },
        other: {
          attributes: [
            {
              attribute: "Gobo",
              featureGroup: "Gobo",
              defaultValue: 0,
              ranges: [{ name: "Something else", from: 0, to: 65535 }],
            },
          ],
        },
      },
    }) as JsonValue;

  const named = [
    { name: "Open", from: 0, to: 9999 },
    { name: "Gobo 1", from: 10000, to: 65535 },
  ];

  it("names the range the value is standing in", () => {
    const [gobo] = bankReadings(programmer([1], [[1, "Gobo", 30000]]), wheel(named), "Gobo", 0);
    expect(gobo?.ranges).toEqual(named);
    expect(gobo?.range).toBe("Gobo 1");
  });

  it("names nothing when the selected fixtures do not agree on a list", () => {
    // Two heads with different wheels in them: offering one of the two lists
    // would name the wrong slot on half the selection, which is the same rule
    // the resting value follows.
    const [gobo] = bankReadings(programmer([1, 2], []), wheel(named), "Gobo", 0);
    expect(gobo?.ranges).toEqual([]);
    expect(gobo?.range).toBeNull();
  });

  it("names nothing when the values differ", () => {
    const [gobo] = bankReadings(
      programmer([1], [[1, "Gobo", 30000]]),
      wheel(named),
      "Gobo",
      0,
    );
    expect(gobo?.range).toBe("Gobo 1");
    // ...and a mixed reading has no single place to be standing in.
    const mixed = bankReadings(
      programmer([1], [[1, "Gobo", 30000]]),
      wheel(named),
      "Gobo",
      0,
    )[0];
    expect(mixed?.mixed).toBe(false);
  });

  it("has none at all for a profile written before ranges existed", () => {
    // A show that embedded its profiles before S51 carries no `ranges` key, and
    // the fixture behaves exactly as it did — which is the right answer for a
    // show somebody is about to run.
    const [gobo] = bankReadings(programmer([1], []), wheel(undefined), "Gobo", 0);
    expect(gobo?.ranges).toEqual([]);
    expect(gobo?.range).toBeNull();
  });

  it("refuses to guess at a list it cannot make sense of", () => {
    // S26's *do not read the show* rule: a mirror one delta behind a schema
    // change draws an encoder with no names rather than a broken one.
    for (const broken of [
      "not a list",
      [{ name: "Open" }],
      [{ from: 0, to: 10 }],
      [{ name: "Open", from: "nought", to: 10 }],
    ]) {
      const [gobo] = bankReadings(programmer([1], []), wheel(broken), "Gobo", 0);
      expect(gobo?.ranges, JSON.stringify(broken)).toEqual([]);
    }
  });

  it("names nothing for a value in a gap the profile does not describe", () => {
    const [gobo] = bankReadings(
      programmer([1], [[1, "Gobo", 40000]]),
      wheel([{ name: "Open", from: 0, to: 9999 }]),
      "Gobo",
      0,
    );
    expect(gobo?.range).toBeNull();
  });
});

/**
 * **What the manufacturer calls the channel** — S53, `AttributeDef::label`.
 *
 * Read out of the show's own embedded profile like the resting value and the
 * ranges beside it, and it follows the **same two rules**: only when the whole
 * selection agrees, and never guessed at. It is a **label and not a key** — both
 * of the fixtures below are `Gobo`, so a preset made on one plays back on the
 * other, which is the whole reason the key stayed a closed enum.
 */
describe("the name the manufacturer gave a channel", () => {
  const named = (mine: unknown, theirs: unknown): JsonValue =>
    ({
      fixtures: { "1": { typeId: "mine" }, "2": { typeId: "theirs" } },
      fixtureTypes: {
        mine: {
          attributes: [
            { attribute: "Gobo", featureGroup: "Gobo", defaultValue: 0, label: mine },
          ],
        },
        theirs: {
          attributes: [
            { attribute: "Gobo", featureGroup: "Gobo", defaultValue: 0, label: theirs },
          ],
        },
      },
    }) as JsonValue;

  it("reads it off the profile the show embedded", () => {
    const [gobo] = bankReadings(
      programmer([1], []),
      named("Rotating Gobo", "Gobo Wheel"),
      "Gobo",
      0,
    );
    expect(gobo?.name).toBe("Rotating Gobo");
    // The desk's own word is still there underneath it, and it is the word the
    // key is spelled with.
    expect(gobo?.label).toBe("Gobo");
    expect(gobo?.attribute).toBe("Gobo");
  });

  it("falls back to the desk's own word when the selection disagrees", () => {
    // Naming one of the two would be wrong about the other half of the
    // selection, which is the rule `home` and `ranges` already follow.
    const [gobo] = bankReadings(
      programmer([1, 2], []),
      named("Rotating Gobo", "Gobo Wheel"),
      "Gobo",
      0,
    );
    expect(gobo?.name).toBeNull();
  });

  it("keeps it when the selection agrees, however they were patched", () => {
    const [gobo] = bankReadings(
      programmer([1, 2], []),
      named("Rotating Gobo", "Rotating Gobo"),
      "Gobo",
      0,
    );
    expect(gobo?.name).toBe("Rotating Gobo");
  });

  it("has none for a profile embedded before S53, or for a generic one", () => {
    const [gobo] = bankReadings(programmer([1], []), named(undefined, undefined), "Gobo", 0);
    expect(gobo?.name).toBeNull();
  });

  it("refuses to read anything that is not a name", () => {
    // S26's *do not read the show* rule: a mirror one delta behind a schema
    // change shows the desk's word rather than a number or an object.
    for (const broken of [7, null, { name: "Gobo" }, ["Gobo"]]) {
      const [gobo] = bankReadings(programmer([1], []), named(broken, broken), "Gobo", 0);
      expect(gobo?.name, JSON.stringify(broken)).toBeNull();
    }
  });
});
