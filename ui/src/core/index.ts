/**
 * **The one place a view imports from.**
 *
 * Everything below `core/` is the connection to `prismd` and the reading of
 * what it sends — written, tested, and not part of the redesign. A component in
 * `src/app/` should need nothing but this file:
 *
 * ```ts
 * import { useShow, useRunLine, listOf, type Fixture } from "../core";
 * ```
 *
 * - **Types** — every type the daemon speaks (`Fixture`, `Cue`, `Command`, …)
 *   is generated from Rust into `src/bindings/` and re-exported here. Do not
 *   edit those files; `npm run bindings` writes them.
 * - **Hooks** — `useShow`, `useSession`, `useProgrammer`, `useConnectionStatus`,
 *   … read the state; `useSend` and `useRunLine` change it, by asking the daemon.
 * - **Model helpers** — pure functions that turn the documents into what a
 *   screen shows: the open windows, the stored views, the programmer readings.
 *
 * `ui/REDESIGN.md` is the guide.
 */

// ---- every generated type, and the run-time tables of their variants -----
export type * from "../bindings";
export * from "../bindings/variants";

// ---- wiring: built once in `main.tsx` --------------------------------------
export { createDesk } from "./desk";
export type { Desk, DeskOptions } from "./desk";
export { DeskProvider } from "./store/context";

// ---- reading and changing the state ----------------------------------------
export * from "./hooks";
export { useAsk, useDesk, useDeskStore, useSend } from "./store/hooks";
export type { DeskState, LibraryUpdateState, Notice } from "./store/desk";
export type { ConnectionStatus } from "./ipc/connection";
export type { DaemonHealth, OutputSnapshot } from "./ipc/protocol";
export type { Documents } from "./mirror/mirror";
export { statusText } from "./status";

// ---- the documents, typed --------------------------------------------------
export * from "./model/documents";
export { booleanAt, numberAt, stringAt, valueAt } from "./mirror/select";

// ---- what a screen shows, derived from the documents -----------------------
export * from "./model/windows";
export * from "./model/geometry";
export * from "./model/session";
export * from "./model/programmer";
export * from "./model/functions";
export { CLEAR_TITLES, titleOf } from "./model/keys";
export type { ConsoleKey } from "./model/keys";

// ---- the desktop shell (file dialogues, full screen) -----------------------
export * from "./shell/bridge";
export * from "./shell/fullscreen";

// ---- logging: `console.log` is not allowed in this code base ----------------
export { logger } from "./log/logger";
export type { Logger } from "./log/logger";
