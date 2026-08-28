/**
 * **A data source is an argument keyboard** — the rule, in isolation.
 *
 * `pickOnto` is the whole of S43's second rebuild in one pure function, and it
 * is worth testing here rather than only through six windows: the windows all
 * call it the same way, and what varies is the *line*. This is that variation,
 * over the grammar the parser actually has.
 */

import { describe, expect, it } from "vitest";

import { CLEARING_VERBS, CONSOLE_WORDS, VERB_WORDS, parseCommandLine } from "./console";
import { appended, objectLine, pickOnto } from "./consoleshell";

describe("pointing at a data source", () => {
  /** The owner's own example, and the shape every window uses. */
  it("appends the object and sends the line when it is whole", () => {
    expect(pickOnto("Delete", "Sequence 2")).toEqual({
      kind: "run",
      line: "Delete Sequence 2",
    });
    expect(pickOnto("Edit", "Sequence 1 Cue 3")).toEqual({
      kind: "run",
      line: "Edit Sequence 1 Cue 3",
    });
    expect(pickOnto("Store", "Group 4")).toEqual({ kind: "run", line: "Store Group 4" });
  });

  /**
   * **A line that still wants a word is left standing** — which is what makes
   * `Copy` two clicks rather than a guess about where the copy goes.
   */
  it("leaves a line standing while it still wants a word", () => {
    expect(pickOnto("Copy", "Sequence 1")).toEqual({
      kind: "write",
      line: "Copy Sequence 1 ",
    });
    // ...and the second click finishes it.
    expect(pickOnto("Copy Sequence 1 ", "Sequence 2")).toEqual({
      kind: "run",
      line: "Copy Sequence 1 Sequence 2",
    });
  });

  /**
   * **The verbs whose missing word means *take it away*** — `CLEARING_VERBS`.
   * `Label Group 3` and `Color Sequence 4` are both whole commands, and both of
   * them undo something; a click that sent one would wipe a name or a colour
   * with a single gesture.
   */
  it("never finishes a verb whose absent argument is an instruction", () => {
    for (const verb of CLEARING_VERBS) {
      const answer = pickOnto(verb, "Sequence 4");
      expect(answer.kind, verb).toBe("write");
    }
    // The premise, asserted rather than assumed: each of them *would* have been
    // sent, because each parses to a command with nothing after the object.
    for (const verb of CLEARING_VERBS) {
      expect(parseCommandLine(`${verb} Sequence 4`).kind, verb).toBe("commands");
    }
  });

  /**
   * **A line that is not a command leaves the row alone** — *passend*, and the
   * answer is the verb. A selection takes fixtures, so a range being built is
   * not a group being named.
   */
  it("hands the row back its own gesture when the line is a selection", () => {
    expect(pickOnto("1 thru", "Group 3")).toEqual({ kind: "own" });
    expect(pickOnto("5", "Sequence 2")).toEqual({ kind: "own" });
    // And nonsense is not a command either.
    expect(pickOnto("banana", "Group 1")).toEqual({ kind: "own" });
  });

  /**
   * **A verb that cannot take *this* object leaves the line standing with the
   * parser's complaint under it**, rather than throwing the verb away.
   *
   * Which nouns a verb takes is the grammar's business, and the grammar says so
   * where an operator can read it. A click that silently discarded the `Store`
   * and selected a fixture instead is the behaviour this whole function exists
   * to remove.
   */
  it("leaves an impossible line standing rather than discarding the verb", () => {
    expect(pickOnto("Store", "Fixture 5")).toEqual({
      kind: "write",
      line: "Store Fixture 5 ",
    });
    expect(parseCommandLine("Store Fixture 5").kind).toBe("error");
    // A line that is already whole is the same case: too many words is a line
    // an operator can see and correct.
    expect(pickOnto("Delete Sequence 2", "Sequence 3").kind).toBe("write");
  });

  /**
   * **The verb table and the completion table agree** — the guard against a
   * verb added to the grammar and not to {@link VERB_WORDS}, which would be a
   * pool that quietly stopped offering it.
   *
   * Every word the console knows is one of three things: a verb, one of the six
   * numbered nouns (plus `fixture`), or one of the handful of modifiers that
   * only ever appear inside a line. A word that is none of them is a word this
   * partition has not been told about.
   */
  it("partitions every word the console knows", () => {
    const nouns = ["fixture", "sequence", "cue", "group", "preset", "view", "executor"];
    const modifiers = ["at", "thru"];
    for (const word of CONSOLE_WORDS) {
      const known =
        VERB_WORDS.includes(word) || nouns.includes(word) || modifiers.includes(word);
      expect(known, `${word} is neither a verb, a noun nor a modifier`).toBe(true);
    }
    // And every verb really is one: a word the parser does not know answers
    // *not a command*, which is what `fixture` gets and what none of these may.
    for (const verb of VERB_WORDS) {
      const read = parseCommandLine(verb);
      const message = read.kind === "error" ? read.message : "";
      expect(message, verb).not.toContain("is not a command");
    }
  });

  /**
   * **An empty line is the row's own gesture, always.**
   *
   * With nothing typed, a click on a group means *switch that group*, and the
   * leading `+` the pools write is their decision (B27) rather than something
   * this function should reinvent. Asked before anything else, so a pool never
   * has to check for it.
   */
  it("does the row's own thing on an empty line", () => {
    expect(pickOnto("", "Group 1")).toEqual({ kind: "own" });
    expect(pickOnto("   ", "Sequence 1")).toEqual({ kind: "own" });
  });

  /** What is sent is a line an operator could have typed, and they type no
      trailing space. */
  it("sends a trimmed line and leaves a standing one open", () => {
    const sent = pickOnto("Delete", "View 2");
    expect(sent.kind === "run" ? sent.line : "").toBe("Delete View 2");
    const standing = pickOnto("Move", "View 2");
    expect(standing.kind === "write" ? standing.line : "").toBe("Move View 2 ");
  });

  /**
   * The words a window appends are `objectLine`'s, and `pickOnto` joins them
   * the way `appended` does — so the line a pointer builds is spelled exactly
   * like the line a person types.
   */
  it("builds the same line the keys and the parser already agree on", () => {
    const words = objectLine({ t: "Cue", sequenceId: 1, cueNumber: "3" });
    expect(words).toBe("Sequence 1 Cue 3");
    const answer = pickOnto("Goto", words);
    expect(answer.kind === "run" ? answer.line : "").toBe(appended("Goto", words).trimEnd());
  });
});
