/**
 * The View Selector Bar, and the menu that manages a view.
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
 * # Storing a view, and making an empty one
 *
 * Two buttons and two commands, which **S43** separated for punch-list B11.
 * `Store View` overwrites the numbered view with the canvas as it now is;
 * `New View` makes an empty one and switches to it. The button labelled *New*
 * used to send the first, so a new view arrived full of the last one's windows —
 * which is the fault B11 describes. Giving `Store View` the second meaning would
 * have been the smaller change and wrong twice over: the name would no longer
 * describe it, and an F-key bound to `Store View 4` would quietly do something
 * else than it did the day before.
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
 *
 * # The menu itself moved out — S43, the owner's rebuild
 *
 * It was built here in S35 and it is `chrome/menu.tsx` now, because the rebuild
 * asks the sequence, group and preset pools to reach their smaller actions the
 * same way. Everything about the behaviour is unchanged — the two ways out, the
 * subject that closes its own menu when it goes, the items that send and close
 * — and this file went first through the shared component rather than a fifth
 * copy of it being written. The test ids are untouched: `view-menu` and every
 * item under it is a contract.
 */

import { useCallback } from "react";

import type { JsonValue } from "../bindings";
import { ContextMenu, MenuField, MenuItem } from "../chrome/menu";
import { useMenuAt } from "../chrome/menuat";
import { activeViewId, storedViews } from "./windows";
import type { StoredView } from "./windows";

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
  readonly onMoveView: (viewId: number, toViewId: number) => void;
  /**
   * Sends a `NewView` — an empty view, selected, canvas cleared.
   *
   * **S43, punch-list B11.** The button used to send `Store View`, which means
   * *keep what is on the canvas*, so a "new" view arrived carrying the last
   * one's windows and had to be emptied by hand. The two meanings are both
   * wanted, so there are two commands and two buttons.
   */
  readonly onNewView: (viewId: number, name: string) => void;
}

/** The bar. */
export function ViewBar({
  session,
  onSelectView,
  onStoreView,
  onRenameView,
  onDeleteView,
  onMoveView,
  onNewView,
}: ViewBarProps) {
  const views = storedViews(session);
  const active = activeViewId(session);
  // A view that has gone — deleted here, or on another screen — takes its menu
  // with it, which is what `present` is for. Without it the menu would stand
  // open over a number no longer on the bar, every item of it refused.
  const present = useCallback(
    (viewId: number) => views.some((view) => view.id === viewId),
    [views],
  );
  const { menu, openMenu, closeMenu } = useMenuAt(present);
  const openOver = menu === null ? undefined : views.find((view) => view.id === menu.subject);

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
          onContextMenu={openMenu(view.id)}
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
        title="Make an empty view and switch to it"
        onClick={() => {
          const next = nextViewId(views);
          onNewView(next, `View ${String(next)}`);
        }}
      >
        New
      </button>

      {menu !== null && openOver !== undefined ? (
        <ViewMenu
          at={menu}
          view={openOver}
          views={views}
          onClose={closeMenu}
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
  readonly at: { readonly x: number; readonly y: number };
  readonly view: StoredView;
  readonly views: readonly StoredView[];
  readonly onClose: () => void;
  readonly onStoreView: (viewId: number, name: string) => void;
  readonly onRenameView: (viewId: number, name: string) => void;
  readonly onDeleteView: (viewId: number) => void;
  readonly onMoveView: (viewId: number, toViewId: number) => void;
}) {
  const at_first = views[0]?.id === view.id;
  const at_last = views.at(-1)?.id === view.id;
  const only = views.length === 1;

  return (
    <ContextMenu
      at={at}
      title={`${String(view.id)} · ${view.name}`}
      label={`Manage view ${String(view.id)}`}
      testId="view-menu"
      subject={String(view.id)}
      onClose={onClose}
    >
      <MenuField
        testId="view-rename"
        label="Rename…"
        verb="Rename"
        initial={view.name}
        onClose={onClose}
        onSubmit={(text) => {
          const name = text.trim();
          if (name !== "") {
            onRenameView(view.id, name);
          }
        }}
      />

      <MenuItem
        testId="view-overwrite"
        onClose={onClose}
        title="Replace this view's layout with the canvas as it is now"
        onChoose={() => {
          onStoreView(view.id, view.name);
        }}
      >
        Store canvas here
      </MenuItem>
      <MenuItem
        testId="view-store-new"
        onClose={onClose}
        // At the end, and not between this view and the next one. Under S35's
        // decision the number *is* the position, so inserting between two
        // consecutive numbers would renumber every view above — which would
        // silently change what an F-key bound to `SelectView 4` reaches. One
        // command that always works, then Move to place it.
        title="Store the canvas as a new view at the end of the bar"
        onChoose={() => {
          const next = nextViewId(views);
          onStoreView(next, `View ${String(next)}`);
        }}
      >
        Store canvas as new view
      </MenuItem>

      {/*
        **The bar knows its neighbour's number and writes the line** — S40.
        `MoveView` was relative until then (`Prev`/`Next`); it is now
        `Move View 1 View 3`, an absolute swap, and one command covers both
        because the *screen* is what turns a direction into a number. That is
        `ARCHITECTURE_SPEC.md` §4.5 entire: the key builds a line an operator
        could have typed.
      */}
      <MenuItem
        testId="view-move-prev"
        onClose={onClose}
        disabled={at_first}
        onChoose={() => {
          const before = neighbour(views, view.id, -1);
          if (before !== null) {
            onMoveView(view.id, before);
          }
        }}
      >
        Move left
      </MenuItem>
      <MenuItem
        testId="view-move-next"
        onClose={onClose}
        disabled={at_last}
        onChoose={() => {
          const after = neighbour(views, view.id, 1);
          if (after !== null) {
            onMoveView(view.id, after);
          }
        }}
      >
        Move right
      </MenuItem>

      <MenuItem
        testId="view-delete"
        onClose={onClose}
        danger
        // The daemon refuses this too (`SessionError::LastView`) — the session
        // must always have a view for `activeViewId` to name. Disabled here as
        // well so the refusal is not the first an operator hears of it.
        disabled={only}
        title={
          only
            ? "The last view cannot be deleted"
            : "Delete this view. What the canvas then shows is the daemon's answer."
        }
        onChoose={() => {
          onDeleteView(view.id);
        }}
      >
        Delete
      </MenuItem>
    </ContextMenu>
  );
}

/**
 * The view one place along the bar, by **number**.
 *
 * The bar draws in number order (S35: the number *is* the order), so *left* and
 * *right* are the previous and next numbers that are actually stored — never
 * `id ± 1`, which would name a place nobody has used.
 */
function neighbour(
  views: readonly { readonly id: number }[],
  id: number,
  step: -1 | 1,
): number | null {
  const index = views.findIndex((view) => view.id === id);
  if (index === -1) {
    return null;
  }
  return views[index + step]?.id ?? null;
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

