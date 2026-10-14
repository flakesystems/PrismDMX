/**
 * The telemetry panel: a canvas, a line of numbers, and **no state at all**.
 *
 * This component renders once. What is on the canvas after that has nothing to
 * do with React — the loop in `./driver.ts` owns the pixels and writes the
 * summary line with `textContent`, and neither of those is a render.
 *
 * `telemetry.render.test.tsx` is the proof rather than this comment: a
 * `<Profiler>` around the whole interface, three hundred frames of 64 universes
 * through a real connection, and the commit count unchanged.
 *
 * # What is client-local, and stays here
 *
 * `ARCHITECTURE_SPEC.md` §4.2: scroll position, zoom and camera are *not*
 * session state. The size of this canvas is exactly that kind of thing — it
 * follows the element, it differs per screen, and it is nobody else's business.
 * So it lives in a ref and in the element, and no command is sent about it.
 */

import { useEffect, useRef } from "react";
import type { ReactNode } from "react";

import type { TelemetryChannel } from "./context";
import { TelemetryContext, useTelemetryChannel } from "./context";
import { driveTelemetry } from "./driver";
import { LevelPainter, canvasSurface, devicePixelRatio, resizeCanvas } from "./painter";

/** Makes one telemetry channel available to everything below it. */
export function TelemetryProvider({
  channel,
  children,
}: {
  readonly channel: TelemetryChannel;
  readonly children: ReactNode;
}) {
  return <TelemetryContext value={channel}>{children}</TelemetryContext>;
}

/**
 * The level view.
 *
 * Renders nothing when no channel is attached: an interface without telemetry
 * is missing a picture, not broken, and a panel that threw would take the
 * command line with it.
 */
export function TelemetryPanel() {
  const channel = useTelemetryChannel();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const readoutRef = useRef<HTMLParagraphElement>(null);

  useEffect(() => {
    if (channel === null) {
      return;
    }
    const canvas = canvasRef.current;
    const readoutNode = readoutRef.current;
    const build = channel.surface ?? canvasSurface;
    const surface = canvas === null ? null : build(canvas);
    const painter = surface === null ? null : new LevelPainter(surface);

    // The chrome is invalidated with the size, because a resized context loses
    // its state and a smoothed blit is a blurred grid.
    const measure = (): void => {
      if (resizeCanvas(canvas, devicePixelRatio())) {
        painter?.invalidate();
      }
    };
    measure();

    const driver = driveTelemetry({
      sink: channel.sink,
      painter,
      readout: (text) => {
        if (readoutNode !== null) {
          readoutNode.textContent = text;
        }
      },
      scale: devicePixelRatio,
      scheduler: channel.scheduler,
      clock: channel.clock,
    });

    const observer = observeSize(canvas, measure);
    return () => {
      observer?.();
      driver.stop();
    };
  }, [channel]);

  if (channel === null) {
    return null;
  }
  return (
    <section className="panel panel-telemetry" data-testid="telemetry">
      <h2>Output levels</h2>
      <canvas ref={canvasRef} className="telemetry-canvas" data-testid="telemetry-canvas" />
      <p ref={readoutRef} className="telemetry-readout" data-testid="telemetry-stats">
        waiting for telemetry
      </p>
    </section>
  );
}

/**
 * Calls `changed` when the element's box changes, answering with the way to
 * stop.
 *
 * A `ResizeObserver` where there is one — which is every browser this ships in
 * and no `jsdom` — and the window's own resize event otherwise, which catches
 * the case that matters most and costs nothing where it is not needed.
 */
function observeSize(element: Element | null, changed: () => void): (() => void) | null {
  if (element !== null && typeof ResizeObserver !== "undefined") {
    const observer = new ResizeObserver(changed);
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }
  if (typeof window === "undefined") {
    return null;
  }
  window.addEventListener("resize", changed);
  return () => {
    window.removeEventListener("resize", changed);
  };
}
