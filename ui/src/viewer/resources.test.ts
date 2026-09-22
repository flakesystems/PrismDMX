/**
 * A fixture's files, fetched from the daemon — **S30b**, held to the daemon's
 * bytes (B55): the three questions `crates/prismd/tests/ui_viewer.rs` asked a
 * running `prismd` are encoded here by `encodeClientMessage` and compared with
 * what that client sent, and the answers are read by `decodeServerMessage`.
 *
 * The parts-and-offsets logic is tested on answers built here, because a file
 * big enough to take two parts has no place in a committed recording; the
 * shape of each part is the recorded one.
 */

import { describe, expect, it } from "vitest";

import type { Answer, Query } from "../bindings";
import { encodeClientMessage } from "../ipc/codec";
import { bytesOf, messageOf, viewerRecording } from "../testing/viewer-recording";
import type { Asker } from "./resources";
import { ResourceCache, fetchResource, fromBase64 } from "./resources";

/** The head's GDTF GUID and profile key, as the recording asked with them. */
const HEAD = { guid: "5A1E0000-0000-4000-8000-0000000030AA", typeId: "prism-test/viewer-head/standard" };

/** The recorded answer to the recorded question `index`. */
function recordedAnswer(index: number): Answer {
  const exchange = viewerRecording.resources[index];
  if (exchange === undefined) {
    throw new Error(`the recording has no question ${String(index)}`);
  }
  const message = messageOf(exchange.answer);
  if (message.t !== "Answer") {
    throw new Error(`a recorded answer is ${message.t}`);
  }
  return message.answer;
}

/** An asker that answers every question with the recorded answer to `index`. */
function answering(index: number, asked: Query[] = []): Asker {
  return (query) => {
    asked.push(query);
    return Promise.resolve(recordedAnswer(index));
  };
}

describe("the recorded questions", () => {
  it("are the ones this client sends", () => {
    const names = ["gobo-dots", "gobo-star", "../description"];
    viewerRecording.resources.forEach((exchange, index) => {
      const payload = encodeClientMessage({
        t: "Query",
        seq: 1000 + index,
        query: {
          t: "FixtureResource",
          fixtureTypeId: HEAD.guid,
          typeId: HEAD.typeId,
          kind: "Wheel",
          name: names[index] ?? "",
          offset: 0,
        },
      });
      expect(Array.from(payload)).toEqual(Array.from(bytesOf(exchange.client)));
    });
  });

  it("are answered with the picture, and with nothing for the other two", () => {
    const picture = recordedAnswer(0);
    expect(picture.t).toBe("FixtureResource");
    if (picture.t !== "FixtureResource") {
      return;
    }
    expect(picture.kind).toBe("Wheel");
    expect(picture.name).toBe("gobo-dots");
    expect(picture.path).toBe("wheels/gobo-dots.png");
    expect(picture.offset).toBe(0);
    const bytes = fromBase64(picture.data);
    expect(bytes.length).toBe(picture.total);
    // A PNG, which is what the viewer hands the browser to decode.
    expect(Array.from(bytes.slice(0, 8))).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

    for (const index of [1, 2]) {
      const nothing = recordedAnswer(index);
      expect(nothing).toMatchObject({ t: "FixtureResource", path: "", total: 0, data: "" });
    }
  });
});

describe("fetchResource", () => {
  it("fetches the recorded picture whole in one part", async () => {
    const asked: Query[] = [];
    const file = await fetchResource(answering(0, asked), HEAD, "Wheel", "gobo-dots");
    expect(file?.path).toBe("wheels/gobo-dots.png");
    const recorded = recordedAnswer(0);
    expect(file?.bytes.length).toBe(recorded.t === "FixtureResource" ? recorded.total : -1);
    expect(asked).toEqual([
      { t: "FixtureResource", fixtureTypeId: HEAD.guid, typeId: HEAD.typeId, kind: "Wheel", name: "gobo-dots", offset: 0 },
    ]);
  });

  it("is null for a file the desk has not got, without asking twice", async () => {
    const asked: Query[] = [];
    expect(await fetchResource(answering(1, asked), HEAD, "Wheel", "gobo-star")).toBeNull();
    expect(asked).toHaveLength(1);
  });

  it("is null when the daemon does not answer, or answers something else", async () => {
    expect(await fetchResource(() => Promise.resolve(null), HEAD, "Model", "head")).toBeNull();
    expect(
      await fetchResource(() => Promise.resolve({ t: "PatchConflicts", conflicts: [] } as unknown as Answer), HEAD, "Model", "head"),
    ).toBeNull();
  });

  it("asks again at the offset the last part ended at, and joins the parts", async () => {
    // Three parts of a nine-byte file, shaped as the daemon shapes one.
    const whole = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    const asked: number[] = [];
    const ask: Asker = (query) => {
      if (query.t !== "FixtureResource") {
        return Promise.resolve(null);
      }
      asked.push(query.offset);
      const part = whole.slice(query.offset, query.offset + 4);
      return Promise.resolve({
        t: "FixtureResource",
        kind: query.kind,
        name: query.name,
        path: "models/gltf/head.glb",
        offset: query.offset,
        total: whole.length,
        data: btoa(String.fromCharCode(...part)),
      });
    };
    const file = await fetchResource(ask, HEAD, "Model", "head");
    expect(asked).toEqual([0, 4, 8]);
    expect(file?.path).toBe("models/gltf/head.glb");
    expect(Array.from(file?.bytes ?? [])).toEqual(whole);
  });

  it("gives up on a part that is not the one it asked for, or an empty one", async () => {
    const wrongOffset: Asker = () =>
      Promise.resolve({ t: "FixtureResource", kind: "Model", name: "head", path: "a", offset: 3, total: 9, data: "AAAA" });
    expect(await fetchResource(wrongOffset, HEAD, "Model", "head")).toBeNull();
    const empty: Asker = () =>
      Promise.resolve({ t: "FixtureResource", kind: "Model", name: "head", path: "a", offset: 0, total: 9, data: "" });
    expect(await fetchResource(empty, HEAD, "Model", "head")).toBeNull();
  });
});

describe("ResourceCache", () => {
  it("fetches one file once however many fixtures ask, and keeps a failure as null", async () => {
    let asked = 0;
    const cache = new ResourceCache((query) => {
      asked += 1;
      return answering(0)(query);
    });
    const [first, second] = await Promise.all([
      cache.get(HEAD, "Wheel", "gobo-dots"),
      cache.get(HEAD, "Wheel", "gobo-dots"),
    ]);
    expect(first).toBe(second);
    expect(asked).toBe(1);
    expect(cache.size).toBe(1);

    const failing = new ResourceCache(() => Promise.reject(new Error("gone")));
    expect(await failing.get(HEAD, "Model", "head")).toBeNull();
  });

  it("keys a device with no GUID by its profile", async () => {
    const cache = new ResourceCache(answering(0));
    await cache.get({ guid: "", typeId: "a" }, "Wheel", "gobo-dots");
    await cache.get({ guid: "", typeId: "b" }, "Wheel", "gobo-dots");
    expect(cache.size).toBe(2);
  });
});
