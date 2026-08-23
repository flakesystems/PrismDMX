/**
 * **The control editor, through the whole interface, against a daemon on a
 * socket** — S38.
 *
 * `settingswindow.test.tsx`'s shape and its reason: what is asserted is what an
 * operator would do — a gesture, the bytes that went out, and, the half that
 * matters, **the panel not having changed** until the answer or the delta came
 * back. A panel that moved on its own would be a client holding a truth of its
 * own, which is what **D3** forbids and what *two editors, one table* is really
 * about.
 *
 * The browser half is `ui/e2e/controls.spec.ts`, which is the one that removes
 * the doubt: a real `prismd`, a real binding table and a real console pressing a
 * real key.
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import type { Answer, BoundControl, Command, Delta, SurfaceControl } from "../bindings";
import { Connection } from "../ipc/connection";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import {
  FakeNetwork,
  ManualTimer,
  machine,
  serverMessage,
  snapshot as baseSnapshot,
} from "../testing/fake-daemon";
import { TelemetryProvider } from "../telemetry/panel";

/** A snapshot with a Settings window open and nothing else on the canvas. */
function withSettings(overrides: Partial<Snapshot> = {}): Snapshot {
  const base = baseSnapshot(overrides);
  return {
    ...base,
    session: {
      session: {
        activeViewId: 1,
        openWindows: [
          { instanceId: 1, type: "Settings", x: 0, y: 0, w: 1920, h: 1080, params: {} },
        ],
        focusedWindow: 1,
        executorPage: 0,
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

/** One row of the table, as the daemon would answer it. */
function row(
  name: string,
  control: BoundControl,
  extra: Partial<SurfaceControl> = {},
): SurfaceControl {
  return { control, name, action: null, permanent: false, reserved: false, ...extra };
}

/**
 * A small table, in `BoundControl::all`'s order.
 *
 * Small on purpose: what this file checks is the *gestures*, and the claim that
 * every control has a row is `crates/prism-surface/tests/bindings.rs`'s, where
 * the count is written out by hand from §2.1.
 */
const CONTROLS: readonly SurfaceControl[] = [
  row("Strip[*].Fader", { t: "StripFader" }, { action: { t: "ExecutorMaster", target: "Strip" } }),
  row("Strip[*].Button.Solo", { t: "StripButton", button: "Solo" }),
  row("Main.Fader", { t: "MainFader" }),
  // §4.3's permanently-ours set: the transport section and the jog wheel.
  row("Global.Jog", { t: "Jog" }, { permanent: true }),
  row(
    "Global.Play",
    { t: "Global", button: "Play" },
    { permanent: true, action: { t: "ExecutorButton", target: "Selected", button: { t: "Function", function: "On" } } },
  ),
  row(
    "Global.F1",
    { t: "Global", button: "F1" },
    { action: { t: "OpenWindow", window: "FixtureSheet" } },
  ),
  row("Global.F5", { t: "Global", button: "F5" }),
  // The one control that may never be bound.
  row("Global.SmpteBeats", { t: "Global", button: "SmpteBeats" }, { reserved: true }),
];

/** The answer a daemon holding {@link CONTROLS} would give. */
function table(overrides: Partial<Extract<Answer, { t: "SurfaceBindings" }>> = {}): Answer {
  return {
    t: "SurfaceBindings",
    controls: [...CONTROLS],
    device: "Behringer X-Touch",
    profile: null,
    revision: 3,
    learning: false,
    ...overrides,
  };
}

/** A whole interface with a daemon the test drives. */
async function desk(overrides: Partial<Snapshot> = {}) {
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
    network.last.deliver(serverMessage({ t: "Snapshot", snapshot: withSettings(overrides) }));
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

  const deliver = async (...deltas: Delta[]): Promise<void> => {
    await act(async () => {
      for (const delta of deltas) {
        network.last.deliver(serverMessage({ t: "Delta", delta }));
      }
      await Promise.resolve();
    });
  };

  // The panel is behind a tab, exactly as an operator finds it.
  fireEvent.click(screen.getByTestId("settings-tab-controls"));
  return { commands, queries, answerQuery, deliver };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the table is asked for", () => {
  it("asks when the panel opens and draws what it is told", async () => {
    const { queries, answerQuery } = await desk();
    expect(queries().filter((query) => query.t === "SurfaceBindings")).toHaveLength(1);

    // Before the answer: nothing invented.
    expect(screen.getByTestId("controls-device").textContent).toBe("No surface");

    await answerQuery("SurfaceBindings", table());
    expect(screen.getByTestId("controls-device").textContent).toBe("Behringer X-Touch");
    expect(screen.getByTestId("controls-revision").textContent).toBe("rev 3");
    // Three of the eight rows carry an action.
    expect(screen.getByTestId("controls-bound").textContent).toBe("3 bound");
    expect(screen.getByTestId("control-does-Global.F1").textContent).toBe("open FixtureSheet");
    expect(screen.getByTestId("control-does-Global.F5").textContent).toBe("—");
  });

  /**
   * **Two clients do not produce two tables**, from this side of the socket.
   *
   * The revision is what a second operator's edit moves, and asking again is
   * what this panel does about it. A panel that redrew from its own copy would
   * be the second table.
   */
  it("asks again when somebody else's edit moves the revision", async () => {
    const { queries, answerQuery, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());
    expect(queries().filter((query) => query.t === "SurfaceBindings")).toHaveLength(1);

    await deliver({ t: "SurfaceBindingsChanged", revision: 4 });
    expect(queries().filter((query) => query.t === "SurfaceBindings")).toHaveLength(2);

    await answerQuery(
      "SurfaceBindings",
      table({
        revision: 4,
        controls: CONTROLS.map((entry) =>
          entry.name === "Global.F5" ? { ...entry, action: { t: "SaveShow" } } : entry,
        ),
      }),
    );
    expect(screen.getByTestId("controls-revision").textContent).toBe("rev 4");
    expect(screen.getByTestId("control-does-Global.F5").textContent).toBe("save the show");
  });
});

describe("changing what a control does", () => {
  it("sends one command naming one control, and does not move until it is told", async () => {
    const { answerQuery, commands, deliver, queries } = await desk();
    await answerQuery("SurfaceBindings", table());

    fireEvent.click(screen.getByTestId("control-choose-Global.F5"));
    expect(screen.getByTestId("control-editor-name").textContent).toBe("Global.F5");

    fireEvent.change(screen.getByTestId("control-action"), { target: { value: "Open window" } });
    fireEvent.change(screen.getByTestId("control-detail"), { target: { value: "Patch" } });
    fireEvent.click(screen.getByTestId("control-apply"));

    // **One control, one command** — `MachineChange`'s rule, and what stops two
    // operators undoing each other.
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: { t: "OpenWindow", window: "Patch" },
      },
    });
    // And the panel has **not** moved: it holds no truth of its own.
    expect(screen.getByTestId("control-does-Global.F5").textContent).toBe("—");

    await deliver({ t: "SurfaceBindingsChanged", revision: 4 });
    await answerQuery(
      "SurfaceBindings",
      table({
        revision: 4,
        controls: CONTROLS.map((entry) =>
          entry.name === "Global.F5"
            ? { ...entry, action: { t: "OpenWindow", window: "Patch" } }
            : entry,
        ),
      }),
    );
    expect(screen.getByTestId("control-does-Global.F5").textContent).toBe("open Patch");
    expect(queries().filter((query) => query.t === "SurfaceBindings")).toHaveLength(2);
  });

  it("unbinds a control with an action of nothing", async () => {
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.click(screen.getByTestId("control-choose-Global.F1"));
    fireEvent.click(screen.getByTestId("control-clear"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F1" },
        action: null,
      },
    });
  });

  it("sends the strip key's own position when no function is named", async () => {
    // `ExecutorButtonRef::Slot` — the executor decides what its second key does,
    // and a client that resolved that would be deciding what a show's own
    // setting means (**D3**).
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.click(screen.getByTestId("control-choose-Strip[*].Button.Solo"));
    fireEvent.change(screen.getByTestId("control-action"), {
      target: { value: "Executor button" },
    });
    fireEvent.change(screen.getByTestId("control-target"), { target: { value: "Strip" } });
    fireEvent.click(screen.getByTestId("control-apply"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "StripButton", button: "Solo" },
        action: {
          t: "ExecutorButton",
          target: "Strip",
          button: { t: "Slot", index: 1 },
        },
      },
    });
  });
});

describe("the reserved control", () => {
  /**
   * It is drawn, and it cannot be chosen.
   *
   * Drawn rather than hidden, because an operator looking for SMPTE/Beats has to
   * find out **why** it is not theirs rather than conclude the list is
   * incomplete — and `reserved` is the daemon's answer rather than a rule this
   * file repeats.
   */
  it("is shown, named and not editable", async () => {
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    const button = screen.getByTestId("control-choose-Global.SmpteBeats");
    expect(button.textContent).toBe("Reserved");
    expect(button.hasAttribute("disabled")).toBe(true);
    fireEvent.click(button);
    expect(screen.queryByTestId("control-editor")).toBeNull();
    expect(commands()).toHaveLength(0);
  });
});

describe("ownership in the combined Xctl+MC mode", () => {
  /**
   * §4.3, drawn and **not computed**.
   *
   * Which controls stay PrismDMX's while the surface is also driving a sound
   * console is a property of the device profile, and this client holds none.
   */
  it("says of every control whether it is always ours or follows the switch", async () => {
    const { answerQuery } = await desk();
    await answerQuery("SurfaceBindings", table());
    expect(screen.getByTestId("control-owned-Global.Play").textContent).toBe("always ours");
    expect(screen.getByTestId("control-owned-Global.Jog").textContent).toBe("always ours");
    expect(screen.getByTestId("control-owned-Global.F1").textContent).toBe("follows the switch");
    expect(screen.getByTestId("control-owned-Strip[*].Fader").textContent).toBe(
      "follows the switch",
    );
  });
});

describe("learn", () => {
  it("arms on the daemon rather than in this panel", async () => {
    const { answerQuery, commands, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());

    fireEvent.click(screen.getByTestId("controls-learn"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: { t: "SurfaceLearn", learning: true },
    });
    // Not armed here until the daemon says so: one desk, one learn, and a panel
    // that armed itself would disagree with a second editor.
    expect(screen.queryByTestId("controls-learning")).toBeNull();

    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    expect(screen.getByTestId("controls-learning")).toBeTruthy();
    expect(screen.getByTestId("controls-learn").textContent).toBe("Press a control…");
  });

  it("stops saying it is armed when the desk names a control", async () => {
    const { answerQuery, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    expect(screen.getByTestId("controls-learning")).toBeTruthy();

    // One shot: the press names the control and disarms.
    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "Global", button: "Play" },
    });
    expect(screen.queryByTestId("controls-learning")).toBeNull();
    expect(screen.getByTestId("controls-learn").textContent).toBe("Learn");
  });

  it("follows a second client arming it, without being told twice", async () => {
    const { answerQuery, deliver, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    // Nobody pressed anything here. The delta is somebody else's gesture.
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    expect(screen.getByTestId("controls-learning")).toBeTruthy();
    expect(commands()).toHaveLength(0);
  });
});

describe("a command line holding the table", () => {
  /**
   * `--surface-profile` is the table for that run, so the editor says so and
   * offers nothing.
   *
   * A box an operator can type into that does nothing is worse than a box that
   * is not there — S37's rule, and this is the row it applies to.
   */
  it("is drawn, named and disabled rather than hidden", async () => {
    const { answerQuery, commands } = await desk({
      machine: machine({ overrides: ["SurfaceProfile"] }),
    });
    await answerQuery("SurfaceBindings", table());
    expect(screen.getByTestId("controls-held").textContent).toContain("--surface-profile");
    expect(screen.getByTestId("controls-learn").hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByTestId("controls-learn"));
    expect(commands()).toHaveLength(0);
  });
});

describe("the chooser's other answers", () => {
  it("offers an encoder bank, a view number and a window, and sends each", async () => {
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());

    fireEvent.click(screen.getByTestId("control-choose-Global.F5"));

    // An encoder bank: the five feature groups, out of the generated table.
    fireEvent.change(screen.getByTestId("control-action"), { target: { value: "Encoder bank" } });
    fireEvent.change(screen.getByTestId("control-detail"), { target: { value: "Beam" } });
    fireEvent.click(screen.getByTestId("control-apply"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: { t: "SetEncoderBank", group: "Beam" },
      },
    });

    // A view number, which is typed rather than chosen.
    fireEvent.change(screen.getByTestId("control-action"), { target: { value: "Jump to view" } });
    fireEvent.change(screen.getByTestId("control-detail"), { target: { value: "4" } });
    fireEvent.click(screen.getByTestId("control-apply"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: { t: "SelectView", view: 4 },
      },
    });

    // A named executor-button function on a panel key — §4.1's transport row,
    // which is the desk's own configuration written by a person.
    fireEvent.change(screen.getByTestId("control-action"), {
      target: { value: "Executor button" },
    });
    fireEvent.change(screen.getByTestId("control-detail"), { target: { value: "LearnSpeed" } });
    fireEvent.click(screen.getByTestId("control-apply"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: {
          t: "ExecutorButton",
          target: "Selected",
          button: { t: "Function", function: "LearnSpeed" },
        },
      },
    });
  });

  it("has no second box for a kind that needs no answer", async () => {
    const { answerQuery } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.click(screen.getByTestId("control-choose-Global.F5"));
    fireEvent.change(screen.getByTestId("control-action"), { target: { value: "Oops" } });
    expect(screen.queryByTestId("control-detail")).toBeNull();
    // And no *On* box either: an Oops acts on no executor.
    expect(screen.queryByTestId("control-target")).toBeNull();
  });

  it("closes the chooser when the same row is clicked again", async () => {
    const { answerQuery } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.click(screen.getByTestId("control-choose-Global.F5"));
    expect(screen.getByTestId("control-editor")).toBeTruthy();
    fireEvent.click(screen.getByTestId("control-choose-Global.F5"));
    expect(screen.queryByTestId("control-editor")).toBeNull();
  });
});

describe("where the table came from", () => {
  it("names the profile it was last read from, when there is one", async () => {
    const { answerQuery } = await desk();
    await answerQuery("SurfaceBindings", table({ profile: "profiles/surface/xtouch.json" }));
    // A **record of where it came from** rather than where it lives: since S38
    // the table in force is this machine's own.
    expect(screen.getByTestId("controls-source").textContent).toContain(
      "profiles/surface/xtouch.json",
    );
  });

  it("says so when it has only ever been the built-in table", async () => {
    const { answerQuery } = await desk();
    await answerQuery("SurfaceBindings", table());
    expect(screen.getByTestId("controls-source").textContent).toContain("built-in bindings");
  });
});
