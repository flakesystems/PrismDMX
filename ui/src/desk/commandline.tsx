/**
 * The command line: the input, what it would do, and the question it is holding.
 *
 * Four things, and `desk/shell.tsx` owns all four — this is the drawing of them:
 *
 * - **what has been typed**, which is the shell's buffer and is mirrored into
 *   `Session::commandLine` at S25's cadence, so a second screen sees it;
 * - **what the line would mean** — the parser's answer, shown as you type, so a
 *   syntax error is visible *before* Enter rather than as a refusal afterwards;
 * - **the words that are legal here**, which is `completions()` and is
 *   client-local (§4.2);
 * - **the question**, when the line would write over something that is already
 *   there: *merge, override or cancel*, in the line rather than in a window over
 *   the canvas.
 *
 * # A syntax error is a message, never a throw
 *
 * `parseCommandLine` answers with commands or with a sentence, for every string
 * there is. This component shows the sentence and refuses to send; it does not
 * catch anything, because there is nothing to catch.
 *
 * # The keys are here too
 *
 * `ARCHITECTURE_SPEC.md` §4.5's three shapes, as a keypad beside the input:
 * `Clear` runs at once, `Store` writes and waits, `Cue` is appended. They are
 * the same words an operator can type, which is the whole point — the screen
 * teaches the vocabulary by building lines in front of them.
 */

import type { FormEvent, KeyboardEvent } from "react";

import { completions } from "./console";
import { readingText } from "./console";
import { PROMPT_MODES, useConsole } from "./consoleshell";

/** What the command line needs: the daemon's line, for the readout beside it. */
export interface CommandLineProps {
  /** The console line **as the daemon holds it**. */
  readonly daemonLine: string;
}

/** A key of the keypad: the word it writes, and which of the three shapes it is. */
interface Key {
  /** The word, spelled as an operator would read it. */
  readonly word: string;
  /** Which shape — see `ARCHITECTURE_SPEC.md` §4.5. */
  readonly shape: "run" | "write" | "append";
  /** What it is for, on the button's title. */
  readonly title: string;
}

/**
 * The keypad, in the order a console has it.
 *
 * Every one of these is a word the parser takes, and the shape decides what
 * pressing it does — never what it *means*, which is the line's.
 */
const CONSOLE_KEYS: readonly Key[] = [
  { word: "Clear", shape: "run", title: "Clear the programmer" },
  { word: "Full", shape: "run", title: "The selection to full" },
  { word: "Update", shape: "run", title: "Store back into the cue being edited" },
  { word: "Oops", shape: "run", title: "Take the last edit back" },
  { word: "Store", shape: "write", title: "Store into…" },
  { word: "Edit", shape: "write", title: "Load a cue into the programmer" },
  { word: "Goto", shape: "write", title: "Jump a playback to a cue" },
  { word: "Move", shape: "write", title: "Move something to another number" },
  { word: "Copy", shape: "write", title: "Copy something onto another number" },
  { word: "Delete", shape: "write", title: "Empty a place on the desk" },
  { word: "Label", shape: "write", title: "Name something" },
  { word: "Assign", shape: "write", title: "Put a sequence on an executor" },
  { word: "Fixture", shape: "append", title: "…a fixture" },
  { word: "Group", shape: "append", title: "…a group" },
  { word: "Sequence", shape: "append", title: "…a sequence" },
  { word: "Cue", shape: "append", title: "…a cue" },
  { word: "Preset", shape: "append", title: "…a preset" },
  { word: "View", shape: "append", title: "…a view" },
  { word: "Executor", shape: "append", title: "…an executor" },
];

/** The command line. */
export function CommandLine({ daemonLine }: CommandLineProps) {
  const console_ = useConsole();
  const { line, reading, prompt } = console_;
  const words = completions(line);

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
        <label htmlFor="command-input">Command</label>
        <input
          id="command-input"
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
      <p className={`command-reading command-${reading.kind}`} id="command-reading">
        <output data-testid="command-reading">{readingText(reading)}</output>
      </p>
      {words.length === 0 ? null : (
        <p className="command-words" data-testid="command-completions">
          {words.slice(0, COMPLETION_LIMIT).map((word) => (
            <button
              key={word}
              type="button"
              className="linkish"
              data-testid={`complete-${word}`}
              onClick={() => {
                console_.write(complete(line, word));
              }}
            >
              {word}
            </button>
          ))}
        </p>
      )}
      <Keypad />
      <p className="daemon-line">
        Engine: <output data-testid="command-line">{daemonLine}</output>
      </p>
    </div>
  );
}

/** How many completions are offered before the row would wrap. */
const COMPLETION_LIMIT = 8;

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
      {PROMPT_MODES[prompt.kind].map((mode) => (
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

/**
 * The three shapes as buttons — `ARCHITECTURE_SPEC.md` §4.5.
 *
 * Nothing here sends a command of its own. A `run` key writes its word and
 * submits, a `write` key writes it and waits, an `append` key adds it to what is
 * there; the line decides the rest, exactly as it does for a typed one.
 */
function Keypad() {
  const { write, append, run } = useConsole();
  return (
    <div className="command-keys" data-testid="command-keys">
      {CONSOLE_KEYS.map((key) => (
        <button
          key={key.word}
          type="button"
          className={`command-key command-key-${key.shape}`}
          data-testid={`key-${key.word.toLowerCase()}`}
          data-shape={key.shape}
          title={key.title}
          onClick={() => {
            if (key.shape === "run") {
              run(key.word);
            } else if (key.shape === "write") {
              write(`${key.word} `);
            } else {
              append(key.word);
            }
          }}
        >
          {key.word}
        </button>
      ))}
    </div>
  );
}

/** The line with its last word replaced by the completion that was chosen. */
function complete(line: string, word: string): string {
  const head = /\s$/.test(line) ? line : line.replace(/\S*$/, "");
  return `${head}${word} `;
}
