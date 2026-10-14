/**
 * Asking the daemon what a patch *would* do, while it is being typed.
 *
 * # The same contract as a drag, with a different event
 *
 * `canvas/drag.ts` wrote it down for a pointer and `desk/valuedrag.ts` for a
 * fader: **ownership is the daemon's, cadence is local, and the local value is
 * dropped when the gesture ends.** A form field is that contract again. What is
 * local is the text in the boxes; what the fixture *is* comes back as a
 * `ShowPatch`; and what the address *would* clash with is asked, because that is
 * the daemon's arithmetic (`prism_core::conflict`) and not the client's.
 *
 * # Why the answers need a generation counter
 *
 * A question per keystroke means several are in flight at once, and they are not
 * guaranteed to be answered in the order they were asked. An answer to a draft
 * that has since been typed over is not merely stale — drawn, it would tell an
 * operator that the address they are looking at clashes when it is the one they
 * *were* looking at that did. So every request carries a number and only the
 * newest is drawn. The rest are dropped, which costs nothing: a query changes
 * nothing by construction.
 */

import type { Answer, PatchAddress, PatchPreview, Query } from "../bindings";

/** How a question reaches the daemon. `DeskStore.ask`, in one word. */
export type Ask = (query: Query) => Promise<Answer | null>;

/**
 * Keeps one preview current while a draft is being edited.
 *
 * Deliberately not a hook: it takes a clock's worth of nothing, holds one
 * number and one callback, and is therefore testable without a renderer — the
 * same reason `ValueDrag` is a class.
 */
export class PreviewRequester {
  readonly #ask: Ask;
  readonly #onAnswer: (preview: PatchPreview | null, query: Query) => void;
  #generation = 0;
  #stopped = false;

  /**
   * `onAnswer` is told which question an answer is to, so a caller can tell
   * the answer to *this* draft from one to the draft before it — S57, where
   * the placements of several fixtures are sent back verbatim and an answer
   * to the previous address would put them in the wrong place.
   */
  constructor(ask: Ask, onAnswer: (preview: PatchPreview | null, query: Query) => void) {
    this.#ask = ask;
    this.#onAnswer = onAnswer;
  }

  /**
   * Asks what patching this fixture would do.
   *
   * The answer reaches `onAnswer` unless a later request has been made in the
   * meantime, or unless this requester has been stopped — which is what an
   * unmounted form does, so an answer cannot arrive at a component that is no
   * longer on the screen.
   */
  request(query: Query): void {
    this.#generation += 1;
    const generation = this.#generation;
    void this.#ask(query).then((answer) => {
      if (this.#stopped || generation !== this.#generation) {
        return;
      }
      this.#onAnswer(previewOf(answer), query);
    });
  }

  /** Forgets what is outstanding. Nothing arrives after this. */
  stop(): void {
    this.#stopped = true;
  }
}

/** The preview inside an answer, or `null` for anything else. */
export function previewOf(answer: Answer | null): PatchPreview | null {
  return answer !== null && answer.t === "PatchPreview" ? answer.preview : null;
}

/** The conflicts inside an answer, or an empty list for anything else. */
export function conflictsOf(answer: Answer | null): readonly PatchConflictLike[] {
  return answer !== null && answer.t === "PatchConflicts" ? answer.conflicts : [];
}

/** The shape both answers carry a list of. */
export interface PatchConflictLike {
  /** The universe the overlap is in. */
  readonly universe: number;
  /** First shared channel. */
  readonly from: number;
  /** Last shared channel. */
  readonly to: number;
  /** The lower fixture number. */
  readonly first: number;
  /** The higher fixture number, which wins the shared channels. */
  readonly second: number;
}

/**
 * Which fixtures are in a conflict, as a set the sheet can ask about per row.
 */
export function conflictedFixtures(
  conflicts: readonly PatchConflictLike[],
): ReadonlySet<number> {
  const found = new Set<number>();
  for (const conflict of conflicts) {
    found.add(conflict.first);
    found.add(conflict.second);
  }
  return found;
}

/**
 * What to tell the operator about a preview, **before** they commit.
 *
 * Three answers rather than two, because *would be refused*, *would overlap* and
 * *is clear* are three different things an operator does three different things
 * about — and the middle one is legal: patching a second fixture onto the first
 * is how a fixture is cloned (`prism_core::conflict`).
 */
export function previewText(preview: PatchPreview | null, id: number, adding = 0): string {
  if (preview === null) {
    return "";
  }
  if (!preview.accepted) {
    return preview.refusal ?? "the daemon would refuse this patch";
  }
  const span =
    preview.lastAddress === null
      ? `${String(preview.footprint)} channels`
      : `${String(preview.footprint)} channels, ending at ${String(preview.lastAddress)}`;
  const several = adding > 1 ? severalText(preview, adding) : "";
  if (preview.conflicts.length === 0) {
    return several === "" ? `Free — ${span}.` : `Free — ${span}. ${several}`;
  }
  const overlaps = preview.conflicts
    .map((conflict) => {
      const other = conflict.first === id ? conflict.second : conflict.first;
      return `${String(conflict.from)}–${String(conflict.to)} with fixture ${String(other)}`;
    })
    .join(", ");
  // Said as a warning and not as a refusal, because it is not one: the higher
  // fixture number wins the shared channels and the engine makes that
  // deterministic (S4). An operator cloning a fixture is doing this on purpose.
  //
  // **And where it would fit** — S57, the owner's fifth point. The daemon's
  // answer, for the whole footprint; nothing here works it out.
  const free =
    preview.nextFree === null
      ? " Nothing is free for it before universe 64 ends."
      : ` Next free: ${placeText(preview.nextFree)}.`;
  const tail = several === "" ? "" : ` ${several}`;
  return `${span}. Overlaps ${overlaps} — the higher fixture number wins.${free}${tail}`;
}

/**
 * Where several new fixtures would go, in one sentence — S57.
 *
 * The first and the last are enough to read the gesture by; the numbers may
 * skip ones that are patched, so the sentence names both ends rather than a
 * count that would imply they run on without a gap.
 */
function severalText(preview: PatchPreview, adding: number): string {
  const first = preview.placements[0];
  const last = preview.placements.at(-1);
  if (first === undefined || last === undefined || preview.placements.length !== adding) {
    return `${String(adding)} of them do not all fit before universe 64 ends.`;
  }
  return `${String(adding)} fixtures, ${String(first.id)} to ${String(last.id)}, at ${placeText(first)} to ${placeText(last)}.`;
}

/** A place as a console writes it: `2.14`, universe and address. */
export function placeText(place: PatchAddress): string {
  return `${String(place.universe)}.${String(place.address)}`;
}

/** Whether a preview says the daemon would take this patch. */
export function isAcceptable(preview: PatchPreview | null): boolean {
  // `null` is *not yet answered*, and a form that refused to submit until an
  // answer arrived would be a form that a disconnected daemon locks. The
  // daemon refuses what it will not take; this only stops the obviously wrong.
  return preview === null || preview.accepted;
}
