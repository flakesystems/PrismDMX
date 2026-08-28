/**
 * One window on the canvas: a frame, a title bar, a corner, and no idea where
 * it is.
 *
 * The rectangle it renders at comes from the session on every render. The only
 * thing it holds is the drag in progress — `ARCHITECTURE_SPEC.md` §4.2's own
 * "drag state", which is client-local by name — and it drops that the moment the
 * button comes up. See `./drag.ts` for why that is the whole design.
 *
 * # Pointer events, and no pointer capture
 *
 * A drag listens on the **window** rather than capturing the pointer on the
 * element. Capturing is the tidier browser API and this deliberately does not
 * use it: a pointer dragged past the edge of a small window has to keep being
 * heard, `setPointerCapture` does not exist in the runtime the unit tests run
 * in, and a listener that is added on pointer-down and removed on pointer-up
 * costs one pair of calls per drag.
 */

import { useEffect, useState } from "react";
import type { PointerEvent as ReactPointerEvent, ReactNode } from "react";

import type { Rect } from "./geometry";
import { asStyle, toCanvasUnits } from "./geometry";
import { WindowDrag } from "./drag";
import type { CanvasWindow } from "./windows";
import { windowTitle } from "./windows";

/** What a window frame needs to be drawn and driven. */
export interface WindowFrameProps {
  /** The window, as the session holds it. */
  readonly window: CanvasWindow;
  /** Whether it is the focused one. */
  readonly focused: boolean;
  /** The canvas element's box, for turning pixels into canvas units. */
  readonly box: () => { readonly width: number; readonly height: number };
  /**
   * Every other open window, as the session holds it.
   *
   * A drag stops against these the way it stops against the canvas edges — S43,
   * B10. They come down from the canvas rather than being read here, because the
   * canvas is what has the list and a frame that fetched its own would be
   * reading the session once per window.
   */
  readonly neighbours: readonly Rect[];
  /** Sends a `PlaceWindow`. */
  readonly onPlace: (instanceId: number, rect: Rect) => void;
  /** Sends a `FocusWindow`. */
  readonly onFocus: (instanceId: number) => void;
  /** Sends a `CloseWindow`. */
  readonly onClose: (instanceId: number) => void;
  /** What the window shows. */
  readonly children: ReactNode;
}

/** One window. */
export function WindowFrame({
  window: instance,
  focused,
  box,
  neighbours,
  onPlace,
  onFocus,
  onClose,
  children,
}: WindowFrameProps) {
  const [drag, setDrag] = useState<WindowDrag | null>(null);
  const [shown, setShown] = useState<Rect | null>(null);

  useEffect(() => {
    if (drag === null) {
      return;
    }
    const move = (event: PointerEvent): void => {
      setShown(drag.to({ x: event.clientX, y: event.clientY }, Date.now()));
    };
    const finish = (): void => {
      drag.end(Date.now());
      // **The local rectangle is dropped here**, not reconciled: what the
      // window renders at from this moment is whatever the session says, which
      // is where the pointer left it if the commands landed and where it
      // really is if they did not.
      setDrag(null);
      setShown(null);
    };
    globalThis.addEventListener("pointermove", move);
    globalThis.addEventListener("pointerup", finish);
    globalThis.addEventListener("pointercancel", finish);
    return () => {
      globalThis.removeEventListener("pointermove", move);
      globalThis.removeEventListener("pointerup", finish);
      globalThis.removeEventListener("pointercancel", finish);
    };
  }, [drag]);

  const begin = (kind: "move" | "resize") => (event: ReactPointerEvent) => {
    // The primary button only: a right-click on a title bar is a context menu,
    // not a drag that never ends because no `pointerup` follows.
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    const started = new WindowDrag({
      instanceId: instance.instanceId,
      kind,
      origin: instance,
      from: { x: event.clientX, y: event.clientY },
      place: (rect) => {
        onPlace(instance.instanceId, rect);
      },
      scale: (dx, dy) => toCanvasUnits(dx, dy, box()),
      neighbours,
    });
    setDrag(started);
    setShown(started.rect);
  };

  const title = windowTitle(instance.type);
  return (
    <section
      className={`window${focused ? " window-focused" : ""}${drag === null ? "" : " window-dragging"}`}
      style={asStyle(shown ?? instance)}
      data-testid={`window-${String(instance.instanceId)}`}
      data-window-type={instance.type}
      data-focused={focused ? "yes" : "no"}
      aria-label={title}
      onPointerDown={() => {
        if (!focused) {
          onFocus(instance.instanceId);
        }
      }}
    >
      <header
        className="window-title"
        data-testid={`title-window-${String(instance.instanceId)}`}
        onPointerDown={begin("move")}
      >
        <span className="window-name">{title}</span>
        <span className="window-number">{instance.instanceId}</span>
        <button
          type="button"
          className="window-close"
          data-testid={`close-window-${String(instance.instanceId)}`}
          aria-label={`Close ${title}`}
          // On pointer-down, so that the click cannot start a drag of the
          // title bar it sits in.
          onPointerDown={(event) => {
            event.stopPropagation();
          }}
          onClick={() => {
            onClose(instance.instanceId);
          }}
        >
          ×
        </button>
      </header>
      <div className="window-body">{children}</div>
      <div
        className="window-grip"
        data-testid={`resize-window-${String(instance.instanceId)}`}
        aria-hidden="true"
        onPointerDown={begin("resize")}
      />
    </section>
  );
}
