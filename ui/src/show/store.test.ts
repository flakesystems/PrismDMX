/**
 * The store preview: the sentence on the button, and the cadence behind it.
 *
 * The sentences are held to **the daemon's own answers**, decoded out of
 * `ui/tests/fixtures/show-recording.json` — so what an operator reads before
 * pressing Store is what `prism_core::ShowFile::preview_store` actually said,
 * and not a second opinion this file wrote down twice.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Answer, Query, StorePreview } from "../bindings";
import { nullSink, setLogSink } from "../log/logger";
import { answerAbout, showRecording } from "../testing/show-recording";
import { StoreRequester, isStorable, storePreviewOf, storeText } from "./store";

const recording = showRecording;

/** The preview inside the recorded answer about something. */
function previewAbout(about: string): StorePreview {
  const preview = storePreviewOf(answerAbout(about));
  if (preview === null) {
    throw new Error(`the step about ${about} is not a store preview`);
  }
  return preview;
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("what the Store button says", () => {
  it("names the mode the daemon named, and never one of its own", () => {
    // The whole reason `StoreMode` is a value on the answer: S39 adds Override
    // and Remove, and a spelled-out "Merge" here would go on looking right.
    const preview = previewAbout("this is the overwrite an operator");
    expect(preview.mode).toBe("Merge");
    expect(storeText(preview, "cue 1")).toContain(preview.mode);
  });

  it("says what an overwrite costs, in the daemon's numbers", () => {
    const preview = previewAbout("this is the overwrite an operator");
    const text = storeText(preview, "cue 1");
    expect(text).toContain("2 added");
    expect(text).toContain("3 replaced");
    // Nothing was kept in that one, and a zero is left out rather than shown:
    // *0 kept* is noise on a button somebody reads at speed.
    expect(text).not.toContain("kept");
  });

  it("says that nothing is there when the store would create it", () => {
    const preview = previewAbout("it does not exist yet");
    expect(preview.exists).toBe(false);
    expect(storeText(preview, "cue 1")).toContain("Nothing is there yet");
    expect(isStorable(preview)).toBe(true);
  });

  it("names what is already filed there when the store would overwrite it", () => {
    // The number that makes Merge legible — three kept — and the name of the
    // preset an operator is about to write over.
    const preview = previewAbout("the three blues are replaced");
    expect(preview.exists).toBe(true);
    const text = storeText(preview, "preset 1");
    expect(text).toContain("Deep blue");
    expect(text).toContain("3 replaced");
    expect(text).toContain("3 kept");
  });

  it("carries the daemon's refusal rather than a word of its own", () => {
    for (const about of ["a sequence that is not there", "nothing to store"]) {
      const preview = previewAbout(about);
      expect(preview.accepted).toBe(false);
      expect(isStorable(preview)).toBe(false);
      expect(storeText(preview, "cue 1")).toBe(preview.refusal);
    }
  });

  it("says something of its own only when a refusal came with no words", () => {
    // The daemon always says why (`ShowError` has a `Display` for every
    // variant), so this is the arm that exists so a refusal can never be a
    // blank button.
    expect(
      storeText(
        {
          accepted: false,
          refusal: null,
          exists: false,
          name: "",
          mode: "Merge",
          added: 0,
          replaced: 0,
          kept: 0,
        },
        "cue 2",
      ),
    ).toBe("cue 2 cannot be stored");
  });

  it("says something plain before an answer has arrived, and stays pressable", () => {
    // A button that stayed dead until an answer arrived would be a button a
    // disconnected daemon locks. The daemon refuses what it will not take.
    expect(storeText(null, "cue 4")).toBe("Store cue 4");
    expect(isStorable(null)).toBe(true);
  });

  it("says so when a store would change nothing at all", () => {
    // Reachable when every value a store writes is already there at the same
    // value: an accepted store with three zero counts. A bare full stop would
    // leave an operator reading a sentence with a hole in it.
    const preview: StorePreview = {
      accepted: true,
      refusal: null,
      exists: true,
      name: "Opening",
      mode: "Merge",
      added: 0,
      replaced: 0,
      kept: 0,
    };
    expect(storeText(preview, "cue 1")).toContain("nothing changes");
  });

  it("is not a preview at all for any other kind of answer", () => {
    expect(storePreviewOf(null)).toBeNull();
    expect(storePreviewOf({ t: "PatchConflicts", conflicts: [] })).toBeNull();
  });
});

describe("the store requester", () => {
  it("asks with the target it was given", async () => {
    const asked: Query[] = [];
    const ask = vi.fn(async (query: Query) => {
      asked.push(query);
      return Promise.resolve<Answer | null>(null);
    });
    const requester = new StoreRequester(ask, () => undefined);
    requester.request({ t: "Cue", sequenceId: 3, cueNumber: "1.5" });
    await Promise.resolve();
    expect(asked).toEqual([
      { t: "StorePreview", target: { t: "Cue", sequenceId: 3, cueNumber: "1.5" } },
    ]);
  });

  it("draws the newest answer and drops the older one", async () => {
    // The case that matters: an answer to a cue number that has since been
    // typed over would tell an operator that the *previous* cue is the one
    // about to be overwritten.
    const pending: ((answer: Answer | null) => void)[] = [];
    const ask = (): Promise<Answer | null> =>
      new Promise((resolve) => {
        pending.push(resolve);
      });
    const seen: (StorePreview | null)[] = [];
    const requester = new StoreRequester(ask, (preview) => seen.push(preview));

    requester.request({ t: "Cue", sequenceId: 1, cueNumber: "1" });
    requester.request({ t: "Cue", sequenceId: 1, cueNumber: "2" });
    // The *older* question is answered first, which is the order this is for.
    pending[0]?.(answerAbout("it does not exist yet"));
    pending[1]?.(answerAbout("this is the overwrite an operator"));
    await Promise.resolve();
    await Promise.resolve();

    expect(seen).toHaveLength(1);
    expect(seen[0]?.exists).toBe(true);
  });

  it("delivers nothing at all after it has been stopped", async () => {
    const pending: ((answer: Answer | null) => void)[] = [];
    const ask = (): Promise<Answer | null> =>
      new Promise((resolve) => {
        pending.push(resolve);
      });
    const seen: (StorePreview | null)[] = [];
    const requester = new StoreRequester(ask, (preview) => seen.push(preview));
    requester.request({ t: "Preset", presetId: 1, pool: "Color" });
    requester.stop();
    pending[0]?.(answerAbout("it does not exist yet"));
    await Promise.resolve();
    await Promise.resolve();
    expect(seen).toEqual([]);
  });
});

describe("the recorded answers", () => {
  it("are of a script that asked the interesting questions", () => {
    // The browser's tests lean on these five being *in* the recording; the Rust
    // guard `a_preview_says_what_the_store_that_follows_it_does` asserts the
    // same thing from the other side.
    const previews = recording.steps
      .filter((step) => step.isQuery && step.answer !== null)
      .map((step) => previewAbout(step.what));
    expect(previews.length).toBeGreaterThanOrEqual(5);
    expect(previews.some((preview) => !preview.exists && preview.accepted)).toBe(true);
    expect(previews.some((preview) => preview.exists && preview.accepted)).toBe(true);
    expect(previews.some((preview) => preview.kept > 0)).toBe(true);
    expect(previews.some((preview) => preview.replaced > 0)).toBe(true);
    expect(previews.some((preview) => !preview.accepted)).toBe(true);
    // And every one of them is a `Merge`, because that is the only mode this
    // build has — the assertion that goes red when S39 lands.
    expect(previews.every((preview) => preview.mode === "Merge")).toBe(true);
  });

  it("came with no deltas at all, because a question changes nothing", () => {
    for (const step of recording.steps) {
      if (step.isQuery) {
        expect(step.deltas, step.what).toEqual([]);
      }
    }
  });
});
