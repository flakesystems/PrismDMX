/**
 * The arithmetic of a canvas, checked against numbers worked out by hand.
 *
 * Every expectation here is written out rather than computed: a test that
 * called the function to work out what the function should answer would pass
 * with the whole coordinate space misunderstood.
 */

import { describe, expect, it } from "vitest";

import {
  CANVAS_HEIGHT,
  CANVAS_WIDTH,
  MIN_WINDOW_HEIGHT,
  MIN_WINDOW_WIDTH,
  asStyle,
  movedBy,
  resizedBy,
  rounded,
  sameRect,
  toCanvasUnits,
} from "./geometry";

/** The rectangle `prism_core::session::DEFAULT_WINDOW` opens a window at. */
const DEFAULT = { x: 0, y: 0, w: 640, h: 480 };

describe("the canvas coordinate space", () => {
  it("is the one the daemon's default window is a third of", () => {
    // Not a free choice: the daemon opens a window at 640 x 480 canvas units,
    // and a grid on which that is a third of the width is the reason 1920 was
    // picked. If either number moves, this is the sentence that has to be
    // reconsidered.
    expect(CANVAS_WIDTH).toBe(1920);
    expect(CANVAS_HEIGHT).toBe(1080);
    expect(DEFAULT.w * 3).toBe(CANVAS_WIDTH);
  });
});

describe("moving a window", () => {
  it("carries the size with it", () => {
    expect(movedBy(DEFAULT, 240, 120)).toEqual({ x: 240, y: 120, w: 640, h: 480 });
  });

  it("stops at the edges rather than leaving the canvas", () => {
    // Right and bottom: the far edge of the window is what stops.
    expect(movedBy(DEFAULT, 10_000, 10_000)).toEqual({
      x: CANVAS_WIDTH - 640,
      y: CANVAS_HEIGHT - 480,
      w: 640,
      h: 480,
    });
    // Left and top.
    expect(movedBy({ x: 300, y: 300, w: 640, h: 480 }, -10_000, -10_000)).toEqual({
      x: 0,
      y: 0,
      w: 640,
      h: 480,
    });
  });

  it("pins a window wider than the canvas at the left rather than off the right", () => {
    // A layout stored on a much wider screen, or a hand-edited show. The range
    // is inverted here — 0 to a negative number — and the low end wins, which
    // is the difference between a window an operator can reach and one that
    // is not on the screen at all.
    const huge = { x: 0, y: 0, w: CANVAS_WIDTH + 500, h: CANVAS_HEIGHT + 500 };
    expect(movedBy(huge, 400, 400)).toEqual(huge);
  });
});

describe("resizing a window", () => {
  it("moves the bottom-right corner and nothing else", () => {
    expect(resizedBy(DEFAULT, 640, 240)).toEqual({ x: 0, y: 0, w: 1280, h: 720 });
  });

  it("will not go below the size that keeps a title bar grabbable", () => {
    expect(resizedBy(DEFAULT, -10_000, -10_000)).toEqual({
      x: 0,
      y: 0,
      w: MIN_WINDOW_WIDTH,
      h: MIN_WINDOW_HEIGHT,
    });
  });

  it("will not grow past the edge it started from", () => {
    const right = { x: 1600, y: 900, w: 200, h: 140 };
    expect(resizedBy(right, 10_000, 10_000)).toEqual({
      x: 1600,
      y: 900,
      w: CANVAS_WIDTH - 1600,
      h: CANVAS_HEIGHT - 900,
    });
  });
});

describe("what goes on the wire", () => {
  it("is whole canvas units", () => {
    expect(rounded({ x: 12.4, y: 12.5, w: 639.6, h: 480.49 })).toEqual({
      x: 12,
      y: 13,
      w: 640,
      h: 480,
    });
  });

  it("can be compared, so a drag that has not moved says nothing", () => {
    expect(sameRect(DEFAULT, { ...DEFAULT })).toBe(true);
    expect(sameRect(DEFAULT, { ...DEFAULT, x: 1 })).toBe(false);
    expect(sameRect(DEFAULT, { ...DEFAULT, h: 1 })).toBe(false);
  });
});

describe("a rectangle as style", () => {
  it("is percentages of the canvas, so the browser does the scaling", () => {
    expect(asStyle({ x: 960, y: 540, w: 480, h: 270 })).toEqual({
      left: "50%",
      top: "50%",
      width: "25%",
      height: "25%",
    });
  });

  it("keeps four decimals, which is a tenth of a pixel on a 4K screen", () => {
    expect(asStyle({ x: 1, y: 1, w: 640, h: 480 }).left).toBe("0.0521%");
  });
});

describe("pixels into canvas units", () => {
  it("scales each axis by its own share of the box", () => {
    // A canvas 960 wide is half the coordinate space, so a pixel is two units.
    expect(toCanvasUnits(10, 10, { width: 960, height: 1080 })).toEqual({ x: 20, y: 10 });
  });

  it("treats a box with no size as one unit per pixel", () => {
    // An element that has not been laid out reports zeros. Dividing by one of
    // those would put `Infinity` in a rectangle and a `NaN` in a command the
    // daemon refuses to decode — so this branch is the difference between a
    // harmless first frame and a rejected command.
    expect(toCanvasUnits(7, 9, { width: 0, height: 0 })).toEqual({ x: 7, y: 9 });
    expect(Number.isFinite(toCanvasUnits(7, 9, { width: 0, height: 0 }).x)).toBe(true);
  });
});
