/**
 * **B70, where it means something**: a real `prismd`, a real browser, a head
 * that really hangs somewhere other than the show says.
 *
 * The operator's job, played by the test: the head is patched and hung where the
 * plan says; four spike marks are entered; for each, the head is driven at the
 * mark (the desk's own `Aim`), then trimmed to where the beam *really* lands -
 * which the test works out from the place the head **really** hangs, with its
 * own arithmetic - and the sighting is taken. The pose the desk reports is
 * compared with the truth, applied in one gesture, and taken back in one Oops.
 *
 * Nothing here touches a device. `--mock-devices` is the daemon's headless mode.
 */

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { mkdirSync } from "node:fs";
import { join } from "node:path";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, openWindow, showFixture, startDaemon } from "./daemon.ts";
import { description, writeGdtf } from "./gdtf.ts";

/** A port of this spec's own. */
const PORT = 7417;

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

interface V3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/**
 * The head's real place, and the arithmetic that says where its pan and tilt
 * must be for the beam to land on a point. Written out here, from the viewer's
 * own definition (`prism_domain::placement`): the rotation is `Ry · Rx · Rz`,
 * the beam leaves along `(-sin p sin t, -cos t, -cos p sin t)` of the fixture's
 * own frame, and the answer is the direction to the mark in that frame.
 */
const TRUTH = { position: { x: 1.2, y: 6.4, z: -0.8 }, heading: 20 };

function panTiltFor(mark: V3): { pan: number; tilt: number } {
  const to = {
    x: mark.x - TRUTH.position.x,
    y: mark.y - TRUTH.position.y,
    z: mark.z - TRUTH.position.z,
  };
  const length = Math.hypot(to.x, to.y, to.z);
  const [dx, dy, dz] = [to.x / length, to.y / length, to.z / length];
  // The transpose of a turn about the vertical by `heading`.
  const [s, c] = [Math.sin((TRUTH.heading * Math.PI) / 180), Math.cos((TRUTH.heading * Math.PI) / 180)];
  const local = { x: c * dx - s * dz, y: dy, z: s * dx + c * dz };
  const tilt = (Math.acos(-local.y) * 180) / Math.PI;
  const pan = (Math.atan2(-local.x, -local.z) * 180) / Math.PI;
  return { pan, tilt };
}

/** A pan of the test head (+-270 degrees) or a tilt (+-135) as the percentage a line takes. */
const panPercent = (degrees: number): string => (((degrees + 270) / 540) * 100).toFixed(4);
const tiltPercent = (degrees: number): string => (((degrees + 135) / 270) * 100).toFixed(4);

const MARKS: readonly { name: string; at: V3 }[] = [
  { name: "DS left", at: { x: -2, y: 0, z: -3 } },
  { name: "DS right", at: { x: 3, y: 0, z: -2.5 } },
  { name: "US centre", at: { x: 0.5, y: 0, z: 3.5 } },
  { name: "Platform", at: { x: 4, y: 0.8, z: 2 } },
];

test("a head is calibrated from four marks, hung where it really is, and taken back in one Oops", async ({
  page,
}) => {
  const fixture = showFixture("patch-rig.prism");
  dataDir = fixture.dataDir;
  mkdirSync(join(fixture.dataDir, "fixtures"), { recursive: true });
  writeGdtf(
    join(fixture.dataDir, "fixtures", "head.gdtf"),
    description("Prism Test", "Viewer E2E", { tilt: true }),
  );
  daemon = await startDaemon(PORT, fixture.dataDir, { show: fixture.show });
  await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");

  // The head, patched as fixture 10.
  await openWindow(page, "Patch");
  await page.getByRole("button", { name: "Add fixture" }).click();
  await page.getByTestId("library-search").fill("viewer e2e");
  await page
    .getByTestId("library-row-prism-test/viewer-e2e/mode-1")
    .locator("td")
    .nth(1)
    .click();
  await page.getByTestId("draft-id").fill("10");
  await page.getByTestId("draft-universe").fill("1");
  await page.getByTestId("draft-address").fill("100");
  await page.getByTestId("draft-apply").click();
  await expect(page.getByTestId("patch-row-10")).toBeVisible();

  // Hung where the plan says: six metres up, at the origin, turned by nothing.
  await openWindow(page, "Viewer3D");
  await command(page, "Fixture 10");
  await page.getByTestId("viewer-place-y").fill("6");
  await page.getByTestId("viewer-place-set").click();
  await expect(page.getByTestId("viewer-place-y")).toHaveValue("6");

  await page.getByTestId("viewer-panel-calibrate").click();
  await expect(page.getByTestId("calibration-target")).toContainText("Fixture 10");

  // The marks are the show's: entered once, whatever head is calibrated.
  for (const mark of MARKS) {
    await page.getByTestId("calibration-field-name").fill(mark.name);
    await page.getByTestId("calibration-field-x").fill(String(mark.at.x));
    await page.getByTestId("calibration-field-y").fill(String(mark.at.y));
    await page.getByTestId("calibration-field-z").fill(String(mark.at.z));
    await page.getByTestId("calibration-save-mark").click();
  }
  await expect(page.getByTestId("calibration-marks").locator("li")).toHaveCount(4);

  // The first mark: the desk drives the head at it from where the plan hangs
  // it. The programmer then holds a pan and a tilt - the thing "Take" reads.
  await page.getByTestId("calibration-aim-1").click();
  await expect(page.getByTestId("calibration-take-1")).toBeEnabled();

  for (const [index, mark] of MARKS.entries()) {
    const id = index + 1;
    // The operator trims until the real beam is on the real mark: here, the pan
    // and tilt that put it there from the head's real place.
    const { pan, tilt } = panTiltFor(mark.at);
    await command(page, `Fixture 10 Pan At ${panPercent(pan)}`);
    await command(page, `Fixture 10 Tilt At ${tiltPercent(tilt)}`);
    await page.getByTestId(`calibration-take-${String(id)}`).click();
    await expect(page.getByTestId(`calibration-sighting-${String(id)}`)).toBeVisible();
    if (id === 2) {
      // Two sightings are not enough for a position, and the panel says so.
      await expect(page.getByTestId("calibration-result")).toContainText("Take 1 more sighting");
    }
  }

  // Four sightings: the pose, with one spare to check it against.
  const pose = page.getByTestId("calibration-pose");
  await expect(pose).toContainText("Hangs at");
  const numbers = ((await pose.textContent()) ?? "").match(/-?\d+\.\d+/g)?.map(Number) ?? [];
  expect(numbers).toHaveLength(6);
  const [x, y, z, , heading] = numbers as [number, number, number, number, number, number];
  // 16 bits of a 540 degree pan is 0.008 degrees a step: millimetres at 8 m.
  expect(x).toBeCloseTo(TRUTH.position.x, 1);
  expect(y).toBeCloseTo(TRUTH.position.y, 1);
  expect(z).toBeCloseTo(TRUTH.position.z, 1);
  expect(heading).toBeCloseTo(TRUTH.heading, 0);
  await expect(page.getByTestId("calibration-result")).toHaveAttribute("data-level", "good");

  // The marks survive a reload - they are the show's, not this window's.
  // (Applying comes first: a reload forgets the sightings, which are not.)
  await page.getByTestId("calibration-apply").click();
  await page.getByTestId("viewer-panel-place").click();
  const placed = async (id: string): Promise<number> =>
    Number(await page.getByTestId(id).inputValue());
  await expect.poll(() => placed("viewer-place-x")).toBeCloseTo(TRUTH.position.x, 1);
  await expect.poll(() => placed("viewer-place-y")).toBeCloseTo(TRUTH.position.y, 1);
  await expect.poll(() => placed("viewer-place-ry")).toBeCloseTo(TRUTH.heading, 0);

  // One Oops takes the whole calibration back: where the plan hung it.
  await command(page, "Oops");
  await expect(page.getByTestId("viewer-place-x")).toHaveValue("0");
  await expect(page.getByTestId("viewer-place-y")).toHaveValue("6");
  await expect(page.getByTestId("viewer-place-ry")).toHaveValue("0");

  await page.reload();
  await expect(page.getByTestId("connection-status")).toHaveText("Connected");
  await command(page, "Fixture 10");
  await page.getByTestId("viewer-panel-calibrate").click();
  await expect(page.getByTestId("calibration-marks").locator("li")).toHaveCount(4);
  await expect(page.getByTestId("calibration-mark-3")).toContainText("US centre");
});
