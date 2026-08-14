/**
 * Reading a document without asserting anything about it.
 *
 * The show and the session are held as documents (see `./mirror.ts`), so a view
 * that wants the executor page asks for `/session/executorPage`. A pointer that
 * names nothing answers `null` rather than throwing: a *view* asking for
 * something that is not in the document is an ordinary state — a window open on
 * a fixture that has just been deleted — and not the divergence that
 * {@link import("./patch").MirrorFault} reports when a *delta* does not fit.
 */

import type { JsonValue } from "../bindings";
import { MirrorFault, getAt, isArray, isObject } from "./patch";

/** The value at a pointer, or `null` if the pointer names nothing. */
/**
 * One key, as a JSON Pointer reference token — RFC 6901 §3.
 *
 * **A key is operator data and may contain a `/`.** A fixture type key out of
 * the Open Fixture Library is `manufacturer/fixture/mode` (S44), so a pointer
 * built by pasting one in would name three levels of a document that has one,
 * and every reader of it would answer `null` — a fixture sheet with no
 * attributes, an encoder bar with no banks, a patch row with no footprint.
 *
 * `prism_core::show::escape` is the same three lines at the other end, and
 * `mirror/patch.ts` is what unescapes them on the way in. This is the third
 * side of that and the one a *view* needs.
 */
export function pointerToken(key: string): string {
  return key.replaceAll("~", "~0").replaceAll("/", "~1");
}

export function valueAt(document: JsonValue | null, pointer: string): JsonValue | null {
  if (document === null) {
    return null;
  }
  try {
    return getAt(document, pointer);
  } catch (cause) {
    // A pointer that is not a pointer is a mistake in the *caller* — a typo in
    // a view — and hiding it behind `null` would leave a panel permanently
    // empty with nothing to explain it. Everything else is a statement about
    // the document, which is allowed to have changed.
    if (cause instanceof MirrorFault && cause.detail.kind !== "MalformedPointer") {
      return null;
    }
    throw cause;
  }
}

/** The string at a pointer, or `null` if it is absent or something else. */
export function stringAt(document: JsonValue | null, pointer: string): string | null {
  const value = valueAt(document, pointer);
  return typeof value === "string" ? value : null;
}

/** The number at a pointer, or `null` if it is absent or something else. */
export function numberAt(document: JsonValue | null, pointer: string): number | null {
  const value = valueAt(document, pointer);
  return typeof value === "number" ? value : null;
}

/** The boolean at a pointer, or `null` if it is absent or something else. */
export function booleanAt(document: JsonValue | null, pointer: string): boolean | null {
  const value = valueAt(document, pointer);
  return typeof value === "boolean" ? value : null;
}

/**
 * How many members or elements are at a pointer, or `null` if it names neither
 * an object nor an array.
 */
export function countAt(document: JsonValue | null, pointer: string): number | null {
  const value = valueAt(document, pointer);
  if (value === null) {
    return null;
  }
  if (isArray(value)) {
    return value.length;
  }
  if (isObject(value)) {
    return Object.keys(value).length;
  }
  return null;
}
