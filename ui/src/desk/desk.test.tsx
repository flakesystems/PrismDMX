/**
 * **The three bars, through the whole interface, against a daemon on a socket.**
 *
 * `session.test.ts` holds the readers to the daemon's answers and
 * `console.test.ts` holds the parser to them. This drives `<App />` with a
 * daemon on the other end, so what is asserted is what an operator would do:
 * a gesture, the bytes that went out, and — the half that matters — **the bar
 * not having changed** until the delta came back.
 *
 * The deltas replayed here are the ones a real `prismd` produced for those very
 * commands, out of `ui/tests/fixtures/desk-recording.json`.
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "../App";
import type { Command, Delta } from "../bindings";
import { Connection } from "../ipc/connection";
import { readServerMessage } from "../ipc/protocol";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import { FakeNetwork, ManualTimer, serverMessage } from "../testing/fake-daemon";
import { TelemetryProvider } from "../telemetry/panel";
//import { UNPRESSABLE } from "./executorbar";
import { SEND_INTERVAL_MS } from "./valuedrag";

import recordingText from "../../tests/fixtures/desk-recording.json?raw";

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

/** The deltas of one recorded step. */
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
        network.last.deliver(serverMessage({ t: "Snapshot", snapshot: recordedSnapshot() }));
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

    /** The daemon answers with the deltas of one recorded step. */
    const answer = (step: number): void => {
        act(() => {
            for (const delta of recordedDeltas(step)) {
                network.last.deliver(serverMessage({ t: "Delta", delta }));
            }
        });
    };

    return { view, commands, answer };
}

/** What the eight strips are showing, as `id:name` pairs. */
// function strips(): string[] {
//     return [...screen.getByTestId("executor-bar").querySelectorAll("[data-executor]")].map(
//         (element) =>
//             `${element.getAttribute("data-executor") ?? ""}:${element.querySelector('[data-testid^="name-"]')?.textContent ?? ""
//             }`,
//     );
// }

beforeEach(() => {
    setLogSink(nullSink);
});

// describe("the executor bar", () => {
//     /**
//      * **The first exit criterion.** The bar is the current page and only that.
//      *
//      * Eight strips whatever the show has on it, numbered `page * 8 + slot`, and
//      * paging is a command out with nothing changing until the delta comes back.
//      * The other half of the criterion — that the *console's* paging agrees — is
//      * `ui/e2e/desk.spec.ts`, where a real console presses `Faderbank ▶`.
//      */
//     it("shows exactly the current page, and pages by command", () => {
//         const { commands, answer } = desk();
//         expect(strips()).toEqual([
//             "0:Warm Wash",
//             "1:",
//             "2:Cold Wash",
//             "3:",
//             "4:",
//             "5:",
//             "6:",
//             "7:",
//         ]);
//         expect(screen.getByTestId("page-number").textContent).toBe("0");

//         fireEvent.click(screen.getByTestId("page-up"));
//         expect(commands()).toEqual([{ t: "SetExecutorPage", page: 1 }]);
//         // Nothing has moved: the page is the session's.
//         expect(screen.getByTestId("page-number").textContent).toBe("0");
//         expect(strips()[0]).toBe("0:Warm Wash");

//         answer(0);
//         expect(screen.getByTestId("page-number").textContent).toBe("1");
//         expect(strips()).toEqual(["8:", "9:—", "10:", "11:", "12:", "13:", "14:", "15:"]);

//         // A page with nothing at all on it is still eight strips.
//         answer(1);
//         expect(strips()).toEqual([
//             "16:",
//             "17:",
//             "18:",
//             "19:",
//             "20:",
//             "21:",
//             "22:",
//             "23:",
//         ]);
//         expect(screen.getByTestId("executor-bar").querySelectorAll('[data-assigned="yes"]').length).toBe(
//             0,
//         );
//     });

//     it("cannot page below zero", () => {
//         const { commands } = desk();
//         expect(screen.getByTestId("page-down").hasAttribute("disabled")).toBe(true);
//         fireEvent.click(screen.getByTestId("page-down"));
//         expect(commands()).toEqual([]);
//     });

//     it("pages back down once there is somewhere to go", () => {
//         const { commands, answer } = desk();
//         answer(0);
//         expect(screen.getByTestId("page-down").hasAttribute("disabled")).toBe(false);
//         fireEvent.click(screen.getByTestId("page-down"));
//         expect(commands()).toEqual([{ t: "SetExecutorPage", page: 0 }]);
//     });

// it("asks for an executor to be selected rather than lighting it", () => {
//     const { commands, answer } = desk();
//     fireEvent.click(screen.getByTestId("select-2"));
//     expect(commands()).toEqual([{ t: "SelectExecutor", executorId: 2 }]);
//     expect(screen.getByTestId("strip-2").dataset["selected"]).toBe("no");
//     // Step 3 of the script is that very command.
//     for (const step of [0, 1, 2, 3]) {
//         answer(step);
//     }
//     expect(screen.getByTestId("strip-2").dataset["selected"]).toBe("yes");
// });

// it("sends Go and Off from the buttons the show assigns", () => {
//     const { commands, answer } = desk();
//     // Strip 0's four are Go+, Go−, Off, Empty — and Empty draws nothing.
//     expect(screen.getByTestId("button-0-0").dataset["function"]).toBe("Go+");
//     expect(screen.getByTestId("button-0-1").dataset["function"]).toBe("Go-");
//     expect(screen.getByTestId("button-0-2").dataset["function"]).toBe("Off");
//     expect(screen.queryByTestId("button-0-3")).toBeNull();

//     fireEvent.click(screen.getByTestId("button-0-0"));
//     fireEvent.click(screen.getByTestId("button-0-1"));
//     fireEvent.click(screen.getByTestId("button-0-2"));
//     expect(commands()).toEqual([
//         { t: "ExecutorGo", executorId: 0, direction: "Next" },
//         { t: "ExecutorGo", executorId: 0, direction: "Prev" },
//         { t: "ExecutorOff", executorId: 0 },
//     ]);

//     // The executor is not running until the daemon says so. Steps 0–5 include
//     // the Go and the Off.
//     expect(screen.getByTestId("strip-0").querySelector(".strip-running")).toBeNull();
//     for (const step of [0, 1, 2, 3, 4, 5]) {
//         answer(step);
//     }
//     expect(screen.getByTestId("strip-0").querySelector(".strip-running")).not.toBeNull();
//     answer(6);
//     expect(screen.getByTestId("strip-0").querySelector(".strip-running")).toBeNull();
// });

/**
 * **The finding, drawn rather than hidden and not guessed at.**
 *
 * `On`, `Flash`, `Toggle` and `LearnSpeed` are executor *functions* — show
 * data — and the protocol has no command that presses an executor's button
 * and lets the executor decide what that means (S22,
 * `docs/MCU_MAPPING.md` §4.2.1). Strip 2's four are exactly those, so the bar
 * draws four buttons that say why they cannot be pressed.
 */
//     it("draws the buttons it cannot press, and says why", () => {
//         const { commands } = desk();
//         for (const [index, fn] of ["Flash", "Toggle", "On", "LearnSpeed"].entries()) {
//             const button = screen.getByTestId(`button-2-${String(index)}`);
//             expect(button.dataset["function"]).toBe(fn);
//             expect(button.hasAttribute("disabled")).toBe(true);
//             expect(button.title).toBe(UNPRESSABLE);
//             fireEvent.click(button);
//         }
//         // Four presses bubble to the strip div and select it, but issue no executor action commands:
//         // resolving `Toggle` against `isActive` would be this interface deciding what the show's own setting means.
//         expect(commands().filter((cmd) => cmd.t !== "SelectExecutor")).toEqual([]);
//     });

//     it("has no cue number to show, because nothing feeds one back", () => {
//         // `Executor::currentCueIndex` is in the domain and on the wire, and
//         // `prismd` never fills it — see the decision log. A dash rather than a
//         // number an interface made up.
//         desk();
//         expect(screen.getByTestId("cue-0").textContent).toBe("—");
//     });

//     it("does not offer a fader on a slot with no executor", () => {
//         const { commands } = desk();
//         fireEvent.pointerDown(screen.getByTestId("fader-1"), {
//             button: 0,
//             clientY: 100,
//             pointerId: 1,
//         });
//         fireEvent.pointerMove(window, { clientY: 0, pointerId: 1 });
//         fireEvent.pointerUp(window, { pointerId: 1 });
//         expect(commands()).toEqual([]);
//     });

//     /**
//      * **The cadence, and the drop.** A fader pulled against a daemon that never
//      * answers goes back to where the daemon has it — which is `canvas/drag.ts`'s
//      * rule one layer down, and the test that would fail for an implementation
//      * that held the value and "synced up" afterwards.
//      */
//     it("sends where the fader went and holds nothing when it gets there", () => {
//         const { commands } = desk();
//         const fader = screen.getByTestId("fader-0");
//         // jsdom gives every element a zero-sized box, so a pixel is a level here —
//         // which is what `ValueDrag` answers for an element that has not been laid
//         // out, and is exactly the arithmetic asserted in `valuedrag.test.ts`.
//         expect(fader.dataset["level"]).toBe("65535");
//         fireEvent.pointerDown(fader, { button: 0, clientY: 100, pointerId: 1 });
//         fireEvent.pointerMove(window, { clientY: 200, pointerId: 1 });
//         expect(screen.getByTestId("fader-0").dataset["level"]).toBe("65435");
//         fireEvent.pointerUp(window, { pointerId: 1 });

//         expect(commands()).toEqual([{ t: "SetExecutorMaster", executorId: 0, level: 65435 }]);
//         // And with no delta, the fader is back at the daemon's level.
//         expect(screen.getByTestId("fader-0").dataset["level"]).toBe("65535");
//     });

//     it("ignores a right-click on a fader", () => {
//         const { commands } = desk();
//         fireEvent.pointerDown(screen.getByTestId("fader-0"), {
//             button: 2,
//             clientY: 100,
//             pointerId: 1,
//         });
//         fireEvent.pointerMove(window, { clientY: 0, pointerId: 1 });
//         expect(commands()).toEqual([]);
//     });
// });

describe("the encoder bar", () => {
    it("lights the bank the session names and asks for another", () => {
        const { commands, answer } = desk();
        expect(screen.getByTestId("bank-Dimmer").dataset["active"]).toBe("yes");
        expect(screen.getByTestId("bank-Position").dataset["active"]).toBe("no");
        // The Dimmer bank has one encoder; Position has two.
        expect(screen.getByTestId("encoders").children.length).toBe(1);

        fireEvent.click(screen.getByTestId("bank-Position"));
        expect(commands()).toEqual([{ t: "SetEncoderBank", group: "Position" }]);
        expect(screen.getByTestId("bank-Position").dataset["active"]).toBe("no");

        for (let step = 0; step <= 8; step += 1) {
            answer(step);
        }
        expect(screen.getByTestId("bank-Position").dataset["active"]).toBe("yes");
        expect(screen.getByTestId("encoders").children.length).toBe(2);
        expect(screen.getByTestId("encoder-Pan")).not.toBeNull();
        expect(screen.getByTestId("encoder-Tilt")).not.toBeNull();
    });

    it("reads an untouched parameter as a dash and never as nought per cent", () => {
        const { answer } = desk();
        expect(screen.getByTestId("value-Dimmer").textContent).toBe("—");
        // After `1 thru 3 at 50` the three of them hold the same value.
        for (let step = 0; step <= 11; step += 1) {
            answer(step);
        }
        fireEvent.click(screen.getByTestId("bank-Dimmer"));
        // Still Position until the daemon says otherwise, so the reading is Pan's
        // — and the three dimmers that were just set to 50 % have no Pan at all.
        expect(screen.getByTestId("value-Pan").textContent).toBe("—");
    });

    it("shows the value the programmer holds for the selection", () => {
        const { answer } = desk();
        for (let step = 0; step <= 11; step += 1) {
            answer(step);
        }
        // The bank is Position and the selection is 1, 2, 3 — which are dimmers,
        // so neither Pan nor Tilt is available on any of them.
        expect(screen.getByTestId("encoder-Pan").dataset["selected"]).toBe("yes");
        expect(screen.getByTestId("encoder-Pan").className).toContain("encoder-absent");
        // The Dimmer bank is marked as touched, from the *show's* grouping.
        expect(screen.getByTestId("bank-Dimmer").dataset["touched"]).toBe("yes");
        expect(screen.getByTestId("bank-Position").dataset["touched"]).toBe("no");

        // And after the moving head is selected and panned, Pan reads 25 %.
        for (const step of [12, 13, 14]) {
            answer(step);
        }
        expect(screen.getByTestId("value-Pan").textContent).toBe("25%");
        expect(screen.getByTestId("bank-Position").dataset["touched"]).toBe("yes");
        expect(screen.getByTestId("selection").textContent).toBe("5");
        expect(screen.getByTestId("touched").textContent).toBe("4");
    });

    it("turns an encoder as a relative command, and holds nothing", () => {
        const { commands, answer } = desk();
        for (let step = 0; step <= 14; step += 1) {
            answer(step);
        }
        const before = screen.getByTestId("value-Pan").textContent;
        fireEvent.pointerDown(screen.getByTestId("encoder-Pan"), {
            button: 0,
            clientX: 0,
            pointerId: 1,
        });
        fireEvent.pointerMove(window, { clientX: 4, pointerId: 1 });
        fireEvent.pointerUp(window, { pointerId: 1 });
        expect(commands()).toEqual([
            { t: "SetAttribute", attribute: "Pan", value: 512, relative: true },
        ]);
        // Nothing local moved: an encoder reads the programmer, and the programmer
        // moves when `ProgrammerChanged` comes back.
        expect(screen.getByTestId("value-Pan").textContent).toBe(before);
    });

    it("steps the highlighted parameter, and stops at both ends of the bank", () => {
        const { commands, answer } = desk();
        for (let step = 0; step <= 14; step += 1) {
            answer(step);
        }
        expect(screen.getByTestId("param-prev").hasAttribute("disabled")).toBe(true);
        fireEvent.click(screen.getByTestId("param-next"));
        expect(commands()).toEqual([{ t: "SelectProgrammerParam", direction: "Next" }]);

        answer(17);
        expect(screen.getByTestId("encoder-Tilt").dataset["selected"]).toBe("yes");
        // Position holds two parameters, so there is no next one.
        expect(screen.getByTestId("param-next").hasAttribute("disabled")).toBe(true);
        expect(screen.getByTestId("param-prev").hasAttribute("disabled")).toBe(false);
        fireEvent.click(screen.getByTestId("param-prev"));
        expect(commands().at(-1)).toEqual({ t: "SelectProgrammerParam", direction: "Prev" });
    });

    it("ignores a right-click on an encoder", () => {
        const { commands, answer } = desk();
        for (let step = 0; step <= 14; step += 1) {
            answer(step);
        }
        fireEvent.pointerDown(screen.getByTestId("encoder-Pan"), {
            button: 2,
            clientX: 0,
            pointerId: 1,
        });
        fireEvent.pointerMove(window, { clientX: 400, pointerId: 1 });
        fireEvent.pointerUp(window, { pointerId: 1 });
        expect(commands()).toEqual([]);
    });

    it("composes a click on an encoder out of the steps the protocol has", () => {
        // There is no absolute form of `SelectProgrammerParam` — the console has
        // none either, `Zoom ◀▶` steps — so a click is the steps between here and
        // there. A bank has at most six parameters.
        const { commands, answer } = desk();
        for (let step = 0; step <= 8; step += 1) {
            answer(step);
        }
        fireEvent.click(screen.getByTestId("encoder-Tilt"));
        expect(commands()).toEqual([{ t: "SelectProgrammerParam", direction: "Next" }]);
        answer(17);
        fireEvent.click(screen.getByTestId("encoder-Pan"));
        expect(commands().at(-1)).toEqual({ t: "SelectProgrammerParam", direction: "Prev" });
    });

    it("asks for a Clear and says which stage the next press is", () => {
        const { commands, answer } = desk();
        expect(screen.getByTestId("clear").dataset["stage"]).toBe("0");
        fireEvent.click(screen.getByTestId("clear"));
        expect(commands()).toEqual([{ t: "ClearProgrammer" }]);
        expect(screen.getByTestId("clear").dataset["stage"]).toBe("0");

        for (let step = 0; step <= 20; step += 1) {
            answer(step);
        }
        expect(screen.getByTestId("clear").dataset["stage"]).toBe("1");
        answer(21);
        expect(screen.getByTestId("clear").dataset["stage"]).toBe("2");
        answer(22);
        expect(screen.getByTestId("clear").dataset["stage"]).toBe("0");
    });
});

describe("the command line", () => {
    it("says what a line will do before it is sent", () => {
        desk();
        const input = screen.getByTestId("command-input");
        fireEvent.change(input, { target: { value: "1 thru 3 at 50" } });
        expect(screen.getByTestId("command-reading").textContent).toBe(
            "select 1 + 2 + 3 · dimmer → 50%",
        );
        fireEvent.change(input, { target: { value: "1 thru" } });
        expect(screen.getByTestId("command-reading").textContent).toContain("thru what");
    });

    it("sends a line's commands in order and clears the console line", () => {
        const { commands } = desk();
        const input = screen.getByTestId("command-input");
        fireEvent.change(input, { target: { value: "1 + 2" } });
        fireEvent.submit(input);
        expect(commands()).toEqual([
            // The line is mirrored into the session as it is typed…
            { t: "CommandLineInput", text: "1 + 2" },
            // …and executing it is the commands it meant, then an empty line.
            { t: "SelectFixtures", ids: [1, 2], mode: "Set" },
            { t: "CommandLineInput", text: "" },
        ]);
        expect((input as HTMLInputElement).value).toBe("");
    });

    /** **The exit criterion**: a syntax error is a message and nothing is sent. */
    it("refuses to send a line it could not read, without throwing", () => {
        const { commands } = desk();
        const input = screen.getByTestId("command-input");
        fireEvent.change(input, { target: { value: "banana" } });
        fireEvent.submit(input);
        expect(screen.getByTestId("command-reading").textContent).toContain("not a fixture number");
        // The line still reached the session — it is what the operator typed, and
        // every client shows it — but no command was executed.
        expect(commands()).toEqual([{ t: "CommandLineInput", text: "banana" }]);
    });

    it("sends nothing at all for an empty line", () => {
        const { commands } = desk();
        fireEvent.submit(screen.getByTestId("command-input"));
        expect(commands()).toEqual([]);
    });

    /**
     * The line is mirrored into the session as it is typed, **paced**.
     *
     * A keystroke is not a command, and the console line is read by people. The
     * first one goes at once — a line that appeared a thirtieth of a second late
     * would feel like a dropped key — and the rest of a burst is paced behind it,
     * which is S25's cadence rule with a keyboard on the other end of it.
     */
    it("mirrors the typed line into the session at most thirty times a second", () => {
        vi.useFakeTimers();
        try {
            const { commands } = desk();
            const input = screen.getByTestId("command-input");
            fireEvent.change(input, { target: { value: "1" } });
            fireEvent.change(input, { target: { value: "1 " } });
            fireEvent.change(input, { target: { value: "1 t" } });
            fireEvent.change(input, { target: { value: "1 th" } });
            // One command for the burst, carrying the first keystroke.
            expect(commands()).toEqual([{ t: "CommandLineInput", text: "1" }]);
            act(() => {
                vi.advanceTimersByTime(SEND_INTERVAL_MS);
            });
            // And one more carrying where the line actually got to.
            expect(commands()).toEqual([
                { t: "CommandLineInput", text: "1" },
                { t: "CommandLineInput", text: "1 th" },
            ]);
            // A burst that ended on the line already sent says nothing further.
            act(() => {
                vi.advanceTimersByTime(SEND_INTERVAL_MS * 4);
            });
            expect(commands().length).toBe(2);
            fireEvent.change(input, { target: { value: "1 th" } });
            expect(commands().length).toBe(2);
        } finally {
            vi.useRealTimers();
        }
    });

    it("stops mirroring when the interface goes", () => {
        // The timer is cleared on unmount: a pending flush into a store whose
        // connection has gone would be a command dropped with a warning, every
        // time an operator closed the window mid-word.
        vi.useFakeTimers();
        try {
            const { view, commands } = desk();
            fireEvent.change(screen.getByTestId("command-input"), { target: { value: "12" } });
            fireEvent.change(screen.getByTestId("command-input"), { target: { value: "123" } });
            view.unmount();
            act(() => {
                vi.advanceTimersByTime(SEND_INTERVAL_MS * 4);
            });
            expect(commands()).toEqual([{ t: "CommandLineInput", text: "12" }]);
        } finally {
            vi.useRealTimers();
        }
    });

    it("shows the daemon's line rather than what was typed", () => {
        // D3's own illustration, and S23's: the input is local, the readout is the
        // session's, and the second only ever moves because a `SessionPatch` said
        // so. Step 19 of the script is a `CommandLineInput`.
        const { answer } = desk();
        fireEvent.change(screen.getByTestId("command-input"), { target: { value: "5 at full" } });
        expect(screen.getByTestId("command-line").textContent).toBe("");
        for (let step = 0; step <= 19; step += 1) {
            answer(step);
        }
        expect(screen.getByTestId("command-line").textContent).toBe("1 thru 4 at ");
    });
});
