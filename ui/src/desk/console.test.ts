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
import { applyMode, parseCommandLine, readingText, MAX_RANGE } from "./console";

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
    // S26 recorded six shapes; S40's script carries the whole vocabulary, so
    // this is a floor rather than a count.
    expect(lines.length).toBeGreaterThanOrEqual(30);
    for (const { line, commands } of lines) {
      const reading = parseCommandLine(line);
      if (reading.kind !== "commands") {
        throw new Error(`"${line}" did not parse: ${JSON.stringify(reading)}`);
      }
      // The **commands**, not the whole answer: a line that carries a mode also
      // carries the question the interface may have to ask about it, and the
      // recording holds what the daemon was sent rather than what was asked.
      expect(reading.commands, `"${line}" means something else`).toEqual(commands);
    }
    // And one of them is a line that produced two commands, which a parser
    // answering with a single command could never have passed.
    expect(lines.some((entry) => entry.commands.length === 2)).toBe(true);
  });

  /**
   * **Every word of the vocabulary is in the recording**, so the test above is
   * a check on all of it rather than on the six shapes S26 had.
   *
   * `IMPLEMENTATION_PLAN.md` S40 lists the lines this session had to make work;
   * this is that list, held to a script a **real daemon** accepted.
   */
  it("covers the whole of S40's vocabulary against a real daemon", () => {
    const typed = typedLines().map((entry) => entry.line);
    for (const word of [
      "sequence 2",
      "group 1",
      "store group 1",
      "store cue 2",
      "store sequence 4",
      "store preset 1",
      "edit cue 2",
      "update",
      "label cue 2",
      "label group 1",
      "label view 1",
      "move cue 2 cue 3",
      "move sequence 5 sequence 6",
      "copy group 1 group 2",
      "copy sequence 1 sequence 5",
      "delete group 2",
      "delete executor 9",
      "delete sequence 404",
      "color sequence 1 red",
      "color executor 2 #ff8800",
      "goto cue 1",
      "on sequence 1",
      "on sequence 4",
      "off sequence 1",
      "go+ executor 0",
      "assign sequence 4 executor 9",
      "assign executor 9 fader speed",
      "preset 1",
      "full",
      "oops",
      "clear",
      "page 0",
    ]) {
      expect(
        typed.some((line) => line.startsWith(word)),
        `no recorded line starts with "${word}"`,
      ).toBe(true);
    }
  });

  /**
   * **B15, at the grammar.** What a fader, an encoder and the four keys do is
   * sayable, so the control editor can write a line rather than send a command
   * of its own — which is `ARCHITECTURE_SPEC.md` §4.5's test applied rather than
   * assumed.
   */
  it("assigns a function to one of an executor's controls", () => {
    expect(parseCommandLine("assign executor 1 fader master")).toEqual({
      kind: "commands",
      commands: [
        { t: "ConfigureExecutor", executorId: 1, change: { t: "Fader", function: "Master" } },
      ],
    });
    expect(parseCommandLine("Assign Executor 9 Encoder Speed")).toEqual({
      kind: "commands",
      commands: [
        { t: "ConfigureExecutor", executorId: 9, change: { t: "Encoder", function: "Speed" } },
      ],
    });
    // A key is numbered **from one** on the line, because that is how an
    // operator counts the keys under a fader, and from zero in the command,
    // because that is how the hardware counts them.
    expect(parseCommandLine("assign executor 3 button 1 go+")).toEqual({
      kind: "commands",
      commands: [
        {
          t: "ConfigureExecutor",
          executorId: 3,
          change: { t: "Button", index: 0, function: "Go+" },
        },
      ],
    });
    expect(parseCommandLine("assign executor 3 button 4 empty")).toEqual({
      kind: "commands",
      commands: [
        {
          t: "ConfigureExecutor",
          executorId: 3,
          change: { t: "Button", index: 3, function: "Empty" },
        },
      ],
    });
  });

  /** The custom row — a key that sends a line an operator wrote. */
  it("gives a key a command line, quoted or not", () => {
    const expected = {
      kind: "commands",
      commands: [
        {
          t: "ConfigureExecutor",
          executorId: 1,
          change: {
            t: "Button",
            index: 3,
            function: { CommandLine: { line: "Go+ Sequence 3" } },
          },
        },
      ],
    };
    expect(parseCommandLine('assign executor 1 button 4 command "Go+ Sequence 3"')).toEqual(
      expected,
    );
    // **A line with punctuation in it has to be quoted**, and that is the
    // tokeniser's rule rather than this verb's: `go+` becomes `go` and `+`
    // becomes a separator, so that `1 + 2` and `go+ executor 0` mean what they
    // say. A quoted chunk is one token, kept exactly as it was typed.
    expect(parseCommandLine("assign executor 1 button 4 command clear")).toEqual({
      kind: "commands",
      commands: [
        {
          t: "ConfigureExecutor",
          executorId: 1,
          change: { t: "Button", index: 3, function: { CommandLine: { line: "clear" } } },
        },
      ],
    });
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
    // **S26's forms, kept and unbroken**: a bare number after one of these
    // words is an executor, which is what it meant when there was nothing else
    // it could be.
    const executor = (executorId: number) => ({ t: "Executor", executorId }) as const;
    expect(parseCommandLine("go 3")).toEqual({
      kind: "commands",
      commands: [{ t: "ExecutorGo", target: executor(3), direction: "Next" }],
    });
    expect(parseCommandLine("go+ 3")).toEqual(parseCommandLine("go 3"));
    expect(parseCommandLine("go- 3")).toEqual({
      kind: "commands",
      commands: [{ t: "ExecutorGo", target: executor(3), direction: "Prev" }],
    });
    expect(parseCommandLine("off 3")).toEqual({
      kind: "commands",
      commands: [{ t: "ExecutorOff", target: executor(3) }],
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
    expect(message("go banana")).toContain("not something to name");
    expect(message("go 1 2")).toContain("more than a playback takes");
    expect(message("go- 1 2")).toContain("more than a playback takes");
    expect(message("cue 5")).toContain("ambiguous");
    expect(message("delete")).toContain("which one");
    expect(message("delete banana")).toContain("not something to name");
    expect(message("delete sequence")).toContain("sequence which one");
    expect(message("delete sequence banana")).toContain("not a sequence number");
    expect(message("copy sequence 1")).toContain("where");
    expect(message("copy sequence 1 group 2")).toContain("not the same kind of thing");
    expect(message("edit sequence 1")).toContain("edit takes a cue");
    expect(message("goto sequence 1")).toContain("goto takes a cue");
    expect(message("assign cue 1 executor 1")).toContain("assign takes a sequence");
    expect(message("assign sequence 1 group 1")).toContain("assigned to an executor");
    // S45's half of the verb, and the complaints an operator can act on.
    expect(message("assign executor 1")).toContain("assign what on it");
    expect(message("assign executor 1 knob master")).toContain("not one of an executor's controls");
    expect(message("assign executor 1 fader")).toContain("a fader does what");
    expect(message("assign executor 1 fader sideways")).toContain("not something a fader does");
    expect(message("assign executor 1 encoder xfade")).toContain("not something an encoder does");
    expect(message("assign executor 1 button")).toContain("which button");
    expect(message("assign executor 1 button 5 go+")).toContain("which button");
    expect(message("assign executor 1 button 0 go+")).toContain("which button");
    expect(message("assign executor 1 button 1 hologram")).toContain("not something a button does");
    expect(message("assign executor 1 button 1 command")).toContain("send which line");
    expect(message("assign executor 1 fader master and then some")).toContain("says more");
    expect(message("store executor 1")).toContain("assigned rather than stored");
    expect(message("on group 1")).toContain("not something that plays back");
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
    expect(readingText(parseCommandLine("go 2"))).toBe("go on executor 2");
    expect(readingText(parseCommandLine("go- 2"))).toBe("back on executor 2");
    expect(readingText(parseCommandLine("off 2"))).toBe("off on executor 2");
    expect(readingText(parseCommandLine("page 2"))).toBe("page 2");
    expect(readingText(parseCommandLine(""))).toBe("");
    expect(readingText(parseCommandLine("banana"))).toContain("not a fixture number");
  });

  /** Every one of S40's verbs reads back as something a person can act on. */
  it("reads S40's verbs back in words too", () => {
    expect(readingText(parseCommandLine("group 3"))).toBe("select group 3");
    expect(readingText(parseCommandLine("preset 4"))).toBe("apply preset 4");
    expect(readingText(parseCommandLine("sequence 5"))).toBe("select sequence 5");
    expect(readingText(parseCommandLine("view 2"))).toBe("view 2");
    expect(readingText(parseCommandLine("executor 3"))).toBe("select executor 3");
    expect(readingText(parseCommandLine("store cue 5"))).toBe("store cue 5");
    expect(readingText(parseCommandLine("store sequence 5 cue 2"))).toBe(
      "store cue 2 of sequence 5",
    );
    expect(readingText(parseCommandLine("store sequence 4"))).toBe("store sequence 4");
    expect(readingText(parseCommandLine("store preset 1"))).toBe("store preset 1");
    expect(readingText(parseCommandLine("store group 3"))).toBe("store group 3");
    expect(readingText(parseCommandLine("store view 2"))).toBe("store view 2");
    expect(readingText(parseCommandLine("edit cue 3"))).toBe("edit cue 3");
    expect(readingText(parseCommandLine("update"))).toBe("update the cue being edited");
    expect(readingText(parseCommandLine("oops"))).toBe("oops");
    expect(readingText(parseCommandLine("goto cue 5"))).toBe(
      "goto cue 5 on the selected sequence",
    );
    expect(readingText(parseCommandLine("goto executor 1 cue 5"))).toBe(
      "goto cue 5 on executor 1",
    );
    expect(readingText(parseCommandLine("delete group 3"))).toBe("delete group 3");
    expect(readingText(parseCommandLine("copy sequence 2 sequence 6"))).toBe(
      "copy sequence 2 to sequence 6",
    );
    expect(readingText(parseCommandLine("move executor 1 executor 5"))).toBe(
      "move executor 1 to executor 5",
    );
    expect(readingText(parseCommandLine('label view 1 "Programmer"'))).toBe(
      'label view 1 "Programmer"',
    );
    expect(readingText(parseCommandLine("color sequence 4 red"))).toBe(
      "colour sequence 4 #ff0000",
    );
    expect(readingText(parseCommandLine("color executor 1 #f80"))).toBe(
      "colour executor 1 #ff8800",
    );
    expect(readingText(parseCommandLine("color sequence 4"))).toBe(
      "take the colour off sequence 4",
    );
    expect(readingText(parseCommandLine("assign sequence 5 executor 1"))).toBe(
      "assign sequence 5 to executor 1",
    );
    expect(readingText(parseCommandLine("assign executor 1 fader xfade"))).toBe(
      "set the fader to Crossfade of executor 1",
    );
    expect(readingText(parseCommandLine("assign executor 1 button 3 flash"))).toBe(
      "set the button 3 to Flash of executor 1",
    );
    expect(readingText(parseCommandLine('assign executor 1 button 4 command "Go+ Sequence 3"'))).toBe(
      'set the button 4 to send "Go+ Sequence 3" of executor 1',
    );
    expect(readingText(parseCommandLine("on"))).toBe("on the selected sequence");
    expect(readingText(parseCommandLine("on sequence 2"))).toBe("on sequence 2");
    expect(readingText(parseCommandLine("full"))).toBe("dimmer → 100%");
  });

  it("has an arm for a command it was not written for", () => {
    // `Command` is the whole protocol and this readout is not where a new one
    // should become a compile error — but it must still say *something*.
    expect(readingText({ kind: "commands", commands: [{ t: "SaveShow" }] })).toBe("SaveShow");
  });
});

describe("a line that cannot be meant is answered rather than sent", () => {
  /**
   * **Every refusal here names the word that was wrong and shows a line that
   * would work.** A desk is operated in the dark by somebody with their hands
   * full; "syntax error" is not an answer, and neither is a silence.
   *
   * The table is written out by hand rather than generated, for `prism-surface`'s
   * reason (S19): a table derived from the parser would pass for any parser.
   */
  const refusals: readonly (readonly [string, string])[] = [
    // Reading an object: the number, the cue keyword, and the cue number.
    ["copy sequence x cue 1 cue 2", "sequence number"],
    ["copy sequence 5 cue", "cue which one"],
    ["copy sequence 5 cue @ cue 2", "not a cue number"],
    ["goto cue @", "not a cue number"],
    // The verbs, each turned away for its own reason.
    ["edit nonsense 3", "nonsense"],
    ["edit cue 3 and then some", "says more"],
    ["edit group 3", "edit takes a cue"],
    ["goto executor x cue 5", "executor which one"],
    ["goto executor 1 nonsense 5", "nonsense"],
    ["goto executor 1 group 3", "goto takes a cue"],
    ["goto nonsense 5", "nonsense"],
    ["goto group 3", "goto takes a cue"],
    ["delete group 3 and then some", "says more"],
    ["delete nonsense 3", "nonsense"],
    ["copy nonsense 3 sequence 6", "nonsense"],
    ["copy sequence 2", "copy sequence 2 where?"],
    ["copy sequence 2 sequence 6 and then some", "says more"],
    ["copy sequence 2 group 6", "not the same kind"],
    ["label nonsense 3 \"x\"", "nonsense"],
    ["color nonsense 3 red", "nonsense"],
    ["color sequence 4 puce", "is not a colour"],
    ["color sequence 4 #ff88", "is not a colour"],
    ["color sequence 4 red and then some", "says more"],
    ["assign nonsense 5 executor 1", "nonsense"],
    ["assign cue 5 executor 1", "assign takes a sequence"],
    ["assign sequence 5", "assign it where?"],
    ["assign sequence 5 group 1", "assigned to an executor"],
    ["assign sequence 5 executor 1 and then some", "says more"],
    // A playback, which takes one object and no more.
    ["on sequence 5 executor 1", "says more than a playback takes"],
    ["on nonsense 5", "nonsense"],
    ["on group 3", "group"],
    // And a bare object, which selects.
    ["preset @", "not a preset number"],
    ["group 3 and then some", "says more"],
    ["cue 5", "ambiguous"],
  ];

  it.each(refusals)("refuses %j", (line, fragment) => {
    const reading = parseCommandLine(line);
    expect(reading?.kind, line).toBe("error");
    if (reading?.kind !== "error") {
      throw new Error("checked above");
    }
    expect(reading.message.toLowerCase(), line).toContain(fragment.toLowerCase());
  });
});

describe("the mode an operator chose travels in the command", () => {
  /**
   * Three commands carry three different mode types, and each takes only its
   * own words. A mode that does not belong to the command is **left alone**
   * rather than forced in: the prompt is built from the command, so the case
   * cannot arise from the interface, and the type is what stops it arising from
   * anywhere else.
   */
  it("puts each kind of mode into the command that has one", () => {
    const store = parseCommandLine("store cue 5");
    expect(store?.kind).toBe("commands");
    if (store?.kind !== "commands") {
      throw new Error("checked above");
    }
    expect(applyMode(store.commands, "Remove")).toEqual([
      { t: "StoreCue", sequenceId: null, cueNumber: "5", mode: "Remove" },
    ]);
    // A sequence store takes Append, and not a cue store's Remove.
    const sequence = parseCommandLine("store sequence 2");
    if (sequence?.kind !== "commands") {
      throw new Error("store sequence 2 is a command");
    }
    expect(applyMode(sequence.commands, "Append")).toEqual([
      { ...sequence.commands[0], mode: "Append" },
    ]);
    expect(applyMode(sequence.commands, "Remove")).toEqual(sequence.commands);

    // A copy takes Merge and Override, and not Append.
    const copy = parseCommandLine("copy sequence 2 sequence 6");
    if (copy?.kind !== "commands") {
      throw new Error("copy sequence 2 sequence 6 is a command");
    }
    expect(applyMode(copy.commands, "Override")).toEqual([
      { ...copy.commands[0], mode: "Override" },
    ]);
    expect(applyMode(copy.commands, "Append")).toEqual(copy.commands);

    // And a command with no mode at all is handed back untouched.
    expect(applyMode([{ t: "ClearProgrammer" }], "Merge")).toEqual([{ t: "ClearProgrammer" }]);
  });
});
