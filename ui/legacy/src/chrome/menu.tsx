/**
 * The pop-up menu — one implementation, wherever a right-click means *manage
 * this*.
 *
 * # Why the desk has one of these at all — S43
 *
 * The owner's rebuild asks the pools to become **grids**: a sequence, a group
 * and a preset are each a box you press, and the half-dozen smaller things you
 * can do to one — rename it, colour it, delete it, copy it — come off a
 * right-click rather than off keys crowded into the box. *Namens und
 * Farbänderungen oder Ähnliches sollen keinen eigenen Knopf bekommen, sondern
 * mit Rechtsklick auf eine Sequence erreichbar sein.*
 *
 * That is a good rule for a console and a bad one to write four times. S35 built
 * the pattern once for the View Selector Bar, complete with the two ways out
 * every menu needs; this is that component with the view-specific parts taken
 * out, and the view bar is its first caller rather than a fifth copy.
 *
 * # What a menu is allowed to be
 *
 * Client-local, always — `ARCHITECTURE_SPEC.md` §4.2. Whether a menu is open,
 * over what, and at which corner of *this* screen is not something a second
 * operator's screen should follow, and the console cannot open one and does not
 * need to: everything on a menu is also a line, which is §4.5.
 *
 * # Two ways out, and neither of them is choosing something
 *
 * Escape and a click anywhere else, both on the document, because a menu that
 * can only be dismissed by picking an item is a menu an operator is trapped in.
 * A menu whose subject has gone — deleted here, or on another screen — takes
 * itself away; `chrome/menuat.ts`'s `useMenuAt` is where that is arranged,
 * because the subject is the caller's and only the caller can say whether it is
 * still there. The hook is a module of its own for `desk/commandinput.ts`'s
 * reason: a file that exports a component and a hook loses fast refresh.
 *
 * # It is clamped to the window
 *
 * `CLAUDE.md` forbids scrolling outside the canvas, and a menu opened near the
 * right edge of the screen used to extend past it — which on a device screen is
 * not a scrollbar but a menu with items nobody can reach. It is measured after
 * it is drawn and pulled back inside; see {@link ContextMenu}.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";

/** What a menu needs. */
export interface ContextMenuProps {
  /** Where the pointer was. */
  readonly at: { readonly x: number; readonly y: number };
  /** What the menu is about, drawn as its heading. */
  readonly title: string;
  /** The screen-reader name of the menu. */
  readonly label: string;
  /** The test id — the contract this menu is found by. */
  readonly testId: string;
  /**
   * What the menu is over, as `data-subject`.
   *
   * A reading rather than decoration: *this menu belongs to that box* is the
   * one thing about a pop-up a test cannot see any other way, and a menu drawn
   * over the wrong subject would send every one of its commands to the wrong
   * object.
   */
  readonly subject?: string;
  /** Closes it without choosing anything. */
  readonly onClose: () => void;
  /** The items, which are {@link MenuItem}s. */
  readonly children: React.ReactNode;
}

/** The menu itself. */
export function ContextMenu({
  at,
  title,
  label,
  testId,
  subject,
  onClose,
  children,
}: ContextMenuProps) {
  const box = useRef<HTMLDivElement | null>(null);
  const [placed, setPlaced] = useState<{ readonly x: number; readonly y: number }>(at);

  // Escape closes, and so does a click anywhere else — see the module
  // documentation for why both are on the document.
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

  // Measured after it is drawn and pulled back inside the window. A layout
  // effect rather than an effect, so the corrected position is painted with the
  // first frame instead of one frame after it — a menu that visibly jumps is a
  // menu an operator clicks the wrong item on.
  useLayoutEffect(() => {
    const element = box.current;
    if (element === null) {
      return;
    }
    const size = element.getBoundingClientRect();
    const margin = 4;
    const x = Math.max(margin, Math.min(at.x, globalThis.innerWidth - size.width - margin));
    const y = Math.max(margin, Math.min(at.y, globalThis.innerHeight - size.height - margin));
    setPlaced({ x, y });
  }, [at.x, at.y]);

  return (
    <div
      ref={box}
      className="menu"
      data-testid={testId}
      data-subject={subject}
      role="menu"
      aria-label={label}
      style={{ left: `${String(placed.x)}px`, top: `${String(placed.y)}px` }}
      onContextMenu={(event) => {
        // A second right-click closes it rather than opening the browser's own
        // menu on top of it — the same rule the window chooser follows.
        event.preventDefault();
        onClose();
      }}
    >
      <p className="menu-title">{title}</p>
      {children}
    </div>
  );
}

/** One item. Choosing it does the thing and closes the menu. */
export function MenuItem({
  testId,
  onChoose,
  onClose,
  disabled = false,
  danger = false,
  title,
  children,
}: {
  readonly testId: string;
  readonly onChoose: () => void;
  readonly onClose: () => void;
  readonly disabled?: boolean;
  /** Drawn as a warning — delete, and nothing else so far. */
  readonly danger?: boolean;
  readonly title?: string;
  readonly children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      className={danger ? "menu-danger" : undefined}
      data-testid={testId}
      disabled={disabled}
      title={title}
      onClick={() => {
        onChoose();
        onClose();
      }}
    >
      {children}
    </button>
  );
}

/**
 * A menu item that asks for one line of text before it does anything.
 *
 * The rename shape, which four of the five pools want. What has been typed is
 * local and is dropped when the menu closes; the name is the daemon's until the
 * command comes back as a patch, which is **D3** in the shape
 * `desk/commandline.tsx` writes it down.
 */
export function MenuField({
  testId,
  label,
  verb,
  initial,
  onSubmit,
  onClose,
}: {
  readonly testId: string;
  readonly label: string;
  /** The word on the key — *Rename*, *Colour*. */
  readonly verb: string;
  readonly initial: string;
  readonly onSubmit: (text: string) => void;
  readonly onClose: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [typed, setTyped] = useState(initial);
  if (!open) {
    return (
      <button
        type="button"
        role="menuitem"
        data-testid={testId}
        onClick={() => {
          setTyped(initial);
          setOpen(true);
        }}
      >
        {label}
      </button>
    );
  }
  return (
    <form
      className="menu-field"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(typed);
        onClose();
      }}
    >
      <input
        type="text"
        data-testid={`${testId}-input`}
        aria-label={label}
        value={typed}
        // eslint-disable-next-line jsx-a11y/no-autofocus -- the item was just
        // chosen; the focus is where the operator put it.
        autoFocus
        onChange={(event) => {
          setTyped(event.target.value);
        }}
      />
      <button type="submit" data-testid={`${testId}-apply`}>
        {verb}
      </button>
    </form>
  );
}
