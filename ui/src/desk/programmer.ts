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
import { FEATURE_GROUP_ATTRIBUTES, FEATURE_GROUP_VARIANTS } from "../bindings/variants";
import { isArray, isObject } from "../mirror/patch";
import { numberAt, pointerToken, stringAt, valueAt } from "../mirror/select";
import { percentOfLevel } from "./level";

/** What one encoder of the bar shows. */
export interface ParameterReading {
  /** Which attribute it turns. */
  readonly attribute: AttributeType;
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

/**
 * The parameters of an encoder bank, in the order the jog wheel walks them.
 *
 * `FEATURE_GROUP_ATTRIBUTES` is generated from `prism_domain::FeatureGroup`,
 * and `prismd::surface::parameter_of` resolves the wheel through the same
 * table. That is the whole of S22's warning answered: there is one order, so
 * the wheel cannot turn something other than what is lit.
 */
export function bankParameters(bank: FeatureGroup): readonly AttributeType[] {
  return FEATURE_GROUP_ATTRIBUTES[bank];
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

/** What the encoders of a bank read, given the selection and the show. */
export function bankReadings(
  programmer: ProgrammerState | null,
  show: JsonValue | null,
  bank: FeatureGroup,
): readonly ParameterReading[] {
  return bankParameters(bank).map((attribute, index) =>
    readingOf(programmer, show, attribute, index),
  );
}

/** One encoder's reading. */
function readingOf(
  programmer: ProgrammerState | null,
  show: JsonValue | null,
  attribute: AttributeType,
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
  for (const fixture of selection) {
    if (groupOf(show, fixture, attribute) !== null) {
      available += 1;
      const rest = homeOf(show, fixture, attribute);
      if (rest !== null && home === null && !mixedHome) {
        home = rest;
      } else if (rest !== home) {
        mixedHome = true;
      }
      // **B38.** The same *only when they agree* rule the resting value
      // follows: two heads with different wheels in them have different lists,
      // and one of the two would name the wrong slot on half the selection.
      const own = rangesOf(show, fixture, attribute);
      if (ranges === null && !mixedRanges) {
        ranges = own;
      } else if (!sameRanges(ranges ?? [], own)) {
        mixedRanges = true;
      }
    }
    const entry = entryFor(programmer, fixture, attribute);
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
    attribute,
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
  attribute: AttributeType,
): readonly AttributeRange[] {
  const attributes = typeAttributes(show, fixture);
  if (attributes === null) {
    return [];
  }
  for (const entry of attributes) {
    if (!isObject(entry) || valueAt(entry, "/attribute") !== attribute) {
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
  attribute: AttributeType,
): number | null {
  const attributes = typeAttributes(show, fixture);
  if (attributes === null) {
    return null;
  }
  if (attribute === "Dimmer" && suppliedIntensity(show, fixture, attributes)) {
    // **Nought, and that is the point of it** — the colour underneath rests
    // open (B1) and this is what keeps the lamp off until somebody asks.
    return 0;
  }
  for (const entry of attributes) {
    if (!isObject(entry) || stringAt(entry, "/attribute") !== attribute) {
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
  attribute: AttributeType,
): ProgrammerState["values"][number] | null {
  if (programmer === null) {
    return null;
  }
  for (const entry of programmer.values) {
    if (entry.fixture === fixture && entry.attribute === attribute) {
      return entry;
    }
  }
  return null;
}

/** The programmer's value for one fixture and attribute, or `null`. */
export function valueFor(
  programmer: ProgrammerState | null,
  fixture: number,
  attribute: AttributeType,
): number | null {
  return entryFor(programmer, fixture, attribute)?.value.value ?? null;
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
    const group = groupOf(show, entry.fixture, entry.attribute);
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
  attribute: AttributeType,
): FeatureGroup | null {
  const attributes = typeAttributes(show, fixture);
  if (attributes === null) {
    return null;
  }
  if (attribute === "Dimmer" && suppliedIntensity(show, fixture, attributes)) {
    return "Dimmer";
  }
  for (const entry of attributes) {
    if (!isObject(entry) || stringAt(entry, "/attribute") !== attribute) {
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
