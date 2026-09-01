/**
 * What the interface can ask the desktop shell for, and what it does without
 * one — S29.
 *
 * # The interface runs in two places and this file is the whole of the difference
 *
 * Inside `prism-app` there is an operating system to ask for a path; in a
 * browser — the Web Remote (S31), the end-to-end suite, `npm run dev` — there is
 * not, and there never will be: `showOpenFilePicker` hands a page a *handle*,
 * never a path, and the daemon needs a path because the daemon is what opens the
 * file. So the box an operator types into stays exactly where S37 put it, and
 * what the shell adds is a **second way to fill it in**.
 *
 * Everything below therefore answers `null` rather than throwing when there is
 * no shell, and every caller draws the extra button only when {@link inShell}
 * says there is one. A panel that offered a *Browse…* button in a browser would
 * be a button that does nothing, which is the fault S37's *held by a flag* rows
 * exist to prevent, one layer up.
 *
 * # Nothing here knows what a dialogue looks like
 *
 * The titles, the filters and whether a place wants an open, a save or a
 * directory are `crates/prism-app/src/dialogs.rs`'s table, because the shell is
 * what opens the dialogue. This file carries the **names** and nothing else, and
 * `bridge.test.ts` holds the list of names to the one the settings window uses.
 */

import { logger } from "../log/logger";

const log = logger("shell");

/**
 * A place in the interface where a path is asked for.
 *
 * The five show-file names are `settings/showfiles.tsx`'s `Asking` values and
 * the protocol's five file commands, spelled identically in all three places —
 * see `prism_app::dialogs::PathKind`, whose own test asserts the correspondence
 * from the Rust side.
 */
export type PathKind =
  | "OpenShow"
  | "SaveShowAs"
  | "NewShow"
  | "ExportShow"
  | "ImportShow"
  | "FixtureLibrary"
  | "SurfaceProfile";

/** What the shell says this machine's start-up entry actually is. */
export interface AutostartReport {
  /** Whether this build can write a start-up entry at all. */
  readonly supported: boolean;
  /** Whether one exists. */
  readonly installed: boolean;
  /** What it carries, when there is one. */
  readonly command: string | null;
  /** Whether that is *this* installation's, rather than another copy's. */
  readonly matchesThisInstall: boolean;
}

/** The half of Tauri this file uses, so a test can supply one. */
interface Invoker {
  (command: string, args?: Record<string, unknown>): Promise<unknown>;
}

/** What Tauri puts on `window` when a page is running inside a shell. */
interface ShellWindow {
  __TAURI_INTERNALS__?: { invoke?: Invoker };
}

/** The invoker this page has, or `null` in a browser. */
function invoker(): Invoker | null {
  if (typeof window === "undefined") {
    return null;
  }
  const internals = (window as unknown as ShellWindow).__TAURI_INTERNALS__;
  return typeof internals?.invoke === "function" ? internals.invoke : null;
}

/**
 * Whether this interface is running inside the desktop shell.
 *
 * Asked at the moment it is needed rather than remembered, because a component
 * can be rendered by a test that has just installed a fake shell — and because
 * a value computed at module load would be a value the end-to-end suite could
 * not change.
 */
export function inShell(): boolean {
  return invoker() !== null;
}

/**
 * Opens the operating system's own file dialogue and answers with what was
 * picked — **B31**.
 *
 * `null` for three different things, all of which mean *leave the box alone*:
 * there is no shell, the operator cancelled, or the shell could not open a
 * dialogue. They are one answer here because the caller does the same thing in
 * every case; the third is logged, because it is the only one that is a fault.
 *
 * `start` is what the box already holds, so a *Save as* on an open show opens in
 * that show's own folder.
 */
export async function choosePath(kind: PathKind, start?: string): Promise<string | null> {
  const invoke = invoker();
  if (invoke === null) {
    return null;
  }
  try {
    const chosen = await invoke("choose_path", {
      kind,
      start: start === undefined || start === "" ? null : start,
    });
    return typeof chosen === "string" && chosen !== "" ? chosen : null;
  } catch (error) {
    log.warn("the file dialogue could not be opened", { kind, error: String(error) });
    return null;
  }
}

/**
 * What this machine's start-up entry actually is, or `null` outside the shell.
 *
 * The **reading** half of *a switch that displays a lie is worse than no
 * switch*: the stored flag says what the operator asked for, and this says what
 * the machine has.
 */
export async function autostartState(): Promise<AutostartReport | null> {
  return await ask("autostart_state", {});
}

/**
 * Writes or removes the start-up entry, the moment the box is ticked.
 *
 * Sent **beside** the `ConfigureMachine` that stores the flag rather than
 * instead of it: the flag is a setting every client can read back and the entry
 * is a fact about this Windows account, and neither can stand in for the other.
 */
export async function autostartApply(wanted: boolean): Promise<AutostartReport | null> {
  return await ask("autostart_apply", { wanted });
}

/** One invocation that answers a report, or `null` for every way of not having one. */
async function ask(
  command: string,
  args: Record<string, unknown>,
): Promise<AutostartReport | null> {
  const invoke = invoker();
  if (invoke === null) {
    return null;
  }
  try {
    return asReport(await invoke(command, args));
  } catch (error) {
    log.warn("the shell could not answer", { command, error: String(error) });
    return null;
  }
}

/**
 * Narrows what came back over the bridge.
 *
 * Checked rather than cast, for `ui/src/ipc/shape.ts`'s reason one bridge along:
 * what arrives is `unknown`, and a panel that drew a tick because a field was
 * `undefined` would be the switch displaying a lie by a different route.
 */
function asReport(value: unknown): AutostartReport | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const record = value as Record<string, unknown>;
  if (typeof record.supported !== "boolean" || typeof record.installed !== "boolean") {
    return null;
  }
  return {
    supported: record.supported,
    installed: record.installed,
    command: typeof record.command === "string" ? record.command : null,
    matchesThisInstall: record.matchesThisInstall === true,
  };
}
