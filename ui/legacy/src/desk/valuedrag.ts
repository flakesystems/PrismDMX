/**
 * Dragging a fader without owning where it is.
 *
 * # This is `canvas/drag.ts`, one layer down
 *
 * S25 settled the question for a window and wrote the answer down: **the
 * resolution is cadence, not ownership.** The daemon owns the value, the screen
 * may show the pointer's value while the button is down, a command goes out at
 * most every {@link SEND_INTERVAL_MS}, and *the local value is dropped the
 * instant the button comes up*. A fader is the same contract with more events
 * per second, so this is that shape rather than that code — the arithmetic
 * differs (one axis, a range rather than a rectangle) and the shared part would
 * have been an abstraction over two callers.
 *
 * The rule that makes it not-optimistic is the last one, and it is the one that
 * is asserted: a fader dragged against a daemon that never answers **springs
 * back**. That is not a failure mode to be smoothed over; it is the operator
 * being told that the engine did not take the value, which on a lighting desk
 * is information.
 *
 * # Why a floor on the rate at all
 *
 * A pointer reports at the display's rate and often faster. S21 established the
 * same thing at the other end of the desk — an unpaced stream of small messages
 * is how a surface is saturated — and the X-Touch's own faders report every
 * 19.8 ms (S20), so thirty a second is *more* than the console sends and well
 * inside `ARCHITECTURE_SPEC.md` §4.3's budget.
 */

import { clampLevel } from "./level";

/**
 * The shortest gap between two commands from one drag.
 *
 * Thirty a second, the same figure `canvas/drag.ts` paces a `PlaceWindow` at
 * and the rate the daemon publishes telemetry at.
 */
export const SEND_INTERVAL_MS = 33;

/** How to start a value drag. */
export interface ValueDragOptions {
  /** The value when the button went down — the daemon's, never the screen's. */
  readonly origin: number;
  /** Where the pointer was, in screen pixels. */
  readonly from: number;
  /**
   * How many pixels of travel are the whole range.
   *
   * The element's length, measured when the button goes down. A zero or absent
   * length answers *one level per pixel*, which is what an element that has not
   * been laid out reports and is the reason this is not a division by zero.
   */
  readonly travel: number;
  /** Whether pulling **down** raises the value, as a vertical fader does. */
  readonly inverted?: boolean;
  /** Sends one command carrying the value. */
  readonly send: (level: number) => void;
}

/**
 * One value drag in progress.
 *
 * Created on pointer-down, fed on pointer-move, ended on pointer-up. It holds
 * no DOM and no clock: the caller passes `now`, which is what lets the pacing be
 * asserted rather than waited for.
 */
export class ValueDrag {
  readonly #options: ValueDragOptions;
  /** The value the pointer describes. What the screen shows, not the state. */
  #level: number;
  /** The last value actually sent, so a drag that has not moved says nothing. */
  #sent: number;
  /** When it was sent. */
  #sentAt: number | null = null;
  /** Set when the pointer is somewhere that has not been sent yet. */
  #owed = false;

  constructor(options: ValueDragOptions) {
    this.#options = options;
    this.#level = clampLevel(options.origin);
    this.#sent = this.#level;
  }

  /** The value the pointer is describing. */
  get level(): number {
    return this.#level;
  }

  /**
   * The pointer moved to `to` pixels, at `now`.
   *
   * Answers the value to show. A command goes out if the pacing allows one and
   * there is something new to say.
   */
  to(to: number, now: number): number {
    const { from, origin, travel, inverted } = this.#options;
    const moved = inverted === true ? from - to : to - from;
    const perPixel = travel > 0 ? 65535 / travel : 1;
    this.#level = clampLevel(origin + moved * perPixel);
    if (this.#level !== this.#sent) {
      this.#owed = true;
    }
    if (this.#owed && (this.#sentAt === null || now - this.#sentAt >= SEND_INTERVAL_MS)) {
      this.#flush(now);
    }
    return this.#level;
  }

  /**
   * The button came up.
   *
   * Sends where the pointer finished, if that has not been sent already —
   * without which a flick shorter than one interval would move the fader on the
   * screen and nowhere else.
   */
  end(now: number): void {
    if (this.#owed) {
      this.#flush(now);
    }
  }

  /** Sends the current value and remembers that it was sent. */
  #flush(now: number): void {
    this.#sent = this.#level;
    this.#sentAt = now;
    this.#owed = false;
    this.#options.send(this.#level);
  }
}

/**
 * How many pixels of pointer travel are the whole range on an encoder.
 *
 * Five hundred and twelve, which is a deliberately slow knob: an encoder trims
 * a value that is already roughly right, and the whole-range gesture is the
 * fader beside it. Holding Shift makes it eight times slower again, which is
 * how a value is set exactly. The console's own V-Pots go through
 * `prism_surface`'s acceleration curves and never reach this class.
 */
export const ENCODER_TRAVEL = 512;

/** The travel Shift makes it, for setting a value precisely. */
export const ENCODER_FINE_TRAVEL = 4096;

/**
 * One encoder turn in progress.
 *
 * The difference from {@link ValueDrag} is the whole of why an encoder is
 * easier: a turn is **relative**, so there is no local value at all. What the
 * bar shows during a turn is the programmer's value as the deltas come back,
 * and D3 is satisfied without anything having to be dropped on pointer-up.
 *
 * What is still local is the cadence: the accumulated turn goes out at most
 * every {@link SEND_INTERVAL_MS}, and each command carries **what has not been
 * sent yet** rather than the total — two commands carrying the same total would
 * move the value twice.
 */
export class EncoderDrag {
  readonly #from: number;
  readonly #travel: number;
  readonly #send: (delta: number) => void;
  /** How far the pointer has turned it, in levels. */
  #turned = 0;
  /** How much of that has gone out. */
  #sent = 0;
  /** When the last command went. */
  #sentAt: number | null = null;

  constructor(options: {
    /** Where the pointer was, in screen pixels. */
    readonly from: number;
    /** Whether the fine rate is in force. */
    readonly fine?: boolean;
    /** Sends one relative command. */
    readonly send: (delta: number) => void;
  }) {
    this.#from = options.from;
    this.#travel = options.fine === true ? ENCODER_FINE_TRAVEL : ENCODER_TRAVEL;
    this.#send = options.send;
  }

  /** How far the pointer has turned the encoder, in levels. */
  get turned(): number {
    return this.#turned;
  }

  /** The pointer moved to `to` pixels, at `now`. */
  to(to: number, now: number): void {
    this.#turned = Math.round(((to - this.#from) * 65535) / this.#travel);
    if (
      this.#turned !== this.#sent &&
      (this.#sentAt === null || now - this.#sentAt >= SEND_INTERVAL_MS)
    ) {
      this.#flush(now);
    }
  }

  /** The button came up: whatever is left over goes now. */
  end(now: number): void {
    if (this.#turned !== this.#sent) {
      this.#flush(now);
    }
  }

  /** Sends the part of the turn that has not gone yet. */
  #flush(now: number): void {
    const delta = this.#turned - this.#sent;
    this.#sent = this.#turned;
    this.#sentAt = now;
    this.#send(delta);
  }
}
