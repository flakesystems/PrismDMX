/**
 * The level view: 64 universes × 512 channels, drawn on a `<canvas>`.
 *
 * `docs/IPC_PROTOCOL.md` §7, last paragraph, is why this is not a React tree.
 * 32 768 numbers thirty times a second is not a rendering problem a virtual DOM
 * can be asked to solve — it is a raster, and this module treats it as one.
 *
 * # How a frame is drawn, and why it is fast
 *
 * One pixel per channel. The levels become an RGBA block 512 wide and one row
 * per universe — 32 768 pixel writes through a 256-entry palette, which is a
 * table lookup and a store — and that block is blitted into the grid rectangle,
 * scaled, with smoothing off. The alternative, a `fillRect` per channel, is
 * 32 768 calls into the canvas API for the same picture.
 *
 * Everything that does not change with the frame — the universe numbers, the
 * channel ticks — is drawn only when it changes, which it does when the window
 * is resized or the daemon starts publishing a different set of universes.
 *
 * # The canvas is a device, so it is an argument
 *
 * Every hardware seam in this project is a trait so a test needs no device
 * (`CLAUDE.md`), and a 2D context is this layer's device: `jsdom` has no
 * rasteriser, so a test that called `getContext("2d")` would be a test that
 * skipped itself. {@link LevelSurface} is the seam — five operations, no canvas
 * types in the signatures — with {@link canvasSurface} over a real context and a
 * recording one in the tests, which is what lets the pixel arithmetic be
 * asserted per channel rather than looked at.
 */

import { CHANNELS_PER_UNIVERSE } from "./frame";
import type { TelemetryFrameView } from "./frame";

/** Where the picture is drawn. Implemented over a 2D context, and over a fake. */
export interface LevelSurface {
  /** Device pixels across. */
  readonly width: number;
  /** Device pixels down. */
  readonly height: number;
  /** Paints the whole surface one colour. */
  clear(colour: string): void;
  /** Paints one rectangle. */
  fill(colour: string, x: number, y: number, width: number, height: number): void;
  /** Draws text with its top-left corner at `x, y`. */
  label(text: string, x: number, y: number, colour: string, size: number): void;
  /**
   * Draws an RGBA block of `columns` × `rows` pixels into a rectangle, scaled,
   * without smoothing.
   *
   * One call per frame. Everything about the budget is here.
   */
  blit(
    pixels: Uint8ClampedArray,
    columns: number,
    rows: number,
    x: number,
    y: number,
    width: number,
    height: number,
  ): void;
}

/** The colours. Dark, because a lighting desk is used in a dark room. */
const INK = {
  /** Behind everything. */
  background: "#0b0e13",
  /** Behind the grid, which is also what a channel at zero looks like. */
  grid: "#141a23",
  /** Universe numbers and channel ticks. */
  chrome: "#7d8896",
  /** The meter's trough. */
  trough: "#1c242f",
  /** A meter at full. */
  meter: "#ffd479",
  /** A meter below full. */
  meterLow: "#4bb3c4",
  /** What is drawn over the picture when it has gone stale. */
  stale: "rgba(11, 14, 19, 0.72)",
} as const;

/** How wide the universe-number column is, at scale 1. */
const GUTTER = 30;

/** How wide the peak meter column is, at scale 1. */
const METER = 46;

/** How tall the channel-tick strip is, at scale 1. */
const HEADER = 12;

/** Padding round the picture, at scale 1. */
const PADDING = 4;

/** The smallest row a universe may have. Below this the grid is a smear. */
const MIN_ROW = 2;

/** Where each part of the picture goes, in device pixels. */
export interface LevelGeometry {
  /** Pixels per CSS pixel — everything below is already multiplied by it. */
  readonly scale: number;
  /** How tall one universe's row is. */
  readonly rowHeight: number;
  /** How many universes fit. */
  readonly rows: number;
  /** The level grid. */
  readonly grid: { readonly x: number; readonly y: number; readonly width: number; readonly height: number };
  /** The peak meters. */
  readonly meter: { readonly x: number; readonly width: number };
  /** The universe-number column. */
  readonly gutter: { readonly x: number; readonly width: number };
}

/**
 * Works out where everything goes for a surface of this size.
 *
 * Answers `null` when there is nothing to draw — no universes, or a surface too
 * small to put a row in. A caller that got a geometry with zero rows would
 * divide by it.
 */
export function levelGeometry(
  width: number,
  height: number,
  universes: number,
  scale = 1,
): LevelGeometry | null {
  if (universes <= 0 || width <= 0 || height <= 0) {
    return null;
  }
  const padding = PADDING * scale;
  const gutter = GUTTER * scale;
  const meter = METER * scale;
  const header = HEADER * scale;
  const gridWidth = width - padding * 2 - gutter - meter;
  const gridTop = padding + header;
  const available = height - gridTop - padding;
  if (gridWidth < 1 || available < MIN_ROW * scale) {
    return null;
  }
  const rowHeight = Math.max(MIN_ROW, Math.floor(available / universes));
  const rows = Math.min(universes, Math.max(1, Math.floor(available / rowHeight)));
  return {
    scale,
    rowHeight,
    rows,
    grid: { x: padding + gutter, y: gridTop, width: gridWidth, height: rowHeight * rows },
    meter: { x: padding + gutter + gridWidth, width: meter },
    gutter: { x: padding, width: gutter },
  };
}

/**
 * The 256 colours a level is drawn in, as packed RGBA words.
 *
 * Built once. The byte order is worked out at run time rather than assumed:
 * a `Uint32Array` over a pixel buffer is little-endian on every machine this
 * runs on, but "every machine this runs on" is the kind of assumption that is
 * true until somebody builds for one that is not, and finding out costs eight
 * bytes here.
 */
function buildPalette(): Uint32Array {
  const probe = new Uint8ClampedArray(4);
  const word = new Uint32Array(probe.buffer);
  const palette = new Uint32Array(256);
  for (let level = 0; level < 256; level += 1) {
    const [red, green, blue] = colourOf(level);
    probe[0] = red;
    probe[1] = green;
    probe[2] = blue;
    probe[3] = 255;
    palette[level] = word[0] ?? 0;
  }
  return palette;
}

/**
 * A level as a colour: dark at nothing, cool through the middle, warm at full.
 *
 * Two straight segments rather than a curve. What this has to support is
 * *recognition* — which channels are up, and roughly how far — and a ramp a
 * person can read off at a glance beats one that is smooth (`CLAUDE.md`: fast
 * recognition of sections over prettiness).
 */
function colourOf(level: number): [number, number, number] {
  if (level <= 0) {
    return [20, 26, 35];
  }
  if (level < 128) {
    const t = level / 127;
    return [Math.round(24 + t * 40), Math.round(70 + t * 130), Math.round(130 + t * 55)];
  }
  const t = (level - 128) / 127;
  return [Math.round(64 + t * 191), Math.round(200 + t * 35), Math.round(185 - t * 35)];
}

const PALETTE = buildPalette();

/** What the painter reports about the frame it has just drawn. */
export interface PaintResult {
  /** How many universe rows were drawn. */
  readonly rows: number;
  /** The loudest channel in the frame, 0…255. */
  readonly peak: number;
}

/** Nothing was drawn, because there was nowhere to draw it. */
const NOTHING_DRAWN: PaintResult = { rows: 0, peak: 0 };

/**
 * Draws telemetry frames onto one surface.
 *
 * Keeps its pixel buffer and its peak array between frames, so a paint costs no
 * allocation once the size has settled — the same reason
 * `TelemetryFrame::encode_into` takes a buffer at the other end of the wire.
 */
export class LevelPainter {
  readonly #surface: LevelSurface;
  #pixels = new Uint8ClampedArray(0);
  #words = new Uint32Array(0);
  #peaks = new Uint8Array(0);
  /** What the chrome was last drawn for, so it is drawn again only when it changes. */
  #chrome = "";

  constructor(surface: LevelSurface) {
    this.#surface = surface;
  }

  /** Forces the chrome to be drawn again — after a resize, or a new surface. */
  invalidate(): void {
    this.#chrome = "";
  }

  /**
   * Draws one frame.
   *
   * `scale` is device pixels per CSS pixel, so text and gutters stay the same
   * physical size on a high-density screen.
   */
  paint(frame: TelemetryFrameView, scale = 1): PaintResult {
    const surface = this.#surface;
    const geometry = levelGeometry(surface.width, surface.height, frame.count, scale);
    if (geometry === null) {
      // Nothing to draw on, or nothing to draw. Not an error: a panel can be
      // one pixel tall for a moment while a window is dragged.
      if (this.#chrome !== "") {
        surface.clear(INK.background);
        this.#chrome = "";
      }
      return NOTHING_DRAWN;
    }

    const { rows } = geometry;
    this.#drawChrome(frame, geometry);
    const peak = this.#raster(frame, rows);
    surface.blit(
      this.#pixels,
      CHANNELS_PER_UNIVERSE,
      rows,
      geometry.grid.x,
      geometry.grid.y,
      geometry.grid.width,
      geometry.grid.height,
    );
    this.#drawMeters(geometry, rows);
    return { rows, peak };
  }

  /**
   * Dims what is on the surface, for a picture that is no longer current.
   *
   * Drawn over rather than cleared: *these were the levels a second ago* is more
   * use to somebody looking at a rig than an empty rectangle, as long as it does
   * not look current. What must never happen is the other thing — a stale
   * picture presented as a live one — and that is what the wash is for.
   */
  dim(): void {
    this.#surface.fill(INK.stale, 0, 0, this.#surface.width, this.#surface.height);
  }

  /** Clears the surface and forgets what was drawn on it. */
  blank(): void {
    this.#surface.clear(INK.background);
    this.#chrome = "";
  }

  /** The universe numbers and the channel ticks, when they have changed. */
  #drawChrome(frame: TelemetryFrameView, geometry: LevelGeometry): void {
    const { rows, rowHeight, scale } = geometry;
    let signature = `${this.#surface.width}x${this.#surface.height}:${rows}:${rowHeight}`;
    for (let row = 0; row < rows; row += 1) {
      signature += `,${frame.universeAt(row)}`;
    }
    if (signature === this.#chrome) {
      return;
    }
    this.#chrome = signature;

    const surface = this.#surface;
    surface.clear(INK.background);

    const size = Math.max(7, Math.min(11, rowHeight - 2 * scale));
    const labelled = rowHeight >= 9 * scale;
    for (let row = 0; row < rows; row += 1) {
      if (labelled) {
        surface.label(
          String(frame.universeAt(row)),
          geometry.gutter.x,
          geometry.grid.y + row * rowHeight,
          INK.chrome,
          size,
        );
      }
    }
    if (!labelled) {
      // Too tight for one number per row, so the ends are named instead: an
      // unlabelled block of 64 rows is unreadable, and a label every row that
      // overlaps its neighbour is worse.
      surface.label(String(frame.universeAt(0)), geometry.gutter.x, geometry.grid.y, INK.chrome, 9 * scale);
      surface.label(
        String(frame.universeAt(rows - 1)),
        geometry.gutter.x,
        geometry.grid.y + geometry.grid.height - 9 * scale,
        INK.chrome,
        9 * scale,
      );
    }

    // The channel ruler, so a lit channel can be found by eye rather than
    // counted. 1 is where DMX starts; the interface says so rather than 0.
    for (const channel of [1, 128, 256, 384, 512]) {
      const x = geometry.grid.x + ((channel - 1) / CHANNELS_PER_UNIVERSE) * geometry.grid.width;
      surface.label(String(channel), x, PADDING * scale, INK.chrome, 9 * scale);
    }
  }

  /** Fills the pixel buffer from the frame, answering with the loudest level. */
  #raster(frame: TelemetryFrameView, rows: number): number {
    const needed = CHANNELS_PER_UNIVERSE * rows * 4;
    if (this.#pixels.length !== needed) {
      this.#pixels = new Uint8ClampedArray(needed);
      this.#words = new Uint32Array(this.#pixels.buffer);
    }
    if (this.#peaks.length < rows) {
      this.#peaks = new Uint8Array(rows);
    }

    const bytes = frame.bytes;
    const words = this.#words;
    let loudest = 0;
    for (let row = 0; row < rows; row += 1) {
      const source = frame.offsetAt(row);
      const target = row * CHANNELS_PER_UNIVERSE;
      let peak = 0;
      for (let channel = 0; channel < CHANNELS_PER_UNIVERSE; channel += 1) {
        const level = bytes[source + channel] ?? 0;
        if (level > peak) {
          peak = level;
        }
        words[target + channel] = PALETTE[level] ?? 0;
      }
      this.#peaks[row] = peak;
      if (peak > loudest) {
        loudest = peak;
      }
    }
    return loudest;
  }

  /** One peak meter per universe, down the right-hand side. */
  #drawMeters(geometry: LevelGeometry, rows: number): void {
    const surface = this.#surface;
    const { rowHeight, scale } = geometry;
    const inset = Math.max(1, Math.floor(rowHeight / 6));
    const x = geometry.meter.x + 4 * scale;
    const width = geometry.meter.width - 8 * scale;
    for (let row = 0; row < rows; row += 1) {
      const y = geometry.grid.y + row * rowHeight + inset;
      const height = Math.max(1, rowHeight - inset * 2);
      surface.fill(INK.trough, x, y, width, height);
      const peak = this.#peaks[row] ?? 0;
      if (peak > 0) {
        surface.fill(
          peak >= 250 ? INK.meter : INK.meterLow,
          x,
          y,
          Math.max(1, (peak / 255) * width),
          height,
        );
      }
    }
  }
}

/**
 * A {@link LevelSurface} over a real canvas, or `null` where there is no 2D
 * context — which is every `jsdom` test and any browser that has run out of
 * canvas contexts.
 *
 * `null` rather than a throw: a telemetry panel is a *view* of something that is
 * happening whether or not it can be drawn, and an interface that failed to
 * start because a canvas would not open would be an interface that lost the
 * command line as well.
 */
export function canvasSurface(canvas: HTMLCanvasElement): LevelSurface | null {
  const context = canvas.getContext("2d", { alpha: false });
  if (context === null) {
    return null;
  }
  // The scratch canvas is where the level block is put before it is scaled up.
  // `putImageData` cannot scale and `drawImage` cannot take pixels, so the two
  // are bridged by a canvas that is exactly one pixel per channel.
  const scratch = document.createElement("canvas");
  const scratchContext = scratch.getContext("2d", { alpha: false });
  if (scratchContext === null) {
    return null;
  }
  context.imageSmoothingEnabled = false;
  // Kept between frames: `createImageData` is 131 kB at 64 universes, and
  // thirty of those a second is four megabytes a second of rubbish for the
  // collector to deal with in the middle of a show.
  let image: ImageData | null = null;

  return {
    get width() {
      return canvas.width;
    },
    get height() {
      return canvas.height;
    },
    clear: (colour) => {
      context.fillStyle = colour;
      context.fillRect(0, 0, canvas.width, canvas.height);
      // A resized canvas resets its context state, so this is re-asserted here
      // rather than once at construction: a smoothed blit is a blurred grid.
      context.imageSmoothingEnabled = false;
    },
    fill: (colour, x, y, width, height) => {
      context.fillStyle = colour;
      context.fillRect(x, y, width, height);
    },
    label: (text, x, y, colour, size) => {
      context.fillStyle = colour;
      context.font = `${size}px ui-monospace, "Cascadia Mono", Menlo, monospace`;
      context.textBaseline = "top";
      context.fillText(text, x, y);
    },
    blit: (pixels, columns, rows, x, y, width, height) => {
      if (scratch.width !== columns || scratch.height !== rows) {
        scratch.width = columns;
        scratch.height = rows;
        image = null;
      }
      if (image === null || image.width !== columns || image.height !== rows) {
        image = scratchContext.createImageData(columns, rows);
      }
      image.data.set(pixels);
      scratchContext.putImageData(image, 0, 0);
      context.drawImage(scratch, 0, 0, columns, rows, x, y, width, height);
    },
  };
}
