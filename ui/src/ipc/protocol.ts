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
  AttributeDef,
  Command,
  Delta,
  FixtureType,
  OutputHealth,
  OutputId,
  PatchConflict,
  PatchPreview,
  ProgrammerState,
  Query,
} from "../bindings";
import {
  ATTRIBUTE_TYPE_VARIANTS,
  FEATURE_GROUP_VARIANTS,
  MERGE_MODE_VARIANTS,
  NOTICE_LEVEL_VARIANTS,
  OUTPUT_HEALTH_VARIANTS,
  PROGRAMMER_VALUE_SOURCE_VARIANTS,
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

/** One DMX output, as the status panel shows it. */
export interface OutputSnapshot {
  /** Which output. */
  readonly id: OutputId;
  /** What the operator calls it. */
  readonly name: string;
  /** Whether frames are reaching the fixtures. */
  readonly health: OutputHealth;
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
   * The profiles **this desk** can embed into a show.
   *
   * Not show content: a show that has embedded one of these owns its copy from
   * then on (S11), and this list is a property of the daemon's build. It is in
   * the snapshot rather than behind a query because a client needs it in order
   * to *offer* the list at all — a brand-new show carries no profiles, so a
   * patch window without this would be a form with an empty menu.
   */
  readonly fixtureLibrary: readonly FixtureType[];
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
    value: readProgrammerValue(field(record, "value"), `${path}.value`),
  };
}

/** The programmer, whole — the third document. */
export function readProgrammerState(value: unknown, path: string): ProgrammerState {
  const record = asRecord(value, path);
  const clearStage = asInteger(field(record, "clearStage"), `${path}.clearStage`);
  if (clearStage !== 0 && clearStage !== 1 && clearStage !== 2) {
    throw new ProtocolFault(`${path}.clearStage`, `one of 0, 1, 2, not ${clearStage}`);
  }
  return {
    selection: asArray(field(record, "selection"), `${path}.selection`).map((id, index) =>
      asInteger(id, `${path}.selection[${index}]`),
    ),
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
    case "ExecutorState":
      return {
        t: "ExecutorState",
        executorId: asInteger(field(record, "executorId"), `${path}.executorId`),
        isActive: asBoolean(field(record, "isActive"), `${path}.isActive`),
        cueIndex: asNullable(field(record, "cueIndex"), `${path}.cueIndex`, asInteger),
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
    default:
      throw new ProtocolFault(`${path}.t`, `an answer this build knows, not ${JSON.stringify(tag)}`);
  }
}

/** One attribute of a profile. */
function readAttributeDef(value: unknown, path: string): AttributeDef {
  const record = asRecord(value, path);
  return {
    attribute: asVariant(field(record, "attribute"), `${path}.attribute`, ATTRIBUTE_TYPE_VARIANTS),
    featureGroup: asVariant(
      field(record, "featureGroup"),
      `${path}.featureGroup`,
      FEATURE_GROUP_VARIANTS,
    ),
    coarseOffset: asInteger(field(record, "coarseOffset"), `${path}.coarseOffset`),
    fineOffset: asNullable(field(record, "fineOffset"), `${path}.fineOffset`, asInteger),
    defaultValue: asInteger(field(record, "defaultValue"), `${path}.defaultValue`),
    mergeMode: asVariant(field(record, "mergeMode"), `${path}.mergeMode`, MERGE_MODE_VARIANTS),
    invert: asBoolean(field(record, "invert"), `${path}.invert`),
    physicalFrom: asNumber(field(record, "physicalFrom"), `${path}.physicalFrom`),
    physicalTo: asNumber(field(record, "physicalTo"), `${path}.physicalTo`),
  };
}

/** One profile this desk can embed. */
function readFixtureType(value: unknown, path: string): FixtureType {
  const record = asRecord(value, path);
  return {
    id: asString(field(record, "id"), `${path}.id`),
    manufacturer: asString(field(record, "manufacturer"), `${path}.manufacturer`),
    name: asString(field(record, "name"), `${path}.name`),
    mode: asString(field(record, "mode"), `${path}.mode`),
    footprint: asInteger(field(record, "footprint"), `${path}.footprint`),
    attributes: asArray(field(record, "attributes"), `${path}.attributes`).map((entry, index) =>
      readAttributeDef(entry, `${path}.attributes[${index}]`),
    ),
  };
}

/** One configured output. */
function readOutputSnapshot(value: unknown, path: string): OutputSnapshot {
  const record = asRecord(value, path);
  return {
    id: asInteger(field(record, "id"), `${path}.id`),
    name: asString(field(record, "name"), `${path}.name`),
    health: asVariant(field(record, "health"), `${path}.health`, OUTPUT_HEALTH_VARIANTS),
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
    fixtureLibrary: asArray(field(record, "fixtureLibrary"), `${path}.fixtureLibrary`).map(
      (entry, index) => readFixtureType(entry, `${path}.fixtureLibrary[${index}]`),
    ),
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
