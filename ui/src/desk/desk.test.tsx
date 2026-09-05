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
import type { Command, Delta, JsonValue } from "../bindings";
import { FEATURE_GROUP_ATTRIBUTES } from "../bindings/variants";
import { Connection } from "../ipc/connection";
import { readServerMessage } from "../ipc/protocol";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import { ranLines, settleReadings } from "../testing/console";
import { FakeNetwork, ManualTimer, serverMessage } from "../testing/fake-daemon";
import { TelemetryProvider } from "../telemetry/panel";
import { ENCODERS_PER_PAGE } from "./programmer";
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

/**
 * The snapshot the recorded script starts from — a real daemon's, with the
 * windows this file's subjects live in opened on its canvas.
 *
 * **S43 moved them into windows.** The executor strip and the console's keys
 * were bands around the canvas and are `Executors` and `CommandKeys` now
 * (punch-list B15 and B12), and the readings are `Status`. Everything else here
 * is the daemon's: only *which windows are open* is invented, exactly as
 * `patch/patchwindow.test.tsx` invents its Patch window, because which windows
 * are open is S25's ground and is asserted there.
 *
 * They are placed side by side rather than stacked, because since B10 the daemon
 * refuses a placement that buries a neighbour — and a fixture the daemon could
 * not have produced is a fixture that proves nothing.
 */
function recordedSnapshot(): Snapshot {
    const message = readServerMessage(decode(payload(recording.initialSnapshot)));
    if (message.t !== "Snapshot") {
        throw new Error("the recording does not start with a snapshot");
    }
    return { ...message.snapshot, session: withWindows(message.snapshot.session) };
}

/**
 * The recorded session document, with three windows opened on its canvas.
 *
 * Narrowed rather than asserted into shape: `CLAUDE.md` forbids `any` and `as`
 * is a claim, not a check — so a recording that stopped carrying a session
 * fails here by name instead of producing a document nothing can read.
 */
function withWindows(document: JsonValue): JsonValue {
    if (document === null || typeof document !== "object" || Array.isArray(document)) {
        throw new Error("the recorded snapshot has no session document");
    }
    const session = document["session"];
    if (session === null || typeof session !== "object" || Array.isArray(session)) {
        throw new Error("the recorded session document has no session in it");
    }
    return {
        ...document,
        session: {
            ...session,
            openWindows: [
                { instanceId: 1, type: "Executors", x: 0, y: 0, w: 960, h: 540, params: {} },
                { instanceId: 2, type: "CommandKeys", x: 960, y: 0, w: 960, h: 540, params: {} },
                { instanceId: 3, type: "Status", x: 0, y: 540, w: 960, h: 540, params: {} },
            ],
            focusedWindow: 1,
        },
    };
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
    store.attach(
        (command) => connection.send(command),
        (query) => connection.ask(query),
    );

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

  /**
   * The commands a gesture produced, without the line it wrote on the way.
   *
   * The three bars send plenty that is **not** a line and never was: an
   * executor page, a fader position, an encoder step (`docs/COMMAND_LINE.md`
   * §1 lists them as the deliberate exceptions). This is those.
   */
  const acted = (): Command[] =>
    commands().filter((command) => command.t !== "CommandLineInput");

  /**
   * The **lines** a gesture ran — S49.
   *
   * The keys that *are* lines go out as lines now, and the daemon reads them.
   * Which line each key writes is what these tests were always about; what a
   * line means is `crates/prism-core/tests/console.rs`.
   */
  const ran = (): string[] => ranLines(commands());

  /** Answers every question about a line that is still outstanding. */
  const settle = () => settleReadings(network.last);

    /** The daemon answers with the deltas of one recorded step. */
    const answer = (step: number): void => {
        act(() => {
            for (const delta of recordedDeltas(step)) {
                network.last.deliver(serverMessage({ t: "Delta", delta }));
            }
        });
    };

    return { view, network, store, commands, acted, ran, answer, settle };
}

/** What the eight strips are showing, as `id:name` pairs. */
function strips(): string[] {
    return [...screen.getByTestId("executor-bar").querySelectorAll("[data-executor]")].map(
        (element) =>
            `${element.getAttribute("data-executor") ?? ""}:${element.querySelector('[data-testid^="name-"]')?.textContent ?? ""
            }`,
    );
}

beforeEach(() => {
    setLogSink(nullSink);
});

describe("the executor bar", () => {
    /**
     * **The first exit criterion.** The bar is the current page and only that.
     *
     * Eight strips whatever the show has on it, numbered `page * 8 + slot`, and
     * paging is a command out with nothing changing until the delta comes back.
     * The other half of the criterion — that the *console's* paging agrees — is
     * `ui/e2e/desk.spec.ts`, where a real console presses `Faderbank ▶`.
     */
    it("shows exactly the current page, and pages by command", async () => {
        const { ran, answer, settle } = desk();
        expect(strips()).toEqual([
            "0:Warm Wash",
            "1:",
            "2:Cold Wash",
            "3:",
            "4:",
            "5:",
            "6:",
            "7:",
        ]);
        expect(screen.getByTestId("page-number").textContent).toBe("0");

        fireEvent.click(screen.getByTestId("page-up"));
        await settle();
        expect(ran()).toEqual(["Page 1"]);
        // Nothing has moved: the page is the session's.
        expect(screen.getByTestId("page-number").textContent).toBe("0");
        expect(strips()[0]).toBe("0:Warm Wash");

        answer(0);
        expect(screen.getByTestId("page-number").textContent).toBe("1");
        expect(strips()).toEqual(["8:", "9:—", "10:", "11:", "12:", "13:", "14:", "15:"]);

        // A page with nothing at all on it is still eight strips.
        answer(1);
        expect(strips()).toEqual([
            "16:",
            "17:",
            "18:",
            "19:",
            "20:",
            "21:",
            "22:",
            "23:",
        ]);
        expect(screen.getByTestId("executor-bar").querySelectorAll('[data-assigned="yes"]').length).toBe(
            0,
        );
    });

    it("cannot page below zero", async () => {
        const { ran, settle } = desk();
        expect(screen.getByTestId("page-down").hasAttribute("disabled")).toBe(true);
        fireEvent.click(screen.getByTestId("page-down"));
        await settle();
        expect(ran()).toEqual([]);
    });

    it("pages back down once there is somewhere to go", async () => {
        const { ran, answer, settle } = desk();
        answer(0);
        expect(screen.getByTestId("page-down").hasAttribute("disabled")).toBe(false);
        fireEvent.click(screen.getByTestId("page-down"));
        await settle();
        expect(ran()).toEqual(["Page 0"]);
    });

it("asks for an executor to be selected rather than lighting it", async () => {
    const { ran, answer, settle } = desk();
    fireEvent.click(screen.getByTestId("select-2"));
    await settle();
    expect(ran()).toEqual(["Executor 2"]);
    expect(screen.getByTestId("strip-2").dataset["selected"]).toBe("no");
    // Step 3 of the script is that very command.
    for (const step of [0, 1, 2, 3]) {
        answer(step);
    }
    expect(screen.getByTestId("strip-2").dataset["selected"]).toBe("yes");
});

/** Presses a strip button and lets it go, the way a finger does. */
function pressButton(testId: string) {
    const button = screen.getByTestId(testId);
    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    fireEvent.pointerUp(button, { pointerId: 1 });
}

/**
 * **A colour is drawn where the operator can see it**, and it is the *cue
 * list's* — so a fader shows the colour of the list standing on it.
 *
 * The last two steps of the recording are the two forms of the line: one
 * naming a sequence, one naming the executor holding it. Strip 0 plays
 * sequence 1 and strip 2 plays sequence 2, on the page the recording opens on.
 */
it("draws the colour of the cue list on each fader, and nothing where there is none", () => {
    const { answer } = desk();
    // Nothing is coloured until the daemon says so — the same claim every
    // other test in this file makes about every other field.
    expect(screen.queryByTestId("color-0")).toBeNull();
    expect(screen.queryByTestId("color-2")).toBeNull();

    answer(56);
    expect(screen.getByTestId("color-0").style.background).toBe("rgb(255, 0, 0)");
    expect(screen.queryByTestId("color-2")).toBeNull();

    answer(57);
    expect(screen.getByTestId("color-2").style.background).toBe("rgb(255, 136, 0)");
    // An empty slot has no colour: there is no cue list to have one.
    expect(screen.queryByTestId("color-1")).toBeNull();
});

it("sends which button was pressed, and never what it means", () => {
    const { acted, answer } = desk();
    // Strip 0's four are Go+, Go−, Off, Empty — and Empty draws nothing.
    expect(screen.getByTestId("button-0-0").dataset["function"]).toBe("Go+");
    expect(screen.getByTestId("button-0-1").dataset["function"]).toBe("Go-");
    expect(screen.getByTestId("button-0-2").dataset["function"]).toBe("Off");
    expect(screen.queryByTestId("button-0-3")).toBeNull();

    pressButton("button-0-0");
    pressButton("button-0-2");
    // **The position, not the function.** The daemon resolves it against this
    // executor's own `buttonFunctions`; a command carrying `ExecutorGo` here
    // would be this interface deciding what a show setting means (S34,
    // `docs/MCU_MAPPING.md` §4.2.1). Both edges go out, because `Flash` is
    // momentary and this bar cannot know which key has one.
    expect(acted()).toEqual([
        { t: "ExecutorButton", executorId: 0, button: { t: "Slot", index: 0 }, pressed: true },
        { t: "ExecutorButton", executorId: 0, button: { t: "Slot", index: 0 }, pressed: false },
        { t: "ExecutorButton", executorId: 0, button: { t: "Slot", index: 2 }, pressed: true },
        { t: "ExecutorButton", executorId: 0, button: { t: "Slot", index: 2 }, pressed: false },
    ]);

    // The executor is not running until the daemon says so. Steps 0–5 include
    // the Go and the Off.
    expect(screen.getByTestId("strip-0").querySelector(".strip-running")).toBeNull();
    for (const step of [0, 1, 2, 3, 4, 5]) {
        answer(step);
    }
    expect(screen.getByTestId("strip-0").querySelector(".strip-running")).not.toBeNull();
    answer(6);
    expect(screen.getByTestId("strip-0").querySelector(".strip-running")).toBeNull();
});

/**
 * **The four buttons S26 had to draw disabled are pressable now, and they are
 * pressed exactly like the other four.**
 *
 * `On`, `Flash`, `Toggle` and `LearnSpeed` are executor *functions* — show data
 * — and until S34 the protocol had no command that pressed an executor's button
 * and let the executor decide what that meant (S22,
 * `docs/MCU_MAPPING.md` §4.2.1). Strip 2's four are exactly those, so this is
 * where the bar would have to invent a meaning if it were ever going to. It
 * sends the position and nothing else.
 */
    it("presses the four functions it once had to draw disabled", () => {
        const { acted } = desk();
        for (const [index, fn] of ["Flash", "Toggle", "On", "LearnSpeed"].entries()) {
            const button = screen.getByTestId(`button-2-${String(index)}`);
            expect(button.dataset["function"]).toBe(fn);
            expect(button.hasAttribute("disabled")).toBe(false);
            pressButton(`button-2-${String(index)}`);
        }
        // Four presses and four releases, all of them the same shape. Not one
        // `ExecutorGo` and not one `ExecutorOff`: this interface never decides
        // what a function means. Not one `SelectExecutor` either — the select
        // target is the strip's head, so pressing a button on a strip does not
        // also select it.
        expect(acted()).toEqual(
            [0, 1, 2, 3].flatMap((index) => [
                { t: "ExecutorButton", executorId: 2, button: { t: "Slot", index }, pressed: true },
                {
                    t: "ExecutorButton",
                    executorId: 2,
                    button: { t: "Slot", index },
                    pressed: false,
                },
            ]),
        );
    });

    it("releases a flash even when the pointer slides off the key", () => {
        // The half of `Flash` that puts the stored master back. A press with no
        // release is a strip left at full for the rest of the show.
        const { acted } = desk();
        const button = screen.getByTestId("button-2-0");
        fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
        fireEvent.pointerCancel(button, { pointerId: 1 });
        expect(acted()).toEqual([
            { t: "ExecutorButton", executorId: 2, button: { t: "Slot", index: 0 }, pressed: true },
            { t: "ExecutorButton", executorId: 2, button: { t: "Slot", index: 0 }, pressed: false },
        ]);
    });

    it("shows the cue the daemon says the playback is on", () => {
        // S26 and S28 both had to draw a dash here, because nothing filled
        // `Executor::currentCueIndex`. S34's readback does, and the recorded
        // script is a real daemon's answer rather than a number this interface
        // made up.
        const { answer } = desk();
        expect(screen.getByTestId("cue-0").textContent).toBe("—");
        for (const step of [0, 1, 2, 3, 4, 5]) {
            answer(step);
        }
        expect(screen.getByTestId("cue-0").textContent).toBe("Q1");
        answer(6);
        expect(screen.getByTestId("cue-0").textContent).toBe("—");
    });

    it("does not offer a fader on a slot with no executor", () => {
        const { acted } = desk();
        fireEvent.pointerDown(screen.getByTestId("fader-1"), {
            button: 0,
            clientY: 100,
            pointerId: 1,
        });
        fireEvent.pointerMove(window, { clientY: 0, pointerId: 1 });
        fireEvent.pointerUp(window, { pointerId: 1 });
        expect(acted()).toEqual([]);
    });

    /**
     * **The cadence, and the drop.** A fader pulled against a daemon that never
     * answers goes back to where the daemon has it — which is `canvas/drag.ts`'s
     * rule one layer down, and the test that would fail for an implementation
     * that held the value and "synced up" afterwards.
     */
    it("sends where the fader went and holds nothing when it gets there", () => {
        const { acted } = desk();
        const fader = screen.getByTestId("fader-0");
        // jsdom gives every element a zero-sized box, so a pixel is a level here —
        // which is what `ValueDrag` answers for an element that has not been laid
        // out, and is exactly the arithmetic asserted in `valuedrag.test.ts`.
        expect(fader.dataset["level"]).toBe("65535");
        fireEvent.pointerDown(fader, { button: 0, clientY: 100, pointerId: 1 });
        fireEvent.pointerMove(window, { clientY: 200, pointerId: 1 });
        expect(screen.getByTestId("fader-0").dataset["level"]).toBe("65435");
        fireEvent.pointerUp(window, { pointerId: 1 });

        expect(acted()).toEqual([{ t: "SetExecutorMaster", executorId: 0, level: 65435 }]);
        // And with no delta, the fader is back at the daemon's level.
        expect(screen.getByTestId("fader-0").dataset["level"]).toBe("65535");
    });

    it("ignores a right-click on a fader", () => {
        const { acted } = desk();
        fireEvent.pointerDown(screen.getByTestId("fader-0"), {
            button: 2,
            clientY: 100,
            pointerId: 1,
        });
        fireEvent.pointerMove(window, { clientY: 0, pointerId: 1 });
        expect(acted()).toEqual([]);
    });
});

/**
 * **The paging exit criteria.** `programmerPage` has been in
 * `ARCHITECTURE_SPEC.md` §4.1 since S12 and until S35 *nothing paged anything*:
 * the bar drew every parameter of a bank at once and showed the number only
 * because the console could change it.
 *
 * What is asserted here is the interface's half — four to a page, the control
 * dead when there is nowhere to go, and a command out with nothing moving until
 * the delta comes back. The other half, that the console's `Zoom ▲▼` moves the
 * same field, is `ui/e2e/desk.spec.ts` against a real `prismd`: two hands on one
 * number is a thing to observe, not to assert.
 */
describe("paging the encoder bar", () => {
    /** Puts the session where a test needs it, in the daemon's own vocabulary. */
    function session(store: DeskStore, fields: Record<string, JsonValue>) {
        act(() => {
            store.applyDelta({
                t: "SessionPatch",
                ops: Object.entries(fields).map(([field, value]) => ({
                    op: "replace" as const,
                    path: `/session/${field}`,
                    value,
                })),
            });
        });
    }

    /** The attributes the bar is drawing, in order. */
    function drawn(): string[] {
        return [...screen.getByTestId("encoders").children].map(
            (encoder) => encoder.querySelector(".encoder-name")?.textContent ?? "",
        );
    }

    it("shows a bank that fits on one page and turns the page control off", () => {
        const { store } = desk();
        // Dimmer has one parameter and Position two — both inside a page.
        expect(drawn()).toEqual(["Dimmer"]);
        expect(screen.getByTestId("programmer-page").textContent).toBe("1/1");
        expect(screen.getByTestId("encoder-page-up").hasAttribute("disabled")).toBe(true);
        expect(screen.getByTestId("encoder-page-down").hasAttribute("disabled")).toBe(true);

        session(store, { encoderBank: "Position" });
        expect(drawn()).toEqual(["Pan", "Tilt"]);
        expect(screen.getByTestId("programmer-page").textContent).toBe("1/1");
        expect(screen.getByTestId("encoder-page-down").hasAttribute("disabled")).toBe(true);
    });

    /**
     * **A bank with more than one page is asserted to exist**, rather than
     * assumed: `FEATURE_GROUP_ATTRIBUTES` is generated from
     * `FeatureGroup::attributes`, so a later session that re-split the banks
     * turns this red instead of quietly leaving the paging untested.
     *
     * **And S43 is that session.** Beam used to carry six attributes and was the
     * bank these tests paged; the seven-bank split took Gobo and Control out of
     * it, leaving three. **Colour** is the one that overflows now — five
     * attributes over a page of four — so the tests below page that instead.
     * This is exactly the failure the paragraph above was written for: the guard
     * went red, and the tests were re-aimed rather than deleted.
     *
     * The second assertion is the one that matters and it is deliberately
     * general: if a later split leaves **no** bank longer than a page, every
     * paging test below becomes a test of nothing, and this says so.
     */
    it("is asserted to have a bank that does not fit on one page", () => {
        expect(FEATURE_GROUP_ATTRIBUTES.Color.length).toBe(5);
        expect(FEATURE_GROUP_ATTRIBUTES.Color.length).toBeGreaterThan(ENCODERS_PER_PAGE);
        const longest = Object.values(FEATURE_GROUP_ATTRIBUTES).reduce(
            (most, bank) => Math.max(most, bank.length),
            0,
        );
        expect(longest, "no bank needs paging, so paging is untested").toBeGreaterThan(
            ENCODERS_PER_PAGE,
        );
    });

    it("draws four at a time and pages by command", () => {
        const { acted, store } = desk();
        session(store, { encoderBank: "Color" });
        expect(drawn()).toEqual(["Red", "Green", "Blue", "White"]);
        expect(screen.getByTestId("programmer-page").textContent).toBe("1/2");
        // Nowhere back from the first page, somewhere forward from it.
        expect(screen.getByTestId("encoder-page-up").hasAttribute("disabled")).toBe(true);
        expect(screen.getByTestId("encoder-page-down").hasAttribute("disabled")).toBe(false);

        fireEvent.click(screen.getByTestId("encoder-page-down"));
        expect(acted()).toEqual([{ t: "SetProgrammerPage", page: 1 }]);
        // D3: the page is the session's, so nothing has moved.
        expect(drawn()).toEqual(["Red", "Green", "Blue", "White"]);
        expect(screen.getByTestId("programmer-page").textContent).toBe("1/2");

        session(store, { programmerPage: 1 });
        // The rest of the bank, and a last page that is not full.
        expect(drawn()).toEqual(["Amber"]);
        expect(screen.getByTestId("programmer-page").textContent).toBe("2/2");
        expect(screen.getByTestId("encoder-page-down").hasAttribute("disabled")).toBe(true);
        expect(screen.getByTestId("encoder-page-up").hasAttribute("disabled")).toBe(false);
    });

    it("pages back down, and cannot page below the first page", () => {
        const { acted, store } = desk();
        session(store, { encoderBank: "Color", programmerPage: 1 });
        fireEvent.click(screen.getByTestId("encoder-page-up"));
        expect(acted()).toEqual([{ t: "SetProgrammerPage", page: 0 }]);

        session(store, { programmerPage: 0 });
        fireEvent.click(screen.getByTestId("encoder-page-up"));
        // Disabled, so the press sent nothing at all.
        expect(acted()).toEqual([{ t: "SetProgrammerPage", page: 0 }]);
    });

    /**
     * The upper bound is the client's (§4.4): `prism-core` deliberately does not
     * know how many parameters a bank has, so it cannot refuse a page past the
     * end. A bar that drew the empty slice would be a screen an operator could
     * not get back.
     */
    it("shows the last page rather than an empty bar when the session runs past the bank", () => {
        const { store } = desk();
        session(store, { encoderBank: "Color", programmerPage: 9 });
        expect(drawn()).toEqual(["Amber"]);
        expect(screen.getByTestId("programmer-page").textContent).toBe("2/2");
        expect(screen.getByTestId("encoder-page-down").hasAttribute("disabled")).toBe(true);

        // And a bank that shrank under a page number does the same thing.
        session(store, { encoderBank: "Dimmer" });
        expect(drawn()).toEqual(["Dimmer"]);
        expect(screen.getByTestId("programmer-page").textContent).toBe("1/1");
    });

    /**
     * The highlight and the page are two session fields, and the bar invents no
     * relationship between them: an index on another page simply is not drawn as
     * selected, because it is not drawn.
     */
    it("lights the highlighted parameter only when its page is the one shown", () => {
        const { store } = desk();
        session(store, { encoderBank: "Color", programmerParamIndex: 4 });
        expect(screen.queryByTestId("encoder-Amber")).toBeNull();

        session(store, { programmerPage: 1 });
        expect(screen.getByTestId("encoder-Amber").dataset["selected"]).toBe("yes");
        expect(screen.queryByTestId("encoder-White")).toBeNull();
    });
});

describe("the encoder bar", () => {
    it("lights the bank the session names and asks for another", () => {
        const { acted, answer } = desk();
        expect(screen.getByTestId("bank-Dimmer").dataset["active"]).toBe("yes");
        expect(screen.getByTestId("bank-Position").dataset["active"]).toBe("no");
        // The Dimmer bank has one encoder; Position has two.
        expect(screen.getByTestId("encoders").children.length).toBe(1);

        fireEvent.click(screen.getByTestId("bank-Position"));
        expect(acted()).toEqual([{ t: "SetEncoderBank", group: "Position" }]);
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
        const { acted, answer } = desk();
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
        expect(acted()).toEqual([
            { t: "SetAttribute", attribute: "Pan", value: 512, relative: true },
        ]);
        // Nothing local moved: an encoder reads the programmer, and the programmer
        // moves when `ProgrammerChanged` comes back.
        expect(screen.getByTestId("value-Pan").textContent).toBe(before);
    });

    /**
     * **The two arrows are gone — S43, and it is a recorded departure from the
     * owner's skeleton.**
     *
     * The skeleton draws a `<` `>` pair beside the encoders for stepping the
     * highlighted parameter. The band has room for four encoders and their
     * readings and not for a second stepper beside the page one, and the gesture
     * the arrows performed is one an operator can make directly: **click the
     * encoder you mean**. So the pair folded into the encoder itself, and what
     * goes on the wire is unchanged — `SelectProgrammerParam` one step at a
     * time, which is what the X-Touch's own keys send, so the two hands cannot
     * disagree about which parameter the jog wheel has.
     */
    it("steps the highlighted parameter by clicking the encoder that is wanted", () => {
        const { acted, answer } = desk();
        for (let step = 0; step <= 14; step += 1) {
            answer(step);
        }
        expect(screen.queryByTestId("param-prev")).toBeNull();
        expect(screen.queryByTestId("param-next")).toBeNull();
        expect(screen.getByTestId("encoder-Pan").dataset["selected"]).toBe("yes");

        // Pan is index 0 and Tilt is index 1, so this is one step forward — and
        // the click **also takes Tilt over** at the value it already has, which
        // is the owner's fourth point from the hand-testing round. Two commands,
        // one gesture, and neither of them decides a value here.
        fireEvent.click(screen.getByTestId("encoder-Tilt"));
        expect(acted()).toEqual([
            { t: "SelectProgrammerParam", direction: "Next" },
            { t: "SetAttribute", attribute: "Tilt", value: 0, relative: true },
        ]);
        // **And nothing has moved**: which parameter is highlighted is the
        // session's, exactly as it was when an arrow sent the same command.
        expect(screen.getByTestId("encoder-Pan").dataset["selected"]).toBe("yes");

        answer(17);
        expect(screen.getByTestId("encoder-Tilt").dataset["selected"]).toBe("yes");

        // And back, which is the other arrow's job.
        fireEvent.click(screen.getByTestId("encoder-Pan"));
        expect(acted().at(-1)).toEqual({ t: "SelectProgrammerParam", direction: "Prev" });
    });

    it("ignores a right-click on an encoder", () => {
        const { acted, answer } = desk();
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
        expect(acted()).toEqual([]);
    });

    it("composes a click on an encoder out of the steps the protocol has", () => {
        // There is no absolute form of `SelectProgrammerParam` — the console has
        // none either, `Zoom ◀▶` steps — so a click is the steps between here and
        // there. A bank has at most six parameters.
        const { acted, answer } = desk();
        for (let step = 0; step <= 8; step += 1) {
            answer(step);
        }
        fireEvent.click(screen.getByTestId("encoder-Tilt"));
        expect(acted()).toEqual([{ t: "SelectProgrammerParam", direction: "Next" }]);
        answer(17);
        fireEvent.click(screen.getByTestId("encoder-Pan"));
        expect(acted().at(-1)).toEqual({ t: "SelectProgrammerParam", direction: "Prev" });
    });

    /**
     * **The stage is derived and the key is dead at nought — S43, punch-list B2.**
     *
     * The stage used to be *counted*: every press advanced it, so it could sit at
     * *clear the selection* while the programmer had values in it again, and an
     * operator could not tell what the next press would take. It is read off the
     * programmer now (`ProgrammerState::stage`), so it cannot go stale — and
     * stage nought means *there is nothing to clear*, which is a **disabled key**
     * rather than a key that sends a command doing nothing.
     */
    it("says what the next press would clear, and does nothing when there is nothing", async () => {
        const { ran, answer, settle } = desk();
        const clear = () => screen.getByTestId("clear");
        expect(clear().dataset["stage"]).toBe("0");
        expect(clear().hasAttribute("disabled")).toBe(true);
        fireEvent.click(clear());
        await settle();
        expect(ran()).toEqual([]);

        // With values in the programmer the key is live, and it says the next
        // press takes the values.
        for (let step = 0; step <= 19; step += 1) {
            answer(step);
        }
        expect(clear().dataset["stage"]).toBe("1");
        expect(clear().hasAttribute("disabled")).toBe(false);
        fireEvent.click(clear());
        await settle();
        expect(ran()).toEqual(["Clear"]);

        // **All four stages, from the daemon's own answers** — the recorded
        // script presses Clear three times over. The values go and the selection
        // is left; the selection goes and the bank is left; the bank goes and
        // there is nothing left to take, which is where the key switches off.
        answer(20);
        expect(clear().dataset["stage"]).toBe("2");
        answer(21);
        expect(clear().dataset["stage"]).toBe("3");
        answer(22);
        expect(clear().dataset["stage"]).toBe("0");
        expect(clear().hasAttribute("disabled")).toBe(true);
    });
});

describe("the command line", () => {
    /**
     * **The reading is the daemon's since S49**, so this asserts that the
     * sentence it sends is the sentence under the box — not what the sentence
     * says, which is `crates/prism-core/tests/console.rs::a_line_reads_back_in_words`.
     */
    it("says what a line will do before it is sent", async () => {
        const { network } = desk();
        const input = screen.getByTestId("command-input");
        fireEvent.change(input, { target: { value: "1 thru 3 at 50" } });
        await settleReadings(network.last, {
            "1 thru 3 at 50": { reading: "select 1 + 2 + 3 · dimmer → 50%", commands: 2 },
        });
        expect(screen.getByTestId("command-reading").textContent).toBe(
            "select 1 + 2 + 3 · dimmer → 50%",
        );
        fireEvent.change(input, { target: { value: "1 thru" } });
        await settleReadings(network.last, {
            "1 thru": {
                kind: "Error",
                reading: "thru what? A range is two numbers, as in 1 thru 4.",
            },
        });
        expect(screen.getByTestId("command-reading").textContent).toContain("thru what");
    });

    /**
     * **One command, and the daemon does the rest** — S49.
     *
     * Before it the interface sent the commands a line meant and then an empty
     * line; a line that fell into two sent two. It sends the *line* now, and
     * what comes back is the deltas of what the daemon did — including the
     * clearing of `Session::commandLine`, which is why nothing empties it here.
     */
    it("sends the line and lets the daemon read it", async () => {
        const { commands, network } = desk();
        const input = screen.getByTestId("command-input");
        fireEvent.change(input, { target: { value: "1 + 2" } });
        fireEvent.submit(input);
        await settleReadings(network.last, { "1 + 2": { commands: 1 } });
        expect(commands()).toEqual([
            // The line is mirrored into the session as it is typed…
            { t: "CommandLineInput", text: "1 + 2", run: false, mode: null },
            // …and running it is that same line with `run` on it.
            { t: "CommandLineInput", text: "1 + 2", run: true, mode: null },
        ]);
        expect((input as HTMLInputElement).value).toBe("");
    });

    /** **The exit criterion**: a syntax error is a message and nothing is run. */
    it("refuses to send a line it could not read, without throwing", async () => {
        const { commands, network } = desk();
        const input = screen.getByTestId("command-input");
        fireEvent.change(input, { target: { value: "banana" } });
        fireEvent.submit(input);
        await settleReadings(network.last, {
            banana: { kind: "Error", reading: '"banana" is not a fixture number.' },
        });
        expect(screen.getByTestId("command-reading").textContent).toContain("not a fixture number");
        // The line still reached the session — it is what the operator typed, and
        // every client shows it — but nothing was run.
        expect(commands()).toEqual([
            { t: "CommandLineInput", text: "banana", run: false, mode: null },
        ]);
    });

    it("sends nothing at all for an empty line", async () => {
        const { commands, network } = desk();
        fireEvent.submit(screen.getByTestId("command-input"));
        await settleReadings(network.last);
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
            expect(commands()).toEqual([{ t: "CommandLineInput", text: "1", run: false, mode: null }]);
            act(() => {
                vi.advanceTimersByTime(SEND_INTERVAL_MS);
            });
            // And one more carrying where the line actually got to.
            expect(commands()).toEqual([
                { t: "CommandLineInput", text: "1", run: false, mode: null },
                { t: "CommandLineInput", text: "1 th", run: false, mode: null },
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
            expect(commands()).toEqual([{ t: "CommandLineInput", text: "12", run: false, mode: null }]);
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

    /**
     * **A key's line does not eat the line typed after it** — B40.
     *
     * Two runs of CI died here and neither said a word about it. Running a line
     * is asynchronous (S49) and the daemon *clears* `Session::commandLine` as
     * part of running one, so a key in §4.5's first shape empties the daemon's
     * field a round trip after the finger came off it. An operator who started
     * typing in that gap had that emptying adopted into their box: the line went
     * blank, the Enter after it ran nothing, and nothing on the screen said so.
     *
     * The deltas below are the daemon's own order for this gesture — the
     * keystroke this client mirrored comes back first, then the clearing the
     * key's line caused — because the mirror is sent before the run.
     */
    it("keeps a line typed while a key's line is still in flight", async () => {
        const { commands, network, store } = desk();
        /** The daemon's line moved, and nothing else. */
        const daemonLine = (text: string) => {
            act(() => {
                store.applyDelta({
                    t: "SessionPatch",
                    ops: [{ op: "replace", path: "/session/commandLine", value: text }],
                });
            });
        };

        // A key writes a whole line and runs it. Nothing has gone out yet: the
        // reading it is waiting on is a round trip.
        fireEvent.click(screen.getByTestId("page-up"));
        // And the operator does not stop typing while that is in flight.
        const input = screen.getByTestId("command-input");
        fireEvent.change(input, { target: { value: "1 thru 3 red at 100" } });
        // Both readings answered, so the key's line is sent now.
        await settleReadings(network.last);

        // The daemon echoes the keystroke this client mirrored…
        daemonLine("1 thru 3 red at 100");
        // …and then clears its field, because it has just run the key's line.
        daemonLine("");
        expect((input as HTMLInputElement).value).toBe("1 thru 3 red at 100");

        // Which is the whole point: the line is still there to be run.
        fireEvent.submit(input);
        await settleReadings(network.last);
        expect(ranLines(commands())).toEqual(["Page 1", "1 thru 3 red at 100"]);
    });

    /**
     * And the second screen this rule exists for still gets through.
     *
     * The clearing above is written down only when this client has put something
     * in the daemon's field, because an unchanged field produces no ops at all
     * (`Session::commit`) — a client waiting for an echo that was never going to
     * arrive would be deaf to the next real one, which is the defect the guard
     * is there to prevent rather than a version of it.
     */
    it("still adopts a line typed on another screen after a key has run one", async () => {
        const { network, store } = desk();
        fireEvent.click(screen.getByTestId("page-up"));
        await settleReadings(network.last);
        act(() => {
            store.applyDelta({
                t: "SessionPatch",
                ops: [{ op: "replace", path: "/session/commandLine", value: "Sequence 4" }],
            });
        });
        expect((screen.getByTestId("command-input") as HTMLInputElement).value).toBe("Sequence 4");
    });

    /**
     * **A pick decided a round trip ago does not land on the line typed since**
     * — B40, and the other half of the same CI failure.
     *
     * What a pick means is the daemon's since S49, so a click on a pool row is a
     * question and an answer with the operator's hands free in between. They
     * clicked a preset and typed `at 100` fifty milliseconds later; the pick's
     * own line went into the box on top of theirs, the Enter after it ran the
     * pick a second time, and `at 100` was never run at all. The cue went into
     * the show with colour and no intensity — which since B34 is a cue that
     * makes no light, and is what `looks.spec.ts:374` was measuring.
     *
     * Both halves are asserted, because a fix that simply dropped the late pick
     * would pass a test that only looked at the box: the click is a thing the
     * operator asked for and it still has to happen.
     */
    it("does not let a pick decided late overwrite the line typed since", async () => {
        const { commands, network } = desk();
        // A strip is a pick: with nothing typed it selects the executor, and
        // deciding that is a round trip.
        fireEvent.click(screen.getByTestId("select-3"));
        // The operator does not wait for it.
        const input = screen.getByTestId("command-input");
        fireEvent.change(input, { target: { value: "at 100" } });
        await settleReadings(network.last);

        expect((input as HTMLInputElement).value).toBe("at 100");
        fireEvent.submit(input);
        await settleReadings(network.last);
        // The click happened, and so did the line typed over it.
        expect(ranLines(commands())).toEqual(["Executor 3", "at 100"]);
    });
});
