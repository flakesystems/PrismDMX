/**
 * A 3D view that is not there: what the scene drew, as data — `RecordingSurface`
 * for `viewer/surface.ts`'s interface. `jsdom` has no rasteriser, so this is
 * how a test asks where a beam went rather than whether one was drawn.
 *
 * Scenery, not a fixture — `src/testing/` is excluded from coverage.
 */

import type { Blend, ViewSurface } from "../viewer/surface";

/** One filled polygon. */
export interface RecordedPolygon {
  readonly points: readonly number[];
  readonly fill: string;
  readonly alpha: number;
  readonly blend: Blend;
  readonly outline: string | undefined;
}

/** One beam. */
export interface RecordedBeam {
  readonly points: readonly number[];
  readonly from: readonly [number, number];
  readonly to: readonly [number, number];
  readonly rgb: string;
  readonly alphaFrom: number;
  readonly alphaTo: number;
}

/** One label. */
export interface RecordedText {
  readonly text: string;
  readonly x: number;
  readonly y: number;
  readonly colour: string;
}

/** A surface that keeps everything drawn on it, cleared on every `clear`. */
export class RecordingView implements ViewSurface {
  width: number;
  height: number;
  clears = 0;
  polygons: RecordedPolygon[] = [];
  beams: RecordedBeam[] = [];
  lines = 0;
  labels: RecordedText[] = [];

  constructor(width = 800, height = 500) {
    this.width = width;
    this.height = height;
  }

  clear(): void {
    this.clears += 1;
    this.polygons = [];
    this.beams = [];
    this.lines = 0;
    this.labels = [];
  }

  polygon(
    points: ArrayLike<number>,
    count: number,
    fill: string,
    alpha: number,
    blend: Blend,
    outline?: string,
  ): void {
    this.polygons.push({ points: Array.from(points).slice(0, count * 2), fill, alpha, blend, outline });
  }

  beam(
    points: ArrayLike<number>,
    count: number,
    fromX: number,
    fromY: number,
    toX: number,
    toY: number,
    rgb: string,
    alphaFrom: number,
    alphaTo: number,
  ): void {
    this.beams.push({
      points: Array.from(points).slice(0, count * 2),
      from: [fromX, fromY],
      to: [toX, toY],
      rgb,
      alphaFrom,
      alphaTo,
    });
  }

  line(): void {
    this.lines += 1;
  }

  label(text: string, x: number, y: number, colour: string): void {
    this.labels.push({ text, x, y, colour });
  }
}
