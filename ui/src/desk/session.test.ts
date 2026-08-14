/**
 * The desk readers, held to the daemon's own answers.
 *
 * `ui/tests/fixtures/desk-recording.json` records, after every command of a
 * script a real `prismd` executed, **what is on the eight strips of the current
 * page, what the programmer holds, and the six session fields the bars read** —
 * taken from a fresh client's snapshot, and checked in Rust against
 * `prism_core`'s mirrors on every commit
 * (`crates/prismd/tests/ui_programmer.rs`).
 *
 * So this file replays the recorded deltas through *this interface's* mirror
 * and compares its readers' answers with those. Nothing in TypeScript decides
 * what the answer should be, which is the trap every session from S19 onwards
 * has found in its own layer.
 */

import { decode } from "@msgpack/msgpack";
import { beforeEach, describe, expect, it } from "vitest";

import type { AttributeType, FeatureGroup } from "../bindings";
import { readServerMessage } from "../ipc/protocol";
import type { Snapshot } from "../ipc/protocol";
import { nullSink, setLogSink } from "../log/logger";
import { DeskStore } from "../store/desk";
import { bankParameters, touchedBanks, valueFor } from "./programmer";
import {
  EXECUTORS_PER_PAGE,
  commandLine,
  encoderBank,
  executorIdAt,
  executorPage,
  pageStrips,
  programmerPage,
  programmerParamIndex,
  selectedExecutor,
} from "./session";

import recordingText from "../../tests/fixtures/desk-recording.json?raw";

/** One strip, as the daemon recorded it. */
interface RecordedStrip {
  readonly slot: number;
  readonly executorId: number;
  readonly assigned: boolean;
  readonly name: string | null;
  readonly masterLevel: number;
  readonly isActive: boolean;
  readonly currentCueIndex: number | null;
  readonly faderFunction: string;
  readonly buttonFunctions: readonly string[];
}

interface RecordedProgrammer {
  readonly selection: readonly number[];
  readonly activeFeatureGroup: string;
  readonly values: readonly [number, string, number, string][];
  readonly clearStage: number;
  readonly touchedBanks: readonly string[];
}

interface RecordedSession {
  readonly executorPage: number;
  readonly selectedExecutor: number | null;
  readonly encoderBank: string;
  readonly programmerPage: number;
  readonly programmerParamIndex: number;
  readonly commandLine: string;
}

interface Recording {
  readonly executorsPerPage: number;
  readonly encoderBanks: Readonly<Record<string, readonly string[]>>;
  readonly initialSnapshot: string;
  readonly steps: readonly {
    readonly what: string;
    readonly deltas: readonly string[];
    readonly refused: boolean;
    readonly strips: readonly RecordedStrip[];
    readonly programmer: RecordedProgrammer;
    readonly session: RecordedSession;
  }[];
  readonly finalSnapshot: string;
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

/** The snapshot the recorded script starts from. */
function recordedSnapshot(encoded: string): Snapshot {
  const message = readServerMessage(decode(payload(encoded)));
  if (message.t !== "Snapshot") {
    throw new Error("that payload is not a snapshot");
  }
  return message.snapshot;
}

/** A store with the recording's opening snapshot in it. */
function opened(): DeskStore {
  const store = new DeskStore();
  store.applySnapshot(recordedSnapshot(recording.initialSnapshot));
  return store;
}

/** Applies one step's deltas. */
function apply(store: DeskStore, step: number): void {
  const entry = recording.steps[step];
  if (entry === undefined) {
    throw new Error(`the recording has no step ${String(step)}`);
  }
  for (const encoded of entry.deltas) {
    const message = readServerMessage(decode(payload(encoded)));
    if (message.t !== "Delta") {
      throw new Error("that payload is not a delta");
    }
    expect(store.applyDelta(message.delta), `step ${String(step)} did not fit`).toBe(true);
  }
}

/** The three documents, which are there because a snapshot was applied. */
function documentsOf(store: DeskStore) {
  const documents = store.getState().documents;
  if (documents === null) {
    throw new Error("the store has no documents");
  }
  return documents;
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the readers, against a daemon's answers", () => {
  /**
   * **The central test.** Every step of the recorded script, every answer.
   *
   * The strips, the programmer's banks, and the six session fields — after each
   * of twenty-three commands, including three the daemon refused, where the
   * answer is that nothing moved.
   */
  it("answers what the daemon says the desk is, at every step", () => {
    const store = opened();
    for (let step = 0; step < recording.steps.length; step += 1) {
      const entry = recording.steps[step];
      if (entry === undefined) {
        throw new Error("no step");
      }
      apply(store, step);
      const { session, show, programmer } = documentsOf(store);
      const where = `step ${String(step)} (${entry.what})`;

      // The session fields the bars read.
      expect(executorPage(session), where).toBe(entry.session.executorPage);
      expect(selectedExecutor(session), where).toBe(entry.session.selectedExecutor);
      expect(encoderBank(session), where).toBe(entry.session.encoderBank);
      expect(programmerPage(session), where).toBe(entry.session.programmerPage);
      expect(programmerParamIndex(session), where).toBe(entry.session.programmerParamIndex);
      expect(commandLine(session), where).toBe(entry.session.commandLine);

      // The eight strips — D7, and the arithmetic that could address the
      // wrong executor.
      const strips = pageStrips(session, show);
      expect(strips.length, where).toBe(EXECUTORS_PER_PAGE);
      expect(
        strips.map((strip) => ({
          slot: strip.slot,
          executorId: strip.executorId,
          assigned: strip.assigned,
          name: strip.name,
          masterLevel: strip.masterLevel,
          isActive: strip.isActive,
          currentCueIndex: strip.currentCueIndex,
          faderFunction: strip.faderFunction ?? "",
          buttonFunctions: strip.buttonFunctions,
        })),
        where,
      ).toEqual(entry.strips);

      // The programmer, read the way the encoder bar reads it.
      expect(programmer.selection, where).toEqual(entry.programmer.selection);
      expect(programmer.clearStage, where).toBe(entry.programmer.clearStage);
      expect(touchedBanks(programmer, show), where).toEqual(entry.programmer.touchedBanks);
      for (const [fixture, attribute, value] of entry.programmer.values) {
        expect(valueFor(programmer, fixture, attributeOf(attribute)), `${where} ${attribute}`).toBe(
          value,
        );
      }
      expect(programmer.values.length, where).toBe(entry.programmer.values.length);
    }
  });

  /** The recording's own figure for **D7**, against the one written here. */
  it("has the daemon's number of executors per page", () => {
    expect(EXECUTORS_PER_PAGE).toBe(recording.executorsPerPage);
    expect(executorIdAt(0, 0)).toBe(0);
    expect(executorIdAt(1, 1)).toBe(9);
    expect(executorIdAt(3, 7)).toBe(31);
  });

  /**
   * **The encoder bar and the jog wheel walk one list.**
   *
   * The recording carries the banks as `prismd::surface::parameter_of` resolves
   * them, and the interface's table is generated from the same Rust function
   * (`FeatureGroup::attributes`). This is the comparison: if they ever differed,
   * the wheel would turn a parameter other than the one highlighted here, and
   * nobody would think to blame a table.
   */
  it("orders each bank's parameters the way the jog wheel walks them", () => {
    const banks = Object.entries(recording.encoderBanks);
    expect(banks.length).toBe(5);
    for (const [bank, attributes] of banks) {
      expect(bankParameters(groupOf(bank))).toEqual(attributes);
    }
  });

  it("says the daemon's own final state, from the deltas alone", () => {
    // The whole script replayed, then compared against the snapshot a client
    // that never saw a delta is served.
    const store = opened();
    for (let step = 0; step < recording.steps.length; step += 1) {
      apply(store, step);
    }
    const final = recordedSnapshot(recording.finalSnapshot);
    const { session, show } = documentsOf(store);
    expect(pageStrips(session, show)).toEqual(pageStrips(final.session, final.show));
    expect(encoderBank(session)).toBe(encoderBank(final.session));
    expect(commandLine(session)).toBe(commandLine(final.session));
  });

  it("shows a refused command as nothing having moved", () => {
    const refusals = recording.steps
      .map((step, index) => ({ step, index }))
      .filter(({ step }) => step.refused);
    expect(refusals.length).toBeGreaterThanOrEqual(3);
    for (const { step, index } of refusals) {
      expect(step.deltas, `step ${String(index)} said something`).toEqual([]);
      const before = recording.steps[index - 1];
      expect(before?.strips).toEqual(step.strips);
      expect(before?.session).toEqual(step.session);
      expect(before?.programmer).toEqual(step.programmer);
    }
  });
});

describe("a document that is not one", () => {
  it("answers with an empty desk rather than throwing", () => {
    // A view asking a document that has gone is an ordinary state — the store
    // drops its documents when the daemon goes — and a bar that threw would
    // take the command line with it.
    expect(executorPage(null)).toBe(0);
    expect(selectedExecutor(null)).toBeNull();
    expect(encoderBank(null)).toBe("Dimmer");
    expect(programmerPage(null)).toBe(0);
    expect(programmerParamIndex(null)).toBe(0);
    expect(commandLine(null)).toBe("");
    const strips = pageStrips(null, null);
    expect(strips.length).toBe(EXECUTORS_PER_PAGE);
    expect(strips.every((strip) => !strip.assigned)).toBe(true);
    expect(touchedBanks(null, null)).toEqual([]);
    expect(valueFor(null, 1, "Dimmer")).toBeNull();
  });

  it("keeps a page of eight even when the show is nonsense", () => {
    const show = { executors: { "0": 5, "1": { masterLevel: "loud" } } };
    const session = { session: { executorPage: 0 } };
    const strips = pageStrips(session, show);
    expect(strips.length).toBe(EXECUTORS_PER_PAGE);
    // An executor that is a number is not an executor.
    expect(strips[0]?.assigned).toBe(false);
    // One whose level is a word is assigned, at zero, with nothing on it —
    // every field is read on its own, so one bad member does not lose a strip.
    expect(strips[1]?.assigned).toBe(true);
    expect(strips[1]?.masterLevel).toBe(0);
    expect(strips[1]?.faderFunction).toBeNull();
    expect(strips[1]?.buttonFunctions).toEqual([]);
  });

  it("leaves out a button function this build has never heard of", () => {
    const show = {
      executors: { "0": { masterLevel: 0, buttonFunctions: ["Go+", "Hologram", 7, "Off"] } },
    };
    expect(pageStrips({ session: { executorPage: 0 } }, show)[0]?.buttonFunctions).toEqual([
      "Go+",
      "Off",
    ]);
  });

  it("falls back to the first bank when the session names one it does not know", () => {
    expect(encoderBank({ session: { encoderBank: "Ultraviolet" } })).toBe("Dimmer");
    expect(encoderBank({ session: { encoderBank: 4 } })).toBe("Dimmer");
  });
});

/** A recorded attribute name as the type. */
function attributeOf(name: string): AttributeType {
  return bankNames().attributes(name);
}

/** A recorded bank name as the type. */
function groupOf(name: string): FeatureGroup {
  return bankNames().groups(name);
}

/**
 * Names out of the generated tables.
 *
 * A test file is allowed to know what it just read out of a fixture, but it is
 * not allowed to invent the vocabulary: both narrowings go through the same
 * tables the interface narrows a decoded message with.
 */
function bankNames(): {
  readonly attributes: (name: string) => AttributeType;
  readonly groups: (name: string) => FeatureGroup;
} {
  return {
    attributes: (name) => {
      for (const bank of Object.keys(recording.encoderBanks)) {
        for (const attribute of bankParameters(groupOf(bank))) {
          if (attribute === name) {
            return attribute;
          }
        }
      }
      throw new Error(`${name} is not an attribute this build knows`);
    },
    groups: (name) => {
      for (const group of ["Dimmer", "Position", "Color", "Beam", "Focus"] as const) {
        if (group === name) {
          return group;
        }
      }
      throw new Error(`${name} is not a feature group this build knows`);
    },
  };
}
