/**
 * **The three exit criteria of S25, through the whole interface.**
 *
 * `canvas.test.tsx` drives the canvas component; this drives `<App />` with a
 * daemon on the other end of a socket, so what is asserted is what an operator
 * would see:
 *
 * 1. **Opening, moving and closing a window issues a session command, and the
 *    interface does not hold that state.** Every gesture is followed to the
 *    bytes on the socket, and then the canvas is checked to have *not* changed
 *    until the delta arrives.
 * 2. **A view switched at the console appears at once.** The console's press
 *    reaches this interface as a `SessionPatch` and nothing else; the delta used
 *    here is one `prism-core` produced, replayed out of the recording, and the
 *    Rust half of the claim — that a console press with a client connected
 *    really does produce it — is `crates/prismd/tests/surface_gate.rs`. The
 *    browser half against a real daemon and a real console is
 *    `ui/e2e/session.spec.ts`.
 * 3. **The layout survives a restart of the interface because it lives in the
 *    session.** Here that is a second interface handed the same snapshot; in
 *    the end-to-end suite it is `page.reload()` against a daemon that is never
 *    told anything happened.
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import type { Command, Delta, JsonValue } from "../bindings";
import { Connection } from "../ipc/connection";
import { readServerMessage } from "../ipc/protocol";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import { FakeNetwork, ManualTimer, serverMessage, snapshot } from "../testing/fake-daemon";
import { TelemetryProvider } from "../telemetry/panel";

import recordingText from "../../tests/fixtures/session-recording.json?raw";

interface Recording {
  readonly initialSnapshot: string;
  readonly steps: readonly { readonly deltas: readonly string[] }[];
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

/** The snapshot the recorded script starts from — a real daemon's. */
function recordedSnapshot(): Snapshot {
  const message = readServerMessage(decode(payload(recording.initialSnapshot)));
  if (message.t !== "Snapshot") {
    throw new Error("the recording does not start with a snapshot");
  }
  return message.snapshot;
}

/** The deltas of one recorded step, as the daemon sent them. */
function recordedDeltas(step: number): Delta[] {
  const entry = recording.steps[step];
  if (entry === undefined) {
    throw new Error(`the recording has no step ${String(step)}`);
  }
  return entry.deltas.map((encoded) => {
    const message = readServerMessage(decode(payload(encoded)));
    if (message.t !== "Delta") {
      throw new Error("that payload is not a delta");
    }
    return message.delta;
  });
}

/** A whole interface with a daemon the test drives. */
function desk(served: Snapshot = recordedSnapshot()) {
  const network = new FakeNetwork();
  const clock = new ManualTimer();
  const store = new DeskStore();
  const events = deskEvents(store, (reason) => {
    connection.resync(reason);
  });
  const connection = new Connection(
    { url: "ws://127.0.0.1:7373/ipc", socketFactory: network.factory, timer: clock.timer },
    events,
  );
  store.attach((command) => connection.send(command));

  const view = render(
    <DeskProvider store={store}>
      <TelemetryProvider channel={{ sink: new TelemetrySink(), surface: () => null }}>
        <App />
      </TelemetryProvider>
    </DeskProvider>,
  );
  act(() => {
    connection.start();
    network.last.open();
    network.last.deliver(serverMessage({ t: "Snapshot", snapshot: served }));
  });

  /** Every command the interface has sent, in order. */
  const commands = (): Command[] =>
    network.last.sent
      .map((sent) => decode(sent))
      .filter(
        (message): message is { t: "Command"; seq: number; command: Command } =>
          typeof message === "object" &&
          message !== null &&
          (message as { t?: unknown }).t === "Command",
      )
      .map((message) => message.command);

  /** The daemon answers with deltas. */
  const answer = (deltas: readonly Delta[]): void => {
    act(() => {
      for (const delta of deltas) {
        network.last.deliver(serverMessage({ t: "Delta", delta }));
      }
    });
  };

  return { network, store, view, commands, answer };
}

/** The numbers of the windows on the canvas, in the order they are drawn. */
function drawn(): string[] {
  return [...screen.getByTestId("canvas").querySelectorAll("[data-window-type]")].map(
    (element) => element.getAttribute("data-testid") ?? "",
  );
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("opening and closing a window", () => {
  it("issues a session command and changes nothing until the daemon answers", () => {
    const { commands, answer } = desk();
    expect(drawn()).toEqual([]);

    // The picker is the whole vocabulary, from the generated table.
    fireEvent.change(screen.getByTestId("open-window"), { target: { value: "DmxSheet" } });

    // **The first exit criterion.** A command went out.
    expect(commands()).toEqual([{ t: "OpenWindow", window: "DmxSheet" }]);
    // And nothing opened: there is nowhere in this interface for a window to
    // be, other than the session (D3).
    expect(drawn()).toEqual([]);
    expect(screen.getByTestId("open-windows").textContent).toBe("0");

    // The daemon's own deltas for the first two steps of the recorded script.
    answer(recordedDeltas(0));
    expect(drawn()).toEqual(["window-1"]);
    answer(recordedDeltas(1));
    expect(drawn()).toEqual(["window-1", "window-2"]);
    expect(screen.getByTestId("open-windows").textContent).toBe("2");
  });

  it("asks for a window to be closed rather than closing one", () => {
    const { commands, answer } = desk();
    answer(recordedDeltas(0));
    answer(recordedDeltas(1));

    fireEvent.click(screen.getByTestId("close-window-2"));
    expect(commands()).toEqual([{ t: "CloseWindow", instanceId: 2 }]);
    expect(drawn()).toEqual(["window-1", "window-2"]);

    // Step 7 of the script is `CloseWindow(2)`, so these are the very deltas
    // this command produces at a daemon.
    for (const step of [2, 3, 4, 5, 6]) {
      answer(recordedDeltas(step));
    }
    answer(recordedDeltas(7));
    expect(drawn()).toEqual(["window-1", "window-3"]);
  });

  it("asks for the window under the pointer to be brought to the front", () => {
    const { commands, answer } = desk();
    answer(recordedDeltas(0));
    answer(recordedDeltas(1));
    // Window 2 is the focused one, so pressing window 1 is a `FocusWindow` —
    // and the stacking order does not change until the delta says so.
    fireEvent.pointerDown(screen.getByTestId("window-1"), { button: 0, pointerId: 1 });
    expect(commands()).toEqual([{ t: "FocusWindow", instanceId: 1 }]);
    expect(drawn()).toEqual(["window-1", "window-2"]);

    answer(recordedDeltas(4));
    expect(drawn()).toEqual(["window-2", "window-1"]);
  });

  it("sends a drag as a PlaceWindow and leaves the window where the daemon has it", () => {
    const { commands, answer } = desk();
    answer(recordedDeltas(0));

    fireEvent.pointerDown(screen.getByTestId("title-window-1"), {
      button: 0,
      clientX: 0,
      clientY: 0,
      pointerId: 1,
    });
    fireEvent.pointerMove(window, { clientX: 240, clientY: 120, pointerId: 1 });
    fireEvent.pointerUp(window, { pointerId: 1 });

    // The command carries whole canvas units, which is what the daemon holds
    // — and there is no `FocusWindow` beside it, because the daemon focuses a
    // window when it opens it and this one is already focused.
    expect(commands()).toEqual([
      { t: "PlaceWindow", instanceId: 1, x: 240, y: 120, w: 640, h: 480 },
    ]);
    // And with no delta, the window is back at the daemon's rectangle.
    const window1 = screen.getByTestId("window-1");
    expect(window1.style.left).toBe("0%");
  });
});

describe("the View Selector Bar", () => {
  it("lights the view the session says is active, and asks for another", () => {
    const { commands, answer } = desk();
    expect(screen.getByTestId("view-1").dataset["active"]).toBe("yes");

    // Store the canvas as view 2 (step 6 of the script), so there are two.
    for (const step of [0, 1, 2, 3, 4, 5]) {
      answer(recordedDeltas(step));
    }
    expect(screen.getByTestId("view-2")).not.toBeNull();
    expect(screen.getByTestId("view-2").dataset["active"]).toBe("no");

    fireEvent.click(screen.getByTestId("view-2"));
    expect(commands().at(-1)).toEqual({ t: "SelectView", viewId: 2 });
    // Not lit yet: `activeViewId` is the daemon's.
    expect(screen.getByTestId("view-2").dataset["active"]).toBe("no");
  });

  it("stores the active view under the name it already has", () => {
    const { commands, answer } = desk();
    for (const step of [0, 1, 2, 3, 4, 5]) {
      answer(recordedDeltas(step));
    }
    fireEvent.click(screen.getByTestId("store-view"));
    expect(commands().at(-1)).toEqual({ t: "StoreView", viewId: 1, name: "View 1" });
  });

  it("stores a new view one past the highest, rather than over somebody's layout", () => {
    const { commands, answer } = desk();
    fireEvent.click(screen.getByTestId("new-view"));
    expect(commands().at(-1)).toEqual({ t: "StoreView", viewId: 2, name: "View 2" });

    // After view 2 exists — step 5 of the script stores it — the next one is 3.
    for (const step of [0, 1, 2, 3, 4, 5]) {
      answer(recordedDeltas(step));
    }
    fireEvent.click(screen.getByTestId("new-view"));
    expect(commands().at(-1)).toEqual({ t: "StoreView", viewId: 3, name: "View 3" });
  });
});

describe("D11, from this end", () => {
  /**
   * **The second exit criterion.** Nothing on this side does anything.
   *
   * The deltas replayed here are the ones a daemon produced for `SelectView`
   * and `OpenWindow` — and `crates/prismd/tests/surface_gate.rs` asserts that a
   * console press with a client connected produces exactly this delta, on the
   * wire, unprompted.
   */
  it("follows a view the console switched, with no local action at all", () => {
    const { answer } = desk();
    // A layout stored as view 2 while this interface was watching.
    for (const step of [0, 1, 2, 3, 4, 5]) {
      answer(recordedDeltas(step));
    }
    // Then the canvas is changed and the operator presses `Channel ▶` on the
    // desk: view 2 comes back. Step 9 of the script is that `SelectView`.
    answer(recordedDeltas(6));
    answer(recordedDeltas(7));
    expect(drawn()).toEqual(["window-1", "window-3"]);

    answer(recordedDeltas(9));
    expect(drawn()).toEqual(["window-2", "window-1"]);
    expect(screen.getByTestId("view-2").dataset["active"]).toBe("yes");
    expect(screen.getByTestId("view-1").dataset["active"]).toBe("no");
    expect(screen.getByTestId("active-view").textContent).toBe("2");
  });
});

describe("restarting the interface", () => {
  /**
   * **The third exit criterion**, at the level a unit test can make it: the
   * layout is not in this process. A second interface, which has never seen a
   * command or a delta, is handed the snapshot the daemon serves and draws the
   * same canvas.
   *
   * The version with a real browser and a real `page.reload()` is
   * `ui/e2e/session.spec.ts`; what this adds is that it holds for a client that
   * never took part in building the layout at all.
   */
  it("draws the layout it is given, having taken no part in making it", () => {
    const first = desk();
    for (const step of [0, 1, 2, 3, 4]) {
      first.answer(recordedDeltas(step));
    }
    const before = drawn();
    const geometry = screen.getByTestId("window-1").style.left;
    expect(before).toEqual(["window-2", "window-1"]);
    first.view.unmount();

    // A fresh interface, and a snapshot rather than a delta — which is what a
    // reloaded page receives.
    const session = sessionAfter(5);
    desk(snapshot({ session }));
    expect(drawn()).toEqual(before);
    expect(screen.getByTestId("window-1").style.left).toBe(geometry);
    expect(screen.getByTestId("window-1").dataset["focused"]).toBe("yes");
  });
});

/** The session document after the first `steps` steps, out of the recording. */
function sessionAfter(steps: number): JsonValue {
  const store = new DeskStore();
  store.applySnapshot(recordedSnapshot());
  for (let step = 0; step < steps; step += 1) {
    for (const delta of recordedDeltas(step)) {
      store.applyDelta(delta);
    }
  }
  const session = store.getState().documents?.session;
  if (session === undefined) {
    throw new Error("the store lost its documents");
  }
  return session;
}
