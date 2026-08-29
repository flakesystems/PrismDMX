/**
 * The look readers, held to a daemon's own answers.
 *
 * `ui/tests/fixtures/show-recording.json` is written by
 * `crates/prismd/tests/ui_show.rs` off a running `prismd`: a show being written,
 * corrected and played, and after each step **the sequences, the presets and the
 * executor grid a fresh client's snapshot says are there**. This file replays
 * the deltas of that script through this interface's own mirror and compares its
 * readers' answers, field by field, with the daemon's.
 *
 * A reader tested against a document the same language built is a test of
 * nothing, which is the trap every session since S23 has found in its own layer.
 */

import { beforeEach, describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";
import { nullSink, setLogSink } from "../log/logger";
import { applyDelta } from "../mirror/mirror";
import type { Documents } from "../mirror/mirror";
import type { RecordedCue, RecordedPreset, RecordedSequence } from "../testing/show-recording";
import { deltaOf, showRecording, snapshotOf, stepAbout } from "../testing/show-recording";
import {
  colorStyle,
  cueEditInForce,
  executorInForce,
  executorsPlaying,
  nextCueNumber,
  nextFreeNumber,
  poolRows,
  presetRows,
  secondsText,
  sequenceInForce,
  sequenceRow,
  sequenceRows,
  triggerText,
} from "./looks";

const recording = showRecording;

/** The documents, as this interface's mirror holds them at the start. */
function initialDocuments(): Documents {
  const snapshot = snapshotOf(recording.initialSnapshot);
  return {
    show: snapshot.show,
    session: snapshot.session,
    programmer: snapshot.programmer,
  };
}

/** This interface's reading of the sequences, in the recording's own shape. */
function sequencesAsRecorded(show: JsonValue | null): RecordedSequence[] {
  return sequenceRows(show).map((row) => ({
    id: row.id,
    name: row.name,
    looping: row.looping,
    cues: row.cues.map((cue) => ({
      number: cue.number,
      name: cue.name,
      fadeIn: cue.fadeIn,
      fadeOut: cue.fadeOut,
      delay: cue.delay,
      trigger: cue.trigger,
      triggerTime: cue.triggerTime,
      parts: cue.parts.map((part) => ({
        fixture: part.fixture,
        attribute: part.attribute,
        value: part.value,
        presetRef: part.presetRef,
      })),
    })),
  }));
}

/** The same for the pools. */
function presetsAsRecorded(show: JsonValue | null): RecordedPreset[] {
  return presetRows(show).map((row) => ({
    id: row.id,
    pool: row.pool,
    name: row.name,
    color: row.color,
    values: row.values,
  }));
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the looks, read out of the show document", () => {
  it("says after every step exactly what the daemon says", () => {
    let documents = initialDocuments();
    recording.steps.forEach((step, index) => {
      for (const encoded of step.deltas) {
        documents = applyDelta(documents, deltaOf(encoded));
      }
      expect(
        sequencesAsRecorded(documents.show),
        `step ${String(index)} (${step.what}): the sequences`,
      ).toEqual(step.sequences.map((row) => ({ ...row })));
      expect(
        presetsAsRecorded(documents.show),
        `step ${String(index)} (${step.what}): the presets`,
      ).toEqual(step.presets.map((row) => ({ ...row })));
    });

    // And what the deltas built is what a client that never saw one is served.
    const end = snapshotOf(recording.finalSnapshot);
    expect(sequenceRows(documents.show)).toEqual(sequenceRows(end.show));
    expect(presetRows(documents.show)).toEqual(presetRows(end.show));
  });

  it("reads the cue list in force and the update state off the daemon's session", () => {
    // **S39's two fields, replayed rather than invented.** The recording carries
    // what a *fresh client's* snapshot said after every step, so these readers
    // are held to the daemon's answer and not to this interface's arithmetic —
    // which is the whole reason the update state is session state at all: a
    // second screen has to blink the same Update key.
    let documents = initialDocuments();
    expect(sequenceInForce(documents.session)).toBeNull();
    expect(cueEditInForce(documents.session)).toBeNull();

    recording.steps.forEach((step, index) => {
      for (const encoded of step.deltas) {
        documents = applyDelta(documents, deltaOf(encoded));
      }
      expect(
        sequenceInForce(documents.session),
        `step ${String(index)} (${step.what}): the cue list in force`,
      ).toEqual(step.selectedSequence);
      expect(
        cueEditInForce(documents.session),
        `step ${String(index)} (${step.what}): the update state`,
      ).toEqual(step.editingCue);
    });

    // And the script really did exercise both, or this would pass over a
    // recording in which nothing was ever selected or loaded.
    expect(recording.steps.some((step) => step.selectedSequence !== null)).toBe(true);
    expect(recording.steps.some((step) => step.editingCue?.modified === true)).toBe(true);
    expect(recording.steps.some((step) => step.editingCue?.modified === false)).toBe(true);
  });

  it("reads a preset link out of the cue rather than looking the preset up", () => {
    // The claim the whole session rests on, read from the daemon's own document:
    // the cue that was stored from an applied preset carries the reference on
    // every part, and when the preset was edited the **values** moved with it.
    const stored = stepAbout("its parts carry the preset reference");
    const edited = stepAbout("which is the whole claim");
    const cueOf = (step: { readonly sequences: readonly RecordedSequence[] }): RecordedCue => {
      const cue = step.sequences[0]?.cues.find((row) => row.number === "3");
      if (cue === undefined) {
        throw new Error("cue 3 is not in that step");
      }
      return cue;
    };
    expect(cueOf(stored).parts.every((part) => part.presetRef === 1)).toBe(true);
    const before = cueOf(stored).parts.map((part) => part.value);
    const after = cueOf(edited).parts.map((part) => part.value);
    expect(after).not.toEqual(before);
    expect(cueOf(edited).parts.every((part) => part.presetRef === 1)).toBe(true);
  });

  it("orders sequences and presets by number rather than by the key's text", () => {
    // The document keys them as strings, so 10 sorts before 2 unless somebody
    // has thought about it.
    const show: JsonValue = {
      sequences: {
        "10": { name: "Ten", cues: [], loop: false },
        "2": { name: "Two", cues: [], loop: true },
      },
      presets: {
        "10": { pool: "Color", name: "Ten", color: null, values: [] },
        "2": { pool: "Color", name: "Two", color: null, values: [] },
      },
    };
    expect(sequenceRows(show).map((row) => row.id)).toEqual([2, 10]);
    expect(presetRows(show).map((row) => row.id)).toEqual([2, 10]);
    expect(sequenceRows(show)[0]?.looping).toBe(true);
  });

  it("keeps the cues in the order the document holds them", () => {
    // Playback order is `Cue::compare_numbers`, applied by the daemon before it
    // writes. A client that sorted again would be a second opinion about what
    // `1.5` means — so this reader does not sort at all, and the recording is
    // what says the document is already in order.
    const numbers = recording.steps
      .flatMap((step) => step.sequences)
      .map((row) => row.cues.map((cue) => cue.number));
    expect(numbers.some((list) => list.includes("1.5"))).toBe(true);
    for (const list of numbers) {
      const sorted = [...list].sort((left, right) => Number(left) - Number(right));
      expect(list).toEqual(sorted);
    }
  });

  it("answers with nothing for a document that has no looks in it", () => {
    for (const show of [null, {}, { sequences: 7 }, { sequences: { "1": 7, x: {} } }]) {
      expect(sequenceRows(show as JsonValue)).toEqual([]);
    }
    for (const show of [null, {}, { presets: 7 }, { presets: { "1": 7, x: {} } }]) {
      expect(presetRows(show as JsonValue)).toEqual([]);
    }
    expect(sequenceRow(null, 1)).toBeNull();
    expect(sequenceRow({ sequences: {} }, null)).toBeNull();
    expect(executorsPlaying(null, 1)).toEqual([]);
  });

  it("reads a sequence whose cue list is not a list as one with no cues", () => {
    // A hand-edited show, or a daemon that changed shape. The sequence is still
    // in the pool, because an operator has to be able to see the thing that is
    // wrong rather than have it vanish.
    const rows = sequenceRows({ sequences: { "1": { name: "Broken", cues: 7 } } });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.cues).toEqual([]);
  });

  it("reads a cue whose fields the document is missing without inventing any", () => {
    const show: JsonValue = {
      sequences: {
        "1": {
          name: "Half a sequence",
          cues: [{ number: "1" }, 7, { number: "2", parts: [{ attribute: "Nonsense" }, 7] }],
        },
      },
    };
    const cues = sequenceRows(show)[0]?.cues ?? [];
    // The element that is not an object is left out; the one with no fields
    // reads as zeros and `Go`, which is what a cue with nothing set *is*.
    expect(cues.map((cue) => cue.number)).toEqual(["1", "2"]);
    expect(cues[0]?.trigger).toBe("Go");
    expect(cues[0]?.fadeIn).toBe(0);
    expect(cues[0]?.triggerTime).toBeNull();
    // And a part naming an attribute this build does not know is left out
    // rather than drawn as something else.
    expect(cues[1]?.parts).toEqual([]);
  });

  it("takes a preset's colour only when it is a whole colour", () => {
    const show: JsonValue = {
      presets: {
        "1": { pool: "Color", name: "Whole", color: { r: 1, g: 2, b: 3 }, values: [] },
        "2": { pool: "Color", name: "Half", color: { r: 1, g: 2 }, values: [] },
        "3": { pool: "Nonsense", name: "None", color: null, values: [{}, {}] },
      },
    };
    const rows = presetRows(show);
    expect(rows[0]?.color).toEqual({ r: 1, g: 2, b: 3 });
    // Half a colour is no colour: a swatch of a colour nobody chose is worse
    // than the plain box the pool draws without one.
    expect(rows[1]?.color).toBeNull();
    // A pool this build does not know reads as `FeatureGroup`'s own default,
    // so the preset is still somewhere an operator can find it.
    expect(rows[2]?.pool).toBe("Dimmer");
    expect(rows[2]?.values).toBe(2);
  });

  it("filters a pool without changing the numbering, which is shared", () => {
    // Preset numbers are unique **across** pools (`Show::store_preset`), because
    // `ApplyPreset` carries a number and no pool.
    const show: JsonValue = {
      presets: {
        "1": { pool: "Color", name: "A", color: null, values: [] },
        "2": { pool: "Position", name: "B", color: null, values: [] },
        "3": { pool: "Color", name: "C", color: null, values: [] },
      },
    };
    expect(poolRows(show, "Color").map((row) => row.id)).toEqual([1, 3]);
    expect(poolRows(show, "Position").map((row) => row.id)).toEqual([2]);
    expect(poolRows(show, "Beam")).toEqual([]);
    // The next free number is the next free one *anywhere*, not in the pool.
    expect(nextFreeNumber(presetRows(show))).toBe(4);
  });

  it("follows the selected executor, and says so when there is none", () => {
    // **The playback is the cue list's** (S45), so executors 0 and 1 — two
    // handles on list 5 — report the same thing, which is punch-list entry B18
    // as the transport line meets it.
    const show: JsonValue = {
      sequences: { "5": { name: "Act 1", isActive: true, currentCueIndex: null } },
      executors: {
        "0": { sequenceId: 5 },
        "1": { sequenceId: 5 },
        "2": { sequenceId: null },
      },
    };
    expect(executorInForce({ session: { selectedExecutor: 0 } }, show)).toEqual({
      executorId: 0,
      sequenceId: 5,
      isActive: true,
      currentCueIndex: null,
    });
    expect(executorInForce({ session: { selectedExecutor: 1 } }, show)).toEqual({
      executorId: 1,
      sequenceId: 5,
      isActive: true,
      currentCueIndex: null,
    });
    expect(executorInForce({ session: { selectedExecutor: 2 } }, show).sequenceId).toBeNull();
    expect(executorInForce({ session: {} }, show)).toEqual({
      executorId: null,
      sequenceId: null,
      isActive: false,
      currentCueIndex: null,
    });
    // A slot the show has no executor for at all reads as an empty one rather
    // than throwing: a session may name an executor a show has not got.
    expect(executorInForce({ session: { selectedExecutor: 40 } }, show).sequenceId).toBeNull();
    expect(executorsPlaying(show, 5)).toEqual([0, 1]);
  });

  it("draws the cue index the daemon reports, and a dash when there is none", () => {
    // **The inverse of what this test said until S34.** S26 recorded the gap —
    // what cue a playback is on lived on the tick thread with no channel back —
    // and S28 wrote this assertion so that closing it would be noticed. The
    // recording asserts the same thing in Rust
    // (`ui_show.rs::the_cue_index_is_a_number_now`), and both turned round
    // together.
    const executors = recording.steps.flatMap((step) => step.executors);
    expect(executors.length).toBeGreaterThan(0);
    // A running executor is on a cue and a stopped one is on none. Both halves
    // are in the script, so neither claim is vacuous.
    expect(
      executors.every((executor) => (executor.currentCueIndex !== null) === executor.isActive),
    ).toBe(true);
    expect(executors.some((executor) => executor.isActive)).toBe(true);
    expect(executors.some((executor) => !executor.isActive)).toBe(true);
    // And the number **moved**: a readback that reported a constant zero would
    // pass everything above.
    const seen = new Set(
      executors.map((executor) => executor.currentCueIndex).filter((index) => index !== null),
    );
    expect(seen.size).toBeGreaterThanOrEqual(2);
  });

  it("offers the next cue number without deciding whether it is free", () => {
    expect(nextCueNumber([])).toBe("1");
    expect(nextCueNumber(cuesNumbered(["1", "1.5", "2"]))).toBe("3");
    // A number that is not a number sorts nowhere and counts for nothing: the
    // daemon accepts one and this is only the box's starting text.
    expect(nextCueNumber(cuesNumbered(["1", "opening"]))).toBe("2");
    expect(nextFreeNumber([{ id: 1 }, { id: 3 }])).toBe(2);
  });

  it("says a time and a trigger the way a cue sheet does", () => {
    expect(secondsText(0)).toBe("0.0s");
    expect(secondsText(5.5)).toBe("5.5s");
    expect(triggerText(cuesNumbered(["1"])[0]!)).toBe("Go");
    expect(
      triggerText({ ...cuesNumbered(["1"])[0]!, trigger: "Time", triggerTime: 4 }),
    ).toBe("Time 4.0s");
    // A `Time` trigger with no time is the state an operator is halfway through
    // typing; it says what it is rather than inventing a zero.
    expect(triggerText({ ...cuesNumbered(["1"])[0]!, trigger: "Time" })).toBe("Time");
  });

  it("turns a preset colour into a CSS value, and nothing into nothing", () => {
    expect(colorStyle({ r: 0, g: 0, b: 255 })).toBe("rgb(0 0 255)");
    expect(colorStyle(null)).toBeNull();
  });
});

/** Cues with nothing in them but numbers. */
function cuesNumbered(numbers: readonly string[]) {
  return numbers.map((number) => ({
    number,
    name: "",
    fadeIn: 0,
    fadeOut: 0,
    delay: 0,
    trigger: "Go" as const,
    triggerTime: null,
    parts: [],
  }));
}
