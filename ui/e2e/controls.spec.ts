/**
 * **S38's exit criteria, in a browser, against a real daemon and a real
 * console.**
 *
 * The unit tests make the same claims in jsdom against a fake socket and they
 * are the ones that run on every commit. This is the one that removes the
 * remaining doubt, because it is the only place where the loop is closed: a key
 * is rebound in a window, and the *same three bytes* from a console the browser
 * has never heard of then do the new thing.
 *
 * 1. **A binding changed in the interface takes effect without restarting the
 *    daemon**, observed through `--mock-surface`.
 * 2. **Learn names the control that was pressed** — and does not fire it.
 * 3. **The reserved control cannot be bound** — not from the panel, and not by
 *    pressing it while learn is armed.
 * 4. **Two clients with the editor open do not produce two tables.**
 * 5. **The list scrolls inside its window**, and nothing outside the canvas
 *    scrolls.
 *
 * # Nothing here touches a device
 *
 * `CLAUDE.md`'s rule, and this machine has a real X-Touch attached. The console
 * is `--mock-surface`: a file the daemon reads MIDI from, which the three layers
 * of `prism-surface` cannot tell from a port. Every note number is written out
 * **by hand from `docs/MCU_MAPPING.md` §2.1** — asking the profile which note to
 * send would be asking the code under test what to press, and here the code
 * under test *is* the table a profile would have been asked.
 *
 * # The daemon is the one a venue runs
 *
 * No output flags and `--mock-devices`, which is S36's finding and S37's rule: a
 * daemon whose outputs came off its command line refuses every machine command,
 * and a binding is one.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { join } from "node:path";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, pressConsole, startDaemon } from "./daemon.ts";

/** A port of this suite's own, so a daemon on 7373 is neither used nor disturbed. */
const PORT = 7401;

/** F1, note 54 — §2.1's "Function" row: F1–F8 = 54–61. */
const F1 = 54;

/** F5, note 58 — the same row, four along. §4.1 leaves it free. */
const F5 = 58;

/** F6, note 59 — the next one along, also free. */
const F6 = 59;

/**
 * SMPTE/Beats, note 53 — §4.3's reserved key.
 *
 * Written out by hand from §2.1 like the rest: asking the profile which note to
 * send would be asking the code under test what to press.
 */
const SMPTE_BEATS = 53;

let daemon: Daemon | null = null;

test.beforeAll(() => {
  buildDaemon();
});

test.afterEach(async () => {
  await daemon?.kill();
  daemon = null;
});

/**
 * Opens the desk against a fresh daemon whose table is its own to edit, with a
 * console it reads from a file.
 */
async function desk(page: Page, port: number): Promise<{ dataDir: string; keys: string }> {
  const keys = join(process.cwd(), "test-results", `console-${String(Date.now())}-${String(port)}.midi`);
  daemon = await startDaemon(port, undefined, { configurable: true, mockSurface: keys });
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // **S43, B9**: the dropdown is gone. A window is opened from the chooser now,
  // which Insert opens — the keyboard route, because a console is operated in
  // the dark.
  await page.keyboard.press("Insert");
  await page.getByTestId("picker-Settings").click();
  await expect(page.getByTestId("settings")).toBeVisible();
  await page.getByTestId("settings-tab-controls").click();
  await expect(page.getByTestId("settings-controls")).toBeVisible();
  return { dataDir: daemon.dataDir, keys };
}

/**
 * **Exit criterion 1.**
 *
 * The claim is not *the table changed* — a readout would satisfy that for a
 * daemon that never told the desk. It is *the key does something else now*, so
 * it is asserted at both ends with the same three bytes.
 */
test("a key rebound in the window does the new thing at the console", async ({ page }) => {
  const { dataDir, keys } = await desk(page, PORT);
  // `McuProfile::name`, which says the emulation as well as the model — the
  // daemon's words rather than this file's.
  await expect(page.getByTestId("controls-device")).toHaveText("Behringer X-Touch (MC mode)");

  // §4.1 out of the box, read off the window rather than assumed — and read
  // along the axis S43's punch-list B3 turned the panel onto: the row is the
  // action, and the keys that reach it are listed in it.
  await expect(page.getByTestId("action-keys-Save show")).toContainText("Global.Save");
  await expect(page.getByTestId("action-keys-Clear programmer")).toContainText("Global.Record");
  // **F1 is a *custom* key** since the rebuild (B24): *open window* is fourteen
  // bindings rather than one action, so there the key is the row and carries
  // its own answer. That is where an operator finds it and changes it.
  await expect(page.getByTestId("custom-Global.F1")).toContainText("FixtureSheet");

  // The press before: F1 opens a Fixture Sheet.
  pressConsole(keys, F1);
  const sheet = page.locator('[data-window-type="FixtureSheet"]');
  await expect(sheet).toHaveCount(1);
  // And it is closed again, because it opens **on top of** the settings window:
  // a canvas is a canvas, and the window the console just opened has the focus.
  const sheetId = await sheet.getAttribute("data-testid");
  await page.getByTestId(`close-${String(sheetId)}`).click();
  await expect(sheet).toHaveCount(0);

  // One control, one command, no restart — and the gesture is the one B3 asked
  // for: say what you want, then press the key you want it on. The key is a
  // **real one**, appended to a file by a process that is neither this browser
  // nor this daemon.
  //
  // It goes through the **custom** section since B24, which is the `+` an
  // operator uses for a key of their own: choose the type, answer it, Learn.
  await page.getByTestId("custom-kind").selectOption("Open window");
  await page.getByTestId("custom-new-detail").selectOption("Patch");
  await page.getByTestId("custom-learn").click();
  await expect(page.getByTestId("controls-learning")).toBeVisible();
  pressConsole(keys, F5);

  // The table moved, and the window is drawing what the daemon answered — a
  // row of its own for the key that now has a job.
  await expect(page.getByTestId("custom-Global.F5")).toContainText("Patch");

  // **The press after.** Three bytes appended to a file by a process that is
  // neither this browser nor this daemon, on a key that did nothing a moment
  // ago.
  pressConsole(keys, F5);
  await expect(page.locator('[data-window-type="Patch"]')).toHaveCount(1);

  await done(dataDir);
});

/**
 * **Exit criterion 2** — S20's method rule run in the other direction.
 *
 * And the half that is worth the code: while learn is armed the control **does
 * not fire**. F1 opens a window out of the box, so a desk that fired it while
 * learning would leave one on the canvas.
 */
test("learn names the key that was pressed, and does not fire it", async ({ page }) => {
  const { dataDir, keys } = await desk(page, PORT + 1);

  await page.getByTestId("action-learn-Oops").click();
  await expect(page.getByTestId("controls-learning")).toBeVisible();
  await expect(page.getByTestId("action-learn-Oops")).toHaveText("Press a key…");

  const windowsBefore = await page.locator("[data-window-type]").count();
  pressConsole(keys, F1);

  // The key the operator pressed is now listed under the row that armed it —
  // which is the whole gesture, in one assertion: press Learn, press the key.
  await expect(page.getByTestId("action-keys-Oops")).toContainText("Global.F1");
  // One shot: it disarms itself, so a client that went away cannot leave a desk
  // whose keys do nothing.
  await expect(page.getByTestId("controls-learning")).toHaveCount(0);
  await expect(page.getByTestId("action-learn-Oops")).toHaveText("Learn");
  // And nothing happened on the canvas — F1 opened a Fixture Sheet when it was
  // obeyed, and it was not.
  expect(await page.locator("[data-window-type]").count()).toBe(windowsBefore);

  // The key works afterwards, which is what says learn was a mode and not a
  // failure — and it now does what it was *taught*, not what it did before.
  pressConsole(keys, F1);
  await expect(page.locator('[data-window-type="FixtureSheet"]')).toHaveCount(0);

  await done(dataDir);
});

/**
 * **Exit criterion 3**, re-aimed by B3.
 *
 * A reserved control has no row of its own to be greyed out in any more — the
 * rows are actions, and a key appears under whatever it is bound to. What must
 * still be true is that it cannot be bound *at all*, and the place that is now
 * visible is the desk: arm learn, press SMPTE/Beats, and nothing is written.
 */
test("the reserved control cannot be learned onto anything", async ({ page }) => {
  const { dataDir, keys } = await desk(page, PORT + 2);
  const revision = await page.getByTestId("controls-revision").innerText();
  // **What the row says before**, rather than a dash. The panel lists the keys
  // that reach an action including the ones the shipped profile puts there, so
  // Oops starts with the X-Touch's own Undo on it — which is B24's point: a
  // default an operator wants to change is a default they can see.
  const before = await page.getByTestId("action-keys-Oops").innerText();
  expect(before).toContain("Global.Undo");

  await page.getByTestId("action-learn-Oops").click();
  await expect(page.getByTestId("controls-learning")).toBeVisible();
  pressConsole(keys, SMPTE_BEATS);

  // It never reaches learn: §4.3 keeps it for the switch between hosts, so the
  // desk does not report it and the table does not move.
  await expect(page.getByTestId("action-keys-Oops")).not.toContainText("Global.SmpteBeats");
  // On substance rather than on layout: the chips are a list, so the text has
  // line breaks in it that `toHaveText` normalises and `innerText` does not.
  await expect(page.getByTestId("action-keys-Oops")).toContainText("Global.Undo");
  await expect(page.getByTestId("action-keys-Oops").locator("li")).toHaveCount(1);
  await expect(page.getByTestId("controls-revision")).toHaveText(revision);
  await done(dataDir);
});

/**
 * **Exit criterion 4**, watched from outside the process.
 *
 * Two tabs, two different keys, in the order two people would. Neither undoes
 * the other and both end up on the same revision — which is what *one table*
 * looks like from a browser.
 */
test("two editors change two keys and there is one table", async ({ page, context }) => {
  const { dataDir, keys } = await desk(page, PORT + 3);
  const url = daemon?.url ?? "";

  // A second tab. It has no window of its own to open, because which windows
  // are open is **session** state — the first tab's settings window is already
  // on its canvas.
  const second = await context.newPage();
  await second.goto(`/?daemon=${encodeURIComponent(url)}`);
  await expect(second.getByTestId("connection-status")).toHaveText("Connected");
  await second.getByTestId("settings-tab-controls").click();
  await expect(second.getByTestId("settings-controls")).toBeVisible();

  const revision = await page.getByTestId("controls-revision").innerText();
  await expect(second.getByTestId("controls-revision")).toHaveText(revision);

  // The first operator binds F5 to *Save show*, by pressing it.
  await page.getByTestId("action-learn-Save show").click();
  pressConsole(keys, F5);
  await expect(page.getByTestId("action-keys-Save show")).toContainText("Global.F5");
  // The second tab is **told**: it asked for nothing.
  await expect(second.getByTestId("action-keys-Save show")).toContainText("Global.F5");

  // The second operator binds F6, and the first one's edit is still standing.
  await second.getByTestId("action-learn-Oops").click();
  pressConsole(keys, F6);
  await expect(second.getByTestId("action-keys-Oops")).toContainText("Global.F6");
  await expect(page.getByTestId("action-keys-Oops")).toContainText("Global.F6");
  await expect(page.getByTestId("action-keys-Save show")).toContainText("Global.F5");

  // One table: the same revision on both screens.
  const after = await page.getByTestId("controls-revision").innerText();
  expect(after).not.toBe(revision);
  await expect(second.getByTestId("controls-revision")).toHaveText(after);

  await second.close();
  await done(dataDir);
});

/**
 * **Exit criterion 5**, and `CLAUDE.md`'s rule checked in a browser rather than
 * reviewed in a stylesheet.
 *
 * The *inside* half is what a check of zeros everywhere would miss: the list is
 * longer than any window and must scroll **within** it, while the document and
 * the canvas read zero on both axes.
 *
 * **S43 turned the panel round** (punch-list B3), so the list is twenty-four
 * actions rather than seventy-three controls — shorter, still longer than a
 * window, and the claim is unchanged. The row that is asserted to be drawn is
 * the last of the last group, because a list that stopped rendering halfway
 * would satisfy a scroll check and fail an operator.
 */
test("the whole action list scrolls inside the window and nothing outside the canvas does", async ({
  page,
}) => {
  const { dataDir } = await desk(page, PORT + 4);
  // No resizing: the list is taller than any window on a 1280 × 720 viewport,
  // so the case this is about is the ordinary one rather than one a drag has to
  // produce.
  await expect(page.getByTestId("action-Redo")).toHaveCount(1);

  // **The window's own body is the scroller**, which is S37's design and not
  // this panel's: one `overflow` per window, so a settings window never nests
  // two scrollbars. What is asserted is that seventy-three rows scroll *there*
  // and nowhere outside it.
  const overflow = await page.getByTestId("settings-body").evaluate((element) => ({
    scrollable: element.scrollHeight - element.clientHeight,
  }));
  expect(overflow.scrollable).toBeGreaterThan(0);

  const outside = await page.evaluate(() => {
    const doc = document.documentElement;
    const canvas = document.querySelector('[data-testid="canvas"]');
    return {
      docX: doc.scrollWidth - doc.clientWidth,
      docY: doc.scrollHeight - doc.clientHeight,
      canvasX: canvas === null ? 0 : canvas.scrollWidth - canvas.clientWidth,
      canvasY: canvas === null ? 0 : canvas.scrollHeight - canvas.clientHeight,
    };
  });
  expect(outside).toEqual({ docX: 0, docY: 0, canvasX: 0, canvasY: 0 });

  await done(dataDir);
});

/**
 * Stops the daemon and removes its directory, in that order.
 *
 * The order is not tidiness: the daemon holds an advisory lock on `prismd.guard`
 * for its whole life (§10.3), so removing the directory while it is running is
 * refused on Windows.
 */
async function done(dataDir: string): Promise<void> {
  await daemon?.kill();
  daemon = null;
  forget(dataDir);
}
