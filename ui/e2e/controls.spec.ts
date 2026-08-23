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
 * 3. **The reserved control cannot be bound**, and the row says why.
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
  await page.getByTestId("open-window").selectOption("Settings");
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

  // §4.1 out of the box, read off the window rather than assumed.
  await expect(page.getByTestId("control-does-Global.F1")).toHaveText("open FixtureSheet");
  await expect(page.getByTestId("control-does-Global.F5")).toHaveText("—");

  // The press before: F1 opens a Fixture Sheet.
  pressConsole(keys, F1);
  const sheet = page.locator('[data-window-type="FixtureSheet"]');
  await expect(sheet).toHaveCount(1);
  // And it is closed again, because it opens **on top of** the settings window:
  // a canvas is a canvas, and the window the console just opened has the focus.
  const sheetId = await sheet.getAttribute("data-testid");
  await page.getByTestId(`close-${String(sheetId)}`).click();
  await expect(sheet).toHaveCount(0);

  // One control, one command, no restart.
  await page.getByTestId("control-choose-Global.F5").click();
  await expect(page.getByTestId("control-editor-name")).toHaveText("Global.F5");
  await page.getByTestId("control-action").selectOption("Open window");
  await page.getByTestId("control-detail").selectOption("Patch");
  await page.getByTestId("control-apply").click();

  // The table moved, and the window is drawing what the daemon answered.
  await expect(page.getByTestId("control-does-Global.F5")).toHaveText("open Patch");

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

  await page.getByTestId("controls-learn").click();
  await expect(page.getByTestId("controls-learning")).toBeVisible();
  await expect(page.getByTestId("controls-learn")).toHaveText("Press a control…");

  const windowsBefore = await page.locator("[data-window-type]").count();
  pressConsole(keys, F1);

  // The editor is now in front of the key the operator pressed.
  await expect(page.getByTestId("control-editor-name")).toHaveText("Global.F1");
  // One shot: it disarms itself, so a client that went away cannot leave a desk
  // whose keys do nothing.
  await expect(page.getByTestId("controls-learning")).toHaveCount(0);
  await expect(page.getByTestId("controls-learn")).toHaveText("Learn");
  // And nothing happened on the canvas — F1 opens a Fixture Sheet when it is
  // obeyed, and it was not.
  expect(await page.locator("[data-window-type]").count()).toBe(windowsBefore);

  // The key still works afterwards, which is what says learn was a mode and not
  // a change.
  pressConsole(keys, F1);
  await expect(page.locator('[data-window-type="FixtureSheet"]')).toHaveCount(1);

  await done(dataDir);
});

/**
 * **Exit criterion 3.**
 *
 * Drawn rather than hidden, because an operator looking for SMPTE/Beats has to
 * find out *why* it is not theirs rather than conclude the list is incomplete.
 */
test("the reserved control is drawn, named and cannot be chosen", async ({ page }) => {
  const { dataDir } = await desk(page, PORT + 2);
  const reserved = page.getByTestId("control-choose-Global.SmpteBeats");
  await expect(reserved).toHaveText("Reserved");
  await expect(reserved).toBeDisabled();
  await expect(page.getByTestId("control-does-Global.SmpteBeats")).toHaveText("—");
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
  const { dataDir } = await desk(page, PORT + 3);
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

  // The first operator binds F5.
  await page.getByTestId("control-choose-Global.F5").click();
  await page.getByTestId("control-action").selectOption("Save show");
  await page.getByTestId("control-apply").click();
  await expect(page.getByTestId("control-does-Global.F5")).toHaveText("save the show");
  // The second tab is **told**: it asked for nothing.
  await expect(second.getByTestId("control-does-Global.F5")).toHaveText("save the show");

  // The second operator binds F6, and the first one's edit is still standing.
  await second.getByTestId("control-choose-Global.F6").click();
  await second.getByTestId("control-action").selectOption("Oops");
  await second.getByTestId("control-apply").click();
  await expect(second.getByTestId("control-does-Global.F6")).toHaveText("oops");
  await expect(page.getByTestId("control-does-Global.F6")).toHaveText("oops");
  await expect(page.getByTestId("control-does-Global.F5")).toHaveText("save the show");

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
 * The *inside* half is what a check of zeros everywhere would miss: the list has
 * seventy-three rows and must scroll **within** the window, while the document
 * and the canvas read zero on both axes.
 */
test("seventy-three rows scroll inside the window and nothing outside the canvas does", async ({
  page,
}) => {
  const { dataDir } = await desk(page, PORT + 4);
  // No resizing: seventy-three rows are taller than any window on a 1280 × 720
  // viewport, so the case this is about is the ordinary one rather than one a
  // drag has to produce.
  await expect(page.getByTestId("control-choose-Global.FootSwitch2")).toHaveCount(1);

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
