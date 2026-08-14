/**
 * Reading the desk out of the session and the show.
 *
 * The same rule `canvas/windows.ts` follows, one layer along: **readers over
 * the documents, never a parsed copy**. S23's finding was that a parsed model is
 * a second thing to keep in step and that RFC 6902 operations only mean
 * anything against a root; nothing here holds anything, so a bar redraws
 * exactly when the document it reads has moved.
 *
 * # What the executor bar has to get right, and why it is here
 *
 * **D7: one page is eight executors, numbered `page * 8 + slot`.** That is one
 * multiplication, and it is the one thing on this screen that can address the
 * wrong executor during a show. So it is written once, checked against
 * `prism_domain::EXECUTORS_PER_PAGE` through the recording, and a slot with
 * nothing in it is a *strip* all the same — an empty page is an ordinary page,
 * not an error, and an operator paging past their executors must see eight
 * empty strips rather than a bar that has vanished.
 *
 * # Every string is narrowed against the generated tables
 *
 * `encoderBank`, the fader function and the button functions all come off a
 * socket as `unknown` and are checked against `bindings/variants.ts` — which is
 * generated from the Rust enums. A view that wrote its own list would be
 * reintroducing exactly the drift that file removes (S23).
 */

import type {
  ExecutorButtonFunction,
  ExecutorFaderFunction,
  FeatureGroup,
  JsonValue,
} from "../bindings";
import {
  EXECUTOR_BUTTON_FUNCTION_VARIANTS,
  EXECUTOR_FADER_FUNCTION_VARIANTS,
  FEATURE_GROUP_VARIANTS,
} from "../bindings/variants";
import { isArray, isObject } from "../mirror/patch";
import { numberAt, stringAt, valueAt } from "../mirror/select";

/**
 * Executors on one page — decision **D7**.
 *
 * `prism_domain::EXECUTORS_PER_PAGE`, and the one number here that could
 * address the wrong executor if it were wrong. `session.test.ts` asserts it
 * against the figure the daemon recorded, so it is a transcription that is
 * checked rather than one that is hoped for.
 */
export const EXECUTORS_PER_PAGE = 8;

/** Where the executor page lives in the session document. */
export const EXECUTOR_PAGE = "/session/executorPage";

/** Where the selected executor lives. */
export const SELECTED_EXECUTOR = "/session/selectedExecutor";

/** Where the encoder bank lives. */
export const ENCODER_BANK = "/session/encoderBank";

/** Where the programmer page lives. */
export const PROGRAMMER_PAGE = "/session/programmerPage";

/** Where the index of the parameter the jog wheel turns lives. */
export const PROGRAMMER_PARAM_INDEX = "/session/programmerParamIndex";

/** Where the console line lives. */
export const COMMAND_LINE = "/session/commandLine";

/** One strip of the executor bar. */
export interface ExecutorStrip {
  /** Which of the eight it is, from the left. */
  readonly slot: number;
  /** `page * 8 + slot` — what every command names it by. */
  readonly executorId: number;
  /** Whether the show has an executor in that slot at all. */
  readonly assigned: boolean;
  /** The sequence's name, or `null` when there is no sequence on it. */
  readonly name: string | null;
  /** The master, `0..=65535`. */
  readonly masterLevel: number;
  /** Whether it is running. */
  readonly isActive: boolean;
  /**
   * Which cue it is in, or `null`.
   *
   * **Always `null` today**, and that is the daemon's gap rather than this
   * reader's: what cue a playback is on lives on the tick thread and nothing
   * feeds it back into the show. See `PROGRESS.md`'s decision log; the bar
   * shows a dash rather than inventing a number.
   */
  readonly currentCueIndex: number | null;
  /** What its fader does, or `null` for an unassigned slot. */
  readonly faderFunction: ExecutorFaderFunction | null;
  /** What its four buttons do, in hardware order: Rec, Solo, Mute, Select. */
  readonly buttonFunctions: readonly ExecutorButtonFunction[];
}

/** The executor page the session is on. */
export function executorPage(session: JsonValue | null): number {
  return numberAt(session, EXECUTOR_PAGE) ?? 0;
}

/** The selected executor's number, or `null` when nothing is selected. */
export function selectedExecutor(session: JsonValue | null): number | null {
  return numberAt(session, SELECTED_EXECUTOR);
}

/**
 * The encoder bank in force.
 *
 * `Dimmer` where the document says something this build does not know, which is
 * `FeatureGroup`'s own default: a bar with no bank lit would leave an operator
 * with no encoders at all, and the first bank is the one a desk starts on.
 */
export function encoderBank(session: JsonValue | null): FeatureGroup {
  const value = stringAt(session, ENCODER_BANK);
  return value !== null && isFeatureGroup(value) ? value : "Dimmer";
}

/** The programmer page the session is on. */
export function programmerPage(session: JsonValue | null): number {
  return numberAt(session, PROGRAMMER_PAGE) ?? 0;
}

/** The index of the parameter the jog wheel turns. */
export function programmerParamIndex(session: JsonValue | null): number {
  return numberAt(session, PROGRAMMER_PARAM_INDEX) ?? 0;
}

/** The console line **as the daemon holds it** — never what has been typed. */
export function commandLine(session: JsonValue | null): string {
  return stringAt(session, COMMAND_LINE) ?? "";
}

/** The executor number in a slot of a page. D7's arithmetic, in one place. */
export function executorIdAt(page: number, slot: number): number {
  return page * EXECUTORS_PER_PAGE + slot;
}

/**
 * The eight strips of the session's current page.
 *
 * Always eight. A slot the show has no executor for is a strip with
 * `assigned: false` — the fader is there, it just has nothing on it, which is
 * what an operator sees on a console and what makes an empty page legible.
 */
export function pageStrips(
  session: JsonValue | null,
  show: JsonValue | null,
): readonly ExecutorStrip[] {
  const page = executorPage(session);
  const strips: ExecutorStrip[] = [];
  for (let slot = 0; slot < EXECUTORS_PER_PAGE; slot += 1) {
    strips.push(stripOf(show, page, slot));
  }
  return strips;
}

/** One strip, read out of the show document. */
function stripOf(show: JsonValue | null, page: number, slot: number): ExecutorStrip {
  const executorId = executorIdAt(page, slot);
  const executor = valueAt(show, `/executors/${String(executorId)}`);
  if (!isObject(executor)) {
    return {
      slot,
      executorId,
      assigned: false,
      name: null,
      masterLevel: 0,
      isActive: false,
      currentCueIndex: null,
      faderFunction: null,
      buttonFunctions: [],
    };
  }
  const sequenceId = numberAt(executor, "/sequenceId");
  return {
    slot,
    executorId,
    assigned: true,
    name:
      sequenceId === null
        ? null
        : stringAt(show, `/sequences/${String(sequenceId)}/name`),
    masterLevel: numberAt(executor, "/masterLevel") ?? 0,
    isActive: valueAt(executor, "/isActive") === true,
    currentCueIndex: numberAt(executor, "/currentCueIndex"),
    faderFunction: faderFunctionOf(stringAt(executor, "/faderFunction")),
    buttonFunctions: buttonFunctionsOf(valueAt(executor, "/buttonFunctions")),
  };
}

/** A fader function this build knows, or `null`. */
function faderFunctionOf(value: string | null): ExecutorFaderFunction | null {
  return value !== null && isFaderFunction(value) ? value : null;
}

/** Whether a string is one of the fader functions this build knows. */
function isFaderFunction(value: string): value is ExecutorFaderFunction {
  return (EXECUTOR_FADER_FUNCTION_VARIANTS as readonly string[]).includes(value);
}

/** The button functions this build knows, in order; unknown ones are left out. */
function buttonFunctionsOf(value: JsonValue | null): readonly ExecutorButtonFunction[] {
  if (value === null || !isArray(value)) {
    return [];
  }
  const functions: ExecutorButtonFunction[] = [];
  for (const entry of value) {
    if (typeof entry === "string" && isButtonFunction(entry)) {
      functions.push(entry);
    }
  }
  return functions;
}

/** Whether a string is one of the feature groups this build knows. */
export function isFeatureGroup(value: string): value is FeatureGroup {
  return (FEATURE_GROUP_VARIANTS as readonly string[]).includes(value);
}

/** Whether a string is one of the button functions this build knows. */
function isButtonFunction(value: string): value is ExecutorButtonFunction {
  return (EXECUTOR_BUTTON_FUNCTION_VARIANTS as readonly string[]).includes(value);
}
