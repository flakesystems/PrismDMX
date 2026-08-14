/**
 * **The second exit criterion of S23**, at the level an operator would see it:
 * the daemon restarts, the interface says *disconnected*, reconnects, takes a
 * fresh snapshot, and **no value from before is still on the screen**.
 *
 * The store's half of that claim is in `src/store/desk.test.ts`. This is the
 * half that matters to the person in the room: a value can only be stale if
 * something renders it, so the assertion here is about the document — the old
 * text is *not in the page*, by search rather than by inspection.
 *
 * Everything runs against the fake socket and the manual clock, so "the daemon
 * restarts" is three lines and the five-second backoff costs nothing. The same
 * scenario against a real `prismd` is `e2e/reconnect.spec.ts`.
 */

import { render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "./App";
import { statusText } from "./status";
import { Connection } from "./ipc/connection";
import { nullSink, setLogSink } from "./log/logger";
import { DeskProvider } from "./store/context";
import { DeskStore, deskEvents } from "./store/desk";
import {
  FakeNetwork,
  ManualTimer,
  serverMessage,
  snapshot as aSnapshot,
} from "./testing/fake-daemon";

beforeEach(() => {
  setLogSink(nullSink);
});

/** A rendered interface with a daemon the test drives. */
function desk() {
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

  render(
    <DeskProvider store={store}>
      <App />
    </DeskProvider>,
  );
  act(() => {
    connection.start();
  });
  return { network, clock, store, connection };
}

/** The daemon answers the handshake with `snapshot`. */
function serve(network: FakeNetwork, snapshot = aSnapshot()) {
  act(() => {
    network.last.open();
    network.last.deliver(serverMessage({ t: "Snapshot", snapshot }));
  });
}

describe("the interface", () => {
  it("says it is connecting before anything has answered", () => {
    desk();
    expect(screen.getByTestId("connection-status").textContent).toBe(
      "Connecting to the engine…",
    );
    expect(screen.queryByTestId("command-line")).toBeNull();
  });

  it("reads the three documents out of the snapshot", () => {
    const { network } = desk();
    serve(network);

    expect(screen.getByTestId("connection-status").textContent).toBe("Connected");
    expect(screen.getByTestId("executor-page").textContent).toBe("3");
    expect(screen.getByTestId("active-view").textContent).toBe("1");
    expect(screen.getByTestId("fixtures").textContent).toBe("1");
    expect(screen.getByTestId("executors").textContent).toBe("1");
    expect(screen.getByTestId("command-line").textContent).toBe("fixture 1 at full");
    expect(screen.getByTestId("tick-hz").textContent).toBe("44.0 Hz");
    expect(screen.getByTestId("output-1").textContent).toBe("Mock: Ok");
    expect(screen.getByTestId("dirty-flag").textContent).toBe("saved");
  });

  it("follows a delta", () => {
    const { network } = desk();
    serve(network);
    act(() => {
      network.last.deliver(
        serverMessage({
          t: "Delta",
          delta: {
            t: "SessionPatch",
            ops: [{ op: "replace", path: "/session/executorPage", value: 6 }],
          },
        }),
      );
      network.last.deliver(
        serverMessage({ t: "Delta", delta: { t: "DirtyFlag", unsavedChanges: true } }),
      );
    });
    expect(screen.getByTestId("executor-page").textContent).toBe("6");
    expect(screen.getByTestId("dirty-flag").textContent).toBe("unsaved changes");
  });

  it("shows a reading that is not in the document as a dash, not as zero", () => {
    const { network } = desk();
    serve(network, aSnapshot({ session: { session: {}, views: {} } }));
    expect(screen.getByTestId("executor-page").textContent).toBe("—");
    expect(screen.getByTestId("open-windows").textContent).toBe("—");
  });

  /**
   * **The exit criterion.** Nothing survives the daemon.
   */
  it("shows no value from the old daemon after it restarts", () => {
    const { network, clock } = desk();
    serve(network);
    expect(document.body.textContent).toContain("fixture 1 at full");

    // The daemon stops.
    act(() => {
      network.last.drop("the daemon stopped");
    });

    expect(screen.getByTestId("connection-status").textContent).toBe(
      "Disconnected — retrying in 0.1 s",
    );
    expect(screen.getByTestId("no-daemon")).not.toBeNull();
    // Not greyed out, not held: gone from the document altogether.
    expect(screen.queryByTestId("command-line")).toBeNull();
    expect(screen.queryByTestId("executor-page")).toBeNull();
    expect(document.body.textContent).not.toContain("fixture 1 at full");
    expect(document.body.textContent).not.toContain("44.0 Hz");

    // It comes back — with a different session, which is what a restarted
    // daemon has: the command line was never saved into the show.
    act(() => {
      clock.fire();
    });
    serve(
      network,
      aSnapshot({
        session: {
          session: {
            activeViewId: 1,
            executorPage: 0,
            encoderBank: "Dimmer",
            commandLine: "",
            openWindows: [],
          },
          views: {},
        },
      }),
    );

    expect(screen.getByTestId("connection-status").textContent).toBe("Connected");
    expect(screen.getByTestId("command-line").textContent).toBe("");
    expect(screen.getByTestId("executor-page").textContent).toBe("0");
    // The old value is not merely overwritten in one place — it is nowhere.
    expect(document.body.textContent).not.toContain("fixture 1 at full");
  });

  it("says which failure it is, and does not call a version mismatch a lost connection", () => {
    const { network } = desk();
    act(() => {
      network.last.open();
      network.last.deliver(
        serverMessage({
          t: "Reject",
          seq: null,
          reason: "ProtocolVersion",
          message: "the daemon speaks 2 and the client speaks 1",
        }),
      );
    });
    const status = screen.getByTestId("connection-status").textContent ?? "";
    expect(status).toContain("different versions");
    expect(status).not.toContain("Disconnected");
    expect(screen.getByTestId("no-daemon").textContent).toContain("Update the interface");
  });

  it("shows what the daemon refused", () => {
    const { network } = desk();
    serve(network);
    act(() => {
      network.last.deliver(
        serverMessage({
          t: "Reject",
          seq: 0,
          reason: "CommandRefused",
          message: "no executor 9 is loaded",
        }),
      );
    });
    expect(screen.getByTestId("notices").textContent).toContain("no executor 9 is loaded");
    // A refused command leaves the connection alone (§5).
    expect(screen.getByTestId("connection-status").textContent).toBe("Connected");
  });

  it("names every connection state", () => {
    expect(statusText({ kind: "connecting", attempt: 0 })).toContain("Connecting");
    expect(statusText({ kind: "connected" })).toBe("Connected");
    expect(
      statusText({ kind: "disconnected", reason: "gone", attempt: 3, retryInMs: 5000 }),
    ).toBe("Disconnected — retrying in 5 s");
    expect(
      statusText({ kind: "incompatible", ourVersion: 1, message: "x", retryInMs: 5000 }),
    ).toContain("different versions");
  });
});

describe("the command line", () => {
  it("sends what was typed and shows only what the daemon answered", async () => {
    const { network } = desk();
    serve(network);

    const input = screen.getByTestId("command-input");
    if (!(input instanceof HTMLInputElement)) {
      throw new Error("the command line is an input");
    }

    const { default: userEvent } = await import("@testing-library/user-event");
    // `delay: null` types without waiting between keys. With the default the
    // fifteen characters here are fifteen awaits, which is nothing on its own
    // and enough to time out when twenty-seven instrumented files are running
    // at once — a test that fails only under `--coverage` is a flake, and S18
    // established that those get fixed rather than retried.
    const user = userEvent.setup({ delay: null });
    await user.type(input, "fixture 2 at 50");

    // D3: what was typed is *local input*. Until the daemon says otherwise, the
    // engine's command line is still what it was.
    expect(input.value).toBe("fixture 2 at 50");
    expect(screen.getByTestId("command-line").textContent).toBe("fixture 1 at full");

    await user.keyboard("{Enter}");
    // The command went out, and *still* nothing was assumed about the result.
    const sent = network.last.sent.at(-1);
    expect(sent).toBeDefined();
    expect(screen.getByTestId("command-line").textContent).toBe("fixture 1 at full");

    act(() => {
      network.last.deliver(
        serverMessage({
          t: "Delta",
          delta: {
            t: "SessionPatch",
            ops: [{ op: "replace", path: "/session/commandLine", value: "fixture 2 at 50" }],
          },
        }),
      );
      network.last.deliver(serverMessage({ t: "Ack", seq: 0 }));
    });
    expect(screen.getByTestId("command-line").textContent).toBe("fixture 2 at 50");
  });
});
