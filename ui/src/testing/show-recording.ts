/**
 * The S28 recording, read once for every test that leans on it.
 *
 * `ui/tests/fixtures/show-recording.json` is written by
 * `crates/prismd/tests/ui_show.rs` off a running `prismd`. Three test files read
 * it — the readers, the sentences and the windows — and each of them used to
 * parse it for itself, which is fine right up until one of them *imports*
 * another and quietly runs its suite twice.
 *
 * So the parsing lives here, beside `fake-daemon.ts` and `telemetry-frames.ts`:
 * scenery, not a test.
 */

import { decode } from "@msgpack/msgpack";

import type { Answer, Delta, JsonValue, RgbColor } from "../bindings";
import { readServerMessage } from "../ipc/protocol";
import type { Snapshot } from "../ipc/protocol";

import recordingText from "../../tests/fixtures/show-recording.json?raw";

/** One value inside a recorded cue. */
export interface RecordedPart {
  readonly fixture: number;
  readonly attribute: string;
  readonly value: number;
  readonly presetRef: number | null;
}

/** One recorded cue, as the daemon's own document says it. */
export interface RecordedCue {
  readonly number: string;
  readonly name: string;
  readonly fadeIn: number;
  readonly fadeOut: number;
  readonly delay: number;
  readonly trigger: string;
  readonly triggerTime: number | null;
  readonly parts: readonly RecordedPart[];
}

/** One recorded sequence. */
export interface RecordedSequence {
  readonly id: number;
  readonly name: string;
  readonly looping: boolean;
  readonly cues: readonly RecordedCue[];
}

/** One recorded preset. */
export interface RecordedPreset {
  readonly id: number;
  readonly pool: string;
  readonly name: string;
  readonly color: RgbColor | null;
  readonly values: number;
}

/** One recorded executor slot. */
export interface RecordedExecutor {
  readonly id: number;
  readonly sequenceId: number | null;
  readonly isActive: boolean;
  readonly currentCueIndex: number | null;
}

/** The update state a step left behind — S39's `Session::editingCue`. */
export interface RecordedCueEdit {
  readonly sequenceId: number;
  readonly cueNumber: string;
  readonly modified: boolean;
}

/** One step of the recorded script. */
export interface RecordedStep {
  readonly what: string;
  readonly isQuery: boolean;
  readonly deltas: readonly string[];
  readonly answer: string | null;
  readonly refused: boolean;
  readonly sequences: readonly RecordedSequence[];
  readonly presets: readonly RecordedPreset[];
  readonly executors: readonly RecordedExecutor[];
  /** The cue list in force afterwards — S39. */
  readonly selectedSequence: number | null;
  /** The update state afterwards — S39. */
  readonly editingCue: RecordedCueEdit | null;
}

/** The whole file. */
export interface ShowRecording {
  readonly initialSnapshot: string;
  readonly finalSnapshot: string;
  readonly steps: readonly RecordedStep[];
}

/** The recording, parsed once. */
export const showRecording = JSON.parse(recordingText) as ShowRecording;

/** Bytes out of a base64 payload, the way a browser does it (S23). */
export function payloadOf(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/**
 * The step whose `what` contains this text.
 *
 * By description rather than by number — S44's finding: the script grows, and an
 * index written down in a test would quietly start pointing at another step.
 */
export function stepAbout(about: string): RecordedStep {
  const found = showRecording.steps.find((step) => step.what.includes(about));
  if (found === undefined) {
    throw new Error(`no recorded step is about ${JSON.stringify(about)}`);
  }
  return found;
}

/** The snapshot at one end of the recording. */
export function snapshotOf(encoded: string): Snapshot {
  const message = readServerMessage(decode(payloadOf(encoded)));
  if (message.t !== "Snapshot") {
    throw new Error("that payload is not a snapshot");
  }
  return message.snapshot;
}

/** One delta out of the recording. */
export function deltaOf(encoded: string): Delta {
  const message = readServerMessage(decode(payloadOf(encoded)));
  if (message.t !== "Delta") {
    throw new Error("that payload is not a delta");
  }
  return message.delta;
}

/** The deltas of the step about something, in order. */
export function deltasAbout(about: string): Delta[] {
  return stepAbout(about).deltas.map(deltaOf);
}

/** The daemon's answer at the step about something. */
export function answerAbout(about: string): Answer {
  const encoded = stepAbout(about).answer;
  if (encoded === null) {
    throw new Error(`the step about ${JSON.stringify(about)} carries no answer`);
  }
  const message = readServerMessage(decode(payloadOf(encoded)));
  if (message.t !== "Answer") {
    throw new Error("that payload is not an answer");
  }
  return message.answer;
}

/** The show document the recording starts from. */
export function initialShow(): JsonValue {
  return snapshotOf(showRecording.initialSnapshot).show;
}
