/**
 * The 3D viewer's recording, read the way the interface reads a daemon — B55's
 * rule: every byte goes through `decodeServerMessage`, and nothing here is an
 * object a fake daemon made up.
 *
 * `crates/prismd/tests/ui_viewer.rs` wrote it: two GDTF heads and a generic
 * PAR patched, hung, a refused spread, an Oops and a Redo, and one telemetry
 * frame with the first head open and panned three quarters of the way.
 *
 * Scenery, not a fixture — `src/testing/` is excluded from coverage.
 */

import type { Delta, JsonValue } from "../bindings";
import { decodeServerMessage } from "../ipc/codec";
import type { ServerMessage } from "../ipc/protocol";
import { overArrayBuffer } from "../ipc/shape";
import type { Documents } from "../mirror/mirror";
import { applyDelta } from "../mirror/mirror";
import { TelemetryFrameView } from "../telemetry/frame";

import recordingText from "../../tests/fixtures/viewer-recording.json?raw";

/** A vector as the recording writes one. */
interface RecordedVec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/** The file, as `ui_viewer.rs` serialises it. */
export interface ViewerRecording {
  readonly protocolVersion: number;
  readonly initialSnapshot: string;
  readonly steps: readonly {
    readonly what: string;
    readonly client: string;
    readonly deltas: readonly string[];
    readonly refused: boolean;
  }[];
  readonly finalSnapshot: string;
  readonly orientations: readonly {
    readonly rotation: RecordedVec3;
    readonly matrix: readonly (readonly [number, number, number])[];
  }[];
  readonly lit: {
    readonly message: string;
    readonly universe: number;
    readonly firstHead: readonly number[];
    readonly secondHead: readonly number[];
  };
}

/** The recording. */
export const viewerRecording: ViewerRecording = JSON.parse(recordingText) as ViewerRecording;

/** Base64 as bytes. */
export function bytesOf(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/** A recorded payload, decoded. */
export function messageOf(base64: string): ServerMessage {
  return decodeServerMessage(overArrayBuffer(bytesOf(base64)));
}

/** The documents a recorded snapshot carries. */
function documentsOf(base64: string): Documents {
  const message = messageOf(base64);
  if (message.t !== "Snapshot") {
    throw new Error(`a recorded snapshot is ${message.t}`);
  }
  return {
    show: message.snapshot.show,
    session: message.snapshot.session,
    programmer: message.snapshot.programmer,
  };
}

/** The show document after each step of the script, in order. */
export function showsAfterEachStep(): readonly JsonValue[] {
  let documents = documentsOf(viewerRecording.initialSnapshot);
  const shows: JsonValue[] = [];
  for (const step of viewerRecording.steps) {
    for (const encoded of step.deltas) {
      const message = messageOf(encoded);
      if (message.t !== "Delta") {
        throw new Error(`a recorded delta is ${message.t}`);
      }
      documents = applyDelta(documents, message.delta satisfies Delta);
    }
    shows.push(documents.show);
  }
  return shows;
}

/** The show a fresh client is served at the end. */
export function finalShow(): JsonValue {
  return documentsOf(viewerRecording.finalSnapshot).show;
}

/** The lit frame's bytes, out of the recorded `Telemetry` message. */
export function litPayload(): Uint8Array {
  const message = messageOf(viewerRecording.lit.message);
  if (message.t !== "Telemetry") {
    throw new Error(`the lit payload is ${message.t}`);
  }
  return message.data;
}

/** The lit frame, read. */
export function litFrame(): TelemetryFrameView {
  const view = new TelemetryFrameView();
  const fault = view.read(litPayload());
  if (fault !== null) {
    throw new Error(`the lit frame did not read: ${fault.kind}`);
  }
  return view;
}
