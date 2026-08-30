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
      // A playback is a cue list's number since S45 — the tagged pair, with an
      // executor in one arm, is what punch-list entry B18 found two of.
      { t: "PlaybackState", playback: 2, isActive: true, cueIndex: null },
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
      selectedGroups: [3],
      manualSelection: [2],
      activeFeatureGroup: "Color",
      values: [
        { fixture: 1, attribute: "Red", value: { value: 65535, source: "Manual", presetRef: null } },
        { fixture: 2, attribute: "Dimmer", value: { value: 0, source: "Preset", presetRef: 4 } },
      ],
      clearStage: 2,
    };
    expect(readProgrammerState(state, "p")).toEqual(state);
  });

  /**
   * **The two provenance lists are optional** — S43, B27. They are
   * `#[serde(default)]` on the daemon side, so a snapshot written before they
   * existed reads with both empty, which is exactly the state *nothing was
   * selected through a group*. Refusing the snapshot over them would be
   * refusing to open a show for a reading nothing on the screen needs.
   */
  it("reads one from a daemon that does not send the group provenance", () => {
    const state = {
      selection: [1],
      activeFeatureGroup: "Dimmer",
      values: [],
      clearStage: 1,
    };
    expect(readProgrammerState(state, "p")).toEqual({
      ...state,
      selectedGroups: [],
      manualSelection: [],
    });
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

  /**
   * **This test asserted the bug.** Until S43 it said *stage 3 is refused*,
   * which was true of a three-stage Clear and became false the moment the key
   * started saying what the **next** press would clear — `Nothing`, `Values`,
   * `Selection`, `All` (punch-list B2). The decoder still refused 3, so a desk
   * that reached the fourth stage faulted **every attached client**, which
   * resynchronised, was served the same state, and faulted again: a
   * connect-and-drop loop that from the operator's seat looks exactly like the
   * daemon having died. It had not; it was still running and still sending DMX.
   *
   * A test that pins a bound has to be read again when the bound moves, and this
   * one now walks every stage the domain has rather than naming the first one it
   * has not got.
   */
  it("reads every stage the Clear key has, and refuses one it has not", () => {
    for (const clearStage of [0, 1, 2, 3]) {
      expect(readProgrammerState({ ...emptyProgrammer(), clearStage }, "p").clearStage).toBe(
        clearStage,
      );
    }
    expect(faultPath(() => readProgrammerState({ ...emptyProgrammer(), clearStage: 4 }, "p"))).toBe(
      "p.clearStage",
    );
    expect(
      faultPath(() => readProgrammerState({ ...emptyProgrammer(), clearStage: -1 }, "p")),
    ).toBe("p.clearStage");
  });
});

describe("the snapshot", () => {
  it("reads the three documents, the outputs and the health", () => {
    const snapshot = aSnapshot();
    expect(readSnapshot(snapshot, "s")).toEqual(snapshot);
  });

  /// S33: a daemon one version behind sends the three fields an output row had
  /// before the rig was data. Meeting a missing field is better than refusing a
  /// snapshot over a status panel, so the four new ones read as absent.
  it("reads an output row from a daemon that predates the rig", () => {
    const older = { ...aSnapshot(), outputs: [{ id: 1, name: "Mock", health: "Ok" }] };
    expect(readSnapshot(older, "s").outputs).toEqual([
      {
        id: 1,
        name: "Mock",
        health: "Ok",
        output: null,
        framesSent: 0,
        lastError: null,
        lastErrorAgoMs: null,
      },
    ]);
  });

  it("reads the configured row beside the light", () => {
    const rig = {
      ...aSnapshot(),
      outputs: [
        {
          id: 2,
          name: "Stage left node",
          health: "Degraded",
          output: {
            id: 2,
            name: "Stage left node",
            kind: { t: "ArtNet", nodes: ["10.0.0.9:6454"], sync: false, ports: [] },
            universes: [5, 6],
            enabled: true,
          },
          framesSent: 4711,
          lastError: "the interface is not connected",
          lastErrorAgoMs: 4000,
        },
      ],
    };
    const read = readSnapshot(rig, "s").outputs[0];
    expect(read?.output?.universes).toEqual([5, 6]);
    expect(read?.framesSent).toBe(4711);
    expect(read?.lastError).toBe("the interface is not connected");
    expect(read?.lastErrorAgoMs).toBe(4000);
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

  /**
   * S48. The one answer whose whole content is arithmetic no client may repeat:
   * what each cue of a list **inherits**. It goes through the real decoder here
   * because a message this build could not read would look like a daemon
   * sending nonsense, and the connection would resynchronise rather than the cue
   * sheet drawing a column.
   */
  it("reads what every cue of a list inherits", () => {
    const tracking = {
      t: "CueTracking",
      sequenceId: 3,
      cues: [
        { number: "1", inherited: [], blocks: true },
        {
          number: "2",
          inherited: [{ fixture: 7, attribute: "Pan", value: 32768 }],
          blocks: false,
        },
      ],
    };
    expect(readAnswer(tracking, "a")).toEqual(tracking);
  });

  it("names the field that was not what it should be", () => {
    expect(faultPath(() => readAnswer({ t: "Elsewhere" }, "a"))).toBe("a.t");
    // And the S48 answer field by field, including the attribute — which is a
    // **checked** string and not an `as`, because `CLAUDE.md` forbids the claim.
    expect(
      faultPath(() => readAnswer({ t: "CueTracking", sequenceId: 1 }, "a")),
    ).toBe("a.cues");
    expect(
      faultPath(() =>
        readAnswer(
          {
            t: "CueTracking",
            sequenceId: 1,
            cues: [
              {
                number: "1",
                inherited: [{ fixture: 1, attribute: "Nonsense", value: 0 }],
                blocks: false,
              },
            ],
          },
          "a",
        ),
      ),
    ).toBe("a.cues[0].inherited[0].attribute");
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
 * How many profiles the desk has — a **number**, since S44 made the library two
 * thousand of them and a snapshot has to fit in a frame. What a client does
 * with the library itself is ask.
 */
describe("the fixture library in the snapshot", () => {
  it("is a count rather than the profiles", () => {
    const snapshot = aSnapshot();
    expect(readSnapshot(snapshot, "s").fixtureLibrary).toBe(snapshot.fixtureLibrary);
    expect(snapshot.fixtureLibrary).toBeGreaterThan(1);
    expect(faultPath(() => readSnapshot({ ...aSnapshot(), fixtureLibrary: "many" }, "s"))).toBe(
      "s.fixtureLibrary",
    );
  });

});
