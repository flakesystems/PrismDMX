/**
 * **The Trackers panel, in a browser, against a real daemon** - S32.
 *
 * `crates/prismd/tests/tracking.rs` holds the whole chain from a PSN datagram to
 * a pan byte on the cable, with a double for the socket; the unit tests in
 * `settings/settingswindow.test.tsx` hold the panel against a fake daemon. What
 * only this can say is the part in between: **a desk configured the way an
 * operator configures one** - through the window, over a real WebSocket, written
 * to a real `machine.json` - and still configured after the page has been
 * closed and opened again.
 *
 * # The daemon is the one a venue runs, with every device a double
 *
 * `--mock-devices`, which is also what keeps the tracker receiver off: a daemon
 * that opens a multicast socket nobody asked for surprises a firewall, and this
 * machine has a real network. So the receiver says why it is not listening, in
 * the daemon's own words, and the *settings* - which are the machine's whether
 * or not anything listens - are what is asserted.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, startDaemon } from "./daemon.ts";

/** A port of this suite's own, so a daemon on 7373 is neither used nor disturbed. */
const PORT = 7415;

let daemon: Daemon | null = null;

test.beforeAll(() => {
  buildDaemon();
});

test.afterEach(async () => {
  await daemon?.kill();
  daemon = null;
});

/** Opens the desk against a fresh daemon, with the Trackers panel showing. */
async function desk(page: Page, port: number): Promise<string> {
  daemon = await startDaemon(port, undefined, { configurable: true });
  await openTrackers(page);
  return daemon.dataDir;
}

async function openTrackers(page: Page): Promise<void> {
  if (daemon === null) {
    throw new Error("no daemon");
  }
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // The window is the daemon's (D11): after a reload it is already there.
  if ((await page.getByTestId("settings").count()) === 0) {
    await page.keyboard.press("Insert");
    await page.getByTestId("picker-Settings").click();
  }
  await expect(page.getByTestId("settings")).toBeVisible();
  await page.getByTestId("settings-tab-trackers").click();
  await expect(page.getByTestId("settings-trackers")).toBeVisible();
}

test("says why it is not listening, and keeps what the operator sets across a reload", async ({
  page,
}) => {
  const dataDir = await desk(page, PORT);

  // Not listening, and **why** - in the daemon's own words. An empty list under a
  // receiver that never opened says nothing about the network.
  await expect(page.getByTestId("trackers-status")).toContainText("--mock-devices");

  // The published defaults, because nobody has said otherwise.
  await expect(page.getByTestId("trackers-group")).toHaveValue("236.10.10.10");
  await expect(page.getByTestId("trackers-port")).toHaveValue("56565");
  await expect(page.getByTestId("trackers-enabled")).not.toBeChecked();

  // One field at a time, each when it is left.
  const group = page.getByTestId("trackers-group");
  await group.fill("236.10.10.99");
  await group.blur();
  await expect(group).toHaveValue("236.10.10.99");

  await page.getByTestId("trackers-scale").fill("0.001");
  await page.getByTestId("trackers-scale").blur();
  // A box that is the daemon's: it flips when the daemon says so, not when it
  // is clicked, which is why this is a click and an expectation and not `check`.
  await page.getByTestId("trackers-axis-x-invert").click();
  await expect(page.getByTestId("trackers-axis-x-invert")).toBeChecked();
  await page.getByTestId("trackers-axis-y-from").selectOption("Y");
  await page.getByTestId("trackers-timeout").fill("900");
  await page.getByTestId("trackers-timeout").blur();

  // A refused value is refused whole and the box goes back to what the daemon
  // holds: a port of nought is not a port.
  await page.getByTestId("trackers-port").fill("0");
  await page.getByTestId("trackers-port").blur();
  await expect(page.getByTestId("trackers-port")).toHaveValue("56565");

  // The whole page is closed and opened again: nothing was held in the browser.
  await openTrackers(page);
  await expect(page.getByTestId("trackers-group")).toHaveValue("236.10.10.99");
  await expect(page.getByTestId("trackers-scale")).toHaveValue("0.001");
  await expect(page.getByTestId("trackers-axis-x-invert")).toBeChecked();
  await expect(page.getByTestId("trackers-axis-y-from")).toHaveValue("Y");
  await expect(page.getByTestId("trackers-timeout")).toHaveValue("900");

  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});

test("nothing outside the canvas scrolls with the panel open", async ({ page }) => {
  const dataDir = await desk(page, PORT + 1);
  const overflow = await page.evaluate(() => ({
    document: document.documentElement.scrollHeight - document.documentElement.clientHeight,
    body: document.body.scrollHeight - document.body.clientHeight,
  }));
  expect(overflow.document).toBeLessThanOrEqual(0);
  expect(overflow.body).toBeLessThanOrEqual(0);
  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});
