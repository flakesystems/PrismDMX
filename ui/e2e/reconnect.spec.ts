/**
 * **The second exit criterion of S23, against a real daemon.**
 *
 * The unit tests make the same claim in jsdom against a fake socket
 * (`src/App.test.tsx`), and they are the ones that run on every commit. This is
 * the one that removes the remaining doubt: a real `prismd` process, a real
 * WebSocket, a real MessagePack conversation, a real browser — and the daemon
 * is killed rather than asked to stop, which is what happens when the machine
 * it runs on is switched off.
 *
 * The show is unaffected by any of it. `--mock-output` is the daemon's headless
 * mode and no test in this project may touch a device (`CLAUDE.md`).
 */

import { expect, test } from "@playwright/test";

import type { Daemon } from "./daemon.ts";
import { buildDaemon, forget, startDaemon } from "./daemon.ts";

/** A port of this suite's own, so a daemon on 7373 is neither used nor disturbed. */
const PORT = 7381;

let daemon: Daemon | null = null;

test.beforeAll(() => {
    buildDaemon();
});

test.afterEach(async () => {
    await daemon?.kill();
    daemon = null;
});

test("the interface follows a real daemon, loses it, and comes back with nothing stale", async ({
    page,
}) => {
    daemon = await startDaemon(PORT);
    const dataDir = daemon.dataDir;
    await page.goto(`/?daemon=${encodeURIComponent(daemon.url)}`);

    // 1. The handshake, over a WebSocket, with MessagePack the daemon encoded.
    await expect(page.getByTestId("connection-status")).toHaveText("Connected");
    await expect(page.getByTestId("protocol")).toHaveText("1");
    await expect(page.getByTestId("output-1")).toContainText("Mock");
    // The engine is running behind it, with this window as a spectator.
    await expect(page.getByTestId("tick-hz")).not.toHaveText("0.0 Hz");

    // 2. A command goes out and a fact comes back. What is typed is local input;
    //    what the readout shows is the daemon's own session, changed by the
    //    `SessionPatch` the command produced. Nothing is optimistic (D3).
    //
    //    Typing is enough: since S26 the console line is mirrored into the
    //    session **as it is typed** (paced), so every other client and the
    //    scribble strips follow a line before it is executed. Enter is what
    //    *executes* it, and executing it clears the line.
    await expect(page.getByTestId("command-line")).toHaveText("");
    await page.getByTestId("command-input").fill("fixture 1 at full");
    await expect(page.getByTestId("command-line")).toHaveText("fixture 1 at full");

    // 3. The daemon is killed. Not asked to stop — killed.
    await daemon.kill();
    await expect(page.getByTestId("connection-status")).toContainText("Disconnected");
    await expect(page.getByTestId("no-daemon")).toBeVisible();
    // The exit criterion: no value from the old daemon is still on the screen.
    await expect(page.getByTestId("command-line")).toHaveCount(0);
    await expect(page.getByTestId("executor-page")).toHaveCount(0);
    await expect(page.locator("body")).not.toContainText("fixture 1 at full");

    // 4. It comes back — the same show file, and a session that was never saved,
    //    so the command line the interface showed a moment ago no longer exists
    //    anywhere. If the interface had kept it, this is where it would show.
    daemon = await startDaemon(PORT, dataDir);
    await expect(page.getByTestId("connection-status")).toHaveText("Connected");
    await expect(page.getByTestId("command-line")).toHaveText("");
    await expect(page.locator("body")).not.toContainText("fixture 1 at full");

    // And the connection is a working one again rather than merely green.
    await page.getByTestId("command-input").fill("group 2 at 50");
    await expect(page.getByTestId("command-line")).toHaveText("group 2 at 50");

    await daemon.kill();
    daemon = null;
    forget(dataDir);
});

test("with no daemon at all it says so and keeps trying", async ({ page }) => {
    // Nothing is listening on this port. §8: the interface reports that the
    // engine is not running rather than pretending to be connected.
    await page.goto(`/?daemon=${encodeURIComponent(`ws://127.0.0.1:${PORT + 1}/ipc`)}`);
    await expect(page.getByTestId("connection-status")).toContainText("Disconnected");
    await expect(page.getByTestId("no-daemon")).toContainText("The engine is not answering");

    // And when one appears, it is found without the page being reloaded.
    daemon = await startDaemon(PORT + 1);
    await expect(page.getByTestId("connection-status")).toHaveText("Connected");
});
