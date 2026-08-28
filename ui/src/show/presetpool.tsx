/**
 * The Preset Pools: named looks per feature group, applied and stored.
 *
 * # A pool is a filter, not a namespace
 *
 * Preset numbers are unique **across** pools, not within them
 * (`prism_core::Show::store_preset`): `Command::ApplyPreset` carries a number
 * and no pool, so a number that meant one thing in Colour and another in
 * Position would make that command ambiguous. The pool is how a preset is filed
 * and which bank shows it; it is not part of its identity. So the tabs here are
 * a filter over one list, and a new preset takes the lowest number **nothing**
 * is filed under.
 *
 * # Multi — the eighth tab, and the one that is not a bank
 *
 * S43, the owner's rebuild: *es kommt allerdings noch ein Multi Preset hinzu,
 * welches Kategorie übergreifend funktioniert*. A Multi preset takes **every**
 * value the programmer holds rather than one bank's, so one number recalls a
 * whole look — colour, position and beam together — which is the preset an
 * operator files a finished state in rather than one ingredient of it.
 *
 * It is a `PresetPool` and not an eighth `FeatureGroup`, because there is no
 * encoder bank that could be *Multi* and no attribute that belongs to one. See
 * `prism_domain::PresetPool`; the daemon needed no new filtering for it, since
 * *no pool named* is the filter a **cue** has taken since S28.
 *
 * # A grid, and the smaller actions on a right-click
 *
 * *Alle kleineren Group bzw. Preset bezogenen Aktionen sollen über Rechtsklick
 * ausgeführt werden.* So a box is the preset and nothing else, and rename,
 * colour, copy, move and delete come off `chrome/menu.tsx`. The **colour** item
 * is new in more than one sense: `Preset::color` has been on the wire since S11
 * and nothing could set it — `Command::Color` refused anything but a sequence
 * until S43 — so the scribble strips showed a colour no operator could choose.
 *
 * # The store bar is gone, and storing is the command line's
 *
 * The owner's second rebuild, and `grouppool.tsx` has the argument. One thing is
 * specific to this window and worth saying: the bar carried a
 * `Query::StorePreview` and put the daemon's own sentence on the button — *two
 * added, three replaced* — which is a real reading and not decoration. It is
 * not lost. `Store Preset 1` typed, or `Store Preset` and a click, raises the
 * console's **prompt** (S39/S40), and that prompt is the same question answered
 * by the same query. What went is the panel, not the answer.
 *
 * The **pool** a stored preset lands in is then `Session::encoderBank` rather
 * than the tab being looked at (`Command::StorePreset`), which is the bank under
 * the operator's hands. A line that wants another says so: `Store Preset 3 Beam`.
 *
 * # Which values a store takes is the daemon's answer
 *
 * A colour preset stores the colour values of the programmer and leaves the
 * position alone — and which bank an attribute is on is the *profile's* answer
 * (`AttributeDef::featureGroup`) rather than the attribute name's, which is why
 * `Command::StorePreset` carries the pool and the daemon does the filtering. The
 * count on the Store button is `Query::StorePreview`'s, for the same reason.
 *
 * # Nothing here is state this interface holds
 *
 * The boxes are `poolRows(show, pool)`. Applying a preset is a line — `Preset 3`
 * — written and submitted, because the pointer has supplied the argument
 * (`ARCHITECTURE_SPEC.md` §4.5). What is local is the number and the name being
 * typed, which is §4.2's category: a half-finished edit is not something a
 * second operator's screen should follow.
 *
 * **The pool tab is client-local** (S40, and still). Which of eight tabs one
 * screen is looking at is `ARCHITECTURE_SPEC.md` §4.2's own category — two
 * operators on two screens legitimately want different ones — and it is a
 * *filter*, not a destination: what pool a store writes into is
 * `Session::encoderBank` unless the line names one, which is
 * `Command::StorePreset`'s own rule and the same answer wherever the line is
 * typed from.
 */

import { useCallback, useMemo, useState } from "react";

import type { JsonValue, PresetPool as Pool } from "../bindings";
import { PRESET_POOL_VARIANTS } from "../bindings/variants";
import { ContextMenu, MenuField, MenuItem } from "../chrome/menu";
import { useMenuAt } from "../chrome/menuat";
import { objectLine, pick, useConsole } from "../desk/consoleshell";
import type { PresetRow } from "./looks";
import { colorStyle, nextFreeNumber, poolRows, presetRows } from "./looks";
import { Swatch } from "./sequencesheet";

/** The whole window. */
export function PresetPool({ show }: { readonly show: JsonValue }) {
  const shell = useConsole();
  const { run } = shell;
  const [pool, setPool] = useState<Pool>("Color");
  const all = useMemo(() => presetRows(show), [show]);
  const rows = useMemo(() => poolRows(show, pool), [show, pool]);
  const present = useCallback((id: number) => all.some((row) => row.id === id), [all]);
  const { menu, openMenu, closeMenu } = useMenuAt(present);
  const over = menu === null ? undefined : all.find((row) => row.id === menu.subject);

  return (
    <div className="looks" data-testid="preset-pool">
      <div className="looks-bar">
        <span className="looks-count" data-testid="preset-count">
          {rows.length} of {all.length} presets
        </span>
        {PRESET_POOL_VARIANTS.map((group) => (
          <button
            key={group}
            type="button"
            className={`pool-tab${group === pool ? " pool-tab-current" : ""}`}
            data-testid={`pool-${group}`}
            data-current={group === pool ? "yes" : "no"}
            // Client-local, and §4.2's category: which of eight tabs one screen
            // is looking at is not something a second screen should follow.
            onClick={() => {
              setPool(group);
            }}
          >
            {group}
          </button>
        ))}
      </div>
      {rows.length === 0 ? (
        <p className="window-note" data-testid="pool-empty">
          The {pool} pool is empty. Put a look in the programmer and type{" "}
          <code>Store Preset 1{pool === "Multi" ? " Multi" : ""}</code>;{" "}
          {pool === "Multi"
            ? "everything the programmer holds goes in, across the categories."
            : "only the values of that pool go in."}
        </p>
      ) : (
        <div className="sheet-scroll" data-testid="pool-scroll">
          <ul className="pool-grid">
            {rows.map((row) => (
              <li key={row.id}>
                <button
                  type="button"
                  className="pool-box"
                  data-testid={`preset-${String(row.id)}`}
                  title={`Apply preset ${String(row.id)} to the selection. Right-click to manage.`}
                  onClick={() => {
                    // **An argument when the line is waiting for one** —
                    // `consoleshell.ts::pickOnto`. `Store` and a click here is
                    // `Store Preset 1`, which is how a preset is made now that
                    // this window has no store bar of its own; the pool it goes
                    // into is `Session::encoderBank`, which is the bank under
                    // the operator's hands (`Command::StorePreset`).
                    //
                    // With nothing typed it is a list pick: the line is written
                    // and submitted at once, because the pointer has supplied
                    // the argument it was waiting for (§4.5).
                    pick(shell, objectLine({ t: "Preset", presetId: row.id }), () => {
                      run(`Preset ${String(row.id)}`);
                    });
                  }}
                  onContextMenu={openMenu(row.id)}
                >
                  <Swatch
                    color={colorStyle(row.color)}
                    testId={`preset-swatch-${String(row.id)}`}
                  />
                  <span className="pool-number">{row.id}</span>
                  <span className="pool-name">{row.name === "" ? "—" : row.name}</span>
                  <span className="pool-note">{row.values} values</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      {menu !== null && over !== undefined ? (
        <PresetMenu at={menu} preset={over} presets={all} onClose={closeMenu} />
      ) : null}
    </div>
  );
}

/** The menu over one preset. */
function PresetMenu({
  at,
  preset,
  presets,
  onClose,
}: {
  readonly at: { readonly x: number; readonly y: number };
  readonly preset: PresetRow;
  readonly presets: readonly PresetRow[];
  readonly onClose: () => void;
}) {
  const { run, write } = useConsole();
  const free = nextFreeNumber(presets);
  return (
    <ContextMenu
      at={at}
      title={`Preset ${String(preset.id)} · ${preset.name === "" ? "—" : preset.name}`}
      label={`Manage preset ${String(preset.id)}`}
      testId="preset-menu"
      subject={String(preset.id)}
      onClose={onClose}
    >
      <MenuField
        testId="preset-rename"
        label="Rename…"
        verb="Rename"
        initial={preset.name}
        onClose={onClose}
        onSubmit={(text) => {
          run(`Label Preset ${String(preset.id)} ${JSON.stringify(text)}`);
        }}
      />
      <MenuField
        testId="preset-colour"
        label="Colour…"
        verb="Colour"
        initial=""
        onClose={onClose}
        onSubmit={(text) => {
          // The colour words and the hex form are the console's own
          // (`desk/console.ts`), and an empty answer takes the colour off —
          // `Label`'s rule one verb along. It writes **only** the colour: a
          // store is what changes a preset's values, and a colour that took the
          // programmer with it would make an operator choose between
          // re-colouring a preset and keeping what is in it.
          run(`Color Preset ${String(preset.id)} ${text.trim() === "" ? "none" : text.trim()}`);
        }}
      />
      <MenuItem
        testId="preset-copy"
        onClose={onClose}
        title={`Copy this preset to preset ${String(free)}`}
        onChoose={() => {
          run(`Copy Preset ${String(preset.id)} Preset ${String(free)}`);
        }}
      >
        Copy to preset {free}
      </MenuItem>
      <MenuItem
        testId="preset-move"
        onClose={onClose}
        title="Write a move line for this preset, and finish it in the command line"
        onChoose={() => {
          write(`Move Preset ${String(preset.id)} Preset `);
        }}
      >
        Move to…
      </MenuItem>
      <MenuItem
        testId="preset-delete"
        onClose={onClose}
        danger
        onChoose={() => {
          run(`Delete Preset ${String(preset.id)}`);
        }}
      >
        Delete
      </MenuItem>
    </ContextMenu>
  );
}
