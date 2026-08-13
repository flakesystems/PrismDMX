/**
 * The loop, driven by hand.
 *
 * What is asserted here is the behaviour §7 and §8 ask for, and each of the
 * three has an obvious wrong implementation that this catches:
 *
 * - **Coalescing.** The sink holds the latest payload and no queue; the loop
 *   draws a payload once. A loop that redrew on every animation frame would
 *   double the cost of the picture for nothing.
 * - **Dropping.** A frame this build cannot read costs one picture. The last
 *   good one stays on the canvas, the fault is counted, and the log says so
 *   *once* per kind rather than thirty times a second.
 * - **Staleness.** A picture that has stopped arriving is dimmed rather than
 *   left looking current, and taken away altogether when the sink is cleared —
 *   which is what losing the daemon does.
 *
 * The frames are the recorded ones, and the clock and the scheduler are
 * arguments, so none of this waits for anything.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import type { LogRecord } from "../log/logger";
import { RecordingSurface } from "../testing/recording-surface";
import { malformedFrames, narrowFrame, wideFrame } from "../testing/telemetry-frames";
import { READOUT_EVERY_MS, driveTelemetry } from "./driver";
import type { Scheduler } from "./driver";
import { LevelPainter } from "./painter";
import { STALE_AFTER_MS } from "./stats";

/** A scheduler the test steps by hand. */
class ManualFrames {
  #run: ((now: number) => void) | null = null;
  /** Whether the loop was stopped. */
  stopped = false;

  readonly scheduler: Scheduler = (run) => {
    this.#run = run;
    return () => {
      this.stopped = true;
      this.#run = null;
    };
  };

  /** One animation frame at `now`. */
  frame(now: number): void {
    if (this.#run === null) {
      throw new Error("nothing is scheduled");
    }
    this.#run(now);
  }
}

/** A driver over a recording surface, with everything drivable by hand. */
function driven() {
  const frames = new ManualFrames();
  const sink = new TelemetrySink();
  const surface = new RecordingSurface();
  const painter = new LevelPainter(surface);
  const said: string[] = [];
  let elapsed = 0;
  const driver = driveTelemetry({
    sink,
    painter,
    readout: (text) => said.push(text),
    scheduler: frames.scheduler,
    // A paint takes a millisecond, as this clock has it: enough that a duration
    // of zero cannot be mistaken for a measurement that happened.
    clock: () => (elapsed += 1),
  });
  return { frames, sink, surface, painter, said, driver };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the loop", () => {
  it("draws a frame that arrives, once", () => {
    const { frames, sink, surface, driver } = driven();

    frames.frame(0);
    expect(surface.blits).toHaveLength(0);

    sink.accept(narrowFrame(0));
    frames.frame(16);
    expect(surface.blits).toHaveLength(1);
    expect(driver.stats.frames).toBe(1);

    // Two more animation frames with nothing new in the sink. A picture of the
    // rig that has not changed is the picture that is already on the canvas.
    frames.frame(33);
    frames.frame(50);
    expect(surface.blits).toHaveLength(1);
  });

  it("draws the newest frame and counts the ones it skipped", () => {
    const { frames, sink, driver, surface } = driven();
    sink.accept(narrowFrame(0));
    frames.frame(16);

    // Three frames arrive between two paints — a tab that was not being drawn,
    // or a daemon that got ahead. The sink keeps the last; the two in between
    // are lost, which the sequence numbers say and the readout shows.
    sink.accept(narrowFrame(1));
    sink.accept(narrowFrame(2));
    sink.accept(narrowFrame(3));
    frames.frame(33);

    expect(surface.blits).toHaveLength(2);
    expect(driver.stats.frames).toBe(2);
    expect(driver.stats.summary(33).lost).toBe(2);
  });

  it("measures what a frame costs, decode and paint together", () => {
    const { frames, sink, driver } = driven();
    sink.accept(wideFrame);
    frames.frame(16);
    // The clock advances by one per reading, so a paint that was timed reads as
    // one millisecond and one that was not reads as zero.
    expect(driver.stats.summary(16).paintMedian).toBe(1);
  });

  it("stops when it is told to", () => {
    const { frames, sink, surface, driver } = driven();
    driver.stop();
    expect(frames.stopped).toBe(true);
    sink.accept(narrowFrame(0));
    expect(surface.blits).toHaveLength(0);
  });
});

describe("a frame that cannot be read", () => {
  it("costs one picture and nothing else", () => {
    const { frames, sink, surface, driver } = driven();
    sink.accept(narrowFrame(0));
    frames.frame(16);
    const drawn = surface.lastBlit;

    for (const malformed of malformedFrames) {
      sink.accept(malformed.bytes);
      frames.frame(33);
    }

    // Not one more blit: the picture on the canvas is the last one that could
    // be read, and it is still there.
    expect(surface.blits).toHaveLength(1);
    expect(surface.lastBlit).toBe(drawn);
    expect(driver.stats.summary(33).dropped).toBe(malformedFrames.length);
    expect(driver.stats.frames).toBe(1);
    expect(driver.view.count).toBe(2);
  });

  it("is logged once for each kind, not thirty times a second", () => {
    const records: LogRecord[] = [];
    setLogSink((record) => records.push(record));
    const { frames, sink } = driven();

    const first = malformedFrames.at(0);
    if (first === undefined) {
      throw new Error("the recording has no malformed frames");
    }
    for (let repeat = 0; repeat < 30; repeat += 1) {
      // A different array each time, so the loop sees a new payload rather than
      // the one it has already drawn.
      sink.accept(Uint8Array.from(first.bytes));
      frames.frame(repeat * 16);
    }

    const warnings = records.filter((record) => record.level === "warn");
    expect(warnings).toHaveLength(1);
    expect(warnings[0]?.fields.fault).toBe(first.fault);
  });
});

describe("a picture that is no longer of now", () => {
  it("is dimmed where it stands", () => {
    const { frames, sink, surface, said } = driven();
    sink.accept(narrowFrame(0));
    frames.frame(0);
    const fills = surface.fills.length;

    frames.frame(STALE_AFTER_MS);
    expect(surface.fills).toHaveLength(fills);

    frames.frame(STALE_AFTER_MS + 1);
    const wash = surface.fills.at(-1);
    expect(wash?.width).toBe(surface.width);
    expect(said.at(-1)?.startsWith("not live")).toBe(true);

    // Once, not on every frame after that.
    const dimmed = surface.fills.length;
    frames.frame(STALE_AFTER_MS + 200);
    expect(surface.fills).toHaveLength(dimmed);

    // And a frame that arrives afterwards is a live picture again.
    sink.accept(narrowFrame(1));
    frames.frame(STALE_AFTER_MS + 300);
    expect(surface.blits).toHaveLength(2);
    expect(said.at(-1)?.startsWith("not live")).toBe(false);
  });

  it("is taken off the canvas altogether when the daemon goes", () => {
    const { frames, sink, surface, said, driver } = driven();
    sink.accept(narrowFrame(0));
    frames.frame(0);
    expect(driver.view.hasFrame).toBe(true);

    // What `createDesk` does when the connection ends: the sink is cleared,
    // because a picture of the rig from a daemon that has stopped is exactly as
    // stale as a fader value from one.
    sink.clear();
    frames.frame(16);

    expect(driver.view.hasFrame).toBe(false);
    expect(driver.stats.frames).toBe(0);
    expect(surface.clears.at(-1)).toBeDefined();
    expect(surface.blits).toHaveLength(1);
    expect(said.at(-1)).toBe("waiting for telemetry");

    // And nothing is redrawn while there is nothing to draw.
    const clears = surface.clears.length;
    frames.frame(33);
    frames.frame(50);
    expect(surface.clears).toHaveLength(clears);
  });
});

describe("the readout", () => {
  it("is rewritten a few times a second rather than thirty", () => {
    const { frames, sink, said } = driven();
    for (let frame = 0; frame < 30; frame += 1) {
      sink.accept(narrowFrame(frame));
      frames.frame(frame * (1000 / 30));
    }
    // One second of telemetry: four lines, not thirty. Writing `textContent`
    // thirty times a second is a layout thirty times a second for a number
    // nobody can read that fast.
    expect(said.length).toBeLessThanOrEqual(1 + 1000 / READOUT_EVERY_MS);
    expect(said.at(-1)).toContain("2 universes");
  });

  it("is written at once when something goes wrong", () => {
    const { frames, sink, said } = driven();
    sink.accept(narrowFrame(0));
    frames.frame(0);
    const lines = said.length;
    const malformed = malformedFrames.at(0);
    if (malformed === undefined) {
      throw new Error("the recording has no malformed frames");
    }
    sink.accept(malformed.bytes);
    frames.frame(1);
    expect(said.length).toBe(lines + 1);
    expect(said.at(-1)).toContain("dropped");
  });
});

describe("with no canvas at all", () => {
  it("still reads the channel and still says what it is doing", () => {
    // Which is what happens in `jsdom`, and in a browser that has run out of
    // canvas contexts. A telemetry panel that could not draw must not be a
    // telemetry panel that throws.
    const frames = new ManualFrames();
    const sink = new TelemetrySink();
    const said: string[] = [];
    const driver = driveTelemetry({
      sink,
      painter: null,
      readout: (text) => said.push(text),
      scheduler: frames.scheduler,
      clock: () => 0,
    });

    sink.accept(wideFrame);
    frames.frame(16);
    expect(driver.stats.frames).toBe(1);
    expect(driver.view.count).toBe(64);
    expect(said.at(-1)).toContain("64 universes");
  });

  it("defaults to the browser's own clock and frames", () => {
    // The defaults are what ships; the arguments are what the tests use. This
    // is the one place that says the defaults exist and are wired up: the loop
    // asks for an animation frame, asks for another from inside it, and stops
    // asking when it is stopped.
    const asked: FrameRequestCallback[] = [];
    let cancelled: number | null = null;
    vi.stubGlobal("requestAnimationFrame", (run: FrameRequestCallback) => {
      asked.push(run);
      return asked.length;
    });
    vi.stubGlobal("cancelAnimationFrame", (handle: number) => {
      cancelled = handle;
    });

    const sink = new TelemetrySink();
    const driver = driveTelemetry({ sink, painter: null });
    expect(asked).toHaveLength(1);

    sink.accept(narrowFrame(0));
    asked[0]?.(16);
    expect(driver.stats.frames).toBe(1);
    // The next frame was asked for before this one was drawn, so a paint that
    // throws cannot stop the loop for good.
    expect(asked).toHaveLength(2);

    driver.stop();
    expect(cancelled).toBe(2);
    vi.unstubAllGlobals();
  });
});
