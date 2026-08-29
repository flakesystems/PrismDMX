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

/** Types a line into the console and sends it. */
async function command(page: import("@playwright/test").Page, line: string): Promise<void> {
  await page.getByTestId("command-input").fill(line);
  await page.getByTestId("command-input").press("Enter");
}

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

  // **Its fader cannot put light up**, which is what makes this a test of the
  // flash rather than of a fader. It reads `XF` since S45 rather than `0%`: the
  // strip is a crossfade, and where a crossfade fader stands is a gesture in
  // progress rather than a level the show holds, so there is no percentage for
  // it to have. The claim the reading used to carry — *a flash never writes the
  // stored master* — is the same claim and is asserted on the byte in
  // `prismd::core::tests::a_flash_is_a_layer_over_the_master`, where the master
  // now lives (on the cue list).
  await expect(page.getByTestId("percent-2")).toHaveText("XF");

  await openWindow(page, "DmxSheet");
  await expect.poll(async () => litPixels(page, METER_FULL), { timeout: 15_000 }).toBe(0);

  // Held. The mouse goes down and stays down: a click would be press and
  // release in one gesture, and a momentary function has two halves.
  const flash = page.getByTestId("button-2-0");
  await flash.hover();
  await page.mouse.down();
  await expect.poll(async () => litPixels(page, METER_FULL), { timeout: 15_000 }).toBeGreaterThan(0);
  // And nothing about the strip has moved while it is held.
  await expect(page.getByTestId("percent-2")).toHaveText("XF");

  // Released.
  await page.mouse.up();
  await expect.poll(async () => litPixels(page, METER_FULL), { timeout: 15_000 }).toBe(0);

  // A reload asks the daemon what it holds, with nothing of this browser's in
  // the answer: the rig is dark again, so the flash left no trace.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  // The window is still open: which windows are on the canvas is session state
  // and the daemon holds it, so a reload finds it there (§4.1).
  await expect(page.getByTestId("percent-2")).toHaveText("XF");
});

/**
 * **Punch-list B15, in a browser**: what a key and a fader do is set from the
 * `Executors` window, and the strip above redraws.
 *
 * *Es passiert nichts wenn man einen Executor rechtsklickt* — the entry. The
 * editor writes a command line and sends it (`ARCHITECTURE_SPEC.md` §4.5), so
 * what is observed here is the whole chain: a chooser, a line, a command, a
 * delta, and a strip that changed.
 */
test("**B15**: an executor's controls are assignable, and the strip follows", async ({ page }) => {
  const started = await deskDaemon(PORT + 3);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await openWindow(page, "Executors");

  // Nothing is selected on a fresh desk, so the editor says what to do rather
  // than drawing a form with nothing behind it.
  await expect(page.getByTestId("editor-none")).toContainText("Select an executor");

  // The strip's head is the select target — a word and a number, so it writes
  // `Executor 2` and sends it.
  await page.getByTestId("select-2").click();
  await expect(page.getByTestId("editor-executor")).toHaveAttribute("data-executor", "2");
  await expect(page.getByTestId("editor-fader")).toHaveValue("XFade");
  await expect(page.getByTestId("editor-button-0")).toHaveValue("Flash");

  // Give the fader a master. The strip above says so, and the percentage comes
  // back with it — the cue list's own level, which is where S45 put it.
  await page.getByTestId("editor-fader").selectOption("Master");
  await expect(page.getByTestId("fader-2")).toHaveAttribute("data-function", "Master");
  await expect(page.getByTestId("percent-2")).toHaveText("0%");

  // And a key: the third one becomes a Go, and the strip relabels it.
  await page.getByTestId("editor-button-2").selectOption("Go+");
  await expect(page.getByTestId("button-2-2")).toHaveAttribute("data-function", "Go+");

  // The custom row — a key that sends a line the operator wrote. Choosing it
  // opens the box and sends nothing; the line is what sends it.
  await page.getByTestId("editor-button-3").selectOption("Command");
  await expect(page.getByTestId("button-2-3")).toHaveAttribute("data-function", "LearnSpeed");
  await page.getByTestId("editor-line-3").fill("Go+ Sequence 2");
  await page.getByTestId("editor-send-3").click();
  await expect(page.getByTestId("button-2-3")).toHaveAttribute("data-function", "CommandLine");
  await expect(page.getByTestId("button-2-3")).toHaveAttribute("title", /Go\+ Sequence 2/);

  // A reload asks the daemon what it holds: the assignment is show state and
  // survives, with nothing of this browser's in the answer.
  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("fader-2")).toHaveAttribute("data-function", "Master");
  await expect(page.getByTestId("button-2-3")).toHaveAttribute("data-function", "CommandLine");
});

/**
 * **Punch-list B18, in a browser**: two executors on one cue list are two
 * handles on one number.
 *
 * *Wenn eine Sequence mehrere Executor hat, die die gleichen Button/Fader Typen
 * haben, kann man diese unabhängig von einander bewegen* — the entry. Here a
 * second fader is put on the list executor 0 already plays, and pulling one is
 * read on the other. The frames are asserted in
 * `prismd::core::tests::two_master_faders_on_one_cue_list_move_one_light`; what
 * this adds is the half an operator sees.
 */
test("**B18**: two executors on one cue list read and move one master", async ({ page }) => {
  const started = await deskDaemon(PORT + 4);
  daemon = started.daemon;
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await openWindow(page, "Executors");
  await openWindow(page, "CommandKeys");

  // Executor 0 plays cue list 1 with a Master fader, at full.
  await expect(page.getByTestId("percent-0")).toHaveText("100%");

  // Put the same list on a second, empty slot and give it a Master too — the
  // two lines S45 added, typed rather than clicked, which is the other way of
  // saying the same thing.
  await command(page, "Assign Sequence 1 Executor 1");
  await command(page, "Assign Executor 1 Fader Master");
  await expect(page.getByTestId("name-1")).toHaveText("Warm Wash");
  // **It reads the list's level, not a fresh one of its own.**
  await expect(page.getByTestId("percent-1")).toHaveText("100%");

  // Move one of them from the line, and both say the new number.
  await command(page, "Assign Executor 1 Fader Speed");
  await expect(page.getByTestId("fader-1")).toHaveAttribute("data-function", "Speed");
  await command(page, "Assign Executor 1 Fader Master");
  await expect(page.getByTestId("percent-1")).toHaveText("100%");

  // Drag the second fader to the bottom. The first shows it too, because there
  // is one number and both of them point at it.
  const fader = page.getByTestId("fader-1");
  const box = await fader.boundingBox();
  if (box === null) {
    throw new Error("the fader has no box");
  }
  await page.mouse.move(box.x + box.width / 2, box.y + 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height + 40);
  await page.mouse.up();
  await expect(page.getByTestId("percent-1")).toHaveText("0%");
  await expect(page.getByTestId("percent-0")).toHaveText("0%");
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
