/**
 * What the calibration window is drawn from, read as **bytes** - B70.
 *
 * `ui/tests/fixtures/calibration.json` is four server messages written by
 * `crates/prismd/tests/ui_calibration.rs` through the real encoder, and this
 * decodes them through the real `decodeServerMessage`. The window's own tests
 * drive a fake daemon that hands over objects, so a field `protocol.ts` forgot
 * would be a connection that drops on the first sighting - punch-list B55's
 * shape, a fourth time.
 */

import { describe, expect, it } from "vitest";

import fixtureText from "../../../tests/fixtures/calibration.json?raw";

import { decodeServerMessage } from "./codec";
import { overArrayBuffer } from "./shape";

interface Fixture {
  readonly solved: string;
  readonly refused: string;
  readonly aimed: string;
  readonly nothingToAim: string;
}

const fixture: Fixture = JSON.parse(fixtureText) as Fixture;

function answerOf(base64: string) {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  const message = decodeServerMessage(overArrayBuffer(bytes));
  if (message.t !== "Answer" || message.answer.t !== "Calibration") {
    throw new Error(`the fixture is ${message.t}, not a calibration answer`);
  }
  return message.answer;
}

describe("the calibration answers", () => {
  it("reads a pose that fits, with the number of the fixture it is about", () => {
    const answer = answerOf(fixture.solved);
    expect(answer.fixture).toBe(7);
    expect(answer.answer).toEqual({
      t: "Solved",
      position: { x: 1.25, y: 6.5, z: -2 },
      rotation: { x: 4.5, y: -170, z: 0 },
      fits: [
        { residual: 0.05, distance: 7.25 },
        { residual: 0.5, distance: 4.5 },
        { residual: 0, distance: 9 },
      ],
      rms: 0.25,
      worst: 0.5,
      positionSigma: 0.0625,
      rotationSigma: 0.125,
      spare: 0,
      iterations: 12,
    });
  });

  it("reads a refusal in the daemon's own words", () => {
    expect(answerOf(fixture.refused).answer).toEqual({
      t: "Refused",
      reason: "2 sightings are not enough: this needs at least 3",
    });
  });

  it("reads where to drive a head, and whether the travel reaches", () => {
    expect(answerOf(fixture.aimed).answer).toEqual({
      t: "Aimed",
      aim: { pan: 40000, tilt: 20000, reached: false },
    });
  });

  it("reads that there is nothing to drive", () => {
    expect(answerOf(fixture.nothingToAim).answer).toEqual({ t: "Aimed", aim: null });
  });
});
