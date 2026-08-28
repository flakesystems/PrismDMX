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
import { buildDaemon, forget, openWindow, startDaemon } from "./daemon.ts";

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
 * Searches the desk's library and sets the **open row** to one profile out of
 * it, embedding it in the show on the way.
 *
 * The library is **downloaded at install time** (S44), so what is typed here is
 * matched against whatever this machine installed — the four built-in generic
 * profiles are the ones that are always there, whatever else is.
 *
 * **It needs a row open, which is S43's change of order.** The search was in
 * the toolbar and embedded a profile on its own; B19 folded it into the Type
 * field and B23 made that field a panel, so a rig is now built by opening a row
 * and choosing what it is, rather than by embedding something first and only
 * then being allowed to open a row.
 */
async function chooseProfile(page: Page, search: string, key: string): Promise<void> {
  await page.getByTestId("library-open").click();
  await page.getByTestId("library-search").fill(search);
  await page.getByTestId(`library-${key}`).click();
}

/** Types a whole number into one of the patch form's fields. */
async function typeNumber(page: Page, testId: string, value: string): Promise<void> {
  const field = page.getByTestId(testId);
  await field.fill(value);
}

test("a rig is built, addressed and edited entirely from the interface", async ({ page }) => {
  const dataDir = await desk(page, PORT);
  await openWindow(page, "Patch");
  await expect(page.getByTestId("patch")).toBeVisible();

  // 1. A brand-new show carries **no profiles at all**, and the count says so.
  //    The window used to say it a second time in a sentence of its own; that
  //    note went with the toolbar's library search in B19, because there is no
  //    longer an empty menu for it to warn about — the row opens either way
  //    and the profile is searched for inside it.
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 0 profiles");

  // 2. Patch a fixture. Number, name, type, universe and address — the five
  //    fields of `PatchFixture`, all of them typed here. The row opens on
  //    **nothing chosen**, which is an ordinary state now rather than a window
  //    that cannot be used.
  await page.getByRole("button", { name: "Add fixture" }).click();
  await expect(page.getByTestId("draft-type")).toHaveText("No profile chosen");

  // The type comes out of the desk's library — which is **searched**, because
  // an installed desk knows two thousand profiles (S44). What comes back is a
  // `ShowPatch`: the show owns its copy of that profile from now on.
  await chooseProfile(page, "generic rgbw", "generic.rgbw.par");
  await expect(page.getByTestId("patch-count")).toHaveText("0 fixtures · 1 profiles");

  await typeNumber(page, "draft-name", "Front left");
  await typeNumber(page, "draft-address", "1");
  await expect(page.getByTestId("patch-preview")).toContainText("Free");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-1")).toContainText("Front left");
  // The count is a `Status` reading, which S43 moved off the bottom bar and
  // into a window of its own.
  await openWindow(page, "Status");
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
  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();
  await chooseProfile(page, "generic rgbw", "generic.rgbw.par");
  await typeNumber(page, "draft-name", "Wash 1");
  await typeNumber(page, "draft-address", "1");
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
  await openWindow(page, "Patch");
  // One row is opened only to put the dimmer into the show and is then thrown
  // away: picking a profile embeds it, and every row opened after this one
  // starts on it, so the loop below never has to touch the library.
  await page.getByRole("button", { name: "Add fixture" }).click();
  await chooseProfile(page, "generic dimmer", "generic.dimmer");
  await page.getByTestId("draft-cancel").click();
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
      // the header, the command line and the programmer band. The two probes
      // that named the old bars were measuring elements that no longer exist,
      // which is a check that passes for the wrong reason.
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
  await openWindow(page, "Patch");

  // A row has to be opened first: the library is chosen **in the form**, for
  // the fixture being patched (B19), and it is a panel of its own (B23).
  await page.getByText("Add fixture").click();
  await page.getByTestId("library-open").click();
  await page.getByTestId("library-search").fill("stage wash 7x10");
  const match = page.getByTestId("library-stage-right/stage-wash-7x10w-led-moving-head/9ch");

  // **Whether a library is installed is the daemon's answer, not the search's.**
  // The search is a `Query` round trip and `locator.count()` does *not* wait, so
  // counting the matches asks "has the answer arrived yet" and reads the answer
  // *no* as "there is no library". On 2026-08-19 that skipped this test on CI
  // with 634 fixtures installed, and a skip is silent. The **count beside the
  // search** carries the number the snapshot brought, which is a fact by the
  // time the panel is open; the match itself is then waited for like anything
  // else.
  const shown = (await page.getByTestId("library-count").textContent()) ?? "";
  const installed = /of (\d+) profiles/.exec(shown);
  if (installed === null || installed[1] === "0") {
    test.skip(true, "no fixture library is installed - run tools/fetch-fixtures");
    return;
  }
  await expect(match).toBeVisible();
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
