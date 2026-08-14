/**
 * The canvas coordinate space, and the arithmetic of moving a window in it.
 *
 * Pure functions over numbers: no DOM, no document, no store. Everything here
 * is what a drag *would* produce, so the component that has the pointer can be
 * about pointers and this can be tested by arithmetic.
 *
 * # Canvas units are not pixels
 *
 * `ARCHITECTURE_SPEC.md` §4.1 puts a window's `x`, `y`, `w` and `h` in the
 * session, which every attached client shares — and clients do not share a
 * screen. So the numbers in the session are **canvas units**: a fixed
 * {@link CANVAS_WIDTH} × {@link CANVAS_HEIGHT} grid that each client stretches
 * over whatever box its own canvas element turned out to be. A layout stored on
 * a 4K desk opens sensibly on a laptop, and the monitor a window is on stays
 * client-local, which is §4.2's first line.
 *
 * The daemon opens a window at 640 × 480 of these (`prism_core::session`'s
 * `DEFAULT_WINDOW`), which is why the grid is 1920 × 1080: a fresh window is a
 * third of the width and a little under half the height, on every screen.
 */

/** A rectangle in canvas units. */
export interface Rect {
  /** Left edge. */
  readonly x: number;
  /** Top edge. */
  readonly y: number;
  /** Width. */
  readonly w: number;
  /** Height. */
  readonly h: number;
}

/** A point in canvas units. */
export interface Point {
  /** Distance from the left edge. */
  readonly x: number;
  /** Distance from the top edge. */
  readonly y: number;
}

/** The width of the canvas coordinate space. */
export const CANVAS_WIDTH = 1920;

/** The height of the canvas coordinate space. */
export const CANVAS_HEIGHT = 1080;

/**
 * The smallest a window may be made, in canvas units.
 *
 * Small enough to tuck four of them into a corner, large enough that the title
 * bar and its close button are still there to grab. A window that can be
 * resized to nothing is a window an operator cannot get back.
 */
export const MIN_WINDOW_WIDTH = 200;

/** The smallest height a window may be made. See {@link MIN_WINDOW_WIDTH}. */
export const MIN_WINDOW_HEIGHT = 140;

/** Keeps a number within a range, whatever order the ends are given in. */
function clamp(value: number, low: number, high: number): number {
  return Math.min(Math.max(value, low), Math.max(low, high));
}

/**
 * Moves a window by a displacement, keeping it on the canvas.
 *
 * The size is untouched: dragging a window off the right edge stops it at the
 * edge rather than squashing it. A window wider than the canvas — which a
 * hand-edited show or a much wider screen's layout can produce — is pinned at
 * the left rather than pushed off to the right, because `clamp` resolves an
 * inverted range in favour of its low end.
 */
export function movedBy(origin: Rect, dx: number, dy: number): Rect {
  return {
    x: clamp(origin.x + dx, 0, CANVAS_WIDTH - origin.w),
    y: clamp(origin.y + dy, 0, CANVAS_HEIGHT - origin.h),
    w: origin.w,
    h: origin.h,
  };
}

/**
 * Resizes a window by its bottom-right corner.
 *
 * The top-left corner does not move, so the minimum size is a floor on the
 * width and the height and never a jump. The maximum is the canvas edge, which
 * is what keeps a window that has been dragged to the right from being resized
 * out of the world.
 */
export function resizedBy(origin: Rect, dx: number, dy: number): Rect {
  return {
    x: origin.x,
    y: origin.y,
    w: clamp(origin.w + dx, MIN_WINDOW_WIDTH, CANVAS_WIDTH - origin.x),
    h: clamp(origin.h + dy, MIN_WINDOW_HEIGHT, CANVAS_HEIGHT - origin.y),
  };
}

/**
 * Whole canvas units.
 *
 * What goes on the wire, so that a pointer moved by half a pixel on a scaled
 * screen does not produce a command. It is also what makes {@link sameRect}
 * worth asking: two rounded rectangles that are equal describe a drag that has
 * not yet gone anywhere.
 */
export function rounded(rect: Rect): Rect {
  return {
    x: Math.round(rect.x),
    y: Math.round(rect.y),
    w: Math.round(rect.w),
    h: Math.round(rect.h),
  };
}

/** Whether two rectangles are the same rectangle. */
export function sameRect(a: Rect, b: Rect): boolean {
  return a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
}

/**
 * A rectangle as the percentages a stylesheet wants.
 *
 * Percentages rather than pixels so the browser does the scaling: the canvas
 * element is the containing block, a resize moves every window without the
 * interface being told about it, and — the point — **no command is sent when a
 * screen changes size**, because a screen's size is client-local (§4.2).
 */
export function asStyle(rect: Rect): {
  readonly left: string;
  readonly top: string;
  readonly width: string;
  readonly height: string;
} {
  return {
    left: `${percent(rect.x, CANVAS_WIDTH)}%`,
    top: `${percent(rect.y, CANVAS_HEIGHT)}%`,
    width: `${percent(rect.w, CANVAS_WIDTH)}%`,
    height: `${percent(rect.h, CANVAS_HEIGHT)}%`,
  };
}

/** One coordinate as a percentage of the axis it is on, to four decimals. */
function percent(value: number, of: number): number {
  return Math.round((value / of) * 1_000_000) / 10_000;
}

/** The box a canvas element occupies on the screen, in CSS pixels. */
export interface Box {
  /** Its width in pixels. */
  readonly width: number;
  /** Its height in pixels. */
  readonly height: number;
}

/**
 * A displacement in screen pixels as one in canvas units.
 *
 * The two axes scale independently, because the canvas fills the box it is
 * given rather than keeping an aspect ratio — a window is a share of the
 * screen, not a shape.
 *
 * A box with no size answers *one unit per pixel*. That is not a fallback for
 * tidiness: an element that has not been laid out yet reports zeros, and
 * dividing by one of those would put `Infinity` into a rectangle and, from
 * there, a `NaN` into a command the daemon would refuse to decode.
 */
export function toCanvasUnits(dx: number, dy: number, box: Box): Point {
  return {
    x: box.width > 0 ? (dx * CANVAS_WIDTH) / box.width : dx,
    y: box.height > 0 ? (dy * CANVAS_HEIGHT) / box.height : dy,
  };
}
