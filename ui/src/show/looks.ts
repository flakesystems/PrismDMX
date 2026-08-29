/**
 * Reading the *looks* out of the show document: sequences, cues and presets.
 *
 * The fourth file of the kind `canvas/windows.ts`, `desk/session.ts` and
 * `patch/patch.ts` started: **readers over the document, never a parsed copy**.
 * Nothing here holds anything, every string is narrowed against the generated
 * tables, and a sheet changes exactly when the document it is read from does.
 *
 * # What is deliberately *not* worked out here
 *
 * **What a store would do.** Whether a cue exists, how many values a store would
 * add and how many it would leave alone, and *which mode it would use* — all
 * three are `Query::StorePreview`, answered by `prism_core::ShowFile`. A client
 * that counted the overlap itself would be a second opinion about
 * `prism_core::Programmer`'s own merge, which is the duplication **D3** exists
 * to prevent, and it could not answer the third question at all.
 *
 * **Which value a preset link resolves to.** A cue part carries a value *and* a
 * `presetRef`, and the value is always current, because the daemon rewrites the
 * linked parts when the preset is stored (`prism_core::Show::relink`). So a cue
 * sheet reads `value` and never looks the preset up: resolving it here would be
 * a second answer to a question the show has already answered, and the two would
 * disagree for exactly as long as a round trip.
 *
 * **Which cue list is in force**, and **which cue the programmer is editing**.
 * Both are `Session`'s since S39 ({@link sequenceInForce},
 * {@link cueEditInForce}) — read out of the session document rather than worked
 * out from the executor grid, which is what S28 had to do and marked as an
 * assumption for this session to settle.
 *
 * **Which cue an executor is on** is the daemon's answer and not a reading.
 * `Executor::currentCueIndex` was in the domain and on the wire from S1 with
 * nothing filling it — what cue a playback is on lives on the tick thread, and
 * until S34 there was no channel back — so S26 and S28 both drew a dash. S34's
 * `prism_engine::PlaybackReport` fills it, and {@link executorInForce} passes it
 * through unchanged. **It is still not something a view may work out**: a sheet
 * that counted Gos would be right until a follow cue fired, and wrong in a way
 * nobody would notice until a show.
 */

import type { AttributeType, CueTrigger, JsonValue, PresetPool, RgbColor } from "../bindings";
import {
  ATTRIBUTE_TYPE_VARIANTS,
  CUE_TRIGGER_VARIANTS,
  PRESET_POOL_VARIANTS,
} from "../bindings/variants";
import { isArray, isObject } from "../mirror/patch";
import { booleanAt, numberAt, stringAt, valueAt } from "../mirror/select";

/** Where the sequence pool lives in the show document. */
export const SEQUENCES = "/sequences";

/** Where the preset pools live. */
export const PRESETS = "/presets";

/** Where the executor grid lives. */
export const EXECUTORS = "/executors";

/** One value inside a cue — one row of the Cue Viewer. */
export interface CuePartRow {
  /** The fixture it applies to. */
  readonly fixture: number;
  /** The attribute it sets. */
  readonly attribute: AttributeType;
  /** The value, `0..=65535`. */
  readonly value: number;
  /**
   * The preset this value follows, or `null`.
   *
   * **A link, not a lookup.** The value beside it is already the preset's: the
   * daemon rewrites every linked part when the preset is stored. This number is
   * what makes the link *visible*, and what a later session will make clickable.
   */
  readonly presetRef: number | null;
}

/** One line of a cue sheet. */
export interface CueRow {
  /** The number an operator types to reach it. A string — `1`, `1.5`, `2`. */
  readonly number: string;
  /** What it is called. May be empty. */
  readonly name: string;
  /** Fade-in seconds. */
  readonly fadeIn: number;
  /** Fade-out seconds. */
  readonly fadeOut: number;
  /** Delay before the fade starts, in seconds. */
  readonly delay: number;
  /** What starts it. */
  readonly trigger: CueTrigger;
  /** Seconds, for a `Time` trigger. */
  readonly triggerTime: number | null;
  /** What it sets. */
  readonly parts: readonly CuePartRow[];
}

/** One line of the sequence pool. */
export interface SequenceRow {
  /** The sequence number, which every command names it by. */
  readonly id: number;
  /** What it is called. */
  readonly name: string;
  /**
   * The scribble-strip colour, or `null`.
   *
   * `Sequence::color` has been on the wire since S11 and nothing on the screen
   * drew it until S43, when the pool became a grid of boxes and the colour
   * became the thing that tells two of them apart at two metres.
   */
  readonly color: RgbColor | null;
  /** Whether the last cue wraps back to the first. */
  readonly looping: boolean;
  /** Its cues, in playback order — the order the document holds them in. */
  readonly cues: readonly CueRow[];
}

/** One box of a preset pool. */
export interface PresetRow {
  /** The preset number. Unique across pools, so `ApplyPreset` is unambiguous. */
  readonly id: number;
  /**
   * Which pool it is filed in.
   *
   * A `PresetPool` and not a `FeatureGroup` since S43: **Multi** is a pool that
   * is not a bank — a preset across the categories, which is the one an
   * operator files a finished look in. See `prism_domain::PresetPool`.
   */
  readonly pool: PresetPool;
  /** What it is called. */
  readonly name: string;
  /** The scribble-strip colour, or `null`. */
  readonly color: RgbColor | null;
  /** How many values it holds. */
  readonly values: number;
}

/** What the cue sheet is looking at, and why. */
export interface ExecutorInForce {
  /** The executor number, or `null` when nothing is selected. */
  readonly executorId: number | null;
  /** The sequence on it, or `null` when there is none. */
  readonly sequenceId: number | null;
  /** Whether it is running — the one half of `Delta::ExecutorState` that arrives. */
  readonly isActive: boolean;
  /** Which cue it is on. **Always `null`** — see the module documentation. */
  readonly currentCueIndex: number | null;
}

/**
 * The sequence pool, in number order.
 *
 * The document keys sequences by number *as a string*, because that is what a
 * JSON object can do; they are ordered numerically here, so sequence 2 comes
 * before sequence 10 rather than after it.
 */
export function sequenceRows(show: JsonValue | null): readonly SequenceRow[] {
  const sequences = valueAt(show, SEQUENCES);
  if (!isObject(sequences)) {
    return [];
  }
  const rows: SequenceRow[] = [];
  for (const [key, entry] of Object.entries(sequences)) {
    const id = Number(key);
    if (!Number.isInteger(id) || !isObject(entry)) {
      continue;
    }
    rows.push({
      id,
      name: stringAt(entry, "/name") ?? "",
      color: colorOf(valueAt(entry, "/color")),
      looping: booleanAt(entry, "/loop") ?? false,
      cues: cueRowsOf(entry),
    });
  }
  return rows.sort((left, right) => left.id - right.id);
}

/** One sequence, or `null` when the show has not got it. */
export function sequenceRow(show: JsonValue | null, id: number | null): SequenceRow | null {
  if (id === null) {
    return null;
  }
  return sequenceRows(show).find((row) => row.id === id) ?? null;
}

/**
 * The cues of one sequence, in the order the document holds them.
 *
 * **Not sorted here.** `prism_core::Show::store_cue` sorts by
 * `Cue::compare_numbers` before it writes, so the document order *is* playback
 * order — and a client that sorted again would be a second opinion about what
 * `1.5` means, which is exactly the kind of arithmetic that disagrees in one
 * place and not the other.
 */
function cueRowsOf(sequence: JsonValue): readonly CueRow[] {
  const cues = valueAt(sequence, "/cues");
  if (!isArray(cues)) {
    return [];
  }
  const rows: CueRow[] = [];
  for (const entry of cues) {
    if (!isObject(entry)) {
      continue;
    }
    rows.push({
      number: stringAt(entry, "/number") ?? "",
      name: stringAt(entry, "/name") ?? "",
      fadeIn: numberAt(entry, "/fadeIn") ?? 0,
      fadeOut: numberAt(entry, "/fadeOut") ?? 0,
      delay: numberAt(entry, "/delay") ?? 0,
      trigger: triggerOf(stringAt(entry, "/trigger")),
      triggerTime: numberAt(entry, "/triggerTime"),
      parts: partRowsOf(entry),
    });
  }
  return rows;
}

/** The values of one cue, in the order the document holds them. */
function partRowsOf(cue: JsonValue): readonly CuePartRow[] {
  const parts = valueAt(cue, "/parts");
  if (!isArray(parts)) {
    return [];
  }
  const rows: CuePartRow[] = [];
  for (const entry of parts) {
    const attribute = stringAt(entry, "/attribute");
    if (!isObject(entry) || attribute === null || !isAttributeType(attribute)) {
      continue;
    }
    rows.push({
      fixture: numberAt(entry, "/fixture") ?? 0,
      attribute,
      value: numberAt(entry, "/value") ?? 0,
      presetRef: numberAt(entry, "/presetRef"),
    });
  }
  return rows;
}

/**
 * The `cues` of one cue list, as they stand.
 *
 * **A dependency, not a reading.** `mirror/patch.ts` shares every container a
 * patch did not touch, so this node keeps its identity while anything else in
 * the show moves. A store preview asked on every show change would be asked on
 * every cue of a chase, which is the warning S28 left in `PROGRESS.md` §7 and
 * S27 left before it about `PatchConflicts`.
 *
 * **It was the whole `/sequences` subtree until S45**, and that stopped being
 * narrow enough the moment a playback's state moved onto the cue list: advancing
 * a cue rewrites `/sequences/<id>/currentCueIndex`, which rebuilds `/sequences`,
 * which would ask the question again. The **cues** of the one list a store is
 * aimed at are what a preview actually depends on — a store into cue 3 means
 * something different once somebody has stored cue 3 — and they keep their
 * identity across a cue advance.
 */
export function cuesDocument(show: JsonValue | null, sequenceId: number): JsonValue | null {
  return valueAt(show, `${SEQUENCES}/${String(sequenceId)}/cues`);
}

/** The `/presets` subtree of the show. The same, for the pools. */
export function presetsDocument(show: JsonValue | null): JsonValue | null {
  return valueAt(show, PRESETS);
}

/** One group, as this window draws it. */
export interface GroupRow {
  /** The group number, which is what a line names. */
  readonly id: number;
  /** Operator-facing name. */
  readonly name: string;
  /** How many fixtures it holds. */
  readonly fixtures: number;
}

/**
 * Every group the show holds, in number order.
 *
 * A reader over the document, like every other reader in this interface: nothing
 * is parsed and kept.
 */
export function groupRows(show: JsonValue | null): readonly GroupRow[] {
  const value = valueAt(show, "/groups");
  if (!isObject(value)) {
    return [];
  }
  return Object.entries(value)
    .map(([key, entry]) => {
      const members = valueAt(entry, "/fixtures");
      const name = valueAt(entry, "/name");
      return {
        id: Number(key),
        name: typeof name === "string" ? name : "",
        fixtures: Array.isArray(members) ? members.length : 0,
      };
    })
    .filter((row) => Number.isInteger(row.id))
    .sort((left, right) => left.id - right.id);
}

/** The preset pools, in number order. */
export function presetRows(show: JsonValue | null): readonly PresetRow[] {
  const presets = valueAt(show, PRESETS);
  if (!isObject(presets)) {
    return [];
  }
  const rows: PresetRow[] = [];
  for (const [key, entry] of Object.entries(presets)) {
    const id = Number(key);
    if (!Number.isInteger(id) || !isObject(entry)) {
      continue;
    }
    rows.push({
      id,
      pool: presetPoolOf(stringAt(entry, "/pool")),
      name: stringAt(entry, "/name") ?? "",
      color: colorOf(valueAt(entry, "/color")),
      values: lengthOf(valueAt(entry, "/values")),
    });
  }
  return rows.sort((left, right) => left.id - right.id);
}

/** The presets of one pool, in number order. */
export function poolRows(show: JsonValue | null, pool: PresetPool): readonly PresetRow[] {
  return presetRows(show).filter((row) => row.pool === pool);
}

/**
 * Which groups the programmer has switched **on** — S43, B27.
 *
 * A group is a switch now rather than a per-fixture toggle, so a pool has to be
 * able to light the ones that are down. The list is the *programmer's*
 * (`ProgrammerState::selectedGroups`), which is what makes the X-Touch and the
 * screen agree about it without either being told.
 *
 * A `null` programmer — before the first snapshot — lights nothing, which is
 * the honest answer rather than a guess.
 */
export function selectedGroups(
  programmer: { readonly selectedGroups: readonly number[] } | null,
): ReadonlySet<number> {
  return new Set(programmer?.selectedGroups ?? []);
}

/**
 * The selected executor and the sequence on it — the **transport**, since S39.
 *
 * **S28's marked assumption is settled and this is no longer the sequence in
 * force.** It was, until S39: `ARCHITECTURE_SPEC.md` §4.1 had no *selected
 * sequence* field, so the sheets followed the sequence on the selected executor
 * and the assumption was written down in three places rather than made
 * permanent. §4.4 named S39 as the session that would decide, and it decided the
 * other way — see {@link sequenceInForce}. What is left here is what an executor
 * is actually for: Go, Back and Off, and which cue the playback is standing on.
 */
export function executorInForce(
  session: JsonValue | null,
  show: JsonValue | null,
): ExecutorInForce {
  const executorId = numberAt(session, "/session/selectedExecutor");
  if (executorId === null) {
    return { executorId: null, sequenceId: null, isActive: false, currentCueIndex: null };
  }
  const executor = valueAt(show, `${EXECUTORS}/${String(executorId)}`);
  const sequenceId = numberAt(executor, "/sequenceId");
  if (sequenceId === null) {
    return { executorId, sequenceId: null, isActive: false, currentCueIndex: null };
  }
  // **The playback is the cue list's** (S45), so what an executor is doing is
  // read through to the list standing on it. Two executors carrying one list
  // therefore say the same thing rather than two, which is punch-list entry B18
  // as the transport line meets it.
  const sequence = valueAt(show, `${SEQUENCES}/${String(sequenceId)}`);
  return {
    executorId,
    sequenceId,
    isActive: valueAt(sequence, "/isActive") === true,
    currentCueIndex: numberAt(sequence, "/currentCueIndex"),
  };
}

/**
 * **The cue list a store goes into** — `Session::selectedSequence`, S39.
 *
 * One line, and it is a decision rather than a lookup: S28 read this off the
 * selected executor because the session had no field for it, and S39 gave it
 * one. The two reasons, both of which the executor reading cannot meet: a
 * `Store Cue 5` typed with no executor selected has to mean something, and a cue
 * list nobody has put on a fader has to be editable without occupying a playback
 * slot to reach it.
 *
 * It is **not** coupled to {@link executorInForce}: an operator programming cue
 * list 7 while executor 3 plays the show is the ordinary case on a console, and
 * a desk that moved this every time a fader was selected would store into
 * whatever was last touched. Choosing one is a `Command::SelectSequence`, so
 * nothing here is state this interface holds and two screens cannot disagree.
 */
export function sequenceInForce(session: JsonValue | null): number | null {
  return numberAt(session, "/session/selectedSequence");
}

/** Which cue the programmer is editing, and whether it has moved since. */
export interface CueEditInForce {
  /** The sequence the cue is in. */
  readonly sequenceId: number;
  /** The cue, by its number. */
  readonly cueNumber: string;
  /** Whether the programmer has changed since it was loaded. */
  readonly modified: boolean;
}

/**
 * **The update state** — `Session::editingCue`, S39.
 *
 * What an Update key blinks on, and it is the daemon's answer rather than
 * anything worked out here: a second screen has to blink the same key, and one
 * that derived this from its own mirror of the programmer would have to know
 * which cue that programmer came from — which is exactly the fact this carries.
 */
export function cueEditInForce(session: JsonValue | null): CueEditInForce | null {
  const editing = valueAt(session, "/session/editingCue");
  if (!isObject(editing)) {
    return null;
  }
  const sequenceId = numberAt(editing, "/sequenceId");
  const cueNumber = stringAt(editing, "/cueNumber");
  if (sequenceId === null || cueNumber === null) {
    return null;
  }
  return {
    sequenceId,
    cueNumber,
    modified: booleanAt(editing, "/modified") === true,
  };
}

/** Which executors are playing a sequence, in slot order. */
export function executorsPlaying(show: JsonValue | null, sequenceId: number): readonly number[] {
  const executors = valueAt(show, EXECUTORS);
  if (!isObject(executors)) {
    return [];
  }
  const found: number[] = [];
  for (const [key, entry] of Object.entries(executors)) {
    const id = Number(key);
    if (Number.isInteger(id) && numberAt(entry, "/sequenceId") === sequenceId) {
      found.push(id);
    }
  }
  return found.sort((left, right) => left - right);
}

/**
 * The lowest number nothing is filed under.
 *
 * A convenience for the *number* field of a new sequence or preset and nothing
 * more — the same one `patch.ts::nextFreeFixtureId` is. The daemon decides
 * whether a number is free and refuses a command that would take one that is
 * not; an operator is free to type any number over it.
 */
export function nextFreeNumber(taken: readonly { readonly id: number }[]): number {
  const used = new Set(taken.map((row) => row.id));
  let candidate = 1;
  while (used.has(candidate)) {
    candidate += 1;
  }
  return candidate;
}

/**
 * The number a *new* cue would sensibly take: one past the highest whole one.
 *
 * The same kind of convenience, and the same rule — an operator types over it,
 * and `1.5` between `1` and `2` is exactly what they type. Deliberately not
 * "the next gap": a cue list is read top to bottom and a new cue belongs at the
 * end unless somebody says otherwise.
 */
export function nextCueNumber(cues: readonly CueRow[]): string {
  let highest = 0;
  for (const cue of cues) {
    const value = Number(cue.number.trim());
    if (Number.isFinite(value) && value > highest) {
      highest = value;
    }
  }
  return String(Math.floor(highest) + 1);
}

/**
 * A time, as a cue sheet shows it.
 *
 * One decimal place, because that is the resolution an operator types and
 * `3.0 s` reads as a time where `3 s` reads as a count.
 */
export function secondsText(seconds: number): string {
  return `${seconds.toFixed(1)}s`;
}

/** What a trigger says, with its time when it has one. */
export function triggerText(cue: CueRow): string {
  if (cue.trigger === "Time" && cue.triggerTime !== null) {
    return `Time ${secondsText(cue.triggerTime)}`;
  }
  return cue.trigger;
}

/**
 * A preset's colour as a CSS value, or `null` when it carries none.
 *
 * `Preset::color` is what the X-Touch scribble strips show
 * (`ARCHITECTURE_SPEC.md` §6), so a pool that ignored it would be a screen that
 * disagreed with the console beside it.
 */
export function colorStyle(color: RgbColor | null): string | null {
  if (color === null) {
    return null;
  }
  return `rgb(${String(color.r)} ${String(color.g)} ${String(color.b)})`;
}

/** How many elements are at a value that may not be an array. */
function lengthOf(value: JsonValue | null): number {
  return value !== null && isArray(value) ? value.length : 0;
}

/**
 * A colour out of the document, or `null`.
 *
 * A colour is three whole numbers or it is nothing: a half-decoded one would be
 * a swatch of a colour nobody chose.
 */
function colorOf(value: JsonValue | null): RgbColor | null {
  if (value === null || !isObject(value)) {
    return null;
  }
  const r = numberAt(value, "/r");
  const g = numberAt(value, "/g");
  const b = numberAt(value, "/b");
  if (r === null || g === null || b === null) {
    return null;
  }
  return { r, g, b };
}

/**
 * A trigger this build knows, or `Go`.
 *
 * `Go` is `CueTrigger`'s own default, and a cue whose trigger this build cannot
 * read is one an operator has to be able to see rather than one that vanishes
 * from the sheet.
 */
function triggerOf(value: string | null): CueTrigger {
  return value !== null && isCueTrigger(value) ? value : "Go";
}

/**
 * A pool name out of the document, or the bank a preset falls back to.
 *
 * Narrowed against the **generated** table rather than asserted, which is S26's
 * rule for every string that comes off the wire. A pool this build does not know
 * reads as `Dimmer` and shows up in that tab rather than in none — a preset
 * filed nowhere would be a preset an operator cannot find, and the show still
 * holds it.
 */
function presetPoolOf(value: string | null): PresetPool {
  return value !== null && isPresetPool(value) ? value : "Dimmer";
}

/** Whether a string is one of the pools this build knows. */
function isPresetPool(value: string): value is PresetPool {
  return (PRESET_POOL_VARIANTS as readonly string[]).includes(value);
}

/** Whether a string is one of the triggers this build knows. */
function isCueTrigger(value: string): value is CueTrigger {
  return (CUE_TRIGGER_VARIANTS as readonly string[]).includes(value);
}


/** Whether a string is one of the attributes this build knows. */
function isAttributeType(value: string): value is AttributeType {
  return (ATTRIBUTE_TYPE_VARIANTS as readonly string[]).includes(value);
}
