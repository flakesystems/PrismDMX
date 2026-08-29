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
 * # Health for a network output is not the socket's opinion — S46
 *
 * An Art-Net driver reports `Ok` as soon as its socket has accepted the
 * datagram, and UDP accepts every datagram there is. This panel drew that word,
 * which is punch-list **B6**: an installer read *OK* over an empty rack. The
 * daemon now folds whether anything answers into the health it reports, so the
 * cell says `Degraded`; what this file adds is the **sentence underneath it**,
 * because *Degraded* alone does not tell somebody which node to go and look at.
 *
 * Nothing here decides any of it. `nodes` on the status row is one `NodeReach`
 * per configured node, empty for every kind that cannot be asked and empty while
 * nothing is listening — and *not listening* is drawn before the node list is,
 * because an empty list under a socket that never opened says nothing about the
 * network and reading it as *no nodes* would be B6 pointed the other way.
 *
 * # Discovered is not configured
 *
 * The second table is `Query::ArtNetNodes`: what is out there, as against what
 * this desk is addressed to. The two **disagreements** — a node outputting a
 * universe this desk sends nothing on, and a universe this desk sends that the
 * node does not have — are the daemon's arithmetic and arrive as fields. A panel
 * that intersected the rig with the discovery table itself would be a second
 * opinion about something the daemon holds both halves of, which is D3.
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

import type {
  ArtNetCounters,
  ArtNetNodeInfo,
  NodeReach,
  OutputInstance,
  OutputKind,
  OutputStatusInfo,
} from "../bindings";
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

/**
 * A wall-clock `HH:MM` from an age in milliseconds — S46.
 *
 * The daemon says *how long ago* because it and a browser share no clock (S33);
 * turning that into a time of day is the client's half, and it is the half an
 * operator actually reads: *stopped answering at 20:14* is a fact somebody can
 * line up against what else happened in the hall, and *stopped answering 512
 * seconds ago* is arithmetic they have to do standing on a ladder.
 *
 * Built from the parts rather than from `toLocaleTimeString`, so the string is
 * the same in every locale the desk might be running under.
 */
function clockOf(agoMs: number, now: number): string {
  const at = new Date(now - agoMs);
  return `${String(at.getHours()).padStart(2, "0")}:${String(at.getMinutes()).padStart(2, "0")}`;
}

/**
 * What to say under an Art-Net row's health, or `null` when there is nothing to
 * add — S46.
 *
 * `null` for a kind that cannot be asked, for a desk that is not listening, and
 * for a rig where every node answers. Those three are drawn the same way on
 * purpose: the line exists to name a fault, and a line that appeared when all
 * was well would be one more thing to read past.
 */
function nodeNote(nodes: readonly NodeReach[], now: number): string | null {
  if (nodes.length === 0) {
    return null;
  }
  const silent = nodes.filter((node) => node.health !== "Answering");
  if (silent.length === 0) {
    return null;
  }
  const stopped = silent.find((node) => node.health === "Stopped");
  if (stopped !== undefined) {
    const at =
      stopped.lastReplyAgoMs === null ? "" : ` at ${clockOf(stopped.lastReplyAgoMs, now)}`;
    return `${stopped.address} stopped answering${at}`;
  }
  return nodes.length === 1
    ? `${silent[0]?.address ?? "the node"} has never answered`
    : `${String(silent.length)} of ${String(nodes.length)} nodes have never answered`;
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
  // What is out there, as against what this desk is addressed to — S46.
  // `listening` starts false and is read before `discovered` ever is: not
  // listening and nothing answering are different facts.
  const [discovered, setDiscovered] = useState<readonly ArtNetNodeInfo[]>([]);
  const [listening, setListening] = useState(false);
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);
  const [counters, setCounters] = useState<ArtNetCounters | null>(null);
  // **One moment for the whole panel.** Every age on the screen is measured
  // against this, so two rows drawn from one answer cannot disagree about when
  // *now* was — which is the same reason the daemon measures every age in one
  // answer against one `Instant::now()`.
  const [now, setNow] = useState(() => Date.now());

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

  // …and what each driver is *doing*, which no delta carries — together with
  // what the network answers, which no delta carries either and for the same
  // reason. **One timer for both**: they are drawn in the same panel at the same
  // moment, and two timers would only make the two halves of one screen
  // disagree about when *now* was.
  useEffect(() => {
    let current = true;
    const read = () => {
      void ask({ t: "OutputStatus" }).then((answer) => {
        if (current && answer !== null && answer.t === "OutputStatus") {
          setStatus(answer.outputs);
        }
      });
      setNow(Date.now());
      void ask({ t: "ArtNetNodes" }).then((answer) => {
        if (current && answer !== null && answer.t === "ArtNetNodes") {
          setDiscovered(answer.nodes);
          setListening(answer.listening);
          setDiscoveryError(answer.error);
          setCounters(answer.counters);
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
          ? { ...row, nodes: [] as readonly NodeReach[] }
          : {
              ...row,
              health: reading.health,
              framesSent: reading.framesSent,
              lastError: reading.lastError,
              lastErrorAgoMs: reading.lastErrorAgoMs,
              nodes: reading.nodes,
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

  /**
   * The deliverable's *one click*: a draft addressed to a node that is out
   * there, with the universes it would carry already filled in.
   *
   * **A draft and not a command.** The row is put in the form rather than added
   * to the rig, because what an operator wants next is almost always to name it
   * and check the universes, and a panel that silently added an output to a
   * running desk would be making a change on a stage on somebody's behalf. The
   * typing it saves is the address and the universes, which is the typing that
   * is easy to get wrong.
   *
   * `suggestedUniverses` is the **daemon's**: it is the default port-address
   * mapping run backwards, and spelling that here would be a third copy of a
   * rule `ARCHITECTURE_SPEC.md` §7.0 already states and `prism-domain` already
   * implements.
   */
  const addDiscovered = useCallback(
    (node: ArtNetNodeInfo) => {
      setDraft({
        id: nextId,
        wasId: null,
        name: node.shortName.trim() === "" ? `Output ${String(nextId)}` : node.shortName,
        kind: "ArtNet",
        serial: "",
        addresses: node.address,
        ttl: 1,
        universes: writeUniverses(node.suggestedUniverses),
      });
    },
    [nextId],
  );

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
        now={now}
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
      <NodeTable
        nodes={discovered}
        listening={listening}
        error={discoveryError}
        counters={counters}
        now={now}
        onAdd={addDiscovered}
      />
    </div>
  );
}

/**
 * What is on the network — S46.
 *
 * Drawn **after** the rig, because it is the second question an installer has:
 * the first is *what have I configured*, and this is *what is actually out
 * there*. `listening` is read before the list is, for the reason in this file's
 * documentation.
 */
function NodeTable({
  nodes,
  listening,
  error,
  counters,
  now,
  onAdd,
}: {
  readonly nodes: readonly ArtNetNodeInfo[];
  readonly listening: boolean;
  readonly error: string | null;
  readonly counters: ArtNetCounters | null;
  readonly now: number;
  readonly onAdd: (node: ArtNetNodeInfo) => void;
}) {
  return (
    <section className="settings-section" data-testid="artnet-nodes">
      <h3>Art-Net nodes on the network</h3>
      {listening ? null : (
        <p className="window-note" data-testid="artnet-not-listening" role="status">
          {/* The reason is the daemon's when there is one, and there is no
              reason at all when nothing is configured — an absence rather than
              a fault. Guessing one here would be the mistake this panel makes
              about health: saying something the program does not know. */}
          {error === null
            ? "This desk is not listening for Art-Net nodes, so nothing below is a statement about the network. It starts listening once an Art-Net output is configured."
            : `This desk is not listening for Art-Net nodes: ${error}. The outputs are still sending, and this desk cannot tell whether anything receives them.`}
        </p>
      )}
      {/* **What this desk has actually done**, and it is here because the first
          thing S46 got wrong in a hall could not be seen from outside: polls
          going out with nothing coming back, nothing being asked at all, and
          something arriving and being dropped are three different faults that
          looked identical from this panel. The numbers are the daemon's; this
          reads them out and does not interpret them. */}
      {counters === null || !listening ? null : (
        <p className="settings-hint" data-testid="artnet-counters">
          {counters.pollsSent} polls sent · {counters.replies} replies
          {counters.malformed === 0 ? "" : ` · ${String(counters.malformed)} unreadable`}
          {counters.dropped === 0 ? "" : ` · ${String(counters.dropped)} dropped`}
          {counters.pollsFailed === 0 ? "" : ` · ${String(counters.pollsFailed)} could not be sent`}
        </p>
      )}
      {nodes.length === 0 ? (
        listening ? (
          <p className="window-note" data-testid="artnet-no-nodes">
            No node has answered yet. A node answers within a few seconds of being
            switched on; one that never answers is either at another address, on
            another network, or behind a firewall that is dropping the reply.
          </p>
        ) : null
      ) : (
        <div className="sheet-scroll">
          <table className="sheet">
            <thead>
              <tr>
                <th scope="col">Name</th>
                <th scope="col">Address</th>
                <th scope="col">Universes</th>
                <th scope="col">Last reply</th>
                <th scope="col">In the rig</th>
                <th scope="col" />
              </tr>
            </thead>
            <tbody>
              {nodes.map((node, index) => (
                <tr key={node.address} data-testid={`artnet-node-${String(index)}`}>
                  <td title={node.longName}>
                    {node.shortName.trim() === "" ? "(unnamed)" : node.shortName}
                  </td>
                  <td>
                    {node.address}
                    {/* The address a node *claims* differs from the one its
                        reply came from when something between is translating,
                        and an installer chasing a node that will not take
                        frames has to be able to see that. */}
                    {node.ip === node.address.split(":")[0] ? null : (
                      <span className="settings-hint"> (says {node.ip})</span>
                    )}
                  </td>
                  <td data-testid={`artnet-node-ports-${String(index)}`}>
                    {node.ports.length === 0
                      ? "—"
                      : node.ports.map((port) => String(port)).join(", ")}
                  </td>
                  <td data-testid={`artnet-node-seen-${String(index)}`}>
                    {clockOf(node.lastReplyAgoMs, now)}
                  </td>
                  <td data-testid={`artnet-node-state-${String(index)}`}>
                    {node.configured ? "configured" : "not configured"}
                    {node.unaddressedPorts.length === 0 ? null : (
                      <div className="settings-warning">
                        outputs {node.unaddressedPorts.map(String).join(", ")}, which this
                        desk sends nothing on
                      </div>
                    )}
                    {node.missingPorts.length === 0 ? null : (
                      <div className="settings-warning">
                        this desk sends {node.missingPorts.map(String).join(", ")}, which it
                        does not have
                      </div>
                    )}
                  </td>
                  <td>
                    <button
                      type="button"
                      data-testid={`artnet-node-add-${String(index)}`}
                      onClick={() => {
                        onAdd(node);
                      }}
                    >
                      {node.configured ? "Add another output" : "Add as output"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

/** The rig, one row per configured output. */
function OutputTable({
  rows,
  now,
  editing,
  onEdit,
  onEnabled,
  onRemove,
}: {
  readonly rows: readonly (OutputSnapshot & { readonly nodes: readonly NodeReach[] })[];
  readonly now: number;
  readonly editing: number | null;
  readonly onEdit: (row: OutputSnapshot & { readonly nodes: readonly NodeReach[] }) => void;
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
                {/* **Punch-list B6's other half** — S46. The word above is the
                    daemon's, folded from whether anything answers; this names
                    the node, because *Degraded* alone does not tell an
                    installer which box to walk to. Absent when every node
                    answers, when the kind cannot be asked, and when this desk
                    is not listening — all three are *nothing to add*. */}
                {nodeNote(row.nodes, now) === null ? null : (
                  <div
                    className="settings-warning"
                    data-testid={`output-nodes-${String(row.id)}`}
                  >
                    {nodeNote(row.nodes, now)}
                  </div>
                )}
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
