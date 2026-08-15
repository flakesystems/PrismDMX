/**
 * **S27's three exit criteria, in a browser, against a real daemon.**
 *
 * The unit tests make the same claims in jsdom against a fake socket and they
 * are the ones that run on every commit. This is the one that removes the
 * remaining doubt, because it is the only place where a rig is built the way an
 * operator builds one:
 *
 * 1. **A rig is patched, addressed and edited entirely from the interface** —
 *    starting from a show that has *nothing* in it, not even a profile.
 * 2. **An address conflict is shown before it is committed**, in the daemon's
 *    own words, with the command not yet sent.
 * 3. **The fixture sheet shows live values** — what the programmer holds and
 *    what is on the cable, which are two different things and are drawn two
 *    different ways.
 *
 * # Nothing here touches a device
 *
 * `CLAUDE.md`'s rule. The output is `--mock-output` and there is no console in
 * this file at all.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, startDaemon } from "./daemon.ts";

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
 * Searches the desk's library and embeds one profile out of it.
 *
 * The library is **downloaded at install time** (S44), so what is typed here is
 * matched against whatever this machine installed — the four built-in generic
 * profiles are the ones that are always there, whatever else is.
 */
async function embedProfile(page: Page, search: string, key: string): Promise<void> {
  const box = page.getByTestId("library-search");
  await box.click();
  await box.fill(search);
  await page.getByTestId(`library-${key}`).click();
}

/** Types a whole number into one of the patch form's fields. */
async function typeNumber(page: Page, testId: string, value: string): Promise<void> {
  const field = page.getByTestId(testId);
  await field.fill(value);
}

test("a rig is built, addressed and edited entirely from the interface", async ({ page }) => {
  const dataDir = await desk(page, PORT);
  await page.getByTestId("open-window").selectOption("Patch");
  await expect(page.getByTestId("patch")).toBeVisible();

  // 1. A brand-new show carries **no profiles at all**, so there is nothing to
  //    patch and the window says so rather than offering an empty menu. That
  //    is the state `Command::EmbedFixtureType` and the desk's own library
  //    exist for.
  await expect(page.getByTestId("patch-no-profiles")).toBeVisible();
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 0 profiles");

  // Take one out of the desk's library — which is **searched**, because an
  // installed desk knows two thousand profiles (S44). What comes back is a
  // `ShowPatch`: the show owns its copy of that profile from now on.
  await embedProfile(page, "generic rgbw", "generic.rgbw.par");
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 1 profiles");
  await expect(page.getByTestId("patch-no-profiles")).toHaveCount(0);

  // 2. Patch a fixture. Number, name, type, universe and address — the five
  //    fields of `PatchFixture`, all of them typed here.
  await page.getByRole("button", { name: "Add fixture" }).click();
  await typeNumber(page, "draft-name", "Front left");
  await typeNumber(page, "draft-address", "1");
  await expect(page.getByTestId("patch-preview")).toContainText("Free");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-1")).toContainText("Front left");
  await expect(page.getByTestId("fixtures")).toHaveText("1");

  // And a second one, clear of the first.
  await page.getByRole("button", { name: "Add fixture" }).click();
  await typeNumber(page, "draft-name", "Front right");
  await typeNumber(page, "draft-address", "10");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-2")).toContainText("Front right");

  // 3. **The conflict, before it is committed.** Fixture 2 is moved onto
  //    fixture 1's channels — and the daemon says so while the form is still
  //    open and nothing has been sent. The sentence is the daemon's own
  //    arithmetic: which channels are shared, and which fixture wins them.
  await page.getByTestId("patch-row-2").click();
  await typeNumber(page, "draft-address", "3");
  const preview = page.getByTestId("patch-preview");
  await expect(preview).toContainText("Overlaps 3–4 with fixture 1");
  await expect(preview).toContainText("higher fixture number wins");
  // The row has **not** moved: this is a question, not a command.
  await expect(page.getByTestId("patch-row-2")).toContainText("10");

  // An address that would be refused says so, and Apply goes dead — again
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
  // And now both rows are painted as sharing channels, which is the same
  // answer asked of the show as it stands.
  await expect(page.getByTestId("patch-row-1")).toHaveClass(/row-conflict/);
  await expect(page.getByTestId("patch-row-2")).toHaveClass(/row-conflict/);

  // 4. Editing: a name, then a **number**, which is its own command because
  //    the number is the key the patch is filed under.
  await page.getByTestId("patch-row-2").click();
  await typeNumber(page, "draft-name", "Front right (moved)");
  await typeNumber(page, "draft-address", "20");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-2")).toContainText("Front right (moved)");
  await expect(page.getByTestId("patch-row-1")).not.toHaveClass(/row-conflict/);

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
  await page.getByTestId("open-window").selectOption("Patch");
  await embedProfile(page, "generic rgbw", "generic.rgbw.par");
  await page.getByRole("button", { name: "Add fixture" }).click();
  await typeNumber(page, "draft-name", "Wash 1");
  await typeNumber(page, "draft-address", "1");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-1")).toBeVisible();

  // The sheet, on the Colour bank, which is where an RGBW PAR's parameters are.
  await page.getByTestId("open-window").selectOption("FixtureSheet");
  await expect(page.getByTestId("fixture-sheet")).toBeVisible();
  await page.getByTestId("bank-Color").click();
  await expect(page.getByTestId("sheet-bank")).toContainText("Color");

  // **Absent is not zero.** Nothing has been touched, so the programmer reads a
  // dash — not 0 %, which would mean something quite different.
  await expect(page.getByTestId("prog-1-Red")).toHaveText("—");
  // And nothing is on the cable either, so the output column is dark.
  expect(await litColumns(page)).toBe(0);

  // Now put red on it from the command line (S26's parser).
  await page.getByTestId("command-input").fill("1 red at 100");
  await page.getByTestId("command-input").press("Enter");

  // The programmer column is a reader over `ProgrammerChanged` and renders in
  // React like everything else.
  await expect(page.getByTestId("prog-1-Red")).toHaveText("100%");
  await expect(page.getByTestId("prog-1-Green")).toHaveText("—");

  // And the **output** column is telemetry, drawn on a canvas outside React —
  // counted here off the pixels rather than read out of a readout that could
  // agree with itself.
  await expect.poll(async () => litColumns(page), { timeout: 15_000 }).toBeGreaterThan(0);

  // Clearing the programmer takes it off both.
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
  const dataDir = await desk(page, PORT + 2);
  await page.getByTestId("open-window").selectOption("Patch");
  await embedProfile(page, "generic dimmer", "generic.dimmer");
  for (let id = 1; id <= 40; id += 1) {
    await page.getByRole("button", { name: "Add fixture" }).click();
    // **The number is typed rather than taken from the form's suggestion**, and
    // that is what makes this loop deterministic. `nextFreeFixtureId` proposes
    // the lowest number *the client currently holds no fixture for*, and the
    // client holds what the daemon has sent it — so on a machine where the
    // round trip is slower than the next click, two drafts get the same number
    // and the second patch is a **repatch** of the first. Thirty-nine fixtures,
    // then, and a test that failed for a reason that has nothing to do with
    // scrolling. The suggestion is a convenience an operator types over, and
    // this is a test typing over it.
    await typeNumber(page, "draft-id", String(id));
    await typeNumber(page, "draft-address", String(id));
    await page.getByTestId("draft-apply").click();
  }
  await expect(page.getByTestId("fixtures")).toHaveText("40");
  await page.getByTestId("open-window").selectOption("FixtureSheet");
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
      executors: box('[data-testid="executor-bar"]'),
      encoders: box('[data-testid="encoder-bar"]'),
      // The one place that *is* allowed to scroll, and has to: forty rows do
      // not fit in a window.
      sheet: box('[data-testid="sheet-scroll"]'),
    };
  });
  expect(overflow.page).toEqual([0, 0]);
  expect(overflow.canvas).toEqual([0, 0]);
  expect(overflow.executors).toEqual([0, 0]);
  expect(overflow.encoders).toEqual([0, 0]);
  expect(overflow.sheet[1]).toBeGreaterThan(0);

  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});

/**
 * How many pixels of the fixture sheet's output column are not background.
 *
 * The column is drawn one bar per channel, so a level on the rig is a run of
 * coloured pixels on that canvas. Counting them is the only way to say *the
 * value reached the output* about a picture rather than about a readout that
 * could agree with itself.
 */
async function litColumns(page: Page): Promise<number> {
  return page.getByTestId("sheet-canvas").evaluate((element) => {
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
      // `#4bb3c4` below full and `#ffd479` at it — the two colours a channel
      // that is up is drawn in, and neither the background nor the trough.
      const red = image.data[at] ?? 0;
      const green = image.data[at + 1] ?? 0;
      const blue = image.data[at + 2] ?? 0;
      if ((red === 75 && green === 179 && blue === 196) || (red === 255 && green === 212 && blue === 121)) {
        found += 1;
      }
    }
    return found;
  });
}

test("a real fixture out of the Open Fixture Library is searched, embedded and patched", async ({
  page,
}) => {
  // **The whole point of S44**, end to end: an operator types the name printed
  // on the light and gets the manufacturer's channel order.
  //
  // The library is downloaded at install time, so this test says why it is
  // skipping rather than failing on a machine that has not installed it — CI
  // installs it, so it runs there on every commit.
  const dataDir = await desk(page, PORT + 3);
  await page.getByTestId("open-window").selectOption("Patch");

  const box = page.getByTestId("library-search");
  await box.click();
  await box.fill("stage wash 7x10");
  const match = page.getByTestId("library-stage-right/stage-wash-7x10w-led-moving-head/9ch");
  if ((await match.count()) === 0) {
    test.skip(true, "no fixture library is installed - run tools/fetch-fixtures");
    return;
  }
  await match.click();
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 1 profiles");

  await page.getByRole("button", { name: "Add fixture" }).click();
  await typeNumber(page, "draft-name", "Head 1");
  await typeNumber(page, "draft-address", "1");
  // Nine channels, which is what that mode is — and the daemon says so before
  // the patch, out of a profile it read from a JSON file this repository does
  // not contain.
  await expect(page.getByTestId("patch-preview")).toContainText("9 channels");
  await expect(page.getByTestId("patch-preview")).toContainText("ending at 9");
  await page.getByTestId("draft-apply").click();

  const row = page.getByTestId("patch-row-1");
  await expect(row).toContainText("Head 1");
  await expect(row).toContainText("Stage Wash");
  await expect(row).toContainText("9");

  await daemon?.kill();
  daemon = null;
  forget(dataDir);
});
