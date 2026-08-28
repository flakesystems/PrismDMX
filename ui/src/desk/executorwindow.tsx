/**
 * The `Executors` window: the strip, in a window — S43.
 *
 * # What moved, and what deliberately did not
 *
 * The bar itself is unchanged. `./executorbar.tsx` still draws the eight strips
 * of the current page, still sends `ExecutorButton` for a press rather than
 * deciding what a press means, and still writes `Executor 3` and `Page 2` as
 * lines because those are a word and a number (§4.5). All this file does is
 * supply the five callbacks from the hooks a window has, instead of from props
 * threaded down through the shell — the shell no longer has a band to thread
 * them through.
 *
 * Two things asked for the move and they agree. The owner's skeleton has no band
 * under the canvas for executors; and punch-list **B15** wants the buttons and
 * the fader to become editable, which needs room a fixed-height band does not
 * have. **S45** is the session that builds the editor, into this window.
 *
 * # The bar is still §4.5's exception
 *
 * A Go is a gesture with timing in it (§4.3) and a fader is a stream of
 * positions at S25's cadence, so the strips' keys and faders go on sending
 * `ExecutorButton` and `SetExecutorMaster` directly rather than writing a line.
 * Being in a window changes nothing about that.
 */

import { useCallback } from "react";

import type { JsonValue } from "../bindings";
import { ExecutorBar } from "./executorbar";
import { objectLine, pick, useConsole } from "./consoleshell";
import { useSend } from "../store/hooks";

/** The window. */
export function ExecutorWindow({
  show,
  session,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
}) {
  const send = useSend();
  const shell = useConsole();
  const { run } = shell;

  const onPage = useCallback(
    (page: number) => {
      run(`Page ${String(page)}`);
    },
    [run],
  );
  const onSelect = useCallback(
    (executorId: number) => {
      // **An argument when the line is waiting for one** —
      // `consoleshell.ts::pickOnto`. `Assign Sequence 2` and a click on a strip
      // finishes the line; with nothing typed it selects the executor, which is
      // what the strip is for.
      //
      // The strip's own **keys** are untouched and stay commands: a Go is a
      // gesture with timing in it (§4.3), which is the one exception §4.5
      // names.
      const words = objectLine({ t: "Executor", executorId });
      pick(shell, words, () => {
        run(words);
      });
    },
    [run, shell],
  );
  const onMaster = useCallback(
    (executorId: number, level: number) => {
      send({ t: "SetExecutorMaster", executorId, level });
    },
    [send],
  );
  // Which button, never what it means: `prism-core` resolves the position
  // against that executor's own `buttonFunctions` (S34). A client that read
  // `isActive` and sent a Go for a `Toggle` would race a second client doing
  // the same — see `./executorbar.tsx`.
  const onButton = useCallback(
    (executorId: number, index: number, pressed: boolean) => {
      send({
        t: "ExecutorButton",
        executorId,
        button: { t: "Slot", index },
        pressed,
      });
    },
    [send],
  );

  return (
    <div className="executor-window" data-testid="executor-window">
      <ExecutorBar
        session={session}
        show={show}
        onPage={onPage}
        onSelect={onSelect}
        onMaster={onMaster}
        onButton={onButton}
      />
    </div>
  );
}
