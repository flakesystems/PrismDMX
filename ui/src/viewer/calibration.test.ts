/**
 * The calibration's logic - B70: what it asks the daemon, what it sends back,
 * and what it tells the operator about an answer.
 */

import { describe, expect, it } from "vitest";

import type { Answer, Mark, ProgrammerState } from "../bindings";
import { emptyProgrammer } from "../testing/fake-daemon";
import type { Solved } from "./calibration";
import {
  BAD_RMS,
  GOOD_RMS,
  UNCERTAIN_POSITION,
  aimOf,
  aimQuery,
  applyCommand,
  canBeCalibrated,
  driveCommands,
  estimateOf,
  heldPanTilt,
  metres,
  noTargetText,
  outcomeOf,
  percent,
  sightingsNeeded,
  solveQuery,
  targetOf,
  verdictOf,
} from "./calibration";
import type { RigFixture } from "./rig";
import { orientation } from "./space";

function head(id: number, overrides: Partial<RigFixture> = {}): RigFixture {
  const channel = { coarse: 0, fine: null, invert: false, from: -270, to: 270, ranges: [] };
  return {
    id,
    name: "",
    universe: 1,
    address: 1,
    position: { x: 1, y: 6, z: 2 },
    rotation: { x: 0, y: 30, z: 0 },
    orientation: orientation({ x: 0, y: 30, z: 0 }),
    follow: null,
    mirror: { pan: false, tilt: false },
    unplaced: false,
    size: { x: 0.3, y: 0.3, z: 0.3 },
    beams: [],
    channels: { Pan: channel, Tilt: channel },
    described: false,
    typeId: "t",
    device: null,
    ...overrides,
  };
}

const par = head(2, { channels: {} });

const marks: Mark[] = [
  { id: 1, name: "A", position: { x: 0, y: 0, z: 0 } },
  { id: 2, name: "", position: { x: 3, y: 0, z: 0 } },
  { id: 3, name: "C", position: { x: 0, y: 0, z: 4 } },
];

function programmerHolding(
  values: readonly (readonly [number, "Pan" | "Tilt" | "Dimmer", number, number?])[],
): ProgrammerState {
  return {
    ...emptyProgrammer(),
    values: values.map(([fixture, attribute, value, occurrence]) => ({
      fixture,
      attribute,
      occurrence: occurrence ?? 0,
      value: { value, source: "Manual", presetRef: null },
    })),
  };
}

const solved: Solved = {
  t: "Solved",
  position: { x: 1.5, y: 6.2, z: 2.25 },
  rotation: { x: 0, y: 31, z: 0 },
  fits: [
    { residual: 0.05, distance: 7 },
    { residual: 0.1, distance: 6 },
    { residual: 0.08, distance: 8 },
    { residual: 0.02, distance: 8 },
  ],
  rms: 0.07,
  worst: 0.1,
  positionSigma: 0.05,
  rotationSigma: 0.05,
  spare: 5,
  iterations: 9,
};

const sightings = [1, 2, 3].map((markId) => ({ markId, pan: 30000, tilt: 20000 }));

describe("the target", () => {
  it("is the one selected head with a pan and a tilt", () => {
    expect(canBeCalibrated(head(1))).toBe(true);
    expect(canBeCalibrated(par)).toBe(false);
    expect(targetOf([head(1), par], [1])?.id).toBe(1);
  });

  it("is nothing for several fixtures, for a fixture that cannot move, or for none", () => {
    expect(targetOf([head(1), head(3)], [1, 3])).toBeNull();
    expect(targetOf([head(1), par], [2])).toBeNull();
    expect(targetOf([head(1)], [])).toBeNull();
    expect(targetOf([head(1)], [9])).toBeNull();
  });

  it("says why there is none, in words that tell the operator what to do", () => {
    const rig = [head(1), head(3), par];
    expect(noTargetText([], rig)).toMatch(/Select the moving head/);
    expect(noTargetText([1, 3], rig)).toMatch(/one head at a time/);
    expect(noTargetText([2], rig)).toMatch(/no pan and tilt/);
    expect(noTargetText([9], rig)).toMatch(/not patched/);
  });
});

describe("what the programmer holds", () => {
  it("is the head's pan and tilt, and nothing until it holds both", () => {
    expect(heldPanTilt(programmerHolding([[1, "Pan", 100], [1, "Tilt", 200]]), 1)).toEqual({
      pan: 100,
      tilt: 200,
    });
    expect(heldPanTilt(programmerHolding([[1, "Pan", 100]]), 1)).toBeNull();
    expect(heldPanTilt(programmerHolding([[2, "Pan", 100], [2, "Tilt", 200]]), 1)).toBeNull();
    expect(heldPanTilt(null, 1)).toBeNull();
  });

  it("reads the first pan and tilt, not a second one", () => {
    expect(
      heldPanTilt(
        programmerHolding([[1, "Pan", 100], [1, "Tilt", 200], [1, "Pan", 999, 1]]),
        1,
      ),
    ).toEqual({ pan: 100, tilt: 200 });
  });
});

describe("the questions", () => {
  it("aim at a mark from an estimate, near where the head already points", () => {
    const estimate = { position: { x: 1, y: 6, z: 2 }, rotation: { x: 0, y: 30, z: 0 } };
    expect(aimQuery(7, marks[1]!, estimate, { pan: 5, tilt: 6 })).toEqual({
      t: "CalibrationAim",
      fixture: 7,
      target: { x: 3, y: 0, z: 0 },
      position: estimate.position,
      rotation: estimate.rotation,
      near: [5, 6],
    });
    expect(aimQuery(7, marks[1]!, estimate, null)).toMatchObject({ near: null });
  });

  it("do not solve until there are enough sightings of marks that exist", () => {
    const start = { position: { x: 0, y: 5, z: 0 }, rotation: { x: 0, y: 0, z: 0 } };
    expect(solveQuery(1, start, false, sightings.slice(0, 2), marks)).toBeNull();
    expect(solveQuery(1, start, true, sightings.slice(0, 2), marks)).not.toBeNull();
    // A sighting of a mark that has been deleted does not count.
    expect(solveQuery(1, start, false, sightings, marks.slice(0, 2))).toBeNull();
    expect(sightingsNeeded(false)).toBe(3);
    expect(sightingsNeeded(true)).toBe(2);
  });

  it("carry the marks' places and the values taken, in the order they were taken", () => {
    const start = { position: { x: 0, y: 5, z: 0 }, rotation: { x: 0, y: 0, z: 0 } };
    expect(solveQuery(1, start, false, sightings, marks)).toEqual({
      t: "CalibrationSolve",
      fixture: 1,
      position: start.position,
      rotation: start.rotation,
      holdPosition: false,
      sightings: [
        { target: marks[0]!.position, pan: 30000, tilt: 20000 },
        { target: marks[1]!.position, pan: 30000, tilt: 20000 },
        { target: marks[2]!.position, pan: 30000, tilt: 20000 },
      ],
    });
  });
});

describe("the answers", () => {
  const calibration = (answer: Extract<Answer, { t: "Calibration" }>["answer"], fixture = 1): Answer => ({
    t: "Calibration",
    fixture,
    answer,
  });

  it("read as an aim, a pose or a refusal, and only for the fixture asked about", () => {
    const aimed = calibration({ t: "Aimed", aim: { pan: 1, tilt: 2, reached: true } });
    expect(aimOf(aimed, 1)).toEqual({ pan: 1, tilt: 2, reached: true });
    expect(aimOf(aimed, 2)).toBeNull();
    expect(aimOf(calibration({ t: "Aimed", aim: null }), 1)).toBeNull();
    expect(aimOf(null, 1)).toBeNull();
    expect(aimOf(calibration(solved), 1)).toBeNull();

    expect(outcomeOf(calibration(solved), 1)).toEqual({ t: "Solved", solved });
    expect(outcomeOf(calibration({ t: "Refused", reason: "no" }), 1)).toEqual({
      t: "Refused",
      reason: "no",
    });
    expect(outcomeOf(calibration(solved), 2)).toBeNull();
    expect(outcomeOf(aimed, 1)).toBeNull();
    expect(outcomeOf(null, 1)).toBeNull();
  });

  it("drive a head with two absolute values, which move the selected fixture", () => {
    expect(driveCommands({ pan: 100, tilt: 200 })).toEqual([
      { t: "SetAttribute", attribute: "Pan", occurrence: 0, value: 100, relative: false },
      { t: "SetAttribute", attribute: "Tilt", occurrence: 0, value: 200, relative: false },
    ]);
    expect(estimateOf(solved)).toEqual({ position: solved.position, rotation: solved.rotation });
  });
});

describe("applying a pose", () => {
  it("hangs the head there and leaves its tracker and mirror alone", () => {
    const fixture = head(1, {
      follow: { tracker: 4, offset: { x: 0, y: 0.4, z: 0 } },
      mirror: { pan: false, tilt: true },
    });
    expect(applyCommand(fixture, solved, false)).toEqual({
      t: "PlaceFixtures",
      placements: [
        {
          id: 1,
          position: solved.position,
          rotation: solved.rotation,
          follow: { tracker: 4, offset: { x: 0, y: 0.4, z: 0 } },
          mirror: { pan: false, tilt: true },
        },
      ],
    });
  });

  it("sends a place with neither when the head has neither", () => {
    const command = applyCommand(head(1), solved, false);
    expect(command.t === "PlaceFixtures" && Object.keys(command.placements[0]!)).toEqual([
      "id",
      "position",
      "rotation",
    ]);
  });

  it("sends a held position back exactly as it was", () => {
    const fixture = head(1, { position: { x: 0.1 + 0.2, y: 6, z: 2 } });
    const command = applyCommand(fixture, solved, true);
    expect(command.t === "PlaceFixtures" && command.placements[0]!.position).toEqual(
      fixture.position,
    );
  });
});

describe("the verdict", () => {
  const verdict = (overrides: Partial<Solved>, hold = false) =>
    verdictOf({ ...solved, ...overrides }, sightings, marks, hold);

  it("is good for sightings that agree, and says how well", () => {
    const good = verdict({});
    expect(good.level).toBe("good");
    expect(good.lines).toEqual(["The sightings agree to 0.07°, the position to about ±0.05 m."]);
    expect(verdict({}, true).lines).toEqual(["The sightings agree to 0.07°."]);
  });

  it("warns that nothing was checked when no equation is left over", () => {
    const result = verdict({ spare: 0 });
    expect(result.level).toBe("warn");
    expect(result.lines[0]).toMatch(/one more sighting/);
  });

  it("warns of sightings that agree only roughly", () => {
    const result = verdict({ rms: (GOOD_RMS + BAD_RMS) / 2 });
    expect(result.level).toBe("warn");
    expect(result.lines[0]).toMatch(/usable, not exact/);
  });

  it("names the mark that is furthest out when the sightings disagree", () => {
    const result = verdict({
      rms: BAD_RMS + 1,
      worst: 4.2,
      fits: [
        { residual: 0.3, distance: 7 },
        { residual: 4.2, distance: 6 },
        { residual: 0.4, distance: 8 },
      ],
    });
    expect(result.level).toBe("bad");
    // Mark 2 has no name, so it is called what a button calls it.
    expect(result.lines[0]).toMatch(/Mark 2 is the furthest out \(4\.2°\)/);
  });

  it("does not name a mark that is gone", () => {
    const result = verdictOf(
      { ...solved, rms: BAD_RMS + 1 },
      [{ markId: 99, pan: 0, tilt: 0 }],
      marks,
      false,
    );
    expect(result.level).toBe("bad");
    expect(result.lines[0]).toMatch(/disagree by 2\.5° on average\.$/);
  });

  it("warns of a position that is not pinned down, unless it was held", () => {
    const loose = verdict({ positionSigma: UNCERTAIN_POSITION + 0.1 });
    expect(loose.level).toBe("warn");
    expect(loose.lines[0]).toMatch(/only good to about ±0\.40 m/);
    expect(verdict({ positionSigma: 5 }, true).level).toBe("good");
  });

  it("never lets a worse level be lowered by a better one", () => {
    const result = verdict({ spare: 0, rms: BAD_RMS + 1, positionSigma: 3 });
    expect(result.level).toBe("bad");
    expect(result.lines).toHaveLength(3);
  });
});

describe("the numbers", () => {
  it("are written to the centimetre, with no negative nought", () => {
    expect(metres(1.2345)).toBe("1.23");
    expect(metres(-0.001)).toBe("0.00");
    expect(metres(-2.5)).toBe("-2.50");
  });

  it("are written as the percentage an operator reads", () => {
    expect(percent(0)).toBe("0.0%");
    expect(percent(65535)).toBe("100.0%");
    expect(percent(32768)).toBe("50.0%");
  });
});
