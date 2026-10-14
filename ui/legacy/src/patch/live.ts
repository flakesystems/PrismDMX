/**
 * The output column of the Fixture Sheet: what is on the **cable**, per fixture,
 * drawn on a canvas.
 *
 * # Two live columns, and only one of them may be React
 *
 * A fixture sheet shows two live things and they are allowed to disagree —
 * that is the whole point of showing both. What the *programmer* holds is
 * control state: it arrives as `Delta::ProgrammerChanged` at the rate an
 * operator turns an encoder, and it renders in React like every other reader.
 * What is on the *cable* is telemetry: 30 Hz, and `docs/IPC_PROTOCOL.md` §7's
 * last paragraph says it must never reach reactive state. So this column is
 * drawn, and `telemetry/render.test.tsx` — which counts React commits over 300
 * frames and must stay at zero — is what says so on every commit.
 *
 * The bricks are S24's, deliberately: {@link drawFixtureLevels} paints through
 * `LevelSurface`, so it is asserted per pixel in a runtime with no rasteriser,
 * and {@link driveFixtureLevels} is `telemetry/driver.ts` with a different
 * picture in it.
 *
 * # Only what is on the screen is drawn
 *
 * A rig of four hundred fixtures is four hundred rows, and a window shows
 * perhaps twenty of them. The canvas is the size of the window's body and the
 * loop reads the scroll offset out of the element each frame — a scroll position
 * is client-local (`ARCHITECTURE_SPEC.md` §4.2), so it is read where it lives
 * and no command is sent about it — so the cost of a frame is the rows visible
 * rather than the rows patched.
 */

import type { JsonValue } from "../bindings";
import { isObject } from "../mirror/patch";
import { numberAt, pointerToken, stringAt, valueAt } from "../mirror/select";
import type { TelemetrySink } from "../ipc/telemetry";
import type { Scheduler } from "../telemetry/driver";
import { animationFrames } from "../telemetry/driver";
import { TelemetryFrameView } from "../telemetry/frame";
import type { LevelSurface } from "../telemetry/painter";

/** How tall one row of the sheet is, in CSS pixels. Matches `.sheet-live` in the stylesheet. */
export const ROW_HEIGHT = 22;

/** The colours. Dark, because a lighting desk is used in a dark room. */
const INK = {
  /** Behind everything. */
  background: "#0b0e13",
  /** A channel at zero, and the trough behind every bar. */
  trough: "#161d27",
  /** A channel below full. */
  level: "#4bb3c4",
  /** A channel at full. */
  full: "#ffd479",
  /** A fixture whose universe the daemon is not publishing. */
  absent: "#2a323d",
} as const;

/** One fixture, as the output column needs it. */
export interface LiveFixture {
  /** The fixture number, so a row can be matched to the table beside it. */
  readonly id: number;
  /** Which universe its channels are in. */
  readonly universe: number;
  /** Its start address, `1..=512`. */
  readonly address: number;
  /** How many channels it occupies. 0 when the show cannot say. */
  readonly footprint: number;
}

/**
 * The patched fixtures with the channels they occupy, in fixture-number order.
 *
 * Read out of the show document, like everything else in this directory. The
 * *footprint* comes from the embedded profile, which is where the show keeps it;
 * what is done with it here is drawing rather than deciding, so this is not the
 * arithmetic `patch.ts` refuses to do (see that file).
 */
export function liveFixtures(show: JsonValue | null): readonly LiveFixture[] {
  const fixtures = valueAt(show, "/fixtures");
  if (!isObject(fixtures)) {
    return [];
  }
  const rows: LiveFixture[] = [];
  for (const [key, entry] of Object.entries(fixtures)) {
    const id = Number(key);
    if (!Number.isInteger(id) || !isObject(entry)) {
      continue;
    }
    const typeId = stringAt(entry, "/typeId") ?? "";
    rows.push({
      id,
      universe: numberAt(entry, "/universe") ?? 0,
      address: numberAt(entry, "/address") ?? 0,
      footprint: numberAt(show, `/fixtureTypes/${pointerToken(typeId)}/footprint`) ?? 0,
    });
  }
  return rows.sort((left, right) => left.id - right.id);
}

/** Where the rows are, relative to the surface. */
export interface LiveGeometry {
  /** Device pixels per CSS pixel. */
  readonly scale: number;
  /** How far the body has been scrolled, in CSS pixels. */
  readonly scrollTop: number;
  /** Index of the first row that is at least partly on the screen. */
  readonly firstRow: number;
  /** Index one past the last row that is at least partly on the screen. */
  readonly lastRow: number;
}

/** Which rows a surface of this height shows, given the scroll offset. */
export function visibleRows(
  height: number,
  scrollTop: number,
  rows: number,
  scale = 1,
): LiveGeometry {
  const rowHeight = ROW_HEIGHT * scale;
  const top = Math.max(0, scrollTop);
  const firstRow = Math.max(0, Math.floor(top / ROW_HEIGHT));
  const visible = rowHeight <= 0 ? 0 : Math.ceil(height / rowHeight) + 1;
  return {
    scale,
    scrollTop: top,
    firstRow: Math.min(firstRow, rows),
    lastRow: Math.min(rows, firstRow + Math.max(0, visible)),
  };
}

/**
 * Draws one bar per channel of every visible fixture.
 *
 * A fixture whose universe the frame does not carry is drawn in
 * {@link INK.absent} rather than at zero: *no output* and *output at zero* are
 * different facts, and a sheet that showed them the same way would tell an
 * operator their rig was dark when it is in fact unpatched — which is the very
 * mistake this session's exit criteria are about. That includes every universe
 * in a show whose output has not been configured, which is **S33**'s to fix and
 * this session's only to make visible.
 */
export function drawFixtureLevels(
  surface: LevelSurface,
  fixtures: readonly LiveFixture[],
  frame: TelemetryFrameView,
  geometry: LiveGeometry,
): number {
  surface.clear(INK.background);
  const { scale } = geometry;
  const rowHeight = ROW_HEIGHT * scale;
  const inset = Math.max(1, Math.floor(rowHeight / 8));
  let drawn = 0;

  for (let row = geometry.firstRow; row < geometry.lastRow; row += 1) {
    const fixture = fixtures[row];
    if (fixture === undefined) {
      continue;
    }
    const y = (row * ROW_HEIGHT - geometry.scrollTop) * scale + inset;
    const height = Math.max(1, rowHeight - inset * 2);
    const index = universeIndex(frame, fixture.universe);
    if (index === null || fixture.footprint <= 0) {
      surface.fill(INK.absent, 0, y, surface.width, height);
      drawn += 1;
      continue;
    }
    const width = surface.width / fixture.footprint;
    for (let channel = 0; channel < fixture.footprint; channel += 1) {
      const level = frame.levelAt(index, fixture.address - 1 + channel);
      const x = channel * width;
      surface.fill(INK.trough, x, y, Math.max(1, width - scale), height);
      if (level > 0) {
        const filled = Math.max(1, (level / 255) * (width - scale));
        surface.fill(level >= 250 ? INK.full : INK.level, x, y, filled, height);
      }
    }
    drawn += 1;
  }
  return drawn;
}

/** Where a universe sits in a frame, or `null` when the frame has not got it. */
function universeIndex(frame: TelemetryFrameView, universe: number): number | null {
  for (let index = 0; index < frame.count; index += 1) {
    if (frame.universeAt(index) === universe) {
      return index;
    }
  }
  return null;
}

/** How to drive the output column. Every clock is an argument, as always. */
export interface LiveDriverOptions {
  /** Where the payloads are. */
  readonly sink: TelemetrySink;
  /** What to draw on, or `null` where there is no canvas. */
  readonly surface: LevelSurface | null;
  /** The rows, read each frame so a repatch is followed without a re-subscribe. */
  readonly fixtures: () => readonly LiveFixture[];
  /** How far the body is scrolled, in CSS pixels. */
  readonly scrollTop: () => number;
  /** Device pixels per CSS pixel. */
  readonly scale?: () => number;
  /** What calls the loop. */
  readonly scheduler?: Scheduler;
}

/** A running output column. */
export interface LiveDriver {
  /** Stops the loop. */
  stop: () => void;
  /** How many frames have been drawn. Read by the tests. */
  readonly painted: () => number;
}

/**
 * Starts drawing the sink's frames into the output column.
 *
 * Redraws when the payload is one it has not drawn, when the sheet has been
 * scrolled, or when the rows have changed — and otherwise does nothing at all,
 * which is what makes a sheet nobody is touching free.
 */
export function driveFixtureLevels(options: LiveDriverOptions): LiveDriver {
  const { sink, surface } = options;
  const scale = options.scale ?? (() => 1);
  const schedule = options.scheduler ?? animationFrames;
  const view = new TelemetryFrameView();
  let drawnPayload: Uint8Array | null = null;
  let drawnTop = -1;
  let drawnRows: readonly LiveFixture[] | null = null;
  let painted = 0;

  const stop = schedule(() => {
    if (surface === null) {
      return;
    }
    const payload = sink.latest;
    const fixtures = options.fixtures();
    const top = options.scrollTop();
    if (payload === drawnPayload && top === drawnTop && fixtures === drawnRows) {
      return;
    }
    drawnPayload = payload;
    drawnTop = top;
    drawnRows = fixtures;

    if (payload !== null && view.read(payload) !== null) {
      // A frame this build cannot read costs one picture and nothing else
      // (§7): the view still holds the last readable frame, so the column goes
      // on showing that rather than emptying.
      return;
    }
    if (payload === null) {
      view.clear();
    }
    drawFixtureLevels(
      surface,
      fixtures,
      view,
      visibleRows(surface.height, top, fixtures.length, scale()),
    );
    painted += 1;
  });

  return { stop, painted: () => painted };
}
