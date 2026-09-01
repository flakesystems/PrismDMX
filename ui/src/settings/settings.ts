/**
 * Reading the settings, and writing the words that describe them — S37.
 *
 * The fifth file of the kind `canvas/windows.ts` started, and it follows the
 * same rule: **nothing here holds anything.** Every function takes what the
 * daemon said and answers; there is no parsed copy of the machine, because a
 * parsed copy is a second model to keep in step.
 *
 * # What is deliberately not computed here
 *
 * Two things, and both are the trap S27 wrote down:
 *
 * - **Which universes go nowhere.** That is the patch intersected with the rig,
 *   and `prism_core::dark_universes` already decides it — so the panel *asks*
 *   (`Query::DarkUniverses`) exactly as the patch sheet asks about overlaps.
 * - **Whether a setting takes effect now.** `MachineChange::needs_restart` is
 *   the domain's answer and {@link needsRestart} is a lookup into a table
 *   generated from the same enum. A list of restart-needing rows written out in
 *   TypeScript would be a second answer, and the one that is wrong is the one an
 *   operator believes.
 */

import type {
  ExitAction,
  LogLevel,
  MachineChange,
  MachineOverride,
  MachineSettings,
  SurfaceHealth,
  SurfaceStatus,
} from "../bindings";
import type { AutostartReport } from "../shell/bridge";

/**
 * The panels, in the order the window draws them.
 *
 * Four from S37 and **Controls** from S38, and it sits next to *Devices* on
 * purpose: that panel names the desk's MIDI port and the file its table was read
 * from, and this one is what that table says. An operator who has just chosen a
 * port is one click from what its keys do.
 */
export const PANELS = [
  "Outputs",
  "Devices",
  "Controls",
  "Show files",
  "This machine",
] as const;

/** Which panel is being looked at. */
export type Panel = (typeof PANELS)[number];

/**
 * Which settings a running daemon cannot change until it is restarted.
 *
 * The **same list** `MachineChange::needs_restart` holds, keyed by the change's
 * tag rather than repeated as prose: a listener is bound once, a frame layout is
 * built once, and a new desk identity waits deliberately — an sACN source that
 * changed its CID mid-show would fight the source it used to be for the two and
 * a half seconds a receiver's network-data-loss timeout runs.
 *
 * It is a table rather than a query because it is a property of the *kind* of
 * change and never of the value, so there is nothing to ask about.
 */
const RESTART_NEEDED: ReadonlySet<MachineChange["t"]> = new Set([
  "Local",
  "Websocket",
  "Token",
  "NewToken",
  "Universes",
  "FixtureLibrary",
  "NewIdentity",
]);

/** Whether this change waits for the next start. */
export function needsRestart(change: MachineChange): boolean {
  return RESTART_NEEDED.has(change.t);
}

/**
 * The flag a command line is holding a setting with, or `null`.
 *
 * The mapping is `MachineOverride::flag`'s, and it is here rather than sent
 * because it is a constant of the *daemon's* command line: a client that was
 * told the flag would be told the same ten strings for ever.
 */
export function flagOf(held: MachineOverride): string {
  switch (held) {
    case "Outputs":
      return "--mock-output";
    case "Surface":
      return "--surface";
    case "Local":
      return "--no-local";
    case "Websocket":
      return "--websocket";
    case "Token":
      return "--token";
    case "LogLevel":
      return "--log-level";
    case "Universes":
      return "--universes";
    case "ExitAction":
      return "--blackout-on-exit";
    case "FixtureLibrary":
      return "--fixtures";
    case "SurfaceProfile":
      return "--surface-profile";
  }
}

/** Whether this run's command line is holding a setting. */
export function isHeld(machine: MachineSettings | null, held: MachineOverride): boolean {
  return machine !== null && machine.overrides.includes(held);
}

/** What to write beside a row a flag is holding. */
export function heldNote(machine: MachineSettings | null, held: MachineOverride): string | null {
  return isHeld(machine, held) ? `${flagOf(held)} is holding this for this run` : null;
}

/**
 * What the WebSocket listener is doing, in one sentence.
 *
 * Three states and all three are ordinary: off, up where it was asked for, and
 * **asked for somewhere it could not bind** — which since S37 is a warning and a
 * daemon that starts, because two daemons on one machine both want 7373 and
 * refusing to start over it would make one stray process able to stop a show.
 */
export function listenerText(machine: MachineSettings): string {
  if (machine.websocket === null) {
    return "not listening — browsers and the Web Remote cannot reach this desk";
  }
  if (machine.websocketOpen === null) {
    return `configured for ${machine.websocket}, listening nowhere — something else has that address`;
  }
  if (machine.websocketOpen !== machine.websocket) {
    return `listening on ${machine.websocketOpen}, which is not the configured ${machine.websocket}`;
  }
  return `listening on ${machine.websocketOpen}`;
}

/**
 * Whether an address reaches other machines, which is what §2.1's rule is about.
 *
 * Deliberately **not** the daemon's decision repeated: the daemon refuses the
 * command, and this only decides whether to warn the operator before they press
 * it. A disagreement costs a sentence, not a wrong outcome.
 */
export function isLoopback(address: string): boolean {
  const host = address.startsWith("[")
    ? address.slice(1, address.indexOf("]"))
    : address.split(":")[0];
  return host === "127.0.0.1" || host === "localhost" || host === "::1" || (host ?? "").startsWith("127.");
}

/** The five log levels, in the order the daemon declares them. */
export const LOG_LEVELS: readonly LogLevel[] = ["Debug", "Info", "Warn", "Error", "Off"];

/** The two exit actions. */
export const EXIT_ACTIONS: readonly ExitAction[] = ["Hold", "Blackout"];

/** What an exit action does, said in an operator's words rather than in a name. */
export function exitText(action: ExitAction): string {
  return action === "Hold"
    ? "leave the last look on stage"
    : "black the stage out before stopping";
}

/**
 * What a surface health means, for a panel that has to say more than a word.
 *
 * The remedy comes from the daemon (`SurfaceStatus::remedy`) rather than from
 * here, because the wording of one of them is the whole point of the state: the
 * obvious advice for an unresponsive desk is *reconnect*, and S20 established
 * that reconnecting is the one thing that cannot recover it.
 */
export function healthText(health: SurfaceHealth): string {
  switch (health) {
    case "Disconnected":
      return "not there";
    case "Connected":
      return "there, and quiet";
    case "Live":
      return "sending";
    case "Probing":
      return "quiet — being asked";
    case "Unresponsive":
      return "stopped sending";
  }
}

/** One counter row, as the Devices panel lists them. */
export interface CounterRow {
  /** What it counts, in words. */
  readonly label: string;
  /** How many. */
  readonly value: number;
}

/**
 * The counters, in the order a person reads them: what went out, what was saved
 * by not sending it, and what happened to the cable.
 */
export function counterRows(status: SurfaceStatus): readonly CounterRow[] {
  return [
    { label: "Sent", value: status.sent },
    { label: "Superseded", value: status.superseded },
    { label: "Held for a hand", value: status.touchSuppressed },
    { label: "Resyncs", value: status.resyncs },
    { label: "Reserved", value: status.reserved },
    { label: "Probes", value: status.probes },
    { label: "Reconnects", value: status.reconnects },
  ];
}

/**
 * The file name out of a path, for a menu of recent shows.
 *
 * The whole path is still shown — as a title, so it is there for an operator who
 * has two `aula.prism` files — because two shows with one name is exactly the
 * case a school produces.
 */
export function fileName(path: string): string {
  const parts = path.split(/[\\/]/u);
  return parts[parts.length - 1] ?? path;
}

/**
 * A comma- or space-separated list of universe numbers, read back as numbers.
 *
 * Answers `null` for anything that is not a list of whole numbers, so a form can
 * refuse to send rather than send `[NaN]` — which the daemon would refuse anyway,
 * one round trip later and less clearly.
 */
export function readUniverses(text: string): number[] | null {
  const parts = text
    .split(/[\s,]+/u)
    .map((part) => part.trim())
    .filter((part) => part !== "");
  const numbers = parts.map(Number);
  if (numbers.some((value) => !Number.isInteger(value) || value < 1)) {
    return null;
  }
  return numbers;
}

/** The same list, written out the way it is typed in. */
export function writeUniverses(universes: readonly number[]): string {
  return universes.join(", ");
}

/**
 * What the row says about the entry itself.
 *
 * Five answers, and the two that matter are the ones where the setting and the
 * machine disagree. Here rather than in the panel, so a test
 * can hold every one of them without rendering anything — which is where the
 * other readings in this file live for the same reason.
 */
export function autostartEntryText(wanted: boolean, entry: AutostartReport | null): string {
  if (entry === null) {
    return "This interface is running in a browser, so it cannot see this machine's start-up entry. The desktop shell writes it.";
  }
  if (!entry.supported) {
    return "This build cannot write a start-up entry on this platform, so the setting is stored and nothing acts on it.";
  }
  // **The setting is read first, and that ordering is the whole of the row's
  // manners.** An entry belonging to another copy of the program is
  // interesting only where the operator asked for one; where they asked for
  // none, *there is still an entry* is what they need to be told, and whose it
  // is does not change what to do about it.
  if (!wanted) {
    return entry.installed
      ? "The setting is off and a start-up entry is still there. Untick the box again to remove it."
      : "There is no start-up entry, which is what the setting says.";
  }
  if (!entry.installed) {
    return "The setting is on and there is no start-up entry — it was removed outside this program. Tick the box again to put it back.";
  }
  if (!entry.matchesThisInstall) {
    return `A start-up entry exists but it starts another copy of the program: ${entry.command ?? "somewhere else"}. Tick the box again to point it at this one.`;
  }
  return "A start-up entry for this installation is in place.";
}
