/**
 * The command line.
 *
 * Three things, and the whole of D3 is the boundary between the first two:
 *
 * - **what has been typed** — local, in the input element, nobody else's
 *   business until Enter;
 * - **what the daemon's console line says** — `/session/commandLine`, shared
 *   with every other client and with the X-Touch's display, and it moves when a
 *   `SessionPatch` says so and not a moment sooner;
 * - **what the line would mean** — the parser's answer, shown as you type, so a
 *   syntax error is visible *before* Enter rather than as a refusal afterwards.
 *
 * The typed line is mirrored into the session with `CommandLineInput`, paced the
 * way every other stream in this interface is paced (S25's cadence rule) — a
 * keystroke is not a command and thirty a second is plenty for a line somebody
 * is reading off a scribble strip.
 *
 * # A syntax error is a message, never a throw
 *
 * `parseCommandLine` answers with commands or with a sentence, for every string
 * there is. This component shows the sentence and refuses to send; it does not
 * catch anything, because there is nothing to catch.
 */

import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";

import type { Command } from "../bindings";
import { parseCommandLine, readingText } from "./console";
import { SEND_INTERVAL_MS } from "./valuedrag";

/** What the command line needs. */
export interface CommandLineProps {
  /** The console line **as the daemon holds it**. */
  readonly daemonLine: string;
  /** Sends the commands a line meant, in order. */
  readonly onCommands: (commands: readonly Command[]) => void;
  /** Sends a `CommandLineInput` carrying the whole line. */
  readonly onText: (text: string) => void;
}

/** The command line. */
export function CommandLine({ daemonLine, onCommands, onText }: CommandLineProps) {
  const [typed, setTyped] = useState("");
  const mirror = useMirror(onText);
  const reading = parseCommandLine(typed);

  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (reading.kind !== "commands") {
      // An empty line does nothing and a bad one says why; neither is sent.
      return;
    }
    onCommands(reading.commands);
    setTyped("");
    // The line has been executed, so the console line is cleared — which is
    // `CommandLineInput { text: "" }`, per `prism_core::session`.
    mirror.now("");
  };

  return (
    <div className="command-line" data-testid="command-line-panel">
      <form onSubmit={submit}>
        <label htmlFor="command-input">Command</label>
        <input
          id="command-input"
          data-testid="command-input"
          value={typed}
          onChange={(event) => {
            setTyped(event.target.value);
            mirror.soon(event.target.value);
          }}
          autoComplete="off"
          spellCheck={false}
          aria-describedby="command-reading"
        />
      </form>
      <p className={`command-reading command-${reading.kind}`} id="command-reading">
        <output data-testid="command-reading">{readingText(reading)}</output>
      </p>
      <p className="daemon-line">
        Engine: <output data-testid="command-line">{daemonLine}</output>
      </p>
    </div>
  );
}

/**
 * Mirroring the typed line into the session, paced.
 *
 * The same rule as every other stream here: at most one command every
 * {@link SEND_INTERVAL_MS}, plus one for whatever is owed. A keystroke is not a
 * command, and a console line is read by people rather than by machines.
 */
function useMirror(onText: (text: string) => void): {
  readonly soon: (text: string) => void;
  readonly now: (text: string) => void;
} {
  const sent = useRef<string | null>(null);
  const owed = useRef<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current !== null) {
        clearTimeout(timer.current);
      }
    },
    [],
  );

  const flush = (text: string): void => {
    sent.current = text;
    owed.current = null;
    onText(text);
  };

  const now = (text: string): void => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    flush(text);
  };

  const soon = (text: string): void => {
    if (sent.current === text) {
      return;
    }
    owed.current = text;
    if (timer.current !== null) {
      return;
    }
    // The first keystroke goes at once — a console line that appeared a
    // thirtieth of a second late would feel like a dropped key — and the rest
    // of the burst is paced behind it.
    flush(text);
    timer.current = setTimeout(() => {
      timer.current = null;
      const pending = owed.current;
      if (pending !== null) {
        flush(pending);
      }
    }, SEND_INTERVAL_MS);
  };

  return { soon, now };
}
