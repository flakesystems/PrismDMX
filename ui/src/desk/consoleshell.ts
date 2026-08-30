/**
 * What every key on the desk talks to, and the two spellings it uses.
 *
 * The provider is `shell.tsx` and carries the reasoning — `ARCHITECTURE_SPEC.md`
 * §4.5's three shapes, why the line belongs to the daemon, and why a prompt is
 * not a modal. This is the half with **no component in it**: the context, the
 * hook, the reading the daemon sends back, and the two pure functions a key uses
 * to build a line.
 *
 * # There is no parser here since S49
 *
 * There was one — `ui/src/desk/console.ts`, the whole grammar of S40 — and it
 * has moved into `prism_core::console`. What a line *means* is asked
 * (`Query::CommandLineReading`) and what a line *does* is sent
 * (`Command::CommandLineInput { run: true }`); this file holds neither rule.
 * That is what makes a key on the X-Touch able to run a line with no client
 * attached at all, and what stops two focused screens running one twice.
 *
 * Split for `oxlint`'s `only-export-components`, which S39 recorded as being
 * right rather than a nuisance: a helper exported beside a component costs
 * React's fast refresh, and two three-line files are cheaper than losing it.
 */

import { createContext, useContext } from "react";

import type { Answer, CommandLineMode, ObjectRef } from "../bindings";

/**
 * What the daemon says the line would do.
 *
 * The answer itself rather than a shape of this file's own: it carries the text
 * it is about, so a reading that overtook a keystroke can be told from one that
 * did not, and every field on it is something only a daemon can say.
 */
export type CommandLineReading = Extract<Answer, { t: "CommandLineReading" }>;

/**
 * The reading of a line nobody has answered about yet.
 *
 * A daemon that is not there answers nothing, and this is what the box shows
 * meanwhile: the line says nothing, offers nothing and asks nothing. It is
 * deliberately not *an error* — a console that complained because it had not
 * heard back would be complaining about itself.
 */
export function unread(text: string): CommandLineReading {
  return {
    t: "CommandLineReading",
    text,
    reading: "",
    kind: "Empty",
    commands: 0,
    verb: false,
    clearing: false,
    question: null,
    completions: [],
  };
}

/** A question the line is holding, and the line it is holding it for. */
export interface Prompt {
  /** What is already there, in the words the operator typed. */
  readonly what: string;
  /** The words to offer, in the order the daemon offers them. */
  readonly modes: readonly CommandLineMode[];
  /** The line to run once one of them has been chosen. */
  readonly line: string;
}

/** What every key on the desk talks to. */
export interface ConsoleShell {
  /** The line as it stands, which is the daemon's unless a keystroke is in flight. */
  readonly line: string;
  /** What the daemon says the line would do, shown under the input as it is typed. */
  readonly reading: CommandLineReading;
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
   * The store bar in the Cue Viewer has a chooser beside the button (S39), so
   * the question the line carries has been answered before it is sent and there
   * is nothing to prompt about. It is the same line and the same command; what
   * differs is only where the answer came from.
   */
  runWithMode: (text: string, mode: CommandLineMode) => void;
  /** Submits whatever is in the line. The Enter key, wherever it is. */
  submit: () => void;
  /** Answers a prompt: a mode, or `null` for cancel. */
  answer: (mode: CommandLineMode | null) => void;
  /** Walks the history: `-1` for the up arrow, `1` for the down arrow. */
  recall: (direction: -1 | 1) => void;
  /**
   * Points at a data source with these words — see {@link pickOnto}.
   *
   * It asks the daemon what the line **would** be, which is why it is a method
   * on the shell rather than a pure function a component calls: since S49 the
   * grammar is not here, and a click is a gesture that can afford a round trip.
   * `own` is the row's own behaviour, for a line that cannot take it.
   */
  pick: (words: string, own: () => void) => void;
}

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
 * keys too.
 *
 * # It reads the **candidate**, and that is what makes it three lines
 *
 * Before S49 this took the line, the words and a parser, and asked three
 * questions of them. It now takes one thing: the daemon's reading of the line
 * the click *would* produce. Appending a noun never changes a line's first word,
 * so the candidate's own `verb` is the standing line's — and `Group 3` clicked
 * onto an empty line is not a verb line, which is exactly *the pool does its own
 * thing*.
 *
 * Three answers, and each is one of the reading's own facts:
 *
 * - **not a verb line** — a selection is being built (`1 thru` and a group tile
 *   is a range in progress), so the row does what a row does;
 * - **a clearing verb** — `Label` and `Color`, whose missing last argument means
 *   *take it away*. A click that wiped a name is the worst kind of shortcut, so
 *   the words are appended and left standing for Enter;
 * - **not yet a command** — `Store Fixture 5` is appended and left standing with
 *   the daemon's own complaint under it, rather than the typed `Store` being
 *   quietly discarded and a fixture selected. Discarding what the operator typed
 *   is the behaviour this rule exists to remove.
 *
 * Anything else is finished, so it is sent: a console that made an operator
 * press Enter after a click they already committed to is a console with a
 * wasted keystroke in it.
 */
export function pickOnto(candidate: CommandLineReading): Pick {
  if (!candidate.verb) {
    return { kind: "own" };
  }
  if (candidate.clearing || candidate.kind !== "Commands") {
    return { kind: "write", line: candidate.text };
  }
  // Trimmed, because what is sent is a line an operator could have typed and
  // nobody types a trailing space.
  return { kind: "run", line: candidate.text.trimEnd() };
}

/**
 * {@link pickOnto} carried out against a shell.
 *
 * The half every caller writes identically, so it is written once — and since
 * S49 it is one call, because deciding which of the three it is needs the daemon
 * and the shell is what has it.
 */
export function pick(shell: ConsoleShell, words: string, own: () => void): void {
  shell.pick(words, own);
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
