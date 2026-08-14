/**
 * The command line: what an operator types, and the commands it means.
 *
 * # It is a parser in the client, and that is not a contradiction
 *
 * `1 thru 3 at 50` is not a command. It is two of them — a `SelectFixtures` and
 * a `SetAttribute` — and turning one into the other is the client's job, in the
 * same way that dragging a window into a `PlaceWindow` is. **D3 is untouched**:
 * nothing here decides what the desk *is*, it decides what was *asked for*, and
 * every answer goes to the daemon to be validated, applied or refused. A line
 * naming a fixture the show has not got parses perfectly and is refused by the
 * daemon — that split is deliberate and is in the recording as a step of its
 * own.
 *
 * The session's `commandLine` is a different thing again: it is the **text**,
 * shared with every other client and with the console's display
 * (`ARCHITECTURE_SPEC.md` §4.1), and it travels as `CommandLineInput`.
 *
 * # It never throws
 *
 * {@link parseCommandLine} answers with commands or with a message, for every
 * string there is. A console that threw would take the interface down over a
 * typo in the middle of a show, and there is no input that is not a typo
 * somewhere. `console.test.ts` runs ten thousand generated strings through it
 * and asserts that none of them throws — which is the exit criterion, not a
 * precaution.
 *
 * # The grammar, in full
 *
 * ```text
 *   line     := select | level | clear | go | off | page | nothing
 *   select   := ["fixture"] fixtures
 *   level    := [fixtures] [attribute] "at" percent
 *   clear    := "clear"
 *   go       := ("go" | "go+" | "go-") executor
 *   off      := "off" executor
 *   page     := "page" number
 *   fixtures := range (("+" | ",") range)*
 *   range    := number ["thru" number]
 * ```
 *
 * `1 thru 4 at 50` · `5 pan at 25` · `at 100` · `1 + 3` · `clear` · `go 0` ·
 * `off 0` · `page 2`. Case does not matter and neither does spacing: `1thru4`
 * is a range, because a console's keypad has no space bar worth reaching for.
 *
 * What is deliberately **not** here: storing cues (S28 owns cues and the
 * `StoreCue` command has a mode question in front of it), groups and presets
 * (S27, S28), and anything that would need the show to be read in order to
 * decide what it meant. A parser that consulted the patch would be a parser
 * that could be wrong about the daemon's state.
 */

import type { AttributeType, Command } from "../bindings";
import { ATTRIBUTE_TYPE_VARIANTS } from "../bindings/variants";
import { levelFromPercent } from "./level";

/** What a line turned out to be. */
export type ConsoleResult =
  /** A line with nothing in it. Enter on an empty console does nothing. */
  | { readonly kind: "empty" }
  /** Commands, in the order they have to be sent. */
  | { readonly kind: "commands"; readonly commands: readonly Command[] }
  /** A line that is not one, and what to tell the operator about it. */
  | { readonly kind: "error"; readonly message: string };

/** The word that makes a range. */
const THRU = "thru";

/**
 * What to show under the input for a line as it stands.
 *
 * An empty line says nothing, a bad one says what is wrong with it, and a good
 * one says **what it will do** — so an operator can see before Enter that
 * `1 thru 3 at 50` is two commands and which two.
 */
export function readingText(reading: ConsoleResult): string {
  switch (reading.kind) {
    case "empty":
      return "";
    case "error":
      return reading.message;
    case "commands":
      return reading.commands.map(describe).join(" · ");
  }
}

/**
 * One command in words.
 *
 * Deliberately not the wire form: `{"t":"SetAttribute"}` is a thing to debug
 * with, not a thing to read at a desk in the dark.
 */
function describe(command: Command): string {
  switch (command.t) {
    case "SelectFixtures":
      return `select ${command.ids.join(" + ")}`;
    case "SetAttribute":
      return `${command.attribute.toLowerCase()} → ${String(
        Math.round((command.value / 65535) * 1000) / 10,
      )}%`;
    case "ClearProgrammer":
      return "clear";
    case "ExecutorGo":
      return `executor ${String(command.executorId)} ${
        command.direction === "Next" ? "go" : "back"
      }`;
    case "ExecutorOff":
      return `executor ${String(command.executorId)} off`;
    case "SetExecutorPage":
      return `page ${String(command.page)}`;
    default:
      // Every command this parser can produce is named above. The arm exists
      // because `Command` is the whole protocol, and a readout is not where a
      // command added to it should become a compile error.
      return command.t;
  }
}

/**
 * Reads a line.
 *
 * Never throws — see the module documentation.
 */
export function parseCommandLine(line: string): ConsoleResult {
  const words = tokenise(line);
  const head = words[0];
  if (head === undefined) {
    return { kind: "empty" };
  }
  switch (head) {
    case "clear":
      return words.length === 1
        ? { kind: "commands", commands: [{ t: "ClearProgrammer" }] }
        : tooMuch("clear", words);
    case "go":
      return executorCommand(words, (executorId) => ({
        t: "ExecutorGo",
        executorId,
        direction: "Next",
      }));
    case "goback":
      return executorCommand(words, (executorId) => ({
        t: "ExecutorGo",
        executorId,
        direction: "Prev",
      }));
    case "off":
      return executorCommand(words, (executorId) => ({ t: "ExecutorOff", executorId }));
    case "page":
      return pageCommand(words);
    default:
      return selectionLine(words);
  }
}

/** `go 3`, `go- 3`, `off 3` — one word and one executor number. */
function executorCommand(
  words: readonly string[],
  build: (executorId: number) => Command,
): ConsoleResult {
  const keyword = spoken(words[0] ?? "");
  const rest = words.slice(1);
  const first = rest[0];
  if (first === undefined) {
    return { kind: "error", message: `${keyword} which executor? Try "${keyword} 1".` };
  }
  if (rest.length > 1) {
    return tooMuch(keyword, words);
  }
  const executorId = wholeNumber(first);
  if (executorId === null) {
    return { kind: "error", message: `"${first}" is not an executor number.` };
  }
  return { kind: "commands", commands: [build(executorId)] };
}

/**
 * A token as the operator typed it.
 *
 * `go-` becomes the token `goback` in {@link tokenise} so that the `+` and `-`
 * of `go+` and `go-` are not read as separators; a message about it has to say
 * the word that was typed.
 */
function spoken(word: string): string {
  return word === "goback" ? "go-" : word;
}

/** `page 2` — the fader bank. */
function pageCommand(words: readonly string[]): ConsoleResult {
  const rest = words.slice(1);
  const first = rest[0];
  if (first === undefined) {
    return { kind: "error", message: 'page which page? Try "page 1".' };
  }
  if (rest.length > 1) {
    return tooMuch("page", words);
  }
  const page = wholeNumber(first);
  if (page === null) {
    return { kind: "error", message: `"${first}" is not a page number.` };
  }
  return { kind: "commands", commands: [{ t: "SetExecutorPage", page }] };
}

/**
 * Everything else: a selection, a level, or both.
 *
 * `fixture` at the front is optional noise an operator may type out of habit,
 * and is accepted for exactly that reason.
 */
function selectionLine(words: readonly string[]): ConsoleResult {
  const rest = words[0] === "fixture" || words[0] === "fixtures" ? words.slice(1) : words;
  const at = rest.indexOf("at");
  const commands: Command[] = [];

  // The attribute may sit on either side of `at` — `5 pan at 25` is how a
  // console reads aloud and `5 at pan 25` is how one is often typed — so it is
  // taken out of the line before either half is read.
  const named = attributeIn(rest);
  const without = named === null ? rest : rest.filter((word) => word !== named.word);
  const atWithout = named === null ? at : without.indexOf("at");

  const selectionWords = atWithout === -1 ? without : without.slice(0, atWithout);
  if (selectionWords.length > 0) {
    const ids = readFixtures(selectionWords);
    if (typeof ids === "string") {
      return { kind: "error", message: ids };
    }
    commands.push({ t: "SelectFixtures", ids, mode: "Set" });
  }

  if (atWithout === -1) {
    if (commands.length === 0) {
      return { kind: "error", message: `"${words.join(" ")}" is not a command.` };
    }
    return { kind: "commands", commands };
  }

  const level = readLevel(without.slice(atWithout + 1), named?.attribute ?? "Dimmer");
  if (typeof level === "string") {
    return { kind: "error", message: level };
  }
  commands.push(level);
  return { kind: "commands", commands };
}

/**
 * The `at` half of a line: a percentage, for the attribute the line named.
 *
 * The attribute defaults to `Dimmer`, because `1 at 50` means intensity on
 * every lighting desk there has ever been.
 */
function readLevel(words: readonly string[], attribute: AttributeType): Command | string {
  const first = words[0];
  if (first === undefined) {
    return "at what? A level is a percentage: 0 to 100.";
  }
  if (words.length > 1) {
    return `"${words.join(" ")}" is more than a level. A level is a percentage: 0 to 100.`;
  }
  if (first === "full") {
    return { t: "SetAttribute", attribute, value: levelFromPercent(100), relative: false };
  }
  if (first === "out" || first === "zero") {
    return { t: "SetAttribute", attribute, value: 0, relative: false };
  }
  const percent = Number(first);
  if (!Number.isFinite(percent) || first === "") {
    return `"${first}" is not a percentage. A level is 0 to 100, or "full".`;
  }
  if (percent < 0 || percent > 100) {
    return `${first} % is outside 0 to 100.`;
  }
  return {
    t: "SetAttribute",
    attribute,
    value: levelFromPercent(percent),
    relative: false,
  };
}

/** The attribute one of these words names, if any of them does. */
function attributeIn(
  words: readonly string[],
): { readonly word: string; readonly attribute: AttributeType } | null {
  for (const word of words) {
    for (const attribute of ATTRIBUTE_TYPE_VARIANTS) {
      if (attribute.toLowerCase() === word) {
        return { word, attribute };
      }
    }
  }
  return null;
}

/**
 * The fixture numbers a selection names.
 *
 * Answers a message rather than throwing on anything that is not one. A range
 * runs in the direction it is written — `4 thru 1` is 4, 3, 2, 1 — because
 * selection order is what an operator sees when they fan a value across it.
 */
function readFixtures(words: readonly string[]): number[] | string {
  const ids: number[] = [];
  const seen = new Set<number>();
  let expecting: "range" | "separator" = "range";
  let index = 0;
  while (index < words.length) {
    const word = words[index] ?? "";
    if (expecting === "separator") {
      if (word !== "+") {
        return `"${word}" is not a fixture. Separate fixtures with + and ranges with thru.`;
      }
      expecting = "range";
      index += 1;
      continue;
    }
    const from = wholeNumber(word);
    if (from === null) {
      return `"${word}" is not a fixture number.`;
    }
    index += 1;
    if (words[index] === THRU) {
      const toWord = words[index + 1];
      if (toWord === undefined) {
        return "thru what? A range is two numbers, as in 1 thru 4.";
      }
      const to = wholeNumber(toWord);
      if (to === null) {
        return `"${toWord}" is not a fixture number.`;
      }
      if (Math.abs(to - from) > MAX_RANGE) {
        return `${String(from)} thru ${String(to)} is more than ${String(MAX_RANGE)} fixtures.`;
      }
      const step = to >= from ? 1 : -1;
      for (let id = from; step > 0 ? id <= to : id >= to; id += step) {
        push(ids, seen, id);
      }
      index += 2;
    } else {
      push(ids, seen, from);
    }
    expecting = "separator";
  }
  if (expecting === "range") {
    return "the line ends with a +, so something is missing after it.";
  }
  return ids;
}

/**
 * The most fixtures one range may name.
 *
 * A guard rather than a limit anybody will meet: `1 thru 999999999` is a typo,
 * and building that array is how a console stops answering. The daemon would
 * refuse the command anyway; the point is to say so before the browser has
 * spent a second on it.
 */
export const MAX_RANGE = 4096;

/** Appends a fixture number once. A selection holds no fixture twice. */
function push(ids: number[], seen: Set<number>, id: number): void {
  if (!seen.has(id)) {
    seen.add(id);
    ids.push(id);
  }
}

/** A whole number that is not negative, or `null`. */
function wholeNumber(word: string): number | null {
  if (!/^\d+$/.test(word)) {
    return null;
  }
  const value = Number(word);
  return Number.isSafeInteger(value) ? value : null;
}

/** The complaint for a line that says more than its first word allows. */
function tooMuch(keyword: string, words: readonly string[]): ConsoleResult {
  return {
    kind: "error",
    message: `"${words.map(spoken).join(" ")}" says more than ${keyword} takes.`,
  };
}

/**
 * A line as words.
 *
 * `+`, `,` and `thru` are separators as well as words, so `1+2thru4` reads the
 * same as `1 + 2 thru 4` — a console keypad has digits and a few keys, and an
 * operator should not have to hunt for a space bar. A comma reads as a `+`,
 * because both mean *and also*.
 */
function tokenise(line: string): string[] {
  return (
    line
      .toLowerCase()
      // `go+` and `go-` first, or the `+` below would split the first of them
      // into a keyword and a separator. `goback` is the token the second
      // becomes, so the two directions are words rather than punctuation.
      .replace(/go\+/g, " go ")
      .replace(/go-/g, " goback ")
      .replace(/,/g, " + ")
      .replace(/\+/g, " + ")
      .replace(/thru/g, " thru ")
      .split(/\s+/)
      .filter((word) => word !== "")
  );
}
