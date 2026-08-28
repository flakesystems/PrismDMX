/**
 * **The conflict is shown before it is committed, and the daemon computed it.**
 *
 * S27's central exit criterion, checked against the answers a real `prismd` gave
 * for the very questions this interface asks — `ui/tests/fixtures/patch-recording.json`
 * carries them beside the patch each one was asked about.
 *
 * Nothing in TypeScript decides what a preview *should* say. What is asserted
 * here is that the interface turns the daemon's answer into the right sentence
 * and the right set of red rows, and that a question asked twice in a row
 * cannot answer with the older of the two.
 */

import { decode } from "@msgpack/msgpack";
import { beforeEach, describe, expect, it } from "vitest";

import type { Answer, PatchPreview, Query } from "../bindings";
import { readServerMessage } from "../ipc/protocol";
import { nullSink, setLogSink } from "../log/logger";
import {
  PreviewRequester,
  conflictedFixtures,
  conflictsOf,
  isAcceptable,
  previewOf,
  previewText,
} from "./preview";

import recordingText from "../../tests/fixtures/patch-recording.json?raw";

interface Recording {
  readonly steps: readonly {
    readonly what: string;
    readonly isQuery: boolean;
    readonly answer: string | null;
  }[];
}

/**
 * A recorded step, found by **what it is for** rather than by its number.
 *
 * The script grows — S44 added three searches in the middle of it — and an index
 * written down here would quietly start pointing at another step and assert
 * something true about the wrong thing.
 */
function stepAbout(about: string): { readonly answer: string | null } {
  const step = recording.steps.find((entry) => entry.what.includes(about));
  if (step === undefined) {
    throw new Error(`no recorded step is about ${JSON.stringify(about)}`);
  }
  return step;
}

const recording = JSON.parse(recordingText) as Recording;

/** Bytes out of a base64 payload. */
function payload(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/** The daemon's answer to the query the step about `about` asked. */
function answerAt(about: string): Answer {
  const encoded = stepAbout(about).answer;
  if (encoded === null || encoded === undefined) {
    throw new Error(`the step about ${JSON.stringify(about)} carries no answer`);
  }
  const message = readServerMessage(decode(payload(encoded)));
  if (message.t !== "Answer") {
    throw new Error("that payload is not an answer");
  }
  return message.answer;
}

/** The preview the step about `about` asked for, or a named failure. */
function previewAt(about: string): PatchPreview {
  const preview = previewOf(answerAt(about));
  if (preview === null) {
    throw new Error(`the step about ${JSON.stringify(about)} is not a preview`);
  }
  return preview;
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("what the operator is told before they commit", () => {
  it("says a clear address is clear, and where the fixture would end", () => {
    // Step 1 of the recorded script: would a PAR fit at address 30?
    const text = previewText(previewAt("would a PAR fit at address 30"), 6);
    expect(text).toContain("Free");
    expect(text).toContain("4 channels");
    // 33, which is the daemon's arithmetic and not this file's.
    expect(text).toContain("33");
  });

  it("says an overlap is an overlap **and** that it is allowed", () => {
    // Step 3: a second PAR at 32, over the one at 30. `prism_core::conflict`
    // reports it and does not refuse it — cloning a fixture is a technique in
    // daily use — so the sentence has to carry both facts at once.
    const preview = previewAt("would a second PAR at 32 clash");
    expect(preview.accepted).toBe(true);
    expect(isAcceptable(preview)).toBe(true);
    const text = previewText(preview, 7);
    expect(text).toContain("Overlaps 32–33 with fixture 6");
    expect(text).toContain("higher fixture number wins");
    expect(text).not.toContain("Free");
  });

  it("says a refusal in the daemon's own words, before anything is sent", () => {
    // Step 6: four channels from 510 runs past the end of a universe. The
    // command was never sent — this is the answer to a question.
    const preview = previewAt("would it fit at 510");
    expect(preview.accepted).toBe(false);
    expect(isAcceptable(preview)).toBe(false);
    expect(previewText(preview, 7)).toContain("510");
    expect(previewText(preview, 7)).toBe(preview.refusal);
  });

  it("says a profile the show has not got is a refusal, until it has it", () => {
    // Steps 7 and 9 are the same question with an `EmbedFixtureType` between
    // them, which is the pair that makes the desk's library worth having.
    expect(previewAt("a profile the show has not got").accepted).toBe(false);
    expect(previewAt("a profile the show has not got").footprint).toBe(0);
    expect(previewAt("the same question again").accepted).toBe(true);
    // Thirteen, not the eleven this line used to say: **B1** gave the moving
    // head a gobo wheel and a control channel on its way to giving every colour
    // channel a home value, so the profile in the recording is two channels
    // wider than it was. The number is read off the recording rather than
    // written down twice — what is asserted is that the sentence carries the
    // footprint the daemon answered with.
    const answered = previewAt("the same question again");
    expect(previewText(answered, 7)).toContain(`${String(answered.footprint)} channels`);
    expect(answered.footprint).toBe(13);
  });

  it("has nothing to say before the daemon has answered", () => {
    // `null` is *not yet answered*, and a form that would not submit until an
    // answer arrived would be a form a slow daemon locks.
    expect(previewText(null, 1)).toBe("");
    expect(isAcceptable(null)).toBe(true);
    // And a refusal with no words in it still says something.
    expect(
      previewText(
        { accepted: false, refusal: null, footprint: 0, lastAddress: null, conflicts: [] },
        1,
      ),
    ).toContain("refuse");
    // A footprint with no end address — which cannot arrive from a daemon that
    // accepted the patch, and would otherwise print `null`.
    expect(
      previewText(
        { accepted: true, refusal: null, footprint: 3, lastAddress: null, conflicts: [] },
        1,
      ),
    ).toContain("3 channels");
  });

  it("names the other fixture, whichever side of the pair it is on", () => {
    // A conflict names the lower number first. The sentence has to name the
    // *other* one, so it reads the same whether the row being edited is the
    // lower or the higher of the two.
    const conflicts = [{ universe: 1, from: 5, to: 8, first: 3, second: 9 }];
    const preview: PatchPreview = {
      accepted: true,
      refusal: null,
      footprint: 4,
      lastAddress: 8,
      conflicts,
    };
    expect(previewText(preview, 9)).toContain("with fixture 3");
    expect(previewText(preview, 3)).toContain("with fixture 9");
  });
});

describe("which rows the sheet paints red", () => {
  it("is the pairs the daemon reports, both halves of each", () => {
    // Step 5: the show has an overlap in it. Step 12: the fixture was moved and
    // it has not.
    expect(
      conflictedFixtures(conflictsOf(answerAt("now the show has an overlap"))),
    ).toEqual(new Set([6, 7]));
    expect(
      conflictedFixtures(conflictsOf(answerAt("takes the overlap away again"))),
    ).toEqual(new Set());
    // The same question before anything was patched.
    expect(conflictsOf(answerAt("what overlaps in the rig as it stands"))).toEqual([]);
  });

  it("reads nothing out of an answer of the other kind", () => {
    // A preview is not a conflict list and a conflict list is not a preview.
    expect(conflictsOf(answerAt("would a PAR fit at address 30"))).toEqual([]);
    expect(previewOf(answerAt("what overlaps in the rig as it stands"))).toBeNull();
    expect(conflictsOf(null)).toEqual([]);
    expect(previewOf(null)).toBeNull();
  });
});

describe("a question asked while the last one is still in flight", () => {
  /** A pending question, resolvable by hand. */
  function pending(): {
    ask: (query: Query) => Promise<Answer | null>;
    answer: (index: number, value: Answer | null) => void;
    asked: Query[];
  } {
    const resolvers: ((value: Answer | null) => void)[] = [];
    const asked: Query[] = [];
    return {
      asked,
      ask: (query) => {
        asked.push(query);
        return new Promise<Answer | null>((resolve) => resolvers.push(resolve));
      },
      answer: (index, value) => {
        resolvers[index]?.(value);
      },
    };
  }

  const query = (address: number): Query => ({
    t: "PatchPreview",
    id: 7,
    typeId: "generic.rgbw.par",
    universe: 1,
    address,
  });

  it("draws the newest answer and drops the older one, whenever it arrives", async () => {
    // The case that matters: two keystrokes, and the *first* answer arrives
    // last. Drawn, it would tell an operator that the address in front of them
    // clashes when it is the one they have already typed over that did.
    const daemon = pending();
    const seen: (PatchPreview | null)[] = [];
    const requester = new PreviewRequester(daemon.ask, (preview) => seen.push(preview));

    requester.request(query(30));
    requester.request(query(32));
    daemon.answer(1, answerAt("would a second PAR at 32 clash"));
    daemon.answer(0, answerAt("would a PAR fit at address 30"));
    await Promise.resolve();
    await Promise.resolve();

    expect(daemon.asked).toEqual([query(30), query(32)]);
    expect(seen).toHaveLength(1);
    expect(seen[0]?.conflicts).toHaveLength(1);
  });

  it("delivers nothing at all once it has been stopped", async () => {
    const daemon = pending();
    const seen: (PatchPreview | null)[] = [];
    const requester = new PreviewRequester(daemon.ask, (preview) => seen.push(preview));
    requester.request(query(30));
    requester.stop();
    daemon.answer(0, answerAt("would a PAR fit at address 30"));
    await Promise.resolve();
    await Promise.resolve();
    expect(seen).toEqual([]);
  });

  it("passes on a daemon that did not answer as no answer", async () => {
    const daemon = pending();
    const seen: (PatchPreview | null)[] = [];
    const requester = new PreviewRequester(daemon.ask, (preview) => seen.push(preview));
    requester.request(query(30));
    daemon.answer(0, null);
    await Promise.resolve();
    await Promise.resolve();
    expect(seen).toEqual([null]);
  });
});
