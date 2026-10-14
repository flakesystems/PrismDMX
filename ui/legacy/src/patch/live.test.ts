/**
 * The output column: what is on the cable, per fixture, drawn on a canvas.
 *
 * The frames are the recorded ones a real `prismd` sent (S24), decoded by this
 * interface's own `TelemetryFrameView`, and the surface is the recording one —
 * so what is asserted here is *where each channel's bar went and how wide it
 * was*, per pixel, in a runtime with no rasteriser. That is what `LevelSurface`
 * is for, and it is the reason this column can be checked at all.
 */

import { beforeEach, describe, expect, it } from "vitest";

import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { RecordingSurface } from "../testing/recording-surface";
import { narrowFrame, narrowFrames, wideFrame } from "../testing/telemetry-frames";
import { TelemetryFrameView } from "../telemetry/frame";
import type { Scheduler } from "../telemetry/driver";
import type { LiveFixture } from "./live";
import { ROW_HEIGHT, driveFixtureLevels, drawFixtureLevels, liveFixtures, visibleRows } from "./live";

/** A frame the recorded daemon actually sent. */
function frameOf(bytes: Uint8Array): TelemetryFrameView {
  const view = new TelemetryFrameView();
  const fault = view.read(bytes);
  if (fault !== null) {
    throw new Error(`the recorded frame did not read: ${fault.kind}`);
  }
  return view;
}

/** A scheduler the test steps by hand. */
function manualFrames(): { scheduler: Scheduler; step: () => void; stopped: () => boolean } {
  let run: ((now: number) => void) | null = null;
  let stopped = false;
  return {
    scheduler: (callback) => {
      run = callback;
      return () => {
        stopped = true;
      };
    },
    step: () => {
      run?.(0);
    },
    stopped: () => stopped,
  };
}

/** Four fixtures on one universe, three channels apart. */
const rig: readonly LiveFixture[] = [
  { id: 1, universe: 1, address: 1, footprint: 1 },
  { id: 2, universe: 1, address: 2, footprint: 1 },
  { id: 3, universe: 1, address: 3, footprint: 1 },
  { id: 4, universe: 9, address: 1, footprint: 4 },
];

beforeEach(() => {
  setLogSink(nullSink);
});

describe("which fixtures the column reads out of the show", () => {
  it("is every patched one, in number order, with the profile's footprint", () => {
    expect(
      liveFixtures({
        fixtures: {
          "10": { typeId: "generic.rgbw.par", universe: 2, address: 5 },
          "2": { typeId: "generic.dimmer", universe: 1, address: 1 },
        },
        fixtureTypes: {
          "generic.dimmer": { footprint: 1 },
          "generic.rgbw.par": { footprint: 4 },
        },
      }),
    ).toEqual([
      { id: 2, universe: 1, address: 1, footprint: 1 },
      { id: 10, universe: 2, address: 5, footprint: 4 },
    ]);
  });

  it("answers with nothing where there is no patch, and 0 where there is no profile", () => {
    expect(liveFixtures(null)).toEqual([]);
    expect(liveFixtures({ fixtures: 7 })).toEqual([]);
    expect(liveFixtures({ fixtures: { x: {}, "1": 7 } })).toEqual([]);
    expect(
      liveFixtures({ fixtures: { "1": { typeId: "gone", universe: 1, address: 1 } } }),
    ).toEqual([{ id: 1, universe: 1, address: 1, footprint: 0 }]);
  });
});

describe("which rows are drawn", () => {
  it("is the ones on the screen, and one more so a half row is not blank", () => {
    // Four hundred rows and a window twenty tall: the cost of a frame is what
    // can be seen, not what is patched.
    const geometry = visibleRows(ROW_HEIGHT * 20, ROW_HEIGHT * 100, 400);
    expect(geometry.firstRow).toBe(100);
    expect(geometry.lastRow).toBe(121);
    expect(geometry.scrollTop).toBe(ROW_HEIGHT * 100);
  });

  it("stops at the end of the rig and starts at the top of it", () => {
    expect(visibleRows(1000, 0, 3)).toMatchObject({ firstRow: 0, lastRow: 3 });
    // Scrolled past the end — which a shrinking rig produces for a moment.
    expect(visibleRows(1000, ROW_HEIGHT * 500, 3)).toMatchObject({ firstRow: 3, lastRow: 3 });
    // And a negative offset, which some browsers produce while rubber-banding.
    expect(visibleRows(1000, -40, 3)).toMatchObject({ firstRow: 0, scrollTop: 0 });
    expect(visibleRows(0, 0, 3)).toMatchObject({ firstRow: 0, lastRow: 1 });
  });
});

describe("what is drawn", () => {
  it("is one bar per channel of every fixture, at the level on the cable", () => {
    const surface = new RecordingSurface(400, ROW_HEIGHT * 4);
    const frame = frameOf(narrowFrame(0));
    const drawn = drawFixtureLevels(
      surface,
      rig,
      frame,
      visibleRows(surface.height, 0, rig.length),
    );
    expect(drawn).toBe(4);
    expect(surface.clears).toHaveLength(1);

    // Fixture 1 is one channel wide, so its bar spans the whole column: a
    // trough and, if the channel is up, a fill in front of it.
    const first = surface.fills.filter((fill) => fill.y < ROW_HEIGHT);
    expect(first[0]?.width).toBeCloseTo(400 - 1, 5);
    // The level the recorded daemon was sending on universe 1 channel 1.
    const level = frame.levelAt(0, 0);
    expect(first).toHaveLength(level > 0 ? 2 : 1);
    if (level > 0) {
      expect(first[1]?.width).toBeCloseTo(((level / 255) * 399), 5);
    }
  });

  it("draws a fixture whose universe is not on the cable as absent, not as dark", () => {
    // *No output* and *output at zero* are different facts. Fixture 4 is on
    // universe 9, which the recorded rig does not patch — so it is drawn in one
    // block rather than as four channels sitting at zero, which is what an
    // operator has to be able to tell apart.
    const surface = new RecordingSurface(400, ROW_HEIGHT * 4);
    drawFixtureLevels(
      surface,
      rig,
      frameOf(narrowFrame(0)),
      visibleRows(surface.height, 0, rig.length),
    );
    const fourth = surface.fills.filter((fill) => fill.y >= ROW_HEIGHT * 3);
    expect(fourth).toHaveLength(1);
    expect(fourth[0]?.width).toBe(400);

    // And a fixture whose profile the show cannot resolve is the same case.
    const unresolved = new RecordingSurface(400, ROW_HEIGHT);
    drawFixtureLevels(
      unresolved,
      [{ id: 1, universe: 1, address: 1, footprint: 0 }],
      frameOf(narrowFrame(0)),
      visibleRows(ROW_HEIGHT, 0, 1),
    );
    expect(unresolved.fills).toHaveLength(1);
  });

  it("scrolls the picture rather than the canvas", () => {
    // The canvas is the size of the window and the rows move under it, so the
    // second row is drawn at the top when the sheet is scrolled by one.
    const surface = new RecordingSurface(400, ROW_HEIGHT * 2);
    drawFixtureLevels(
      surface,
      rig,
      frameOf(narrowFrame(0)),
      visibleRows(surface.height, ROW_HEIGHT, rig.length),
    );
    expect(Math.min(...surface.fills.map((fill) => fill.y))).toBeLessThan(ROW_HEIGHT);
    expect(Math.max(...surface.fills.map((fill) => fill.y))).toBeLessThan(ROW_HEIGHT * 3);
  });

  it("draws a wide rig's fixtures on the universes it carries", () => {
    // The 64-universe frame, so a fixture high up the rig is found rather than
    // read off the first section.
    const surface = new RecordingSurface(400, ROW_HEIGHT);
    const frame = frameOf(wideFrame);
    const high = frame.universeAt(frame.count - 1);
    drawFixtureLevels(
      surface,
      [{ id: 1, universe: high, address: 1, footprint: 2 }],
      frame,
      visibleRows(ROW_HEIGHT, 0, 1),
    );
    // Two channels, so two troughs — an absent universe would have been one
    // block of the whole width.
    expect(surface.fills.filter((fill) => fill.width < 400).length).toBeGreaterThanOrEqual(2);
  });
});

describe("the loop that keeps it current", () => {
  it("redraws when the frame, the scroll or the rig changes, and not otherwise", () => {
    const sink = new TelemetrySink();
    const surface = new RecordingSurface(400, ROW_HEIGHT * 4);
    const frames = manualFrames();
    let scrollTop = 0;
    let fixtures = rig;
    const driver = driveFixtureLevels({
      sink,
      surface,
      fixtures: () => fixtures,
      scrollTop: () => scrollTop,
      scheduler: frames.scheduler,
    });

    sink.accept(narrowFrame(0));
    frames.step();
    expect(driver.painted()).toBe(1);

    // Nothing new: nothing drawn.
    frames.step();
    frames.step();
    expect(driver.painted()).toBe(1);

    // A new frame, a scroll, and a repatch — each one on its own.
    sink.accept(narrowFrame(1));
    frames.step();
    expect(driver.painted()).toBe(2);
    scrollTop = ROW_HEIGHT;
    frames.step();
    expect(driver.painted()).toBe(3);
    fixtures = rig.slice(0, 2);
    frames.step();
    expect(driver.painted()).toBe(4);

    driver.stop();
    expect(frames.stopped()).toBe(true);
  });

  it("keeps the last picture when a frame arrives that this build cannot read", () => {
    // §7: a frame this build cannot read costs one picture and nothing else.
    const sink = new TelemetrySink();
    const surface = new RecordingSurface(400, ROW_HEIGHT * 4);
    const frames = manualFrames();
    const driver = driveFixtureLevels({
      sink,
      surface,
      fixtures: () => rig,
      scrollTop: () => 0,
      scale: () => 1,
      scheduler: frames.scheduler,
    });

    sink.accept(narrowFrame(0));
    frames.step();
    const good = surface.fills.length;
    expect(good).toBeGreaterThan(0);

    sink.accept(new Uint8Array([1, 2, 3]));
    frames.step();
    expect(driver.painted()).toBe(1);
    expect(surface.fills).toHaveLength(good);
    driver.stop();
  });

  it("empties the column when the daemon goes", () => {
    const sink = new TelemetrySink();
    const surface = new RecordingSurface(400, ROW_HEIGHT * 4);
    const frames = manualFrames();
    const driver = driveFixtureLevels({
      sink,
      surface,
      fixtures: () => rig,
      scrollTop: () => 0,
      scheduler: frames.scheduler,
    });
    sink.accept(narrowFrame(0));
    frames.step();
    // A picture of the rig from a daemon that has stopped is exactly as stale
    // as a fader value from one, so every row reads *absent*.
    sink.clear();
    frames.step();
    expect(surface.fills).toHaveLength(rig.length);
    expect(surface.fills.every((fill) => fill.width === 400)).toBe(true);
    driver.stop();
  });

  it("does nothing at all where there is no canvas to draw on", () => {
    const sink = new TelemetrySink();
    const frames = manualFrames();
    const driver = driveFixtureLevels({
      sink,
      surface: null,
      fixtures: () => rig,
      scrollTop: () => 0,
      scheduler: frames.scheduler,
    });
    sink.accept(narrowFrames[0] ?? new Uint8Array());
    frames.step();
    expect(driver.painted()).toBe(0);
    driver.stop();
  });
});
