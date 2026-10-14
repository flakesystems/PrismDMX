/**
 * What the telemetry channel is doing, in numbers a person can read.
 *
 * Two things live here. One is the diagnostic a lighting desk owes its operator
 * — *is the picture live, and how much of it am I losing* — and the other is
 * **the measurement S24's frame budget is asserted against**: the paint
 * durations, kept as a rolling window so `e2e/telemetry.spec.ts` can read a p99
 * off a real browser drawing 64 real universes.
 *
 * None of it is React state. The summary is written into a DOM node by hand
 * (see `./driver.ts`), which is the whole arrangement §7's last paragraph asks
 * for: telemetry reaches the screen without going through a render.
 *
 * # Frames lost are counted, not inferred
 *
 * The daemon coalesces and then drops (§8), and every frame carries a sequence
 * number for exactly this reason: a client that sees a gap has lost frames.
 * That is allowed, it is worth showing, and it is *not* an error — which is why
 * losing telemetry appears here as a number and nowhere as a fault.
 */

import type { TelemetryFault } from "./frame";

/** How many paint durations are kept. Four seconds at 30 Hz. */
export const PAINT_WINDOW = 120;

/** How long a picture may stand before it is called stale. */
export const STALE_AFTER_MS = 600;

/** A summary of the channel, as the readout shows it. */
export interface TelemetrySummary {
  /** Frames read since the connection was made. */
  readonly frames: number;
  /** Frames the daemon sent that this client never saw, by sequence gap. */
  readonly lost: number;
  /** Frames that could not be read at all, by fault. */
  readonly dropped: number;
  /** Universes in the last frame. */
  readonly universes: number;
  /** Frames a second, over the last window. */
  readonly hz: number;
  /** Median paint, milliseconds. */
  readonly paintMedian: number;
  /** 99th-percentile paint, milliseconds. */
  readonly paintP99: number;
  /** The worst paint seen since the connection was made. */
  readonly paintWorst: number;
  /** Whether the picture is older than {@link STALE_AFTER_MS}. */
  readonly stale: boolean;
}

/** Counts what the channel does and how long the picture takes to draw. */
export class TelemetryStats {
  #frames = 0;
  #lost = 0;
  #dropped = 0;
  #universes = 0;
  #lastSequence: bigint | null = null;
  /** `null` rather than 0: a clock whose zero is a real instant would make the
   * first frame's arrival look like "never", and the rate would be worked out
   * over one interval too few. */
  #firstAt: number | null = null;
  #lastAt: number | null = null;
  #paints = new Float64Array(PAINT_WINDOW);
  #paintCount = 0;
  #paintWorst = 0;
  #faults = new Map<TelemetryFault["kind"], number>();

  /**
   * A frame was read.
   *
   * `sequence` is the frame's own number, so a gap is a fact rather than a
   * guess. It is a `u64` on the wire and a `bigint` here; the difference is
   * taken in `bigint` and narrowed once, which is the only place it could
   * overflow if it were not.
   */
  accept(sequence: bigint, universes: number, at: number): void {
    const previous = this.#lastSequence;
    if (previous !== null && sequence > previous + 1n) {
      this.#lost += Number(sequence - previous - 1n);
    }
    this.#lastSequence = sequence;
    this.#frames += 1;
    this.#universes = universes;
    this.#firstAt ??= at;
    this.#lastAt = at;
  }

  /** A frame could not be read, and was dropped (§7). */
  dropped(fault: TelemetryFault): void {
    this.#dropped += 1;
    this.#faults.set(fault.kind, (this.#faults.get(fault.kind) ?? 0) + 1);
  }

  /** How many frames were dropped with this fault. */
  faults(kind: TelemetryFault["kind"]): number {
    return this.#faults.get(kind) ?? 0;
  }

  /** One paint took this many milliseconds. */
  painted(milliseconds: number): void {
    this.#paints[this.#paintCount % PAINT_WINDOW] = milliseconds;
    this.#paintCount += 1;
    if (milliseconds > this.#paintWorst) {
      this.#paintWorst = milliseconds;
    }
  }

  /** When the last frame was read, as the caller's clock had it, or 0. */
  get lastAt(): number {
    return this.#lastAt ?? 0;
  }

  /** Frames read. */
  get frames(): number {
    return this.#frames;
  }

  /** Forgets everything — the connection went, so the numbers are of nothing. */
  reset(): void {
    this.#frames = 0;
    this.#lost = 0;
    this.#dropped = 0;
    this.#universes = 0;
    this.#lastSequence = null;
    this.#firstAt = null;
    this.#lastAt = null;
    this.#paintCount = 0;
    this.#paintWorst = 0;
    this.#faults.clear();
  }

  /** Everything, as of `now`. */
  summary(now: number): TelemetrySummary {
    const held = Math.min(this.#paintCount, PAINT_WINDOW);
    const sorted = Array.from(this.#paints.subarray(0, held)).sort((a, b) => a - b);
    const seconds = ((this.#lastAt ?? 0) - (this.#firstAt ?? 0)) / 1000;
    return {
      frames: this.#frames,
      lost: this.#lost,
      dropped: this.#dropped,
      universes: this.#universes,
      // Frames *between* the first and the last, so one frame is not 1 Hz.
      hz: seconds > 0 ? (this.#frames - 1) / seconds : 0,
      paintMedian: percentile(sorted, 0.5),
      paintP99: percentile(sorted, 0.99),
      paintWorst: this.#paintWorst,
      stale: this.#lastAt === null || now - this.#lastAt > STALE_AFTER_MS,
    };
  }
}

/** The value at `fraction` through a sorted window, or 0 if it is empty. */
function percentile(sorted: readonly number[], fraction: number): number {
  if (sorted.length === 0) {
    return 0;
  }
  // Nearest rank. With 120 samples the 99th percentile is the 119th of them,
  // which is the number an 8 ms budget should be judged on: the worst frame in
  // four seconds, not the average of four seconds.
  const rank = Math.min(sorted.length - 1, Math.ceil(fraction * sorted.length) - 1);
  return sorted[Math.max(0, rank)] ?? 0;
}

/** The readout line, which is what the end-to-end suite reads the budget from. */
export function summaryText(summary: TelemetrySummary): string {
  if (summary.frames === 0) {
    return summary.dropped > 0
      ? `no readable telemetry — ${summary.dropped} frame${summary.dropped === 1 ? "" : "s"} dropped`
      : "waiting for telemetry";
  }
  const parts = [
    `${summary.universes} universes`,
    `${summary.hz.toFixed(1)} Hz`,
    `paint ${summary.paintMedian.toFixed(2)} ms (p99 ${summary.paintP99.toFixed(2)} ms)`,
    `${summary.frames} frames`,
  ];
  if (summary.lost > 0) {
    parts.push(`${summary.lost} lost`);
  }
  if (summary.dropped > 0) {
    parts.push(`${summary.dropped} dropped`);
  }
  if (summary.stale) {
    // Said first as well as last: the one thing that must never be ambiguous is
    // whether the picture on the canvas is of now.
    return `not live — ${parts.join(" · ")}`;
  }
  return parts.join(" · ");
}
