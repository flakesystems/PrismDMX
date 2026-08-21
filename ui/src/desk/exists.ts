/**
 * Whether one of the six numbered things a line names is already there.
 *
 * # This is not the parser reading the show
 *
 * S26's rule stands and S40 keeps it: {@link parseCommandLine} does not consult
 * the show, so `Copy Sequence 2 Sequence 6` means the same thing whether or not
 * sequence 2 exists. What this answers is a different question — *should the
 * desk ask before sending* — and the answer is only ever a prompt, never a
 * meaning.
 *
 * Reading the mirror for it is honest for one reason: the mirror **is** the
 * show, one delta behind at worst (`ipc/connection.ts`), so a desk that were
 * wrong here would be drawing the wrong cue list on every window at the same
 * time. What may *not* be read from it is what a store would **cost** — how many
 * values it would replace or throw away — because that is derived, and S28 built
 * `Query::StorePreview` so the daemon derives it. See `show/store.ts`.
 */

import type { JsonValue, ObjectRef } from "../bindings";
import { valueAt } from "../mirror/select";
import { sequenceInForce } from "../show/looks";

/**
 * Whether the show or the session already holds what this reference names.
 *
 * A **cue** that names no sequence means the selected one (§4.1), and with
 * nothing selected there is nothing for it to be already holding — so the answer
 * is `false` and the command goes out to be refused by the daemon, which is the
 * split S26 wrote down.
 */
export function objectExists(
  show: JsonValue | null,
  session: JsonValue | null,
  target: ObjectRef,
): boolean {
  switch (target.t) {
    case "Sequence":
      return valueAt(show, `/sequences/${String(target.sequenceId)}`) !== null;
    case "Cue": {
      const sequenceId = target.sequenceId ?? sequenceInForce(session);
      if (sequenceId === null) {
        return false;
      }
      const cues = valueAt(show, `/sequences/${String(sequenceId)}/cues`);
      if (!Array.isArray(cues)) {
        return false;
      }
      return cues.some(
        (cue) =>
          typeof cue === "object" &&
          cue !== null &&
          !Array.isArray(cue) &&
          (cue as Record<string, JsonValue>).number === target.cueNumber.trim(),
      );
    }
    case "Group":
      return valueAt(show, `/groups/${String(target.groupId)}`) !== null;
    case "Preset":
      return valueAt(show, `/presets/${String(target.presetId)}`) !== null;
    case "View":
      return valueAt(session, `/views/${String(target.viewId)}`) !== null;
    case "Executor":
      return valueAt(show, `/executors/${String(target.executorId)}`) !== null;
  }
}
