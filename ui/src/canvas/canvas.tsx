/**
 * The canvas: the windows the session says are open, in the order it says they
 * are stacked, and nothing else.
 *
 * # It holds no layout
 *
 * There is no list of windows here, no positions, no z-index bookkeeping, no
 * "which one is on top". All four are read out of the session document on every
 * render, and every one of the four gestures — open, move, resize, close — is a
 * command that goes out and a delta that comes back. Nothing is applied
 * optimistically (**D3**), which is why the exit criterion *the layout survives
 * a restart of the interface* is true by construction rather than by an effort
 * to persist something.
 *
 * The stacking order is the array order, so a window is brought to the front by
 * `FocusWindow` moving it to the end of `openWindows` in the daemon. There is
 * deliberately no `z-index` in the stylesheet: document order is the stacking
 * order, and letting CSS have an opinion about it would be a second source of
 * truth for something the session already decides.
 *
 * # The canvas fills what it is given
 *
 * `CLAUDE.md`: a device screen, no scrolling outside the canvas. The element is
 * the containing block for absolutely positioned windows and it never scrolls;
 * windows are placed in percentages of it, so a window keeps its share of the
 * screen when the screen changes size — and no command is sent when that
 * happens, because the size of a screen is client-local (§4.2).
 */

import { useCallback, useRef } from "react";

import type { JsonValue } from "../bindings";
import { WindowContent } from "./content";
import type { Rect } from "./geometry";
import { WindowFrame } from "./window";
import type { CanvasWindow } from "./windows";
import { focusedWindow, openWindows } from "./windows";

/** What the canvas needs: two documents and the four commands. */
export interface CanvasProps {
  /** The session document. */
  readonly session: JsonValue;
  /** The show document, for what the windows display. */
  readonly show: JsonValue;
  /** Sends a `PlaceWindow`. */
  readonly onPlace: (instanceId: number, rect: Rect) => void;
  /** Sends a `FocusWindow`. */
  readonly onFocus: (instanceId: number) => void;
  /** Sends a `CloseWindow`. */
  readonly onClose: (instanceId: number) => void;
}

/** The whole canvas. */
export function Canvas({ session, show, onPlace, onFocus, onClose }: CanvasProps) {
  const surface = useRef<HTMLDivElement>(null);
  const windows = openWindows(session);
  const focused = focusedWindow(session);

  /**
   * The canvas element's box in CSS pixels, asked for at the moment a drag
   * needs it rather than kept.
   *
   * Kept, it would be one more thing to invalidate on every resize, and it is
   * needed a few times a second while a pointer is down and never otherwise.
   */
  const box = useCallback(() => {
    const element = surface.current;
    if (element === null) {
      return { width: 0, height: 0 };
    }
    const measured = element.getBoundingClientRect();
    return { width: measured.width, height: measured.height };
  }, []);

  return (
    <div className="canvas" ref={surface} data-testid="canvas" data-windows={windows.length}>
      {windows.length === 0 ? <EmptyCanvas /> : null}
      {windows.map((instance: CanvasWindow) => (
        <WindowFrame
          key={instance.instanceId}
          window={instance}
          focused={instance.instanceId === focused}
          box={box}
          onPlace={onPlace}
          onFocus={onFocus}
          onClose={onClose}
        >
          <WindowContent window={instance} show={show} />
        </WindowFrame>
      ))}
    </div>
  );
}

/**
 * What an empty canvas says.
 *
 * An empty view is an ordinary state — `View 1` starts empty — and the two ways
 * out of it are both named, because one of them is not on this screen.
 */
function EmptyCanvas() {
  return (
    <p className="canvas-empty" data-testid="canvas-empty">
      No windows are open in this view. Add one above, or press F1&ndash;F8 on the console.
    </p>
  );
}
