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
    expect(sent).toEqual([{ t: "CommandLineInput", text: "Store " }]);
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
  it("has a key for each of the three shapes", () => {
    const { acted, sent } = shell();
    fireEvent.click(screen.getByTestId("key-cue"));
    expect(input().value).toBe("Cue ");
    fireEvent.click(screen.getByTestId("key-store"));
    expect(input().value).toBe("Store ");
    expect(acted()).toEqual([]);
    fireEvent.click(screen.getByTestId("key-oops"));
    expect(acted()).toEqual([{ t: "Oops" }]);
    expect(sent.at(-1)).toEqual({ t: "CommandLineInput", text: "" });
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
  it("offers the words that are legal at this point in the line", () => {
    shell();
    fireEvent.change(input(), { target: { value: "de" } });
    expect(screen.getByTestId("complete-delete")).not.toBeNull();
    // A grammar answer, never a show answer: the word `sequence`, never the
    // sequences there are.
    fireEvent.change(input(), { target: { value: "delete " } });
    expect(screen.getByTestId("complete-sequence")).not.toBeNull();
    expect(screen.queryByTestId("complete-1")).toBeNull();
  });

  it("takes a completion on Tab and on a click", () => {
    shell();
    fireEvent.change(input(), { target: { value: "de" } });
    fireEvent.keyDown(input(), { key: "Tab" });
    expect(input().value).toBe("delete ");
    fireEvent.click(screen.getByTestId("complete-sequence"));
    expect(input().value).toBe("delete sequence ");
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
