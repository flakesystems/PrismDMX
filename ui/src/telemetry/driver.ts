/**
 * The loop that turns received bytes into a picture — and the one place in this
 * interface where §7's last paragraph is actually kept.
 *
 * Nothing here is React. The sink holds the latest payload, this loop wakes on
 * an animation frame, decodes **that one payload** and draws it. No state is
 * set, no context is published, no component is told anything: the only thing
 * that reaches the DOM outside the canvas is one line of text, written with
 * `textContent` by hand.
 *
 * # Why the decode is here and not where the frame arrives
 *
 * Two reasons, and the second is the important one.
 *
 * It is cheaper: telemetry arrives at 30 Hz and a screen paints at 60, but the
 * *browser* decides when it can paint, and a tab in the background paints not at
 * all. Decoding on arrival would be work done for pictures nobody sees.
 *
 * And it puts the decoder on the far side of the sink from the control channel.
 * A frame this build cannot read is discovered *here*, in a loop whose only
 * output is a canvas — so a malformed telemetry frame cannot reach the store,
 * the connection or the mirror even by mistake. That is S24's third exit
 * criterion made structural rather than promised: dropped telemetry degrades the
 * picture and nothing else.
 *
 * # Coalescing, again
 *
 * The daemon coalesces and drops (§8); the sink keeps the latest payload and no
 * queue; and this loop redraws only when the payload it is holding is one it has
 * not drawn. Three frames arriving between two paints cost one paint, and the
 * two that were skipped are counted as lost by their sequence numbers rather
 * than queued — because a picture of the rig that is three frames old has no
 * value at all.
 */

import { logger } from "../log/logger";
import type { TelemetrySink } from "../ipc/telemetry";
import { TelemetryFrameView, faultText } from "./frame";
import type { LevelPainter } from "./painter";
import { STALE_AFTER_MS, TelemetryStats, summaryText } from "./stats";

const log = logger("telemetry");

/** How often the readout line is rewritten. */
export const READOUT_EVERY_MS = 250;

/**
 * Something that calls `run` once per frame until it is stopped.
 *
 * The clock is an argument for the reason every clock in this project is: a test
 * that waited for real animation frames would be a test that waited.
 */
export type Scheduler = (run: (now: number) => void) => () => void;

/** The browser's animation frames. */
export const animationFrames: Scheduler = (run) => {
  let handle = requestAnimationFrame(function frame(now) {
    handle = requestAnimationFrame(frame);
    run(now);
  });
  return () => {
    cancelAnimationFrame(handle);
  };
};

/** How to drive the picture. Everything has a default that works in a browser. */
export interface TelemetryDriverOptions {
  /** Where the payloads are. */
  readonly sink: TelemetrySink;
  /** What draws them, or `null` where there is no canvas to draw on. */
  readonly painter: LevelPainter | null;
  /** Where the summary line goes. */
  readonly readout?: (text: string) => void;
  /** Device pixels per CSS pixel, read each frame so a moved window follows. */
  readonly scale?: () => number;
  /** What calls the loop. */
  readonly scheduler?: Scheduler;
  /** What times it. `performance.now` by default. */
  readonly clock?: () => number;
}

/** A running picture. */
export interface TelemetryDriver {
  /** Stops the loop. */
  stop: () => void;
  /** What the channel has been doing. Read by the panel and by the e2e suite. */
  readonly stats: TelemetryStats;
  /** The frame currently on the canvas. */
  readonly view: TelemetryFrameView;
}

/**
 * Starts drawing the sink's frames.
 *
 * The returned {@link TelemetryDriver.stop} must be called when the panel goes,
 * or an interface with a closed telemetry window would go on decoding 32 kB
 * thirty times a second for a canvas nobody can see.
 */
export function driveTelemetry(options: TelemetryDriverOptions): TelemetryDriver {
  const { sink, painter } = options;
  const readout = options.readout ?? (() => {});
  const scale = options.scale ?? (() => 1);
  const schedule = options.scheduler ?? animationFrames;
  const clock = options.clock ?? (() => performance.now());

  const view = new TelemetryFrameView();
  const stats = new TelemetryStats();
  let drawn: Uint8Array | null = null;
  let dimmed = false;
  // `null` until the first line is written, so the panel says what it is doing
  // as soon as it knows rather than a quarter of a second later.
  let saidAt: number | null = null;

  const say = (now: number, force: boolean): void => {
    if (!force && saidAt !== null && now - saidAt < READOUT_EVERY_MS) {
      return;
    }
    saidAt = now;
    readout(summaryText(stats.summary(now)));
  };

  const stop = schedule((now) => {
    const payload = sink.latest;

    if (payload === null) {
      // The connection went, and the sink was cleared with it: a picture of the
      // rig from a daemon that has stopped is exactly as stale as a fader value
      // from one, so it is taken off the screen rather than dimmed.
      if (view.hasFrame) {
        view.clear();
        stats.reset();
        drawn = null;
        dimmed = false;
        painter?.blank();
        say(now, true);
      }
      say(now, false);
      return;
    }

    if (payload !== drawn) {
      drawn = payload;
      // The whole cost of a frame — reading the bytes and drawing them — is
      // measured as one number, because that is what has to fit in the budget.
      const started = clock();
      const fault = view.read(payload);
      if (fault === null) {
        painter?.paint(view, scale());
        stats.painted(clock() - started);
        stats.accept(view.sequence, view.count, now);
        dimmed = false;
        say(now, false);
        return;
      }
      stats.dropped(fault);
      if (stats.faults(fault.kind) === 1) {
        // Once per kind: a daemon of another build sends thirty of these a
        // second, and a log that repeated them would bury everything else.
        log.warn("a telemetry frame was dropped", { fault: fault.kind, detail: faultText(fault) });
      }
      say(now, true);
      return;
    }

    // Nothing new. The one thing left to do is notice that what is on the canvas
    // is no longer a picture of now.
    if (!dimmed && stats.frames > 0 && now - stats.lastAt > STALE_AFTER_MS) {
      dimmed = true;
      painter?.dim();
      say(now, true);
      return;
    }
    say(now, false);
  });

  return { stop, stats, view };
}
