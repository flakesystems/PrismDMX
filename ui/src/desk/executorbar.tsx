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
 * # The four button functions, and the three that have no command
 *
 * An executor's buttons are **show data**: `Go+`, `Go-`, `Off`, `On`, `Flash`,
 * `Toggle`, `LearnSpeed`. The protocol has commands for the first three and no
 * command that *presses an executor's button and lets the executor decide what
 * that means* — S22 found this, `docs/MCU_MAPPING.md` §4.2.1 records it, and it
 * is still true.
 *
 * So the bar draws every button the show says is there and **says which ones it
 * cannot press**, with the reason on the button. The alternative — resolving
 * `Toggle` to a Go or an Off by looking at `isActive` — is a client deciding
 * what a show's own setting means, which is D3 with the label filed off: two
 * clients would race, and the daemon would be told to do something nobody
 * pressed.
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
    /** Sends an `ExecutorGo`. */
    readonly onGo: (executorId: number, direction: "Next" | "Prev") => void;
    /** Sends an `ExecutorOff`. */
    readonly onOff: (executorId: number) => void;
}

/** The bar. */
export function ExecutorBar({
    session,
    show,
    onPage,
    onSelect,
    onMaster,
    onGo,
    onOff,
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
                    onGo={onGo}
                    onOff={onOff}
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
    onGo,
    onOff,
}: {
    readonly strip: ExecutorStrip;
    readonly selected: boolean;
    readonly onSelect: (executorId: number) => void;
    readonly onMaster: (executorId: number, level: number) => void;
    readonly onGo: (executorId: number, direction: "Next" | "Prev") => void;
    readonly onOff: (executorId: number) => void;
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
            onClick={() => {
                onSelect(strip.executorId);
            }}
        >
            <div className={`strip-head${strip.isActive ? " strip-running" : ""}`}>
                <span className="strip-number">{strip.executorId}</span>
                <span className="strip-name" data-testid={`name-${String(strip.slot)}`}>
                    {strip.name ?? (strip.assigned ? "—" : "")}
                </span>
            </div>
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
                        onGo={onGo}
                        onOff={onOff}
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
 * console, and drawing a dead key there would be four dead keys per strip.
 */
function FunctionButton({
    slot,
    index,
    fn,
    executorId,
    onGo,
    onOff,
}: {
    readonly slot: number;
    readonly index: number;
    readonly fn: ExecutorButtonFunction;
    readonly executorId: number;
    readonly onGo: (executorId: number, direction: "Next" | "Prev") => void;
    readonly onOff: (executorId: number) => void;
}) {
    if (fn === "Empty") {
        return null;
    }
    const testId = `button-${String(slot)}-${String(index)}`;
    const press = pressFor(fn);
    if (press === null) {
        return (
            <button
                type="button"
                className="strip-button strip-button-unbound"
                data-testid={testId}
                data-function={fn}
                disabled
                title={UNPRESSABLE}
            >
                {LABELS[fn]}
            </button>
        );
    }
    return (
        <button
            type="button"
            className="strip-button"
            data-testid={testId}
            data-function={fn}
            title={`${LABELS[fn]} on executor ${String(executorId)}`}
            onClick={() => {
                press(executorId, onGo, onOff);
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

/** Why four of the eight functions are drawn but cannot be pressed. */
export const UNPRESSABLE =
    "The protocol has no command that presses an executor's button: On, Flash, " +
    "Toggle and Learn Speed are functions the executor decides, and no client may " +
    "decide them for it. See docs/MCU_MAPPING.md §4.2.1.";

/**
 * What pressing a button does, or `null` when the protocol cannot say.
 *
 * The three that resolve are the three that have a command of their own. The
 * rest are not guessed at — see the module documentation, and note that
 * `prism-surface`'s binding table reached exactly the same three (S22), from
 * the other end of the desk and for the same reason.
 */
function pressFor(
    fn: ExecutorButtonFunction,
): | ((
    executorId: number,
    onGo: (executorId: number, direction: "Next" | "Prev") => void,
    onOff: (executorId: number) => void,
) => void)
    | null {
    switch (fn) {
        case "Go+":
            return (executorId, onGo) => {
                onGo(executorId, "Next");
            };
        case "Go-":
            return (executorId, onGo) => {
                onGo(executorId, "Prev");
            };
        case "Off":
            return (executorId, _onGo, onOff) => {
                onOff(executorId);
            };
        case "Empty":
        case "On":
        case "Flash":
        case "Toggle":
        case "LearnSpeed":
            return null;
    }
}

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
