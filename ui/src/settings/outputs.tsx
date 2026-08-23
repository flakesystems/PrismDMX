/**
 * The Outputs panel: the rig of S33, edited — S37.
 *
 * # Nothing here is state this interface holds
 *
 * The table is `state.outputs`, which is the snapshot's and is kept current by
 * `Delta::OutputsChanged` and `Delta::OutputHealth`. What is local is exactly
 * one thing — **the row being typed into** — which is `ARCHITECTURE_SPEC.md`
 * §4.2's own category, and it is dropped the moment Apply is pressed rather than
 * merged with what comes back. That is `patch/patchwindow.tsx`'s resolution
 * taken a second time, and `canvas/drag.ts`'s before it.
 *
 * # Which universes go nowhere is asked, never worked out
 *
 * A patched universe no output carries is `prism_core::ShowIssue::UniverseNotOutput`,
 * and the panel asks for it (`Query::DarkUniverses`) rather than intersecting
 * the patch with the rig. Working it out here would be a second opinion about
 * something the daemon already decides — the duplication **D3** exists to
 * prevent, and the exact trap S27 wrote down about address overlaps.
 *
 * # The counters are asked for, on a cadence of this panel's own
 *
 * S33 left exactly one thing a settings window would need and could not be told:
 * a **live** frame counter. The snapshot carries one, so a row added while a
 * client is connected would show zero frames for ever — and the row an operator
 * has just made is the one they are watching to see whether it works. So the
 * panel asks (`Query::OutputStatus`) once a second while it is open, which is
 * `Query::MidiPorts`' shape one device along and is what a status panel wants.
 *
 * The **configuration** is not asked for and must not be: it arrives whole in
 * `Delta::OutputsChanged` whenever it moves, and a rig on the wire once a second
 * to carry a counter would be the wrong trade twice over.
 *
 * # One field per command
 *
 * `ConfigureOutput` carries one member, so the form sends one command per field
 * that actually changed rather than the whole row: two operators in two settings
 * windows, one re-addressing a node and one renaming it, must not each undo the
 * other. The **number** is not among them — it is the key the row is filed
 * under, and changing it is a `RemoveOutput` and an `AddOutput`, said out loud.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import type { OutputInstance, OutputKind, OutputStatusInfo } from "../bindings";
import type { OutputSnapshot } from "../ipc/protocol";
import { useAsk, useDesk, useSend } from "../store/hooks";
import type { DeskState } from "../store/desk";
import { readUniverses, writeUniverses } from "./settings";

const selectOutputs = (state: DeskState): readonly OutputSnapshot[] | null => state.outputs;
const selectShow = (state: DeskState) => state.documents?.show ?? null;

/**
 * How often the counters are re-read while this panel is open.
 *
 * A second: the number climbs at the output's own cadence (44 Hz for a network
 * node, 35 for an Open DMX cable), and a panel that redrew it that fast would be
 * unreadable as well as expensive. Nothing is asked when the panel is closed.
 */
const STATUS_INTERVAL_MS = 1000;

/** The kinds an operator can choose between, in `OutputKind`'s own order. */
const KINDS = ["Mock", "OpenDmx", "ArtNet", "Sacn"] as const;

/** What one of them is called on the screen. */
function kindLabel(kind: (typeof KINDS)[number]): string {
  switch (kind) {
    case "Mock":
      return "Mock — accepts frames and puts them nowhere";
    case "OpenDmx":
      return "Open DMX USB — one universe per cable";
    case "ArtNet":
      return "Art-Net — unicast to a node";
    case "Sacn":
      return "sACN (E1.31)";
  }
}

/** The row being typed into. Local, and dropped when it is submitted. */
interface Draft {
  /** The output number, which is the key everything else uses. */
  readonly id: number;
  /** The number this row started at, or `null` for a row that is being added. */
  readonly wasId: number | null;
  readonly name: string;
  readonly kind: (typeof KINDS)[number];
  /** The Open DMX adapter's serial, which pins one cable when there are two. */
  readonly serial: string;
  /** Node or receiver addresses, one per line. */
  readonly addresses: string;
  /** The sACN multicast hop limit. */
  readonly ttl: number;
  /** The universes, as they are typed. */
  readonly universes: string;
}

/** A draft built from a configured row. */
function draftOf(output: OutputInstance): Draft {
  return {
    id: output.id,
    wasId: output.id,
    name: output.name,
    kind: output.kind.t,
    serial: output.kind.t === "OpenDmx" ? (output.kind.serial ?? "") : "",
    addresses:
      output.kind.t === "ArtNet"
        ? output.kind.nodes.join("\n")
        : output.kind.t === "Sacn"
          ? output.kind.receivers.join("\n")
          : "",
    ttl: output.kind.t === "Sacn" ? output.kind.ttl : 1,
    universes: writeUniverses(output.universes),
  };
}

/**
 * The kind a draft describes.
 *
 * The **port mappings are kept from the row that is being edited** rather than
 * rebuilt: an Art-Net node's four rows are what an installer read off the back
 * of it, and a rename that threw them away would be the worst kind of silent
 * loss. A row being added has none, which is `ARCHITECTURE_SPEC.md` §7.0's
 * default — universe N goes to port N − 1.
 */
function kindOf(draft: Draft, existing: OutputInstance | null): OutputKind {
  const lines = draft.addresses
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
  switch (draft.kind) {
    case "Mock":
      return { t: "Mock" };
    case "OpenDmx":
      return { t: "OpenDmx", serial: draft.serial.trim() === "" ? null : draft.serial.trim() };
    case "ArtNet":
      return {
        t: "ArtNet",
        nodes: lines,
        sync: existing?.kind.t === "ArtNet" ? existing.kind.sync : false,
        ports: existing?.kind.t === "ArtNet" ? existing.kind.ports : [],
      };
    case "Sacn":
      return {
        t: "Sacn",
        receivers: lines,
        ttl: draft.ttl,
        ports: existing?.kind.t === "Sacn" ? existing.kind.ports : [],
      };
  }
}

/** The whole panel. */
export function OutputsPanel() {
  const outputs = useDesk(selectOutputs);
  const show = useDesk(selectShow);
  const send = useSend();
  const ask = useAsk();
  const [draft, setDraft] = useState<Draft | null>(null);
  const [dark, setDark] = useState<readonly number[]>([]);
  const [status, setStatus] = useState<readonly OutputStatusInfo[]>([]);

  // Which patched universes go nowhere — the daemon's answer, asked again
  // whenever the rig or the show moves, which is exactly when it can have
  // changed.
  useEffect(() => {
    let current = true;
    void ask({ t: "DarkUniverses" }).then((answer) => {
      if (current && answer !== null && answer.t === "DarkUniverses") {
        setDark(answer.universes);
      }
    });
    return () => {
      current = false;
    };
  }, [ask, outputs, show]);

  // …and what each driver is *doing*, which no delta carries.
  useEffect(() => {
    let current = true;
    const read = () => {
      void ask({ t: "OutputStatus" }).then((answer) => {
        if (current && answer !== null && answer.t === "OutputStatus") {
          setStatus(answer.outputs);
        }
      });
    };
    read();
    const timer = setInterval(read, STATUS_INTERVAL_MS);
    return () => {
      current = false;
      clearInterval(timer);
    };
  }, [ask]);

  // The configured row is the mirror's and the reading is the answer's, joined
  // on the number. A row with no reading yet keeps what the snapshot said, which
  // is right for the first second after the panel opens and for a client that
  // has just connected.
  const rows = useMemo(
    () =>
      (outputs ?? []).map((row) => {
        const reading = status.find((entry) => entry.id === row.id);
        return reading === undefined
          ? row
          : {
              ...row,
              health: reading.health,
              framesSent: reading.framesSent,
              lastError: reading.lastError,
              lastErrorAgoMs: reading.lastErrorAgoMs,
            };
      }),
    [outputs, status],
  );
  const nextId = useMemo(() => {
    const taken = new Set(rows.map((row) => row.id));
    let candidate = 1;
    while (taken.has(candidate)) {
      candidate += 1;
    }
    return candidate;
  }, [rows]);

  const add = useCallback(() => {
    setDraft({
      id: nextId,
      wasId: null,
      name: `Output ${String(nextId)}`,
      kind: "ArtNet",
      serial: "",
      addresses: "",
      ttl: 1,
      universes: "1",
    });
  }, [nextId]);

  const apply = useCallback(() => {
    if (draft === null) {
      return;
    }
    const universes = readUniverses(draft.universes);
    if (universes === null) {
      return;
    }
    const existing = rows.find((row) => row.id === draft.wasId)?.output ?? null;
    const kind = kindOf(draft, existing);
    if (draft.wasId === null || draft.wasId !== draft.id) {
      // A number change is a remove and an add, **said out loud**: the number is
      // the key the row is filed under, and `ConfigureOutput` deliberately
      // carries no `id` for that reason. The remove goes first, so the number
      // being taken up is free when the add arrives; both travel on one ordered
      // channel.
      if (draft.wasId !== null) {
        send({ t: "RemoveOutput", id: draft.wasId });
      }
      send({
        t: "AddOutput",
        output: {
          id: draft.id,
          name: draft.name,
          kind,
          universes,
          enabled: existing?.enabled ?? true,
        },
      });
    } else {
      // One command per field that actually moved. A rename costs the rig
      // nothing (`OutputInstance::needs_restart`), so sending three commands
      // where one field changed would make an operator watch their rig blink
      // for a typo they corrected.
      if (existing === null || existing.name !== draft.name) {
        send({ t: "ConfigureOutput", id: draft.id, change: { t: "Name", name: draft.name } });
      }
      if (existing === null || JSON.stringify(existing.kind) !== JSON.stringify(kind)) {
        send({ t: "ConfigureOutput", id: draft.id, change: { t: "Kind", kind } });
      }
      if (existing === null || writeUniverses(existing.universes) !== writeUniverses(universes)) {
        send({ t: "ConfigureOutput", id: draft.id, change: { t: "Universes", universes } });
      }
    }
    // **Dropped, not kept.** What the rig is comes back as `OutputsChanged`; a
    // draft held here until the delta arrived would be this interface having an
    // opinion about the machine for as long as the round trip took.
    setDraft(null);
  }, [draft, rows, send]);

  return (
    <div className="settings-panel" data-testid="settings-outputs">
      <div className="settings-bar">
        <span data-testid="output-count">
          {rows.length} {rows.length === 1 ? "output" : "outputs"}
        </span>
        <button type="button" onClick={add} data-testid="output-add">
          Add output
        </button>
      </div>
      {dark.length === 0 ? null : (
        <p className="settings-warning" data-testid="dark-universes" role="status">
          {dark.length === 1 ? "Universe" : "Universes"} {dark.join(", ")}{" "}
          {dark.length === 1 ? "is" : "are"} patched and no output carries{" "}
          {dark.length === 1 ? "it" : "them"}. Nothing on{" "}
          {dark.length === 1 ? "that universe" : "those universes"} will light.
        </p>
      )}
      <OutputTable
        rows={rows}
        editing={draft?.wasId ?? null}
        onEdit={(row) => {
          setDraft(row.output === null ? null : draftOf(row.output));
        }}
        onEnabled={(id, enabled) => {
          send({ t: "SetOutputEnabled", id, enabled });
        }}
        onRemove={(id) => {
          send({ t: "RemoveOutput", id });
          setDraft(null);
        }}
      />
      {draft === null ? null : (
        <OutputForm
          draft={draft}
          onChange={setDraft}
          onApply={apply}
          onCancel={() => {
            setDraft(null);
          }}
        />
      )}
    </div>
  );
}

/** The rig, one row per configured output. */
function OutputTable({
  rows,
  editing,
  onEdit,
  onEnabled,
  onRemove,
}: {
  readonly rows: readonly OutputSnapshot[];
  readonly editing: number | null;
  readonly onEdit: (row: OutputSnapshot) => void;
  readonly onEnabled: (id: number, enabled: boolean) => void;
  readonly onRemove: (id: number) => void;
}) {
  if (rows.length === 0) {
    return (
      <p className="window-note" data-testid="no-outputs">
        This desk has no outputs configured. Nothing it plays reaches a lamp until one is added —
        which is a legitimate state for a desk that is being prepared, and not one for a desk in a
        show.
      </p>
    );
  }
  return (
    <div className="sheet-scroll">
      <table className="sheet">
        <thead>
          <tr>
            <th scope="col">Out</th>
            <th scope="col">Name</th>
            <th scope="col">Kind</th>
            <th scope="col">Universes</th>
            <th scope="col">Health</th>
            <th scope="col">Frames</th>
            <th scope="col">Last error</th>
            <th scope="col">On</th>
            <th scope="col" />
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={row.id}
              className={editing === row.id ? "row-editing" : ""}
              data-testid={`output-row-${String(row.id)}`}
            >
              <td>
                <button
                  type="button"
                  className="linkish"
                  aria-label={`Edit output ${String(row.id)}`}
                  onClick={() => {
                    onEdit(row);
                  }}
                >
                  {row.id}
                </button>
              </td>
              <td>{row.name}</td>
              <td>{row.output === null ? "—" : row.output.kind.t}</td>
              <td data-testid={`output-universes-${String(row.id)}`}>
                {row.output === null ? "—" : writeUniverses(row.output.universes)}
              </td>
              <td
                className={`output-health output-${row.health.toLowerCase()}`}
                data-testid={`output-health-${String(row.id)}`}
              >
                {row.health}
              </td>
              <td data-testid={`output-frames-${String(row.id)}`}>{row.framesSent}</td>
              {/* An **age**, not a time: the daemon and a browser have no shared
                  clock, so S33 put how-long-ago on the wire and this prints it. */}
              <td data-testid={`output-error-${String(row.id)}`}>
                {row.lastError === null
                  ? "—"
                  : `${row.lastError}${
                      row.lastErrorAgoMs === null
                        ? ""
                        : ` (${String(Math.round(row.lastErrorAgoMs / 1000))} s ago)`
                    }`}
              </td>
              <td>
                <input
                  type="checkbox"
                  checked={row.output?.enabled ?? false}
                  aria-label={`Output ${String(row.id)} enabled`}
                  data-testid={`output-enabled-${String(row.id)}`}
                  onChange={(event) => {
                    onEnabled(row.id, event.target.checked);
                  }}
                />
              </td>
              <td>
                <button
                  type="button"
                  data-testid={`output-remove-${String(row.id)}`}
                  onClick={() => {
                    onRemove(row.id);
                  }}
                >
                  Remove
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** The row being typed into. */
function OutputForm({
  draft,
  onChange,
  onApply,
  onCancel,
}: {
  readonly draft: Draft;
  readonly onChange: (draft: Draft) => void;
  readonly onApply: () => void;
  readonly onCancel: () => void;
}) {
  const universes = readUniverses(draft.universes);
  return (
    <form
      className="settings-form"
      data-testid="output-form"
      onSubmit={(event) => {
        event.preventDefault();
        onApply();
      }}
    >
      <fieldset>
        <legend>{draft.wasId === null ? "New output" : `Output ${String(draft.wasId)}`}</legend>
        <label>
          Number
          <input
            data-testid="output-draft-id"
            inputMode="numeric"
            value={String(draft.id)}
            onChange={(event) => {
              const typed = Number(event.target.value.trim());
              if (Number.isInteger(typed) && typed > 0) {
                onChange({ ...draft, id: typed });
              }
            }}
          />
        </label>
        <label>
          Name
          <input
            data-testid="output-draft-name"
            value={draft.name}
            onChange={(event) => {
              onChange({ ...draft, name: event.target.value });
            }}
          />
        </label>
        <label>
          Kind
          <select
            data-testid="output-draft-kind"
            value={draft.kind}
            onChange={(event) => {
              const kind = KINDS.find((candidate) => candidate === event.target.value);
              if (kind !== undefined) {
                onChange({ ...draft, kind });
              }
            }}
          >
            {KINDS.map((kind) => (
              <option key={kind} value={kind}>
                {kindLabel(kind)}
              </option>
            ))}
          </select>
        </label>
        {draft.kind === "OpenDmx" ? (
          <label>
            Serial
            <input
              data-testid="output-draft-serial"
              value={draft.serial}
              placeholder="the adapter's serial, when there are two"
              onChange={(event) => {
                onChange({ ...draft, serial: event.target.value });
              }}
            />
          </label>
        ) : null}
        {draft.kind === "ArtNet" || draft.kind === "Sacn" ? (
          <label>
            {draft.kind === "ArtNet" ? "Nodes" : "Receivers"}
            <textarea
              data-testid="output-draft-addresses"
              rows={2}
              value={draft.addresses}
              placeholder="192.168.1.50:6454, one per line"
              onChange={(event) => {
                onChange({ ...draft, addresses: event.target.value });
              }}
            />
          </label>
        ) : null}
        {draft.kind === "Sacn" ? (
          <label>
            Hop limit
            <input
              data-testid="output-draft-ttl"
              inputMode="numeric"
              value={String(draft.ttl)}
              onChange={(event) => {
                const typed = Number(event.target.value.trim());
                if (Number.isInteger(typed) && typed >= 0) {
                  onChange({ ...draft, ttl: typed });
                }
              }}
            />
          </label>
        ) : null}
        <label>
          Universes
          <input
            data-testid="output-draft-universes"
            value={draft.universes}
            placeholder="1, 2, 3"
            onChange={(event) => {
              onChange({ ...draft, universes: event.target.value });
            }}
          />
        </label>
      </fieldset>
      <p className="settings-hint" role="status" data-testid="output-draft-note">
        {universes === null
          ? "The universes have to be whole numbers, 1 upwards."
          : draft.kind === "OpenDmx" && universes.length > 1
            ? "An Open DMX adapter is one DMX line and carries exactly one universe."
            : "Changing the kind, the universes or the number restarts this output's driver. A rename costs it nothing."}
      </p>
      <div className="settings-actions">
        <button type="submit" data-testid="output-draft-apply" disabled={universes === null}>
          {draft.wasId === null ? "Add" : "Apply"}
        </button>
        <button type="button" onClick={onCancel} data-testid="output-draft-cancel">
          Cancel
        </button>
      </div>
    </form>
  );
}
