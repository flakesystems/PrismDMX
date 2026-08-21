/**
 * What every key on the desk talks to, and the two spellings it uses.
 *
 * The provider is `shell.tsx` and carries the reasoning — `ARCHITECTURE_SPEC.md`
 * §4.5's three shapes, why the line belongs to the daemon, and why a prompt is
 * not a modal. This is the half with **no component in it**: the context, the
 * hook, the words a prompt offers and the two pure functions a key uses to build
 * a line.
 *
 * Split for `oxlint`'s `only-export-components`, which S39 recorded as being
 * right rather than a nuisance: a helper exported beside a component costs
 * React's fast refresh, and two three-line files are cheaper than losing it.
 */

import { createContext, useContext } from "react";

import type { Command, ObjectRef } from "../bindings";
import type { ConsoleResult, ModeQuestion } from "./console";

/** A question the line is holding, and the commands it is holding it for. */
export interface Prompt {
  /** Which words to offer. */
  readonly kind: ModeQuestion["kind"];
  /** What is already there, in the words the operator typed. */
  readonly what: string;
  /** The commands to send once a mode has been chosen. */
  readonly commands: readonly Command[];
}

/** What every key on the desk talks to. */
export interface ConsoleShell {
  /** The line as it stands, which is the daemon's unless a keystroke is in flight. */
  readonly line: string;
  /** What the line would do, shown under the input as it is typed. */
  readonly reading: ConsoleResult;
  /** The question the line is holding, or nothing. */
  readonly prompt: Prompt | null;
  /** Replaces the line and leaves it there — a command that needs arguments. */
  write: (text: string) => void;
  /** Appends a word to the line as it stands — an argument keyword. */
  append: (word: string) => void;
  /** Writes a line and submits it — a whole command, or an item picked in a list. */
  run: (text: string) => void;
  /**
   * Writes a line and submits it with a mode **already chosen**.
   *
   * The store bars on the canvas have a chooser beside the button (S39), so the
   * question the line carries has been answered before it is sent and there is
   * nothing to prompt about. It is the same line and the same command; what
   * differs is only where the answer came from.
   */
  runWithMode: (text: string, mode: string) => void;
  /** Submits whatever is in the line. The Enter key, wherever it is. */
  submit: () => void;
  /** Answers a prompt: a mode, or `null` for cancel. */
  answer: (mode: string | null) => void;
  /** Walks the history: `-1` for the up arrow, `1` for the down arrow. */
  recall: (direction: -1 | 1) => void;
}

/** The words each kind of prompt offers, in the order it offers them. */
export const PROMPT_MODES: Readonly<Record<ModeQuestion["kind"], readonly string[]>> = {
  // A cue or a preset: `StoreMode`, whose Remove is what takes values back out.
  store: ["Merge", "Override", "Remove"],
  // A whole cue list: `SequenceStoreMode`, whose Append is the one that cannot
  // lose a cue and is therefore first.
  sequence: ["Append", "Override", "Merge"],
  // A copy, a move or a group store: `OverwriteMode`, which has two.
  overwrite: ["Merge", "Override"],
};



/**
 * The context every key reads. `shell.tsx` is what puts a value in it.
 */
export const ConsoleContext = createContext<ConsoleShell | null>(null);

/**
 * The console, for every key on the desk.
 *
 * Throws when there is no provider, which is a programming error rather than a
 * state a running desk can be in.
 */
export function useConsole(): ConsoleShell {
  const shell = useContext(ConsoleContext);
  if (shell === null) {
    throw new Error("a key was pressed outside the console shell");
  }
  return shell;
}

/**
 * A word added to the line as it stands.
 *
 * One space between words and one at the end, so the operator's next keystroke
 * is a number rather than a correction. An empty line does not gain a leading
 * space, because `Fixture 1` and ` Fixture 1` are the same line and only one of
 * them reads well on a scribble strip.
 */
export function appended(line: string, word: string): string {
  const head = line.trimEnd();
  return head === "" ? `${word} ` : `${head} ${word} `;
}

/**
 * The line that names one object, for a key or a list row to write.
 *
 * One place rather than a template string per component, so `Delete Group 3` is
 * spelled the same way wherever it is written from — and so the words the
 * pointer produces are exactly the words the parser takes.
 */
export function objectLine(target: ObjectRef): string {
  switch (target.t) {
    case "Sequence":
      return `Sequence ${String(target.sequenceId)}`;
    case "Cue":
      return target.sequenceId === null
        ? `Cue ${target.cueNumber}`
        : `Sequence ${String(target.sequenceId)} Cue ${target.cueNumber}`;
    case "Group":
      return `Group ${String(target.groupId)}`;
    case "Preset":
      return `Preset ${String(target.presetId)}`;
    case "View":
      return `View ${String(target.viewId)}`;
    case "Executor":
      return `Executor ${String(target.executorId)}`;
  }
}
