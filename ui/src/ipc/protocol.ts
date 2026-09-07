/**
 * `docs/IPC_PROTOCOL.md` §4 in TypeScript: the envelopes, and the readers that
 * turn a decoded payload into one.
 *
 * # Why this file is written by hand when `ui/src/bindings` is generated
 *
 * The bindings are `prism-domain`'s — `Command`, `Delta`, `Session`, everything
 * the daemon and the interface both call by name. The *envelope* is
 * `prism-ipc`'s (`crates/prism-ipc/src/message.rs`), and that crate exports no
 * TypeScript, because it is the wire and not the vocabulary. So these types are
 * transcribed, and `crates/prism-ipc/tests/interface_protocol.rs` reads this
 * file back to check the transcription: every `RejectReason`, every message tag,
 * every `ClientKind` and the protocol version itself. A transcription nobody
 * checks is a second source of truth, which is exactly what D3 forbids.
 *
 * # Two envelopes, and the reason there are two
 *
 * `ClientMessage` carries `Hello` and `Command`; `ServerMessage` carries the
 * rest. A type that could carry either would let a client send a `Delta`, and
 * **D3** says it cannot. That asymmetry is the whole protocol: a client sends
 * intent and receives facts.
 */

import type {
  Answer,
  ArtNetCounters,
  ArtNetNodeInfo,
  BoundControl,
  Command,
  CommandLineQuestion,
  Delta,
  ExecutorButtonRef,
  ExecutorTarget,
  LibraryEntry,
  MachineOverride,
  MidiPortInfo,
  MachineSettings,
  NodeReach,
  OutputHealth,
  OutputId,
  OutputInstance,
  OutputKind,
  OutputStatusInfo,
  PatchConflict,
  PatchPreview,
  ExecutorButtonFunction,
  ProgrammerState,
  Query,
  ShowFileInfo,
  SurfaceAction,
  SurfaceControl,
  SurfaceStatus,
  StoreMode,
  CueTrackingRow,
  TrackedValue,
  StorePreview,
} from "../bindings";
import {
  ATTRIBUTE_TYPE_VARIANTS,
  COMMAND_LINE_MODE_VARIANTS,
  COMMAND_LINE_READING_KIND_VARIANTS,
  EXECUTOR_TARGET_VARIANTS,
  EXIT_ACTION_VARIANTS,
  FEATURE_GROUP_VARIANTS,
  GLOBAL_BUTTON_VARIANTS,
  GO_DIRECTION_VARIANTS,
  LOG_LEVEL_VARIANTS,
  MACHINE_OVERRIDE_VARIANTS,
  NODE_HEALTH_VARIANTS,
  NOTICE_LEVEL_VARIANTS,
  OUTPUT_HEALTH_VARIANTS,
  PARAM_DIRECTION_VARIANTS,
  PROGRAMMER_VALUE_SOURCE_VARIANTS,
  STEP_VARIANTS,
  STORE_MODE_VARIANTS,
  STRIP_BUTTON_VARIANTS,
  SURFACE_HEALTH_VARIANTS,
  WINDOW_TYPE_VARIANTS,
} from "../bindings";
import type { JsonPatchOp, JsonValue, ProgrammerEntry, ProgrammerValue } from "../bindings";
import type { Payload } from "./shape";
import {
  ProtocolFault,
  asArray,
  asBoolean,
  asBytes,
  asInteger,
  asJsonValue,
  asNullable,
  asNumber,
  asRecord,
  asString,
  asVariant,
  field,
} from "./shape";
import { FIXED_BUTTON_FUNCTIONS } from "../desk/functions";

/**
 * The version this build speaks, incremented on any breaking change.
 *
 * Mirrors `prism_ipc::PROTOCOL_VERSION`. A mismatch is an explicit `Reject`
 * naming **both** numbers (§4.2), because an operator whose interface will not
 * connect has to be told which half to update.
 */
export const PROTOCOL_VERSION = 1;

/** What kind of client is connecting. Informational — there is no privileged client. */
export const CLIENT_KINDS = ["Desktop", "WebRemote", "Other"] as const;

/** One of {@link CLIENT_KINDS}. */
export type ClientKind = (typeof CLIENT_KINDS)[number];

/** The first message on every connection. */
export interface Hello {
  /** The version the client speaks. Must equal the daemon's. */
  readonly protocolVersion: number;
  /** What kind of client this is. */
  readonly clientKind: ClientKind;
  /** The token from §2.1, required when the listener is not on loopback. */
  readonly token: string | null;
}

/**
 * Everything a client may send.
 *
 * Three shapes, since S27: a handshake, an intent, and a **question**. A query
 * changes nothing and is answered to this client alone, which is why it is not
 * a command with a special reply — see `crates/prism-domain/src/query.rs`. It
 * shares the command numbering because both travel on the one ordered channel.
 */
export type ClientMessage =
  | { readonly t: "Hello"; readonly hello: Hello }
  | { readonly t: "Command"; readonly seq: number; readonly command: Command }
  | { readonly t: "Query"; readonly seq: number; readonly query: Query };

/** Why something was refused. */
export const REJECT_REASONS = [
  "ProtocolVersion",
  "Unauthorised",
  "OutOfOrder",
  "CommandRefused",
  "Undecodable",
  "Backpressure",
  "ShuttingDown",
] as const;

/** One of {@link REJECT_REASONS}. */
export type RejectReason = (typeof REJECT_REASONS)[number];

/**
 * Whether a rejection for this reason ends the connection.
 *
 * The two that do not are the two an ordinary client meets: a refused command
 * leaves the connection perfectly usable (§5, the command changed nothing), and
 * a message the daemon could not decode is not a disconnection either — the
 * framing found the boundary, so the next message is read normally (§8, last
 * paragraph). Everything else is a statement about the connection.
 */
export function closesTheConnection(reason: RejectReason): boolean {
  return reason !== "CommandRefused" && reason !== "Undecodable";
}

/**
 * One DMX output, as the status panel shows it.
 *
 * **Grown in S33** from three fields to seven, and the extra four are what a
 * settings window has to draw: the configured row — kind, parameters, universes,
 * whether it is enabled — how much it has sent, and the last thing that went
 * wrong with how long ago that was. Before S33 a client could be told an output
 * was red and nothing about why, or about what had gone dark with it.
 */
export interface OutputSnapshot {
  /** Which output. */
  readonly id: OutputId;
  /** What the operator calls it. */
  readonly name: string;
  /** Whether frames are reaching the fixtures. */
  readonly health: OutputHealth;
  /**
   * The configured row, or `null` from a daemon older than S33.
   *
   * `null` rather than absent, because *this daemon does not send one* and
   * *this output has no configuration* are the same statement here and there is
   * no second reading to keep apart.
   */
  readonly output: OutputInstance | null;
  /**
   * Universes put on the wire since this driver started.
   *
   * One frame is one universe, so a two-universe node counts two per cadence —
   * and it is deliberately not a datagram count, because both network outputs
   * suppress a universe that has not changed.
   */
  readonly framesSent: number;
  /** The last thing that went wrong, in words, or `null` if nothing has. */
  readonly lastError: string | null;
  /**
   * How long ago that was, in milliseconds, measured when the snapshot was
   * taken. An age rather than a time: the daemon and this client have no shared
   * clock.
   */
  readonly lastErrorAgoMs: number | null;
}

/** The daemon's own state at snapshot time. */
export interface DaemonHealth {
  /** The version the daemon speaks, so a client can show it beside its own. */
  readonly protocolVersion: number;
  /** Measured tick rate. 44 Hz when all is well. */
  readonly tickHz: number;
  /** Ticks that missed their deadline since start-up. */
  readonly missedTicks: number;
  /** Whether the show has unsaved changes. */
  readonly unsavedChanges: boolean;
}

/**
 * The world, as of the moment this client connected.
 *
 * Three documents. The show and the session travel as documents rather than as
 * models because `ShowPatch` and `SessionPatch` are RFC 6902 operations and an
 * operation is only meaningful against a document root; the programmer travels
 * whole, and it is here rather than in a delta straight afterwards because §9's
 * *snapshot completeness* row would otherwise be false for it.
 */
export interface Snapshot {
  /** The show document: patch, groups, presets, sequences, executors. */
  readonly show: JsonValue;
  /** The session document: `{ session, views }`. */
  readonly session: JsonValue;
  /** The programmer, whole. */
  readonly programmer: ProgrammerState;
  /** One entry per configured DMX output. */
  readonly outputs: readonly OutputSnapshot[];
  /** How the daemon itself is doing. */
  readonly health: DaemonHealth;
  /**
   * How many profiles **this desk** can embed into a show.
   *
   * A number and not the profiles — S44. S27 carried the whole list here, which
   * was right for four and impossible for two thousand: a snapshot has to fit in
   * a frame (§3), and a menu of two thousand entries is not a menu. So a client
   * **searches** (`Query::SearchLibrary`) and this is only what it needs to say
   * *2 157 profiles* beside the box.
   */
  readonly fixtureLibrary: number;
  /** What this machine is set to — S37's fourth panel. */
  readonly machine: MachineSettings;
  /** Which show file is open, and what the autosave is doing — S37. */
  readonly showFile: ShowFileInfo;
}

/** Everything the daemon may send. */
export type ServerMessage =
  | { readonly t: "Snapshot"; readonly snapshot: Snapshot }
  | { readonly t: "Delta"; readonly delta: Delta }
  | { readonly t: "Telemetry"; readonly data: Payload }
  | { readonly t: "Ack"; readonly seq: number }
  | { readonly t: "Answer"; readonly seq: number; readonly answer: Answer }
  | {
      readonly t: "Reject";
      readonly seq: number | null;
      readonly reason: RejectReason;
      readonly message: string;
    };

/** A hello for this build. */
export function hello(clientKind: ClientKind, token: string | null = null): Hello {
  return { protocolVersion: PROTOCOL_VERSION, clientKind, token };
}

/* ------------------------------------------------------------------------- */
/* Readers                                                                     */
/* ------------------------------------------------------------------------- */

/** One RFC 6902 operation. */
function readPatchOp(value: unknown, path: string): JsonPatchOp {
  const record = asRecord(value, path);
  const op = asString(field(record, "op"), `${path}.op`);
  switch (op) {
    case "add":
      return {
        op: "add",
        path: asString(field(record, "path"), `${path}.path`),
        value: asJsonValue(field(record, "value"), `${path}.value`),
      };
    case "remove":
      return { op: "remove", path: asString(field(record, "path"), `${path}.path`) };
    case "replace":
      return {
        op: "replace",
        path: asString(field(record, "path"), `${path}.path`),
        value: asJsonValue(field(record, "value"), `${path}.value`),
      };
    case "move":
      return {
        op: "move",
        from: asString(field(record, "from"), `${path}.from`),
        path: asString(field(record, "path"), `${path}.path`),
      };
    case "copy":
      return {
        op: "copy",
        from: asString(field(record, "from"), `${path}.from`),
        path: asString(field(record, "path"), `${path}.path`),
      };
    case "test":
      return {
        op: "test",
        path: asString(field(record, "path"), `${path}.path`),
        value: asJsonValue(field(record, "value"), `${path}.value`),
      };
    default:
      throw new ProtocolFault(`${path}.op`, `an RFC 6902 operation, not ${JSON.stringify(op)}`);
  }
}

/** The operations of a patch delta. */
function readPatchOps(value: unknown, path: string): JsonPatchOp[] {
  return asArray(value, path).map((op, index) => readPatchOp(op, `${path}[${index}]`));
}

/** One touched value. */
function readProgrammerValue(value: unknown, path: string): ProgrammerValue {
  const record = asRecord(value, path);
  return {
    value: asInteger(field(record, "value"), `${path}.value`),
    source: asVariant(field(record, "source"), `${path}.source`, PROGRAMMER_VALUE_SOURCE_VARIANTS),
    presetRef: asNullable(field(record, "presetRef"), `${path}.presetRef`, asInteger),
  };
}

/** One entry of the programmer's flat value list. */
function readProgrammerEntry(value: unknown, path: string): ProgrammerEntry {
  const record = asRecord(value, path);
  return {
    fixture: asInteger(field(record, "fixture"), `${path}.fixture`),
    attribute: asVariant(field(record, "attribute"), `${path}.attribute`, ATTRIBUTE_TYPE_VARIANTS),
    occurrence: readOccurrence(record, path),
    value: readProgrammerValue(field(record, "value"), `${path}.value`),
  };
}

/**
 * Which channel of a kind an entry is — **S52**, and it is read here rather
 * than left out.
 *
 * # What leaving it out cost
 *
 * It reached the Rust struct, the command, the applier, the serialiser and the
 * generated binding, and it did not reach the two hand-written decoders. So a
 * daemon that sent `White` occurrence 1 was understood as occurrence 0: on a
 * Stairville MH z195 — whose profile declares a warm *and* a cold white, both
 * as `White` — turning the cold one moved the lamp and moved the **warm**
 * encoder's number, while the encoder that was turned sat still.
 *
 * The reason nothing caught it is worth keeping. The field is optional on the
 * wire, because a `.prism` file written before S52 does not carry one, so the
 * binding is `occurrence?: number` and an object without it is a valid
 * `ProgrammerEntry` as far as the compiler is concerned. **Optionality granted
 * for the sake of an old file is what made forgetting it legal**, and that is
 * the shape to look for wherever a `#[serde(default)]` meets a hand decoder.
 *
 * # Absent is the first, and it is normalised here
 *
 * Not left absent for every reader to default: the two provenance lists a few
 * lines below are already normalised to `[]` the same way, and the alternative
 * is `?? 0` written at each call site — the second one of which is the one that
 * gets forgotten.
 */
function readOccurrence(record: Record<string, unknown>, path: string): number {
  return asInteger(field(record, "occurrence") ?? 0, `${path}.occurrence`);
}

/**
 * The stages the Clear key can be at — `prism_domain::ClearStage`.
 *
 * **Four since S43**, and this constant exists because three was written out by
 * hand here and the widening on the Rust side did not reach it. What that cost
 * is in the module documentation above; what it buys is that the bound is stated
 * once and named, so the next session to widen the enum has something to grep
 * for.
 *
 * `ProgrammerState["clearStage"]` is generated (`#[ts(type = "0 | 1 | 2 | 3")]`),
 * so a stage this build does not know is a **type** error at every call site the
 * moment the binding is regenerated — and a run-time fault here for a daemon
 * that is newer than this interface.
 */
const CLEAR_STAGES: readonly ProgrammerState["clearStage"][] = [0, 1, 2, 3];

/** The programmer, whole — the third document. */
export function readProgrammerState(value: unknown, path: string): ProgrammerState {
  const record = asRecord(value, path);
  const stage = asInteger(field(record, "clearStage"), `${path}.clearStage`);
  const clearStage = CLEAR_STAGES.find((known) => known === stage);
  if (clearStage === undefined) {
    throw new ProtocolFault(
      `${path}.clearStage`,
      `one of ${CLEAR_STAGES.join(", ")}, not ${stage}`,
    );
  }
  return {
    selection: asArray(field(record, "selection"), `${path}.selection`).map((id, index) =>
      asInteger(id, `${path}.selection[${index}]`),
    ),
    // **The two provenance lists, optional on the wire** — S43, B27. They are
    // `#[serde(default)]` on the daemon side, so a snapshot written before they
    // existed decodes with both empty, and an empty pair is exactly the state
    // *nothing was selected through a group*. Refusing a snapshot over them
    // would be refusing to open a show for a reading nothing on the screen
    // needs to be right about.
    selectedGroups: asArray(field(record, "selectedGroups") ?? [], `${path}.selectedGroups`).map(
      (id, index) => asInteger(id, `${path}.selectedGroups[${index}]`),
    ),
    manualSelection: asArray(
      field(record, "manualSelection") ?? [],
      `${path}.manualSelection`,
    ).map((id, index) => asInteger(id, `${path}.manualSelection[${index}]`)),
    activeFeatureGroup: asVariant(
      field(record, "activeFeatureGroup"),
      `${path}.activeFeatureGroup`,
      FEATURE_GROUP_VARIANTS,
    ),
    values: asArray(field(record, "values"), `${path}.values`).map((entry, index) =>
      readProgrammerEntry(entry, `${path}.values[${index}]`),
    ),
    clearStage,
  };
}

/** One change the daemon has already applied. */
export function readDelta(value: unknown, path: string): Delta {
  const record = asRecord(value, path);
  const tag = asString(field(record, "t"), `${path}.t`);
  switch (tag) {
    case "ShowPatch":
      return { t: "ShowPatch", ops: readPatchOps(field(record, "ops"), `${path}.ops`) };
    case "SessionPatch":
      return { t: "SessionPatch", ops: readPatchOps(field(record, "ops"), `${path}.ops`) };
    case "ProgrammerChanged":
      return {
        t: "ProgrammerChanged",
        state: readProgrammerState(field(record, "state"), `${path}.state`),
      };
    case "PlaybackState":
      return {
        t: "PlaybackState",
        // **A cue list number** since S45: a playback is a sequence's, and
        // there is exactly one of them per list (`prism_domain::PlaybackId`).
        // It was a tagged pair with an executor in one arm, and the executors
        // are what punch-list entry B18 found two of.
        playback: asInteger(field(record, "playback"), `${path}.playback`),
        isActive: asBoolean(field(record, "isActive"), `${path}.isActive`),
        cueIndex: asNullable(field(record, "cueIndex"), `${path}.cueIndex`, asInteger),
      };
    // **S33's two and S37's two, and the first pair were missing.** A daemon
    // whose rig changed sent `OutputsChanged`, this decoder threw, and the
    // connection resynchronised — which looked like nothing at all until S37
    // built a panel that changes a rig from a browser. The store has handled
    // this delta since S33; what it never had was a way through the door.
    case "OutputsChanged":
      return {
        t: "OutputsChanged",
        outputs: asArray(field(record, "outputs"), `${path}.outputs`).map((output, index) =>
          readOutputInstance(output, `${path}.outputs[${index}]`),
        ),
      };
    case "SurfaceChanged":
      return {
        t: "SurfaceChanged",
        port: readOptionalString(field(record, "port"), `${path}.port`),
      };
    // **S38's two, and S37 is why they are written the same day the panel is.**
    // A delta with no caller is a decoder arm nobody writes: `readDelta` had no
    // `OutputsChanged` and no `SurfaceChanged` until a browser could change a
    // rig, and a daemon that sent one would have faulted the connection. So the
    // control editor's two arrive with the editor, and `ui_session` sends both
    // in a recording.
    case "SurfaceBindingsChanged":
      return {
        t: "SurfaceBindingsChanged",
        revision: asInteger(field(record, "revision"), `${path}.revision`),
      };
    case "SurfaceLearnChanged":
      return {
        t: "SurfaceLearnChanged",
        learning: asBoolean(field(record, "learning"), `${path}.learning`),
        control: readOptionalBoundControl(field(record, "control"), `${path}.control`),
      };
    case "MachineChanged":
      return {
        t: "MachineChanged",
        settings: readMachineSettings(field(record, "settings"), `${path}.settings`),
      };
    case "ShowFileChanged":
      return {
        t: "ShowFileChanged",
        file: readShowFileInfo(field(record, "file"), `${path}.file`),
      };
    case "OutputHealth":
      return {
        t: "OutputHealth",
        outputId: asInteger(field(record, "outputId"), `${path}.outputId`),
        health: asVariant(field(record, "health"), `${path}.health`, OUTPUT_HEALTH_VARIANTS),
      };
    case "DirtyFlag":
      return {
        t: "DirtyFlag",
        unsavedChanges: asBoolean(field(record, "unsavedChanges"), `${path}.unsavedChanges`),
      };
    case "Notice":
      return {
        t: "Notice",
        level: asVariant(field(record, "level"), `${path}.level`, NOTICE_LEVEL_VARIANTS),
        message: asString(field(record, "message"), `${path}.message`),
      };
    default:
      throw new ProtocolFault(`${path}.t`, `a delta this build knows, not ${JSON.stringify(tag)}`);
  }
}

/** One overlapping pair, as the daemon worked it out. */
function readPatchConflict(value: unknown, path: string): PatchConflict {
  const record = asRecord(value, path);
  return {
    universe: asInteger(field(record, "universe"), `${path}.universe`),
    from: asInteger(field(record, "from"), `${path}.from`),
    to: asInteger(field(record, "to"), `${path}.to`),
    first: asInteger(field(record, "first"), `${path}.first`),
    second: asInteger(field(record, "second"), `${path}.second`),
  };
}

/** The list of them, in the daemon's order — which is patch-sheet order. */
function readPatchConflicts(value: unknown, path: string): PatchConflict[] {
  return asArray(value, path).map((entry, index) =>
    readPatchConflict(entry, `${path}[${index}]`),
  );
}

/** What patching a fixture would do. */
function readPatchPreview(value: unknown, path: string): PatchPreview {
  const record = asRecord(value, path);
  return {
    accepted: asBoolean(field(record, "accepted"), `${path}.accepted`),
    refusal: asNullable(field(record, "refusal"), `${path}.refusal`, asString),
    footprint: asInteger(field(record, "footprint"), `${path}.footprint`),
    lastAddress: asNullable(field(record, "lastAddress"), `${path}.lastAddress`, asInteger),
    conflicts: readPatchConflicts(field(record, "conflicts"), `${path}.conflicts`),
  };
}

/**
 * What a store would do, as the daemon worked it out.
 *
 * The `mode` is narrowed against the generated table rather than taken as read.
 * It is the mode the **question** carried since S39, echoed back, so a bar
 * drawing an answer beside a chooser that has since moved cannot describe the
 * wrong one — and a daemon naming a mode this build cannot draw is refused
 * rather than rendered as a word nobody chose.
 */
function readStorePreview(value: unknown, path: string): StorePreview {
  const record = asRecord(value, path);
  const mode = asString(field(record, "mode"), `${path}.mode`);
  if (!isStoreMode(mode)) {
    throw new ProtocolFault(
      `${path}.mode`,
      `a store mode this build knows, not ${JSON.stringify(mode)}`,
    );
  }
  return {
    accepted: asBoolean(field(record, "accepted"), `${path}.accepted`),
    refusal: asNullable(field(record, "refusal"), `${path}.refusal`, asString),
    exists: asBoolean(field(record, "exists"), `${path}.exists`),
    name: asString(field(record, "name"), `${path}.name`),
    mode,
    added: asInteger(field(record, "added"), `${path}.added`),
    replaced: asInteger(field(record, "replaced"), `${path}.replaced`),
    kept: asInteger(field(record, "kept"), `${path}.kept`),
    removed: asInteger(field(record, "removed"), `${path}.removed`),
  };
}

/** Whether a string is one of the store modes this build knows. */
function isStoreMode(value: string): value is StoreMode {
  return (STORE_MODE_VARIANTS as readonly string[]).includes(value);
}

/** The daemon's answer to a question. */
export function readAnswer(value: unknown, path: string): Answer {
  const record = asRecord(value, path);
  const tag = asString(field(record, "t"), `${path}.t`);
  switch (tag) {
    case "PatchConflicts":
      return {
        t: "PatchConflicts",
        conflicts: readPatchConflicts(field(record, "conflicts"), `${path}.conflicts`),
      };
    case "PatchPreview":
      return {
        t: "PatchPreview",
        preview: readPatchPreview(field(record, "preview"), `${path}.preview`),
      };
    case "StorePreview":
      return {
        t: "StorePreview",
        preview: readStorePreview(field(record, "preview"), `${path}.preview`),
      };
    case "LibraryMatches":
      return {
        t: "LibraryMatches",
        matches: asArray(field(record, "matches"), `${path}.matches`).map((entry, index) =>
          readLibraryEntry(entry, `${path}.matches[${index}]`),
        ),
        total: asInteger(field(record, "total"), `${path}.total`),
      };
    // **S36's and S37's, and the first was missing too.** `Query::MidiPorts`
    // has existed since S36 and nothing in this interface had asked one until
    // S37's Devices panel; an answer this decoder threw on would have looked
    // like a daemon that sends nonsense, and the connection would have
    // resynchronised rather than the panel drawing a list.
    case "MidiPorts":
      return {
        t: "MidiPorts",
        ports: asArray(field(record, "ports"), `${path}.ports`).map((port, index) =>
          readMidiPortInfo(port, `${path}.ports[${index}]`),
        ),
        configured: readOptionalString(field(record, "configured"), `${path}.configured`),
        open: readOptionalString(field(record, "open"), `${path}.open`),
        status: readOptionalSurfaceStatus(field(record, "status"), `${path}.status`),
      };
    case "OutputStatus":
      return {
        t: "OutputStatus",
        outputs: asArray(field(record, "outputs"), `${path}.outputs`).map((entry, index) =>
          readOutputStatusInfo(entry, `${path}.outputs[${index}]`),
        ),
      };
    // S46. `listening` is read before `nodes` is, everywhere this answer is
    // drawn: an empty list under a socket that never opened says nothing at all
    // about the network, and reading it as *no nodes* would be punch-list B6's
    // mistake pointed the other way.
    case "ArtNetNodes":
      return {
        t: "ArtNetNodes",
        nodes: asArray(field(record, "nodes"), `${path}.nodes`).map((node, index) =>
          readArtNetNodeInfo(node, `${path}.nodes[${index}]`),
        ),
        listening: asBoolean(field(record, "listening"), `${path}.listening`),
        error: readOptionalString(field(record, "error"), `${path}.error`),
        counters: readArtNetCounters(field(record, "counters"), `${path}.counters`),
        // Words, and the daemon's — never a sentence this file writes. The
        // obvious one a client would compose from the counters is *check the
        // node*, and the node is the one thing that is working when this
        // appears. `Answer::MidiPorts`' remedy is the precedent (S37).
        remedy: readOptionalString(field(record, "remedy"), `${path}.remedy`),
      };
    // S48. What every cue of one list **inherits**, which is the one thing a
    // cue sheet cannot work out for itself: folding the cues would be a second
    // implementation of the rule the engine resolves a `Goto` through, and two
    // answers to *where am I* is the fault S48 exists to remove.
    case "CueTracking":
      return {
        t: "CueTracking",
        sequenceId: asInteger(field(record, "sequenceId"), `${path}.sequenceId`),
        cues: asArray(field(record, "cues"), `${path}.cues`).map((row, index) =>
          readCueTrackingRow(row, `${path}.cues[${index}]`),
        ),
      };
    // S49. What the line under the operator's fingers would do — the reading
    // under the box, and the two things only the daemon can say about it:
    // whether the destination a store names is already there, and which words
    // to ask with. A client that worked either out would be the second opinion
    // moving the parser was meant to remove.
    case "CommandLineReading":
      return {
        t: "CommandLineReading",
        text: asString(field(record, "text"), `${path}.text`),
        reading: asString(field(record, "reading"), `${path}.reading`),
        kind: asVariant(
          field(record, "kind"),
          `${path}.kind`,
          COMMAND_LINE_READING_KIND_VARIANTS,
        ),
        commands: asInteger(field(record, "commands"), `${path}.commands`),
        verb: asBoolean(field(record, "verb"), `${path}.verb`),
        clearing: asBoolean(field(record, "clearing"), `${path}.clearing`),
        question: readOptionalCommandLineQuestion(
          field(record, "question"),
          `${path}.question`,
        ),
        completions: asArray(field(record, "completions"), `${path}.completions`).map(
          (word, index) => asString(word, `${path}.completions[${index}]`),
        ),
      };
    case "DarkUniverses":
      return {
        t: "DarkUniverses",
        universes: asArray(field(record, "universes"), `${path}.universes`).map(
          (universe, index) => asInteger(universe, `${path}.universes[${index}]`),
        ),
      };
    // S38. The table in force, one row per control the surface has - see
    // `readSurfaceControl` for the two fields on each row that are the device
    // profile's rather than the table's.
    case "SurfaceBindings":
      return {
        t: "SurfaceBindings",
        controls: asArray(field(record, "controls"), `${path}.controls`).map((entry, index) =>
          readSurfaceControl(entry, `${path}.controls[${index}]`),
        ),
        device: asString(field(record, "device"), `${path}.device`),
        // The two an **export** needs — S43. Optional on the wire for the same
        // reason the programmer's provenance lists are: a daemon one version
        // behind sends the row without them, and an export that cannot name its
        // device is better than a settings window that refuses to open.
        deviceKey: asString(field(record, "deviceKey") ?? "", `${path}.deviceKey`),
        profileVersion: asInteger(
          field(record, "profileVersion") ?? 0,
          `${path}.profileVersion`,
        ),
        profile: readOptionalString(field(record, "profile"), `${path}.profile`),
        revision: asInteger(field(record, "revision"), `${path}.revision`),
        learning: asBoolean(field(record, "learning"), `${path}.learning`),
      };
    default:
      throw new ProtocolFault(`${path}.t`, `an answer this build knows, not ${JSON.stringify(tag)}`);
  }
}

/**
 * The question a line is holding, or nothing at all — S49.
 *
 * Absent on almost every line, so it is read as optional rather than as a null:
 * a daemon that sends nothing for it is a line with nothing to ask, which is the
 * ordinary case and not a message this build could not read.
 */
function readOptionalCommandLineQuestion(
  value: unknown,
  path: string,
): CommandLineQuestion | null {
  if (value === undefined || value === null) {
    return null;
  }
  const record = asRecord(value, path);
  return {
    what: asString(field(record, "what"), `${path}.what`),
    // **Checked against the generated table**, never asserted: the words are
    // what the prompt's buttons send back in `CommandLineInput.mode`, and a
    // word this build does not know would be a button that means nothing.
    modes: asArray(field(record, "modes"), `${path}.modes`).map((mode, index) =>
      asVariant(mode, `${path}.modes[${index}]`, COMMAND_LINE_MODE_VARIANTS),
    ),
  };
}

/**
 * What one cue inherits, and whether anything reaches past it — S48.
 *
 * `blocks` is the daemon's own reading and not `inherited.length === 0` worked
 * out here. The two agree today and the client must not be the one deciding
 * that they do: what counts as *asserting everything* is the tracking rule's
 * answer, and this file has no tracking rule in it — deliberately.
 */
function readCueTrackingRow(value: unknown, path: string): CueTrackingRow {
  const record = asRecord(value, path);
  return {
    number: asString(field(record, "number"), `${path}.number`),
    inherited: asArray(field(record, "inherited"), `${path}.inherited`).map((entry, index) =>
      readTrackedValue(entry, `${path}.inherited[${index}]`),
    ),
    blocks: asBoolean(field(record, "blocks"), `${path}.blocks`),
  };
}

/** One attribute of one fixture, held at a value — S48. */
function readTrackedValue(value: unknown, path: string): TrackedValue {
  const record = asRecord(value, path);
  return {
    fixture: asInteger(field(record, "fixture"), `${path}.fixture`),
    attribute: asVariant(field(record, "attribute"), `${path}.attribute`, ATTRIBUTE_TYPE_VARIANTS),
    // The same field, dropped in the same way — see {@link readOccurrence}. A
    // cue sheet without it shows the second colour wheel's tracked value on the
    // first one's row.
    occurrence: readOccurrence(record, path),
    value: asInteger(field(record, "value"), `${path}.value`),
  };
}

/**
 * One control of the surface, as the control editor draws it — S38.
 *
 * `permanent` and `reserved` are **not** derived here and must not be: both are
 * properties of the device profile (`docs/MCU_MAPPING.md` §4.3) and this client
 * holds none. Telling an operator that a key is always in reach while the desk
 * is showing the sound console is exactly the mistake that section exists to
 * prevent, so the daemon is asked.
 *
 * The `action` is decoded rather than trusted: it arrives as a tagged union with
 * as many shapes as the vocabulary has, and `CLAUDE.md` forbids `as`.
 */
function readSurfaceControl(value: unknown, path: string): SurfaceControl {
  const record = asRecord(value, path);
  return {
    control: readBoundControl(field(record, "control"), `${path}.control`),
    name: asString(field(record, "name"), `${path}.name`),
    action: readOptionalSurfaceAction(field(record, "action"), `${path}.action`),
    permanent: asBoolean(field(record, "permanent"), `${path}.permanent`),
    reserved: asBoolean(field(record, "reserved"), `${path}.reserved`),
  };
}

/** Which control a binding names — `docs/MCU_MAPPING.md` §4.2's control list. */
function readBoundControl(value: unknown, path: string): BoundControl {
  const record = asRecord(value, path);
  const tag = asString(field(record, "t"), `${path}.t`);
  switch (tag) {
    case "StripFader":
    case "StripEncoder":
    case "MainFader":
    case "Jog":
      return { t: tag };
    case "StripButton":
      return {
        t: "StripButton",
        button: asVariant(field(record, "button"), `${path}.button`, STRIP_BUTTON_VARIANTS),
      };
    case "Global":
      return {
        t: "Global",
        button: asVariant(field(record, "button"), `${path}.button`, GLOBAL_BUTTON_VARIANTS),
      };
    default:
      throw new ProtocolFault(`${path}.t`, `a control this build knows, not ${JSON.stringify(tag)}`);
  }
}

/** What a control does, or nothing at all. */
function readOptionalSurfaceAction(value: unknown, path: string): SurfaceAction | null {
  return value === null || value === undefined ? null : readSurfaceAction(value, path);
}

/**
 * What a control does — the whole vocabulary of `docs/MCU_MAPPING.md` §4.
 *
 * Written out arm by arm rather than cast, for the reason every decoder in this
 * file is: what arrives off a socket is `unknown`, and a union with seventeen
 * shapes is seventeen chances for a daemon one version ahead to hand this build
 * a field it does not have.
 */
function readSurfaceAction(value: unknown, path: string): SurfaceAction {
  const record = asRecord(value, path);
  const tag = asString(field(record, "t"), `${path}.t`);
  const target = (): ExecutorTarget =>
    asVariant(field(record, "target"), `${path}.target`, EXECUTOR_TARGET_VARIANTS);
  switch (tag) {
    case "ExecutorMaster":
    case "ExecutorOff":
    case "SelectExecutor":
      return { t: tag, target: target() };
    case "ExecutorGo":
      return {
        t: "ExecutorGo",
        target: target(),
        direction: asVariant(field(record, "direction"), `${path}.direction`, GO_DIRECTION_VARIANTS),
      };
    case "ExecutorButton":
      return {
        t: "ExecutorButton",
        target: target(),
        button: readExecutorButtonRef(field(record, "button"), `${path}.button`),
      };
    case "ClearProgrammer":
    case "AdjustParameter":
    case "SaveShow":
    case "Oops":
    case "Redo":
      return { t: tag };
    case "ExecutorPage":
    case "ProgrammerPage":
      return { t: tag, delta: asInteger(field(record, "delta"), `${path}.delta`) };
    case "SelectView":
      return { t: "SelectView", view: asInteger(field(record, "view"), `${path}.view`) };
    case "StepView":
      return {
        t: "StepView",
        direction: asVariant(field(record, "direction"), `${path}.direction`, STEP_VARIANTS),
      };
    case "SelectProgrammerParam":
      return {
        t: "SelectProgrammerParam",
        direction: asVariant(
          field(record, "direction"),
          `${path}.direction`,
          PARAM_DIRECTION_VARIANTS,
        ),
      };
    case "SetEncoderBank":
      return {
        t: "SetEncoderBank",
        group: asVariant(field(record, "group"), `${path}.group`, FEATURE_GROUP_VARIANTS),
      };
    case "OpenWindow":
      return {
        t: "OpenWindow",
        window: asVariant(field(record, "window"), `${path}.window`, WINDOW_TYPE_VARIANTS),
      };
    default:
      throw new ProtocolFault(`${path}.t`, `an action this build knows, not ${JSON.stringify(tag)}`);
  }
}

/** Which of an executor's buttons — a hardware slot, or a named function. */
function readExecutorButtonRef(value: unknown, path: string): ExecutorButtonRef {
  const record = asRecord(value, path);
  const tag = asString(field(record, "t"), `${path}.t`);
  switch (tag) {
    case "Slot":
      return { t: "Slot", index: asInteger(field(record, "index"), `${path}.index`) };
    case "Function":
      return {
        t: "Function",
        function: readExecutorButtonFunction(field(record, "function"), `${path}.function`),
      };
    default:
      throw new ProtocolFault(`${path}.t`, `a button this build knows, not ${JSON.stringify(tag)}`);
  }
}

/**
 * What a key does: one of the eight fixed functions, or a line an operator
 * wrote — S45's custom row.
 *
 * Narrowed rather than asserted, like every other reader here: the eight go
 * through the generated table, and the ninth is checked member by member.
 * `desk/functions.ts` owns that check, because the strip and the control editor
 * make it against a mirrored document rather than against the wire.
 */
function readExecutorButtonFunction(value: unknown, path: string): ExecutorButtonFunction {
  if (typeof value === "string") {
    return asVariant(value, path, FIXED_BUTTON_FUNCTIONS);
  }
  const record = asRecord(value, path);
  const custom = asRecord(field(record, "CommandLine"), `${path}.CommandLine`);
  return { CommandLine: { line: asString(field(custom, "line"), `${path}.CommandLine.line`) } };
}

/** The control learn just named, if this delta is that moment. */
function readOptionalBoundControl(value: unknown, path: string): BoundControl | null {
  return value === null || value === undefined ? null : readBoundControl(value, path);
}

/**
 * What one output's driver is doing — S37.
 *
 * The **configuration** is not in it and must not be: that arrives whole in
 * `Delta::OutputsChanged` whenever it moves, and repeating a rig on the wire once
 * a second to carry a counter would be the wrong trade twice over.
 */
function readOutputStatusInfo(value: unknown, path: string): OutputStatusInfo {
  const record = asRecord(value, path);
  return {
    id: asInteger(field(record, "id"), `${path}.id`),
    health: asVariant(field(record, "health"), `${path}.health`, OUTPUT_HEALTH_VARIANTS),
    framesSent: asInteger(field(record, "framesSent"), `${path}.framesSent`),
    lastError: readOptionalString(field(record, "lastError"), `${path}.lastError`),
    lastErrorAgoMs: readOptionalInteger(
      field(record, "lastErrorAgoMs"),
      `${path}.lastErrorAgoMs`,
    ),
    // S46. Optional on the wire, and **empty** rather than absent when there is
    // nothing to say: a daemon one version behind sends the row without it, and
    // a settings window that refused to open over a missing field would be worse
    // than one drawing a health column it cannot qualify.
    nodes: asArray(field(record, "nodes") ?? [], `${path}.nodes`).map((node, index) =>
      readNodeReach(node, `${path}.nodes[${index}]`),
    ),
  };
}

/**
 * One node an output sends to, and whether anything there answers — S46.
 *
 * `lastReplyAgoMs` is an **age**, not a time: the daemon and a browser share no
 * clock, so the daemon says how long ago and this renders it against the
 * browser's own — S33's rule for `lastErrorAgoMs` one field along.
 */
function readNodeReach(value: unknown, path: string): NodeReach {
  const record = asRecord(value, path);
  return {
    address: asString(field(record, "address"), `${path}.address`),
    health: asVariant(field(record, "health"), `${path}.health`, NODE_HEALTH_VARIANTS),
    name: readOptionalString(field(record, "name"), `${path}.name`),
    lastReplyAgoMs: readOptionalInteger(
      field(record, "lastReplyAgoMs"),
      `${path}.lastReplyAgoMs`,
    ),
  };
}

/**
 * What this desk's discovery has done and been sent — S46.
 *
 * Optional on the wire, and **zero** rather than absent when it is missing: a
 * daemon one version behind sends the answer without it, and a settings window
 * that refused to open over a missing counter would be worse than one drawing
 * zeroes.
 */
function readArtNetCounters(value: unknown, path: string): ArtNetCounters {
  const record = value === undefined || value === null ? {} : asRecord(value, path);
  const count = (name: string): number => asInteger(field(record, name) ?? 0, `${path}.${name}`);
  return {
    pollsSent: count("pollsSent"),
    pollsFailed: count("pollsFailed"),
    replies: count("replies"),
    malformed: count("malformed"),
    dropped: count("dropped"),
    readErrors: count("readErrors"),
  };
}

/**
 * One node that answered an `ArtPoll` — S46.
 *
 * `configured`, `unaddressedPorts` and `missingPorts` are **not** derived here
 * and must not be: they are the rig intersected with the discovery table, and a
 * client that did that intersection itself would be a second opinion about
 * something the daemon holds both halves of. That is `PatchPreview`'s trap, one
 * panel along, and the one that drifts the first time a port-address default
 * changes.
 */
function readArtNetNodeInfo(value: unknown, path: string): ArtNetNodeInfo {
  const record = asRecord(value, path);
  const ports = (name: string): number[] =>
    asArray(field(record, name), `${path}.${name}`).map((port, index) =>
      asInteger(port, `${path}.${name}[${index}]`),
    );
  return {
    address: asString(field(record, "address"), `${path}.address`),
    ip: asString(field(record, "ip"), `${path}.ip`),
    shortName: asString(field(record, "shortName"), `${path}.shortName`),
    longName: asString(field(record, "longName"), `${path}.longName`),
    mac: asString(field(record, "mac"), `${path}.mac`),
    firmware: asInteger(field(record, "firmware"), `${path}.firmware`),
    style: asInteger(field(record, "style"), `${path}.style`),
    status1: asInteger(field(record, "status1"), `${path}.status1`),
    status2: asInteger(field(record, "status2"), `${path}.status2`),
    ports: ports("ports"),
    inputs: ports("inputs"),
    configured: asBoolean(field(record, "configured"), `${path}.configured`),
    unaddressedPorts: ports("unaddressedPorts"),
    missingPorts: ports("missingPorts"),
    // The default port-address mapping run backwards, and the daemon's rather
    // than this file's: `ARCHITECTURE_SPEC.md` §7.0 states it and `prism-domain`
    // implements it, so a third spelling here would drift the first time it
    // moved.
    suggestedUniverses: asArray(
      field(record, "suggestedUniverses"),
      `${path}.suggestedUniverses`,
    ).map((universe, index) => asInteger(universe, `${path}.suggestedUniverses[${index}]`)),
    replies: asInteger(field(record, "replies"), `${path}.replies`),
    lastReplyAgoMs: asInteger(field(record, "lastReplyAgoMs"), `${path}.lastReplyAgoMs`),
  };
}

/** One MIDI port, as the operating system offers it. */
function readMidiPortInfo(value: unknown, path: string): MidiPortInfo {
  const record = asRecord(value, path);
  return {
    name: asString(field(record, "name"), `${path}.name`),
    input: asBoolean(field(record, "input"), `${path}.input`),
    output: asBoolean(field(record, "output"), `${path}.output`),
  };
}

/**
 * What the attached surface is doing, or `null` when there is none.
 *
 * `null` and *disconnected* are different facts and the protocol keeps them
 * apart: a laptop with no port configured has nothing to report, and a desk that
 * is switched off has a health and a set of counters that happen to be zero.
 */
function readOptionalSurfaceStatus(value: unknown, path: string): SurfaceStatus | null {
  if (value === undefined || value === null) {
    return null;
  }
  const record = asRecord(value, path);
  return {
    health: asVariant(field(record, "health"), `${path}.health`, SURFACE_HEALTH_VARIANTS),
    remedy: readOptionalString(field(record, "remedy"), `${path}.remedy`),
    sent: asInteger(field(record, "sent"), `${path}.sent`),
    superseded: asInteger(field(record, "superseded"), `${path}.superseded`),
    touchSuppressed: asInteger(field(record, "touchSuppressed"), `${path}.touchSuppressed`),
    resyncs: asInteger(field(record, "resyncs"), `${path}.resyncs`),
    reserved: asInteger(field(record, "reserved"), `${path}.reserved`),
    probes: asInteger(field(record, "probes"), `${path}.probes`),
    reconnects: asInteger(field(record, "reconnects"), `${path}.reconnects`),
    profile: readOptionalString(field(record, "profile"), `${path}.profile`),
    boundControls: asInteger(field(record, "boundControls"), `${path}.boundControls`),
  };
}

/** One profile of the desk's library, as a menu shows it. */
function readLibraryEntry(value: unknown, path: string): LibraryEntry {
  const record = asRecord(value, path);
  return {
    id: asString(field(record, "id"), `${path}.id`),
    manufacturer: asString(field(record, "manufacturer"), `${path}.manufacturer`),
    name: asString(field(record, "name"), `${path}.name`),
    mode: asString(field(record, "mode"), `${path}.mode`),
    footprint: asInteger(field(record, "footprint"), `${path}.footprint`),
    // **B43.** Absent means *not the venue's own*, which is what every entry
    // was before S51 and what a recording made before it carries. Read
    // defensively rather than through `asBoolean`, for the reason the whole
    // file exists: an entry is a mirror of a document, and a missing field must
    // draw an unmarked row rather than fault the whole answer.
    own: field(record, "own") === true,
  };
}

/** One configured output. */
function readOutputSnapshot(value: unknown, path: string): OutputSnapshot {
  const record = asRecord(value, path);
  return {
    id: asInteger(field(record, "id"), `${path}.id`),
    name: asString(field(record, "name"), `${path}.name`),
    health: asVariant(field(record, "health"), `${path}.health`, OUTPUT_HEALTH_VARIANTS),
    // The four S33 added, each optional on the wire: a daemon one version
    // behind sends a three-field row, and meeting a missing field is better
    // than refusing a snapshot over a status panel.
    output: readOptionalOutputInstance(field(record, "output"), `${path}.output`),
    framesSent: asInteger(field(record, "framesSent") ?? 0, `${path}.framesSent`),
    lastError: readOptionalString(field(record, "lastError"), `${path}.lastError`),
    lastErrorAgoMs: readOptionalInteger(
      field(record, "lastErrorAgoMs"),
      `${path}.lastErrorAgoMs`,
    ),
  };
}

/** A configured output row, or `null` when there is none. */
function readOutputInstance(value: unknown, path: string): OutputInstance {
  const record = asRecord(value, path);
  return {
    id: asInteger(field(record, "id"), `${path}.id`),
    name: asString(field(record, "name"), `${path}.name`),
    // The kind is a tagged union whose arms a status panel does not narrow: it
    // is drawn through `OutputKind`'s own `t`, and a client that re-derived the
    // arms here would be a second copy of the generated type.
    kind: field(record, "kind") as OutputKind,
    universes: asArray(field(record, "universes"), `${path}.universes`).map((universe, index) =>
      asInteger(universe, `${path}.universes[${index}]`),
    ),
    enabled: asBoolean(field(record, "enabled"), `${path}.enabled`),
  };
}

function readOptionalOutputInstance(value: unknown, path: string): OutputInstance | null {
  return value === undefined || value === null ? null : readOutputInstance(value, path);
}

function readOptionalString(value: unknown, path: string): string | null {
  return value === undefined || value === null ? null : asString(value, path);
}

function readOptionalInteger(value: unknown, path: string): number | null {
  return value === undefined || value === null ? null : asInteger(value, path);
}

/**
 * What this machine is set to — S37's fourth panel.
 *
 * Every field is checked rather than asserted, like every other reader here:
 * `CLAUDE.md` forbids `any` and `as` is a claim rather than a check. The two
 * addresses are strings on the wire (`prism_domain::socket`) and stay strings
 * here, because what a panel does with one is print it.
 */
function readMachineSettings(value: unknown, path: string): MachineSettings {
  const record = asRecord(value, path);
  return {
    deskId: asString(field(record, "deskId"), `${path}.deskId`),
    dataDir: asString(field(record, "dataDir"), `${path}.dataDir`),
    local: asBoolean(field(record, "local"), `${path}.local`),
    websocket: readOptionalString(field(record, "websocket"), `${path}.websocket`),
    websocketOpen: readOptionalString(field(record, "websocketOpen"), `${path}.websocketOpen`),
    token: readOptionalString(field(record, "token"), `${path}.token`),
    logLevel: asVariant(field(record, "logLevel"), `${path}.logLevel`, LOG_LEVEL_VARIANTS),
    universes: asInteger(field(record, "universes"), `${path}.universes`),
    exitAction: asVariant(
      field(record, "exitAction"),
      `${path}.exitAction`,
      EXIT_ACTION_VARIANTS,
    ),
    autostart: asBoolean(field(record, "autostart"), `${path}.autostart`),
    fixtureLibrary: readOptionalString(field(record, "fixtureLibrary"), `${path}.fixtureLibrary`),
    surfaceProfile: readOptionalString(field(record, "surfaceProfile"), `${path}.surfaceProfile`),
    // A row this build does not know is **left out** rather than refused, which
    // is `canvas/windows.ts`'s rule for a window type: a daemon one version
    // ahead should cost a greyed-out row, not a connection.
    overrides: asArray(field(record, "overrides"), `${path}.overrides`).filter(
      (entry): entry is MachineOverride =>
        typeof entry === "string" &&
        (MACHINE_OVERRIDE_VARIANTS as readonly string[]).includes(entry),
    ),
  };
}

/** The settings, or what a daemon that has none looks like. */
function readOptionalMachineSettings(value: unknown, path: string): MachineSettings {
  return value === undefined || value === null ? NO_MACHINE : readMachineSettings(value, path);
}

/** What a daemon one version behind says about itself, which is nothing. */
const NO_MACHINE: MachineSettings = {
  deskId: "",
  dataDir: "",
  local: false,
  websocket: null,
  websocketOpen: null,
  token: null,
  logLevel: "Info",
  universes: 0,
  exitAction: "Hold",
  autostart: false,
  fixtureLibrary: null,
  surfaceProfile: null,
  overrides: [],
};

/** The show file, or what a daemon that does not say looks like. */
function readOptionalShowFileInfo(value: unknown, path: string): ShowFileInfo {
  return value === undefined || value === null ? NO_SHOW_FILE : readShowFileInfo(value, path);
}

/** A daemon that says nothing about its show file. */
const NO_SHOW_FILE: ShowFileInfo = {
  path: "",
  recent: [],
  unsavedChanges: false,
  recovery: false,
  autosaveSeconds: 0,
};

/** Which show file is open, and what the autosave is doing — S37. */
function readShowFileInfo(value: unknown, path: string): ShowFileInfo {
  const record = asRecord(value, path);
  return {
    path: asString(field(record, "path"), `${path}.path`),
    recent: asArray(field(record, "recent"), `${path}.recent`).map((entry, index) =>
      asString(entry, `${path}.recent[${index}]`),
    ),
    unsavedChanges: asBoolean(field(record, "unsavedChanges"), `${path}.unsavedChanges`),
    recovery: asBoolean(field(record, "recovery"), `${path}.recovery`),
    autosaveSeconds: asInteger(field(record, "autosaveSeconds"), `${path}.autosaveSeconds`),
  };
}

/** The daemon's own state. */
function readDaemonHealth(value: unknown, path: string): DaemonHealth {
  const record = asRecord(value, path);
  return {
    protocolVersion: asInteger(field(record, "protocolVersion"), `${path}.protocolVersion`),
    tickHz: asNumber(field(record, "tickHz"), `${path}.tickHz`),
    missedTicks: asInteger(field(record, "missedTicks"), `${path}.missedTicks`),
    unsavedChanges: asBoolean(field(record, "unsavedChanges"), `${path}.unsavedChanges`),
  };
}

/** The world. */
export function readSnapshot(value: unknown, path: string): Snapshot {
  const record = asRecord(value, path);
  return {
    show: asJsonValue(field(record, "show"), `${path}.show`),
    session: asJsonValue(field(record, "session"), `${path}.session`),
    programmer: readProgrammerState(field(record, "programmer"), `${path}.programmer`),
    outputs: asArray(field(record, "outputs"), `${path}.outputs`).map((output, index) =>
      readOutputSnapshot(output, `${path}.outputs[${index}]`),
    ),
    health: readDaemonHealth(field(record, "health"), `${path}.health`),
    fixtureLibrary: asInteger(field(record, "fixtureLibrary"), `${path}.fixtureLibrary`),
    // **Absent is a daemon one version behind, not a fault.** Both are
    // `#[serde(default)]` on the Rust side for exactly that reason, and this is
    // the same tolerance from the other end: a client that refused a snapshot
    // over a settings panel would refuse to draw a desk it can otherwise drive.
    // The rule is `OutputSnapshot`'s four S33 fields, one message out.
    machine: readOptionalMachineSettings(field(record, "machine"), `${path}.machine`),
    showFile: readOptionalShowFileInfo(field(record, "showFile"), `${path}.showFile`),
  };
}

/**
 * A decoded payload as a message the daemon may send.
 *
 * @throws {ProtocolFault} naming the field that was not what it should be.
 */
export function readServerMessage(value: unknown, path = "ServerMessage"): ServerMessage {
  const record = asRecord(value, path);
  const tag = asString(field(record, "t"), `${path}.t`);
  switch (tag) {
    case "Snapshot":
      return {
        t: "Snapshot",
        snapshot: readSnapshot(field(record, "snapshot"), `${path}.snapshot`),
      };
    case "Delta":
      return { t: "Delta", delta: readDelta(field(record, "delta"), `${path}.delta`) };
    case "Telemetry":
      return { t: "Telemetry", data: asBytes(field(record, "data"), `${path}.data`) };
    case "Ack":
      return { t: "Ack", seq: asInteger(field(record, "seq"), `${path}.seq`) };
    case "Answer":
      return {
        t: "Answer",
        seq: asInteger(field(record, "seq"), `${path}.seq`),
        answer: readAnswer(field(record, "answer"), `${path}.answer`),
      };
    case "Reject":
      return {
        t: "Reject",
        seq: asNullable(field(record, "seq"), `${path}.seq`, asInteger),
        reason: asVariant(field(record, "reason"), `${path}.reason`, REJECT_REASONS),
        message: asString(field(record, "message"), `${path}.message`),
      };
    default:
      throw new ProtocolFault(`${path}.t`, `a message this build knows, not ${JSON.stringify(tag)}`);
  }
}

/** A message's name, for an error a person has to read. */
export function describeServerMessage(message: ServerMessage): string {
  switch (message.t) {
    case "Snapshot":
      return "a snapshot";
    case "Delta":
      return "a delta";
    case "Telemetry":
      return "a telemetry frame";
    case "Ack":
      return "an acknowledgement";
    case "Answer":
      return "an answer";
    case "Reject":
      return "a rejection";
  }
}
