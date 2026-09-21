/**
 * The keypad's table, and the join between the words and their titles — S59.
 *
 * The words moved into Rust this session (`prism_domain::CONSOLE_KEYS`) so that
 * the screen's keypad and the keys a surface control may be bound to are **one**
 * list. What stayed in TypeScript is the prose: a title is about this interface
 * and not about the grammar. That leaves exactly one seam worth a test — a word
 * generated with no sentence beside it — and this is it.
 */

import { describe, expect, it } from "vitest";

import { CONSOLE_KEYS, KEY_SHAPE_VARIANTS } from "../bindings";
import { CLEAR_TITLES, describedWords, titleOf } from "./keys";

describe("the console keypad", () => {
  it("is the generated table, and it is not empty", () => {
    // A guard against the table generating as `[]`, which would draw a keypad
    // with no keys on it and pass every other test in this file.
    expect(CONSOLE_KEYS.length).toBeGreaterThan(15);
  });

  /**
   * **Every generated word has a sentence.**
   *
   * The failure this prevents: somebody adds a word to `prism_domain::
   * CONSOLE_KEYS`, the keypad grows a button, and the button's tooltip is the
   * word again because nobody came here. `titleOf` falls back to the word for
   * a daemon one version ahead, which is right at run time and would hide this
   * at build time — so the check is against the table of titles itself.
   */
  it("has a title for every word", () => {
    const described = new Set(describedWords());
    for (const key of CONSOLE_KEYS) {
      expect(described.has(key.word), `${key.word} has no title`).toBe(true);
      expect(titleOf(key.word)).not.toBe(key.word);
    }
  });

  /** And no title for a word that is no longer on the keypad. */
  it("has no title left over from a word that has gone", () => {
    const words = new Set(CONSOLE_KEYS.map((key) => key.word));
    for (const described of describedWords()) {
      expect(words.has(described), `${described} is titled but not on the keypad`).toBe(true);
    }
  });

  /** A word the daemon knows and this build does not still draws something. */
  it("falls back to the word itself", () => {
    expect(titleOf("Wibble")).toBe("Wibble");
  });

  /**
   * Every shape a key carries is one the generated union has.
   *
   * `ARCHITECTURE_SPEC.md` §4.5 has four, and the panel styles a key by its
   * shape (`command-key-${shape}`), so a fifth arriving unstyled would be a
   * button with no look rather than an error.
   */
  it("gives every key one of the generated shapes", () => {
    for (const key of CONSOLE_KEYS) {
      expect(KEY_SHAPE_VARIANTS).toContain(key.shape);
    }
  });

  /**
   * **The four words S59 added, on both devices.**
   *
   * `Color` was in §4.5's own table from the start and never on the keypad,
   * which was a gap rather than a decision; `New`, `At` and `Thru` are the
   * owner's, so that the desk can reach the whole grammar. They are asserted by
   * name because *they are the point* — a regeneration that quietly dropped one
   * would leave the panel looking correct.
   */
  it("carries the four words S59 added", () => {
    const words = CONSOLE_KEYS.map((key) => key.word);
    expect(words).toContain("Color");
    expect(words).toContain("New");
    expect(words).toContain("At");
    expect(words).toContain("Thru");
  });

  /** And the shapes they were given, which is what pressing one does. */
  it("gives the four the shapes their grammar asks for", () => {
    const shapeOf = (word: string): string | undefined =>
      CONSOLE_KEYS.find((key) => key.word === word)?.shape;
    // Verbs write and wait; argument keywords append to the line as it stands.
    expect(shapeOf("Color")).toBe("write");
    expect(shapeOf("New")).toBe("write");
    expect(shapeOf("At")).toBe("append");
    expect(shapeOf("Thru")).toBe("append");
    // And the three that run at once are still the three that run at once.
    expect(shapeOf("Clear")).toBe("run");
    expect(shapeOf("Full")).toBe("run");
    expect(shapeOf("Update")).toBe("run");
    expect(shapeOf("Oops")).toBe("oops");
  });
});

describe("what the next press of Clear would take", () => {
  /**
   * One title per `ClearStage`, and index 0 is *nothing left to clear* — which
   * is also the stage the daemon reads to decide whether a bound Clear key is
   * lit (S59, the owner's correction of 2026-09-20). The tooltip and the lamp
   * say the same thing on two devices, so they are held to the same length.
   */
  it("has a sentence for every stage of the machine", () => {
    expect(CLEAR_TITLES).toHaveLength(4);
    for (const title of CLEAR_TITLES) {
      expect(title.length).toBeGreaterThan(0);
    }
    expect(CLEAR_TITLES[0]).toContain("nothing");
  });
});
