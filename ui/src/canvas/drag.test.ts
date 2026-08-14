/**
 * The cadence rule, asserted rather than described.
 *
 * `./drag.ts` says three things, and each of them is a test here: the daemon is
 * told at a bounded rate; the rectangle the pointer describes is available to
 * the screen but is never the state; and a drag that moved nowhere says
 * nothing.
 *
 * There is no clock in this file and no DOM. `now` is an argument, so a
 * thirty-third of a second is a number rather than a wait.
 */

import { describe, expect, it } from "vitest";

import { WindowDrag, PLACE_INTERVAL_MS } from "./drag";
import type { Rect } from "./geometry";

/** Where a window starts. */
const ORIGIN: Rect = { x: 100, y: 100, w: 640, h: 480 };

/** A drag that records what it sent, with one canvas unit per pixel. */
function drag(kind: "move" | "resize" = "move") {
  const sent: Rect[] = [];
  const subject = new WindowDrag({
    instanceId: 7,
    kind,
    origin: ORIGIN,
    from: { x: 500, y: 400 },
    place: (rect) => sent.push(rect),
    scale: (dx, dy) => ({ x: dx, y: dy }),
  });
  return { subject, sent };
}

describe("a drag", () => {
  it("names the window it is of, and starts where the daemon has it", () => {
    const { subject, sent } = drag();
    expect(subject.instanceId).toBe(7);
    expect(subject.rect).toEqual(ORIGIN);
    expect(sent).toEqual([]);
  });

  it("sends the first movement at once, and then no faster than the floor", () => {
    const { subject, sent } = drag();
    // The first is immediate: an operator who nudges a window should see it
    // move now, not a thirtieth of a second from now.
    subject.to({ x: 510, y: 400 }, 1000);
    expect(sent).toEqual([{ x: 110, y: 100, w: 640, h: 480 }]);

    // Everything inside the interval is shown and not sent.
    subject.to({ x: 520, y: 400 }, 1000 + PLACE_INTERVAL_MS - 1);
    expect(subject.rect).toEqual({ x: 120, y: 100, w: 640, h: 480 });
    expect(sent).toHaveLength(1);

    // And the next one after it is sent — carrying where the pointer is *now*,
    // not the position that was skipped.
    subject.to({ x: 530, y: 400 }, 1000 + PLACE_INTERVAL_MS);
    expect(sent).toEqual([
      { x: 110, y: 100, w: 640, h: 480 },
      { x: 130, y: 100, w: 640, h: 480 },
    ]);
  });

  it("sends where the pointer finished, however short the drag was", () => {
    const { subject, sent } = drag();
    subject.to({ x: 510, y: 400 }, 0);
    subject.to({ x: 516, y: 406 }, 1);
    // The second movement was inside the interval, so it has not been sent.
    expect(sent).toHaveLength(1);
    subject.end(2);
    expect(sent.at(-1)).toEqual({ x: 116, y: 106, w: 640, h: 480 });
  });

  it("says nothing at all when the pointer went nowhere", () => {
    // A click on a title bar to focus a window is a drag of zero pixels, and
    // it must not become a command.
    const { subject, sent } = drag();
    subject.to({ x: 500, y: 400 }, 0);
    subject.to({ x: 500, y: 400 }, 5000);
    subject.end(5001);
    expect(sent).toEqual([]);
  });

  it("says nothing when the pointer moved and the rectangle did not", () => {
    // Dragging further into the edge the window is already against. The
    // clamping is `movedBy`'s; what this asserts is that an unchanged
    // rectangle is not resent thirty times a second.
    const { subject, sent } = drag();
    subject.to({ x: 0, y: 0 }, 0);
    expect(sent).toEqual([{ x: 0, y: 0, w: 640, h: 480 }]);
    subject.to({ x: -100, y: -100 }, 1000);
    subject.to({ x: -200, y: -200 }, 2000);
    subject.end(3000);
    expect(sent).toHaveLength(1);
  });

  it("resizes by the corner when that is what was grabbed", () => {
    const { subject, sent } = drag("resize");
    subject.to({ x: 600, y: 500 }, 0);
    expect(sent).toEqual([{ x: 100, y: 100, w: 740, h: 580 }]);
  });

  it("measures from where the pointer started, not from the last event", () => {
    // The rectangle is `origin + total displacement`, so a stream of events
    // cannot accumulate rounding — a drag out and back returns exactly.
    const { subject } = drag();
    subject.to({ x: 500.4, y: 400.4 }, 0);
    subject.to({ x: 700.6, y: 400.6 }, 100);
    subject.to({ x: 500, y: 400 }, 200);
    expect(subject.rect).toEqual(ORIGIN);
  });

  it("scales pixels into canvas units through whatever it was given", () => {
    const sent: Rect[] = [];
    const subject = new WindowDrag({
      instanceId: 1,
      kind: "move",
      origin: ORIGIN,
      from: { x: 0, y: 0 },
      place: (rect) => sent.push(rect),
      // A canvas half the width of the coordinate space: one pixel, two units.
      scale: (dx, dy) => ({ x: dx * 2, y: dy }),
    });
    subject.to({ x: 10, y: 10 }, 0);
    expect(sent).toEqual([{ x: 120, y: 110, w: 640, h: 480 }]);
  });
});
