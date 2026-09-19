/**
 * **Punch-list B52, in a browser, against a real daemon** — a knob follows the
 * channel that switches it.
 *
 * An ADJ Flat Par QA12 in its 8-channel mode: *Mode Select* (channel 7) decides
 * whether channel 6 is a strobe, a program speed or a sound sensitivity, and
 * whether channel 8 is nothing, a colour macro or a program. The knob names
 * follow **what is on the cable**: the daemon reads the output it sends, so the
 * test drives *Mode Select* from the command line and reads the encoder's name.
 *
 * The fixture comes out of the library the machine installed, so the test says
 * why it is skipping on a machine without one.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, startDaemon } from "./daemon.ts";

/** A port of this suite's own. */
const PORT = 7401;

let daemon: Daemon | null = null;

test.beforeAll(() => {
  buildDaemon();
});

test.afterEach(async () => {
  await daemon?.kill();
  daemon = null;
});

/** Runs one line on the command line. */
async function line(page: Page, text: string): Promise<void> {
  await page.getByTestId("command-input").fill(text);
  await page.getByTestId("command-input").press("Enter");
}

test("a switched knob is named after the position its mode channel has on the wire", async ({
  page,
}) => {
  daemon = await startDaemon(PORT);
  const dataDir = daemon.dataDir;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();
  const count = page.getByTestId("library-count");
  await expect(count).toHaveText(/of \d+ fixtures/);
  const total = /of (\d+) fixtures/.exec((await count.textContent()) ?? "");
  if (total === null || Number(total[1]) <= 4) {
    test.skip(true, "no fixture library is installed - run tools/fetch-fixtures");
    return;
  }
  await page.getByTestId("library-search").fill("flat par qa12");
  await page.getByTestId("library-row-american-dj/flat-par-qa12/1ch").locator("td").nth(1).click();
  await page.getByTestId("draft-mode").selectOption("american-dj/flat-par-qa12/8ch");
  await expect(page.getByTestId("patch-preview")).toContainText("8 channels");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-1")).toBeVisible();

  await line(page, "Fixture 1");
  await page.getByTestId("bank-Control").click();
  const switched = page.getByTestId("encoder-Raw").locator(".encoder-name");
  const programs = page.getByTestId("encoder-Raw-2").locator(".encoder-name");

  // At home Mode Select is 0: dimmer mode, so channel 6 is the strobe and
  // channel 8 does nothing.
  await expect(switched).toHaveText("Strobe");
  await expect(programs).toHaveText("Unused");

  // Half way is a colour change program: the speed and the programs.
  await line(page, "1 control at 50");
  await expect(switched).toHaveText("Program Speed");
  await expect(programs).toHaveText("Color Change Programs");

  // Near the top is sound mode.
  await line(page, "1 control at 90");
  await expect(switched).toHaveText("Sound Sensitivity");
  await expect(programs).toHaveText("Sound Active Programs");

  // And back: the knob follows the wire both ways.
  await line(page, "1 control at 0");
  await expect(switched).toHaveText("Strobe");

  await daemon.kill();
  daemon = null;
  forget(dataDir);
});
