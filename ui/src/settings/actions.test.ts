/**
 * The binding vocabulary, both ways round — S38.
 *
 * The point of this file is the **correspondence**: `ACTION_KINDS` is the
 * flattened list an operator picks from and `SurfaceAction` is what travels, so
 * `actionOfKind` and `kindOf` have to be inverse over every kind. A kind that
 * cannot be built would draw a chooser entry that does nothing; an action that
 * cannot be named would draw a blank row for a key that is bound. Neither is
 * visible without walking the whole list, so the whole list is walked.
 */

import { describe, expect, it } from "vitest";

import type { SurfaceAction } from "../bindings";
import { ACTION_KINDS, actionOfKind, actionText, kindOf, sameControl, slotOf } from "./actions";
import type { ActionKind } from "./actions";

/** The one extra answer a kind needs, where it needs one. */
const DETAIL: Partial<Record<ActionKind, string>> = {
  "Executor button": "Flash",
  "Open window": "Patch",
  "Encoder bank": "Color",
  "Jump to view": "3",
};

describe("the vocabulary an operator picks from", () => {
  it("builds an action for every kind, and names it back", () => {
    for (const kind of ACTION_KINDS) {
      const action = actionOfKind(kind, "Selected", DETAIL[kind] ?? "");
      if (kind === "Nothing") {
        expect(action).toBeNull();
        expect(kindOf(action)).toBe("Nothing");
        continue;
      }
      expect(action, `${kind} builds nothing`).not.toBeNull();
      expect(kindOf(action), `${kind} does not name itself back`).toBe(kind);
    }
  });

  it("gives every action a sentence, and none of them is the tag", () => {
    for (const kind of ACTION_KINDS) {
      const action = actionOfKind(kind, "Strip", DETAIL[kind] ?? "");
      const text = actionText(action);
      expect(text.length, `${kind} has no words`).toBeGreaterThan(0);
      if (action !== null) {
        // The list is what teaches an operator the vocabulary, so a row that
        // just repeated the wire tag would be teaching them the wrong one.
        expect(text).not.toBe(action.t);
      }
    }
  });

  it("says nothing at all for a control that is not bound", () => {
    expect(actionText(null)).toBe("—");
    expect(kindOf(null)).toBe("Nothing");
  });
});

describe("the three kinds that need an answer and do not have one", () => {
  it("refuse to build rather than guessing", () => {
    // A chooser that had not been told which window would otherwise send the
    // first one in the list, and an operator would find a key bound to
    // something they never picked.
    expect(actionOfKind("Open window", "Selected", "")).toBeNull();
    expect(actionOfKind("Encoder bank", "Selected", "")).toBeNull();
    expect(actionOfKind("Jump to view", "Selected", "")).toBeNull();
    expect(actionOfKind("Jump to view", "Selected", "nought")).toBeNull();
    expect(actionOfKind("Jump to view", "Selected", "0")).toBeNull();
  });

  it("refuse a word that is not one of the generated variants", () => {
    // `CLAUDE.md` forbids `any` and `as` is a claim rather than a check, so a
    // value that did not come out of the table answers `null` instead of being
    // asserted into the type and sent to a daemon that has never heard of it.
    expect(actionOfKind("Open window", "Selected", "Nonesuch")).toBeNull();
    expect(actionOfKind("Encoder bank", "Selected", "Sparkle")).toBeNull();
    expect(actionOfKind("Executor button", "Selected", "Wibble")).toBeNull();
  });
});

describe("an executor button with no function named", () => {
  it("is the key in that position, and the executor decides", () => {
    // `ExecutorButtonRef::Slot`, which is the whole of **D3** for playback: a
    // client that resolved `Toggle` against `isActive` would be deciding what a
    // show's own setting means, and two clients would race.
    const action = actionOfKind("Executor button", "Strip", "", 2);
    expect(action).toEqual<SurfaceAction>({
      t: "ExecutorButton",
      target: "Strip",
      button: { t: "Slot", index: 2 },
    });
  });

  it("is the function itself when the profile names one outright", () => {
    // §4.1's transport row: the desk's own configuration, written by a person.
    expect(actionOfKind("Executor button", "Selected", "On")).toEqual<SurfaceAction>({
      t: "ExecutorButton",
      target: "Selected",
      button: { t: "Function", function: "On" },
    });
  });
});

describe("which of a strip's keys a control name is", () => {
  it("is §2.1's order, read off the name", () => {
    // A binding on `Strip[*].Button.Mute` that resolved to slot 0 would put the
    // wrong key on the wrong executor function, which is the one mistake
    // `ExecutorButtonRef::Slot` exists to make impossible.
    expect(slotOf("Strip[*].Button.Rec")).toBe(0);
    expect(slotOf("Strip[*].Button.Solo")).toBe(1);
    expect(slotOf("Strip[*].Button.Mute")).toBe(2);
    expect(slotOf("Strip[*].Button.Select")).toBe(3);
    expect(slotOf("Strip[*].Button.VPotPush")).toBe(4);
  });

  it("is nought for anything that is not one of the five", () => {
    expect(slotOf("Global.Play")).toBe(0);
    expect(slotOf("Main.Fader")).toBe(0);
  });
});

describe("the directions the flattened list carries", () => {
  it("keeps the sign it was picked with", () => {
    expect(actionOfKind("Executor go +", "Selected", "")).toEqual<SurfaceAction>({
      t: "ExecutorGo",
      target: "Selected",
      direction: "Next",
    });
    expect(actionOfKind("Executor go −", "Selected", "")).toEqual<SurfaceAction>({
      t: "ExecutorGo",
      target: "Selected",
      direction: "Prev",
    });
    expect(actionOfKind("Executor page −", "Selected", "")).toEqual<SurfaceAction>({
      t: "ExecutorPage",
      delta: -1,
    });
    expect(actionOfKind("Programmer page +", "Selected", "")).toEqual<SurfaceAction>({
      t: "ProgrammerPage",
      delta: 1,
    });
    expect(actionOfKind("Previous view", "Selected", "")).toEqual<SurfaceAction>({
      t: "StepView",
      direction: "Prev",
    });
    expect(actionOfKind("Next parameter", "Selected", "")).toEqual<SurfaceAction>({
      t: "SelectProgrammerParam",
      direction: "Next",
    });
  });
});

describe("whether two controls are the same control", () => {
  it("compares structurally, because two decodings are two objects", () => {
    expect(sameControl({ t: "Jog" }, { t: "Jog" })).toBe(true);
    expect(sameControl({ t: "StripFader" }, { t: "StripEncoder" })).toBe(false);
    expect(
      sameControl({ t: "Global", button: "Play" }, { t: "Global", button: "Play" }),
    ).toBe(true);
    expect(
      sameControl({ t: "Global", button: "Play" }, { t: "Global", button: "Stop" }),
    ).toBe(false);
    expect(
      sameControl(
        { t: "StripButton", button: "Solo" },
        { t: "StripButton", button: "Solo" },
      ),
    ).toBe(true);
    expect(
      sameControl(
        { t: "StripButton", button: "Solo" },
        { t: "StripButton", button: "Mute" },
      ),
    ).toBe(false);
    // Two different shapes are never the same control, whatever they carry.
    expect(sameControl({ t: "Global", button: "Play" }, { t: "MainFader" })).toBe(false);
  });
});
