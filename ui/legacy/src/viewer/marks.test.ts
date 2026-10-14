/**
 * Stage marks as the calibration window reads and writes them - B70.
 */

import { describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";
import {
  BLANK_MARK,
  MAX_MARK_NAME,
  fieldsOfMark,
  markFieldsAreValid,
  markLabel,
  markOfFields,
  marksOf,
  nextMarkId,
  removeMarks,
  setMark,
} from "./marks";

const show: JsonValue = {
  marks: {
    "10": { id: 10, name: "", position: { x: 1, y: 0, z: 2 } },
    "2": { id: 2, name: "DS centre", position: { x: 0, y: 0, z: -3.5 } },
    // Things a hand-edited file might hold: each is skipped, none throws.
    "x": { id: 0, name: "not a number", position: { x: 0, y: 0, z: 0 } },
    "5": { id: 5, name: "no place" },
    "6": { id: 6, name: "no height", position: { x: 1, z: 1 } },
    "7": 7,
  },
};

describe("the marks of a show", () => {
  it("are read in number order, and what is not a mark is left out", () => {
    expect(marksOf(show).map((mark) => mark.id)).toEqual([2, 10]);
    expect(marksOf(show)[0]).toEqual({
      id: 2,
      name: "DS centre",
      position: { x: 0, y: 0, z: -3.5 },
    });
  });

  it("are none for a show from before there were any", () => {
    expect(marksOf({})).toEqual([]);
    expect(marksOf(null)).toEqual([]);
    expect(marksOf({ marks: [] })).toEqual([]);
  });

  it("get a name on a button even while they have none", () => {
    expect(markLabel({ id: 3, name: "" })).toBe("Mark 3");
    expect(markLabel({ id: 3, name: "  " })).toBe("Mark 3");
    expect(markLabel({ id: 3, name: "Spike" })).toBe("Spike");
  });

  it("number a new one above every one there is, so it never replaces one", () => {
    expect(nextMarkId([])).toBe(1);
    expect(nextMarkId(marksOf(show))).toBe(11);
  });
});

describe("the mark form", () => {
  it("reads three numbers and a name, a comma as a decimal point", () => {
    expect(markOfFields(4, { name: " Spike ", x: "1,5", y: "0", z: "-2.25" })).toEqual({
      id: 4,
      name: "Spike",
      position: { x: 1.5, y: 0, z: -2.25 },
    });
  });

  it("is not a mark while a number is blank - a height never typed is not a floor", () => {
    expect(markOfFields(1, { ...BLANK_MARK, x: "1", z: "2" })).toBeNull();
    expect(markFieldsAreValid({ name: "", x: "1", y: "", z: "2" })).toBe(false);
  });

  it("is not a mark while a number is not one, or the name is too long", () => {
    expect(markFieldsAreValid({ name: "", x: "1", y: "up", z: "2" })).toBe(false);
    expect(markFieldsAreValid({ name: "", x: "Infinity", y: "0", z: "2" })).toBe(false);
    expect(
      markFieldsAreValid({ name: "n".repeat(MAX_MARK_NAME + 1), x: "1", y: "0", z: "2" }),
    ).toBe(false);
    expect(markFieldsAreValid({ name: "n".repeat(MAX_MARK_NAME), x: "1", y: "0", z: "2" })).toBe(
      true,
    );
  });

  it("starts an edit from the mark as it stands, to the millimetre", () => {
    expect(fieldsOfMark({ id: 1, name: "A", position: { x: 1.23456, y: 0, z: -2 } })).toEqual({
      name: "A",
      x: "1.235",
      y: "0",
      z: "-2",
    });
  });
});

describe("the commands", () => {
  it("are one EditMarks each, whatever they touch", () => {
    const mark = { id: 1, name: "A", position: { x: 0, y: 0, z: 0 } };
    expect(setMark(mark)).toEqual({ t: "EditMarks", set: [mark], remove: [] });
    expect(removeMarks([1, 2])).toEqual({ t: "EditMarks", set: [], remove: [1, 2] });
  });
});
