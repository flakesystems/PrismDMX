/**
 * Where a pop-up menu is open, and the handler that opens one.
 *
 * A module of its own, and that is not tidiness: `menu.tsx` draws components,
 * and a file that exports both a component and a hook loses fast refresh for the
 * whole module — the interface reloads instead of updating, which is a worse
 * thing to live with than one extra file. `desk/commandinput.ts` is the same
 * split for the same reason, and S39 recorded that rule as being right rather
 * than a nuisance.
 *
 * What the state *is* is `ARCHITECTURE_SPEC.md` §4.2's own category: whether a
 * menu is open, over what, and at which corner of **this** screen is not
 * something a second operator's screen should follow, and the console cannot
 * open one and does not need to — everything on a menu is also a line (§4.5).
 */

import { useCallback, useEffect, useState } from "react";

/** Where a menu is open, and over which subject. */
export interface MenuAt<Id> {
  /** What the menu is about — a sequence number, a group id, a preset id. */
  readonly subject: Id;
  /** Viewport coordinates of the pointer that opened it. */
  readonly x: number;
  readonly y: number;
}

/**
 * The open-menu state, and the handler that opens one.
 *
 * `present` is what keeps a menu honest: it answers whether the subject still
 * exists, and a menu whose subject has gone closes itself. Without it a delete
 * chosen from the menu would leave the menu standing over a number that is no
 * longer in the pool, with every one of its items refused by the daemon.
 */
export function useMenuAt<Id>(present: (subject: Id) => boolean): {
  readonly menu: MenuAt<Id> | null;
  readonly openMenu: (subject: Id) => (event: React.MouseEvent) => void;
  readonly closeMenu: () => void;
} {
  const [menu, setMenu] = useState<MenuAt<Id> | null>(null);
  const closeMenu = useCallback(() => {
    setMenu(null);
  }, []);
  const openMenu = useCallback(
    (subject: Id) => (event: React.MouseEvent) => {
      event.preventDefault();
      setMenu({ subject, x: event.clientX, y: event.clientY });
    },
    [],
  );
  const gone = menu !== null && !present(menu.subject);
  useEffect(() => {
    if (gone) {
      setMenu(null);
    }
  }, [gone]);
  return { menu: gone ? null : menu, openMenu, closeMenu };
}
