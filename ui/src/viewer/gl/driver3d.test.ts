/**
 * The loop that draws the 3D stage — **S30b** — with a stage that records what
 * it was asked, since jsdom has no WebGL: it draws when something changed and
 * not otherwise, it draws every frame while the stage says something moves on
 * its own, and **the governor** holds it back after a frame that cost more than
 * the screen's own, so the viewer can never take the console's page from it.
 */

import { describe, expect, it } from "vitest";

import { TelemetrySink } from "../../ipc/telemetry";
import type { Scheduler } from "../../telemetry/driver";
import { finalShow, litPayload } from "../../testing/viewer-recording";
import { newCamera } from "../camera";
import type { ViewerReadout } from "../driver";
import { rigOf } from "../rig";
import type { SceneStats } from "../scene";
import { noStats } from "../scene";
import { driveStage, nextDrawAt } from "./driver3d";
import type { Stage } from "./stage";

const rig = rigOf(finalShow());

/** A stage that draws nothing and remembers everything. */
class FakeStage {
  draws = 0;
  /** Reads of the rig that drew nothing. */
  reads = 0;
  sizes: string[] = [];
  animated = false;
  disposed = false;
  lastHaze = -1;
  #changed: () => void = () => {};

  resize(width: number, height: number, ratio: number): boolean {
    const key = `${String(width)}x${String(height)}@${String(ratio)}`;
    const changed = this.sizes.at(-1) !== key;
    this.sizes.push(key);
    return changed;
  }
  setRig(): void {}
  setSelection(): void {}
  draw(_view: unknown, _camera: unknown, _seconds: number, haze: number, render = true): SceneStats {
    if (render) {
      this.draws += 1;
      this.#busy = this.busyFor;
    } else {
      this.reads += 1;
    }
    this.lastHaze = haze;
    return { ...noStats(), fixtures: rig.length, lit: 1 };
  }
  pick(): number | null {
    return 7;
  }
  /** Ticks the GPU is still busy for after each draw. */
  busyFor = 0;
  #busy = 0;
  gpuBusy(): boolean {
    if (this.#busy > 0) {
      this.#busy -= 1;
      return true;
    }
    return false;
  }
  onChange(listener: () => void): void {
    this.#changed = listener;
  }
  change(): void {
    this.#changed();
  }
  dispose(): void {
    this.disposed = true;
  }
}

function setUp(minimumFrameMs = 0) {
  let run: ((now: number) => void) | null = null;
  const scheduler: Scheduler = (callback) => {
    run = callback;
    return () => {
      run = null;
    };
  };
  const stage = new FakeStage();
  const sink = new TelemetrySink();
  const lines: ViewerReadout[] = [];
  let haze = 0.5;
  let box = { width: 400, height: 300, ratio: 1 };
  const nothingSelected: ReadonlySet<number> = new Set();
  const driver = driveStage({
    sink,
    stage: stage as unknown as Stage,
    rig: () => rig,
    selection: () => nothingSelected,
    camera: newCamera(),
    haze: () => haze,
    size: () => box,
    readout: (line) => lines.push(line),
    scheduler,
    clock: () => 0,
    minimumFrameMs,
  });
  return {
    stage,
    sink,
    driver,
    lines,
    step: (now: number) => run?.(now),
    running: () => run !== null,
    setHaze: (value: number) => {
      haze = value;
    },
    setSize: (width: number, height: number) => {
      box = { width, height, ratio: 1 };
    },
  };
}

describe("nextDrawAt", () => {
  it("never holds back a frame that cost no more than the screen's own", () => {
    expect(nextDrawAt(1000, 16.7, 16.7)).toBe(1000);
    expect(nextDrawAt(1000, 24, 16.7)).toBe(1000);
  });

  it("rests twice a draw's own time on this thread, so the viewer takes at most a third", () => {
    expect(nextDrawAt(1000, 16.7, 16.7, 20)).toBe(1040);
    expect(nextDrawAt(1000, 16.7, 16.7, 1)).toBe(1002);
  });

  it("rests twice the excess after an expensive frame, and never more than two seconds", () => {
    // Half a second a frame where the screen's frame is 17 ms: about a second's rest.
    expect(nextDrawAt(1000, 500, 1000 / 60)).toBeCloseTo(1000 + 2 * (500 - 25), 6);
    expect(nextDrawAt(1000, 5000, 1000 / 60)).toBe(3000);
  });
});

describe("driveStage", () => {
  it("draws the first frame, then only when something changed", () => {
    const setup = setUp();
    setup.step(0);
    expect(setup.stage.draws).toBe(1);
    setup.step(16);
    setup.step(33);
    expect(setup.stage.draws).toBe(1);

    setup.sink.accept(litPayload());
    setup.step(50);
    expect(setup.stage.draws).toBe(2);

    setup.setHaze(0.9);
    setup.step(66);
    expect(setup.stage.draws).toBe(3);
    expect(setup.stage.lastHaze).toBe(0.9);

    // A model or a gobo arriving is a change the stage reports itself.
    setup.stage.change();
    setup.step(83);
    expect(setup.stage.draws).toBe(4);
    expect(setup.lines.at(-1)?.fixtures).toBe(rig.length);
  });

  it("does not draw a frame that says what the last one said, whatever its sequence number", () => {
    const setup = setUp();
    setup.sink.accept(litPayload());
    setup.step(0);
    expect(setup.stage.draws).toBe(1);
    // The same levels, the next sequence number: nothing to draw.
    const next = litPayload().slice();
    next[8] = (next[8] ?? 0) + 1;
    setup.sink.accept(next);
    setup.step(16);
    expect(setup.stage.draws).toBe(1);
    // A level that moved is drawn.
    const moved = next.slice();
    moved[20] = (moved[20] ?? 0) ^ 0xff;
    setup.sink.accept(moved);
    setup.step(33);
    expect(setup.stage.draws).toBe(2);
  });

  it("draws every frame while something moves on its own", () => {
    const setup = setUp();
    setup.stage.animated = true;
    for (let frame = 0; frame < 5; frame += 1) {
      setup.step(frame * 16);
    }
    expect(setup.stage.draws).toBe(5);
  });

  it("holds back after frames that came back late, and draws again once rested", () => {
    const setup = setUp();
    setup.stage.animated = true;
    // A software renderer: the frame drawn comes back half a second later.
    setup.step(0);
    setup.step(500);
    expect(setup.stage.draws).toBe(1);
    // Resting twice what it took — until a little under a second — with the
    // page free.
    for (let now = 516; now < 940; now += 16) {
      setup.step(now);
    }
    expect(setup.stage.draws).toBe(1);
    setup.step(1000);
    expect(setup.stage.draws).toBe(2);
  });

  it("reads the rig for the readout while hidden, draws nothing, and draws once shown", () => {
    const setup = setUp();
    setup.setSize(0, 0);
    setup.stage.animated = true;
    setup.step(0);
    expect(setup.stage.reads).toBe(1);
    expect(setup.lines.at(-1)?.fixtures).toBe(rig.length);
    // Hidden, a beam moving on its own is no reason to read again.
    setup.step(16);
    setup.step(33);
    expect(setup.stage.reads).toBe(1);
    // A frame arriving is.
    setup.sink.accept(litPayload());
    setup.step(50);
    expect(setup.stage.reads).toBe(2);
    expect(setup.stage.draws).toBe(0);
    expect(setup.driver.painted()).toBe(0);
    // Shown: drawn at once.
    setup.setSize(400, 300);
    setup.step(66);
    expect(setup.stage.draws).toBe(1);
  });

  it("draws nothing new while the GPU is still on the last frame, and counts the time it took", () => {
    const setup = setUp();
    setup.stage.animated = true;
    setup.stage.busyFor = 3;
    setup.step(0);
    expect(setup.stage.draws).toBe(1);
    // Three ticks of the GPU still drawing it: nothing queued behind it.
    setup.step(16);
    setup.step(33);
    setup.step(50);
    expect(setup.stage.draws).toBe(1);
    // Done at 66: some fifty milliseconds past a screen's frame on the GPU,
    // so the next waits twice that — until about 100.
    setup.step(66);
    setup.step(83);
    expect(setup.stage.draws).toBe(1);
    setup.step(116);
    expect(setup.stage.draws).toBe(2);
  });

  it("keeps a floor under the time between frames where it is given one", () => {
    const setup = setUp(250);
    setup.stage.animated = true;
    for (let now = 0; now < 1000; now += 16) {
      setup.step(now);
    }
    // 0, 256, 512, 768: four frames in a second.
    expect(setup.stage.draws).toBe(4);
  });

  it("stops the loop and the stage together, and picks through the stage", () => {
    const setup = setUp();
    setup.step(0);
    expect(setup.driver.pick(10, 10, 12)).toBe(7);
    expect(setup.driver.painted()).toBe(1);
    setup.driver.invalidate();
    setup.step(16);
    expect(setup.driver.painted()).toBe(2);
    setup.driver.stop();
    expect(setup.running()).toBe(false);
    expect(setup.stage.disposed).toBe(true);
  });
});
