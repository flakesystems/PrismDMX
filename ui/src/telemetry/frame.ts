/**
 * `docs/IPC_PROTOCOL.md` §7's fixed layout, read in the browser.
 *
 * ```text
 * ┌────────┬───────┬───────┬────────────┬──────────┐
 * │ "PTLM" │ ver u8│ res u8│ count u16LE│ seq u64LE│   16-byte header
 * └────────┴───────┴───────┴────────────┴──────────┘
 * ┌────────────┬───────────────────────────────────┐
 * │ universe   │ 512 level bytes                   │   × count, in universe
 * │ u16 LE     │                                   │   order
 * └────────────┴───────────────────────────────────┘
 * ```
 *
 * This is `crates/prism-ipc/src/telemetry.rs` from the other end, and it is
 * checked against it two ways: `crates/prism-ipc/tests/interface_telemetry.rs`
 * reads the constants below out of this file, and `frame.test.ts` decodes frames
 * a **running daemon** produced against the answers `TelemetryFrame::decode`
 * gave for them.
 *
 * # It reads; it does not build
 *
 * There is no encoder here, and that is deliberate: nothing in a client ever
 * sends telemetry (§4 — `Telemetry` is daemon → client), and an encoder would
 * exist only so that a test could feed the decoder its own idea of the format.
 * The bytes come from Rust.
 *
 * # Nothing is allocated per frame
 *
 * A frame is 32 912 bytes at 64 universes and there are thirty a second. So
 * {@link TelemetryFrameView} is read **into**: it keeps the payload and an index
 * of where each universe's levels begin, and the levels are read out of the
 * payload where they lie. Nothing is copied, nothing is parsed into objects, and
 * the only allocation is the one-off DataView over the header.
 *
 * # A layout this build does not know is dropped
 *
 * Not guessed at, and not an error either. Telemetry is droppable by definition
 * (§7), so dropping a frame is the *correct* answer here rather than the lazy
 * one — the picture on the canvas simply stays where it was and goes stale,
 * which is what {@link TelemetryFrameView.read} answering with a fault lets the
 * caller do.
 */

/** The four bytes every telemetry frame starts with. */
export const TELEMETRY_MAGIC = "PTLM";

/** The layout version this build reads. `prism_ipc::TELEMETRY_VERSION`. */
export const TELEMETRY_VERSION = 1;

/** Bytes of fixed header. `prism_ipc::TELEMETRY_HEADER_BYTES`. */
export const TELEMETRY_HEADER_BYTES = 16;

/** Channels in one DMX universe. */
export const CHANNELS_PER_UNIVERSE = 512;

/** Bytes of one universe section: the universe number, then its levels. */
export const UNIVERSE_SECTION_BYTES = 2 + CHANNELS_PER_UNIVERSE;

/**
 * Universes a desk has room for — `prism_domain::UniverseId::MAX`.
 *
 * The index arrays start at this size and grow if a frame ever carries more.
 * Growing rather than refusing is on purpose: `TelemetryFrame::decode` has no
 * such limit, and a decoder that refused what the daemon's own decoder accepts
 * would be a second opinion about the wire.
 */
export const MAX_UNIVERSES = 64;

/** Why a telemetry frame could not be read. Mirrors `prism_ipc::TelemetryError`. */
export type TelemetryFault =
  /** The bytes do not start with the magic. */
  | { readonly kind: "not-telemetry" }
  /** A layout this build does not know. The frame is dropped. */
  | { readonly kind: "unknown-version"; readonly found: number; readonly expected: number }
  /** The frame is not the length its own header says it is. */
  | { readonly kind: "truncated"; readonly found: number; readonly expected: number };

/** What to tell a person about a fault. */
export function faultText(fault: TelemetryFault): string {
  switch (fault.kind) {
    case "not-telemetry":
      return "the frame does not begin with the telemetry magic";
    case "unknown-version":
      return `telemetry layout version ${fault.found}, and this build reads ${fault.expected}`;
    case "truncated":
      return `the telemetry frame declares ${fault.expected} bytes and carries ${fault.found}`;
  }
}

/** The magic as bytes, compared without decoding the payload as text. */
const MAGIC = Uint8Array.from(TELEMETRY_MAGIC, (letter) => letter.charCodeAt(0));

/** A payload before the first frame has arrived. */
const NOTHING = new Uint8Array(0);

/**
 * One telemetry frame, read in place.
 *
 * A view rather than a value: it is reused frame after frame, and a caller that
 * kept one across a {@link read} would find it describing the newer frame. That
 * is exactly what the renderer wants — a frame is a picture of *now*, and one
 * three frames old has no value — and it is why nothing here is exposed as an
 * array that could outlive the payload it points into.
 */
export class TelemetryFrameView {
  #bytes: Uint8Array = NOTHING;
  #count = 0;
  #sequence = 0n;
  #read = false;
  #numbers = new Uint16Array(MAX_UNIVERSES);
  #offsets = new Int32Array(MAX_UNIVERSES);

  /**
   * Reads `payload` into this view.
   *
   * Answers `null` when the frame was read, or the fault when it was not — and
   * a fault leaves the view holding the **last frame that was readable**, so a
   * caller can go on drawing the picture it has while it decides what to say
   * about the one it did not get.
   */
  read(payload: Uint8Array): TelemetryFault | null {
    if (payload.byteLength < TELEMETRY_HEADER_BYTES) {
      return { kind: "truncated", expected: TELEMETRY_HEADER_BYTES, found: payload.byteLength };
    }
    for (let index = 0; index < MAGIC.length; index += 1) {
      if (payload[index] !== MAGIC[index]) {
        return { kind: "not-telemetry" };
      }
    }
    const header = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    const version = header.getUint8(4);
    if (version !== TELEMETRY_VERSION) {
      return { kind: "unknown-version", found: version, expected: TELEMETRY_VERSION };
    }
    // Byte 5 is reserved: the room a later section is added in. This build does
    // not read it, and a frame that used it would announce a later version.
    const count = header.getUint16(6, true);
    const expected = TELEMETRY_HEADER_BYTES + count * UNIVERSE_SECTION_BYTES;
    if (payload.byteLength !== expected) {
      // Longer counts as well as shorter, which is the case that would read a
      // short frame and leave rubbish behind it.
      return { kind: "truncated", expected, found: payload.byteLength };
    }

    if (count > this.#numbers.length) {
      this.#numbers = new Uint16Array(count);
      this.#offsets = new Int32Array(count);
    }
    for (let index = 0; index < count; index += 1) {
      const section = TELEMETRY_HEADER_BYTES + index * UNIVERSE_SECTION_BYTES;
      this.#numbers[index] = header.getUint16(section, true);
      this.#offsets[index] = section + 2;
    }
    this.#bytes = payload;
    this.#count = count;
    this.#sequence = header.getBigUint64(8, true);
    this.#read = true;
    return null;
  }

  /** Forgets the frame, as though none had arrived. */
  clear(): void {
    this.#bytes = NOTHING;
    this.#count = 0;
    this.#sequence = 0n;
    this.#read = false;
  }

  /** Whether a frame has been read into this view. */
  get hasFrame(): boolean {
    return this.#read;
  }

  /**
   * The frame's sequence number.
   *
   * A `bigint` because the field is a `u64` and a `number` is exact only to
   * 2^53. Nothing here needs the range — thirty a second reaches 2^53 in nine
   * million years — but a decoder that quietly narrowed a wire field would be
   * the kind of thing that is right until it is not.
   */
  get sequence(): bigint {
    return this.#sequence;
  }

  /** How many universes the frame carries. */
  get count(): number {
    return this.#count;
  }

  /** The bytes the levels lie in. */
  get bytes(): Uint8Array {
    return this.#bytes;
  }

  /** The number of the universe at `index`, or 0 past the end. */
  universeAt(index: number): number {
    return index >= 0 && index < this.#count ? (this.#numbers[index] ?? 0) : 0;
  }

  /**
   * Where the levels of the universe at `index` begin in {@link bytes}.
   *
   * The renderer reads through this rather than through a copy: a `subarray`
   * per universe per frame would be 1 920 objects a second for nothing.
   */
  offsetAt(index: number): number {
    return index >= 0 && index < this.#count ? (this.#offsets[index] ?? 0) : 0;
  }

  /** One level, 0 for a channel or universe the frame does not carry. */
  levelAt(index: number, channel: number): number {
    if (index < 0 || index >= this.#count || channel < 0 || channel >= CHANNELS_PER_UNIVERSE) {
      return 0;
    }
    return this.#bytes[this.offsetAt(index) + channel] ?? 0;
  }
}
