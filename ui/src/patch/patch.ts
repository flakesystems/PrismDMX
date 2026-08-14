/**
 * Reading the patch out of the show document.
 *
 * The third file of the kind `canvas/windows.ts` and `desk/session.ts` started:
 * **readers over the document, never a parsed copy**. Nothing here holds
 * anything, every string is narrowed against the generated tables, and a row
 * changes exactly when the document it is read from does.
 *
 * # What is deliberately *not* worked out here
 *
 * Two things, and both are the same rule.
 *
 * **Whether an address clashes.** `prism_core::conflict` decides that, and the
 * interface asks (`Query::PatchConflicts`, `Query::PatchPreview`). A client that
 * intersected the spans itself would be a second opinion about something the
 * daemon owns — the duplication **D3** exists to prevent — and it would be the
 * copy that went stale the first time a footprint rule changed.
 *
 * **Which channels a fixture ends on.** That is `address + footprint − 1`, which
 * looks harmless and is the *same* arithmetic the overlap search runs. So the
 * sheet shows the start address and the footprint, and the **span** is only ever
 * shown from a `PatchPreview`, where the daemon computed it. One less place for
 * the two to disagree.
 *
 * What *is* here is the show document, read: which fixtures there are, what they
 * are called, which profile each instantiates and how wide it is.
 */

import type { JsonValue } from "../bindings";
import { isObject } from "../mirror/patch";
import { numberAt, stringAt, valueAt } from "../mirror/select";

/** Where the patch lives in the show document. */
export const FIXTURES = "/fixtures";

/** Where the embedded profiles live. */
export const FIXTURE_TYPES = "/fixtureTypes";

/** One line of the patch sheet. */
export interface PatchRow {
  /** The fixture number, which is what every command names it by. */
  readonly id: number;
  /** What the operator called it. */
  readonly name: string;
  /** The profile key it instantiates. */
  readonly typeId: string;
  /**
   * The profile's name, or the key when the show does not carry the profile.
   *
   * A patched fixture whose profile is missing is a real state — only a
   * hand-edited file produces it, and `prism_core::ShowIssue` reports it — so
   * the row shows the key rather than a blank, because the key is the thing
   * somebody would have to go and find.
   */
  readonly typeName: string;
  /** The universe. */
  readonly universe: number;
  /** The start address, `1..=512`. */
  readonly address: number;
  /** How many channels it occupies, or 0 when the profile is missing. */
  readonly footprint: number;
}

/** One profile, whether embedded in the show or offered by the desk. */
export interface ProfileRow {
  /** The key `PatchFixture` and `EmbedFixtureType` name it by. */
  readonly id: string;
  /** Who makes it. */
  readonly manufacturer: string;
  /** What it is called. */
  readonly name: string;
  /** Which mode of it. */
  readonly mode: string;
  /** How many channels one of them takes. */
  readonly footprint: number;
}

/**
 * The patch, in fixture-number order.
 *
 * The document keys fixtures by number *as a string*, because that is what a
 * JSON object can do; they are ordered numerically here, so fixture 2 comes
 * before fixture 10 rather than after it.
 */
export function patchRows(show: JsonValue | null): readonly PatchRow[] {
  const fixtures = valueAt(show, FIXTURES);
  if (!isObject(fixtures)) {
    return [];
  }
  const rows: PatchRow[] = [];
  for (const [key, entry] of Object.entries(fixtures)) {
    const id = Number(key);
    if (!Number.isInteger(id) || !isObject(entry)) {
      continue;
    }
    const typeId = stringAt(entry, "/typeId") ?? "";
    rows.push({
      id,
      name: stringAt(entry, "/name") ?? "",
      typeId,
      typeName: stringAt(show, `${FIXTURE_TYPES}/${typeId}/name`) ?? typeId,
      universe: numberAt(entry, "/universe") ?? 0,
      address: numberAt(entry, "/address") ?? 0,
      footprint: footprintOf(show, typeId),
    });
  }
  return rows.sort((left, right) => left.id - right.id);
}

/** The profiles this show has embedded, in key order. */
export function embeddedProfiles(show: JsonValue | null): readonly ProfileRow[] {
  const types = valueAt(show, FIXTURE_TYPES);
  if (!isObject(types)) {
    return [];
  }
  const rows: ProfileRow[] = [];
  for (const [key, entry] of Object.entries(types)) {
    if (!isObject(entry)) {
      continue;
    }
    rows.push({
      id: key,
      manufacturer: stringAt(entry, "/manufacturer") ?? "",
      name: stringAt(entry, "/name") ?? key,
      mode: stringAt(entry, "/mode") ?? "",
      footprint: numberAt(entry, "/footprint") ?? 0,
    });
  }
  return rows.sort((left, right) => left.id.localeCompare(right.id));
}

/** How wide one of a profile is, or 0 when the show has not got it. */
export function footprintOf(show: JsonValue | null, typeId: string): number {
  return numberAt(show, `${FIXTURE_TYPES}/${typeId}/footprint`) ?? 0;
}

/**
 * The lowest fixture number nothing is patched at.
 *
 * A convenience for the *number* field of a new row and nothing more: the
 * daemon decides whether a number is free, and refuses a `PatchFixture` that
 * would take one it is not. An operator is free to type any number over it.
 */
export function nextFreeFixtureId(rows: readonly PatchRow[]): number {
  const taken = new Set(rows.map((row) => row.id));
  let candidate = 1;
  while (taken.has(candidate)) {
    candidate += 1;
  }
  return candidate;
}

/**
 * How the profile menu names one entry.
 *
 * `Generic RGBW PAR · 4ch · 4 ch`, trimmed of the parts a profile does not
 * carry, because a list of keys is not something an operator can choose from.
 */
export function profileLabel(profile: ProfileRow): string {
  const words = [profile.manufacturer, profile.name].filter((part) => part !== "").join(" ");
  const parts = [words === "" ? profile.id : words];
  if (profile.mode !== "") {
    parts.push(profile.mode);
  }
  parts.push(`${String(profile.footprint)} ch`);
  return parts.join(" · ");
}
