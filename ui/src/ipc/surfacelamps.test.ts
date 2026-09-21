/**
 * **S59** — which keys of the surface are lit, as bytes.
 *
 * B55's rule before the fault rather than after it: `ui/tests/fixtures/
 * surface-lamps.json` is one real `Delta::SurfaceLampsChanged`, written by
 * `crates/prismd/tests/ui_surface_lamps.rs`, and it goes through the real
 * `decodeServerMessage` and into the real store.
 *
 * The fault this prevents is the one B55 was: a new `Delta` variant that a fake
 * daemon hands the store as an object, a decoder with no arm for it, and a
 * client that drops its connection the first time a real daemon sends one.
 */

import { describe, expect, it } from "vitest";

import fixtureText from "../../tests/fixtures/surface-lamps.json?raw";

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

describe("which keys of the surface are lit", () => {
  it("reads the daemon's delta off the wire", () => {
    const message = decodeServerMessage(overArrayBuffer(bytesOf(fixture.payload)));
    if (message.t !== "Delta" || message.delta.t !== "SurfaceLampsChanged") {
      throw new Error(`the fixture is ${message.t}, not a lamp reading`);
    }
    // **The names are the ones the rows carry.** That is the join the drawing
    // makes, so it is asserted as text rather than as a count: a delta that
    // spelled a control differently from `SurfaceControl::name` would light
    // nothing and say nothing about why.
    expect(message.delta.lit).toEqual([
      "Global.Save",
      "Global.F1",
      "Strip[*].Button.Select",
    ]);
  });

  it("is what the store holds after the delta, and nothing after the daemon has gone", () => {
    const message = decodeServerMessage(overArrayBuffer(bytesOf(fixture.payload)));
    if (message.t !== "Delta") {
      throw new Error("not a delta");
    }
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    // Nothing is lit until a daemon says so, which is also what a desk with no
    // surface plugged in looks like.
    expect(store.getState().surfaceLamps).toEqual([]);
    store.applyDelta(message.delta);
    expect(store.getState().surfaceLamps).toHaveLength(3);
    store.disconnected();
    expect(store.getState().surfaceLamps).toEqual([]);
  });
});
