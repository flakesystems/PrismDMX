/**
 * **Punch-list B52** — which position every switched slot is in, as bytes.
 *
 * B55's rule before the fault rather than after it: `ui/tests/fixtures/
 * switch-positions.json` is one real `Delta::SwitchPositions`, written by
 * `crates/prismd/tests/ui_switch_positions.rs`, and it goes through the real
 * `decodeServerMessage` and into the real store.
 */

import { describe, expect, it } from "vitest";

import fixtureText from "../../tests/fixtures/switch-positions.json?raw";

import { DeskStore } from "../store/desk";
import { snapshot } from "../testing/fake-daemon";
import { decodeServerMessage } from "./codec";
import { overArrayBuffer } from "./shape";

interface Fixture {
  readonly payload: string;
}

const fixture: Fixture = JSON.parse(fixtureText) as Fixture;

function bytesOf(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

describe("which position a switched slot is in", () => {
  it("reads the daemon's delta off the wire, a position and a none", () => {
    const message = decodeServerMessage(overArrayBuffer(bytesOf(fixture.payload)));
    if (message.t !== "Delta" || message.delta.t !== "SwitchPositions") {
      throw new Error(`the fixture is ${message.t}, not a switch reading`);
    }
    expect(message.delta.positions).toEqual([
      { fixture: 9, offset: 1, position: 1 },
      { fixture: 9, offset: 3, position: null },
    ]);
  });

  it("is what the store holds after the delta, and nothing after the daemon has gone", () => {
    const message = decodeServerMessage(overArrayBuffer(bytesOf(fixture.payload)));
    if (message.t !== "Delta") {
      throw new Error("not a delta");
    }
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    expect(store.getState().switchPositions).toEqual([]);
    store.applyDelta(message.delta);
    expect(store.getState().switchPositions).toHaveLength(2);
    store.disconnected();
    expect(store.getState().switchPositions).toEqual([]);
  });
});
