/**
 * The lines this operator has typed, and walking back through them.
 *
 * # Client-local, and that is `ARCHITECTURE_SPEC.md` §4.2
 *
 * `Session::commandLine` is session state: what is *half typed* is shared with
 * every attached client, because the console's display shows it and a second
 * screen has to agree. What was typed **before** is not: two operators on two
 * screens each have their own train of thought, and a history that mixed them
 * would offer one of them the other's last line under the up arrow.
 *
 * So this is a plain object in the browser, alongside scroll position and hover
 * state — §4.2's own category, and the one place in this session where the
 * answer is *not* a command.
 */

/** How many lines are remembered. */
export const HISTORY_LIMIT = 100;

/**
 * The lines an operator has submitted, newest last, and a cursor into them.
 *
 * The cursor sits **past the end** when nothing is being recalled, which is what
 * makes the first press of the up arrow offer the most recent line rather than
 * the one before it.
 */
export class History {
  #lines: string[] = [];
  #cursor = 0;
  /** What was in the line when the walk began, so Down can put it back. */
  #draft = "";

  /** Every line, oldest first. For tests and for a completion list later. */
  get lines(): readonly string[] {
    return this.#lines;
  }

  /**
   * Files a line that has just been submitted.
   *
   * A line identical to the one before it is **not** filed twice: pressing Go
   * five times should not cost five presses of the up arrow to walk past.
   * Blank lines are not filed at all — Enter on an empty console does nothing,
   * so there is nothing to remember.
   */
  remember(line: string): void {
    const text = line.trim();
    this.#cursor = this.#lines.length;
    this.#draft = "";
    if (text === "" || this.#lines.at(-1) === text) {
      this.#cursor = this.#lines.length;
      return;
    }
    this.#lines.push(text);
    if (this.#lines.length > HISTORY_LIMIT) {
      this.#lines.shift();
    }
    this.#cursor = this.#lines.length;
  }

  /**
   * Walks one step: `-1` for the up arrow, `1` for the down arrow.
   *
   * Answers the line to put in the console, or `null` when there is nowhere to
   * go — the top of an empty history, or a step down past the end, which puts
   * back what was being typed when the walk began.
   *
   * `current` is what is in the line now, so that starting a walk does not lose
   * a half-typed line.
   */
  walk(direction: -1 | 1, current: string): string | null {
    if (this.#lines.length === 0) {
      return null;
    }
    if (this.#cursor === this.#lines.length && direction === -1) {
      this.#draft = current;
    }
    const next = this.#cursor + direction;
    if (next < 0) {
      return null;
    }
    if (next >= this.#lines.length) {
      this.#cursor = this.#lines.length;
      return this.#draft;
    }
    this.#cursor = next;
    return this.#lines[next] ?? null;
  }
}
