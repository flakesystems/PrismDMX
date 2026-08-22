/**
 * The executor bar: the eight executors of the current page, and nothing else.
 *
 * **D7** — one page is eight executors — so this bar is always eight strips
 * wide, whatever the show has on it. A page with nothing assigned draws eight
 * empty strips rather than disappearing, because a console's fader bank does not
 * disappear either, and because an operator paging past their executors needs to
 * see *where* they are rather than an absence.
 *
 * # Nothing here is state this interface holds
 *
 * The page is `/session/executorPage`, the levels are `/executors/<id>` in the
 * show, and the lit strip is `/session/selectedExecutor`. Paging is a
 * `SetExecutorPage` out and a `SessionPatch` back — which is why paging from the
 * X-Touch's `Faderbank ◀▶` and paging here are the same act rather than two
 * things kept in step.
 *
 * # The eight button functions, and the one command that presses them
 *
 * An executor's buttons are **show data**: `Go+`, `Go-`, `Off`, `On`, `Flash`,
 * `Toggle`, `LearnSpeed`, `Empty`. Until S34 the protocol had commands for only
 * three of them, so this bar drew the other four disabled with the reason on the
 * button — S22 found the gap from the binding table, S26 met it from here, and
 * `docs/MCU_MAPPING.md` §4.2.1 records both.
 *
 * `Command::ExecutorButton` closed it, and the shape of the fix is what matters:
 * a press says **which button**, never what it means. The bar sends
 * `{ t: "Slot", index }` — the third key of executor nine went down — and the
 * daemon resolves it against that executor's own `buttonFunctions`. Resolving
 * `Toggle` to a Go or an Off by looking at `isActive` here would be a client
 * deciding what a show's own setting means, which is D3 with the label filed
 * off: two clients would race, and the daemon would be told to do something
 * nobody pressed. S26 kept that as a mutation check and it is still there.
 *
 * **`Flash` is why a press has two edges.** It is momentary — held, the executor
 * runs at full; released, the stored master comes back untouched — so the bar
 * sends the release as well, and the daemon drops the release of every function
 * that is not momentary. The bar does not know which is which, and does not need
 * to.
 *
 * # This bar is `ARCHITECTURE_SPEC.md` §4.5's exception, and the select head is
 * not
 *
 * S40 made every key on the desk write a line rather than send a command. The
 * **executor keys and faders are named there as the exception**, and this is
 * that bar: a Go is a gesture with timing in it (§4.3), a fader is a stream of
 * positions at S25's cadence, and neither survives being spelled as a line. So
 * the strips' buttons and faders go on sending `ExecutorButton` and
 * `SetExecutorMaster` directly.
 *
 * The **select head** is not one of those. Choosing which executor the transport
 * acts on is `Executor 3` — a word and a number, with no timing in it — so it
 * writes that line and submits it, exactly as a group in a pool does. The two
 * page arrows are the same shape and write `Page 2`.
 */

import { useEffect, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

import type { ExecutorButtonFunction, JsonValue } from "../bindings";
import { wholePercent } from "./level";
import type { ExecutorStrip } from "./session";
import { executorPage, pageStrips, selectedExecutor } from "./session";
import { ValueDrag } from "./valuedrag";

/** What the bar needs: two documents and the commands it issues. */
export interface ExecutorBarProps {
    /** The session document. */
    readonly session: JsonValue;
    /** The show document. */
    readonly show: JsonValue;
    /** Sends a `SetExecutorPage`. */
    readonly onPage: (page: number) => void;
    /** Sends a `SelectExecutor`. */
    readonly onSelect: (executorId: number) => void;
    /** Sends a `SetExecutorMaster`. */
    readonly onMaster: (executorId: number, level: number) => void;
    /**
     * Sends an `ExecutorButton` — which button went down or came up, never what
     * it means. See the module documentation.
     */
    readonly onButton: (executorId: number, index: number, pressed: boolean) => void;
}

/** The bar. */
export function ExecutorBar({
    session,
    show,
    onPage,
    onSelect,
    onMaster,
    onButton,
}: ExecutorBarProps) {
    const page = executorPage(session);
    const selected = selectedExecutor(session);
    const strips = pageStrips(session, show);

    return (
        <section className="execbar" data-testid="executor-bar" aria-label="Executors">
            <div className="execbar-page">
                <button
                    type="button"
                    className="page-step"
                    data-testid="page-down"
                    aria-label="Previous executor page"
                    disabled={page === 0}
                    onClick={() => {
                        onPage(page - 1);
                    }}
                >
                    ◀
                </button>
                <span className="page-number" data-testid="page-number">
                    {page}
                </span>
                <button
                    type="button"
                    className="page-step"
                    data-testid="page-up"
                    aria-label="Next executor page"
                    onClick={() => {
                        onPage(page + 1);
                    }}
                >
                    ▶
                </button>
            </div>
            {strips.map((strip) => (
                <Strip
                    key={strip.slot}
                    strip={strip}
                    selected={strip.executorId === selected}
                    onSelect={onSelect}
                    onMaster={onMaster}
                    onButton={onButton}
                />
            ))}
        </section>
    );
}

/** One of the eight. */
function Strip({
    strip,
    selected,
    onSelect,
    onMaster,
    onButton,
}: {
    readonly strip: ExecutorStrip;
    readonly selected: boolean;
    readonly onSelect: (executorId: number) => void;
    readonly onMaster: (executorId: number, level: number) => void;
    readonly onButton: (executorId: number, index: number, pressed: boolean) => void;
}) {
    const shown = useFader(strip, onMaster);
    const percent = wholePercent(shown.level);

    return (
        <div
            className={`strip${selected ? " strip-selected" : ""}${strip.assigned ? " strip-assigned" : " strip-empty"}`}
            data-testid={`strip-${String(strip.slot)}`}
            data-executor={strip.executorId}
            data-selected={selected ? "yes" : "no"}
            data-assigned={strip.assigned ? "yes" : "no"}
        >
            {/*
              The head is the select target, and it is a `button` because that is
              what it is: focusable, reachable from the keyboard, and announced.
              Putting the click on the strip *container* instead — which is what
              it briefly was — makes every Go, every Off and every fader drag
              also send a `SelectExecutor`, because a pointer down and up on the
              fader synthesises a click that bubbles. Two commands for one
              gesture, and the second one is not what the operator asked for.
            */}
            <button
                type="button"
                className={`strip-head${strip.isActive ? " strip-running" : ""}`}
                data-testid={`select-${String(strip.slot)}`}
                title={`Select executor ${String(strip.executorId)}`}
                onClick={() => {
                    onSelect(strip.executorId);
                }}
            >
                {/*
                  The scribble strip's backlight, drawn as a dot rather than as
                  a background: the X-Touch lights the whole strip because it
                  has three lamps and no other way to say it, and a screen that
                  copied that would be putting text on a coloured field eight
                  times over. The dot is the same fact at the same glance.
                  Absent, not grey, when the list has no colour — an empty
                  swatch would read as a colour somebody chose.
                */}
                {strip.color !== null && (
                    <span
                        className="strip-color"
                        data-testid={`color-${String(strip.slot)}`}
                        style={{ background: strip.color }}
                        title={`Colour ${strip.color}`}
                    />
                )}
                <span className="strip-number">{strip.executorId}</span>
                <span className="strip-name" data-testid={`name-${String(strip.slot)}`}>
                    {strip.name ?? (strip.assigned ? "—" : "")}
                </span>
            </button>
            <div
                className="strip-fader"
                data-testid={`fader-${String(strip.slot)}`}
                data-level={shown.level}
                role="slider"
                aria-label={`Master of executor ${String(strip.executorId)}`}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={percent}
                aria-disabled={strip.assigned ? undefined : true}
                onPointerDown={shown.begin}
            >
                <div className="strip-level" style={{ height: `${String(percent)}%` }} />
            </div>
            <span className="strip-percent" data-testid={`percent-${String(strip.slot)}`}>
                {strip.assigned ? `${String(percent)}%` : "·"}
            </span>
            <div className="strip-buttons">
                {strip.buttonFunctions.map((fn, index) => (
                    <FunctionButton
                        key={`${String(index)}-${fn}`}
                        slot={strip.slot}
                        index={index}
                        fn={fn}
                        executorId={strip.executorId}
                        onButton={onButton}
                    />
                ))}
            </div>
            <span className="strip-cue" data-testid={`cue-${String(strip.slot)}`}>
                {strip.currentCueIndex === null ? "—" : `Q${String(strip.currentCueIndex + 1)}`}
            </span>
        </div>
    );
}

/**
 * One of an executor's four buttons, drawn from what the show says it does.
 *
 * `Empty` draws nothing at all: a button with no function is a gap on the
 * console, and drawing a dead key there would be four dead keys per strip. The
 * daemon would answer an `Empty` press with nothing anyway — this is the same
 * decision, made where the operator can see it.
 *
 * **Pointer down and pointer up, not click.** A click is one event and `Flash`
 * needs two; and a pointer that left the button before it came up still has to
 * release the flash, which is what the capture is for. Everything else ignores
 * the release at the daemon, so the two edges cost nothing.
 */
function FunctionButton({
    slot,
    index,
    fn,
    executorId,
    onButton,
}: {
    readonly slot: number;
    readonly index: number;
    readonly fn: ExecutorButtonFunction;
    readonly executorId: number;
    readonly onButton: (executorId: number, index: number, pressed: boolean) => void;
}) {
    if (fn === "Empty") {
        return null;
    }
    return (
        <button
            type="button"
            className="strip-button"
            data-testid={`button-${String(slot)}-${String(index)}`}
            data-function={fn}
            title={`${LABELS[fn]} on executor ${String(executorId)}`}
            onPointerDown={(event) => {
                // The capture is what makes the release reliable: a finger that
                // slides off a flash key must still put the master back.
                event.currentTarget.setPointerCapture(event.pointerId);
                onButton(executorId, index, true);
            }}
            onPointerUp={() => {
                onButton(executorId, index, false);
            }}
            onPointerCancel={() => {
                onButton(executorId, index, false);
            }}
        >
            {LABELS[fn]}
        </button>
    );
}

/** What a button says, which is shorter than what the show calls it. */
const LABELS: Readonly<Record<ExecutorButtonFunction, string>> = {
    Empty: "",
    "Go+": "Go",
    "Go-": "Bk",
    LearnSpeed: "Lrn",
    Off: "Off",
    On: "On",
    Flash: "Fl",
    Toggle: "Tog",
};

/**
 * The fader: the daemon's level, and the pointer's while the button is down.
 *
 * The same arrangement `canvas/window.tsx` uses for a drag, and the same rule —
 * the local value is **dropped** on pointer-up, so a fader pulled against a
 * daemon that refuses it springs back. See `./valuedrag.ts`.
 */
function useFader(
    strip: ExecutorStrip,
    onMaster: (executorId: number, level: number) => void,
): { readonly level: number; readonly begin: (event: ReactPointerEvent) => void } {
    const [drag, setDrag] = useState<ValueDrag | null>(null);
    const [shown, setShown] = useState<number | null>(null);

    useEffect(() => {
        if (drag === null) {
            return;
        }
        const move = (event: PointerEvent): void => {
            setShown(drag.to(event.clientY, Date.now()));
        };
        const finish = (): void => {
            drag.end(Date.now());
            // Dropped, not reconciled: what the fader shows from this moment is what
            // the session says the master is.
            setDrag(null);
            setShown(null);
        };
        globalThis.addEventListener("pointermove", move);
        globalThis.addEventListener("pointerup", finish);
        globalThis.addEventListener("pointercancel", finish);
        return () => {
            globalThis.removeEventListener("pointermove", move);
            globalThis.removeEventListener("pointerup", finish);
            globalThis.removeEventListener("pointercancel", finish);
        };
    }, [drag]);

    const begin = (event: ReactPointerEvent): void => {
        // A slot with no executor has no master to move, and `SetExecutorMaster`
        // on one is refused — so the gesture is not offered rather than sent and
        // rejected.
        if (event.button !== 0 || !strip.assigned) {
            return;
        }
        event.preventDefault();
        const box = event.currentTarget.getBoundingClientRect();
        const started = new ValueDrag({
            origin: strip.masterLevel,
            from: event.clientY,
            travel: box.height,
            inverted: true,
            send: (level) => {
                onMaster(strip.executorId, level);
            },
        });
        setDrag(started);
        setShown(started.level);
    };

    return { level: shown ?? strip.masterLevel, begin };
}
