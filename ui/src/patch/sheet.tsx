/**
 * The Fixture Sheet: what every patched fixture is being told to do, live.
 *
 * # One live column, since S43
 *
 * The **programmer** column is what the operator has laid on top — sparse,
 * absolute priority, and *absent is not zero* (`prism_core::programmer`), so an
 * untouched attribute reads as a dash and never as 0 %.
 *
 * There used to be a second: a canvas drawn over the last column showing what
 * was actually on the cable, thirty times a second, outside React (S27's
 * `./live.ts`). **Punch-list B8 took it out.** The owner's words were that its
 * scaling was wrong and that the table beside it was enough, and both halves are
 * right — a bar whose length did not mean what it looked like was worse than no
 * bar, and *what is on the cable* is what the `DMX Sheet` window is for
 * (`telemetry/panel.tsx`), channel by channel and with no fixtures in the way.
 *
 * `./live.ts` is **kept**, and that is deliberate rather than an oversight: it is
 * the pattern S27 recorded for any later view with per-row live values — a
 * canvas the size of the window, `scrollTop` read off the container each frame,
 * only the visible rows drawn — and it is covered by its own tests. What was
 * removed is this sheet's use of it.
 *
 * # Which attributes are shown
 *
 * The ones on the encoder bank in force, which is `session.encoderBank` — so the
 * sheet follows the bank buttons and the console's Encoder Assign keys, and an
 * operator working in Position sees pan and tilt rather than fifteen columns of
 * everything.
 *
 * **Since S52 those are the ones the selection actually has**, repeats
 * numbered: `desk/programmer.ts`'s `bankParameters`, which is the same call the
 * encoder band draws from and the same rule the jog wheel walks
 * (`prism_core::Programmer::bank_parameters`). A sheet with a column nothing on
 * it could ever hold is a column of blanks, and this sheet is where columns are
 * expensive.
 *
 * # Selecting — S43, B16 and B17
 *
 * **The whole row is the target**, not the number in its first cell. An operator
 * pointing at a fixture points at the fixture, and a four-pixel-wide number was
 * a target you had to aim at.
 *
 * **And a click adds to the selection rather than replacing it** (B17), which is
 * how a console works: you pick the four lamps you want and then set them.
 * `SelectionMode::Toggle` is what a row sends, so clicking a selected row takes
 * it back out again; the way to start over is **Clear**, which is the owner's
 * answer and the one the punch list gives in as many words.
 *
 * That is a difference between the pointer and the line, and it is a deliberate
 * one: a typed `Fixture 5` still *replaces*, because a line naming a fixture is
 * a statement about what the selection is. The rows write
 * `Fixture 5 + ` — the grammar's own word for *and this one too* — so the
 * gesture and the line are still the same thing, which is §4.5's whole point.
 *
 * # Scrolling
 *
 * `CLAUDE.md` forbids scrolling *outside* the canvas. A rig of four hundred
 * fixtures scrolls **inside this window**, which is what a window is for.
 */

import { useMemo } from "react";

import type { JsonValue, ProgrammerState } from "../bindings";
import type { ParameterKey } from "../desk/programmer";
import { bankParametersOf, groupOf, parameterLabel, valueFor } from "../desk/programmer";
import { encoderBank, programmerOccurrence } from "../desk/session";
import { pick, useConsole } from "../desk/consoleshell";
import { percentOfLevel } from "../desk/level";
import { ROW_HEIGHT } from "./live";
import { patchRows } from "./patch";

/** The whole sheet. */
export function FixtureSheet({
    show,
    session,
    programmer,
}: {
    readonly show: JsonValue;
    readonly session: JsonValue;
    readonly programmer: ProgrammerState | null;
}) {
    const rows = useMemo(() => patchRows(show), [show]);
    const bank = encoderBank(session);
    // **The rows and not the selection** — S52. This sheet draws a row per
    // patched fixture, so its columns are what *those* fixtures have; the
    // encoder band asks the same question about the selection instead.
    const attributes = bankParametersOf(
        show,
        rows.map((row) => row.id),
        bank,
        programmerOccurrence(session),
    );
    const selected = new Set(programmer?.selection ?? []);

    if (rows.length === 0) {
        return <p className="window-note">Nothing is patched. Build a rig in the Patch window.</p>;
    }
    return (
        <div className="fixture-sheet" data-testid="fixture-sheet">
            <p className="sheet-bank" data-testid="sheet-bank">
                {bank} — what the programmer holds. Click a row to add it to the selection.
            </p>
            <SheetBody
                show={show}
                rows={rows.map((row) => ({ id: row.id, name: row.name }))}
                attributes={attributes}
                programmer={programmer}
                selected={selected}
            />
        </div>
    );
}

/** One row's identity, which is all the table needs from the patch. */
interface SheetRow {
    /** The fixture number. */
    readonly id: number;
    /** What it is called. */
    readonly name: string;
}

/** The scrolling half: the table. */
function SheetBody({
    show,
    rows,
    attributes,
    programmer,
    selected,
}: {
    readonly show: JsonValue;
    readonly rows: readonly SheetRow[];
    readonly attributes: readonly ParameterKey[];
    readonly programmer: ProgrammerState | null;
    readonly selected: ReadonlySet<number>;
}) {
    const shell = useConsole();
    const { run } = shell;
    // **An argument when the line is waiting for one** — S43's second rebuild,
    // `consoleshell.ts::pickOnto`. A line the fixture cannot go into answers
    // `own`, and the row does what it has always done: `Store Fixture 5` is not
    // a line, so a click with `Store` standing still selects.
    const onPick = (id: number): void => {
        pick(shell, `Fixture ${String(id)}`, () => {
            run(`+ Fixture ${String(id)}`);
        });
    };

    return (
        <div className="sheet-live">
            <div className="sheet-scroll" data-testid="sheet-scroll">
                <table className="sheet">
                    <thead>
                        <tr>
                            <th scope="col">Fx</th>
                            <th scope="col">Name</th>
                            {attributes.map((key) => (
                                <th scope="col" key={columnKey(key)}>
                                    {parameterLabel(key)}
                                </th>
                            ))}
                        </tr>
                    </thead>
                    <tbody>
                        {rows.map((row) => (
                            /*
                              **The whole row is the pick** — S43, B16. It writes
                              `+ Fixture 12` and submits: the pointer has supplied
                              the argument the line was waiting for, and the
                              leading `+` is the grammar's own word for *and this
                              one too* (B17). Clicking a selected row writes the
                              same line, which takes it back out — `Toggle`.
                            */
                            <tr
                                key={row.id}
                                style={{ height: `${String(ROW_HEIGHT)}px` }}
                                className={`sheet-pick${selected.has(row.id) ? " row-selected" : ""}`}
                                data-testid={`sheet-row-${String(row.id)}`}
                                data-selected={selected.has(row.id) ? "yes" : "no"}
                                title={
                                    selected.has(row.id)
                                        ? `Take fixture ${String(row.id)} out of the selection`
                                        : `Add fixture ${String(row.id)} to the selection`
                                }
                                // A row is not a button, so it needs both: a
                                // pointer gesture and a keyboard one. A console
                                // is operated in the dark and Tab has to reach
                                // every fixture in the sheet.
                                tabIndex={0}
                                role="button"
                                aria-pressed={selected.has(row.id)}
                                onClick={() => {
                                    onPick(row.id);
                                }}
                                onKeyDown={(event) => {
                                    if (event.key === "Enter" || event.key === " ") {
                                        event.preventDefault();
                                        onPick(row.id);
                                    }
                                }}
                            >
                                <td data-testid={`sheet-select-${String(row.id)}`}>{row.id}</td>
                                <td>{row.name === "" ? "—" : row.name}</td>
                                {attributes.map((key) => (
                                    <td
                                        key={columnKey(key)}
                                        data-testid={`prog-${String(row.id)}-${columnKey(key)}`}
                                    >
                                        {programmerText(show, programmer, row.id, key)}
                                    </td>
                                ))}
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
        </div>
    );
}

/**
 * What the programmer holds for one fixture and attribute.
 *
 * Three answers, and they are three different facts. A dash: the programmer is
 * **sparse** and this attribute is absent, so the playbacks decide it — which is
 * not the same thing as 0 %, and an interface that showed them alike would teach
 * an operator that they are. A blank: the fixture's profile has no such
 * attribute at all, so there is nothing there to hold. Otherwise the percentage.
 */
function programmerText(
    show: JsonValue,
    programmer: ProgrammerState | null,
    fixture: number,
    key: ParameterKey,
): string {
    if (groupOf(show, fixture, key) === null) {
        return "";
    }
    const value = valueFor(programmer, fixture, key);
    return value === null ? "—" : `${String(percentOfLevel(value))}%`;
}

/**
 * What one column is called in the DOM — S52.
 *
 * The attribute alone for the first of a kind, so every test id written before
 * a fixture could have two colour wheels still names the same cell.
 */
function columnKey(key: ParameterKey): string {
    return key.occurrence === 0 ? key.attribute : `${key.attribute}-${String(key.occurrence + 1)}`;
}
