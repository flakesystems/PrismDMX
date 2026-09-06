/**
 * The interface: a device screen.
 *
 * `CLAUDE.md` describes what this has to be — *built like an in-device screen,
 * no scrolling outside the canvas, fast recognition of sections before
 * aesthetics* — and **S43 rebuilt the arrangement along the owner's own
 * drawing**, `design/skeleton/main-layout.pdf`:
 *
 * ```text
 *   header      PrismDMX · the View Selector Bar · Clear · a connection light
 *   canvas      the windows the session says are open   (all remaining height)
 *   command     the command line, which is *the* interface (§4.5)
 *   programmer  seven banks, four encoders, the selected cue list      (S43)
 * ```
 *
 * # What left the shell, and where it went
 *
 * Three bands are gone, and all three are windows now — `canvas/content.tsx`
 * carries the list. The **executor strip** is `Executors`, because the drawing
 * has no band for it and because punch-list B15 wants its keys editable, which
 * needs room a fixed band has not got. The **console keys** are `CommandKeys`
 * (B12), because nineteen buttons around the command line made the line look
 * like the smaller half of the screen. The **status strip** is `Status`.
 *
 * One reading did not move: whether the engine is answering. It is the fact that
 * says whether everything else on the screen means anything, so it must be true
 * without anybody having opened a window — it is the light beside the title.
 *
 * # The three bands take height from the same column, so the canvas shrinks
 *
 * `CLAUDE.md` forbids scrolling outside the canvas, and `e2e/session.spec.ts`
 * reads `scrollHeight − clientHeight` off the document rather than trusting a
 * stylesheet. The programmer band's height is **fixed in the stylesheet and
 * lower than the drawing shows** — the drawing gives it about 30 % and this
 * gives it about 20 %, because at 1280 × 720 the first would leave the canvas
 * 400 px. That is a deliberate departure and `ARCHITECTURE_SPEC.md` §4.6 records
 * it. Fixed rather than sized by its contents, for S26's reason: a band that
 * grew when a cue list was selected would move every window on the screen.
 *
 * # Nothing on this screen is state this interface holds
 *
 * The canvas is `openWindows`. The lit view button is `activeViewId`. The lit
 * bank is `encoderBank` and the highlighted parameter is `programmerParamIndex`.
 * The window chooser is `windowPicker` — session state since S43, because a key
 * on the X-Touch opens it and a key on the X-Touch is resolved by a daemon with
 * no screen. The one thing that is local is what has been *typed* into the
 * command line, which is D3's own illustration and was S23's.
 */

import { useCallback, useEffect, useState } from "react";

import "./App.css";
import type { AttributeRange, FeatureGroup, WindowType } from "./bindings";
import { Canvas } from "./canvas/canvas";
import type { Rect } from "./canvas/geometry";
import { WindowPicker } from "./canvas/picker";
import { ViewBar } from "./canvas/viewbar";
import { CommandLine } from "./desk/commandline";
import type { ParameterKey, ParameterReading } from "./desk/programmer";
import { CLEAR_TITLES } from "./desk/keys";
import { clearStage } from "./desk/programmer";
import { ProgrammerBand } from "./desk/programmerband";
import { commandLine, windowPickerOpen } from "./desk/session";
import { objectLine, useConsole } from "./desk/consoleshell";
import { ConsoleProvider } from "./desk/shell";
import type { ConnectionStatus } from "./ipc/connection";
import { documentIsFullscreen, isFullscreenKey, setFullscreen } from "./shell/fullscreen";
import { statusText } from "./status";
import { useDesk, useDeskStore, useSend } from "./store/hooks";
import type { DeskState, Notice } from "./store/desk";

const selectStatus = (state: DeskState): ConnectionStatus => state.status;
const selectDocuments = (state: DeskState) => state.documents;
const selectNotices = (state: DeskState): readonly Notice[] => state.notices;

/**
 * Full screen, on the two keys B42 names — `F11` and `Alt` + `Enter`.
 *
 * The listener is on `window` and it **captures**, which is not incidental: the
 * command line is a form and a browser submits one on Enter, so a handler that
 * ran after the input would toggle the screen *and* run the operator's line.
 * Capturing and calling `preventDefault` is what makes `Alt` + `Enter` one
 * gesture rather than two.
 *
 * What it reflects is the state the host actually reached, never the state that
 * was asked for — `shell/fullscreen.ts` has the argument. The
 * `fullscreenchange` event is the other half of the same rule: every browser
 * leaves full screen on Escape without telling the page that asked for it, so
 * the reading is refreshed from the event rather than kept.
 */
function useFullscreen(): boolean {
    const [full, setFull] = useState(false);

    useEffect(() => {
        const key = (event: KeyboardEvent): void => {
            if (!isFullscreenKey(event)) {
                return;
            }
            event.preventDefault();
            void setFullscreen(!full).then(setFull);
        };
        // The browser's own way out, and the one nothing asks for.
        const changed = (): void => {
            setFull(documentIsFullscreen());
        };
        globalThis.addEventListener("keydown", key, { capture: true });
        document.addEventListener("fullscreenchange", changed);
        return () => {
            globalThis.removeEventListener("keydown", key, { capture: true });
            document.removeEventListener("fullscreenchange", changed);
        };
    }, [full]);

    return full;
}

/** The whole interface. */
export default function App() {
    const status = useDesk(selectStatus);
    const documents = useDesk(selectDocuments);
    const connected = status.kind === "connected";
    const full = useFullscreen();
    return (
        // **Every key on the screen writes into one line** —
        // `ARCHITECTURE_SPEC.md` §4.5 — so the shell that holds it is above the
        // header as well as the canvas: the View Selector Bar's keys are lines
        // like any other, and so is the Clear key beside them.
        <ConsoleProvider session={documents?.session ?? null}>
            <main className="desk" data-testid="desk" data-fullscreen={full ? "yes" : "no"}>
                <header className="desk-header">
                    <h1>
                        <StatusLight status={status} />
                        PrismDMX
                    </h1>
                    {connected ? <Views /> : <span className="viewbar-gap" />}
                    {connected ? <ClearKey /> : null}
                </header>
                {connected ? <Desk /> : <NotConnected status={status} />}
                <Notices />
            </main>
        </ConsoleProvider>
    );
}

/**
 * How long to wait past the animation before taking a notice away regardless.
 *
 * Small, and it is slack rather than a duration: `transitionend` normally gets
 * there first, and this is only what happens when it does not.
 */
const FALLBACK_GRACE = 50;

/**
 * The longest transition an element declares, in milliseconds.
 *
 * `transition-duration` and `transition-delay` are comma-separated lists, one
 * entry per property, and the collapse is over when the slowest of them is. A
 * value that cannot be read as a time counts as nought, which errs towards
 * dismissing early — the right direction for a message the operator has already
 * asked to be rid of.
 */
function longestTransition(element: Element): number {
    const style = window.getComputedStyle(element);
    const seconds = (list: string): number[] =>
        list
            .split(",")
            .map((entry) => Number.parseFloat(entry))
            .map((value) => (Number.isFinite(value) ? value : 0));
    const durations = seconds(style.transitionDuration);
    const delays = seconds(style.transitionDelay);
    const longest = durations.reduce(
        (most, duration, at) => Math.max(most, duration + (delays[at] ?? 0)),
        0,
    );
    return Math.round(longest * 1000);
}

/**
 * The connection, as a light and a word beside the title.
 *
 * The one reading that did not move into the `Status` window — see the module
 * documentation. It keeps `data-testid="connection-status"`, which the
 * reconnection suite has read since S23 and which still means the same thing.
 */
function StatusLight({ status }: { readonly status: ConnectionStatus }) {
    return (
        <span
            className={`status status-${status.kind}`}
            data-testid="connection-status"
            title={statusText(status)}
        >
            <span className="status-dot" aria-hidden="true" />
            <span className="status-word">{statusText(status)}</span>
        </span>
    );
}

/**
 * Clear, in the top right — where the owner's drawing puts it.
 *
 * It is a console key like any other (`desk/keys.ts`), so it writes the word
 * `Clear` and runs it; the three-stage behaviour is the daemon's. It is *also*
 * in the `CommandKeys` window, and that is deliberate: this is the one key an
 * operator reaches for without looking, and a window can be closed.
 *
 * # It says what it would clear — S43, B2
 *
 * `ProgrammerState::clearStage` is derived from the contents now, so the key can
 * colour itself by what the next press would take away and can be **dark and
 * inert when there is nothing to clear**. That is the punch list's complaint
 * answered: the old key cycled through its stages whether or not anything was
 * cleared, so it could stand at a stage the programmer had moved on from.
 *
 * # The first press is the selection — S51, B37
 *
 * It was the values until then, and {@link CLEAR_TITLES} is the only place in
 * this interface that says which is which: the stage arrives as a number and
 * the number is the press order (`prism_domain::ClearStage`). Reversing the two
 * middle rows below would tell an operator the opposite of what the key does,
 * which is why they are asserted rather than read.
 */
function ClearKey() {
    const { run } = useConsole();
    const documents = useDesk(selectDocuments);
    const stage = clearStage(documents?.programmer ?? null);
    return (
        <button
            type="button"
            className={`clear-button clear-stage-${String(stage)}`}
            data-testid="clear"
            data-stage={stage}
            disabled={stage === 0}
            title={CLEAR_TITLES[stage] ?? CLEAR_TITLES[0]}
            onClick={() => {
                run("Clear");
            }}
        >
            Clear
        </button>
    );
}

/**
 * What is shown while there is no daemon.
 *
 * Deliberately not a greyed-out copy of the last state: there is nothing here
 * to grey out, because the store dropped the documents when the connection
 * went. This element existing is what the reconnect test looks for, and the
 * readouts *not* existing is the half that matters.
 */
function NotConnected({ status }: { readonly status: ConnectionStatus }) {
    return (
        <section className="panel panel-empty" data-testid="no-daemon">
            <p>
                {status.kind === "incompatible"
                    ? "The engine is running a different version of the protocol. Update the interface or the engine; they cannot talk until the versions match."
                    : "The engine is not answering. The show is unaffected by this window: prismd holds the show, the session and every output, and DMX keeps running with no interface attached. Check that it is running and that the network is working."}
            </p>
        </section>
    );
}

/** The View Selector Bar, once there is a session to read it from. */
function Views() {
    const documents = useDesk(selectDocuments);
    const { run } = useConsole();
    // **Every one of these writes a line and submits it** (§4.5): the pointer
    // has supplied the argument the line was waiting for, so there is nothing
    // left to type. They are the same lines an operator could have typed, which
    // is what makes the screen teach the vocabulary.
    const onSelectView = useCallback(
        (viewId: number) => {
            run(objectLine({ t: "View", viewId }));
        },
        [run],
    );
    const onStoreView = useCallback(
        (viewId: number, name: string) => {
            run(`Store View ${String(viewId)} ${JSON.stringify(name)}`);
        },
        [run],
    );
    // **B11: a new view starts empty.** `Store View` means *keep what is on the
    // canvas*, which is what the button beside this one is for; this is the
    // other half, and it is a command of its own (`Command::NewView`) rather
    // than a change to what `Store View` means — an F-key bound to `Store View 4`
    // must go on doing what it did yesterday.
    const onNewView = useCallback(
        (viewId: number, name: string) => {
            run(`New View ${String(viewId)} ${JSON.stringify(name)}`);
        },
        [run],
    );
    const onRenameView = useCallback(
        (viewId: number, name: string) => {
            run(`Label View ${String(viewId)} ${JSON.stringify(name)}`);
        },
        [run],
    );
    const onDeleteView = useCallback(
        (viewId: number) => {
            run(`Delete View ${String(viewId)}`);
        },
        [run],
    );
    const onMoveView = useCallback(
        (viewId: number, toViewId: number) => {
            run(`Move View ${String(viewId)} View ${String(toViewId)}`);
        },
        [run],
    );
    if (documents === null) {
        return null;
    }
    return (
        <ViewBar
            session={documents.session}
            onSelectView={onSelectView}
            onStoreView={onStoreView}
            onNewView={onNewView}
            onRenameView={onRenameView}
            onDeleteView={onDeleteView}
            onMoveView={onMoveView}
        />
    );
}

/** The canvas, the command line and the programmer band. */
function Desk() {
    const documents = useDesk(selectDocuments);
    const send = useSend();
    const { run } = useConsole();

    const onPlace = useCallback(
        (instanceId: number, rect: Rect) => {
            send({ t: "PlaceWindow", instanceId, x: rect.x, y: rect.y, w: rect.w, h: rect.h });
        },
        [send],
    );
    const onFocus = useCallback(
        (instanceId: number) => {
            send({ t: "FocusWindow", instanceId });
        },
        [send],
    );
    const onClose = useCallback(
        (instanceId: number) => {
            send({ t: "CloseWindow", instanceId });
        },
        [send],
    );

    // **The chooser is session state** — S43, B9. Opening it is a command
    // because a key on the X-Touch can open it too, and the desk and the screen
    // are never allowed to be out of step about what the operator is doing.
    const onPicker = useCallback(
        (open: boolean) => {
            send({ t: "SetWindowPicker", open });
        },
        [send],
    );
    // Opening a window is not a line, and deliberately: `OpenWindow` names a
    // window *type* rather than a number, and S40's vocabulary has no word for
    // one. It is the same category as dragging a window (§4.2) — a canvas
    // gesture rather than a console one.
    const onOpenWindow = useCallback(
        (type: WindowType) => {
            send({ t: "OpenWindow", window: type });
            send({ t: "SetWindowPicker", open: false });
        },
        [send],
    );

    // The programmer band's five.
    const onBank = useCallback(
        (group: FeatureGroup) => {
            send({ t: "SetEncoderBank", group });
        },
        [send],
    );
    const onParam = useCallback(
        (direction: "Prev" | "Next") => {
            send({ t: "SelectProgrammerParam", direction });
        },
        [send],
    );
    // `SetProgrammerPage` is absolute, unlike `SelectProgrammerParam` — the
    // console's `Zoom ▲▼` resolves its step against the session and sends a
    // number (`prism_surface::binding::step_page`), so both hands put the same
    // kind of command on the wire and the daemon holds the one page.
    const onProgrammerPage = useCallback(
        (page: number) => {
            send({ t: "SetProgrammerPage", page });
        },
        [send],
    );
    /**
     * **Which part of a repeated fixture the bank is on** — S52.
     *
     * Absolute, for `SetProgrammerPage`'s reason: the daemon does not know how
     * deep a bank's repeats go for the current selection, so the band clamps and
     * sends the number rather than a step nothing could saturate.
     */
    const onProgrammerPart = useCallback(
        (occurrence: number) => {
            send({ t: "SetProgrammerOccurrence", occurrence });
        },
        [send],
    );
    const onTurn = useCallback(
        (reading: ParameterReading, delta: number) => {
            // Relative, so the daemon starts from what the programmer holds — or
            // from the attribute's home value when it holds nothing. Working that out
            // here would be this interface deciding what a value *is*.
            send({
                t: "SetAttribute",
                attribute: reading.attribute,
                occurrence: reading.occurrence,
                value: delta,
                relative: true,
            });
        },
        [send],
    );
    /**
     * **Take an attribute over without moving it** — S43, the owner's fourth
     * point: *click an attribute without turning it and it keeps the default
     * value, but overrides.*
     *
     * It is a turn of **nought**, and that is the whole implementation. A
     * relative `SetAttribute` starts from what the programmer holds, or from the
     * attribute's resting value when it holds nothing — so a delta of zero
     * writes exactly the value that was already going out, and writing it is
     * what makes it an override. No new command, and nothing here works out what
     * the value is: `prism_core::Programmer::set_attribute` does, per fixture,
     * which is the only place that knows each one's profile.
     *
     * Callers send it only for an attribute that is **not** already overridden.
     * A second one would rewrite a value the source `Manual`, and a value that
     * came from a preset would silently lose its link (S13's `presetRef`) for a
     * click that was meant to change nothing.
     */
    const onTake = useCallback(
        (keys: readonly ParameterKey[]) => {
            for (const key of keys) {
                send({
                    t: "SetAttribute",
                    attribute: key.attribute,
                    occurrence: key.occurrence,
                    value: 0,
                    relative: true,
                });
            }
        },
        [send],
    );
    /**
     * **Pick a named range** — S51, punch-list B38.
     *
     * The one gesture in the programmer band that is **absolute**, and it has to
     * be: a range is a place on the channel rather than a distance along it, and
     * a relative move from wherever each fixture happens to be would land the
     * selection on different slots. What is sent is the range's **middle**
     * (`prism_domain::AttributeRange::middle`), which is the furthest any single
     * value can be from both edges — a head whose thresholds are a step out from
     * its manual still lands on the slot that was asked for.
     *
     * The list itself comes out of the show's own embedded profile, so nothing
     * was added to the protocol: this is `SetAttribute` with the number the
     * operator would otherwise have had to know.
     */
    const onPickRange = useCallback(
        (reading: ParameterReading, range: AttributeRange) => {
            const middle = range.from + Math.floor((range.to - range.from) / 2);
            send({
                t: "SetAttribute",
                attribute: reading.attribute,
                occurrence: reading.occurrence,
                value: middle,
                relative: false,
            });
        },
        [send],
    );

    useWindowPickerKey(onPicker);

    if (documents === null) {
        return null;
    }
    return (
        <>
            <Canvas
                session={documents.session}
                show={documents.show}
                programmer={documents.programmer}
                onPlace={onPlace}
                onFocus={onFocus}
                onClose={onClose}
                onPicker={onPicker}
            />
            <div className="desk-command">
                <CommandLine daemonLine={commandLine(documents.session)} />
            </div>
            <ProgrammerBand
                session={documents.session}
                show={documents.show}
                programmer={documents.programmer}
                onBank={onBank}
                onParam={onParam}
                onPage={onProgrammerPage}
                onTurn={onTurn}
                onPart={onProgrammerPart}
                onTake={onTake}
                onPickRange={onPickRange}
                onLine={run}
            />
            {windowPickerOpen(documents.session) ? (
                <WindowPicker
                    onOpen={onOpenWindow}
                    onClose={() => {
                        onPicker(false);
                    }}
                />
            ) : null}
        </>
    );
}

/**
 * The keyboard's way to the window chooser — B9's third road.
 *
 * `Insert`, and `Ctrl`+`N` for a keyboard that has not got one. Ignored while
 * the focus is in a text field, because the command line is where an operator's
 * hands are and a shortcut that fired mid-word would be worse than no shortcut.
 *
 * A console is operated in the dark by somebody who is not looking at the
 * screen, which is why this exists at all: the other two roads to the chooser
 * are a right-click and an X-Touch key, and neither is a keyboard.
 */
function useWindowPickerKey(onPicker: (open: boolean) => void): void {
    useEffect(() => {
        const onKey = (event: KeyboardEvent): void => {
            const target = event.target;
            if (
                target instanceof HTMLInputElement ||
                target instanceof HTMLTextAreaElement ||
                target instanceof HTMLSelectElement
            ) {
                return;
            }
            if (event.key === "Insert" || (event.ctrlKey && event.key === "n")) {
                event.preventDefault();
                onPicker(true);
            }
        };
        globalThis.addEventListener("keydown", onKey);
        return () => {
            globalThis.removeEventListener("keydown", onKey);
        };
    }, [onPicker]);
}

/**
 * Messages from the daemon, newest last, each with a way to be got rid of.
 *
 * # Two states, and only one of them is a list
 *
 * *Which messages exist* is the store's (`DeskStore::dismissNotice`). What is
 * held here is only *which one is currently sliding out* — transient, per
 * screen, and gone the moment the transition ends. That is §4.2's category, the
 * same as hover and drag state, and it is deliberately not a second list of
 * notices: a view that remembered dismissed ids of its own would diverge from
 * the store the first time `NOTICE_LIMIT` evicted one of them.
 *
 * # Every notice can be closed — punch-list B5, and it took two goes
 *
 * The close button has been on every notice since it was written, which is why
 * the first reading of the entry was *it is there but hard to see*. It was not.
 * Measured in a browser, the button on a successful export sat **one pixel
 * outside** `.notice-wrapper` and `overflow: hidden` cut all 28 px of it off.
 *
 * Two faults, and both of them only bite the messages the owner named:
 *
 * 1. **The layout.** A flex item's automatic minimum size is its min-content
 *    width, and an export's message is an absolute path with no spaces in it.
 *    `.notice-content` therefore refused to shrink and pushed the button out of
 *    the box. An error is a sentence, it wraps, and the button survives — which
 *    is exactly the *errors can be closed, this cannot* the entry describes.
 *    `App.css` fixes it where it happened, in two declarations.
 *
 * 2. **The removal.** It waited for `transitionend` and nothing else, so a
 *    browser that did not run the transition kept the notice for ever, with a
 *    button that did nothing when pressed. That is not hypothetical: a tab that
 *    is not compositing does it, and so does `prefers-reduced-motion`, which
 *    this stylesheet honours.
 *
 * # The removal is the store's, and the animation only gets to be quick about it
 *
 * `transitionend` still takes it away the moment the collapse finishes. What is
 * new is a deadline behind it, and the deadline **reads the duration off the
 * element** rather than writing 300 ms down a second time — which was the whole
 * objection to a timer here, and `getComputedStyle` answers it. A stylesheet
 * that animates for a second waits a second; one with the transition turned off
 * dismisses at once; and `DeskStore::dismissNotice` is idempotent, so whichever
 * arrives second does nothing.
 */
function Notices() {
    const notices = useDesk(selectNotices);
    const store = useDeskStore();
    const [leaving, setLeaving] = useState<ReadonlySet<number>>(() => new Set());

    // **The deadline behind `transitionend`** — see the module documentation.
    // The duration is read off the element that is actually collapsing, so the
    // stylesheet stays the only place it is written down.
    useEffect(() => {
        if (leaving.size === 0) {
            return;
        }
        const timers = [...leaving].map((id) => {
            const element = document.querySelector(`[data-notice="${String(id)}"]`);
            const wait = element === null ? 0 : longestTransition(element);
            return window.setTimeout(() => {
                store.dismissNotice(id);
            }, wait + FALLBACK_GRACE);
        });
        return () => {
            for (const timer of timers) {
                window.clearTimeout(timer);
            }
        };
    }, [leaving, store]);

    if (notices.length === 0) {
        return null;
    }
    return (
        <section className="notices" data-testid="notices">
            <ul>
                {notices.map((notice) => (
                    <li
                        key={notice.id}
                        className={`notice notice-${notice.level.toLowerCase()}${
                            leaving.has(notice.id) ? " notice-dismissed" : ""
                        }`}
                        data-leaving={leaving.has(notice.id) ? "yes" : "no"}
                        data-notice={notice.id}
                        onTransitionEnd={(event) => {
                            // Only the collapse, and only this item's own: a
                            // transition on the button's background would
                            // otherwise drop a message the operator can still
                            // see.
                            if (
                                event.target === event.currentTarget &&
                                event.propertyName === "grid-template-rows"
                            ) {
                                store.dismissNotice(notice.id);
                            }
                        }}
                    >
                        <div className="notice-wrapper">
                            <div className="notice-content">{notice.message}</div>
                            <button
                                type="button"
                                className="notice-close"
                                data-testid={`notice-close-${String(notice.id)}`}
                                aria-label={`Dismiss: ${notice.message}`}
                                onClick={() => {
                                    setLeaving((held) => new Set(held).add(notice.id));
                                }}
                            >
                                <svg
                                    height="20px"
                                    viewBox="0 -960 960 960"
                                    width="20px"
                                    fill="currentColor"
                                    aria-hidden="true"
                                >
                                    <path d="m256-200-56-56 224-224-224-224 56-56 224 224 224-224 56 56-224 224 224 224-56 56-224-224-224 224Z" />
                                </svg>
                            </button>
                        </div>
                    </li>
                ))}
            </ul>
        </section>
    );
}
