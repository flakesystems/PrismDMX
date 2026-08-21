/**
 * *Is this already there* — the one question the desk asks the mirror before it
 * sends a line (S40).
 *
 * The answer only ever decides whether a **prompt** appears, so what matters is
 * that it is right about all six numbered things and that it is `false` rather
 * than a throw for anything it cannot see. A `false` sends the line and lets the
 * daemon refuse it, which is S26's split; a throw would take the console down.
 */

import { describe, expect, it } from "vitest";

import type { JsonValue, ObjectRef } from "../bindings";
import { objectExists } from "./exists";

const SHOW: JsonValue = {
  sequences: {
    "1": { id: 1, name: "Act 1", cues: [{ number: "1" }, { number: "2.5" }] },
    "2": { id: 2, name: "Act 2", cues: "not a list" },
  },
  groups: { "3": { id: 3, name: "Front" } },
  presets: { "4": { id: 4, name: "Deep blue" } },
  executors: { "0": { id: 0, sequenceId: 1 } },
};

const SESSION: JsonValue = {
  session: { selectedSequence: 1 },
  views: { "1": { id: 1, name: "View 1", windows: [] } },
};

describe("the five show pools and the one session pool", () => {
  it("answers for each of the six kinds a line can name", () => {
    const cases: readonly (readonly [ObjectRef, boolean])[] = [
      [{ t: "Sequence", sequenceId: 1 }, true],
      [{ t: "Sequence", sequenceId: 9 }, false],
      [{ t: "Group", groupId: 3 }, true],
      [{ t: "Group", groupId: 9 }, false],
      [{ t: "Preset", presetId: 4 }, true],
      [{ t: "Preset", presetId: 9 }, false],
      [{ t: "Executor", executorId: 0 }, true],
      [{ t: "Executor", executorId: 9 }, false],
      [{ t: "View", viewId: 1 }, true],
      [{ t: "View", viewId: 9 }, false],
      [{ t: "Cue", sequenceId: 1, cueNumber: "1" }, true],
      [{ t: "Cue", sequenceId: 1, cueNumber: "9" }, false],
    ];
    for (const [target, expected] of cases) {
      expect(objectExists(SHOW, SESSION, target), JSON.stringify(target)).toBe(expected);
    }
  });

  /**
   * A cue number is compared **trimmed**, because the command line hands a word
   * over with the spacing it was typed and `prism_core` trims it too.
   */
  it("matches a cue number the way the daemon does", () => {
    expect(objectExists(SHOW, SESSION, { t: "Cue", sequenceId: 1, cueNumber: " 2.5 " })).toBe(true);
  });

  /**
   * A cue that names no list means the **selected** one (§4.1) — and on a desk
   * with none selected there is nothing it could already be, so the line goes
   * out to be refused rather than being asked about.
   */
  it("reads a bare cue number against the selected cue list", () => {
    expect(objectExists(SHOW, SESSION, { t: "Cue", sequenceId: null, cueNumber: "1" })).toBe(true);
    expect(objectExists(SHOW, null, { t: "Cue", sequenceId: null, cueNumber: "1" })).toBe(false);
  });

  /**
   * Nothing here may throw. The mirror is a `JsonValue` the daemon sent, and a
   * console that fell over on a shape it did not expect would take the whole
   * desk's one input with it.
   */
  it("says no rather than throwing when the documents are not there", () => {
    const targets: readonly ObjectRef[] = [
      { t: "Sequence", sequenceId: 1 },
      { t: "Cue", sequenceId: 2, cueNumber: "1" },
      { t: "Group", groupId: 3 },
      { t: "Preset", presetId: 4 },
      { t: "View", viewId: 1 },
      { t: "Executor", executorId: 0 },
    ];
    for (const target of targets) {
      expect(objectExists(null, null, target), JSON.stringify(target)).toBe(false);
    }
    // A cue list whose `cues` is not an array — the same answer, no throw.
    expect(objectExists(SHOW, SESSION, { t: "Cue", sequenceId: 2, cueNumber: "1" })).toBe(false);
  });
});
