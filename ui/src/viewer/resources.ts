/**
 * A fixture's own files — its 3D models and its gobo pictures — fetched from
 * the daemon a part at a time — **S30b**.
 *
 * `Query::FixtureResource` answers half a megabyte at a time, as base64, with
 * the file's full length; this asks again at the offset the last part ended at
 * until it has the whole file. Each file is fetched **once** per device and
 * name however many fixtures of that device are patched: twenty heads of one
 * type share one model and one set of gobos, and the cache is keyed by the
 * device's GUID, which is what makes a show's copy and the library's the same
 * device.
 *
 * A file the desk does not have — a profile from another desk whose archive is
 * not here, a model that is only a primitive — is `null`, and the viewer draws
 * what it can without it. That is the ordinary case, not an error, and nothing
 * is logged about it.
 */

import type { Answer, Query, ResourceKind } from "../bindings";

/** One fetched file. */
export interface Resource {
  /** Its path inside the archive, which says its format. */
  readonly path: string;
  readonly bytes: Uint8Array<ArrayBuffer>;
}

/** Something that asks the daemon a question — `useAsk` in the window. */
export type Asker = (query: Query) => Promise<Answer | null>;

/** The most parts one file may take — 64 MiB at half a megabyte a part. */
const MAX_PARTS = 128;

/** Base64 as bytes. */
export function fromBase64(text: string): Uint8Array<ArrayBuffer> {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/** Fetches one file whole, or `null` when the desk has none. */
export async function fetchResource(
  ask: Asker,
  device: { readonly guid: string; readonly typeId: string },
  kind: ResourceKind,
  name: string,
): Promise<Resource | null> {
  const parts: Uint8Array[] = [];
  let received = 0;
  let total = -1;
  let path = "";
  for (let part = 0; part < MAX_PARTS; part += 1) {
    const answer = await ask({
      t: "FixtureResource",
      fixtureTypeId: device.guid,
      typeId: device.typeId,
      kind,
      name,
      offset: received,
    });
    if (answer === null || answer.t !== "FixtureResource" || answer.path === "" || answer.total === 0) {
      return null;
    }
    if (answer.offset !== received) {
      // An answer to another question, or a file that changed under us.
      return null;
    }
    path = answer.path;
    total = answer.total;
    const chunk = fromBase64(answer.data);
    if (chunk.length === 0) {
      return null;
    }
    parts.push(chunk);
    received += chunk.length;
    if (received >= total) {
      break;
    }
  }
  if (received !== total) {
    return null;
  }
  const bytes = new Uint8Array(total);
  let at = 0;
  for (const chunk of parts) {
    bytes.set(chunk, at);
    at += chunk.length;
  }
  return { path, bytes };
}

/**
 * Files already asked for, by device, kind and name — each one a promise, so
 * twenty heads asking at once cause one fetch.
 */
export class ResourceCache {
  #ask: Asker;
  #files = new Map<string, Promise<Resource | null>>();

  constructor(ask: Asker) {
    this.#ask = ask;
  }

  /** The file, fetched once. */
  get(device: { readonly guid: string; readonly typeId: string }, kind: ResourceKind, name: string): Promise<Resource | null> {
    const key = `${device.guid !== "" ? device.guid : device.typeId}|${kind}|${name}`;
    let held = this.#files.get(key);
    if (held === undefined) {
      held = fetchResource(this.#ask, device, kind, name).catch(() => null);
      this.#files.set(key, held);
    }
    return held;
  }

  /** How many files have been asked for. Read by the tests. */
  get size(): number {
    return this.#files.size;
  }
}
