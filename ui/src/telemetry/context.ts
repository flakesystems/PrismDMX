/**
 * The telemetry channel, as a component tree sees it.
 *
 * Deliberately **not** the desk store. `store/store-context.ts` carries the read
 * model, which is state a render depends on; this carries a sink, a scheduler
 * and a way of getting a canvas — a *device*, in the sense every other seam in
 * this project uses the word. Nothing a component reads from here changes, ever,
 * so nothing a component reads from here can cause a render.
 *
 * That is the arrangement §7's last paragraph asks for, expressed in the type
 * system: there is no way to subscribe to telemetry, because there is nothing
 * here to subscribe to.
 */

import { createContext, use } from "react";

import type { TelemetrySink } from "../ipc/telemetry";
import type { Scheduler } from "./driver";
import type { LevelSurface } from "./painter";

/** What the telemetry panel needs, and what a test may replace. */
export interface TelemetryChannel {
  /** Where the payloads arrive. Held, never subscribed to. */
  readonly sink: TelemetrySink;
  /** What drives the picture. Animation frames in a browser. */
  readonly scheduler?: Scheduler;
  /** What times a paint. `performance.now` in a browser. */
  readonly clock?: () => number;
  /**
   * How to get a surface out of the canvas element.
   *
   * `canvasSurface` in a browser, and a recording one in the tests — `jsdom` has
   * no rasteriser, so this is the seam that keeps the drawing testable at all.
   */
  readonly surface?: (canvas: HTMLCanvasElement) => LevelSurface | null;
}

/** The channel this subtree draws, or `null` where none is attached. */
export const TelemetryContext = createContext<TelemetryChannel | null>(null);

/**
 * The channel, or `null`.
 *
 * `null` rather than a throw, unlike {@link import("../store/hooks").useDeskStore}:
 * an interface with no telemetry attached is a legitimate interface — the show
 * runs, the command line works, and the only thing missing is a picture of the
 * levels.
 */
export function useTelemetryChannel(): TelemetryChannel | null {
  return use(TelemetryContext);
}
