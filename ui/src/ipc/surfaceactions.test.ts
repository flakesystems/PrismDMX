/**
 * **Punch-list B55 (GitHub #23).** Binding a key in the Controls panel
 * disconnected the client, and every later visit to the panel did it again.
 *
 * `readSurfaceAction` is written out arm by arm and had no arm for S43's
 * `OpenWindowPicker` or `WriteCommandLine`, so the first table that carried one
 * could not be read and the connection was dropped. The panel's own tests never
 * saw it: they drive a fake daemon that hands over objects, not bytes.
 *
 * So this file reads bytes. `ui/tests/fixtures/surface-actions.json` is one
 * `Answer::SurfaceBindings` that binds **every** variant, written by
 * `crates/prismd/tests/ui_surface_actions.rs` — whose variant list is a `match`
 * with no wildcard, so a variant added in Rust cannot reach a release without
 * reaching this test first.
 */

import { describe, expect, it } from "vitest";

import fixtureText from "../../tests/fixtures/surface-actions.json?raw";

import { decodeServerMessage } from "./codec";
import { overArrayBuffer } from "./shape";

interface Fixture {
  readonly actions: number;
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

describe("every action a key can be bound to", () => {
  it("reads a binding table that uses all of them, rather than dropping the connection", () => {
    const message = decodeServerMessage(overArrayBuffer(bytesOf(fixture.payload)));
    if (message.t !== "Answer" || message.answer.t !== "SurfaceBindings") {
      throw new Error(`the fixture is ${message.t}, not a binding table`);
    }
    const actions = message.answer.controls.map((control) => control.action);
    expect(actions).toHaveLength(fixture.actions);
    // Written out by hand rather than read off the binding: the tag set is the
    // thing under test, and the fixture's own Rust is what guarantees it is the
    // whole vocabulary.
    expect(new Set(actions.map((action) => action?.t))).toEqual(
      new Set([
        "ExecutorMaster",
        "ExecutorGo",
        "ExecutorOff",
        "ExecutorOn",
        "ConsoleWord",
        "ExecutorButton",
        "SelectExecutor",
        "ClearProgrammer",
        "ExecutorPage",
        "SelectView",
        "StepView",
        "ProgrammerPage",
        "SelectProgrammerParam",
        "AdjustParameter",
        "SetEncoderBank",
        "OpenWindow",
        "OpenWindowPicker",
        "WriteCommandLine",
        "SaveShow",
        "Oops",
        "Redo",
      ]),
    );
  });

  it("keeps a bound line and whether it is sent", () => {
    const message = decodeServerMessage(overArrayBuffer(bytesOf(fixture.payload)));
    if (message.t !== "Answer" || message.answer.t !== "SurfaceBindings") {
      throw new Error("the fixture is not a binding table");
    }
    const lines = message.answer.controls
      .map((control) => control.action)
      .filter((action) => action?.t === "WriteCommandLine");
    expect(lines).toEqual([
      { t: "WriteCommandLine", line: "Store Cue", submit: false },
      { t: "WriteCommandLine", line: "Go Executor 1", submit: true },
    ]);
  });
});
