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
import { buildDaemon, forget, showFixture, startDaemon } from "./daemon.ts";

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
  for (let open = await closers.count(); open > 0; open -= 1) {
    await closers.first().click();
  }
  await expect(page.getByTestId("open-windows")).toHaveText("0");
}

/** Types a line into the console command line and presses Enter. */
async function command(page: Page, line: string): Promise<void> {
  const input = page.getByTestId("command-input");
  await input.fill(line);
  await input.press("Enter");
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
  await page.getByTestId("open-window").selectOption("SequenceSheet");
  await expect(page.getByTestId("sequence-sheet")).toBeVisible();

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
  await expect(page.getByTestId("cue-parts-1")).toHaveText("3");

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
  await page.getByTestId("open-window").selectOption("SequenceSheet");
  await page.getByTestId("new-sequence").click();
  await expect(page.getByTestId("sequence-count")).toHaveText("1 sequences");
  // And no executor was selected to get here, which is the other half of S39's
  // decision: a cue list is written before anybody decides which fader it is on.
  await expect(page.getByTestId("looks-executor-name")).toHaveText("No executor selected");

  // A look of five values, stored into cue 1.
  await command(page, "1 thru 3 red at 100");
  await command(page, "1 thru 2 green at 60");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-parts-1")).toHaveText("5");

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
  await expect(page.getByTestId("cue-parts-1")).toHaveText("1");

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
  await expect(page.getByTestId("cue-parts-1")).toHaveText("2");
  await expect(page.getByTestId("update-cue")).not.toHaveClass(/update-blinking/);

  // Clearing the programmer ends the edit, which is one of the three rules
  // `prism_core` asserts and the one an operator meets by accident.
  await clearProgrammer(page);
  await expect(page.getByTestId("update-cue")).toHaveCount(0);

  // Nothing of this is held here either.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("cue-parts-1")).toHaveText("2");
});

test("a preset link is alive: editing the preset changes the light a cue puts out", async ({
  page,
}) => {
  await desk(page, PORT + 1);

  // A colour in the programmer, stored as a preset. Which values go in is the
  // **daemon's** filter over the pool, not this interface's.
  await page.getByTestId("open-window").selectOption("PresetPool");
  await expect(page.getByTestId("pool-empty")).toBeVisible();
  await command(page, "1 thru 3 red at 100");
  await expect(page.getByTestId("store-preset")).toContainText("Nothing is there yet");
  await page.getByTestId("store-preset").click();
  await expect(page.getByTestId("preset-1")).toBeVisible();

  // Apply it, so the programmer's values carry the **link**, and store a cue
  // out of that. A cue whose parts follow a preset is what makes "change the
  // red everywhere" one gesture rather than forty.
  await clearProgrammer(page);
  await command(page, "1 thru 3");
  await page.getByTestId("preset-1").click();
  // One window at a time: they all open at 0, 0 at the same size, so a second
  // one on top of the first is a window an operator cannot click through.
  await closeWindows(page);

  await page.getByTestId("open-window").selectOption("SequenceSheet");
  await page.getByTestId("select-3").click();
  await page.getByTestId("new-sequence").click();
  // The fader is its own line since S40 — see the note in the first test.
  await command(page, "Assign Sequence 1 Executor 3");
  await page.getByTestId("store-cue").click();
  await expect(page.getByTestId("cue-row-1")).toBeVisible();
  await closeWindows(page);

  // The Cue Viewer shows the link, by the preset's own name.
  await page.getByTestId("open-window").selectOption("CueViewer");
  await expect(page.getByTestId("link-1-1-Red")).toHaveText("1 Color 1");
  await closeWindows(page);

  // Fire the cue from the executor bar, so the preset's value is **on the rig**
  // — and count it on the telemetry canvas rather than in a readout, which is
  // S26's method. The programmer is cleared first, so what is on the cable is
  // the *cue's* doing and not the programmer's.
  await clearProgrammer(page);
  await page.getByTestId("open-window").selectOption("DmxSheet");
  await expect.poll(async () => litPixels(page, FULL), { timeout: 15_000 }).toBe(0);
  await page.getByTestId("button-3-0").click();
  await expect(page.getByTestId("select-3")).toHaveClass(/strip-running/);
  await expect.poll(async () => litPixels(page, FULL), { timeout: 15_000 }).toBeGreaterThan(0);

  // Stop it again, so what follows is about the *cue* and not about a fade in
  // flight — see the note below.
  await page.getByTestId("button-3-2").click();
  await expect.poll(async () => litPixels(page, FULL), { timeout: 15_000 }).toBe(0);

  // Now **edit the preset**, without the cue being touched or stored again.
  await page.getByTestId("open-window").selectOption("PresetPool");
  await command(page, "1 thru 3 red at 50");
  await page.getByTestId("preset-number").fill("1");
  await expect(page.getByTestId("store-preset")).toContainText("replaced");
  await page.getByTestId("store-preset").click();
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
  await expect.poll(async () => litPixels(page, FULL), { timeout: 15_000 }).toBe(0);
});

test("a cue sheet of four hundred rows scrolls inside its own window", async ({ page }) => {
  // `CLAUDE.md` forbids scrolling *outside* the canvas, and a long cue list is
  // exactly the case that breaks it: S27 checked the same thing with forty
  // fixtures, and this is the version with a cue list on the screen.
  await desk(page, PORT + 2);
  await page.getByTestId("open-window").selectOption("SequenceSheet");
  await page.getByTestId("select-3").click();
  await page.getByTestId("new-sequence").click();
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
      cues: scroll(document.querySelector('[data-testid="cue-scroll"]')),
    };
  });
  expect(overflow.page).toEqual([0, 0]);
  expect(overflow.canvas).toEqual([0, 0]);
  // And the sheet's own body **does** scroll, because a check that only
  // demanded zeros everywhere would pass for a sheet that had clipped its rows.
  expect(overflow.cues[1]).toBeGreaterThan(0);
});
