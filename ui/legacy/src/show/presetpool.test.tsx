/**
 * **The preset pools, through the whole interface, against a daemon on a socket.**
 *
 * The same shape as `sequencesheet.test.tsx`: a gesture, the bytes that went
 * out, and the pool **not having changed** until the delta came back. The
 * snapshot, the deltas and the answers are a real `prismd`'s, out of
 * `ui/tests/fixtures/show-recording.json`.
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import type { Answer, Command, JsonValue } from "../bindings";
import type { CommandLineReading } from "../desk/consoleshell";
import { Connection } from "../ipc/connection";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { Shell } from "../testing/shell";
import { DeskStore, deskEvents } from "../store/desk";
import { attachDaemon, ranLines, settleReadings, writtenLine } from "../testing/console";
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
   * The **lines** a gesture ran — S49.
   *
   * It used to be *the commands a gesture produced*, and it cannot be: a key
   * writes into `Session::commandLine` and the line is what is sent, so the
   * daemon is what turns one into commands. Which line each gesture writes is
   * what these tests were always about; what a line means is
   * `crates/prism-core/tests/console.rs`.
   */
  const acted = (): string[] => ranLines(commands());

  /** Answers every question about a line that is still outstanding. */
  const settle = (readings: Readonly<Record<string, Partial<CommandLineReading>>> = {}) =>
    settleReadings(network.last, readings);

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

  return { commands, acted, queries, answerQuery, applyStep, settle };
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

/** Two presets in the Colour pool, so the free number the menu offers is 3. */
const MENU_SHOW: JsonValue = {
  presets: {
    "1": { pool: "Color", name: "Warm", color: null, values: [] },
    "2": { pool: "Color", name: "", color: null, values: [] },
  },
};

/** The session document the window reads its line out of. */
const MENU_SESSION: JsonValue = { session: { commandLine: "" }, views: {} };

/**
 * The window on its own, with a way to read the lines it sent.
 *
 * The same shape as `grouppool.test.tsx`'s, and for the same reason: what the
 * menu does is write lines, so a test of it is a test of the traffic.
 */
function menu() {
  const store = new DeskStore();
  const sent: Command[] = [];
  attachDaemon(store, sent);
  render(
    <Shell store={store} session={MENU_SESSION}>
      <PresetPool show={MENU_SHOW} />
    </Shell>,
  );
  return {
    /** The lines that were run. */
    acted: (): string[] => ranLines(sent),
    /** The last line a *write* key left standing. */
    line: (): string => writtenLine(sent),
  };
}

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
    const { acted, applyStep, settle } = await desk();
    await applyStep(...A_PRESET);
    fireEvent.click(screen.getByTestId("preset-1"));
    await settle();
    // A number and no pool: the line is unambiguous because preset numbers are
    // unique across pools, and `Preset 1` is what applies one.
    expect(acted()).toEqual(["Preset 1"]);
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
    const { acted, applyStep, settle } = await desk();
    await applyStep(...A_PRESET);
    type("command-input", "Delete");
    fireEvent.click(screen.getByTestId("preset-1"));
    await settle({ "Delete Preset 1 ": { verb: true } });
    expect(acted().at(-1)).toBe("Delete Preset 1");
    // And it did **not** apply the preset: an operator who typed a verb was
    // asking for an argument.
    expect(acted().includes("Preset 1")).toBe(false);
  });

  /** With nothing typed, the box is the box: a preset is applied. */
  it("applies the preset when nothing is waiting for an argument", async () => {
    const { acted, applyStep, settle } = await desk();
    await applyStep(...A_PRESET);
    fireEvent.click(screen.getByTestId("preset-1"));
    await settle();
    expect(acted().at(-1)).toBe("Preset 1");
  });

  /**
   * **The smaller actions are a right-click** — S43, the owner's rebuild: *alle
   * kleineren Group bzw. Preset bezogenen Aktionen sollen über Rechtsklick
   * ausgeführt werden*. The box is only the apply key now, and every line the
   * menu writes is one an operator could have typed.
   */
  describe("the menu over a preset", () => {
    it("renames, copies and deletes with the lines every pool shares", async () => {
      const { acted } = menu();
      expect(screen.queryByTestId("preset-menu")).toBeNull();

      fireEvent.contextMenu(screen.getByTestId("preset-1"));
      expect(screen.getByTestId("preset-menu").dataset["subject"]).toBe("1");
      fireEvent.click(screen.getByTestId("preset-copy"));
      // The free number is the pool's own arithmetic and 2 is taken, so the
      // copy lands on 3 — and the line names both ends, as `Copy` requires.
      await waitFor(() => {
        expect(acted().at(-1)).toBe("Copy Preset 1 Preset 3");
      });

      fireEvent.contextMenu(screen.getByTestId("preset-1"));
      fireEvent.click(screen.getByTestId("preset-rename"));
      fireEvent.change(screen.getByTestId("preset-rename-input"), {
        target: { value: "Deep red" },
      });
      fireEvent.submit(
        screen.getByTestId("preset-rename-input").closest("form") as HTMLFormElement,
      );
      await waitFor(() => {
        expect(acted().at(-1)).toBe('Label Preset 1 "Deep red"');
      });

      fireEvent.contextMenu(screen.getByTestId("preset-2"));
      fireEvent.click(screen.getByTestId("preset-delete"));
      await waitFor(() => {
        expect(acted().at(-1)).toBe("Delete Preset 2");
      });
    });

    /**
     * **A preset can be given a colour, and that is a gap this session closed.**
     *
     * `Preset::color` has been on the wire since S11 with nothing able to set
     * it. The line writes *only* the colour: a store is what changes a preset's
     * values, and a colour that took the programmer with it would make an
     * operator choose between re-colouring a preset and keeping what is in it.
     * An empty answer takes the colour off, which is `Label`'s rule one verb
     * along.
     */
    it("sets a colour without touching the values, and clears it with nothing", async () => {
      const { acted } = menu();
      fireEvent.contextMenu(screen.getByTestId("preset-1"));
      fireEvent.click(screen.getByTestId("preset-colour"));
      fireEvent.change(screen.getByTestId("preset-colour-input"), {
        target: { value: "red" },
      });
      fireEvent.submit(
        screen.getByTestId("preset-colour-input").closest("form") as HTMLFormElement,
      );
      await waitFor(() => {
        expect(acted().at(-1)).toBe("Color Preset 1 red");
      });

      fireEvent.contextMenu(screen.getByTestId("preset-1"));
      fireEvent.click(screen.getByTestId("preset-colour"));
      fireEvent.submit(
        screen.getByTestId("preset-colour-input").closest("form") as HTMLFormElement,
      );
      // **The empty answer takes the colour off**, and the menu says it out
      // loud: `none` is the word for it (`docs/COMMAND_LINE.md` §2.4), so what
      // the line does is legible on the line rather than implied by a word that
      // is not there.
      await waitFor(() => {
        expect(acted().at(-1)).toBe("Color Preset 1 none");
      });
    });

    /**
     * A move is §4.5's **second** shape: the line is written and left standing,
     * because the destination is the argument the operator still has to type.
     * Nothing is sent.
     */
    it("writes a move line and sends nothing", async () => {
      const { acted, line } = menu();
      fireEvent.contextMenu(screen.getByTestId("preset-1"));
      fireEvent.click(screen.getByTestId("preset-move"));
      await waitFor(() => {
        expect(line()).toBe("Move Preset 1 Preset ");
      });
      expect(acted()).toEqual([]);
    });
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
