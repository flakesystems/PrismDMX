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
 * Whether two rectangles share any area at all.
 *
 * Edges that merely touch do not overlap, which is what lets two windows be put
 * side by side with no gap — the arrangement {@link slidTo} exists to make easy.
 */
export function overlaps(one: Rect, two: Rect): boolean {
  return (
    one.x < two.x + two.w && two.x < one.x + one.w && one.y < two.y + two.h && two.y < one.y + one.h
  );
}

/**
 * The far end of a move, stopped where it meets a neighbour — S43, punch-list
 * B10 second half.
 *
 * # What was wrong with refusing instead
 *
 * The daemon has refused an overlapping placement since B10
 * (`prism_core::layout::may_place`), and refusing is right: it is the daemon's
 * rule and the daemon keeps it. What an operator saw, though, was the window
 * following the pointer *over* its neighbour and then jumping back when the
 * refusal arrived — so putting two windows edge to edge meant aiming at a
 * position you could not see yourself reach, and being told afterwards.
 *
 * The canvas edges never did that, because {@link movedBy} clamps to them: a
 * window pushed at the right-hand wall slides along it. This is that, with the
 * neighbours as walls too.
 *
 * # This predicts, it does not decide
 *
 * §4.2's line, and the same one {@link movedBy} is on: **where a window is** is
 * the session's, and every drag still goes out as a `PlaceWindow` for the daemon
 * to accept or refuse. What is local is only the rectangle shown while the
 * button is down, and this keeps that rectangle somewhere the daemon will say
 * yes to — which is why the jump disappears rather than being hidden. If the
 * prediction is ever wrong the old behaviour is still underneath: the window
 * goes back to where the session says it is.
 *
 * # A neighbour that is already overlapped is not a wall
 *
 * `may_place`'s rule, kept in step here: *overlap may only shrink*. A window
 * that starts underneath another one — which a hand-edited show or a layout
 * saved before B10 can produce — must still be draggable out from under it, so
 * a neighbour that is overlapped at the start of the drag blocks nothing for the
 * length of that drag.
 *
 * # One axis at a time, and that is what makes it slide
 *
 * The move is resolved along x with the window at its original y, then along y
 * from where x finished. Resolving both at once would stop the window dead at
 * the first corner it met; doing it in turn lets it run along the edge it is
 * pressed against, which is the gesture an operator is actually making when
 * they push one window up against another.
 */
export function slidTo(origin: Rect, wanted: Rect, neighbours: readonly Rect[]): Rect {
  const walls = neighbours.filter((rect) => !overlaps(origin, rect));
  const x = stopped(origin.x, wanted.x, origin.w, spanOf(origin, "y"), walls, "x");
  const at = { ...origin, x };
  const y = stopped(origin.y, wanted.y, origin.h, spanOf(at, "x"), walls, "y");
  return { x, y, w: origin.w, h: origin.h };
}

/**
 * The far corner of a resize, stopped where it meets a neighbour.
 *
 * The same rule as {@link slidTo} and for the same reason: a corner dragged into
 * a neighbour used to grow over it and snap back. The top-left corner does not
 * move, so what is limited is the width and the height.
 */
export function grownTo(origin: Rect, wanted: Rect, neighbours: readonly Rect[]): Rect {
  const walls = neighbours.filter((rect) => !overlaps(origin, rect));
  // No floor is applied here and none is needed: `resizedBy` has already put one
  // on, and a wall that is not overlapped at the start is by definition at least
  // `origin.w` away — so a window can always stay the size it was.
  const w = limit(origin.x, wanted.w, spanOf(origin, "y"), walls, "x");
  const h = limit(origin.y, wanted.h, spanOf({ ...origin, w }, "x"), walls, "y");
  return { x: origin.x, y: origin.y, w, h };
}

/** The interval a rectangle covers on one axis, as `[from, to)`. */
function spanOf(rect: Rect, axis: "x" | "y"): readonly [number, number] {
  return axis === "x" ? [rect.x, rect.x + rect.w] : [rect.y, rect.y + rect.h];
}

/** Whether two half-open intervals share any length. */
function crosses(one: readonly [number, number], two: readonly [number, number]): boolean {
  return one[0] < two[1] && two[0] < one[1];
}

/**
 * Where a move along one axis has to stop.
 *
 * `from` is where the near edge starts, `to` where it wants to go, `size` how
 * far the far edge is beyond it, and `across` the interval the window covers on
 * the *other* axis — a neighbour that does not cross that interval is beside the
 * window rather than in front of it and cannot stop anything.
 */
function stopped(
  from: number,
  to: number,
  size: number,
  across: readonly [number, number],
  walls: readonly Rect[],
  axis: "x" | "y",
): number {
  if (to === from) {
    return from;
  }
  const other = axis === "x" ? "y" : "x";
  let stop = to;
  for (const wall of walls) {
    if (!crosses(across, spanOf(wall, other))) {
      continue;
    }
    const [near, far] = spanOf(wall, axis);
    if (to > from) {
      // Moving towards the far end: only a wall already ahead of the window can
      // stop it, and it stops it at its near edge.
      if (near >= from + size) {
        stop = Math.min(stop, near - size);
      }
    } else if (far <= from) {
      stop = Math.max(stop, far);
    }
  }
  return stop;
}

/**
 * How large one side may grow before it meets a neighbour.
 *
 * {@link stopped}'s twin for a resize: the near edge is pinned, so what is
 * limited is the distance to the far one.
 */
function limit(
  near: number,
  size: number,
  across: readonly [number, number],
  walls: readonly Rect[],
  axis: "x" | "y",
): number {
  const other = axis === "x" ? "y" : "x";
  let most = size;
  for (const wall of walls) {
    if (!crosses(across, spanOf(wall, other))) {
      continue;
    }
    const start = spanOf(wall, axis)[0];
    if (start >= near) {
      most = Math.min(most, start - near);
    }
  }
  return most;
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
