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
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import type { Answer, Command, JsonValue, WindowType } from "../bindings";
import { Connection } from "../ipc/connection";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import { FakeNetwork, ManualTimer, serverMessage } from "../testing/fake-daemon";
import { answerAbout, deltasAbout, showRecording, snapshotOf } from "../testing/show-recording";
import { TelemetryProvider } from "../telemetry/panel";
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
  window: WindowType = "SequenceSheet",
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
    // A fresh show has no sequences and nothing is in force, which is a state
    // the sheet has to be legible in rather than blank.
    expect(screen.getByTestId("no-sequence")).toBeTruthy();
    expect(screen.getByTestId("looks-executor-name").textContent).toBe("Executor 0");
  });

  it("says that no cue list is in force rather than blaming the executor", async () => {
    // **S39's decision, seen from the screen.** Until it landed this note said
    // *select an executor first*, because the sheet followed the executor's
    // sequence; a cue list is chosen in its own right now, so an operator with
    // no executor selected is told what to do about the cue list rather than
    // about a fader they may not want yet.
    await desk("SequenceSheet", null, null);
    expect(screen.getByTestId("looks-executor-name").textContent).toBe("No executor selected");
    expect(screen.getByTestId("no-sequence").textContent).toContain("No cue list is in force");
    expect(screen.getByTestId("no-sequence").textContent).not.toContain("executor");
    // And the three transport keys are dead, because there is nothing to fire.
    for (const key of ["looks-go", "looks-back", "looks-off"]) {
      expect(screen.getByTestId(key).hasAttribute("disabled")).toBe(true);
    }
  });

  it("shows a cue list nobody has put on a fader", async () => {
    // The other half of S39's decision, and the one an operator meets first: a
    // show is written before anybody decides which fader each list goes on.
    const { applyStep } = await desk("SequenceSheet", null, 1);
    await applyStep(...A_CUE_LIST);
    expect(screen.getByTestId("looks-executor-name").textContent).toBe("No executor selected");
    expect(cueNumbers()).toEqual(["1", "2"]);
  });

  it("shows the cue list the daemon is holding, once there is one", async () => {
    const { applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    expect(screen.getByTestId("sequence-count").textContent).toBe("1 sequences");
    expect(cueNumbers()).toEqual(["1", "2"]);
    expect(screen.getByTestId("cue-name-2").textContent).toBe("Green wash");
    expect(screen.getByTestId("cue-fadeIn-2").textContent).toBe("5.5s");
    expect(screen.getByTestId("cue-parts-1").textContent).toBe("5");
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
    // remembers. No executor is selected and the chip is live all the same.
    expect(acted()).toEqual([{ t: "SelectSequence", sequenceId: 1 }]);
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

/** What is in a text box. */
function numberIn(testId: string): string {
  const field = screen.getByTestId(testId);
  if (!(field instanceof HTMLInputElement)) {
    throw new Error(`${testId} is not an input`);
  }
  return field.value;
}

describe("the cue viewer", () => {
  it("says what it is following when nothing is", async () => {
    await desk("CueViewer", null);
    expect(screen.getByTestId("cue-viewer-empty").textContent).toContain("No cue list is in force");
  });

  it("shows every value of every cue, with the preset link visible", async () => {
    const { applyStep } = await desk("CueViewer");
    await applyStep(
      ...A_CUE_LIST,
      "clear the programmer",
      "select the PARs again",
      "dial a blue",
      "and a dimmer value on a PAR's white",
      "store it, with a name and a scribble-strip colour",
      "clear again",
      "select the PARs",
      "and apply the preset",
      "store a cue out of it",
    );
    // The cue stored from an applied preset carries the link on every part —
    // and the viewer shows the preset's *name*, which is what makes the link
    // something an operator can act on.
    const link = screen.getByTestId("link-3-1-Blue");
    expect(link.textContent).toBe("1 Deep blue");
    expect(screen.getByTestId("part-1-1-Red")).toBeTruthy();
    // A value nobody linked reads as a dash rather than as preset zero.
    expect(screen.getByTestId("link-1-1-Red").textContent).toBe("—");
  });

  it("says a cue list holds no values rather than drawing an empty table", async () => {
    const { applyStep } = await desk("CueViewer");
    await applyStep("make a cue list to store into", "put it on an executor");
    expect(screen.getByTestId("cue-viewer-empty").textContent).toContain("holds no values yet");
  });

  it("names a link to a preset that is not there rather than drawing nothing", async () => {
    // A dangling link is a real state — `Show::remove_preset` leaves the value
    // and the link, and `Show::issues` reports it — so the viewer says which
    // preset is missing rather than hiding the row an operator has to fix.
    const show: JsonValue = {
      sequences: {
        "1": {
          name: "Act 1",
          loop: false,
          cues: [
            {
              number: "1",
              name: "",
              fadeIn: 0,
              fadeOut: 0,
              delay: 0,
              trigger: "Go",
              triggerTime: null,
              parts: [{ fixture: 1, attribute: "Red", value: 65535, presetRef: 44 }],
            },
          ],
        },
      },
      executors: { "0": { sequenceId: 1, isActive: false, currentCueIndex: null } },
    };
    const { applyStep } = await desk("CueViewer");
    await applyStep();
    await act(async () => {
      // The whole show replaced in one operation, which is what a `ShowPatch`
      // rooted at `/` is allowed to be.
      await Promise.resolve();
    });
    // Rendered directly rather than through a delta: what is being checked is
    // the reading, and the mirror is `mirror.test.ts`'s ground.
    const { unmount } = render(
      <DeskProvider store={new DeskStore()}>
        <CueViewer show={show} session={{ session: { selectedExecutor: 0 } }} />
      </DeskProvider>,
    );
    expect(screen.getByTestId("link-1-1-Red").textContent).toBe("44 (missing)");
    unmount();
  });
});
