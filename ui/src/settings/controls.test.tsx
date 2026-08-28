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
 *
 * # **S43 turned the panel round, and every test here with it** — punch-list B3
 *
 * The rows were controls and are actions. Every claim below was written against
 * the control-first table and is re-aimed rather than deleted: the gestures are
 * different, what they must *produce* is not — one `MachineChange` naming one
 * control, a panel that does not move until it is told, a reserved control that
 * cannot be bound, and one learn on one desk.
 *
 * The two that could not survive the turn say so where they stand: there is no
 * *chooser* to open and close any more, because the chooser is the list.
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
    deviceKey: "behringer-x-touch",
    profileVersion: 1,
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
    // Three of the eight controls carry an action.
    expect(screen.getByTestId("controls-bound").textContent).toBe("3 bound");
    // **And the reading runs the other way now** (B3): the row is the action and
    // the keys are what is written in it. Nothing opens the window chooser, so
    // that row is empty.
    expect(screen.getByTestId("action-keys-Choose a window").textContent).toBe("—");
    // **`Global.F1` opens the fixture sheet, and *Open window* is a custom
    // kind** since the rebuild — fourteen windows is fourteen bindings, not one
    // row — so the key is its own row in the custom section rather than a chip
    // under an action. See `actions.ts::CUSTOM_KINDS`.
    expect(screen.getByTestId("custom-Global.F1").textContent).toContain("FixtureSheet");
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
    expect(screen.getByTestId("action-keys-Save show").textContent).toContain("Global.F5");
  });

  /**
   * **Several keys on one action, which is the shape the turn made possible.**
   *
   * Under the control-first table this fact was spread over as many rows as
   * there were keys and could only be seen by reading all of them. It is one row
   * now, and an operator who has put Go on three keys sees three keys.
   */
  it("lists every key that reaches one action, not just the first", async () => {
    const { answerQuery } = await desk();
    await answerQuery(
      "SurfaceBindings",
      table({
        controls: CONTROLS.map((entry) =>
          entry.name === "Global.F5" || entry.name === "Main.Fader"
            ? { ...entry, action: { t: "SaveShow" } }
            : entry,
        ),
      }),
    );
    const keys = screen.getByTestId("action-keys-Save show").textContent ?? "";
    expect(keys).toContain("Global.F5");
    expect(keys).toContain("Main.Fader");
  });
});

describe("changing what a control does", () => {
  it("learns a key onto a row, sends one command, and does not move until it is told", async () => {
    const { answerQuery, commands, deliver, queries } = await desk();
    await answerQuery("SurfaceBindings", table());

    // The gesture an operator makes: choose the bank on the *Encoder bank* row,
    // press Learn, press the key. Arming goes to the daemon — one desk, one
    // learn — and nothing is bound until the desk names a control.
    fireEvent.change(screen.getByTestId("action-detail-Encoder bank"), {
      target: { value: "Beam" },
    });
    fireEvent.click(screen.getByTestId("action-learn-Encoder bank"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: { t: "SurfaceLearn", learning: true },
    });

    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "Global", button: "F5" },
    });

    // **One control, one command** — `MachineChange`'s rule, and what stops two
    // operators undoing each other.
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: { t: "SetEncoderBank", group: "Beam" },
      },
    });
    // And the panel has **not** moved: it holds no truth of its own.
    expect(screen.getByTestId("action-keys-Encoder bank").textContent).not.toContain("Global.F5");

    await deliver({ t: "SurfaceBindingsChanged", revision: 4 });
    await answerQuery(
      "SurfaceBindings",
      table({
        revision: 4,
        controls: CONTROLS.map((entry) =>
          entry.name === "Global.F5"
            ? { ...entry, action: { t: "SetEncoderBank", group: "Beam" } }
            : entry,
        ),
      }),
    );
    expect(screen.getByTestId("action-keys-Encoder bank").textContent).toContain("Global.F5");
    expect(queries().filter((query) => query.t === "SurfaceBindings")).toHaveLength(2);
  });

  /**
   * **A row that has not been told which window cannot be learned onto.**
   *
   * The daemon would otherwise be asked to guess, and an operator would find a
   * key bound to something they never picked — which is what `actionOfKind`
   * answering `null` has always been for. What is new is that the panel has to
   * say so *before* the key is pressed rather than when Apply is.
   */
  it("will not arm a row whose second box is unanswered", async () => {
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    const learn = screen.getByTestId("action-learn-Encoder bank");
    expect(learn.hasAttribute("disabled")).toBe(true);
    fireEvent.click(learn);
    expect(commands()).toHaveLength(0);

    fireEvent.change(screen.getByTestId("action-detail-Encoder bank"), {
      target: { value: "Beam" },
    });
    expect(screen.getByTestId("action-learn-Encoder bank").hasAttribute("disabled")).toBe(false);
  });

  /**
   * **A control named while this panel is not arming is left alone.**
   *
   * Learn is one desk's, so a second client can arm it; acting on that here
   * would bind a key somebody else pressed to whichever row this operator last
   * touched.
   */
  it("binds nothing when the desk names a control this panel did not ask for", async () => {
    const { answerQuery, commands, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.change(screen.getByTestId("action-detail-Encoder bank"), {
      target: { value: "Beam" },
    });
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "Global", button: "F5" },
    });
    expect(commands()).toHaveLength(0);
  });

  it("unbinds a control with an action of nothing", async () => {
    // The unbind is on the **key** now rather than on a chooser: it is the key
    // that stops doing something, and the row it was listed under is still
    // there for the next key. `Global.Play` is bound to an executor button,
    // which is one of the fixed rows, so its unbind is a chip's.
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.click(screen.getByTestId("action-unbind-Global.Play"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "Play" },
        action: null,
      },
    });

    // And a custom key's *Remove* is the same command one row-shape along.
    fireEvent.click(screen.getByTestId("custom-unbind-Global.F1"));
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
    //
    // **And the position now comes from the key that was pressed**, which is the
    // one thing an action-first row cannot know about itself: `slotOfControl`
    // reads it off the `BoundControl` learn handed over, where S38 read it off
    // the row's name. Solo is §2.1's second key, so index 1.
    const { answerQuery, commands, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.change(screen.getByTestId("action-target-Executor button"), {
      target: { value: "Strip" },
    });
    fireEvent.click(screen.getByTestId("action-learn-Executor button"));
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "StripButton", button: "Solo" },
    });
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
  it("cannot be unbound, wherever it is listed", async () => {
    // **Re-aimed rather than dropped** (B3). A reserved control has no row of
    // its own to be greyed out in any more — it is a key like any other and it
    // appears under whatever it is bound to. What must still be true is that
    // this panel cannot take it away from the desk switch, so its unbind is
    // dead. `reserved` is the daemon's answer, not a rule this file repeats.
    const { answerQuery, commands } = await desk();
    await answerQuery(
      "SurfaceBindings",
      table({
        controls: CONTROLS.map((entry) =>
          entry.name === "Global.SmpteBeats" ? { ...entry, action: { t: "SaveShow" } } : entry,
        ),
      }),
    );
    const unbind = screen.getByTestId("action-unbind-Global.SmpteBeats");
    expect(unbind.hasAttribute("disabled")).toBe(true);
    fireEvent.click(unbind);
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
  it("marks a key that follows the switch, and leaves a permanent one unmarked", async () => {
    // The fact moved from a column of its own to the key chip when the rows
    // became actions (B3) — it is a property of the *key*, and the key is no
    // longer the row. A star and a tooltip rather than a sentence per row:
    // twenty-four rows with a repeated column would be a column of noise.
    const { answerQuery } = await desk();
    await answerQuery("SurfaceBindings", table());
    // `Global.Play` is permanently ours (§4.3's transport section), so no star.
    expect(screen.getByTestId("action-keys-Executor button").textContent).toContain("Global.Play");
    expect(screen.getByTestId("action-keys-Executor button").textContent).not.toContain("*");
    // `Global.F1` is not, so it carries one — on its custom row, which is where
    // a key that is its own row wears the fact.
    expect(screen.getByTestId("custom-Global.F1").textContent).toContain("Global.F1 *");
  });
});

describe("learn", () => {
  it("arms on the daemon rather than in this panel", async () => {
    const { answerQuery, commands, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());

    fireEvent.click(screen.getByTestId("action-learn-Oops"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: { t: "SurfaceLearn", learning: true },
    });
    // Not armed here until the daemon says so: one desk, one learn, and a panel
    // that armed itself would disagree with a second editor.
    expect(screen.queryByTestId("controls-learning")).toBeNull();

    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    expect(screen.getByTestId("controls-learning")).toBeTruthy();
    expect(screen.getByTestId("action-learn-Oops").textContent).toBe("Press a key…");
    // And the message names the row that is waiting, because a desk with two
    // people at it has to say *what* the next key is about to become.
    expect(screen.getByTestId("controls-learning").textContent).toContain("oops");
  });

  it("stops saying it is armed when the desk names a control", async () => {
    const { answerQuery, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.click(screen.getByTestId("action-learn-Oops"));
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    expect(screen.getByTestId("controls-learning")).toBeTruthy();

    // One shot: the press names the control and disarms.
    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "Global", button: "Play" },
    });
    expect(screen.queryByTestId("controls-learning")).toBeNull();
    expect(screen.getByTestId("action-learn-Oops").textContent).toBe("Learn");
  });

  /**
   * **A key another client learned is not bound by arming a row here.**
   *
   * The control learn named is broadcast to every client and kept in the store,
   * so it is still standing there when the next row arms learn — on this screen
   * or on anybody's. Arming acted on it, which bound a key nobody had pressed,
   * to a row that had merely been armed. Found by `e2e/controls.spec.ts`'s two
   * editors, where the second one's Learn silently took the first one's key.
   */
  it("does not bind the key the last learn named when a row arms", async () => {
    const { answerQuery, deliver, commands } = await desk();
    await answerQuery("SurfaceBindings", table());

    // Somebody else's learn, complete: the control arrives here as a fact.
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "Global", button: "F5" },
    });
    expect(commands()).toHaveLength(0);

    // Now a row here arms learn. The only command that may go out is the arming
    // itself — no binding, because nothing has been pressed since.
    fireEvent.click(screen.getByTestId("action-learn-Oops"));
    expect(commands()).toEqual<Command[]>([
      { t: "ConfigureMachine", change: { t: "SurfaceLearn", learning: true } },
    ]);

    // And the key that *is* pressed afterwards is the one that binds.
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "Global", button: "F6" },
    });
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F6" },
        action: { t: "Oops" },
      },
    });
  });

  it("follows a second client arming it, without being told twice", async () => {
    const { answerQuery, deliver, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    // Nobody pressed anything here. The delta is somebody else's gesture.
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    expect(screen.getByTestId("controls-learning")).toBeTruthy();
    // And it says whose it is, rather than telling this operator to press a key
    // for a row they never armed.
    expect(screen.getByTestId("controls-learning").textContent).toContain("another client");
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
    const learn = screen.getByTestId("action-learn-Oops");
    expect(learn.hasAttribute("disabled")).toBe(true);
    fireEvent.click(learn);
    // And the boxes with it: a select an operator can change that binds nothing
    // is the same fault one control further along.
    expect(screen.getByTestId("action-detail-Encoder bank").hasAttribute("disabled")).toBe(true);
    // The custom section is held too, both the rows that exist and the `+`.
    expect(screen.getByTestId("custom-kind").hasAttribute("disabled")).toBe(true);
    expect(screen.getByTestId("custom-unbind-Global.F1").hasAttribute("disabled")).toBe(true);
    expect(commands()).toHaveLength(0);
  });
});

describe("the rows that need a second answer", () => {
  /** Learns a key onto `kind` and answers with `control`. */
  async function learnOnto(
    deliver: (...deltas: Delta[]) => Promise<void>,
    kind: string,
    control: BoundControl,
  ): Promise<void> {
    fireEvent.click(screen.getByTestId(`action-learn-${kind}`));
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    await deliver({ t: "SurfaceLearnChanged", learning: false, control });
  }

  it("offers an encoder bank, a view number and a window, and sends each", async () => {
    const { answerQuery, commands, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());

    // An encoder bank: the seven feature groups, out of the generated table.
    fireEvent.change(screen.getByTestId("action-detail-Encoder bank"), {
      target: { value: "Beam" },
    });
    await learnOnto(deliver, "Encoder bank", { t: "Global", button: "F5" });
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: { t: "SetEncoderBank", group: "Beam" },
      },
    });

    // A named executor-button function on a panel key — §4.1's transport row,
    // which is the desk's own configuration written by a person.
    fireEvent.change(screen.getByTestId("action-detail-Executor button"), {
      target: { value: "LearnSpeed" },
    });
    await learnOnto(deliver, "Executor button", { t: "Global", button: "F5" });
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
    expect(screen.queryByTestId("action-detail-Oops")).toBeNull();
    // And no *On* box either: an Oops acts on no executor.
    expect(screen.queryByTestId("action-target-Oops")).toBeNull();
    // While one that does have both, has both.
    expect(screen.getByTestId("action-target-Executor button")).toBeTruthy();
    expect(screen.getByTestId("action-detail-Executor button")).toBeTruthy();
  });

  it("disarms when the same row's Learn is pressed again", async () => {
    // The way out for an operator who armed the wrong row: the desk is left
    // unarmed rather than waiting for a key nobody is going to press.
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.click(screen.getByTestId("action-learn-Oops"));
    fireEvent.click(screen.getByTestId("action-learn-Oops"));
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: { t: "SurfaceLearn", learning: false },
    });
  });
});

/**
 * **The custom section** — S43, the owner's rebuild.
 *
 * *In der Custom Befehle Sektion sollte es einen Keybind hinzufügen (oder
 * einfach +) Knopf geben. Dort kann dann der Typ ausgewählt werden: Send
 * Command / Open Window / Jump to View / Execute Macro.*
 *
 * The shape is what makes it a section of its own: the **key** is the row, not
 * the action, because *open window* is fourteen bindings and *type a command* is
 * as many as an operator can think of.
 */
describe("custom keys", () => {
  it("adds one with a type, an answer and a key", async () => {
    const { answerQuery, commands, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());

    // Empty, it is not a binding: a key that writes nothing into the line is a
    // key that does nothing, said obscurely.
    expect(screen.getByTestId("custom-learn").hasAttribute("disabled")).toBe(true);

    fireEvent.change(screen.getByTestId("custom-new-detail"), {
      target: { value: "Go Executor 3" },
    });
    expect(screen.getByTestId("custom-learn").hasAttribute("disabled")).toBe(false);

    fireEvent.click(screen.getByTestId("custom-learn"));
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    // The message names what the next key is about to become, in the words the
    // chooser used — a desk with two people at it has to say so.
    expect(screen.getByTestId("controls-learning").textContent).toContain("send command");

    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "Global", button: "F5" },
    });
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: { t: "WriteCommandLine", line: "Go Executor 3", submit: false },
      },
    });
  });

  /**
   * **The box the owner asked for**: *den Text nur in die Konsole schreiben,
   * oder schreiben und direkt absenden, umschaltbar mit einem Kästchen.*
   *
   * A key bound to `Go Executor 1` that needs Enter afterwards is not a Go key;
   * a key that writes `Store Cue ` for the operator to finish is exactly right.
   * Both are wanted, so the binding carries the answer.
   */
  it("carries the send box on the binding, both ways", async () => {
    const { answerQuery, commands, deliver } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.change(screen.getByTestId("custom-new-detail"), {
      target: { value: "Go Executor 3" },
    });
    fireEvent.click(screen.getByTestId("custom-new-submit"));
    fireEvent.click(screen.getByTestId("custom-learn"));
    await deliver({ t: "SurfaceLearnChanged", learning: true, control: null });
    await deliver({
      t: "SurfaceLearnChanged",
      learning: false,
      control: { t: "Global", button: "F5" },
    });
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: { t: "WriteCommandLine", line: "Go Executor 3", submit: true },
      },
    });
  });

  /** The box is only offered where it means something. */
  it("offers the send box for a line and for nothing else", async () => {
    const { answerQuery } = await desk();
    await answerQuery("SurfaceBindings", table());
    expect(screen.getByTestId("custom-new-submit")).toBeTruthy();
    fireEvent.change(screen.getByTestId("custom-kind"), { target: { value: "Open window" } });
    expect(screen.queryByTestId("custom-new-submit")).toBeNull();
    // And the answer is dropped with the type: a window name is not a view
    // number, and sending it would be asking the daemon to read one as the
    // other.
    fireEvent.change(screen.getByTestId("custom-kind"), { target: { value: "Jump to view" } });
    const detail = screen.getByTestId("custom-new-detail");
    expect(detail instanceof HTMLInputElement ? detail.value : "?").toBe("");
  });

  /**
   * **A custom row is the binding, so it is edited in place.**
   *
   * That is the half a fixed row cannot give: changing the line on an existing
   * key would otherwise mean unbinding it and learning it again.
   */
  it("edits a key that is already bound, without a second Learn", async () => {
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    fireEvent.change(screen.getByTestId("custom-detail-Global.F1"), {
      target: { value: "Patch" },
    });
    expect(commands().at(-1)).toEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F1" },
        action: { t: "OpenWindow", window: "Patch" },
      },
    });
    // And the row has not moved: the table is the daemon's.
    expect(screen.getByTestId("custom-Global.F1").textContent).toContain("FixtureSheet");
  });

  it("says so when there are no custom keys, rather than drawing an empty list", async () => {
    const { answerQuery } = await desk();
    await answerQuery(
      "SurfaceBindings",
      table({ controls: CONTROLS.map((entry) => ({ ...entry, action: null })) }),
    );
    expect(screen.getByTestId("custom-empty")).toBeTruthy();
    // The `+` is there all the same: it is how the section stops being empty.
    expect(screen.getByTestId("custom-new")).toBeTruthy();
  });
});

/**
 * **Export and import** — S43: *die Controls sollen exportiert und importiert
 * werden können.*
 *
 * What travels is a **profile file**, the same document
 * `prism_surface::Bindings::parse` reads, so an export is also a file the daemon
 * can be pointed at. `controlfile.test.ts` holds the document itself; what is
 * asserted here is the panel's half — that an import becomes one
 * `SurfaceBinding` per control, and that a file this desk cannot use is a
 * sentence rather than a silence.
 */
describe("the table as a file", () => {
  it("reads a profile back as one binding per control", async () => {
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());

    const profile = JSON.stringify({
      profileVersion: 1,
      device: "behringer-x-touch",
      bindings: [
        { control: "Global.F5", action: { t: "Oops" } },
        { control: "Main.Fader", action: null },
      ],
    });
    await importFile(profile);

    // **Every control the surface has**, not only the two the file names: an
    // import is *this table becomes that table*, and a merge would leave
    // whatever was bound in the gaps — the state neither file describes.
    const bindings = commands().filter(
      (command) => command.t === "ConfigureMachine" && command.change.t === "SurfaceBinding",
    );
    expect(bindings).toHaveLength(CONTROLS.length);
    expect(bindings).toContainEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F5" },
        action: { t: "Oops" },
      },
    });
    // `Global.F1` opened the fixture sheet and the file does not mention it, so
    // it is unbound rather than left standing.
    expect(bindings).toContainEqual<Command>({
      t: "ConfigureMachine",
      change: {
        t: "SurfaceBinding",
        control: { t: "Global", button: "F1" },
        action: null,
      },
    });
    expect(screen.getByTestId("controls-note").textContent).toContain("2 bindings read");
  });

  it("says which wrong thing a file is, rather than doing nothing", async () => {
    const { answerQuery, commands } = await desk();
    await answerQuery("SurfaceBindings", table());
    await importFile(
      JSON.stringify({ profileVersion: 1, device: "some-other-desk", bindings: [] }),
    );
    expect(commands()).toHaveLength(0);
    expect(screen.getByTestId("controls-note").textContent).toContain("some-other-desk");
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

/**
 * Hands the panel a file, the way the browser's own dialogue would.
 *
 * `File.text()` is a promise, so the assertion has to be after it settles —
 * which is what the extra `act` is for.
 */
async function importFile(text: string): Promise<void> {
  const picker = screen.getByTestId("controls-file");
  const file = new File([text], "controls.json", { type: "application/json" });
  await act(async () => {
    fireEvent.change(picker, { target: { files: [file] } });
    await Promise.resolve();
    await Promise.resolve();
  });
}
