/**
 * The loop that turns received bytes into a picture of the rig — S24's
 * `telemetry/driver.ts` with a different picture in it, and the reason the
 * viewer **never blocks the telemetry canvas or the engine**.
 *
 * - **Not the engine**, because nothing here asks the daemon anything. The
 *   picture is drawn from the show document the client already mirrors and
 *   the telemetry frame the daemon already publishes; the tick does not know a
 *   viewer is open.
 * - **Not React**, because nothing here sets state. The sink is read, the
 *   frame is decoded and the canvas is painted; the readout line is written
 *   with `textContent` and `dataset` by hand. `docs/IPC_PROTOCOL.md` §7's last
 *   paragraph holds for this window exactly as for the DMX Sheet.
 * - **Not the DMX Sheet**, because a frame is painted only when something in
 *   it changed — the payload, the rig, the selection, the camera or the size —
 *   and a rig nobody is touching costs nothing at all. What a paint does cost
 *   is measured, and `ui/e2e/viewer.spec.ts` holds both canvases to their
 *   budgets with both open on the widest rig this desk carries.
 */

import type { TelemetrySink } from "../ipc/telemetry";
import type { Scheduler } from "../telemetry/driver";
import { animationFrames } from "../telemetry/driver";
import { TelemetryFrameView } from "../telemetry/frame";
import type { Camera } from "./camera";
import type { RigFixture } from "./rig";
import type { SceneStats } from "./scene";
import { Scene, noStats } from "./scene";
import type { ViewSurface } from "./surface";

/** How often the readout line is rewritten. */
export const READOUT_EVERY_MS = 250;

/** How many paint times the percentiles are taken over. */
const KEPT = 120;

/** What the readout says. */
export interface ViewerReadout extends SceneStats {
  /** Pictures painted since the window opened. */
  readonly painted: number;
  /** The median paint, in milliseconds, over the last {@link KEPT}. */
  readonly median: number;
  /** The 99th percentile paint, in milliseconds. */
  readonly p99: number;
}

/** The line an operator reads under the picture. */
export function readoutText(readout: ViewerReadout): string {
  if (readout.fixtures === 0) {
    return "nothing is patched";
  }
  const parts = [
    `${String(readout.fixtures)} ${readout.fixtures === 1 ? "fixture" : "fixtures"}`,
    `${String(readout.lit)} lit`,
  ];
  if (readout.unplaced > 0) {
    parts.push(`${String(readout.unplaced)} not placed`);
  }
  if (readout.absent > 0) {
    parts.push(`${String(readout.absent)} on no output`);
  }
  parts.push(`${readout.median.toFixed(1)} ms`);
  return parts.join(" · ");
}

/** How to drive the viewer. Every clock is an argument, as always. */
export interface ViewerDriverOptions {
  /** Where the payloads are, or `null` where there is no telemetry. */
  readonly sink: TelemetrySink | null;
  /** What to draw on, or `null` where there is no canvas. */
  readonly surface: ViewSurface | null;
  /** The rig, read each frame so a repatch or a placement is followed. */
  readonly rig: () => readonly RigFixture[];
  /** The selection, read each frame. */
  readonly selection: () => ReadonlySet<number>;
  /** The camera, mutated by the pointer and read here. */
  readonly camera: Camera;
  /** Device pixels per CSS pixel. */
  readonly scale?: () => number;
  /** Where the readout goes. */
  readonly readout?: (readout: ViewerReadout) => void;
  /** What calls the loop. */
  readonly scheduler?: Scheduler;
  /** What times a paint. */
  readonly clock?: () => number;
}

/** A running viewer. */
export interface ViewerDriver {
  /** Stops the loop. */
  stop: () => void;
  /** Asks for a picture on the next frame whatever has changed — a resize. */
  invalidate: () => void;
  /** The fixture drawn nearest a point on the surface, in device pixels. */
  pick: (x: number, y: number, radius: number) => number | null;
  /** How many pictures have been painted. Read by the tests. */
  readonly painted: () => number;
}

/** The value `share` of the way up a sorted copy of `times`. */
function percentile(times: Float64Array, count: number, share: number): number {
  if (count === 0) {
    return 0;
  }
  const sorted = times.slice(0, count).sort();
  const index = Math.min(count - 1, Math.max(0, Math.ceil(share * count) - 1));
  return sorted[index] ?? 0;
}

/**
 * Starts drawing. The returned {@link ViewerDriver.stop} must be called when
 * the window goes, or a closed viewer would go on decoding frames for nobody.
 */
export function driveViewer(options: ViewerDriverOptions): ViewerDriver {
  const { sink, surface, camera } = options;
  const scale = options.scale ?? (() => 1);
  const schedule = options.scheduler ?? animationFrames;
  const clock = options.clock ?? (() => performance.now());
  const readout = options.readout ?? (() => {});

  const scene = new Scene();
  const view = new TelemetryFrameView();
  const times = new Float64Array(KEPT);
  let kept = 0;
  let next = 0;
  let painted = 0;
  let forced = true;
  let drawnPayload: Uint8Array | null = null;
  let drawnRig: readonly RigFixture[] | null = null;
  let drawnSelection: ReadonlySet<number> | null = null;
  let drawnCamera = -1;
  let last: SceneStats = noStats();
  let saidAt: number | null = null;
  let said = "";

  const say = (now: number, force: boolean): void => {
    if (!force && saidAt !== null && now - saidAt < READOUT_EVERY_MS) {
      return;
    }
    const line: ViewerReadout = {
      ...last,
      painted,
      median: percentile(times, kept, 0.5),
      p99: percentile(times, kept, 0.99),
    };
    const key = `${String(line.fixtures)}/${String(line.lit)}/${String(line.unplaced)}/${String(line.absent)}/${String(painted)}`;
    if (key === said && !force) {
      saidAt = now;
      return;
    }
    said = key;
    saidAt = now;
    readout(line);
  };

  const stop = schedule((now) => {
    if (surface === null) {
      return;
    }
    const payload = sink?.latest ?? null;
    const rig = options.rig();
    const selection = options.selection();
    if (
      !forced &&
      payload === drawnPayload &&
      rig === drawnRig &&
      selection === drawnSelection &&
      camera.version === drawnCamera
    ) {
      say(now, false);
      return;
    }
    const countsBefore = `${String(last.fixtures)}/${String(last.lit)}/${String(last.unplaced)}/${String(last.absent)}`;
    forced = false;
    drawnRig = rig;
    drawnSelection = selection;
    drawnCamera = camera.version;
    if (payload !== drawnPayload) {
      drawnPayload = payload;
      if (payload === null) {
        // The connection went and the sink was cleared: a picture of beams
        // from a daemon that has stopped is as stale as a fader value from
        // one, so the rig goes dark rather than freezing lit.
        view.clear();
      } else {
        // A frame this build cannot read costs one picture and nothing else:
        // the view still holds the last readable frame, and that is drawn.
        view.read(payload);
      }
    }

    const started = clock();
    last = scene.draw(surface, rig, view.hasFrame ? view : null, camera, selection, scale());
    times[next] = clock() - started;
    next = (next + 1) % KEPT;
    kept = Math.min(KEPT, kept + 1);
    painted += 1;
    const countsAfter = `${String(last.fixtures)}/${String(last.lit)}/${String(last.unplaced)}/${String(last.absent)}`;
    say(now, countsAfter !== countsBefore);
  });

  return {
    stop,
    invalidate: () => {
      forced = true;
    },
    pick: (x, y, radius) => scene.pick(x, y, radius),
    painted: () => painted,
  };
}
