/**
 * The loop that draws the 3D stage — **S30b** — on the same terms as
 * `../driver.ts` draws the 2D fallback and `telemetry/driver.ts` the DMX
 * Sheet: outside React, off the telemetry sink, and only when something
 * changed — the payload, the rig, the selection, the camera, the size, the
 * haze, a model or a gobo arriving. The one addition is **time**: while a
 * beam strobes or a gobo spins the picture changes with nothing else changing,
 * so the stage says so and the loop draws every frame until it stops.
 *
 * # The governor
 *
 * **The viewer may never cost the console its command line.** A frame's real
 * price is not the milliseconds `Stage.draw` takes on this thread — WebGL
 * queues the work — but how late the *next* frame comes back. On a machine
 * whose graphics are done in software (a thin client, a virtual machine, the
 * browser the end-to-end suite runs in) a frame of volumetric beams can take
 * half a second, and drawing one after another left the page no time to read
 * its socket: the daemon's connection dropped, on the first real rig tried.
 *
 * So the loop measures that price, and after an expensive frame it waits
 * **twice the excess** before the next one: a frame that costs 500 ms where
 * the screen's own frame is 17 is followed by about a second of nothing,
 * which keeps the viewer under half the page's time. A frame that costs no
 * more than the screen's own is never held back.
 *
 * The other price is this thread's own: three.js walking a scene of five
 * hundred fixtures is twenty milliseconds of JavaScript before the GPU sees
 * anything, and the DMX Sheet paints on the same thread. So a draw is also
 * followed by **twice its own time** — the viewer takes at most a third of
 * the page — which the 64-universe test in `e2e/viewer.spec.ts` holds the DMX
 * Sheet's rate to. A draw of a couple of milliseconds, an ordinary rig, waits
 * a few and is never noticed.
 *
 * And the third is the GPU's, which neither of those sees: a frame handed to
 * WebGL is queued, and on a software renderer the queue is the processor the
 * page and every other canvas on it share. The stage fences each frame
 * (`Stage.gpuBusy`); **no frame is drawn while the last is still on the GPU**,
 * and the time it took there counts like the other two. That, not the first
 * two, is what kept the DMX Sheet at its rate beside a viewer being orbited
 * round five hundred fixtures on SwiftShader.
 */

import type { TelemetrySink } from "../../ipc/telemetry";
import type { Scheduler } from "../../telemetry/driver";
import { animationFrames } from "../../telemetry/driver";
import { TelemetryFrameView } from "../../telemetry/frame";
import type { Camera } from "../camera";
import type { ViewerDriver, ViewerReadout } from "../driver";
import type { RigFixture } from "../rig";
import type { SceneStats } from "../scene";
import { noStats } from "../scene";
import type { Stage } from "./stage";

/** How often the readout is rewritten. */
const READOUT_EVERY_MS = 250;

/** How many paint times the percentiles are taken over. */
const KEPT = 120;

/** How much longer than its own cost the loop rests after an expensive frame. */
const REST = 2;

/** The longest rest, so a very slow machine still shows the rig moving. */
const MAX_REST_MS = 2000;

/**
 * When the next frame may be drawn, given when the last one was drawn, what
 * frames cost until the next came back, the screen's own frame time, and how
 * long the last draw took on this thread. Pure, for the tests.
 */
export function nextDrawAt(drawnAt: number, cost: number, screen: number, work = 0): number {
  const excess = Math.max(0, cost - screen * 1.5);
  return drawnAt + Math.min(MAX_REST_MS, Math.max(excess, work) * REST);
}

/** How to drive the stage. */
export interface StageDriverOptions {
  readonly sink: TelemetrySink | null;
  readonly stage: Stage;
  readonly rig: () => readonly RigFixture[];
  readonly selection: () => ReadonlySet<number>;
  readonly camera: Camera;
  readonly haze: () => number;
  /** The canvas's size in CSS pixels and the screen's pixel ratio. */
  readonly size: () => { width: number; height: number; ratio: number };
  readonly readout?: (readout: ViewerReadout) => void;
  readonly scheduler?: Scheduler;
  readonly clock?: () => number;
  /**
   * The shortest time between two frames, milliseconds — nought for none.
   * A software renderer gets {@link SOFTWARE_FRAME_MS}; see the module
   * documentation.
   */
  readonly minimumFrameMs?: number;
}

/**
 * Four frames a second, where WebGL is done in software. Measured on the
 * 64-universe rig with the viewer being orbited: at six frames a second the
 * DMX Sheet beside it fell to twenty, a third below its thirty, because every
 * software frame holds up the next frame of the whole page; the governor
 * cannot see that price, since the GPU process reports the frame done long
 * before the page is free. A machine without graphics hardware cannot show a
 * large rig moving smoothly either way; it can show it, and keep the console.
 */
export const SOFTWARE_FRAME_MS = 250;

function percentile(times: Float64Array, count: number, share: number): number {
  if (count === 0) {
    return 0;
  }
  const sorted = times.slice(0, count).sort();
  return sorted[Math.min(count - 1, Math.max(0, Math.ceil(share * count) - 1))] ?? 0;
}

/** Where a frame's sequence number is: bytes 8 to 16 of the header (`../../telemetry/frame.ts`). */
const SEQUENCE_FROM = 8;
const SEQUENCE_TO = 16;

/**
 * Whether `payload` says what the first `length` bytes of `held` said — every
 * byte but the sequence number, which is new in every frame by design.
 */
function sameBytes(payload: Uint8Array, held: Uint8Array, length: number): boolean {
  if (payload.length !== length) {
    return false;
  }
  for (let index = 0; index < length; index += 1) {
    if (index === SEQUENCE_FROM) {
      index = SEQUENCE_TO - 1;
      continue;
    }
    if (payload[index] !== held[index]) {
      return false;
    }
  }
  return true;
}

/** Starts drawing. The returned `stop` must be called when the window goes. */
export function driveStage(options: StageDriverOptions): ViewerDriver {
  const { sink, stage, camera } = options;
  const schedule = options.scheduler ?? animationFrames;
  const clock = options.clock ?? (() => performance.now());
  const readout = options.readout ?? (() => {});
  const view = new TelemetryFrameView();
  const times = new Float64Array(KEPT);
  let kept = 0;
  let next = 0;
  let painted = 0;
  let forced = true;
  let drawnPayload: Uint8Array | null = null;
  // The bytes last drawn from. The daemon sends a frame 44 times a second
  // whether or not a level moved, and a rig of five hundred fixtures redrawn
  // for a frame that says what the last one said is the console's time spent
  // on nothing; so a new payload with the same bytes is no change.
  let drawnBytes = new Uint8Array(0);
  let drawnLength = -1;
  let drawnRig: readonly RigFixture[] | null = null;
  let drawnSelection: ReadonlySet<number> | null = null;
  let drawnCamera = -1;
  let drawnHaze = -1;
  let last: SceneStats = noStats();
  let saidAt: number | null = null;
  let said = "";
  let width = 0;
  let height = 0;
  // The governor: when the last frame was drawn, the tick it was drawn in,
  // what frames cost, and the screen's own frame time.
  let drawnAt = Number.NEGATIVE_INFINITY;
  let drewLastTick = false;
  let cost = 0;
  let screen = 1000 / 60;
  let previousTick: number | null = null;
  // Whether the rig was read while the picture was hidden and not drawn since.
  let owed = false;
  // How long the last draw took on this thread — or on the GPU, when that was longer.
  let work = 0;
  // When the frame still on the GPU was handed to it.
  let gpuSince: number | null = null;
  stage.onChange(() => {
    forced = true;
  });

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
    saidAt = now;
    if (key === said && !force) {
      return;
    }
    said = key;
    readout(line);
  };

  const stop = schedule((now) => {
    if (previousTick !== null) {
      const interval = now - previousTick;
      if (drewLastTick) {
        // What the frame drawn last tick really cost: how late this one came.
        cost = cost === 0 ? interval : cost * 0.7 + interval * 0.3;
      } else if (interval > 0) {
        screen = Math.min(screen * 0.9 + interval * 0.1, 1000 / 20);
      }
    }
    previousTick = now;
    drewLastTick = false;
    const rig = options.rig();
    const selection = options.selection();
    const haze = options.haze();
    const payload = sink?.latest ?? null;
    const box = options.size();
    const hidden = box.width <= 0 || box.height <= 0;
    if (!hidden && stage.resize(box.width, box.height, box.ratio)) {
      // The size is taken every frame, not only when it changes: a stage
      // built on a canvas another stage already sized finds nothing to change.
      forced = true;
    }
    width = hidden ? 0 : box.width;
    height = hidden ? 0 : box.height;
    if (rig !== drawnRig) {
      stage.setRig(rig);
    }
    if (selection !== drawnSelection) {
      stage.setSelection(selection);
    }
    if (payload !== drawnPayload && payload !== null && sameBytes(payload, drawnBytes, drawnLength)) {
      drawnPayload = payload;
    }
    const changed =
      forced ||
      payload !== drawnPayload ||
      rig !== drawnRig ||
      selection !== drawnSelection ||
      camera.version !== drawnCamera ||
      haze !== drawnHaze ||
      (!hidden && (stage.animated || owed));
    // A hidden picture — a window behind another, a tab not shown — has no
    // size. Nothing is drawn, but the rig is still read when it changes, so
    // the readout says what is patched and lit whether anybody can see the
    // picture or not; and the picture is owed a draw for when it is shown.
    if (gpuSince !== null && !stage.gpuBusy()) {
      // The last frame has left the GPU. Found done at the next tick is done
      // within a frame, which is no cost worth resting for; beyond that, it
      // is what the GPU took.
      work = Math.max(work, now - gpuSince - screen);
      gpuSince = null;
    }
    const waiting = gpuSince !== null || now - drawnAt < (options.minimumFrameMs ?? 0);
    if (!changed || (!hidden && (waiting || now < nextDrawAt(drawnAt, cost, screen, work)))) {
      say(now, false);
      return;
    }
    if (!hidden) {
      drawnAt = now;
      drewLastTick = true;
    }
    const before = `${String(last.fixtures)}/${String(last.lit)}/${String(last.unplaced)}/${String(last.absent)}`;
    forced = false;
    drawnRig = rig;
    drawnSelection = selection;
    drawnCamera = camera.version;
    drawnHaze = haze;
    if (payload !== drawnPayload) {
      drawnPayload = payload;
      if (payload === null) {
        view.clear();
        drawnLength = -1;
      } else {
        view.read(payload);
        if (drawnBytes.length < payload.length) {
          drawnBytes = new Uint8Array(payload.length);
        }
        drawnBytes.set(payload);
        drawnLength = payload.length;
      }
    }
    owed = hidden;
    if (hidden) {
      last = stage.draw(view.hasFrame ? view : null, camera, now / 1000, haze, false);
    } else {
      const started = clock();
      last = stage.draw(view.hasFrame ? view : null, camera, now / 1000, haze);
      work = clock() - started;
      times[next] = work;
      gpuSince = now;
      next = (next + 1) % KEPT;
      kept = Math.min(KEPT, kept + 1);
      painted += 1;
    }
    const after = `${String(last.fixtures)}/${String(last.lit)}/${String(last.unplaced)}/${String(last.absent)}`;
    say(now, after !== before);
  });

  return {
    stop: () => {
      stop();
      stage.dispose();
    },
    invalidate: () => {
      forced = true;
    },
    pick: (x, y) => stage.pick(x, y, Math.max(1, width), Math.max(1, height)),
    painted: () => painted,
  };
}
