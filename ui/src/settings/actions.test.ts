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
import { CONSOLE_KEYS } from "../bindings";
import {
  ACTION_GROUPS,
  ACTION_KINDS,
  ADVANCED_KINDS,
  CONSOLE_KINDS,
  CUSTOM_KINDS,
  actionOfKind,
  actionText,
  detailOf,
  isCustom,
  kindOf,
  sameControl,
  slotOf,
  slotOfControl,
} from "./actions";
import type { ActionKind } from "./actions";

/**
 * The one extra answer a kind needs, where it needs one.
 *
 * **Six, since S59.** *Type a command* joined them with B4: its detail is the
 * line itself, which is why it is the one kind whose answer is free text rather
 * than a name out of a generated table. *Console key* joined them with S59, and
 * is the opposite case — a word out of the most generated table there is.
 */
const DETAIL: Partial<Record<ActionKind, string>> = {
  "Executor button": "Flash",
  "Open window": "Patch",
  "Encoder bank": "Color",
  "Jump to view": "3",
  "Type a command": "Go Executor 1",
  // **Six, since S59.** *Console key* carries the word, and unlike the four
  // above it the word is narrowed against the generated table rather than a
  // local list — so this is one of `CONSOLE_KEYS`' own, not a plausible string.
  "Console key": "Store",
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

describe("the panel's own list of rows", () => {
  /**
   * **S43, B3.** The panel is a list of actions now, so a kind that fell out of
   * `ACTION_GROUPS` would be a piece of the vocabulary an operator could no
   * longer reach from anywhere — and nothing else in the interface would
   * notice, because the kinds are still all buildable and still all nameable.
   * This is the test that would.
   */
  it("carries every kind except Nothing, exactly once, across the sections", () => {
    // **Six tables since S59**, and every one of them is a different shape for
    // a reason. The owner asked for an Executor, a Programmer and an *other
    // internal commands* section of rows; **Custom** is a different shape
    // because the key is the row there, since *open window* is fourteen
    // bindings and *type a command* is as many as an operator can think of;
    // **Console keys** is one kind with twenty-three rows, the words of
    // `prism_domain::CONSOLE_KEYS`; and **Advanced** holds the two the strips
    // took with them when they left the list.
    //
    // The claim is unchanged and is what matters: a kind that fell out of
    // **every** table would be unreachable from anywhere, and nothing else in
    // the interface would notice.
    const listed = [
      ...ACTION_GROUPS.flatMap((group) => group.kinds),
      ...CUSTOM_KINDS,
      ...CONSOLE_KINDS,
      ...ADVANCED_KINDS,
    ];
    expect([...listed].sort()).toEqual(
      [...ACTION_KINDS].filter((kind) => kind !== "Nothing").sort(),
    );
    expect(new Set(listed).size).toBe(listed.length);
  });

  /** The custom kinds are the ones a section of rows could not have held. */
  it("keeps the custom kinds out of the fixed sections", () => {
    const fixed = ACTION_GROUPS.flatMap((group) => group.kinds);
    for (const kind of CUSTOM_KINDS) {
      expect(fixed).not.toContain(kind);
      expect(isCustom(kind)).toBe(true);
    }
    for (const kind of fixed) {
      expect(isCustom(kind)).toBe(false);
    }
  });

  /**
   * **The strips left the list** — S59, the owner's decision of 2026-09-20.
   *
   * *Die Executor Strips … sollten aus den Controls raus.* As rows that is a
   * master, which is a strip fader or the main fader, and selecting an
   * executor, which only a strip key can mean. What did **not** leave are the
   * functions of the selected executor, and that half is asserted too — the
   * answer to that question came back twice and the second answer was the one
   * that counts.
   */
  it("keeps the strips out of the list and the selected executor in it", () => {
    const fixed = ACTION_GROUPS.flatMap((group) => group.kinds);
    for (const kind of ADVANCED_KINDS) {
      expect(fixed).not.toContain(kind);
    }
    for (const kind of ["Executor go +", "Executor go −", "Executor on", "Executor off"]) {
      expect(fixed).toContain(kind);
    }
  });

  /**
   * **One vocabulary for the screen and the desk** — S59.
   *
   * The keypad section is `CONSOLE_KEYS` itself, so this is the assertion that
   * a word added in Rust reaches the control editor without anybody editing
   * TypeScript. A separate list here is exactly what the generated table
   * replaced.
   */
  it("offers every word of the keypad as a binding", () => {
    expect(CONSOLE_KEYS.length).toBeGreaterThan(0);
    for (const key of CONSOLE_KEYS) {
      const action = actionOfKind("Console key", "Selected", key.word);
      expect(action, `${key.word} could not be bound`).not.toBeNull();
      expect(action).toEqual({ t: "ConsoleWord", word: key.word });
      expect(kindOf(action)).toBe("Console key");
      expect(detailOf(action!)).toBe(key.word);
    }
  });

  /** A word no key writes is not a binding, however plausible it looks. */
  it("refuses a word that is not on the keypad", () => {
    expect(actionOfKind("Console key", "Selected", "Enter")).toBeNull();
    expect(actionOfKind("Console key", "Selected", "")).toBeNull();
  });

  it("does not list Nothing, because unbinding is a key's button and not a row", () => {
    // A row *Nothing* would be a row an operator learns a key onto in order to
    // make that key do nothing, which is the unbind that is already on the key.
    expect(ACTION_GROUPS.flatMap((group) => group.kinds)).not.toContain("Nothing");
  });

  it("puts every group under a title, and none of them empty", () => {
    for (const group of ACTION_GROUPS) {
      expect(group.title.length, "a group with no title").toBeGreaterThan(0);
      expect(group.kinds.length, `${group.title} is empty`).toBeGreaterThan(0);
    }
  });
});

describe("which of a strip's keys a **control** is", () => {
  /**
   * `slotOf`'s twin — S43, B3. The panel used to know the row's *name* and read
   * the position off it; learn hands over a `BoundControl` instead, and the two
   * have to agree or a key learned onto *the key in this position* would bind
   * the wrong position.
   */
  it("agrees with the name-reading half, key for key", () => {
    const keys = ["Rec", "Solo", "Mute", "Select", "VPotPush"] as const;
    for (const [index, key] of keys.entries()) {
      expect(slotOfControl({ t: "StripButton", button: key })).toBe(index);
      expect(slotOf(`Strip[*].Button.${key}`)).toBe(index);
    }
  });

  it("is nought for anything that is not a strip key", () => {
    expect(slotOfControl({ t: "Global", button: "Play" })).toBe(0);
    expect(slotOfControl({ t: "MainFader" })).toBe(0);
    expect(slotOfControl({ t: "Jog" })).toBe(0);
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
