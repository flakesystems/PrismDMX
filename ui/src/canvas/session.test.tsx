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
  readonly steps: readonly { readonly what: string; readonly deltas: readonly string[] }[];
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

  /**
   * The commands a gesture produced, without the line it wrote on the way.
   *
   * A key writes into `Session::commandLine` and then runs the line (S40,
   * `ARCHITECTURE_SPEC.md` §4.5), so every gesture sends a `CommandLineInput`
   * before the command and another one clearing the line after it. What most of
   * these tests are about is *which command*, and this is that.
   */
  const acted = (): Command[] =>
    commands().filter((command) => command.t !== "CommandLineInput");

  /** The daemon answers with deltas. */
  const answer = (deltas: readonly Delta[]): void => {
    act(() => {
      for (const delta of deltas) {
        network.last.deliver(serverMessage({ t: "Delta", delta }));
      }
    });
  };

  return { network, store, view, commands, acted, answer };
}

/**
 * The index of the recorded step about `what`.
 *
 * By text rather than by number, which is S44's finding: inserting a step into
 * the script used to move every hard-coded index in two languages quietly.
 */
function stepAbout(what: string): number {
  const at = recording.steps.findIndex((step) => step.what.includes(what));
  if (at < 0) {
    throw new Error(`the recording has no step about ${what}`);
  }
  return at;
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
    const { acted, answer } = desk();
    expect(drawn()).toEqual([]);

    // The picker is the whole vocabulary, from the generated table.
    fireEvent.change(screen.getByTestId("open-window"), { target: { value: "DmxSheet" } });

    // **The first exit criterion.** A command went out.
    expect(acted()).toEqual([{ t: "OpenWindow", window: "DmxSheet" }]);
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
    const { acted, answer } = desk();
    answer(recordedDeltas(0));
    answer(recordedDeltas(1));

    fireEvent.click(screen.getByTestId("close-window-2"));
    expect(acted()).toEqual([{ t: "CloseWindow", instanceId: 2 }]);
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
    const { acted, answer } = desk();
    answer(recordedDeltas(0));
    answer(recordedDeltas(1));
    // Window 2 is the focused one, so pressing window 1 is a `FocusWindow` —
    // and the stacking order does not change until the delta says so.
    fireEvent.pointerDown(screen.getByTestId("window-1"), { button: 0, pointerId: 1 });
    expect(acted()).toEqual([{ t: "FocusWindow", instanceId: 1 }]);
    expect(drawn()).toEqual(["window-1", "window-2"]);

    answer(recordedDeltas(4));
    expect(drawn()).toEqual(["window-2", "window-1"]);
  });

  it("sends a drag as a PlaceWindow and leaves the window where the daemon has it", () => {
    const { acted, answer } = desk();
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
    expect(acted()).toEqual([
      { t: "PlaceWindow", instanceId: 1, x: 240, y: 120, w: 640, h: 480 },
    ]);
    // And with no delta, the window is back at the daemon's rectangle.
    const window1 = screen.getByTestId("window-1");
    expect(window1.style.left).toBe("0%");
  });
});

describe("the View Selector Bar", () => {
  it("lights the view the session says is active, and asks for another", () => {
    const { acted, answer } = desk();
    expect(screen.getByTestId("view-1").dataset["active"]).toBe("yes");

    // Store the canvas as view 2 (step 6 of the script), so there are two.
    for (const step of [0, 1, 2, 3, 4, 5]) {
      answer(recordedDeltas(step));
    }
    expect(screen.getByTestId("view-2")).not.toBeNull();
    expect(screen.getByTestId("view-2").dataset["active"]).toBe("no");

    fireEvent.click(screen.getByTestId("view-2"));
    expect(acted().at(-1)).toEqual({ t: "SelectView", viewId: 2 });
    // Not lit yet: `activeViewId` is the daemon's.
    expect(screen.getByTestId("view-2").dataset["active"]).toBe("no");
  });

  it("stores the active view under the name it already has", () => {
    const { acted, answer } = desk();
    for (const step of [0, 1, 2, 3, 4, 5]) {
      answer(recordedDeltas(step));
    }
    fireEvent.click(screen.getByTestId("store-view"));
    expect(acted().at(-1)).toEqual({ t: "StoreView", viewId: 1, name: "View 1" });
  });

  it("stores a new view one past the highest, rather than over somebody's layout", () => {
    const { acted, answer } = desk();
    fireEvent.click(screen.getByTestId("new-view"));
    expect(acted().at(-1)).toEqual({ t: "StoreView", viewId: 2, name: "View 2" });

    // After view 2 exists — step 5 of the script stores it — the next one is 3.
    for (const step of [0, 1, 2, 3, 4, 5]) {
      answer(recordedDeltas(step));
    }
    fireEvent.click(screen.getByTestId("new-view"));
    expect(acted().at(-1)).toEqual({ t: "StoreView", viewId: 3, name: "View 3" });
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

/**
 * **S35's view management, held to the daemon.**
 *
 * Every expectation about what a `Label`, a `Delete` or a `Move` over a view
 * *does* comes out of `tests/fixtures/session-recording.json`, which
 * `crates/prismd/tests/ui_session.rs` writes by driving a real `prismd` and
 * which four non-ignored Rust tests read on every commit. Nothing here is a
 * second opinion in TypeScript about the daemon's behaviour: what is asserted on
 * this side is the *gesture* — that a menu item sends a command, that nothing
 * moves before the delta, and that the bar redraws whatever the delta says.
 *
 * Steps are found by what they are **for** rather than by number (S44's
 * finding), so inserting one into the script does not silently move these.
 */
describe("managing a view", () => {
  /** Replays the script up to and including the step about `what`. */
  function upTo(answer: (deltas: Delta[]) => void, what: string) {
    const last = recording.steps.findIndex((step) => step.what.includes(what));
    if (last < 0) {
      throw new Error(`the recording has no step about ${what}`);
    }
    for (let step = 0; step <= last; step += 1) {
      answer(recordedDeltas(step));
    }
  }

  /** Opens the menu over one view button. */
  function menuOver(viewId: number) {
    fireEvent.contextMenu(screen.getByTestId(`view-${String(viewId)}`));
    return screen.getByTestId("view-menu");
  }

  /** The view bar as it is drawn, left to right. */
  function bar(): string[] {
    return [...screen.getByTestId("viewbar").querySelectorAll("[data-testid^='view-']")]
      .filter((element) => /^view-\d+$/.test(element.getAttribute("data-testid") ?? ""))
      .map(
        (element) =>
          `${element.querySelector(".view-number")?.textContent ?? ""}:${
            element.querySelector(".view-name")?.textContent ?? ""
          }`,
      );
  }

  it("opens over the view that was right-clicked, and closes on Escape", () => {
    const { answer } = desk();
    upTo(answer, "store the canvas as view 5");
    expect(screen.queryByTestId("view-menu")).toBeNull();

    expect(menuOver(5).dataset["view"]).toBe("5");
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByTestId("view-menu")).toBeNull();

    // And a click anywhere else, which is the other way out of a menu.
    menuOver(5);
    fireEvent.pointerDown(document.body);
    expect(screen.queryByTestId("view-menu")).toBeNull();
  });

  it("renames by command, holding no name of its own", () => {
    const { acted, answer } = desk();
    upTo(answer, "store the canvas as view 5");
    menuOver(5);
    fireEvent.click(screen.getByTestId("view-rename"));

    const input = screen.getByTestId("view-rename-input");
    expect((input as HTMLInputElement).value).toBe("Busking");
    fireEvent.change(input, { target: { value: "Front of house" } });
    fireEvent.click(screen.getByTestId("view-rename-apply"));

    // **`Label` over an `ObjectRef::View`** since S40: renaming a view is the
    // same act as renaming a cue, a group or a preset, so it is the same word.
    expect(acted().at(-1)).toEqual({
      t: "Label",
      target: { t: "View", viewId: 5 },
      name: "Front of house",
    });
    // D3: the button still reads the daemon's name, and the typed one is gone.
    expect(bar()).toContain("5:Busking");
    expect(screen.queryByTestId("view-menu")).toBeNull();

    // The daemon's answer is what changes it.
    answer(recordedDeltas(stepAbout("rename view 5")));
    expect(bar()).toContain("5:Front of house");
  });

  it("sends nothing for a rename that is only whitespace", () => {
    const { acted, answer } = desk();
    upTo(answer, "store the canvas as view 5");
    const before = acted().length;
    menuOver(5);
    fireEvent.click(screen.getByTestId("view-rename"));
    fireEvent.change(screen.getByTestId("view-rename-input"), { target: { value: "   " } });
    fireEvent.click(screen.getByTestId("view-rename-apply"));
    expect(acted().length).toBe(before);
    expect(screen.queryByTestId("view-menu")).toBeNull();
  });

  /**
   * **The ordering criterion, from this end.** The bar draws what the daemon
   * says, in the order the daemon says it — and after a move that order is
   * different because the *numbers* moved. There is no order held here to be
   * wrong about.
   */
  it("moves by command, and draws the order the daemon answers with", () => {
    const { acted, answer } = desk();
    upTo(answer, "rename view 5");
    expect(bar()).toEqual(["1:View 1", "2:Programming", "5:Front of house"]);

    menuOver(5);
    fireEvent.click(screen.getByTestId("view-move-prev"));
    // **The bar knows its neighbour's number and writes the line** (S40).
    // `MoveView` was relative until then; one absolute form covers both, and
    // turning *left* into *view 2* is the screen's job rather than the
    // protocol's — `ARCHITECTURE_SPEC.md` §4.5.
    expect(acted().at(-1)).toEqual({
      t: "Move",
      from: { t: "View", viewId: 5 },
      to: { t: "View", viewId: 2 },
      mode: "Merge",
    });
    // Nothing has moved: the order is the daemon's.
    expect(bar()).toEqual(["1:View 1", "2:Programming", "5:Front of house"]);

    answer(recordedDeltas(stepAbout("move view 5 onto view 2")));
    // The names travelled and the numbers stayed: that is the decision.
    expect(bar()).toEqual(["1:View 1", "2:Front of house", "5:Programming"]);
  });

  it("offers no move at the end of the bar it is already at", () => {
    const { answer } = desk();
    upTo(answer, "rename view 5");
    menuOver(1);
    expect(screen.getByTestId("view-move-prev").hasAttribute("disabled")).toBe(true);
    expect(screen.getByTestId("view-move-next").hasAttribute("disabled")).toBe(false);

    fireEvent.keyDown(window, { key: "Escape" });
    menuOver(5);
    expect(screen.getByTestId("view-move-prev").hasAttribute("disabled")).toBe(false);
    expect(screen.getByTestId("view-move-next").hasAttribute("disabled")).toBe(true);
  });

  it("will not offer to delete the only view there is", () => {
    const { answer } = desk();
    // Before anything is stored, view 1 is the whole library.
    expect(bar()).toEqual(["1:View 1"]);
    menuOver(1);
    expect(screen.getByTestId("view-delete").hasAttribute("disabled")).toBe(true);

    fireEvent.keyDown(window, { key: "Escape" });
    upTo(answer, "store the canvas as view 2");
    menuOver(1);
    expect(screen.getByTestId("view-delete").hasAttribute("disabled")).toBe(false);
  });

  /**
   * **The exit criterion about the canvas.** Deleting the active view leaves it
   * in a state the *daemon* defines: the interface sends `DeleteView` and draws
   * whatever comes back, and there is no code here that picks a successor.
   */
  it("deletes by command and lets the daemon say what the canvas becomes", () => {
    const { acted, answer } = desk();
    upTo(answer, "select view 2 — which is now the layout");
    expect(screen.getByTestId("view-2").dataset["active"]).toBe("yes");
    const before = drawn();

    menuOver(2);
    fireEvent.click(screen.getByTestId("view-delete"));
    expect(acted().at(-1)).toEqual({ t: "Delete", target: { t: "View", viewId: 2 } });
    // Still there, still active, canvas untouched: nothing is applied here.
    expect(screen.getByTestId("view-2").dataset["active"]).toBe("yes");
    expect(drawn()).toEqual(before);

    answer(recordedDeltas(stepAbout("delete the active view")));
    expect(screen.queryByTestId("view-2")).toBeNull();
    // Whatever the daemon chose is lit, and it is a view that exists.
    const lit = [...screen.getByTestId("viewbar").querySelectorAll('[data-active="yes"]')];
    expect(lit.length).toBe(1);
    // The menu went with the view it was over.
    expect(screen.queryByTestId("view-menu")).toBeNull();
  });

  it("moves the other way by the same command", () => {
    const { acted, answer } = desk();
    upTo(answer, "rename view 5");
    menuOver(2);
    fireEvent.click(screen.getByTestId("view-move-next"));
    expect(acted().at(-1)).toEqual({
      t: "Move",
      from: { t: "View", viewId: 2 },
      to: { t: "View", viewId: 5 },
      mode: "Merge",
    });
    expect(bar()).toEqual(["1:View 1", "2:Programming", "5:Front of house"]);
  });

  /**
   * A view can go while its menu is open — deleted on another screen, or by the
   * console. Every item would then name a number that is not there, and the
   * daemon would refuse all six. The menu goes with the view instead.
   */
  it("closes when the view it is over stops existing", () => {
    const { answer } = desk();
    upTo(answer, "select view 2 — which is now the layout");
    menuOver(2);
    expect(screen.getByTestId("view-menu")).not.toBeNull();

    // The delta arrives without this interface having asked for anything.
    answer(recordedDeltas(stepAbout("delete the active view")));
    expect(screen.queryByTestId("view-2")).toBeNull();
    expect(screen.queryByTestId("view-menu")).toBeNull();
  });

  it("stores over a view and stores a new one, both by command", () => {
    const { acted, answer } = desk();
    upTo(answer, "store the canvas as view 5");

    menuOver(2);
    fireEvent.click(screen.getByTestId("view-overwrite"));
    expect(acted().at(-1)).toEqual({ t: "StoreView", viewId: 2, name: "Programming" });

    menuOver(2);
    fireEvent.click(screen.getByTestId("view-store-new"));
    // One past the highest, which is 5 — not 3, and not over anybody's layout.
    expect(acted().at(-1)).toEqual({ t: "StoreView", viewId: 6, name: "View 6" });
  });
});
