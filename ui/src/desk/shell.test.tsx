/**
 * The console shell: a key writes into the line, a prompt does not block, and
 * the history is this operator's own — S40.
 *
 * # What this file is for that `desk.test.tsx` is not
 *
 * `desk.test.tsx` drives the whole `<App />` over a socket and asks *which
 * command did that key send*. This one is about the shell itself: the three
 * shapes of `ARCHITECTURE_SPEC.md` §4.5, the question a line holds when its
 * destination is taken, and the two client-local things beside it (§4.2).
 *
 * The store is real and the socket is not: what a gesture *sends* is the point,
 * so the commands are collected straight out of `DeskStore::attach`.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import type { Command, JsonValue } from "../bindings";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore } from "../store/desk";
import { CommandLine } from "./commandline";
import { completions } from "./console";
import { Keypad } from "./keypad";
import { History } from "./history";
import { appended, objectLine, useConsole } from "./consoleshell";
import { ConsoleProvider } from "./shell";

/** A desk with one cue list, one preset, one group and one view already in it. */
const SHOW: JsonValue = {
  sequences: { "1": { id: 1, name: "Act 1", cues: [{ number: "1", name: "Cue 1" }] } },
  presets: { "4": { id: 4, name: "Deep blue" } },
  groups: { "3": { id: 3, name: "Front" } },
  executors: { "0": { id: 0, sequenceId: 1 } },
};

const SESSION: JsonValue = {
  session: { commandLine: "", selectedSequence: 1, encoderBank: "Color" },
  views: { "1": { id: 1, name: "View 1", windows: [] } },
};

/** The shell, with a way to read what it sent and a key of its own to press. */
function shell(session: JsonValue = SESSION, show: JsonValue = SHOW) {
  const store = new DeskStore();
  const sent: Command[] = [];
  store.attach(
    (command) => {
      sent.push(command);
      return sent.length;
    },
    () => null,
  );
  render(
    <DeskProvider store={store}>
      <ConsoleProvider session={session} show={show}>
        <CommandLine daemonLine="" />
        {/*
          **The keys are a window since S43** (punch-list B12) and the line is a
          band, so they are two subtrees of the canvas rather than one control.
          They are rendered together here because what this file is about is the
          rule that binds them: every key writes into the line and none of them
          sends a command of its own.
        */}
        <Keypad />
        <Probe />
      </ConsoleProvider>
    </DeskProvider>,
  );
  /** What was sent, without the line it wrote on the way. */
  const acted = (): Command[] => sent.filter((command) => command.t !== "CommandLineInput");
  return { sent, acted };
}

/** A button for each of the three shapes, so a test can press one directly. */
function Probe() {
  const { write, append, run, line } = useConsole();
  return (
    <>
      <button type="button" data-testid="probe-run" onClick={() => { run("Clear"); }}>
        run
      </button>
      <button type="button" data-testid="probe-write" onClick={() => { write("Store "); }}>
        write
      </button>
      <button type="button" data-testid="probe-append" onClick={() => { append("Cue"); }}>
        append
      </button>
      <button
        type="button"
        data-testid="probe-store"
        onClick={() => { run("Store Cue 1"); }}
      >
        store
      </button>
      <button
        type="button"
        data-testid="probe-store-free"
        onClick={() => { run("Store Cue 9"); }}
      >
        store free
      </button>
      <output data-testid="probe-line">{line}</output>
    </>
  );
}

/** The command input, typed. */
function input(): HTMLInputElement {
  const element = screen.getByTestId("command-input");
  if (!(element instanceof HTMLInputElement)) {
    throw new Error("the command line is an input");
  }
  return element;
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("a key writes a word into the line", () => {
  /** §4.5's first shape: written and executed at once. */
  it("runs a whole command with no argument", () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-run"));
    expect(acted()).toEqual([{ t: "ClearProgrammer" }]);
    expect(input().value).toBe("");
  });

  /** The second: written, and **nothing** sent. */
  it("writes a command that needs arguments and waits", () => {
    const { acted, sent } = shell();
    fireEvent.click(screen.getByTestId("probe-write"));
    expect(input().value).toBe("Store ");
    expect(acted()).toEqual([]);
    // The line itself did go out, because `Session::commandLine` is session
    // state and every attached client draws it.
    expect(sent).toEqual([{ t: "CommandLineInput", text: "Store ", run: false }]);
  });

  /** The third: appended to the line as it stands. */
  it("appends an argument keyword to what is already there", () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-write"));
    fireEvent.click(screen.getByTestId("probe-append"));
    expect(input().value).toBe("Store Cue ");
    expect(acted()).toEqual([]);
  });

  /** And the keypad in the footer is those three shapes, spelled out. */
  it("has a key for each of the three shapes, in the window they now live in", () => {
    const { acted, sent } = shell();
    fireEvent.click(screen.getByTestId("key-cue"));
    expect(input().value).toBe("Cue ");
    fireEvent.click(screen.getByTestId("key-store"));
    expect(input().value).toBe("Store ");
    expect(acted()).toEqual([]);
    fireEvent.click(screen.getByTestId("key-oops"));
    expect(acted()).toEqual([{ t: "Oops" }]);
    expect(sent.at(-1)).toEqual({ t: "CommandLineInput", text: "", run: false });
  });
});

describe("the question a line holds", () => {
  /**
   * **A store onto something that is there asks first**, and asks in the line
   * rather than in a window over the canvas (`CLAUDE.md`).
   */
  it("asks merge, override or cancel when the cue is already there", () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-store"));
    expect(acted()).toEqual([]);
    expect(screen.getByTestId("command-prompt")).not.toBeNull();
    expect(screen.getByTestId("command-prompt-what").textContent).toContain("cue 1");

    fireEvent.click(screen.getByTestId("prompt-Override"));
    expect(acted()).toEqual([
      { t: "StoreCue", sequenceId: null, cueNumber: "1", mode: "Override" },
    ]);
  });

  /** **A cancelled prompt sends nothing at all**, and leaves the line standing. */
  it("cancels without sending anything and keeps the line", () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-store"));
    fireEvent.click(screen.getByTestId("prompt-cancel"));
    expect(acted()).toEqual([]);
    expect(screen.queryByTestId("command-prompt")).toBeNull();
    expect(input().value).toBe("Store Cue 1");
  });

  /** Escape is the same answer from the keyboard. */
  it("cancels on Escape", () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-store"));
    fireEvent.keyDown(input(), { key: "Escape" });
    expect(acted()).toEqual([]);
    expect(screen.queryByTestId("command-prompt")).toBeNull();
  });

  /** And nothing is asked when the number is free: there is nothing to lose. */
  it("does not ask about a number nobody has used", () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-store-free"));
    expect(screen.queryByTestId("command-prompt")).toBeNull();
    expect(acted()).toEqual([
      { t: "StoreCue", sequenceId: null, cueNumber: "9", mode: "Merge" },
    ]);
  });

  /** A new line is a new question: the old one was about a line that is gone. */
  it("drops the question when the line is typed over", () => {
    shell();
    fireEvent.click(screen.getByTestId("probe-store"));
    expect(screen.getByTestId("command-prompt")).not.toBeNull();
    fireEvent.change(input(), { target: { value: "1 thru 3" } });
    expect(screen.queryByTestId("command-prompt")).toBeNull();
  });
});

describe("the line follows the daemon", () => {
  /**
   * **A second client sees the half-typed line**, because `commandLine` is
   * session state (`ARCHITECTURE_SPEC.md` §4.1). This is that from the other
   * end: the session says a line, and the input is it.
   */
  it("shows the line the session is holding", () => {
    shell({
      session: { commandLine: "Fixture 12 thru ", selectedSequence: 1 },
      views: {},
    });
    expect(input().value).toBe("Fixture 12 thru ");
  });
});

describe("completion and history", () => {
  /**
   * **The strip of suggestions is gone — S43, punch-list B13.**
   *
   * It sat under the line at all times and was read as clutter rather than as
   * help. What went is the *display*; `completions()` and Tab are untouched, and
   * they are what this pair of tests holds now. The grammar claim survives with
   * them: what is offered is the word `sequence`, never the sequences there are.
   */
  it("offers the words that are legal at this point in the line, without drawing them", () => {
    shell();
    fireEvent.change(input(), { target: { value: "de" } });
    expect(completions("de")).toContain("Delete");
    expect(screen.queryByTestId("complete-delete")).toBeNull();
    fireEvent.change(input(), { target: { value: "delete " } });
    expect(completions("delete ")).toContain("Sequence");
    expect(completions("delete ")).not.toContain("1");
  });

  it("takes a completion on Tab", () => {
    // **Capitalised since B14**, so a completed word reads the way the same word
    // reads everywhere else in the desk. The line itself is still
    // case-insensitive — what changed is what it is *offered*.
    shell();
    fireEvent.change(input(), { target: { value: "de" } });
    fireEvent.keyDown(input(), { key: "Tab" });
    expect(input().value).toBe("Delete ");
    fireEvent.keyDown(input(), { key: "Tab" });
    expect(input().value).toBe("Delete Sequence ");
  });

  it("walks back through the lines this operator typed", () => {
    shell();
    fireEvent.change(input(), { target: { value: "1 thru 3" } });
    fireEvent.submit(input());
    fireEvent.change(input(), { target: { value: "at 50" } });
    fireEvent.submit(input());

    fireEvent.keyDown(input(), { key: "ArrowUp" });
    expect(input().value).toBe("at 50");
    fireEvent.keyDown(input(), { key: "ArrowUp" });
    expect(input().value).toBe("1 thru 3");
    fireEvent.keyDown(input(), { key: "ArrowDown" });
    expect(input().value).toBe("at 50");
  });

  /** Escape with no question standing clears the line rather than answering. */
  it("clears the line on Escape when there is nothing to cancel", () => {
    shell();
    fireEvent.change(input(), { target: { value: "1 thru 3" } });
    fireEvent.keyDown(input(), { key: "Escape" });
    expect(input().value).toBe("");
  });
});

describe("the history itself", () => {
  it("does not file a blank line or the same line twice", () => {
    const history = new History();
    history.remember("");
    history.remember("   ");
    history.remember("clear");
    history.remember("clear");
    expect(history.lines).toEqual(["clear"]);
  });

  it("keeps what was being typed when the walk began", () => {
    const history = new History();
    history.remember("clear");
    expect(history.walk(-1, "half a line")).toBe("clear");
    expect(history.walk(1, "clear")).toBe("half a line");
  });

  it("has nowhere to go in an empty history, and says so", () => {
    const history = new History();
    expect(history.walk(-1, "")).toBeNull();
    expect(history.walk(1, "")).toBeNull();
  });

  it("stops at the top rather than walking past it", () => {
    const history = new History();
    history.remember("one");
    history.remember("two");
    expect(history.walk(-1, "")).toBe("two");
    expect(history.walk(-1, "")).toBe("one");
    expect(history.walk(-1, "")).toBeNull();
  });

  it("remembers a bounded number of lines", () => {
    const history = new History();
    for (let index = 0; index < 150; index += 1) {
      history.remember(`line ${String(index)}`);
    }
    expect(history.lines.length).toBe(100);
    expect(history.lines[0]).toBe("line 50");
  });
});

describe("the two spellings a key uses", () => {
  it("adds one space between words and one at the end", () => {
    expect(appended("", "Fixture")).toBe("Fixture ");
    expect(appended("Store ", "Cue")).toBe("Store Cue ");
    expect(appended("Store", "Cue")).toBe("Store Cue ");
  });

  it("spells an object the way the parser reads it", () => {
    expect(objectLine({ t: "Sequence", sequenceId: 4 })).toBe("Sequence 4");
    expect(objectLine({ t: "Cue", sequenceId: null, cueNumber: "1.5" })).toBe("Cue 1.5");
    expect(objectLine({ t: "Cue", sequenceId: 2, cueNumber: "1.5" })).toBe("Sequence 2 Cue 1.5");
    expect(objectLine({ t: "Group", groupId: 3 })).toBe("Group 3");
    expect(objectLine({ t: "Preset", presetId: 3 })).toBe("Preset 3");
    expect(objectLine({ t: "View", viewId: 3 })).toBe("View 3");
    expect(objectLine({ t: "Executor", executorId: 3 })).toBe("Executor 3");
  });
});

describe("the line the daemon holds, and the one being typed", () => {
  /**
   * **A stale echo of our own line must not eat what has been typed since.**
   *
   * This is the defect the local machine could not reproduce and CI found in
   * three tests at once. `Session::commandLine` is §4.1 state, so every keystroke
   * is mirrored to the daemon and comes back as a session delta — and by the time
   * it comes back the operator has typed more. Adopting that echo puts the input
   * back to a **prefix** of what they typed; on a fast machine the round trip
   * lands between keystrokes and nothing is ever seen, on a loaded one a whole
   * line disappears and the store that follows it is refused for an empty
   * programmer.
   *
   * The rule: while a line this client sent has not come back yet, ours wins.
   */
  it("keeps what is typed while an older echo of our own line arrives", () => {
    const store = new DeskStore();
    store.attach(
      () => 1,
      () => null,
    );
    const holding = (line: string): JsonValue => ({
      ...(SESSION as Record<string, JsonValue>),
      session: { commandLine: line, selectedSequence: 1, encoderBank: "Color" },
    });

    const view = render(
      <DeskProvider store={store}>
        <ConsoleProvider session={holding("")} show={SHOW}>
          <CommandLine daemonLine="" />
        </ConsoleProvider>
      </DeskProvider>,
    );

    fireEvent.change(input(), { target: { value: "1 thru 3 red at 100" } });
    expect(input().value).toBe("1 thru 3 red at 100");

    // The daemon answers the *first* keystroke of that burst, long after it was
    // typed. It is a line this client sent, so it is an echo and not news.
    view.rerender(
      <DeskProvider store={store}>
        <ConsoleProvider session={holding("1")} show={SHOW}>
          <CommandLine daemonLine="1" />
        </ConsoleProvider>
      </DeskProvider>,
    );
    expect(input().value).toBe("1 thru 3 red at 100");
  });

  /**
   * **And a line somebody else typed still wins**, which is the whole reason the
   * input follows the session at all: a key pressed on a second screen, or on the
   * X-Touch, appears here.
   */
  it("adopts a line this client did not send", () => {
    const store = new DeskStore();
    store.attach(
      () => 1,
      () => null,
    );
    const holding = (line: string): JsonValue => ({
      ...(SESSION as Record<string, JsonValue>),
      session: { commandLine: line, selectedSequence: 1, encoderBank: "Color" },
    });
    const view = render(
      <DeskProvider store={store}>
        <ConsoleProvider session={holding("")} show={SHOW}>
          <CommandLine daemonLine="" />
        </ConsoleProvider>
      </DeskProvider>,
    );
    expect(input().value).toBe("");

    view.rerender(
      <DeskProvider store={store}>
        <ConsoleProvider session={holding("Group ")} show={SHOW}>
          <CommandLine daemonLine="Group " />
        </ConsoleProvider>
      </DeskProvider>,
    );
    expect(input().value).toBe("Group ");
  });
});
