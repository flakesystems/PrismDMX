/**
 * How much the visualiser draws — **S30b**, and the owner's own condition:
 * *detail may be a setting, for performance, but there has to be a lot more
 * in it.*
 *
 * The setting is **client-local** (`ARCHITECTURE_SPEC.md` §4.2): a laptop on
 * the tech table and a big screen in the gallery look at one rig at two
 * levels of detail, and neither is the desk's business. It is kept in the
 * browser's storage and read back when the window opens.
 */

/** The four levels, lowest first. */
export const DETAIL_LEVELS = ["low", "medium", "high", "ultra"] as const;

/** One of {@link DETAIL_LEVELS}. */
export type DetailLevel = (typeof DETAIL_LEVELS)[number];

/** What a level turns on. */
export interface Detail {
  /** The fixtures' own 3D models, where the desk has them. */
  readonly models: boolean;
  /** Samples along each ray through a beam; nought draws a flat cone. */
  readonly steps: number;
  /** Beams thrown onto the floor with their gobo, colour and blades — the brightest first. */
  readonly projectors: number;
  /**
   * Beams drawn in the haze, the brightest first; the rest still light the
   * floor and their lens. Every shaft is drawn over everything behind it, and
   * two hundred and fifty of them in one place — the 64-universe test's rig,
   * all at the origin — is more filling than a software renderer can do
   * beside a console.
   */
  readonly volumes: number;
  /** Edge of the square the gobo, iris and blades are drawn into, pixels. */
  readonly mask: number;
  /** Segments round a beam's cone. */
  readonly segments: number;
  /** A glow around bright light. */
  readonly bloom: boolean;
  /** Device pixels per CSS pixel is capped at this. */
  readonly pixelRatio: number;
}

/** The levels. */
export const DETAIL: Readonly<Record<DetailLevel, Detail>> = {
  low: { models: false, steps: 0, projectors: 16, volumes: 24, mask: 64, segments: 24, bloom: false, pixelRatio: 0.75 },
  medium: { models: true, steps: 12, projectors: 32, volumes: 64, mask: 128, segments: 40, bloom: false, pixelRatio: 1 },
  high: { models: true, steps: 24, projectors: 48, volumes: 160, mask: 256, segments: 64, bloom: true, pixelRatio: 1.5 },
  ultra: { models: true, steps: 48, projectors: 64, volumes: 400, mask: 512, segments: 96, bloom: true, pixelRatio: 2 },
};

/** The level a window starts at when the browser remembers none. */
export const DEFAULT_DETAIL: DetailLevel = "medium";

/** The words the setting is shown as. */
export const DETAIL_NAMES: Readonly<Record<DetailLevel, string>> = {
  low: "Low",
  medium: "Medium",
  high: "High",
  ultra: "Ultra",
};

const STORAGE_KEY = "prismdmx.viewer.detail";
const HAZE_KEY = "prismdmx.viewer.haze";

/** Whether a string is a level. */
export function isDetailLevel(value: unknown): value is DetailLevel {
  return typeof value === "string" && (DETAIL_LEVELS as readonly string[]).includes(value);
}

/**
 * The level this browser last chose, or `fallback` when it has chosen none —
 * the default, or the lowest on a software renderer (`./graphics.ts`).
 */
export function rememberedDetail(fallback: DetailLevel = DEFAULT_DETAIL): DetailLevel {
  try {
    const held = globalThis.localStorage?.getItem(STORAGE_KEY);
    return isDetailLevel(held) ? held : fallback;
  } catch {
    return fallback;
  }
}

/** Remembers a level for this browser. A browser that will not store is not an error. */
export function rememberDetail(level: DetailLevel): void {
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, level);
  } catch {
    // Private windows and full disks: the setting simply does not survive.
  }
}

/** How thick the haze is, `0..=1`, as this browser last chose. */
export function rememberedHaze(): number {
  try {
    const held = Number(globalThis.localStorage?.getItem(HAZE_KEY));
    return Number.isFinite(held) && held >= 0 && held <= 1 && globalThis.localStorage?.getItem(HAZE_KEY) !== null ? held : 0.5;
  } catch {
    return 0.5;
  }
}

/** Remembers the haze for this browser. */
export function rememberHaze(haze: number): void {
  try {
    globalThis.localStorage?.setItem(HAZE_KEY, String(haze));
  } catch {
    // As above.
  }
}
