/**
 * **A data source is an argument keyboard** — the rule, in isolation.
 *
 * `pickOnto` is the whole of S43's second rebuild in one pure function, and it
 * is worth testing here rather than only through six windows: the windows all
 * call it the same way, and what varies is the *line*.
 *
 * # What this file stopped being able to test, and where that went
 *
 * Until S49 the variation was over the grammar, because the grammar was in this
 * directory. It is `prism_core::console` now, so what varies here is the
 * **reading the daemon sent back** — three facts about the candidate line, and
 * three answers. Which lines produce which facts is asserted where the rule
 * lives (`crates/prism-core/tests/console.rs`:
 * `a_clearing_verb_is_one_whose_absent_argument_is_an_instruction` and
 * `a_line_that_is_a_selection_is_not_a_verb_line`), and that the two halves meet
 * over a socket is `shell.test.tsx` and `ui/e2e/console.spec.ts`.
 *
 * That is a smaller test than the one it replaces, and deliberately so: a
 * TypeScript table of which lines are verbs would be the second opinion S49
 * exists to remove, restated as a fixture.
 */

import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { CommandLineReading } from "./consoleshell";
import { appended, objectLine, pickOnto, unread, useConsole } from "./consoleshell";

/** A daemon's reading of a candidate line, with the facts a pick turns on. */
function reading(
  text: string,
  facts: Partial<Pick<CommandLineReading, "kind" | "verb" | "clearing">>,
): CommandLineReading {
  return { ...unread(text), commands: 1, ...facts };
}

describe("pointing at a data source", () => {
  /** The owner's own example, and the shape every window uses. */
  it("sends the line when the candidate is a whole command", () => {
    expect(pickOnto(reading("Delete Sequence 2 ", { kind: "Commands", verb: true }))).toEqual({
      kind: "run",
      line: "Delete Sequence 2",
    });
    expect(
      pickOnto(reading("Edit Sequence 1 Cue 3 ", { kind: "Commands", verb: true })),
    ).toEqual({ kind: "run", line: "Edit Sequence 1 Cue 3" });
  });

  /**
   * **A line that still wants a word is left standing** — which is what makes
   * `Copy` two clicks rather than a guess about where the copy goes, and what
   * makes `Store Fixture 5` a line an operator can see and correct rather than
   * a `Store` that was quietly discarded.
   */
  it("leaves a line standing while it is not yet a command", () => {
    expect(pickOnto(reading("Copy Sequence 1 ", { kind: "Error", verb: true }))).toEqual({
      kind: "write",
      line: "Copy Sequence 1 ",
    });
    expect(pickOnto(reading("Store Fixture 5 ", { kind: "Error", verb: true }))).toEqual({
      kind: "write",
      line: "Store Fixture 5 ",
    });
  });

  /**
   * **The verbs whose missing word means *take it away*.** `Label Group 3` and
   * `Color Sequence 4` are both whole commands, and both of them undo
   * something; a click that sent one would wipe a name or a colour with a
   * single gesture. The daemon says which those are — `clearing` — because the
   * daemon is what knows the grammar.
   */
  it("never finishes a verb whose absent argument is an instruction", () => {
    const candidate = reading("Label Sequence 4 ", {
      kind: "Commands",
      verb: true,
      clearing: true,
    });
    expect(pickOnto(candidate)).toEqual({ kind: "write", line: "Label Sequence 4 " });
  });

  /**
   * **A line that is not a verb line leaves the row alone** — *passend*, and
   * the answer is the verb. A selection takes fixtures, so a range being built
   * is not a group being named; and an empty line plus a noun is not a verb
   * line either, which is how *with nothing typed a click on a group means
   * switch that group* falls out rather than being a case of its own.
   */
  it("hands the row back its own gesture when the line is not a verb line", () => {
    expect(pickOnto(reading("1 thru Group 3 ", { kind: "Error" }))).toEqual({ kind: "own" });
    expect(pickOnto(reading("Group 1 ", { kind: "Commands" }))).toEqual({ kind: "own" });
    expect(pickOnto(unread(""))).toEqual({ kind: "own" });
  });

  /**
   * What is sent is a line an operator could have typed, and they type no
   * trailing space; what is left standing keeps its space, so the next
   * keystroke is a word rather than a correction.
   */
  it("sends a trimmed line and leaves a standing one open", () => {
    const sent = pickOnto(reading("Delete View 2 ", { kind: "Commands", verb: true }));
    expect(sent.kind === "run" ? sent.line : "").toBe("Delete View 2");
    const standing = pickOnto(reading("Move View 2 ", { kind: "Error", verb: true }));
    expect(standing.kind === "write" ? standing.line : "").toBe("Move View 2 ");
  });
});

describe("the two spellings a key uses", () => {
  it("adds one space between words and one at the end", () => {
    expect(appended("", "Fixture")).toBe("Fixture ");
    expect(appended("Store ", "Cue")).toBe("Store Cue ");
    expect(appended("Store", "Cue")).toBe("Store Cue ");
  });

  /**
   * The words a window appends are `objectLine`'s, and the candidate a pick
   * reads is `appended`'s — so the line a pointer builds is spelled exactly
   * like the line a person types, which is the whole of `ARCHITECTURE_SPEC.md`
   * §4.5's *a pool is a key*.
   */
  it("spells an object the way the grammar reads it", () => {
    expect(objectLine({ t: "Sequence", sequenceId: 4 })).toBe("Sequence 4");
    expect(objectLine({ t: "Cue", sequenceId: null, cueNumber: "1.5" })).toBe("Cue 1.5");
    expect(objectLine({ t: "Cue", sequenceId: 2, cueNumber: "1.5" })).toBe("Sequence 2 Cue 1.5");
    expect(objectLine({ t: "Group", groupId: 3 })).toBe("Group 3");
    expect(objectLine({ t: "Preset", presetId: 3 })).toBe("Preset 3");
    expect(objectLine({ t: "View", viewId: 3 })).toBe("View 3");
    expect(objectLine({ t: "Executor", executorId: 3 })).toBe("Executor 3");
    expect(appended("Goto", objectLine({ t: "Cue", sequenceId: 1, cueNumber: "3" }))).toBe(
      "Goto Sequence 1 Cue 3 ",
    );
  });
});

describe("a reading nobody has answered yet", () => {
  /**
   * **A console that had not heard back must not complain about itself.** The
   * empty reading is *nothing to say*, not *that is not a command*: a daemon
   * that is slow, or not there at all, leaves the box quiet rather than red.
   */
  it("says nothing, offers nothing and asks nothing", () => {
    const nothing = unread("1 thru 3");
    expect(nothing.text).toBe("1 thru 3");
    expect(nothing.reading).toBe("");
    expect(nothing.kind).toBe("Empty");
    expect(nothing.question).toBeNull();
    expect(nothing.completions).toEqual([]);
    expect(nothing.verb).toBe(false);
  });
});

describe("a key pressed outside the shell", () => {
  /**
   * **It throws, and that is right.** Every key on the screen writes into one
   * line (`ARCHITECTURE_SPEC.md` §4.5), so a key rendered outside the provider
   * is a tree somebody built wrong rather than a state a running desk can be
   * in — and a hook that answered `null` instead would leave the mistake to be
   * found by a key that silently did nothing.
   */
  it("says so rather than answering with nothing", () => {
    expect(() => renderHook(() => useConsole())).toThrow("outside the console shell");
  });
});
