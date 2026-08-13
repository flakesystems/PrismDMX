/**
 * The store: what a delta does to it, and what losing the daemon does to it.
 *
 * The second half is **the second exit criterion of S23** at the level it is
 * decided at — the React half is in `src/App.test.tsx`, and both are needed:
 * this file says the values are gone, that one says nothing renders them.
 */

import { beforeEach, describe, expect, it } from "vitest";

import { nullSink, setLogSink } from "../log/logger";
import { emptyProgrammer, health, snapshot } from "../testing/fake-daemon";
import { DeskStore, INITIAL_STATE, NOTICE_LIMIT, deskEvents, programmerOf } from "./desk";

beforeEach(() => {
  setLogSink(nullSink);
});

describe("a store with no daemon", () => {
  it("holds nothing and says so", () => {
    const store = new DeskStore();
    expect(store.getState()).toBe(INITIAL_STATE);
    expect(store.getState().documents).toBeNull();
    expect(programmerOf(store.getState())).toBeNull();
    // A command with nowhere to go is refused rather than queued.
    expect(store.send({ t: "Oops" })).toBeNull();
  });

  it("notifies subscribers exactly when the state changes", () => {
    const store = new DeskStore();
    let notified = 0;
    const unsubscribe = store.subscribe(() => {
      notified += 1;
    });

    store.applySnapshot(snapshot());
    expect(notified).toBe(1);
    // Nothing to drop, so nothing changed and nobody is told.
    store.setStatus(store.getState().status);
    expect(notified).toBe(1);

    unsubscribe();
    store.disconnected();
    expect(notified).toBe(1);
  });
});

describe("the snapshot", () => {
  it("replaces everything rather than merging into it", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.applyDelta({ t: "DirtyFlag", unsavedChanges: true });
    expect(store.getState().unsavedChanges).toBe(true);

    store.applySnapshot(snapshot({ health: health({ unsavedChanges: false }) }));
    expect(store.getState().unsavedChanges).toBe(false);
    expect(store.getState().documents?.show).toEqual(snapshot().show);
  });
});

describe("deltas", () => {
  it("patches the documents the daemon patched", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    expect(
      store.applyDelta({
        t: "SessionPatch",
        ops: [{ op: "replace", path: "/session/executorPage", value: 5 }],
      }),
    ).toBe(true);
    expect(store.getState().documents?.session).toEqual({
      session: {
        activeViewId: 1,
        executorPage: 5,
        encoderBank: "Dimmer",
        commandLine: "fixture 1 at full",
        openWindows: [],
      },
      views: {},
    });
  });

  it("writes the two fields an executor delta carries", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.applyDelta({ t: "ExecutorState", executorId: 0, isActive: true, cueIndex: 2 });
    expect(store.getState().documents?.show).toEqual({
      fixtures: { "1": { name: "Front", universe: 1, address: 1 } },
      groups: {},
      sequences: {},
      executors: { "0": { isActive: true, currentCueIndex: 2, masterLevel: 65535 } },
    });
  });

  it("takes the programmer whole", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    const state = {
      ...emptyProgrammer(),
      selection: [1, 2],
      activeFeatureGroup: "Color" as const,
      clearStage: 1 as const,
    };
    store.applyDelta({ t: "ProgrammerChanged", state });
    expect(programmerOf(store.getState())).toEqual(state);
  });

  it("keeps the state object when a delta changes nothing", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    const before = store.getState();
    store.applyDelta({ t: "ShowPatch", ops: [] });
    store.applyDelta({ t: "SessionPatch", ops: [] });
    // Identity, because that is what a selector compares.
    expect(store.getState()).toBe(before);
  });

  it("reports a delta that does not fit instead of applying half of it", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    const before = store.getState();
    expect(
      store.applyDelta({
        t: "ShowPatch",
        ops: [{ op: "replace", path: "/fixtures/9/name", value: "nowhere" }],
      }),
    ).toBe(false);
    expect(store.getState()).toBe(before);
  });

  it("applies nothing before the snapshot and says nothing is wrong", () => {
    const store = new DeskStore();
    expect(store.applyDelta({ t: "DirtyFlag", unsavedChanges: true })).toBe(true);
    expect(store.getState().documents).toBeNull();
  });

  it("moves the save lamp and the output panel", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.applyDelta({ t: "DirtyFlag", unsavedChanges: true });
    expect(store.getState().unsavedChanges).toBe(true);

    store.applyDelta({ t: "OutputHealth", outputId: 1, health: "Degraded" });
    expect(store.getState().outputs).toEqual([{ id: 1, name: "Mock", health: "Degraded" }]);
    // An output nobody has heard of changes nothing rather than inventing a row.
    store.applyDelta({ t: "OutputHealth", outputId: 9, health: "Disconnected" });
    expect(store.getState().outputs).toEqual([{ id: 1, name: "Mock", health: "Degraded" }]);
  });

  it("keeps notices, newest last, and forgets the oldest", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    for (let index = 0; index < NOTICE_LIMIT + 5; index += 1) {
      store.applyDelta({ t: "Notice", level: "Warn", message: `notice ${index}` });
    }
    const notices = store.getState().notices;
    expect(notices).toHaveLength(NOTICE_LIMIT);
    expect(notices[0]?.message).toBe("notice 5");
    expect(notices.at(-1)?.message).toBe(`notice ${NOTICE_LIMIT + 4}`);
    // The identifiers are distinct, so two identical messages are two lines.
    expect(new Set(notices.map((notice) => notice.id)).size).toBe(NOTICE_LIMIT);
  });

  it("shows a refused command to the operator", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.refused(3, "CommandRefused", "no executor 9");
    expect(store.getState().notices.at(-1)?.message).toBe("no executor 9");
    store.notice("Info", "the show was saved");
    expect(store.getState().notices.at(-1)?.level).toBe("Info");
  });
});

describe("losing the daemon", () => {
  it("drops every value that came from it", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.applyDelta({ t: "DirtyFlag", unsavedChanges: true });
    store.notice("Warn", "the output is degraded");

    store.disconnected();

    const state = store.getState();
    expect(state.documents).toBeNull();
    expect(state.outputs).toBeNull();
    expect(state.health).toBeNull();
    expect(state.unsavedChanges).toBe(false);
    // The message is the one thing that is still true.
    expect(state.notices).toHaveLength(1);
  });

  it("dropping twice is not a change", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.disconnected();
    const empty = store.getState();
    store.disconnected();
    expect(store.getState()).toBe(empty);
  });

  it("comes back with the daemon's state and nothing of its own", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.disconnected();
    store.applySnapshot(
      snapshot({ session: { session: { commandLine: "" }, views: {} } }),
    );
    expect(store.getState().documents?.session).toEqual({
      session: { commandLine: "" },
      views: {},
    });
  });
});

describe("the connection events", () => {
  it("drops the documents on any status that is not connected", () => {
    const store = new DeskStore();
    const events = deskEvents(store, () => {});
    store.applySnapshot(snapshot());
    events.onStatus?.({ kind: "disconnected", reason: "gone", attempt: 1, retryInMs: 100 });
    expect(store.getState().documents).toBeNull();
    expect(store.getState().status.kind).toBe("disconnected");
  });

  it("asks for a resynchronisation when a delta does not fit", () => {
    const store = new DeskStore();
    const reasons: string[] = [];
    const events = deskEvents(store, (reason) => reasons.push(reason));
    store.applySnapshot(snapshot());

    events.onDelta?.({ t: "DirtyFlag", unsavedChanges: true });
    expect(reasons).toEqual([]);

    events.onDelta?.({
      t: "SessionPatch",
      ops: [{ op: "remove", path: "/session/nothing" }],
    });
    expect(reasons).toEqual(["a delta did not fit the mirror"]);
  });

  it("carries a refusal through to the operator", () => {
    const store = new DeskStore();
    const events = deskEvents(store, () => {});
    events.onRefused?.(1, "CommandRefused", "nothing to store");
    expect(store.getState().notices.at(-1)?.message).toBe("nothing to store");
  });

  it("has no way to receive telemetry", () => {
    // §7: telemetry must never reach reactive state. The store is not merely
    // told not to subscribe — there is nothing to subscribe with.
    const events = deskEvents(new DeskStore(), () => {});
    expect(events.onTelemetry).toBeUndefined();
  });

  it("sends through whatever it is attached to", () => {
    const store = new DeskStore();
    const sent: string[] = [];
    store.attach((command) => {
      sent.push(command.t);
      return 7;
    });
    expect(store.send({ t: "SaveShow" })).toBe(7);
    expect(sent).toEqual(["SaveShow"]);
  });
});
