/**
 * Percentages and levels, in one place.
 *
 * An operator says *50*, the protocol carries **0…65535**, and there is exactly
 * one conversion between them. It lives here rather than in the console, the
 * encoder bar and the executor bar separately, because three of them would
 * disagree in the last digit and the disagreement would only ever show up in a
 * cue somebody stored.
 *
 * # Truncating, deliberately
 *
 * `at 50` is 32 767 and not 32 768. A percentage names the level **at or below**
 * it, so a value never rounds up past what was asked for; `at 0` is off and
 * `at 100` is exactly full, which are the two an operator will check. The
 * daemon accepted these numbers in `ui/tests/fixtures/desk-recording.json`, so
 * the arithmetic is checked against a real programmer rather than against
 * itself.
 */

/** The largest level the protocol carries — `u16::MAX`. */
export const FULL = 65535;

/** How many per cent a level is, rounded to one decimal for display. */
export function percentOfLevel(level: number): number {
  return Math.round((clampLevel(level) / FULL) * 1000) / 10;
}

/**
 * The level a percentage names, truncated.
 *
 * A percentage outside 0…100 is clamped rather than refused: the console
 * refuses it before it gets here (that is a syntax error with a message), and a
 * fader dragged past its end stop is an ordinary gesture.
 */
export function levelFromPercent(percent: number): number {
  if (!Number.isFinite(percent)) {
    return 0;
  }
  return clampLevel(Math.floor((Math.min(Math.max(percent, 0), 100) * FULL) / 100));
}

/** A level as whole per cent, for a readout that has one line to say it in. */
export function wholePercent(level: number): number {
  return Math.round((clampLevel(level) / FULL) * 100);
}

/** Keeps a level inside the range the protocol carries, as a whole number. */
export function clampLevel(level: number): number {
  if (!Number.isFinite(level)) {
    return 0;
  }
  return Math.min(Math.max(Math.round(level), 0), FULL);
}
