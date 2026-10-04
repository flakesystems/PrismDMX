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
 * **A place carries the tracker too** (S32): where a head hangs and what it is
 * aimed at from there are one fact about it, so one command and one Oops. A
 * tracker field left blank keeps the fixture's own, which is what makes a drag
 * of a following head leave it following.
 *
 * Nothing here checks what the daemon checks: a place off the stage or a
 * fixture that has gone is refused there, by name, and the line says so.
 */

import type { FixturePlace, FollowTarget } from "../bindings";
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
  /**
   * The tracker a head follows - **S32**. A tracker number; blank *keeps* what
   * the fixture has, and {@link NO_TRACKER} takes it away.
   */
  readonly tracker: string;
  /** Where on the performer to aim, in metres added to the tracker's position. */
  readonly ox: string;
  readonly oy: string;
  readonly oz: string;
  /**
   * Whether pan and tilt run the other way from the viewer's - **S32**: `on`,
   * `off`, or blank to *keep* what each fixture has, which is what a selection
   * that disagrees starts as.
   */
  readonly mirrorPan: MirrorChoice;
  readonly mirrorTilt: MirrorChoice;
}

/** One axis's mirror, as a form holds it. */
export type MirrorChoice = "" | "on" | "off";

/** Every field blank. */
export const BLANK: PlaceFields = {
  x: "",
  y: "",
  z: "",
  rx: "",
  ry: "",
  rz: "",
  spacing: "",
  tracker: "",
  ox: "",
  oy: "",
  oz: "",
  mirrorPan: "",
  mirrorTilt: "",
};

/** What is typed into the tracker field to take a tracker away. */
export const NO_TRACKER = "-";

/** The most trackers there are, as `prism_domain::MAX_TRACKER` says. */
export const MAX_TRACKER = 1023;

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
  // The tracker and the mirrors are not numbers and are checked on their own.
  const { tracker, mirrorPan: _pan, mirrorTilt: _tilt, ...numbers } = fields;
  return Object.values(numbers).every((text) => !Number.isNaN(readField(text) ?? 0)) && trackerIsValid(tracker);
}

/**
 * Whether the tracker field is something the daemon will take: blank, the
 * *none* sign, or a whole number from nought to {@link MAX_TRACKER}.
 */
export function trackerIsValid(text: string): boolean {
  const trimmed = text.trim();
  if (trimmed === "" || trimmed === NO_TRACKER) {
    return true;
  }
  const value = Number(trimmed);
  return Number.isInteger(value) && value >= 0 && value <= MAX_TRACKER;
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
    tracker: fixture.follow === null ? "" : String(fixture.follow.tracker),
    ox: fixture.follow === null ? "" : text(fixture.follow.offset.x),
    oy: fixture.follow === null ? "" : text(fixture.follow.offset.y),
    oz: fixture.follow === null ? "" : text(fixture.follow.offset.z),
    mirrorPan: fixture.mirror.pan ? "on" : "off",
    mirrorTilt: fixture.mirror.tilt ? "on" : "off",
  };
}

/**
 * What a selection agrees about one mirror: `on` or `off` when every fixture
 * says so, and blank - *keep each its own* - when they differ.
 */
export function mirrorChoice(
  fixtures: readonly RigFixture[],
  axis: "pan" | "tilt",
): MirrorChoice {
  const [first, ...rest] = fixtures;
  if (first === undefined) {
    return "";
  }
  return rest.every((fixture) => fixture.mirror[axis] === first.mirror[axis])
    ? first.mirror[axis]
      ? "on"
      : "off"
    : "";
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

/**
 * The tracker a fixture has after a gesture: its own where the tracker field is
 * blank, none where it says {@link NO_TRACKER}, and the typed number - with the
 * typed offset over the fixture's own - otherwise. A fixture that follows
 * nothing and is given only an offset is still following nothing: an offset is
 * a part of a performer, and there is no performer.
 */
function followAfter(fixture: RigFixture, fields: PlaceFields): FollowTarget | null {
  const typed = fields.tracker.trim();
  if (typed === NO_TRACKER) {
    return null;
  }
  const own = fixture.follow;
  const tracker = typed === "" ? (own?.tracker ?? null) : Number(typed);
  if (tracker === null) {
    return null;
  }
  const [ox, oy, oz] = [fields.ox, fields.oy, fields.oz].map(readField);
  return {
    tracker,
    offset: merged(own?.offset ?? { x: 0, y: 0, z: 0 }, ox ?? null, oy ?? null, oz ?? null),
  };
}

/** A mirror after a gesture: the choice where there is one, the fixture's own otherwise. */
function mirrorAfter(own: boolean, choice: MirrorChoice): boolean {
  return choice === "" ? own : choice === "on";
}

/** *Set*: every fixture at the typed numbers, blanks kept. */
export function placeSet(fixtures: readonly RigFixture[], fields: PlaceFields): FixturePlace[] {
  const [x, y, z, rx, ry, rz] = [fields.x, fields.y, fields.z, fields.rx, fields.ry, fields.rz].map(readField);
  return fixtures.map((fixture) => {
    const follow = followAfter(fixture, fields);
    const mirror = {
      pan: mirrorAfter(fixture.mirror.pan, fields.mirrorPan),
      tilt: mirrorAfter(fixture.mirror.tilt, fields.mirrorTilt),
    };
    return {
      id: fixture.id,
      position: merged(fixture.position, x ?? null, y ?? null, z ?? null),
      rotation: merged(fixture.rotation, rx ?? null, ry ?? null, rz ?? null),
      // Absent is *no tracker*, which is what the daemon reads it as - so a
      // fixture that follows nothing sends the place it always sent.
      ...(follow === null ? {} : { follow }),
      // Absent is *neither*, for the same reason: a head that mirrors nothing
      // sends the place it always sent.
      ...(mirror.pan || mirror.tilt ? { mirror } : {}),
    };
  });
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
