/**
 * The Clock Viewer — S43.
 *
 * Two things are worth a test here and they are different in kind. The **face**
 * is client-local state on a timer, so it is asserted against a clock this file
 * controls rather than against the machine's; and **what is running** is a
 * reading of the show document through `pageStrips`, so it is asserted the way
 * every other window's reading is — by giving it a document and reading the rows
 * back.
 */

import { render, screen } from "@testing-library/react";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { JsonValue } from "../bindings";
import { ClockViewer } from "./clock";

/**
 * A rig on page 0 with executor 1 running and executor 3 assigned but stopped.
 *
 * Executor 2 is running with **no cue index**, which is the state a playback is
 * in between being switched on and the tick answering back — S26 and S28 both
 * had to draw a dash for it.
 */
const SHOW: JsonValue = {
  executors: {
    "1": { id: 1, sequenceId: 1, isActive: true, currentCueIndex: 0, masterLevel: 65535 },
    "2": { id: 2, sequenceId: 2, isActive: true, currentCueIndex: null, masterLevel: 65535 },
    "3": { id: 3, sequenceId: 1, isActive: false, currentCueIndex: null, masterLevel: 0 },
  },
  sequences: {
    "1": { id: 1, name: "Act one", cues: [] },
    "2": { id: 2, name: "", cues: [] },
  },
};

/** Page 0, which is where executors 0–7 are. */
const SESSION: JsonValue = { session: { executorPage: 0 } };

beforeEach(() => {
  vi.useFakeTimers();
  // A time with a fractional second in it, so the alignment below has something
  // to align to.
  vi.setSystemTime(new Date(2026, 7, 28, 14, 32, 7, 900));
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the clock viewer", () => {
  it("shows the time and the date in a form that cannot be read the American way", () => {
    render(<ClockViewer show={{}} session={{}} />);
    expect(screen.getByTestId("clock-time").textContent).toBe("14:32:07");
    // `28 Aug 2026` — the month is a word, so nothing here is `08/28` or
    // `28/08` and a reader from either side of the Atlantic gets it right.
    expect(screen.getByTestId("clock-date").textContent).toBe("28 Aug 2026");
  });

  /**
   * **Aligned to the next whole second, then a plain interval.**
   *
   * A clock started at an arbitrary offset reads the previous second for most of
   * the next one, which on a face an operator glances at from the back of the
   * room is simply a wrong clock. The first timer is therefore the remainder of
   * the current second — 100 ms here — and every one after it is a full second.
   */
  it("aligns to the next whole second and then ticks once a second", () => {
    render(<ClockViewer show={{}} session={{}} />);
    expect(screen.getByTestId("clock-time").textContent).toBe("14:32:07");

    // 99 ms is not yet the boundary: the face has not moved.
    act(() => {
      vi.advanceTimersByTime(99);
    });
    expect(screen.getByTestId("clock-time").textContent).toBe("14:32:07");

    // The remaining millisecond crosses it.
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.getByTestId("clock-time").textContent).toBe("14:32:08");

    // And from here it is a second at a time, on the whole second.
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(screen.getByTestId("clock-time").textContent).toBe("14:32:09");
  });

  it("stops its timers when the window is closed", () => {
    const { unmount } = render(<ClockViewer show={{}} session={{}} />);
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    unmount();
    // Nothing is left to fire, so a closed window costs a shut desk nothing —
    // and a `setState` after unmount would be an error in the console rather
    // than a silent leak.
    expect(vi.getTimerCount()).toBe(0);
  });

  describe("what is running", () => {
    it("says so plainly when nothing on this page is", () => {
      render(<ClockViewer show={{ executors: {} }} session={SESSION} />);
      expect(screen.getByTestId("clock-idle").textContent).toContain("Nothing is running");
      expect(screen.queryByTestId("clock-running")).toBeNull();
    });

    /**
     * One row per **running** executor of the page, and the step column is the
     * position in the list rather than the cue's number — `currentCueIndex` is
     * an index, and a column headed *Step* showing `1` for index 0 is the one
     * reading that cannot be confused with the `2.5` in the Sequence Sheet.
     */
    it("lists the running executors of this page, with the step counted from one", () => {
      render(<ClockViewer show={SHOW} session={SESSION} />);
      expect(screen.getByTestId("clock-row-1")).toBeTruthy();
      expect(screen.getByTestId("clock-row-2")).toBeTruthy();
      // Assigned but stopped, so it is not here: this window is about what is
      // *running*.
      expect(screen.queryByTestId("clock-row-3")).toBeNull();
      expect(screen.queryByTestId("clock-idle")).toBeNull();

      expect(screen.getByTestId("clock-row-1").textContent).toContain("Act one");
      expect(screen.getByTestId("clock-step-1").textContent).toBe("1");
      // A playback the tick has not answered for yet: a dash, not a zero and
      // not a one.
      expect(screen.getByTestId("clock-step-2").textContent).toBe("—");
    });

    /**
     * **The page and not the whole show** — D7. An operator looking at page 1
     * is looking at executors 8–15, and a rig with twelve pages of chases would
     * otherwise fill this window with rows nobody is watching.
     */
    it("follows the executor page", () => {
      render(<ClockViewer show={SHOW} session={{ session: { executorPage: 1 } }} />);
      expect(screen.getByTestId("clock-idle")).toBeTruthy();
      expect(screen.queryByTestId("clock-row-1")).toBeNull();
    });
  });
});
