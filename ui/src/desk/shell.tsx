/**
 * The console shell: the one place a gesture becomes a line — S40, S49.
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
 * is {@link ConsoleShell.pick}, because the pointer has supplied the argument
 * the line was waiting for.
 *
 * # The line is run by the daemon since S49
 *
 * There was a parser in this directory and there is not any more. A line is
 * **sent** — `Command::CommandLineInput { text, run: true }` — and the daemon
 * reads it, applies what it means and clears the line; what a line *would* do is
 * **asked** — `Query::CommandLineReading` — and drawn under the box. Neither
 * rule is here, and that is the point: a key on the X-Touch can now run a line
 * with no client attached at all, and two focused screens cannot run one twice.
 *
 * What is still here is everything about *this screen*: the buffer that makes
 * typing feel immediate, the history, and the prompt.
 *
 * # The line is the daemon's, and the buffer is a latency shim
 *
 * `Session::commandLine` is §4.1 state, so what an operator is part-way through
 * typing is already on every attached client. Typing into an `<input>` cannot
 * wait for a round trip, so the text is held here **and** mirrored to the daemon
 * at S25's cadence — and the moment the daemon's line differs from the last
 * thing this client sent, the daemon's wins. That is what makes a key pressed on
 * a second screen appear in the input on this one.
 *
 * # A prompt is not a modal
 *
 * A line whose destination already holds something asks *merge, override or
 * cancel* — in the command line, not in a window over the canvas (`CLAUDE.md`
 * forbids one). **Whether there is anything to ask about is the daemon's**
 * answer since S49, so a client one delta behind can no longer ask about a cue
 * that has just been deleted. The show carries on while the question stands:
 * nothing is blocked, no other client is blocked, and a Go from the X-Touch does
 * not wait on it. Escape cancels, and a cancelled prompt sends **nothing**.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";

import type { Answer, Command, CommandLineMode, JsonValue } from "../bindings";
import { useAsk, useSend } from "../store/hooks";
import { ConsoleContext, appended, pickOnto, unread } from "./consoleshell";
import type { CommandLineReading, ConsoleShell, Prompt } from "./consoleshell";
import { History } from "./history";
import { commandLine } from "./session";
import { SEND_INTERVAL_MS } from "./valuedrag";

/** What the provider needs to read. */
export interface ConsoleProviderProps {
  /** The session document — the line itself. */
  readonly session: JsonValue | null;
  /** The tree below, which is the whole interface. */
  readonly children: ReactNode;
}

/** The reading in an answer, or the empty one for anything else. */
function readingOf(answer: Answer | null, text: string): CommandLineReading {
  return answer !== null && answer.t === "CommandLineReading" ? answer : unread(text);
}

/** Holds the line and hands the shapes to everything below. */
export function ConsoleProvider({ session, children }: ConsoleProviderProps) {
  const send = useSend();
  const ask = useAsk();
  const daemonLine = commandLine(session);
  const [typed, setTyped] = useState("");
  const [reading, setReading] = useState<CommandLineReading>(() => unread(""));
  const [prompt, setPrompt] = useState<Prompt | null>(null);
  const history = useRef(new History());
  const mirror = useMirror(send);

  // **The daemon's line wins — but never over a keystroke that has not reached
  // it yet.**
  //
  // A key pressed on a second screen, a console typing on the X-Touch and this
  // client's own paced mirror all arrive the same way, so the line has to be
  // adopted from the session or two screens would drift apart. What must *not*
  // happen is the one this cost a CI run to find: our own `CommandLineInput`
  // comes back as a session delta some milliseconds later, and by then the
  // operator has typed more. Adopting that echo puts the input back to a prefix
  // of what they typed — on a fast machine the echo lands between keystrokes and
  // nothing is ever seen, on a loaded one a whole line is eaten.
  //
  // So every line this client sends is remembered until its echo comes back, and
  // while any is outstanding the session's line is somebody else's opinion about
  // a line we are still writing. Ours wins until we are level again.
  //
  // **A line this client *ran* is one of the lines it sent** — S50, and the
  // second CI run this rule has cost. The daemon clears `Session::commandLine`
  // as part of running a line (`ShowFile::run_command_line`), so a key that
  // writes and runs a whole line — every key in §4.5's first shape — empties the
  // daemon's field a round trip after it was pressed. An operator who started
  // typing in that gap had their line wiped by that emptying: the box went
  // blank, Enter ran nothing, and **nothing said so**. See {@link useMirror}'s
  // `ran` for the half of the fix that queues it.
  const outstanding = mirror.outstanding;
  useEffect(() => {
    const queue = outstanding.current;
    const mine = queue.indexOf(daemonLine);
    if (mine !== -1) {
      // Our own echo. Everything sent before it is superseded, because the
      // session holds one line and this is what it holds.
      queue.splice(0, mine + 1);
      return;
    }
    if (queue.length > 0) {
      // A send of ours has not come back yet, so this is an older state of the
      // field than the one we are already writing.
      return;
    }
    setTyped(daemonLine);
  }, [daemonLine, outstanding]);

  /**
   * The last reading the daemon sent, whatever line it was about.
   *
   * A cache of one, and it is enough: the reading for the line in the box is
   * asked on every change, so by the time Enter is pressed the answer for that
   * exact text is almost always the one being held. What it saves is a round
   * trip on the common Enter; what it must never do is answer about the wrong
   * line, which is why the text is compared rather than assumed.
   */
  const held = useRef<CommandLineReading>(unread(""));

  const readFor = useCallback(
    async (line: string): Promise<CommandLineReading> => {
      if (held.current.text === line) {
        return held.current;
      }
      const answer = readingOf(await ask({ t: "CommandLineReading", text: line }), line);
      held.current = answer;
      return answer;
    },
    [ask],
  );

  // **What the line would do, asked as it is typed.** A question rather than a
  // parser, since S49: see the module documentation. The cleanup is what stops
  // an answer that overtook a keystroke being drawn under a line it is not
  // about — the answer carries its own text, and this drops the ones that lost.
  useEffect(() => {
    let live = true;
    void ask({ t: "CommandLineReading", text: typed }).then((answer) => {
      if (!live) {
        return;
      }
      const read = readingOf(answer, typed);
      held.current = read;
      setReading(read);
    });
    return () => {
      live = false;
    };
  }, [ask, typed]);

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

  /**
   * Sends the line, with the mode an operator chose if they were asked.
   *
   * **One command, not the commands the line falls into** — S49. What `1 thru 3
   * at 50` means is the daemon's arithmetic now, and what comes back is the
   * deltas of the two commands it applied. The line is cleared here as well as
   * there, because an input that waited for a round trip before emptying would
   * show the operator a line they have already run.
   *
   * # It empties the box only if the box still holds the line
   *
   * **CI found this and nothing on a fast machine could.** Running a line is
   * asynchronous since S49: Enter asks the daemon what the line means and
   * dispatches when the answer comes back. An operator — or a test — who has
   * started the *next* line in that gap would have it wiped by an emptying that
   * belongs to the line before it, and the Enter after that would run an empty
   * box. Six end-to-end tests failed for it, all of them on the second command
   * of a sequence and none of them here.
   *
   * So the clear is a comparison rather than an assignment, which is the same
   * rule the echo guard above obeys one message along: **what this client is
   * writing wins over anything about a line it has finished with.**
   */
  const dispatch = useCallback(
    (line: string, mode: CommandLineMode | null) => {
      send({ t: "CommandLineInput", text: line, run: true, mode });
      history.current.remember(line);
      setPrompt(null);
      setTyped((current) => (current === line ? "" : current));
      // And the keystroke still owed goes with it, for the same reason and under
      // the same condition — see `ran`, which also writes down the clearing this
      // run is about to cause so the effect above does not read it as news.
      mirror.ran(line);
    },
    [mirror, send],
  );

  const execute = useCallback(
    async (line: string) => {
      const answer = await readFor(line);
      if (answer.kind !== "Commands") {
        // An empty line does nothing and a bad one says why — the reading under
        // the box, which every screen draws. Neither is sent.
        return;
      }
      if (answer.question !== null) {
        // **The show carries on while this stands.** It is a piece of state on
        // one screen, not a lock: another client, the console and the tick are
        // all untouched by it.
        //
        // The line goes into the session **here and not before**: a question
        // that stands is a line sitting in the box being asked about, and every
        // screen and the X-Touch's display should show what it is about. A line
        // that runs straight through never needs to be written down, because
        // running it is what it was for.
        mirror.now(line);
        setPrompt({
          what: answer.question.what,
          modes: answer.question.modes,
          line,
        });
        return;
      }
      dispatch(line, null);
    },
    [dispatch, mirror, readFor],
  );

  const submit = useCallback(() => {
    void execute(typed);
  }, [execute, typed]);

  const run = useCallback(
    (text: string) => {
      setTyped(text);
      void execute(text);
    },
    [execute],
  );

  const runWithMode = useCallback(
    (text: string, mode: CommandLineMode) => {
      setTyped(text);
      dispatch(text, mode);
    },
    [dispatch],
  );

  const answer = useCallback(
    (mode: CommandLineMode | null) => {
      const held_ = prompt;
      if (held_ === null) {
        return;
      }
      if (mode === null) {
        // **Cancel changes nothing at all.** No command is sent, and the line is
        // left standing so the operator can correct it rather than retype it.
        setPrompt(null);
        return;
      }
      dispatch(held_.line, mode);
    },
    [dispatch, prompt],
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

  const pick = useCallback(
    (words: string, own: () => void) => {
      const candidate = appended(typed, words);
      void readFor(candidate).then((answer) => {
        const chosen = pickOnto(answer);
        if (chosen.kind === "own") {
          own();
          return;
        }
        if (chosen.kind === "run") {
          run(chosen.line);
          return;
        }
        write(chosen.line);
      });
    },
    [readFor, run, typed, write],
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
      pick,
    }),
    [typed, reading, prompt, write, append, run, runWithMode, submit, answer, recall, pick],
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
  readonly ran: (line: string) => void;
  readonly outstanding: { current: string[] };
} {
  const sent = useRef("");
  const owed = useRef<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  /**
   * The lines this client has sent and not yet seen come back.
   *
   * A queue rather than a single value, because the pacing below sends a burst
   * as two commands and both are in flight at once. See the effect that reads
   * it for what it is for.
   */
  const outstanding = useRef<string[]>([]);

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
      outstanding.current.push(text);
      // `run: false` always: a keystroke is a keystroke. Running a line is
      // `ConsoleShell.run` and its own command, and a mirror that set the flag
      // here would run a line an operator is still halfway through typing.
      send({ t: "CommandLineInput", text, run: false, mode: null });
    };
    const now = (text: string): void => {
      if (timer.current !== null) {
        clearTimeout(timer.current);
        timer.current = null;
      }
      flush(text);
    };
    /**
     * Writes down that this client has asked the daemon to **run** a line.
     *
     * # The keystroke still owed for it is dropped — S49
     *
     * The daemon clears the line itself as part of running it, so a flush still
     * owed for that line would put half of what was typed back into a box the
     * operator has finished with.
     *
     * **Only for that line.** What is owed may already be the *next* one: Enter
     * is asynchronous since S49, and an operator does not stop typing while a
     * round trip is in flight. Cancelling it then would leave the second screen
     * and the X-Touch's display showing a line nobody is writing any more.
     *
     * # And the clearing it causes is queued as ours — S50
     *
     * That same clearing arrives as a `SessionPatch` carrying an **empty** line,
     * and it is this client's own doing however the run was started. Left
     * unqueued it was read as news, and adopting it emptied a box the operator
     * was still typing into: a whole line eaten, with the daemon never told and
     * nothing on the screen to say so. It is queued exactly when it will
     * produce a delta at all — `sent` is what this client last put in the
     * daemon's field, so an empty one means the field is already empty and an
     * unchanged field produces no ops (`Session::commit`). Queueing an echo
     * that never arrives would deafen this client to the next real one, which is
     * the second screen this rule exists for.
     */
    const ran = (line: string): void => {
      if (owed.current === null || owed.current === line) {
        if (timer.current !== null) {
          clearTimeout(timer.current);
          timer.current = null;
        }
        owed.current = null;
      }
      if (sent.current !== "") {
        outstanding.current.push("");
      }
      // The daemon's field is empty now, whatever this client last put in it, so
      // the next keystroke mirrors the line the operator is left holding rather
      // than being deduplicated away against a value that is no longer there.
      sent.current = "";
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
    return { soon, now, ran, outstanding };
  }, [send]);
}
