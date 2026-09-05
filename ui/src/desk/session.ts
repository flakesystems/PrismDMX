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
  ExecutorEncoderFunction,
  ExecutorFaderFunction,
  FeatureGroup,
  JsonValue,
} from "../bindings";
import { FEATURE_GROUP_VARIANTS } from "../bindings/variants";
import { buttonFunctionOf, isEncoderFunction, isFaderFunction } from "./functions";
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

/**
 * Buttons per executor — Rec, Solo, Mute, Select (`docs/MCU_MAPPING.md` §2.1).
 *
 * `prism_domain::EXECUTOR_BUTTONS`, held to it the way the number above is: the
 * desk recording carries it, and `session.test.ts` compares the two rather than
 * letting an interface assume a number and offer a key the daemon refuses.
 */
export const EXECUTOR_BUTTONS = 4;

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

/** Where the window chooser's open flag lives. */
export const WINDOW_PICKER = "/session/windowPicker";

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
  /**
   * The sequence's colour as `#rrggbb`, or `null` when it has none.
   *
   * The **sequence's**, like the name beside it: an executor is a place and a
   * colour belongs to the cue list standing in it, so a list moved to another
   * fader takes its colour along. What the X-Touch shows is this quantised to
   * one of eight (`docs/MCU_MAPPING.md` §2.3); a screen has no such limit, so
   * the bar draws the colour an operator actually chose.
   */
  readonly color: string | null;
  /** The cue list this strip's controls reach, or `null` when it has none. */
  readonly sequenceId: number | null;
  /**
   * What the fader stands at, `0..=65535`, or **`null` when the desk has no
   * number for it** — S45, and `null` since S51 (B36).
   *
   * `Master` reads the cue list's master level and `Speed` reads its rate;
   * both are the *list's* since S45, so two strips whose faders are both
   * `Master` on one list draw the same figure and move together, which is
   * punch-list entry B18.
   *
   * A **crossfade** reads `null`, and the difference between that and the
   * nought it used to read is the whole of B36 on this side of the wire: a
   * number means *draw the fader here*, and drawing a crossfade fader at nought
   * after every movement is the desk taking the operator's hand off it. Where a
   * crossfade fader stands is client-local (`ARCHITECTURE_SPEC.md` §4.2) and
   * `executorbar.tsx` is what holds it. A fader with nothing on it reads `null`
   * for the same reason: there is no number.
   */
  readonly faderLevel: number | null;
  /** Whether the cue list on it is running. */
  readonly isActive: boolean;
  /**
   * Which cue it is in, or `null` when the playback is stopped.
   *
   * **The tick's answer**, arriving through `Delta::ExecutorState` a fraction of
   * a second after whatever started the playback (S34) — not with the command,
   * because the command has only been queued when it is acknowledged. It was
   * `null` for ever before that, and the bar drew a dash; it still draws one
   * when the daemon reports none, and it still never invents a number. A bar
   * that counted the Gos it had sent would be right until a follow cue fired.
   */
  readonly currentCueIndex: number | null;
  /** What its fader does, or `null` for an unassigned slot. */
  readonly faderFunction: ExecutorFaderFunction | null;
  /** What its encoder does, or `null` for an unassigned slot. */
  readonly encoderFunction: ExecutorEncoderFunction | null;
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

/**
 * Whether the window chooser is standing open — S43, B9.
 *
 * Session state rather than this screen's, and the reason is the owner's rule
 * for this desk: a key on the X-Touch opens the chooser, a key on the X-Touch is
 * resolved by a daemon with no screen, and the desk and the interface are never
 * allowed to be out of step. See `prism_domain::Session::window_picker`.
 */
export function windowPickerOpen(session: JsonValue | null): boolean {
  return valueAt(session, WINDOW_PICKER) === true;
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
      sequenceId: null,
      name: null,
      color: null,
      // An empty slot has no number either — B36. `null` rather than nought,
      // for the same reason: nought is a *position*, and a strip with nothing
      // on it is not claiming one.
      faderLevel: null,
      isActive: false,
      currentCueIndex: null,
      faderFunction: null,
      encoderFunction: null,
      buttonFunctions: [],
    };
  }
  // **Everything the strip shows about the playback is the cue list's** — S45.
  // The name and the colour always were; the level, whether it is running and
  // which cue it stands on moved there, because a playback is a sequence's and
  // two executors on one list must not be two opinions about it (B18).
  const sequenceId = numberAt(executor, "/sequenceId");
  const sequence =
    sequenceId === null ? null : `/sequences/${String(sequenceId)}`;
  const faderFunction = faderFunctionOf(stringAt(executor, "/faderFunction"));
  return {
    slot,
    executorId,
    assigned: true,
    sequenceId,
    name: sequence === null ? null : stringAt(show, `${sequence}/name`),
    color: sequence === null ? null : hexAt(show, `${sequence}/color`),
    faderLevel: faderReading(show, sequence, faderFunction),
    isActive: sequence !== null && valueAt(show, `${sequence}/isActive`) === true,
    currentCueIndex:
      sequence === null ? null : numberAt(show, `${sequence}/currentCueIndex`),
    faderFunction,
    encoderFunction: encoderFunctionOf(stringAt(executor, "/encoderFunction")),
    buttonFunctions: buttonFunctionsOf(valueAt(executor, "/buttonFunctions")),
  };
}

/**
 * What a strip's fader stands at, or `null` when the desk has no number for it.
 *
 * `prismd::surface::fader_reading` is the same answer for the motor fader on
 * the X-Touch, and the two have to agree — a screen and a desk that disagreed
 * about where a fader is would be the fault B18 is about, one layer up. Both
 * ask `ExecutorFaderFunction::desk_may_move_it`, which is where the rule lives.
 */
function faderReading(
  show: JsonValue | null,
  sequence: string | null,
  faderFunction: ExecutorFaderFunction | null,
): number | null {
  if (sequence === null) {
    return null;
  }
  switch (faderFunction) {
    case "Master":
      return numberAt(show, `${sequence}/masterLevel`) ?? 0;
    case "Speed":
      return numberAt(show, `${sequence}/speed`) ?? 0;
    // **B36.** A crossfade has no number the desk may write, and neither has an
    // unassigned fader.
    default:
      return null;
  }
}

/**
 * An `{ r, g, b }` at a pointer, as `#rrggbb` — or `null` when there is none.
 *
 * Defensive about the three channels rather than trusting the document: this
 * reads a mirror, and a mirror one delta behind a schema change is exactly the
 * shape S26 wrote the *do not read the show* rule about. A colour it cannot make
 * sense of is no colour, which draws as an uncoloured strip rather than as a
 * broken one.
 */
function hexAt(document: JsonValue | null, pointer: string): string | null {
  const value = valueAt(document, pointer);
  if (!isObject(value)) {
    return null;
  }
  const channels = ["/r", "/g", "/b"].map((channel) => numberAt(value, channel));
  if (channels.some((channel) => channel === null)) {
    return null;
  }
  return `#${channels
    .map((channel) => Math.max(0, Math.min(255, channel ?? 0)).toString(16).padStart(2, "0"))
    .join("")}`;
}

/** A fader function this build knows, or `null`. */
function faderFunctionOf(value: string | null): ExecutorFaderFunction | null {
  return value !== null && isFaderFunction(value) ? value : null;
}

/** An encoder function this build knows, or `null`. */
function encoderFunctionOf(value: string | null): ExecutorEncoderFunction | null {
  return value !== null && isEncoderFunction(value) ? value : null;
}

/** The button functions this build knows, in order; unknown ones are left out. */
function buttonFunctionsOf(value: JsonValue | null): readonly ExecutorButtonFunction[] {
  if (value === null || !isArray(value)) {
    return [];
  }
  const functions: ExecutorButtonFunction[] = [];
  for (const entry of value) {
    // **Positions, not a filtered list** — S45. A row may now hold a custom
    // command line as well as the eight fixed functions, and an entry this
    // build cannot read has to leave its *place* behind: dropping it would move
    // every key after it one to the left, and the third key would send what the
    // fourth was bound to.
    functions.push(buttonFunctionOf(entry) ?? "Empty");
  }
  return functions;
}

/** Whether a string is one of the feature groups this build knows. */
export function isFeatureGroup(value: string): value is FeatureGroup {
  return (FEATURE_GROUP_VARIANTS as readonly string[]).includes(value);
}


