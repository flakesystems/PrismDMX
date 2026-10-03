/**
 * The *Trackers* panel: PSN, from the network to the stage — **S32**.
 *
 * # What a tracker is to this desk
 *
 * A tracking system (openfollow.app, Zactrack, BlackTrax through a bridge…)
 * streams where each performer is, in *its own* units and *its own* axes. This
 * desk listens (`ARCHITECTURE_SPEC.md` §8), puts every position on **this**
 * stage, and a moving head that has been given a tracker in the 3D viewer's
 * *Follows* box points at it in proportion to its **Follow** value — which a
 * cue stores like any other, so a cue can say *from here, this head is on her*.
 *
 * Three questions, one section each, and none of the answers is computed here:
 *
 * - **Is the desk listening, and where?** The receiver is a machine setting
 *   (`TrackerSettings`), one command per field, and takes effect on the spot.
 * - **How do the system's axes line up with the stage?** The format does not
 *   say which way is up or what a unit is, so the installer does — and the list
 *   at the bottom is how they find out within a minute: it shows each tracker's
 *   position **after** the mapping, so a performer walking upstage that reads as
 *   walking up is a mapping to fix.
 * - **What is out there?** `Query::Trackers`, asked while the panel is open and
 *   at no other time: a tracker is an observation about the network, neither
 *   show nor machine, and no command causes one.
 *
 * # A tracker that is quiet is held, not lost
 *
 * The desk keeps the last position of a tracker that has stopped, and the heads
 * following it stay aimed there — nothing jumps (§8). The row says *Quiet* and
 * how long, and the operator is told once in the notice line.
 *
 * # Why the list says whether the receiver is open before it says anything else
 *
 * An empty list under a receiver that never opened says nothing about the
 * network, and reading it as *no trackers* is `ArtNetNodes`' mistake (B6)
 * pointed at a different protocol. `listening` is drawn first.
 */

import { useCallback, useEffect, useState } from "react";

import type {
  MachineChange,
  MachineSettings,
  SeenTracker,
  ShowAxis,
  TrackerChange,
} from "../bindings";
import { SHOW_AXIS_VARIANTS, SOURCE_AXIS_VARIANTS } from "../bindings";
import type { DeskState } from "../store/desk";
import { useAsk, useDesk, useSend } from "../store/hooks";
import { ageText, readNumber, showAxisText, trackerStatusText, trackersOf } from "./settings";

const selectMachine = (state: DeskState): MachineSettings | null => state.machine;

/**
 * How often the list is re-read while the panel is open.
 *
 * Twice a second: a tracker sends thirty to sixty times a second and a person
 * reads a position a few times a second at most. Nothing is asked when the panel
 * is closed.
 */
const LIST_INTERVAL_MS = 500;

/** What the last `Query::Trackers` said. */
interface Hearing {
  readonly trackers: readonly SeenTracker[];
  readonly listening: boolean;
  readonly error: string | null;
  readonly rejected: number;
  readonly datagrams: number;
  readonly from: string | null;
  readonly interfaces: readonly string[];
  readonly remedy: string | null;
}

/** The whole panel. */
export function TrackersPanel() {
  const machine = useDesk(selectMachine);
  const send = useSend();
  const change = useCallback(
    (what: TrackerChange) => {
      const wrapped: MachineChange = { t: "Tracker", change: what };
      send({ t: "ConfigureMachine", change: wrapped });
    },
    [send],
  );

  if (machine === null) {
    return <p className="window-note">The daemon has not said what this machine is set to.</p>;
  }
  return (
    <div className="settings-panel" data-testid="settings-trackers">
      <Receiver machine={machine} onChange={change} />
      <Axes machine={machine} onChange={change} />
      <Heard machine={machine} />
    </div>
  );
}

/** Whether the desk listens, where, and how long a tracker may be quiet. */
function Receiver({
  machine,
  onChange,
}: {
  readonly machine: MachineSettings;
  readonly onChange: (change: TrackerChange) => void;
}) {
  const settings = trackersOf(machine);
  const [group, setGroup] = useState<string | null>(null);
  const [port, setPort] = useState<string | null>(null);
  const [iface, setIface] = useState<string | null>(null);
  const [timeout, setTimeoutText] = useState<string | null>(null);

  const typedGroup = group ?? settings.group;
  const typedPort = port ?? String(settings.port);
  const typedInterface = iface ?? settings.interface ?? "";
  const typedTimeout = timeout ?? String(settings.timeoutMs);

  return (
    <section className="settings-group" data-testid="trackers-receiver">
      <h3>Receiver</h3>
      <label>
        <input
          type="checkbox"
          data-testid="trackers-enabled"
          checked={settings.enabled}
          onChange={(event) => {
            onChange({ t: "Enabled", enabled: event.target.checked });
          }}
        />
        Listen for trackers (PSN)
      </label>
      <div className="settings-row">
        <label>
          Group
          <input
            data-testid="trackers-group"
            value={typedGroup}
            aria-label="Multicast group"
            onChange={(event) => {
              setGroup(event.target.value);
            }}
            onBlur={() => {
              if (group !== null && group.trim() !== settings.group) {
                onChange({ t: "Group", group: group.trim() });
              }
              setGroup(null);
            }}
          />
        </label>
        <label>
          Port
          <input
            data-testid="trackers-port"
            inputMode="numeric"
            value={typedPort}
            aria-label="UDP port"
            onChange={(event) => {
              setPort(event.target.value);
            }}
            onBlur={() => {
              const value = port === null ? null : readNumber(port);
              if (value !== null && Number.isInteger(value) && value !== settings.port) {
                onChange({ t: "Port", port: value });
              }
              setPort(null);
            }}
          />
        </label>
        <label>
          Interface
          <input
            data-testid="trackers-interface"
            value={typedInterface}
            placeholder="any"
            aria-label="Network interface address"
            onChange={(event) => {
              setIface(event.target.value);
            }}
            onBlur={() => {
              if (iface !== null && iface.trim() !== (settings.interface ?? "")) {
                onChange({ t: "Interface", address: iface.trim() === "" ? null : iface.trim() });
              }
              setIface(null);
            }}
          />
        </label>
        <label>
          Quiet after, ms
          <input
            data-testid="trackers-timeout"
            inputMode="numeric"
            value={typedTimeout}
            aria-label="Milliseconds before a tracker reads as quiet"
            onChange={(event) => {
              setTimeoutText(event.target.value);
            }}
            onBlur={() => {
              const value = timeout === null ? null : readNumber(timeout);
              if (value !== null && Number.isInteger(value) && value !== settings.timeoutMs) {
                onChange({ t: "Timeout", milliseconds: value });
              }
              setTimeoutText(null);
            }}
          />
        </label>
      </div>
      <p className="settings-hint">
        The published group is <code>236.10.10.10</code>, port <code>56565</code>. An address that is
        not a multicast group is listened on directly, which is how a tracker that sends to this
        machine alone is received. On a laptop with Wi-Fi and a lighting network, name the
        interface&rsquo;s address: the operating system&rsquo;s choice is often the wrong one. The
        firewall has to allow this program in on UDP.
      </p>
    </section>
  );
}

/** How the tracking system's axes line up with this stage. */
function Axes({
  machine,
  onChange,
}: {
  readonly machine: MachineSettings;
  readonly onChange: (change: TrackerChange) => void;
}) {
  const mapping = trackersOf(machine).mapping;
  const [scale, setScale] = useState<string | null>(null);
  const [offset, setOffset] = useState<{ x: string; y: string; z: string } | null>(null);
  const shownOffset = offset ?? {
    x: String(mapping.offset.x),
    y: String(mapping.offset.y),
    z: String(mapping.offset.z),
  };

  const sendOffset = (next: { x: string; y: string; z: string }) => {
    const [x, y, z] = [readNumber(next.x), readNumber(next.y), readNumber(next.z)];
    if (
      x !== null &&
      y !== null &&
      z !== null &&
      (x !== mapping.offset.x || y !== mapping.offset.y || z !== mapping.offset.z)
    ) {
      onChange({ t: "Offset", offset: { x, y, z } });
    }
    setOffset(null);
  };

  return (
    <section className="settings-group" data-testid="trackers-axes">
      <h3>Axes</h3>
      <div className="sheet-scroll">
        <table className="sheet">
          <thead>
            <tr>
              <th scope="col">On this stage</th>
              <th scope="col">Read from the tracker&rsquo;s</th>
              <th scope="col">Reversed</th>
            </tr>
          </thead>
          <tbody>
            {SHOW_AXIS_VARIANTS.map((axis: ShowAxis) => {
              const source = mapping[axis === "X" ? "x" : axis === "Y" ? "y" : "z"];
              return (
                <tr key={axis} data-testid={`trackers-axis-${axis.toLowerCase()}`}>
                  <td>{showAxisText(axis)}</td>
                  <td>
                    <select
                      aria-label={`${showAxisText(axis)} is read from`}
                      data-testid={`trackers-axis-${axis.toLowerCase()}-from`}
                      value={source.from}
                      onChange={(event) => {
                        // Checked rather than claimed: a value that is not one of
                        // the three is a page that has been tampered with.
                        const from = SOURCE_AXIS_VARIANTS.find(
                          (candidate) => candidate === event.target.value,
                        );
                        if (from !== undefined) {
                          onChange({ t: "Axis", axis, from, invert: source.invert });
                        }
                      }}
                    >
                      {SOURCE_AXIS_VARIANTS.map((from) => (
                        <option key={from} value={from}>
                          {from}
                        </option>
                      ))}
                    </select>
                  </td>
                  <td>
                    <input
                      type="checkbox"
                      aria-label={`${showAxisText(axis)} is reversed`}
                      data-testid={`trackers-axis-${axis.toLowerCase()}-invert`}
                      checked={source.invert}
                      onChange={(event) => {
                        onChange({ t: "Axis", axis, from: source.from, invert: event.target.checked });
                      }}
                    />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="settings-row">
        <label>
          Metres per unit
          <input
            data-testid="trackers-scale"
            inputMode="decimal"
            value={scale ?? String(mapping.scale)}
            aria-label="Metres per unit of the tracking system"
            onChange={(event) => {
              setScale(event.target.value);
            }}
            onBlur={() => {
              const value = scale === null ? null : readNumber(scale);
              if (value !== null && value !== mapping.scale) {
                onChange({ t: "Scale", scale: value });
              }
              setScale(null);
            }}
          />
        </label>
        {(["x", "y", "z"] as const).map((key) => (
          <label key={key}>
            Origin {key.toUpperCase()}, m
            <input
              data-testid={`trackers-offset-${key}`}
              inputMode="decimal"
              value={shownOffset[key]}
              aria-label={`Where the tracking system's origin is, ${key}, in metres`}
              onChange={(event) => {
                setOffset({ ...shownOffset, [key]: event.target.value });
              }}
              onBlur={() => {
                sendOffset(shownOffset);
              }}
            />
          </label>
        ))}
      </div>
      <p className="settings-hint">
        PSN does not say which way is up or what a unit is, so this does. The default reads a
        right-handed tracking space with <em>z</em> up and <em>y</em> upstage, in metres. Walk a
        performer to the front-left corner and read the list below: the position after the axes have
        been lined up is what each head is aimed at.
      </p>
    </section>
  );
}

/** What is out there, as the daemon hears it. */
function Heard({ machine }: { readonly machine: MachineSettings }) {
  const ask = useAsk();
  const [heard, setHeard] = useState<Hearing | null>(null);

  useEffect(() => {
    let current = true;
    const read = () => {
      void ask({ t: "Trackers" }).then((answer) => {
        if (current && answer !== null && answer.t === "Trackers") {
          setHeard({
            trackers: answer.trackers,
            listening: answer.listening,
            error: answer.error,
            rejected: answer.rejected,
            datagrams: answer.datagrams,
            from: answer.from,
            interfaces: answer.interfaces,
            remedy: answer.remedy,
          });
        }
      });
    };
    read();
    const timer = setInterval(read, LIST_INTERVAL_MS);
    return () => {
      current = false;
      clearInterval(timer);
    };
  }, [ask]);

  return (
    <section className="settings-group" data-testid="trackers-heard">
      <h3>Heard</h3>
      <p
        className={heard !== null && heard.error !== null ? "settings-warning" : "settings-reading"}
        data-testid="trackers-status"
      >
        {trackerStatusText(trackersOf(machine).enabled, heard)}
      </p>
      {heard === null || heard.trackers.length === 0 ? null : (
        <div className="sheet-scroll">
          <table className="sheet">
            <thead>
              <tr>
                <th scope="col">Tracker</th>
                <th scope="col">Name</th>
                <th scope="col">X</th>
                <th scope="col">Y</th>
                <th scope="col">Z</th>
                <th scope="col">Last heard</th>
                <th scope="col">Heads</th>
              </tr>
            </thead>
            <tbody>
              {heard.trackers.map((tracker) => (
                <tr key={tracker.id} data-testid={`tracker-row-${String(tracker.id)}`}>
                  <td>{tracker.id}</td>
                  <td>{tracker.name ?? "—"}</td>
                  <td>{tracker.position.x.toFixed(2)}</td>
                  <td>{tracker.position.y.toFixed(2)}</td>
                  <td>{tracker.position.z.toFixed(2)}</td>
                  <td
                    className={tracker.health === "Live" ? "output-ok" : "output-degraded"}
                    data-testid={`tracker-health-${String(tracker.id)}`}
                  >
                    {ageText(tracker)}
                  </td>
                  <td data-testid={`tracker-heads-${String(tracker.id)}`}>{tracker.followers}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className="settings-hint">
        Positions are in metres of this stage — <em>X</em> across it, <em>Y</em> up, <em>Z</em>{" "}
        upstage — after the axes above. A tracker nobody has moved is named and has never been
        heard. A head follows a tracker once it has been given one in the 3D viewer
        (<em>Follows</em>) and its <em>Follow</em> value is above nought: the Position bank&rsquo;s
        fourth encoder, or <code>Fixture 1 Follow At 100</code>.
      </p>
    </section>
  );
}
