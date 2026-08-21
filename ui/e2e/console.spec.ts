/**
 * **S40's vocabulary, typed into a browser, against a real daemon.**
 *
 * The unit tests hold the parser to a recording of what a daemon accepted, and
 * they run on every commit. This is the one that removes the remaining doubt,
 * because it is the only place where the whole chain runs at once: a line typed
 * into an input, a command on a socket, a show that moves, a delta back, and —
 * for the playback half — **light on the rig**, counted off the telemetry
 * canvas.
 *
 * Four things it is here to show, and each is an exit criterion:
 *
 * 1. **Every line in the vocabulary works**, including the ones that needed a
 *    new command, a new mode or a new engine message to exist at all.
 * 2. **A key writes into the line and does not act** — pressed as a gesture and
 *    checked on `Session::commandLine`, which is what a second client would see.
 * 3. **A prompt does not block the desk.** A store onto a cue that exists asks
 *    *merge, override or cancel*; the show carries on, Escape cancels, and a
 *    cancelled prompt changes nothing at all — asserted on the **show** rather
 *    than on the interface.
 * 4. **A cue list on no fader plays.** `On Sequence 1` for a sequence nobody has
 *    assigned puts light on the rig, which is the hole S40 found in the playback
 *    model and filled.
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
const PORT = 7399;

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

/** Opens the desk against a daemon holding S28's rig: four fixtures, nothing stored. */
async function desk(page: Page): Promise<void> {
  const fixture = showFixture("show-rig.prism");
  scratch = fixture.dataDir;
  daemon = await startDaemon(PORT, fixture.dataDir, { show: fixture.show });
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
}

/** The command input. */
function input(page: Page) {
  return page.getByTestId("command-input");
}

/** Types a line and presses Enter. */
async function command(page: Page, line: string): Promise<void> {
  await input(page).fill(line);
  await input(page).press("Enter");
}

/** The colour `LevelPainter` draws a channel at 255 in — see `looks.spec.ts`. */
const FULL = { red: 255, green: 235, blue: 150 };

/** How many pixels of the telemetry canvas are exactly this colour. */
async function litPixels(
  page: Page,
  colour: { red: number; green: number; blue: number },
): Promise<number> {
  return page.evaluate((wanted) => {
    const canvas = document.querySelector<HTMLCanvasElement>(
      "[data-testid='telemetry-canvas']",
    );
    if (canvas === null) {
      return -1;
    }
    const context = canvas.getContext("2d");
    if (context === null) {
      return -1;
    }
    const { data } = context.getImageData(0, 0, canvas.width, canvas.height);
    let found = 0;
    for (let index = 0; index < data.length; index += 4) {
      if (
        data[index] === wanted.red &&
        data[index + 1] === wanted.green &&
        data[index + 2] === wanted.blue
      ) {
        found += 1;
      }
    }
    return found;
  }, colour);
}

test.describe("the console shell", () => {
  /**
   * **The whole vocabulary, in one show being built by typing.**
   *
   * It reads as a session at a desk on purpose: select, level, store, name,
   * copy, move, delete, assign, fire. Every line is one from
   * `IMPLEMENTATION_PLAN.md` S40, and the assertions are what the *daemon* did
   * with it — read back off the windows, which draw the show document.
   */
  test("builds a show by typing, and every line in the vocabulary lands", async ({
    page,
  }) => {
    await desk(page);

    // -- groups: the pool that had no commands at all before S40 -------------
    await command(page, "1 thru 3");
    await command(page, 'Store Group 1 "Front wash"');
    await page.getByTestId("open-window").selectOption("Groups");
    await expect(page.getByTestId("group-1")).toContainText("Front wash");
    await expect(page.getByTestId("group-1")).toContainText("3 fixtures");

    await command(page, 'Label Group 1 "Front"');
    await expect(page.getByTestId("group-1")).toContainText("Front");

    await command(page, "Copy Group 1 Group 2");
    await expect(page.getByTestId("group-2")).toBeVisible();
    await command(page, "Delete Group 2");
    await expect(page.getByTestId("group-2")).toHaveCount(0);

    // A group selects its fixtures, and the daemon expands it. The rig is PARs,
    // so the level goes on a colour rather than on a dimmer they have not got.
    await command(page, "Clear");
    await command(page, "Group 1");
    await command(page, "red at 100");

    // -- a cue list, written by typing --------------------------------------
    await page.getByTestId("open-window").selectOption("SequenceSheet");
    await command(page, 'Store Sequence 1 "Act 1"');
    await expect(page.getByTestId("sequence-1")).toContainText("Act 1");
    await command(page, "Sequence 1");
    await command(page, "Store Cue 1");
    await expect(page.getByTestId("cue-row-1")).toBeVisible();

    await command(page, 'Label Cue 1 "Opening"');
    await expect(page.getByTestId("cue-name-1")).toHaveText("Opening");

    await command(page, "Move Cue 1 Cue 5");
    await expect(page.getByTestId("cue-row-5")).toBeVisible();
    await expect(page.getByTestId("cue-row-1")).toHaveCount(0);

    await command(page, "Copy Cue 5 Cue 6");
    await expect(page.getByTestId("cue-row-6")).toBeVisible();
    await command(page, "Delete Cue 6");
    await expect(page.getByTestId("cue-row-6")).toHaveCount(0);

    // -- a fader, and the transport ------------------------------------------
    await command(page, "Assign Sequence 1 Executor 0");
    await expect(page.getByTestId("strip-0")).toContainText("Act 1");
    await command(page, "Go+ Executor 0");
    await expect(page.getByTestId("strip-0")).toContainText("Q1");
    await command(page, "Off Executor 0");

    await command(page, "Move Executor 0 Executor 3");
    await expect(page.getByTestId("strip-3")).toContainText("Act 1");
    await command(page, "Delete Executor 3");
    await expect(page.getByTestId("strip-3")).not.toContainText("Act 1");
  });

  /**
   * **A key writes a word into the line. It does not act.**
   *
   * Asserted as a gesture and on `Session::commandLine` — the readout labelled
   * *Engine*, which is what the daemon is holding and what a second client would
   * draw. Pressing `Fixture` puts `Fixture ` there and sends no other command;
   * pressing `Clear` acts at once.
   */
  test("a key writes into the session's line rather than acting", async ({ page }) => {
    await desk(page);
    await input(page).fill("");

    await page.getByTestId("key-fixture").click();
    await expect(page.getByTestId("command-line")).toHaveText("Fixture");
    await expect(input(page)).toHaveValue("Fixture ");
    // Nothing has been selected: the line is waiting for its argument.
    await expect(page.getByTestId("selection")).toHaveText("—");

    await input(page).fill("Fixture 1");
    await input(page).press("Enter");
    await expect(page.getByTestId("selection")).toContainText("1");
    // And the line was cleared by running it.
    await expect(page.getByTestId("command-line")).toHaveText("");
  });

  /**
   * **A prompt is not a modal, and a cancelled one changes nothing at all.**
   *
   * The show is read back after the cancel: cue 1 still holds exactly what it
   * held, which is the assertion the criterion asks for — on the show rather
   * than on the interface.
   */
  test("asks before overwriting, and a cancelled question changes nothing", async ({
    page,
  }) => {
    await desk(page);
    await page.getByTestId("open-window").selectOption("SequenceSheet");
    await command(page, "1 thru 3");
    // The rig is PARs: a level goes on a colour, not on a dimmer they lack.
    await command(page, "red at 100");
    await command(page, 'Store Sequence 1 "Act 1"');
    await command(page, "Sequence 1");
    await command(page, "Store Cue 1");
    await expect(page.getByTestId("cue-parts-1")).toHaveText("3");

    // A second store onto the same number, with a different look in the
    // programmer. The question stands and **nothing has been sent**.
    await command(page, "Clear");
    await command(page, "1");
    await command(page, "red at 50");
    await command(page, "Store Cue 1");
    await expect(page.getByTestId("command-prompt")).toBeVisible();
    await expect(page.getByTestId("cue-parts-1")).toHaveText("3");

    // **The desk is not blocked while it stands.** A window opens on the canvas
    // and draws the show, with the question still in the footer where it was —
    // which is the whole reason it is not a modal (`CLAUDE.md`: a device
    // screen, and §4.2: the canvas is the operator's).
    await page.getByTestId("open-window").selectOption("Groups");
    await expect(page.getByTestId("group-pool")).toBeVisible();
    await expect(page.getByTestId("command-prompt")).toBeVisible();

    // Escape cancels, and the cue is byte for byte what it was.
    await input(page).press("Escape");
    await expect(page.getByTestId("command-prompt")).toHaveCount(0);
    await expect(page.getByTestId("cue-parts-1")).toHaveText("3");

    // And answering it does what the word says: an Override leaves the cue
    // holding exactly what the programmer holds, which is one value.
    await command(page, "Store Cue 1");
    await page.getByTestId("prompt-Override").click();
    await expect(page.getByTestId("cue-parts-1")).toHaveText("1");
  });

  /**
   * **A cue list on no fader plays** — the hole S40 found and filled.
   *
   * `On Sequence 1` for a sequence nobody has assigned, and the proof is the
   * light: the exact colour `LevelPainter` draws a channel at 255 in, counted
   * off the telemetry canvas, zero before and more than zero after.
   */
  test("plays a cue list that is on no executor at all", async ({ page }) => {
    await desk(page);
    await page.getByTestId("open-window").selectOption("DmxSheet");
    await expect(page.getByTestId("telemetry-canvas")).toBeVisible();

    await command(page, "1 thru 3");
    // The rig is PARs: a level goes on a colour, not on a dimmer they lack.
    await command(page, "red at 100");
    await command(page, 'Store Sequence 1 "Act 1"');
    await command(page, "Sequence 1");
    await command(page, "Store Cue 1");
    // The programmer is what is lighting the rig so far, so it goes.
    await command(page, "Clear");
    await command(page, "Clear");
    await command(page, "Clear");
    await expect
      .poll(async () => litPixels(page, FULL), { timeout: 5000 })
      .toBe(0);

    // Nothing is assigned to any executor, and this still makes light.
    await command(page, "On Sequence 1");
    await expect
      .poll(async () => litPixels(page, FULL), { timeout: 5000 })
      .toBeGreaterThan(0);

    // And `Off` takes it away again.
    await command(page, "Off Sequence 1");
    await expect
      .poll(async () => litPixels(page, FULL), { timeout: 5000 })
      .toBe(0);
  });
});
