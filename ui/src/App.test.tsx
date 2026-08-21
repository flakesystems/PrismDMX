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

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
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
    /**
     * **A client that connects to a desk mid-word shows the word** — S40.
     *
     * `Session::commandLine` is §4.1 state, which is what makes a key on one
     * screen write into the line on every other one. S26 kept the input purely
     * local and the daemon's line beside it; §4.5 made the line *the* interface,
     * so the input follows the session and the readout beside it is what a
     * delta has actually confirmed.
     */
    it("opens on the line the daemon is already holding", () => {
        const { network } = desk();
        serve(network);

        const input = screen.getByTestId("command-input");
        if (!(input instanceof HTMLInputElement)) {
            throw new Error("the command line is an input");
        }
        expect(input.value).toBe("fixture 1 at full");
        expect(screen.getByTestId("command-line").textContent).toBe("fixture 1 at full");
    });

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
        // The input opens on the daemon's line since S40, so it is emptied
        // first — with `fireEvent`, because `user.clear` is another fifteen
        // awaits and this test already carries a note about that.
        fireEvent.change(input, { target: { value: "" } });
        await user.type(input, "fixture 2 at 50");

        // D3: what was typed has been *mirrored* but not confirmed. Until a
        // delta says otherwise, the engine's command line is still what it was.
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

    /**
     * **The exit criterion, as a gesture rather than a claim.**
     *
     * `ARCHITECTURE_SPEC.md` §4.5's three shapes, each pressed and each checked
     * for what it did *and* for what it did not do:
     *
     * - an **argument keyword** puts its word in the line and sends **no**
     *   `Command` at all beyond the line itself;
     * - a **command that needs arguments** does the same and waits;
     * - a **whole command** runs at once.
     */
    it("writes into the line with a key, and only the third shape acts", () => {
        const { network } = desk();
        serve(network);
        const input = screen.getByTestId("command-input");
        if (!(input instanceof HTMLInputElement)) {
            throw new Error("the command line is an input");
        }
        fireEvent.change(input, { target: { value: "" } });

        const acted = (): string[] =>
            network.last.sent
                .map((bytes) => decode(bytes))
                .filter(
                    (message): message is { t: "Command"; command: { t: string } } =>
                        typeof message === "object" &&
                        message !== null &&
                        (message as { t?: unknown }).t === "Command",
                )
                .map((message) => message.command.t)
                .filter((t) => t !== "CommandLineInput");

        // An argument keyword: appended, and nothing acted.
        fireEvent.click(screen.getByTestId("key-fixture"));
        expect(input.value).toBe("Fixture ");
        expect(acted()).toEqual([]);

        // A command that needs arguments: written, and still nothing acted.
        fireEvent.click(screen.getByTestId("key-store"));
        expect(input.value).toBe("Store ");
        expect(acted()).toEqual([]);

        // A whole command with no argument: executed at once.
        fireEvent.click(screen.getByTestId("key-clear"));
        expect(acted()).toEqual(["ClearProgrammer"]);
        expect(input.value).toBe("");
    });
});

/**
 * **Dismissing a message, and where the two halves of that live.**
 *
 * The class goes on at once and the message stays in the document; the store
 * only drops it when the collapse has actually finished. So the assertions are
 * on both moments — a component that removed the message on click would pass a
 * test that only looked at the end, and the animation would never be seen.
 *
 * `jsdom` runs no transitions, so the end of one is dispatched here. That is the
 * honest shape: the event is the contract, and `App.css` owns the duration.
 */
describe("notices", () => {
    /** Raises one message through the same path a refusal takes. */
    function withNotice() {
        const desked = desk();
        serve(desked.network);
        act(() => {
            desked.network.last.deliver(
                serverMessage({
                    t: "Reject",
                    seq: 1,
                    reason: "CommandRefused",
                    message: "no executor 9 is loaded",
                }),
            );
        });
        return desked;
    }

    /** The end of the collapse, which is what `App.tsx` listens for. */
    function endCollapse(item: Element) {
        act(() => {
            item.dispatchEvent(
                new TransitionEvent("transitionend", {
                    bubbles: true,
                    propertyName: "grid-template-rows",
                }),
            );
        });
    }

    it("keeps the message up until the collapse has finished", () => {
        const { store } = withNotice();
        const notices = screen.getByTestId("notices");
        expect(notices.textContent).toContain("no executor 9 is loaded");

        const id = store.getState().notices[0]?.id ?? -1;
        act(() => {
            screen.getByTestId(`notice-close-${String(id)}`).click();
        });

        // Marked as leaving, and still there: this is the frame the transition
        // runs in, and a message removed here would never animate.
        const item = notices.querySelector(".notice");
        expect(item?.classList.contains("notice-dismissed")).toBe(true);
        expect(screen.queryByTestId("notices")).not.toBeNull();
        expect(store.getState().notices).toHaveLength(1);

        endCollapse(item as Element);
        expect(screen.queryByTestId("notices")).toBeNull();
        expect(store.getState().notices).toEqual([]);
    });

    it("ignores the end of a transition that is not the collapse", () => {
        const { store } = withNotice();
        const item = screen.getByTestId("notices").querySelector(".notice");
        act(() => {
            (item as Element).dispatchEvent(
                new TransitionEvent("transitionend", {
                    bubbles: true,
                    // The close button's hover, which bubbles to the same list
                    // item and must not drop a message still on the screen.
                    propertyName: "background-color",
                }),
            );
        });
        expect(store.getState().notices).toHaveLength(1);
        expect(screen.queryByTestId("notices")).not.toBeNull();
    });

    it("drops only the message that was dismissed", () => {
        const { network, store } = withNotice();
        act(() => {
            network.last.deliver(
                serverMessage({
                    t: "Reject",
                    seq: 2,
                    reason: "CommandRefused",
                    message: "no fixture 12 is patched",
                }),
            );
        });
        expect(store.getState().notices).toHaveLength(2);

        const first = store.getState().notices[0]?.id ?? -1;
        act(() => {
            screen.getByTestId(`notice-close-${String(first)}`).click();
        });
        const items = [...screen.getByTestId("notices").querySelectorAll(".notice")];
        expect(items.map((item) => item.getAttribute("data-leaving"))).toEqual(["yes", "no"]);

        endCollapse(items[0] as Element);
        expect(store.getState().notices.map((notice) => notice.message)).toEqual([
            "no fixture 12 is patched",
        ]);
        // The frame is still up, because there is still something in it.
        expect(screen.getByTestId("notices").textContent).toContain("no fixture 12 is patched");
    });
});
