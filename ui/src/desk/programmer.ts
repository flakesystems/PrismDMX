/**
 * Reading the programmer, for the encoder bar.
 *
 * The programmer arrives whole rather than as a patch
 * (`docs/IPC_PROTOCOL.md` §6), so this is a reader over a typed value rather
 * than over a document — but it follows the same rule as the rest: it walks
 * what it is given and answers, and it holds nothing.
 *
 * # Sparse means sparse
 *
 * An attribute that was never touched is **absent**, not zero
 * (`prism_core::programmer`). So an encoder with no value under it reads as a
 * dash, and it must never read as 0 %: a programmer that laid down zeros would
 * black the stage out, and an encoder bar that *displayed* zeros would teach an
 * operator to expect it.
 *
 * # The bank an attribute is on is the profile's answer, not the name's
 *
 * `AttributeType::Dimmer` is normally on the Dimmer bank, but a fixture type
 * may file its own channel somewhere else — `AttributeDef::featureGroup` is per
 * profile. So {@link touchedBanks} walks the show: fixture → type → attribute →
 * group. `prism_core::Programmer::feature_groups` does exactly this and names it
 * an **S26 requirement**, and the recording carries its answers so this reader
 * is held to them.
 */

import type {
  AttributeRange,
  AttributeType,
  FeatureGroup,
  JsonValue,
  ProgrammerState,
  ProgrammerValueSource,
} from "../bindings";
import {
  FEATURE_GROUP_ATTRIBUTES,
  FEATURE_GROUP_VARIANTS,
  INLINE_OCCURRENCES,
} from "../bindings/variants";
import { isArray, isObject } from "../mirror/patch";
import { numberAt, pointerToken, stringAt, valueAt } from "../mirror/select";
import { percentOfLevel } from "./level";

/** What one encoder of the bar shows. */
export interface ParameterReading {
  /** Which attribute it turns. */
  readonly attribute: AttributeType;
  /**
   * Which channel of that kind — **S52**, counted from nought.
   *
   * A head may have two colour wheels and a tube a red per pixel. Nought is
   * the one there has always been.
   */
  readonly occurrence: number;
  /**
   * What the encoder is called: `Gobo` for the first of a kind, `Gobo 2` for
   * the second — `prism_domain::AttributeKey`'s `Display`, and the one place
   * on this side where the nought-based key is read out one-based.
   *
   * **This is the desk's word.** {@link ParameterReading.name} is the
   * manufacturer's, and the encoder prefers that one where there is one.
   */
  readonly label: string;
  /**
   * **What the manufacturer calls this channel** — S53, out of the show's own
   * embedded profile (S11), like {@link ParameterReading.home} and
   * {@link ParameterReading.ranges}.
   *
   * *Rotating Gobo*, *Color Wheel 2*, *Frost / Prism* — the words on the
   * fixture's own data sheet, which is what an operator is holding. `null`
   * where the profile has none (a generic one) or where two selected fixtures
   * call it different things, for `home`'s reason: naming one of the two would
   * be wrong about half the selection, and the desk's own word is right about
   * all of it.
   */
  readonly name: string | null;
  /** Its index on the bank — the number the jog wheel counts with. */
  readonly index: number;
  /**
   * The value, when every selected fixture that has this attribute holds the
   * same one; `null` when none does or when they differ.
   */
  readonly level: number | null;
  /** Whether the selected fixtures hold **different** values for it. */
  readonly mixed: boolean;
  /** How many of the selected fixtures the programmer holds this value for. */
  readonly held: number;
  /** How many of the selected fixtures have this attribute at all. */
  readonly available: number;
  /**
   * Where the held values came from, when every one of them came from the same
   * place; `null` when nothing is held or when they disagree.
   *
   * A value an operator dialled in and a value a preset put there are worth
   * telling apart before storing a cue — `presetRef` is what keeps a preset link
   * alive through a store (S13), so *this encoder is on a preset* is the thing
   * that would otherwise only be discovered afterwards.
   */
  readonly source: ProgrammerValueSource | null;
  /**
   * What the attribute **rests at** when the programmer is not holding it — the
   * profile's own `defaultValue`, when every selected fixture that has this
   * attribute agrees on one; `null` when they differ or none has it.
   *
   * S43, and it is the owner's third point: a colour channel now rests **open**
   * (B1), so an operator mixes by pulling colours *down*. An encoder that read a
   * dash there was telling them the truth about the programmer and nothing at
   * all about the lamp — and the first thing they did was push the colours up
   * from what looked like zero.
   */
  readonly home: number | null;
  /**
   * Whether this attribute is being **overridden**: the programmer holds it for
   * at least one selected fixture, so whatever a playback says, this value goes
   * out.
   *
   * The distinction the dash used to carry on its own, and the reason it can
   * stop carrying it: it is a mark of its own now, on the encoder and in the
   * sheet, so the value can always be a number.
   */
  readonly overriding: boolean;
  /**
   * The channel's **named ranges**, when every selected fixture that has this
   * attribute agrees on the same list; empty when they differ or there are none
   * — punch-list **B38**, S51.
   *
   * An Open Fixture Library channel can say *0–9 open, 10–19 gobo 1, 20–29 gobo
   * 2*, and until S51 none of it was read: a gobo wheel was a number an
   * operator had to know by heart. The list comes out of the show's own
   * embedded profile (S11), like {@link ParameterReading.home} does, so nothing
   * was added to the protocol for it.
   *
   * **Only when they agree**, for {@link ParameterReading.home}'s reason: two
   * different heads selected together have different wheels in them, and
   * offering one of the two lists would name the wrong slot on half the
   * selection.
   */
  readonly ranges: readonly AttributeRange[];
  /**
   * The name of the range the value is standing in, or `null`.
   *
   * `null` covers every way of not having one: no ranges, a mixed value, a
   * value in a gap the profile does not describe. An encoder that named the
   * nearest range instead would be telling an operator they are on a gobo when
   * they are between two.
   */
  readonly range: string | null;
}

/** One attribute of one fixture, told apart from the next one like it — S52. */
export interface ParameterKey {
  /** What it controls. */
  readonly attribute: AttributeType;
  /** Which channel of that kind, counted from nought. */
  readonly occurrence: number;
}

/**
 * What an attribute is called: `Gobo`, or `Gobo 2` for the second of a kind.
 *
 * `prism_domain::AttributeKey`'s `Display`, and the **only** place on this side
 * where the nought-based key is read out one-based. Anywhere else adding one is
 * a bug waiting for a rig with three colour wheels on it.
 */
export function parameterLabel(key: ParameterKey): string {
  return key.occurrence === 0 ? key.attribute : `${key.attribute} ${String(key.occurrence + 1)}`;
}

/**
 * The parameters of an encoder bank **for the current selection** — S52.
 *
 * # It used to be a table, and two things ended that
 *
 * Until S52 this was `FEATURE_GROUP_ATTRIBUTES[bank]`: the same knobs for every
 * selection, with a dash under every one nothing selected had. The owner asked
 * for a band that shows **only what the fixtures have**, and a fixture may have
 * **two of a parameter** — how many colour wheels a bank has is a fact about
 * the selection, which no fixed table can hold.
 *
 * S22's warning comes with it and is answered the same way it always was:
 * `prismd::surface::parameter_of` resolves the jog wheel through
 * `prism_core::Programmer::bank_parameters`, which is this function's rule in
 * Rust, and the recording holds the two together. If they disagreed an operator
 * would turn the wheel and watch a parameter other than the highlighted one
 * move.
 *
 * # Occurrence-major, and `part`
 *
 * Every first occurrence in `FEATURE_GROUP_ATTRIBUTES` order, then every
 * second, then every third — so *Red, Green, Blue, White* stays the first page
 * of the colour bank on a rig that has two of each, and *the second of
 * everything* is one contiguous run.
 *
 * A bank whose deepest repeat is at most `INLINE_OCCURRENCES` draws them all
 * and ignores `part`; past that it draws **one**, because an eight-pixel tube
 * would otherwise give the colour bank six pages of things called *Red*.
 */
export function bankParameters(
  programmer: ProgrammerState | null,
  show: JsonValue | null,
  bank: FeatureGroup,
  part: number,
): readonly ParameterKey[] {
  return bankParametersOf(show, programmer?.selection ?? [], bank, part);
}

/**
 * The same list, for a set of fixtures that is not the selection.
 *
 * The Fixture Sheet is the caller: it draws a **row per patched fixture** and a
 * column per parameter, so its columns are what those fixtures have and not
 * what happens to be selected. Same rule, same order, one implementation —
 * `bankParameters` is this with the programmer's selection put in.
 */
export function bankParametersOf(
  show: JsonValue | null,
  fixtures: readonly number[],
  bank: FeatureGroup,
  part: number,
): readonly ParameterKey[] {
  const repeats = bankRepeatsOf(show, fixtures, bank);
  if (repeats === 0) {
    return [];
  }
  const occurrences =
    repeats <= INLINE_OCCURRENCES
      ? Array.from({ length: repeats }, (_, at) => at)
      : [Math.min(Math.max(Math.trunc(part), 0), repeats - 1)];
  const parameters: ParameterKey[] = [];
  for (const occurrence of occurrences) {
    for (const attribute of FEATURE_GROUP_ATTRIBUTES[bank]) {
      const key = { attribute, occurrence };
      if (fixtures.some((fixture) => groupOf(show, fixture, key) !== null)) {
        parameters.push(key);
      }
    }
  }
  return parameters;
}

/**
 * How deep this bank's repeats go for the current selection — S52.
 *
 * The **largest** number of channels of one kind any selected fixture has on
 * this bank: 1 for an ordinary head, 2 for one with two colour wheels, 8 for a
 * tube with a red per pixel. Nought when nothing selected has anything here,
 * which is what draws an empty band rather than a row of dashes.
 *
 * The maximum and not the minimum, so a selection of two different heads offers
 * every wheel one of them has — `prism_core::Programmer::bank_repeats` says the
 * same and gives the reason.
 */
export function bankRepeats(
  programmer: ProgrammerState | null,
  show: JsonValue | null,
  bank: FeatureGroup,
): number {
  return bankRepeatsOf(show, programmer?.selection ?? [], bank);
}

/**
 * {@link bankRepeats} for a set of fixtures that is not the selection.
 *
 * **Which bank an attribute is on is `FEATURE_GROUP_ATTRIBUTES` and not the
 * profile's own `featureGroup`**, and that distinction predates S52. A profile
 * may file its dimmer under `Color` — an odd head, and a legal one — and what
 * that decides is whether the masters may scale it and which bank key lights up
 * to say the programmer is holding something ({@link touchedBanks}). It does
 * not move the knob, or the knob would be somewhere different for every head in
 * the selection.
 */
export function bankRepeatsOf(
  show: JsonValue | null,
  fixtures: readonly number[],
  bank: FeatureGroup,
): number {
  const onBank: readonly string[] = FEATURE_GROUP_ATTRIBUTES[bank];
  let deepest = 0;
  for (const fixture of fixtures) {
    for (const def of attributeDefs(show, fixture)) {
      const attribute = stringAt(def, "/attribute");
      if (attribute !== null && onBank.includes(attribute)) {
        deepest = Math.max(deepest, (numberAt(def, "/occurrence") ?? 0) + 1);
      }
    }
  }
  return deepest;
}

/**
 * How many parameters one page of the encoder bar holds.
 *
 * **The interface's decision, and only the interface's.** Four is what fits with
 * the name, the value, where the value came from and how much of the selection
 * it covers all legible at once — which is the room S35 moved the bar into one
 * band to get. The daemon has no opinion: `programmerPage` is a bare number in
 * `ARCHITECTURE_SPEC.md` §4.1, and a desk with a different screen would page it
 * differently.
 */
export const ENCODERS_PER_PAGE = 4;

/** One page of a bank's encoders, and where it sits in the bank. */
export interface EncoderPage {
  /** The parameters to draw, at most {@link ENCODERS_PER_PAGE} of them. */
  readonly readings: readonly ParameterReading[];
  /** The page actually being shown, counted from zero. */
  readonly page: number;
  /** How many pages the bank has. Never zero: an empty bank is one empty page. */
  readonly pages: number;
}

/**
 * The page of a bank the session is asking for, clamped to the bank.
 *
 * **The upper bound is the client's**, exactly as it already is for
 * `SelectProgrammerParam` (`ARCHITECTURE_SPEC.md` §4.4): `prism-core`
 * deliberately does not know how many parameters a bank has (S13), so it cannot
 * refuse a page past the end and the bar has to stop at one. A bank that fits on
 * a single page reports `pages === 1`, which is what disables the page control.
 *
 * A session page past the last one shows the **last** page rather than an empty
 * bar — a screen that went blank because a number was too big would be a screen
 * an operator cannot get back.
 */
export function encoderPage(
  readings: readonly ParameterReading[],
  page: number,
): EncoderPage {
  const pages = Math.max(1, Math.ceil(readings.length / ENCODERS_PER_PAGE));
  // `programmerPage` is a `u32` on the wire, so it cannot be negative; the lower
  // clamp is here because a document is not a promise.
  const shown = Math.min(Math.max(Math.trunc(page), 0), pages - 1);
  const from = shown * ENCODERS_PER_PAGE;
  return { readings: readings.slice(from, from + ENCODERS_PER_PAGE), page: shown, pages };
}

/**
  * What the encoders of a bank read, given the selection and the show.
  *
  * **One reading per parameter the selection actually has** since S52 — see
  * {@link bankParameters}. There is no longer such a thing as an encoder
  * nothing selected has: it is simply not drawn.
  */
export function bankReadings(
  programmer: ProgrammerState | null,
  show: JsonValue | null,
  bank: FeatureGroup,
  part: number,
): readonly ParameterReading[] {
  return bankParameters(programmer, show, bank, part).map((key, index) =>
    readingOf(programmer, show, key, index),
  );
}

/** One encoder's reading. */
function readingOf(
  programmer: ProgrammerState | null,
  show: JsonValue | null,
  key: ParameterKey,
  index: number,
): ParameterReading {
  const selection = programmer?.selection ?? [];
  let available = 0;
  let held = 0;
  let level: number | null = null;
  let mixed = false;
  let source: ProgrammerValueSource | null = null;
  let mixedSource = false;
  let home: number | null = null;
  let mixedHome = false;
  let ranges: readonly AttributeRange[] | null = null;
  let mixedRanges = false;
  let name: string | null = null;
  let mixedName = false;
  for (const fixture of selection) {
    if (groupOf(show, fixture, key) !== null) {
      available += 1;
      const rest = homeOf(show, fixture, key);
      if (rest !== null && home === null && !mixedHome) {
        home = rest;
      } else if (rest !== home) {
        mixedHome = true;
      }
      // **B38.** The same *only when they agree* rule the resting value
      // follows: two heads with different wheels in them have different lists,
      // and one of the two would name the wrong slot on half the selection.
      const own = rangesOf(show, fixture, key);
      if (ranges === null && !mixedRanges) {
        ranges = own;
      } else if (!sameRanges(ranges ?? [], own)) {
        mixedRanges = true;
      }
      // **S53.** The manufacturer's word for the channel, under the same *only
      // when they agree* rule: two heads whose gobo wheels are called different
      // things fall back to the desk's own word.
      const called = nameOf(show, fixture, key);
      if (name === null && !mixedName) {
        name = called;
      } else if (called !== name) {
        mixedName = true;
      }
    }
    const entry = entryFor(programmer, fixture, key);
    const value = entry?.value.value ?? null;
    if (entry === null || value === null) {
      continue;
    }
    if (held === 0) {
      source = entry.value.source;
    } else if (source !== entry.value.source) {
      mixedSource = true;
    }
    held += 1;
    if (level === null) {
      level = value;
    } else if (level !== value) {
      mixed = true;
    }
  }
  const shown = mixed ? null : (level ?? (mixedHome ? null : home));
  const agreed = mixedRanges ? [] : (ranges ?? []);
  return {
    attribute: key.attribute,
    occurrence: key.occurrence,
    label: parameterLabel(key),
    name: mixedName ? null : name,
    index,
    level: mixed ? null : level,
    mixed,
    held,
    available,
    source: mixedSource ? null : source,
    home: mixedHome ? null : home,
    overriding: held > 0,
    ranges: agreed,
    range:
      shown === null
        ? null
        : (agreed.find((range) => range.from <= shown && shown <= range.to)?.name ?? null),
  };
}

/** Whether two range lists are the same list. */
function sameRanges(
  left: readonly AttributeRange[],
  right: readonly AttributeRange[],
): boolean {
  return (
    left.length === right.length &&
    left.every((range, at) => {
      const other = right[at];
      return (
        other !== undefined &&
        range.name === other.name &&
        range.from === other.from &&
        range.to === other.to
      );
    })
  );
}

/**
 * One fixture's named ranges for an attribute, out of the show's own profile.
 *
 * Read from the embedded profile rather than from a query, exactly as
 * {@link homeOf} is: a show carries its own copies (S11), so the ranges an
 * operator is offered are the ones the *show* has — a library re-download does
 * not change what a patched fixture's wheel is called, which is the whole point
 * of embedding.
 *
 * A profile embedded before S51 has no `ranges` at all and answers with none,
 * which is the fixture behaving exactly as it did.
 */
export function rangesOf(
  show: JsonValue | null,
  fixture: number,
  key: ParameterKey,
): readonly AttributeRange[] {
  for (const entry of attributeDefs(show, fixture)) {
    if (!isKey(entry, key)) {
      continue;
    }
    const ranges = valueAt(entry, "/ranges");
    if (!isArray(ranges)) {
      return [];
    }
    const read: AttributeRange[] = [];
    for (const range of ranges) {
      const name = stringAt(range, "/name");
      const from = numberAt(range, "/from");
      const to = numberAt(range, "/to");
      if (name === null || from === null || to === null) {
        // A mirror one delta behind a schema change, which is S26's *do not
        // read the show* rule: a list this cannot make sense of is no list,
        // which draws an encoder with no names rather than a broken one.
        return [];
      }
      read.push({ name, from, to });
    }
    return read;
  }
  return [];
}

/**
 * Whether one attribute definition is the key asked for — **S52**.
 *
 * An absent `occurrence` **is** the first, which is what makes a profile
 * embedded in a `.prism` file before S52 answer exactly as it did.
 */
function isKey(entry: JsonValue, key: ParameterKey): boolean {
  return (
    isObject(entry) &&
    stringAt(entry, "/attribute") === key.attribute &&
    (numberAt(entry, "/occurrence") ?? 0) === key.occurrence
  );
}

/**
 * Every attribute one fixture has, the desk's supplied intensity included.
 *
 * `prism_core::Show::attribute_defs`, and here for the same reason it is there:
 * *what a fixture has* is one question, and a reader that walked the profile's
 * own list would answer it differently for a colour-only PAR (S43).
 *
 * The supplied intensity comes **first**, where a profile would have put a
 * dimmer channel.
 */
function attributeDefs(show: JsonValue | null, fixture: number): readonly JsonValue[] {
  const attributes = typeAttributes(show, fixture);
  if (attributes === null) {
    return [];
  }
  return suppliedIntensity(show, fixture, attributes)
    ? [SUPPLIED_DIMMER, ...attributes]
    : attributes;
}

/**
 * The intensity the desk supplies to a fixture whose profile has none — S43.
 *
 * `prism_core::show::SOFTWARE_DIMMER`, as much of it as this side reads: the
 * bank it is on, where it rests, and that it is the **first** and only dimmer.
 * Nought is the answer that matters — it is what keeps a rig of colour-only
 * fixtures dark at home now that a colour rests open (B1).
 */
const SUPPLIED_DIMMER: JsonValue = {
  attribute: "Dimmer",
  occurrence: 0,
  featureGroup: "Dimmer",
  defaultValue: 0,
};

/**
 * What one fixture's profile calls this channel — **S53**.
 *
 * Read out of the embedded profile like {@link homeOf} is, so the word an
 * operator reads is the one the *show* carries: a library re-download does not
 * rename a patched fixture's channels, which is the point of embedding.
 *
 * `null` for a profile that carries no name — a generic one, and every profile
 * a show embedded before S53.
 */
export function nameOf(
  show: JsonValue | null,
  fixture: number,
  key: ParameterKey,
): string | null {
  for (const entry of attributeDefs(show, fixture)) {
    if (isKey(entry, key)) {
      return stringAt(entry, "/label");
    }
  }
  return null;
}

/**
 * What one fixture's attribute rests at, out of the show's own profile.
 *
 * The **bottom of the merge stack** (`docs/DMX_MERGE.md`), which is what a lamp
 * does when nothing is driving it — and since B1 that is wide open for a colour
 * and shut for everything else. Read out of the embedded profile rather than
 * assumed, because a show carries its own copies (S11) and two profiles for the
 * same lamp may rest differently.
 */
export function homeOf(
  show: JsonValue | null,
  fixture: number,
  key: ParameterKey,
): number | null {
  for (const entry of attributeDefs(show, fixture)) {
    if (!isKey(entry, key)) {
      continue;
    }
    const rest = valueAt(entry, "/defaultValue");
    return typeof rest === "number" ? rest : null;
  }
  return null;
}

/**
 * The attribute list of the profile one fixture instantiates, or `null` when
 * the show cannot resolve it.
 *
 * One lookup for the two readings that need it, because they must agree about
 * what a fixture *has*: a bank that offered an encoder the home value could not
 * answer for would be a knob reading a dash it never leaves.
 */
function typeAttributes(show: JsonValue | null, fixture: number): readonly JsonValue[] | null {
  const typeId = stringAt(show, `/fixtures/${String(fixture)}/typeId`);
  if (typeId === null) {
    return null;
  }
  const attributes = valueAt(show, `/fixtureTypes/${pointerToken(typeId)}/attributes`);
  return attributes === null || !isArray(attributes) ? null : attributes;
}

/**
 * Whether the desk supplies this fixture's intensity — S43, and the same
 * question `prism_domain::Fixture::has_software_dimmer` answers in the daemon.
 *
 * The two halves are the operator's switch and the profile: a fixture whose
 * profile has an intensity channel already has one, and a fixture whose switch
 * is off has asked for none. **Absent means supplied**, which is the serde
 * default one crate along and the safe answer — a colour-only fixture without
 * it comes up lit, because a colour rests open.
 *
 * This is a *reading* of the show document and not a second opinion: the value
 * it produces is what the daemon has already put in the merge, and the encoder
 * draws it rather than deciding it.
 */
function suppliedIntensity(
  show: JsonValue | null,
  fixture: number,
  attributes: readonly JsonValue[],
): boolean {
  if (valueAt(show, `/fixtures/${String(fixture)}/softwareDimmer`) === false) {
    return false;
  }
  return !attributes.some(
    (entry) => isObject(entry) && stringAt(entry, "/attribute") === "Dimmer",
  );
}

/** The programmer's whole entry for one fixture and attribute, or `null`. */
function entryFor(
  programmer: ProgrammerState | null,
  fixture: number,
  key: ParameterKey,
): ProgrammerState["values"][number] | null {
  if (programmer === null) {
    return null;
  }
  for (const entry of programmer.values) {
    if (
      entry.fixture === fixture &&
      entry.attribute === key.attribute &&
      (entry.occurrence ?? 0) === key.occurrence
    ) {
      return entry;
    }
  }
  return null;
}

/** The programmer's value for one fixture and attribute, or `null`. */
export function valueFor(
  programmer: ProgrammerState | null,
  fixture: number,
  key: ParameterKey,
): number | null {
  return entryFor(programmer, fixture, key)?.value.value ?? null;
}

/**
 * Where an encoder's value came from, in one short word.
 *
 * Empty when nothing is held — there is no source for a value that does not
 * exist, and a dash there would compete with the dash the value itself shows.
 * `~` when the selection holds values from different places, for the same reason
 * `valueText` says *mixed*: naming one of them would be picking a winner.
 */
export function sourceText(reading: ParameterReading): string {
  if (reading.held === 0) {
    return "";
  }
  switch (reading.source) {
    case "Manual":
      return "man";
    case "Preset":
      return "preset";
    case "Recalled":
      return "cue";
    case null:
      return "~";
  }
}

/**
 * The encoder banks the programmer is holding a value on, in bank order.
 *
 * What an operator working in Position needs in order to see that they have
 * also touched colour. The group is the show's — see the module documentation —
 * and a value the show can no longer resolve is simply not on any bank, which
 * is what `prism_core::Programmer::feature_groups` does with it too.
 */
export function touchedBanks(
  programmer: ProgrammerState | null,
  show: JsonValue | null,
): readonly FeatureGroup[] {
  if (programmer === null) {
    return [];
  }
  const touched = new Set<FeatureGroup>();
  for (const entry of programmer.values) {
    const group = groupOf(show, entry.fixture, {
      attribute: entry.attribute,
      occurrence: entry.occurrence ?? 0,
    });
    if (group !== null) {
      touched.add(group);
    }
  }
  return FEATURE_GROUP_VARIANTS.filter((group) => touched.has(group));
}

/**
 * The bank a fixture's attribute is filed under, or `null` when the show
 * cannot resolve it.
 *
 * `null` is an ordinary answer: a fixture that has been unpatched, or one whose
 * profile was replaced by a mode without that attribute. S6 drops such values
 * on the way into the engine, and an encoder bar that pretended they were on a
 * bank would be showing an operator a knob that does nothing.
 */
export function groupOf(
  show: JsonValue | null,
  fixture: number,
  key: ParameterKey,
): FeatureGroup | null {
  for (const entry of attributeDefs(show, fixture)) {
    if (!isKey(entry, key)) {
      continue;
    }
    const group = stringAt(entry, "/featureGroup");
    if (group !== null && isFeatureGroup(group)) {
      return group;
    }
  }
  return null;
}

/**
 * What one encoder reads.
 *
 * # It used to be a dash for untouched, and S43 is where that stopped
 *
 * The old rule was **absent is not zero**: the programmer is sparse, absence
 * means *the playbacks decide*, and 0 % would have said *black*. That reasoning
 * is still right and it is why a dash was the only honest number available —
 * until B1 gave every attribute a resting value worth reading, and the owner
 * asked to see it: colours rest **open**, so mixing is pulling them down, and an
 * encoder reading a dash sent an operator looking for the value at the bottom.
 *
 * So the number is now the programmer's when it holds one and the **resting
 * value** when it does not. What stops that from being the confusion the old
 * rule guarded against is that the difference is no longer carried by the number
 * at all: `overriding` is a mark of its own, on the encoder and in the sheet.
 *
 * A dash is left for the case where there is genuinely nothing to say — nothing
 * selected has this attribute — and `mixed` for a selection that disagrees,
 * because averaging would invent a number nobody set.
 */
export function valueText(reading: ParameterReading): string {
  if (reading.mixed) {
    return "mixed";
  }
  const level = reading.level ?? reading.home;
  return level === null ? "—" : `${String(percentOfLevel(level))}%`;
}

/** How many fixtures are selected. */
export function selectionSize(programmer: ProgrammerState | null): number {
  return programmer?.selection.length ?? 0;
}

/** Whether a string is one of the feature groups this build knows. */
function isFeatureGroup(value: string): value is FeatureGroup {
  return (FEATURE_GROUP_VARIANTS as readonly string[]).includes(value);
}

/** How many programmer values there are, for the readout. */
export function touchedCount(programmer: ProgrammerState | null): number {
  return programmer?.values.length ?? 0;
}

/** The fixture numbers the programmer has selected, for the readout. */
export function selectionText(programmer: ProgrammerState | null): string {
  const selection = programmer?.selection ?? [];
  return selection.length === 0 ? "—" : selection.map(String).join(" + ");
}

/** The stage the Clear button has reached, `0`, `1` or `2`. */
export function clearStage(programmer: ProgrammerState | null): number {
  return programmer?.clearStage ?? 0;
}
