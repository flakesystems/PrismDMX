/**
 * The Fixture Sheet: what every patched fixture is being told to do, live.
 *
 * # Two live columns, and they are allowed to disagree
 *
 * That disagreement is the reason both are here. The **programmer** column is
 * what the operator has laid on top — sparse, absolute priority, and *absent is
 * not zero* (`prism_core::programmer`), so an untouched attribute reads as a
 * dash and never as 0 %. The **output** column is what is actually on the cable
 * after the merge, the masters and the encoding. A fixture with nothing in the
 * programmer and a level on the cable is a playback running; one with a
 * programmer value and nothing on the cable is a grand master at zero, or a
 * universe with no output configured at all.
 *
 * # And only one of them may be React
 *
 * `docs/IPC_PROTOCOL.md` §7, last paragraph. The programmer arrives as
 * `Delta::ProgrammerChanged` at the rate a human turns a knob and renders like
 * every other reader. The output arrives thirty times a second and is drawn on a
 * canvas by `./live.ts`, outside React entirely — `telemetry/render.test.tsx`
 * counts React commits over 300 frames and must stay at zero, so a sheet that
 * put levels in a `useState` would turn that test red.
 *
 * # Which attributes are shown
 *
 * The ones on the encoder bank in force, which is `session.encoderBank` — so the
 * sheet follows the bank buttons and the console's Encoder Assign keys, and an
 * operator working in Position sees pan and tilt rather than fifteen columns of
 * everything. The list is `FEATURE_GROUP_ATTRIBUTES`, generated from
 * `prism_domain::FeatureGroup::attributes`, which is the same table the encoder
 * bar and the jog wheel walk (S26).
 *
 * # Scrolling
 *
 * `CLAUDE.md` forbids scrolling *outside* the canvas. A rig of four hundred
 * fixtures scrolls **inside this window**, which is what a window is for, and
 * the output column is drawn only for the rows that are on the screen.
 */

import { useEffect, useMemo, useRef } from "react";

import type { AttributeType, JsonValue, ProgrammerState } from "../bindings";
import { bankParameters, groupOf, valueFor } from "../desk/programmer";
import { encoderBank } from "../desk/session";
import { percentOfLevel } from "../desk/level";
import { useTelemetryChannel } from "../telemetry/context";
import { canvasSurface, devicePixelRatio, resizeCanvas } from "../telemetry/painter";
import type { LiveFixture } from "./live";
import { ROW_HEIGHT, driveFixtureLevels, liveFixtures } from "./live";
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
    const fixtures = useMemo(() => liveFixtures(show), [show]);
    const bank = encoderBank(session);
    const attributes = bankParameters(bank);
    const selected = new Set(programmer?.selection ?? []);

    if (rows.length === 0) {
        return <p className="window-note">Nothing is patched. Build a rig in the Patch window.</p>;
    }
    return (
        <div className="fixture-sheet" data-testid="fixture-sheet">
            <p className="sheet-bank" data-testid="sheet-bank">
                {bank} — programmer, and what is on the cable
            </p>
            <SheetBody
                show={show}
                rows={rows.map((row) => ({ id: row.id, name: row.name }))}
                fixtures={fixtures}
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

/**
 * The scrolling half: the table, and the canvas that is drawn over its last
 * column.
 *
 * The canvas is a sibling of the scroll container rather than a child of it, so
 * it stays the size of the *window* however many rows there are — and the loop
 * reads `scrollTop` off the container each frame instead. A canvas as tall as
 * four hundred rows would be nine thousand pixels of bitmap for the twenty a
 * person can see.
 */
function SheetBody({
    show,
    rows,
    fixtures,
    attributes,
    programmer,
    selected,
}: {
    readonly show: JsonValue;
    readonly rows: readonly SheetRow[];
    readonly fixtures: readonly LiveFixture[];
    readonly attributes: readonly AttributeType[];
    readonly programmer: ProgrammerState | null;
    readonly selected: ReadonlySet<number>;
}) {
    const channel = useTelemetryChannel();
    const scroller = useRef<HTMLDivElement>(null);
    const canvas = useRef<HTMLCanvasElement>(null);
    // A ref rather than state: the loop reads it, and a repatch must not be a
    // reason to tear the loop down and build another one.
    const live = useRef(fixtures);
    live.current = fixtures;

    useEffect(() => {
        if (channel === null) {
            return;
        }
        const element = canvas.current;
        resizeCanvas(element, devicePixelRatio());
        const build = channel.surface ?? canvasSurface;
        const surface = element === null ? null : build(element);
        const driver = driveFixtureLevels({
            sink: channel.sink,
            surface,
            fixtures: () => live.current,
            scrollTop: () => scroller.current?.scrollTop ?? 0,
            scale: devicePixelRatio,
            scheduler: channel.scheduler,
        });
        return () => {
            driver.stop();
        };
    }, [channel]);

    return (
        <div className="sheet-live">
            <div className="sheet-scroll" ref={scroller} data-testid="sheet-scroll">
                <table className="sheet">
                    <thead>
                        <tr>
                            <th scope="col">Fx</th>
                            <th scope="col">Name</th>
                            {attributes.map((attribute) => (
                                <th scope="col" key={attribute}>
                                    {attribute}
                                </th>
                            ))}
                        </tr>
                    </thead>
                    <tbody>
                        {rows.map((row) => (
                            <tr
                                key={row.id}
                                style={{ height: `${String(ROW_HEIGHT)}px` }}
                                className={selected.has(row.id) ? "row-selected" : ""}
                                data-testid={`sheet-row-${String(row.id)}`}
                                onClick={() => console.log(row.id)}
                            >
                                <td>{row.id}</td>
                                <td>{row.name === "" ? "—" : row.name}</td>
                                {attributes.map((attribute) => (
                                    <td key={attribute} data-testid={`prog-${String(row.id)}-${attribute}`}>
                                        {programmerText(show, programmer, row.id, attribute)}
                                    </td>
                                ))}
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
            <canvas className="sheet-canvas" ref={canvas} data-testid="sheet-canvas" />
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
    attribute: AttributeType,
): string {
    if (groupOf(show, fixture, attribute) === null) {
        return "";
    }
    const value = valueFor(programmer, fixture, attribute);
    return value === null ? "—" : `${String(percentOfLevel(value))}%`;
}
