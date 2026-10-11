/**
 * Calibrating a head from the viewer - **B70**, the logic.
 *
 * The workflow is the one a rigger would do with a tape and a torch, with the
 * arithmetic done by the desk:
 *
 * 1. The show has *marks* - points of the room whose place is known (`marks.ts`).
 * 2. For each mark the desk **aims** the head at it from its best estimate of
 *    where the head hangs (`Query::CalibrationAim`); the operator trims pan and
 *    tilt with the encoders until the real beam is on the real mark, and
 *    **takes** the sighting - the pan and tilt the programmer then holds.
 * 3. From three sightings (two, if the position is held) the daemon works out
 *    where the head really hangs (`Query::CalibrationSolve`). Every further
 *    sighting improves the estimate the *next* aim starts from, and checks the
 *    ones before it.
 * 4. **Apply** sends one `PlaceFixtures` - one Oops.
 *
 * Everything that is arithmetic is the daemon's. This file reads values out of
 * the programmer, builds the questions, and says in words what the answers
 * mean. It holds no state: the panel does.
 */

import type {
  AimedAt,
  Answer,
  CalibrationAnswer,
  Command,
  FixturePlace,
  Mark,
  ProgrammerState,
  Query,
} from "../bindings";
import { markLabel } from "./marks";
import type { RigFixture } from "./rig";
import type { V3 } from "./space";

/** One sighting the operator took: this mark was on the beam at this pan and tilt. */
export interface Sighting {
  /** The mark. */
  readonly markId: number;
  /** Pan, as the programmer held it - `0..=65535`. */
  readonly pan: number;
  /** Tilt, as the programmer held it. */
  readonly tilt: number;
}

/** A pose the search starts from, or ended at. */
export interface Estimate {
  readonly position: V3;
  readonly rotation: V3;
}

/** The solved case of {@link CalibrationAnswer}. */
export type Solved = Extract<CalibrationAnswer, { t: "Solved" }>;

/** How many sightings a calibration needs: `Hold::unknowns` in Rust, halved and rounded up. */
export function sightingsNeeded(holdPosition: boolean): number {
  return holdPosition ? 2 : 3;
}

/**
 * Whether a fixture can be calibrated at all: it has a pan and a tilt. The
 * daemon answers the same question (`Show::pan_tilt_axes`); this one only
 * decides whether to offer the panel.
 */
export function canBeCalibrated(fixture: RigFixture): boolean {
  return fixture.channels.Pan !== undefined && fixture.channels.Tilt !== undefined;
}

/**
 * The fixture the panel works on: the one fixture selected, if it has a pan and
 * a tilt. A selection of several is refused rather than shortened to its first,
 * because `SetAttribute` - which is how a head is driven - moves **every**
 * selected fixture, and aiming one head would swing six.
 */
export function targetOf(
  rig: readonly RigFixture[],
  selection: readonly number[],
): RigFixture | null {
  if (selection.length !== 1) {
    return null;
  }
  const fixture = rig.find((each) => each.id === selection[0]);
  return fixture !== undefined && canBeCalibrated(fixture) ? fixture : null;
}

/** Why there is no target, in words. */
export function noTargetText(selection: readonly number[], rig: readonly RigFixture[]): string {
  if (selection.length === 0) {
    return "Select the moving head to calibrate - click it in the view, in the Fixture Sheet or on the line.";
  }
  if (selection.length > 1) {
    return "Select one head at a time: driving a head moves everything that is selected.";
  }
  const fixture = rig.find((each) => each.id === selection[0]);
  return fixture === undefined
    ? "That fixture is not patched."
    : `Fixture ${String(fixture.id)} has no pan and tilt to drive.`;
}

/** The pan and tilt the programmer holds for a fixture, or `null` while it holds neither. */
export function heldPanTilt(
  programmer: ProgrammerState | null,
  fixture: number,
): { readonly pan: number; readonly tilt: number } | null {
  let pan: number | null = null;
  let tilt: number | null = null;
  for (const entry of programmer?.values ?? []) {
    if (entry.fixture !== fixture || entry.occurrence !== 0) {
      continue;
    }
    if (entry.attribute === "Pan") {
      pan = entry.value.value;
    } else if (entry.attribute === "Tilt") {
      tilt = entry.value.value;
    }
  }
  return pan === null || tilt === null ? null : { pan, tilt };
}

/** The question: where must the head point to hit this mark, if it hangs as estimated? */
export function aimQuery(
  fixture: number,
  mark: Pick<Mark, "position">,
  estimate: Estimate,
  near: { readonly pan: number; readonly tilt: number } | null,
): Query {
  return {
    t: "CalibrationAim",
    fixture,
    target: mark.position,
    position: estimate.position,
    rotation: estimate.rotation,
    near: near === null ? null : [near.pan, near.tilt],
  };
}

/** The two commands that drive the selected head to an aim. */
export function driveCommands(aim: Pick<AimedAt, "pan" | "tilt">): Command[] {
  return [
    { t: "SetAttribute", attribute: "Pan", occurrence: 0, value: aim.pan, relative: false },
    { t: "SetAttribute", attribute: "Tilt", occurrence: 0, value: aim.tilt, relative: false },
  ];
}

/** Reads an aim out of an answer, or `null` for anything else. */
export function aimOf(answer: Answer | null, fixture: number): AimedAt | null {
  if (answer?.t !== "Calibration" || answer.fixture !== fixture) {
    return null;
  }
  return answer.answer.t === "Aimed" ? answer.answer.aim : null;
}

/**
 * The question: where does the head hang, if these sightings are true? `null`
 * while there are not enough of them - a question the daemon would only refuse.
 */
export function solveQuery(
  fixture: number,
  start: Estimate,
  holdPosition: boolean,
  sightings: readonly Sighting[],
  marks: readonly Mark[],
): Query | null {
  const known = new Map(marks.map((mark) => [mark.id, mark]));
  const usable = sightings.flatMap((sighting) => {
    const mark = known.get(sighting.markId);
    return mark === undefined
      ? []
      : [{ target: mark.position, pan: sighting.pan, tilt: sighting.tilt }];
  });
  if (usable.length < sightingsNeeded(holdPosition)) {
    return null;
  }
  return {
    t: "CalibrationSolve",
    fixture,
    position: start.position,
    rotation: start.rotation,
    holdPosition,
    sightings: usable,
  };
}

/** What a solve came back with: a pose, the daemon's refusal, or nothing for another fixture. */
export type Outcome =
  | { readonly t: "Solved"; readonly solved: Solved }
  | { readonly t: "Refused"; readonly reason: string };

/** Reads the outcome of a solve out of an answer. */
export function outcomeOf(answer: Answer | null, fixture: number): Outcome | null {
  if (answer?.t !== "Calibration" || answer.fixture !== fixture) {
    return null;
  }
  const body = answer.answer;
  if (body.t === "Solved") {
    return { t: "Solved", solved: body };
  }
  return body.t === "Refused" ? { t: "Refused", reason: body.reason } : null;
}

/** The pose a solve found, as the estimate the next aim starts from. */
export function estimateOf(solved: Solved): Estimate {
  return { position: solved.position, rotation: solved.rotation };
}

/** Where a fixture hangs now, as an estimate. */
export function placeOf(fixture: RigFixture): Estimate {
  return { position: fixture.position, rotation: fixture.rotation };
}

/**
 * The `PlaceFixtures` that applies a solve: the solved pose, and everything else
 * about the fixture's place (the tracker it follows, the mirror) as it was - a
 * place is sent whole.
 */
export function applyCommand(fixture: RigFixture, solved: Solved, holdPosition: boolean): Command {
  const place: FixturePlace = {
    id: fixture.id,
    // A held position is sent back exactly as it was, not as the daemon echoes
    // it: the same number, to the last bit.
    position: holdPosition ? fixture.position : solved.position,
    rotation: solved.rotation,
    ...(fixture.follow === null ? {} : { follow: fixture.follow }),
    ...(fixture.mirror.pan || fixture.mirror.tilt ? { mirror: fixture.mirror } : {}),
  };
  return { t: "PlaceFixtures", placements: [place] };
}

/** How well the sightings agree, in the colours of a status line. */
export type Level = "good" | "warn" | "bad";

/** A verdict on a solve: how much to trust it, and what to do if not. */
export interface Verdict {
  readonly level: Level;
  readonly lines: readonly string[];
}

/**
 * Sightings that agree to within this many degrees are as good as a person can
 * aim a beam by eye at a mark on the floor.
 */
export const GOOD_RMS = 0.5;

/** Past this the sightings disagree: a mark is wrong, or was not on the beam. */
export const BAD_RMS = 1.5;

/** A position this uncertain, in metres, is not worth applying without more marks. */
export const UNCERTAIN_POSITION = 0.3;

/**
 * What the numbers of a solve mean, in words an operator can act on. The
 * thresholds are named above and the tests hold them: a number nobody can read
 * is a calibration nobody trusts or everybody trusts.
 */
export function verdictOf(
  solved: Solved,
  sightings: readonly Sighting[],
  marks: readonly Mark[],
  holdPosition: boolean,
): Verdict {
  const lines: string[] = [];
  let level: Level = "good";
  const raise = (to: Level): void => {
    if (to === "bad" || (to === "warn" && level === "good")) {
      level = to;
    }
  };
  if (solved.spare === 0) {
    raise("warn");
    lines.push(
      "Nothing was left over to check this against: take one more sighting on a different mark.",
    );
  }
  if (solved.rms > BAD_RMS) {
    raise("bad");
    const worst = solved.fits.reduce(
      (found, fit, index) => (fit.residual > (solved.fits[found]?.residual ?? -1) ? index : found),
      0,
    );
    const culprit = sightings[worst];
    const mark = marks.find((each) => each.id === culprit?.markId);
    lines.push(
      `The sightings disagree by ${solved.rms.toFixed(1)}° on average` +
        (mark === undefined
          ? "."
          : `; ${markLabel(mark)} is the furthest out (${solved.worst.toFixed(1)}°) - check that mark's place, or take it again.`),
    );
  } else if (solved.rms > GOOD_RMS) {
    raise("warn");
    lines.push(`The sightings agree to ${solved.rms.toFixed(2)}°: usable, not exact.`);
  }
  if (!holdPosition && solved.positionSigma > UNCERTAIN_POSITION) {
    raise("warn");
    lines.push(
      `The position is only good to about ±${solved.positionSigma.toFixed(2)} m: use marks further apart, ` +
        "or at different heights, or hold a position you have measured.",
    );
  }
  if (lines.length === 0) {
    lines.push(
      holdPosition
        ? `The sightings agree to ${solved.rms.toFixed(2)}°.`
        : `The sightings agree to ${solved.rms.toFixed(2)}°, the position to about ±${solved.positionSigma.toFixed(2)} m.`,
    );
  }
  return { level, lines };
}

/** A number for a table: two decimals, and no negative nought. */
export function metres(value: number): string {
  const text = (Math.round(value * 100) / 100).toFixed(2);
  return text === "-0.00" ? "0.00" : text;
}

/** A pan or a tilt value as the percentage an operator reads. */
export function percent(value: number): string {
  return `${(Math.round((value / 65535) * 1000) / 10).toFixed(1)}%`;
}
