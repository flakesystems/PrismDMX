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
import type { Page } from "@playwright/test";
import { join } from "node:path";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, pressConsole, startDaemon } from "./daemon.ts";

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

/**
 * Where window 1 is, once it has stopped moving.
 *
 * A drag is optimistic in `canvas/drag.ts` and authoritative only when the
 * daemon's `SessionPatch` arrives, so reading `style.left` the instant the mouse
 * comes up reads a value that is about to be corrected — which is how this file
 * once compared a position taken mid-flight against the one that survived a
 * reload, and failed by exactly one drag step. Two agreeing reads a frame apart
 * is *the delta has arrived* said in the only terms a browser has.
 */
async function settled(page: Page): Promise<{ left: string; top: string }> {
  const read = async (): Promise<{ left: string; top: string }> =>
    page.getByTestId("window-1").evaluate((element) => ({
      left: (element as HTMLElement).style.left,
      top: (element as HTMLElement).style.top,
    }));
  let previous = await read();
  for (let attempt = 0; attempt < 50; attempt += 1) {
    await page.waitForTimeout(100);
    const current = await read();
    if (current.left === previous.left && current.top === previous.top) {
      return current;
    }
    previous = current;
  }
  throw new Error(`window 1 never stopped moving: ${JSON.stringify(previous)}`);
}

test("windows live in the session: the canvas survives a reload the daemon never hears about", async ({
  page,
}) => {
  daemon = await startDaemon(PORT);
  const dataDir = daemon.dataDir;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  // Nothing is open, because a fresh show's view 1 is empty.
  await expect(page.getByTestId("canvas-empty")).toBeVisible();
  await expect(page.locator("[data-window-type]")).toHaveCount(0);

  // 1. Open one window. That is an `OpenWindow` out and a `SessionPatch` back;
  //    the canvas draws what came back.
  await openWindow(page, "DmxSheet");
  await expect(page.getByTestId("window-1")).toBeVisible();
  await expect(page.locator("[data-window-type]")).toHaveCount(1);

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

  // Where it ended up is the **daemon's** answer, so it has to be read after the
  // delta has arrived — and *waited for*, which this did not do until a CI run
  // caught it one drag step short. Placing a window is a command out and a
  // delta back (**D3 applies to the suite**, S43's rule from `closeWindows`), so
  // the value is read when it has stopped moving rather than the instant the
  // mouse comes up: the last step is still optimistic in `canvas/drag.ts` at
  // that moment, and the reload below compares against what the daemon kept.
  const moved = await settled(page);
  expect(moved.left).not.toBe("0%");
  expect(moved.top).not.toBe("0%");

  // 3. **The third criterion.** The page is reloaded — every scrap of state in
  //    this process is destroyed — and the daemon is not told anything: no
  //    restart, no save, nothing. The layout comes back because it was never
  //    here.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("window-1")).toBeVisible();
  const after = await settled(page);
  expect(after).toEqual(moved);

  // 4. A second window, opened after the reload: the numbering is the
  //    daemon's, so it is 2 even though this page has never seen a window 1
  //    being made.
  await openWindow(page, "Patch");
  await expect(page.getByTestId("window-2")).toBeVisible();
  await expect(page.locator("[data-window-type]")).toHaveCount(2);

  // 5. And closing one is the same contract in reverse.
  await page.getByTestId("close-window-2").click();
  await expect(page.getByTestId("window-2")).toHaveCount(0);
  await expect(page.locator("[data-window-type]")).toHaveCount(1);

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
  // **`New` makes an empty view and switches to it** — S43, punch-list B12. It
  // used to store the canvas as a new view, which is why the layout for a view
  // is now built *in* it: New, open the windows, Store. `Store` writes the
  // canvas into the active view (`prism_core::Session::store_view`); nothing
  // else does, so a view switched away from without one keeps what it had.
  await page.getByTestId("new-view").click();
  await expect(page.getByTestId("view-2")).toHaveAttribute("data-active", "yes");
  await openWindow(page, "Groups");
  await expect(page.getByTestId("window-1")).toBeVisible();
  await page.getByTestId("store-view").click();

  // Back to view 1, which was left empty — so a view switch is visible rather
  // than a no-op.
  await page.getByTestId("view-1").click();
  await expect(page.getByTestId("canvas-empty")).toBeVisible();
  await expect(page.getByTestId("view-1")).toHaveAttribute("data-active", "yes");

  // **The press.** Three bytes appended to a file, by a process that is not
  // this browser and not this daemon. Nothing in the page is touched.
  pressConsole(keys, CHANNEL_RIGHT);

  // And the interface follows. It was told; it did not ask.
  await expect(page.getByTestId("view-2")).toHaveAttribute("data-active", "yes");
  await expect(page.getByTestId("window-1")).toBeVisible();
  await expect(page.getByTestId("window-1")).toHaveAttribute("data-window-type", "Groups");

  // The other half of the D11 gate: F1 opens a window, from the same table.
  pressConsole(keys, F1);
  await expect(page.getByTestId("window-2")).toBeVisible();
  await expect(page.getByTestId("window-2")).toHaveAttribute("data-window-type", "FixtureSheet");
  await expect(page.locator("[data-window-type]")).toHaveCount(2);

  await daemon.kill();
  daemon = null;
  forget(dataDir);
});

test("**after a move, `Channel ▶` steps to the view that is drawn next**", async ({ page }) => {
  // **S35's last exit criterion.** The bar draws views in number order and
  // `prismd::surface::context_of` steps the same numbers, so moving a view has
  // to move what the console reaches — and it does, because moving *is*
  // exchanging the numbers. There is no second order for the two to disagree
  // about; this test is what says so out loud, with a real console and a real
  // daemon.
  const directory = join(process.cwd(), "test-results");
  keys = join(directory, `console-${String(Date.now())}.midi`);
  daemon = await startDaemon(PORT + 3, undefined, { mockSurface: keys });
  const dataDir = daemon.dataDir;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  // Three views, each with a layout that says which one it is: view 1 empty,
  // view 2 with a Groups window, view 3 with a Patch window.
  // **`New` makes an empty view and switches to it** — S43, punch-list B12. It
  // used to store the canvas as a new view, which is why the layout for a view
  // is now built *in* it: New, open the windows, Store. `Store` writes the
  // canvas into the active view (`prism_core::Session::store_view`); nothing
  // else does, so a view switched away from without one keeps what it had.
  await page.getByTestId("new-view").click();
  await expect(page.getByTestId("view-2")).toHaveAttribute("data-active", "yes");
  await openWindow(page, "Groups");
  await expect(page.getByTestId("window-1")).toBeVisible();
  await page.getByTestId("store-view").click();

  await page.getByTestId("new-view").click();
  await expect(page.getByTestId("view-3")).toHaveAttribute("data-active", "yes");
  await openWindow(page, "Patch");
  await expect(page.getByTestId("window-2")).toBeVisible();
  await page.getByTestId("store-view").click();

  // Back to view 1, which is where `Channel ▶` counts from.
  await page.getByTestId("view-1").click();
  await expect(page.getByTestId("view-1")).toHaveAttribute("data-active", "yes");

  // Before the move: view 2 holds the Groups window, so `Channel ▶` reaches it.
  pressConsole(keys, CHANNEL_RIGHT);
  await expect(page.getByTestId("view-2")).toHaveAttribute("data-active", "yes");
  await expect(page.getByTestId("window-1")).toHaveAttribute("data-window-type", "Groups");

  // **The move**, from the interface: view 3 goes left, so it becomes view 2
  // and the Groups layout becomes view 3.
  await page.getByTestId("view-3").click({ button: "right" });
  await expect(page.getByTestId("view-menu")).toBeVisible();
  await page.getByTestId("view-move-prev").click();
  await expect(page.getByTestId("view-menu")).toHaveCount(0);

  // The daemon answered, and the active view followed the *view* rather than
  // the number: the canvas still shows what it showed.
  await expect(page.getByTestId("view-3")).toHaveAttribute("data-active", "yes");
  await expect(page.getByTestId("window-1")).toHaveAttribute("data-window-type", "Groups");

  // Back to view 1 again, and the console's next step now reaches the view that
  // is *drawn* second — which is the Patch layout, not the Groups one.
  await page.getByTestId("view-1").click();
  await expect(page.getByTestId("view-1")).toHaveAttribute("data-active", "yes");

  pressConsole(keys, CHANNEL_RIGHT);
  await expect(page.getByTestId("view-2")).toHaveAttribute("data-active", "yes");
  await expect(page.getByTestId("window-2")).toHaveAttribute("data-window-type", "Patch");

  await daemon.kill();
  daemon = null;
  forget(dataDir);
});

test("a view is renamed and deleted from the interface, and the daemon says what follows", async ({
  page,
}) => {
  daemon = await startDaemon(PORT + 4);
  const dataDir = daemon.dataDir;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  // View 1 is left empty and view 2 gets the layout — see the note in the D11
  // test: `New` empties the canvas and `Store` is what writes it into a view.
  await page.getByTestId("new-view").click();
  await expect(page.getByTestId("view-2")).toContainText("View 2");
  await openWindow(page, "Groups");
  await expect(page.getByTestId("window-1")).toBeVisible();
  await page.getByTestId("store-view").click();

  // Rename: a command out, and the name that comes back is the daemon's.
  await page.getByTestId("view-2").click({ button: "right" });
  await page.getByTestId("view-rename").click();
  await page.getByTestId("view-rename-input").fill("Front of house");
  await page.getByTestId("view-rename-apply").click();
  await expect(page.getByTestId("view-2")).toContainText("Front of house");
  // And the layout it was storing is untouched by a rename.
  await page.getByTestId("view-2").click();
  await expect(page.getByTestId("window-1")).toHaveAttribute("data-window-type", "Groups");

  // Delete the **active** view. What the canvas becomes is the daemon's answer:
  // view 1 was stored empty, so the canvas empties.
  await expect(page.getByTestId("view-2")).toHaveAttribute("data-active", "yes");
  await page.getByTestId("view-2").click({ button: "right" });
  await page.getByTestId("view-delete").click();

  await expect(page.getByTestId("view-2")).toHaveCount(0);
  await expect(page.getByTestId("view-1")).toHaveAttribute("data-active", "yes");
  await expect(page.getByTestId("canvas-empty")).toBeVisible();

  // The last view cannot go: the session must always have one for
  // `activeViewId` to name, and the interface says so before the daemon has to.
  await page.getByTestId("view-1").click({ button: "right" });
  await expect(page.getByTestId("view-delete")).toBeDisabled();

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
    await openWindow(page, type);
  }
  // Four on the canvas. The *count* readout moved into the `Status` window in
  // S43, so what is counted here is the canvas itself — which is the thing this
  // test is about anyway.
  await expect(page.locator("[data-window-type]")).toHaveCount(4);

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
