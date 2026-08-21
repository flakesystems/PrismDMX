/**
 * **The first exit criterion of S23.** Deltas applied to the mirror reproduce
 * the daemon's state — property-tested against a recorded delta stream.
 *
 * # Where the cases come from, and why not from here
 *
 * `ui/tests/fixtures/daemon-recording.json` is what a real `prismd` said over a
 * real socket: twelve scripted sequences, each one a snapshot, the deltas a
 * random command script produced, and the snapshot a **second client** was
 * served afterwards. It is written by
 * `crates/prismd/tests/ui_recording.rs`, which is also where the reasoning is.
 *
 * The property is `docs/IPC_PROTOCOL.md` §9's *snapshot completeness* row:
 *
 * > a fresh client's snapshot equals the state an existing client reached by
 * > accumulating deltas.
 *
 * Both halves of that comparison come from the daemon. Nothing in this file
 * computes what the answer should be; if it did — if the expected document were
 * built by applying the same operations a second time — the test would pass for
 * an applier that misunderstood JSON Patch, as long as it misunderstood it
 * consistently. That is the trap S19 through S22 each found in their own layer,
 * and this is its shape here.
 *
 * # What is actually being exercised
 *
 * The whole path a byte takes into the store: base64 → MessagePack →
 * `readServerMessage` (which is `unknown` narrowed by hand, with no `as`) →
 * `applyDelta` → a document compared against one the daemon serialised. A break
 * anywhere along it fails here, which is why this is one test and not four.
 */

import { describe, expect, it } from "vitest";

// As text rather than as a module: a 130 kB JSON import would give TypeScript a
// literal type with every recorded payload in it, and nothing here wants that.
import recordingText from "../../tests/fixtures/daemon-recording.json?raw";

import type { Delta } from "../bindings";
import { decodeServerMessage } from "../ipc/codec";
import { encodeClientMessage } from "../ipc/codec";
import type { ClientMessage, ServerMessage, Snapshot } from "../ipc/protocol";
import { PROTOCOL_VERSION } from "../ipc/protocol";
import { overArrayBuffer } from "../ipc/shape";
import type { Documents } from "./mirror";
import { applyDelta } from "./mirror";

/** One scripted sequence, as `ui_recording.rs` wrote it. */
interface Case {
  readonly commands: readonly string[];
  readonly snapshot: string;
  readonly deltas: readonly string[];
  readonly final_snapshot: string;
}

/** The file. */
interface Recording {
  readonly note: string;
  readonly protocolVersion: number;
  readonly clientMessages: readonly { readonly what: string; readonly payload: string }[];
  readonly cases: readonly Case[];
}

const recording: Recording = JSON.parse(recordingText) as Recording;

/** A recorded payload, decoded. */
function messageOf(base64: string): ServerMessage {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return decodeServerMessage(overArrayBuffer(bytes));
}

/** The snapshot inside a recorded payload. */
function snapshotOf(base64: string): Snapshot {
  const message = messageOf(base64);
  if (message.t !== "Snapshot") {
    throw new Error(`a recorded snapshot is ${message.t}`);
  }
  return message.snapshot;
}

/** The delta inside a recorded payload. */
function deltaOf(base64: string): Delta {
  const message = messageOf(base64);
  if (message.t !== "Delta") {
    throw new Error(`a recorded delta is ${message.t}`);
  }
  return message.delta;
}

describe("the recorded delta stream", () => {
  it("is a recording of this protocol version", () => {
    expect(recording.protocolVersion).toBe(PROTOCOL_VERSION);
    expect(recording.cases.length).toBeGreaterThanOrEqual(12);
  });

  it.each(recording.cases.map((value, index) => [index, value] as const))(
    "case %i: accumulated deltas reproduce the daemon's own snapshot",
    (index, recorded) => {
      const start = snapshotOf(recorded.snapshot);
      let documents: Documents = {
        show: start.show,
        session: start.session,
        programmer: start.programmer,
      };

      for (const [step, encoded] of recorded.deltas.entries()) {
        try {
          documents = applyDelta(documents, deltaOf(encoded));
        } catch (cause) {
          throw new Error(
            `case ${index} step ${step} (${recorded.commands.join(", ")}): ${String(cause)}`,
          );
        }
      }

      const end = snapshotOf(recorded.final_snapshot);
      expect(documents.show).toEqual(end.show);
      expect(documents.session).toEqual(end.session);
      expect(documents.programmer).toEqual(end.programmer);
    },
  );

  it("would have noticed a mirror that did nothing", () => {
    // The assertion above is only worth having if the documents move. This is
    // the same guard `ui_recording.rs` carries in Rust, restated here so a
    // recording that had gone empty could not pass quietly.
    let showsMoved = 0;
    let sessionsMoved = 0;
    let programmersMoved = 0;
    for (const recorded of recording.cases) {
      const start = snapshotOf(recorded.snapshot);
      const end = snapshotOf(recorded.final_snapshot);
      if (JSON.stringify(start.show) !== JSON.stringify(end.show)) showsMoved += 1;
      if (JSON.stringify(start.session) !== JSON.stringify(end.session)) sessionsMoved += 1;
      if (JSON.stringify(start.programmer) !== JSON.stringify(end.programmer)) programmersMoved += 1;
      expect(recorded.deltas.length).toBeGreaterThan(0);
    }
    expect(showsMoved).toBeGreaterThan(0);
    expect(sessionsMoved).toBeGreaterThan(0);
    expect(programmersMoved).toBeGreaterThan(0);
  });

  it("carries every kind of delta the mirror has a branch for", () => {
    const kinds = new Set<string>();
    for (const recorded of recording.cases) {
      for (const encoded of recorded.deltas) {
        kinds.add(deltaOf(encoded).t);
      }
    }
    // The four that touch a document. `OutputHealth` and `Notice` are the
    // daemon's own and do not appear in a recording of commands.
    expect(kinds).toContain("ShowPatch");
    expect(kinds).toContain("SessionPatch");
    expect(kinds).toContain("ProgrammerChanged");
    expect(kinds).toContain("PlaybackState");
  });

  it("is reproduced byte for byte by this build's encoder", () => {
    // The other direction, and the only way to check an encoder against a
    // decoder that is not in this process: the daemon's own `rmp-serde` wrote
    // these payloads, and `@msgpack/msgpack` has to write the same bytes for
    // the same message — the tag first, the fields in declaration order, the
    // narrowest integer that fits, and an absent optional field absent.
    for (const recorded of recording.clientMessages) {
      const expected = atob(recorded.payload);
      const bytes = encodeClientMessage(messageFor(recorded.what));
      let actual = "";
      for (const byte of bytes) {
        actual += String.fromCharCode(byte);
      }
      expect(actual, `${recorded.what} does not encode the way the daemon reads it`).toBe(expected);
    }
  });
});

/**
 * The message `ui_recording.rs` recorded under this name.
 *
 * Written out by hand rather than derived from the recording, for the reason
 * every table in this project is: a test that asked the recording what to
 * encode would be comparing the file with itself.
 */
function messageFor(what: string): ClientMessage {
  switch (what) {
    case "Hello, Desktop, no token":
      return { t: "Hello", hello: { protocolVersion: 1, clientKind: "Desktop", token: null } };
    case "Hello, WebRemote, with a token":
      return {
        t: "Hello",
        hello: { protocolVersion: 1, clientKind: "WebRemote", token: "hunter2" },
      };
    case "SelectFixtures":
      return { t: "Command", seq: 0, command: { t: "SelectFixtures", ids: [1, 2], mode: "Add" } };
    case "SetAttribute":
      return {
        t: "Command",
        seq: 1,
        command: { t: "SetAttribute", attribute: "Dimmer", value: -257, relative: true },
      };
    case "ClearProgrammer":
      return { t: "Command", seq: 2, command: { t: "ClearProgrammer" } };
    case "SetExecutorMaster":
      return {
        t: "Command",
        seq: 300,
        command: { t: "SetExecutorMaster", executorId: 7, level: 65535 },
      };
    case "ExecutorGo":
      return {
        t: "Command",
        seq: 4,
        command: {
          t: "ExecutorGo",
          target: { t: "Executor", executorId: 0 },
          direction: "Prev",
        },
      };
    case "PatchFixture":
      return {
        t: "Command",
        seq: 5,
        command: {
          t: "PatchFixture",
          id: 42,
          name: "Fixture 42",
          typeId: "generic.dimmer",
          universe: 2,
          address: 271,
        },
      };
    case "OpenWindow without params":
      return { t: "Command", seq: 6, command: { t: "OpenWindow", window: "Patch" } };
    case "OpenWindow with params":
      return {
        t: "Command",
        seq: 7,
        command: { t: "OpenWindow", window: "PresetPool", params: { pool: "Color" } },
      };
    case "CommandLineInput":
      return {
        t: "Command",
        seq: 8,
        command: { t: "CommandLineInput", text: "fixture 1 thru 4 at full" },
      };
    case "SelectView":
      return { t: "Command", seq: 9, command: { t: "SelectView", viewId: 2 } };
    case "SaveShow":
      return { t: "Command", seq: 10, command: { t: "SaveShow" } };
    default:
      throw new Error(`the recording holds a client message this test does not know: ${what}`);
  }
}
