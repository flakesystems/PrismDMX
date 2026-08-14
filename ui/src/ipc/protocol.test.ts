/**
 * The readers: `unknown` in, a message or a named fault out.
 *
 * The point of every case here is that the fault says **where**. A daemon and
 * an interface of different builds disagree about one field, and "the message
 * was malformed" sends whoever reads the log to the wrong place.
 */

import { describe, expect, it } from "vitest";

import { snapshot as aSnapshot, emptyProgrammer } from "../testing/fake-daemon";
import {
  CLIENT_KINDS,
  PROTOCOL_VERSION,
  REJECT_REASONS,
  closesTheConnection,
  describeServerMessage,
  hello,
  readAnswer,
  readDelta,
  readProgrammerState,
  readServerMessage,
  readSnapshot,
} from "./protocol";
import { ProtocolFault } from "./shape";

/** The path a reader complained about. */
function faultPath(run: () => unknown): string {
  try {
    run();
  } catch (cause) {
    if (cause instanceof ProtocolFault) {
      return cause.path;
    }
    throw cause;
  }
  throw new Error("the reader was expected to refuse this and did not");
}

describe("the envelope", () => {
  it("reads every message the daemon may send", () => {
    expect(readServerMessage({ t: "Ack", seq: 4 })).toEqual({ t: "Ack", seq: 4 });
    expect(
      readServerMessage({ t: "Reject", seq: null, reason: "ShuttingDown", message: "bye" }),
    ).toEqual({ t: "Reject", seq: null, reason: "ShuttingDown", message: "bye" });
    expect(
      readServerMessage({ t: "Telemetry", data: new Uint8Array([1, 2, 3]) }),
    ).toEqual({ t: "Telemetry", data: new Uint8Array([1, 2, 3]) });
    expect(
      readServerMessage({ t: "Delta", delta: { t: "DirtyFlag", unsavedChanges: true } }),
    ).toEqual({ t: "Delta", delta: { t: "DirtyFlag", unsavedChanges: true } });
  });

  it("refuses a message it does not know, naming the tag", () => {
    expect(faultPath(() => readServerMessage({ t: "Wibble" }))).toBe("ServerMessage.t");
    expect(faultPath(() => readServerMessage({ seq: 1 }))).toBe("ServerMessage.t");
    expect(faultPath(() => readServerMessage(null))).toBe("ServerMessage");
    expect(faultPath(() => readServerMessage([1, 2]))).toBe("ServerMessage");
    expect(faultPath(() => readServerMessage(new Map()))).toBe("ServerMessage");
  });

  it("names the field that was wrong, not the message", () => {
    expect(faultPath(() => readServerMessage({ t: "Ack", seq: "four" }))).toBe(
      "ServerMessage.seq",
    );
    expect(faultPath(() => readServerMessage({ t: "Ack", seq: 1.5 }))).toBe("ServerMessage.seq");
    expect(
      faultPath(() => readServerMessage({ t: "Reject", seq: 1, reason: "Nope", message: "" })),
    ).toBe("ServerMessage.reason");
    expect(
      faultPath(() => readServerMessage({ t: "Telemetry", data: [1, 2, 3] })),
    ).toBe("ServerMessage.data");
  });

  it("knows which refusals end a connection", () => {
    // The two an ordinary client meets leave it open.
    expect(closesTheConnection("CommandRefused")).toBe(false);
    expect(closesTheConnection("Undecodable")).toBe(false);
    for (const reason of REJECT_REASONS) {
      if (reason !== "CommandRefused" && reason !== "Undecodable") {
        expect(closesTheConnection(reason), reason).toBe(true);
      }
    }
  });

  it("names each message for an error a person reads", () => {
    expect(describeServerMessage({ t: "Snapshot", snapshot: aSnapshot() })).toBe("a snapshot");
    expect(
      describeServerMessage({ t: "Delta", delta: { t: "DirtyFlag", unsavedChanges: false } }),
    ).toBe("a delta");
    expect(describeServerMessage({ t: "Telemetry", data: new Uint8Array() })).toBe(
      "a telemetry frame",
    );
    expect(describeServerMessage({ t: "Ack", seq: 0 })).toBe("an acknowledgement");
    expect(
      describeServerMessage({ t: "Reject", seq: null, reason: "OutOfOrder", message: "" }),
    ).toBe("a rejection");
  });

  it("builds a hello for this build", () => {
    expect(hello("Desktop")).toEqual({
      protocolVersion: PROTOCOL_VERSION,
      clientKind: "Desktop",
      token: null,
    });
    expect(hello("WebRemote", "t").token).toBe("t");
    expect(CLIENT_KINDS).toEqual(["Desktop", "WebRemote", "Other"]);
  });
});

describe("deltas", () => {
  it("reads all seven", () => {
    const deltas = [
      { t: "ShowPatch", ops: [{ op: "add", path: "/a", value: 1 }] },
      { t: "SessionPatch", ops: [] },
      { t: "ProgrammerChanged", state: emptyProgrammer() },
      { t: "ExecutorState", executorId: 2, isActive: true, cueIndex: null },
      { t: "OutputHealth", outputId: 1, health: "Degraded" },
      { t: "DirtyFlag", unsavedChanges: false },
      { t: "Notice", level: "Warn", message: "careful" },
    ];
    for (const delta of deltas) {
      expect(readDelta(delta, "Delta")).toEqual(delta);
    }
  });

  it("reads every RFC 6902 operation", () => {
    const ops = [
      { op: "add", path: "/a", value: null },
      { op: "remove", path: "/a" },
      { op: "replace", path: "/a", value: [1, 2] },
      { op: "move", from: "/a", path: "/b" },
      { op: "copy", from: "/a", path: "/b" },
      { op: "test", path: "/a", value: { b: true } },
    ];
    expect(readDelta({ t: "ShowPatch", ops }, "Delta")).toEqual({ t: "ShowPatch", ops });
  });

  it("refuses an operation it does not know", () => {
    expect(
      faultPath(() => readDelta({ t: "ShowPatch", ops: [{ op: "increment", path: "/a" }] }, "d")),
    ).toBe("d.ops[0].op");
    expect(faultPath(() => readDelta({ t: "ShowPatch", ops: [{ path: "/a" }] }, "d"))).toBe(
      "d.ops[0].op",
    );
    expect(faultPath(() => readDelta({ t: "ShowPatch", ops: "all of them" }, "d"))).toBe("d.ops");
  });

  it("refuses a delta it does not know", () => {
    expect(faultPath(() => readDelta({ t: "Everything" }, "d"))).toBe("d.t");
  });
});

describe("the programmer", () => {
  it("reads a full one", () => {
    const state = {
      selection: [1, 2],
      activeFeatureGroup: "Color",
      values: [
        { fixture: 1, attribute: "Red", value: { value: 65535, source: "Manual", presetRef: null } },
        { fixture: 2, attribute: "Dimmer", value: { value: 0, source: "Preset", presetRef: 4 } },
      ],
      clearStage: 2,
    };
    expect(readProgrammerState(state, "p")).toEqual(state);
  });

  it("refuses a value the daemon's vocabulary does not have", () => {
    const state = { ...emptyProgrammer(), activeFeatureGroup: "Smell" };
    expect(faultPath(() => readProgrammerState(state, "p"))).toBe("p.activeFeatureGroup");

    expect(
      faultPath(() =>
        readProgrammerState(
          {
            ...emptyProgrammer(),
            values: [
              { fixture: 1, attribute: "Smell", value: { value: 1, source: "Manual", presetRef: null } },
            ],
          },
          "p",
        ),
      ),
    ).toBe("p.values[0].attribute");

    expect(
      faultPath(() =>
        readProgrammerState(
          {
            ...emptyProgrammer(),
            values: [
              { fixture: 1, attribute: "Red", value: { value: 1, source: "Guessed", presetRef: null } },
            ],
          },
          "p",
        ),
      ),
    ).toBe("p.values[0].value.source");
  });

  it("refuses a clear stage that is not one of the three", () => {
    expect(faultPath(() => readProgrammerState({ ...emptyProgrammer(), clearStage: 3 }, "p"))).toBe(
      "p.clearStage",
    );
  });
});

describe("the snapshot", () => {
  it("reads the three documents, the outputs and the health", () => {
    const snapshot = aSnapshot();
    expect(readSnapshot(snapshot, "s")).toEqual(snapshot);
  });

  it("names the document that was wrong", () => {
    expect(faultPath(() => readSnapshot({ ...aSnapshot(), show: undefined }, "s"))).toBe("s.show");
    expect(
      faultPath(() => readSnapshot({ ...aSnapshot(), programmer: { selection: [] } }, "s")),
    ).toBe("s.programmer.clearStage");
    expect(
      faultPath(() =>
        readSnapshot(
          { ...aSnapshot(), programmer: { ...emptyProgrammer(), activeFeatureGroup: undefined } },
          "s",
        ),
      ),
    ).toBe("s.programmer.activeFeatureGroup");
    expect(
      faultPath(() =>
        readSnapshot({ ...aSnapshot(), outputs: [{ id: 1, name: "Mock", health: "Fine" }] }, "s"),
      ),
    ).toBe("s.outputs[0].health");
    expect(
      faultPath(() =>
        readSnapshot({ ...aSnapshot(), health: { ...aSnapshot().health, tickHz: "44" } }, "s"),
      ),
    ).toBe("s.health.tickHz");
  });
});

/**
 * The third shape, added in S27: a question changes nothing and its answer is
 * a **fact the daemon computed** — which is exactly why it is decoded as
 * strictly as a delta rather than trusted.
 */
describe("an answer", () => {
  it("reads the two the daemon can give", () => {
    const conflicts = { t: "PatchConflicts", conflicts: [] };
    expect(readAnswer(conflicts, "a")).toEqual(conflicts);

    const preview = {
      t: "PatchPreview",
      preview: {
        accepted: true,
        refusal: null,
        footprint: 4,
        lastAddress: 33,
        conflicts: [{ universe: 1, from: 32, to: 33, first: 6, second: 7 }],
      },
    };
    expect(readAnswer(preview, "a")).toEqual(preview);
  });

  it("names the field that was not what it should be", () => {
    expect(faultPath(() => readAnswer({ t: "Elsewhere" }, "a"))).toBe("a.t");
    expect(faultPath(() => readAnswer({ t: "PatchConflicts" }, "a"))).toBe("a.conflicts");
    expect(
      faultPath(() =>
        readAnswer({ t: "PatchConflicts", conflicts: [{ universe: 1, from: 1 }] }, "a"),
      ),
    ).toBe("a.conflicts[0].to");
    expect(
      faultPath(() =>
        readAnswer(
          {
            t: "PatchPreview",
            preview: {
              accepted: "yes",
              refusal: null,
              footprint: 1,
              lastAddress: null,
              conflicts: [],
            },
          },
          "a",
        ),
      ),
    ).toBe("a.preview.accepted");
  });

  it("arrives inside a server message, addressed by the question's number", () => {
    const message = readServerMessage({
      t: "Answer",
      seq: 4,
      answer: { t: "PatchConflicts", conflicts: [] },
    });
    expect(message).toEqual({
      t: "Answer",
      seq: 4,
      answer: { t: "PatchConflicts", conflicts: [] },
    });
    expect(describeServerMessage(message)).toBe("an answer");
  });
});

/**
 * The desk's own profiles, which ride in the snapshot because a client needs
 * them to *offer* the list at all — a brand-new show carries none.
 */
describe("the fixture library in the snapshot", () => {
  it("reads every field of every profile", () => {
    const snapshot = aSnapshot();
    expect(readSnapshot(snapshot, "s").fixtureLibrary).toEqual(snapshot.fixtureLibrary);
    expect(snapshot.fixtureLibrary.length).toBeGreaterThan(1);
  });

  it("names the profile and the attribute that was wrong", () => {
    const broken = (attributes: unknown) => ({
      ...aSnapshot(),
      fixtureLibrary: [
        { id: "x", manufacturer: "m", name: "n", mode: "1ch", footprint: 1, attributes },
      ],
    });
    expect(faultPath(() => readSnapshot(broken(7), "s"))).toBe("s.fixtureLibrary[0].attributes");
    expect(
      faultPath(() => readSnapshot(broken([{ attribute: "Nonesuch" }]), "s")),
    ).toBe("s.fixtureLibrary[0].attributes[0].attribute");
    expect(
      faultPath(() =>
        readSnapshot(
          broken([
            {
              attribute: "Dimmer",
              featureGroup: "Dimmer",
              coarseOffset: 0,
              fineOffset: null,
              defaultValue: 0,
              mergeMode: "MTP",
              invert: false,
              physicalFrom: 0,
              physicalTo: 1,
            },
          ]),
          "s",
        ),
      ),
    ).toBe("s.fixtureLibrary[0].attributes[0].mergeMode");
    expect(faultPath(() => readSnapshot({ ...aSnapshot(), fixtureLibrary: 7 }, "s"))).toBe(
      "s.fixtureLibrary",
    );
  });
});
