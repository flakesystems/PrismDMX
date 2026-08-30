/**
 * **Punch-list B15, as an operator meets it**: what an executor's fader,
 * encoder and four keys do is set from a window.
 *
 * The editor is driven directly here rather than through `<App />`: what is
 * being asserted is which **line** each chooser writes, and a whole interface
 * around it would add a socket to a question that has none in it.
 *
 * # The line is the assertion, and that is the point
 *
 * `ARCHITECTURE_SPEC.md` §4.5 makes the command line the interface, and S45 gave
 * the grammar the words for an assignment — so this window writes a line rather
 * than sending a command of its own, and the exit criterion *the window, the
 * line and a bound key produce the same state* is true by construction rather
 * than by three code paths being kept in step. `console.test.ts` is the other
 * half: it holds each of these lines to the command it parses to.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";
import { ConsoleContext, unread } from "./consoleshell";
import type { ConsoleShell } from "./consoleshell";
import { ExecutorEditor } from "./executoreditor";
import { nullSink, setLogSink } from "../log/logger";

setLogSink(nullSink);

/**
 * Executor 1 on cue list 7, with a `Master` fader, three assigned keys and a
 * fourth carrying a line.
 */
const SHOW: JsonValue = {
  sequences: { "7": { name: "Act one", masterLevel: 65535, speed: 1024 } },
  executors: {
    "1": {
      id: 1,
      sequenceId: 7,
      faderFunction: "Master",
      encoderFunction: "Empty",
      buttonFunctions: ["Go+", "Go-", "Off", { CommandLine: { line: "Page 2" } }],
    },
  },
};

/** A session on page 0 with executor 1 selected. */
const SESSION: JsonValue = { session: { executorPage: 0, selectedExecutor: 1 } };

/** The lines the editor wrote, and a shell that records them. */
function shell(): { readonly lines: string[]; readonly value: ConsoleShell } {
  const lines: string[] = [];
  return {
    lines,
    value: {
      line: "",
      reading: unread(""),
      prompt: null,
      write: () => undefined,
      append: () => undefined,
      run: (text: string) => {
        lines.push(text);
      },
      runWithMode: () => undefined,
      submit: () => undefined,
      answer: () => undefined,
      recall: () => undefined,
      pick: () => undefined,
    },
  };
}

/** The editor, with a recording console under it. */
function editor(show: JsonValue = SHOW, session: JsonValue = SESSION): string[] {
  const recorder = shell();
  render(
    <ConsoleContext.Provider value={recorder.value}>
      <ExecutorEditor show={show} session={session} />
    </ConsoleContext.Provider>,
  );
  return recorder.lines;
}

describe("the control editor", () => {
  it("draws what the executor's five controls do now", () => {
    editor();
    expect(screen.getByTestId("editor-executor").getAttribute("data-executor")).toBe("1");
    expect((screen.getByTestId("editor-fader") as HTMLSelectElement).value).toBe("Master");
    expect((screen.getByTestId("editor-encoder") as HTMLSelectElement).value).toBe("Empty");
    expect((screen.getByTestId("editor-button-0") as HTMLSelectElement).value).toBe("Go+");
    expect((screen.getByTestId("editor-button-2") as HTMLSelectElement).value).toBe("Off");
    // The custom row, with the line it carries in the box beside it.
    expect((screen.getByTestId("editor-button-3") as HTMLSelectElement).value).toBe("Command");
    expect((screen.getByTestId("editor-line-3") as HTMLInputElement).value).toBe("Page 2");
  });

  it("writes the line that says what a control does", () => {
    const lines = editor();
    fireEvent.change(screen.getByTestId("editor-fader"), { target: { value: "XFade" } });
    fireEvent.change(screen.getByTestId("editor-encoder"), { target: { value: "Speed" } });
    // Counted from one on the line, because that is how an operator counts the
    // keys under a fader.
    fireEvent.change(screen.getByTestId("editor-button-1"), { target: { value: "Flash" } });
    expect(lines).toEqual([
      "Assign Executor 1 Fader XFade",
      "Assign Executor 1 Encoder Speed",
      "Assign Executor 1 Button 2 Flash",
    ]);
  });

  it("sends a custom row only once the operator has written the line", () => {
    const lines = editor();
    // Choosing the row on a key that has no line yet opens the box and sends
    // nothing: there is nothing to send.
    fireEvent.change(screen.getByTestId("editor-button-0"), { target: { value: "Command" } });
    expect(lines).toEqual([]);
    expect((screen.getByTestId("editor-line-0") as HTMLInputElement).value).toBe("");

    // An empty box sends nothing either, rather than a command that would be
    // refused.
    fireEvent.click(screen.getByTestId("editor-send-0"));
    expect(lines).toEqual([]);

    fireEvent.change(screen.getByTestId("editor-line-0"), {
      target: { value: "Go+ Sequence 3" },
    });
    fireEvent.click(screen.getByTestId("editor-send-0"));
    // **Quoted**, because a line has spaces and punctuation in it and the
    // tokeniser keeps a quoted chunk exactly as it was typed — see
    // `prism_core::console::assign_control_line`.
    expect(lines).toEqual(['Assign Executor 1 Button 1 Command "Go+ Sequence 3"']);
  });

  it("sends the line on Enter as well as on the button", () => {
    const lines = editor();
    fireEvent.keyDown(screen.getByTestId("editor-line-3"), { key: "Enter" });
    expect(lines).toEqual(['Assign Executor 1 Button 4 Command "Page 2"']);
  });

  it("says what to do when nothing is selected, rather than drawing an empty form", () => {
    editor(SHOW, { session: { executorPage: 0 } });
    expect(screen.getByTestId("editor-none").textContent).toContain("Select an executor");
    expect(screen.queryByTestId("editor-fader")).toBeNull();
  });

  it("says so when the selected executor is not on this page", () => {
    editor(SHOW, { session: { executorPage: 3, selectedExecutor: 1 } });
    expect(screen.getByTestId("editor-none").textContent).toContain("not on this page");
  });
});
