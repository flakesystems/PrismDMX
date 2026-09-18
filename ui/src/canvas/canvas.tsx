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
 * `FocusWindow` moving it to the end of `openWindows` in the daemon.
 *
 * # The elements do not move — B57
 *
 * Until the beta that array order was also the **document** order, on the
 * argument that a `z-index` would be a second source of truth. It cost a click:
 * focusing a window reordered the array, React moved that window's element to
 * the end of the canvas while the button was still down, and Chromium fires no
 * `click` on an element that left the document between press and release. So
 * the first click into an unfocused window focused it and selected nothing.
 *
 * Now the elements are drawn in the order of their **numbers**, which never
 * changes while a window is open, and the session's order is each element's
 * `z-index` — computed from that one list on every render, so it is a
 * rendering of the daemon's decision rather than a second opinion about it. The
 * stylesheet still sets none.
 *
 * # The canvas fills what it is given
 *
 * `CLAUDE.md`: a device screen, no scrolling outside the canvas. The element is
 * the containing block for absolutely positioned windows and it never scrolls;
 * windows are placed in percentages of it, so a window keeps its share of the
 * screen when the screen changes size — and no command is sent when that
 * happens, because the size of a screen is client-local (§4.2).
 */

import { useCallback, useEffect, useRef, useState } from "react";

import type { JsonValue, ProgrammerState } from "../bindings";
import { ModalLayer } from "../chrome/modal";
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
  /** The programmer, for the windows that show what is being programmed. */
  readonly programmer: ProgrammerState | null;
  /** Sends a `PlaceWindow`. */
  readonly onPlace: (instanceId: number, rect: Rect) => void;
  /** Sends a `FocusWindow`. */
  readonly onFocus: (instanceId: number) => void;
  /** Sends a `CloseWindow`. */
  readonly onClose: (instanceId: number) => void;
  /** Sends a `SetWindowPicker` — a right-click on an empty part opens it. */
  readonly onPicker: (open: boolean) => void;
}

/** The whole canvas. */
export function Canvas({
  session,
  show,
  programmer,
  onPlace,
  onFocus,
  onClose,
  onPicker,
}: CanvasProps) {
  const surface = useRef<HTMLDivElement>(null);
  // The canvas, for a modal opened inside one of its windows (S57): see
  // `ModalLayer`. State rather than the ref, so the windows are drawn again
  // once there is an element to portal into.
  const [layer, setLayer] = useState<HTMLElement | null>(null);
  useEffect(() => {
    setLayer(surface.current);
  }, []);
  const windows = openWindows(session);
  const focused = focusedWindow(session);
  // By number, so an element never moves in the document while it is open —
  // see the module documentation (B57).
  const byNumber = [...windows].sort((a, b) => a.instanceId - b.instanceId);

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
    <div
      className="canvas"
      ref={surface}
      data-testid="canvas"
      data-windows={windows.length}
      // **A right-click on an empty part opens the window chooser** — S43, B9.
      // `event.target === event.currentTarget` is what *empty* means: a click
      // over a window is that window's business, and a browser context menu on
      // top of a console is nobody's.
      onContextMenu={(event) => {
        if (event.target === event.currentTarget) {
          event.preventDefault();
          onPicker(true);
        }
      }}
    >
      {windows.length === 0 ? <EmptyCanvas /> : null}
      <ModalLayer.Provider value={layer}>
        {byNumber.map((instance: CanvasWindow) => (
          <WindowFrame
            key={instance.instanceId}
            window={instance}
            stack={windows.indexOf(instance) + 1}
            focused={instance.instanceId === focused}
            box={box}
            // Every window but this one, so a drag stops against them the way it
            // stops against the canvas edges (S43, B10). It is the session's own
            // list, which is what makes it a prediction of the daemon's rule
            // rather than a second opinion about the layout.
            neighbours={windows.filter(
              (other) => other.instanceId !== instance.instanceId,
            )}
            onPlace={onPlace}
            onFocus={onFocus}
            onClose={onClose}
          >
            <WindowContent
              window={instance}
              show={show}
              session={session}
              programmer={programmer}
            />
          </WindowFrame>
        ))}
      </ModalLayer.Provider>
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
      No windows are open in this view. Right-click here, press Insert, or use
      an F-key on the console.
    </p>
  );
}
