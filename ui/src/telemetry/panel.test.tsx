/**
 * The panel: the element, the sizing, and the loop being stopped.
 *
 * `render.test.tsx` makes the claim that matters — no renders — and this covers
 * what is around it. Two of these are worth having for their own sake:
 *
 * - **The bitmap follows the element.** A canvas has two sizes, and one left at
 *   its 300 × 150 default is the classic reason a canvas looks blurred. What is
 *   drawn is device pixels, so the two have to be kept in step.
 * - **The loop is stopped when the panel goes.** Otherwise an interface that
 *   closed this window would go on decoding 32 kB thirty times a second for a
 *   canvas nobody can see — and the S25 window system is about to make closing
 *   windows an ordinary thing to do.
 */

import { render, screen } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TelemetrySink } from "../ipc/telemetry";
import { RecordingSurface } from "../testing/recording-surface";
import { narrowFrame } from "../testing/telemetry-frames";
import type { TelemetryChannel } from "./context";
import type { Scheduler } from "./driver";
import { TelemetryPanel, TelemetryProvider } from "./panel";

/** A scheduler the test steps by hand, which also records being stopped. */
class ManualFrames {
  #run: ((now: number) => void) | null = null;
  stopped = false;

  readonly scheduler: Scheduler = (run) => {
    this.#run = run;
    return () => {
      this.stopped = true;
      this.#run = null;
    };
  };

  frame(now: number): void {
    this.#run?.(now);
  }

  get running(): boolean {
    return this.#run !== null;
  }
}

/** A mounted panel with everything drivable. */
function panel(overrides: Partial<TelemetryChannel> = {}) {
  const frames = new ManualFrames();
  const sink = new TelemetrySink();
  const surface = new RecordingSurface();
  const channel: TelemetryChannel = {
    sink,
    scheduler: frames.scheduler,
    clock: () => 0,
    surface: () => surface,
    ...overrides,
  };
  const view = render(
    <TelemetryProvider channel={channel}>
      <TelemetryPanel />
    </TelemetryProvider>,
  );
  return { frames, sink, surface, view };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the panel", () => {
  it("renders nothing at all where no channel is attached", () => {
    // Which is the case in every S23 test, and in any interface built without
    // telemetry: a missing picture, not a broken window.
    render(<TelemetryPanel />);
    expect(screen.queryByTestId("telemetry")).toBeNull();
  });

  it("draws the frames that arrive and says what it is doing", () => {
    const { frames, sink, surface } = panel();
    expect(screen.getByTestId("telemetry-canvas")).not.toBeNull();
    expect(screen.getByTestId("telemetry-stats").textContent).toBe("waiting for telemetry");

    act(() => {
      sink.accept(narrowFrame(0));
      frames.frame(0);
    });

    expect(surface.blits).toHaveLength(1);
    expect(screen.getByTestId("telemetry-stats").textContent).toContain("2 universes");
  });

  it("stops the loop when it goes", () => {
    const { frames, view } = panel();
    expect(frames.running).toBe(true);
    view.unmount();
    expect(frames.stopped).toBe(true);
  });

  it("matches the canvas's pixels to its box, and to the screen's density", () => {
    vi.stubGlobal("devicePixelRatio", 2);
    const { frames, sink, surface } = panel();
    const canvas = screen.getByTestId("telemetry-canvas");
    if (!(canvas instanceof HTMLCanvasElement)) {
      throw new Error("the panel draws on a canvas");
    }
    // jsdom does no layout, so the box is stated rather than measured — which
    // is exactly what a browser would have measured.
    Object.defineProperty(canvas, "clientWidth", { value: 800, configurable: true });
    Object.defineProperty(canvas, "clientHeight", { value: 300, configurable: true });

    act(() => {
      window.dispatchEvent(new Event("resize"));
      sink.accept(narrowFrame(0));
      frames.frame(0);
    });

    // Twice the CSS size, because a pixel on this screen is half a CSS pixel.
    expect([canvas.width, canvas.height]).toEqual([1600, 600]);
    expect(surface.blits).toHaveLength(1);
  });

  it("follows the element with a ResizeObserver where there is one", () => {
    // There is one in every browser this ships in and none in `jsdom`, so the
    // panel has both paths and this is the one an operator gets.
    const observed: Element[] = [];
    let disconnected = 0;
    let notify: (() => void) | null = null;
    class FakeResizeObserver {
      constructor(callback: () => void) {
        notify = callback;
      }
      observe(element: Element): void {
        observed.push(element);
      }
      disconnect(): void {
        disconnected += 1;
      }
      unobserve(): void {}
    }
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    vi.stubGlobal("devicePixelRatio", 1);

    const { view } = panel();
    const canvas = screen.getByTestId("telemetry-canvas");
    expect(observed).toEqual([canvas]);
    if (!(canvas instanceof HTMLCanvasElement)) {
      throw new Error("the panel draws on a canvas");
    }

    Object.defineProperty(canvas, "clientWidth", { value: 640, configurable: true });
    Object.defineProperty(canvas, "clientHeight", { value: 200, configurable: true });
    act(() => {
      notify?.();
    });
    expect([canvas.width, canvas.height]).toEqual([640, 200]);

    view.unmount();
    expect(disconnected).toBe(1);
  });

  it("carries on when the canvas will not give a context", () => {
    // A browser that has run out of contexts, or `jsdom`. The channel is read,
    // the numbers are right, and there is simply no picture.
    const { frames, sink } = panel({ surface: () => null });
    act(() => {
      sink.accept(narrowFrame(0));
      frames.frame(0);
    });
    expect(screen.getByTestId("telemetry-stats").textContent).toContain("2 universes");
  });
});
