/**
 * Asking the daemon what a store *would* do, before the operator presses Store.
 *
 * # The exit criterion, in one file
 *
 * S28: **a store that would overwrite says what it will do before it does it.**
 * So the Store button does not say *Store*; it says what will happen — which cue
 * or preset it is going into, whether that already exists, and how many values
 * it will add, write over, leave alone and take away.
 *
 * Every one of those numbers is the daemon's (`Query::StorePreview` →
 * `prism_core::ShowFile::preview_store`), because a client cannot work any of
 * them out: it would need what the programmer holds, what is already stored, and
 * what a store mode **does**.
 *
 * # The mode is the operator's, and the word on the button is still the
 * daemon's
 *
 * S39 gave `StoreMode` its other two values, so the choice is now the
 * operator's: it travels in the command *and* in the question, and the answer
 * says what that choice would cost. What has **not** changed is that no client
 * spells the word — {@link storeText} renders `preview.mode`, which is the mode
 * the daemon was asked about and echoed back. A bar that spelled the chosen mode
 * itself would describe the mode the chooser reads *now* beside counts that
 * answered an older question, which is the one way this can still mislead.
 *
 * # The cadence is `patch/preview.ts`'s, one gesture along
 *
 * A question per keystroke means several are in flight at once, and they are not
 * answered in the order they were asked. An answer to a draft that has since
 * been typed over would tell an operator that the *previous* cue number is the
 * one about to be overwritten. So every request carries a number and only the
 * newest is drawn. Dropping the rest costs nothing: a query changes nothing by
 * construction (`docs/IPC_PROTOCOL.md` §5.2).
 */

import type { Answer, Query, StoreMode, StorePreview, StoreTarget } from "../bindings";
import type { Ask } from "../patch/preview";

/**
 * Keeps one store preview current while a target is being chosen.
 *
 * The same shape as `patch/preview.ts`'s `PreviewRequester` and deliberately not
 * a hook: it holds one number and one callback, so it is testable without a
 * renderer.
 */
export class StoreRequester {
  readonly #ask: Ask;
  readonly #onAnswer: (preview: StorePreview | null) => void;
  #generation = 0;
  #stopped = false;

  constructor(ask: Ask, onAnswer: (preview: StorePreview | null) => void) {
    this.#ask = ask;
    this.#onAnswer = onAnswer;
  }

  /**
   * Asks what storing into this target **in this mode** would do.
   *
   * The answer reaches `onAnswer` unless a later request has been made in the
   * meantime, or unless this requester has been stopped — which is what an
   * unmounted window does, so an answer cannot arrive at a component that is no
   * longer on the screen. Since S39 the mode is one of the things that can make
   * a request later: changing the chooser asks the question again.
   */
  request(target: StoreTarget, mode: StoreMode): void {
    this.#generation += 1;
    const generation = this.#generation;
    const query: Query = { t: "StorePreview", target, mode };
    void this.#ask(query).then((answer) => {
      if (this.#stopped || generation !== this.#generation) {
        return;
      }
      this.#onAnswer(storePreviewOf(answer));
    });
  }

  /** Forgets what is outstanding. Nothing arrives after this. */
  stop(): void {
    this.#stopped = true;
  }
}

/** The store preview inside an answer, or `null` for anything else. */
export function storePreviewOf(answer: Answer | null): StorePreview | null {
  return answer !== null && answer.t === "StorePreview" ? answer.preview : null;
}

/**
 * What the Store button says.
 *
 * Three sentences rather than one word, and each is a different thing an
 * operator does something different about:
 *
 * - **it would be refused** — the daemon's own reason, and the button is dead;
 * - **nothing is there** — this creates it, and there is nothing to lose;
 * - **something is there** — this is the overwrite, and the counts say exactly
 *   what it costs. Whichever of `kept` and `removed` is not zero is the number
 *   that makes the chosen mode legible: they are the same values, named by what
 *   the mode does to them.
 *
 * The mode is `preview.mode`, spelled by the daemon. See the module
 * documentation for why it is not spelled here.
 */
export function storeText(preview: StorePreview | null, what: string): string {
  if (preview === null) {
    return `Store ${what}`;
  }
  if (!preview.accepted) {
    return preview.refusal ?? `${what} cannot be stored`;
  }
  const named = preview.name === "" ? what : `${what} — ${preview.name}`;
  if (!preview.exists) {
    return `${preview.mode} into ${what}: ${counts(preview)}. Nothing is there yet.`;
  }
  return `${preview.mode} into ${named}: ${counts(preview)}.`;
}

/** The three numbers, with the ones that are zero left out. */
function counts(preview: StorePreview): string {
  const parts: string[] = [];
  if (preview.added > 0) {
    parts.push(`${String(preview.added)} added`);
  }
  if (preview.replaced > 0) {
    parts.push(`${String(preview.replaced)} replaced`);
  }
  if (preview.kept > 0) {
    parts.push(`${String(preview.kept)} kept`);
  }
  if (preview.removed > 0) {
    // Last, and it is the clause the operator is being warned by: under an
    // Override these are the values nobody asked to lose, and under a Remove
    // they are the whole of what the store does.
    parts.push(`${String(preview.removed)} removed`);
  }
  // An accepted store always changes something — `Programmer::cue` refuses one
  // that would not — so an empty list here means every value it writes is one
  // it is replacing with the same thing, which is worth saying rather than
  // showing a bare full stop.
  return parts.length === 0 ? "nothing changes" : parts.join(", ");
}

/** Whether a preview says the daemon would take this store. */
export function isStorable(preview: StorePreview | null): boolean {
  // `null` is *not yet answered*, and a button that stayed dead until an answer
  // arrived would be a button a disconnected daemon locks. The daemon refuses
  // what it will not take; this only stops the obviously wrong. The same
  // reading `patch/preview.ts::isAcceptable` takes.
  return preview === null || preview.accepted;
}
