/**
 * Reading the canvas out of the session document.
 *
 * S23 left the rule these follow: *the pointers the views read are
 * `prism-core`'s document shapes, not a model*, and typed accessors over the
 * session belong beside `mirror/select.ts` as **readers** rather than as a
 * parsed copy. A parsed copy would be a second model to keep in step, and RFC
 * 6902 operations only mean anything against a root.
 *
 * So nothing here holds anything. Each function walks the document it is given
 * and answers; the identity of what it answers changes whenever the document
 * does, which is exactly when a canvas has to be redrawn.
 *
 * # A window this build does not understand is not drawn
 *
 * `type` is narrowed against {@link WINDOW_TYPE_VARIANTS}, which is generated
 * from the Rust enum. A daemon newer than this interface can hold a window type
 * this one has never heard of, and the honest answer is to leave it out of the
 * canvas rather than to draw an empty frame or to throw and take the whole
 * interface down. The same goes for a window whose geometry is not four finite
 * numbers, which only a hand-edited show file produces.
 */

import type { JsonValue, WindowType } from "../bindings";
import { WINDOW_TYPE_VARIANTS } from "../bindings/variants";
import { logger } from "../log/logger";
import { isArray, isObject } from "../mirror/patch";
import { numberAt, stringAt, valueAt } from "../mirror/select";
import type { Rect } from "./geometry";

const log = logger("canvas");

/** Where the open windows live in the session document. */
export const OPEN_WINDOWS = "/session/openWindows";

/** Where the focused window lives. */
export const FOCUSED_WINDOW = "/session/focusedWindow";

/** Where the active view number lives. */
export const ACTIVE_VIEW = "/session/activeViewId";

/** Where the stored views live. */
export const VIEWS = "/views";

/** One open window, as the canvas needs it. */
export interface CanvasWindow extends Rect {
  /** Its number within the session, which is what commands name it by. */
  readonly instanceId: number;
  /** What it shows. */
  readonly type: WindowType;
}

/** One stored layout, as the View Selector Bar needs it. */
export interface StoredView {
  /** Its number, which `SelectView` names. */
  readonly id: number;
  /** What the operator called it. */
  readonly name: string;
  /** How many windows it restores. */
  readonly windowCount: number;
}

/**
 * The open windows, in stacking order — first is furthest back.
 *
 * The order is the document's, and it is *not* sorted: `prism-core` moves a
 * focused window to the end of the list, so the array order is the stacking
 * order and re-sorting it by number would silently undo every `FocusWindow`.
 */
export function openWindows(session: JsonValue | null): readonly CanvasWindow[] {
  const value = valueAt(session, OPEN_WINDOWS);
  if (!isArray(value)) {
    return [];
  }
  const windows: CanvasWindow[] = [];
  for (const entry of value) {
    const window = asWindow(entry);
    if (window !== null) {
      windows.push(window);
    }
  }
  return windows;
}

/** The focused window's number, or `null` when nothing is focused. */
export function focusedWindow(session: JsonValue | null): number | null {
  return numberAt(session, FOCUSED_WINDOW);
}

/** The active view's number, or `null` when the document has none. */
export function activeViewId(session: JsonValue | null): number | null {
  return numberAt(session, ACTIVE_VIEW);
}

/**
 * The stored views, in number order.
 *
 * The document keys them by number *as a string*, because that is what a JSON
 * object can do; they are ordered numerically here, so the View Selector Bar
 * shows view 2 before view 10.
 */
export function storedViews(session: JsonValue | null): readonly StoredView[] {
  const value = valueAt(session, VIEWS);
  if (!isObject(value)) {
    return [];
  }
  const views: StoredView[] = [];
  for (const [key, entry] of Object.entries(value)) {
    const id = Number(key);
    if (!Number.isInteger(id) || !isObject(entry)) {
      continue;
    }
    const name = entry["name"];
    const windows = entry["windows"];
    views.push({
      id,
      name: typeof name === "string" ? name : `View ${String(id)}`,
      windowCount: windows !== undefined && isArray(windows) ? windows.length : 0,
    });
  }
  return views.sort((left, right) => left.id - right.id);
}

/** One window's rectangle, or `null` when it is not open. */
export function windowRect(session: JsonValue | null, instanceId: number): Rect | null {
  const window = openWindows(session).find((open) => open.instanceId === instanceId);
  return window === undefined ? null : { x: window.x, y: window.y, w: window.w, h: window.h };
}

/**
 * The name to put on a window's title bar.
 *
 * Derived from the type rather than stored: `WindowInstance` has no title, and
 * inventing a field for one would be the interface adding to the session.
 * `FixtureSheet` reads as *Fixture Sheet*, `DmxSheet` as *DMX Sheet*, and
 * `Viewer3D` as *Viewer 3D*.
 */
export function windowTitle(type: WindowType): string {
  if (type === "DmxSheet") {
    return "DMX Sheet";
  }
  // `FixtureSheet` to `Fixture Sheet`, `Viewer3D` to `Viewer 3D` — a break
  // before a capital or a digit that follows a lower-case letter, which leaves
  // `3D` together because the D follows a digit.
  return type.replace(/([a-z])([A-Z0-9])/g, "$1 $2");
}

/** One entry of `openWindows`, if it is a window this build can draw. */
function asWindow(entry: JsonValue): CanvasWindow | null {
  if (!isObject(entry)) {
    return null;
  }
  const type = stringAt(entry, "/type");
  if (type === null || !isWindowType(type)) {
    // A daemon that knows a window type this build does not. Said once per
    // kind would need state; said at debug level costs nothing and is where
    // somebody looking for a missing window will find the answer.
    log.debug("a window of an unknown type is not drawn", { type: type ?? "absent" });
    return null;
  }
  const instanceId = numberAt(entry, "/instanceId");
  const x = numberAt(entry, "/x");
  const y = numberAt(entry, "/y");
  const w = numberAt(entry, "/w");
  const h = numberAt(entry, "/h");
  if (instanceId === null || x === null || y === null || w === null || h === null) {
    log.warn("a window without a number or a rectangle is not drawn", { type });
    return null;
  }
  return { instanceId, type, x, y, w, h };
}

/** Whether a string is one of the window types this build knows. */
function isWindowType(value: string): value is WindowType {
  return (WINDOW_TYPE_VARIANTS as readonly string[]).includes(value);
}
