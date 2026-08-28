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
import { CLEARING_VERBS, VERB_WORDS, parseCommandLine } from "./console";
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

/** What pointing at a data source should do with the line as it stands. */
export type Pick =
  /** The line cannot take this, so the row does its own thing — usually select. */
  | { readonly kind: "own" }
  /** Appended, and left standing: the line still wants a word. */
  | { readonly kind: "write"; readonly line: string }
  /** Appended, and sent: the line is a whole command now. */
  | { readonly kind: "run"; readonly line: string };

/**
 * **A data source is an argument keyboard** — S43, the owner's second rebuild.
 *
 * *Alle Datenquellen sollen, solange ein passender Befehl in der Command Line
 * steht, statt select auszuführen, an den Command angehängt werden.* Type
 * `Store`, click sequence 2, and the line becomes `Store Sequence 2` and goes.
 *
 * That is `ARCHITECTURE_SPEC.md` §4.5 finished rather than extended. The rule
 * was already *every key writes a line*; what was missing is that the pools are
 * keys too. An operator who has typed a verb is **asking for an argument**, and
 * the fastest way to give one on a console is to point at it — which is what
 * the pool is for. Until now pointing at it did the pool's own thing instead,
 * so a half-typed verb was silently abandoned and the click selected something.
 *
 * # What makes a line *passend*, and why it is the verb
 *
 * `passend` is the owner's word and the answer is {@link VERB_WORDS}: a line
 * that begins with a verb is doing something **to a named object**, and that is
 * exactly the line an object belongs in. A line that does not begin with one is
 * a fixture selection (`selectionLine`), and a selection takes fixtures — so
 * typing `1 thru` and clicking a group is a range being built, not a group being
 * named, and the group's box does what a group's box does.
 *
 * That is one comparison and no table of which verb takes which noun. Which
 * *nouns* a verb takes is the grammar's business and stays there: the candidate
 * line is read by the parser, and a verb that cannot take this object leaves the
 * line standing with the parser's own complaint under it — `Store Fixture 5`
 * reads *"fixture" is not something to name*. **Leaving it there is the point.**
 * The alternative is a click that silently discards the `Store` the operator
 * typed and selects a fixture instead, which is the behaviour this whole
 * function exists to remove; a line an operator can see and correct is better
 * than a gesture that quietly did something else.
 *
 * Then *finished*: a line that parses to commands is sent, because a console
 * that made an operator press Enter after a click they already committed to is
 * a console with a wasted keystroke in it. The exception is {@link
 * CLEARING_VERBS} — `Label` and `Color`, whose missing argument means *take it
 * away*. A click that wiped a name would be the worst kind of shortcut.
 *
 * An **empty** line is `own` before anything else is asked: with nothing typed,
 * a click on a group means *switch that group*, and the `+` the pools write is
 * their own decision (B27) rather than something this function should reinvent.
 */
export function pickOnto(line: string, words: string): Pick {
  const verb = line.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
  if (verb === "" || !VERB_WORDS.includes(verb)) {
    return { kind: "own" };
  }
  const candidate = appended(line, words);
  const read = parseCommandLine(candidate);
  if (read.kind === "commands" && !CLEARING_VERBS.includes(verb)) {
    // Trimmed, because what is sent is a line an operator could have typed and
    // nobody types a trailing space.
    return { kind: "run", line: candidate.trimEnd() };
  }
  return { kind: "write", line: candidate };
}

/**
 * {@link pickOnto} carried out against a shell.
 *
 * The half every caller writes identically, so it is written once: three cases,
 * three methods, and the row's own gesture as the fallback.
 */
export function pick(shell: ConsoleShell, words: string, own: () => void): void {
  const answer = pickOnto(shell.line, words);
  if (answer.kind === "own") {
    own();
    return;
  }
  if (answer.kind === "run") {
    shell.run(answer.line);
    return;
  }
  shell.write(answer.line);
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
