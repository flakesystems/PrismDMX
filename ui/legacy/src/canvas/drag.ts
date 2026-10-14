/**
 * Dragging a window without owning where it is.
 *
 * # The problem, stated plainly
 *
 * `ARCHITECTURE_SPEC.md` §4.1 puts a window's position and size in the
 * **session**, which lives in the daemon; §4.2 keeps hover and drag state
 * client-local; and **D3** forbids applying anything optimistically. Taken
 * together those say: while a window is being dragged, the interface may know
 * where the pointer is, but it may not decide where the window *is*.
 *
 * The temptation is to hold the position in component state during the drag and
 * "sync it up" afterwards, because that looks smoother. That is optimistic
 * application with a different name, and it fails the moment the daemon refuses
 * the command or a second client moves the same window.
 *
 * # The resolution is cadence, not ownership
 *
 * Ownership never moves: the position is the daemon's, and the canvas renders
 * `openWindows` and nothing else. What is local is only **how often** the
 * daemon is told, and **what the screen shows in between**:
 *
 * - the pointer's rectangle is shown while the button is down — that is hover
 *   and drag state, §4.2's own list, and it lives here;
 * - `PlaceWindow` goes out at most every {@link PLACE_INTERVAL_MS}, plus once
 *   when the button comes up;
 * - the moment the button comes up, **the local rectangle is dropped**. The
 *   window is wherever the session says it is. If the commands landed, that is
 *   where the pointer left it and nothing moves; if the daemon refused them,
 *   the window visibly returns to where it really is.
 *
 * The last point is the whole design in one sentence, and it is the one that is
 * asserted: a drag against a daemon that never answers leaves the window
 * exactly where it started.
 *
 * # Why not send on every pointer event
 *
 * A pointer reports at the display's rate and often faster — 120 Hz on the
 * screens this will be run on. S21 established the same thing at the other end
 * of the desk: an unpaced stream of small messages is how a surface is
 * saturated, and the fix is a floor on the gap rather than a bigger buffer.
 * Thirty a second is the rate the daemon already publishes telemetry at, and it
 * is well under `ARCHITECTURE_SPEC.md` §4.3's budget for a round trip.
 */

import type { Point, Rect } from "./geometry";
import { grownTo, movedBy, resizedBy, rounded, sameRect, slidTo } from "./geometry";

/**
 * The shortest gap between two `PlaceWindow` commands from one drag.
 *
 * Thirty a second. See the module documentation for why there is a floor at
 * all.
 */
export const PLACE_INTERVAL_MS = 33;

/** What a drag is doing to the window. */
export type DragKind = "move" | "resize";

/** How to start one. */
export interface DragOptions {
  /** Which window, so the command can name it. */
  readonly instanceId: number;
  /** Whether the pointer is on the title bar or the corner. */
  readonly kind: DragKind;
  /** Where the window was when the button went down — the daemon's rectangle. */
  readonly origin: Rect;
  /** Where the pointer was, in screen pixels. */
  readonly from: Point;
  /** Sends one `PlaceWindow`. */
  readonly place: (rect: Rect) => void;
  /** Turns a displacement in pixels into one in canvas units. */
  readonly scale: (dx: number, dy: number) => Point;
  /**
   * The other windows on the canvas, as the session holds them — S43, B10.
   *
   * The drag stops against them the way it stops against the canvas edges,
   * rather than crossing them and being pulled back when the daemon refuses.
   * They are the *daemon's* rectangles, read out of `openWindows`, which is why
   * this is a prediction of the daemon's own rule and not a second opinion —
   * see `geometry.ts`'s `slidTo`.
   *
   * Read once when the button goes down. A second operator moving a window
   * during this drag will not be taken into account, and that is the right
   * trade: the daemon still decides, and re-reading the layout on every pointer
   * event would make a window move under a hand that is already moving.
   */
  readonly neighbours: readonly Rect[];
}

/**
 * One drag in progress.
 *
 * Created on pointer-down, fed on pointer-move, ended on pointer-up. It holds
 * no DOM and no clock of its own: the caller passes `now`, which is what lets
 * the pacing be asserted rather than waited for.
 */
export class WindowDrag {
  readonly #options: DragOptions;
  /** The rectangle the pointer describes. What the screen shows, not the state. */
  #rect: Rect;
  /** The last rectangle actually sent, so an unchanged drag says nothing. */
  #sent: Rect;
  /** When it was sent. */
  #sentAt: number | null = null;
  /** Set when the pointer has moved somewhere that has not been sent yet. */
  #owed = false;

  constructor(options: DragOptions) {
    this.#options = options;
    this.#rect = rounded(options.origin);
    this.#sent = this.#rect;
  }

  /** Which window this drag is of. */
  get instanceId(): number {
    return this.#options.instanceId;
  }

  /** The rectangle the pointer is describing, in canvas units. */
  get rect(): Rect {
    return this.#rect;
  }

  /**
   * The pointer moved to `to`, at `now`.
   *
   * Answers the rectangle to show. A command goes out if the pacing allows one
   * and there is something new to say.
   */
  to(to: Point, now: number): Rect {
    const { from, kind, neighbours, origin, scale } = this.#options;
    const by = scale(to.x - from.x, to.y - from.y);
    // The walls first, then the neighbours: `movedBy` keeps the window on the
    // canvas and `slidTo` keeps it out of the windows already on it. Both are
    // clamps rather than refusals, which is what makes a window slide along
    // whatever it is pressed against instead of stopping dead.
    this.#rect = rounded(
      kind === "move"
        ? slidTo(origin, movedBy(origin, by.x, by.y), neighbours)
        : grownTo(origin, resizedBy(origin, by.x, by.y), neighbours),
    );
    if (!sameRect(this.#rect, this.#sent)) {
      this.#owed = true;
    }
    if (this.#owed && (this.#sentAt === null || now - this.#sentAt >= PLACE_INTERVAL_MS)) {
      this.#flush(now);
    }
    return this.#rect;
  }

  /**
   * The button came up.
   *
   * Sends where the pointer finished, if that has not been sent already —
   * without which a drag shorter than one interval would move the window on
   * the screen and nowhere else.
   */
  end(now: number): void {
    if (this.#owed) {
      this.#flush(now);
    }
  }

  /** Sends the current rectangle and remembers that it was sent. */
  #flush(now: number): void {
    this.#sent = this.#rect;
    this.#sentAt = now;
    this.#owed = false;
    this.#options.place(this.#rect);
  }
}
