/**
 * The console keys: the words, and which of the three shapes each one is.
 *
 * A table with no component in it, which is S39's rule: `oxlint` is right about
 * `only-export-components`, so a table two components read lives in a file of
 * its own rather than beside one of them. Two read it since S43 — the
 * `CommandKeys` window (`./keypad.tsx`) and the header's Clear key.
 *
 * Every word here is a word `prism_core::console` takes. Pressing one **writes**,
 * it does not act: `docs/COMMAND_LINE.md` §1 is the whole design, and the shape
 * decides what writing means — run it at once, write it and wait, or append it
 * to the line as it stands. What a line *means* is the line's, never a key's.
 */

/** Which of `ARCHITECTURE_SPEC.md` §4.5's three shapes a key is. */
export type KeyShape = "run" | "write" | "append";

/** A key of the keypad: the word it writes, and which shape it is. */
export interface ConsoleKey {
  /** The word, spelled as an operator would read it. */
  readonly word: string;
  /** Which shape — see `ARCHITECTURE_SPEC.md` §4.5. */
  readonly shape: KeyShape;
  /** What it is for, on the button's title. */
  readonly title: string;
}

/**
 * The keypad, in the order a console has it.
 *
 * `Clear` is first and is also drawn in the header, because it is the one key an
 * operator reaches for without looking and the `CommandKeys` window can be
 * closed. It is the same word and the same gesture in both places.
 */
export const CONSOLE_KEYS: readonly ConsoleKey[] = [
  { word: "Clear", shape: "run", title: "Clear the programmer" },
  { word: "Full", shape: "run", title: "The selection to full" },
  { word: "Update", shape: "run", title: "Store back into the cue being edited" },
  { word: "Oops", shape: "run", title: "Take the last edit back" },
  { word: "Store", shape: "write", title: "Store into…" },
  { word: "Edit", shape: "write", title: "Load a cue into the programmer" },
  { word: "Goto", shape: "write", title: "Jump a playback to a cue" },
  { word: "Move", shape: "write", title: "Move something to another number" },
  { word: "Copy", shape: "write", title: "Copy something onto another number" },
  { word: "Delete", shape: "write", title: "Empty a place on the desk" },
  { word: "Label", shape: "write", title: "Name something" },
  { word: "Assign", shape: "write", title: "Put a sequence on an executor" },
  { word: "Fixture", shape: "append", title: "…a fixture" },
  { word: "Group", shape: "append", title: "…a group" },
  { word: "Sequence", shape: "append", title: "…a sequence" },
  { word: "Cue", shape: "append", title: "…a cue" },
  { word: "Preset", shape: "append", title: "…a preset" },
  { word: "View", shape: "append", title: "…a view" },
  { word: "Executor", shape: "append", title: "…an executor" },
];
