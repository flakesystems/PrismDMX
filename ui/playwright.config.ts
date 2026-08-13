/**
 * Playwright against a daemon in mock-output mode, per `ARCHITECTURE_SPEC.md`
 * §12's last row.
 *
 * `--host 127.0.0.1` is not decoration: `vite preview` binds `localhost`
 * otherwise, and on a machine where that resolves to `::1` first, nothing is
 * listening on the IPv4 address the browser is pointed at.
 *
 * The interface is served from the **built** assets rather than from the dev
 * server: what an operator runs is the build, and a test of the build catches
 * the things only a bundler does. The daemon is not started here — each spec
 * starts and kills its own, because starting and killing it is the thing being
 * tested.
 *
 * Nothing here touches a device. `--mock-output` is the daemon's headless mode:
 * a DMX output that accepts every frame and puts it nowhere.
 */

import { defineConfig, devices } from "@playwright/test";

/** Where the built interface is served. */
const PORT = 4173;

export default defineConfig({
  testDir: "e2e",
  // A daemon has to start, a browser has to connect and a backoff has to
  // elapse; a minute is generous and still bounded, which is S16's lesson
  // about a CI job that never ended.
  timeout: 60_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI === undefined ? "list" : [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: devices["Desktop Chrome"] }],
  webServer: {
    command: `npm run build && npm run preview -- --host 127.0.0.1 --port ${PORT} --strictPort`,
    url: `http://127.0.0.1:${PORT}`,
    reuseExistingServer: process.env.CI === undefined,
    timeout: 180_000,
    stdout: "ignore",
    stderr: "pipe",
  },
});
