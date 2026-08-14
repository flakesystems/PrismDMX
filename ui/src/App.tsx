/**
 * The interface: a device screen.
 *
 * `CLAUDE.md` describes what this has to be — *built like an in-device screen,
 * no scrolling outside the canvas, fast recognition of sections before
 * aesthetics* — so the shape is fixed and the middle is where everything
 * happens:
 *
 * ```text
 *   header   title, connection, the View Selector Bar, Add window
 *   canvas   the windows the session says are open   (all remaining height)
 *   footer   the command line, and one strip of readings
 * ```
 *
 * # Nothing on this screen is state this interface holds
 *
 * The canvas is `openWindows`. The lit view button is `activeViewId`. The
 * readings are the mirror. The one thing that is local is what has been *typed*
 * into the command line, which is D3's own illustration and was S23's.
 *
 * That is the whole of S25's first exit criterion: opening, moving and closing
 * a window issues a session command, and there is nowhere here for the answer
 * to be kept instead.
 */

import type { FormEvent } from "react";
import { useCallback, useState } from "react";

import "./App.css";
import type { JsonValue, WindowType } from "./bindings";
import { Canvas } from "./canvas/canvas";
import type { Rect } from "./canvas/geometry";
import { ViewBar } from "./canvas/viewbar";
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
          : "The engine is not answering. The show is unaffected by this window: prismd holds the show, the session and every output, and DMX keeps running with no interface attached."}
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

/** The canvas and the strip under it. */
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

  if (documents === null) {
    return null;
  }
  return (
    <>
      <Canvas
        session={documents.session}
        show={documents.show}
        onPlace={onPlace}
        onFocus={onFocus}
        onClose={onClose}
      />
      <footer className="desk-footer">
        <CommandLine daemonLine={stringAt(documents.session, "/session/commandLine") ?? ""} />
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

/**
 * The command line, and the smallest honest illustration of **D3**.
 *
 * The input holds what has been typed — *local* input, which the daemon has
 * never been told about and which is nobody else's business. Pressing Enter
 * sends a `CommandLineInput` command. What is displayed beside it is the
 * session's command line **as the daemon holds it**, and it changes when the
 * `SessionPatch` comes back and not a moment sooner. Nothing here is
 * optimistic; there is no state to roll back if the command is refused.
 *
 * S26 owns the real one, including the parser.
 */
function CommandLine({ daemonLine }: { readonly daemonLine: string }) {
  const send = useSend();
  const [typed, setTyped] = useState("");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    send({ t: "CommandLineInput", text: typed });
  };

  return (
    <div className="command-line" data-testid="command-line-panel">
      <form onSubmit={submit}>
        <label htmlFor="command-input">Command</label>
        <input
          id="command-input"
          data-testid="command-input"
          value={typed}
          onChange={(event) => {
            setTyped(event.target.value);
          }}
          autoComplete="off"
        />
      </form>
      <p className="daemon-line">
        Engine: <output data-testid="command-line">{daemonLine}</output>
      </p>
    </div>
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
  if (notices.length === 0) {
    return null;
  }
  return (
    <section className="notices" data-testid="notices">
      <ul>
        {notices.map((notice) => (
          <li key={notice.id} className={`notice notice-${notice.level.toLowerCase()}`}>
            {notice.message}
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
