/**
 * The console keys: their titles, and the table they are read from.
 *
 * A table with no component in it, which is S39's rule: `oxlint` is right about
 * `only-export-components`, so a table two components read lives in a file of
 * its own rather than beside one of them. Three read it since S59 — the
 * `CommandKeys` window (`./keypad.tsx`), the header's Clear key, and the control
 * editor, which offers the same words as bindings for a key on the surface.
 *
 * Every word here is a word `prism_core::console` takes. Pressing one **writes**,
 * it does not act: `docs/COMMAND_LINE.md` §1 is the whole design, and the shape
 * decides what writing means — run it at once, write it and wait, or append it
 * to the line as it stands. What a line *means* is the line's, never a key's.
 *
 * # The words moved to Rust — S59
 *
 * They were written out here, and that was right while this keypad was the only
 * thing that had them. It is not: a key on the X-Touch may be bound to a word
 * now, and pressing it has to do what pressing the same word here does. Two
 * hand-written tables would make that a promise rather than a fact, and the
 * failure would be the quiet kind — a word added to one of them binds a key that
 * does nothing, and no compiler anywhere says so.
 *
 * So {@link CONSOLE_KEYS} is `prism_domain::CONSOLE_KEYS`, generated into
 * `../bindings/variants.ts`, and what stays here is the **titles**: prose about
 * this interface rather than a fact about the grammar, in the language this
 * interface is written in. {@link titleOf} is the join, and
 * `keys.test.ts` holds it — a generated word with no title fails a test rather
 * than drawing a button nobody can read.
 */

import { CONSOLE_KEYS } from "../bindings";
import type { KeyShape } from "../bindings";

export { CONSOLE_KEYS };
export type { KeyShape };

/** One key of the keypad, as the generated table carries it. */
export interface ConsoleKey {
  /** The word, spelled as an operator would read it. */
  readonly word: string;
  /** Which shape — see `ARCHITECTURE_SPEC.md` §4.5. */
  readonly shape: KeyShape;
}

/**
 * What each key is for, on the button's title.
 *
 * Keyed by the generated word so that the two cannot fall out of order, which a
 * parallel array would allow. A word with no entry here is a word somebody added
 * in Rust and did not describe — `keys.test.ts` is what says so.
 */
const TITLES: Readonly<Record<string, string>> = {
  Clear: "Clear the programmer",
  Full: "The selection to full",
  Update: "Store back into the cue being edited",
  Oops: "Take the last word off the line — or, on an empty line, the last edit",
  Store: "Store into…",
  Edit: "Load a cue into the programmer",
  Goto: "Jump a playback to a cue",
  Move: "Move something to another number",
  Copy: "Copy something onto another number",
  Delete: "Empty a place on the desk",
  Label: "Name something",
  Color: "Give something a colour, or take its colour away",
  Assign: "Put a sequence on an executor",
  New: "Make an empty view and switch to it",
  Fixture: "…a fixture",
  Group: "…a group",
  Sequence: "…a sequence",
  Cue: "…a cue",
  Preset: "…a preset",
  View: "…a view",
  Executor: "…an executor",
  At: "…at a level",
  Thru: "…through another number",
};

/**
 * What a key is for, or the word itself.
 *
 * The fallback is the word rather than an empty title, because a button with no
 * tooltip is better than one whose tooltip is a blank box — and because a daemon
 * one version ahead can name a word this build has no sentence for.
 */
export function titleOf(word: string): string {
  return TITLES[word] ?? word;
}

/** Every word that has a title, for the test that holds the two together. */
export function describedWords(): readonly string[] {
  return Object.keys(TITLES);
}

/**
 * What the next press of Clear would take away, by the stage it reports —
 * **S51, punch-list B37**.
 *
 * Indexed by `ProgrammerState::clearStage`, whose numbering **is** the order of
 * the presses, so this array read top to bottom is the sequence a hand makes:
 * the selection, then the values, then the rest.
 *
 * Here rather than beside the key it titles, and that is S39's rule at the top
 * of this file: a table a component reads lives in a file with no component in
 * it. `App.test.tsx` asserts the sequence — a desk that promised one thing in
 * the tooltip and did another would be worse than one that said nothing.
 *
 * Since S59 it is also what the **lamp** of a bound Clear key follows: the
 * daemon lights it while the stage is not `Nothing`, which is this array's
 * index 0. The tooltip and the lamp therefore say the same thing in two places
 * on two devices, which is the owner's correction of 2026-09-20 — the lamp
 * reads the stage, not whether the programmer is empty.
 */
export const CLEAR_TITLES: readonly string[] = [
  "There is nothing to clear",
  "Clear the fixture selection, keeping the values",
  "Clear the programmer values as well",
  "Clear everything, including the encoder bank and the page",
];
