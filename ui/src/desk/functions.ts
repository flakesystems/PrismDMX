/**
 * What an executor's controls can be made to do, in one place — S45.
 *
 * `ExecutorButtonFunction` stopped being a union of strings in S45: eight fixed
 * functions and a ninth that carries a line an operator wrote
 * (`{ CommandLine: { line } }`), which is punch-list entry B15's *custom row*.
 * Four places have to read one — the executor strip, the control editor, the
 * protocol decoder and the surface-binding editor — and each of them needs the
 * same three answers: is this a function this build knows, what is it called on
 * a key, and does it carry a line. Those three, and nothing speculative beside
 * them: a helper nobody calls is a line of coverage nobody can justify.
 *
 * The **lists** are not here. `EXECUTOR_BUTTON_FUNCTION_VARIANTS`,
 * `EXECUTOR_FADER_FUNCTION_VARIANTS` and `EXECUTOR_ENCODER_FUNCTION_VARIANTS`
 * are generated from Rust (`prism_domain::export`), so a function added there
 * appears in the chooser without a second list being edited — the rule
 * `ARCHITECTURE_SPEC.md` §4.6 states for `WindowType`, applied to the three
 * enums this window offers.
 */

import type {
  ExecutorButtonFunction,
  ExecutorEncoderFunction,
  ExecutorFaderFunction,
  JsonValue,
} from "../bindings";
import {
  EXECUTOR_BUTTON_FUNCTION_VARIANTS,
  EXECUTOR_ENCODER_FUNCTION_VARIANTS,
  EXECUTOR_FADER_FUNCTION_VARIANTS,
} from "../bindings";

/** The eight fixed button functions, as the show writes them. */
export type FixedButtonFunction = Extract<ExecutorButtonFunction, string>;

/**
 * The eight fixed functions, narrowed to the strings they are.
 *
 * The generated table is typed `readonly ExecutorButtonFunction[]` because that
 * is the type it belongs to, and since S45 that type also has an object arm the
 * table can never hold. Two places need the narrower type — the surface
 * binding editor, which offers only the eight (a profile row that wanted a line
 * says `WriteCommandLine`, which has been the way to say it since S43), and the
 * control editor's chooser.
 */
export const FIXED_BUTTON_FUNCTIONS: readonly FixedButtonFunction[] =
  EXECUTOR_BUTTON_FUNCTION_VARIANTS.filter((fn): fn is FixedButtonFunction => typeof fn === "string");

/** Whether a string is one of the fixed button functions this build knows. */
export function isFixedButtonFunction(value: string): value is FixedButtonFunction {
  return (EXECUTOR_BUTTON_FUNCTION_VARIANTS as readonly string[]).includes(value);
}

/** Whether a string is one of the fader functions this build knows. */
export function isFaderFunction(value: string): value is ExecutorFaderFunction {
  return (EXECUTOR_FADER_FUNCTION_VARIANTS as readonly string[]).includes(value);
}

/** Whether a string is one of the encoder functions this build knows. */
export function isEncoderFunction(value: string): value is ExecutorEncoderFunction {
  return (EXECUTOR_ENCODER_FUNCTION_VARIANTS as readonly string[]).includes(value);
}

/**
 * One button function read out of a mirrored document, or `null` for anything
 * this build cannot make sense of.
 *
 * **Narrowed rather than asserted**, which is this interface's rule for every
 * document it reads: `as` is a claim and the mirror may be one schema change
 * behind. A row it cannot read draws as a key with nothing on it, which is a
 * legible answer rather than a broken one.
 */
export function buttonFunctionOf(value: JsonValue | null): ExecutorButtonFunction | null {
  if (typeof value === "string") {
    return isFixedButtonFunction(value) ? value : null;
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const custom: unknown = value.CommandLine;
  if (custom === null || typeof custom !== "object" || Array.isArray(custom)) {
    return null;
  }
  const line: unknown = (custom as Record<string, unknown>).line;
  return typeof line === "string" ? { CommandLine: { line } } : null;
}

/** The line a custom row sends, or `null` when the key is a fixed function. */
export function commandLineOf(fn: ExecutorButtonFunction): string | null {
  return typeof fn === "string" ? null : fn.CommandLine.line;
}

/** What a button says on a strip, which is shorter than what the show calls it. */
export function buttonLabel(fn: ExecutorButtonFunction): string {
  return typeof fn === "string" ? SHORT[fn] : "Cmd";
}

/** What a button is called in a list an operator reads. */
export function buttonName(fn: ExecutorButtonFunction): string {
  return typeof fn === "string" ? NAMES[fn] : "Command line";
}

/** What a fader function is called in a list an operator reads. */
export function faderName(fn: ExecutorFaderFunction): string {
  return FADER_NAMES[fn];
}

/** What an encoder function is called in a list an operator reads. */
export function encoderName(fn: ExecutorEncoderFunction): string {
  return ENCODER_NAMES[fn];
}

/** The short form, for a key three characters wide. */
const SHORT: Readonly<Record<FixedButtonFunction, string>> = {
  Empty: "",
  "Go+": "Go",
  "Go-": "Bk",
  LearnSpeed: "Lrn",
  Off: "Off",
  On: "On",
  Flash: "Fl",
  Toggle: "Tog",
};

/** The long form, for the chooser. */
const NAMES: Readonly<Record<FixedButtonFunction, string>> = {
  Empty: "Nothing",
  "Go+": "Go forward",
  "Go-": "Go back",
  LearnSpeed: "Learn speed",
  Off: "Off",
  On: "On",
  Flash: "Flash",
  Toggle: "Toggle",
};

/** The fader's, in the words `docs/DMX_MERGE.md` §4 uses. */
const FADER_NAMES: Readonly<Record<ExecutorFaderFunction, string>> = {
  Empty: "Nothing",
  Master: "Master",
  Speed: "Speed",
  // **The two crossfades** — S51, B36. Named by what they do rather than by
  // their own words, because *Fade* and *XFade* differ by one letter and an
  // operator choosing between them in a menu has to be told which is which.
  Fade: "Fade out and in",
  XFade: "Crossfade",
};

/** The encoder's. */
const ENCODER_NAMES: Readonly<Record<ExecutorEncoderFunction, string>> = {
  Empty: "Nothing",
  Master: "Master",
  Speed: "Speed",
};
