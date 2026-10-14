/**
 * **S27's three exit criteria and S57's eight points, in a browser, against a
 * real daemon.**
 *
 * The unit tests make the same claims in jsdom against a fake socket and they
 * are the ones that run on every commit. This is the one that removes the
 * remaining doubt, because it is the only place where a rig is built the way an
 * operator builds one:
 *
 * 1. **A rig is patched, addressed and edited entirely from the interface** —
 *    starting from a show that has *nothing* in it, not even a profile.
 * 2. **An address conflict is shown before it is committed**, in the daemon's
 *    own words, with the command not yet sent — and since S57 with the next
 *    free address beside it.
 * 3. **The fixture sheet shows live values** — what the programmer holds and
 *    what is on the cable, which are two different things and are drawn two
 *    different ways.
 *
 * And S57, punch-list **B60**: every one of the owner's eight points is driven
 * here, against the library the machine installed.
 *
 * # Nothing here touches a device
 *
 * `CLAUDE.md`'s rule. The output is `--mock-output` and there is no console in
 * this file at all.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, startDaemon } from "./daemon.ts";
import { description, writeGdtf } from "./gdtf.ts";

/** A port of this suite's own, so a daemon on 7373 is neither used nor disturbed. */
const PORT = 7393;

let daemon: Daemon | null = null;

test.beforeAll(() => {
  buildDaemon();
});

test.afterEach(async () => {
  await daemon?.kill();
  daemon = null;
});

/** Opens the desk against a fresh daemon with an empty show. */
async function desk(page: Page, port: number): Promise<string> {
  daemon = await startDaemon(port);
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  return daemon.dataDir;
}

/**
 * Picks a fixture out of the library panel that is open, by what is typed and
 * the key of its first mode.
 *
 * **Since S57 the panel is what *Add fixture* opens** (B60's fourth point), and
 * a click anywhere on the row picks it (the second). Picking embeds nothing:
 * the profile is embedded with the fixtures, in the same step.
 */
async function pick(page: Page, search: string, key: string): Promise<void> {
  await page.getByTestId("library-search").fill(search);
  // The **name** cell, not the first one: the whole row picks.
  await page.getByTestId(`library-row-${key}`).locator("td").nth(1).click();
}

/** Types a whole number into one of the patch form's fields. */
async function typeNumber(page: Page, testId: string, value: string): Promise<void> {
  const field = page.getByTestId(testId);
  await field.fill(value);
}

/** Whether this machine installed the Open Fixture Library, asked of the open panel. */
async function libraryInstalled(page: Page): Promise<boolean> {
  // **The count is the daemon's answer**, and it is waited for rather than
  // read: `textContent()` does not wait, and a count read before the first
  // page arrived would say *nothing installed* on a machine that has two
  // thousand fixtures. Four is the built-in generics, and nothing else.
  const count = page.getByTestId("library-count");
  await expect(count).toHaveText(/of \d+ fixtures/);
  const total = /of (\d+) fixtures/.exec((await count.textContent()) ?? "");
  return total !== null && Number(total[1]) > 4;
}

test("a rig is built, addressed and edited entirely from the interface", async ({ page }) => {
  const dataDir = await desk(page, PORT);
  await openWindow(page, "Patch");
  await expect(page.getByTestId("patch")).toBeVisible();

  // 1. A brand-new show carries **no profiles at all**, and the count says so.
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 0 profiles");

  // 2. Patch a fixture. *Add fixture* **is** the library (S57): the search is
  //    there at once, beside the fixture's settings.
  await page.getByRole("button", { name: "Add fixture" }).click();
  await expect(page.getByTestId("library-search")).toBeVisible();
  await expect(page.getByTestId("draft-type")).toContainText("Choose a fixture");
  await pick(page, "generic rgbw", "generic.rgbw.par");
  await expect(page.getByTestId("draft-type")).toHaveText("Generic RGBW PAR");
  // **Picking embedded nothing** — the show is as it was.
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 0 profiles");

  await typeNumber(page, "draft-name", "Front left");
  await expect(page.getByTestId("patch-preview")).toContainText("Free");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-1")).toContainText("Front left");
  // The profile came with the fixture, in the same step.
  await expect(page.getByTestId("patch-count")).toHaveText("1 fixtures · 1 profiles");
  // The count is a `Status` reading, which S43 moved off the bottom bar and
  // into a window of its own.
  await openWindow(page, "Status");
  await expect(page.getByTestId("fixtures")).toHaveText("1");

  // And a second one. **It starts at the next free address** (S57): the PAR
  // is 1-4, so it is 5, and nobody typed it.
  await page.getByRole("button", { name: "Add fixture" }).click();
  await pick(page, "generic rgbw", "generic.rgbw.par");
  await expect(page.getByTestId("draft-address")).toHaveValue("5");
  // **A fixture with no name is named after its type** (S57): the field says
  // so, and the daemon does it.
  await expect(page.getByTestId("draft-name")).toHaveAttribute("placeholder", "RGBW PAR");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-2")).toContainText("RGBW PAR");
  await expect(page.getByTestId("patch-row-2")).toContainText("5");

  // 3. **The conflict, before it is committed.** Fixture 2 is moved onto
  //    fixture 1's channels — and the daemon says so while the panel is still
  //    open and nothing has been sent, **with where it would fit** (S57).
  await page.getByTestId("patch-row-2").click();
  await typeNumber(page, "draft-address", "3");
  const preview = page.getByTestId("patch-preview");
  await expect(preview).toContainText("Overlaps 3–4 with fixture 1");
  await expect(preview).toContainText("higher fixture number wins");
  await expect(preview).toContainText("Next free: 1.5.");
  await expect(page.getByTestId("draft-next-free")).toHaveText("Move to 1.5");
  // The row has **not** moved: this is a question, not a command.
  await expect(page.getByTestId("patch-row-2")).toContainText("5");

  // An address that would be refused says so, and Patch goes dead — again
  // before anything is sent.
  await typeNumber(page, "draft-address", "510");
  await expect(preview).toContainText("510");
  await expect(page.getByTestId("draft-apply")).toBeDisabled();

  // Back to the overlap, which is legal: an operator cloning a fixture does
  // this on purpose, so the button comes back and the patch goes through.
  await typeNumber(page, "draft-address", "3");
  await expect(page.getByTestId("draft-apply")).toBeEnabled();
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-2")).toContainText("3");
  await expect(page.getByTestId("patch-row-1")).toHaveClass(/row-conflict/);
  await expect(page.getByTestId("patch-row-2")).toHaveClass(/row-conflict/);

  // 4. Editing: a name, and the next free address on one key.
  await page.getByTestId("patch-row-2").click();
  await typeNumber(page, "draft-name", "Front right (moved)");
  await page.getByTestId("draft-next-free").click();
  await expect(page.getByTestId("draft-address")).toHaveValue("5");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-2")).toContainText("Front right (moved)");
  await expect(page.getByTestId("patch-row-1")).not.toHaveClass(/row-conflict/);

  // Then a **number**, which is its own command because the number is the key
  // the patch is filed under.
  await page.getByTestId("patch-row-2").click();
  await typeNumber(page, "draft-id", "20");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-20")).toContainText("Front right (moved)");
  await expect(page.getByTestId("patch-row-2")).toHaveCount(0);

  // 5. And taking one out again.
  await page.getByTestId("patch-row-20").click();
  await page.getByTestId("draft-remove").click();
  await expect(page.getByTestId("patch-row-20")).toHaveCount(0);
  await expect(page.getByTestId("fixtures")).toHaveText("1");

  // 6. The whole rig lives in the daemon, so a reload finds it there. Nothing
  //    was saved and the daemon was not told anything.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("patch-row-1")).toContainText("Front left");

  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});

test("the fixture sheet shows the programmer and the cable, and they can differ", async ({
  page,
}) => {
  const dataDir = await desk(page, PORT + 1);

  // A rig, built through the patch window as above but without the detours.
  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();
  await pick(page, "generic rgbw", "generic.rgbw.par");
  await typeNumber(page, "draft-name", "Wash 1");
  await expect(page.getByTestId("patch-preview")).toContainText("Free");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-1")).toBeVisible();
  // The sheet, on the Colour bank, which is where an RGBW PAR's parameters are.
  await openWindow(page, "FixtureSheet");
  await expect(page.getByTestId("fixture-sheet")).toBeVisible();
  await page.getByTestId("bank-Color").click();
  await expect(page.getByTestId("sheet-bank")).toContainText("Color");

  // **Absent is not zero.** Nothing has been touched, so the programmer reads a
  // dash — not 0 %, which would mean something quite different.
  await expect(page.getByTestId("prog-1-Red")).toHaveText("—");
  // And nothing is on the cable either, so the picture of the rig is dark.
  await openWindow(page, "DmxSheet");
  await expect(page.getByTestId("telemetry-canvas")).toBeVisible();
  await expect.poll(async () => litColumns(page), { timeout: 15_000 }).toBe(0);

  // Now put red on it from the command line (S26's parser).
  await page.getByTestId("command-input").fill("1 red at 100");
  await page.getByTestId("command-input").press("Enter");
  // **And the intensity, which is the desk's since S43.** These PARs have no
  // dimmer channel of their own, so the desk supplies one that scales their
  // colour — and it rests at nought, which is what stops a rig of them coming
  // up white. Colour without intensity is no light, exactly as on a fixture with
  // a real dimmer, so an operator who wants to see something brings it up.
  await page.getByTestId("command-input").fill("1 at 100");
  await page.getByTestId("command-input").press("Enter");

  // The programmer column is a reader over `ProgrammerChanged` and renders in
  // React like everything else.
  await expect(page.getByTestId("prog-1-Red")).toHaveText("100%");
  await expect(page.getByTestId("prog-1-Green")).toHaveText("—");

  // And the **output** column is telemetry, drawn on a canvas outside React —
  // counted here off the pixels rather than read out of a readout that could
  // agree with itself.
  await expect.poll(async () => litColumns(page), { timeout: 15_000 }).toBeGreaterThan(0);

  // Clearing the programmer takes it off both — two presses since S51 (B37),
  // the selection then the values.
  await page.getByTestId("clear").click();
  await page.getByTestId("clear").click();
  await expect(page.getByTestId("prog-1-Red")).toHaveText("—");
  await expect.poll(async () => litColumns(page), { timeout: 15_000 }).toBe(0);

  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});

test("a sheet with more rows than it has room for scrolls inside its window", async ({ page }) => {
  // `CLAUDE.md` forbids scrolling *outside* the canvas and a window is exactly
  // where a long list belongs. So: forty fixtures, both sheets open, and the
  // document, the canvas and both bars still measuring zero.
  //
  // **Forty in one gesture since S57** (B60's eighth point): a count, placed
  // by the daemon one after the other. It used to be a loop of forty panels,
  // each with its number typed over the form's suggestion.
  const dataDir = await desk(page, PORT + 2);
  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();
  await pick(page, "generic dimmer", "generic.dimmer");
  await typeNumber(page, "draft-count", "40");
  await expect(page.getByTestId("patch-preview")).toContainText("40 fixtures, 1 to 40, at 1.1 to 1.40");
  await page.getByTestId("draft-apply").click();
  await openWindow(page, "Status");
  await expect(page.getByTestId("fixtures")).toHaveText("40");
  await openWindow(page, "FixtureSheet");
  await expect(page.getByTestId("fixture-sheet")).toBeVisible();

  const overflow = await page.evaluate(() => {
    const box = (selector: string): [number, number] => {
      const element = document.querySelector(selector);
      return element === null
        ? [0, 0]
        : [element.scrollWidth - element.clientWidth, element.scrollHeight - element.clientHeight];
    };
    return {
      page: [
        document.documentElement.scrollWidth - document.documentElement.clientWidth,
        document.documentElement.scrollHeight - document.documentElement.clientHeight,
      ] as [number, number],
      canvas: box('[data-testid="canvas"]'),
      // **The one band left** — S43 took the executor strip and the readings
      // off the screen and made them windows, so what is outside the canvas is
      // the header, the command line and the programmer band.
      band: box('[data-testid="programmer-band"]'),
      // The one place that *is* allowed to scroll, and has to: forty rows do
      // not fit in a window.
      sheet: box('[data-testid="sheet-scroll"]'),
    };
  });
  expect(overflow.page).toEqual([0, 0]);
  expect(overflow.canvas).toEqual([0, 0]);
  expect(overflow.band).toEqual([0, 0]);
  expect(overflow.sheet[1]).toBeGreaterThan(0);

  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});

/**
 * How many pixels of the **DMX Sheet** are drawn in a colour a channel that is
 * up is drawn in.
 *
 * Counting pixels is the only way to say *the value reached the output* about a
 * picture rather than about a readout that could agree with itself.
 *
 * **It used to count the fixture sheet's own output column** — a canvas over
 * the last column, drawn thirty times a second. **Punch-list B8 took that column
 * out**: the owner's reading was that its scaling was wrong and that the table
 * beside it was enough. So *what is on the cable* is read where it now lives,
 * which is the `DMX Sheet` window, channel by channel and with no fixtures in
 * the way. The claim of the test that uses this is unchanged: what the
 * programmer holds and what is on the cable are two facts and can differ.
 */
async function litColumns(page: Page): Promise<number> {
  return page.getByTestId("telemetry-canvas").evaluate((element) => {
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
      // `#ffd479` — `INK.meter`, the colour a universe's peak meter is filled
      // in when something in it is at full. **The meter and not the grid**, and
      // that is what this rig makes necessary: one channel of five hundred and
      // twelve is up here, so the grid gives it a fraction of a pixel and what
      // lands on the canvas is a blend of it and its neighbours. The meter is a
      // block. `executors.spec.ts` reads the picture the same way and for the
      // same reason.
      const red = image.data[at] ?? 0;
      const green = image.data[at + 1] ?? 0;
      const blue = image.data[at + 2] ?? 0;
      if (red === 255 && green === 212 && blue === 121) {
        found += 1;
      }
    }
    return found;
  });
}

test("a real fixture is one row with its modes, patched three at a time and taken back in one", async ({
  page,
}) => {
  // **S44's point and S57's eight, end to end**: an operator types the name
  // printed on the light, gets the manufacturer's channel order, chooses the
  // mode beside it, and patches three of them in one gesture.
  //
  // The library is downloaded at install time, so this test says why it is
  // skipping rather than failing on a machine that has not installed it — CI
  // installs it, so it runs there on every commit.
  const dataDir = await desk(page, PORT + 3);
  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();
  if (!(await libraryInstalled(page))) {
    test.skip(true, "no fixture library is installed - run tools/fetch-fixtures");
    return;
  }

  // 1. **One row per fixture**, its two modes listed in it — it used to be two
  //    rows, one per mode.
  await page.getByTestId("library-search").fill("stage wash 7x10");
  const key = "stage-right/stage-wash-7x10w-led-moving-head/9ch";
  const row = page.getByTestId(`library-row-${key}`);
  await expect(row).toBeVisible();
  await expect(row.locator("td").nth(2)).toHaveText("9ch · 14ch");
  await expect(page.getByTestId("library-row-stage-right/stage-wash-7x10w-led-moving-head/14ch")).toHaveCount(0);

  // 2. **The whole row picks** — the modes cell, the one that used to do
  //    nothing.
  await row.locator("td").nth(2).click();
  await expect(page.getByTestId("draft-type")).toContainText("Stage Wash 7x10W");
  // Nine channels, out of a profile read from a JSON file this repository
  // does not contain — and nothing embedded yet.
  await expect(page.getByTestId("patch-preview")).toContainText("9 channels");
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 0 profiles");

  // The mode, chosen beside it.
  await page
    .getByTestId("draft-mode")
    .selectOption("stage-right/stage-wash-7x10w-led-moving-head/14ch");
  await expect(page.getByTestId("patch-preview")).toContainText("14 channels, ending at 14");

  // 8. **Three at once**, placed one after the other by the daemon.
  await typeNumber(page, "draft-count", "3");
  await expect(page.getByTestId("patch-preview")).toContainText("3 fixtures, 1 to 3, at 1.1 to 1.29");
  await page.getByTestId("draft-apply").click();
  for (const [id, address, index] of [
    [1, "1", "1"],
    [2, "15", "2"],
    [3, "29", "3"],
  ] as const) {
    const patched = page.getByTestId(`patch-row-${String(id)}`);
    // 7. **Named after their type**, and told apart by their place.
    await expect(patched).toContainText(`Stage Wash 7x10W LED Moving Head ${index}`);
    await expect(patched.locator("td").nth(4)).toHaveText(address);
    await expect(patched.locator("td").nth(5)).toHaveText("14");
  }
  // None of them shares a channel with another.
  await expect(page.getByTestId("patch-row-2")).not.toHaveClass(/row-conflict/);
  await expect(page.getByTestId("patch-count")).toHaveText("3 fixtures · 1 profiles");

  // **One Oops takes all three back**, and the profile they came with. Typed
  // and run, which is an undo whatever stands on the line (B58).
  await page.getByTestId("command-input").fill("Oops");
  await page.getByTestId("command-input").press("Enter");
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 0 profiles");

  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});

test("the library loads as it is scrolled, and nothing scrolls outside the canvas", async ({
  page,
}) => {
  // 3. **The whole library is reachable without typing a search first** —
  //    a page at a time, asked for when the list reaches its end. And the
  //    panel, open at 1280 × 720, scrolls inside itself and nowhere else.
  const dataDir = await desk(page, PORT + 4);
  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();
  if (!(await libraryInstalled(page))) {
    test.skip(true, "no fixture library is installed - run tools/fetch-fixtures");
    return;
  }
  const rows = page.getByTestId(/^library-row-/);
  // A page is sixty — and a page that does not fill the list is followed by
  // the next at once, so what is drawn is at least one page.
  await expect.poll(async () => rows.count()).toBeGreaterThanOrEqual(60);
  const before = await rows.count();

  // Scrolled to the end: the next page arrives.
  await page.getByTestId("library-scroll").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
    element.dispatchEvent(new Event("scroll"));
  });
  await expect.poll(async () => rows.count()).toBeGreaterThan(before);

  const viewport = page.viewportSize();
  expect(viewport).toEqual({ width: 1280, height: 720 });
  const overflow = await page.evaluate(() => {
    const box = (selector: string): [number, number] => {
      const element = document.querySelector(selector);
      return element === null
        ? [-1, -1]
        : [element.scrollWidth - element.clientWidth, element.scrollHeight - element.clientHeight];
    };
    return {
      page: [
        document.documentElement.scrollWidth - document.documentElement.clientWidth,
        document.documentElement.scrollHeight - document.documentElement.clientHeight,
      ] as [number, number],
      canvas: box('[data-testid="canvas"]'),
      list: box('[data-testid="library-scroll"]'),
    };
  });
  expect(overflow.page).toEqual([0, 0]);
  expect(overflow.canvas).toEqual([0, 0]);
  // The list is the one thing that scrolls, and it does.
  expect(overflow.list[1]).toBeGreaterThan(0);
  // And the form beside it is all on the screen: its Patch key can be reached.
  await expect(page.getByTestId("draft-apply")).toBeInViewport();

  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});

/**
 * **A GDTF profile, end to end** — S61.
 *
 * The one test in this file that needs no installed library, and it is the one
 * that could not have any: GDTF's upstream has no anonymous download, so
 * nothing a CI job runs can fetch a published archive. What it can do is what
 * a venue does — drop a `.gdtf` into `fixtures/` inside the desk's data
 * directory (B43) — and everything after that is the real path: the real ZIP
 * reader, the real XML reader, the real daemon, the real browser.
 *
 * What is asserted is what GDTF buys that the Open Fixture Library did not:
 * the row says which format it came from, the form says the device has a model
 * and a beam, and the patched fixture takes the footprint the file states.
 */
test("a .gdtf dropped into the desk's own folder is patched, and says what it carries", async ({
  page,
}) => {
  const dataDir = mkdtempSync(join(tmpdir(), "prismdmx-gdtf-"));
  writeGdtf(join(dataDir, "fixtures", "anything.gdtf"), description("Robe Lighting", "Robin T1 E2E"));

  daemon = await startDaemon(PORT + 4, dataDir);
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();

  // The key is what the **file** says the fixture is, not what it is called:
  // the archive above is `anything.gdtf`.
  const key = "robe-lighting/robin-t1-e2e/mode-1";
  await page.getByTestId("library-search").fill("robin t1 e2e");
  const row = page.getByTestId(`library-row-${key}`);
  await expect(row).toBeVisible();

  // The Format column, and the Source column beside it: the venue's own GDTF.
  const format = page.getByTestId(`library-format-${key}`);
  await expect(format).toHaveText("GDTF");
  await expect(format).toHaveAttribute("data-gdtf", "yes");
  await expect(page.getByTestId(`library-source-${key}`)).toHaveText("yours");

  // Picking it says what the 3D viewer will have to draw with.
  await row.locator("td").nth(1).click();
  await expect(page.getByTestId("draft-physical")).toHaveText("3D model · 1 beam");

  // And it patches, at the footprint the file states: two bytes of pan, a
  // dimmer and a gobo wheel.
  await typeNumber(page, "draft-id", "1");
  await typeNumber(page, "draft-universe", "1");
  await typeNumber(page, "draft-address", "1");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-1")).toBeVisible();
  await expect(page.getByTestId("patch-row-1")).toContainText("Robin T1 E2E");
  await expect(page.getByTestId("patch-row-1")).toContainText("4");

  await daemon.kill();
  daemon = null;
  forget(dataDir);
});
