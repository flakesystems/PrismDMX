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

import { spawn, spawnSync } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
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
      "--mock-output",
      "--websocket",
      `127.0.0.1:${port}`,
      "--no-local",
      "--log-level",
      "warn",
      ...(options.show === undefined ? [] : ["--show", options.show]),
      ...(options.universes === undefined ? [] : ["--universes", String(options.universes)]),
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
