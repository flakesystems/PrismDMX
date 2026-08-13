/**
 * RFC 6902 over a `JsonValue`, immutably — the browser's half of
 * `prism_core::JsonMirror`.
 *
 * `docs/IPC_PROTOCOL.md` §6 promises that *a client that has applied every
 * delta since its snapshot holds state identical to the daemon's*. That is a
 * claim about two pieces of code, a generator in Rust and an applier here, and
 * it is only checked by having both. This is the applier; the check is
 * `recording.test.ts`, against a delta stream a running daemon actually sent.
 *
 * Every rule below is `prism_core::mirror`'s rule, deliberately: the two must
 * agree about `-` at the end of an array, about `01` not being an index, about
 * a `move` into its own source, and about `replace` needing something to
 * replace. Where they cannot agree is written down at the point it happens.
 *
 * # Immutability, and why it is not a copy
 *
 * `CLAUDE.md` asks for immutable updates so re-renders stay cheap and
 * deterministic. Every operation here rebuilds only the containers along the
 * pointer and shares everything else, so a patch that changes one fixture
 * leaves every other fixture's object identical — `===` identical, which is
 * what lets a selector decide it has nothing to redraw.
 *
 * # A failed operation is a bug, and says so
 *
 * Clients apply deltas without validating them, because the daemon has already
 * decided. An operation that does not fit therefore means the two have already
 * diverged, and the honest answer is to say which one failed and re-snapshot —
 * not to guess. {@link applyOps} stops at the first failure and reports it;
 * what to do about it belongs to the store, and what it does is reconnect.
 */

import type { JsonPatchOp, JsonValue } from "../bindings";

/** Why an operation could not be applied. */
export type MirrorFaultKind =
  | { readonly kind: "MalformedPointer"; readonly path: string }
  | { readonly kind: "NoSuchPath"; readonly path: string }
  | { readonly kind: "NotAnIndex"; readonly path: string; readonly token: string }
  | { readonly kind: "IndexOutOfRange"; readonly path: string; readonly index: number }
  | { readonly kind: "TestFailed"; readonly path: string }
  | { readonly kind: "MoveIntoSelf"; readonly from: string; readonly path: string };

/** An operation that does not fit the document. */
export class MirrorFault extends Error {
  /** Which failure it was, so a caller can match rather than parse. */
  readonly detail: MirrorFaultKind;

  constructor(detail: MirrorFaultKind) {
    super(describe(detail));
    this.name = "MirrorFault";
    this.detail = detail;
  }
}

/** What to show a person. */
function describe(detail: MirrorFaultKind): string {
  switch (detail.kind) {
    case "MalformedPointer":
      return `${JSON.stringify(detail.path)} is not a JSON Pointer`;
    case "NoSuchPath":
      return `nothing at ${JSON.stringify(detail.path)}`;
    case "NotAnIndex":
      return `${JSON.stringify(detail.path)} indexes an array with ${JSON.stringify(detail.token)}`;
    case "IndexOutOfRange":
      return `${JSON.stringify(detail.path)} indexes element ${detail.index} past the end`;
    case "TestFailed":
      return `the value at ${JSON.stringify(detail.path)} is not the one tested`;
    case "MoveIntoSelf":
      return `${JSON.stringify(detail.path)} is inside ${JSON.stringify(detail.from)}, so the move is undefined`;
  }
}

/** A JSON object, as distinct from an array or a leaf. */
export type JsonObject = { [key in string]: JsonValue };

/** Whether a value is an object rather than an array or a leaf. */
export function isObject(value: JsonValue): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Whether a value is an array. */
export function isArray(value: JsonValue): value is JsonValue[] {
  return Array.isArray(value);
}

/**
 * Splits a JSON Pointer into its unescaped reference tokens (RFC 6901 §3).
 *
 * `~1` is unescaped before `~0`, or an escaped `~1` would be turned into a
 * separator.
 */
export function tokens(path: string): string[] {
  if (path === "") {
    return [];
  }
  if (!path.startsWith("/")) {
    throw new MirrorFault({ kind: "MalformedPointer", path });
  }
  return path
    .split("/")
    .slice(1)
    .map((token) => token.replaceAll("~1", "/").replaceAll("~0", "~"));
}

/**
 * A reference token used as an array index.
 *
 * RFC 6901: an index is either `0` or digits not starting with zero, so `01`
 * and `+1` are not indexes.
 */
export function indexOf(path: string, token: string): number {
  if (!/^(0|[1-9][0-9]*)$/.test(token)) {
    throw new MirrorFault({ kind: "NotAnIndex", path, token });
  }
  return Number(token);
}

/** Whether `path` names something inside `from`. */
export function isInside(from: string, path: string): boolean {
  return path === from || path.startsWith(`${from}/`);
}

/**
 * Structural equality over two documents.
 *
 * Used by the `test` operation and by anything that wants to know whether a
 * patch changed the document. Object key order is not part of the comparison —
 * the daemon's documents are key-ordered maps, and MessagePack preserves that
 * order, but equality of two documents is not a claim about their encoding.
 *
 * **Where this cannot match Rust:** `prism_domain::JsonValue` distinguishes
 * `Int(1)` from `Float(1.0)` and JavaScript has one number type, so a `test`
 * against `1.0` where the document holds `1` succeeds here and fails there.
 * `prism-core` generates no `test` operations, so the difference is
 * unreachable through the protocol; it is written down rather than hidden.
 */
export function equals(left: JsonValue, right: JsonValue): boolean {
  if (left === right) {
    return true;
  }
  if (isArray(left) && isArray(right)) {
    return (
      left.length === right.length &&
      left.every((item, index) => {
        const other = right[index];
        return other !== undefined && equals(item, other);
      })
    );
  }
  if (isObject(left) && isObject(right)) {
    const keys = Object.keys(left);
    if (keys.length !== Object.keys(right).length) {
      return false;
    }
    return keys.every((key) => {
      const mine = left[key];
      const theirs = right[key];
      return mine !== undefined && theirs !== undefined && equals(mine, theirs);
    });
  }
  return false;
}

/**
 * The value at a pointer.
 *
 * @throws {MirrorFault} if the pointer is malformed or names nothing.
 */
export function getAt(root: JsonValue, path: string): JsonValue {
  let node = root;
  for (const token of tokens(path)) {
    if (isObject(node)) {
      const member = node[token];
      if (member === undefined) {
        throw new MirrorFault({ kind: "NoSuchPath", path });
      }
      node = member;
    } else if (isArray(node)) {
      const element = node[indexOf(path, token)];
      if (element === undefined) {
        throw new MirrorFault({ kind: "NoSuchPath", path });
      }
      node = element;
    } else {
      throw new MirrorFault({ kind: "NoSuchPath", path });
    }
  }
  return node;
}

/**
 * Rebuilds the containers along `parents` and hands the innermost one to
 * `change`, which answers with its replacement.
 *
 * This is where the structural sharing happens: every container not on the
 * path is carried over by reference.
 */
function through(
  node: JsonValue,
  path: string,
  parents: readonly string[],
  change: (parent: JsonValue) => JsonValue,
): JsonValue {
  const head = parents[0];
  if (head === undefined) {
    return change(node);
  }
  const rest = parents.slice(1);
  if (isObject(node)) {
    const child = node[head];
    if (child === undefined) {
      throw new MirrorFault({ kind: "NoSuchPath", path });
    }
    return { ...node, [head]: through(child, path, rest, change) };
  }
  if (isArray(node)) {
    const position = indexOf(path, head);
    const child = node[position];
    if (child === undefined) {
      throw new MirrorFault({ kind: "NoSuchPath", path });
    }
    const copy = node.slice();
    copy[position] = through(child, path, rest, change);
    return copy;
  }
  throw new MirrorFault({ kind: "NoSuchPath", path });
}

/** RFC 6902 `add`: insert into an object or an array, or replace the document. */
function add(root: JsonValue, path: string, value: JsonValue): JsonValue {
  const reference = tokens(path);
  const last = reference.at(-1);
  if (last === undefined) {
    // The empty pointer names the whole document.
    return value;
  }
  const parents = reference.slice(0, -1);
  return through(root, path, parents, (parent) => {
    if (isObject(parent)) {
      return { ...parent, [last]: value };
    }
    if (isArray(parent)) {
      const position = last === "-" ? parent.length : indexOf(path, last);
      if (position > parent.length) {
        throw new MirrorFault({ kind: "IndexOutOfRange", path, index: position });
      }
      return [...parent.slice(0, position), value, ...parent.slice(position)];
    }
    throw new MirrorFault({ kind: "NoSuchPath", path });
  });
}

/** RFC 6902 `remove`, answering with the new document and what left it. */
function remove(root: JsonValue, path: string): { root: JsonValue; removed: JsonValue } {
  const reference = tokens(path);
  const last = reference.at(-1);
  if (last === undefined) {
    throw new MirrorFault({ kind: "NoSuchPath", path });
  }
  const parents = reference.slice(0, -1);
  let removed: JsonValue = null;
  const next = through(root, path, parents, (parent) => {
    if (isObject(parent)) {
      const member = parent[last];
      if (member === undefined) {
        throw new MirrorFault({ kind: "NoSuchPath", path });
      }
      removed = member;
      const copy: JsonObject = {};
      for (const [key, value] of Object.entries(parent)) {
        if (key !== last) {
          copy[key] = value;
        }
      }
      return copy;
    }
    if (isArray(parent)) {
      const position = indexOf(path, last);
      const element = parent[position];
      if (element === undefined) {
        throw new MirrorFault({ kind: "IndexOutOfRange", path, index: position });
      }
      removed = element;
      return [...parent.slice(0, position), ...parent.slice(position + 1)];
    }
    throw new MirrorFault({ kind: "NoSuchPath", path });
  });
  return { root: next, removed };
}

/**
 * Applies one RFC 6902 operation, answering with the new document.
 *
 * @throws {MirrorFault} if the operation does not fit.
 */
export function applyOp(root: JsonValue, op: JsonPatchOp): JsonValue {
  switch (op.op) {
    case "add":
      return add(root, op.path, op.value);
    case "remove":
      return remove(root, op.path).root;
    case "replace":
      // RFC 6902: replace is only defined where something already is.
      getAt(root, op.path);
      return add(root, op.path, op.value);
    case "move": {
      if (isInside(op.from, op.path)) {
        throw new MirrorFault({ kind: "MoveIntoSelf", from: op.from, path: op.path });
      }
      const taken = remove(root, op.from);
      return add(taken.root, op.path, taken.removed);
    }
    case "copy":
      return add(root, op.path, getAt(root, op.from));
    case "test":
      if (!equals(getAt(root, op.path), op.value)) {
        throw new MirrorFault({ kind: "TestFailed", path: op.path });
      }
      return root;
  }
}

/**
 * Applies operations in order, stopping at the first that does not fit.
 *
 * @throws {MirrorFault} for the operation that failed. The operations before it
 * are in the document it threw from, not in the one that comes back — which is
 * `prism_core`'s behaviour too, and for the same reason: an all-or-nothing
 * batch would need a copy of the whole show per delta to protect a case in
 * which the mirror is already wrong.
 */
export function applyOps(root: JsonValue, ops: readonly JsonPatchOp[]): JsonValue {
  let document = root;
  for (const op of ops) {
    document = applyOp(document, op);
  }
  return document;
}
