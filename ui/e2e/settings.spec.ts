/**
 * **S37's five exit criteria, in a browser, against a real daemon.**
 *
 * The unit tests make the same claims in jsdom against a fake socket and they
 * are the ones that run on every commit. This is the one that removes the
 * remaining doubt, because it is the only place where a desk is *configured* the
 * way an operator configures one:
 *
 * 1. **S33's worked example is built entirely from the interface**, and the
 *    frames arrive where they should — the same assertion `crates/prismd/tests/outputs.rs`
 *    makes in Rust, driven from a browser.
 * 2. **A show is saved, saved under a new name and reopened** without touching
 *    a command line.
 * 3. **Changing one output does not interrupt the others**, watched in a browser
 *    against a running daemon.
 * 4. **Every panel is a reader over daemon state**: closing and reopening the
 *    window shows the same thing, and a second client sees the change.
 * 5. **A panel with more rows than it has room for scrolls inside its window** —
 *    nothing outside the canvas scrolls.
 *
 * # The daemon here is the one a venue runs
 *
 * No output flags and `--mock-devices`, which is what S36 carried out of itself:
 * a daemon whose outputs came off its command line refuses every machine
 * command, so a settings test must not use `--mock-output`. Every driver is
 * still a double, so **nothing is opened and no datagram leaves this machine** —
 * and this machine has a real SH-RS09B and a real X-Touch attached.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, startDaemon } from "./daemon.ts";

/** A port of this suite's own, so a daemon on 7373 is neither used nor disturbed. */
const PORT = 7397;

let daemon: Daemon | null = null;

test.beforeAll(() => {
  buildDaemon();
});

test.afterEach(async () => {
  await daemon?.kill();
  daemon = null;
});

/**
 * Stops the daemon and removes its directory, in that order.
 *
 * The order is not tidiness: the daemon holds an advisory lock on
 * `prismd.guard` for its whole life (§10.3), so removing the directory while it
 * is running is refused on Windows — and a test that ignored the refusal would
 * leave a directory behind on every run.
 */
async function done(dataDir: string): Promise<void> {
  await daemon?.kill();
  daemon = null;
  forget(dataDir);
}

/** Opens the desk against a fresh daemon whose rig is its own to configure. */
async function desk(page: Page, port: number): Promise<string> {
  daemon = await startDaemon(port, undefined, { configurable: true });
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // **S43, B9**: the dropdown is gone. A window is opened from the chooser, and
  // Insert is the keyboard route — a console is operated in the dark.
  await page.keyboard.press("Insert");
  await page.getByTestId("picker-Settings").click();
  await expect(page.getByTestId("settings")).toBeVisible();
  return daemon.dataDir;
}

/** Fills in the output form and applies it. */
async function addOutput(
  page: Page,
  row: {
    id: string;
    name: string;
    kind: "Mock" | "OpenDmx" | "ArtNet" | "Sacn";
    addresses?: string;
    serial?: string;
    universes: string;
  },
): Promise<void> {
  await page.getByTestId("output-add").click();
  await page.getByTestId("output-draft-id").fill(row.id);
  await page.getByTestId("output-draft-name").fill(row.name);
  await page.getByTestId("output-draft-kind").selectOption(row.kind);
  if (row.serial !== undefined) {
    await page.getByTestId("output-draft-serial").fill(row.serial);
  }
  if (row.addresses !== undefined) {
    await page.getByTestId("output-draft-addresses").fill(row.addresses);
  }
  await page.getByTestId("output-draft-universes").fill(row.universes);
  await page.getByTestId("output-draft-apply").click();
  await expect(page.getByTestId(`output-row-${row.id}`)).toBeVisible();
}

/** How many frames one output has been given, as the panel is showing it. */
async function frames(page: Page, id: string): Promise<number> {
  return Number(await page.getByTestId(`output-frames-${id}`).innerText());
}

/**
 * **Exit criterion 1 and 3.**
 *
 * The rig S33 worked out — twelve universes across five outputs of three kinds —
 * built from nothing but this window, and then reconfigured under a running
 * show while the outputs that did not change go on sending.
 */
test("a venue's rig is built entirely from the settings window", async ({ page }) => {
  const dataDir = await desk(page, PORT);

  await expect(page.getByTestId("no-outputs")).toBeVisible();
  await expect(page.getByTestId("output-count")).toHaveText("0 outputs");

  await addOutput(page, {
    id: "1",
    name: "Stage left dimmers",
    kind: "OpenDmx",
    serial: "B0037HIY",
    universes: "1",
  });
  await addOutput(page, {
    id: "2",
    name: "Stage right dimmers",
    kind: "OpenDmx",
    serial: "A50285BI",
    universes: "2",
  });
  await addOutput(page, { id: "3", name: "Hall gateway", kind: "Sacn", universes: "3, 4" });
  await addOutput(page, {
    id: "4",
    name: "Bridge node",
    kind: "ArtNet",
    addresses: "127.0.0.1:6454",
    universes: "5, 6, 7, 8",
  });
  await addOutput(page, {
    id: "5",
    name: "Gallery node",
    kind: "ArtNet",
    addresses: "127.0.0.1:6455",
    universes: "9, 10, 11, 12",
  });

  await expect(page.getByTestId("output-count")).toHaveText("5 outputs");
  // Each row carries exactly the universes it was given, which is the routing
  // claim: not one of them was handed a universe it was not configured for.
  await expect(page.getByTestId("output-universes-1")).toHaveText("1");
  await expect(page.getByTestId("output-universes-3")).toHaveText("3, 4");
  await expect(page.getByTestId("output-universes-4")).toHaveText("5, 6, 7, 8");
  await expect(page.getByTestId("output-universes-5")).toHaveText("9, 10, 11, 12");

  // **And the frames arrive.** A browser cannot look at a wire, so it looks at
  // the counter the daemon keeps per driver — which is the same number
  // `MockOutputHandle::frame_count` answers in Rust, one layer out.
  for (const id of ["1", "2", "3", "4", "5"]) {
    await expect
      .poll(async () => frames(page, id), { timeout: 10_000 })
      .toBeGreaterThan(3);
  }

  // Exit criterion 3: change one, and watch the others carry on. Output 4 is
  // re-addressed while 1, 2, 3 and 5 keep sending.
  const before = await Promise.all(["1", "2", "3", "5"].map(async (id) => frames(page, id)));
  await page.getByLabel("Edit output 4").click();
  await page.getByTestId("output-draft-addresses").fill("127.0.0.1:6456");
  await page.getByTestId("output-draft-apply").click();
  for (const [index, id] of ["1", "2", "3", "5"].entries()) {
    await expect
      .poll(async () => frames(page, id), { timeout: 10_000 })
      .toBeGreaterThan(before[index] ?? 0);
  }
  // …and the one that changed comes back by itself.
  await expect.poll(async () => frames(page, "4"), { timeout: 10_000 }).toBeGreaterThan(0);

  // The rig is the **machine's**, so it is on the disk rather than in this tab.
  // The window comes back on its own as well, because which windows are open is
  // session state (S25) — so nothing is opened here, and a second one would be a
  // second window rather than the same one.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("settings")).toBeVisible();
  await expect(page.getByTestId("output-count")).toHaveText("5 outputs");
  await done(dataDir);
});

/**
 * **Exit criterion 2.**
 *
 * A show is saved, saved under a new name and reopened, without a command line
 * anywhere in it — which is what the *Show files* panel exists for and what
 * `SaveShow` alone could not do.
 */
test("a show is saved, renamed and reopened from the window", async ({ page }) => {
  const dataDir = await desk(page, PORT + 1);
  await page.getByTestId("settings-tab-show-files").click();

  const opened = await page.getByTestId("show-path").innerText();
  expect(opened).toBe("default.prism");

  // What is *in* the two files is asserted in Rust, where the bytes can be read
  // (`crates/prismd/tests/settings.rs`). What only a browser can say is that an
  // operator gets there without a command line, which is this.
  await page.getByTestId("show-SaveShowAs").click();
  await page.getByTestId("show-path-input").fill("panto.prism");
  await page.getByTestId("show-form-apply").click();
  await expect(page.getByTestId("show-path")).toHaveText("panto.prism");
  await expect(page.getByTestId("show-dirty")).toHaveText("saved");

  // Back to the one that was open, out of the recent list — one click, because
  // the pointer has supplied the argument (§4.5).
  await page.getByTestId("show-recent-default.prism").click();
  await expect(page.getByTestId("show-path")).toHaveText("default.prism");
  await expect(page.getByTestId("show-recent-panto.prism")).toBeVisible();

  // And the other one opens again by name.
  await page.getByTestId("show-OpenShow").click();
  await page.getByTestId("show-path-input").fill("panto.prism");
  await page.getByTestId("show-form-apply").click();
  await expect(page.getByTestId("show-path")).toHaveText("panto.prism");
  await done(dataDir);
});

/**
 * **Exit criterion 4**, in the form only a browser can take: *a second client
 * sees the change*.
 *
 * Two tabs, two clients, one daemon. One changes a setting and the other's panel
 * follows without anybody touching it — because neither of them is holding it.
 */
test("a second client sees what the first one changed", async ({ page, context }) => {
  const dataDir = await desk(page, PORT + 2);
  const second = await context.newPage();
  await second.goto(`/?daemon=${encodeURIComponent(daemon?.url ?? "")}`);
  await expect(second.getByTestId("connection-status")).toHaveText("Connected");
  // **It does not open a window.** Which windows are open is session state
  // (§4.1), so the settings window the first tab opened is already on this
  // canvas — which is D11 met a third time, from a second browser rather than
  // from a console.
  await expect(second.getByTestId("settings")).toBeVisible();
  await second.getByTestId("settings-tab-this-machine").click();
  await expect(second.getByTestId("machine-universes")).toHaveValue("64");

  await page.getByTestId("settings-tab-this-machine").click();
  await page.getByTestId("machine-universes").fill("12");
  await page.getByTestId("machine-universes-apply").click();

  // Nobody told the second tab anything. It knows.
  await expect(second.getByTestId("machine-universes")).toHaveValue("12");

  // And the exit action, which is a setting a running daemon acts on at once.
  // (The log level is not used here: this harness passes `--log-level warn`, so
  // that row is one a flag is holding — which the token test asserts.)
  await page.getByTestId("machine-exit").selectOption("Blackout");
  await expect(second.getByTestId("machine-exit")).toHaveValue("Blackout");

  // Closing the window and opening it again shows the same thing, because no
  // panel kept any of it.
  await second.close();
  await page.getByTestId("close-window-1").click();
  await expect(page.getByTestId("settings")).toBeHidden();
  await openWindow(page, "Settings");
  await page.getByTestId("settings-tab-this-machine").click();
  await expect(page.getByTestId("machine-universes")).toHaveValue("12");
  await done(dataDir);
});

/**
 * **Exit criterion 5**, checked in a browser rather than reviewed in a
 * stylesheet.
 *
 * `CLAUDE.md`'s rule: nothing outside the canvas scrolls, ever. A settings panel
 * with more outputs than it has room for scrolls **inside its own window**,
 * which is what a window is for — and a check that only demanded zeros
 * everywhere would pass for a panel that had quietly clipped its rows.
 */
test("a panel with more rows than it has room for scrolls inside its window", async ({ page }) => {
  const dataDir = await desk(page, PORT + 3);

  // A window small enough that a dozen rows will not fit, put there the way an
  // operator would — by dragging its corner.
  const corner = page.getByTestId("resize-window-1");
  const box = await corner.boundingBox();
  expect(box).not.toBeNull();
  if (box === null) {
    throw new Error("the window has no corner");
  }
  await page.mouse.move(box.x + 4, box.y + 4);
  await page.mouse.down();
  await page.mouse.move(box.x - 300, box.y - 260, { steps: 8 });
  await page.mouse.up();

  for (let id = 1; id <= 12; id += 1) {
    await addOutput(page, {
      id: String(id),
      name: `Node ${String(id)}`,
      kind: "Mock",
      universes: String(id),
    });
  }

  const overflow = await page.evaluate(() => {
    const body = document.querySelector('[data-testid="settings"] .settings-body');
    const root = document.documentElement;
    const canvas = document.querySelector('[data-testid="canvas"]');
    return {
      insideWindow: body === null ? 0 : body.scrollHeight - body.clientHeight,
      documentWidth: root.scrollWidth - root.clientWidth,
      documentHeight: root.scrollHeight - root.clientHeight,
      canvasWidth: canvas === null ? 0 : canvas.scrollWidth - canvas.clientWidth,
      canvasHeight: canvas === null ? 0 : canvas.scrollHeight - canvas.clientHeight,
    };
  });

  // Inside: yes, and that is the half a check of zeros everywhere would miss.
  expect(overflow.insideWindow).toBeGreaterThan(0);
  // Outside: nowhere.
  expect(overflow.documentWidth).toBe(0);
  expect(overflow.documentHeight).toBe(0);
  expect(overflow.canvasWidth).toBe(0);
  expect(overflow.canvasHeight).toBe(0);
  await done(dataDir);
});

/**
 * The Devices panel against a real daemon, with **nothing plugged in as far as
 * this test is concerned**.
 *
 * `CLAUDE.md` forbids a test from touching a device, and this machine has a real
 * X-Touch attached — so what is asserted is what is true either way: the
 * enumeration answers, the panel draws it, and pressing *rescan* asks again.
 * Which ports come back is the machine's business and is not asserted.
 */
test("the devices panel enumerates without opening anything", async ({ page }) => {
  const dataDir = await desk(page, PORT + 4);
  await page.getByTestId("settings-tab-devices").click();

  // It answers. A machine with no MIDI device answers with an empty list, which
  // is an answer rather than a failure to look — and is what CI runs.
  await expect(page.getByTestId("port-count")).toContainText("MIDI port");
  await expect(page.getByTestId("port-table")).toBeVisible();
  await page.getByTestId("ports-rescan").click();
  await expect(page.getByTestId("port-count")).toContainText("MIDI port");

  // No surface is configured, and that is drawn rather than left blank.
  await expect(page.getByTestId("no-surface")).toBeVisible();
  await done(dataDir);
});

/**
 * **The token is the daemon's to make and the interface's to show** — §2.1 — and
 * **a row a flag is holding is greyed out and names the flag**.
 *
 * The two are in one test because this harness produces the second by
 * construction: every daemon it starts is given `--websocket 127.0.0.1:<port>`,
 * so that it has a port of its own. That flag is the value for the run, and the
 * WebSocket row is therefore held — which is exactly the state S37's override
 * mechanism exists to draw, met here against a real daemon rather than against a
 * fixture. The §2.1 *refusal* is asserted where the address can be changed:
 * `crates/prismd/tests/settings.rs`.
 */
test("a token is made by the daemon, and a flag greys the row it holds", async ({ page }) => {
  const dataDir = await desk(page, PORT + 5);
  await page.getByTestId("settings-tab-this-machine").click();
  await expect(page.getByTestId("machine-token")).toHaveText("none");

  // §2.1: the daemon generates it and the interface displays it. Nothing about
  // the value came from this browser.
  await page.getByTestId("machine-new-token").click();
  await expect(page.getByTestId("machine-token")).not.toHaveText("none");
  const token = await page.getByTestId("machine-token").innerText();
  expect(token).toHaveLength(26);

  // A row this run's command line is holding: drawn, disabled, and the flag
  // named — rather than a box that would appear to work and vanish at the next
  // restart.
  await expect(page.getByTestId("machine-websocket")).toBeDisabled();
  await expect(page.getByTestId("machine-network")).toContainText("--websocket");
  // …and a row nothing is holding is still the operator's.
  await expect(page.getByTestId("machine-autostart")).toBeEnabled();
  await page.getByTestId("machine-autostart").click();
  await expect(page.getByTestId("machine-autostart")).toBeChecked();
  await done(dataDir);
});

/**
 * **Punch-list B5, and the reason it needed a browser to see.**
 *
 * The owner reported that some notices cannot be closed, and named the one that
 * proves it: a successful JSON export. Every notice has carried a close button
 * since it was written, so in `jsdom` — which has no layout — every unit test
 * about it passed, and the entry was read as *hard to see* and wrongly closed.
 *
 * It was not hard to see. An export's message is an **absolute path with no
 * spaces in it**, a flex item's automatic minimum size is its min-content width,
 * and so the message refused to shrink and pushed the button one pixel past
 * `.notice-wrapper`, where `overflow: hidden` removed all 28 px of it. An error
 * is a sentence, it wraps, and its button stays — which is exactly the
 * difference the owner described.
 *
 * So the assertion is a **box inside a box**, on the message that caused it, in
 * a browser that does layout. Nothing about the DOM would have caught this, and
 * that is the point of the test.
 */
test("the close button of a long notice is inside the notice", async ({ page }) => {
  const dataDir = await desk(page, PORT + 6);
  await page.getByTestId("settings-tab-show-files").click();
  await page.getByTestId("show-ExportShow").click();
  // A name, so the path the daemon answers with is a long absolute one — which
  // is the whole of the fault.
  await page.getByTestId("settings-showfiles").getByRole("textbox").fill("b5-export.json");
  await page.getByTestId("show-form-apply").click();

  const close = page.getByTestId("notice-close-1");
  await expect(close).toBeVisible();
  const boxes = await page.evaluate(() => {
    const button = document.querySelector(".notice-close");
    const wrapper = document.querySelector(".notice-wrapper");
    if (button === null || wrapper === null) {
      return null;
    }
    const one = button.getBoundingClientRect();
    const two = wrapper.getBoundingClientRect();
    return {
      width: Math.round(one.width),
      overhang: Math.round(one.right - two.right),
    };
  });
  expect(boxes?.width).toBeGreaterThan(0);
  // Not past the edge that clips it. One pixel was enough to lose the button.
  expect(boxes?.overhang).toBeLessThanOrEqual(0);

  // And pressing it takes the message away — the second half of the entry. The
  // removal used to wait on `transitionend` alone, so a browser that did not run
  // the collapse kept the notice for ever with a button that did nothing.
  await close.click();
  await expect(page.getByTestId("notices")).toHaveCount(0);
  await done(dataDir);
});

/**
 * **Punch-list B6, and the half a browser can answer.**
 *
 * The entry is *Art-Net shows Health OK with no node plugged in*, and the whole
 * of it cannot be tested here: the fault is a **node that does not answer**, and
 * a node that does answer is a device this suite may not have. What a browser
 * can answer, and what nothing else does, is that the panel **never draws a node
 * list without saying whether this desk is listening at all** — because an empty
 * list under a socket that never opened says nothing about the network, and
 * reading it as *no nodes* is the same mistake B6 was, pointed the other way.
 *
 * The daemon here runs with `--mock-devices`, so it opens no socket of any kind.
 * That is exactly the state the note exists for, and the reason it draws is the
 * **daemon's own words** rather than a sentence this interface guessed.
 */
test("the node list never appears without saying whether the desk is listening", async ({
  page,
}) => {
  const dataDir = await desk(page, PORT + 7);

  const note = page.getByTestId("artnet-not-listening");
  await expect(note).toBeVisible();
  await expect(note).toContainText("not listening for Art-Net nodes");
  // The reason is the daemon's: this run was told not to listen, which is a
  // different fact from a rig with no Art-Net row in it.
  await expect(note).toContainText("--mock-devices");
  // …and the panel does not claim the network is empty while it is deaf.
  await expect(page.getByTestId("artnet-no-nodes")).toHaveCount(0);
  await expect(page.getByTestId("artnet-node-0")).toHaveCount(0);

  // Configuring an Art-Net output does not change that, because this daemon was
  // told not to listen — which is what makes the note a *reason* rather than an
  // instruction the operator has just followed for nothing.
  await addOutput(page, {
    id: "1",
    name: "Bridge node",
    kind: "ArtNet",
    addresses: "127.0.0.1:6454",
    universes: "1",
  });
  await expect(note).toBeVisible();
  await expect(note).toContainText("--mock-devices");

  // The health column is still the daemon's word and nothing is folded into it,
  // because a desk that is not listening knows nothing about the far end.
  await expect(page.getByTestId("output-health-1")).toHaveText("Ok");
  await expect(page.getByTestId("output-nodes-1")).toHaveCount(0);

  await done(dataDir);
});
