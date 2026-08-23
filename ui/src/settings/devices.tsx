/**
 * The Devices panel: the MIDI ports of S36, chosen and watched — S37.
 *
 * # The list is asked for, and that is the decision rather than a detail
 *
 * What is plugged into the daemon's machine is **not state the daemon owns**: it
 * changes when a person moves a plug, no command causes it, and a client that
 * mirrored it would be holding the operating system's opinion from whenever it
 * last connected. So the panel asks when it opens and asks again when the
 * operator presses *Rescan* — which is the gesture that exists precisely because
 * plugging a desk in produces no message.
 *
 * `Delta::SurfaceChanged` is the other half: it says which port is *chosen*, so
 * two settings windows cannot disagree about which desk this is, and it is what
 * tells this panel that asking again is worth it.
 *
 * # Configured and absent at the same time
 *
 * A desk that is switched off is named and not there, and that is an ordinary
 * state rather than an error — `configured` need not appear in `ports`. The
 * panel draws it as a row of its own, because an operator who cannot see the
 * name they chose cannot tell *nothing is configured* from *the thing I chose is
 * unplugged*.
 */

import { useCallback, useEffect, useState } from "react";

import type { Answer, MidiPortInfo, SurfaceStatus } from "../bindings";
import { useAsk, useDesk, useSend } from "../store/hooks";
import type { DeskState } from "../store/desk";
import { counterRows, healthText, heldNote, isHeld } from "./settings";

const selectSurfacePort = (state: DeskState): string | null => state.surfacePort;
const selectMachine = (state: DeskState) => state.machine;

/** What the daemon last said about this machine's MIDI. */
interface Ports {
  readonly ports: readonly MidiPortInfo[];
  readonly configured: string | null;
  readonly open: string | null;
  readonly status: SurfaceStatus | null;
}

const NOTHING: Ports = { ports: [], configured: null, open: null, status: null };

/** The answer, if it is the one that was asked for. */
function portsOf(answer: Answer | null): Ports | null {
  return answer !== null && answer.t === "MidiPorts"
    ? {
        ports: answer.ports,
        configured: answer.configured,
        open: answer.open,
        status: answer.status,
      }
    : null;
}

/** The whole panel. */
export function DevicesPanel() {
  const ask = useAsk();
  const send = useSend();
  // The configured port from the mirror rather than from the answer, so a
  // *second* client choosing one moves this list without anybody pressing
  // Rescan. The answer carries it too, and the two agree; this is the one that
  // arrives by itself.
  const chosen = useDesk(selectSurfacePort);
  const machine = useDesk(selectMachine);
  const [ports, setPorts] = useState<Ports>(NOTHING);
  const [asked, setAsked] = useState(0);

  useEffect(() => {
    let current = true;
    void ask({ t: "MidiPorts" }).then((answer) => {
      const read = portsOf(answer);
      if (current && read !== null) {
        setPorts(read);
      }
    });
    return () => {
      current = false;
    };
    // `asked` is what *Rescan* increments, and `chosen` is what a second client
    // changing the port moves. Both are reasons to ask again; nothing else is.
  }, [ask, asked, chosen]);

  const choose = useCallback(
    (port: string | null) => {
      send({ t: "SetSurfacePort", port });
    },
    [send],
  );

  const held = isHeld(machine, "Surface");
  const rows = ports.ports.filter((port) => port.input || port.output);
  const configuredIsThere =
    chosen === null || rows.some((port) => port.name === chosen) || ports.open === chosen;

  return (
    <div className="settings-panel" data-testid="settings-devices">
      <div className="settings-bar">
        <span data-testid="port-count">
          {rows.length} MIDI {rows.length === 1 ? "port" : "ports"}
        </span>
        <button
          type="button"
          data-testid="ports-rescan"
          onClick={() => {
            setAsked((count) => count + 1);
          }}
        >
          Rescan
        </button>
      </div>
      {held ? (
        <p className="settings-hint" data-testid="surface-held">
          {heldNote(machine, "Surface")}, so choosing one here is refused. Start the daemon without
          that flag to configure the port from this window.
        </p>
      ) : null}
      <table className="sheet" data-testid="port-table">
        <thead>
          <tr>
            <th scope="col">Port</th>
            <th scope="col">Direction</th>
            <th scope="col" />
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td colSpan={3} data-testid="no-ports">
                No MIDI ports. That is what a laptop with nothing plugged in looks like, and it is
                an answer rather than a failure to look.
              </td>
            </tr>
          ) : null}
          {rows.map((port) => (
            <tr
              key={port.name}
              className={port.name === chosen ? "row-editing" : ""}
              data-testid={`port-${port.name}`}
            >
              <td>{port.name}</td>
              <td>{port.input && port.output ? "in + out" : port.input ? "in" : "out"}</td>
              <td>
                <button
                  type="button"
                  disabled={held || port.name === chosen || !(port.input && port.output)}
                  data-testid={`port-choose-${port.name}`}
                  onClick={() => {
                    choose(port.name);
                  }}
                >
                  {port.name === chosen ? "Chosen" : "Use this"}
                </button>
              </td>
            </tr>
          ))}
          {/* Named and absent at the same time: the row an operator has to see,
              because *nothing is configured* and *the desk I chose is switched
              off* are different facts. */}
          {configuredIsThere ? null : (
            <tr className="row-conflict" data-testid="port-missing">
              <td>{chosen}</td>
              <td>configured, not there</td>
              <td>
                <button
                  type="button"
                  disabled={held}
                  data-testid="port-clear"
                  onClick={() => {
                    choose(null);
                  }}
                >
                  Forget it
                </button>
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <SurfaceReadings chosen={chosen} open={ports.open} status={ports.status} />
      <ProfileRow />
    </div>
  );
}

/** What the attached desk is doing, if there is one. */
function SurfaceReadings({
  chosen,
  open,
  status,
}: {
  readonly chosen: string | null;
  readonly open: string | null;
  readonly status: SurfaceStatus | null;
}) {
  if (status === null) {
    return (
      <p className="window-note" data-testid="no-surface">
        {chosen === null
          ? "No control surface is configured. A desk being programmed on a laptop has none, and that is an ordinary state."
          : `${chosen} is configured and no surface is attached to this run.`}
      </p>
    );
  }
  return (
    <section className="settings-readings" data-testid="surface-readings">
      <dl className="status-strip">
        <Reading label="Health" value={healthText(status.health)} testId="surface-health" />
        <Reading label="Open on" value={open ?? "—"} testId="surface-open" />
        <Reading
          label="Bindings"
          value={`${String(status.boundControls)} controls`}
          testId="surface-bound"
        />
        {counterRows(status).map((row) => (
          <Reading
            key={row.label}
            label={row.label}
            value={String(row.value)}
            testId={`surface-${row.label.toLowerCase().replace(/\s+/gu, "-")}`}
          />
        ))}
      </dl>
      {/* The remedy is the **daemon's words**, not this file's, and that is the
          point of carrying it: the obvious advice for a desk that has stopped
          sending is *reconnect*, and S20 established that reconnecting is the
          one thing that cannot recover it. */}
      {status.remedy === null ? null : (
        <p className="settings-warning" role="status" data-testid="surface-remedy">
          {status.remedy}
        </p>
      )}
    </section>
  );
}

/**
 * The binding table, named and reloaded.
 *
 * **Naming it again is the reload**, which is why there is no second command: a
 * table edited beside a running daemon is picked up by pointing at it once more,
 * and a separate *reload* verb would be a second way of saying one thing. A
 * profile that is missing or malformed is reported and the built-in table
 * stands — S22's rule, so this can never leave a desk without keys.
 */
function ProfileRow() {
  const send = useSend();
  const machine = useDesk(selectMachine);
  const held = isHeld(machine, "SurfaceProfile");
  const [typed, setTyped] = useState<string | null>(null);
  const path = typed ?? machine?.surfaceProfile ?? "";

  return (
    <form
      className="settings-form"
      data-testid="profile-form"
      onSubmit={(event) => {
        event.preventDefault();
        send({
          t: "ConfigureMachine",
          change: { t: "SurfaceProfile", path: path.trim() === "" ? null : path.trim() },
        });
        setTyped(null);
      }}
    >
      <fieldset>
        <legend>Binding profile</legend>
        <label>
          File
          <input
            data-testid="profile-path"
            value={path}
            disabled={held}
            placeholder="profiles/surface/xtouch.json — blank for the built-in table"
            onChange={(event) => {
              setTyped(event.target.value);
            }}
          />
        </label>
      </fieldset>
      <p className="settings-hint" data-testid="profile-note">
        {held
          ? `${heldNote(machine, "SurfaceProfile") ?? ""}.`
          : "Naming a file again re-reads it, which is how a table edited beside the daemon is picked up. A file that is missing or malformed is reported and the built-in bindings stand."}
      </p>
      <div className="settings-actions">
        <button type="submit" data-testid="profile-apply" disabled={held}>
          Read it
        </button>
      </div>
    </form>
  );
}

/** One label and one value, as the status strip draws them. */
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
