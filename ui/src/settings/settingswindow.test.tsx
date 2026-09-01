/**
 * **The settings window, through the whole interface, against a daemon on a
 * socket** — S37.
 *
 * The shape `patch/patchwindow.test.tsx` established, and for its reason: what
 * is asserted is what an operator would do — a gesture, the bytes that went out,
 * and, the half that matters, **the panel not having changed** until the delta
 * came back. A panel that moved on its own would be a client holding a truth of
 * its own, which is what D3 forbids and what every one of S37's exit criteria is
 * really about.
 *
 * There is no recording here, deliberately. The four panels read `outputs`,
 * `machine`, `showFile` and `surfacePort` — four fields of the snapshot rather
 * than a document — so the fixture that matters is the snapshot itself, and
 * `crates/prismd/tests/settings.rs` is what holds the daemon to the same shape.
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import type { Answer, Command, Delta, MachineSettings, ShowFileInfo } from "../bindings";
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

/**
 * What Tauri puts on `window` when the interface is inside the desktop shell —
 * S29, and the whole of what `shell/bridge.ts` looks for.
 */
function installShell(
  invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>,
) {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = { invoke };
}

/** …and taking it away again, so no other test in this file finds one. */
function removeShell() {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
}

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

  const view = render(
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

  /** Moves to a panel, the way an operator does. */
  const openPanel = (name: string): void => {
    fireEvent.click(screen.getByTestId(`settings-tab-${name}`));
  };

  return { view, sent, commands, queries, answerQuery, deliver, openPanel, store };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the window itself", () => {
  it("is a window on the canvas and has the four panels S37 asks for", async () => {
    await desk();
    // It is inside a window frame, not over one: `CLAUDE.md` forbids a modal
    // and the canvas is what holds this.
    expect(screen.getByTestId("window-1").contains(screen.getByTestId("settings"))).toBe(true);
    for (const tab of ["outputs", "devices", "show-files", "this-machine"]) {
      expect(screen.getByTestId(`settings-tab-${tab}`)).toBeTruthy();
    }
    // Outputs first, because a rig is the thing an operator opens this for.
    expect(screen.getByTestId("settings-outputs")).toBeTruthy();
  });

  /**
   * **Which panel is showing is client-local**, which is §4.2's own category.
   *
   * The assertion is on what is *not* sent: moving between tabs is a gesture
   * inside one screen, and a tab that travelled would move under a second
   * operator's hand.
   */
  it("changes panel without telling the daemon anything", async () => {
    const { openPanel, commands } = await desk();
    const before = commands().length;
    openPanel("this-machine");
    expect(screen.getByTestId("settings-machine")).toBeTruthy();
    expect(screen.queryByTestId("settings-outputs")).toBeNull();
    expect(commands()).toHaveLength(before);
  });
});

describe("the Outputs panel", () => {
  it("draws the rig the daemon sent, with its health and its counters", async () => {
    await desk();
    expect(screen.getByTestId("output-row-1")).toBeTruthy();
    expect(screen.getByTestId("output-health-1").textContent).toBe("Ok");
    expect(screen.getByTestId("output-universes-1").textContent).toBe("1");
  });

  /**
   * **Nothing here is state this interface holds**, asserted on what happens
   * before the delta.
   */
  it("sends a command and leaves the table where it was", async () => {
    const { openPanel, commands, deliver } = await desk();
    openPanel("outputs");
    fireEvent.click(screen.getByTestId("output-add"));
    fireEvent.change(screen.getByTestId("output-draft-name"), {
      target: { value: "Bridge node" },
    });
    fireEvent.change(screen.getByTestId("output-draft-universes"), {
      target: { value: "5, 6" },
    });
    fireEvent.submit(screen.getByTestId("output-form"));

    expect(commands().at(-1)).toEqual({
      t: "AddOutput",
      output: {
        id: 2,
        name: "Bridge node",
        kind: { t: "ArtNet", nodes: [], sync: false, ports: [] },
        universes: [5, 6],
        enabled: true,
      },
    });
    // …and the table has not moved: there is one row, and it is the one the
    // daemon sent.
    expect(screen.queryByTestId("output-row-2")).toBeNull();

    // Only when the daemon says so.
    await deliver({
      t: "OutputsChanged",
      outputs: [
        { id: 1, name: "Mock", kind: { t: "Mock" }, universes: [1], enabled: true },
        {
          id: 2,
          name: "Bridge node",
          kind: { t: "ArtNet", nodes: [], sync: false, ports: [] },
          universes: [5, 6],
          enabled: true,
        },
      ],
    });
    expect(screen.getByTestId("output-row-2")).toBeTruthy();
    expect(screen.getByTestId("output-universes-2").textContent).toBe("5, 6");
    // A row that has just arrived is `Disconnected` until its driver says
    // otherwise, which is the truth rather than an omission.
    expect(screen.getByTestId("output-health-2").textContent).toBe("Disconnected");
  });

  /**
   * **One command per field that actually moved**, which is `ConfigureOutput`'s
   * whole reason for carrying one member: a rename must not restart a driver.
   */
  it("renames without touching the kind or the universes", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByLabelText("Edit output 1"));
    fireEvent.change(screen.getByTestId("output-draft-name"), { target: { value: "Hall" } });
    fireEvent.submit(screen.getByTestId("output-form"));
    expect(commands().slice(-1)).toEqual([
      { t: "ConfigureOutput", id: 1, change: { t: "Name", name: "Hall" } },
    ]);
  });

  /**
   * **A number change is a remove and an add, said out loud**, and the remove
   * goes first — the number is the key the row is filed under.
   */
  it("renumbers by removing and adding, in that order", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByLabelText("Edit output 1"));
    fireEvent.change(screen.getByTestId("output-draft-id"), { target: { value: "4" } });
    fireEvent.submit(screen.getByTestId("output-form"));
    const last = commands().slice(-2);
    expect(last[0]).toEqual({ t: "RemoveOutput", id: 1 });
    expect(last[1]?.t).toBe("AddOutput");
  });

  /**
   * **Which universes go nowhere is asked, never worked out.**
   *
   * The panel has the patch and it has the rig; intersecting them here is the
   * shortcut S27 named and D3 forbids, so it asks and draws the answer.
   */
  it("asks the daemon which universes go nowhere and says so", async () => {
    const { queries, answerQuery } = await desk();
    expect(queries().some((query) => query.t === "DarkUniverses")).toBe(true);
    await answerQuery("DarkUniverses", { t: "DarkUniverses", universes: [3, 7] });
    expect(screen.getByTestId("dark-universes").textContent).toContain("3, 7");
    expect(screen.getByTestId("dark-universes").textContent).toContain("no output carries");
  });

  it("says nothing at all when every patched universe has a cable", async () => {
    const { answerQuery } = await desk();
    await answerQuery("DarkUniverses", { t: "DarkUniverses", universes: [] });
    expect(screen.queryByTestId("dark-universes")).toBeNull();
  });

  it("enables and disables a row without a form", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByTestId("output-enabled-1"));
    expect(commands().at(-1)).toEqual({ t: "SetOutputEnabled", id: 1, enabled: false });
  });
});

describe("the Devices panel", () => {
  /** What is plugged in is **asked for**, never mirrored. */
  it("asks what is plugged in when it opens", async () => {
    const { openPanel, queries, answerQuery } = await desk();
    openPanel("devices");
    expect(queries().some((query) => query.t === "MidiPorts")).toBe(true);
    await answerQuery("MidiPorts", {
      t: "MidiPorts",
      ports: [{ name: "X-Touch", input: true, output: true }],
      configured: null,
      open: null,
      status: null,
    });
    expect(screen.getByTestId("port-X-Touch")).toBeTruthy();
    expect(screen.getByTestId("port-count").textContent).toContain("1 MIDI port");
  });

  it("asks again when the operator presses rescan", async () => {
    const { openPanel, queries, answerQuery } = await desk();
    openPanel("devices");
    await answerQuery("MidiPorts", {
      t: "MidiPorts",
      ports: [],
      configured: null,
      open: null,
      status: null,
    });
    const before = queries().filter((query) => query.t === "MidiPorts").length;
    await act(async () => {
      fireEvent.click(screen.getByTestId("ports-rescan"));
      await Promise.resolve();
    });
    expect(queries().filter((query) => query.t === "MidiPorts").length).toBe(before + 1);
  });

  it("chooses a port with a command and follows the delta, not the click", async () => {
    const { openPanel, commands, answerQuery, deliver } = await desk();
    openPanel("devices");
    await answerQuery("MidiPorts", {
      t: "MidiPorts",
      ports: [{ name: "X-Touch", input: true, output: true }],
      configured: null,
      open: null,
      status: null,
    });
    fireEvent.click(screen.getByTestId("port-choose-X-Touch"));
    expect(commands().at(-1)).toEqual({ t: "SetSurfacePort", port: "X-Touch" });
    // Still not chosen: the button says what it said, because the daemon has
    // not answered.
    expect(screen.getByTestId("port-choose-X-Touch").textContent).toBe("Use this");

    await deliver({ t: "SurfaceChanged", port: "X-Touch" });
    expect(screen.getByTestId("port-choose-X-Touch").textContent).toBe("Chosen");
  });

  /**
   * **Named and absent at the same time**, which is the state a panel has to
   * draw and the reason `Answer::MidiPorts` carries the configuration beside the
   * enumeration.
   */
  it("draws a configured desk that is not plugged in", async () => {
    const { openPanel, answerQuery, deliver } = await desk();
    openPanel("devices");
    await deliver({ t: "SurfaceChanged", port: "X-Touch" });
    await answerQuery("MidiPorts", {
      t: "MidiPorts",
      ports: [{ name: "Some synthesiser", input: true, output: true }],
      configured: "X-Touch",
      open: null,
      status: null,
    });
    expect(screen.getByTestId("port-missing").textContent).toContain("configured, not there");
  });

  it("draws the counters and the daemon's own remedy", async () => {
    const { openPanel, answerQuery } = await desk();
    openPanel("devices");
    await answerQuery("MidiPorts", {
      t: "MidiPorts",
      ports: [{ name: "X-Touch", input: true, output: true }],
      configured: "X-Touch",
      open: "X-Touch",
      status: {
        health: "Unresponsive",
        remedy: "the surface has stopped sending: power-cycle it",
        sent: 156,
        superseded: 4,
        touchSuppressed: 2,
        resyncs: 1,
        reserved: 0,
        probes: 1,
        reconnects: 3,
        profile: null,
        boundControls: 120,
      },
    });
    expect(screen.getByTestId("surface-health").textContent).toBe("stopped sending");
    expect(screen.getByTestId("surface-sent").textContent).toBe("156");
    expect(screen.getByTestId("surface-reconnects").textContent).toBe("3");
    expect(screen.getByTestId("surface-bound").textContent).toBe("120 controls");
    // **The words are the daemon's**, and that is the point: the obvious advice
    // for a desk that has stopped sending is *reconnect*, and S20 established
    // that reconnecting is the one thing that cannot recover it.
    expect(screen.getByTestId("surface-remedy").textContent).toContain("power-cycle");
  });

  it("names a binding profile again to re-read it", async () => {
    const { openPanel, commands } = await desk();
    openPanel("devices");
    fireEvent.change(screen.getByTestId("profile-path"), {
      target: { value: "profiles/surface/xtouch.json" },
    });
    fireEvent.submit(screen.getByTestId("profile-form"));
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "SurfaceProfile", path: "profiles/surface/xtouch.json" },
    });
  });
});

describe("the Show files panel", () => {
  it("draws which show is open, whether it is saved, and the autosave", async () => {
    const { openPanel } = await desk();
    openPanel("show-files");
    expect(screen.getByTestId("show-path").textContent).toBe("aula.prism");
    expect(screen.getByTestId("show-full-path").textContent).toBe("D:/shows/aula.prism");
    expect(screen.getByTestId("show-dirty").textContent).toBe("saved");
    expect(screen.getByTestId("show-autosave").textContent).toContain("30 s");
  });

  it("saves with one command and no argument at all", async () => {
    const { openPanel, commands } = await desk();
    openPanel("show-files");
    fireEvent.click(screen.getByTestId("show-save"));
    expect(commands().at(-1)).toEqual({ t: "SaveShow" });
  });

  it("saves under a new name and drops what was typed", async () => {
    const { openPanel, commands } = await desk();
    openPanel("show-files");
    fireEvent.click(screen.getByTestId("show-SaveShowAs"));
    fireEvent.change(screen.getByTestId("show-path-input"), {
      target: { value: "panto.prism" },
    });
    fireEvent.submit(screen.getByTestId("show-form"));
    expect(commands().at(-1)).toEqual({ t: "SaveShowAs", path: "panto.prism" });
    // The form is gone, and the panel still says the daemon's file: it has not
    // been told the save happened.
    expect(screen.queryByTestId("show-form")).toBeNull();
    expect(screen.getByTestId("show-path").textContent).toBe("aula.prism");
  });

  it("follows the daemon when the file changes", async () => {
    const { openPanel, deliver } = await desk();
    openPanel("show-files");
    const next: ShowFileInfo = {
      path: "D:/shows/panto.prism",
      recent: ["D:/shows/aula.prism"],
      unsavedChanges: false,
      recovery: false,
      autosaveSeconds: 30,
    };
    await deliver({ t: "ShowFileChanged", file: next });
    expect(screen.getByTestId("show-path").textContent).toBe("panto.prism");
    expect(screen.getByTestId("show-recent-aula.prism")).toBeTruthy();
  });

  /**
   * An item picked out of a **list** is sent at once — §4.5's rule, because the
   * pointer has supplied the argument.
   */
  it("opens a recent show in one click", async () => {
    const { openPanel, commands } = await desk();
    openPanel("show-files");
    fireEvent.click(screen.getByTestId("show-recent-panto.prism"));
    expect(commands().at(-1)).toEqual({ t: "OpenShow", path: "D:/shows/panto.prism" });
  });

  it("draws the Save lamp from the daemon's flag", async () => {
    const { openPanel, deliver } = await desk();
    openPanel("show-files");
    await deliver({ t: "DirtyFlag", unsavedChanges: true });
    expect(screen.getByTestId("show-dirty").textContent).toBe("unsaved changes");
  });

  it("exports and imports JSON", async () => {
    const { openPanel, commands } = await desk();
    openPanel("show-files");
    fireEvent.click(screen.getByTestId("show-ExportShow"));
    fireEvent.change(screen.getByTestId("show-path-input"), { target: { value: "aula.json" } });
    fireEvent.submit(screen.getByTestId("show-form"));
    expect(commands().at(-1)).toEqual({ t: "ExportShow", path: "aula.json" });

    fireEvent.click(screen.getByTestId("show-ImportShow"));
    fireEvent.change(screen.getByTestId("show-path-input"), { target: { value: "aula.json" } });
    fireEvent.submit(screen.getByTestId("show-form"));
    expect(commands().at(-1)).toEqual({ t: "ImportShow", path: "aula.json" });
  });

  /**
   * **Punch-list B31, in the browser half.** There is no shell here, so there is
   * no dialogue to offer and no button offering one — a *Browse…* that did
   * nothing would be the fault S37's *held by a flag* rows exist to prevent.
   * The box is what makes this panel work in a browser, and it is still here.
   */
  it("offers no file dialogue in a browser, and the box still works", async () => {
    const { openPanel, commands } = await desk();
    openPanel("show-files");
    fireEvent.click(screen.getByTestId("show-OpenShow"));
    expect(screen.queryByTestId("show-form-browse")).toBeNull();
    fireEvent.change(screen.getByTestId("show-path-input"), { target: { value: "typed.prism" } });
    fireEvent.submit(screen.getByTestId("show-form"));
    expect(commands().at(-1)).toEqual({ t: "OpenShow", path: "typed.prism" });
  });

  /**
   * **Punch-list B31, in the shell half.** The dialogue fills the box; the
   * command is the same one, carrying the same field, and it is still the Apply
   * button that sends it — so a path picked by mistake is corrected exactly the
   * way a path typed by mistake is.
   */
  it("puts what the operating system's dialogue returned into the box the daemon is sent", async () => {
    const asked: { kind?: unknown; start?: unknown } = {};
    installShell((command, args) => {
      Object.assign(asked, args);
      expect(command).toBe("choose_path");
      return Promise.resolve("D:/shows/panto.prism");
    });
    try {
      const { openPanel, commands } = await desk();
      openPanel("show-files");
      fireEvent.click(screen.getByTestId("show-SaveShowAs"));
      await act(async () => {
        fireEvent.click(screen.getByTestId("show-form-browse"));
      });
      // The dialogue opened where the show already is, rather than wherever
      // some other program was last used.
      expect(asked).toEqual({ kind: "SaveShowAs", start: "D:/shows/aula.prism" });
      expect((screen.getByTestId("show-path-input") as HTMLInputElement).value).toBe(
        "D:/shows/panto.prism",
      );
      // Nothing has been sent yet: the form still has its Apply button.
      expect(commands().at(-1)).not.toEqual({ t: "SaveShowAs", path: "D:/shows/panto.prism" });

      fireEvent.submit(screen.getByTestId("show-form"));
      expect(commands().at(-1)).toEqual({ t: "SaveShowAs", path: "D:/shows/panto.prism" });
    } finally {
      removeShell();
    }
  });

  it("leaves the box alone when the operator cancels the dialogue", async () => {
    installShell(() => Promise.resolve(null));
    try {
      const { openPanel } = await desk();
      openPanel("show-files");
      fireEvent.click(screen.getByTestId("show-OpenShow"));
      fireEvent.change(screen.getByTestId("show-path-input"), { target: { value: "half-typed" } });
      await act(async () => {
        fireEvent.click(screen.getByTestId("show-form-browse"));
      });
      expect((screen.getByTestId("show-path-input") as HTMLInputElement).value).toBe("half-typed");
    } finally {
      removeShell();
    }
  });
});

describe("the This machine panel", () => {
  it("draws the identity, the directory and what the listener is doing", async () => {
    const { openPanel } = await desk();
    openPanel("this-machine");
    expect(screen.getByTestId("machine-desk-id").textContent).toBe(
      "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
    );
    expect(screen.getByTestId("machine-data-dir").textContent).toBe("C:/ProgramData/PrismDMX");
    expect(screen.getByTestId("listener-state").textContent).toBe("listening on 127.0.0.1:7373");
  });

  it("sends one command per setting and holds nothing", async () => {
    const { openPanel, commands } = await desk();
    openPanel("this-machine");

    fireEvent.change(screen.getByTestId("machine-log-level"), { target: { value: "Debug" } });
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "LogLevel", level: "Debug" },
    });

    fireEvent.click(screen.getByTestId("machine-autostart"));
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "Autostart", autostart: true },
    });

    fireEvent.change(screen.getByTestId("machine-exit"), { target: { value: "Blackout" } });
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "ExitAction", action: "Blackout" },
    });

    // …and the panel still says what the daemon said, because none of them has
    // been answered.
    expect(screen.getByTestId("machine-log-level")).toHaveProperty("value", "Info");
  });

  it("follows the daemon when a setting changes", async () => {
    const { openPanel, deliver } = await desk();
    openPanel("this-machine");
    const next: MachineSettings = machine({ logLevel: "Debug", autostart: true });
    await deliver({ t: "MachineChanged", settings: next });
    expect(screen.getByTestId("machine-log-level")).toHaveProperty("value", "Debug");
    expect(screen.getByTestId("machine-autostart")).toHaveProperty("checked", true);
  });

  /**
   * **S37's switch, acted on at last.** In a browser there is nothing to act,
   * and the row says so rather than drawing a tick beside a start-up entry it
   * cannot see.
   */
  it("says a browser cannot see this machine's start-up entry", async () => {
    const { openPanel } = await desk();
    openPanel("this-machine");
    expect(screen.getByTestId("autostart-entry").textContent).toContain("browser");
  });

  /**
   * **The switch writes two things, and the second one waits for the daemon.**
   * The box sends `ConfigureMachine` and nothing else; the start-up entry is
   * written when the *delta* comes back, because the setting is the daemon's
   * and a second window — or a Web Remote — can turn it on just as well. A
   * shell that wrote the registry on its own click would be acting on a value it
   * does not own, and would write one for a command that was refused.
   */
  it("writes the start-up entry when the daemon confirms the setting, not when the box is clicked", async () => {
    const calls: { command: string; args?: Record<string, unknown> }[] = [];
    installShell((command, args) => {
      calls.push({ command, args });
      return Promise.resolve({
        supported: true,
        installed: command === "autostart_apply" && args?.wanted === true,
        command: '"C:\PrismDMX\PrismDMX.exe" --hidden',
        matchesThisInstall: true,
      });
    });
    try {
      const { openPanel, commands, deliver } = await desk();
      openPanel("this-machine");
      // Read on the way in, and **read** rather than written: an entry somebody
      // deleted in Task Manager would otherwise be repaired by opening a panel,
      // and nobody would learn it had gone.
      expect(calls).toEqual([{ command: "autostart_state", args: {} }]);

      await act(async () => {
        fireEvent.click(screen.getByTestId("machine-autostart"));
      });
      expect(commands().at(-1)).toEqual({
        t: "ConfigureMachine",
        change: { t: "Autostart", autostart: true },
      });
      expect(calls).toHaveLength(1);

      await deliver({ t: "MachineChanged", settings: machine({ autostart: true }) });
      // The delta is what writes it, and the row then says what the machine has.
      expect(calls.at(-1)).toEqual({ command: "autostart_apply", args: { wanted: true } });
      expect(screen.getByTestId("autostart-entry").textContent).toContain("in place");
    } finally {
      removeShell();
    }
  });

  /**
   * **§2.1, said before the operator presses it.**
   *
   * The daemon refuses the address without a token, and this only warns — a
   * disagreement between the two costs a sentence rather than a wrong outcome.
   */
  it("warns before an address that reaches other machines", async () => {
    const { openPanel } = await desk();
    openPanel("this-machine");
    expect(screen.queryByTestId("needs-token")).toBeNull();
    fireEvent.change(screen.getByTestId("machine-websocket"), {
      target: { value: "0.0.0.0:7373" },
    });
    expect(screen.getByTestId("needs-token").textContent).toContain("access token");
  });

  it("asks the daemon for a token rather than choosing one", async () => {
    const { openPanel, commands } = await desk();
    openPanel("this-machine");
    fireEvent.click(screen.getByTestId("machine-new-token"));
    // No value at all: a client that chose the token would be choosing this
    // desk's password.
    expect(commands().at(-1)).toEqual({ t: "ConfigureMachine", change: { t: "NewToken" } });
  });

  it("shows the token once the daemon has made one", async () => {
    const { openPanel, deliver } = await desk();
    openPanel("this-machine");
    expect(screen.getByTestId("machine-token").textContent).toBe("none");
    await deliver({ t: "MachineChanged", settings: machine({ token: "ABCDEFGHJKMNPQRSTVWXYZ0123" }) });
    // Shown rather than hidden: an operator who cannot read it cannot type it
    // into the phone in the auditorium (§2.1).
    expect(screen.getByTestId("machine-token").textContent).toBe("ABCDEFGHJKMNPQRSTVWXYZ0123");
  });

  it("asks for a new identity rather than sending one", async () => {
    const { openPanel, commands } = await desk();
    openPanel("this-machine");
    fireEvent.click(screen.getByTestId("machine-new-identity"));
    expect(commands().at(-1)).toEqual({ t: "ConfigureMachine", change: { t: "NewIdentity" } });
  });

  /**
   * **A row a flag is holding is drawn, disabled, with the flag named.**
   *
   * A box an operator can type into that does nothing is worse than a box that
   * is not there.
   */
  it("greys out what a command line is holding and says which flag", async () => {
    const { openPanel } = await desk({
      machine: machine({ overrides: ["Universes", "Websocket"] }),
    });
    openPanel("this-machine");
    expect(screen.getByTestId("machine-universes")).toHaveProperty("disabled", true);
    expect(screen.getByTestId("machine-websocket")).toHaveProperty("disabled", true);
    // …and the ones nothing is holding are still the operator's.
    expect(screen.getByTestId("machine-log-level")).toHaveProperty("disabled", false);
    expect(screen.getByTestId("machine-network").textContent).toContain("--websocket");
    expect(screen.getByTestId("machine-behaviour").textContent).toContain("--universes");
  });

  it("refuses a universe count outside the desk's range before sending it", async () => {
    const { openPanel, commands } = await desk();
    openPanel("this-machine");
    const before = commands().length;
    fireEvent.change(screen.getByTestId("machine-universes"), { target: { value: "0" } });
    expect(screen.getByTestId("machine-universes-apply")).toHaveProperty("disabled", true);
    fireEvent.change(screen.getByTestId("machine-universes"), { target: { value: "65" } });
    expect(screen.getByTestId("machine-universes-apply")).toHaveProperty("disabled", true);
    expect(commands()).toHaveLength(before);

    fireEvent.change(screen.getByTestId("machine-universes"), { target: { value: "12" } });
    fireEvent.submit(screen.getByTestId("universes-form"));
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "Universes", universes: 12 },
    });
  });
});

describe("closing the window and opening it again", () => {
  /**
   * **Every panel is a reader over daemon state** — S37's fourth exit criterion,
   * in the strongest form a unit test can take: the window is destroyed and the
   * same store draws the same thing, because nothing was kept in it.
   */
  it("shows the same thing, because no panel held anything", async () => {
    const { view, deliver } = await desk();
    await deliver({ t: "MachineChanged", settings: machine({ universes: 24 }) });
    fireEvent.click(screen.getByTestId("settings-tab-this-machine"));
    expect(screen.getByTestId("machine-universes")).toHaveProperty("value", "24");

    // The window goes; the daemon's session is what says so.
    await deliver({
      t: "SessionPatch",
      ops: [{ op: "replace", path: "/session/openWindows", value: [] }],
    });
    expect(screen.queryByTestId("settings")).toBeNull();

    // And comes back, with the same answers in it.
    await deliver({
      t: "SessionPatch",
      ops: [
        {
          op: "replace",
          path: "/session/openWindows",
          value: [{ instanceId: 2, type: "Settings", x: 0, y: 0, w: 1920, h: 1080, params: {} }],
        },
      ],
    });
    fireEvent.click(screen.getByTestId("settings-tab-this-machine"));
    expect(screen.getByTestId("machine-universes")).toHaveProperty("value", "24");
    view.unmount();
  });

  /**
   * A **second** interface, which has never sent a command or seen a delta,
   * draws the same panels from the same snapshot.
   *
   * The stronger form of the same claim, and the one S25 used for the canvas:
   * it holds for a client that took no part in any of it.
   */
  it("looks the same to a client that has just arrived", async () => {
    const first = await desk();
    await first.deliver({ t: "MachineChanged", settings: machine({ universes: 24 }) });
    first.view.unmount();

    const second = await desk({ machine: machine({ universes: 24 }) });
    second.openPanel("this-machine");
    expect(screen.getByTestId("machine-universes")).toHaveProperty("value", "24");
  });
});

describe("the rows that are easy to leave untested", () => {
  /**
   * Every field of the output form, including the ones only one kind has.
   *
   * A form whose Open DMX serial or sACN hop limit was never typed into is a
   * form whose two least-used rows are the ones a venue needs: the serial is
   * what pins one cable when there are two (S8), and the hop limit is what an
   * sACN gateway behind a router needs (§7.2).
   */
  it("carries the fields only one kind of output has", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByTestId("output-add"));

    fireEvent.change(screen.getByTestId("output-draft-kind"), { target: { value: "OpenDmx" } });
    fireEvent.change(screen.getByTestId("output-draft-serial"), { target: { value: "B0037HIY" } });
    // An adapter is one DMX line, and the form says so before the daemon has to.
    fireEvent.change(screen.getByTestId("output-draft-universes"), { target: { value: "1, 2" } });
    expect(screen.getByTestId("output-draft-note").textContent).toContain("exactly one universe");
    fireEvent.change(screen.getByTestId("output-draft-universes"), { target: { value: "1" } });
    fireEvent.submit(screen.getByTestId("output-form"));
    expect(commands().at(-1)).toEqual({
      t: "AddOutput",
      output: {
        id: 2,
        name: "Output 2",
        kind: { t: "OpenDmx", serial: "B0037HIY" },
        universes: [1],
        enabled: true,
      },
    });

    fireEvent.click(screen.getByTestId("output-add"));
    fireEvent.change(screen.getByTestId("output-draft-kind"), { target: { value: "Sacn" } });
    fireEvent.change(screen.getByTestId("output-draft-ttl"), { target: { value: "8" } });
    fireEvent.change(screen.getByTestId("output-draft-addresses"), {
      target: { value: "192.168.1.20:5568\n192.168.1.21:5568" },
    });
    fireEvent.change(screen.getByTestId("output-draft-universes"), { target: { value: "3 4" } });
    fireEvent.submit(screen.getByTestId("output-form"));
    expect(commands().at(-1)).toEqual({
      t: "AddOutput",
      output: {
        id: 2,
        name: "Output 2",
        kind: {
          t: "Sacn",
          receivers: ["192.168.1.20:5568", "192.168.1.21:5568"],
          ttl: 8,
          ports: [],
        },
        universes: [3, 4],
        enabled: true,
      },
    });

    // A mock, which is the one kind with nothing to configure at all.
    fireEvent.click(screen.getByTestId("output-add"));
    fireEvent.change(screen.getByTestId("output-draft-kind"), { target: { value: "Mock" } });
    fireEvent.submit(screen.getByTestId("output-form"));
    expect(commands().at(-1)).toMatchObject({ t: "AddOutput" });
  });

  it("will not send a list of universes that is not a list of universes", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByTestId("output-add"));
    const before = commands().length;
    fireEvent.change(screen.getByTestId("output-draft-universes"), { target: { value: "one" } });
    expect(screen.getByTestId("output-draft-apply")).toHaveProperty("disabled", true);
    expect(screen.getByTestId("output-draft-note").textContent).toContain("whole numbers");
    fireEvent.submit(screen.getByTestId("output-form"));
    expect(commands()).toHaveLength(before);
  });

  it("removes a row, and cancelling sends nothing at all", async () => {
    const { commands } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByLabelText("Edit output 1"));
    fireEvent.click(screen.getByTestId("output-draft-cancel"));
    expect(screen.queryByTestId("output-form")).toBeNull();
    expect(commands()).toHaveLength(before);

    fireEvent.click(screen.getByTestId("output-remove-1"));
    expect(commands().at(-1)).toEqual({ t: "RemoveOutput", id: 1 });
  });

  /** An **age**, not a time — S33's rule, drawn. */
  it("draws the last error with how long ago it was", async () => {
    const { deliver, answerQuery } = await desk();
    await deliver({ t: "OutputsChanged", outputs: [] });
    expect(screen.getByTestId("no-outputs")).toBeTruthy();

    await deliver({
      t: "OutputsChanged",
      outputs: [{ id: 1, name: "Node", kind: { t: "Mock" }, universes: [1], enabled: true }],
    });
    await answerQuery("OutputStatus", {
      t: "OutputStatus",
      outputs: [
        {
          id: 1,
          health: "Degraded",
          framesSent: 4123,
          lastError: "the cable came out",
          lastErrorAgoMs: 4200,
          nodes: [],
        },
      ],
    });
    expect(screen.getByTestId("output-frames-1").textContent).toBe("4123");
    expect(screen.getByTestId("output-health-1").textContent).toBe("Degraded");
    expect(screen.getByTestId("output-error-1").textContent).toBe("the cable came out (4 s ago)");
    expect(screen.queryByTestId("output-nodes-1")).toBeNull();
  });

  /**
   * **Punch-list B6, where an operator reads it** — S46.
   *
   * The health word is the daemon's; what this asserts is that the panel draws
   * it *and* names the node, because `Degraded` on its own does not tell an
   * installer which box to walk to.
   */
  it("never draws Ok over a node that has never answered, and says which one", async () => {
    const { deliver, answerQuery } = await desk();
    await deliver({
      t: "OutputsChanged",
      outputs: [
        {
          id: 1,
          name: "Stage left",
          kind: { t: "ArtNet", nodes: ["10.0.0.9:6454"], sync: false, ports: [] },
          universes: [1],
          enabled: true,
        },
      ],
    });
    await answerQuery("OutputStatus", {
      t: "OutputStatus",
      outputs: [
        {
          id: 1,
          // What the daemon folds: the socket is happy and nothing answers.
          health: "Degraded",
          framesSent: 4123,
          lastError: null,
          lastErrorAgoMs: null,
          nodes: [
            {
              address: "10.0.0.9:6454",
              health: "NeverAnswered",
              name: null,
              lastReplyAgoMs: null,
            },
          ],
        },
      ],
    });
    expect(screen.getByTestId("output-health-1").textContent).toContain("Degraded");
    expect(screen.getByTestId("output-health-1").textContent).not.toContain("Ok");
    expect(screen.getByTestId("output-nodes-1").textContent).toBe(
      "10.0.0.9:6454 has never answered",
    );
  });

  /** The third state, and the time on it — S46. */
  it("says when a node stopped answering, as a time of day", async () => {
    const { deliver, answerQuery } = await desk();
    await deliver({
      t: "OutputsChanged",
      outputs: [
        {
          id: 1,
          name: "Stage left",
          kind: { t: "ArtNet", nodes: ["10.0.0.9:6454"], sync: false, ports: [] },
          universes: [1],
          enabled: true,
        },
      ],
    });
    // Ninety seconds ago, on the daemon's clock. The panel turns the age into a
    // time against its own, which is the half that is the client's.
    const ago = 90_000;
    await answerQuery("OutputStatus", {
      t: "OutputStatus",
      outputs: [
        {
          id: 1,
          health: "Degraded",
          framesSent: 4123,
          lastError: null,
          lastErrorAgoMs: null,
          nodes: [
            {
              address: "10.0.0.9:6454",
              health: "Stopped",
              name: "Stage left",
              lastReplyAgoMs: ago,
            },
          ],
        },
      ],
    });
    const at = new Date(Date.now() - ago);
    const clock = `${String(at.getHours()).padStart(2, "0")}:${String(at.getMinutes()).padStart(2, "0")}`;
    expect(screen.getByTestId("output-nodes-1").textContent).toBe(
      `10.0.0.9:6454 stopped answering at ${clock}`,
    );
  });

  /** A node that answers adds nothing to the row — S46. */
  it("says nothing under the health of an output whose nodes all answer", async () => {
    const { deliver, answerQuery } = await desk();
    await deliver({
      t: "OutputsChanged",
      outputs: [
        {
          id: 1,
          name: "Stage left",
          kind: { t: "ArtNet", nodes: ["10.0.0.9:6454"], sync: false, ports: [] },
          universes: [1],
          enabled: true,
        },
      ],
    });
    await answerQuery("OutputStatus", {
      t: "OutputStatus",
      outputs: [
        {
          id: 1,
          health: "Ok",
          framesSent: 4123,
          lastError: null,
          lastErrorAgoMs: null,
          nodes: [
            {
              address: "10.0.0.9:6454",
              health: "Answering",
              name: "Stage left",
              lastReplyAgoMs: 120,
            },
          ],
        },
      ],
    });
    expect(screen.getByTestId("output-health-1").textContent).toBe("Ok");
    expect(screen.queryByTestId("output-nodes-1")).toBeNull();
  });

  /**
   * *Not listening* is drawn before the list is — S46.
   *
   * An empty list under a socket that never opened says nothing at all about
   * the network, and reading it as *no nodes* would be B6 pointed the other way.
   */
  it("says it is not listening before it says there are no nodes", async () => {
    const { answerQuery } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      nodes: [],
      listening: false,
      error: null,
      counters: { pollsSent: 0, pollsFailed: 0, replies: 0, malformed: 0, dropped: 0, readErrors: 0 },
      remedy: null,
    });
    expect(screen.getByTestId("artnet-not-listening").textContent).toContain(
      "once an Art-Net output is configured",
    );
    expect(screen.queryByTestId("artnet-no-nodes")).toBeNull();
  });

  /** The other cause of *not listening*, and the one that has a reason — S46. */
  it("says why it could not listen, in the daemon's own words", async () => {
    const { answerQuery } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      nodes: [],
      listening: false,
      error: "the local address could not be bound",
      counters: { pollsSent: 0, pollsFailed: 0, replies: 0, malformed: 0, dropped: 0, readErrors: 0 },
      remedy: null,
    });
    expect(screen.getByTestId("artnet-not-listening").textContent).toContain(
      "the local address could not be bound",
    );
    expect(screen.queryByTestId("artnet-no-nodes")).toBeNull();
  });

  /**
   * **What the desk has actually done** — S46, added after a real node read
   * `Degraded` while it was answering every poll.
   *
   * Polls going out with nothing coming back, nothing being asked at all, and
   * something arriving and being dropped are three different faults, and from
   * this panel they looked identical. The numbers are the daemon's; the panel
   * reads them out and interprets nothing.
   */
  it("says how many polls went out and how many replies came back", async () => {
    const { answerQuery } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      nodes: [],
      listening: true,
      error: null,
      counters: { pollsSent: 42, pollsFailed: 0, replies: 0, malformed: 3, dropped: 0, readErrors: 0 },
      remedy: null,
    });
    const line = screen.getByTestId("artnet-counters").textContent ?? "";
    expect(line).toContain("42 polls sent");
    expect(line).toContain("0 replies");
    expect(line, "a reply that arrived and could not be read is its own fact").toContain(
      "3 unreadable",
    );
    // The quiet counters stay off the line: a panel that printed six zeroes
    // every second would be six things to read past.
    expect(line).not.toContain("dropped");
    expect(line).not.toContain("could not be sent");
  });

  /**
   * **The evening this cost, as a line on the screen** — S46.
   *
   * A node that was connected, reachable and answering every poll read
   * `Degraded`, because the inbound firewall rule for `prismd` covered the
   * Private profile and the lighting network was Public. The daemon could see
   * the fingerprint the whole time. Now it says so, in its own words — this
   * panel renders them and writes none of its own, because the sentence a
   * client would compose from the counters is *check the node*, and the node is
   * the one thing that is working.
   */
  it("passes on what the daemon suggests when nothing at all comes back", async () => {
    const { answerQuery } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      nodes: [],
      listening: true,
      error: null,
      counters: { pollsSent: 21, pollsFailed: 0, replies: 0, malformed: 0, dropped: 0, readErrors: 0 },
      remedy:
        "21 polls have gone out and nothing at all has come back — check that inbound UDP on port 6454 is allowed for prismd",
    });
    expect(screen.getByTestId("artnet-remedy").textContent).toContain("inbound UDP on port 6454");
    expect(screen.getByTestId("artnet-counters").textContent).toContain("21 polls sent");
  });

  /** …and says nothing when the daemon has nothing to suggest. */
  it("suggests nothing of its own when the daemon suggests nothing", async () => {
    const { answerQuery } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      nodes: [],
      listening: true,
      error: null,
      counters: { pollsSent: 2, pollsFailed: 0, replies: 0, malformed: 0, dropped: 0, readErrors: 0 },
      remedy: null,
    });
    expect(screen.queryByTestId("artnet-remedy")).toBeNull();
  });

  /** Nothing is counted at a desk that never opened a socket. */
  it("counts nothing while it is not listening", async () => {
    const { answerQuery } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      nodes: [],
      listening: false,
      error: null,
      counters: { pollsSent: 0, pollsFailed: 0, replies: 0, malformed: 0, dropped: 0, readErrors: 0 },
      remedy: null,
    });
    expect(screen.queryByTestId("artnet-counters")).toBeNull();
    expect(screen.getByTestId("artnet-not-listening")).toBeTruthy();
  });

  /** A desk that *is* listening and has heard nothing says a different thing. */
  it("tells a silent network apart from a socket that never opened", async () => {
    const { answerQuery } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      nodes: [],
      listening: true,
      error: null,
      counters: { pollsSent: 4, pollsFailed: 0, replies: 0, malformed: 0, dropped: 0, readErrors: 0 },
      remedy: null,
    });
    expect(screen.queryByTestId("artnet-not-listening")).toBeNull();
    expect(screen.getByTestId("artnet-no-nodes")).toBeTruthy();
  });

  /**
   * *Discovered is not configured*, and the one click — S46.
   *
   * The universes come from `suggestedUniverses`, which is the daemon's: the
   * default port-address mapping run backwards. A panel that computed `port + 1`
   * would be a third spelling of a rule stated in `ARCHITECTURE_SPEC.md` §7.0.
   */
  it("lists what is out there, where it disagrees, and adds one in a click", async () => {
    const { answerQuery, commands } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      listening: true,
      error: null,
      counters: { pollsSent: 12, pollsFailed: 0, replies: 12, malformed: 0, dropped: 0, readErrors: 0 },
      remedy: null,
      nodes: [
        {
          address: "10.0.0.11:6454",
          ip: "10.0.0.11",
          shortName: "New node",
          longName: "A node in the gallery",
          mac: "00:1a:2b:3c:4d:5e",
          firmware: 260,
          style: 0,
          status1: 208,
          status2: 14,
          ports: [3, 4],
          inputs: [],
          configured: false,
          unaddressedPorts: [3, 4],
          missingPorts: [],
          suggestedUniverses: [4, 5],
          replies: 2,
          lastReplyAgoMs: 500,
        },
      ],
    });

    expect(screen.getByTestId("artnet-node-0").textContent).toContain("New node");
    expect(screen.getByTestId("artnet-node-ports-0").textContent).toBe("3, 4");
    expect(screen.getByTestId("artnet-node-state-0").textContent).toContain("not configured");
    expect(screen.getByTestId("artnet-node-state-0").textContent).toContain(
      "outputs 3, 4, which this desk sends nothing on",
    );

    const before = commands().length;
    fireEvent.click(screen.getByTestId("artnet-node-add-0"));
    expect(
      commands(),
      "the click fills the form; it does not change a rig on a stage",
    ).toHaveLength(before);
    expect(screen.getByTestId("output-draft-addresses")).toHaveProperty(
      "value",
      "10.0.0.11:6454",
    );
    expect(screen.getByTestId("output-draft-universes")).toHaveProperty("value", "4, 5");
    expect(screen.getByTestId("output-draft-name")).toHaveProperty("value", "New node");
    expect(screen.getByTestId("output-draft-kind")).toHaveProperty("value", "ArtNet");

    // …and applying it is the ordinary Add, with nothing new in it.
    fireEvent.submit(screen.getByTestId("output-form"));
    expect(commands().at(-1)).toMatchObject({
      t: "AddOutput",
      output: {
        name: "New node",
        universes: [4, 5],
        kind: { t: "ArtNet", nodes: ["10.0.0.11:6454"] },
      },
    });
  });

  /** The disagreement in the other direction — S46. */
  it("says when this desk sends a node a universe the node does not have", async () => {
    const { answerQuery } = await desk();
    await answerQuery("ArtNetNodes", {
      t: "ArtNetNodes",
      listening: true,
      error: null,
      counters: { pollsSent: 12, pollsFailed: 0, replies: 12, malformed: 0, dropped: 0, readErrors: 0 },
      remedy: null,
      nodes: [
        {
          address: "10.0.0.9:6454",
          ip: "10.0.0.9",
          shortName: "Stage left",
          longName: "Stage left node",
          mac: "00:1a:2b:3c:4d:5e",
          firmware: 260,
          style: 0,
          status1: 208,
          status2: 14,
          ports: [0],
          inputs: [],
          configured: true,
          unaddressedPorts: [],
          missingPorts: [9],
          suggestedUniverses: [1],
          replies: 12,
          lastReplyAgoMs: 200,
        },
      ],
    });
    expect(screen.getByTestId("artnet-node-state-0").textContent).toContain("configured");
    expect(screen.getByTestId("artnet-node-state-0").textContent).toContain(
      "this desk sends 9, which it does not have",
    );
    expect(screen.getByTestId("artnet-node-add-0").textContent).toBe("Add another output");
  });

  it("changes the network settings one command at a time", async () => {
    const { openPanel, commands } = await desk();
    openPanel("this-machine");

    fireEvent.click(screen.getByTestId("machine-local"));
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "Local", local: false },
    });

    fireEvent.change(screen.getByTestId("machine-websocket"), {
      target: { value: "127.0.0.1:9001" },
    });
    fireEvent.submit(screen.getByTestId("websocket-form"));
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "Websocket", address: "127.0.0.1:9001" },
    });

    // A cleared box is *no listener at all*, which is what `null` means on the
    // wire — and is a different thing from a box nobody has touched.
    fireEvent.change(screen.getByTestId("machine-websocket"), { target: { value: "  " } });
    fireEvent.submit(screen.getByTestId("websocket-form"));
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "Websocket", address: null },
    });

    fireEvent.change(screen.getByTestId("machine-library"), { target: { value: "D:/fixtures" } });
    fireEvent.submit(screen.getByTestId("library-form"));
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "FixtureLibrary", path: "D:/fixtures" },
    });
  });

  it("takes the token away, which is what closes the door it held open", async () => {
    const { openPanel, commands, deliver } = await desk();
    openPanel("this-machine");
    // Nothing to remove until there is one.
    expect(screen.queryByTestId("machine-clear-token")).toBeNull();
    await deliver({ t: "MachineChanged", settings: machine({ token: "SECRET" }) });
    fireEvent.click(screen.getByTestId("machine-clear-token"));
    expect(commands().at(-1)).toEqual({
      t: "ConfigureMachine",
      change: { t: "Token", token: null },
    });
  });

  it("cancels a file prompt without sending anything", async () => {
    const { openPanel, commands } = await desk();
    openPanel("show-files");
    const before = commands().length;
    fireEvent.click(screen.getByTestId("show-NewShow"));
    expect(screen.getByTestId("show-form-hint").textContent).toContain("refused rather than replaced");
    fireEvent.click(screen.getByTestId("show-form-cancel"));
    expect(screen.queryByTestId("show-form")).toBeNull();
    expect(commands()).toHaveLength(before);
  });

  it("says when a recovery copy is standing beside the show", async () => {
    const { openPanel, deliver } = await desk();
    openPanel("show-files");
    await deliver({
      t: "ShowFileChanged",
      file: {
        path: "D:/shows/aula.prism",
        recent: [],
        unsavedChanges: true,
        recovery: true,
        autosaveSeconds: 30,
      },
    });
    expect(screen.getByTestId("show-autosave").textContent).toContain("recovery copy is standing");
    // With nothing before it, there is no recent list rather than an empty one.
    expect(screen.queryByTestId("show-recent")).toBeNull();
  });

  /**
   * A panel with nothing behind it says so rather than drawing an empty frame.
   *
   * Reachable in one ordinary case: a daemon one version behind, whose snapshot
   * carries neither field — which `ipc/protocol.ts` tolerates rather than
   * refusing, for `OutputSnapshot`'s S33 reason.
   */
  it("says so when the daemon has said nothing about itself", async () => {
    const { openPanel, store } = await desk();
    act(() => {
      store.applySnapshot({
        ...withSettings(),
        machine: null as unknown as MachineSettings,
        showFile: null as unknown as ShowFileInfo,
      });
    });
    openPanel("this-machine");
    expect(screen.getByText(/has not said what this machine is set to/u)).toBeTruthy();
    openPanel("show-files");
    expect(screen.getByText(/has not said which show it is holding/u)).toBeTruthy();
  });

  it("offers no port it cannot use as a surface", async () => {
    const { openPanel, answerQuery } = await desk();
    openPanel("devices");
    await answerQuery("MidiPorts", {
      t: "MidiPorts",
      ports: [
        { name: "X-Touch", input: true, output: true },
        // A keyboard with no lamps, and a synthesiser with no keys. A desk needs
        // both directions — motor faders and scribble strips are the outbound
        // half — so neither of these can be chosen.
        { name: "Keys only", input: true, output: false },
        { name: "Lamps only", input: false, output: true },
      ],
      configured: null,
      open: null,
      status: null,
    });
    expect(screen.getByTestId("port-choose-Keys only")).toHaveProperty("disabled", true);
    expect(screen.getByTestId("port-choose-Lamps only")).toHaveProperty("disabled", true);
    expect(screen.getByTestId("port-choose-X-Touch")).toHaveProperty("disabled", false);
  });

  it("forgets a configured port whose desk is not there", async () => {
    const { openPanel, commands, answerQuery, deliver } = await desk();
    openPanel("devices");
    await deliver({ t: "SurfaceChanged", port: "X-Touch" });
    await answerQuery("MidiPorts", {
      t: "MidiPorts",
      ports: [],
      configured: "X-Touch",
      open: null,
      status: null,
    });
    fireEvent.click(screen.getByTestId("port-clear"));
    expect(commands().at(-1)).toEqual({ t: "SetSurfacePort", port: null });
  });

  it("re-addresses a row without renaming it", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByLabelText("Edit output 1"));
    fireEvent.change(screen.getByTestId("output-draft-kind"), { target: { value: "ArtNet" } });
    fireEvent.change(screen.getByTestId("output-draft-addresses"), {
      target: { value: "192.168.1.50:6454" },
    });
    fireEvent.change(screen.getByTestId("output-draft-universes"), { target: { value: "5, 6" } });
    fireEvent.submit(screen.getByTestId("output-form"));
    // Two commands, and no rename among them: `ConfigureOutput` carries one
    // member so that a field nobody touched costs the rig nothing.
    expect(commands().slice(-2)).toEqual([
      {
        t: "ConfigureOutput",
        id: 1,
        change: {
          t: "Kind",
          kind: { t: "ArtNet", nodes: ["192.168.1.50:6454"], sync: false, ports: [] },
        },
      },
      { t: "ConfigureOutput", id: 1, change: { t: "Universes", universes: [5, 6] } },
    ]);
  });

  it("greys the surface rows a --surface flag is holding", async () => {
    const { openPanel, answerQuery } = await desk({
      machine: machine({ overrides: ["Surface", "SurfaceProfile"] }),
    });
    openPanel("devices");
    await answerQuery("MidiPorts", {
      t: "MidiPorts",
      ports: [{ name: "X-Touch", input: true, output: true }],
      configured: null,
      open: null,
      status: null,
    });
    expect(screen.getByTestId("surface-held").textContent).toContain("--surface");
    expect(screen.getByTestId("port-choose-X-Touch")).toHaveProperty("disabled", true);
    expect(screen.getByTestId("profile-path")).toHaveProperty("disabled", true);
    expect(screen.getByTestId("profile-note").textContent).toContain("--surface-profile");
  });
});

describe("a daemon that has gone", () => {
  it("takes the settings with it rather than showing a machine that is not there", async () => {
    const { store } = await desk();
    act(() => {
      store.disconnected();
    });
    expect(screen.queryByTestId("settings")).toBeNull();
    expect(store.getState().machine).toBeNull();
    expect(store.getState().showFile).toBeNull();
  });
});
