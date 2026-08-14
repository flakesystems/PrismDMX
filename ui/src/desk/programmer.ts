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

import type { AttributeType, FeatureGroup, JsonValue, ProgrammerState } from "../bindings";
import { FEATURE_GROUP_ATTRIBUTES, FEATURE_GROUP_VARIANTS } from "../bindings/variants";
import { isArray, isObject } from "../mirror/patch";
import { pointerToken, stringAt, valueAt } from "../mirror/select";
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
  for (const fixture of selection) {
    if (groupOf(show, fixture, attribute) !== null) {
      available += 1;
    }
    const value = valueFor(programmer, fixture, attribute);
    if (value === null) {
      continue;
    }
    held += 1;
    if (level === null) {
      level = value;
    } else if (level !== value) {
      mixed = true;
    }
  }
  return { attribute, index, level: mixed ? null : level, mixed, held, available };
}

/** The programmer's value for one fixture and attribute, or `null`. */
export function valueFor(
  programmer: ProgrammerState | null,
  fixture: number,
  attribute: AttributeType,
): number | null {
  if (programmer === null) {
    return null;
  }
  for (const entry of programmer.values) {
    if (entry.fixture === fixture && entry.attribute === attribute) {
      return entry.value.value;
    }
  }
  return null;
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
  const typeId = stringAt(show, `/fixtures/${String(fixture)}/typeId`);
  if (typeId === null) {
    return null;
  }
  const attributes = valueAt(show, `/fixtureTypes/${pointerToken(typeId)}/attributes`);
  if (attributes === null || !isArray(attributes)) {
    return null;
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
 * A dash for untouched, because **absent is not zero**: the programmer is
 * sparse, absence means *the playbacks decide*, and an encoder that displayed
 * 0 % would teach an operator that the two are the same thing. `mixed` for a
 * selection holding two different values, because averaging them would invent a
 * number nobody set.
 */
export function valueText(reading: ParameterReading): string {
  if (reading.mixed) {
    return "mixed";
  }
  return reading.level === null ? "—" : `${String(percentOfLevel(reading.level))}%`;
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
