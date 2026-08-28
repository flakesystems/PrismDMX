/**
 * The canvas, and the two claims that make it S25 rather than a window library.
 *
 * 1. **What is on the screen is what the daemon says is open.** The document
 *    these tests render is the one `crates/prismd/tests/ui_session.rs` recorded
 *    off a running `prismd`, so the windows, their order and their coordinates
 *    are not this file's invention.
 * 2. **Nothing is applied optimistically.** A drag sends `PlaceWindow`, and if
 *    no delta ever comes back the window is exactly where it started the moment
 *    the button comes up. That is the assertion the whole design turns on, and
 *    it is the one an implementation that "held the position during the drag
 *    and synced up afterwards" would fail.
 *
 * The canvas element has no size in this runtime, so a pixel is one canvas unit
 * (`toCanvasUnits` answers that deliberately, and `geometry.test.ts` says why).
 * That makes the arithmetic in these tests plain rather than scaled.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { decode } from "@msgpack/msgpack";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Delta, JsonValue } from "../bindings";
import { readServerMessage } from "../ipc/protocol";
import { nullSink, setLogSink } from "../log/logger";
import { applyDelta } from "../mirror/mirror";
import type { Documents } from "../mirror/mirror";
import { TelemetrySink } from "../ipc/telemetry";
import { Shell } from "../testing/shell";
import { DeskStore } from "../store/desk";
import { TelemetryProvider } from "../telemetry/panel";
import { Canvas } from "./canvas";
import { PLACE_INTERVAL_MS } from "./drag";
import type { Rect } from "./geometry";
import { asStyle } from "./geometry";

import recordingText from "../../tests/fixtures/session-recording.json?raw";

interface Step {
  readonly what: string;
  readonly deltas: readonly string[];
}
interface Recording {
  readonly initialSnapshot: string;
  readonly steps: readonly Step[];
}

const recording = JSON.parse(recordingText) as Recording;

/** Bytes out of a base64 payload, the way a browser does it (S23). */
function payload(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/**
 * The documents after the recorded step **whose description starts with
 * `what`** — the state of the canvas at a named moment of the script.
 *
 * **S43 replaced a count with a name here.** These tests used to say
 * `documentsThrough("resize the DMX sheet")`, and when punch-list B10 added a step to the script — the
 * drag the daemon now refuses — every one of those numbers quietly started
 * pointing one step short. They did not fail loudly; they rendered an earlier
 * canvas and compared it against coordinates from a later one. A description is
 * a reference that either resolves or throws.
 */
function documentsThrough(what: string): Documents {
  const at = recording.steps.findIndex((step) => step.what.startsWith(what));
  if (at < 0) {
    throw new Error(`the recorded script has no step "${what}"`);
  }
  return documentsAfter(at + 1);
}

/**
 * The documents after the first `steps` steps of the recorded script.
 *
 * Built by applying the daemon's deltas rather than by writing a document out:
 * what these tests render is a session `prism-core` produced.
 */
function documentsAfter(steps: number): Documents {
  const snapshot = readServerMessage(decode(payload(recording.initialSnapshot)));
  if (snapshot.t !== "Snapshot") {
    throw new Error("the recording does not start with a snapshot");
  }
  let documents: Documents = {
    show: snapshot.snapshot.show,
    session: snapshot.snapshot.session,
    programmer: snapshot.snapshot.programmer,
  };
  for (const step of recording.steps.slice(0, steps)) {
    for (const encoded of step.deltas) {
      const message = readServerMessage(decode(payload(encoded)));
      if (message.t === "Delta") {
        documents = applyDelta(documents, message.delta as Delta);
      }
    }
  }
  return documents;
}

/** A rendered canvas, with what it asked the daemon for. */
function canvas(documents: Documents) {
  const placed: { instanceId: number; rect: Rect }[] = [];
  const focused: number[] = [];
  const closed: number[] = [];
  const view = render(
    // A telemetry channel, because one window type *is* the level view. The
    // surface is `null` — jsdom has no rasteriser and S24's own tests are
    // where the drawing is asserted; what this file needs is the canvas
    // element to be in the window. And a store, because the Patch window
    // sends commands and asks questions like every other part of the desk.
    <Shell store={new DeskStore()} session={documents.session} show={documents.show}>
      <TelemetryProvider channel={{ sink: new TelemetrySink(), surface: () => null }}>
        <Canvas
          session={documents.session}
          show={documents.show}
          programmer={null}
          onPlace={(instanceId, rect) => placed.push({ instanceId, rect })}
          onFocus={(instanceId) => focused.push(instanceId)}
          onClose={(instanceId) => closed.push(instanceId)}
          onPicker={() => 0}
        />
      </TelemetryProvider>
    </Shell>,
  );
  return { placed, focused, closed, view, documents };
}

/** The `style` a window element carries, as the four things that matter. */
function styleOf(instanceId: number) {
  const element = screen.getByTestId(`window-${String(instanceId)}`);
  if (!(element instanceof HTMLElement)) {
    throw new Error("a window is an element");
  }
  return {
    left: element.style.left,
    top: element.style.top,
    width: element.style.width,
    height: element.style.height,
  };
}

/** Presses the pointer on an element. */
function press(testId: string, at: { x: number; y: number }, button = 0): void {
  fireEvent.pointerDown(screen.getByTestId(testId), {
    button,
    clientX: at.x,
    clientY: at.y,
    pointerId: 1,
  });
}

/** Moves the pointer, which the window listens for. */
function drag(to: { x: number; y: number }): void {
  fireEvent.pointerMove(window, { clientX: to.x, clientY: to.y, pointerId: 1 });
}

/** Lets it go. */
function release(): void {
  fireEvent.pointerUp(window, { pointerId: 1 });
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("what the canvas draws", () => {
  it("is the windows the daemon says are open, where it says they are", () => {
    // Window 1 dragged to 240,520 and window 2 resized to 1280 x 480 at 640,0.
    // Both are the daemon's numbers, and **both moved in S43**: punch-list B10
    // put the placement in the daemon and forbade a move that buries a
    // neighbour, so the recorded script's drags go into room that is actually
    // free. The two windows are now beside each other rather than one over the
    // other, which is the arrangement the rule exists to produce.
    canvas(documentsThrough("resize the DMX sheet"));
    expect(screen.getByTestId("canvas").dataset["windows"]).toBe("2");
    expect(styleOf(1)).toEqual(asStyle({ x: 240, y: 520, w: 640, h: 480 }));
    expect(styleOf(2)).toEqual(asStyle({ x: 640, y: 0, w: 1280, h: 480 }));
    expect(screen.getByTestId("window-1").dataset["windowType"]).toBe("FixtureSheet");
    expect(screen.getByTestId("window-2").dataset["windowType"]).toBe("DmxSheet");
  });

  it("stacks them in the order the daemon holds them, which is document order", () => {
    // After `FocusWindow(1)` the daemon's list is [2, 1]. There is no z-index
    // in the stylesheet, so the later element is the one in front — and that
    // is only right if the elements are in the daemon's order.
    canvas(documentsThrough("focus the fixture sheet"));
    const drawn = screen
      .getByTestId("canvas")
      .querySelectorAll("[data-window-type]");
    expect([...drawn].map((element) => element.getAttribute("data-testid"))).toEqual([
      "window-2",
      "window-1",
    ]);
    expect(screen.getByTestId("window-1").dataset["focused"]).toBe("yes");
    expect(screen.getByTestId("window-2").dataset["focused"]).toBe("no");
  });

  it("says so plainly when a view has nothing open in it", () => {
    // The last step of the script selects view 1, which was stored empty.
    canvas(documentsThrough("select view 1"));
    expect(screen.getByTestId("canvas-empty")).not.toBeNull();
    expect(screen.queryByTestId("window-1")).toBeNull();
  });

  it("puts the level view in the DMX sheet and the patch in the patch window", () => {
    canvas(documentsThrough("open a patch window"));
    // S24's canvas, in a window sized by the window.
    expect(screen.getByTestId("telemetry-canvas")).not.toBeNull();
    // And the show, read out of the show document by pointer.
    expect(screen.getByTestId("window-3").textContent).toContain("Fixture 1");
    expect(screen.getByTestId("window-3").textContent).toContain("Add fixture");
  });
});

describe("dragging a window", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it("sends where the pointer went and holds nothing when it gets there", () => {
    const { placed } = canvas(documentsThrough("resize the DMX sheet"));
    const before = styleOf(1);

    press("title-window-1", { x: 500, y: 400 });
    drag({ x: 560, y: 430 });

    // While the button is down the window follows the pointer — that is
    // §4.2's "drag state", and it is the only thing that is local. Down and to
    // the right, away from the other window, so nothing stops it.
    expect(styleOf(1)).toEqual(asStyle({ x: 300, y: 550, w: 640, h: 480 }));
    expect(placed).toEqual([{ instanceId: 1, rect: { x: 300, y: 550, w: 640, h: 480 } }]);

    // **The criterion.** The button comes up and no delta ever arrives: the
    // window is back where the daemon has it. Nothing was applied.
    release();
    expect(styleOf(1)).toEqual(before);
  });

  it("moves for good when the daemon says so, and not before", () => {
    const { placed, view, documents } = canvas(documentsThrough("resize the DMX sheet"));
    press("title-window-1", { x: 0, y: 0 });
    drag({ x: 100, y: 50 });
    release();
    expect(placed).toHaveLength(1);
    expect(styleOf(1)).toEqual(asStyle({ x: 240, y: 520, w: 640, h: 480 }));

    // The delta the daemon would have answered with. Only now does the window
    // move, and it moves because the document did.
    const moved = applyDelta(documents, {
      t: "SessionPatch",
      ops: [
        {
          op: "replace",
          path: "/session/openWindows/0/x",
          value: 340,
        },
      ],
    });
    view.rerender(
      <Shell store={new DeskStore()} session={moved.session} show={moved.show}>
        <TelemetryProvider channel={{ sink: new TelemetrySink(), surface: () => null }}>
          <Canvas
            session={moved.session}
            show={moved.show}
            programmer={null}
            onPlace={() => undefined}
            onFocus={() => undefined}
            onClose={() => undefined}
            onPicker={() => undefined}
          />
        </TelemetryProvider>
      </Shell>,
    );
    expect(styleOf(1)).toEqual(asStyle({ x: 340, y: 520, w: 640, h: 480 }));
  });

  it("paces what it sends, and always says where the pointer finished", () => {
    const { placed } = canvas(documentsThrough("resize the DMX sheet"));
    press("title-window-1", { x: 0, y: 0 });
    for (let step = 1; step <= 20; step += 1) {
      drag({ x: step, y: 0 });
      vi.advanceTimersByTime(1);
    }
    // Twenty pointer events over twenty milliseconds: one command, because the
    // floor is a thirtieth of a second (see `drag.ts`).
    expect(placed).toHaveLength(1);
    expect(PLACE_INTERVAL_MS).toBeGreaterThan(20);

    release();
    // And the last position is not lost to the pacing.
    expect(placed.at(-1)?.rect).toEqual({ x: 260, y: 520, w: 640, h: 480 });
  });

  it("resizes from the corner", () => {
    // **Two walls at once, and neither is a refusal.** Window 2 sits at 640,0
    // and already runs to the right-hand edge, so `resizedBy` clamps the width
    // there exactly as it always has. Downwards it is stopped by window 1, which
    // starts at y 520 and overlaps it in x — so a corner dragged 100 units down
    // grows 40 and then meets the neighbour. Before S43 it would have grown all
    // 100, sent a `PlaceWindow` the daemon refused, and snapped back.
    const { placed } = canvas(documentsThrough("resize the DMX sheet"));
    press("resize-window-2", { x: 0, y: 0 });
    drag({ x: 200, y: 100 });
    expect(styleOf(2)).toEqual(asStyle({ x: 640, y: 0, w: 1280, h: 520 }));
    release();
    expect(placed).toEqual([{ instanceId: 2, rect: { x: 640, y: 0, w: 1280, h: 520 } }]);
  });

  it("is not started by a button that is not the primary one", () => {
    // A right-click opens a context menu and produces no `pointerup`, so a
    // drag begun on one would never end.
    const { placed } = canvas(documentsThrough("resize the DMX sheet"));
    press("title-window-1", { x: 0, y: 0 }, 2);
    drag({ x: 300, y: 300 });
    release();
    expect(placed).toEqual([]);
    expect(styleOf(1)).toEqual(asStyle({ x: 240, y: 520, w: 640, h: 480 }));
  });

  /**
   * **S43, punch-list B10 second half.** The daemon refused an overlapping
   * placement from the start; what an operator saw was the window crossing its
   * neighbour and then being pulled back when the refusal arrived. It stops at
   * the edge now — and, the half that matters here, **the command that would
   * have been refused is never sent**.
   */
  it("stops a drag at the window beside it, and sends only what would be taken", () => {
    const { placed } = canvas(documentsThrough("resize the DMX sheet"));
    // Window 1 is at y 520 and window 2 fills 0…480 in the row above it. Dragged
    // hard upwards, window 1 stops with its top edge on window 2's bottom one.
    press("title-window-1", { x: 0, y: 0 });
    drag({ x: 0, y: -400 });
    expect(styleOf(1)).toEqual(asStyle({ x: 240, y: 480, w: 640, h: 480 }));
    release();
    expect(placed).toEqual([{ instanceId: 1, rect: { x: 240, y: 480, w: 640, h: 480 } }]);
  });

  it("stops listening when the pointer is cancelled", () => {
    // A touch turned into a scroll, or a window that lost the pointer. The
    // drag ends the way a release ends it.
    const { placed } = canvas(documentsThrough("resize the DMX sheet"));
    press("title-window-1", { x: 0, y: 0 });
    drag({ x: 60, y: 0 });
    fireEvent.pointerCancel(window, { pointerId: 1 });
    expect(placed).toHaveLength(1);
    drag({ x: 500, y: 500 });
    expect(placed).toHaveLength(1);
    expect(styleOf(1)).toEqual(asStyle({ x: 240, y: 520, w: 640, h: 480 }));
  });
});

describe("the other two gestures", () => {
  it("asks the daemon to focus a window that is not focused, and not one that is", () => {
    const { focused } = canvas(documentsThrough("resize the DMX sheet"));
    // Window 2 is the focused one after four steps.
    press("window-2", { x: 0, y: 0 });
    expect(focused).toEqual([]);
    press("window-1", { x: 0, y: 0 });
    expect(focused).toEqual([1]);
  });

  it("asks the daemon to close a window, and closes nothing itself", () => {
    const { closed } = canvas(documentsThrough("resize the DMX sheet"));
    fireEvent.click(screen.getByTestId("close-window-1"));
    expect(closed).toEqual([1]);
    // Still there: what closes it is the delta, not the click.
    expect(screen.getByTestId("window-1")).not.toBeNull();
  });

  it("does not drag the title bar when the close button is pressed", () => {
    const { placed, closed } = canvas(documentsThrough("resize the DMX sheet"));
    press("close-window-1", { x: 0, y: 0 });
    drag({ x: 400, y: 400 });
    release();
    expect(placed).toEqual([]);
    expect(closed).toEqual([]);
  });
});

describe("a session the canvas cannot read", () => {
  it("draws an empty canvas rather than throwing", () => {
    const empty: JsonValue = { session: {} };
    canvas({ show: {}, session: empty, programmer: documentsAfter(0).programmer });
    expect(screen.getByTestId("canvas").dataset["windows"]).toBe("0");
    expect(screen.getByTestId("canvas-empty")).not.toBeNull();
  });
});
