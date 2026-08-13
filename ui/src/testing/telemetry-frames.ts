/**
 * The recorded telemetry frames, as bytes, for the tests that need a frame
 * rather than a claim about one.
 *
 * The file is `ui/tests/fixtures/telemetry-recording.json`, written by
 * `crates/prismd/tests/ui_telemetry.rs` off a running `prismd`. Every test in
 * this session that needs telemetry uses these bytes and not a frame built in
 * TypeScript — a renderer fed a frame this project invented would be a renderer
 * tested against its own decoder's idea of the layout.
 *
 * `frame.test.ts` reads the same file with the expectations beside the payloads;
 * this is only the loader for everybody else.
 */

import recordingText from "../../tests/fixtures/telemetry-recording.json?raw";

interface RecordedFrame {
  readonly what: string;
  readonly payload: string;
}

interface Recording {
  readonly narrow: readonly RecordedFrame[];
  readonly wide: readonly RecordedFrame[];
  readonly malformed: readonly (RecordedFrame & { readonly fault: string })[];
}

const recording: Recording = JSON.parse(recordingText) as Recording;

/** A base64 payload as bytes. */
function bytesOf(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/** Frames from the ordinary three-dimmer rig: two universes. */
export const narrowFrames: readonly Uint8Array[] = recording.narrow.map((frame) =>
  bytesOf(frame.payload),
);

/** One frame from a rig patched across all 64 universes. */
export const wideFrame: Uint8Array = bytesOf(
  recording.wide[0]?.payload ??
    (() => {
      throw new Error("the recording has no 64-universe frame");
    })(),
);

/** Frames that must be dropped, with the fault `prism-ipc` gives each one. */
export const malformedFrames: readonly { what: string; fault: string; bytes: Uint8Array }[] =
  recording.malformed.map((frame) => ({
    what: frame.what,
    fault: frame.fault,
    bytes: bytesOf(frame.payload),
  }));

/** The narrow frame at `index`, wrapping round — for a test that wants several. */
export function narrowFrame(index: number): Uint8Array {
  const frame = narrowFrames[index % narrowFrames.length];
  if (frame === undefined) {
    throw new Error("the recording has no narrow frames");
  }
  return frame;
}
