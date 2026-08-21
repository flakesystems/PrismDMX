/**
 * The store: what a delta does to it, and what losing the daemon does to it.
 *
 * The second half is **the second exit criterion of S23** at the level it is
 * decided at — the React half is in `src/App.test.tsx`, and both are needed:
 * this file says the values are gone, that one says nothing renders them.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Answer, Query } from "../bindings";
import { nullSink, setLogSink } from "../log/logger";
import { emptyProgrammer, health, snapshot } from "../testing/fake-daemon";
import {
  DeskStore,
  INITIAL_STATE,
  NOTICE_LIMIT,
  QUERY_TIMEOUT_MS,
  deskEvents,
  programmerOf,
} from "./desk";

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
    // The whole document, not the one member: an applier that replaced the
    // root would pass an assertion about `executorPage` alone.
    expect(store.getState().documents?.session).toEqual({
      session: {
        activeViewId: 1,
        executorPage: 5,
        encoderBank: "Dimmer",
        commandLine: "fixture 1 at full",
        openWindows: [
          { instanceId: 1, type: "DmxSheet", x: 0, y: 0, w: 640, h: 480, params: {} },
        ],
        focusedWindow: 1,
      },
      views: { "1": { id: 1, name: "View 1", windows: [] } },
    });
  });

  it("writes the two fields a playback delta carries", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.applyDelta({
      t: "PlaybackState",
      playback: { t: "Executor", executorId: 0 },
      isActive: true,
      cueIndex: 2,
    });
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

  /**
   * Dismissing is what makes the notices list the only one there is. A view
   * that kept its own set of dismissed identifiers would still be holding
   * `notice 0` long after `NOTICE_LIMIT` had evicted it — which is the case
   * asserted here, because it is the one that produced an empty frame nobody
   * could get rid of.
   */
  it("drops a dismissed notice and leaves the rest alone", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.notice("Warn", "one");
    store.notice("Warn", "two");
    store.notice("Warn", "three");
    const [, second] = store.getState().notices;

    store.dismissNotice(second?.id ?? -1);
    expect(store.getState().notices.map((notice) => notice.message)).toEqual(["one", "three"]);
  });

  it("dismissing a notice that is not there is not a change", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    store.notice("Warn", "one");
    const before = store.getState();

    store.dismissNotice(9999);
    // The same object, not merely an equal one: a new state here would be a
    // re-render of every subscriber for nothing.
    expect(store.getState()).toBe(before);
  });

  it("forgets a dismissed identifier rather than remembering it", () => {
    const store = new DeskStore();
    store.applySnapshot(snapshot());
    for (let index = 0; index < NOTICE_LIMIT + 5; index += 1) {
      store.notice("Warn", `notice ${index}`);
    }
    // Dismiss every one that is still held. The list is then empty, and the
    // count of what has been dismissed is not kept anywhere to disagree with it.
    for (const notice of [...store.getState().notices]) {
      store.dismissNotice(notice.id);
    }
    expect(store.getState().notices).toEqual([]);
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

/**
 * **Questions (§5.2), and what happens when one is not answered.**
 *
 * A question changes nothing by construction, so what there is to get wrong is
 * the waiting: an answer that arrives for a question nobody is waiting for, an
 * answer that never arrives at all, and a connection that goes while one is in
 * flight. All three end the same way — the caller is told *no answer* and shows
 * what it last knew, which is the only honest thing a client with no daemon can
 * do (D3).
 */
describe("asking the daemon a question", () => {
  const conflicts: Answer = { t: "PatchConflicts", conflicts: [] };

  it("carries the answer back to the caller that asked", async () => {
    const store = new DeskStore();
    const asked: Query[] = [];
    store.attach(
      () => null,
      (query) => {
        asked.push(query);
        return 3;
      },
    );
    const waiting = store.ask({ t: "PatchConflicts" });
    store.answered(3, conflicts);
    await expect(waiting).resolves.toEqual(conflicts);
    expect(asked).toEqual([{ t: "PatchConflicts" }]);
  });

  it("answers with nothing when there is no daemon to ask", async () => {
    // The default enquirer, which is what a store with no connection attached
    // has. A caller shows what it last knew; it never guesses.
    await expect(new DeskStore().ask({ t: "PatchConflicts" })).resolves.toBeNull();
  });

  it("answers with nothing when the daemon says nothing for long enough", async () => {
    vi.useFakeTimers();
    try {
      const store = new DeskStore();
      store.attach(
        () => null,
        () => 1,
      );
      const waiting = store.ask({ t: "PatchConflicts" });
      vi.advanceTimersByTime(QUERY_TIMEOUT_MS);
      await expect(waiting).resolves.toBeNull();
      // And the answer arriving afterwards is dropped rather than resolving a
      // promise that has already been settled.
      store.answered(1, conflicts);
    } finally {
      vi.useRealTimers();
    }
  });

  it("answers every question in flight when the connection goes", async () => {
    // A patch form waiting on a preview from a daemon that has stopped would
    // otherwise wait out the timeout and then draw an answer about a show
    // nobody is holding any more.
    const store = new DeskStore();
    let seq = 0;
    store.attach(
      () => null,
      () => {
        seq += 1;
        return seq;
      },
    );
    const first = store.ask({ t: "PatchConflicts" });
    const second = store.ask({ t: "PatchConflicts" });
    store.disconnected();
    await expect(first).resolves.toBeNull();
    await expect(second).resolves.toBeNull();
  });

  it("ignores an answer nobody is waiting for", () => {
    // A keystroke two keystrokes ago, or one that timed out. There is nothing
    // to do with it: an answer changes no state by construction.
    const store = new DeskStore();
    const before = store.getState();
    store.answered(99, conflicts);
    expect(store.getState()).toBe(before);
  });

  it("routes the daemon's answers into the store", () => {
    const store = new DeskStore();
    let seq: number | null = null;
    store.attach(
      () => null,
      () => 5,
    );
    const waiting = store.ask({ t: "PatchConflicts" }).then((answer) => answer);
    const events = deskEvents(store, () => {});
    events.onAnswer?.(5, conflicts);
    seq = 5;
    expect(seq).toBe(5);
    return expect(waiting).resolves.toEqual(conflicts);
  });
});
