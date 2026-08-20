/**
 * **The preset pools, through the whole interface, against a daemon on a socket.**
 *
 * The same shape as `sequencesheet.test.tsx`: a gesture, the bytes that went
 * out, and the pool **not having changed** until the delta came back. The
 * snapshot, the deltas and the answers are a real `prismd`'s, out of
 * `ui/tests/fixtures/show-recording.json`.
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import type { Answer, Command } from "../bindings";
import { Connection } from "../ipc/connection";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import { FakeNetwork, ManualTimer, serverMessage } from "../testing/fake-daemon";
import { answerAbout, deltasAbout, showRecording, snapshotOf } from "../testing/show-recording";
import { TelemetryProvider } from "../telemetry/panel";
import { PresetPool } from "./presetpool";

/** The recorded snapshot, with a Preset Pool window open. */
function recordedSnapshot(): Snapshot {
  return {
    ...snapshotOf(showRecording.initialSnapshot),
    session: {
      session: {
        activeViewId: 1,
        openWindows: [
          { instanceId: 1, type: "PresetPool", x: 0, y: 0, w: 1920, h: 1080, params: {} },
        ],
        focusedWindow: 1,
        executorPage: 0,
        selectedExecutor: 0,
        encoderBank: "Dimmer",
        commandLine: "",
        programmerPage: 0,
        programmerParamIndex: 0,
      },
      views: {},
    },
  };
}

/** One decoded thing the interface sent. */
type Sent =
  | { readonly t: "Command"; readonly seq: number; readonly command: Command }
  | { readonly t: "Query"; readonly seq: number; readonly query: { readonly t: string } };

/** A whole interface with a daemon the test drives. */
async function desk() {
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

  render(
    <DeskProvider store={store}>
      <TelemetryProvider channel={{ sink: new TelemetrySink(), surface: () => null }}>
        <App />
      </TelemetryProvider>
    </DeskProvider>,
  );
  await act(async () => {
    connection.start();
    network.last.open();
    network.last.deliver(serverMessage({ t: "Snapshot", snapshot: recordedSnapshot() }));
    await Promise.resolve();
  });

  const sent = (): Sent[] =>
    network.last.sent
      .map((bytes) => decode(bytes))
      .filter(
        (message): message is Sent =>
          typeof message === "object" &&
          message !== null &&
          ((message as { t?: unknown }).t === "Command" ||
            (message as { t?: unknown }).t === "Query"),
      );

  const commands = (): Command[] =>
    sent()
      .filter((message) => message.t === "Command")
      .map((message) => message.command);

  const queries = (): { seq: number; t: string }[] =>
    sent()
      .filter((message) => message.t === "Query")
      .map((message) => ({ seq: message.seq, t: message.query.t }));

  const answerQuery = async (kind: string, answer: Answer): Promise<void> => {
    const question = queries()
      .filter((query) => query.t === kind)
      .at(-1);
    if (question === undefined) {
      throw new Error(`nothing asked ${kind}`);
    }
    await act(async () => {
      network.last.deliver(serverMessage({ t: "Answer", seq: question.seq, answer }));
      await Promise.resolve();
    });
  };

  const applyStep = async (...about: string[]): Promise<void> => {
    await act(async () => {
      for (const one of about) {
        for (const delta of deltasAbout(one)) {
          network.last.deliver(serverMessage({ t: "Delta", delta }));
        }
      }
      await Promise.resolve();
    });
  };

  return { commands, queries, answerQuery, applyStep };
}

/** The recorded script up to the point where preset 1 exists. */
const A_PRESET = [
  "clear the programmer",
  "select the PARs again",
  "dial a blue",
  "and a dimmer value on a PAR's white",
  "store it, with a name and a scribble-strip colour",
] as const;

/** What is in a text box. */
function valueIn(testId: string): string {
  const field = screen.getByTestId(testId);
  if (!(field instanceof HTMLInputElement)) {
    throw new Error(`${testId} is not an input`);
  }
  return field.value;
}

/** Types into a field. */
function type(testId: string, value: string): void {
  const field = screen.getByTestId(testId);
  if (!(field instanceof HTMLInputElement)) {
    throw new Error(`${testId} is not an input`);
  }
  fireEvent.change(field, { target: { value } });
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the preset pools", () => {
  it("says a pool is empty rather than drawing nothing", async () => {
    await desk();
    expect(screen.getByTestId("preset-count").textContent).toBe("0 of 0 presets");
    expect(screen.getByTestId("pool-empty").textContent).toContain("Color pool is empty");
  });

  it("shows the presets the daemon is holding, with the scribble-strip colour", async () => {
    const { applyStep } = await desk();
    await applyStep(...A_PRESET);
    expect(screen.getByTestId("preset-count").textContent).toBe("1 of 1 presets");
    const box = screen.getByTestId("preset-1");
    expect(box.textContent).toContain("Deep blue");
    // Six values, because the preset took every colour value the programmer
    // held — and which bank an attribute is on is the profile's answer, not
    // this interface's.
    expect(box.textContent).toContain("6 values");
    expect(screen.getByTestId("preset-swatch-1").getAttribute("data-color")).toBe(
      "rgb(0 0 255)",
    );
  });

  it("shows only the pool that is chosen, and the numbering is shared", async () => {
    const { applyStep } = await desk();
    await applyStep(...A_PRESET);
    fireEvent.click(screen.getByTestId("pool-Position"));
    expect(screen.getByTestId("pool-empty")).toBeTruthy();
    // The count says both numbers, so an operator can see that the preset they
    // stored is somewhere rather than gone.
    expect(screen.getByTestId("preset-count").textContent).toBe("0 of 1 presets");
    fireEvent.click(screen.getByTestId("pool-Color"));
    expect(screen.getByTestId("preset-1")).toBeTruthy();
  });

  it("applies a preset by number, and holds nothing about the answer", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_PRESET);
    fireEvent.click(screen.getByTestId("preset-1"));
    // A number and no pool: `ApplyPreset` is unambiguous because preset numbers
    // are unique across pools.
    expect(commands()).toEqual([{ t: "ApplyPreset", presetId: 1 }]);
  });

  it("asks the daemon what a store would do, and puts the answer on the button", async () => {
    const { queries, answerQuery, applyStep } = await desk();
    await applyStep(...A_PRESET);
    expect(queries().some((query) => query.t === "StorePreview")).toBe(true);
    await answerQuery("StorePreview", answerAbout("the three blues are replaced"));
    const button = screen.getByTestId("store-preset");
    expect(button.textContent).toContain("Merge");
    expect(button.textContent).toContain("Deep blue");
    expect(button.textContent).toContain("3 kept");
  });

  it("stores into the pool that is chosen, with the number and the name in the boxes", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_PRESET);
    fireEvent.click(screen.getByTestId("pool-Beam"));
    // Preset 1 is taken, so the box offers 2 — the next number free *anywhere*.
    expect(valueIn("preset-number")).toBe("2");
    type("preset-name", "Tight");
    fireEvent.submit(screen.getByTestId("preset-store"));
    expect(commands()).toEqual([
      { t: "StorePreset", presetId: 2, pool: "Beam", name: "Tight", color: null, mode: "Merge" },
    ]);
    // **Dropped, not kept**: the boxes go back to offering what the daemon's
    // pool says next.
    expect(valueIn("preset-name")).toBe("Beam 2");
  });

  it("carries the colour that is already there through a relabel", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_PRESET);
    type("preset-number", "1");
    // The name follows the number: moving onto a preset that exists offers its
    // name rather than the last one typed.
    expect(valueIn("preset-name")).toBe("Deep blue");
    type("preset-name", "Darker blue");
    fireEvent.submit(screen.getByTestId("preset-store"));
    expect(commands()).toEqual([
      {
        t: "StorePreset",
        presetId: 1,
        pool: "Color",
        name: "Darker blue",
        // A relabel that dropped the colour would throw away something an
        // operator chose and nothing on this screen can put back.
        color: { r: 0, g: 0, b: 255 },
        // The chooser's default, carried rather than assumed by the daemon —
        // S39. A relabel is a Merge, which is the mode that cannot lose a value.
        mode: "Merge",
      },
    ]);
  });

  it("draws a box with no colour as a plain one rather than as black", () => {
    // A preset that carries no scribble-strip colour is an ordinary preset —
    // `Preset::color` is optional — and painting one black would say that
    // somebody had chosen black.
    const { unmount } = render(
      <DeskProvider store={new DeskStore()}>
        <PresetPool
          show={{
            presets: {
              "3": { pool: "Color", name: "Plain", color: null, values: [] },
            },
          }}
          programmer={null}
        />
      </DeskProvider>,
    );
    expect(screen.getByTestId("preset-swatch-3").getAttribute("data-color")).toBe("");
    expect(screen.getByTestId("preset-swatch-3").getAttribute("style")).toBeNull();
    unmount();
  });

  it("goes dead when the daemon says the store would be refused", async () => {
    const { answerQuery, applyStep } = await desk();
    await applyStep(...A_PRESET);
    await answerQuery("StorePreview", answerAbout("nothing to store"));
    const button = screen.getByTestId("store-preset");
    expect(button.hasAttribute("disabled")).toBe(true);
  });

  it("asks again when the pool or the number changes", async () => {
    const { queries, applyStep } = await desk();
    await applyStep(...A_PRESET);
    const before = queries().filter((query) => query.t === "StorePreview").length;
    fireEvent.click(screen.getByTestId("pool-Focus"));
    type("preset-number", "9");
    expect(queries().filter((query) => query.t === "StorePreview").length).toBeGreaterThan(
      before + 1,
    );
  });

  it("refuses a preset number that is not a number, rather than sending a zero", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_PRESET);
    type("preset-number", "");
    type("preset-number", "nonsense");
    // The box still offers what it offered: an empty box is *no answer yet*.
    expect(valueIn("preset-number")).toBe("2");
    fireEvent.submit(screen.getByTestId("preset-store"));
    expect(commands()).toEqual([
      { t: "StorePreset", presetId: 2, pool: "Color", name: "Color 2", color: null, mode: "Merge" },
    ]);
  });
});
