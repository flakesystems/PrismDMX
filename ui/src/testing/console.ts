/**
 * A daemon that reads a command line, for a test that renders one window.
 *
 * # Why a window's test needs one at all, since S49
 *
 * `ARCHITECTURE_SPEC.md` §4.5: every key on the screen writes a line and the
 * line is what is sent. Until S49 the interface also *read* the line, so a
 * window's test could press a tile and watch a `Delete Group 2` go out. The
 * parser is `prism_core::console` now, so what goes out is the **line** —
 * `Command::CommandLineInput { text: "Delete Group 2", run: true }` — and
 * whether it is finished is an answer that has to come from somewhere.
 *
 * This is that somewhere, and it is deliberately **not** a parser: it says *yes,
 * that is a whole command* to everything, because what a window's test is about
 * is which line the tile wrote. What a line **means** is asserted where the rule
 * lives (`crates/prism-core/tests/console.rs`, against a recording of what a
 * real `prismd` accepted) and end to end in `ui/e2e/console.spec.ts`. A table of
 * meanings here would be the second opinion S49 removed, restated as scenery.
 *
 * Scenery, not a fixture: `src/testing/` is excluded from coverage for exactly
 * this reason (S28's note about `show-recording.ts`).
 */

import { decode } from "@msgpack/msgpack";
import { act } from "react";

import type { Answer, Command } from "../bindings";
import { unread } from "../desk/consoleshell";
import type { CommandLineReading } from "../desk/consoleshell";
import type { DeskStore } from "../store/desk";
import { serverMessage } from "./fake-daemon";
import type { FakeSocket } from "./fake-daemon";

/**
 * The answer a daemon that says yes gives about a line.
 *
 * **A whole command, and not a verb line.** Those two defaults are what a
 * window's test wants almost everywhere: a key that writes a line and runs it
 * needs the first, and a tile clicked onto an empty line needs the second — with
 * nothing typed, `Group 3` is not a verb line, so the box does its own thing.
 * A test whose gesture *appends* to a verb overrides that one line by name,
 * which is where the interesting cases are and where they should be visible.
 */
export function whole(text: string): CommandLineReading {
  return {
    ...unread(text),
    kind: "Commands",
    commands: 1,
    reading: text,
  };
}

/**
 * Attaches a store to a recording dispatcher and a daemon that reads lines.
 *
 * `sent` collects every command, in order. `readings` overrides the answer for
 * the lines a test wants a *different* answer about — a line that is not
 * finished, a store that would write over something, a clearing verb — keyed by
 * the exact text.
 */
export function attachDaemon(
  store: DeskStore,
  sent: Command[],
  readings: Readonly<Record<string, Partial<CommandLineReading>>> = {},
): void {
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
        const answer: Answer = {
          ...whole(query.text),
          ...readings[query.text],
          text: query.text,
        };
        // One microtask later, as a daemon answers one message later — and
        // never inside `ask`, which has not filed the question yet.
        queueMicrotask(() => {
          store.answered(id, answer);
        });
      }
      return id;
    },
  );
}

/** The lines a gesture **ran**, in order. */
export function ranLines(sent: readonly Command[]): string[] {
  return sent.flatMap((command) =>
    command.t === "CommandLineInput" && command.run ? [command.text] : [],
  );
}

/** The last line the console **wrote** and left standing, or the empty one. */
export function writtenLine(sent: readonly Command[]): string {
  const written = sent.flatMap((command) =>
    command.t === "CommandLineInput" && !command.run ? [command.text] : [],
  );
  return written.at(-1) ?? "";
}

/**
 * Answers every question this socket has been asked and not yet answered, then
 * lets the promises waiting on them run.
 *
 * The socket half of {@link attachDaemon}, for the files that drive `<App />`
 * over a `FakeNetwork` rather than over a store. A question is answered once:
 * which sequence numbers have been answered is remembered on the socket itself,
 * so a test may call this after every gesture without replaying the lot.
 *
 * **It runs until the interface stops asking**, because one gesture can ask
 * twice: a click on a pool tile asks what the *candidate* line would be, and the
 * answer to that is what makes it run the line — which asks again about the line
 * it is running. Bounded, so a defect that asked forever is a failing test
 * rather than a hanging one.
 */
export async function settleReadings(
  socket: FakeSocket,
  readings: Readonly<Record<string, Partial<CommandLineReading>>> = {},
): Promise<void> {
  for (let round = 0; round < 8; round += 1) {
    if (!(await answerRound(socket, readings))) {
      return;
    }
  }
}

/** One pass over the outstanding questions. Answers `true` if it answered any. */
async function answerRound(
  socket: FakeSocket,
  readings: Readonly<Record<string, Partial<CommandLineReading>>>,
): Promise<boolean> {
  const answered = seen.get(socket) ?? new Set<number>();
  seen.set(socket, answered);
  let any = false;
  for (const sent of socket.sent) {
    const message: unknown = decode(sent);
    if (
      typeof message !== "object" ||
      message === null ||
      (message as { t?: unknown }).t !== "Query"
    ) {
      continue;
    }
    const { seq, query } = message as { seq: number; query: { t: string; text?: string } };
    if (answered.has(seq) || query.t !== "CommandLineReading") {
      continue;
    }
    answered.add(seq);
    any = true;
    const text = query.text ?? "";
    const answer: Answer = { ...whole(text), ...readings[text], text };
    act(() => {
      socket.deliver(serverMessage({ t: "Answer", seq, answer }));
    });
  }
  await act(async () => {
    await Promise.resolve();
  });
  return any;
}

/** Which questions each socket has already been answered. */
const seen = new WeakMap<FakeSocket, Set<number>>();
