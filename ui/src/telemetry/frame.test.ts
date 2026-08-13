/**
 * **The decoder, against frames a real daemon sent.**
 *
 * `ui/tests/fixtures/telemetry-recording.json` is written by
 * `crates/prismd/tests/ui_telemetry.rs`: the bytes are
 * `prism_ipc::TelemetryFrame::encode`'s, off a running `prismd` over a real
 * socket, and **every expectation beside them is `TelemetryFrame::decode`'s own
 * answer**. Nothing in this file works out what a frame ought to mean.
 *
 * That is the whole point. A TypeScript encoder feeding this decoder would pass
 * with the header misread, the endianness reversed and the section stride wrong,
 * as long as all three were wrong in the same direction — which is the trap S19
 * through S23 each found in their own layer.
 *
 * The digest is the one thing computed on both sides: FNV-1a over a universe's
 * 512 levels, eleven lines in each language. A mistake in either implementation
 * turns this red rather than quietly green, which is the property that matters.
 */

import { describe, expect, it } from "vitest";

// As text rather than as a module, for the reason `mirror/recording.test.ts`
// gives: a 100 kB JSON import would give TypeScript a literal type with every
// recorded payload in it.
import recordingText from "../../tests/fixtures/telemetry-recording.json?raw";

import {
  CHANNELS_PER_UNIVERSE,
  TELEMETRY_HEADER_BYTES,
  TELEMETRY_MAGIC,
  TELEMETRY_VERSION,
  TelemetryFrameView,
  UNIVERSE_SECTION_BYTES,
  faultText,
} from "./frame";
import type { TelemetryFault } from "./frame";

/** What one universe section decodes to, as `ui_telemetry.rs` wrote it. */
interface UniverseExpectation {
  readonly universe: number;
  readonly levels?: string;
  readonly sum: number;
  readonly nonZero: number;
  readonly fingerprint: number;
  readonly samples: readonly (readonly [number, number])[];
}

/** What one recorded payload decodes to. */
interface FrameExpectation {
  readonly sequence: number;
  readonly universeCount: number;
  readonly universes: readonly UniverseExpectation[];
}

/** One frame as it came off the socket, and what it means. */
interface RecordedFrame {
  readonly what: string;
  readonly payload: string;
  readonly expect: FrameExpectation;
}

/** One frame with something wrong with it, and the fault this build must give. */
interface MalformedFrame {
  readonly what: string;
  readonly payload: string;
  readonly fault: TelemetryFault["kind"];
  readonly found?: number;
  readonly expected?: number;
}

/** The file. */
interface Recording {
  readonly note: string;
  readonly telemetryVersion: number;
  readonly magic: string;
  readonly headerBytes: number;
  readonly channelsPerUniverse: number;
  readonly sectionBytes: number;
  readonly narrow: readonly RecordedFrame[];
  readonly wide: readonly RecordedFrame[];
  readonly malformed: readonly MalformedFrame[];
}

const recording: Recording = JSON.parse(recordingText) as Recording;

/** A recorded payload as bytes. */
function bytesOf(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/**
 * FNV-1a, 32-bit, over one universe's levels as this decoder reads them.
 *
 * `Math.imul` because the multiply is modulo 2^32 and JavaScript's `*` is not.
 */
function fingerprint(view: TelemetryFrameView, index: number): number {
  let hash = 0x811c_9dc5;
  const bytes = view.bytes;
  const offset = view.offsetAt(index);
  for (let channel = 0; channel < CHANNELS_PER_UNIVERSE; channel += 1) {
    hash = (hash ^ (bytes[offset + channel] ?? 0)) >>> 0;
    hash = Math.imul(hash, 0x0100_0193) >>> 0;
  }
  return hash;
}

/** Every frame in the file, narrow and wide. */
const frames: readonly RecordedFrame[] = [...recording.narrow, ...recording.wide];

describe("the recorded layout", () => {
  it("is the layout this build reads", () => {
    // Not decoration: these five numbers are the layout, and a recording of
    // another one must fail here rather than in the middle of a universe.
    expect(recording.telemetryVersion).toBe(TELEMETRY_VERSION);
    expect(recording.magic).toBe(TELEMETRY_MAGIC);
    expect(recording.headerBytes).toBe(TELEMETRY_HEADER_BYTES);
    expect(recording.channelsPerUniverse).toBe(CHANNELS_PER_UNIVERSE);
    expect(recording.sectionBytes).toBe(UNIVERSE_SECTION_BYTES);
  });

  it("is a recording of a rig that is lit, at both sizes", () => {
    expect(recording.narrow.length).toBeGreaterThanOrEqual(2);
    const wide = recording.wide.at(0);
    expect(wide?.expect.universeCount).toBe(64);
    // A frame that is all zeros would pass every assertion below.
    expect(
      frames.some((frame) => frame.expect.universes.some((universe) => universe.sum > 0)),
    ).toBe(true);
  });
});

describe("a frame the daemon sent", () => {
  it.each(frames.map((frame) => [frame.what, frame] as const))(
    "%s decodes to what prism_ipc::TelemetryFrame::decode said it does",
    (_what, frame) => {
      const view = new TelemetryFrameView();
      const fault = view.read(bytesOf(frame.payload));

      expect(fault).toBeNull();
      expect(view.hasFrame).toBe(true);
      // A `bigint` against the number `serde_json` wrote, which is exact for
      // every sequence a recording can hold.
      expect(view.sequence).toBe(BigInt(frame.expect.sequence));
      expect(view.count).toBe(frame.expect.universeCount);
      expect(view.count).toBe(frame.expect.universes.length);

      for (const [index, universe] of frame.expect.universes.entries()) {
        expect(view.universeAt(index)).toBe(universe.universe);

        let sum = 0;
        let nonZero = 0;
        for (let channel = 0; channel < CHANNELS_PER_UNIVERSE; channel += 1) {
          const level = view.levelAt(index, channel);
          sum += level;
          if (level !== 0) {
            nonZero += 1;
          }
        }
        expect({ universe: universe.universe, sum, nonZero }).toEqual({
          universe: universe.universe,
          sum: universe.sum,
          nonZero: universe.nonZero,
        });
        // The digest catches what a sum cannot: a universe whose levels are all
        // there but one channel to the left.
        expect(fingerprint(view, index)).toBe(universe.fingerprint);

        for (const [channel, level] of universe.samples) {
          expect([channel, view.levelAt(index, channel)]).toEqual([channel, level]);
        }

        // Where the daemon wrote the levels out, every one of the 512 is
        // compared — the narrow frames, which is where a copy is affordable.
        if (universe.levels !== undefined) {
          const levels = bytesOf(universe.levels);
          expect(levels.length).toBe(CHANNELS_PER_UNIVERSE);
          const read = new Uint8Array(CHANNELS_PER_UNIVERSE);
          for (let channel = 0; channel < CHANNELS_PER_UNIVERSE; channel += 1) {
            read[channel] = view.levelAt(index, channel);
          }
          expect(read).toEqual(levels);
        }
      }

      // Past the end is 0 rather than `undefined`: a renderer indexes this in a
      // loop, and one `undefined` in a canvas is a black row nobody explains.
      expect(view.levelAt(view.count, 0)).toBe(0);
      expect(view.levelAt(0, CHANNELS_PER_UNIVERSE)).toBe(0);
      expect(view.levelAt(-1, 0)).toBe(0);
      expect(view.universeAt(view.count)).toBe(0);
      expect(view.offsetAt(view.count)).toBe(0);
    },
  );

  it("reads the universe number out of the section rather than from its position", () => {
    // The wide rig patches 1…64 in order, so it cannot show this. The narrow one
    // patches 1 and 2 — also in order — which is why the check is on the offset
    // arithmetic instead: universe *n* begins where the layout says it does.
    const view = new TelemetryFrameView();
    const first = recording.narrow.at(0);
    if (first === undefined) {
      throw new Error("the recording has no narrow frame");
    }
    expect(view.read(bytesOf(first.payload))).toBeNull();
    for (let index = 0; index < view.count; index += 1) {
      expect(view.offsetAt(index)).toBe(
        TELEMETRY_HEADER_BYTES + index * UNIVERSE_SECTION_BYTES + 2,
      );
    }
  });
});

describe("a frame that cannot be read", () => {
  it.each(recording.malformed.map((frame) => [frame.what, frame] as const))(
    "%s is dropped with the fault prism_ipc gives it",
    (_what, frame) => {
      const view = new TelemetryFrameView();
      const fault = view.read(bytesOf(frame.payload));
      expect(fault).not.toBeNull();
      expect(fault?.kind).toBe(frame.fault);
      if (fault !== null && fault.kind !== "not-telemetry") {
        expect({ found: fault.found, expected: fault.expected }).toEqual({
          found: frame.found,
          expected: frame.expected,
        });
      }
      // Nothing was read into the view, so nothing can be drawn from it.
      expect(view.hasFrame).toBe(false);
      expect(view.count).toBe(0);
    },
  );

  it("leaves the picture that was already there standing", () => {
    // §7: telemetry is droppable. A frame this build cannot read is not an
    // error to recover from — it is one missing picture, and the last good one
    // is still the best answer until the next arrives.
    const view = new TelemetryFrameView();
    const good = recording.narrow.at(0);
    const bad = recording.malformed.at(0);
    if (good === undefined || bad === undefined) {
      throw new Error("the recording is missing a case");
    }
    expect(view.read(bytesOf(good.payload))).toBeNull();
    const sequence = view.sequence;
    const level = view.levelAt(0, 0);

    expect(view.read(bytesOf(bad.payload))).not.toBeNull();

    expect(view.sequence).toBe(sequence);
    expect(view.count).toBe(good.expect.universeCount);
    expect(view.levelAt(0, 0)).toBe(level);

    // And it can be forgotten deliberately, which is what losing the daemon does.
    view.clear();
    expect(view.hasFrame).toBe(false);
    expect(view.count).toBe(0);
    expect(view.sequence).toBe(0n);
    expect(view.levelAt(0, 0)).toBe(0);
  });

  it("says something a person could act on", () => {
    const faults: TelemetryFault[] = [
      { kind: "not-telemetry" },
      { kind: "unknown-version", found: 2, expected: TELEMETRY_VERSION },
      { kind: "truncated", found: 4, expected: TELEMETRY_HEADER_BYTES },
    ];
    for (const fault of faults) {
      expect(faultText(fault).length).toBeGreaterThan(20);
    }
    expect(faultText({ kind: "unknown-version", found: 9, expected: 1 })).toContain("9");
  });

  it("refuses a frame carrying more universes than its bytes", () => {
    // The complement of the recorded cases, built by hand because no daemon
    // sends it: a header that claims one universe with nothing behind it.
    const bytes = new Uint8Array(TELEMETRY_HEADER_BYTES);
    bytes.set(Uint8Array.from(TELEMETRY_MAGIC, (letter) => letter.charCodeAt(0)));
    bytes[4] = TELEMETRY_VERSION;
    bytes[6] = 1;
    const view = new TelemetryFrameView();
    expect(view.read(bytes)).toEqual({
      kind: "truncated",
      expected: TELEMETRY_HEADER_BYTES + UNIVERSE_SECTION_BYTES,
      found: TELEMETRY_HEADER_BYTES,
    });
  });

  it("reads a frame that carries no universes at all", () => {
    // Which is what a daemon with an empty show sends: a header and nothing.
    const bytes = new Uint8Array(TELEMETRY_HEADER_BYTES);
    bytes.set(Uint8Array.from(TELEMETRY_MAGIC, (letter) => letter.charCodeAt(0)));
    bytes[4] = TELEMETRY_VERSION;
    bytes[8] = 3;
    const view = new TelemetryFrameView();
    expect(view.read(bytes)).toBeNull();
    expect(view.count).toBe(0);
    expect(view.sequence).toBe(3n);
    expect(view.hasFrame).toBe(true);
  });

  it("grows past the sixty-four universes a desk has room for", () => {
    // `TelemetryFrame::decode` has no such limit, and a decoder that refused
    // what the daemon's own decoder accepts would be a second opinion about the
    // wire. Built by hand for the same reason as above.
    const count = 70;
    const bytes = new Uint8Array(TELEMETRY_HEADER_BYTES + count * UNIVERSE_SECTION_BYTES);
    bytes.set(Uint8Array.from(TELEMETRY_MAGIC, (letter) => letter.charCodeAt(0)));
    bytes[4] = TELEMETRY_VERSION;
    bytes[6] = count;
    for (let index = 0; index < count; index += 1) {
      const section = TELEMETRY_HEADER_BYTES + index * UNIVERSE_SECTION_BYTES;
      bytes[section] = index + 1;
      bytes[section + 2] = 200;
    }
    const view = new TelemetryFrameView();
    expect(view.read(bytes)).toBeNull();
    expect(view.count).toBe(count);
    expect(view.universeAt(count - 1)).toBe(count);
    expect(view.levelAt(count - 1, 0)).toBe(200);
  });
});
