/**
 * The console shell: the one place a gesture becomes a line — S40.
 *
 * # A key writes a word into the command line. It does not act.
 *
 * `ARCHITECTURE_SPEC.md` §4.5 is the decision and this module is where it is
 * implemented, once, for every control on the screen. Three shapes, and every
 * key in this interface calls one of the three:
 *
 * | Shape | Example | Method |
 * |---|---|---|
 * | a whole command with no argument | `Clear`, `Oops`, `Update`, `Full` | {@link ConsoleShell.run} |
 * | a command that needs arguments | `Store`, `Edit`, `Goto`, `Move`, `Copy`, `Delete`, `Label`, `Assign` | {@link ConsoleShell.write} |
 * | an argument keyword | `Fixture`, `Group`, `Sequence`, `Cue`, `Preset`, `View`, `Executor` | {@link ConsoleShell.append} |
 *
 * Picking an item out of a **list** — a group in the pool, a cue in the sheet —
 * is `run` as well, because the pointer has supplied the argument the line was
 * waiting for.
 *
 * # The line is the daemon's, and the buffer is a latency shim
 *
 * `Session::commandLine` is §4.1 state, so what an operator is part-way through
 * typing is already on every attached client. Typing into a `<input>` cannot
 * wait for a round trip, so the text is held here **and** mirrored to the daemon
 * at S25's cadence — and the moment the daemon's line differs from the last
 * thing this client sent, the daemon's wins. That is what makes a key pressed on
 * a second screen appear in the input on this one.
 *
 * # A prompt is not a modal
 *
 * A line whose destination already holds something asks *merge, override or
 * cancel* — in the command line, not in a window over the canvas (`CLAUDE.md`
 * forbids one). The show carries on while the question stands: nothing is
 * blocked, no other client is blocked, and a Go from the X-Touch does not wait
 * on it, because the question is a piece of this component's state and not a
 * lock on anything. Escape cancels, and a cancelled prompt sends **nothing**.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";

import type { Command, JsonValue } from "../bindings";
import { useSend } from "../store/hooks";
import { applyMode, parseCommandLine } from "./console";
import { ConsoleContext, appended } from "./consoleshell";
import type { ConsoleShell, Prompt } from "./consoleshell";
import { objectExists } from "./exists";
import { History } from "./history";
import { commandLine } from "./session";
import { SEND_INTERVAL_MS } from "./valuedrag";

/** What the provider needs to read. */
export interface ConsoleProviderProps {
  /** The session document — the line itself, and the selected cue list. */
  readonly session: JsonValue | null;
  /** The show document, for deciding whether a destination is already taken. */
  readonly show: JsonValue | null;
  /** The tree below, which is the whole interface. */
  readonly children: ReactNode;
}

/** Holds the line and hands the three shapes to everything below. */
export function ConsoleProvider({ session, show, children }: ConsoleProviderProps) {
  const send = useSend();
  const daemonLine = commandLine(session);
  const [typed, setTyped] = useState("");
  const [prompt, setPrompt] = useState<Prompt | null>(null);
  const history = useRef(new History());
  const mirror = useMirror(send);

  // **The daemon's line wins the moment it differs from ours.** A key pressed on
  // a second screen, a console typing on the X-Touch, and this client's own
  // paced mirror all arrive the same way; the ref is what stops the echo of our
  // own send from fighting the keystroke after it.
  const echoed = mirror.sent;
  useEffect(() => {
    if (daemonLine !== echoed.current) {
      echoed.current = daemonLine;
      setTyped(daemonLine);
    }
  }, [daemonLine, echoed]);

  // **Memoised on the line, and that is a render budget rather than a tidy-up.**
  // The shell's value goes into a context every window reads, so a `reading`
  // rebuilt on every render would change that value on every render — and S24's
  // *zero React commits over 300 telemetry frames* would stop being true the
  // moment the show document moved. One object per line is what keeps it.
  const reading = useMemo(() => parseCommandLine(typed), [typed]);

  const write = useCallback(
    (text: string) => {
      setTyped(text);
      mirror.soon(text);
      // A new line is a new question. The old one was about a line that is gone.
      setPrompt(null);
    },
    [mirror],
  );

  const append = useCallback(
    (word: string) => {
      write(appended(typed, word));
    },
    [typed, write],
  );

  const dispatch = useCallback(
    (commands: readonly Command[], line: string) => {
      for (const command of commands) {
        send(command);
      }
      history.current.remember(line);
      setTyped("");
      setPrompt(null);
      // The line has been executed, so the console line is cleared — which is
      // `CommandLineInput { text: "" }`, per `prism_core::session`.
      mirror.now("");
    },
    [mirror, send],
  );

  const execute = useCallback(
    (line: string) => {
      const result = parseCommandLine(line);
      if (result.kind !== "commands") {
        // An empty line does nothing and a bad one says why; neither is sent.
        return;
      }
      if (result.question !== undefined && objectExists(show, session, result.question.at)) {
        // **The show carries on while this stands.** It is a piece of state on
        // one screen, not a lock: another client, the console and the tick are
        // all untouched by it.
        setPrompt({
          kind: result.question.kind,
          what: result.question.what,
          commands: result.commands,
        });
        return;
      }
      dispatch(result.commands, line);
    },
    [dispatch, session, show],
  );

  const submit = useCallback(() => {
    execute(typed);
  }, [execute, typed]);

  const run = useCallback(
    (text: string) => {
      setTyped(text);
      mirror.now(text);
      execute(text);
    },
    [execute, mirror],
  );

  const runWithMode = useCallback(
    (text: string, mode: string) => {
      const result = parseCommandLine(text);
      if (result.kind !== "commands") {
        return;
      }
      setTyped(text);
      mirror.now(text);
      dispatch(applyMode(result.commands, mode as never), text);
    },
    [dispatch, mirror],
  );

  const answer = useCallback(
    (mode: string | null) => {
      const held = prompt;
      if (held === null) {
        return;
      }
      if (mode === null) {
        // **Cancel changes nothing at all.** No command is sent, and the line is
        // left standing so the operator can correct it rather than retype it.
        setPrompt(null);
        return;
      }
      dispatch(applyMode(held.commands, mode as never), typed);
    },
    [dispatch, prompt, typed],
  );

  const recall = useCallback(
    (direction: -1 | 1) => {
      const line = history.current.walk(direction, typed);
      if (line !== null) {
        write(line);
      }
    },
    [typed, write],
  );

  const shell = useMemo<ConsoleShell>(
    () => ({
      line: typed,
      reading,
      prompt,
      write,
      append,
      run,
      runWithMode,
      submit,
      answer,
      recall,
    }),
    [typed, reading, prompt, write, append, run, runWithMode, submit, answer, recall],
  );

  return <ConsoleContext.Provider value={shell}>{children}</ConsoleContext.Provider>;
}

/**
 * Mirroring the typed line into the session, paced.
 *
 * The same rule as every other stream here: at most one command every
 * {@link SEND_INTERVAL_MS}, plus one for whatever is owed. A keystroke is not a
 * command, and a console line is read by people rather than by machines.
 */
function useMirror(send: (command: Command) => void): {
  readonly soon: (text: string) => void;
  readonly now: (text: string) => void;
  readonly sent: { current: string };
} {
  const sent = useRef("");
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

  return useMemo(() => {
    const flush = (text: string): void => {
      sent.current = text;
      owed.current = null;
      send({ t: "CommandLineInput", text });
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
    return { soon, now, sent };
  }, [send]);
}
