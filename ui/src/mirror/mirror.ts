/**
 * The three documents a client holds, and what each delta does to them.
 *
 * The browser's half of `prism_core::ShowMirror` and `SessionMirror`, plus the
 * programmer — which is the third document and arrives whole rather than as a
 * patch (`docs/IPC_PROTOCOL.md` §4.1).
 *
 * # Documents, not models
 *
 * `ShowPatch` and `SessionPatch` are RFC 6902 operations, and an operation is
 * only meaningful against a **document root**. So the show and the session are
 * held here as the `JsonValue` the snapshot carried, not as a parsed model that
 * would have to be re-derived after every operation and could not be pointed
 * at. Views that want a typed shape read it out with a selector; the thing the
 * deltas are applied to stays the document the daemon patched.
 */

import type { Delta, ProgrammerState } from "../bindings";
import { applyOp, applyOps } from "./patch";
import type { JsonValue } from "../bindings";

/** The three documents, as of one moment. */
export interface Documents {
  /** The show: patch, groups, presets, sequences, executors. */
  readonly show: JsonValue;
  /** The session: `{ session, views }` — the operating state every client shares. */
  readonly session: JsonValue;
  /** The programmer: selection plus the sparse set of touched values. */
  readonly programmer: ProgrammerState;
}

/**
 * The pointer a playback's state lives at, matching `prism_core::show`.
 *
 * The **cue list's**, and only the cue list's, since S45: a playback is a
 * sequence's. It was two collections until then, and the executors are what
 * punch-list entry B18 found two of.
 */
const SEQUENCES = "sequences";

/**
 * Applies one delta, answering with the documents that result.
 *
 * The answer is the *same object* when a delta changes nothing here, so a
 * caller can compare by identity to decide whether to notify anybody.
 *
 * `PlaybackState` writes the two fields it carries into the show. The protocol
 * gives running playbacks a delta of their own so a client does not have to
 * diff the show to draw a moving executor bar, and a mirror that ignored it
 * would drift on exactly those fields — S18 checked that by removing it.
 *
 * **Which collection it writes into is the playback's own answer** (S40): an
 * executor's state is on its row of the grid, and a cue list playing on no fader
 * keeps it on the sequence. `prism_core::Show::record_playback_state` writes the
 * same two places from the same delta, which is what keeps this document and the
 * daemon's agreeing.
 *
 * @throws {import("./patch").MirrorFault} if an operation does not fit. That is
 * not a recoverable condition: it means this client and the daemon have already
 * diverged, and the caller's answer is to re-snapshot.
 */
export function applyDelta(documents: Documents, delta: Delta): Documents {
  switch (delta.t) {
    case "ShowPatch": {
      if (delta.ops.length === 0) {
        return documents;
      }
      return { ...documents, show: applyOps(documents.show, delta.ops) };
    }
    case "SessionPatch": {
      if (delta.ops.length === 0) {
        return documents;
      }
      return { ...documents, session: applyOps(documents.session, delta.ops) };
    }
    case "ProgrammerChanged":
      return { ...documents, programmer: delta.state };
    case "PlaybackState": {
      // **The cue list's row, always** — S45. A playback is a sequence's, so
      // there is one place for the two fields; an executor row draws them by
      // reading through to the list standing on it, which is what makes two
      // executors of one sequence say the same thing. `prism_core::mirror` is
      // the other end of this claim and writes the same pointer.
      const base = `/${SEQUENCES}/${String(delta.playback)}`;
      const active = applyOp(documents.show, {
        op: "replace",
        path: `${base}/isActive`,
        value: delta.isActive,
      });
      return {
        ...documents,
        show: applyOp(active, {
          op: "replace",
          path: `${base}/currentCueIndex`,
          value: delta.cueIndex,
        }),
      };
    }
    // Everything else describes something that is not one of the three
    // documents: the output patch and its status lights, the save lamp, and a
    // message for the operator. The store holds those; the mirror does not.
    //
    // `OutputsChanged` is deliberately here rather than in the show: the rig
    // belongs to the **machine** and not to the show (S33), so writing it into
    // the show document would put a hall's cabling into what a client believes
    // the show to be - and a save would be next.
    case "OutputsChanged":
    // `SurfaceChanged` is here for the same reason (S36): which desk is in the
    // rack belongs to the **machine**, so writing it into the show document
    // would put a hall's hardware into what a client believes the show to be.
    case "SurfaceChanged":
    // And S38's two, for the fourth time and the same reason: what this
    // machine's desk keys *do*, and whether learn is armed on it, belong to the
    // machine. A table written into the show document would travel to another
    // hall on a stick, which is the whole of `prism_core::outputs`' argument.
    case "SurfaceBindingsChanged":
    case "SurfaceLearnChanged":
    // And S37's two, for the third time and the same reason: what this machine
    // is *set to*, and which show file is open, are the machine's rather than
    // the show's — a mirror that wrote either into the show document would put
    // a desk's own settings into what a client believes the show to be.
    case "MachineChanged":
    case "ShowFileChanged":
    case "OutputHealth":
    case "DirtyFlag":
    case "Notice":
      return documents;
  }
}

/**
 * Applies deltas in order.
 *
 * @throws {import("./patch").MirrorFault} for the first delta that does not fit.
 */
export function applyDeltas(documents: Documents, deltas: readonly Delta[]): Documents {
  let current = documents;
  for (const delta of deltas) {
    current = applyDelta(current, delta);
  }
  return current;
}
