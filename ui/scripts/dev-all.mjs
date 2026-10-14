/**
 * `npm run dev:all` — the daemon and the interface together, in one terminal.
 *
 * Starts `prismd` (see `daemon.mjs`) and Vite's dev server, and stops both on
 * Ctrl+C or when either one ends. Open the address Vite prints; every save
 * reloads the page, and the interface reconnects to the daemon by itself.
 *
 *   npm run dev:all -- --fresh   starts again from the original demo show
 */

import { join } from "node:path";

import { prepareDevData, startDaemon } from "./daemon.mjs";
import { UI_DIR, start, step } from "./lib.mjs";

prepareDevData(process.argv.includes("--fresh"));
const daemon = startDaemon();

step("Starting the interface (Vite)");
const vite = start(process.execPath, [join(UI_DIR, "node_modules", "vite", "bin", "vite.js")], UI_DIR);

let stopping = false;
function stopAll(code) {
  if (stopping) {
    return;
  }
  stopping = true;
  for (const child of [vite, daemon]) {
    if (child.exitCode === null) {
      child.kill();
    }
  }
  process.exitCode = code;
}

daemon.on("exit", (code) => stopAll(code ?? 0));
vite.on("exit", (code) => stopAll(code ?? 0));
process.on("SIGINT", () => stopAll(0));
process.on("SIGTERM", () => stopAll(0));
