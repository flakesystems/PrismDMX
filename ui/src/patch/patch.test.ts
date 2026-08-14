/**
 * The patch readers, held to a daemon's own answers.
 *
 * `ui/tests/fixtures/patch-recording.json` is written by
 * `crates/prismd/tests/ui_patch.rs` off a running `prismd`: nineteen steps of a
 * rig being built, corrected and taken apart, and after each one **the rows and
 * the profiles a fresh client's snapshot says are there**. This file replays the
 * deltas of that script through this interface's own mirror and compares its
 * readers' answers, field by field, with the daemon's.
 *
 * A reader tested against a document the same language built is a test of
 * nothing, which is the trap every session since S23 has found in its own layer.
 */

import { decode } from "@msgpack/msgpack";
import { beforeEach, describe, expect, it } from "vitest";

import type { Delta, JsonValue } from "../bindings";
import { readServerMessage } from "../ipc/protocol";
import type { Snapshot } from "../ipc/protocol";
import { nullSink, setLogSink } from "../log/logger";
import { applyDelta } from "../mirror/mirror";
import type { Documents } from "../mirror/mirror";
import {
  embeddedProfiles,
  footprintOf,
  nextFreeFixtureId,
  patchRows,
  profileLabel,
} from "./patch";

import recordingText from "../../tests/fixtures/patch-recording.json?raw";

interface RecordedRow {
  readonly id: number;
  readonly name: string;
  readonly typeId: string;
  readonly typeName: string;
  readonly universe: number;
  readonly address: number;
  readonly footprint: number;
}

interface RecordedProfile {
  readonly id: string;
  readonly manufacturer: string;
  readonly name: string;
  readonly mode: string;
  readonly footprint: number;
}

interface Recording {
  readonly initialSnapshot: string;
  readonly finalSnapshot: string;
  readonly steps: readonly {
    readonly what: string;
    readonly isQuery: boolean;
    readonly deltas: readonly string[];
    readonly rows: readonly RecordedRow[];
    readonly profiles: readonly RecordedProfile[];
  }[];
}

const recording = JSON.parse(recordingText) as Recording;

/** Bytes out of a base64 payload, the way a browser does it (S23). */
function payload(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/** The snapshot at one end of the recording. */
function snapshotOf(encoded: string): Snapshot {
  const message = readServerMessage(decode(payload(encoded)));
  if (message.t !== "Snapshot") {
    throw new Error("that payload is not a snapshot");
  }
  return message.snapshot;
}

/** One delta out of the recording. */
function deltaOf(encoded: string): Delta {
  const message = readServerMessage(decode(payload(encoded)));
  if (message.t !== "Delta") {
    throw new Error("that payload is not a delta");
  }
  return message.delta;
}

/** The documents, as this interface's mirror holds them at the start. */
function initialDocuments(): Documents {
  const snapshot = snapshotOf(recording.initialSnapshot);
  return {
    show: snapshot.show,
    session: snapshot.session,
    programmer: snapshot.programmer,
  };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the patch, read out of the show document", () => {
  it("says after every step exactly what the daemon says", () => {
    let documents = initialDocuments();
    recording.steps.forEach((step, index) => {
      for (const encoded of step.deltas) {
        documents = applyDelta(documents, deltaOf(encoded));
      }
      expect(
        patchRows(documents.show).map((row) => ({ ...row })),
        `step ${String(index)} (${step.what}): the rows`,
      ).toEqual(step.rows.map((row) => ({ ...row })));
      expect(
        embeddedProfiles(documents.show).map((profile) => ({ ...profile })),
        `step ${String(index)} (${step.what}): the profiles`,
      ).toEqual(step.profiles.map((profile) => ({ ...profile })));
    });

    // And what the deltas built is what a client that never saw one is served.
    const end = snapshotOf(recording.finalSnapshot);
    expect(patchRows(documents.show)).toEqual(patchRows(end.show));
    expect(embeddedProfiles(documents.show)).toEqual(embeddedProfiles(end.show));
  });

  it("orders fixtures by number rather than by the key's text", () => {
    // The document keys them as strings, so 10 sorts before 2 unless somebody
    // has thought about it. The recording reaches 70, which is the case that
    // catches it.
    const rows = patchRows({
      fixtures: {
        "10": { name: "Ten", typeId: "x", universe: 1, address: 1 },
        "2": { name: "Two", typeId: "x", universe: 1, address: 2 },
        "70": { name: "Seventy", typeId: "x", universe: 1, address: 3 },
      },
    });
    expect(rows.map((row) => row.id)).toEqual([2, 10, 70]);
  });

  it("shows the profile's name, and the key when the profile is gone", () => {
    const show: JsonValue = {
      fixtures: {
        "1": { name: "One", typeId: "generic.dimmer", universe: 1, address: 1 },
        "2": { name: "Two", typeId: "vanished", universe: 1, address: 2 },
      },
      fixtureTypes: {
        "generic.dimmer": { name: "Dimmer", manufacturer: "Generic", mode: "1ch", footprint: 1 },
      },
    };
    const rows = patchRows(show);
    expect(rows[0]?.typeName).toBe("Dimmer");
    expect(rows[0]?.footprint).toBe(1);
    // A patched fixture whose profile is missing is a real state — only a
    // hand-edited file makes one, and `ShowIssue::MissingFixtureType` reports
    // it. The key is shown, because the key is what somebody has to go and find.
    expect(rows[1]?.typeName).toBe("vanished");
    expect(rows[1]?.footprint).toBe(0);
  });

  it("answers with nothing for a document that has no patch in it", () => {
    for (const show of [null, {}, { fixtures: 7 }, { fixtures: { "1": 7, x: {} } }]) {
      expect(patchRows(show as JsonValue)).toEqual([]);
    }
    for (const show of [null, {}, { fixtureTypes: 7 }, { fixtureTypes: { a: 7 } }]) {
      expect(embeddedProfiles(show as JsonValue)).toEqual([]);
    }
    expect(footprintOf(null, "anything")).toBe(0);
  });

  it("fills the gaps in the numbering before it goes past the end", () => {
    const rows = (ids: number[]) =>
      ids.map((id) => ({
        id,
        name: "",
        typeId: "",
        typeName: "",
        universe: 1,
        address: 1,
        footprint: 0,
      }));
    expect(nextFreeFixtureId([])).toBe(1);
    expect(nextFreeFixtureId(rows([1, 2, 3]))).toBe(4);
    expect(nextFreeFixtureId(rows([1, 3]))).toBe(2);
    expect(nextFreeFixtureId(rows([2, 3]))).toBe(1);
  });

  it("names a profile the way an operator would choose one", () => {
    expect(
      profileLabel({
        id: "generic.rgbw.par",
        manufacturer: "Generic",
        name: "RGBW PAR",
        mode: "4ch",
        footprint: 4,
      }),
    ).toBe("Generic RGBW PAR · 4ch · 4 ch");
    // A profile that carries almost nothing still has to be choosable, so the
    // key stands in for a name that is not there.
    expect(
      profileLabel({ id: "bare", manufacturer: "", name: "", mode: "", footprint: 2 }),
    ).toBe("bare · 2 ch");
  });

  it("reads the profiles the recorded show carries, with their footprints", () => {
    const start = snapshotOf(recording.initialSnapshot);
    const profiles = embeddedProfiles(start.show);
    expect(profiles.map((profile) => profile.id)).toEqual([
      "generic.dimmer",
      "generic.rgbw.par",
    ]);
    expect(footprintOf(start.show, "generic.rgbw.par")).toBe(4);
    expect(footprintOf(start.show, "generic.movinghead")).toBe(0);
  });
});
