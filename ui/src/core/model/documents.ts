/**
 * The show and the session, **typed**.
 *
 * The store holds both documents as `JsonValue`, because that is what RFC 6902
 * patches are applied to (`../mirror/mirror.ts`). A view wants entities with
 * names, so this file is the one place the two documents are read as what the
 * daemon serialised them from:
 *
 * - the show is `prism_core::Show` — one object per kind of entity, each keyed
 *   by its number **as a string**, because a JSON object's keys are strings;
 * - the session document is `prism_core::SessionState` — `{ session, views }`,
 *   where `session` is `prism_domain::Session` and `views` the stored layouts.
 *
 * Only the top level is checked. The handshake already refuses a daemon of a
 * different protocol version (`docs/IPC_PROTOCOL.md` §4.2), and the entities
 * themselves are the generated types from `../../bindings`, so a deep walk here
 * would be a second copy of guarantees the connection already makes. What *is*
 * checked is the thing that can actually go wrong in a client — being handed
 * `null` or a document of the other kind — and the answer to that is `null`,
 * never a throw.
 */

import type {
  Executor,
  Fixture,
  FixtureType,
  Group,
  JsonValue,
  Mark,
  Preset,
  Sequence,
  Session,
  View,
} from "../../bindings";
import type { Documents } from "../mirror/mirror";
import type { JsonObject } from "../mirror/patch";
import { isObject } from "../mirror/patch";

/** Entities keyed by their number, written as a string — `"1"`, `"2"`, …. */
export type ById<T> = { readonly [id: string]: T };

/** The show document: everything that is saved in a `.prism` file. */
export interface ShowDocument {
  /** The fixture profiles the patch uses, keyed by type id. */
  readonly fixtureTypes: ById<FixtureType>;
  /** The patch. */
  readonly fixtures: ById<Fixture>;
  readonly groups: ById<Group>;
  readonly presets: ById<Preset>;
  /** Cue lists. */
  readonly sequences: ById<Sequence>;
  /** Faders and their buttons. */
  readonly executors: ById<Executor>;
  /** Positions on stage that fixtures can aim at. */
  readonly marks: ById<Mark>;
}

/** The session document: the operating state every client shares. */
export interface SessionDocument {
  /** Open windows, the command line, the selected executor, the encoder bank, …. */
  readonly session: Session;
  /** The stored layouts the View Selector Bar switches between, keyed by view number. */
  readonly views: ById<View>;
}

const SHOW_KEYS = [
  "fixtureTypes",
  "fixtures",
  "groups",
  "presets",
  "sequences",
  "executors",
] as const;

/** Whether every one of `keys` is an object on `value`. */
function hasObjects(value: JsonObject, keys: readonly string[]): boolean {
  return keys.every((key) => {
    const held = value[key];
    return held !== undefined && isObject(held);
  });
}

const withMarks = new WeakMap<JsonObject, ShowDocument>();

/** Reads the show document, or `null` if `value` is not one. */
export function asShow(value: JsonValue | null): ShowDocument | null {
  if (value === null || !isObject(value) || !hasObjects(value, SHOW_KEYS)) {
    return null;
  }
  const marks = value["marks"];
  if (marks !== undefined && isObject(marks)) {
    // The narrowing ends here: the top level is checked, the entities are the
    // daemon's own serialisation of the generated types (see the module notes).
    return value as unknown as ShowDocument;
  }
  // `marks` arrived later than the rest and an older file may not carry it.
  // Remembered per document, so a selector built on this answers the *same*
  // object until the show changes and React does not re-render for nothing.
  const known = withMarks.get(value);
  if (known !== undefined) {
    return known;
  }
  const shaped = { ...value, marks: {} } as unknown as ShowDocument;
  withMarks.set(value, shaped);
  return shaped;
}

/** Reads the session document, or `null` if `value` is not one. */
export function asSession(value: JsonValue | null): SessionDocument | null {
  if (value === null || !isObject(value) || !hasObjects(value, ["session", "views"])) {
    return null;
  }
  return value as unknown as SessionDocument;
}

/** The show out of a set of documents, or `null` while there is none. */
export function showOf(documents: Documents | null): ShowDocument | null {
  return documents === null ? null : asShow(documents.show);
}

/** The session out of a set of documents, or `null` while there is none. */
export function sessionOf(documents: Documents | null): SessionDocument | null {
  return documents === null ? null : asSession(documents.session);
}

/** The values of a keyed collection, in number order. */
export function listOf<T>(byId: ById<T>): T[] {
  return Object.keys(byId)
    .sort((a, b) => Number(a) - Number(b))
    .flatMap((id) => {
      const entity = byId[id];
      return entity === undefined ? [] : [entity];
    });
}
