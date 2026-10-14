/**
 * Where telemetry goes, which is **not** into React.
 *
 * `docs/IPC_PROTOCOL.md` §7, last paragraph: *clients must not put telemetry
 * into reactive state*. 64 universes × 512 channels at 30 Hz through React
 * state would make the interface unusable, and that — not authority, not
 * ordering — is the whole reason the second channel exists.
 *
 * So this is the sink: a plain object that holds the latest payload and counts
 * what has been through it, with no subscribers and no way to notify one. S24
 * decodes the fixed-layout frame inside and renders it on a `<canvas>` from
 * here; S23's job is to receive the channel without letting it near a render.
 *
 * It keeps the **latest** payload and not a queue, which is the same decision
 * the daemon makes at the other end (§8: telemetry is coalesced, then dropped).
 * A picture of the lights that is three frames old has no value at all.
 */

/** The latest telemetry payload, and what has been through this sink. */
export class TelemetrySink {
  #latest: Uint8Array | null = null;
  #received = 0;
  #bytes = 0;
  #at = 0;

  /** Takes one payload. Cheap on purpose: this runs 30 times a second. */
  accept = (payload: Uint8Array): void => {
    this.#latest = payload;
    this.#received += 1;
    this.#bytes += payload.byteLength;
    this.#at = Date.now();
  };

  /** The most recent payload, undecoded, or `null` if none has arrived. */
  get latest(): Uint8Array | null {
    return this.#latest;
  }

  /** How many payloads have arrived. */
  get received(): number {
    return this.#received;
  }

  /** How many bytes have arrived, for the diagnostics panel. */
  get bytes(): number {
    return this.#bytes;
  }

  /** `Date.now()` of the last payload, or 0. */
  get at(): number {
    return this.#at;
  }

  /**
   * Forgets the latest payload.
   *
   * Called when the connection goes: a telemetry frame is a picture of what the
   * rig is doing *now*, and holding the last one from a daemon that has stopped
   * is the canvas equivalent of a stale fader value.
   */
  clear(): void {
    this.#latest = null;
    this.#at = 0;
  }
}
