/**
 * **S34's exit criteria, in a browser, against a real daemon and a real
 * console.**
 *
 * The unit suites make the same claims — `prism-engine` on frames,
 * `prism-core` on effects, `prismd` on the wire, and the browser's own tests in
 * jsdom — and those run on every commit. Three of them are only *observed*
 * here, because each needs the whole chain at once:
 *
 * 1. **A `Flash` is a layer, not a master move.** Held, the strip's cue is on
 *    the rig at full; released, the light goes and the stored master reads what
 *    it read before, to the percent. The strip's fader sits at **0 %**, so a
 *    flash that had written into the master would leave the fader somewhere
 *    else and the light on.
 * 2. **A `Toggle` latches, and it is the daemon that decides which way.** The
 *    same key, pressed twice: on, then off. Nothing in the browser reads
 *    `isActive` to work that out — the mutation check in
 *    `desk.test.tsx` is what says so, and this is the same claim from outside.
 * 3. **The cue number is the tick's answer.** S26 and S28 both had to draw a
 *    dash. Here the strip shows `Q1` while the list runs and a dash when it
 *    does not — and the run is started **from the console**, so the number
 *    cannot be a count of anything this browser sent.
 *
 * # Nothing here touches a device
 *
 * `CLAUDE.md`'s rule. The console is `--mock-surface`: a file the daemon reads
 * MIDI from. The output is `--mock-output`. The rig is
 * `ui/tests/fixtures/desk-rig.prism`, written by
 * `crates/prismd/tests/ui_programmer.rs` and **dark at home**, so a level in
 * the picture is one this test put there — and executor 2 carries `Flash`,
 * `Toggle`, `On` and `LearnSpeed` on its four keys, which is the strip S26 had
 * to draw disabled.
 */

import { expect, test } from "@playwright/test";
import { join } from "node:path";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, pressConsole, showFixture, startDaemon } from "./daemon.ts";

/** A port of this spec's own, so no other suite's daemon is disturbed. */
const PORT = 7401;

/** `Play`, note 94 — `docs/MCU_MAPPING.md` §2.1. §4.1 gives it `On`. */
const PLAY = 94;

/** `Stop`, note 93. §4.1 gives it `Off`. */
const STOP = 93;

/**
 * The colour `LevelPainter` fills a universe's **peak meter** with when
 * something in it is at full — `INK.meter`, `#ffd479`.
 *
 * Worked out from `src/telemetry/painter.ts` by hand rather than imported,
 * because a test that asked the painter what it drew would be asking the code
 * under test.
 *
 * The meter rather than the grid, and that is the point of this rig: **one**
 * channel of five hundred and twelve is lit here, so the grid gives it a
 * fraction of a pixel and what lands on the canvas is a blend. The meter is a
 * solid block whose colour says *this universe has a channel at 250 or more*,
 * which is exactly the claim a flash makes. `looks.spec.ts` counts grid pixels
 * instead, because its rig lights three fixtures at once.
 */
const METER_FULL = { red: 255, green: 212, blue: 121 };

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

/** How many pixels of the telemetry canvas are exactly this colour. */
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

test("**a flash is a layer**: held it lights the rig, released it gives the master back", async ({
  page,
}) => {
  const started = await deskDaemon(PORT);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // **S43 made the strip a window.** It was a band under the canvas; the
  // owner's skeleton has no bands, so an operator opens it. The claims below
  // are the same ones, read off the same strip.
  await openWindow(page, "Executors");

  // Strip 2 is the one S26 had to draw four disabled buttons on. All four are
  // pressable now, and their labels are the show's.
  await expect(page.getByTestId("button-2-0")).toHaveAttribute("data-function", "Flash");
  await expect(page.getByTestId("button-2-1")).toHaveAttribute("data-function", "Toggle");
  await expect(page.getByTestId("button-2-2")).toHaveAttribute("data-function", "On");
  await expect(page.getByTestId("button-2-3")).toHaveAttribute("data-function", "LearnSpeed");
  await expect(page.getByTestId("button-2-0")).toBeEnabled();

  // **Its master is at zero**, which is what makes this a test of the flash
  // rather than of a fader: the cue could not put light on the rig by itself.
  await expect(page.getByTestId("percent-2")).toHaveText("0%");

  await openWindow(page, "DmxSheet");
  await expect.poll(async () => litPixels(page, METER_FULL), { timeout: 15_000 }).toBe(0);

  // Held. The mouse goes down and stays down: a click would be press and
  // release in one gesture, and a momentary function has two halves.
  const flash = page.getByTestId("button-2-0");
  await flash.hover();
  await page.mouse.down();
  await expect.poll(async () => litPixels(page, METER_FULL), { timeout: 15_000 }).toBeGreaterThan(0);
  // And the stored master has not moved while it is held — this is the byte the
  // exit criterion is about, read where an operator reads it.
  await expect(page.getByTestId("percent-2")).toHaveText("0%");

  // Released.
  await page.mouse.up();
  await expect.poll(async () => litPixels(page, METER_FULL), { timeout: 15_000 }).toBe(0);
  await expect(page.getByTestId("percent-2")).toHaveText("0%");

  // A reload asks the daemon what it holds, with nothing of this browser's in
  // the answer: the master is still what it was, so the flash left no trace.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("percent-2")).toHaveText("0%");
});

test("**a toggle latches**, and the cue number comes back from the tick", async ({ page }) => {
  const started = await deskDaemon(PORT + 1);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await openWindow(page, "Executors");

  // Executor 0 plays "Warm Wash" and it is stopped: a dash rather than a
  // number, because a stopped playback is on no cue.
  await expect(page.getByTestId("name-0")).toHaveText("Warm Wash");
  await expect(page.getByTestId("cue-0")).toHaveText("—");

  // Strip 2's first key is a `Toggle`. One press starts it — and the number
  // that appears is the **tick's**, arriving a poll after the command rather
  // than with it.
  await page.getByTestId("button-2-1").click();
  await expect(page.getByTestId("strip-2").locator(".strip-running")).toHaveCount(1);
  await expect(page.getByTestId("cue-2")).toHaveText("Q1");

  // The same key again stops it. Nothing in this browser worked out which way
  // it should go: the command carries the button's *position*, and the daemon
  // reads `isActive`.
  await page.getByTestId("button-2-1").click();
  await expect(page.getByTestId("strip-2").locator(".strip-running")).toHaveCount(0);
  await expect(page.getByTestId("cue-2")).toHaveText("—");

  // **And from the console, on the other executor.** §4.1 gives Play `On` and
  // Stop `Off` on the *selected* executor — two of the three rows that could
  // not be bound as written until S34 — so this selects executor 0 and drives
  // it with three bytes appended to a file by neither the browser nor the
  // daemon. The cue number that appears is therefore a number **no client
  // asked for**: the console pressed, the tick answered, and the browser drew.
  await page.getByTestId("select-0").click();
  await expect(page.getByTestId("strip-0")).toHaveAttribute("data-selected", "yes");

  pressConsole(started.keys, PLAY);
  await expect(page.getByTestId("cue-0")).toHaveText("Q1");
  await expect(page.getByTestId("strip-0").locator(".strip-running")).toHaveCount(1);

  // Pressed again: `On` does not restart a list that is already running, so the
  // desk stays exactly where it is.
  pressConsole(started.keys, PLAY);
  await expect(page.getByTestId("cue-0")).toHaveText("Q1");

  pressConsole(started.keys, STOP);
  await expect(page.getByTestId("cue-0")).toHaveText("—");
  await expect(page.getByTestId("strip-0").locator(".strip-running")).toHaveCount(0);
});
