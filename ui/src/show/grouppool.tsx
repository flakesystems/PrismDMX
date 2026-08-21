/**
 * The Group Pool: the fixture groups, selected by pointer or by typing.
 *
 * # Why this window exists at all
 *
 * `prism_core::Show::store_group` has been in the daemon since S11 with **no
 * command in front of it** — `PROGRESS.md` §7 carried that out of S28 — so a
 * group could be read, selected and merged and never made. S40 gave it
 * `Command::StoreGroup`, and this is where an operator reaches it: a pool of
 * boxes, a store bar and nothing else. Until now the `Groups` window drew a
 * plain list of names, which is what a window looks like before the commands
 * behind it exist.
 *
 * # Every key here writes a line
 *
 * `ARCHITECTURE_SPEC.md` §4.5. Clicking a box writes `Group 3` and submits it,
 * because the pointer has supplied the argument the line was waiting for; the
 * store bar writes `Store Group 4 "Front wash"`; the two keys on a box write
 * `Label Group 3 …` and `Delete Group 3`. Each of them is a line the operator
 * could have typed, and each of them is the same line the console produces.
 *
 * # A group is a list of fixtures, not a look
 *
 * That is what makes `Group 3` a *selection* rather than a set of values, and it
 * is why the store takes the programmer's **selection** and nothing else
 * (`prism_core::Programmer::group`). The look over the same fixtures is a
 * preset, which is the window next door.
 */

import { useCallback, useMemo, useState } from "react";

import type { JsonValue } from "../bindings";
import { useConsole } from "../desk/consoleshell";
import { groupRows } from "./looks";
import type { GroupRow } from "./looks";

/** The lowest number nothing is filed under. */
function nextFreeGroup(rows: readonly GroupRow[]): number {
  let candidate = 1;
  for (const row of rows) {
    if (row.id === candidate) {
      candidate += 1;
    }
  }
  return candidate;
}

/** The whole window. */
export function GroupPool({ show }: { readonly show: JsonValue }) {
  const rows = useMemo(() => groupRows(show), [show]);
  const { run } = useConsole();

  const select = useCallback(
    (groupId: number) => {
      run(`Group ${String(groupId)}`);
    },
    [run],
  );

  return (
    <div className="looks" data-testid="group-pool">
      <div className="looks-bar">
        <span className="looks-count" data-testid="group-count">
          {rows.length} groups
        </span>
      </div>
      {rows.length === 0 ? (
        <p className="window-note" data-testid="group-empty">
          There are no groups. Select some fixtures and store one below — a group is a list of
          fixtures, so it is the selection that goes in.
        </p>
      ) : (
        <div className="sheet-scroll" data-testid="group-scroll">
          <ul className="pool-grid">
            {rows.map((row) => (
              <GroupBox key={row.id} group={row} onSelect={select} />
            ))}
          </ul>
        </div>
      )}
      <GroupStoreBar rows={rows} />
    </div>
  );
}

/** One box: the number, the name, how many fixtures, and two keys. */
function GroupBox({
  group,
  onSelect,
}: {
  readonly group: GroupRow;
  readonly onSelect: (groupId: number) => void;
}) {
  const { write, run } = useConsole();
  return (
    <li>
      <button
        type="button"
        className="preset-box"
        data-testid={`group-${String(group.id)}`}
        title={`Select the fixtures of group ${String(group.id)}`}
        onClick={() => {
          onSelect(group.id);
        }}
      >
        <span className="pool-number">{group.id}</span>
        <span className="pool-name">{group.name === "" ? "—" : group.name}</span>
        <span className="pool-note">{group.fixtures} fixtures</span>
      </button>
      <span className="pool-keys">
        <button
          type="button"
          className="linkish"
          data-testid={`group-label-${String(group.id)}`}
          aria-label={`Label group ${String(group.id)}`}
          // **Written and left standing**, not submitted: the name is the
          // argument the line is still waiting for, and the operator types it.
          // That is §4.5's second shape, and it is why this key does not open a
          // box of its own.
          title="Write a label line for this group"
          onClick={() => {
            write(`Label Group ${String(group.id)} ${JSON.stringify(group.name)}`);
          }}
        >
          Label
        </button>
        <button
          type="button"
          className="linkish"
          data-testid={`group-delete-${String(group.id)}`}
          aria-label={`Delete group ${String(group.id)}`}
          onClick={() => {
            run(`Delete Group ${String(group.id)}`);
          }}
        >
          ×
        </button>
      </span>
    </li>
  );
}

/**
 * The store bar: a number, a name and the mode.
 *
 * There is no preview here and that is not an omission. `Query::StorePreview`
 * counts *values* (S28), and a group holds fixtures rather than values — the
 * question a group store raises is only *is something already filed under that
 * number*, which the mode answers. See `desk/exists.ts` for why reading that
 * from the mirror is honest and reading the counts from it would not be.
 */
function GroupStoreBar({ rows }: { readonly rows: readonly GroupRow[] }) {
  const { runWithMode } = useConsole();
  const [number, setNumber] = useState<number | null>(null);
  const [name, setName] = useState<string | null>(null);
  const [mode, setMode] = useState<"Merge" | "Override">("Merge");
  const groupId = number ?? nextFreeGroup(rows);
  const existing = rows.find((row) => row.id === groupId);
  const wantedName = name ?? existing?.name ?? `Group ${String(groupId)}`;

  return (
    <form
      className="store-bar"
      data-testid="group-store"
      onSubmit={(event) => {
        event.preventDefault();
        runWithMode(`Store Group ${String(groupId)} ${JSON.stringify(wantedName)}`, mode);
        setNumber(null);
        setName(null);
      }}
    >
      <label>
        Group
        <input
          className="cell-input cell-input-narrow"
          data-testid="group-number"
          inputMode="numeric"
          value={String(groupId)}
          onChange={(event) => {
            const typed = Number(event.target.value.trim());
            if (event.target.value.trim() !== "" && Number.isInteger(typed) && typed > 0) {
              setNumber(typed);
              // The name follows the number until somebody types one: moving to
              // a group that exists should offer *its* name, not the last one.
              setName(null);
            }
          }}
        />
      </label>
      <label>
        Name
        <input
          className="cell-input"
          data-testid="group-name"
          value={wantedName}
          onChange={(event) => {
            setName(event.target.value);
          }}
        />
      </label>
      <label>
        Mode
        <select
          data-testid="group-store-mode"
          value={mode}
          onChange={(event) => {
            setMode(event.target.value === "Override" ? "Override" : "Merge");
          }}
        >
          <option value="Merge">Merge</option>
          <option value="Override">Override</option>
        </select>
      </label>
      <button type="submit" data-testid="store-group">
        {existing === undefined
          ? `Store group ${String(groupId)}`
          : `${mode} into group ${String(groupId)}`}
      </button>
    </form>
  );
}
