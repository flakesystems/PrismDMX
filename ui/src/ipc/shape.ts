/**
 * Turning `unknown` into a type, one field at a time.
 *
 * A decoded MessagePack payload is `unknown`, and `CLAUDE.md` says the way out
 * of that is a type guard rather than `any` — and `as` is not a guard, it is a
 * claim. Everything in this module is therefore a *reader*: it is handed a
 * value and a path, and it either answers with the type it promised or throws a
 * [`ProtocolFault`] naming the field that was wrong.
 *
 * The path is not decoration. A daemon and an interface of different builds
 * disagree about one field, and "the message was malformed" sends whoever reads
 * the log to the wrong place.
 */

import type { JsonValue } from "../bindings";

/** The nesting limit, matching `docs/IPC_PROTOCOL.md` §3 and `prism-ipc`. */
export const MAX_NESTING_DEPTH = 128;

/** A message that is not the shape this build understands. */
export class ProtocolFault extends Error {
  /** Where in the message the fault is, as a dotted path. */
  readonly path: string;

  constructor(path: string, expected: string) {
    super(`${path}: expected ${expected}`);
    this.name = "ProtocolFault";
    this.path = path;
  }
}

/** What a value is, for an error message a person has to read. */
export function describe(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "an array";
  if (value instanceof Uint8Array) return "a byte string";
  return typeof value;
}

/**
 * One payload, in the form a `WebSocket` will take.
 *
 * `Uint8Array` is generic over its buffer since TypeScript 5.7, and
 * `WebSocket.send` wants a view over an `ArrayBuffer` specifically — a
 * `SharedArrayBuffer` cannot be transferred. Saying so in the type is what
 * keeps {@link overArrayBuffer}'s branch honest instead of asserting past it.
 */
export type Payload = Uint8Array<ArrayBuffer>;

/**
 * The same bytes over an `ArrayBuffer`.
 *
 * A view when the buffer already is one — no copy on the path a command takes
 * — and a copy when it is shared, which nothing in this interface produces and
 * which the type system is right to insist on.
 */
export function overArrayBuffer(payload: Uint8Array): Payload {
  const { buffer } = payload;
  if (buffer instanceof ArrayBuffer) {
    return new Uint8Array(buffer, payload.byteOffset, payload.byteLength);
  }
  const copy = new Uint8Array(payload.byteLength);
  copy.set(payload);
  return copy;
}

/** A plain object, which is what MessagePack maps decode to. */
export function asRecord(value: unknown, path: string): Record<string, unknown> {
  // A plain object or a null-prototype one, and nothing else: a `Map`, a
  // `Date` or a byte string is an object to `typeof` and is not a map from the
  // daemon.
  const prototype: unknown =
    typeof value === "object" && value !== null ? Object.getPrototypeOf(value) : undefined;
  if (
    typeof value !== "object" ||
    value === null ||
    (prototype !== null && prototype !== Object.prototype)
  ) {
    throw new ProtocolFault(path, `an object, not ${describe(value)}`);
  }
  const record: Record<string, unknown> = {};
  for (const [key, member] of Object.entries(value)) {
    record[key] = member;
  }
  return record;
}

/** The member at `key`, absent members reading as `undefined`. */
export function field(record: Record<string, unknown>, key: string): unknown {
  return Object.hasOwn(record, key) ? record[key] : undefined;
}

/** A string. */
export function asString(value: unknown, path: string): string {
  if (typeof value !== "string") {
    throw new ProtocolFault(path, `a string, not ${describe(value)}`);
  }
  return value;
}

/**
 * A finite number.
 *
 * Finite because the domain is (S1): MessagePack carries NaN and infinity
 * faithfully, and a client comparing a NaN tick rate against itself would
 * redraw its status panel for ever.
 */
export function asNumber(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new ProtocolFault(path, `a finite number, not ${describe(value)}`);
  }
  return value;
}

/** A whole number, which is what every identifier and counter on the wire is. */
export function asInteger(value: unknown, path: string): number {
  const number = asNumber(value, path);
  if (!Number.isInteger(number)) {
    throw new ProtocolFault(path, `a whole number, not ${number}`);
  }
  return number;
}

/** A boolean. */
export function asBoolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") {
    throw new ProtocolFault(path, `a boolean, not ${describe(value)}`);
  }
  return value;
}

/** An array, its elements still `unknown`. */
export function asArray(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new ProtocolFault(path, `an array, not ${describe(value)}`);
  }
  return value;
}

/** A byte string, which is how telemetry travels inside the envelope (§7). */
export function asBytes(value: unknown, path: string): Payload {
  if (!(value instanceof Uint8Array)) {
    throw new ProtocolFault(path, `a byte string, not ${describe(value)}`);
  }
  return overArrayBuffer(value);
}

/** `null`, or whatever `read` makes of the value. */
export function asNullable<T>(
  value: unknown,
  path: string,
  read: (value: unknown, path: string) => T,
): T | null {
  return value === null ? null : read(value, path);
}

/**
 * One of a closed set of strings.
 *
 * The sets come from `ui/src/bindings/variants.ts`, which `prism-domain`
 * generates out of the very unions it generates the types from — so this is a
 * check against the daemon's own vocabulary rather than against a list somebody
 * kept in step by hand.
 */
export function asVariant<T extends string>(
  value: unknown,
  path: string,
  variants: readonly T[],
): T {
  if (typeof value === "string") {
    const found = variants.find((variant) => variant === value);
    if (found !== undefined) {
      return found;
    }
  }
  throw new ProtocolFault(path, `one of ${variants.join(", ")}, not ${describe(value)}`);
}

/**
 * A schema-free value: `null`, a boolean, a finite number, a string, an array
 * of them, or an object of them — and nothing else.
 *
 * The depth limit is [`MAX_NESTING_DEPTH`], for the reason `prism-ipc`'s
 * `scan.rs` has one: `JsonValue` is recursive, MessagePack has no limit of its
 * own, and a few kilobytes can nest a hundred thousand deep. In the browser
 * that costs a `RangeError` rather than a process, but a `RangeError` thrown
 * inside a state update is a UI that stops rather than one that reconnects.
 */
export function asJsonValue(value: unknown, path: string, depth = 0): JsonValue {
  if (depth > MAX_NESTING_DEPTH) {
    throw new ProtocolFault(path, `a value nested at most ${MAX_NESTING_DEPTH} deep`);
  }
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return value;
  }
  if (typeof value === "number") {
    return asNumber(value, path);
  }
  if (Array.isArray(value)) {
    return value.map((element, index) => asJsonValue(element, `${path}[${index}]`, depth + 1));
  }
  const record = asRecord(value, path);
  const members: Record<string, JsonValue> = {};
  for (const [key, member] of Object.entries(record)) {
    members[key] = asJsonValue(member, `${path}.${key}`, depth + 1);
  }
  return members;
}
