/**
 * The level view, asserted per pixel.
 *
 * The frames are the recorded ones — bytes a running `prismd` sent — so what is
 * being checked is *where a level ends up*, not whether the painter agrees with
 * a frame this test made up. Channel 5 of universe 3 has one place to be, and a
 * stride error, a transposed grid or an off-by-one in the gutter each put it
 * somewhere else.
 *
 * `jsdom` has no rasteriser. `RecordingSurface` is the seam, and it keeps the
 * pixels rather than counting the calls.
 */

import { describe, expect, it } from "vitest";

import { RecordingSurface } from "../testing/recording-surface";
import { narrowFrame, wideFrame } from "../testing/telemetry-frames";
import { CHANNELS_PER_UNIVERSE, TelemetryFrameView } from "./frame";
import { LevelPainter, levelGeometry } from "./painter";

/** A view holding one of the recorded frames. */
function frameOf(bytes: Uint8Array): TelemetryFrameView {
  const view = new TelemetryFrameView();
  const fault = view.read(bytes);
  if (fault !== null) {
    throw new Error(`the recorded frame did not read: ${fault.kind}`);
  }
  return view;
}

describe("the geometry", () => {
  it("puts the gutter, the grid and the meters side by side inside the surface", () => {
    const geometry = levelGeometry(1024, 640, 64);
    expect(geometry).not.toBeNull();
    if (geometry === null) return;

    expect(geometry.gutter.x).toBeGreaterThan(0);
    expect(geometry.grid.x).toBe(geometry.gutter.x + geometry.gutter.width);
    expect(geometry.meter.x).toBe(geometry.grid.x + geometry.grid.width);
    expect(geometry.meter.x + geometry.meter.width).toBeLessThanOrEqual(1024);
    expect(geometry.grid.y + geometry.grid.height).toBeLessThanOrEqual(640);
    expect(geometry.rows).toBe(64);
    expect(geometry.rowHeight * geometry.rows).toBe(geometry.grid.height);
  });

  it("scales everything but the grid with the pixel ratio", () => {
    const one = levelGeometry(1024, 640, 8, 1);
    const two = levelGeometry(2048, 1280, 8, 2);
    expect(one).not.toBeNull();
    expect(two).not.toBeNull();
    if (one === null || two === null) return;
    // The same picture on a screen with twice the pixels: the gutter is twice
    // as many pixels wide and therefore the same size on the desk.
    expect(two.gutter.width).toBe(one.gutter.width * 2);
    expect(two.meter.width).toBe(one.meter.width * 2);
    expect(two.rowHeight).toBeGreaterThan(one.rowHeight);
  });

  it("answers with nothing rather than with a division by zero", () => {
    expect(levelGeometry(1024, 640, 0)).toBeNull();
    expect(levelGeometry(0, 640, 4)).toBeNull();
    expect(levelGeometry(1024, 0, 4)).toBeNull();
    expect(levelGeometry(1024, -10, 4)).toBeNull();
    // Too narrow for a grid, and too short for a row.
    expect(levelGeometry(60, 640, 4)).toBeNull();
    expect(levelGeometry(1024, 18, 4)).toBeNull();
  });

  it("keeps a row readable when there are more universes than pixels", () => {
    const geometry = levelGeometry(1024, 200, 64);
    expect(geometry).not.toBeNull();
    if (geometry === null) return;
    expect(geometry.rowHeight).toBeGreaterThanOrEqual(2);
    // Rather than 64 rows of nothing: as many as fit, and the readout says how
    // many universes there are.
    expect(geometry.rows).toBeLessThanOrEqual(64);
    expect(geometry.grid.height).toBeLessThanOrEqual(200);
  });
});

describe("a painted frame", () => {
  it("is one pixel per channel, one row per universe", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    const view = frameOf(wideFrame);

    const result = painter.paint(view);

    expect(result.rows).toBe(64);
    const blit = surface.lastBlit;
    expect(blit.columns).toBe(CHANNELS_PER_UNIVERSE);
    expect(blit.rows).toBe(64);
    expect(blit.pixels.length).toBe(CHANNELS_PER_UNIVERSE * 64 * 4);
    // One call for the whole picture. 32 768 `fillRect`s would draw the same
    // thing and is the reason this module exists.
    expect(surface.blits).toHaveLength(1);
  });

  it("puts each channel where the frame says it is", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    const view = frameOf(wideFrame);
    painter.paint(view);

    // Every pixel, against the frame it was drawn from. Not a sample: a stride
    // error is a thing that is right for the first universe and wrong for the
    // sixty-fourth.
    let checked = 0;
    for (let row = 0; row < view.count; row += 1) {
      for (let channel = 0; channel < CHANNELS_PER_UNIVERSE; channel += 1) {
        const level = view.levelAt(row, channel);
        const [red, green, blue, alpha] = surface.pixel(channel, row);
        expect(alpha).toBe(255);
        if (level === 0) {
          // The colour of nothing is the colour of the grid, and it is dark.
          expect(red + green + blue).toBeLessThan(120);
        } else {
          expect(red + green + blue).toBeGreaterThan(120);
        }
        checked += 1;
      }
    }
    expect(checked).toBe(64 * CHANNELS_PER_UNIVERSE);
  });

  it("draws a brighter colour for a higher level", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    // The narrow rig has three distinct levels in one frame: a dimmer at full,
    // one at 40 %, and everything else at nothing.
    const view = frameOf(narrowFrame(0));
    painter.paint(view);

    const brightness = (column: number, row: number): number => {
      const [red, green, blue] = surface.pixel(column, row);
      return red + green + blue;
    };
    const full = brightness(0, 0); // universe 1, channel 1 — at 255
    const part = brightness(0, 1); // universe 2, channel 1 — at 102
    const dark = brightness(1, 0); // universe 1, channel 2 — at 0
    expect(view.levelAt(0, 0)).toBe(255);
    expect(view.levelAt(1, 0)).toBe(102);
    expect(view.levelAt(0, 1)).toBe(0);
    expect(full).toBeGreaterThan(part);
    expect(part).toBeGreaterThan(dark);
  });

  it("draws a peak meter per universe, and marks the ones at full", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    const view = frameOf(narrowFrame(0));
    const geometry = levelGeometry(surface.width, surface.height, view.count);
    painter.paint(view);
    if (geometry === null) throw new Error("the surface is too small");

    const meters = surface.fills.filter((fill) => fill.x >= geometry.meter.x);
    // A trough and a bar for each of the two universes.
    expect(meters.length).toBe(4);
    const bars = meters.filter((_, index) => index % 2 === 1);
    expect(bars).toHaveLength(2);
    // Universe 1 is at 255 and universe 2 at 102, so the first bar is longer —
    // and the two are drawn in different colours, because *at full* is the one
    // thing an operator looks for.
    expect(bars[0]?.width).toBeGreaterThan(bars[1]?.width ?? 0);
    expect(bars[0]?.colour).not.toBe(bars[1]?.colour);
  });

  it("reports the loudest channel in the frame", () => {
    const painter = new LevelPainter(new RecordingSurface());
    expect(painter.paint(frameOf(narrowFrame(0))).peak).toBe(255);
  });
});

describe("what is drawn again and what is not", () => {
  it("draws the chrome once and the levels every time", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    const view = frameOf(narrowFrame(0));

    painter.paint(view);
    const labels = surface.labels.length;
    expect(labels).toBeGreaterThan(0);
    expect(surface.clears).toHaveLength(1);

    painter.paint(frameOf(narrowFrame(1)));
    painter.paint(frameOf(narrowFrame(2)));

    // Three frames, three blits — and the universe numbers were not redrawn,
    // because they did not change. At 30 Hz that is the difference between a
    // panel that costs a paint and one that costs sixty-four `fillText`s.
    expect(surface.blits).toHaveLength(3);
    expect(surface.clears).toHaveLength(1);
    expect(surface.labels).toHaveLength(labels);
  });

  it("draws the chrome again when the universes change", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    painter.paint(frameOf(narrowFrame(0)));
    expect(surface.clears).toHaveLength(1);

    // A fixture patched into a universe nothing was patched in changes the set
    // of universes the daemon publishes, and the gutter with it.
    painter.paint(frameOf(wideFrame));
    expect(surface.clears).toHaveLength(2);
    expect(surface.labels.length).toBeGreaterThan(2);
  });

  it("names the ends when there is no room for a number on every row", () => {
    const surface = new RecordingSurface(1024, 150);
    const painter = new LevelPainter(surface);
    const geometry = levelGeometry(surface.width, surface.height, 64);
    painter.paint(frameOf(wideFrame));
    if (geometry === null) throw new Error("the surface is too small");
    // Two labels in the gutter — the first universe and the last — rather than
    // 64 numbers on top of one another.
    const universeLabels = surface.labels.filter((label) => label.x === geometry.gutter.x);
    expect(universeLabels).toHaveLength(2);
    expect(universeLabels[0]?.text).toBe("1");
  });

  it("draws nothing at all when there is nowhere to draw it", () => {
    const surface = new RecordingSurface(20, 20);
    const painter = new LevelPainter(surface);
    expect(painter.paint(frameOf(narrowFrame(0)))).toEqual({ rows: 0, peak: 0 });
    expect(surface.blits).toHaveLength(0);
  });

  it("takes the picture away when the panel is dragged smaller than a row", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    painter.paint(frameOf(narrowFrame(0)));
    expect(surface.blits).toHaveLength(1);

    surface.height = 8;
    expect(painter.paint(frameOf(narrowFrame(1)))).toEqual({ rows: 0, peak: 0 });
    // Cleared rather than left with the last picture stretched across it, and
    // cleared **once** however many frames arrive afterwards.
    expect(surface.clears).toHaveLength(2);
    painter.paint(frameOf(narrowFrame(2)));
    expect(surface.clears).toHaveLength(2);
    expect(surface.blits).toHaveLength(1);
  });

  it("clears what it drew when the picture goes, and dims it when it goes stale", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    painter.paint(frameOf(narrowFrame(0)));

    painter.dim();
    const wash = surface.fills.at(-1);
    // Over the whole surface, and translucent: *these were the levels a second
    // ago* is worth more than an empty rectangle, as long as it cannot be
    // mistaken for now.
    expect(wash).toEqual({
      colour: expect.stringContaining("rgba") as unknown as string,
      x: 0,
      y: 0,
      width: surface.width,
      height: surface.height,
    });

    painter.blank();
    expect(surface.clears).toHaveLength(2);
    // And the chrome comes back on the next frame, because the surface is empty.
    painter.paint(frameOf(narrowFrame(0)));
    expect(surface.clears).toHaveLength(3);
  });

  it("draws the chrome again after the surface is resized", () => {
    const surface = new RecordingSurface();
    const painter = new LevelPainter(surface);
    painter.paint(frameOf(narrowFrame(0)));
    painter.invalidate();
    painter.paint(frameOf(narrowFrame(0)));
    expect(surface.clears).toHaveLength(2);
  });
});
