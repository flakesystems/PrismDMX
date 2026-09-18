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

/** The deltas of the recorded step about something. */
function recordedDeltas(about: string): Delta[] {
  const entry = recording.steps.find((step) => step.what.includes(about));
  if (entry === undefined) {
    throw new Error(`the recording has no step about ${JSON.stringify(about)}`);
  }
  return entry.deltas.map((encoded) => {
    const message = readServerMessage(decode(payload(encoded)));
    if (message.t !== "Delta") {
      throw new Error("that payload is not a delta");
    }
    return message.delta;
  });
}

/**
 * The daemon's answer at the recorded step **about** something.
 *
 * By description rather than by number: the script grows, and an index written
 * down here would quietly start pointing at another step.
 */
function recordedAnswer(about: string): Answer {
  const encoded = recording.steps.find((step) => step.what.includes(about))?.answer;
  if (encoded === null || encoded === undefined) {
    throw new Error(`no recorded step about ${JSON.stringify(about)} carries an answer`);
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

  /** Answers the question sent with `seq`, whichever it was — S57. */
  const answerSeq = async (seq: number, answer: Answer): Promise<void> => {
    await act(async () => {
      network.last.deliver(serverMessage({ t: "Answer", seq, answer }));
      await Promise.resolve();
    });
  };

  /** The daemon answers with the deltas of the step about something. */
  const applyStep = async (about: string): Promise<void> => {
    await act(async () => {
      for (const delta of recordedDeltas(about)) {
        network.last.deliver(serverMessage({ t: "Delta", delta }));
      }
      await Promise.resolve();
    });
  };

  return { view, sent, commands, queries, answerQuery, answerSeq, applyStep };
}

/** The questions of one kind the interface asked, in order. */
function asked(
  messages: readonly Sent[],
  kind: string,
): { readonly seq: number; readonly query: { readonly t: string } }[] {
  const found: { readonly seq: number; readonly query: { readonly t: string } }[] = [];
  for (const message of messages) {
    if (message.t === "Query" && message.query.t === kind) {
      found.push(message);
    }
  }
  return found;
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
    await answerQuery("PatchConflicts", recordedAnswer("now the show has an overlap"));
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
    await answerQuery("PatchPreview", recordedAnswer("would a PAR fit at address 30"));
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
    await answerQuery("PatchPreview", recordedAnswer("would a second PAR at 32 clash"));

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
    await answerQuery("PatchPreview", recordedAnswer("would it fit at 510"));

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
      // **S43**: the row carries whether the desk supplies this fixture's
      // intensity, like every other thing the row *is*. A dimmer has one of its
      // own, so the flag says nothing here — and it still travels, because the
      // form sends the whole row.
      softwareDimmer: true,
    });
    // **And the table has not moved.** There is nowhere for the answer to be
    // kept: the rows are `patchRows(show)` and the draft was dropped when it
    // was submitted.
    expect(screen.getByTestId("patch-row-1").textContent).toContain("Fixture 1");
    expect(screen.getByTestId("patch-row-1").textContent).not.toContain("Front left");
    expect(screen.queryByTestId("patch-form")).toBeNull();

    // Step 2 of the recording is a patch, so its deltas are a `ShowPatch` this
    // mirror can follow — and then the row is what the daemon says it is.
    await applyStep("patch it there");
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
        softwareDimmer: true,
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

  /**
   * **The desk-supplied intensity, and the switch for it** — S43.
   *
   * Drawn only for a profile that has no dimmer of its own, because that is the
   * only fixture it does anything to: a checkbox on a row with a real dimmer
   * would read as an offer to take that dimmer away.
   *
   * Fixture 5 is the RGBW PAR and fixture 1 is a dimmer, so the same form says
   * two different things depending on which row is open.
   */
  it("offers the desk's dimmer only to a fixture whose profile has none", async () => {
    const { commands } = await desk();

    fireEvent.click(screen.getByTestId("patch-row-1"));
    expect(screen.queryByTestId("draft-software-dimmer")).toBeNull();

    fireEvent.click(screen.getByTestId("patch-row-5"));
    const check = screen.getByTestId("draft-software-dimmer") as HTMLInputElement;
    expect(check.checked).toBe(true);

    // Switched off and applied: the flag travels with the rest of the row, and
    // the daemon is what decides what happens to it.
    fireEvent.click(check);
    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().at(-1)).toMatchObject({
      t: "PatchFixture",
      id: 5,
      softwareDimmer: false,
    });
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

  /**
   * **S57, punch-list B60 (GitHub #28) — the owner's eight points**, each as an
   * operator meets it. The library answers are the daemon's own, out of the
   * recording, wherever the recording has one.
   */
  it("opens the library and the fixture's settings together, from Add fixture", async () => {
    // The fourth point: *Add fixture* is the library, not a form with a key
    // that opens the library. And nothing is sent for opening it (§4.2).
    const { commands, queries } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByText("Add fixture"));
    expect(screen.getByTestId("patch-editor")).not.toBeNull();
    expect(screen.getByTestId("library-search")).not.toBeNull();
    expect(screen.getByTestId("patch-form")).not.toBeNull();
    expect(queries().some((query) => query.t === "BrowseLibrary")).toBe(true);
    expect(screen.getByTestId("draft-type").textContent).toContain("Choose a fixture");
    // 1, 2 and 5 are patched, so 3 is the first gap.
    expect((screen.getByTestId("draft-id") as HTMLInputElement).value).toBe("3");
    expect(screen.getByTestId("draft-apply").hasAttribute("disabled")).toBe(true);
    expect(commands()).toHaveLength(before);
  });

  it("lists a fixture once, with its modes, and the whole row picks it", async () => {
    // The first and second points. The recorded page is the Robe wash, whose
    // two modes used to be two rows.
    const { commands, queries, sent, answerQuery } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByText("Add fixture"));
    await answerQuery("BrowseLibrary", recordedAnswer("the page after it"));
    const row = screen.getByTestId("library-row-robe/wash-7q5/4ch");
    const cells = [...row.querySelectorAll("td")].map((cell) => cell.textContent);
    expect(cells).toEqual(["Robe", "Wash 7Q5", "4ch · 2ch", "library"]);
    expect(screen.queryByTestId("library-row-robe/wash-7q5/2ch")).toBeNull();

    // **Not the first cell** — the modes cell, which used to do nothing.
    const modes = row.querySelectorAll("td")[2];
    if (modes === undefined) {
      throw new Error("the row has no modes cell");
    }
    fireEvent.click(modes);
    expect(screen.getByTestId("draft-type").textContent).toBe("Robe Wash 7Q5");
    const mode = screen.getByTestId("draft-mode") as HTMLSelectElement;
    expect([...mode.options].map((option) => option.textContent)).toEqual(["4ch", "2ch"]);
    expect(mode.value).toBe("robe/wash-7q5/4ch");
    // **Picking embeds nothing** — S57. It is a question now, asked of the
    // library's copy.
    expect(commands()).toHaveLength(before);
    expect(asked(sent(), "PatchPreview").at(-1)?.query).toEqual({
      t: "PatchPreview",
      id: 3,
      typeId: "robe/wash-7q5/4ch",
      universe: 1,
      address: 1,
      adding: 1,
    });
    expect(row.className).toContain("row-selected");

    // The mode is chosen beside it, and the question follows.
    fireEvent.change(mode, { target: { value: "robe/wash-7q5/2ch" } });
    expect(queries().filter((query) => query.t === "PatchPreview").length).toBeGreaterThanOrEqual(2);
  });

  it("asks for the next page when the list is scrolled to its end, and not before", async () => {
    // The third point. The recorded first page is one fixture of two.
    const { sent, answerQuery } = await desk();
    fireEvent.click(screen.getByText("Add fixture"));
    await answerQuery("BrowseLibrary", recordedAnswer("the first page of one"));
    expect(screen.getByTestId("library-count").textContent).toBe("2 of 6 fixtures");
    const browsed = () => asked(sent(), "BrowseLibrary").map((message) => message.query);

    const list = screen.getByTestId("library-scroll");
    Object.defineProperty(list, "clientHeight", { value: 200, configurable: true });
    Object.defineProperty(list, "scrollHeight", { value: 1000, configurable: true });
    // Half way down: nothing more is asked.
    list.scrollTop = 300;
    fireEvent.scroll(list);
    expect(browsed()).toHaveLength(1);
    // At the end: the next page, from where the list stops.
    list.scrollTop = 800;
    fireEvent.scroll(list);
    expect(browsed().at(-1)).toMatchObject({ t: "BrowseLibrary", text: "", offset: 1 });
    // Scrolling on while it is on its way asks nothing twice.
    fireEvent.scroll(list);
    expect(browsed()).toHaveLength(2);
    await answerQuery("BrowseLibrary", recordedAnswer("the page after it"));
    expect(screen.getAllByTestId(/^library-row-/)).toHaveLength(2);
    // Everything is here: there is nothing left to ask for.
    expect(screen.queryByTestId("library-more")).toBeNull();
  });

  it("starts a new fixture at the next free address, and follows it until one is typed", async () => {
    // The sixth point. The daemon's answer for a wash at 2.1 says the moving
    // head is in the way and the whole wash fits at 2.14.
    const { answerQuery, queries } = await desk();
    fireEvent.click(screen.getByText("Add fixture"));
    type("draft-universe", "2");
    await answerQuery("BrowseLibrary", recordedAnswer("the page after it"));
    fireEvent.click(screen.getByTestId("library-row-robe/wash-7q5/4ch"));
    await answerQuery("PatchPreview", recordedAnswer("on the moving head's channels"));
    expect((screen.getByTestId("draft-universe") as HTMLInputElement).value).toBe("2");
    expect((screen.getByTestId("draft-address") as HTMLInputElement).value).toBe("14");

    // Typed, the address is the operator's: an answer naming somewhere else
    // does not move it.
    type("draft-address", "1");
    const asked = queries().filter((query) => query.t === "PatchPreview").length;
    await answerQuery("PatchPreview", recordedAnswer("on the moving head's channels"));
    expect((screen.getByTestId("draft-address") as HTMLInputElement).value).toBe("1");
    expect(queries().filter((query) => query.t === "PatchPreview").length).toBe(asked);
  });

  it("names the next free address in an overlap, and moves there on one key", async () => {
    // The fifth point, on a fixture that is already patched.
    const { answerQuery, commands } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-5"));
    type("draft-address", "32");
    await answerQuery("PatchPreview", recordedAnswer("would a second PAR at 32 clash"));
    expect(screen.getByTestId("patch-preview").textContent).toContain("Next free: 1.34.");
    fireEvent.click(screen.getByTestId("draft-next-free"));
    expect((screen.getByTestId("draft-address") as HTMLInputElement).value).toBe("34");
    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().at(-1)).toMatchObject({ t: "PatchFixture", id: 5, address: 34 });
  });

  it("patches several of one fixture as one command, where the daemon placed them", async () => {
    // The eighth point: one gesture, one command, one Oops — and the profile
    // comes with it, so nothing was embedded on the way.
    const { answerQuery, commands } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByText("Add fixture"));
    await answerQuery("BrowseLibrary", recordedAnswer("the page after it"));
    fireEvent.click(screen.getByTestId("library-row-robe/wash-7q5/4ch"));
    fireEvent.change(screen.getByTestId("draft-mode"), { target: { value: "robe/wash-7q5/2ch" } });
    type("draft-count", "3");
    // **Not before the daemon has placed them**: three places are its answer.
    expect(screen.getByTestId("draft-apply").hasAttribute("disabled")).toBe(true);
    await answerQuery("PatchPreview", recordedAnswer("three new washes"));
    // The answer named 2.14 as the next free place, so the new row moved
    // there and asked again — and this is the daemon's answer to *that*.
    expect((screen.getByTestId("draft-address") as HTMLInputElement).value).toBe("14");
    await answerQuery("PatchPreview", recordedAnswer("three new washes"));
    expect(screen.getByTestId("patch-preview").textContent).toContain("3 fixtures, 20 to 22, at 2.14 to 2.18");
    // The seventh point, where the operator sees it: an empty name is the type's.
    expect((screen.getByTestId("draft-name") as HTMLInputElement).placeholder).toBe("Wash 7Q5");

    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().slice(before)).toEqual([
      {
        t: "PatchFixtures",
        typeId: "robe/wash-7q5/2ch",
        name: "",
        softwareDimmer: true,
        placements: [
          { id: 20, universe: 2, address: 14 },
          { id: 21, universe: 2, address: 16 },
          { id: 22, universe: 2, address: 18 },
        ],
      },
    ]);
    expect(screen.queryByTestId("patch-editor")).toBeNull();
  });

  it("will not send a count that is not a number, and says which field it is", async () => {
    const { answerQuery } = await desk();
    fireEvent.click(screen.getByText("Add fixture"));
    await answerQuery("BrowseLibrary", recordedAnswer("the page after it"));
    fireEvent.click(screen.getByTestId("library-row-robe/wash-7q5/4ch"));
    for (const rubbish of ["", "0", "two"]) {
      type("draft-count", rubbish);
      expect(screen.getByTestId("patch-preview").textContent).toContain("Count");
      expect(screen.getByTestId("draft-apply").hasAttribute("disabled")).toBe(true);
    }
  });

  it("cancels without sending anything and without changing the table", async () => {
    const { commands } = await desk();
    const before = commands().length;
    fireEvent.click(screen.getByTestId("patch-row-1"));
    type("draft-name", "never mind");
    type("draft-address", "300");
    fireEvent.click(screen.getByTestId("patch-editor-cancel"));
    expect(commands()).toHaveLength(before);
    expect(screen.queryByTestId("patch-form")).toBeNull();
    expect(tableIds()).toEqual([1, 2, 5]);
  });

  /**
   * **The picker says whose a fixture is** — punch-list entry **B43**, and a
   * column still, because what an operator does with it is scan down it.
   */
  it("marks a fixture as the venue's own, beside one that came with the desk", async () => {
    const { answerQuery } = await desk();
    fireEvent.click(screen.getByText("Add fixture"));
    await answerQuery("BrowseLibrary", {
      t: "LibraryFixtures",
      fixtures: [
        {
          manufacturer: "Robe",
          name: "Wash 7Q5",
          own: true,
          modes: [{ id: "robe/wash-7q5/4ch", mode: "4ch", footprint: 4, hasIntensity: true }],
        },
        {
          manufacturer: "Robe",
          name: "LEDBeam 150",
          own: false,
          modes: [{ id: "robe/ledbeam/1ch", mode: "1ch", footprint: 1, hasIntensity: true }],
        },
      ],
      matched: 2,
      total: 2,
    });
    const source = (id: string) => screen.getByTestId(`library-source-${id}`);
    expect(source("robe/wash-7q5/4ch").getAttribute("data-own")).toBe("yes");
    expect(source("robe/wash-7q5/4ch").textContent).toBe("yours");
    expect(source("robe/ledbeam/1ch").getAttribute("data-own")).toBe("no");
    expect(source("robe/ledbeam/1ch").textContent).toBe("library");
  });

  it("says so when the library has nothing matching, rather than showing everything", async () => {
    const { answerQuery } = await desk();
    fireEvent.click(screen.getByText("Add fixture"));
    fireEvent.change(screen.getByTestId("library-search"), { target: { value: "no such light" } });
    await answerQuery("BrowseLibrary", { t: "LibraryFixtures", fixtures: [], matched: 0, total: 6 });
    expect(screen.getByTestId("library-empty").textContent).toContain("Nothing in the library");
    expect(screen.queryByTestId("library-matches")).toBeNull();
  });

  it("drops the answer to a search that has been typed over", async () => {
    const { answerSeq, sent } = await desk();
    fireEvent.click(screen.getByText("Add fixture"));
    fireEvent.change(screen.getByTestId("library-search"), { target: { value: "robe" } });
    const searches = asked(sent(), "BrowseLibrary").map((message) => message.seq);
    expect(searches).toHaveLength(2);
    // The answer to the **first** search, the empty one, arrives last.
    await answerSeq(searches[1] ?? 0, recordedAnswer("the page after it"));
    await answerSeq(searches[0] ?? 0, recordedAnswer("the first page of one"));
    expect(screen.getAllByTestId(/^library-row-/).map((row) => row.getAttribute("data-testid"))).toEqual([
      "library-row-robe/wash-7q5/4ch",
    ]);
  });

  it("changes a patched fixture's profile, embedding it before the repatch stands on it", async () => {
    // A fixture that is already patched opens on its own fixture — asked of
    // the daemon, so its modes are there without a search — and a different
    // one picked out of the list is embedded first, because `PatchFixture`
    // patches from the show's copy.
    const { commands, queries, answerQuery } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    expect(queries().some((query) => query.t === "FixtureOfMode")).toBe(true);
    await answerQuery("FixtureOfMode", {
      t: "FixtureOfMode",
      fixture: {
        manufacturer: "Generic",
        name: "Dimmer",
        own: false,
        modes: [{ id: "generic.dimmer", mode: "", footprint: 1, hasIntensity: true }],
      },
    });
    expect(screen.getByTestId("draft-type").textContent).toBe("Generic Dimmer");
    const before = commands().length;
    await answerQuery("BrowseLibrary", recordedAnswer("the page after it"));
    fireEvent.click(screen.getByTestId("library-row-robe/wash-7q5/4ch"));
    type("draft-universe", "2");
    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().slice(before)).toEqual([
      { t: "EmbedFixtureType", typeId: "robe/wash-7q5/4ch" },
      {
        t: "PatchFixture",
        id: 1,
        name: "Fixture 1",
        typeId: "robe/wash-7q5/4ch",
        universe: 2,
        address: 1,
        softwareDimmer: true,
      },
    ]);
  });

  it("opens on a show with no profiles at all, because the panel is the library", async () => {
    const { commands, queries } = await desk({ fixtures: {}, fixtureTypes: {} });
    const add = screen.getByText("Add fixture");
    expect(add.hasAttribute("disabled")).toBe(false);
    fireEvent.click(add);
    expect(screen.queryByTestId("patch-form")).not.toBeNull();
    expect(queries().some((query) => query.t === "BrowseLibrary")).toBe(true);
    // Nothing has been sent: opening a row is local until Patch (§4.2).
    expect(commands()).toHaveLength(0);
  });

  /**
   * **Punch-list B53 (GitHub #21).** A number field used to refuse anything that
   * was not a number *as it was typed*, so the box could never be empty and the
   * first digit of a number could not be changed. The box takes what is typed;
   * **Apply** is what refuses.
   */
  it("lets a number be cleared and typed again, and refuses only at Apply", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    const address = screen.getByTestId("draft-address") as HTMLInputElement;
    const apply = screen.getByTestId("draft-apply");
    const before = commands().length;

    type("draft-address", "");
    expect(address.value).toBe("");
    expect(apply.hasAttribute("disabled")).toBe(true);
    expect(screen.getByTestId("patch-preview").textContent).toContain("Address");
    fireEvent.click(apply);
    expect(commands()).toHaveLength(before);

    type("draft-address", "3");
    expect(address.value).toBe("3");
    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().at(-1)).toMatchObject({ t: "PatchFixture", address: 3 });
  });

  it("shows what was typed when it is not a number, and will not send it", async () => {
    const { commands } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    const before = commands().length;
    for (const [field, label] of [
      ["draft-id", "Number"],
      ["draft-universe", "Universe"],
      ["draft-address", "Address"],
    ] as const) {
      for (const rubbish of ["  ", "-4", "1.5", "twelve"]) {
        type(field, rubbish);
        expect((screen.getByTestId(field) as HTMLInputElement).value).toBe(rubbish);
        expect(screen.getByTestId("draft-apply").hasAttribute("disabled")).toBe(true);
        expect(screen.getByTestId("patch-preview").textContent).toContain(label);
      }
      type(field, "1");
    }
    // Nothing went out for any of it: no preview asked about a number that is
    // not one, and no patch.
    fireEvent.click(screen.getByTestId("draft-apply"));
    expect(commands().slice(before)).toHaveLength(1);
  });
});
