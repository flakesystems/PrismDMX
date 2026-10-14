/**
 * What the helper scripts share: where things are, and how to run a program so
 * that it works the same on Windows, Linux and macOS.
 *
 * Plain JavaScript run by Node, on purpose: no shell script, no PowerShell, no
 * build step — `npm run <name>` works wherever npm does.
 */

import { spawn, spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** `ui/`. */
export const UI_DIR = resolve(dirname(fileURLToPath(import.meta.url)), "..");
/** The repository root, where `cargo` is run. */
export const REPO_DIR = resolve(UI_DIR, "..");
/** Where the development daemon keeps its configuration and its show. Ignored by git. */
export const DEV_DATA_DIR = join(UI_DIR, ".dev-data");
/** The demo show it opens — a copy, so the test fixture it came from is never written. */
export const DEMO_SHOW = join(DEV_DATA_DIR, "demo.prism");
/** Which rig the demo show is copied from. */
export const DEMO_SOURCE = join(UI_DIR, "tests", "fixtures", "show-rig.prism");
/** The address the interface connects to by default (`src/core/ipc/endpoint.ts`). */
export const DAEMON_HOST = "127.0.0.1";
export const DAEMON_PORT = 7373;

const WINDOWS = process.platform === "win32";

/** Prints a step, so a long `cargo` run is not a silent terminal. */
export function step(text) {
  process.stdout.write(`\n\u001b[36m▶ ${text}\u001b[0m\n`);
}

/** Prints why the script stopped and stops it. */
export function fail(text) {
  process.stderr.write(`\n\u001b[31m✖ ${text}\u001b[0m\n`);
  process.exit(1);
}

/**
 * Whether `command` needs a shell to start.
 *
 * `npm` is a `.cmd` file on Windows, which Node only starts through a shell.
 * Nothing else gets one: a daemon started through a shell is a shell, and
 * stopping it would leave the daemon running.
 */
function shellFor(command) {
  return WINDOWS && command === "npm";
}

/** Whether `command` can be started at all. */
export function has(command) {
  const probe = spawnSync(command, ["--version"], { stdio: "ignore", shell: shellFor(command) });
  return probe.status === 0;
}

/** Runs a program to completion, its output on this terminal; stops the script if it fails. */
export function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, stdio: "inherit", shell: shellFor(command) });
  if (result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed (exit code ${String(result.status)})`);
  }
}

/** Starts a long-running program, its output on this terminal, and answers the child. */
export function start(command, args, cwd) {
  return spawn(command, args, { cwd, stdio: "inherit", shell: shellFor(command) });
}

/** The daemon binary `cargo build -p prismd` leaves behind. */
export const DAEMON_BINARY = join(REPO_DIR, "target", "debug", WINDOWS ? "prismd.exe" : "prismd");

/** The arguments that start a daemon for development. */
export function daemonArgs() {
  return [
    // A DMX output that accepts every frame and sends it nowhere: no hardware,
    // no network traffic, nothing a venue's rig could receive.
    "--mock-output",
    "--data-dir",
    DEV_DATA_DIR,
    "--show",
    DEMO_SHOW,
    "--websocket",
    `${DAEMON_HOST}:${String(DAEMON_PORT)}`,
    // No named pipe / socket: an installed PrismDMX on the same machine keeps its own.
    "--no-local",
    "--no-artnet-discovery",
  ];
}
