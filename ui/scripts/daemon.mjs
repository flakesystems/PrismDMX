/**
 * `npm run daemon` — starts `prismd`, the engine, for development.
 *
 * Headless and harmless: a mock DMX output, its own data directory
 * (`ui/.dev-data/`, ignored by git) and a copy of a demo show with fixtures,
 * groups and cue lists in it. Stop it with Ctrl+C.
 *
 *   npm run daemon             keeps the demo show from the last run
 *   npm run daemon -- --fresh  starts again from the original demo show
 */

import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  DAEMON_BINARY,
  DEMO_SHOW,
  DEMO_SOURCE,
  DEV_DATA_DIR,
  REPO_DIR,
  daemonArgs,
  fail,
  has,
  run,
  start,
  step,
} from "./lib.mjs";

/** Puts the demo show in place, from scratch if asked. */
export function prepareDevData(fresh) {
  if (fresh && existsSync(DEV_DATA_DIR)) {
    rmSync(DEV_DATA_DIR, { recursive: true, force: true });
  }
  mkdirSync(DEV_DATA_DIR, { recursive: true });
  if (!existsSync(DEMO_SHOW)) {
    copyFileSync(DEMO_SOURCE, DEMO_SHOW);
  }
}

/**
 * Builds the daemon if Rust changed since the last build (a no-op otherwise),
 * starts it, and answers the child process.
 */
export function startDaemon() {
  if (!has("cargo")) {
    fail("cargo was not found — run `npm run setup` first.");
  }
  step("Building prismd (only does work when the Rust code changed)");
  run("cargo", ["build", "--quiet", "-p", "prismd"], REPO_DIR);
  step("Starting prismd on ws://127.0.0.1:7373 (mock output, demo show)");
  return start(DAEMON_BINARY, daemonArgs(), REPO_DIR);
}

// Run directly (`npm run daemon`) rather than imported by dev-all.mjs.
const direct = process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (direct) {
  prepareDevData(process.argv.includes("--fresh"));
  const daemon = startDaemon();
  daemon.on("exit", (code) => process.exit(code ?? 0));
}
