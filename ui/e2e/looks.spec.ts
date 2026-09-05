/**
 * **S28's four exit criteria, in a browser, against a real daemon.**
 *
 * The unit tests make the same claims in jsdom against a fake socket and they
 * are the ones that run on every commit. This is the one that removes the
 * remaining doubt, because it is the only place where a show is written the way
 * an operator writes one:
 *
 * 1. **Cues are stored, edited and fired from the interface** — starting from a
 *    rig with no sequence, no cue and no executor on it.
 * 2. **A store that would overwrite says what it will do first** — the counts
 *    are on the button, in the daemon's own words, with the command not sent.
 * 3. **Preset pools apply and store, and the links stay alive** — a preset is
 *    stored, applied, put into a cue, and then *edited*; the cue follows it, and
 *    the proof is the **light on the rig**, counted off the telemetry canvas.
 * 4. **Nothing about a cue, a sequence or a preset is held here** — every
 *    gesture is a command out and a `ShowPatch` back, and a `page.reload()`
 *    finds the show exactly as the daemon left it.
 *
 * S39 added a fifth, which is the answer to the question S28 raised and did not
 * answer: **the operator chooses the store mode**, the question carries the
 * choice, the sentence on the button is the daemon's answer about *that* choice,
 * and a cue can be loaded back into the programmer and put down again.
 *
 * # Nothing here touches a device
 *
 * `CLAUDE.md`'s rule. The output is `--mock-output` and there is no console in
 * this file at all.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, showFixture, startDaemon } from "./daemon.ts";

/** A port of this suite's own, so a daemon on 7373 is neither used nor disturbed. */
const PORT = 7397;

let daemon: Daemon | null = null;
let scratch: string | null = null;

test.beforeAll(() => {
  buildDaemon();
});

test.afterEach(async () => {
  await daemon?.kill();
  daemon = null;
  if (scratch !== null) {
    forget(scratch);
    scratch = null;
  }
});

/**
 * Opens the desk against a daemon holding the S28 rig.
 *
 * `show-rig.prism` is written by `crates/prismd/tests/ui_show.rs`: four fixtures
 * and **nothing stored**, so every sequence, cue and preset in this test is one
 * the browser made.
 */
async function desk(page: Page, port: number): Promise<void> {
  const fixture = showFixture("show-rig.prism");
  scratch = fixture.dataDir;
  daemon = await startDaemon(port, fixture.dataDir, { show: fixture.show });
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
}

/**
 * Empties the programmer: three presses of Clear, which is the three-stage
 * Clear `CLAUDE.md` asks for and `prism_core::Programmer` implements.
 */
async function clearProgrammer(page: Page): Promise<void> {
  for (let press = 0; press < 3; press += 1) {
    await command(page, "clear");
  }
}

/**
 * Closes every open window.
 *
 * The windows all open at 0, 0 at the same size (`OpenWindow` carries no
 * geometry — S25 decided that deliberately), so a second one on top of the first
 * is a window an operator cannot click through. Closing rather than counting
 * instance numbers, because the daemon picks those and a test that guessed them
 * would be asserting on the wrong thing.
 */
async function closeWindows(page: Page): Promise<void> {
  const closers = page.locator("[data-testid^='close-window-']");
  // **One close at a time, and each one is waited for** — which is D3 turning up
  // in the test suite. Closing a window is a command out and a `SessionPatch`
  // back: the canvas is the daemon's, so the key going down proves nothing about
  // the window going away.
  //
  // This used to take `count()` once and click `first()` that many times. On a
  // fast machine each delta arrived before the next click and it worked; on the
  // runner it did not, and the next click landed on a window that was already on
  // its way out — leaving the last one open and the assertion below reading 1.
  // `toHaveCount` is what waits, so the loop asks for the count to *drop* before
  // it clicks again.
  //
  // The bound is a guard against a close that never lands, so that a broken
  // build fails here with the count rather than hanging: fourteen window types
  // and nothing opens more than one of each.
  for (let guard = 0; guard < 20; guard += 1) {
    const open = await closers.count();
    if (open === 0) {
      break;
    }
    await closers.first().click();
    await expect(closers).toHaveCount(open - 1);
  }
  await expect(page.locator("[data-window-type]")).toHaveCount(0);
}

/** Types a line into the console command line and presses Enter. */
async function command(page: Page, line: string): Promise<void> {
  const input = page.getByTestId("command-input");
  await input.fill(line);
  await input.press("Enter");
}

/**
 * The same, answering the mode question if the line raises one.
 *
 * A store onto something that is already there asks which mode before it sends
 * anything (S39, S40) — which is the console's own behaviour and not something
 * to work around, so a test that stores twice says which answer it means.
 */
async function store(page: Page, line: string, mode: string): Promise<void> {
  await command(page, line);
  const prompt = page.getByTestId("command-prompt");
  if ((await prompt.count()) > 0) {
    await page.getByTestId(`prompt-${mode}`).click();
  }
}

/**
 * The colour `LevelPainter` draws a channel at 255 in.
 *
 * `colourOf(255)` — the top of the warm half of the ramp, worked out from
 * `src/telemetry/painter.ts` by hand rather than imported, because a test that
 * asked the painter what it drew would be asking the code under test.
 * `at 100` is level 65 535, which the engine encodes to the byte 255.
 */
const FULL = { red: 255, green: 235, blue: 150 };

/** The same for a channel at 127 — `at 50`, which is level 32 767. */
const HALF = { red: 64, green: 200, blue: 185 };

/** How many pixels of the telemetry canvas are exactly this colour. */
async function litPixels(
  page: Page,
  colour: { red: number; green: number; blue: number },
): Promise<number> {
  return page.getByTestId("telemetry-canvas").evaluate((element, wanted) => {
    if (!(element instanceof HTMLCanvasElement)) {
      return -1;
    }
    const context = element.getContext("2d");
    if (context === null) {
      return -1;
    }
    const image = context.getImageData(0, 0, element.width, element.height);
    let found = 0;
    for (let at = 0; at < image.data.length; at += 4) {
      if (
        image.data[at] === wanted.red &&
        image.data[at + 1] === wanted.green &&
        image.data[at + 2] === wanted.blue
      ) {
        found += 1;
      }
    }
    return found;
  }, colour);
}

test("a show is written, corrected and fired entirely from the interface", async ({ page }) => {
  await desk(page, PORT);
  await openWindow(page, "SequenceSheet");
  await expect(page.getByTestId("sequence-sheet")).toBeVisible();
  // **The cue work is the Cue Viewer's since S43** — the Sequence Sheet is the
  // pool and nothing else. Both windows, because this test walks the whole
  // gesture: choose a list on the left, edit its cues on the right.
  await openWindow(page, "CueViewer");
  await expect(page.getByTestId("cue-viewer")).toBeVisible();
  // **And the strip is a window too since S43.** Selecting an executor is its
  // gesture, so a test that selects one opens it. The daemon tiles the three
  // rather than stacking them (`prism_core::layout`).
  await openWindow(page, "Executors");

  // 1. The rig has no sequences and no executors, so the sheet says so rather
  //    than drawing an empty table — and it says *why* there is nothing in
  //    force, which is that no executor is selected.
  await expect(page.getByTestId("sequence-count")).toHaveText("0 sequences");
  await expect(page.getByTestId("looks-executor-name")).toHaveText("No executor selected");

  // Selecting an executor is the executor bar's gesture (S26) and the sheet
  // follows it — S28's marked assumption, and the reason there is no *selected
  // sequence* anywhere in this interface.
  await page.getByTestId("select-3").click();
  await expect(page.getByTestId("looks-executor-name")).toHaveText("Executor 3");

  // A cue list, made from the browser: two commands, and the second is what
  // makes it playable at all.
  // A cue list, made from the browser. **One key, one line** since S40
  // (`ARCHITECTURE_SPEC.md` §4.5): `Store Sequence 1` makes the list on a free
  // number and the daemon puts it in force, because the next `Store Cue 1`
  // names no list and means the selected one. Giving it a fader is a second
  // act, and it has a line of its own.
  await page.getByTestId("new-sequence").click();
  await expect(page.getByTestId("sequence-count")).toHaveText("1 sequences");
  await expect(page.getByTestId("no-cues")).toBeVisible();
  await command(page, "Assign Sequence 1 Executor 3");
  await expect(page.getByTestId("looks-executor-name")).toHaveText("Executor 3 · Sequence 1");

  // 2. A look in the programmer, and the Store button says what it will do
  //    **before** it is pressed: nothing is there yet, so this is a create.
  //    The rig is PARs, so the value goes on a colour rather than on a dimmer
  //    they have not got.
  await command(page, "1 thru 3 red at 100");
  await expect(page.getByTestId("store-cue")).toContainText("Nothing is there yet");
  await expect(page.getByTestId("store-cue")).toContainText("Merge");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-row-1")).toBeVisible();
  await expect(page.getByTestId("cue-viewer-count")).toContainText("3 values");

  // And now the sentence changes, because something *is* there: the same
  // gesture over the same number is an overwrite, and the counts say what it
  // costs. This is the exit criterion in one assertion.
  await page.getByTestId("store-number").fill("1");
  await expect(page.getByTestId("store-cue")).toContainText("3 replaced");
  await expect(page.getByTestId("store-cue")).not.toContainText("Nothing is there yet");

  // 3. A cue is edited in place, one field at a time, and the sheet redraws
  //    from what the daemon answers rather than from what was typed.
  await page.getByTestId("cue-name-1").click();
  await page.getByTestId("cue-name-1-input").fill("Opening");
  await page.getByTestId("cue-name-1-input").press("Enter");
  await expect(page.getByTestId("cue-name-1")).toHaveText("Opening");

  await page.getByTestId("cue-fadeIn-1").click();
  await page.getByTestId("cue-fadeIn-1-input").fill("4.5");
  await page.getByTestId("cue-fadeIn-1-input").press("Enter");
  await expect(page.getByTestId("cue-fadeIn-1")).toHaveText("4.5s");

  // A store into a second number, and then a renumber that **moves the cue up
  // the list** — the sort is the daemon's (`Cue::compare_numbers`) and this
  // interface never sorts anything.
  await page.getByTestId("store-number").fill("2");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-row-2")).toBeVisible();
  await page.getByTestId("cue-number-2").click();
  await page.getByTestId("cue-number-2-input").fill("0.5");
  await page.getByTestId("cue-number-2-input").press("Enter");
  await expect(page.getByTestId("cue-row-0.5")).toBeVisible();
  await expect(page.locator("[data-testid^='cue-row-']").first()).toHaveAttribute(
    "data-testid",
    "cue-row-0.5",
  );

  // 4. And it fires. Both halves of `Delta::ExecutorState` arrive now: S28 had
  //    to assert a **dash** here because nothing filled `currentCueIndex`, and
  //    S34's readback out of the tick fills it. The cue list is ordered `0.5`,
  //    `1`, so the first Go lands on cue 0.5 — which is the daemon's ordering
  //    and not this interface's.
  await expect(page.getByTestId("looks-executor-state")).toHaveText("stopped");
  await expect(page.getByTestId("looks-executor-cue")).toHaveText("cue —");
  await page.getByTestId("looks-go").click();
  await expect(page.getByTestId("looks-executor-state")).toHaveText("running");
  await expect(page.getByTestId("looks-executor-cue")).toHaveText("cue 1");
  await expect(page.getByTestId("cue-row-0.5")).toHaveAttribute("data-running", "yes");
  await page.getByTestId("looks-off").click();
  await expect(page.getByTestId("looks-executor-state")).toHaveText("stopped");
  await expect(page.getByTestId("looks-executor-cue")).toHaveText("cue —");

  // A cue is deleted, and the numbers that are left do not close up.
  await page.getByTestId("cue-delete-1").click();
  await expect(page.getByTestId("cue-row-1")).toHaveCount(0);
  await expect(page.getByTestId("cue-row-0.5")).toBeVisible();

  // **Nothing about any of this is held here.** The daemon has it all: a
  // reload finds the same cue list, because it was never here.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("sequence-sheet")).toBeVisible();
  await expect(page.getByTestId("cue-row-0.5")).toBeVisible();
  // The count is a `Status` reading, which S43 moved off the bottom bar and
  // into a window of its own.
  await openWindow(page, "Status");
  await expect(page.getByTestId("sequences")).toHaveText("1");
});

test("**the operator chooses the mode, and the daemon says what it will cost first**", async ({
  page,
}) => {
  // **S39, in a browser.** S28 shipped a Store button that said *Merge* because
  // that was the only mode the daemon had, and left two tests written to go red
  // the day the other two landed. This is the gesture those tests were about:
  // the operator picks, the question carries the pick, and the sentence on the
  // button is the daemon's answer about **that** pick.
  await desk(page, PORT);
  await openWindow(page, "SequenceSheet");
  await page.getByTestId("new-sequence").click();
  await expect(page.getByTestId("sequence-count")).toHaveText("1 sequences");
  // The store bar is the Cue Viewer's since S43 — the Sequence Sheet is the
  // pool. Which cue list it edits is `Session::selectedSequence`, and a new one
  // is put in force by the store that made it.
  await openWindow(page, "CueViewer");
  // And no executor was selected to get here, which is the other half of S39's
  // decision: a cue list is written before anybody decides which fader it is on.
  await expect(page.getByTestId("looks-executor-name")).toHaveText("No executor selected");

  // A look of five values, stored into cue 1.
  await command(page, "1 thru 3 red at 100");
  await command(page, "1 thru 2 green at 60");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-viewer-count")).toContainText("5 values");

  // A different, smaller look: one fixture, one attribute.
  await clearProgrammer(page);
  await command(page, "1 blue at 100");
  await page.getByTestId("store-number").fill("1");

  // Merge is the default and it says so: one value added, nothing lost.
  await expect(page.getByTestId("store-cue")).toContainText("Merge into cue 1");
  await expect(page.getByTestId("store-cue")).toContainText("1 added");
  await expect(page.getByTestId("store-cue")).not.toContainText("removed");

  // **Choosing Override asks the daemon again**, and the answer is a different
  // sentence about the same gesture: five values would go.
  await page.getByTestId("cue-store-mode").selectOption("Override");
  await expect(page.getByTestId("store-cue")).toContainText("Override into cue 1");
  await expect(page.getByTestId("store-cue")).toContainText("5 removed");

  // And a Remove writes nothing at all — refused here, because the blue is not
  // in the cue, and the refusal is the daemon's own words on the button.
  await page.getByTestId("cue-store-mode").selectOption("Remove");
  await expect(page.getByTestId("store-cue")).toContainText("nothing to remove");
  await expect(page.getByTestId("store-cue")).toBeDisabled();

  // Back to Override, and press it: the cue afterwards is what was promised.
  await page.getByTestId("cue-store-mode").selectOption("Override");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-viewer-count")).toContainText("1 values");

  // **And a cue is loaded back and put down again.** S39's `EditCue` and
  // `Update`: the values come back into the programmer, one is changed, and the
  // Update key puts the change into the cue without a number being typed.
  await expect(page.getByTestId("cue-edit-1")).toBeVisible();
  await page.getByTestId("cue-edit-1").click();
  await expect(page.getByTestId("update-cue")).toHaveText("Update cue 1");
  await expect(page.getByTestId("update-cue")).not.toHaveClass(/update-blinking/);
  await command(page, "1 white at 100");
  await expect(page.getByTestId("update-cue")).toHaveClass(/update-blinking/);
  await page.getByTestId("update-cue").click();
  await expect(page.getByTestId("cue-viewer-count")).toContainText("2 values");
  await expect(page.getByTestId("update-cue")).not.toHaveClass(/update-blinking/);

  // Clearing the programmer ends the edit, which is one of the three rules
  // `prism_core` asserts and the one an operator meets by accident.
  await clearProgrammer(page);
  await expect(page.getByTestId("update-cue")).toHaveCount(0);

  // Nothing of this is held here either.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("cue-viewer-count")).toContainText("2 values");
});

test("a preset link is alive: editing the preset changes the light a cue puts out", async ({
  page,
}) => {
  await desk(page, PORT + 1);

  // A colour in the programmer, stored as a preset. Which values go in is the
  // **daemon's** filter over the pool, not this interface's.
  await openWindow(page, "PresetPool");
  await expect(page.getByTestId("pool-empty")).toBeVisible();
  await command(page, "1 thru 3 red at 100");
  // **The store bar is gone** — S43's second rebuild: a preset is stored from
  // the command line, which is where the line was being built anyway. The
  // window's own job is to show what came of it.
  //
  // The pool is **named**, and that is S43's second change here. A line that
  // names none means `Session::encoderBank` (`Command::StorePreset`), which is
  // whichever bank the encoders are on and is not what this test is about; the
  // window is a tab per pool since B30, so the test says which tab it expects
  // the preset to appear in rather than depending on where the encoders happen
  // to be standing.
  // And **named**, because the assertion below is that an operator can read
  // which preset a cue follows off the cell. A line that names a pool and no
  // name leaves the preset without one (`joinName` takes what is after the pool
  // word), and `preset 1` on its own would be a weaker claim than the one this
  // test is here to make.
  await store(page, 'Store Preset 1 Color "Deep red"', "Merge");
  await expect(page.getByTestId("preset-1")).toBeVisible();

  // Apply it, so the programmer's values carry the **link**, and store a cue
  // out of that. A cue whose parts follow a preset is what makes "change the
  // red everywhere" one gesture rather than forty.
  await clearProgrammer(page);
  await command(page, "1 thru 3");
  await page.getByTestId("preset-1").click();
  // **And the intensity, which is the desk's since S43.** These PARs have no
  // dimmer channel of their own, so the desk supplies one that scales their
  // colour — and it rests at nought, which is what stops a rig of them coming
  // up white. Colour without intensity is no light, exactly as on a fixture with
  // a real dimmer, so an operator who wants to see something brings it up.
  // The preset is a **Colour** preset, so it carries the red and not this: the
  // cue stored below is what carries both.
  await command(page, "at 100");
  // One window at a time: they all open at 0, 0 at the same size, so a second
  // one on top of the first is a window an operator cannot click through.
  await closeWindows(page);

  await openWindow(page, "SequenceSheet");
  await openWindow(page, "Executors");
  // The store bar and the cue rows are the Cue Viewer's since B29.
  await openWindow(page, "CueViewer");
  await page.getByTestId("select-3").click();
  await page.getByTestId("new-sequence").click();
  await expect(page.getByTestId("sequence-count")).toHaveText("1 sequences");
  // The fader is its own line since S40 — see the note in the first test.
  await command(page, "Assign Sequence 1 Executor 3");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-row-1")).toBeVisible();
  await closeWindows(page);

  // **The Cue Viewer shows the link on the cell** — S43, B29. The window is a
  // cue per row and an attribute per column now, so a link is a marked cell
  // rather than a column of its own, and the preset's own name is in the title
  // beside the fixture it belongs to. What is asserted is unchanged: an
  // operator can see that this cue follows the preset.
  await openWindow(page, "CueViewer");
  const linked = page.getByTestId("cue-1-Red");
  await expect(linked).toHaveAttribute("data-linked", "yes");
  await expect(linked).toHaveAttribute("title", /preset 1 Deep red/);
  await closeWindows(page);

  // Fire the cue from the executor bar, so the preset's value is **on the rig**
  // — and count it on the telemetry canvas rather than in a readout, which is
  // S26's method. The programmer is cleared first, so what is on the cable is
  // the *cue's* doing and not the programmer's.
  await clearProgrammer(page);
  await openWindow(page, "DmxSheet");
  // The strip is a window since S43, and `closeWindows` above took it away with
  // the rest. The daemon tiles the two (`prism_core::layout`), so the picture of
  // the rig and the keys that fire it are both reachable.
  await openWindow(page, "Executors");
  await expect.poll(async () => litPixels(page, FULL), { timeout: 15_000 }).toBe(0);
  await page.getByTestId("button-3-0").click();
  await expect(page.getByTestId("select-3")).toHaveClass(/strip-running/);
  await expect.poll(async () => litPixels(page, FULL), { timeout: 15_000 }).toBeGreaterThan(0);
  // How much of the rig is at full with the preset's red at 100, which is what
  // the second Go is measured against below.
  const atFullBefore = await litPixels(page, FULL);

  // Stop it again, so what follows is about the *cue* and not about a fade in
  // flight — see the note below.
  await page.getByTestId("button-3-2").click();
  await expect.poll(async () => litPixels(page, FULL), { timeout: 15_000 }).toBe(0);

  // Now **edit the preset**, without the cue being touched or stored again.
  await openWindow(page, "PresetPool");
  await command(page, "1 thru 3 red at 50");
  // Storing over a preset that is there raises the console's own question —
  // which mode — and an Override is what *replace these three values* means.
  await store(page, 'Store Preset 1 Color "Deep red"', "Override");
  await clearProgrammer(page);

  // And the same Go now puts a **different level** on the rig, because the cue
  // followed the preset. Nothing between the two Gos touched the cue: that is
  // the whole of *preset links remain live*, seen in light rather than in a
  // document.
  //
  // The Go is needed, and that is a finding rather than a convenience: a
  // playback that is **already running** goes on outputting the look it faded
  // to, because `prismd::Core::rebuild` installs a new merge body and the
  // player keeps the levels it had reached. It is the safe behaviour for a show
  // in progress and it is not the behaviour a console has; `PROGRESS.md` §7
  // carries it for S34, which is the session that owns playback.
  await page.getByTestId("button-3-0").click();
  await expect(page.getByTestId("select-3")).toHaveClass(/strip-running/);
  await expect.poll(async () => litPixels(page, HALF), { timeout: 15_000 }).toBeGreaterThan(0);

  // **Fewer channels at full than before, not none** — and the difference is
  // punch-list B1 doing exactly what it was asked to.
  //
  // A colour rests **open**, so the green, blue and white of these PARs sit at
  // full whenever nothing drives them. This cue touches the red and nothing
  // else, so the other three go on resting open while the red halves — which is
  // *tracking*, and is the same thing a CMY head does when you pull one colour
  // and leave the rest. The old assertion here was `toBe(0)`, from a rig whose
  // colours rested at nought: it was reading *the rig went dark* as *the cue
  // changed*, and the two stopped being the same claim in S43.
  //
  // What the cue changed is therefore measured as a **drop**: red left the full
  // band for the half band, so the count at full falls and the count at half
  // rises. Both halves are asserted, because either alone would pass for a rig
  // that went dark.
  await expect
    .poll(async () => litPixels(page, FULL), { timeout: 15_000 })
    .toBeLessThan(atFullBefore);
  await expect.poll(async () => litPixels(page, FULL), { timeout: 15_000 }).toBeGreaterThan(0);
});

test("a cue sheet of four hundred rows scrolls inside its own window", async ({ page }) => {
  // `CLAUDE.md` forbids scrolling *outside* the canvas, and a long cue list is
  // exactly the case that breaks it: S27 checked the same thing with forty
  // fixtures, and this is the version with a cue list on the screen.
  await desk(page, PORT + 2);
  await openWindow(page, "SequenceSheet");
  await openWindow(page, "Executors");
  // The store bar and the rows are the Cue Viewer's since B29.
  await openWindow(page, "CueViewer");
  await page.getByTestId("select-3").click();
  await page.getByTestId("new-sequence").click();
  await expect(page.getByTestId("sequence-count")).toHaveText("1 sequences");
  await command(page, "1 thru 3 red at 100");
  for (let number = 1; number <= 30; number += 1) {
    await page.getByTestId("store-number").fill(String(number));
    await page.getByTestId("store-cue").click();
  }
  await expect(page.getByTestId("cue-row-30")).toBeVisible();

  const overflow = await page.evaluate(() => {
    const scroll = (element: Element | null) =>
      element === null
        ? [0, 0]
        : [element.scrollWidth - element.clientWidth, element.scrollHeight - element.clientHeight];
    return {
      page: scroll(document.documentElement),
      canvas: scroll(document.querySelector('[data-testid="canvas"]')),
      cues: scroll(document.querySelector('[data-testid="cue-viewer-scroll"]')),
    };
  });
  expect(overflow.page).toEqual([0, 0]);
  expect(overflow.canvas).toEqual([0, 0]);
  // And the sheet's own body **does** scroll, because a check that only
  // demanded zeros everywhere would pass for a sheet that had clipped its rows.
  expect(overflow.cues[1]).toBeGreaterThan(0);
});

/**
 * **S48, in a browser against a real daemon: what a cue asserts and what it
 * inherits.**
 *
 * The engine's `tests/cue_tracking.rs` asserts the *output* — the same cue
 * reached two ways puts out the same frames. This asserts the *reading*, and it
 * is the only place where the whole path runs: a cue stored from the programmer,
 * a `Query::CueTracking` on the wire, the daemon folding the list, and a cell
 * carrying a number no cue in it names.
 *
 * The last third is the edit. Blocking cue 2 writes what it inherits into it, so
 * the cell that was a reading becomes an assertion — and that is the whole of
 * what a blocking cue is.
 */
test("a cue sheet says what a cue asserts and what it inherits from the cues above", async ({
  page,
}) => {
  await desk(page, PORT);
  await openWindow(page, "SequenceSheet");
  await openWindow(page, "CueViewer");
  await openWindow(page, "Executors");
  await page.getByTestId("select-3").click();
  await page.getByTestId("new-sequence").click();
  await expect(page.getByTestId("sequence-count")).toHaveText("1 sequences");
  await command(page, "Assign Sequence 1 Executor 3");

  // Cue 1 sets red on three PARs. Cue 2 sets **green** and says nothing at all
  // about red — which is the shape the whole session is about.
  await command(page, "1 thru 3 red at 100");
  await page.getByTestId("store-number").fill("1");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-row-1")).toBeVisible();

  await clearProgrammer(page);
  await command(page, "1 thru 3 green at 50");
  await page.getByTestId("store-number").fill("2");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-row-2")).toBeVisible();

  // **The reading the session exists for.** Cue 2 does not set red, and the
  // cell says what the list holds it at anyway — in brackets and resting,
  // because it is a reading rather than this cue's assertion. Nothing in this
  // browser worked that out: it is the daemon's answer to `Query::CueTracking`.
  const inherited = page.getByTestId("cue-2-Red");
  await expect(inherited).toHaveAttribute("data-set", "no");
  await expect(inherited).toHaveAttribute("data-inherited", "yes");
  await expect(inherited).toHaveText("(100%)");

  // Cue 1 inherits nothing — there is nothing above it — so it blocks by
  // construction, and cue 2 does not.
  await expect(page.getByTestId("cue-tracking-1")).toHaveAttribute("data-blocks", "yes");
  await expect(page.getByTestId("cue-tracking-2")).toHaveAttribute("data-blocks", "no");

  // **The edit.** Blocking cue 2 writes the red it inherits into it, so nothing
  // above it reaches past it — and the cell that was a reading is now an
  // assertion carrying the same number.
  await page.getByTestId("cue-tracking-2").selectOption("Block");
  await expect(page.getByTestId("cue-2-Red")).toHaveAttribute("data-set", "yes");
  await expect(page.getByTestId("cue-2-Red")).toHaveText("100%");
  await expect(page.getByTestId("cue-tracking-2")).toHaveAttribute("data-blocks", "yes");

  // And none of it is held here: a reload finds the cue the daemon wrote.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("cue-2-Red")).toHaveAttribute("data-set", "yes");
});
