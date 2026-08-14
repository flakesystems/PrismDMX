/**
 * The console parser, held to a daemon's answers and to one absolute rule.
 *
 * # Where the expectations come from
 *
 * `ui/tests/fixtures/desk-recording.json` carries every line an operator typed
 * in the recorded script **beside the commands a real `prismd` accepted for
 * it**, and `crates/prismd/tests/ui_programmer.rs` asserts that those payloads
 * are the commands the script names. So the central test here decodes the
 * recorded bytes and compares the parser's answer against them. Nothing in
 * TypeScript decides what `1 thru 3 at 50` ought to mean.
 *
 * # And the rule
 *
 * **It never throws.** Not for a typo, not for a control character, not for a
 * hundred-thousand-fixture range. That is S26's third exit criterion, and it is
 * checked here over ten thousand generated strings rather than over the six a
 * person would think of.
 */

import { decode } from "@msgpack/msgpack";
import { describe, expect, it } from "vitest";

import type { Command } from "../bindings";
import { parseCommandLine, readingText, MAX_RANGE } from "./console";

import recordingText from "../../tests/fixtures/desk-recording.json?raw";

interface Recording {
  readonly steps: readonly {
    readonly typed: string | null;
    readonly typedLine: number | null;
    readonly client: string;
  }[];
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

/** The command inside a recorded client payload. */
function recordedCommand(encoded: string): Command {
  const message = decode(payload(encoded));
  if (
    typeof message !== "object" ||
    message === null ||
    !("command" in message) ||
    (message as { t?: unknown }).t !== "Command"
  ) {
    throw new Error("that payload is not a command");
  }
  const { command } = message as { command: Command };
  return command;
}

/**
 * The typed lines of the recording, each with the commands the daemon was sent
 * for it.
 *
 * Grouped by the recorded line **number** rather than by equal text: `clear` is
 * pressed three times in a row and those are three lines, not one line with
 * three commands on it. `crates/prismd/tests/ui_programmer.rs` asserts that a
 * line's steps are consecutive and that no number is reused, so this grouping
 * is the one the recorder meant.
 */
function typedLines(): { line: string; commands: Command[] }[] {
  const lines: { number: number; line: string; commands: Command[] }[] = [];
  for (const step of recording.steps) {
    if (step.typed === null || step.typedLine === null) {
      continue;
    }
    const last = lines.at(-1);
    if (last !== undefined && last.number === step.typedLine) {
      last.commands.push(recordedCommand(step.client));
    } else {
      lines.push({
        number: step.typedLine,
        line: step.typed,
        commands: [recordedCommand(step.client)],
      });
    }
  }
  return lines;
}

describe("what a line means", () => {
  /**
   * **The central test.** Every line the recording carries, against the
   * commands a running daemon was sent for it.
   *
   * The `clear` line appears three times in the script — the button is a
   * three-stage machine and the stages differ — and the parser answers the same
   * way each time, which is right: the stage is the *daemon's*, and a client
   * that tracked it would be holding programmer state.
   */
  it("produces the commands the daemon was sent for that line", () => {
    const lines = typedLines();
    expect(lines.length).toBeGreaterThanOrEqual(6);
    for (const { line, commands } of lines) {
      const reading = parseCommandLine(line);
      expect(reading, `"${line}" did not parse`).toEqual({ kind: "commands", commands });
    }
    // And one of them is a line that produced two commands, which a parser
    // answering with a single command could never have passed.
    expect(lines.some((entry) => entry.commands.length === 2)).toBe(true);
  });

  it("reads a range in the direction it is written, and does not repeat a fixture", () => {
    expect(parseCommandLine("1 thru 3")).toEqual({
      kind: "commands",
      commands: [{ t: "SelectFixtures", ids: [1, 2, 3], mode: "Set" }],
    });
    // Downwards, because selection order is what an operator sees when a value
    // is fanned across it.
    expect(parseCommandLine("3 thru 1")).toEqual({
      kind: "commands",
      commands: [{ t: "SelectFixtures", ids: [3, 2, 1], mode: "Set" }],
    });
    expect(parseCommandLine("1 thru 3 + 2")).toEqual({
      kind: "commands",
      commands: [{ t: "SelectFixtures", ids: [1, 2, 3], mode: "Set" }],
    });
    expect(parseCommandLine("1 thru 1")).toEqual({
      kind: "commands",
      commands: [{ t: "SelectFixtures", ids: [1], mode: "Set" }],
    });
  });

  it("does not need spaces, and does not mind case", () => {
    // A console keypad has digits and a few keys; hunting for a space bar in
    // the dark is not part of the job.
    const spaced = parseCommandLine("1 thru 3 + 5 at 50");
    expect(parseCommandLine("1THRU3,5 AT 50")).toEqual(spaced);
    expect(parseCommandLine("  1 thru 3 + 5 at 50  ")).toEqual(spaced);
  });

  it("takes the word `fixture` as the noise it is", () => {
    expect(parseCommandLine("fixture 4")).toEqual(parseCommandLine("4"));
    expect(parseCommandLine("fixtures 4")).toEqual(parseCommandLine("4"));
  });

  it("names an attribute on either side of `at`, and defaults to the dimmer", () => {
    const pan = { t: "SetAttribute", attribute: "Pan", value: 16383, relative: false };
    expect(parseCommandLine("5 pan at 25")).toEqual({
      kind: "commands",
      commands: [{ t: "SelectFixtures", ids: [5], mode: "Set" }, pan],
    });
    expect(parseCommandLine("5 at pan 25")).toEqual({
      kind: "commands",
      commands: [{ t: "SelectFixtures", ids: [5], mode: "Set" }, pan],
    });
    // With no fixtures, which is the ordinary way to trim what is selected.
    expect(parseCommandLine("pan at 25")).toEqual({ kind: "commands", commands: [pan] });
    expect(parseCommandLine("at 50")).toEqual({
      kind: "commands",
      commands: [{ t: "SetAttribute", attribute: "Dimmer", value: 32767, relative: false }],
    });
  });

  it("knows the three words an operator says instead of a number", () => {
    const full = { t: "SetAttribute", attribute: "Dimmer", value: 65535, relative: false };
    const out = { t: "SetAttribute", attribute: "Dimmer", value: 0, relative: false };
    expect(parseCommandLine("at full")).toEqual({ kind: "commands", commands: [full] });
    expect(parseCommandLine("at out")).toEqual({ kind: "commands", commands: [out] });
    expect(parseCommandLine("at zero")).toEqual({ kind: "commands", commands: [out] });
    expect(parseCommandLine("at 100")).toEqual({ kind: "commands", commands: [full] });
    expect(parseCommandLine("at 0")).toEqual({ kind: "commands", commands: [out] });
  });

  it("truncates a percentage, so nothing rounds up past what was asked for", () => {
    const level = (line: string): number => {
      const reading = parseCommandLine(line);
      if (reading.kind !== "commands") {
        throw new Error(reading.kind);
      }
      const [command] = reading.commands;
      if (command?.t !== "SetAttribute") {
        throw new Error("not a level");
      }
      return command.value;
    };
    expect(level("at 50")).toBe(32767);
    expect(level("at 25")).toBe(16383);
    expect(level("at 33.3")).toBe(21823);
    expect(level("at 100")).toBe(65535);
    expect(level("at 0")).toBe(0);
  });

  it("reads the executor and page words", () => {
    expect(parseCommandLine("go 3")).toEqual({
      kind: "commands",
      commands: [{ t: "ExecutorGo", executorId: 3, direction: "Next" }],
    });
    expect(parseCommandLine("go+ 3")).toEqual(parseCommandLine("go 3"));
    expect(parseCommandLine("go- 3")).toEqual({
      kind: "commands",
      commands: [{ t: "ExecutorGo", executorId: 3, direction: "Prev" }],
    });
    expect(parseCommandLine("off 3")).toEqual({
      kind: "commands",
      commands: [{ t: "ExecutorOff", executorId: 3 }],
    });
    expect(parseCommandLine("page 4")).toEqual({
      kind: "commands",
      commands: [{ t: "SetExecutorPage", page: 4 }],
    });
    expect(parseCommandLine("clear")).toEqual({
      kind: "commands",
      commands: [{ t: "ClearProgrammer" }],
    });
  });

  it("does not validate against the show, because it cannot", () => {
    // Fixture 9 is not in the rig, and the daemon refused this very line in the
    // recording. The parser is right and the refusal is the daemon's — that
    // split is D3, and it is why a console must not consult the patch.
    expect(parseCommandLine("9")).toEqual({
      kind: "commands",
      commands: [{ t: "SelectFixtures", ids: [9], mode: "Set" }],
    });
  });
});

describe("a line that is not one", () => {
  it("says what is wrong and sends nothing", () => {
    const message = (line: string): string => {
      const reading = parseCommandLine(line);
      if (reading.kind !== "error") {
        throw new Error(`"${line}" parsed: ${JSON.stringify(reading)}`);
      }
      return reading.message;
    };
    expect(message("at")).toContain("percentage");
    expect(message("at 200")).toContain("outside 0 to 100");
    expect(message("at -1")).toContain("outside 0 to 100");
    expect(message("at banana")).toContain("not a percentage");
    expect(message("at 50 60")).toContain("more than a level");
    expect(message("1 thru")).toContain("thru what");
    expect(message("1 thru banana")).toContain("not a fixture number");
    expect(message("1 +")).toContain("something is missing");
    expect(message("banana")).toContain("not a fixture number");
    expect(message("fixture")).toContain("not a command");
    expect(message("go")).toContain("which executor");
    expect(message("go banana")).toContain("not an executor number");
    expect(message("go 1 2")).toContain("more than go takes");
    expect(message("go- 1 2")).toContain("more than go- takes");
    expect(message("page")).toContain("which page");
    expect(message("page banana")).toContain("not a page number");
    expect(message("page 1 2")).toContain("more than page takes");
    expect(message("clear everything")).toContain("more than clear takes");
    expect(message("1 2")).toContain("Separate fixtures");
    expect(message(`1 thru ${String(MAX_RANGE + 2)}`)).toContain(String(MAX_RANGE));
  });

  it("says nothing at all about an empty line", () => {
    for (const line of ["", "   ", "\t\n"]) {
      expect(parseCommandLine(line)).toEqual({ kind: "empty" });
    }
  });

  /**
   * **The exit criterion.** Ten thousand generated lines, and not one throw.
   *
   * The alphabet is deliberately made of the pieces a real line is made of —
   * keywords, digits, separators — plus the things that break parsers: control
   * characters, huge numbers, lone separators. A console that threw would take
   * the interface down over a typo in the middle of a show.
   */
  it("never throws, for any line there is", () => {
    const alphabet = [
      "1",
      "0",
      "999999999999999999999",
      "-1",
      "1.5",
      "thru",
      "+",
      ",",
      "at",
      "full",
      "out",
      "clear",
      "go",
      "go-",
      "off",
      "page",
      "fixture",
      "pan",
      "dimmer",
      "banana",
      "",
      " ",
      " ",
      "",
      "🌈",
      "NaN",
      "Infinity",
      "1e400",
      "0x10",
      "01",
    ];
    // A deterministic generator, so a failure is reproducible: a seeded
    // xorshift rather than `Math.random`, which would make a red run unusable.
    let seed = 0x5eed_1234;
    const next = (): number => {
      seed ^= seed << 13;
      seed ^= seed >>> 17;
      seed ^= seed << 5;
      return Math.abs(seed);
    };
    for (let attempt = 0; attempt < 10_000; attempt += 1) {
      const words: string[] = [];
      for (let word = 0; word < next() % 7; word += 1) {
        words.push(alphabet[next() % alphabet.length] ?? "");
      }
      const line = words.join(next() % 2 === 0 ? " " : "");
      const reading = parseCommandLine(line);
      expect(["empty", "commands", "error"], line).toContain(reading.kind);
      // And whatever it answered can be shown to somebody.
      expect(typeof readingText(reading)).toBe("string");
    }
  });

  it("never builds an enormous selection, however big the range", () => {
    // `1 thru 999999999` is a typo, and building that array is how a console
    // stops answering. The daemon would refuse it anyway; the point is to say
    // so before the browser has spent a second on it.
    const started = Date.now();
    expect(parseCommandLine("1 thru 999999999").kind).toBe("error");
    expect(Date.now() - started).toBeLessThan(1000);
  });
});

describe("what the line says it will do", () => {
  it("reads a parsed line back in words", () => {
    expect(readingText(parseCommandLine("1 thru 3 at 50"))).toBe("select 1 + 2 + 3 · dimmer → 50%");
    expect(readingText(parseCommandLine("clear"))).toBe("clear");
    expect(readingText(parseCommandLine("go 2"))).toBe("executor 2 go");
    expect(readingText(parseCommandLine("go- 2"))).toBe("executor 2 back");
    expect(readingText(parseCommandLine("off 2"))).toBe("executor 2 off");
    expect(readingText(parseCommandLine("page 2"))).toBe("page 2");
    expect(readingText(parseCommandLine(""))).toBe("");
    expect(readingText(parseCommandLine("banana"))).toContain("not a fixture number");
  });

  it("has an arm for a command it was not written for", () => {
    // `Command` is the whole protocol and this readout is not where a new one
    // should become a compile error — but it must still say *something*.
    expect(readingText({ kind: "commands", commands: [{ t: "SaveShow" }] })).toBe("SaveShow");
  });
});
