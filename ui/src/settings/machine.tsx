/**
 * The *This machine* panel: what `prismd` used to be told on a command line —
 * S37.
 *
 * # Every row is one command
 *
 * `ConfigureMachine` carries one field, which is `ConfigureOutput`'s rule and
 * `SetCueProperty`'s before it: a command carrying the whole of
 * `MachineSettings` would make this panel read it, change one member and send
 * the rest back — and two operators in two settings windows, one turning on
 * autostart and one changing the log level, would each undo the other.
 *
 * # A row a flag is holding says so
 *
 * A flag is the value for that run and the stored setting is neither read nor
 * written (S33's rule for the rig, S36's for the surface, S37's for the rest).
 * So a held row is drawn, disabled, with the flag named beside it — because a
 * box an operator can type into that does nothing is worse than a box that is
 * not there.
 *
 * # And a row that waits for a restart says that too
 *
 * `MachineChange::needs_restart` is the domain's answer and
 * {@link needsRestart} is the lookup. A listener is bound once and a frame
 * layout is built once; **a new desk identity is deliberately in that group**,
 * because an sACN source that changed its CID mid-show would fight the source it
 * used to be for the two and a half seconds a receiver's timeout runs.
 *
 * # The autostart row says what the machine has, not only what was asked for
 *
 * S37 wrote the switch down and nobody acted on it; **S29's shell is what acts**
 * — it is the one process that can write a `HKCU\…\Run` value without
 * administrator rights (`ARCHITECTURE_SPEC.md` §10.3). Two states follow from
 * that, and they can disagree: the *setting*, which is `MachineConfig`'s and
 * which every client reads back, and the *start-up entry*, which is a fact about
 * this Windows account that a person can delete in Task Manager without this
 * program hearing about it.
 *
 * A tick beside a setting whose entry is not there would be a switch displaying
 * a lie, so the row draws both: the box sends `ConfigureMachine` as it always
 * did, and beside it {@link autostartState} says what the machine actually has.
 * In a browser there is no shell to ask, and the row says *that* rather than
 * guessing — which is `isHeld`'s rule about a box that does nothing, one panel
 * along.
 *
 * # The data directory is shown and not edited
 *
 * The settings themselves live in it, so a daemon told to move it would have to
 * be told somewhere else — `ARCHITECTURE_SPEC.md` §10.3 has the mechanism it
 * would need and does not have. What the panel does instead is name it, so an
 * operator looking for `machine.json` knows where to look.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import type { ExitAction, LogLevel, MachineChange, MachineSettings } from "../bindings";
import type { AutostartReport } from "../shell/bridge";
import { autostartApply, autostartState, choosePath, inShell } from "../shell/bridge";
import { useDesk, useSend } from "../store/hooks";
import type { DeskState } from "../store/desk";
import {
  EXIT_ACTIONS,
  LOG_LEVELS,
  autostartEntryText,
  exitText,
  heldNote,
  isHeld,
  isLoopback,
  listenerText,
} from "./settings";

const selectMachine = (state: DeskState): MachineSettings | null => state.machine;

/** The whole panel. */
export function MachinePanel() {
  const machine = useDesk(selectMachine);
  const send = useSend();
  const change = useCallback(
    (what: MachineChange) => {
      send({ t: "ConfigureMachine", change: what });
    },
    [send],
  );

  if (machine === null) {
    return <p className="window-note">The daemon has not said what this machine is set to.</p>;
  }

  return (
    <div className="settings-panel" data-testid="settings-machine">
      <Identity machine={machine} onChange={change} />
      <Network machine={machine} onChange={change} />
      <Behaviour machine={machine} onChange={change} />
    </div>
  );
}

/** Who this desk is, and where its own files are. */
function Identity({
  machine,
  onChange,
}: {
  readonly machine: MachineSettings;
  readonly onChange: (change: MachineChange) => void;
}) {
  return (
    <section className="settings-group" data-testid="machine-identity">
      <h3>This desk</h3>
      <dl className="status-strip">
        <div className="reading">
          <dt>Identity</dt>
          <dd data-testid="machine-desk-id">{machine.deskId}</dd>
        </div>
        <div className="reading">
          <dt>Data directory</dt>
          <dd data-testid="machine-data-dir">{machine.dataDir}</dd>
        </div>
      </dl>
      <p className="settings-hint">
        The identity is this desk&rsquo;s sACN source, generated once. Two desks sharing one look to
        a receiver like a single source contradicting itself, which is what a laptop imaged onto
        twenty produces — give this one a new identity if that has happened.{" "}
        <strong>It takes effect at the next start</strong>: changing it while sACN is running would
        make a second source that fights the first for two and a half seconds.
      </p>
      <p className="settings-hint">
        The data directory holds the lock file, <code>machine.json</code> and this desk&rsquo;s own
        show. It is shown rather than edited, because these settings are in it.
      </p>
      <button
        type="button"
        data-testid="machine-new-identity"
        onClick={() => {
          onChange({ t: "NewIdentity" });
        }}
      >
        Give this desk a new identity
      </button>
    </section>
  );
}

/** How this desk can be reached, and what protects it. */
function Network({
  machine,
  onChange,
}: {
  readonly machine: MachineSettings;
  readonly onChange: (change: MachineChange) => void;
}) {
  const [typed, setTyped] = useState<string | null>(null);
  const address = typed ?? machine.websocket ?? "";
  const heldSocket = isHeld(machine, "Websocket");
  const heldToken = isHeld(machine, "Token");
  const wouldReachOut = address.trim() !== "" && !isLoopback(address.trim());

  return (
    <section className="settings-group" data-testid="machine-network">
      <h3>Network</h3>
      <p className="settings-reading" data-testid="listener-state">
        {listenerText(machine)}
      </p>
      <label>
        <input
          type="checkbox"
          checked={machine.local}
          disabled={isHeld(machine, "Local")}
          data-testid="machine-local"
          onChange={(event) => {
            onChange({ t: "Local", local: event.target.checked });
          }}
        />
        Open the local pipe, which is what the desktop shell uses
        <Held note={heldNote(machine, "Local")} restart />
      </label>
      <form
        data-testid="websocket-form"
        onSubmit={(event) => {
          event.preventDefault();
          onChange({ t: "Websocket", address: address.trim() === "" ? null : address.trim() });
          setTyped(null);
        }}
      >
        <label>
          WebSocket address
          <input
            data-testid="machine-websocket"
            value={address}
            disabled={heldSocket}
            placeholder="127.0.0.1:7373 — blank for no listener at all"
            onChange={(event) => {
              setTyped(event.target.value);
            }}
          />
        </label>
        <button type="submit" data-testid="machine-websocket-apply" disabled={heldSocket}>
          Apply
        </button>
        <Held note={heldNote(machine, "Websocket")} restart />
      </form>
      {wouldReachOut && machine.token === null ? (
        <p className="settings-warning" role="status" data-testid="needs-token">
          That address is reachable from other machines, so it needs an access token. Make one
          first — the daemon refuses the address without it, because an unauthenticated lighting
          console on a school network is not something to leave to a click.
        </p>
      ) : null}
      <div className="settings-row" data-testid="machine-token-row">
        <span>Access token</span>
        <code data-testid="machine-token">{machine.token ?? "none"}</code>
        <button
          type="button"
          data-testid="machine-new-token"
          disabled={heldToken}
          onClick={() => {
            onChange({ t: "NewToken" });
          }}
        >
          Make one
        </button>
        {machine.token === null ? null : (
          <button
            type="button"
            data-testid="machine-clear-token"
            disabled={heldToken}
            onClick={() => {
              onChange({ t: "Token", token: null });
            }}
          >
            Remove it
          </button>
        )}
        <Held note={heldNote(machine, "Token")} restart />
      </div>
      <p className="settings-hint">
        The token is made by the daemon and shown here so it can be typed into the phone in the
        auditorium. Removing it puts the listener back on loopback, because a listener off loopback
        with no token is a state this desk will not be in.
      </p>
    </section>
  );
}

/** What this desk does, as opposed to who it is. */
function Behaviour({
  machine,
  onChange,
}: {
  readonly machine: MachineSettings;
  readonly onChange: (change: MachineChange) => void;
}) {
  const [universes, setUniverses] = useState<string | null>(null);
  const [library, setLibrary] = useState<string | null>(null);
  const typedUniverses = universes ?? String(machine.universes);
  const count = Number(typedUniverses.trim());
  const usable = Number.isInteger(count) && count >= 1 && count <= 64;

  return (
    <section className="settings-group" data-testid="machine-behaviour">
      <h3>This daemon</h3>
      <label>
        Log level
        <select
          data-testid="machine-log-level"
          value={machine.logLevel}
          disabled={isHeld(machine, "LogLevel")}
          onChange={(event) => {
            const level = LOG_LEVELS.find((candidate) => candidate === event.target.value);
            if (level !== undefined) {
              onChange({ t: "LogLevel", level: level satisfies LogLevel });
            }
          }}
        >
          {LOG_LEVELS.map((level) => (
            <option key={level} value={level}>
              {level}
            </option>
          ))}
        </select>
        <Held note={heldNote(machine, "LogLevel")} restart={false} />
      </label>
      <label>
        When the daemon stops
        <select
          data-testid="machine-exit"
          value={machine.exitAction}
          disabled={isHeld(machine, "ExitAction")}
          onChange={(event) => {
            const action = EXIT_ACTIONS.find((candidate) => candidate === event.target.value);
            if (action !== undefined) {
              onChange({ t: "ExitAction", action: action satisfies ExitAction });
            }
          }}
        >
          {EXIT_ACTIONS.map((action) => (
            <option key={action} value={action}>
              {exitText(action)}
            </option>
          ))}
        </select>
        <Held note={heldNote(machine, "ExitAction")} restart={false} />
      </label>
      <Autostart machine={machine} onChange={onChange} />
      <form
        data-testid="universes-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (usable) {
            onChange({ t: "Universes", universes: count });
            setUniverses(null);
          }
        }}
      >
        <label>
          Universes
          <input
            data-testid="machine-universes"
            inputMode="numeric"
            value={typedUniverses}
            disabled={isHeld(machine, "Universes")}
            onChange={(event) => {
              setUniverses(event.target.value);
            }}
          />
        </label>
        <button
          type="submit"
          data-testid="machine-universes-apply"
          disabled={!usable || isHeld(machine, "Universes")}
        >
          Apply
        </button>
        <Held note={heldNote(machine, "Universes")} restart />
      </form>
      <form
        data-testid="library-form"
        onSubmit={(event) => {
          event.preventDefault();
          const path = (library ?? machine.fixtureLibrary ?? "").trim();
          onChange({ t: "FixtureLibrary", path: path === "" ? null : path });
          setLibrary(null);
        }}
      >
        <label>
          Fixture library
          <input
            data-testid="machine-library"
            value={library ?? machine.fixtureLibrary ?? ""}
            disabled={isHeld(machine, "FixtureLibrary")}
            placeholder="blank to look beside the executable"
            onChange={(event) => {
              setLibrary(event.target.value);
            }}
          />
        </label>
        {inShell() && !isHeld(machine, "FixtureLibrary") ? (
          <button
            type="button"
            data-testid="machine-library-browse"
            onClick={() => {
              void choosePath("FixtureLibrary", library ?? machine.fixtureLibrary ?? "").then(
                (chosen) => {
                  if (chosen !== null) {
                    setLibrary(chosen);
                  }
                },
              );
            }}
          >
            Browse&hellip;
          </button>
        ) : null}
        <button
          type="submit"
          data-testid="machine-library-apply"
          disabled={isHeld(machine, "FixtureLibrary")}
        >
          Apply
        </button>
        <Held note={heldNote(machine, "FixtureLibrary")} restart />
      </form>
    </section>
  );
}

/**
 * The switch, and what this machine actually has.
 *
 * The box writes the **setting** (`ConfigureMachine`), which is what every
 * client reads back and what the daemon stores; the shell writes the **entry**,
 * which is what Windows reads at log-in. Both, in that order, because the second
 * is not always possible and the first must be recorded whether or not it was:
 * a desk configured from a browser stores the flag now and the shell obeys it at
 * its next start, which is exactly what `autostart::reconcile` is for.
 */
function Autostart({
  machine,
  onChange,
}: {
  readonly machine: MachineSettings;
  readonly onChange: (change: MachineChange) => void;
}) {
  const [entry, setEntry] = useState<AutostartReport | null>(null);
  const wanted = machine.autostart;
  const settled = useRef<boolean | null>(null);

  // **The entry is written when the daemon confirms the setting, not when the
  // box is clicked** — which is D3 applied to a side effect. The switch is the
  // *daemon's* state: a second window, or a Web Remote, can turn it on, and a
  // shell that acted on its own click would be acting on a value it does not
  // own and might not get. Following `machine.autostart` means every one of
  // those routes reaches the registry, and it means a refused command writes
  // nothing.
  //
  // The first pass **reads** rather than writes, which is what lets this row
  // show a disagreement at all: an entry somebody deleted in Task Manager would
  // otherwise be silently repaired by opening the panel, and nobody would ever
  // learn it had gone. Putting it back is a click, and the row says so.
  useEffect(() => {
    let current = true;
    const first = settled.current === null;
    const changed = settled.current !== wanted;
    settled.current = wanted;
    const asking = first || !changed ? autostartState() : autostartApply(wanted);
    void asking.then((report) => {
      if (current) {
        setEntry(report);
      }
    });
    return () => {
      current = false;
    };
  }, [wanted]);

  return (
    <>
      <label>
        <input
          type="checkbox"
          checked={machine.autostart}
          data-testid="machine-autostart"
          onChange={(event) => {
            onChange({ t: "Autostart", autostart: event.target.checked });
          }}
        />
        Start this desk when the machine starts
      </label>
      <p className="settings-hint" data-testid="autostart-note">
        Written down here and acted on by the desktop shell, which is the one process that can add
        a start-up entry without administrator rights. A daemon started by hand is unaffected.
      </p>
      <p className="settings-reading" data-testid="autostart-entry">
        {autostartEntryText(wanted, entry)}
      </p>
    </>
  );
}

/**
 * The note beside a row: which flag is holding it, and whether it waits.
 *
 * `restart` is what the domain says of the change this row sends —
 * `MachineChange::needs_restart`, mirrored by `settings.ts`'s `needsRestart`,
 * and `machine.test.tsx` is what holds the two together: it asks the reader
 * about the change every marked row actually sends. A second list written out
 * here would be the one an operator believes, and it would be the wrong one.
 */
function Held({ note, restart }: { readonly note: string | null; readonly restart: boolean }) {
  if (note === null && !restart) {
    return null;
  }
  return (
    <small className="settings-note">
      {note === null ? "" : `${note}. `}
      {restart ? "Takes effect at the next start." : ""}
    </small>
  );
}
