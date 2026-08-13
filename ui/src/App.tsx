/**
 * The interface, as far as S23 takes it: what the daemon says, and one way to
 * say something back.
 *
 * There is no canvas, no window system and no executor bar yet — those are
 * S25 and S26. What is here is the thing the rest of them stand on, made
 * visible so it can be tested through a browser rather than argued about:
 *
 * - the connection state, said plainly and never hidden (§8);
 * - the three documents, read out of the mirror by pointer;
 * - **nothing at all** when there is no daemon, because a value from an engine
 *   that has stopped is worse than no value;
 * - a command line, which is the smallest complete illustration of D3: what is
 *   typed is *local input*, what is displayed underneath is the **daemon's**
 *   command line, and the second only ever changes because a delta said so.
 */

import type { FormEvent } from "react";
import { useState } from "react";

import "./App.css";
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
  return (
    <main className="desk">
      <header className="desk-header">
        <h1>PrismDMX</h1>
        <StatusPill status={status} />
      </header>
      {status.kind === "connected" ? <Connected /> : <NotConnected status={status} />}
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

/** Everything the mirror holds. */
function Connected() {
  const documents = useDesk(selectDocuments);
  const health = useDesk(selectHealth);
  const outputs = useDesk(selectOutputs);
  const unsaved = useDesk(selectUnsaved);
  if (documents === null || health === null || outputs === null) {
    return null;
  }

  const show = documents.show;
  const session = documents.session;
  return (
    <>
      <section className="panel" data-testid="engine">
        <h2>Engine</h2>
        <dl>
          <Reading label="Protocol" value={String(health.protocolVersion)} testId="protocol" />
          <Reading label="Tick" value={`${health.tickHz.toFixed(1)} Hz`} testId="tick-hz" />
          <Reading label="Missed ticks" value={String(health.missedTicks)} testId="missed-ticks" />
          <Reading
            label="Show"
            value={unsaved ? "unsaved changes" : "saved"}
            testId="dirty-flag"
          />
        </dl>
        <ul className="outputs">
          {outputs.map((output) => (
            <li key={output.id} data-testid={`output-${output.id}`}>
              {output.name}: {output.health}
            </li>
          ))}
        </ul>
      </section>

      <section className="panel" data-testid="session">
        <h2>Session</h2>
        <dl>
          <Reading
            label="Active view"
            value={text(numberAt(session, "/session/activeViewId"))}
            testId="active-view"
          />
          <Reading
            label="Executor page"
            value={text(numberAt(session, "/session/executorPage"))}
            testId="executor-page"
          />
          <Reading
            label="Encoder bank"
            value={text(stringAt(session, "/session/encoderBank"))}
            testId="encoder-bank"
          />
          <Reading
            label="Open windows"
            value={text(countAt(session, "/session/openWindows"))}
            testId="open-windows"
          />
        </dl>
      </section>

      <section className="panel" data-testid="show">
        <h2>Show</h2>
        <dl>
          <Reading label="Fixtures" value={text(countAt(show, "/fixtures"))} testId="fixtures" />
          <Reading label="Groups" value={text(countAt(show, "/groups"))} testId="groups" />
          <Reading
            label="Sequences"
            value={text(countAt(show, "/sequences"))}
            testId="sequences"
          />
          <Reading
            label="Executors"
            value={text(countAt(show, "/executors"))}
            testId="executors"
          />
        </dl>
      </section>

      <section className="panel" data-testid="programmer">
        <h2>Programmer</h2>
        <dl>
          <Reading
            label="Selected"
            value={String(documents.programmer.selection.length)}
            testId="selection"
          />
          <Reading
            label="Touched values"
            value={String(documents.programmer.values.length)}
            testId="touched"
          />
          <Reading
            label="Encoder bank"
            value={documents.programmer.activeFeatureGroup}
            testId="programmer-bank"
          />
        </dl>
      </section>

      <CommandLine daemonLine={stringAt(session, "/session/commandLine") ?? ""} />
    </>
  );
}

/**
 * The command line, and the smallest honest illustration of **D3**.
 *
 * The input holds what has been typed — *local* input, which the daemon has
 * never been told about and which is nobody else's business. Pressing Enter
 * sends a `CommandLineInput` command. What is displayed underneath is the
 * session's command line **as the daemon holds it**, and it changes when the
 * `SessionPatch` comes back and not a moment sooner. Nothing here is
 * optimistic; there is no state to roll back if the command is refused.
 */
function CommandLine({ daemonLine }: { readonly daemonLine: string }) {
  const send = useSend();
  const [typed, setTyped] = useState("");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    send({ t: "CommandLineInput", text: typed });
  };

  return (
    <section className="panel" data-testid="command-line-panel">
      <h2>Command line</h2>
      <form onSubmit={submit}>
        <label htmlFor="command-input">Type, then Enter</label>
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
        The engine&rsquo;s command line: <output data-testid="command-line">{daemonLine}</output>
      </p>
    </section>
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
    <section className="panel" data-testid="notices">
      <h2>Messages</h2>
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
