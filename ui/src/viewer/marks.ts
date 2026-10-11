/**
 * Stage marks, as the calibration window needs them - **B70**.
 *
 * A mark is a point of the room whose place is known: a spike on the floor, the
 * corner of a platform. They belong to the **show** (`prism_domain::Mark`), so
 * they are read out of the show document like the rig is, and an edit is one
 * `EditMarks` - one gesture, one Oops, however many marks it touches.
 *
 * Nothing here checks what the daemon checks. A name that is too long or a place
 * off the stage is refused there, by number, and the line says so.
 */

import type { Command, JsonValue, Mark } from "../bindings";
import { isObject } from "../mirror/patch";
import { valueAt } from "../mirror/select";
import type { V3 } from "./space";

/** The most characters a mark's name may have, as `prism_domain::MAX_MARK_NAME` says. */
export const MAX_MARK_NAME = 64;

/** What the operator typed for one mark, one string per field. */
export interface MarkFields {
  readonly name: string;
  readonly x: string;
  readonly y: string;
  readonly z: string;
}

/** The form of a mark that has not been typed yet. */
export const BLANK_MARK: MarkFields = { name: "", x: "", y: "", z: "" };

/** A number, or `null` for anything that is not a finite one. */
function finite(value: JsonValue | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/** Every mark of the show, in number order. */
export function marksOf(show: JsonValue | null): readonly Mark[] {
  const pool = valueAt(show, "/marks");
  if (pool === null || !isObject(pool)) {
    return [];
  }
  const marks: Mark[] = [];
  for (const [key, entry] of Object.entries(pool)) {
    const id = Number(key);
    if (!Number.isInteger(id) || !isObject(entry)) {
      continue;
    }
    const place = entry.position;
    if (place === undefined || !isObject(place)) {
      continue;
    }
    const [x, y, z] = [finite(place.x), finite(place.y), finite(place.z)];
    if (x === null || y === null || z === null) {
      continue;
    }
    marks.push({
      id,
      name: typeof entry.name === "string" ? entry.name : "",
      position: { x, y, z },
    });
  }
  return marks.sort((left, right) => left.id - right.id);
}

/** What a mark is called on a button: its name, or `Mark 3` while it has none. */
export function markLabel(mark: Pick<Mark, "id" | "name">): string {
  return mark.name.trim() === "" ? `Mark ${String(mark.id)}` : mark.name;
}

/** The lowest number above every mark there is, so a new mark never replaces one. */
export function nextMarkId(marks: readonly Pick<Mark, "id">[]): number {
  return marks.reduce((highest, mark) => Math.max(highest, mark.id), 0) + 1;
}

/** The form for a mark as it stands. */
export function fieldsOfMark(mark: Mark): MarkFields {
  const text = (value: number): string => String(Math.round(value * 1000) / 1000);
  return {
    name: mark.name,
    x: text(mark.position.x),
    y: text(mark.position.y),
    z: text(mark.position.z),
  };
}

/**
 * A typed number, or `NaN` for anything that is not one. A comma is a decimal
 * point - a German keyboard types `2,5` - and a blank field is *not* nought:
 * a mark whose height was never typed is a mark that has not been placed.
 */
function typed(text: string): number {
  const trimmed = text.trim().replace(",", ".");
  return trimmed === "" ? Number.NaN : Number(trimmed);
}

/** Whether a form can be sent: three numbers and a name that fits. */
export function markFieldsAreValid(fields: MarkFields): boolean {
  return (
    [fields.x, fields.y, fields.z].every((text) => Number.isFinite(typed(text))) &&
    [...fields.name].length <= MAX_MARK_NAME
  );
}

/** The mark a form stands for, or `null` while it is not valid. */
export function markOfFields(id: number, fields: MarkFields): Mark | null {
  if (!markFieldsAreValid(fields)) {
    return null;
  }
  return {
    id,
    name: fields.name.trim(),
    position: { x: typed(fields.x), y: typed(fields.y), z: typed(fields.z) },
  };
}

/** Puts a mark in the show - or replaces the one with its number. */
export function setMark(mark: Mark): Command {
  return { t: "EditMarks", set: [mark], remove: [] };
}

/** Takes marks out of the show. */
export function removeMarks(ids: readonly number[]): Command {
  return { t: "EditMarks", set: [], remove: [...ids] };
}

/** A mark's place as the viewer's vector. */
export function markPosition(mark: Mark): V3 {
  return mark.position;
}
