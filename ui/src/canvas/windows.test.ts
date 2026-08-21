/**
 * **The canvas, held to the daemon's own answers.**
 *
 * `ui/tests/fixtures/session-recording.json` was written by
 * `crates/prismd/tests/ui_session.rs` off a running `prismd`: twelve commands
 * from a canvas being used, the payloads a client sent, the deltas the daemon
 * answered with, and — the part that matters — **which windows were then open,
 * in what order, at what coordinates, and which one had the focus**, taken from
 * a fresh client's snapshot rather than worked out.
 *
 * Nothing in this file decides what the answer should be. The commands are
 * encoded here and compared byte for byte with what `rmp-serde` made of them;
 * the deltas are applied through the interface's own mirror; and the readers in
 * `./windows.ts` are then asked the three questions the canvas asks, with the
 * daemon's answers on the other side of the equals sign.
 *
 * That is S19–S24's rule in this session's terms: *a test that uses the
 * function under test to work out what the answer should be is not a test.*
 */

import { decode, encode } from "@msgpack/msgpack";
import { beforeEach, describe, expect, it } from "vitest";

// As text rather than as a module, for S23's reason: a JSON import would give
// TypeScript a literal type with every recorded payload in it.
import recordingText from "../../tests/fixtures/session-recording.json?raw";

import type { Command, Delta, JsonValue } from "../bindings";
import { PROTOCOL_VERSION } from "../ipc/protocol";
import { readServerMessage } from "../ipc/protocol";
import { nullSink, setLogSink } from "../log/logger";
import type { Documents } from "../mirror/mirror";
import { applyDelta } from "../mirror/mirror";
import { activeViewId, focusedWindow, openWindows, storedViews, windowRect, windowTitle } from "./windows";

/** One window, exactly as the daemon reported it. */
interface RecordedWindow {
  readonly instanceId: number;
  readonly type: string;
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
}

/** One step of the script. */
interface Step {
  readonly what: string;
  readonly client: string;
  readonly clientInteger: string | null;
  readonly deltas: readonly string[];
  readonly refused: boolean;
  readonly windows: readonly RecordedWindow[];
  readonly focusedWindow: number | null;
  readonly activeViewId: number;
  readonly storedViews: readonly [number, string, number][];
}

/** The file. */
interface Recording {
  readonly protocolVersion: number;
  readonly defaultWindow: readonly [number, number, number, number];
  readonly initialSnapshot: string;
  readonly steps: readonly Step[];
  readonly finalSnapshot: string;
}

/** The recording, as the Rust target wrote it. */
const recording: Recording = JSON.parse(recordingText) as Recording;

/**
 * The commands the script sent, in order — written here so the payloads can be
 * checked against something rather than merely replayed.
 *
 * This list is `crates/prismd/tests/ui_session.rs`'s `script()`, transcribed.
 * If the two ever disagree the bytes will say so on the next run, in both
 * languages.
 */
const SCRIPT: readonly Command[] = [
  { t: "OpenWindow", window: "FixtureSheet" },
  { t: "OpenWindow", window: "DmxSheet" },
  { t: "PlaceWindow", instanceId: 1, x: 240, y: 120, w: 640, h: 480 },
  { t: "PlaceWindow", instanceId: 2, x: 0, y: 0, w: 1280, h: 720 },
  { t: "FocusWindow", instanceId: 1 },
  { t: "StoreView", viewId: 2, name: "Programming" },
  { t: "OpenWindow", window: "Patch" },
  { t: "CloseWindow", instanceId: 2 },
  { t: "PlaceWindow", instanceId: 2, x: 900, y: 900, w: 100, h: 100 },
  { t: "SelectView", viewId: 2 },
  { t: "SelectView", viewId: 9 },
  { t: "SelectView", viewId: 1 },
  // S35's eight, in the order `crates/prismd/tests/ui_session.rs` sends them —
  // said in S40's words, which is the whole of what changed: managing a view is
  // `Label`, `Move`, `Delete` and `Copy` over an `ObjectRef::View`, the same
  // four verbs every other pool takes.
  { t: "StoreView", viewId: 5, name: "Busking" },
  { t: "Label", target: { t: "View", viewId: 5 }, name: "Front of house" },
  { t: "Label", target: { t: "View", viewId: 9 }, name: "Nowhere" },
  {
    t: "Move",
    from: { t: "View", viewId: 5 },
    to: { t: "View", viewId: 2 },
    mode: "Merge",
  },
  {
    t: "Move",
    from: { t: "View", viewId: 1 },
    to: { t: "View", viewId: 1 },
    mode: "Merge",
  },
  {
    t: "Copy",
    from: { t: "View", viewId: 2 },
    to: { t: "View", viewId: 7 },
    mode: "Merge",
  },
  { t: "SelectView", viewId: 2 },
  { t: "Delete", target: { t: "View", viewId: 2 } },
  { t: "Delete", target: { t: "View", viewId: 9 } },
];

/** Bytes out of a base64 payload, the way a browser does it (S23). */
function payload(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/** The documents a recorded snapshot carries. */
function documentsOf(text: string): Documents {
  const message = readServerMessage(decode(payload(text)));
  if (message.t !== "Snapshot") {
    throw new Error("that payload is not a snapshot");
  }
  return {
    show: message.snapshot.show,
    session: message.snapshot.session,
    programmer: message.snapshot.programmer,
  };
}

/** The delta a recorded payload carries. */
function deltaOf(text: string): Delta {
  const message = readServerMessage(decode(payload(text)));
  if (message.t !== "Delta") {
    throw new Error("that payload is not a delta");
  }
  return message.delta;
}

/** What the readers say about a session document, in the recording's shape. */
function answers(session: JsonValue) {
  return {
    windows: openWindows(session).map((window) => ({
      instanceId: window.instanceId,
      type: String(window.type),
      x: window.x,
      y: window.y,
      w: window.w,
      h: window.h,
    })),
    focusedWindow: focusedWindow(session),
    activeViewId: activeViewId(session),
    storedViews: storedViews(session).map(
      (view): [number, string, number] => [view.id, view.name, view.windowCount],
    ),
  };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the recording", () => {
  it("is of the protocol this build speaks", () => {
    expect(recording.protocolVersion).toBe(PROTOCOL_VERSION);
    expect(recording.steps).toHaveLength(SCRIPT.length);
  });

  it("says where the daemon puts a window that has never been moved", () => {
    // The interface does not choose this and must not assume it: `OpenWindow`
    // carries no geometry, so a fresh window is wherever `prism-core` decided.
    expect(recording.defaultWindow).toEqual([0, 0, 640, 480]);
  });
});

describe("what the canvas reads out of a session", () => {
  /**
   * **The whole script, step by step.** The commands are this interface's own
   * bytes; the answers are the daemon's.
   */
  it("is what the daemon says is open, after every command in the script", () => {
    let documents = documentsOf(recording.initialSnapshot);
    // The canvas starts empty and view 1 is active, which is what a daemon
    // that has just opened a show holds.
    expect(answers(documents.session).windows).toEqual([]);

    recording.steps.forEach((step, index) => {
      const command = SCRIPT[index];
      expect(command, `step ${String(index)} has a command`).toBeDefined();

      // 1. The interface's encoder against the daemon's decoder, on bytes: the
      //    tag first, the fields in declaration order, the narrowest integer
      //    that fits, and no member at all for an absent `params`.
      //
      //    `clientInteger`, where the recording has one, is the same command
      //    with its whole-number coordinates written as MessagePack integers —
      //    which is what this encoder produces, because JavaScript has one kind
      //    of number and `rounded()` has already made them whole. Both forms
      //    were written by Rust, and `prism-ipc` is asserted **in Rust** to
      //    decode them to the same command, so this is still a comparison
      //    against something the daemon made.
      const encoded = encode({ t: "Command", seq: index + 1, command }, { ignoreUndefined: true });
      expect(Array.from(encoded), `step ${String(index)}: ${step.what}`).toEqual(
        Array.from(payload(step.clientInteger ?? step.client)),
      );

      // 2. The deltas the daemon answered with, through the interface's mirror.
      for (const encodedDelta of step.deltas) {
        documents = applyDelta(documents, deltaOf(encodedDelta));
      }
      // A refused command produces no delta at all — not an empty one. Only
      // that direction: a command can also be *accepted* and change nothing,
      // which `prism-core` answers with no delta rather than a `SessionPatch`
      // holding no operations. S35's script has one — moving the first view
      // further left — and it is a no-op, not a refusal.
      if (step.refused) {
        expect(step.deltas, `step ${String(index)}: a refusal spoke`).toEqual([]);
      }

      // 3. The three questions the canvas asks, against the daemon's answers.
      expect(answers(documents.session), `step ${String(index)}: ${step.what}`).toEqual({
        windows: step.windows,
        focusedWindow: step.focusedWindow,
        activeViewId: step.activeViewId,
        storedViews: step.storedViews.map(([id, name, count]) => [id, name, count]),
      });
    });

    // And what the deltas built is what a client that never saw one is served.
    expect(documents.session).toEqual(documentsOf(recording.finalSnapshot).session);
  });

  it("keeps the stacking order the daemon chose, rather than sorting by number", () => {
    // Step 5 is `FocusWindow(1)`, which moves window 1 to the end of the list.
    // A canvas that sorted its windows would draw the same two rectangles in
    // the wrong order, and every assertion about *which* windows are open
    // would still pass.
    const step = recording.steps[4];
    expect(step).toBeDefined();
    const order = step?.windows.map((window) => window.instanceId) ?? [];
    expect(order).toEqual([2, 1]);
    expect([...order].sort((left, right) => left - right)).not.toEqual(order);
  });

  it("finds one window's rectangle, and nothing for one that is not open", () => {
    let documents = documentsOf(recording.initialSnapshot);
    for (const step of recording.steps.slice(0, 4)) {
      for (const encodedDelta of step.deltas) {
        documents = applyDelta(documents, deltaOf(encodedDelta));
      }
    }
    expect(windowRect(documents.session, 1)).toEqual({ x: 240, y: 120, w: 640, h: 480 });
    expect(windowRect(documents.session, 99)).toBeNull();
  });
});

describe("a session document that is not one", () => {
  it("answers with nothing rather than throwing", () => {
    // A view rendered against a document that has just been dropped, or one
    // from a daemon that holds something this build has never seen. Neither is
    // an error to report: a canvas with no windows on it is a canvas.
    for (const document of [null, {}, { session: {} }, { session: { openWindows: 3 } }]) {
      const session = document as JsonValue;
      expect(openWindows(session)).toEqual([]);
      expect(focusedWindow(session)).toBeNull();
      expect(activeViewId(session)).toBeNull();
      expect(storedViews(session)).toEqual([]);
    }
  });

  it("leaves out a window this build cannot draw, and keeps the rest", () => {
    // A daemon newer than the interface. The honest answer is a canvas with
    // the windows it understands on it, not an empty frame and not a crash.
    const session: JsonValue = {
      session: {
        openWindows: [
          { instanceId: 1, type: "Hologram", x: 0, y: 0, w: 10, h: 10 },
          { instanceId: 2, type: "Patch", x: 1, y: 2, w: 3, h: 4 },
          { instanceId: 3, type: "Patch", x: 1, y: 2 },
          { instanceId: 4 },
          "not a window",
        ],
      },
    };
    expect(openWindows(session)).toEqual([
      { instanceId: 2, type: "Patch", x: 1, y: 2, w: 3, h: 4 },
    ]);
  });

  it("numbers its views numerically, however the document keys them", () => {
    const session: JsonValue = {
      views: {
        "10": { id: 10, name: "Busking", windows: [{}, {}] },
        "2": { id: 2, name: "Programming", windows: [] },
        "x": { id: 0, name: "not a view", windows: [] },
        "3": 7,
        "4": { id: 4, windows: 9 },
      },
    };
    expect(storedViews(session)).toEqual([
      { id: 2, name: "Programming", windowCount: 0 },
      { id: 4, name: "View 4", windowCount: 0 },
      { id: 10, name: "Busking", windowCount: 2 },
    ]);
  });
});

describe("a window's title", () => {
  it("is its type, made readable, and comes from the type rather than the session", () => {
    // `WindowInstance` has no title. Inventing a field for one would be the
    // interface adding to a document the daemon owns.
    expect(windowTitle("FixtureSheet")).toBe("Fixture Sheet");
    expect(windowTitle("DmxSheet")).toBe("DMX Sheet");
    expect(windowTitle("Viewer3D")).toBe("Viewer 3D");
    expect(windowTitle("PresetPool")).toBe("Preset Pool");
    expect(windowTitle("Patch")).toBe("Patch");
  });
});
