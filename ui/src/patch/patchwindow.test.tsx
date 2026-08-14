/**
 * **The patch window, through the whole interface, against a daemon on a socket.**
 *
 * `patch.test.ts` holds the readers to the daemon's answers and
 * `preview.test.ts` holds the sentences to them. This drives `<App />` with a
 * daemon on the other end, so what is asserted is what an operator would do: a
 * gesture, the bytes that went out, and — the half that matters — **the table
 * not having changed** until the delta came back.
 *
 * The snapshot, the deltas and the answers are all a real `prismd`'s, out of
 * `ui/tests/fixtures/patch-recording.json`. The one thing this file invents is
 * the *window*: the recorded show was opened with a fresh session, so a Patch
 * window is put into it here. Which windows are open is S25's ground and is
 * asserted there.
 */

import { decode } from "@msgpack/msgpack";
import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import type { Answer, Command, Delta, JsonValue } from "../bindings";
import { Connection } from "../ipc/connection";
import { readServerMessage } from "../ipc/protocol";
import type { Snapshot } from "../ipc/protocol";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import { FakeNetwork, ManualTimer, serverMessage } from "../testing/fake-daemon";
import { TelemetryProvider } from "../telemetry/panel";

import recordingText from "../../tests/fixtures/patch-recording.json?raw";

interface Recording {
  readonly initialSnapshot: string;
  readonly steps: readonly {
    readonly what: string;
    readonly deltas: readonly string[];
    readonly answer: string | null;
  }[];
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

/** The recorded snapshot, with a Patch window open on the canvas. */
function recordedSnapshot(show?: JsonValue): Snapshot {
  const message = readServerMessage(decode(payload(recording.initialSnapshot)));
  if (message.t !== "Snapshot") {
    throw new Error("the recording does not start with a snapshot");
  }
  return {
    ...message.snapshot,
    show: show ?? message.snapshot.show,
    session: {
      session: {
        activeViewId: 1,
        openWindows: [
          { instanceId: 1, type: "Patch", x: 0, y: 0, w: 1920, h: 1080, params: {} },
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

/** The daemon's answer at one recorded step. */
function recordedAnswer(step: number): Answer {
  const encoded = recording.steps[step]?.answer;
  if (encoded === null || encoded === undefined) {
    throw new Error(`step ${String(step)} carries no answer`);
  }
  const message = readServerMessage(decode(payload(encoded)));
  if (message.t !== "Answer") {
    throw new Error("that payload is not an answer");
  }
  return message.answer;
}

/** One decoded thing the interface sent. */
type Sent =
  | { readonly t: "Command"; readonly seq: number; readonly command: Command }
  | { readonly t: "Query"; readonly seq: number; readonly query: { readonly t: string } };

/** A whole interface with a daemon the test drives. */
async function desk(show?: JsonValue) {
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
    network.last.deliver(serverMessage({ t: "Snapshot", snapshot: recordedSnapshot(show) }));
    await Promise.resolve();
  });

  /** Everything the interface has sent, decoded, in order. */
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

  /** Just the commands. */
  const commands = (): Command[] =>
    sent()
      .filter((message) => message.t === "Command")
      .map((message) => message.command);

  /** Just the questions. */
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

  /** The daemon answers with the deltas of one recorded step. */
  const applyStep = async (step: number): Promise<void> => {
    await act(async () => {
      for (const delta of recordedDeltas(step)) {
        network.last.deliver(serverMessage({ t: "Delta", delta }));
      }
      await Promise.resolve();
    });
  };

  return { view, sent, commands, queries, answerQuery, applyStep };
}

/** The fixture numbers the table is showing, in order. */
function tableIds(): number[] {
  return [...screen.getAllByTestId(/^patch-row-/)].map((row) =>
    Number((row.getAttribute("data-testid") ?? "").replace("patch-row-", "")),
  );
}

/** Types `value` into one of the form's fields. */
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

describe("the patch window", () => {
  it("shows the rig the daemon is holding, and asks it what overlaps", async () => {
    const { queries } = await desk();
    expect(tableIds()).toEqual([1, 2, 5]);
    expect(screen.getByTestId("patch-count").textContent).toBe("3 fixtures · 2 profiles");
    // The overlaps are the daemon's answer, asked for rather than worked out.
    expect(queries().some((query) => query.t === "PatchConflicts")).toBe(true);
  });

  it("paints nothing red when the daemon names a pair that is not on this sheet", async () => {
    // Step 5 of the recorded script: fixtures 6 and 7 share channels 32-33.
    // The rows here are 1, 2 and 5, so none of them is in it — which is the
    // half that says the marking follows the answer rather than lighting up.
    const { answerQuery } = await desk();
    await answerQuery("PatchConflicts", recordedAnswer(5));
    for (const id of [1, 2, 5]) {
      expect(screen.getByTestId(`patch-row-${String(id)}`).className).not.toContain(
        "row-conflict",
      );
    }
  });

  it("paints red both halves of every pair the daemon does name", async () => {
    const { answerQuery } = await desk();
    await answerQuery("PatchConflicts", {
      t: "PatchConflicts",
      conflicts: [{ universe: 1, from: 1, to: 1, first: 1, second: 2 }],
    });
    expect(screen.getByTestId("patch-row-1").className).toContain("row-conflict");
    expect(screen.getByTestId("patch-row-2").className).toContain("row-conflict");
    expect(screen.getByTestId("patch-row-5").className).not.toContain("row-conflict");
  });

  it("asks the daemon what a typed address would do, and says the answer", async () => {
    const { queries, answerQuery } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-5"));
    // Opening the row asks straight away, so the operator sees where the
    // fixture they are looking at actually sits.
    expect(queries().some((query) => query.t === "PatchPreview")).toBe(true);

    type("draft-address", "30");
    const asked = queries().filter((query) => query.t === "PatchPreview").length;
    expect(asked).toBeGreaterThanOrEqual(2);

    // The daemon's own answer for a PAR at 30: free, four channels, ending 33.
    await answerQuery("PatchPreview", recordedAnswer(1));
    expect(screen.getByTestId("patch-preview").textContent).toContain("Free");
    expect(screen.getByTestId("patch-preview").textContent).toContain("33");
    expect(screen.getByTestId("patch-preview").className).toContain("preview-clear");
  });

  it("shows an overlap before it is committed, and still lets it be committed", async () => {
    // The exit criterion, as an operator meets it. `prism_core::conflict`
    // reports an overlap and does not refuse it — cloning a fixture onto
    // another is a technique in daily use — so Apply stays live.
    const { answerQuery, commands } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-5"));
    type("draft-address", "32");
    await answerQuery("PatchPreview", recordedAnswer(3));

    const line = screen.getByTestId("patch-preview");
    expect(line.textContent).toContain("Overlaps 32–33 with fixture 6");
    expect(line.className).toContain("preview-overlap");
    const apply = screen.getByTestId("draft-apply");
    expect(apply.hasAttribute("disabled")).toBe(false);
    fireEvent.click(apply);
    expect(commands().at(-1)).toMatchObject({ t: "PatchFixture", address: 32 });
  });

  it("refuses to send what the daemon has already said it would refuse", async () => {
    const { answerQuery, commands } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByTestId("patch-row-5"));
    type("draft-address", "510");
    await answerQuery("PatchPreview", recordedAnswer(6));

    expect(screen.getByTestId("patch-preview").textContent).toContain("510");
    expect(screen.getByTestId("patch-preview").className).toContain("preview-refused");
    const apply = screen.getByTestId("draft-apply");
    expect(apply.hasAttribute("disabled")).toBe(true);
    fireEvent.click(apply);
    expect(commands()).toHaveLength(before);
  });

  it("sends the patch and **holds nothing** until the delta comes back", async () => {
    const { commands, applyStep } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    type("draft-name", "Front left");
    type("draft-address", "7");
    fireEvent.click(screen.getByTestId("draft-apply"));

    expect(commands().at(-1)).toEqual({
      t: "PatchFixture",
      id: 1,
      name: "Front left",
      typeId: "generic.dimmer",
      universe: 1,
      address: 7,
    });
    // **And the table has not moved.** There is nowhere for the answer to be
    // kept: the rows are `patchRows(show)` and the draft was dropped when it
    // was submitted.
    expect(screen.getByTestId("patch-row-1").textContent).toContain("Fixture 1");
    expect(screen.getByTestId("patch-row-1").textContent).not.toContain("Front left");
    expect(screen.queryByTestId("patch-form")).toBeNull();

    // Step 2 of the recording is a patch, so its deltas are a `ShowPatch` this
    // mirror can follow — and then the row is what the daemon says it is.
    await applyStep(2);
    expect(tableIds()).toEqual([1, 2, 5, 6]);
  });

  it("changes a number with one command, and the repatch after it", async () => {
    // A renumber is a remove and an insert, so doing it as an unpatch and a
    // patch would leave the rig without that fixture in between — which is why
    // `RenumberFixture` exists. It goes first, and the repatch follows.
    const { commands } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByTestId("patch-row-5"));
    type("draft-id", "70");
    type("draft-name", "PAR 70");
    fireEvent.click(screen.getByTestId("draft-apply"));

    expect(commands().slice(before)).toEqual([
      { t: "RenumberFixture", id: 5, to: 70 },
      {
        t: "PatchFixture",
        id: 70,
        name: "PAR 70",
        typeId: "generic.rgbw.par",
        universe: 1,
        address: 20,
      },
    ]);
  });

  it("does not renumber a fixture to the number it already has", async () => {
    const { commands } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByTestId("patch-row-2"));
    type("draft-name", "Two");
    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().slice(before)).toHaveLength(1);
    expect(commands().at(-1)).toMatchObject({ t: "PatchFixture", id: 2 });
  });

  it("unpatches by the number the row started at, not the one being typed", async () => {
    // The trap: an operator types a new number, changes their mind and presses
    // Unpatch. The fixture that exists is still the old number.
    const { commands } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-2"));
    type("draft-id", "44");
    fireEvent.click(screen.getByTestId("draft-remove"));
    expect(commands().at(-1)).toEqual({ t: "UnpatchFixture", id: 2 });
    expect(screen.queryByTestId("patch-form")).toBeNull();
  });

  it("adds a fixture at the first free number, with a profile the show carries", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByText("Add fixture"));
    // 1, 2 and 5 are patched, so 3 is the first gap — a convenience, and the
    // daemon still decides whether the number is free.
    expect((screen.getByTestId("draft-id") as HTMLInputElement).value).toBe("3");
    type("draft-name", "New one");
    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().at(-1)).toEqual({
      t: "PatchFixture",
      id: 3,
      name: "New one",
      typeId: "generic.dimmer",
      universe: 1,
      address: 1,
    });
  });

  it("cancels without sending anything and without changing the table", async () => {
    const { commands } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByTestId("patch-row-1"));
    type("draft-name", "never mind");
    type("draft-address", "300");
    fireEvent.click(screen.getByTestId("draft-cancel"));
    expect(commands()).toHaveLength(before);
    expect(screen.queryByTestId("patch-form")).toBeNull();
    expect(tableIds()).toEqual([1, 2, 5]);
  });

  it("offers the desk's profiles the show has not got, and embeds the one chosen", async () => {
    const { commands } = await desk();
    const menu = screen.getByTestId("patch-library");
    if (!(menu instanceof HTMLSelectElement)) {
      throw new Error("the library is a menu");
    }
    // The desk's library less what this show already carries: the recorded show
    // has the dimmer and the PAR, so what is offered is the rest.
    const offered = [...menu.options].map((option) => option.value).filter((value) => value !== "");
    expect(offered).toContain("generic.movinghead");
    expect(offered).not.toContain("generic.dimmer");

    fireEvent.change(menu, { target: { value: "generic.movinghead" } });
    expect(commands().at(-1)).toEqual({
      t: "EmbedFixtureType",
      typeId: "generic.movinghead",
    });
    // The menu snaps back to *choose…* rather than showing what was picked:
    // what the show carries is the daemon's answer, and it arrives as a delta.
    expect(menu.value).toBe("");
  });

  it("says plainly when a show has no profiles, and offers nothing to patch", async () => {
    // The state a brand-new show is in, and the reason `EmbedFixtureType` and
    // the desk's library exist at all: without them this window would be a form
    // with an empty menu and no way out of it.
    const { commands } = await desk({ fixtures: {}, fixtureTypes: {} });
    expect(screen.getByTestId("patch-no-profiles").textContent).toContain(
      "no fixture profiles",
    );
    const add = screen.getByText("Add fixture");
    expect(add.hasAttribute("disabled")).toBe(true);
    fireEvent.click(add);
    expect(screen.queryByTestId("patch-form")).toBeNull();
    expect(commands()).toHaveLength(0);
  });

  it("changes a fixture's profile and universe from the form", async () => {
    // The two fields a table of five columns is otherwise short of, and the
    // pair a preview has to be asked about again: a different profile is a
    // different footprint, and a different universe is a different set of
    // neighbours.
    const { commands, queries } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    const asked = queries().filter((query) => query.t === "PatchPreview").length;

    const type_ = screen.getByTestId("draft-type");
    if (!(type_ instanceof HTMLSelectElement)) {
      throw new Error("the type is a menu");
    }
    fireEvent.change(type_, { target: { value: "generic.rgbw.par" } });
    type("draft-universe", "2");
    expect(queries().filter((query) => query.t === "PatchPreview").length).toBeGreaterThan(asked);

    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().at(-1)).toEqual({
      t: "PatchFixture",
      id: 1,
      name: "Fixture 1",
      typeId: "generic.rgbw.par",
      universe: 2,
      address: 1,
    });
  });

  it("ignores a number typed as something that is not one", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    for (const rubbish of ["", "  ", "-4", "1.5", "twelve"]) {
      type("draft-address", rubbish);
      expect((screen.getByTestId("draft-address") as HTMLInputElement).value).toBe("1");
    }
    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().at(-1)).toMatchObject({ address: 1 });
  });
});
