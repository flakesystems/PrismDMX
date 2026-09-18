/**
 * The desk's fixture library, a fixture at a time and a page at a time — S57,
 * punch-list **B60** (GitHub #28).
 *
 * # Why a pager and not a search
 *
 * The owner's third point: the list stopped at sixty, and the only way to reach
 * a fixture past them was to know its name well enough to type it. The library
 * is two thousand profiles and neither a frame nor a screen takes them at once
 * (`LibraryEntry` says so in its own documentation), so the list is **paged**:
 * `Query::BrowseLibrary` answers `offset` fixtures in, and the list asks for
 * the next page when it is scrolled to its end.
 *
 * # One per fixture
 *
 * The first point: a fixture with four modes was four rows. The daemon answers
 * one `LibraryFixture` per fixture with its modes inside it, and the mode is
 * chosen in the form. The key a show embeds is still per mode, so nothing about
 * a stored show changed.
 *
 * # Deliberately not a hook
 *
 * `PreviewRequester`'s reason: a class that holds one generation counter and a
 * list is testable without a renderer, and the paging rules — a newer search
 * drops the answers to the older one, a page is asked for once, nothing is
 * asked past the end — are exactly the part worth testing on their own.
 */

import type { LibraryFixture, LibraryMode } from "../bindings";
import type { Ask } from "./preview";

/**
 * How many fixtures one page holds.
 *
 * Sixty, which is what the old search answered with: more than a panel shows
 * at once at 1280 × 720, so the first page already scrolls, and few enough
 * that a page is a small frame.
 */
export const PAGE_SIZE = 60;

/** What the list shows. */
export interface LibraryView {
  /** Every fixture asked for so far, in the daemon's order. */
  readonly fixtures: readonly LibraryFixture[];
  /** How many match in all, or `null` before the first answer. */
  readonly matched: number | null;
  /** How many fixtures the library holds, or `null` before the first answer. */
  readonly total: number | null;
  /** Whether a page is on its way. */
  readonly loading: boolean;
}

/** Nothing asked yet. */
export const EMPTY_VIEW: LibraryView = { fixtures: [], matched: null, total: null, loading: false };

/** Pages through the library for one search at a time. */
export class LibraryPager {
  readonly #ask: Ask;
  readonly #onChange: (view: LibraryView) => void;
  #text = "";
  #generation = 0;
  #view: LibraryView = EMPTY_VIEW;
  #stopped = false;

  constructor(ask: Ask, onChange: (view: LibraryView) => void) {
    this.#ask = ask;
    this.#onChange = onChange;
  }

  /** What is shown now. */
  get view(): LibraryView {
    return this.#view;
  }

  /**
   * Starts over for what was typed: the list empties and the first page is
   * asked for. Answers to an earlier search are dropped when they arrive.
   */
  search(text: string): void {
    this.#text = text;
    this.#generation += 1;
    this.#set({ fixtures: [], matched: null, total: this.#view.total, loading: false });
    this.#page();
  }

  /** Whether there is a page left to ask for. */
  get hasMore(): boolean {
    const { matched, fixtures } = this.#view;
    return matched === null || fixtures.length < matched;
  }

  /**
   * Asks for the next page — what a list scrolled to its end does. Nothing
   * when one is already on its way or when there is nothing left.
   */
  more(): void {
    if (this.#view.loading || this.#view.matched === null || !this.hasMore) {
      return;
    }
    this.#page();
  }

  /** Forgets what is outstanding. Nothing arrives after this. */
  stop(): void {
    this.#stopped = true;
  }

  #page(): void {
    const generation = this.#generation;
    const offset = this.#view.fixtures.length;
    this.#set({ ...this.#view, loading: true });
    void this.#ask({ t: "BrowseLibrary", text: this.#text, offset, limit: PAGE_SIZE }).then(
      (answer) => {
        if (this.#stopped || generation !== this.#generation) {
          return;
        }
        if (answer === null || answer.t !== "LibraryFixtures") {
          // No answer — a daemon that went away. The list keeps what it has
          // and stops saying it is loading, so it can be asked again.
          this.#set({ ...this.#view, loading: false });
          return;
        }
        this.#set({
          fixtures: [...this.#view.fixtures, ...answer.fixtures],
          matched: answer.matched,
          total: answer.total,
          loading: false,
        });
      },
    );
  }

  #set(view: LibraryView): void {
    this.#view = view;
    this.#onChange(view);
  }
}

/** A fixture as a row names it: the make and the model. */
export function fixtureLabel(fixture: LibraryFixture): string {
  return [fixture.manufacturer, fixture.name].filter((part) => part !== "").join(" ");
}

/**
 * A mode as the mode chooser names it: `Extended · 16 ch`, or the width alone —
 * and a name that already *is* the width, as most of the library's are, once.
 */
export function modeLabel(mode: LibraryMode): string {
  const width = `${String(mode.footprint)} ch`;
  if (mode.mode === "") {
    return width;
  }
  const says = /^(\d+)\s*ch(annels?)?$/i.exec(mode.mode.trim());
  return says !== null && Number(says[1]) === mode.footprint ? mode.mode : `${mode.mode} · ${width}`;
}

/** The modes of a fixture, as the Modes column reads them. */
export function modesSummary(fixture: LibraryFixture): string {
  return fixture.modes.map((mode) => (mode.mode === "" ? `${String(mode.footprint)} ch` : mode.mode)).join(" · ");
}
