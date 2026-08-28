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
import { Shell } from "../testing/shell";
import { DeskStore, deskEvents } from "../store/desk";
import { FakeNetwork, ManualTimer, serverMessage } from "../testing/fake-daemon";
import { deltasAbout, showRecording, snapshotOf } from "../testing/show-recording";
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

  return { commands, acted, queries, answerQuery, applyStep };
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
    const { acted, applyStep } = await desk();
    await applyStep(...A_PRESET);
    fireEvent.click(screen.getByTestId("preset-1"));
    // A number and no pool: `ApplyPreset` is unambiguous because preset numbers
    // are unique across pools.
    expect(acted()).toEqual([{ t: "ApplyPreset", presetId: 1 }]);
  });

  /**
   * **The store bar is gone, and this is what took its place** — S43, the
   * owner's second rebuild: *die Store Sektionen sollen entfernt werden, das
   * soll nur über die Command Line gemacht werden.*
   *
   * Five tests went with it, and their claims are worth saying once so that
   * nobody looks for them: the bar offered the lowest free number anywhere, it
   * followed a typed number to that preset's own name, it wrote the **pool**
   * into the line so a tab could not be silently disagreed with, it put the
   * daemon's own store preview on the button, and it refused a number that was
   * not one. All five described a panel that built one line out of three
   * fields.
   *
   * Two of those are not lost, only moved. The preview is the console's
   * **prompt** — the same `Query::StorePreview`, asked by the same line — and
   * the pool is `Session::encoderBank` unless the line names one, which is
   * `Command::StorePreset`'s own rule and what a typed line has always done.
   */
  it("has no store bar at all", async () => {
    await desk();
    for (const gone of ["preset-store", "preset-number", "preset-name", "store-preset"]) {
      expect(screen.queryByTestId(gone), gone).toBeNull();
    }
    // And the empty pool says what to type rather than pointing at a bar that
    // has gone.
    expect(screen.getByTestId("pool-empty").textContent).toContain("Store Preset 1");
  });

  /**
   * **A pool is an argument keyboard** — `consoleshell.ts::pickOnto`, and the
   * gesture the owner described. `Delete Preset 1` is whole, so it goes at once
   * rather than waiting for an Enter the operator already committed to.
   */
  it("finishes a waiting line and sends it at once", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_PRESET);
    type("command-input", "Delete");
    fireEvent.click(screen.getByTestId("preset-1"));
    expect(acted().at(-1)).toEqual({ t: "Delete", target: { t: "Preset", presetId: 1 } });
    // And it did **not** apply the preset: an operator who typed a verb was
    // asking for an argument.
    expect(acted().some((command) => command.t === "ApplyPreset")).toBe(false);
  });

  /** With nothing typed, the box is the box: a preset is applied. */
  it("applies the preset when nothing is waiting for an argument", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_PRESET);
    fireEvent.click(screen.getByTestId("preset-1"));
    expect(acted().at(-1)).toEqual({ t: "ApplyPreset", presetId: 1 });
  });

  it("draws a box with no colour as a plain one rather than as black", () => {
    // A preset that carries no scribble-strip colour is an ordinary preset —
    // `Preset::color` is optional — and painting one black would say that
    // somebody had chosen black.
    const { unmount } = render(
      <Shell store={new DeskStore()}>
        <PresetPool
          show={{
            presets: {
              "3": { pool: "Color", name: "Plain", color: null, values: [] },
            },
          }}
        />
      </Shell>,
    );
    expect(screen.getByTestId("preset-swatch-3").getAttribute("data-color")).toBe("");
    expect(screen.getByTestId("preset-swatch-3").getAttribute("style")).toBeNull();
    unmount();
  });

});
