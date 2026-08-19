/**
 * The View Selector Bar, the one control that opens a window, and the menu that
 * manages a view.
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
 *
 * # Managing one — and why the number moves rather than an order beside it
 *
 * S35 added `RenameView`, `DeleteView` and `MoveView`. Moving **exchanges the
 * two views' numbers**, because the numbers *are* the order: they are what this
 * bar draws in, what `prismd::surface::context_of` steps for `Channel ◀▶`, and
 * what `SelectView` names. An order recorded beside the map would have been a
 * second thing to keep in step with the first, and the failure would have been
 * the console stepping to a view other than the one drawn next. See
 * `prism_domain::Command::MoveView`.
 *
 * # Nothing here is applied before the daemon has agreed
 *
 * **D3.** The menu sends a command and closes; what the view *is* arrives as a
 * `SessionPatch`. The rename field is the same contract with a different event —
 * what has been typed is local, exactly as in `desk/commandline.tsx`, and it is
 * dropped when the field closes. This component holds no view list, no order and
 * no names: {@link storedViews} walks the session document every render.
 *
 * The **menu's own** state — whether it is open, over which view, and at which
 * corner of the screen — is §4.2 client-local, the same category as hover and
 * drag state. The console cannot open a menu and does not need to.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import type { JsonValue, WindowType } from "../bindings";
import { WINDOW_TYPE_VARIANTS } from "../bindings/variants";
import { activeViewId, storedViews, windowTitle } from "./windows";
import type { StoredView } from "./windows";

/** Which way a view moves along the bar. */
export type MoveDirection = "Prev" | "Next";

/** What the bar needs. */
export interface ViewBarProps {
  /** The session document. */
  readonly session: JsonValue;
  /** Sends a `SelectView`. */
  readonly onSelectView: (viewId: number) => void;
  /** Sends a `StoreView`. */
  readonly onStoreView: (viewId: number, name: string) => void;
  /** Sends a `RenameView`. */
  readonly onRenameView: (viewId: number, name: string) => void;
  /** Sends a `DeleteView`. */
  readonly onDeleteView: (viewId: number) => void;
  /** Sends a `MoveView`. */
  readonly onMoveView: (viewId: number, direction: MoveDirection) => void;
  /** Sends an `OpenWindow`. */
  readonly onOpenWindow: (type: WindowType) => void;
}

/** Where a menu is open, and over which view. */
interface MenuAt {
  readonly viewId: number;
  readonly x: number;
  readonly y: number;
}

/** The bar. */
export function ViewBar({
  session,
  onSelectView,
  onStoreView,
  onRenameView,
  onDeleteView,
  onMoveView,
  onOpenWindow,
}: ViewBarProps) {
  const views = storedViews(session);
  const active = activeViewId(session);
  const [menu, setMenu] = useState<MenuAt | null>(null);

  const close = useCallback(() => {
    setMenu(null);
  }, []);

  // A view that has gone — deleted here, or on another screen — takes its menu
  // with it. Without this the menu would be open over a number that is no
  // longer on the bar, and every one of its items would be refused.
  const openOver = menu === null ? undefined : views.find((view) => view.id === menu.viewId);
  useEffect(() => {
    if (menu !== null && openOver === undefined) {
      setMenu(null);
    }
  }, [menu, openOver]);

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
          title={`${view.name} — ${String(view.windowCount)} window(s). Right-click to manage.`}
          onClick={() => {
            onSelectView(view.id);
          }}
          onContextMenu={(event) => {
            event.preventDefault();
            setMenu({ viewId: view.id, x: event.clientX, y: event.clientY });
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

      {menu !== null && openOver !== undefined ? (
        <ViewMenu
          at={menu}
          view={openOver}
          views={views}
          onClose={close}
          onStoreView={onStoreView}
          onRenameView={onRenameView}
          onDeleteView={onDeleteView}
          onMoveView={onMoveView}
        />
      ) : null}
    </nav>
  );
}

/**
 * The menu over one view.
 *
 * Every item sends a command and closes. Nothing here waits for an answer and
 * nothing here shows one: the bar redraws when the `SessionPatch` arrives, which
 * is the only thing that can say what happened.
 */
function ViewMenu({
  at,
  view,
  views,
  onClose,
  onStoreView,
  onRenameView,
  onDeleteView,
  onMoveView,
}: {
  readonly at: MenuAt;
  readonly view: StoredView;
  readonly views: readonly StoredView[];
  readonly onClose: () => void;
  readonly onStoreView: (viewId: number, name: string) => void;
  readonly onRenameView: (viewId: number, name: string) => void;
  readonly onDeleteView: (viewId: number) => void;
  readonly onMoveView: (viewId: number, direction: MoveDirection) => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [typed, setTyped] = useState(view.name);
  const box = useRef<HTMLDivElement | null>(null);

  // Escape closes, and so does a click anywhere else. Both are on the document
  // because a menu that could only be dismissed by choosing something is a menu
  // an operator is trapped in.
  useEffect(() => {
    const key = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    const away = (event: MouseEvent): void => {
      if (box.current !== null && !box.current.contains(event.target as Node)) {
        onClose();
      }
    };
    globalThis.addEventListener("keydown", key);
    globalThis.addEventListener("pointerdown", away);
    return () => {
      globalThis.removeEventListener("keydown", key);
      globalThis.removeEventListener("pointerdown", away);
    };
  }, [onClose]);

  const at_first = views[0]?.id === view.id;
  const at_last = views.at(-1)?.id === view.id;
  const only = views.length === 1;

  return (
    <div
      ref={box}
      className="view-menu"
      data-testid="view-menu"
      data-view={view.id}
      role="menu"
      aria-label={`Manage view ${String(view.id)}`}
      style={{ left: `${String(at.x)}px`, top: `${String(at.y)}px` }}
    >
      <p className="view-menu-title">
        {view.id} · {view.name}
      </p>

      {renaming ? (
        // What has been typed is local and is dropped when this closes; the name
        // is the daemon's until `RenameView` comes back as a patch. D3, in the
        // shape `desk/commandline.tsx` writes it down.
        <form
          className="view-menu-rename"
          onSubmit={(event) => {
            event.preventDefault();
            const name = typed.trim();
            if (name !== "") {
              onRenameView(view.id, name);
            }
            onClose();
          }}
        >
          <input
            type="text"
            data-testid="view-rename-input"
            aria-label={`New name for view ${String(view.id)}`}
            value={typed}
            autoFocus
            onChange={(event) => {
              setTyped(event.target.value);
            }}
          />
          <button type="submit" data-testid="view-rename-apply">
            Rename
          </button>
        </form>
      ) : (
        <button
          type="button"
          role="menuitem"
          data-testid="view-rename"
          onClick={() => {
            setTyped(view.name);
            setRenaming(true);
          }}
        >
          Rename…
        </button>
      )}

      <button
        type="button"
        role="menuitem"
        data-testid="view-overwrite"
        title="Replace this view's layout with the canvas as it is now"
        onClick={() => {
          onStoreView(view.id, view.name);
          onClose();
        }}
      >
        Store canvas here
      </button>
      <button
        type="button"
        role="menuitem"
        data-testid="view-store-new"
        // At the end, and not between this view and the next one. Under S35's
        // decision the number *is* the position, so inserting between two
        // consecutive numbers would renumber every view above — which would
        // silently change what an F-key bound to `SelectView 4` reaches. One
        // command that always works, then Move to place it.
        title="Store the canvas as a new view at the end of the bar"
        onClick={() => {
          const next = nextViewId(views);
          onStoreView(next, `View ${String(next)}`);
          onClose();
        }}
      >
        Store canvas as new view
      </button>

      <button
        type="button"
        role="menuitem"
        data-testid="view-move-prev"
        disabled={at_first}
        onClick={() => {
          onMoveView(view.id, "Prev");
          onClose();
        }}
      >
        Move left
      </button>
      <button
        type="button"
        role="menuitem"
        data-testid="view-move-next"
        disabled={at_last}
        onClick={() => {
          onMoveView(view.id, "Next");
          onClose();
        }}
      >
        Move right
      </button>

      <button
        type="button"
        role="menuitem"
        className="view-menu-danger"
        data-testid="view-delete"
        // The daemon refuses this too (`SessionError::LastView`) — the session
        // must always have a view for `activeViewId` to name. Disabled here as
        // well so the refusal is not the first an operator hears of it.
        disabled={only}
        title={
          only
            ? "The last view cannot be deleted"
            : "Delete this view. What the canvas then shows is the daemon's answer."
        }
        onClick={() => {
          onDeleteView(view.id);
          onClose();
        }}
      >
        Delete
      </button>
    </div>
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
