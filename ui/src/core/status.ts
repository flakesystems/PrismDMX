/**
 * What each connection state is called on screen.
 *
 * Its own module because it is the one piece of `App.tsx` that is not a
 * component: the wording of `docs/IPC_PROTOCOL.md` §8 — *a clear disconnected
 * state* — and of §4.2 — *the interface and the engine are different versions*
 * — is a decision worth testing on its own, and worth keeping out of a file
 * that fast refresh wants to hold components only.
 */

import type { ConnectionStatus } from "./ipc/connection";

/** What to call each state. */
export function statusText(status: ConnectionStatus): string {
  switch (status.kind) {
    case "connecting":
      return "Connecting to the engine…";
    case "connected":
      return "Connected";
    case "disconnected":
      return `Disconnected — retrying in ${Math.round(status.retryInMs / 100) / 10} s`;
    case "incompatible":
      // §4.2: not "connection lost". The operator has to be told which half to
      // update, and waiting will not fix it.
      return `The interface and the engine are different versions — this interface speaks ${status.ourVersion}`;
  }
}
