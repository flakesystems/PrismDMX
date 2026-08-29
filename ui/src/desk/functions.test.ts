/**
 * **What an executor's controls can be made to do, read out of a document** —
 * S45.
 *
 * `ExecutorButtonFunction` stopped being a union of strings when the custom row
 * arrived: eight fixed functions and a ninth carrying a line an operator wrote.
 * Four places read one, so the reading lives in one module — and it is
 * **narrowed rather than asserted**, because a mirror may be one schema change
 * behind and `CLAUDE.md` forbids the `as` that would paper over it.
 */

import { describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";

import {
  FIXED_BUTTON_FUNCTIONS,
  buttonFunctionOf,
  buttonLabel,
  buttonName,
  commandLineOf,
  encoderName,
  faderName,
  isEncoderFunction,
  isFaderFunction,
  isFixedButtonFunction,
} from "./functions";

describe("what a control does", () => {
  it("is a table generated from Rust, and the custom row is not in it", () => {
    // The eight are `prism_domain::ExecutorButtonFunction::ALL`, which is what
    // makes a function added in Rust appear in the chooser without a second
    // list being edited. The ninth is a choice **plus a line**, so it is a row
    // an operator fills in rather than an entry in a list.
    expect(FIXED_BUTTON_FUNCTIONS).toEqual([
      "Empty",
      "Go+",
      "Go-",
      "LearnSpeed",
      "Off",
      "On",
      "Flash",
      "Toggle",
    ]);
    expect(isFixedButtonFunction("Flash")).toBe(true);
    expect(isFixedButtonFunction("Hologram")).toBe(false);
    expect(isFaderFunction("XFade")).toBe(true);
    expect(isFaderFunction("Master")).toBe(true);
    expect(isFaderFunction("Hologram")).toBe(false);
    expect(isEncoderFunction("Speed")).toBe(true);
    // A fader function an encoder has not got.
    expect(isEncoderFunction("XFade")).toBe(false);
  });

  it("reads one out of a document, and refuses everything it cannot make sense of", () => {
    expect(buttonFunctionOf("Go+")).toBe("Go+");
    expect(buttonFunctionOf({ CommandLine: { line: "Page 2" } })).toEqual({
      CommandLine: { line: "Page 2" },
    });
    const nonsenses: readonly JsonValue[] = [
      null,
      7,
      "Hologram",
      [],
      {},
      { CommandLine: null },
      { CommandLine: [] },
      { CommandLine: { line: 7 } },
    ];
    for (const nonsense of nonsenses) {
      expect(buttonFunctionOf(nonsense), JSON.stringify(nonsense)).toBeNull();
    }
  });

  it("says what a control does in the two lengths a desk needs", () => {
    // Three characters on a strip, because that is the key; the words in the
    // editor, because that is a list somebody is reading.
    expect(buttonLabel("Go+")).toBe("Go");
    expect(buttonLabel("LearnSpeed")).toBe("Lrn");
    expect(buttonLabel({ CommandLine: { line: "Page 2" } })).toBe("Cmd");
    expect(buttonName("Go-")).toBe("Go back");
    expect(buttonName({ CommandLine: { line: "Page 2" } })).toBe("Command line");
    expect(faderName("XFade")).toBe("Crossfade");
    expect(encoderName("Empty")).toBe("Nothing");
  });

  it("hands back the line a custom row sends, and nothing for the eight", () => {
    expect(commandLineOf({ CommandLine: { line: "Go+ Sequence 3" } })).toBe("Go+ Sequence 3");
    expect(commandLineOf("Flash")).toBeNull();
  });
});
