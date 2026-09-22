/**
 * **S30's exit criteria, where they mean something**: a real `prismd`, a real
 * browser, a real canvas.
 *
 * - *Every patched fixture appears with correct position and orientation* — a
 *   rig with a GDTF head in it is hung from the viewer's own panel, one Oops
 *   takes the whole hang back, and the readout says how many are still at the
 *   origin at every step.
 * - *Beams reflect live output* — a line on the command line puts the rig at
 *   full, the viewer counts it lit, and the canvas has light on it read back
 *   out of the bitmap. A readout can be right about a canvas that is blank.
 * - *The viewer never blocks the telemetry canvas or the engine* — the widest
 *   rig this desk carries, 64 universes, with the DMX Sheet and the viewer open
 *   side by side: the DMX Sheet keeps its S24 budget and its 30 Hz, and the
 *   viewer's own paint is measured and written down.
 *
 * Nothing here touches a device. `--mock-output` is the daemon's headless mode.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { mkdirSync } from "node:fs";
import { join } from "node:path";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, showFixture, startDaemon } from "./daemon.ts";
import { description, writeGdtf } from "./gdtf.ts";

/** A port of this spec's own. */
const PORT = 7405;

/** The DMX Sheet's budget, from `IMPLEMENTATION_PLAN.md` S24. */
const TELEMETRY_BUDGET_MS = 8;

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

/** Types a line and presses Enter. */
async function command(page: Page, line: string): Promise<void> {
  const input = page.getByTestId("command-input");
  await input.fill(line);
  await input.press("Enter");
}

/**
 * How many pixels of the viewer are bright, read back out of what the browser
 * **shows** — a screenshot of the canvas, decoded in the page. A WebGL canvas
 * has no 2D context to read, and its drawing buffer is cleared once it is on
 * the screen, so the screen is what is asked.
 */
async function brightPixels(page: Page): Promise<number> {
  const png = (await page.getByTestId("viewer-canvas").screenshot()).toString("base64");
  return page.evaluate(async (data) => {
    const blob = await (await fetch(`data:image/png;base64,${data}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const context = canvas.getContext("2d");
    if (context === null) {
      return -1;
    }
    context.drawImage(bitmap, 0, 0);
    const image = context.getImageData(0, 0, canvas.width, canvas.height);
    let bright = 0;
    for (let at = 0; at < image.data.length; at += 4) {
      if ((image.data[at] ?? 0) + (image.data[at + 1] ?? 0) + (image.data[at + 2] ?? 0) > 360) {
        bright += 1;
      }
    }
    return bright;
  }, png);
}

test("a rig is hung from the viewer, taken back in one Oops, and lit from the line", async ({
  page,
}) => {
  const fixture = showFixture("patch-rig.prism");
  dataDir = fixture.dataDir;
  // A venue's own GDTF, dropped into the desk's folder (B43): the head the
  // viewer draws from its archive's size and beam rather than as a box.
  mkdirSync(join(fixture.dataDir, "fixtures"), { recursive: true });
  writeGdtf(join(fixture.dataDir, "fixtures", "head.gdtf"), description("Prism Test", "Viewer E2E"));
  daemon = await startDaemon(PORT, fixture.dataDir, { show: fixture.show });
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  // The head, patched as fixture 10 from the Patch window.
  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();
  const key = "prism-test/viewer-e2e/mode-1";
  await page.getByTestId("library-search").fill("viewer e2e");
  await page.getByTestId(`library-row-${key}`).locator("td").nth(1).click();
  await page.getByTestId("draft-id").fill("10");
  await page.getByTestId("draft-universe").fill("1");
  await page.getByTestId("draft-address").fill("100");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-10")).toBeVisible();

  await openWindow(page, "Viewer3D");
  const stats = page.getByTestId("viewer-stats");
  // A browser with WebGL draws the rig in 3D (S30b); the 2D picture is only
  // for one without.
  await expect(stats).toHaveAttribute("data-renderer", "webgl");
  // Four fixtures, and every one of them where a patch leaves it.
  await expect(stats).toHaveAttribute("data-fixtures", "4");
  await expect(stats).toHaveAttribute("data-unplaced", "4");
  await expect(stats).toContainText("4 not placed");

  // Select them on the line — by name, because a range across numbers that
  // are not patched is refused — and spread them along a truss six metres up.
  await command(page, "Fixture 1 + 2 + 5 + 10");
  await expect(page.getByTestId("viewer-place-selection")).toHaveText("4 fixtures: 1, 2, 5, 10");
  await page.getByTestId("viewer-place-y").fill("6");
  await page.getByTestId("viewer-place-spacing").fill("2");
  await page.getByTestId("viewer-place-spread").click();
  await expect(stats).toHaveAttribute("data-unplaced", "0");
  // The form follows where the first of them now hangs: the far left of a
  // spread of four, two metres apart, centred on nought.
  await expect(page.getByTestId("viewer-place-x")).toHaveValue("-3");

  // One Oops takes the whole spread back — it was one gesture.
  await command(page, "Oops");
  await expect(stats).toHaveAttribute("data-unplaced", "4");
  await page.getByTestId("viewer-place-y").fill("6");
  await page.getByTestId("viewer-place-spacing").fill("2");
  await page.getByTestId("viewer-place-spread").click();
  await expect(stats).toHaveAttribute("data-unplaced", "0");

  // Dark until the line says otherwise; then every one of them lit, and the
  // light is on the canvas as well as in the readout.
  await expect(stats).toHaveAttribute("data-lit", "0");
  await command(page, "Fixture 1 + 2 + 5 + 10 At Full");
  await expect(stats).toHaveAttribute("data-lit", "4");
  await expect.poll(() => brightPixels(page), { timeout: 10_000 }).toBeGreaterThan(200);

  // And they go out with the line.
  await command(page, "Fixture 1 + 2 + 5 + 10 At 0");
  await expect(stats).toHaveAttribute("data-lit", "0");

  // A click on the canvas is not a command; a view button is not either. The
  // camera is this screen's own (§4.2), so the rig is where it was.
  await page.getByTestId("viewer-view-top").click();
  await page.getByTestId("viewer-view-front").click();
  await expect(stats).toHaveAttribute("data-unplaced", "0");
});

test("with the DMX Sheet and the viewer open on 64 universes, the DMX Sheet keeps its budget", async ({
  page,
}) => {
  const fixture = showFixture("wide-rig.prism");
  dataDir = fixture.dataDir;
  daemon = await startDaemon(PORT + 1, fixture.dataDir, { show: fixture.show, universes: 64 });
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  await openWindow(page, "DmxSheet");
  await openWindow(page, "Viewer3D");
  const viewer = page.getByTestId("viewer-stats");
  // Eight dimmers in each of 64 universes (`ui_telemetry.rs::wide_show`) and
  // the ordinary rig's few beside them.
  await expect(viewer).toHaveAttribute("data-fixtures", /^5\d\d$/);
  // Half of them rest at full and half at nought (`generic.dimmer.dark`), so
  // the viewer is drawing some two hundred and fifty beams on every frame the
  // daemon sends, all from one place — the origin, where a patch leaves them,
  // which is the worst case for overdraw.
  await expect.poll(async () => Number((await viewer.getAttribute("data-lit")) ?? 0)).toBeGreaterThan(200);

  // An operator orbiting the camera the whole time, so the viewer is really
  // drawing while the DMX Sheet is measured — a still rig on a steady cable
  // draws nothing at all (`src/viewer/gl/driver3d.ts`).
  const canvas = page.getByTestId("viewer-canvas");
  const box = await canvas.boundingBox();
  let orbiting = box !== null;
  const orbit = (async () => {
    if (box === null) {
      return;
    }
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    while (orbiting) {
      await page.mouse.move(x, y);
      await page.mouse.down();
      await page.mouse.move(x + 40, y + 5, { steps: 4 });
      await page.mouse.up();
    }
  })();

  // Let both run for the length of the DMX Sheet's paint window.
  const telemetry = page.getByTestId("telemetry-stats");
  await expect
    .poll(async () => Number(/(\d+) frames/.exec((await telemetry.textContent()) ?? "")?.[1] ?? 0), {
      timeout: 40_000,
      intervals: [500],
    })
    .toBeGreaterThanOrEqual(150);
  // The viewer keeps drawing — at the rate this browser allows it. Here WebGL
  // is done in software (SwiftShader), so the viewer starts at Low, draws at
  // half resolution without multisampling and at most four frames a second
  // (`src/viewer/gl/graphics.ts`, `driver3d.ts`), and the page stays the
  // console's. The rate is written down, not required.
  await expect
    .poll(async () => Number((await viewer.getAttribute("data-painted")) ?? 0), { timeout: 20_000 })
    .toBeGreaterThanOrEqual(10);

  orbiting = false;
  await orbit;

  const line = (await telemetry.textContent()) ?? "";
  const hz = Number(/([\d.]+) Hz/.exec(line)?.[1] ?? 0);
  const p99 = Number(/p99 ([\d.]+) ms/.exec(line)?.[1] ?? Number.POSITIVE_INFINITY);
  const viewerLine = `${(await viewer.textContent()) ?? ""} · ${String(await viewer.getAttribute("data-painted"))} painted (median ${String(await viewer.getAttribute("data-median"))} ms, p99 ${String(await viewer.getAttribute("data-p99"))} ms)`;
  // Onto the console and into the report: `PROGRESS.md` records these.
  test.info().annotations.push({ type: "telemetry", description: line });
  test.info().annotations.push({ type: "viewer", description: viewerLine });
  console.log(`[telemetry with the viewer open] ${line}`);
  console.log(`[viewer, the wide rig] ${viewerLine}`);

  // **The criterion.** The DMX Sheet is still drawing every frame the daemon
  // publishes, and its worst frame is inside its own budget.
  expect(hz).toBeGreaterThan(20);
  expect(p99).toBeLessThan(TELEMETRY_BUDGET_MS);
});
