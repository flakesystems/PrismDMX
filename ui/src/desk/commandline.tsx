/**
 * The command line: the input, what it would do, and the question it is holding.
 *
 * Four things, and `desk/shell.tsx` owns all four — this is the drawing of them:
 *
 * - **what has been typed**, which is the shell's buffer and is mirrored into
 *   `Session::commandLine` at S25's cadence, so a second screen sees it;
 * - **what the line would mean** — the **daemon's** answer since S49
 *   (`Query::CommandLineReading`), shown as you type, so a syntax error is
 *   visible *before* Enter rather than as a refusal afterwards;
 * - **the question**, when the line would write over something that is already
 *   there: *merge, override or cancel*, in the line rather than in a window over
 *   the canvas.
 *
 * # A syntax error is a message, never a throw
 *
 * `prism_core::console` answers with commands or with a sentence, for every
 * string there is, and this component shows the sentence. It does not catch
 * anything, because there is nothing to catch — and since S49 the sentence is
 * the daemon's, so a line refused at the console and a line refused here read
 * the same.
 *
 * # Two things left in S43, and both were the owner's
 *
 * **The row of suggestions is gone** (B13). The completions still arrive with
 * the reading and Tab still takes the first — what went is the strip of them
 * under the input, which was on the screen at all times and was read as clutter
 * rather than as help. Tab is the gesture; a list of everything you could type
 * next is not.
 *
 * **The keypad is a window** (B12). §4.5's three shapes are still buttons and
 * still write into this line rather than sending anything of their own — they
 * are `desk/keypad.tsx` now, opened as `CommandKeys`. Nineteen buttons around
 * the line made the line look like the smaller half of the screen, and the line
 * *is* the interface.
 */

import type { FormEvent, KeyboardEvent } from "react";

import { COMMAND_INPUT_ID } from "./commandinput";
import { useConsole } from "./consoleshell";

/** What the command line needs: the daemon's line, for the readout beside it. */
export interface CommandLineProps {
  /** The console line **as the daemon holds it**. */
  readonly daemonLine: string;
}

/** The command line. */
export function CommandLine({ daemonLine }: CommandLineProps) {
  const console_ = useConsole();
  const { line, reading, prompt } = console_;
  // The daemon's, and only when it is about the line in the box: an answer that
  // overtook a keystroke completes a line nobody is typing.
  const words = reading.text === line ? reading.completions : [];

  const submit = (event: FormEvent): void => {
    event.preventDefault();
    console_.submit();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "ArrowUp") {
      event.preventDefault();
      console_.recall(-1);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      console_.recall(1);
      return;
    }
    if (event.key === "Escape") {
      // **Escape cancels the question and leaves the line standing**, so an
      // operator who meant something else can correct it rather than retype it.
      event.preventDefault();
      if (prompt === null) {
        console_.write("");
      } else {
        console_.answer(null);
      }
      return;
    }
    if (event.key === "Tab" && words.length > 0) {
      event.preventDefault();
      const first = words[0];
      if (first !== undefined) {
        console_.write(complete(line, first));
      }
    }
  };

  return (
    <div className="command-line" data-testid="command-line-panel">
      <form onSubmit={submit}>
        <label htmlFor={COMMAND_INPUT_ID}>Command</label>
        <input
          id={COMMAND_INPUT_ID}
          data-testid="command-input"
          value={line}
          onChange={(event) => {
            console_.write(event.target.value);
          }}
          onKeyDown={onKeyDown}
          autoComplete="off"
          spellCheck={false}
          aria-describedby="command-reading"
        />
        <button type="submit" data-testid="command-enter" title="Run the line">
          Enter
        </button>
      </form>
      {prompt === null ? null : <PromptBar />}
      <p className={`command-reading command-${reading.kind.toLowerCase()}`} id="command-reading">
        <output data-testid="command-reading">{reading.text === line ? reading.reading : ""}</output>
      </p>
      <p className="daemon-line">
        Engine: <output data-testid="command-line">{daemonLine}</output>
      </p>
    </div>
  );
}

/**
 * The question a line is holding, in the line rather than over the canvas.
 *
 * `CLAUDE.md` asks for a device screen with no scrolling outside the canvas, and
 * a modal over it would be neither. What this is instead is a row inside the
 * footer: the show carries on behind it, every other client is untouched, and a
 * Go from the X-Touch does not wait on it.
 */
function PromptBar() {
  const { prompt, answer } = useConsole();
  if (prompt === null) {
    return null;
  }
  return (
    <p className="command-prompt" data-testid="command-prompt" role="group" aria-label="Overwrite">
      <span data-testid="command-prompt-what">{prompt.what} is already there.</span>
      {prompt.modes.map((mode) => (
        <button
          key={mode}
          type="button"
          data-testid={`prompt-${mode}`}
          onClick={() => {
            answer(mode);
          }}
        >
          {mode}
        </button>
      ))}
      <button
        type="button"
        data-testid="prompt-cancel"
        onClick={() => {
          answer(null);
        }}
      >
        Cancel
      </button>
    </p>
  );
}

/** The line with its last word replaced by the completion that was chosen. */
function complete(line: string, word: string): string {
  const head = /\s$/.test(line) ? line : line.replace(/\S*$/, "");
  return `${head}${word} `;
}
