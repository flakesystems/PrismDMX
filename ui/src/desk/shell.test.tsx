/**
 * The console shell: a key writes into the line, a prompt does not block, and
 * the history is this operator's own — S40, S49.
 *
 * # What this file is for that `desk.test.tsx` is not
 *
 * `desk.test.tsx` drives the whole `<App />` over a socket and asks *which
 * command did that key send*. This one is about the shell itself: the three
 * shapes of `ARCHITECTURE_SPEC.md` §4.5, the question a line holds when its
 * destination is taken, and the two client-local things beside it (§4.2).
 *
 * # What it stopped asserting in S49, and why that is right
 *
 * It used to check that pressing `Clear` sent a `ClearProgrammer`. It cannot,
 * because this interface no longer decides that: a line is **sent** as
 * `CommandLineInput { run: true }` and the daemon reads it. So what is asserted
 * here is that the right *line* goes out, once; what a line **means** is
 * `crates/prism-core/tests/console.rs`, held to a recording of what a real
 * `prismd` accepted, and that the two halves meet is `ui/e2e/console.spec.ts`
 * against a running daemon.
 *
 * The store is real and the socket is not: a fake daemon answers
 * `Query::CommandLineReading` out of the table below, because a *second* parser
 * in TypeScript — even a test's — is the thing S49 removed.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import type { Answer, Command, JsonValue } from "../bindings";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore } from "../store/desk";
import { CommandLine } from "./commandline";
import { Keypad } from "./keypad";
import { History } from "./history";
import { unread, useConsole } from "./consoleshell";
import type { CommandLineReading } from "./consoleshell";
import { ConsoleProvider } from "./shell";

/** A desk holding a cue list whose cue 1 is already there. */
const SESSION: JsonValue = {
  session: { commandLine: "", selectedSequence: 1, encoderBank: "Color" },
  views: { "1": { id: 1, name: "View 1", windows: [] } },
};

/**
 * What a daemon answers about the lines this file types.
 *
 * A table and not a parser: `prism_core::console` is the only parser there is
 * since S49, and a test that grew a second one would be asserting against
 * itself. Every entry is what a real `prismd` answers for that exact line —
 * `cue 1` is in the show and `cue 9` is not, which is the whole difference
 * between the two store lines below.
 */
const READINGS: Readonly<Record<string, Partial<CommandLineReading>>> = {
  "": { kind: "Empty", completions: ["At", "Clear", "Store"] },
  Clear: { kind: "Commands", commands: 1, reading: "clear", verb: true },
  Oops: { kind: "Commands", commands: 1, reading: "oops", verb: true },
  "Store ": { kind: "Error", reading: "which one?", verb: true, completions: ["Sequence"] },
  "Store Cue ": { kind: "Error", reading: "cue which one?", verb: true },
  "Cue ": { kind: "Error", reading: "a cue on its own is ambiguous." },
  "Store Cue 1": {
    kind: "Commands",
    commands: 1,
    reading: "store cue 1",
    verb: true,
    question: { what: "cue 1", modes: ["Merge", "Override", "Remove"] },
  },
  "Store Cue 9": { kind: "Commands", commands: 1, reading: "store cue 9", verb: true },
  "1 thru 3": { kind: "Commands", commands: 1, reading: "select 1 + 2 + 3" },
  "at 50": { kind: "Commands", commands: 1, reading: "dimmer → 50%" },
  de: { kind: "Error", reading: '"de" is not a fixture number.', completions: ["Delete"] },
  "Delete ": { kind: "Error", reading: "which one?", verb: true, completions: ["Sequence"] },
  "Delete Sequence ": {
    kind: "Error",
    reading: "sequence which one?",
    verb: true,
    completions: ["Cue"],
  },
};

/** The daemon's answer for a line, or the refusal it gives anything else. */
function answerFor(text: string): Answer {
  return {
    ...unread(text),
    reading: `"${text}" is not a command.`,
    kind: "Error",
    ...READINGS[text],
    text,
  };
}

/**
 * The shell, with a way to read what it sent and a key of its own to press.
 *
 * The `enquire` half is what is new in S49: every question about a line is
 * answered out of {@link READINGS}, one microtask later, exactly as a daemon
 * answers one message later.
 */
function shell(session: JsonValue = SESSION) {
  const store = new DeskStore();
  const sent: Command[] = [];
  let seq = 0;
  store.attach(
    (command) => {
      sent.push(command);
      seq += 1;
      return seq;
    },
    (query) => {
      seq += 1;
      const id = seq;
      if (query.t === "CommandLineReading") {
        const answer = answerFor(query.text);
        queueMicrotask(() => {
          store.answered(id, answer);
        });
      }
      return id;
    },
  );
  render(
    <DeskProvider store={store}>
      <ConsoleProvider session={session}>
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
  /** The lines that were **run**, as opposed to the keystrokes mirrored on the way. */
  const acted = (): string[] =>
    sent.flatMap((command) =>
      command.t === "CommandLineInput" && command.run ? [command.text] : [],
    );
  /** What was mirrored on the way, which is every keystroke. */
  const written = (): string[] =>
    sent.flatMap((command) =>
      command.t === "CommandLineInput" && !command.run ? [command.text] : [],
    );
  return { sent, acted, written };
}

/** A button for each of the three shapes, so a test can press one directly. */
function Probe() {
  const { write, append, run, line, reading } = useConsole();
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
      <output data-testid="probe-reading">{reading.reading}</output>
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
  /** §4.5's first shape: written and run at once, as one command. */
  it("runs a whole command with no argument", async () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-run"));
    await waitFor(() => {
      expect(acted()).toEqual(["Clear"]);
    });
    expect(input().value).toBe("");
  });

  /** The second: written, and **nothing** run. */
  it("writes a command that needs arguments and waits", async () => {
    const { acted, written } = shell();
    fireEvent.click(screen.getByTestId("probe-write"));
    expect(input().value).toBe("Store ");
    // The line itself did go out, because `Session::commandLine` is session
    // state and every attached client draws it.
    expect(written()).toContain("Store ");
    await waitFor(() => {
      expect(screen.getByTestId("probe-reading").textContent).toBe("which one?");
    });
    expect(acted()).toEqual([]);
  });

  /** The third: appended to the line as it stands. */
  it("appends an argument keyword to what is already there", async () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-write"));
    fireEvent.click(screen.getByTestId("probe-append"));
    expect(input().value).toBe("Store Cue ");
    await waitFor(() => {
      expect(screen.getByTestId("probe-reading").textContent).toBe("cue which one?");
    });
    expect(acted()).toEqual([]);
  });

  /** And the keypad in the footer is those three shapes, spelled out. */
  it("has a key for each of the three shapes, in the window they now live in", async () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("key-cue"));
    expect(input().value).toBe("Cue ");
    fireEvent.click(screen.getByTestId("key-store"));
    expect(input().value).toBe("Store ");
    expect(acted()).toEqual([]);
    fireEvent.click(screen.getByTestId("key-oops"));
    await waitFor(() => {
      expect(acted()).toEqual(["Oops"]);
    });
    expect(input().value).toBe("");
  });
});

describe("the question a line holds", () => {
  /**
   * **A store onto something that is there asks first**, and asks in the line
   * rather than in a window over the canvas (`CLAUDE.md`).
   *
   * **Whether there is anything to ask about is the daemon's since S49.** The
   * client used to look in its own mirror; a client one delta behind would ask
   * about a cue somebody had just deleted, and now it cannot.
   */
  it("asks merge, override or cancel when the cue is already there", async () => {
    const { acted } = shell();
    // **Typed rather than pressed**, which is the ordinary way to reach a
    // question — and it is the case that has a keystroke still owed to the
    // daemon when the prompt goes up: the line has to reach the session anyway,
    // because a question that stands is a line every screen should be showing.
    fireEvent.change(input(), { target: { value: "Store Cue 1" } });
    fireEvent.submit(input());
    expect(await screen.findByTestId("command-prompt")).not.toBeNull();
    expect(screen.getByTestId("command-prompt-what").textContent).toContain("cue 1");
    expect(acted()).toEqual([]);

    fireEvent.click(screen.getByTestId("prompt-Override"));
    await waitFor(() => {
      expect(acted()).toEqual(["Store Cue 1"]);
    });
    expect(screen.queryByTestId("command-prompt")).toBeNull();
  });

  /**
   * **The mode travels in the command** — S28's rule, and the reason the prompt
   * exists at all rather than the daemon guessing an outcome nobody asked for.
   */
  it("sends the word the operator pressed", async () => {
    const { sent } = shell();
    fireEvent.click(screen.getByTestId("probe-store"));
    fireEvent.click(await screen.findByTestId("prompt-Remove"));
    await waitFor(() => {
      expect(sent).toContainEqual({
        t: "CommandLineInput",
        text: "Store Cue 1",
        run: true,
        mode: "Remove",
      });
    });
  });

  /** **A cancelled prompt sends nothing at all**, and leaves the line standing. */
  it("cancels without sending anything and keeps the line", async () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-store"));
    fireEvent.click(await screen.findByTestId("prompt-cancel"));
    expect(acted()).toEqual([]);
    expect(screen.queryByTestId("command-prompt")).toBeNull();
    expect(input().value).toBe("Store Cue 1");
  });

  /** Escape is the same answer from the keyboard. */
  it("cancels on Escape", async () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-store"));
    await screen.findByTestId("command-prompt");
    fireEvent.keyDown(input(), { key: "Escape" });
    expect(acted()).toEqual([]);
    expect(screen.queryByTestId("command-prompt")).toBeNull();
  });

  /** And nothing is asked when the number is free: there is nothing to lose. */
  it("does not ask about a number nobody has used", async () => {
    const { acted } = shell();
    fireEvent.click(screen.getByTestId("probe-store-free"));
    await waitFor(() => {
      expect(acted()).toEqual(["Store Cue 9"]);
    });
    expect(screen.queryByTestId("command-prompt")).toBeNull();
  });

  /** A new line is a new question: the old one was about a line that is gone. */
  it("drops the question when the line is typed over", async () => {
    shell();
    fireEvent.click(screen.getByTestId("probe-store"));
    await screen.findByTestId("command-prompt");
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

describe("the reading under the box", () => {
  /**
   * **The sentence is the daemon's** — S49, and that is what makes a line
   * refused at the console and a line refused on a screen read the same.
   */
  it("draws the daemon's sentence for the line that is in the box", async () => {
    shell();
    fireEvent.change(input(), { target: { value: "at 50" } });
    await waitFor(() => {
      expect(screen.getByTestId("command-reading").textContent).toBe("dimmer → 50%");
    });
    fireEvent.change(input(), { target: { value: "de" } });
    await waitFor(() => {
      expect(screen.getByTestId("command-reading").textContent).toContain(
        "not a fixture number",
      );
    });
  });

  /** A line that is not a command is not run, and says why instead. */
  it("does not run a line that is not one", async () => {
    const { acted } = shell();
    fireEvent.change(input(), { target: { value: "de" } });
    await waitFor(() => {
      expect(screen.getByTestId("command-reading").textContent).not.toBe("");
    });
    fireEvent.submit(input());
    await waitFor(() => {
      expect(screen.getByTestId("command-reading").textContent).not.toBe("");
    });
    expect(acted()).toEqual([]);
    expect(input().value).toBe("de");
  });
});

describe("completion and history", () => {
  /**
   * **The strip of suggestions is gone — S43, punch-list B13**, and since S49
   * the words themselves are the daemon's: they arrive with the reading, so the
   * grammar claim survives without a grammar here. What is offered is the word
   * `Sequence`, never the sequences there are.
   */
  it("takes a completion on Tab", async () => {
    // **Capitalised since B14**, so a completed word reads the way the same word
    // reads everywhere else in the desk. The line itself is still
    // case-insensitive — what changed is what it is *offered*.
    shell();
    fireEvent.change(input(), { target: { value: "de" } });
    await waitFor(() => {
      expect(screen.getByTestId("command-reading").textContent).not.toBe("");
    });
    expect(screen.queryByTestId("complete-delete")).toBeNull();
    fireEvent.keyDown(input(), { key: "Tab" });
    expect(input().value).toBe("Delete ");
    await waitFor(() => {
      expect(screen.getByTestId("command-reading").textContent).toBe("which one?");
    });
    fireEvent.keyDown(input(), { key: "Tab" });
    expect(input().value).toBe("Delete Sequence ");
  });

  /**
   * A completion is never offered for a line that has moved on — the answer
   * carries the text it is about, and a reading that lost the race completes
   * nothing.
   */
  it("offers nothing while the answer is about an older line", () => {
    shell();
    fireEvent.change(input(), { target: { value: "de" } });
    fireEvent.keyDown(input(), { key: "Tab" });
    expect(input().value).toBe("de");
  });

  it("walks back through the lines this operator typed", async () => {
    shell();
    fireEvent.change(input(), { target: { value: "1 thru 3" } });
    fireEvent.submit(input());
    await waitFor(() => {
      expect(input().value).toBe("");
    });
    fireEvent.change(input(), { target: { value: "at 50" } });
    fireEvent.submit(input());
    await waitFor(() => {
      expect(input().value).toBe("");
    });

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

describe("running a line takes a round trip, and the operator does not wait", () => {
  /**
   * **The box is emptied only if it still holds the line that ran** — S49, and
   * CI found it in six tests at once.
   *
   * Enter is asynchronous now: it asks the daemon what the line means and
   * dispatches when the answer comes back. An operator typing the next line in
   * that gap — which is exactly what `command()` does in `ui/e2e/console.spec.ts`,
   * twice a second — would have it wiped by an emptying that belongs to the line
   * before it, and the Enter after that would run an empty box. Every one of the
   * six failures was on the **second** command of a sequence, and none of them
   * was reproducible on this machine.
   *
   * The daemon here answers only when the test says so, so the gap is the
   * subject rather than the weather.
   */
  it("does not empty a box the operator has already refilled", async () => {
    const store = new DeskStore();
    const sent: Command[] = [];
    const waiting: (() => void)[] = [];
    let seq = 0;
    store.attach(
      (command) => {
        sent.push(command);
        seq += 1;
        return seq;
      },
      (query) => {
        seq += 1;
        const id = seq;
        if (query.t === "CommandLineReading") {
          const answer = answerFor(query.text);
          waiting.push(() => {
            store.answered(id, answer);
          });
        }
        return id;
      },
    );
    /** Lets every question asked so far be answered. */
    const settle = (): void => {
      for (const answer of waiting.splice(0)) {
        answer();
      }
    };
    /** The lines that were run, in order. */
    const ran = (): string[] =>
      sent.flatMap((command) =>
        command.t === "CommandLineInput" && command.run ? [command.text] : [],
      );
    render(
      <DeskProvider store={store}>
        <ConsoleProvider session={SESSION}>
          <CommandLine daemonLine="" />
        </ConsoleProvider>
      </DeskProvider>,
    );

    // The first line is typed and Enter is pressed. Nothing is answered yet, so
    // nothing has run.
    fireEvent.change(input(), { target: { value: "Clear" } });
    fireEvent.submit(input());
    expect(input().value).toBe("Clear");

    // **The operator starts the next line while the answer is in flight.**
    fireEvent.change(input(), { target: { value: "1 thru 3" } });
    expect(input().value).toBe("1 thru 3");

    // Now the daemon answers. The first line runs — and the box keeps what is
    // being written into it.
    settle();
    await waitFor(() => {
      expect(ran()).toEqual(["Clear"]);
    });
    expect(input().value).toBe("1 thru 3");

    // And Enter on it runs *that* line rather than an empty box.
    fireEvent.submit(input());
    settle();
    await waitFor(() => {
      expect(ran()).toEqual(["Clear", "1 thru 3"]);
    });
    expect(input().value).toBe("");
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
        <ConsoleProvider session={holding("")}>
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
        <ConsoleProvider session={holding("1")}>
          <CommandLine daemonLine="1" />
        </ConsoleProvider>
      </DeskProvider>,
    );
    expect(input().value).toBe("1 thru 3 red at 100");
  });

  /**
   * **And running a line does not stop the echoes of the keystrokes that built
   * it arriving** — S49, and CI found it.
   *
   * Running a line clears `Session::commandLine` at the daemon, so the shell
   * drops the keystroke it still owed. The first version dropped the
   * **outstanding queue** with it — and the queue is the whole of the rule
   * above. On a machine slow enough for the echo of the line just run to arrive
   * *after* the operator started typing the next one, the input went back to the
   * finished line and the next command was typed over. Every command after the
   * first in `ui/e2e/console.spec.ts` failed for it, and nothing on this machine
   * ever did.
   */
  it("does not adopt the echo of a line it has just run", async () => {
    const store = new DeskStore();
    const holding = (line: string): JsonValue => ({
      ...(SESSION as Record<string, JsonValue>),
      session: { commandLine: line, selectedSequence: 1, encoderBank: "Color" },
    });
    let seq = 0;
    store.attach(
      () => {
        seq += 1;
        return seq;
      },
      (query) => {
        seq += 1;
        const id = seq;
        if (query.t === "CommandLineReading") {
          const answer = answerFor(query.text);
          queueMicrotask(() => {
            store.answered(id, answer);
          });
        }
        return id;
      },
    );
    const view = render(
      <DeskProvider store={store}>
        <ConsoleProvider session={holding("")}>
          <CommandLine daemonLine="" />
        </ConsoleProvider>
      </DeskProvider>,
    );

    // A line is typed and run.
    fireEvent.change(input(), { target: { value: "Clear" } });
    fireEvent.submit(input());
    await waitFor(() => {
      expect(input().value).toBe("");
    });

    // The next one is typed before the daemon has said anything at all.
    fireEvent.change(input(), { target: { value: "1 thru 3" } });
    expect(input().value).toBe("1 thru 3");

    // **Now the echo of the first line arrives**, which is what a loaded runner
    // does. It is a line this client sent, so it is not news.
    view.rerender(
      <DeskProvider store={store}>
        <ConsoleProvider session={holding("Clear")}>
          <CommandLine daemonLine="Clear" />
        </ConsoleProvider>
      </DeskProvider>,
    );
    expect(input().value).toBe("1 thru 3");
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
        <ConsoleProvider session={holding("")}>
          <CommandLine daemonLine="" />
        </ConsoleProvider>
      </DeskProvider>,
    );
    expect(input().value).toBe("");

    view.rerender(
      <DeskProvider store={store}>
        <ConsoleProvider session={holding("Group ")}>
          <CommandLine daemonLine="Group " />
        </ConsoleProvider>
      </DeskProvider>,
    );
    expect(input().value).toBe("Group ");
  });
});
