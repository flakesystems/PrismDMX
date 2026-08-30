/**
 * **The look windows, through the whole interface, against a daemon on a socket.**
 *
 * `looks.test.ts` holds the readers to the daemon's answers and `store.test.ts`
 * holds the sentences to them. This drives `<App />` with a daemon on the other
 * end, so what is asserted is what an operator would do: a gesture, the bytes
 * that went out, and — the half that matters — **the sheet not having changed**
 * until the delta came back.
 *
 * The snapshot, the deltas and the answers are all a real `prismd`'s, out of
 * `ui/tests/fixtures/show-recording.json`. The three things this file invents
 * are the *windows*, the *selected executor* and the *selected sequence*: the
 * recorded show was opened with a fresh session, and which windows are open is
 * S25's ground while the two selections are S26's and S39's, all asserted there.
 *
 * # Two windows now, and the tests moved with the panels — S43
 *
 * The owner's rebuild made the Sequence Sheet a **pool** and moved the cue
 * table, the transport and the store bar into the Cue Viewer. Almost every
 * assertion in this file was true before the move and is true after it; what
 * changed is which window has to be open for it. So the default window of
 * {@link desk} is the Cue Viewer, the pool's own handful of tests name the
 * Sequence Sheet, and nothing was deleted for being in the wrong file.
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import type { Answer, AttributeType, Command, JsonValue, WindowType } from "../bindings";
import { ATTRIBUTE_TYPE_VARIANTS } from "../bindings/variants";
import { Connection } from "../ipc/connection";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import { FakeNetwork, ManualTimer, serverMessage } from "../testing/fake-daemon";
import { answerAbout, deltasAbout, showRecording, snapshotOf } from "../testing/show-recording";
import { TelemetryProvider } from "../telemetry/panel";
import { ConsoleContext } from "../desk/consoleshell";
import type { ConsoleShell } from "../desk/consoleshell";
import { CueViewer } from "./cueviewer";

/**
 * The recorded snapshot, with one window open, one executor selected and one cue
 * list in force.
 *
 * The two selections are **separate** since S39, and this helper takes them
 * separately for that reason: the sheet follows `selectedSequence` and the
 * transport line follows `selectedExecutor`, and a test that could only set them
 * together could not tell one from the other.
 */
function recordedSnapshot(
  window: WindowType,
  selectedExecutor: number | null,
  selectedSequence: number | null,
): Snapshot {
  return {
    ...snapshotOf(showRecording.initialSnapshot),
    session: {
      session: {
        activeViewId: 1,
        openWindows: [{ instanceId: 1, type: window, x: 0, y: 0, w: 1920, h: 1080, params: {} }],
        focusedWindow: 1,
        executorPage: 0,
        selectedExecutor,
        selectedSequence,
        editingCue: null,
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
async function desk(
  window: WindowType = "CueViewer",
  selectedExecutor: number | null = 0,
  selectedSequence: number | null = 1,
) {
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
    network.last.deliver(
      serverMessage({
        t: "Snapshot",
        snapshot: recordedSnapshot(window, selectedExecutor, selectedSequence),
      }),
    );
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

  /** Answers the newest outstanding question of `kind` with `answer`. */
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

  /** The daemon answers with the deltas of the step about something. */
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

  return { view, sent, commands, acted, queries, answerQuery, applyStep };
}

/** The show gets a cue list on executor 0, out of the recorded script. */
const A_CUE_LIST = [
  "make a cue list to store into",
  "put it on an executor",
  "and put it in force",
  "store it",
  "store a second cue",
  "name it",
  "give it a fade",
  "and a trigger that carries a time",
] as const;

/** Types into a field. */
function type(testId: string, value: string): void {
  const field = screen.getByTestId(testId);
  if (!(field instanceof HTMLInputElement)) {
    throw new Error(`${testId} is not an input`);
  }
  fireEvent.change(field, { target: { value } });
}

/** The cue numbers the sheet is showing, in order. */
function cueNumbers(): string[] {
  return [...screen.getAllByTestId(/^cue-row-/)].map((row) =>
    (row.getAttribute("data-testid") ?? "").replace("cue-row-", ""),
  );
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the sequence sheet", () => {
  it("says what there is to look at when there is nothing", async () => {
    await desk("SequenceSheet", 0, null);
    expect(screen.getByTestId("sequence-count").textContent).toBe("0 sequences");
    // A fresh show has no sequences, which is a state the pool has to be
    // legible in rather than blank.
    expect(screen.getByTestId("no-sequence")).toBeTruthy();
    // **And no transport line** — S43. It went to the Cue Viewer with the cue
    // table, because it is about one list and this window is about which.
    expect(screen.queryByTestId("looks-executor")).toBeNull();
    expect(screen.queryByTestId("cue-store")).toBeNull();
  });

  it("shows the cue lists the daemon is holding, as a grid of boxes", async () => {
    const { applyStep } = await desk("SequenceSheet");
    await applyStep(...A_CUE_LIST);
    expect(screen.getByTestId("sequence-count").textContent).toBe("1 sequences");
    const box = screen.getByTestId("sequence-1");
    expect(box.textContent).toContain("2 cues");
    // The one that is being edited is lit, and it is the **session's** answer:
    // `selectedSequence`, which the recording put in force.
    expect(box.dataset["current"]).toBe("yes");
    // No cue rows: the list is the other window's.
    expect(screen.queryAllByTestId(/^cue-row-/)).toHaveLength(0);
  });

  /**
   * **The smaller actions are on a right-click** — S43, the owner's rebuild:
   * *Namens und Farbänderungen oder Ähnliches sollen keinen eigenen Knopf
   * bekommen, sondern mit Rechtsklick auf eine Sequence erreichbar sein.*
   *
   * Each item is a line S40 gave to all six pools, which is what makes this a
   * menu rather than five commands only this window can send.
   */
  it("manages a cue list from a right-click, with the lines every pool shares", async () => {
    const { acted, applyStep } = await desk("SequenceSheet");
    await applyStep(...A_CUE_LIST);
    expect(screen.queryByTestId("sequence-menu")).toBeNull();

    fireEvent.contextMenu(screen.getByTestId("sequence-1"));
    const menu = screen.getByTestId("sequence-menu");
    expect(menu.dataset["subject"]).toBe("1");

    fireEvent.click(screen.getByTestId("sequence-rename"));
    type("sequence-rename-input", "Act one");
    fireEvent.submit(screen.getByTestId("sequence-rename-input").closest("form") as HTMLFormElement);
    expect(acted().at(-1)).toEqual({
      t: "Label",
      target: { t: "Sequence", sequenceId: 1 },
      name: "Act one",
    });
    // Choosing an item closes the menu.
    expect(screen.queryByTestId("sequence-menu")).toBeNull();

    fireEvent.contextMenu(screen.getByTestId("sequence-1"));
    fireEvent.click(screen.getByTestId("sequence-colour"));
    type("sequence-colour-input", "blue");
    fireEvent.submit(screen.getByTestId("sequence-colour-input").closest("form") as HTMLFormElement);
    expect(acted().at(-1)).toEqual({
      t: "Color",
      target: { t: "Sequence", sequenceId: 1 },
      color: { r: 0, g: 0, b: 255 },
    });

    fireEvent.contextMenu(screen.getByTestId("sequence-1"));
    fireEvent.click(screen.getByTestId("sequence-copy"));
    expect(acted().at(-1)).toEqual({
      t: "Copy",
      from: { t: "Sequence", sequenceId: 1 },
      to: { t: "Sequence", sequenceId: 2 },
      mode: "Merge",
    });

    fireEvent.contextMenu(screen.getByTestId("sequence-1"));
    fireEvent.click(screen.getByTestId("sequence-delete"));
    expect(acted().at(-1)).toEqual({ t: "Delete", target: { t: "Sequence", sequenceId: 1 } });
    // The box is still there: the pool is the daemon's.
    expect(screen.getByTestId("sequence-1")).toBeTruthy();
  });

  /**
   * **A Move is written and left standing** — `ARCHITECTURE_SPEC.md` §4.5's
   * second shape. The destination is the argument the line is still waiting
   * for, and the operator types it, which is why the item opens no box.
   */
  it("writes a move line rather than guessing where the operator wants it", async () => {
    const { commands, applyStep } = await desk("SequenceSheet");
    await applyStep(...A_CUE_LIST);
    fireEvent.contextMenu(screen.getByTestId("sequence-1"));
    fireEvent.click(screen.getByTestId("sequence-move"));
    expect(commands().at(-1)).toEqual({
      t: "CommandLineInput",
      text: "Move Sequence 1 Sequence ",
      run: false,
    });
  });

  /**
   * **One command, and it is a line** — S40.
   *
   * S28 sent three (`CreateSequence`, `SelectSequence`, `AssignExecutor`) and
   * S39 sent two. S40 sends one, because `Store Sequence 1` on a free number is
   * both acts: the command line cannot know whether the cue list is there, so
   * the command does both and the parser stays out of the show (S26).
   */
  it("makes a cue list with the line an operator could have typed", async () => {
    const { acted } = await desk("SequenceSheet", 0, null);
    fireEvent.click(screen.getByTestId("new-sequence"));
    expect(acted()).toEqual([
      { t: "StoreSequence", sequenceId: 1, name: "Sequence 1", mode: "Append" },
    ]);
    // And nothing on the screen moved: the sheet still says there is nothing.
    expect(screen.getByTestId("sequence-count").textContent).toBe("0 sequences");
  });

  it("makes one the same way with no executor selected", async () => {
    // **S39.** Before it this sent one command and left the operator with a cue
    // list they could not look at; a fader is a separate decision now.
    const { acted } = await desk("SequenceSheet", null, null);
    fireEvent.click(screen.getByTestId("new-sequence"));
    expect(acted()).toEqual([
      { t: "StoreSequence", sequenceId: 1, name: "Sequence 1", mode: "Append" },
    ]);
  });

  it("chooses a cue list with a command and not by remembering it", async () => {
    const { acted, applyStep } = await desk("SequenceSheet", null, null);
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("sequence-1"));
    // **The cue list in force is the session's** — S39's decision, and it is
    // why choosing one is a command rather than a click this interface
    // remembers. No executor is selected and the box is live all the same.
    expect(acted()).toEqual([{ t: "SelectSequence", sequenceId: 1 }]);
  });
});

describe("the cue viewer", () => {
  it("says that no cue list is being edited rather than blaming the executor", async () => {
    // **S39's decision, seen from the screen**, and S43's move. Until S39 this
    // note said *select an executor first*, because the window followed the
    // executor's sequence; a cue list is chosen in its own right now, so an
    // operator with none chosen is told to choose one — in the Sequence Sheet,
    // which is where choosing happens since the rebuild.
    await desk("CueViewer", null, null);
    expect(screen.getByTestId("looks-executor-name").textContent).toBe("No executor selected");
    expect(screen.getByTestId("cue-viewer-empty").textContent).toContain(
      "No cue list is being edited",
    );
    // And the three transport keys are dead, because there is nothing to fire.
    for (const key of ["looks-go", "looks-back", "looks-off"]) {
      expect(screen.getByTestId(key).hasAttribute("disabled")).toBe(true);
    }
  });

  it("shows a cue list nobody has put on a fader", async () => {
    // The other half of S39's decision, and the one an operator meets first: a
    // show is written before anybody decides which fader each list goes on.
    const { applyStep } = await desk("CueViewer", null, 1);
    await applyStep(...A_CUE_LIST);
    expect(screen.getByTestId("looks-executor-name").textContent).toBe("No executor selected");
    expect(cueNumbers()).toEqual(["1", "2"]);
  });

  it("shows the cue list the daemon is holding, once there is one", async () => {
    const { applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    expect(cueNumbers()).toEqual(["1", "2"]);
    expect(screen.getByTestId("cue-name-2").textContent).toBe("Green wash");
    expect(screen.getByTestId("cue-fadeIn-2").textContent).toBe("5.5s");
  });

  /**
   * **Naming a cue is `Label`** since S40: it is the same act as naming a
   * sequence, a group, a preset or a view, so it is the same word.
   * `CueProperty` lost its `Name` when the verb arrived.
   */
  it("names a cue with the verb every pool shares, under the number it started at", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-name-1"));
    type("cue-name-1-input", "Opening");
    fireEvent.keyDown(screen.getByTestId("cue-name-1-input"), { key: "Enter" });
    expect(acted()).toEqual([
      {
        t: "Label",
        target: { t: "Cue", sequenceId: 1, cueNumber: "1" },
        name: "Opening",
      },
    ]);
    // **And the cell still reads what the daemon said**, because the draft was
    // dropped rather than merged: what the cue is comes back as a `ShowPatch`.
    expect(screen.getByTestId("cue-name-1").textContent).toBe("—");
  });

  /**
   * **Renumbering a cue is `Move`** since S40, for `Label`'s reason: moving a
   * cue to another number is the same act as moving a sequence, a group or a
   * preset, and one verb is what makes `Move Cue 3 Cue 8` a line an operator
   * can type.
   */
  it("renumbers with a move, under the old number, which is the key the cue is filed by", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-number-2"));
    type("cue-number-2-input", "1.5");
    fireEvent.keyDown(screen.getByTestId("cue-number-2-input"), { key: "Enter" });
    expect(acted()).toEqual([
      {
        t: "Move",
        from: { t: "Cue", sequenceId: 1, cueNumber: "2" },
        to: { t: "Cue", sequenceId: 1, cueNumber: "1.5" },
        mode: "Merge",
      },
    ]);
    // The list has not reordered: it reorders when the daemon says so.
    expect(cueNumbers()).toEqual(["1", "2"]);
  });

  it("takes a fade time as a number and sends nothing at all for a blank one", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-fadeIn-1"));
    type("cue-fadeIn-1-input", "2.5");
    fireEvent.keyDown(screen.getByTestId("cue-fadeIn-1-input"), { key: "Enter" });
    expect(acted()).toEqual([
      {
        t: "SetCueProperty",
        sequenceId: 1,
        cueNumber: "1",
        property: { t: "FadeIn", seconds: 2.5 },
      },
    ]);

    fireEvent.click(screen.getByTestId("cue-fadeOut-1"));
    type("cue-fadeOut-1-input", "4");
    fireEvent.keyDown(screen.getByTestId("cue-fadeOut-1-input"), { key: "Enter" });
    expect(acted().at(-1)).toEqual({
      t: "SetCueProperty",
      sequenceId: 1,
      cueNumber: "1",
      property: { t: "FadeOut", seconds: 4 },
    });

    // An empty box is *no answer yet*, not zero. A fade that silently became
    // 0 s is a cue that snaps in the middle of a show.
    fireEvent.click(screen.getByTestId("cue-delay-1"));
    type("cue-delay-1-input", "   ");
    fireEvent.keyDown(screen.getByTestId("cue-delay-1-input"), { key: "Enter" });
    expect(acted()).toHaveLength(2);
  });

  it("abandons an edit on Escape and commits one on blur", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-name-1"));
    type("cue-name-1-input", "Gone");
    fireEvent.keyDown(screen.getByTestId("cue-name-1-input"), { key: "Escape" });
    expect(acted()).toEqual([]);
    expect(screen.getByTestId("cue-name-1")).toBeTruthy();

    fireEvent.click(screen.getByTestId("cue-delay-1"));
    type("cue-delay-1-input", "1");
    fireEvent.blur(screen.getByTestId("cue-delay-1-input"));
    expect(acted()).toEqual([
      {
        t: "SetCueProperty",
        sequenceId: 1,
        cueNumber: "1",
        property: { t: "Delay", seconds: 1 },
      },
    ]);
  });

  it("sends a trigger and its time together, because one means nothing without the other", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    const trigger = screen.getByTestId("cue-trigger-1");
    fireEvent.change(trigger, { target: { value: "Time" } });
    expect(acted()).toEqual([
      {
        t: "SetCueProperty",
        sequenceId: 1,
        cueNumber: "1",
        property: { t: "Trigger", trigger: "Time", triggerTime: 0 },
      },
    ]);
    // Cue 2 already carries a `Time` trigger out of the recording, so its time
    // box is the one on the screen.
    fireEvent.blur(screen.getByTestId("cue-trigger-time-2"), { target: { value: "7.5" } });
    expect(acted().at(-1)).toEqual({
      t: "SetCueProperty",
      sequenceId: 1,
      cueNumber: "2",
      property: { t: "Trigger", trigger: "Time", triggerTime: 7.5 },
    });
  });

  it("deletes a cue by the number the row started at", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-delete-2"));
    expect(acted()).toEqual([
      { t: "Delete", target: { t: "Cue", sequenceId: 1, cueNumber: "2" } },
    ]);
    // Still two rows: the list is the daemon's.
    expect(cueNumbers()).toEqual(["1", "2"]);
  });

  it("fires the executor the sheet is following, and only the three keys the protocol has", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("looks-go"));
    fireEvent.click(screen.getByTestId("looks-back"));
    fireEvent.click(screen.getByTestId("looks-off"));
    expect(acted()).toEqual([
      { t: "ExecutorGo", target: { t: "Executor", executorId: 0 }, direction: "Next" },
      { t: "ExecutorGo", target: { t: "Executor", executorId: 0 }, direction: "Prev" },
      { t: "ExecutorOff", target: { t: "Executor", executorId: 0 } },
    ]);
    // Nothing on the screen says it is running: `isActive` is the daemon's.
    expect(screen.getByTestId("looks-executor-state").textContent).toBe("stopped");
  });

  it("says a sequence has no cues rather than drawing an empty table", async () => {
    const { applyStep } = await desk();
    await applyStep("make a cue list to store into", "put it on an executor");
    expect(screen.getByTestId("no-cues").textContent).toContain("no cues");
    // And the store bar is there all the same, because storing is how it stops
    // having none.
    expect(screen.getByTestId("cue-store")).toBeTruthy();
  });

  it("drops an open editor when the cue it names stops existing", async () => {
    // The trap: a cell open on cue 2 while somebody else renumbers cue 2 would
    // otherwise commit an edit onto whatever cue holds that number now.
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-name-2"));
    expect(screen.getByTestId("cue-name-2-input")).toBeTruthy();
    await applyStep("renumber it to 1.5");
    expect(screen.queryByTestId("cue-name-2-input")).toBeNull();
    expect(acted()).toEqual([]);
  });

  it("shows what is running and which cue it is standing on", async () => {
    const { applyStep } = await desk();
    // Before anything runs: a dash, because a stopped playback is on no cue.
    await applyStep(...A_CUE_LIST);
    expect(screen.getByTestId("looks-executor-state").textContent).toBe("stopped");
    expect(screen.getByTestId("looks-executor-cue").textContent).toBe("cue —");
    expect(screen.getByTestId("cue-row-1").dataset["running"]).toBe("no");

    // **S26 and S28 both had to draw a dash here.** S34's readback out of the
    // tick fills it, and this number is the daemon's own answer replayed from a
    // recorded script — not a count of the Gos this interface sent, which would
    // be right until a follow cue fired.
    await applyStep("fire the list");
    expect(screen.getByTestId("looks-executor-state").textContent).toBe("running");
    expect(screen.getByTestId("looks-executor-cue").textContent).toBe("cue 1");
    // And the row the playback is standing on says so.
    expect(screen.getByTestId("cue-row-1").dataset["running"]).toBe("yes");

    await applyStep("step it again");
    expect(screen.getByTestId("looks-executor-cue").textContent).toBe("cue 2");
    expect(screen.getByTestId("cue-row-1").dataset["running"]).toBe("no");
    expect(screen.getByTestId("cue-row-2").dataset["running"]).toBe("yes");
  });
});

describe("the store bar", () => {
  /**
   * **The question is asked when the answer can have changed, and not per
   * frame.**
   *
   * S28 left the warning and named S34 as the session that would test it:
   * `Query::StorePreview` is asked once per delta, which is fine at the rate a
   * cue sheet changes and is *not* fine at playback rates — and S34's readback
   * is what makes the show document move while a playback runs. Firing the list
   * and stepping it produce two `ExecutorState` deltas and no new question; a
   * chase of instantaneous cues would otherwise ask forty-four times a second.
   */
  it("does not ask again when a playback advances a cue", async () => {
    const { queries, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    const before = queries().filter((query) => query.t === "StorePreview").length;
    expect(before).toBeGreaterThan(0);

    await applyStep("fire the list", "step it again");
    expect(screen.getByTestId("looks-executor-cue").textContent).toBe("cue 2");
    expect(queries().filter((query) => query.t === "StorePreview").length).toBe(before);
  });


  it("asks the daemon what the store would do, and puts the answer on the button", async () => {
    const { queries, answerQuery, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    expect(queries().some((query) => query.t === "StorePreview")).toBe(true);
    await answerQuery("StorePreview", answerAbout("this is the overwrite an operator"));
    const button = screen.getByTestId("store-cue");
    // The daemon's own words and the daemon's own mode: two added, three
    // replaced, nothing kept.
    expect(button.textContent).toContain("Merge");
    expect(button.textContent).toContain("2 added");
    expect(button.textContent).toContain("3 replaced");
  });

  it("stores into the number in the box, and offers the next one after that", async () => {
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    // Two cues, so the box offers 3.
    expect(numberIn("store-number")).toBe("3");
    type("store-number", "1");
    fireEvent.submit(screen.getByTestId("cue-store"));
    expect(acted()).toEqual([
      // **The mode is on the command** since S39, and it is the chooser's
      // default rather than the daemon's assumption.
      { t: "StoreCue", sequenceId: 1, cueNumber: "1", mode: "Merge" },
    ]);
    // **Dropped, not kept**: the box goes back to offering the next number of
    // whatever the daemon ends up holding.
    expect(numberIn("store-number")).toBe("3");
  });

  it("asks again in the mode the operator chose, and sends that mode", async () => {
    // **S39, and the two halves have to move together.** Choosing a mode is not
    // a label change: it asks `Query::StorePreview` again with that mode on it,
    // because the counts beside the word are what *that* mode would cost. A bar
    // that changed the word without re-asking would put "Override" over a
    // Merge's numbers, which is worse than the hard-coded "Merge" S28 refused.
    const { acted, queries, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    const before = queries().filter((query) => query.t === "StorePreview").length;

    const chooser = screen.getByTestId("cue-store-mode");
    if (!(chooser instanceof HTMLSelectElement)) {
      throw new Error("the mode chooser is not a select");
    }
    // The three the daemon has, drawn from the generated table rather than from
    // a list this file keeps.
    expect([...chooser.options].map((option) => option.value)).toEqual([
      "Merge",
      "Override",
      "Remove",
    ]);
    expect(chooser.value).toBe("Merge");

    fireEvent.change(chooser, { target: { value: "Override" } });
    expect(queries().filter((query) => query.t === "StorePreview").length).toBe(before + 1);

    fireEvent.submit(screen.getByTestId("cue-store"));
    expect(acted().at(-1)).toEqual({
      t: "StoreCue",
      sequenceId: 1,
      cueNumber: "3",
      mode: "Override",
    });
  });

  it("puts the daemon's word on the button and never one of its own", async () => {
    // The mode on the button is `preview.mode` — the mode the *answer* carried,
    // not the one the chooser reads now. Answering an Override question while
    // the chooser says Override is the ordinary case; this asserts the wiring
    // by answering with a mode the chooser is *not* on, which is the state a
    // client is in for one round trip after every change.
    const { answerQuery, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    await answerQuery("StorePreview", answerAbout("one an operator has to be warned about"));
    const button = screen.getByTestId("store-cue");
    expect(button.textContent).toContain("Override");
    expect(button.textContent).toContain("4 removed");
    const chooser = screen.getByTestId("cue-store-mode");
    if (!(chooser instanceof HTMLSelectElement)) {
      throw new Error("the mode chooser is not a select");
    }
    expect(chooser.value).toBe("Merge");
  });

  it("loads a cue with a command and puts it back with one that carries nothing", async () => {
    // **S39's `EditCue` and `Update`.** Neither of them is a gesture this
    // window resolves: `EditCue` names the cue and the *daemon* fills the
    // programmer, and `Update` names nothing at all because which cue and which
    // mode are both the desk's.
    const { acted, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    // No cue is loaded, so there is no key to press.
    expect(screen.queryByTestId("update-cue")).toBeNull();

    fireEvent.click(screen.getByTestId("cue-edit-1"));
    expect(acted().at(-1)).toEqual({ t: "EditCue", sequenceId: 1, cueNumber: "1" });
    // **And the key still is not there**, because nothing about the update
    // state is held here: it arrives as a `SessionPatch`.
    expect(screen.queryByTestId("update-cue")).toBeNull();

    await applyStep("load cue 3 back into the programmer");
    const key = screen.getByTestId("update-cue");
    expect(key.textContent).toBe("Update cue 3");
    expect(key.getAttribute("data-modified")).toBe("no");
    expect(key.className).not.toContain("update-blinking");

    // The blink is the daemon's state and not a timer this window keeps.
    await applyStep("now change something while the cue is loaded");
    expect(screen.getByTestId("update-cue").getAttribute("data-modified")).toBe("yes");
    expect(screen.getByTestId("update-cue").className).toContain("update-blinking");

    fireEvent.click(screen.getByTestId("update-cue"));
    expect(acted().at(-1)).toEqual({ t: "Update" });

    // And a cleared programmer ends the edit, so the key goes.
    await applyStep("an Update after the programmer has been cleared");
    expect(screen.queryByTestId("update-cue")).toBeNull();
  });

  it("goes dead when the daemon says the store would be refused", async () => {
    const { answerQuery, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    await answerQuery("StorePreview", answerAbout("nothing to store"));
    const button = screen.getByTestId("store-cue");
    expect(button.hasAttribute("disabled")).toBe(true);
    // And it says why, in the daemon's words rather than in this file's.
    expect(button.textContent).toContain("nothing to store");
  });

  it("asks again when the cue number is typed over", async () => {
    const { queries, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    const before = queries().filter((query) => query.t === "StorePreview").length;
    type("store-number", "9");
    expect(queries().filter((query) => query.t === "StorePreview")).toHaveLength(before + 1);
  });
});

/**
 * What every cue of a list inherits, as the daemon would answer it — S48.
 *
 * Written here rather than recorded because what is being checked is the
 * **reading**: a cue sheet given these rows has to draw them, and a script that
 * happened to produce them would be checking the script.
 */
function trackingAnswer(rows: { number: string; inherited: Inherited[] }[]): Answer {
  return {
    t: "CueTracking",
    sequenceId: 1,
    cues: rows.map((row) => ({
      number: row.number,
      inherited: row.inherited,
      // The daemon's own reading, and this test writes it the way the daemon
      // computes it — a cue that inherits nothing asserts everything.
      blocks: row.inherited.length === 0,
    })),
  };
}

/** One inherited value, as the answer carries it. */
interface Inherited {
  readonly fixture: number;
  readonly attribute: AttributeType;
  readonly value: number;
}



/** What is in a text box. */
function numberIn(testId: string): string {
  const field = screen.getByTestId(testId);
  if (!(field instanceof HTMLInputElement)) {
    throw new Error(`${testId} is not an input`);
  }
  return field.value;
}

describe("what a cue inherits", () => {
  beforeEach(() => {
    setLogSink(nullSink);
  });

  /**
   * **The question, and the reason it is a question at all.** Every cue of this
   * list is in the client's mirror, so the window could fold them itself — and
   * must not: that fold is the rule the engine resolves a `Goto` through, and a
   * second implementation of it here would be a second answer to the one thing
   * S48 exists to give a single answer to.
   */
  it("asks the daemon what each cue inherits rather than folding the cues itself", async () => {
    const { queries, applyStep } = await desk("CueViewer");
    await applyStep(...A_CUE_LIST);
    expect(queries().some((query) => query.t === "CueTracking")).toBe(true);
  });

  /**
   * S45's finding, one window along: a playback advancing a cue rewrites the
   * `/sequences` subtree, so an effect keyed on that would put this question on
   * the wire once per cue of a chase. The dependency is the list's `cues` node.
   */
  it("does not ask again when a playback advances a cue", async () => {
    const { queries, applyStep } = await desk("CueViewer");
    await applyStep(...A_CUE_LIST);
    const before = queries().filter((query) => query.t === "CueTracking").length;
    expect(before).toBeGreaterThan(0);

    await applyStep("fire the list", "step it again");
    expect(screen.getByTestId("looks-executor-cue").textContent).toBe("cue 2");
    expect(queries().filter((query) => query.t === "CueTracking").length).toBe(before);
  });

  /**
   * **The half that is new.** It used to be a dash, and a dash answers *this cue
   * does not set it* without answering *so what comes out?* — which is the
   * question this window is opened with.
   */
  it("draws the inherited value where a cue leaves an attribute to track", async () => {
    // Cue 1 sets Red, cue 2 sets Green and says nothing about Red — so Red at
    // cue 2 is the inherited reading, and it is the **daemon** that says what
    // was inherited.
    const show = showWith(
      [{ fixture: 1, attribute: "Red", value: 65535, presetRef: null }],
      [{ fixture: 1, attribute: "Green", value: 65535, presetRef: null }],
    );
    const { unmount } = await renderViewerTracking(
      show,
      trackingAnswer([
        { number: "1", inherited: [] },
        { number: "2", inherited: [{ fixture: 1, attribute: "Red", value: 65535 }] },
      ]),
    );
    const cell = screen.getByTestId("cue-2-Red");
    expect(cell.dataset["set"]).toBe("no");
    expect(cell.dataset["inherited"]).toBe("yes");
    // In brackets and resting, because it is not this cue's assertion — and it
    // carries the number, which is the whole point.
    expect(cell.textContent).toBe("(100%)");
    expect(cell.getAttribute("title")).toContain("fixture 1 100%");
    unmount();
  });

  /**
   * A dash still means something, and it is a different thing: **no cue of this
   * list has ever set it**, so the list is not the one deciding it and the
   * attribute rests at its home value.
   */
  it("keeps the dash where nothing at all is held", async () => {
    // The same two cues, and a daemon that says cue 2 inherits nothing —
    // because cue 1 held its Red **cue-only** and handed it back. The cell is a
    // dash, and it means something different from the one above it: no cue of
    // this list is deciding that attribute, so it rests at its home value.
    const show = showWith(
      [{ fixture: 1, attribute: "Red", value: 65535, presetRef: null, tracking: "CueOnly" }],
      [{ fixture: 1, attribute: "Green", value: 65535, presetRef: null }],
    );
    const { unmount } = await renderViewerTracking(
      show,
      trackingAnswer([
        { number: "1", inherited: [] },
        { number: "2", inherited: [] },
      ]),
    );
    const cell = screen.getByTestId("cue-2-Red");
    expect(cell.dataset["inherited"]).toBe("no");
    expect(cell.textContent).toBe("—");
    unmount();
  });

  /**
   * A cue that inherits nothing asserts everything, and the row says so. It is
   * the daemon's reading and not `inherited.length === 0` worked out here — what
   * counts as *asserting everything* is the tracking rule's answer.
   */
  it("marks a cue that inherits nothing as a blocking cue", async () => {
    const { answerQuery, applyStep } = await desk("CueViewer");
    await applyStep(...A_CUE_LIST);
    await answerQuery(
      "CueTracking",
      trackingAnswer([
        { number: "1", inherited: [] },
        { number: "2", inherited: [{ fixture: 1, attribute: "Red", value: 32768 }] },
      ]),
    );
    expect(screen.getByTestId("cue-tracking-1").dataset["blocks"]).toBe("yes");
    expect(screen.getByTestId("cue-tracking-2").dataset["blocks"]).toBe("no");
  });

  /** The three answers to one question, as one command. */
  it("sends SetCueTracking when a cue is told what to do about tracking", async () => {
    const { acted, applyStep } = await desk("CueViewer");
    await applyStep(...A_CUE_LIST);
    const chooser = screen.getByTestId("cue-tracking-2");
    fireEvent.change(chooser, { target: { value: "CueOnly" } });
    expect(acted().at(-1)).toEqual({
      t: "SetCueTracking",
      sequenceId: 1,
      cueNumber: "2",
      tracking: "CueOnly",
    });

    fireEvent.change(chooser, { target: { value: "Block" } });
    expect(acted().at(-1)).toEqual({
      t: "SetCueTracking",
      sequenceId: 1,
      cueNumber: "2",
      tracking: "Block",
    });
  });

  /**
   * **A cue-only value is still an assertion**, and is drawn as one: while the
   * cue is current this is what goes out. The mark says the list hands it back
   * afterwards, which a dimmed cell would not.
   */
  it("marks a cue-only value rather than drawing it as an absence", async () => {
    const show = showWith([
      { fixture: 1, attribute: "Red", value: 65535, presetRef: null, tracking: "CueOnly" },
      { fixture: 1, attribute: "Green", value: 65535, presetRef: null },
    ]);
    const { unmount } = renderViewer(show);
    const oneOff = screen.getByTestId("cue-1-Red");
    expect(oneOff.dataset["set"]).toBe("yes");
    expect(oneOff.dataset["oneOff"]).toBe("yes");
    expect(oneOff.getAttribute("title")).toContain("taken back");
    // And a part with no `tracking` at all — a daemon one version behind —
    // reads as the tracking value it was, rather than being dropped.
    const tracked = screen.getByTestId("cue-1-Green");
    expect(tracked.dataset["set"]).toBe("yes");
    expect(tracked.dataset["oneOff"]).toBe("no");
    unmount();
  });
});

describe("the cue grid", () => {
  /**
   * **One row per cue and one column per attribute** — S43, the owner's
   * rebuild: *der Cue Viewer soll nur eine Zeile pro Cue haben und für jedes
   * Attribut eine Spalte*.
   *
   * It was one row per **value** — cue, fixture, attribute, value, link —
   * which for a rig of any size is thousands of rows, and which cannot answer
   * the question this window is opened with: *what does this cue change, and
   * what does it leave alone?* A column of dashes is that answer.
   */
  it("draws a column per attribute and a dash where a cue sets nothing", async () => {
    const { applyStep } = await desk("CueViewer");
    await applyStep(...A_CUE_LIST);
    // The columns are the attributes the list touches, in the generated order
    // rather than in one invented here — `AttributeType::ALL`, which is also
    // the order the encoder banks walk.
    const columns = [...screen.getAllByTestId(/^cue-column-/)].map(
      (cell) => cell.textContent ?? "",
    );
    expect(columns.length).toBeGreaterThan(0);
    expect(columns).toEqual([...columns].sort(byAttributeOrder));

    // Every cue of the recording carries a value in the columns it touches,
    // and reads them as percentages.
    for (const attribute of columns) {
      expect(screen.getByTestId(`cue-1-${attribute}`).textContent).toMatch(/(%|—)$/);
    }
  });

  /**
   * **The dash is the reading this window exists for**, and it is asserted over
   * a show built here rather than over the recording: what is being measured is
   * *an attribute one cue sets and another does not*, and a fixture written to
   * say exactly that says it better than a script that happens to.
   */
  it("says with a dash where a cue leaves an attribute to track", async () => {
    const show = showWith([{ fixture: 1, attribute: "Red", value: 65535, presetRef: null }], [
      { fixture: 1, attribute: "Green", value: 65535, presetRef: null },
    ]);
    const { unmount } = renderViewer(show);
    expect(screen.getByTestId("cue-1-Red").dataset["set"]).toBe("yes");
    expect(screen.getByTestId("cue-1-Green").dataset["set"]).toBe("no");
    expect(screen.getByTestId("cue-1-Green").textContent).toBe("—");
    expect(screen.getByTestId("cue-2-Red").dataset["set"]).toBe("no");
    expect(screen.getByTestId("cue-2-Green").dataset["set"]).toBe("yes");
    unmount();
  });

  it("says the range when the fixtures of a cue disagree, and never an average", async () => {
    // Not an average, which is a number no fixture is at; not the first, which
    // would be a claim about the others. The detail is in the title, which is
    // where it went rather than where it was lost.
    const show = showWith([
      { fixture: 1, attribute: "Red", value: 65535, presetRef: null },
      { fixture: 2, attribute: "Red", value: 0, presetRef: null },
      { fixture: 1, attribute: "Green", value: 32768, presetRef: null },
    ]);
    const { unmount } = renderViewer(show);
    const red = screen.getByTestId("cue-1-Red");
    expect(red.textContent).toBe("0–100%");
    expect(red.getAttribute("title")).toContain("Fixture 1: 100%");
    expect(red.getAttribute("title")).toContain("Fixture 2: 0%");
    // One fixture, one number — no range where there is nothing to range over.
    expect(screen.getByTestId("cue-1-Green").textContent).toBe("50%");
    unmount();
  });

  it("marks a linked value and names the preset in the title", async () => {
    const show = showWith([
      { fixture: 1, attribute: "Blue", value: 65535, presetRef: 1 },
    ]);
    const { unmount } = renderViewer(show);
    const cell = screen.getByTestId("cue-1-Blue");
    expect(cell.dataset["linked"]).toBe("yes");
    expect(cell.getAttribute("title")).toContain("preset 1 Deep blue");
    unmount();
  });

  it("names a link to a preset that is not there rather than drawing nothing", async () => {
    // A dangling link is a real state — `Show::remove_preset` leaves the value
    // and the link, and `Show::issues` reports it — so the viewer says which
    // preset is missing rather than hiding the cell an operator has to fix.
    const show = showWith([
      { fixture: 1, attribute: "Red", value: 65535, presetRef: 44 },
    ]);
    const { unmount } = renderViewer(show);
    expect(screen.getByTestId("cue-1-Red").getAttribute("title")).toContain(
      "preset 44 (missing)",
    );
    unmount();
  });

  it("says a cue list has no cues rather than drawing an empty grid", async () => {
    const { applyStep } = await desk("CueViewer");
    await applyStep("make a cue list to store into", "put it on an executor");
    expect(screen.getByTestId("no-cues").textContent).toContain("no cues");
    // And the store bar is there all the same, because storing is how it stops
    // having none.
    expect(screen.getByTestId("cue-store")).toBeTruthy();
  });
});

/**
 * One part of a cue, as this file writes them.
 *
 * A `JsonValue` record and not an interface: a named type is not assignable to
 * the recursive index signature `JsonValue` has, and the shape is what the
 * daemon writes rather than something this file gets to define.
 */
type Part = Record<string, JsonValue>;

/** A show of one cue per argument, and a preset for a link to reach. */
function showWith(...cues: Part[][]): JsonValue {
  return {
    presets: { "1": { pool: "Color", name: "Deep blue", color: null, values: [] } },
    sequences: {
      "1": {
        name: "Act 1",
        loop: false,
        cues: cues.map((parts, index) => ({
          number: String(index + 1),
          name: "",
          fadeIn: 0,
          fadeOut: 0,
          delay: 0,
          trigger: "Go",
          triggerTime: null,
          parts,
        })),
      },
    },
    executors: { "0": { sequenceId: 1, isActive: false, currentCueIndex: null } },
  };
}

/**
 * The window over a show written here, rather than through a daemon.
 *
 * What is being checked in these four is the **reading** — which cell says
 * what — and the mirror is `mirror.test.ts`'s ground. The console shell is
 * brought along because the cue keys write lines (§4.5) and a window that could
 * not reach the console would throw before it drew anything.
 */
function renderViewer(show: JsonValue) {
  return render(
    <DeskProvider store={new DeskStore()}>
      <ConsoleContext.Provider value={NO_CONSOLE}>
        <CueViewer show={show} session={SELECTED} programmer={null} />
      </ConsoleContext.Provider>
    </DeskProvider>,
  );
}

/**
 * The same window, over a store whose one outstanding question is answered with
 * `rows`.
 *
 * The inherited half of a cell is the **daemon's** answer, so a test of the
 * reading has to supply one. `DeskStore::ask` resolves when `answered` is
 * called with the sequence number the enquiry returned, which is the same path a
 * real answer takes — this stands in for the socket and for nothing else.
 */
async function renderViewerTracking(show: JsonValue, rows: Answer) {
  const store = new DeskStore();
  let asked: number | null = null;
  store.attach(
    () => null,
    () => {
      asked = 1;
      return asked;
    },
  );
  const view = render(
    <DeskProvider store={store}>
      <ConsoleContext.Provider value={NO_CONSOLE}>
        <CueViewer show={show} session={SELECTED} programmer={null} />
      </ConsoleContext.Provider>
    </DeskProvider>,
  );
  await act(async () => {
    if (asked !== null) {
      store.answered(asked, rows);
    }
    await Promise.resolve();
  });
  return view;
}

/** A console shell that records nothing: these tests press no cue key. */
const NO_CONSOLE: ConsoleShell = {
  line: "",
  reading: { kind: "empty" },
  prompt: null,
  write: () => undefined,
  append: () => undefined,
  run: () => undefined,
  runWithMode: () => undefined,
  submit: () => undefined,
  answer: () => undefined,
  recall: () => undefined,
};

/** A session with sequence 1 chosen and executor 0 selected. */
const SELECTED: JsonValue = {
  session: { selectedExecutor: 0, selectedSequence: 1 },
};

/** The generated order of two attribute names. */
function byAttributeOrder(left: string, right: string): number {
  const order = (name: string): number =>
    (ATTRIBUTE_TYPE_VARIANTS as readonly string[]).indexOf(name);
  return order(left) - order(right);
}
