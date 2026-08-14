/**
 * **S25's three exit criteria, in a browser, against a real daemon.**
 *
 * The unit tests make the same three claims in jsdom against a fake socket, and
 * they are the ones that run on every commit. This is the one that removes the
 * remaining doubt, and in particular it is the only place where the second
 * criterion is *observed* rather than reconstructed:
 *
 * 1. Opening, moving and closing a window issues a session command — checked by
 *    the layout surviving things that would erase local state.
 * 2. **A view switched at the X-Touch appears at once.** A real `prismd`, a
 *    real binding table, real MIDI bytes, and a real Chromium that is doing
 *    nothing at the moment they arrive.
 * 3. **The layout survives a restart of the interface**, checked by reloading
 *    the page while the daemon carries on knowing nothing about it.
 *
 * # Nothing here touches a device
 *
 * `CLAUDE.md`'s rule. The console is `--mock-surface`: a file the daemon reads
 * MIDI from, which the three layers of `prism-surface` cannot tell from a port.
 * The output is `--mock-output`. Nothing is plugged into anything.
 */

import { expect, test } from "@playwright/test";
import { join } from "node:path";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, pressConsole, startDaemon } from "./daemon.ts";

/** A port of this suite's own, so a daemon on 7373 is neither used nor disturbed. */
const PORT = 7383;

/** `Channel ▶`, note 49 — `docs/MCU_MAPPING.md` §2.1's "Fader banks" row. D8 makes it `SelectView`. */
const CHANNEL_RIGHT = 49;

/** F1, note 54 — §2.1's "Function" row. The default profile opens a Fixture Sheet. */
const F1 = 54;

let daemon: Daemon | null = null;
let keys = "";

test.beforeAll(() => {
  buildDaemon();
});

test.afterEach(async () => {
  await daemon?.kill();
  daemon = null;
});

test("windows live in the session: the canvas survives a reload the daemon never hears about", async ({
  page,
}) => {
  daemon = await startDaemon(PORT);
  const dataDir = daemon.dataDir;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  // Nothing is open, because a fresh show's view 1 is empty.
  await expect(page.getByTestId("canvas-empty")).toBeVisible();
  await expect(page.getByTestId("open-windows")).toHaveText("0");

  // 1. Open one window. That is an `OpenWindow` out and a `SessionPatch` back;
  //    the canvas draws what came back.
  await page.getByTestId("open-window").selectOption("DmxSheet");
  await expect(page.getByTestId("window-1")).toBeVisible();
  await expect(page.getByTestId("open-windows")).toHaveText("1");

  // 2. Drag it somewhere it plainly was not. The daemon opens every window at
  //    the same 0,0 — `OpenWindow` carries no geometry — so this is done
  //    before the second window is opened, or the pointer would land on the
  //    one in front of it. Any movement is visible in `style.left`.
  const title = page.getByTestId("title-window-1");
  const grab = await title.boundingBox();
  expect(grab).not.toBeNull();
  if (grab === null) {
    throw new Error("the title bar has no box");
  }
  await page.mouse.move(grab.x + 40, grab.y + 8);
  await page.mouse.down();
  // In steps, so the pacing in `canvas/drag.ts` is exercised the way a hand
  // exercises it rather than by one jump.
  for (let step = 1; step <= 8; step += 1) {
    await page.mouse.move(grab.x + 40 + step * 30, grab.y + 8 + step * 15);
    await page.waitForTimeout(20);
  }
  await page.mouse.up();

  // Where it ended up is the **daemon's** answer, so it is read back off the
  // element after the delta has arrived.
  const moved = await page.getByTestId("window-1").evaluate((element) => ({
    left: (element as HTMLElement).style.left,
    top: (element as HTMLElement).style.top,
  }));
  expect(moved.left).not.toBe("0%");
  expect(moved.top).not.toBe("0%");

  // 3. **The third criterion.** The page is reloaded — every scrap of state in
  //    this process is destroyed — and the daemon is not told anything: no
  //    restart, no save, nothing. The layout comes back because it was never
  //    here.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("window-1")).toBeVisible();
  const after = await page.getByTestId("window-1").evaluate((element) => ({
    left: (element as HTMLElement).style.left,
    top: (element as HTMLElement).style.top,
  }));
  expect(after).toEqual(moved);

  // 4. A second window, opened after the reload: the numbering is the
  //    daemon's, so it is 2 even though this page has never seen a window 1
  //    being made.
  await page.getByTestId("open-window").selectOption("Patch");
  await expect(page.getByTestId("window-2")).toBeVisible();
  await expect(page.getByTestId("open-windows")).toHaveText("2");

  // 5. And closing one is the same contract in reverse.
  await page.getByTestId("close-window-2").click();
  await expect(page.getByTestId("window-2")).toHaveCount(0);
  await expect(page.getByTestId("open-windows")).toHaveText("1");

  await daemon.kill();
  daemon = null;
  forget(dataDir);
});

test("**D11**: a view switched at the console appears in the browser, unasked", async ({ page }) => {
  const directory = join(process.cwd(), "test-results");
  keys = join(directory, `console-${String(Date.now())}.midi`);
  daemon = await startDaemon(PORT + 1, undefined, { mockSurface: keys });
  const dataDir = daemon.dataDir;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  // A layout, stored as view 2 — the second view, because `Channel ▶` means
  // *the next one* and a desk with one view has no next one.
  await page.getByTestId("open-window").selectOption("Groups");
  await expect(page.getByTestId("window-1")).toBeVisible();
  await page.getByTestId("new-view").click();
  await expect(page.getByTestId("view-2")).toBeVisible();

  // Then the canvas is changed, so that a view switch is visible rather than a
  // no-op: the window is closed and view 1 is active.
  await page.getByTestId("close-window-1").click();
  await expect(page.getByTestId("canvas-empty")).toBeVisible();
  await expect(page.getByTestId("view-1")).toHaveAttribute("data-active", "yes");

  // **The press.** Three bytes appended to a file, by a process that is not
  // this browser and not this daemon. Nothing in the page is touched.
  pressConsole(keys, CHANNEL_RIGHT);

  // And the interface follows. It was told; it did not ask.
  await expect(page.getByTestId("view-2")).toHaveAttribute("data-active", "yes");
  await expect(page.getByTestId("active-view")).toHaveText("2");
  await expect(page.getByTestId("window-1")).toBeVisible();
  await expect(page.getByTestId("window-1")).toHaveAttribute("data-window-type", "Groups");

  // The other half of the D11 gate: F1 opens a window, from the same table.
  pressConsole(keys, F1);
  await expect(page.getByTestId("window-2")).toBeVisible();
  await expect(page.getByTestId("window-2")).toHaveAttribute("data-window-type", "FixtureSheet");
  await expect(page.getByTestId("open-windows")).toHaveText("2");

  await daemon.kill();
  daemon = null;
  forget(dataDir);
});

test("the screen is a device screen: nothing outside the canvas scrolls", async ({ page }) => {
  // `CLAUDE.md`, checked rather than asserted in a stylesheet review: with
  // several windows open and a fixture sheet full of rows, the *page* must
  // still be exactly the height of the window.
  daemon = await startDaemon(PORT + 2);
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  for (const type of ["DmxSheet", "Patch", "Settings", "Groups"]) {
    await page.getByTestId("open-window").selectOption(type);
  }
  await expect(page.getByTestId("open-windows")).toHaveText("4");

  const overflow = await page.evaluate(() => ({
    pageWidth: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    pageHeight: document.documentElement.scrollHeight - document.documentElement.clientHeight,
    canvasWidth: (() => {
      const canvas = document.querySelector('[data-testid="canvas"]');
      return canvas === null ? 0 : canvas.scrollWidth - canvas.clientWidth;
    })(),
    canvasHeight: (() => {
      const canvas = document.querySelector('[data-testid="canvas"]');
      return canvas === null ? 0 : canvas.scrollHeight - canvas.clientHeight;
    })(),
  }));
  expect(overflow.pageWidth).toBe(0);
  expect(overflow.pageHeight).toBe(0);
  expect(overflow.canvasWidth).toBe(0);
  expect(overflow.canvasHeight).toBe(0);
});
