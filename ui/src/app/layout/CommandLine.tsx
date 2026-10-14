/**
 * The command line — the console's real interface.
 *
 * The daemon parses every line (`Fixture 1 Thru 4 At 50`, `Store Cue 3`,
 * `Group 2`, `Clear`), so the interface never needs to know the syntax: it
 * sends text and shows what comes back. The line itself is **shared**: what is
 * typed here appears on every other screen and is what the X-Touch's keys add
 * to, which is why the box shows the session's `commandLine`, not local state.
 *
 * The old interface added history, completion and a pending-keystroke guard
 * on top of this (`ui/legacy/src/desk/shell.tsx`).
 */

import type { FormEvent } from "react";

import { useRunLine, useSession, useTypeLine } from "../../core";

export function CommandLine() {
  const session = useSession();
  const run = useRunLine();
  const type = useTypeLine();
  const line = session?.session.commandLine ?? "";

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (line.trim() !== "") {
      run(line);
    }
  };

  return (
    <form className="band command" onSubmit={submit}>
      <label htmlFor="command-input" className="command-prompt">
        &gt;
      </label>
      <input
        id="command-input"
        className="command-input"
        data-testid="command-input"
        autoComplete="off"
        spellCheck={false}
        value={line}
        onChange={(event) => type(event.target.value)}
      />
      <button type="submit" className="key">
        Enter
      </button>
    </form>
  );
}
