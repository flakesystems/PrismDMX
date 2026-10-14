/**
 * A canvas that is not there: what the level painter drew, as data.
 *
 * `jsdom` has no rasteriser, so `getContext("2d")` answers `null` and a test
 * that painted on a real canvas would be a test that skipped itself. The seam is
 * `LevelSurface` — five operations and no canvas types — and this is the other
 * side of it, the same arrangement `FakeNetwork` is for sockets.
 *
 * Recording rather than counting: the pixels are kept, so a test can ask where
 * channel 5 of universe 3 ended up rather than that *something* was drawn. That
 * is the difference between a test of the raster and a test that a function was
 * called.
 *
 * Nothing here is shipped, and it is excluded from coverage for the reason
 * `prism-core`'s `testkit` is a module of its own: scenery that measured itself
 * would flatter every figure in `PROGRESS.md`.
 */

import type { LevelSurface } from "../telemetry/painter";

/** One rectangle that was painted. */
export interface RecordedFill {
  readonly colour: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** One piece of text that was drawn. */
export interface RecordedLabel {
  readonly text: string;
  readonly x: number;
  readonly y: number;
  readonly colour: string;
  readonly size: number;
}

/** One block of pixels that was blitted, with a copy of them. */
export interface RecordedBlit {
  readonly pixels: Uint8ClampedArray;
  readonly columns: number;
  readonly rows: number;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** A surface that keeps everything drawn on it. */
export class RecordingSurface implements LevelSurface {
  /** Not `readonly`, so a test can shrink the panel the way a drag does. */
  width: number;
  height: number;
  /** Every `clear`, in order. */
  readonly clears: string[] = [];
  /** Every `fill`, in order. */
  readonly fills: RecordedFill[] = [];
  /** Every `label`, in order. */
  readonly labels: RecordedLabel[] = [];
  /** Every `blit`, in order, each with the pixels as they were. */
  readonly blits: RecordedBlit[] = [];

  constructor(width = 1024, height = 640) {
    this.width = width;
    this.height = height;
  }

  /**
   * As a real one behaves: what was drawn before is gone.
   *
   * Which is what lets a test say *the chrome was drawn once* — the labels
   * standing in this list are the ones still on the surface, not the ones ever
   * asked for.
   */
  clear(colour: string): void {
    this.clears.push(colour);
    this.fills.length = 0;
    this.labels.length = 0;
  }

  fill(colour: string, x: number, y: number, width: number, height: number): void {
    this.fills.push({ colour, x, y, width, height });
  }

  label(text: string, x: number, y: number, colour: string, size: number): void {
    this.labels.push({ text, x, y, colour, size });
  }

  blit(
    pixels: Uint8ClampedArray,
    columns: number,
    rows: number,
    x: number,
    y: number,
    width: number,
    height: number,
  ): void {
    // A copy, because the painter reuses its buffer: a test holding the live one
    // would be looking at the next frame.
    this.blits.push({ pixels: pixels.slice(), columns, rows, x, y, width, height });
  }

  /** The most recent blit, or a named failure. */
  get lastBlit(): RecordedBlit {
    const blit = this.blits.at(-1);
    if (blit === undefined) {
      throw new Error("nothing has been blitted");
    }
    return blit;
  }

  /** The RGBA of one pixel of the most recent blit. */
  pixel(column: number, row: number): [number, number, number, number] {
    const blit = this.lastBlit;
    const at = (row * blit.columns + column) * 4;
    return [
      blit.pixels[at] ?? 0,
      blit.pixels[at + 1] ?? 0,
      blit.pixels[at + 2] ?? 0,
      blit.pixels[at + 3] ?? 0,
    ];
  }

  /** Forgets everything drawn so far. */
  forget(): void {
    this.clears.length = 0;
    this.fills.length = 0;
    this.labels.length = 0;
    this.blits.length = 0;
  }
}
