/**
 * The View Selector Bar, and the one control that opens a window.
 *
 * `ARCHITECTURE_SPEC.md` §4.1 calls `activeViewId` "canvas layout from the View
 * Selector Bar", and **D8** makes the X-Touch's `Channel ◀▶` the same thing:
 * this bar and those two buttons issue the identical `SelectView`, so pressing
 * one moves the other. That is not a coincidence to be maintained — it is
 * `docs/IPC_PROTOCOL.md` §5's second group, which the console and the interface
 * share on purpose.
 *
 * Which view is lit is read from the session, so a view switched at the console
 * lights up here without this component being told anything.
 *
 * # Storing a view
 *
 * `StoreView` overwrites the numbered view with the canvas as it now is, and it
 * is the **one** session command that lights the Save lamp (`prism-core`:
 * paging faders is operating a desk, storing a layout is authoring one). It is
 * therefore a deliberate button and not something that happens when a window is
 * dragged.
 */

import type { WindowType } from "../bindings";
import { WINDOW_TYPE_VARIANTS } from "../bindings/variants";
import type { JsonValue } from "../bindings";
import { activeViewId, storedViews, windowTitle } from "./windows";

/** What the bar needs. */
export interface ViewBarProps {
  /** The session document. */
  readonly session: JsonValue;
  /** Sends a `SelectView`. */
  readonly onSelectView: (viewId: number) => void;
  /** Sends a `StoreView`. */
  readonly onStoreView: (viewId: number, name: string) => void;
  /** Sends an `OpenWindow`. */
  readonly onOpenWindow: (type: WindowType) => void;
}

/** The bar. */
export function ViewBar({ session, onSelectView, onStoreView, onOpenWindow }: ViewBarProps) {
  const views = storedViews(session);
  const active = activeViewId(session);

  return (
    <nav className="viewbar" data-testid="viewbar" aria-label="Views and windows">
      <span className="viewbar-label">Views</span>
      {views.map((view) => (
        <button
          key={view.id}
          type="button"
          className={`view-button${view.id === active ? " view-active" : ""}`}
          data-testid={`view-${String(view.id)}`}
          data-active={view.id === active ? "yes" : "no"}
          title={`${view.name} — ${String(view.windowCount)} window(s)`}
          onClick={() => {
            onSelectView(view.id);
          }}
        >
          <span className="view-number">{view.id}</span>
          <span className="view-name">{view.name}</span>
        </button>
      ))}
      <button
        type="button"
        className="view-store"
        data-testid="store-view"
        title="Overwrite the active view with the canvas as it is now"
        onClick={() => {
          if (active !== null) {
            const stored = views.find((view) => view.id === active);
            onStoreView(active, stored?.name ?? `View ${String(active)}`);
          }
        }}
      >
        Store
      </button>
      <button
        type="button"
        className="view-store"
        data-testid="new-view"
        title="Store the canvas as a new view"
        onClick={() => {
          const next = nextViewId(views);
          onStoreView(next, `View ${String(next)}`);
        }}
      >
        New
      </button>

      <span className="viewbar-label viewbar-windows">Add window</span>
      {/*
        Every window type there is, from the generated table rather than from a
        list written here — S23's finding: a view that hand-writes one of these
        reintroduces exactly the drift `bindings/variants.ts` removes.
      */}
      <select
        className="window-picker"
        data-testid="open-window"
        aria-label="Open a window"
        // A picker with no value: it is an action, not a setting, and what is
        // open is the session's business rather than this element's.
        value=""
        onChange={(event) => {
          const chosen = event.target.value;
          if (isWindowType(chosen)) {
            onOpenWindow(chosen);
          }
        }}
      >
        <option value="">Open…</option>
        {WINDOW_TYPE_VARIANTS.map((type) => (
          <option key={type} value={type}>
            {windowTitle(type)}
          </option>
        ))}
      </select>
    </nav>
  );
}

/**
 * The number to store the canvas as when the operator asks for a *new* view.
 *
 * One past the highest that exists, so `New` never overwrites a layout
 * somebody was keeping. It is worked out here rather than asked for because
 * `StoreView` names the number: the protocol has no *store as the next one*,
 * and inventing a command for what is one addition would be the wrong end to
 * fix it at.
 */
function nextViewId(views: readonly { readonly id: number }[]): number {
  return views.reduce((highest, view) => Math.max(highest, view.id), 0) + 1;
}

/** Whether a string is one of the window types this build knows. */
function isWindowType(value: string): value is WindowType {
  return (WINDOW_TYPE_VARIANTS as readonly string[]).includes(value);
}
