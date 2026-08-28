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

  /** The daemon answers with the deltas of the step about something. */
  const applyStep = async (about: string): Promise<void> => {
    await act(async () => {
      for (const delta of recordedDeltas(about)) {
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
      softwareDimmer: true,
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

  it("patches straight out of the desk's library, from the row being patched", async () => {
    // **A search and not a menu** — S44. The desk's library is the Open Fixture
    // Library, and neither a frame nor an operator can take two thousand
    // entries: what is typed goes to the daemon and what comes back is drawn.
    //
    // **S43, B19 moved it into the form.** It used to sit in the toolbar and do
    // one thing — embed — which an operator then had to follow with a second
    // gesture in a second place to actually use. One field does both now, and
    // the assertion is the pair: the show gets its copy *and* the row is set to
    // it, from one click.
    const { commands, queries, answerQuery } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    // **The library is a panel since B23**, so it is opened rather than focused:
    // a table with columns needs room, and the room is a modal over the canvas.
    fireEvent.click(screen.getByTestId("library-open"));
    const search = screen.getByTestId("library-search");
    fireEvent.change(search, { target: { value: "robe wash" } });
    expect(queries().some((query) => query.t === "SearchLibrary")).toBe(true);

    // The daemon's own answer to that very search, out of the recording.
    await answerQuery("SearchLibrary", recordedAnswer("search the desk's library"));
    // **Read off the row, column by column** — the make, the model, the mode
    // and the width are four cells now rather than one run-together line, which
    // is the whole of B23. Two modes of the one fixture come back and they are
    // told apart by the two columns an operator actually uses to tell them
    // apart.
    const row = screen.getByTestId("library-row-robe/wash-7q5/4ch");
    const cells = [...row.querySelectorAll("td")].map((cell) => cell.textContent);
    expect(cells.slice(0, 4)).toEqual(["Robe", "Wash 7Q5", "4ch", "4"]);
    expect(screen.getByTestId("library-row-robe/wash-7q5/2ch")).not.toBeNull();

    fireEvent.click(screen.getByTestId("library-robe/wash-7q5/4ch"));
    expect(commands().at(-1)).toEqual({
      t: "EmbedFixtureType",
      typeId: "robe/wash-7q5/4ch",
    });
    // The row is on it, and the preview has been asked about it: the profile is
    // in the show by the time the question arrives, because both went out on one
    // ordered channel.
    // The **key**, not the pretty label: what the field draws comes from the
    // show's own profiles, and the show has not answered yet. It reads as the
    // manufacturer and mode one delta later. A row that showed the label it had
    // just been clicked would be this client holding an opinion about the show
    // for the length of a round trip — the fault §4.2 names.
    expect(screen.getByTestId("draft-type").textContent).toContain("robe/wash-7q5/4ch");
    // And the panel closes on the pick rather than being dismissed afterwards:
    // the question it was asking has been answered, and a chooser left standing
    // over the form is a chooser an operator has to put away by hand.
    expect(screen.queryByTestId("library-modal")).toBeNull();
  });

  it("says so when the library has nothing matching, rather than showing everything", async () => {
    // The failure a search that ignored an unmatched word would produce, and
    // which an operator would read as *the library is broken*.
    const { answerQuery } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    fireEvent.click(screen.getByTestId("library-open"));
    await answerQuery("SearchLibrary", recordedAnswer("a search that matches nothing"));
    // The note replaces the table rather than sitting under an empty one: a
    // header row with nothing beneath it reads as *still loading*.
    expect(screen.getByTestId("library-empty").textContent).toContain("Nothing in the library");
    expect(screen.queryByTestId("library-matches")).toBeNull();
  });

  /**
   * **The assertion at the bottom of this test was itself the fault**, and the
   * owner found it — second attempt at B1.
   *
   * It used to say the embedding must not happen twice. That sounded like thrift
   * and was a trap: a show that embedded a profile before a library fix kept the
   * **stale copy for ever**, because picking that profile again sent nothing and
   * no other gesture in this interface could replace it. So after B1 gave colour
   * channels a home value of full, an existing rig went on reading its colours
   * at nought and re-patching changed nothing at all — which is exactly what was
   * reported, twice.
   *
   * Picking a profile out of the library means *use the library's copy of it*,
   * every time. The daemon refuses the replacement when it would break a patch
   * already standing on it (`ShowError::TypeChangeBreaksPatch`), and that is the
   * guard that makes it safe to do.
   */
  it("re-reads a profile the show already carries, rather than keeping the old copy", async () => {
    // Marked rather than hidden: an operator who had just added one would look
    // for it elsewhere. And not disabled — the second lamp of a rig is the
    // commonest patch there is.
    const { commands, answerQuery } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    const before = commands().length;
    fireEvent.click(screen.getByTestId("library-open"));
    await answerQuery("SearchLibrary", {
      t: "LibraryMatches",
      matches: [
        {
          id: "generic.dimmer",
          manufacturer: "Generic",
          name: "Dimmer",
          mode: "1ch",
          footprint: 1,
        },
      ],
      total: 4,
    });
    // The sentence is on the **row** now that the panel has columns — the
    // *In this show* one — rather than trailing off the end of the key.
    const row = screen.getByTestId("library-row-generic.dimmer");
    expect(row.textContent).toContain("picking re-reads it");
    const already = screen.getByTestId("library-generic.dimmer");
    expect(already.hasAttribute("disabled")).toBe(false);

    fireEvent.click(already);
    expect(screen.getByTestId("draft-type").textContent).toContain("Dimmer");
    // **The embed goes anyway**, and that is the whole of the fix: it is what
    // replaces a copy the show has been carrying since before a library change.
    expect(commands().slice(before)).toContainEqual({
      t: "EmbedFixtureType",
      typeId: "generic.dimmer",
    });
  });

  it("opens a row on a show with no profiles at all, because the field is the library", async () => {
    // **The claim this test used to make was the fault** — S43, B19. It said a
    // show with no profiles offers nothing to patch, and asserted the Add button
    // disabled and the form refusing to open: correct while the Type field was a
    // menu of what had been embedded, and a dead end for the one show that is
    // guaranteed to be in this state — a new one.
    //
    // The field is a search over the desk's whole library now, so an empty show
    // is an ordinary starting point: open a row, type, pick.
    const { commands, queries } = await desk({ fixtures: {}, fixtureTypes: {} });
    const add = screen.getByText("Add fixture");
    expect(add.hasAttribute("disabled")).toBe(false);
    fireEvent.click(add);
    expect(screen.queryByTestId("patch-form")).not.toBeNull();
    expect(screen.getByTestId("draft-type").textContent).toContain("No profile chosen");

    fireEvent.click(screen.getByTestId("library-open"));
    expect(queries().some((query) => query.t === "SearchLibrary")).toBe(true);
    // Nothing has been sent: opening a row is local until Apply (§4.2).
    expect(commands()).toHaveLength(0);
  });

  it("changes a fixture's profile and universe from the form", async () => {
    // The two fields a table of five columns is otherwise short of, and the
    // pair a preview has to be asked about again: a different profile is a
    // different footprint, and a different universe is a different set of
    // neighbours.
    const { commands, queries, answerQuery } = await desk();
    fireEvent.click(screen.getByTestId("patch-row-1"));
    const asked = queries().filter((query) => query.t === "PatchPreview").length;

    // The type is a search now rather than a menu (B19), so the profile is
    // chosen the way an operator chooses it: open the library and pick a row.
    fireEvent.click(screen.getByTestId("library-open"));
    await answerQuery("SearchLibrary", {
      t: "LibraryMatches",
      matches: [
        {
          id: "generic.rgbw.par",
          manufacturer: "Generic",
          name: "RGBW PAR",
          mode: "4ch",
          footprint: 4,
        },
      ],
      total: 4,
    });
    fireEvent.click(screen.getByTestId("library-generic.rgbw.par"));
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
      softwareDimmer: true,
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
