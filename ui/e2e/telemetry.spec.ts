/**
 * **The second exit criterion of S24, measured where it means something.**
 *
 * *The canvas render stays under 8 ms at 64 universes.* A number from `jsdom`
 * would be worth nothing — there is no rasteriser in it — so this is a real
 * `prismd` publishing **64 real universes** at §7's 30 Hz, a real WebSocket, a
 * real Chromium, and the interface's own `performance.now()` measurements read
 * back out of the readout line.
 *
 * The rig is `ui/tests/fixtures/wide-rig.prism`, written by
 * `crates/prismd/tests/ui_telemetry.rs` and opened by a Rust test on every
 * commit: 64 universes with eight dimmers patched into each. The daemon filters
 * telemetry down to the universes a show actually patches, so this is the only
 * honest way to have 64 of them — a wide *layout* over a narrow show carries
 * exactly as little as before.
 *
 * Nothing here touches a device. `--mock-output` is the daemon's headless mode.
 */

import { expect, test } from "@playwright/test";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, showFixture, startDaemon } from "./daemon.ts";

/** A port of this spec's own. */
const PORT = 7383;

/** The budget, in milliseconds, from `IMPLEMENTATION_PLAN.md` S24. */
const BUDGET_MS = 8;

/** How many frames to gather before the numbers are read. */
const FRAMES = 150;

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

/** The readout line, as the panel wrote it. */
interface Readout {
  readonly universes: number;
  readonly hz: number;
  readonly median: number;
  readonly p99: number;
  readonly frames: number;
  readonly lost: number;
  readonly dropped: number;
  readonly live: boolean;
}

/** Reads the numbers out of the line the driver wrote with `textContent`. */
function parse(line: string): Readout | null {
  const numbers =
    /^(?<stale>not live — )?(?<universes>\d+) universes · (?<hz>[\d.]+) Hz · paint (?<median>[\d.]+) ms \(p99 (?<p99>[\d.]+) ms\) · (?<frames>\d+) frames(?<rest>.*)$/.exec(
      line,
    );
  const found = numbers?.groups;
  if (found === undefined) {
    return null;
  }
  const after = found.rest ?? "";
  const count = (what: string): number => Number(new RegExp(`(\\d+) ${what}`).exec(after)?.[1] ?? 0);
  return {
    universes: Number(found.universes),
    hz: Number(found.hz),
    median: Number(found.median),
    p99: Number(found.p99),
    frames: Number(found.frames),
    lost: count("lost"),
    dropped: count("dropped"),
    live: found.stale === undefined,
  };
}

test("64 universes at 30 Hz, drawn inside the frame budget", async ({ page }) => {
  const fixture = showFixture("wide-rig.prism");
  dataDir = fixture.dataDir;
  daemon = await startDaemon(PORT, fixture.dataDir, { show: fixture.show, universes: 64 });
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);

  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await expect(page.getByTestId("telemetry")).toBeVisible();

  // The channel is carrying the whole rig. This is the assertion that says the
  // decoder read a 32 912-byte frame in a browser rather than in a test.
  const readout = page.getByTestId("telemetry-stats");
  await expect(readout).toContainText("64 universes", { timeout: 20_000 });

  // Let it run. The paint window holds four seconds, so the p99 below is of a
  // full window rather than of the first frame after the page loaded.
  await expect
    .poll(async () => parse((await readout.textContent()) ?? "")?.frames ?? 0, {
      timeout: 40_000,
      intervals: [500],
    })
    .toBeGreaterThanOrEqual(FRAMES);

  const line = (await readout.textContent()) ?? "";
  const measured = parse(line);
  expect(measured, `the readout did not parse: ${line}`).not.toBeNull();
  if (measured === null) {
    return;
  }
  // Into the report *and* onto the console, because the figure recorded in
  // `PROGRESS.md` §3 has to be one somebody can re-run and read. A measurement
  // nobody can reproduce is worse than no measurement.
  test.info().annotations.push({ type: "telemetry", description: line });
  console.log(`[telemetry] ${line}`);

  expect(measured.universes).toBe(64);
  expect(measured.live).toBe(true);
  // §7's rate, allowing for the fact that a browser paints when it can: what is
  // being checked is that the daemon is publishing at its 30 Hz and the
  // interface is keeping up with it, not that a clock is exact.
  expect(measured.hz).toBeGreaterThan(20);
  expect(measured.hz).toBeLessThan(35);
  // **The criterion.** Reading 32 912 bytes and drawing 32 768 channels, the
  // worst frame in the last four seconds is inside the budget.
  expect(measured.p99).toBeLessThan(BUDGET_MS);
  expect(measured.median).toBeLessThan(BUDGET_MS);
  // Nothing was unreadable. Frames *lost* are allowed — the daemon coalesces
  // and drops (§8) — but a frame this build could not read would be a defect.
  expect(measured.dropped).toBe(0);

  // And the picture is a picture: the canvas has lit pixels on it, read back
  // out of the bitmap. A readout can be right about a canvas that is blank.
  const lit = await page.getByTestId("telemetry-canvas").evaluate((element) => {
    if (!(element instanceof HTMLCanvasElement)) {
      return -1;
    }
    const context = element.getContext("2d");
    if (context === null) {
      return -1;
    }
    const image = context.getImageData(0, 0, element.width, element.height);
    let bright = 0;
    for (let at = 0; at < image.data.length; at += 4) {
      const red = image.data[at] ?? 0;
      const green = image.data[at + 1] ?? 0;
      const blue = image.data[at + 2] ?? 0;
      if (red + green + blue > 300) {
        bright += 1;
      }
    }
    return bright;
  });
  expect(lit).toBeGreaterThan(64);

  await daemon.kill();
  daemon = null;
  // The picture goes with the daemon: a rig drawn from an engine that has
  // stopped is exactly as stale as a fader value from one.
  await expect(page.getByTestId("connection-status")).toContainText("Disconnected");
  await expect(page.getByTestId("telemetry")).toHaveCount(0);
});
