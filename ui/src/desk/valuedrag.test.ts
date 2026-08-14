/**
 * The cadence, asserted rather than waited for.
 *
 * The clock is an argument, which is what lets "at most one command every
 * 33 ms" be a test rather than a stopwatch — the rule `canvas/drag.test.ts`
 * established for windows, one layer down.
 *
 * The claim that matters is the last one in the first block: **the local value
 * is dropped**, so a fader pulled against a daemon that never answers springs
 * back to where the daemon has it. There is no state here to roll back, which
 * is the whole of D3 in this layer.
 */

import { describe, expect, it } from "vitest";

import { clampLevel, levelFromPercent, percentOfLevel, wholePercent } from "./level";
import { EncoderDrag, ENCODER_FINE_TRAVEL, ENCODER_TRAVEL, SEND_INTERVAL_MS, ValueDrag } from "./valuedrag";

describe("a fader", () => {
  /** A drag over a 100-pixel fader starting at half, collecting what goes out. */
  function fader(origin = 32768) {
    const sent: number[] = [];
    const drag = new ValueDrag({
      origin,
      from: 100,
      travel: 100,
      inverted: true,
      send: (level) => sent.push(level),
    });
    return { drag, sent };
  }

  it("shows where the pointer is, and pulling up raises the level", () => {
    const { drag } = fader(0);
    // A hundred pixels of travel is the whole range, and up is *less* clientY.
    expect(drag.to(50, 0)).toBe(32768);
    expect(drag.to(0, 0)).toBe(65535);
    expect(drag.to(200, 0)).toBe(0);
  });

  it("sends at most one command every interval, plus one on release", () => {
    const { drag, sent } = fader(0);
    drag.to(99, 0);
    expect(sent.length).toBe(1);
    // Nine more moves inside the interval say nothing.
    for (let step = 1; step <= 9; step += 1) {
      drag.to(99 - step, step * 3);
    }
    expect(sent.length).toBe(1);
    drag.to(80, SEND_INTERVAL_MS);
    expect(sent.length).toBe(2);
    // And the release carries where the pointer actually finished.
    drag.to(60, SEND_INTERVAL_MS + 1);
    drag.end(SEND_INTERVAL_MS + 2);
    expect(sent.length).toBe(3);
    expect(sent.at(-1)).toBe(drag.level);
  });

  it("says nothing at all for a drag that has not moved", () => {
    const { drag, sent } = fader();
    drag.to(100, 0);
    drag.to(100, 1000);
    drag.end(2000);
    expect(sent).toEqual([]);
  });

  it("sends a flick shorter than one interval when the button comes up", () => {
    // Without the release flush, a quick nudge would move the fader on the
    // screen and nowhere else.
    const { drag, sent } = fader(0);
    drag.to(90, 0);
    drag.end(1);
    expect(sent).toEqual([6554]);
  });

  it("holds nothing after the button comes up", () => {
    // The rule. The drag object is finished with; what the fader renders from
    // this moment is the session's level, and the component drops the local
    // value — asserted through the component in `executorbar.test.tsx`.
    const { drag, sent } = fader(0);
    drag.to(0, 0);
    drag.end(1);
    expect(sent).toEqual([65535]);
    // A second `end` says nothing: there is nothing owed.
    drag.end(2);
    expect(sent).toEqual([65535]);
  });

  it("answers one level per pixel for an element that has not been laid out", () => {
    // A zero-height element is what `getBoundingClientRect` reports before a
    // layout, and dividing by it would put `Infinity` into a command.
    const sent: number[] = [];
    const drag = new ValueDrag({
      origin: 0,
      from: 0,
      travel: 0,
      send: (level) => sent.push(level),
    });
    expect(drag.to(10, 0)).toBe(10);
    expect(Number.isFinite(drag.level)).toBe(true);
  });

  it("clamps at both ends rather than sending a level the protocol has not got", () => {
    const { drag, sent } = fader(32768);
    drag.to(-1000, 0);
    expect(drag.level).toBe(65535);
    drag.to(2000, 1000);
    expect(drag.level).toBe(0);
    expect(sent.every((level) => level >= 0 && level <= 65535)).toBe(true);
  });

  it("raises the level as the pointer goes down when it is not inverted", () => {
    const sent: number[] = [];
    const drag = new ValueDrag({ origin: 0, from: 0, travel: 100, send: (l) => sent.push(l) });
    expect(drag.to(50, 0)).toBe(32768);
  });
});

describe("an encoder", () => {
  /** A turn, collecting the relative deltas that go out. */
  function encoder(fine = false) {
    const sent: number[] = [];
    const drag = new EncoderDrag({ from: 0, fine, send: (delta) => sent.push(delta) });
    return { drag, sent };
  }

  it("sends what has not been sent yet, never the total", () => {
    // Two commands carrying the same total would move the value twice.
    const { drag, sent } = encoder();
    drag.to(ENCODER_TRAVEL / 4, 0);
    drag.to(ENCODER_TRAVEL / 2, SEND_INTERVAL_MS);
    drag.end(SEND_INTERVAL_MS * 2);
    expect(sent).toEqual([16384, 16384]);
    // Which adds up to exactly where the pointer went.
    expect(sent.reduce((total, delta) => total + delta, 0)).toBe(drag.turned);
  });

  it("is paced like everything else, and flushes what is owed on release", () => {
    const { drag, sent } = encoder();
    drag.to(10, 0);
    expect(sent.length).toBe(1);
    drag.to(20, 5);
    drag.to(30, 10);
    expect(sent.length).toBe(1);
    drag.end(11);
    expect(sent.length).toBe(2);
  });

  it("says nothing for a turn that went nowhere", () => {
    const { drag, sent } = encoder();
    drag.to(0, 0);
    drag.end(100);
    expect(sent).toEqual([]);
  });

  it("turns eight times more slowly with the fine rate", () => {
    const coarse = encoder(false);
    coarse.drag.to(64, 0);
    const fine = encoder(true);
    fine.drag.to(64, 0);
    expect(ENCODER_FINE_TRAVEL / ENCODER_TRAVEL).toBe(8);
    expect(coarse.drag.turned).toBe(fine.drag.turned * 8);
  });

  it("goes both ways, because an encoder does", () => {
    const { drag, sent } = encoder();
    drag.to(-ENCODER_TRAVEL / 2, 0);
    expect(sent).toEqual([-32767]);
  });
});

describe("percentages and levels", () => {
  it("truncates a percentage and never rounds past it", () => {
    expect(levelFromPercent(0)).toBe(0);
    expect(levelFromPercent(50)).toBe(32767);
    expect(levelFromPercent(100)).toBe(65535);
    // Outside the range is clamped: a fader pulled past its end stop is an
    // ordinary gesture, and the console refuses a bad percentage before here.
    expect(levelFromPercent(-5)).toBe(0);
    expect(levelFromPercent(500)).toBe(65535);
    expect(levelFromPercent(Number.NaN)).toBe(0);
  });

  it("reads a level back as a percentage, to one decimal", () => {
    expect(percentOfLevel(0)).toBe(0);
    expect(percentOfLevel(65535)).toBe(100);
    expect(percentOfLevel(32767)).toBe(50);
    expect(percentOfLevel(16383)).toBe(25);
    expect(wholePercent(32767)).toBe(50);
    expect(wholePercent(1)).toBe(0);
  });

  it("keeps a level inside the range the protocol carries", () => {
    expect(clampLevel(-1)).toBe(0);
    expect(clampLevel(70000)).toBe(65535);
    expect(clampLevel(1.6)).toBe(2);
    expect(clampLevel(Number.POSITIVE_INFINITY)).toBe(0);
  });
});
