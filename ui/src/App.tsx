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
import type { Command, FeatureGroup, JsonValue, WindowType } from "./bindings";
import { Canvas } from "./canvas/canvas";
import type { Rect } from "./canvas/geometry";
import { ViewBar } from "./canvas/viewbar";
import { CommandLine } from "./desk/commandline";
import { EncoderBar } from "./desk/encoderbar";
import { ExecutorBar } from "./desk/executorbar";
import type { ParameterReading } from "./desk/programmer";
import { commandLine } from "./desk/session";
import type { ConnectionStatus } from "./ipc/connection";
import { countAt, numberAt, stringAt } from "./mirror/select";
import { statusText } from "./status";
import { useDesk, useSend } from "./store/hooks";
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
    const connected = status.kind === "connected";
    return (
        <main className="desk">
            <header className="desk-header">
                <h1>PrismDMX</h1>
                <StatusPill status={status} />
                {connected ? <Views /> : null}
            </header>
            {connected ? <Desk /> : <NotConnected status={status} />}
            <Notices />
        </main>
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
                    : "The engine is not answering. Check that it is running and that the network is working."}
            </p>
        </section>
    );
}

/** The View Selector Bar, once there is a session to read it from. */
function Views() {
    const documents = useDesk(selectDocuments);
    const send = useSend();
    const onSelectView = useCallback(
        (viewId: number) => {
            send({ t: "SelectView", viewId });
        },
        [send],
    );
    const onStoreView = useCallback(
        (viewId: number, name: string) => {
            send({ t: "StoreView", viewId, name });
        },
        [send],
    );
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
            onOpenWindow={onOpenWindow}
        />
    );
}

/** The canvas, the two bars and the strip under them. */
function Desk() {
    const documents = useDesk(selectDocuments);
    const send = useSend();

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

    // The executor bar's five.
    const onPage = useCallback(
        (page: number) => {
            send({ t: "SetExecutorPage", page });
        },
        [send],
    );
    const onSelect = useCallback(
        (executorId: number) => {
            send({ t: "SelectExecutor", executorId });
        },
        [send],
    );
    const onMaster = useCallback(
        (executorId: number, level: number) => {
            send({ t: "SetExecutorMaster", executorId, level });
        },
        [send],
    );
    const onGo = useCallback(
        (executorId: number, direction: "Next" | "Prev") => {
            send({ t: "ExecutorGo", executorId, direction });
        },
        [send],
    );
    const onOff = useCallback(
        (executorId: number) => {
            send({ t: "ExecutorOff", executorId });
        },
        [send],
    );

    // The encoder bar's four.
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
    const onTurn = useCallback(
        (reading: ParameterReading, delta: number) => {
            // Relative, so the daemon starts from what the programmer holds — or
            // from the attribute's home value when it holds nothing. Working that out
            // here would be this interface deciding what a value *is*.
            send({ t: "SetAttribute", attribute: reading.attribute, value: delta, relative: true });
        },
        [send],
    );
    const onClear = useCallback(() => {
        send({ t: "ClearProgrammer" });
    }, [send]);

    // The command line's two.
    const onCommands = useCallback(
        (commands: readonly Command[]) => {
            for (const command of commands) {
                send(command);
            }
        },
        [send],
    );
    const onText = useCallback(
        (text: string) => {
            send({ t: "CommandLineInput", text });
        },
        [send],
    );

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
            <EncoderBar
                session={documents.session}
                show={documents.show}
                programmer={documents.programmer}
                onBank={onBank}
                onParam={onParam}
                onTurn={onTurn}
                onClear={onClear}
            />
            <ExecutorBar
                session={documents.session}
                show={documents.show}
                onPage={onPage}
                onSelect={onSelect}
                onMaster={onMaster}
                onGo={onGo}
                onOff={onOff}
            />
            <footer className="desk-footer">
                <CommandLine
                    daemonLine={commandLine(documents.session)}
                    onCommands={onCommands}
                    onText={onText}
                />
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
        <dl className="strip" data-testid="status-strip">
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

/** Messages from the daemon, newest last. */
function Notices() {
    const notices = useDesk(selectNotices);
    const [dismissedIds, setDismissedIds] = useState<Array<number | string>>([]);

    const handleDismiss = (id: number | string) => {
        // 1. Klasse 'notice-dismissed' sofort setzen (startet CSS-Animation)
        setDismissedIds((prev) => [...prev, id]);

        // 2. Element erst nach Ende der CSS-Animation (300ms) komplett entfernen
        setTimeout(() => {
            // Falls du eine Action hast, kannst du hier auch optional den Store benachrichtigen
        }, 300);
    };

    // Prüfen, ob noch nicht vollständig ausgeblendete Elemente existieren
    const hasVisibleNotices = notices.some((notice) => !dismissedIds.includes(notice.id));

    // Wenn keine Notices mehr da sind, wird der Rahmen (.notices) aus dem DOM entfernt
    if (notices.length === 0 || (!hasVisibleNotices && dismissedIds.length === notices.length)) {
        return null;
    }

    return (
        <section className="notices" data-testid="notices">
            <ul>
                {notices.map((notice) => {
                    const isDismissed = dismissedIds.includes(notice.id);

                    return (
                        <li
                            key={notice.id}
                            className={`notice notice-${notice.level.toLowerCase()} ${isDismissed ? "notice-dismissed" : ""
                                }`}
                        >
                            <div className="notice-wrapper">
                                <div className="notice-content">
                                    {notice.message}
                                </div>
                                <button
                                    type="button"
                                    className="notice-close"
                                    data-testid={`notice-close-${notice.id}`}
                                    onClick={() => handleDismiss(notice.id)}
                                    aria-label="Notice schließen"
                                >
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        height="20px"
                                        viewBox="0 -960 960 960"
                                        width="20px"
                                        fill="currentColor"
                                    >
                                        <path d="m256-200-56-56 224-224-224-224 56-56 224 224 224-224 56 56-224 224 224 224-56 56-224-224-224 224Z" />
                                    </svg>
                                </button>
                            </div>
                        </li>
                    );
                })}
            </ul>
        </section>
    );
}

/** A reading that is not in the document reads as an em dash, not as zero. */
function text(value: number | string | null): string {
    return value === null ? "—" : String(value);
}
