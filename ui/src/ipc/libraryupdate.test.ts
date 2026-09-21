/**
 * **S62** — how far an update of the fixture library has got, as bytes.
 *
 * B55's rule before the fault rather than after it:
 * `ui/tests/fixtures/library-update.json` holds three real
 * `Delta::LibraryUpdate`s, written by
 * `crates/prismd/tests/ui_library_update.rs`, and each one goes through the
 * real `decodeServerMessage` and into the real store. Three, because the
 * variant has three states and the progress row draws each differently.
 */

import { describe, expect, it } from "vitest";

import fixtureText from "../../tests/fixtures/library-update.json?raw";

import { libraryUpdateText } from "../settings/settings";
import { DeskStore } from "../store/desk";
import { snapshot } from "../testing/fake-daemon";
import { decodeServerMessage } from "./codec";
import { overArrayBuffer } from "./shape";
import type { Delta } from "../bindings";

interface Fixture {
  readonly starting: string;
  readonly running: string;
  readonly finished: string;
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

/** One payload, decoded and narrowed to the delta it carries. */
function deltaOf(base64: string): Delta {
  const message = decodeServerMessage(overArrayBuffer(bytesOf(base64)));
  if (message.t !== "Delta") {
    throw new Error(`the fixture is ${message.t}, not a delta`);
  }
  return message.delta;
}

describe("an update of the fixture library", () => {
  it("reads the daemon's three states off the wire", () => {
    expect(deltaOf(fixture.starting)).toEqual({
      t: "LibraryUpdate",
      done: 0,
      total: 0,
      finished: false,
      message: "",
    });
    expect(deltaOf(fixture.running)).toEqual({
      t: "LibraryUpdate",
      done: 412,
      total: 3000,
      finished: false,
      message: "",
    });
    expect(deltaOf(fixture.finished)).toEqual({
      t: "LibraryUpdate",
      done: 3000,
      total: 3000,
      finished: true,
      message: "Library updated: 2998 fixtures from 3000 published, 2 skipped",
    });
  });

  it("is what the store holds, and nothing after the daemon has gone", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    expect(store.getState().libraryUpdate).toBeNull();

    store.applyDelta(deltaOf(fixture.starting));
    expect(store.getState().libraryUpdate).toEqual({
      done: 0,
      total: 0,
      finished: false,
      message: "",
    });
    // Still running, so nothing has been said to the operator yet.
    store.applyDelta(deltaOf(fixture.running));
    expect(store.getState().notices).toEqual([]);
    expect(store.getState().libraryUpdate?.done).toBe(412);

    // The end of it is a notice as well, so it reaches a desk whose settings
    // window is shut — which is where it will be, ten minutes in.
    store.applyDelta(deltaOf(fixture.finished));
    expect(store.getState().libraryUpdate?.finished).toBe(true);
    expect(store.getState().notices).toHaveLength(1);
    expect(store.getState().notices[0]?.message).toContain("2998 fixtures");

    // The download was the daemon's and it went with it.
    store.disconnected();
    expect(store.getState().libraryUpdate).toBeNull();
  });

  it("says something different about each state in the progress row", () => {
    const rows = [fixture.starting, fixture.running, fixture.finished].map((payload) => {
      const delta = deltaOf(payload);
      if (delta.t !== "LibraryUpdate") {
        throw new Error("not a library update");
      }
      return libraryUpdateText({
        done: delta.done,
        total: delta.total,
        finished: delta.finished,
        message: delta.message,
      });
    });
    expect(rows[0]).toBe("Signing in and asking what is published…");
    expect(rows[1]).toBe("412 of 3000 fixtures…");
    expect(rows[2]).toBe("Library updated: 2998 fixtures from 3000 published, 2 skipped");
    expect(new Set(rows).size).toBe(3);
  });
});
