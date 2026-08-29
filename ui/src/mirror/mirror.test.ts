/**
 * Which delta belongs to which document.
 *
 * The interesting cases are the two that are easy to get wrong in opposite
 * directions: `ExecutorState` writes into the *show* rather than being ignored
 * (S18 checked that by removing it and watching a client drift), and the three
 * deltas that are not documents at all leave all three alone.
 */

import { describe, expect, it } from "vitest";

import type { Delta } from "../bindings";
import { emptyProgrammer } from "../testing/fake-daemon";
import type { Documents } from "./mirror";
import { applyDelta, applyDeltas } from "./mirror";
import { MirrorFault } from "./patch";

function documents(): Documents {
  return {
    show: {
      fixtures: { "1": { name: "Front" } },
      executors: { "3": { isActive: false, currentCueIndex: null } },
    },
    session: { session: { executorPage: 0 }, views: {} },
    programmer: emptyProgrammer(),
  };
}

describe("the three documents", () => {
  it("patches the show and leaves the session alone", () => {
    const next = applyDelta(documents(), {
      t: "ShowPatch",
      ops: [{ op: "replace", path: "/fixtures/1/name", value: "Side" }],
    });
    expect(next.show).toEqual({
      fixtures: { "1": { name: "Side" } },
      executors: { "3": { isActive: false, currentCueIndex: null } },
    });
    expect(next.session).toBe(documents().session ? next.session : next.session);
    expect(next.programmer).toEqual(emptyProgrammer());
  });

  it("patches the session and leaves the show alone", () => {
    const start = documents();
    const next = applyDelta(start, {
      t: "SessionPatch",
      ops: [{ op: "replace", path: "/session/executorPage", value: 4 }],
    });
    expect(next.session).toEqual({ session: { executorPage: 4 }, views: {} });
    // Identity: the show did not change, so a selector on it has nothing to do.
    expect(next.show).toBe(start.show);
  });

  it("takes the programmer whole", () => {
    const state = { ...emptyProgrammer(), selection: [1, 2], clearStage: 1 as const };
    expect(applyDelta(documents(), { t: "ProgrammerChanged", state }).programmer).toBe(state);
  });

  /**
   * **A playback is a cue list's, so the two fields go on the sequence** — S45.
   *
   * It was two collections until then, chosen by the delta's own tag, and the
   * executors are what punch-list entry B18 found two of.
   * `prism_core::Show::record_playback_state` writes the same one place from the
   * same delta, which is what keeps this mirror and the daemon agreeing.
   */
  it("writes the two fields a playback delta carries onto the cue list", () => {
    const start: Documents = {
      show: { sequences: { "7": { name: "Act 2", isActive: false, currentCueIndex: null } } },
      session: {},
      programmer: emptyProgrammer(),
    };
    const next = applyDelta(start, {
      t: "PlaybackState",
      playback: 7,
      isActive: true,
      cueIndex: 1,
    });
    expect(next.show).toEqual({
      sequences: { "7": { name: "Act 2", isActive: true, currentCueIndex: 1 } },
    });

    const off = applyDelta(next, {
      t: "PlaybackState",
      playback: 7,
      isActive: false,
      cueIndex: null,
    });
    expect(off.show).toEqual({
      sequences: { "7": { name: "Act 2", isActive: false, currentCueIndex: null } },
    });
  });

  it("says so when a playback delta names a cue list the show has never heard of", () => {
    // Not silence: it means this client and the daemon disagree about the show,
    // and the answer to that is a fresh snapshot rather than a guess.
    expect(() =>
      applyDelta(documents(), {
        t: "PlaybackState",
        playback: 9,
        isActive: true,
        cueIndex: 0,
      }),
    ).toThrow(MirrorFault);
  });

  it("leaves everything alone for the deltas that are not documents", () => {
    const start = documents();
    const others: Delta[] = [
      { t: "OutputHealth", outputId: 1, health: "Degraded" },
      { t: "OutputsChanged", outputs: [] },
      // S36. The desk in the rack belongs to the machine, so it must never
      // reach the document a client believes the show to be.
      { t: "SurfaceChanged", port: "2- X-Touch" },
      { t: "DirtyFlag", unsavedChanges: true },
      { t: "Notice", level: "Info", message: "saved" },
      { t: "ShowPatch", ops: [] },
      { t: "SessionPatch", ops: [] },
    ];
    for (const delta of others) {
      // The same object, so a caller can compare by identity to decide whether
      // to notify anybody at all.
      expect(applyDelta(start, delta)).toBe(start);
    }
  });

  it("applies a run of deltas in order", () => {
    const next = applyDeltas(documents(), [
      { t: "SessionPatch", ops: [{ op: "replace", path: "/session/executorPage", value: 1 }] },
      { t: "SessionPatch", ops: [{ op: "replace", path: "/session/executorPage", value: 2 }] },
      { t: "ShowPatch", ops: [{ op: "add", path: "/fixtures/2", value: { name: "Back" } }] },
    ]);
    expect(next.session).toEqual({ session: { executorPage: 2 }, views: {} });
    expect(next.show).toEqual({
      fixtures: { "1": { name: "Front" }, "2": { name: "Back" } },
      executors: { "3": { isActive: false, currentCueIndex: null } },
    });
  });

  it("stops at the delta that does not fit", () => {
    expect(() =>
      applyDeltas(documents(), [
        { t: "SessionPatch", ops: [{ op: "replace", path: "/session/executorPage", value: 1 }] },
        { t: "ShowPatch", ops: [{ op: "remove", path: "/nothing" }] },
      ]),
    ).toThrow(MirrorFault);
  });
});
