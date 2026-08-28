/**
 * The Group Pool: the fixture groups, as a grid of switches.
 *
 * # Why this window exists at all
 *
 * `prism_core::Show::store_group` has been in the daemon since S11 with **no
 * command in front of it** — `PROGRESS.md` §7 carried that out of S28 — so a
 * group could be read, selected and merged and never made. S40 gave it
 * `Command::StoreGroup`, and this is where an operator reaches it.
 *
 * # A group is a switch, not a toggle of its fixtures — S43, B27
 *
 * The owner's report: *wird eine Gruppe selektiert, werden alle Fixtures dieser
 * Gruppe getoggelt, das heißt, wenn sie vorher an waren gehen sie aus. Das ist
 * schlecht.* And it was: pressing a second group that shared a lamp with the
 * first took that lamp back out, so two overlapping groups could not be held
 * together at all.
 *
 * Pressing a box now **switches the group on**; pressing it again switches it
 * off and takes back only the fixtures no other group that is still on, and no
 * direct pick, is holding. None of that arithmetic is here: the line is the same
 * `+ Group 3` it always was, the daemon does the subtraction
 * (`prism_core::Programmer::select_group`), and which switches are down is
 * `ProgrammerState::selectedGroups` — read, not remembered, which is what makes
 * the X-Touch and this screen agree without either being told.
 *
 * # A grid, and the smaller actions on a right-click
 *
 * *Auch das Group Sheet und das Preset Sheet sollen ein Grid werden. Alle
 * kleineren Group bzw. Preset bezogenen Aktionen sollen über Rechtsklick
 * ausgeführt werden.* So a box carries the number, the name and how many
 * fixtures, and label, copy, move and delete come off `chrome/menu.tsx`.
 *
 * # The store bar is gone, and storing is the command line's
 *
 * The owner's second rebuild: *sowohl beim Group Sheet als auch beim Preset Pool
 * sollen die Store Sektionen entfernt werden, das soll nur über die Command Line
 * gemacht werden.*
 *
 * It costs nothing, because the bar was already writing a line — it built
 * `Store Group 4 "Front wash"` out of a number box, a name box and a mode
 * chooser and submitted it. What it was, in other words, was a second way to
 * type one sentence, taking a third of the window to do it. And it is *less*
 * than the line: `Store Group` and a click on a box is now `Store Group 3`,
 * because a pool is an argument keyboard (`consoleshell.ts::pickOnto`) — which
 * the bar could not do, since its number box only ever offered the next free
 * one.
 *
 * What the operator loses is the offered number. That is the count on the bar
 * above and a glance at the grid, and the console asks before it overwrites
 * anything.
 *
 * # Every key here writes a line
 *
 * `ARCHITECTURE_SPEC.md` §4.5. Pressing a box writes `+ Group 3`, or finishes
 * the line that was waiting for it; the menu writes `Label Group 3 …`,
 * `Copy Group 3 Group 4`, `Move …` and `Delete Group 3`. Each is a line the
 * operator could have typed, and each is the same line the console produces.
 *
 * # A group is a list of fixtures, not a look
 *
 * That is what makes `Group 3` a *selection* rather than a set of values, and it
 * is why the store takes the programmer's **selection** and nothing else
 * (`prism_core::Programmer::group`). The look over the same fixtures is a
 * preset, which is the window next door.
 */

import { useCallback, useMemo } from "react";

import type { JsonValue, ProgrammerState } from "../bindings";
import { ContextMenu, MenuField, MenuItem } from "../chrome/menu";
import { useMenuAt } from "../chrome/menuat";
import { objectLine, pick, useConsole } from "../desk/consoleshell";
import { groupRows, nextFreeNumber, selectedGroups } from "./looks";
import type { GroupRow } from "./looks";

/** The whole window. */
export function GroupPool({
  show,
  programmer,
}: {
  readonly show: JsonValue;
  /**
   * What the programmer holds — for `selectedGroups`, and nothing else.
   *
   * Which switches are down is the **programmer's** answer (B27). A pool that
   * remembered which boxes had been pressed would disagree with the console the
   * moment somebody pressed a group key on the X-Touch, which is exactly the
   * unsynchronised state this desk is built not to have.
   */
  readonly programmer: ProgrammerState | null;
}) {
  const rows = useMemo(() => groupRows(show), [show]);
  const on = useMemo(() => selectedGroups(programmer), [programmer]);
  const shell = useConsole();
  const { run } = shell;
  const present = useCallback((id: number) => rows.some((row) => row.id === id), [rows]);
  const { menu, openMenu, closeMenu } = useMenuAt(present);
  const over = menu === null ? undefined : rows.find((row) => row.id === menu.subject);

  return (
    <div className="looks" data-testid="group-pool">
      <div className="looks-bar">
        <span className="looks-count" data-testid="group-count">
          {rows.length} groups
        </span>
        <span className="looks-count" data-testid="group-on">
          {on.size} on
        </span>
      </div>
      {rows.length === 0 ? (
        <p className="window-note" data-testid="group-empty">
          There are no groups. Select some fixtures and type <code>Store Group 1</code> — a group
          is a list of fixtures, so it is the selection that goes in.
        </p>
      ) : (
        <div className="sheet-scroll" data-testid="group-scroll">
          <ul className="pool-grid">
            {rows.map((row) => (
              <li key={row.id}>
                <button
                  type="button"
                  className={`pool-box${on.has(row.id) ? " pool-box-on" : ""}`}
                  data-testid={`group-${String(row.id)}`}
                  data-on={on.has(row.id) ? "yes" : "no"}
                  title={
                    on.has(row.id)
                      ? `Switch group ${String(row.id)} off. Fixtures another group or a direct pick still holds stay selected.`
                      : `Add the fixtures of group ${String(row.id)} to the selection. Right-click to manage.`
                  }
                  onClick={() => {
                    // **An argument when the line is waiting for one** —
                    // `consoleshell.ts::pickOnto`. `Store` and a click here is
                    // `Store Group 3`, which is how a group is made now that
                    // this window has no store bar of its own.
                    //
                    // With nothing typed it is the switch: `+ Group 3` is the
                    // grammar's own word for *and these too*, and since B27 it
                    // means the group's switch — the daemon turns it on, or off
                    // and works out what to release. A typed `Group 3` still
                    // replaces, for `patch/sheet.tsx`'s reason: a line naming a
                    // thing is a statement about what the selection is.
                    pick(shell, objectLine({ t: "Group", groupId: row.id }), () => {
                      run(`+ Group ${String(row.id)}`);
                    });
                  }}
                  onContextMenu={openMenu(row.id)}
                >
                  <span className="pool-number">{row.id}</span>
                  <span className="pool-name">{row.name === "" ? "—" : row.name}</span>
                  <span className="pool-note">{row.fixtures} fixtures</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      {menu !== null && over !== undefined ? (
        <GroupMenu at={menu} group={over} groups={rows} onClose={closeMenu} />
      ) : null}
    </div>
  );
}

/**
 * The menu over one group.
 *
 * No colour item: `Group` has no colour field and `Command::Color` refuses
 * anything but a sequence and a preset (`ShowError::NotColourable`), so an item
 * here would be a control with nothing behind it.
 */
function GroupMenu({
  at,
  group,
  groups,
  onClose,
}: {
  readonly at: { readonly x: number; readonly y: number };
  readonly group: GroupRow;
  readonly groups: readonly GroupRow[];
  readonly onClose: () => void;
}) {
  const { run, write } = useConsole();
  const free = nextFreeNumber(groups);
  return (
    <ContextMenu
      at={at}
      title={`Group ${String(group.id)} · ${group.name === "" ? "—" : group.name}`}
      label={`Manage group ${String(group.id)}`}
      testId="group-menu"
      subject={String(group.id)}
      onClose={onClose}
    >
      <MenuField
        testId="group-rename"
        label="Rename…"
        verb="Rename"
        initial={group.name}
        onClose={onClose}
        onSubmit={(text) => {
          run(`Label Group ${String(group.id)} ${JSON.stringify(text)}`);
        }}
      />
      <MenuItem
        testId="group-copy"
        onClose={onClose}
        title={`Copy this group to group ${String(free)}`}
        onChoose={() => {
          run(`Copy Group ${String(group.id)} Group ${String(free)}`);
        }}
      >
        Copy to group {free}
      </MenuItem>
      <MenuItem
        testId="group-move"
        onClose={onClose}
        // **Written and left standing**, not submitted: the destination is the
        // argument the line is still waiting for, and the operator types it —
        // §4.5's second shape.
        title="Write a move line for this group, and finish it in the command line"
        onChoose={() => {
          write(`Move Group ${String(group.id)} Group `);
        }}
      >
        Move to…
      </MenuItem>
      <MenuItem
        testId="group-delete"
        onClose={onClose}
        danger
        onChoose={() => {
          run(`Delete Group ${String(group.id)}`);
        }}
      >
        Delete
      </MenuItem>
    </ContextMenu>
  );
}
