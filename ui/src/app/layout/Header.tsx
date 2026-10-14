/**
 * The top band: title and connection light, the View Selector Bar, the window
 * chooser's key, Clear.
 *
 * The light is the one reading that must be true without any window open — it
 * says whether everything else on the screen means anything.
 */

import {
  CLEAR_TITLES,
  clearStage,
  listOf,
  statusText,
  useConnected,
  useConnectionStatus,
  useProgrammer,
  useRunLine,
  useSend,
  useSession,
} from "../../core";
import { WindowPickerKey } from "./WindowPicker";

export function Header() {
  const connected = useConnected();
  return (
    <header className="band header">
      <StatusLight />
      <span className="title">PrismDMX</span>
      {connected ? <ViewBar /> : <span className="spacer" />}
      {connected ? <WindowPickerKey /> : null}
      {connected ? <ClearKey /> : null}
    </header>
  );
}

function StatusLight() {
  const status = useConnectionStatus();
  const text = statusText(status);
  return (
    <span className={`status status-${status.kind}`} data-testid="connection-status" title={text}>
      <span className="status-dot" aria-hidden="true" />
      <span className="status-word">{text}</span>
    </span>
  );
}

/** One key per stored view; the lit one is the session's `activeViewId`. */
function ViewBar() {
  const session = useSession();
  const send = useSend();
  if (session === null) {
    return <span className="spacer" />;
  }
  const active = session.session.activeViewId;
  return (
    <nav className="viewbar" aria-label="Views">
      {listOf(session.views).map((view) => (
        <button
          key={view.id}
          type="button"
          className="key"
          aria-pressed={view.id === active}
          onClick={() => send({ t: "SelectView", viewId: view.id })}
        >
          {view.name}
        </button>
      ))}
    </nav>
  );
}

/**
 * Clear: three presses, three stages (selection → active values → everything).
 * The daemon counts the stages; this only shows which one the next press is.
 */
function ClearKey() {
  const programmer = useProgrammer();
  const run = useRunLine();
  const stage = clearStage(programmer);
  return (
    <button
      type="button"
      className="key key-clear"
      data-testid="clear"
      data-stage={stage}
      disabled={stage === 0}
      title={CLEAR_TITLES[stage] ?? CLEAR_TITLES[0]}
      onClick={() => run("Clear")}
    >
      Clear
    </button>
  );
}
