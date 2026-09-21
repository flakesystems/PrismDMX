/**
 * The loop: it paints when something changed and not otherwise, it goes dark
 * when the connection goes, it survives a frame it cannot read, and it says
 * what it drew without React.
 */

import { describe, expect, it } from "vitest";

import { TelemetrySink } from "../ipc/telemetry";
import type { Scheduler } from "../telemetry/driver";
import { RecordingView } from "../testing/recording-view";
import { finalShow, litPayload } from "../testing/viewer-recording";
import { look, newCamera } from "./camera";
import type { ViewerReadout } from "./driver";
import { READOUT_EVERY_MS, driveViewer, readoutText } from "./driver";
import { rigOf } from "./rig";

const rig = rigOf(finalShow());

function manualFrames(): { scheduler: Scheduler; step: (now?: number) => void; running: () => boolean } {
  let run: ((now: number) => void) | null = null;
  return {
    scheduler: (callback) => {
      run = callback;
      return () => {
        run = null;
      };
    },
    step: (now = 0) => {
      run?.(now);
    },
    running: () => run !== null,
  };
}

function setUp(options: { readonly surface?: RecordingView | null } = {}) {
  const sink = new TelemetrySink();
  const frames = manualFrames();
  const surface = options.surface === undefined ? new RecordingView() : options.surface;
  const camera = newCamera();
  const lines: ViewerReadout[] = [];
  let selection: ReadonlySet<number> = new Set();
  let currentRig = rig;
  let tick = 0;
  const driver = driveViewer({
    sink,
    surface,
    rig: () => currentRig,
    selection: () => selection,
    camera,
    scheduler: frames.scheduler,
    clock: () => {
      tick += 1;
      return tick;
    },
    readout: (line) => lines.push(line),
  });
  return {
    sink,
    frames,
    surface,
    camera,
    lines,
    driver,
    select: (next: ReadonlySet<number>) => {
      selection = next;
    },
    repatch: (next: typeof rig) => {
      currentRig = next;
    },
  };
}

describe("the viewer's loop", () => {
  it("paints once, and not again until something changes", () => {
    const { frames, driver, sink, camera, select, repatch } = setUp();
    frames.step(0);
    frames.step(16);
    expect(driver.painted()).toBe(1);

    sink.accept(litPayload());
    frames.step(32);
    expect(driver.painted()).toBe(2);

    look(camera, "top");
    frames.step(48);
    expect(driver.painted()).toBe(3);

    select(new Set([1]));
    frames.step(64);
    repatch(rig.slice(0, 2));
    frames.step(80);
    driver.invalidate();
    frames.step(96);
    expect(driver.painted()).toBe(6);
    frames.step(112);
    expect(driver.painted()).toBe(6);
  });

  it("says what it drew, at once when the counts move and otherwise at most four times a second", () => {
    const { frames, sink, lines } = setUp();
    frames.step(0);
    expect(lines.at(-1)).toMatchObject({ fixtures: 3, lit: 0 });
    sink.accept(litPayload());
    frames.step(10);
    expect(lines.at(-1)).toMatchObject({ fixtures: 3, lit: 1, painted: 2 });
    // Measured: two ticks of the fake clock per paint.
    expect(lines.at(-1)?.median).toBe(1);
    const said = lines.length;
    frames.step(20);
    expect(lines.length).toBe(said);
    frames.step(20 + READOUT_EVERY_MS);
    expect(lines.length).toBe(said);
  });

  it("goes dark when the connection goes, rather than freezing lit", () => {
    const { frames, sink, lines } = setUp();
    sink.accept(litPayload());
    frames.step(0);
    expect(lines.at(-1)?.lit).toBe(1);
    sink.clear();
    frames.step(16);
    expect(lines.at(-1)?.lit).toBe(0);
  });

  it("draws the last frame it could read when one arrives that it cannot", () => {
    const { frames, sink, lines, driver } = setUp();
    sink.accept(litPayload());
    frames.step(0);
    sink.accept(new Uint8Array([1, 2, 3]));
    frames.step(16);
    expect(driver.painted()).toBe(2);
    expect(lines.at(-1)?.lit).toBe(1);
  });

  it("picks from the picture it drew, and stops when told", () => {
    const { frames, driver, surface } = setUp();
    frames.step(0);
    expect(surface?.polygons.length).toBeGreaterThan(0);
    expect(driver.pick(-1000, -1000, 5)).toBeNull();
    driver.stop();
    expect(frames.running()).toBe(false);
  });

  it("does nothing at all without a surface", () => {
    const { frames, driver, lines } = setUp({ surface: null });
    frames.step(0);
    expect(driver.painted()).toBe(0);
    expect(lines).toHaveLength(0);
  });
});

describe("the readout line", () => {
  const base: ViewerReadout = {
    fixtures: 12,
    lit: 3,
    unplaced: 0,
    absent: 0,
    beams: 3,
    painted: 10,
    median: 1.24,
    p99: 3,
  };

  it("says how many are lit and how long a picture took", () => {
    expect(readoutText(base)).toBe("12 fixtures · 3 lit · 1.2 ms");
  });

  it("says what is not placed and what is on no output, and when nothing is patched", () => {
    expect(readoutText({ ...base, fixtures: 1, lit: 0, unplaced: 1, absent: 1 })).toBe(
      "1 fixture · 0 lit · 1 not placed · 1 on no output · 1.2 ms",
    );
    expect(readoutText({ ...base, fixtures: 0 })).toBe("nothing is patched");
  });
});
