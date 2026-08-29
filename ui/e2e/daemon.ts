/**
 * A real `prismd`, started and killed by the test that needs one.
 *
 * The daemon is found the way `docs/IPC_PROTOCOL.md` §2.2 says a client finds
 * it — by reading the lock file it writes into its data directory — so
 * "the daemon is ready" is not a sleep: the file names an endpoint only once a
 * listener is bound to it.
 *
 * Every daemon started here gets a data directory of its own and a port of its
 * own, so a developer's own daemon on 7373 is neither used nor disturbed.
 */

import { expect } from "@playwright/test";
import type { Page } from "@playwright/test";

import { spawn, spawnSync } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { appendFileSync, copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

/** The workspace root, from `ui/e2e`. */
const ROOT = resolve(import.meta.dirname, "../..");

/** The daemon binary a `cargo build` leaves behind. */
function binary(): string {
  return join(ROOT, "target", "debug", process.platform === "win32" ? "prismd.exe" : "prismd");
}

/** Builds the daemon, so the first spec does not pay for it in its own timeout. */
export function buildDaemon(): void {
  const built = spawnSync("cargo", ["build", "-p", "prismd"], {
    cwd: ROOT,
    // No shell: `cargo` is an executable on every platform this runs on, and a
    // shell here is one more thing between the test and the error it needs.
    stdio: "inherit",
  });
  if (built.status !== 0) {
    throw new Error(`cargo build -p prismd failed with ${String(built.status)}`);
  }
}

/** A running daemon. */
export interface Daemon {
  /** The WebSocket URL a client connects to. */
  readonly url: string;
  /** Where its files are, so a restart can use the same show. */
  readonly dataDir: string;
  /** Kills it the way switching a machine off would. */
  kill: () => Promise<void>;
}

/** What else a daemon may be told to be. */
export interface DaemonOptions {
  /** A show file to open, instead of the empty one in the data directory. */
  readonly show?: string;
  /** How many universes the frame layout carries. */
  readonly universes?: number;
  /**
   * A file to feed the daemon MIDI bytes through, as though a console were
   * plugged in (`--mock-surface`).
   *
   * The console half of `--mock-output`, and the only way to observe **D11**
   * from outside the process: a real daemon, a real browser and a real button
   * press, with nothing plugged into anything. Use {@link pressConsole} to
   * press one.
   */
  readonly mockSurface?: string;
  /**
   * Start the daemon the way a **venue** runs one — S37.
   *
   * No output flags at all and `--mock-devices`, so the rig comes out of this
   * machine's configuration and the machine commands are reachable. A daemon
   * whose outputs came off its command line refuses *every* machine command
   * (`MachineError::ConfiguredOnTheCommandLine`), which is right — nothing is
   * written down that run — and which means a settings test must not use
   * `--mock-output`.
   *
   * The drivers are still doubles, so nothing is opened and no datagram leaves
   * the machine: `CLAUDE.md`'s rule is untouched.
   */
  readonly configurable?: boolean;
}

/** Starts a daemon on `port`, in `dataDir` if one is given. */
export async function startDaemon(
  port: number,
  dataDir?: string,
  options: DaemonOptions = {},
): Promise<Daemon> {
  const directory = dataDir ?? mkdtempSync(join(tmpdir(), "prismdmx-e2e-"));
  const child: ChildProcess = spawn(
    binary(),
    [
      "--data-dir",
      directory,
      ...(options.configurable === true ? ["--mock-devices"] : ["--mock-output"]),
      "--websocket",
      `127.0.0.1:${port}`,
      "--no-local",
      "--log-level",
      "warn",
      ...(options.show === undefined ? [] : ["--show", options.show]),
      ...(options.universes === undefined ? [] : ["--universes", String(options.universes)]),
      ...(options.mockSurface === undefined ? [] : ["--mock-surface", options.mockSurface]),
    ],
    { stdio: "ignore" },
  );

  const url = await waitForEndpoint(directory, child);
  return {
    url,
    dataDir: directory,
    kill: async () => {
      if (child.exitCode === null) {
        child.kill();
        await new Promise<void>((done) => {
          child.on("exit", () => done());
        });
      }
    },
  };
}

/**
 * Presses a button on the console the daemon is reading from a file.
 *
 * The bytes are `docs/MCU_MAPPING.md` §2.1's, written out by the caller: Note
 * On on MIDI channel 1 with velocity 127, then the same note with velocity 0.
 * Asking the profile which note to send would be asking the code under test
 * what to press — S19's finding, and S20's method rule.
 */
export function pressConsole(path: string, note: number): void {
  sendConsole(path, [0x90, note, 127, 0x90, note, 0]);
}

/**
 * Turns the jog wheel on the console the daemon is reading from a file.
 *
 * `CC 60`, relative, sign-magnitude: bit 6 is the sign and bits 0–5 the step
 * count, so `0x01` is one detent clockwise and `0x41` one anticlockwise
 * (`docs/MCU_MAPPING.md` §2.1, and §2.7's measurement that this wheel only ever
 * sends ±1 however hard it is spun). Written out by the caller, like the note
 * numbers: asking the profile what to send would be asking the code under test.
 */
export function turnConsoleWheel(path: string, detents: number): void {
  const magnitude = Math.min(Math.abs(detents), 63);
  sendConsole(path, [0xb0, 60, detents < 0 ? 0x40 | magnitude : magnitude]);
}

/** Appends raw MIDI bytes to the file the daemon reads its console from. */
export function sendConsole(path: string, bytes: readonly number[]): void {
  appendFileSync(path, Buffer.from(bytes));
}

/** Removes a daemon's data directory once nothing is using it. */
export function forget(dataDir: string): void {
  rmSync(dataDir, { recursive: true, force: true });
}

/**
 * Copies a committed show fixture into a directory of its own and answers with
 * the path a daemon should open.
 *
 * A **copy**, because a daemon writes to the show it opens — it autosaves, and
 * it keeps a recovery file beside it — and a test that let it write to the
 * fixture would leave the working tree changed and the next run measuring
 * something else.
 */
export function showFixture(name: string): { show: string; dataDir: string } {
  const dataDir = mkdtempSync(join(tmpdir(), "prismdmx-e2e-"));
  const show = join(dataDir, name);
  copyFileSync(join(ROOT, "ui", "tests", "fixtures", name), show);
  return { show, dataDir };
}

/**
 * The WebSocket endpoint out of the lock file, once the daemon has published
 * one.
 *
 * Polling a file rather than a port: the daemon rewrites the document whenever
 * a listener binds, so an endpoint in it is one that already exists (§2.2).
 */
async function waitForEndpoint(dataDir: string, child: ChildProcess): Promise<string> {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`prismd stopped with ${String(child.exitCode)} before it published a lock file`);
    }
    const endpoint = readEndpoint(join(dataDir, "prismd.lock"));
    if (endpoint !== null) {
      return `ws://${endpoint}/ipc`;
    }
    await new Promise((wake) => setTimeout(wake, 50));
  }
  throw new Error("prismd did not publish a WebSocket endpoint");
}

/** The `websocket` member of a lock file, if it is there and readable. */
function readEndpoint(path: string): string | null {
  let text: string;
  try {
    text = readFileSync(path, "utf8");
  } catch {
    return null;
  }
  let document: unknown;
  try {
    document = JSON.parse(text);
  } catch {
    // The daemon is part-way through writing it.
    return null;
  }
  const endpoint = member(document, "websocket");
  return typeof endpoint === "string" ? endpoint : null;
}

/**
 * The member at `key`, without asserting anything about the value.
 *
 * `as` would be a claim about a file another process is writing, and the whole
 * point of polling it is that it may be half-written.
 */
function member(value: unknown, key: string): unknown {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }
  const entries: [string, unknown][] = Object.entries(value);
  return entries.find(([name]) => name === key)?.[1];
}

/**
 * Opens a window on the canvas, the way an operator does since S43.
 *
 * **Punch-list B9 took the dropdown away.** A window is opened from a chooser
 * now, and the chooser is opened by a right-click on an empty part of the
 * canvas, by Insert, or by an X-Touch key. Insert is the one used here: it is
 * the keyboard route, and a console is operated in the dark by somebody who is
 * not looking at the screen.
 *
 * Which windows are open is **session** state, so this waits for the window to
 * appear rather than for the click to return — the daemon decides, and it also
 * decides *where* (`prism_core::layout`), which is why nothing here says a
 * position.
 */
export async function openWindow(page: Page, type: string): Promise<void> {
  await openPicker(page);
  await page.getByTestId(`picker-${type}`).click();
  await expect(page.locator(`[data-window-type="${type}"]`).first()).toBeVisible();
}

/**
 * Opens the window chooser and **waits for it to be there**.
 *
 * Two things this does that a bare `keyboard.press("Insert")` does not, and both
 * were found the hard way.
 *
 * **The focus leaves the text field first.** `Insert` is ignored while the focus
 * is in one (`App.tsx::useWindowPickerKey`) and that is deliberate: the command
 * line is where an operator's hands are, and a shortcut that fired mid-word
 * would be worse than no shortcut. A test that has just typed a line — or opened
 * a window that focuses something — is in exactly that state, so it does what an
 * operator's hand does rather than asserting the rule away.
 *
 * **And it waits.** The chooser is a React render away from the key press, and a
 * caller that read the DOM straight afterwards raced it. Playwright's locators
 * wait for *actions* and assertions; `evaluateAll` does not, so it returned an
 * empty list on a loaded runner and the test read that as *the build has no
 * window types*. This is where the wait belongs, once, for every caller.
 */
export async function openPicker(page: Page): Promise<void> {
  await page.evaluate(() => {
    const focused = document.activeElement;
    if (focused instanceof HTMLElement) {
      focused.blur();
    }
  });
  await page.keyboard.press("Insert");
  await expect(page.getByTestId("window-picker")).toBeVisible();
}
