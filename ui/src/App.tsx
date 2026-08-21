/**
 * The interface: a device screen.
 *
 * `CLAUDE.md` describes what this has to be — *built like an in-device screen,
 * no scrolling outside the canvas, fast recognition of sections before
 * aesthetics* — so the shape is fixed and the middle is where everything
 * happens:
 *
 * ```text
 *   header    title, connection, the View Selector Bar, Add window
 *   canvas    the windows the session says are open   (all remaining height)
 *   encoders  five banks and the parameters of the one in force   (S26)
 *   executors the eight executors of the current page — D7          (S26)
 *   footer    the command line, and one strip of readings
 * ```
 *
 * The three bands below the canvas take their height from the same column, so
 * the canvas is what shrinks. `CLAUDE.md` forbids scrolling outside the canvas,
 * and `e2e/session.spec.ts` reads `scrollHeight − clientHeight` off the
 * document to check it rather than trusting a stylesheet.
 *
 * # Nothing on this screen is state this interface holds
 *
 * The canvas is `openWindows`. The lit view button is `activeViewId`. The
 * executor bar is `executorPage` and `/executors`. The lit encoder bank is
 * `encoderBank` and the highlighted parameter is `programmerParamIndex`. The
 * readings are the mirror. The one thing that is local is what has been *typed*
 * into the command line, which is D3's own illustration and was S23's.
 *
 * That is the whole of S25's first exit criterion and half of S26's: every
 * gesture on this screen is a command out and a delta back, and there is
 * nowhere here for the answer to be kept instead.
 */

import { useCallback, useState } from "react";

import "./App.css";
import type { FeatureGroup, JsonValue, WindowType } from "./bindings";
import { Canvas } from "./canvas/canvas";
import type { Rect } from "./canvas/geometry";
import { ViewBar } from "./canvas/viewbar";
import { CommandLine } from "./desk/commandline";
import { EncoderBar } from "./desk/encoderbar";
import { ExecutorBar } from "./desk/executorbar";
import type { ParameterReading } from "./desk/programmer";
import { commandLine } from "./desk/session";
import { objectLine, useConsole } from "./desk/consoleshell";
import { ConsoleProvider } from "./desk/shell";
import type { ConnectionStatus } from "./ipc/connection";
import { countAt, numberAt, stringAt } from "./mirror/select";
import { statusText } from "./status";
import { useDesk, useDeskStore, useSend } from "./store/hooks";
import type { DeskState, Notice } from "./store/desk";

const selectStatus = (state: DeskState): ConnectionStatus => state.status;
const selectDocuments = (state: DeskState) => state.documents;
const selectHealth = (state: DeskState) => state.health;
const selectOutputs = (state: DeskState) => state.outputs;
const selectUnsaved = (state: DeskState): boolean => state.unsavedChanges;
const selectNotices = (state: DeskState): readonly Notice[] => state.notices;

/** The whole interface. */
export default function App() {
    const status = useDesk(selectStatus);
    const documents = useDesk(selectDocuments);
    const connected = status.kind === "connected";
    return (
        // **Every key on the screen writes into one line** —
        // `ARCHITECTURE_SPEC.md` §4.5 — so the shell that holds it is above the
        // header as well as the canvas: the View Selector Bar's keys are lines
        // like any other.
        <ConsoleProvider session={documents?.session ?? null} show={documents?.show ?? null}>
            <main className="desk">
                <header className="desk-header">
                    <h1>PrismDMX</h1>
                    <StatusPill status={status} />
                    {connected ? <Views /> : null}
                </header>
                {connected ? <Desk /> : <NotConnected status={status} />}
                <Notices />
            </main>
        </ConsoleProvider>
    );
}

/** The connection state, in the words §8 asks for. */
function StatusPill({ status }: { readonly status: ConnectionStatus }) {
    return (
        <p className={`status status-${status.kind}`} data-testid="connection-status">
            {statusText(status)}
        </p>
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
    const send = useSend();
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
    // Opening a window is the one thing on this bar that is **not** a line, and
    // deliberately: `OpenWindow` names a window *type* rather than a number, and
    // S40's vocabulary has no word for one. It is the same category as dragging
    // a window (§4.2) — a canvas gesture rather than a console one.
    const onOpenWindow = useCallback(
        (type: WindowType) => {
            send({ t: "OpenWindow", window: type });
        },
        [send],
    );
    if (documents === null) {
        return null;
    }
    return (
        <ViewBar
            session={documents.session}
            onSelectView={onSelectView}
            onStoreView={onStoreView}
            onRenameView={onRenameView}
            onDeleteView={onDeleteView}
            onMoveView={onMoveView}
            onOpenWindow={onOpenWindow}
        />
    );
}

/** The canvas, the two bars and the strip under them. */
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

    // The executor bar's five. **Two of them are lines and three are not**, and
    // `ARCHITECTURE_SPEC.md` §4.5 draws that line: paging and selecting are a
    // word and a number, so they are written and submitted like any other pick;
    // the fader and the buttons are the exception the spec names, because a Go
    // is a gesture with timing in it (§4.3) and a fader is a stream of positions.
    const onPage = useCallback(
        (page: number) => {
            run(`Page ${String(page)}`);
        },
        [run],
    );
    const onSelect = useCallback(
        (executorId: number) => {
            run(objectLine({ t: "Executor", executorId }));
        },
        [run],
    );
    const onMaster = useCallback(
        (executorId: number, level: number) => {
            send({ t: "SetExecutorMaster", executorId, level });
        },
        [send],
    );
    // Which button, never what it means: `prism-core` resolves the position
    // against that executor's own `buttonFunctions` (S34). A client that read
    // `isActive` and sent a Go for a `Toggle` would race a second client doing
    // the same — see `desk/executorbar.tsx`.
    const onButton = useCallback(
        (executorId: number, index: number, pressed: boolean) => {
            send({
                t: "ExecutorButton",
                executorId,
                button: { t: "Slot", index },
                pressed,
            });
        },
        [send],
    );

    // The encoder bar's five.
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
    const onTurn = useCallback(
        (reading: ParameterReading, delta: number) => {
            // Relative, so the daemon starts from what the programmer holds — or
            // from the attribute's home value when it holds nothing. Working that out
            // here would be this interface deciding what a value *is*.
            send({ t: "SetAttribute", attribute: reading.attribute, value: delta, relative: true });
        },
        [send],
    );
    // A whole command with no argument: written and executed at once (§4.5).
    // The three-stage Clear is still the daemon's — the key says *clear*, and
    // which stage that is is `prism_core::Programmer`'s answer.
    const onClear = useCallback(() => {
        run("Clear");
    }, [run]);


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
            />
            {/*
              One band, two halves (S35). S26 gave each bar a full-width band of
              its own, which cost the canvas two bands of height and left the
              encoders 3.1 rem for five banks and six parameters. Side by side
              they take one, and the encoders get the room a real encoder needs.
              The band's height is fixed in the stylesheet rather than taken from
              its contents, for S26's reason: a bar that grew when an executor
              was assigned would move every window on the screen.
            */}
            <div className="deskband" data-testid="desk-band">
                <EncoderBar
                    session={documents.session}
                    show={documents.show}
                    programmer={documents.programmer}
                    onBank={onBank}
                    onParam={onParam}
                    onPage={onProgrammerPage}
                    onTurn={onTurn}
                    onClear={onClear}
                />
                <ExecutorBar
                    session={documents.session}
                    show={documents.show}
                    onPage={onPage}
                    onSelect={onSelect}
                    onMaster={onMaster}
                    onButton={onButton}
                />
            </div>
            <footer className="desk-footer">
                <CommandLine daemonLine={commandLine(documents.session)} />
                <StatusStrip session={documents.session} show={documents.show} />
            </footer>
        </>
    );
}

/**
 * One line of readings: the show, the session, and the engine.
 *
 * A console's status strip. It is here rather than in a window because it must
 * be true at a glance and without anybody having opened anything — and because
 * a `Settings` window that could be closed is the wrong place for *is the
 * engine running*.
 */
function StatusStrip({ session, show }: { readonly session: JsonValue; readonly show: JsonValue }) {
    const health = useDesk(selectHealth);
    const outputs = useDesk(selectOutputs);
    const unsaved = useDesk(selectUnsaved);
    if (health === null || outputs === null) {
        return null;
    }
    return (
        <dl className="status-strip" data-testid="status-strip">
            <Reading label="Fixtures" value={text(countAt(show, "/fixtures"))} testId="fixtures" />
            <Reading label="Groups" value={text(countAt(show, "/groups"))} testId="groups" />
            <Reading label="Seqs" value={text(countAt(show, "/sequences"))} testId="sequences" />
            <Reading label="Execs" value={text(countAt(show, "/executors"))} testId="executors" />
            <Reading
                label="View"
                value={text(numberAt(session, "/session/activeViewId"))}
                testId="active-view"
            />
            <Reading
                label="Windows"
                value={text(countAt(session, "/session/openWindows"))}
                testId="open-windows"
            />
            <Reading
                label="Page"
                value={text(numberAt(session, "/session/executorPage"))}
                testId="executor-page"
            />
            <Reading
                label="Bank"
                value={text(stringAt(session, "/session/encoderBank"))}
                testId="encoder-bank"
            />
            <Reading
                label="Param"
                value={text(numberAt(session, "/session/programmerParamIndex"))}
                testId="param-index"
            />
            <Reading label="Protocol" value={String(health.protocolVersion)} testId="protocol" />
            <Reading label="Tick" value={`${health.tickHz.toFixed(1)} Hz`} testId="tick-hz" />
            <Reading label="Missed" value={String(health.missedTicks)} testId="missed-ticks" />
            <Reading label="Show" value={unsaved ? "unsaved changes" : "saved"} testId="dirty-flag" />
            {outputs.map((output) => (
                <Reading
                    key={output.id}
                    label={`Out ${String(output.id)}`}
                    value={`${output.name}: ${output.health}`}
                    testId={`output-${String(output.id)}`}
                />
            ))}
        </dl>
    );
}

/** One label and one value. */
function Reading({
    label,
    value,
    testId,
}: {
    readonly label: string;
    readonly value: string;
    readonly testId: string;
}) {
    return (
        <div className="reading">
            <dt>{label}</dt>
            <dd data-testid={testId}>{value}</dd>
        </div>
    );
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
 * # The removal waits for the animation, and does not use a timer
 *
 * `transitionend` is what says the collapse has finished, so the duration lives
 * in the stylesheet alone. A `setTimeout` here would be a second copy of the
 * 300 ms, and the two would disagree the first time somebody adjusted the CSS.
 */
function Notices() {
    const notices = useDesk(selectNotices);
    const store = useDeskStore();
    const [leaving, setLeaving] = useState<ReadonlySet<number>>(() => new Set());

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

/** A reading that is not in the document reads as an em dash, not as zero. */
function text(value: number | string | null): string {
    return value === null ? "—" : String(value);
}
