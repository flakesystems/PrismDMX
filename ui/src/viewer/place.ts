/**
 * The two gestures that hang a rig, as the commands they send — **S30**.
 *
 * Both are one `Command::PlaceFixtures`, so both are one Oops however many
 * fixtures they move (the rule S57 gave patching several at once). Both work
 * on the **selection**, which is the programmer's — the fixtures the operator
 * has picked on the line, in the Fixture Sheet or by clicking them in the view
 * — and both send what each fixture *is* after the gesture, whole: a field
 * left blank keeps the fixture's own value rather than sending a nought.
 *
 * - **Set** puts every selected fixture at the numbers typed. Six heads given
 *   the same height and the same tip is the common case: a truss.
 * - **Spread** lays them out across the stage in selection order, centred on
 *   the `X` typed (or on nought) and the spacing apart. `Fixture 1 Thru 8`,
 *   *Spread*, is a truss of eight hung in one gesture.
 *
 * Nothing here checks what the daemon checks: a place off the stage or a
 * fixture that has gone is refused there, by name, and the line says so.
 */

import type { FixturePlace } from "../bindings";
import type { RigFixture } from "./rig";
import type { V3 } from "./space";

/** What the operator typed, one string per field — blank is *keep*. */
export interface PlaceFields {
  readonly x: string;
  readonly y: string;
  readonly z: string;
  /** Rotation about the across axis — the tip. */
  readonly rx: string;
  /** Rotation about the vertical — the heading. */
  readonly ry: string;
  /** Rotation about the depth axis — the roll. */
  readonly rz: string;
  /** How far apart a spread puts them, in metres. */
  readonly spacing: string;
}

/** Every field blank. */
export const BLANK: PlaceFields = { x: "", y: "", z: "", rx: "", ry: "", rz: "", spacing: "" };

/** How far apart a spread puts fixtures when no spacing is typed, in metres. */
export const DEFAULT_SPACING = 1;

/**
 * A typed number, or `null` for a blank field. A comma is read as a decimal
 * point — a German keyboard types `2,5` — and anything else that is not a
 * finite number is `NaN`, which the form refuses to send.
 */
export function readField(text: string): number | null {
  const trimmed = text.trim().replace(",", ".");
  if (trimmed === "") {
    return null;
  }
  const value = Number(trimmed);
  return Number.isFinite(value) ? value : Number.NaN;
}

/** Whether every field is blank or a number. */
export function fieldsAreNumbers(fields: PlaceFields): boolean {
  return Object.values(fields).every((text) => !Number.isNaN(readField(text) ?? 0));
}

/** The fields for one fixture as it hangs, so the form starts from the truth. */
export function fieldsOf(fixture: RigFixture | undefined): PlaceFields {
  if (fixture === undefined) {
    return BLANK;
  }
  const text = (value: number): string => String(Math.round(value * 1000) / 1000);
  return {
    x: text(fixture.position.x),
    y: text(fixture.position.y),
    z: text(fixture.position.z),
    rx: text(fixture.rotation.x),
    ry: text(fixture.rotation.y),
    rz: text(fixture.rotation.z),
    spacing: "",
  };
}

/** The selected fixtures that are in the rig, in selection order. */
export function selectedFixtures(
  rig: readonly RigFixture[],
  selection: readonly number[],
): readonly RigFixture[] {
  const byId = new Map(rig.map((fixture) => [fixture.id, fixture]));
  const chosen: RigFixture[] = [];
  for (const id of selection) {
    const fixture = byId.get(id);
    if (fixture !== undefined && !chosen.includes(fixture)) {
      chosen.push(fixture);
    }
  }
  return chosen;
}

/** A vector with each typed field in place of the fixture's own. */
function merged(own: V3, x: number | null, y: number | null, z: number | null): V3 {
  return { x: x ?? own.x, y: y ?? own.y, z: z ?? own.z };
}

/** *Set*: every fixture at the typed numbers, blanks kept. */
export function placeSet(fixtures: readonly RigFixture[], fields: PlaceFields): FixturePlace[] {
  const [x, y, z, rx, ry, rz] = [fields.x, fields.y, fields.z, fields.rx, fields.ry, fields.rz].map(readField);
  return fixtures.map((fixture) => ({
    id: fixture.id,
    position: merged(fixture.position, x ?? null, y ?? null, z ?? null),
    rotation: merged(fixture.rotation, rx ?? null, ry ?? null, rz ?? null),
  }));
}

/**
 * *Spread*: across the stage in selection order, centred on the typed `X` (or
 * nought) and `spacing` apart; the other fields as *Set* reads them.
 */
export function placeSpread(fixtures: readonly RigFixture[], fields: PlaceFields): FixturePlace[] {
  const centre = readField(fields.x) ?? 0;
  const spacing = readField(fields.spacing) ?? DEFAULT_SPACING;
  const first = centre - ((fixtures.length - 1) * spacing) / 2;
  return placeSet(fixtures, { ...fields, x: "" }).map((place, index) => ({
    ...place,
    position: { ...place.position, x: first + index * spacing },
  }));
}
