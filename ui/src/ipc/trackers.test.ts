/**
 * What the tracker panel is drawn from, read as **bytes** — S32.
 *
 * `ui/tests/fixtures/trackers.json` is three server messages written by
 * `crates/prismd/tests/ui_trackers.rs` through the real encoder, and this decodes
 * them through the real `decodeServerMessage`. The panel's own tests drive a fake
 * daemon that hands over objects, so a field `protocol.ts` forgot would be a
 * connection that drops the first time a desk has a tracker and nothing there
 * would say so — punch-list B55's shape, a third time.
 *
 * The age of a tracker nobody has heard is `u64::MAX`, the one integer a
 * JavaScript number cannot hold, and it is in the fixture on purpose.
 */

import { describe, expect, it } from "vitest";

import fixtureText from "../../tests/fixtures/trackers.json?raw";

import { decodeServerMessage } from "./codec";
import { overArrayBuffer } from "./shape";

interface Fixture {
  readonly answer: string;
  readonly machine: string;
  readonly follows: string;
}

const fixture: Fixture = JSON.parse(fixtureText) as Fixture;

function decoded(base64: string) {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return decodeServerMessage(overArrayBuffer(bytes));
}

describe("the tracker answer", () => {
  const message = decoded(fixture.answer);
  if (message.t !== "Answer" || message.answer.t !== "Trackers") {
    throw new Error(`the fixture is ${message.t}, not a tracker answer`);
  }
  const answer = message.answer;

  it("says whether the receiver is open before it lists anything", () => {
    expect(answer.listening).toBe(true);
    expect(answer.error).toBeNull();
    expect(answer.rejected).toBe(7);
  });

  it("reads a tracker that is being heard", () => {
    expect(answer.trackers[0]).toEqual({
      id: 3,
      name: "Anna",
      position: { x: 1.25, y: 1.7, z: -4.5 },
      ageMs: 40,
      health: "Live",
      followers: 2,
    });
  });

  it("reads one that has gone quiet, with no name", () => {
    expect(answer.trackers[1]).toMatchObject({ id: 4, name: null, ageMs: 12345, health: "Quiet" });
  });

  it("reads one that was named and never heard, whose age is the largest integer there is", () => {
    const never = answer.trackers[2];
    expect(never?.id).toBe(9);
    expect(never?.name).toBe("Ben");
    // Not an exact number: a double cannot hold `u64::MAX`, and what matters is
    // that it is huge and that the connection did not drop reading it.
    expect(never?.ageMs).toBeGreaterThan(Number.MAX_SAFE_INTEGER);
    expect(never?.followers).toBe(1);
  });
});

describe("the machine's tracker settings", () => {
  it("reads the settings the daemon sent and not the defaults it knows", () => {
    const message = decoded(fixture.machine);
    if (message.t !== "Delta" || message.delta.t !== "MachineChanged") {
      throw new Error(`the fixture is ${message.t}, not a machine change`);
    }
    expect(message.delta.settings.trackers).toEqual({
      enabled: true,
      interface: "192.168.1.20",
      group: "236.10.10.11",
      port: 56570,
      mapping: {
        x: { from: "X", invert: true },
        y: { from: "Z", invert: false },
        z: { from: "Y", invert: true },
        scale: 0.001,
        offset: { x: 1.5, y: 0, z: -2 },
      },
      timeoutMs: 750,
    });
  });
});

describe("a fixture's tracker, in the show document", () => {
  it("arrives as the three operations a placement makes", () => {
    const message = decoded(fixture.follows);
    if (message.t !== "Delta" || message.delta.t !== "ShowPatch") {
      throw new Error(`the fixture is ${message.t}, not a show patch`);
    }
    expect(message.delta.ops.map((op) => [op.op, op.path])).toEqual([
      ["add", "/fixtures/1/follow"],
      ["replace", "/fixtures/1/follow"],
      ["remove", "/fixtures/1/follow"],
    ]);
  });
});
