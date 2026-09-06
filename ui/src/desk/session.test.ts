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
import { FEATURE_GROUP_VARIANTS } from "../bindings";
import { FEATURE_GROUP_ATTRIBUTES } from "../bindings/variants";
import { readServerMessage } from "../ipc/protocol";
import type { Snapshot } from "../ipc/protocol";
import { nullSink, setLogSink } from "../log/logger";
import { DeskStore } from "../store/desk";
import { touchedBanks, valueFor } from "./programmer";
import {
  EXECUTOR_BUTTONS,
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
  readonly faderLevel: number | null;
  readonly isActive: boolean;
  readonly currentCueIndex: number | null;
  readonly faderFunction: string;
  readonly encoderFunction: string;
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
  readonly executorButtons: number;
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
          faderLevel: strip.faderLevel,
          isActive: strip.isActive,
          currentCueIndex: strip.currentCueIndex,
          faderFunction: strip.faderFunction ?? "",
          encoderFunction: strip.encoderFunction ?? "",
          // The custom row reads as the line it sends, in braces — the shape
          // `prismd`'s recorder writes, so the two say the same thing.
          buttonFunctions: strip.buttonFunctions.map((fn) =>
            typeof fn === "string" ? fn : `{${fn.CommandLine.line}}`,
          ),
        })),
        where,
      ).toEqual(entry.strips);

      // The programmer, read the way the encoder bar reads it.
      expect(programmer.selection, where).toEqual(entry.programmer.selection);
      expect(programmer.clearStage, where).toBe(entry.programmer.clearStage);
      expect(touchedBanks(programmer, show), where).toEqual(entry.programmer.touchedBanks);
      for (const [fixture, attribute, value] of entry.programmer.values) {
        expect(
          valueFor(programmer, fixture, {
            attribute: attributeOf(attribute),
            occurrence: 0,
          }),
          `${where} ${attribute}`,
        ).toBe(value);
      }
      expect(programmer.values.length, where).toBe(entry.programmer.values.length);
    }
  });

  /** The recording's own figure for **D7**, against the one written here. */
  it("has the daemon's number of executors per page", () => {
    expect(EXECUTORS_PER_PAGE).toBe(recording.executorsPerPage);
    // S45: the control editor draws one row per key, and a client that assumed a
    // different number would offer a key `Show::configure_executor` refuses.
    expect(EXECUTOR_BUTTONS).toBe(recording.executorButtons);
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
    // **Seven since S43**, when Gobo and Control came out of Beam and got banks
    // of their own — one per Encoder Assign key on the X-Touch, which is what
    // makes every bank reachable in one press. The count is written out rather
    // than read off the recording for the reason this whole file exists: a
    // number that came from the thing under test proves nothing.
    expect(banks.length).toBe(7);
    for (const [bank, attributes] of banks) {
      // **The table, not the filtered list** — S52. `bankParameters` answers a
      // question about the *selection* now; what this holds to the daemon is
      // the table both sides filter, which is the order S26 made one of.
      expect(FEATURE_GROUP_ATTRIBUTES[groupOf(bank)]).toEqual(attributes);
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
    expect(valueFor(null, 1, { attribute: "Dimmer", occurrence: 0 })).toBeNull();
  });

  it("keeps a page of eight even when the show is nonsense", () => {
    const show = { executors: { "0": 5, "1": { faderFunction: "loud" } } };
    const session = { session: { executorPage: 0 } };
    const strips = pageStrips(session, show);
    expect(strips.length).toBe(EXECUTORS_PER_PAGE);
    // An executor that is a number is not an executor.
    expect(strips[0]?.assigned).toBe(false);
    // One whose level is a word is assigned, at zero, with nothing on it —
    // every field is read on its own, so one bad member does not lose a strip.
    expect(strips[1]?.assigned).toBe(true);
    expect(strips[1]?.faderLevel).toBeNull();
    expect(strips[1]?.faderFunction).toBeNull();
    expect(strips[1]?.buttonFunctions).toEqual([]);
  });

  /**
   * The colour is the sequence's, and it is read channel by channel — a mirror
   * one delta behind a schema change is the shape S26 wrote the *do not read
   * the show* rule about, so a colour this build cannot make sense of is no
   * colour rather than a broken strip.
   */
  it("reads a cue list's colour as hex, and refuses to guess at a broken one", () => {
    const session = { session: { executorPage: 0 } };
    const show = {
      executors: {
        "0": { masterLevel: 0, sequenceId: 1 },
        "1": { masterLevel: 0, sequenceId: 2 },
        "2": { masterLevel: 0, sequenceId: 3 },
        "3": { masterLevel: 0, sequenceId: 4 },
        "4": { masterLevel: 0 },
      },
      sequences: {
        "1": { name: "Warm", color: { r: 255, g: 136, b: 0 } },
        // Every channel at its ends, so the two-digit padding is exercised in
        // both directions: `#00ff0f` and not `#0ff0f` or `#0FF0F`.
        "2": { name: "Edges", color: { r: 0, g: 255, b: 15 } },
        "3": { name: "Uncoloured", color: null },
        "4": { name: "Nonsense", color: { r: "warm", g: 0, b: 0 } },
      },
    };
    const strips = pageStrips(session, show);
    expect(strips[0]?.color).toBe("#ff8800");
    expect(strips[1]?.color).toBe("#00ff0f");
    expect(strips[2]?.color).toBeNull();
    expect(strips[3]?.color).toBeNull();
    // An executor with no cue list on it has no colour either: the colour
    // belongs to the list, not to the place it is standing in.
    expect(strips[4]?.color).toBeNull();
    expect(strips[5]?.color).toBeNull();
  });

  /**
   * **A position, not a filtered list** — S45. An entry this build cannot read
   * has to leave its place behind: dropping it would move every key after it
   * one to the left, and the third key would send what the fourth was bound to.
   */
  it("draws a button function this build has never heard of as an empty key", () => {
    const show = {
      executors: { "0": { buttonFunctions: ["Go+", "Hologram", 7, "Off"] } },
    };
    expect(pageStrips({ session: { executorPage: 0 } }, show)[0]?.buttonFunctions).toEqual([
      "Go+",
      "Empty",
      "Empty",
      "Off",
    ]);
  });

  /** S45's custom row, read out of a mirrored document. */
  it("reads a key that carries a command line, and refuses a broken one", () => {
    const show = {
      executors: {
        "0": {
          buttonFunctions: [
            { CommandLine: { line: "Go+ Sequence 3" } },
            { CommandLine: { line: 7 } },
            { CommandLine: "Go+" },
          ],
        },
      },
    };
    expect(pageStrips({ session: { executorPage: 0 } }, show)[0]?.buttonFunctions).toEqual([
      { CommandLine: { line: "Go+ Sequence 3" } },
      "Empty",
      "Empty",
    ]);
  });

  /**
   * **B18, as the strip reads it.** Two executors on one cue list read the same
   * level, because there is one number and both of them point at it; a strip
   * whose fader is a `Speed` reads the list's rate instead, and a crossfade
   * reads nought because where it stands is a gesture rather than show state.
   */
  it("reads the number the strip's own fader function names, off the cue list", () => {
    const show = {
      sequences: { "1": { name: "Act 1", masterLevel: 20000, speed: 2048 } },
      executors: {
        "0": { sequenceId: 1, faderFunction: "Master" },
        "1": { sequenceId: 1, faderFunction: "Master" },
        "2": { sequenceId: 1, faderFunction: "Speed" },
        "3": { sequenceId: 1, faderFunction: "XFade" },
        "4": { sequenceId: 1, faderFunction: "Empty" },
        "5": { faderFunction: "Master" },
      },
    };
    const strips = pageStrips({ session: { executorPage: 0 } }, show);
    expect(strips[0]?.faderLevel).toBe(20000);
    expect(strips[1]?.faderLevel).toBe(20000);
    expect(strips[2]?.faderLevel).toBe(2048);
    // **`null` and not nought, since S51** (B36). A crossfade fader has no
    // number the desk may write: where it stands is the operator's hand, and a
    // nought here is what put it back at the bottom after every movement.
    expect(strips[3]?.faderLevel).toBeNull();
    expect(strips[5]?.faderLevel).toBeNull();
    expect(strips[4]?.faderLevel).toBeNull();
    // A slot with no cue list on it has no number to read at all.
    expect(strips[5]?.sequenceId).toBeNull();
  });

  /**
   * **Both crossfade modes read as *the hand's*** — S51, B36.
   *
   * The two are one rule (`ExecutorFaderFunction::desk_may_move_it`) and the
   * screen asks it the same way the desk does, so a mode added later has to
   * decide which side of the line it is on rather than defaulting to *the desk
   * may move it*.
   */
  it("gives neither crossfade a number the desk could write back", () => {
    const show = {
      sequences: { "1": { name: "Act 1", masterLevel: 20000, speed: 2048 } },
      executors: {
        "0": { sequenceId: 1, faderFunction: "XFade" },
        "1": { sequenceId: 1, faderFunction: "Fade" },
        "2": { sequenceId: 1, faderFunction: "Master" },
      },
    };
    const strips = pageStrips({ session: { executorPage: 0 } }, show);
    expect(strips[0]?.faderLevel).toBeNull();
    expect(strips[1]?.faderLevel).toBeNull();
    expect(strips[2]?.faderLevel).toBe(20000);
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
        for (const attribute of FEATURE_GROUP_ATTRIBUTES[groupOf(bank)]) {
          if (attribute === name) {
            return attribute;
          }
        }
      }
      throw new Error(`${name} is not an attribute this build knows`);
    },
    // **Out of the generated table since S43**, which is what the paragraph
    // above already asked for and this half was not doing: it was five names
    // written by hand, so the day Gobo and Control became banks of their own the
    // recording carried seven and this threw. A generated list cannot go stale
    // that way — and it still narrows rather than asserts, so a name the build
    // has never heard of is a failure and not a cast.
    groups: (name) => {
      for (const group of FEATURE_GROUP_VARIANTS) {
        if (group === name) {
          return group;
        }
      }
      throw new Error(`${name} is not a feature group this build knows`);
    },
  };
}
