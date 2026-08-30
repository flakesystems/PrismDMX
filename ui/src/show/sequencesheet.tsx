/**
 * The Sequence Sheet: the cue lists, as a pool of boxes.
 *
 * # What the three look windows are for — and how S43 split them again
 *
 * S27 settled what the three *light* windows are (Patch, Fixture Sheet, DMX
 * Sheet); S28 wrote the same paragraph for the three **look** windows, and the
 * owner's rebuild moved the line between two of them:
 *
 * - **Sequence Sheet** — the *pool*. Which cue lists there are, which one is
 *   being edited, and nothing else. It is a **selection surface**: press a box
 *   to work on that list, right-click one to manage it.
 * - **Cue Viewer** — the *list*. Its cues, one row each, with a column per
 *   attribute; its transport; and the store bar. Every cue edit is there.
 * - **Preset Pool** — the *pools*. Named looks per pool, applied and stored,
 *   with the colour the scribble strips use.
 *
 * The owner's words for the move: *das Sequence Sheet soll die Sequences in
 * einem Grid anzeigen, ohne Details wie Cues, Executor etc. und ohne
 * Eingabefelder zum storen. Alle Cue Editierungen sollen im Cue Viewer gemacht
 * werden.*
 *
 * It is the right line. The cue table, the executor transport and the store bar
 * were three panels stacked under a strip of chips, in one window, and the
 * window was named after the strip. Everything below the strip was about **one**
 * sequence; everything in the strip was about *which*. Those are two windows,
 * and an operator who wants both can open both — which is what a canvas is for.
 *
 * # The smaller actions are on a right-click, and that is the rule now
 *
 * *Namens und Farbänderungen oder Ähnliches sollen keinen eigenen Knopf
 * bekommen, sondern mit Rechtsklick auf eine Sequence erreichbar sein.* So a box
 * carries the number, the name and how many cues, and rename, colour, copy, move
 * and delete come off `chrome/menu.tsx` — the same component the view bar has
 * used since S35, and the same one the group and preset pools use.
 *
 * # The sequence in force is the desk's
 *
 * **S28's marked assumption is settled.** §4.1 had no *selected sequence* and
 * this window used the selected executor's, with the assumption written down
 * rather than made permanent; §4.4 named S39 as the session that would decide,
 * and S39 gave the session a field of its own. So which cue list is being edited
 * is `Session::selectedSequence` and choosing one is a `Command::SelectSequence`
 * — a command out, a `SessionPatch` back, and not a local selection two screens
 * could disagree about.
 *
 * # Every key here writes a line
 *
 * `ARCHITECTURE_SPEC.md` §4.5. Pressing a box writes `Sequence 3`; the menu
 * writes `Label Sequence 3 "…"`, `Color Sequence 3 …`, `Copy Sequence 3
 * Sequence 4`, `Move …` and `Delete Sequence 3`. Each is a line an operator
 * could have typed, and each is the same line the console produces.
 */

import { useCallback, useMemo } from "react";

import type { JsonValue } from "../bindings";
import { ContextMenu, MenuField, MenuItem } from "../chrome/menu";
import { useMenuAt } from "../chrome/menuat";
import { objectLine, pick, useConsole } from "../desk/consoleshell";
import type { SequenceRow } from "./looks";
import { colorStyle, nextFreeNumber, sequenceInForce, sequenceRows } from "./looks";

/** The whole window. */
export function SequenceSheet({
  show,
  session,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
}) {
  const shell = useConsole();
  const { run } = shell;
  const sequences = useMemo(() => sequenceRows(show), [show]);
  const chosen = useMemo(() => sequenceInForce(session), [session]);
  const present = useCallback(
    (id: number) => sequences.some((row) => row.id === id),
    [sequences],
  );
  const { menu, openMenu, closeMenu } = useMenuAt(present);
  const over = menu === null ? undefined : sequences.find((row) => row.id === menu.subject);

  const create = useCallback(() => {
    // `Store Sequence n` on a free number makes the cue list — S40 folded
    // `CreateSequence` into it, because the command line cannot know which of
    // the two acts it is (the parser does not read the show, S26).
    run(`Store Sequence ${String(nextFreeNumber(sequences))}`);
  }, [run, sequences]);

  return (
    <div className="looks" data-testid="sequence-sheet">
      <div className="looks-bar">
        <span className="looks-count" data-testid="sequence-count">
          {sequences.length} sequences
        </span>
        <button type="button" data-testid="new-sequence" onClick={create}>
          New sequence
        </button>
      </div>
      {sequences.length === 0 ? (
        <p className="window-note" data-testid="no-sequence">
          There are no cue lists. Make one — a cue list does not need a fader, and what goes into
          it is stored from the Cue Viewer.
        </p>
      ) : (
        <div className="sheet-scroll" data-testid="sequence-scroll">
          <ul className="pool-grid">
            {sequences.map((row) => (
              <li key={row.id}>
                <button
                  type="button"
                  className={`pool-box${row.id === chosen ? " pool-box-current" : ""}`}
                  data-testid={`sequence-${String(row.id)}`}
                  data-current={row.id === chosen ? "yes" : "no"}
                  title={`Edit ${row.name}. Right-click to manage.`}
                  onClick={() => {
                    // **An argument when the line is waiting for one, and a
                    // selection otherwise** — see `consoleshell.ts::pickOnto`.
                    // Type `Store`, click here, and the line goes as
                    // `Store Sequence 2`; with nothing typed the click chooses
                    // the list, which needs no executor since S39 and is half
                    // the reason the session field exists.
                    pick(shell, objectLine({ t: "Sequence", sequenceId: row.id }), () => {
                      run(`Sequence ${String(row.id)}`);
                    });
                  }}
                  onContextMenu={openMenu(row.id)}
                >
                  <Swatch color={colorStyle(row.color)} testId={`sequence-swatch-${String(row.id)}`} />
                  <span className="pool-number">{row.id}</span>
                  <span className="pool-name">{row.name === "" ? "—" : row.name}</span>
                  <span className="pool-note">{row.cues.length} cues</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      {menu !== null && over !== undefined ? (
        <SequenceMenu at={menu} sequence={over} sequences={sequences} onClose={closeMenu} />
      ) : null}
    </div>
  );
}

/** The colour a scribble strip would show, or the empty frame for none. */
export function Swatch({
  color,
  testId,
}: {
  readonly color: string | null;
  readonly testId: string;
}) {
  return (
    <span
      className="preset-swatch"
      data-testid={testId}
      data-color={color ?? ""}
      style={color === null ? undefined : { background: color }}
    />
  );
}

/**
 * The menu over one cue list.
 *
 * Every item is a line, and every line is one S40 gave to all six pools. There
 * is no *looping* item and that is not an oversight: `Sequence::loop` is on the
 * wire and no command sets it, so an item here would be a control with nothing
 * behind it. It is written down in `PROGRESS.md` §7 rather than drawn.
 */
function SequenceMenu({
  at,
  sequence,
  sequences,
  onClose,
}: {
  readonly at: { readonly x: number; readonly y: number };
  readonly sequence: SequenceRow;
  readonly sequences: readonly SequenceRow[];
  readonly onClose: () => void;
}) {
  const { run, write } = useConsole();
  const free = nextFreeNumber(sequences);
  return (
    <ContextMenu
      at={at}
      title={`Sequence ${String(sequence.id)} · ${sequence.name === "" ? "—" : sequence.name}`}
      label={`Manage sequence ${String(sequence.id)}`}
      testId="sequence-menu"
      subject={String(sequence.id)}
      onClose={onClose}
    >
      <MenuField
        testId="sequence-rename"
        label="Rename…"
        verb="Rename"
        initial={sequence.name}
        onClose={onClose}
        onSubmit={(text) => {
          run(`Label Sequence ${String(sequence.id)} ${JSON.stringify(text)}`);
        }}
      />
      <MenuField
        testId="sequence-colour"
        label="Colour…"
        verb="Colour"
        initial=""
        onClose={onClose}
        onSubmit={(text) => {
          // The colour words and the hex form are the console's own
          // (`prism_core::console`), and an empty answer takes the colour off —
          // which is `Label`'s rule one verb along. Writing the line rather
          // than sending `Command::Color` is §4.5: what a picker would send,
          // an operator can also type.
          run(`Color Sequence ${String(sequence.id)} ${text.trim() === "" ? "none" : text.trim()}`);
        }}
      />
      <MenuItem
        testId="sequence-copy"
        onClose={onClose}
        title={`Copy this cue list to sequence ${String(free)}`}
        onChoose={() => {
          run(`Copy Sequence ${String(sequence.id)} Sequence ${String(free)}`);
        }}
      >
        Copy to sequence {free}
      </MenuItem>
      <MenuItem
        testId="sequence-move"
        onClose={onClose}
        // **Written and left standing**, not submitted: the destination is the
        // argument the line is still waiting for, and the operator types it.
        // That is §4.5's second shape, and it is why this item opens no box of
        // its own.
        title="Write a move line for this cue list, and finish it in the command line"
        onChoose={() => {
          write(`Move Sequence ${String(sequence.id)} Sequence `);
        }}
      >
        Move to…
      </MenuItem>
      <MenuItem
        testId="sequence-delete"
        onClose={onClose}
        danger
        onChoose={() => {
          run(`Delete Sequence ${String(sequence.id)}`);
        }}
      >
        Delete
      </MenuItem>
    </ContextMenu>
  );
}
