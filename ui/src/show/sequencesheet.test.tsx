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
 * `ui/tests/fixtures/show-recording.json`. The two things this file invents are
 * the *windows* and the *selected executor*: the recorded show was opened with a
 * fresh session, and which windows are open is S25's ground while the selection
 * is S26's, both asserted there.
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

/** The recorded snapshot, with one window open and one executor selected. */
function recordedSnapshot(window: WindowType, selectedExecutor: number | null): Snapshot {
  return {
    ...snapshotOf(showRecording.initialSnapshot),
    session: {
      session: {
        activeViewId: 1,
        openWindows: [{ instanceId: 1, type: window, x: 0, y: 0, w: 1920, h: 1080, params: {} }],
        focusedWindow: 1,
        executorPage: 0,
        selectedExecutor,
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
async function desk(window: WindowType = "SequenceSheet", selectedExecutor: number | null = 0) {
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
      serverMessage({ t: "Snapshot", snapshot: recordedSnapshot(window, selectedExecutor) }),
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

  return { view, sent, commands, queries, answerQuery, applyStep };
}

/** The show gets a cue list on executor 0, out of the recorded script. */
const A_CUE_LIST = [
  "make a cue list to store into",
  "put it on an executor",
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
    await desk("SequenceSheet", 0);
    expect(screen.getByTestId("sequence-count").textContent).toBe("0 sequences");
    // A fresh show has no sequences and the selected executor has none on it,
    // which is a state the sheet has to be legible in rather than blank.
    expect(screen.getByTestId("no-sequence")).toBeTruthy();
    expect(screen.getByTestId("looks-executor-name").textContent).toBe("Executor 0");
  });

  it("says that nothing is in force when no executor is selected", async () => {
    await desk("SequenceSheet", null);
    expect(screen.getByTestId("looks-executor-name").textContent).toBe("No executor selected");
    expect(screen.getByTestId("no-sequence").textContent).toContain("No executor is selected");
    // And the three keys are dead, because there is nothing to fire.
    for (const key of ["looks-go", "looks-back", "looks-off"]) {
      expect(screen.getByTestId(key).hasAttribute("disabled")).toBe(true);
    }
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

  it("creates a sequence and puts it on the selected executor, in that order", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByTestId("new-sequence"));
    // Two commands, and the order is the one that can succeed: a sequence has
    // to exist before an executor can be given it.
    expect(commands()).toEqual([
      { t: "CreateSequence", sequenceId: 1, name: "Sequence 1" },
      { t: "AssignExecutor", executorId: 0, sequenceId: 1 },
    ]);
    // And nothing on the screen moved: the sheet still says there is nothing.
    expect(screen.getByTestId("sequence-count").textContent).toBe("0 sequences");
  });

  it("creates a sequence and nothing else when no executor is selected", async () => {
    const { commands } = await desk("SequenceSheet", null);
    fireEvent.click(screen.getByTestId("new-sequence"));
    expect(commands()).toEqual([{ t: "CreateSequence", sequenceId: 1, name: "Sequence 1" }]);
  });

  it("chooses a sequence by putting it on the executor, not by remembering it", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("sequence-1"));
    // **The selected sequence is the selected executor's** — S28's marked
    // assumption, and it is why choosing one is a command rather than a click
    // this interface remembers.
    expect(commands()).toEqual([{ t: "AssignExecutor", executorId: 0, sequenceId: 1 }]);
  });

  it("edits one field of one cue, naming the cue by the number it started at", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-name-1"));
    type("cue-name-1-input", "Opening");
    fireEvent.keyDown(screen.getByTestId("cue-name-1-input"), { key: "Enter" });
    expect(commands()).toEqual([
      {
        t: "SetCueProperty",
        sequenceId: 1,
        cueNumber: "1",
        property: { t: "Name", name: "Opening" },
      },
    ]);
    // **And the cell still reads what the daemon said**, because the draft was
    // dropped rather than merged: what the cue is comes back as a `ShowPatch`.
    expect(screen.getByTestId("cue-name-1").textContent).toBe("—");
  });

  it("sends a renumber under the old number, which is the key the cue is filed by", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-number-2"));
    type("cue-number-2-input", "1.5");
    fireEvent.keyDown(screen.getByTestId("cue-number-2-input"), { key: "Enter" });
    expect(commands()).toEqual([
      {
        t: "SetCueProperty",
        sequenceId: 1,
        cueNumber: "2",
        property: { t: "Number", number: "1.5" },
      },
    ]);
    // The list has not reordered: it reorders when the daemon says so.
    expect(cueNumbers()).toEqual(["1", "2"]);
  });

  it("takes a fade time as a number and sends nothing at all for a blank one", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-fadeIn-1"));
    type("cue-fadeIn-1-input", "2.5");
    fireEvent.keyDown(screen.getByTestId("cue-fadeIn-1-input"), { key: "Enter" });
    expect(commands()).toEqual([
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
    expect(commands().at(-1)).toEqual({
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
    expect(commands()).toHaveLength(2);
  });

  it("abandons an edit on Escape and commits one on blur", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-name-1"));
    type("cue-name-1-input", "Gone");
    fireEvent.keyDown(screen.getByTestId("cue-name-1-input"), { key: "Escape" });
    expect(commands()).toEqual([]);
    expect(screen.getByTestId("cue-name-1")).toBeTruthy();

    fireEvent.click(screen.getByTestId("cue-delay-1"));
    type("cue-delay-1-input", "1");
    fireEvent.blur(screen.getByTestId("cue-delay-1-input"));
    expect(commands()).toEqual([
      {
        t: "SetCueProperty",
        sequenceId: 1,
        cueNumber: "1",
        property: { t: "Delay", seconds: 1 },
      },
    ]);
  });

  it("sends a trigger and its time together, because one means nothing without the other", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    const trigger = screen.getByTestId("cue-trigger-1");
    fireEvent.change(trigger, { target: { value: "Time" } });
    expect(commands()).toEqual([
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
    expect(commands().at(-1)).toEqual({
      t: "SetCueProperty",
      sequenceId: 1,
      cueNumber: "2",
      property: { t: "Trigger", trigger: "Time", triggerTime: 7.5 },
    });
  });

  it("deletes a cue by the number the row started at", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-delete-2"));
    expect(commands()).toEqual([{ t: "DeleteCue", sequenceId: 1, cueNumber: "2" }]);
    // Still two rows: the list is the daemon's.
    expect(cueNumbers()).toEqual(["1", "2"]);
  });

  it("fires the executor the sheet is following, and only the three keys the protocol has", async () => {
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("looks-go"));
    fireEvent.click(screen.getByTestId("looks-back"));
    fireEvent.click(screen.getByTestId("looks-off"));
    expect(commands()).toEqual([
      { t: "ExecutorGo", executorId: 0, direction: "Next" },
      { t: "ExecutorGo", executorId: 0, direction: "Prev" },
      { t: "ExecutorOff", executorId: 0 },
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
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    fireEvent.click(screen.getByTestId("cue-name-2"));
    expect(screen.getByTestId("cue-name-2-input")).toBeTruthy();
    await applyStep("renumber it to 1.5");
    expect(screen.queryByTestId("cue-name-2-input")).toBeNull();
    expect(commands()).toEqual([]);
  });

  it("shows what is running and draws the cue number as a dash", async () => {
    const { applyStep } = await desk();
    await applyStep(...A_CUE_LIST, "fire the list");
    expect(screen.getByTestId("looks-executor-state").textContent).toBe("running");
    // **The dash, and it is deliberate**: nothing fills `currentCueIndex` until
    // S34 builds the channel back from the tick, so the sheet says which
    // executor is running and not where it is.
    expect(screen.getByTestId("looks-executor-cue").textContent).toBe("cue —");
  });
});

describe("the store bar", () => {
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
    const { commands, applyStep } = await desk();
    await applyStep(...A_CUE_LIST);
    // Two cues, so the box offers 3.
    expect(numberIn("store-number")).toBe("3");
    type("store-number", "1");
    fireEvent.submit(screen.getByTestId("cue-store"));
    expect(commands()).toEqual([{ t: "StoreCue", sequenceId: 1, cueNumber: "1" }]);
    // **Dropped, not kept**: the box goes back to offering the next number of
    // whatever the daemon ends up holding.
    expect(numberIn("store-number")).toBe("3");
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
