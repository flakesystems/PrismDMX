/**
 * **S26's exit criteria, in a browser, against a real daemon and a real
 * console.**
 *
 * The unit suite makes the same claims in jsdom against a fake socket and runs
 * on every commit. Two of them can only be *observed* here:
 *
 * 1. **Paging at the X-Touch and paging in the interface agree.** Not "both
 *    send `SetExecutorPage`" — the console presses `Faderbank ▶`, the browser
 *    is doing nothing, and the bar moves; then the browser pages and the
 *    console's next press carries on from there. One page number, two hands on
 *    it.
 * 2. **An encoder change reaches the programmer and appears in the output
 *    within a tick.** Command out, `ProgrammerChanged` back, and the level
 *    **in the picture** — read off the telemetry canvas as the exact colour
 *    `LevelPainter` draws that level in, not as a readout that agrees with
 *    itself.
 *
 * And the one that ties the two bars to the console's own idea of the desk:
 * the encoder bank and the highlighted parameter follow the console's Encoder
 * Assign and `Zoom ◀▶`, and **the jog wheel then turns the parameter that is
 * highlighted** — which is S22's open warning, watched rather than argued.
 *
 * # Nothing here touches a device
 *
 * `CLAUDE.md`'s rule. The console is `--mock-surface`: a file the daemon reads
 * MIDI from, which the three layers of `prism-surface` cannot tell from a port.
 * The output is `--mock-output`. The rig is `ui/tests/fixtures/desk-rig.prism`,
 * written by `crates/prismd/tests/ui_programmer.rs` and **dark at home**, so a
 * level in the picture is one this test put there.
 */

import { expect, test } from "@playwright/test";
import { join } from "node:path";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, pressConsole, showFixture, startDaemon, turnConsoleWheel } from "./daemon.ts";

/** A port of this spec's own, so a daemon on 7373 is neither used nor disturbed. */
const PORT = 7393;

/** `Faderbank ◀`, note 46 — `docs/MCU_MAPPING.md` §2.1. D7 makes it the executor page. */
const BANK_LEFT = 46;

/** `Faderbank ▶`, note 47. */
const BANK_RIGHT = 47;

/** Encoder Assign / Pan, note 42 — the default profile's Colour bank. */
const ASSIGN_COLOR = 42;

/** Cursor ▶, note 99 — `XTouch.txt`'s `Zoom ▶`: the next programmer parameter. */
const CURSOR_RIGHT = 99;

/** Cursor ▲, note 96 — `Zoom ▲`: the previous programmer page (§4.1). */
const ZOOM_UP = 96;

/** Cursor ▼, note 97 — `Zoom ▼`: the next programmer page. */
const ZOOM_DOWN = 97;

/**
 * The colour `LevelPainter` draws a channel at 127 in.
 *
 * `colourOf(127)` — the top of the cool half of the ramp, worked out from
 * `src/telemetry/painter.ts` by hand rather than imported, because a test that
 * asked the painter what it drew would be asking the code under test.
 * `at 50` is level 32 767, which the engine encodes to the byte 127.
 */
const HALF = { red: 64, green: 200, blue: 185 };

let daemon: Daemon | null = null;
let dataDir: string | null = null;

test.beforeAll(() => {
  buildDaemon();
});

test.afterEach(async () => {
  await daemon?.kill();
  daemon = null;
  if (dataDir !== null) {
    forget(dataDir);
    dataDir = null;
  }
});

/** Starts a daemon on the desk rig, with a console it reads from a file. */
async function deskDaemon(port: number): Promise<{ daemon: Daemon; keys: string }> {
  const fixture = showFixture("desk-rig.prism");
  dataDir = fixture.dataDir;
  const keys = join(process.cwd(), "test-results", `console-${String(Date.now())}.midi`);
  const started = await startDaemon(port, fixture.dataDir, {
    show: fixture.show,
    mockSurface: keys,
  });
  return { daemon: started, keys };
}

test("**paging agrees**: the console and the executor bar move one page number", async ({
  page,
}) => {
  const started = await deskDaemon(PORT);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // **S43 moved these into windows.** The executor strip, the console's
  // keys and the readings were bands around the canvas; the owner's
  // skeleton has none, so they are windows an operator opens.
  await openWindow(page, "Status");
  await openWindow(page, "Executors");

  // The rig's page 0: executor 0 plays "Warm Wash" and executor 2 plays
  // "Cold Wash". Eight strips, whatever is on them.
  await expect(page.getByTestId("executor-window").locator("[data-executor]")).toHaveCount(8);
  await expect(page.getByTestId("page-number")).toHaveText("0");
  await expect(page.getByTestId("name-0")).toHaveText("Warm Wash");
  await expect(page.getByTestId("name-2")).toHaveText("Cold Wash");
  await expect(page.getByTestId("strip-0")).toHaveAttribute("data-executor", "0");

  // **The press.** Three bytes appended to a file by a process that is neither
  // this browser nor this daemon. Nothing in the page is touched.
  pressConsole(started.keys, BANK_RIGHT);

  await expect(page.getByTestId("page-number")).toHaveText("1");
  await expect(page.getByTestId("strip-0")).toHaveAttribute("data-executor", "8");
  await expect(page.getByTestId("strip-1")).toHaveAttribute("data-executor", "9");
  // Executor 9 is assigned with no sequence on it, so it shows a dash rather
  // than a name, and the other seven slots are empty.
  await expect(page.getByTestId("name-1")).toHaveText("—");
  await expect(page.getByTestId("executor-window").locator('[data-assigned="yes"]')).toHaveCount(1);

  // Now the *interface* pages, and the console's next press carries on from
  // where the browser left it — which is the criterion: one page number.
  await page.getByTestId("page-up").click();
  await expect(page.getByTestId("page-number")).toHaveText("2");
  await expect(page.getByTestId("executor-window").locator('[data-assigned="yes"]')).toHaveCount(0);

  pressConsole(started.keys, BANK_LEFT);
  await expect(page.getByTestId("page-number")).toHaveText("1");
  pressConsole(started.keys, BANK_LEFT);
  await expect(page.getByTestId("page-number")).toHaveText("0");
  await expect(page.getByTestId("name-0")).toHaveText("Warm Wash");
  // And the status strip, which reads the same field by a different pointer.
  await expect(page.getByTestId("executor-page")).toHaveText("0");
});

test("**the wheel turns what is highlighted**: the encoder bar follows the console", async ({
  page,
}) => {
  const started = await deskDaemon(PORT + 1);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // **S43 moved these into windows.** The executor strip, the console's
  // keys and the readings were bands around the canvas; the owner's
  // skeleton has none, so they are windows an operator opens.
  await openWindow(page, "Status");
  await openWindow(page, "Executors");

  // Select the moving head, which is the only fixture with colour on it.
  await page.getByTestId("command-input").fill("5");
  await page.getByTestId("command-input").press("Enter");
  await expect(page.getByTestId("selection")).toHaveText("5");

  // **The console switches the encoder bank.** Encoder Assign / Pan is the
  // Colour bank in the shipped profile.
  await expect(page.getByTestId("bank-Dimmer")).toHaveAttribute("data-active", "yes");
  pressConsole(started.keys, ASSIGN_COLOR);
  await expect(page.getByTestId("bank-Color")).toHaveAttribute("data-active", "yes");
  await expect(page.getByTestId("encoder-Red")).toHaveAttribute("data-selected", "yes");
  await expect(page.getByTestId("encoder-Green")).toHaveAttribute("data-selected", "no");

  // **And the parameter.** `Zoom ▶` is the next parameter of the bank, and the
  // bar highlights it — which is only true because the interface's table and
  // `prismd::surface::parameter_of` are the same table.
  pressConsole(started.keys, CURSOR_RIGHT);
  await expect(page.getByTestId("encoder-Green")).toHaveAttribute("data-selected", "yes");
  await expect(page.getByTestId("param-index")).toHaveText("1");
  // **Untouched is a mark, not a dash** — S43, punch-list B1. An encoder used to
  // read `—` when the programmer held nothing; it reads the attribute's
  // **resting value** now (a colour rests open, so Green reads 0 %), and *is
  // this one mine?* is `data-overriding` instead. The claim is unchanged: the
  // programmer holds nothing on Green yet.
  await expect(page.getByTestId("encoder-Green")).toHaveAttribute("data-overriding", "no");

  // **The wheel.** Ten detents clockwise, one message each, into the parameter
  // the interface says is highlighted. If the two orders disagreed, Red would
  // move and Green would not.
  for (let detent = 0; detent < 10; detent += 1) {
    turnConsoleWheel(started.keys, 1);
  }
  await expect(page.getByTestId("encoder-Green")).toHaveAttribute("data-overriding", "yes");
  await expect(page.getByTestId("encoder-Red")).toHaveAttribute("data-overriding", "no");
  await expect(page.getByTestId("bank-Color")).toHaveAttribute("data-touched", "yes");
});

test("**an encoder change reaches the programmer and the output within a tick**", async ({
  page,
}) => {
  const started = await deskDaemon(PORT + 2);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // **S43 moved these into windows.** The executor strip, the console's
  // keys and the readings were bands around the canvas; the owner's
  // skeleton has none, so they are windows an operator opens.
  await openWindow(page, "Status");
  await openWindow(page, "Executors");

  // The picture of the rig, which is what "in the output" means. The rig is
  // dark at home, so nothing on it is lit before this test lights it.
  await openWindow(page, "DmxSheet");
  await expect(page.getByTestId("telemetry")).toBeVisible();
  await expect(page.getByTestId("telemetry-stats")).toContainText("universes", {
    timeout: 20_000,
  });
  expect(await litPixels(page, HALF)).toBe(0);

  // The command. The clock starts at **Enter** rather than at the typing: what
  // is being measured is the round trip, and `fill` is a Playwright round trip
  // of its own that has nothing to do with the daemon.
  await page.getByTestId("command-input").fill("1 thru 3 at 50");
  const started_at = Date.now();
  await page.getByTestId("command-input").press("Enter");

  // 1. `ProgrammerChanged` came back: three values, on three fixtures.
  //    Polled every 10 ms rather than at the default interval, because the
  //    number below is a measurement and not a pass mark — a coarse poll would
  //    be measuring Playwright.
  await expect
    .poll(async () => page.getByTestId("touched").textContent(), {
      timeout: 10_000,
      intervals: [10],
    })
    .toBe("3");
  const programmerMs = Date.now() - started_at;
  await expect(page.getByTestId("selection")).toHaveText("1 + 2 + 3");

  // 2. And the level is **in the picture** — the exact colour the painter
  //    draws a channel at 127 in, which is what the engine encodes 32 767 to.
  await expect
    .poll(async () => litPixels(page, HALF), { timeout: 10_000, intervals: [25] })
    .toBeGreaterThan(0);
  const outputMs = Date.now() - started_at;

  const line = `programmer ${String(programmerMs)} ms · output ${String(outputMs)} ms`;
  test.info().annotations.push({ type: "programmer", description: line });
  console.log(`[desk] ${line}`);

  // A tick is 22.7 ms and telemetry publishes at 30 Hz, so the honest bound on
  // *observing* it is a frame or two plus a round trip. The generous number
  // here is what a build server under load can be held to; the figure printed
  // above is the one recorded in `PROGRESS.md`.
  expect(outputMs).toBeLessThan(1000);

  // 3. And the encoder bar says so too, on the bank the value landed on.
  await expect(page.getByTestId("bank-Dimmer")).toHaveAttribute("data-touched", "yes");
  await expect(page.getByTestId("value-Dimmer")).toHaveText("50%");

  // Clearing it takes the light off the rig again — the three-stage Clear's
  // first stage, and the picture following the programmer in both directions.
  await page.getByTestId("clear").click();
  await expect(page.getByTestId("touched")).toHaveText("0");
  await expect.poll(async () => litPixels(page, HALF), { timeout: 10_000 }).toBe(0);
});

test("the console line is the session's, and a syntax error is a message", async ({ page }) => {
  const started = await deskDaemon(PORT + 3);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // **S43 moved these into windows.** The executor strip, the console's
  // keys and the readings were bands around the canvas; the owner's
  // skeleton has none, so they are windows an operator opens.
  await openWindow(page, "Status");
  await openWindow(page, "Executors");

  // Typed text reaches the session, so every other client and the scribble
  // strips see it. It is *not* executed until Enter.
  await page.getByTestId("command-input").fill("1 thru 4 at ");
  await expect(page.getByTestId("command-line")).toHaveText("1 thru 4 at ", { timeout: 10_000 });
  await expect(page.getByTestId("command-reading")).toContainText("percentage");
  await expect(page.getByTestId("touched")).toHaveText("0");

  // Enter on a line that will not parse sends no command and does not throw:
  // the interface is still here and still connected afterwards.
  await page.getByTestId("command-input").press("Enter");
  await expect(page.getByTestId("touched")).toHaveText("0");
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("command-reading")).toContainText("percentage");

  // A line the daemon refuses is the other half: the parser is right and the
  // refusal is the daemon's. Fixture 9 is not in this rig.
  await page.getByTestId("command-input").fill("9");
  await page.getByTestId("command-input").press("Enter");
  await expect(page.getByTestId("notices")).toContainText("9");
  await expect(page.getByTestId("selection")).toHaveText("—");
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
});

test("**paging the encoders agrees**: the console's `Zoom ▲▼` and the bar move one page", async ({
  page,
}) => {
  // **S35's second exit criterion, observed rather than asserted.** The two page
  // controls in the encoder bar and the two keys on the console change the same
  // `programmerPage`, so a browser that had its own idea of the page would drift
  // from the desk within two presses. Nothing here reads a page out of the
  // interface's own state: every number below is drawn from a `SessionPatch`.
  const started = await deskDaemon(PORT + 5);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // **S43 moved these into windows.** The executor strip, the console's
  // keys and the readings were bands around the canvas; the owner's
  // skeleton has none, so they are windows an operator opens.
  await openWindow(page, "Status");
  await openWindow(page, "Executors");

  // Dimmer has one parameter, so there is one page and nowhere to go.
  await expect(page.getByTestId("bank-Dimmer")).toHaveAttribute("data-active", "yes");
  await expect(page.getByTestId("programmer-page")).toHaveText("1/1");
  await expect(page.getByTestId("encoder-page-down")).toBeDisabled();
  await expect(page.getByTestId("encoder-page-up")).toBeDisabled();

  // **The press.** Three bytes appended to a file by a process that is neither
  // this browser nor this daemon: Encoder Assign / Pan, which the default
  // profile binds to the Colour bank.
  //
  // **It was the Beam bank until S43**, which is the bank the owner's drawing
  // took apart: `Beam` was six knobs — gobo, prism, iris, zoom, shutter and
  // control — and is three now, with `Gobo` and `Control` banks of their own
  // (`AttributeType::feature_group`). Three knobs is one page and no paging to
  // observe, so the second page this test needs is Colour's: five parameters,
  // four drawn and one over.
  pressConsole(started.keys, ASSIGN_COLOR);
  await expect(page.getByTestId("bank-Color")).toHaveAttribute("data-active", "yes");

  // Colour has five parameters, so four are drawn and there is a second page.
  await expect(page.getByTestId("encoders").locator("[data-index]")).toHaveCount(4);
  await expect(page.getByTestId("encoder-Red")).toBeVisible();
  await expect(page.getByTestId("encoder-White")).toBeVisible();
  await expect(page.getByTestId("encoder-Amber")).toHaveCount(0);
  await expect(page.getByTestId("programmer-page")).toHaveText("1/2");
  await expect(page.getByTestId("encoder-page-down")).toBeEnabled();

  // **`Zoom ▼` on the console pages the browser.**
  pressConsole(started.keys, ZOOM_DOWN);
  await expect(page.getByTestId("programmer-page")).toHaveText("2/2");
  await expect(page.getByTestId("encoders").locator("[data-index]")).toHaveCount(1);
  await expect(page.getByTestId("encoder-Amber")).toBeVisible();
  await expect(page.getByTestId("encoder-Red")).toHaveCount(0);

  // Now the *interface* pages, and the console's next press carries on from
  // where the browser left it — which is the criterion: **one page number**.
  await page.getByTestId("encoder-page-up").click();
  await expect(page.getByTestId("programmer-page")).toHaveText("1/2");
  await expect(page.getByTestId("encoder-Red")).toBeVisible();

  pressConsole(started.keys, ZOOM_DOWN);
  await expect(page.getByTestId("programmer-page")).toHaveText("2/2");
  pressConsole(started.keys, ZOOM_UP);
  await expect(page.getByTestId("programmer-page")).toHaveText("1/2");
  await expect(page.getByTestId("encoder-page-up")).toBeDisabled();
});

/**
 * **`CLAUDE.md`'s device screen, and S43's punch-list B7** — rewritten twice
 * over.
 *
 * S25 checked the page and the canvas with two bands beside them. S43 took the
 * bands away: the executor strip, the console's keys and the readings are
 * windows now, and the screen is a header, a canvas, the command line and the
 * programmer band. So the shape of the check moved with it — what is asserted
 * is the same sentence it always was, *nothing scrolls outside the canvas*, over
 * an interface that has almost nothing outside the canvas left.
 *
 * The entry itself is about the **programmer**: parts of it ran off the edge of
 * the screen and were cut off, on the menu bar and on the colour and beam menus.
 * A band that overflows is a band with a scrollbar or a band with something
 * unreachable in it, and both read zero here.
 *
 * **Every window type at once**, which is the session's own exit criterion, and
 * at two sizes: 1280 x 720 is the smallest screen this is meant for, and 4K is
 * where a layout in percentages can go wrong in the other direction.
 */
for (const viewport of [
  { name: "1280 x 720", width: 1280, height: 720 },
  { name: "4K", width: 3840, height: 2160 },
]) {
  test(`the screen is a device screen at ${viewport.name}, with every window open`, async ({
    page,
  }) => {
    const started = await deskDaemon(PORT + 4);
    daemon = started.daemon;
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
    await expect(page.getByTestId("connection-status")).toHaveText("Connected");

    // **Every type the build has**, out of the chooser rather than out of a list
    // written down here: `WINDOW_TYPE_VARIANTS` is generated from
    // `prism_domain::WindowType`, so a type added later is covered by this test
    // the day it exists. The daemon places them (B10) and refuses the ones there
    // is no room for, which is itself the answer to *does this fit*.
    await page.keyboard.press("Insert");
    const types = await page.locator('[data-testid^="picker-"]').evaluateAll((nodes) =>
      nodes
        .map((node) => node.getAttribute("data-testid") ?? "")
        .filter((id) => id !== "picker-cancel")
        .map((id) => id.replace("picker-", "")),
    );
    expect(types.length).toBeGreaterThan(10);
    for (const type of types) {
      await page.keyboard.press("Insert");
      const key = page.getByTestId(`picker-${type}`);
      if ((await key.count()) > 0) {
        await key.click();
      }
    }
    await expect(page.locator("[data-window-type]").first()).toBeVisible();

    const overflow = await page.evaluate(() => {
      const scroll = (element: Element | null) =>
        element === null
          ? null
          : [element.scrollWidth - element.clientWidth, element.scrollHeight - element.clientHeight];
      const bodies = [...document.querySelectorAll("[data-window-type]")].map((element) => ({
        type: element.getAttribute("data-window-type") ?? "",
        overflow: scroll(element),
      }));
      return {
        page: scroll(document.documentElement),
        body: scroll(document.body),
        canvas: scroll(document.querySelector('[data-testid="canvas"]')),
        programmer: scroll(document.querySelector('[data-testid="programmer-band"]')),
        line: scroll(document.querySelector('[data-testid="command-line-panel"]')),
        windows: bodies,
      };
    });

    // Nothing outside the canvas scrolls, in either direction.
    expect(overflow.page).toEqual([0, 0]);
    expect(overflow.body).toEqual([0, 0]);
    expect(overflow.canvas).toEqual([0, 0]);
    // **B7**: the programmer band, which is the one the entry is about.
    expect(overflow.programmer).toEqual([0, 0]);
    expect(overflow.line).toEqual([0, 0]);
    // And a window frame does not scroll either — what scrolls is the body
    // *inside* it, one scroller per window, which is the rule S37 wrote down.
    for (const window of overflow.windows) {
      expect(window.overflow, `${window.type} scrolls in its frame`).toEqual([0, 0]);
    }
  });
}

/**
 * How many pixels of the telemetry canvas are exactly this colour.
 *
 * The painter blits one pixel per channel through a 256-entry palette with
 * smoothing off, so a level that is on the rig is on the canvas in exactly the
 * colour that level maps to. Counting them is the only way to say *the value
 * reached the output* about a picture rather than about a readout.
 */
async function litPixels(
  page: import("@playwright/test").Page,
  colour: { red: number; green: number; blue: number },
): Promise<number> {
  return page.getByTestId("telemetry-canvas").evaluate((element, wanted) => {
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
      if (
        image.data[at] === wanted.red &&
        image.data[at + 1] === wanted.green &&
        image.data[at + 2] === wanted.blue
      ) {
        found += 1;
      }
    }
    return found;
  }, colour);
}
