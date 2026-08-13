/**
 * The wiring: one store, one connection, one telemetry sink.
 *
 * Everything below this file is testable without the others — a store with no
 * socket, a connection with no React — and this is the twenty lines that put
 * them together. `main.tsx` calls it once; the end-to-end tests get the same
 * object the operator does, and the unit tests build one with a socket they can
 * drive.
 */

import { Connection } from "./ipc/connection";
import type { ConnectionOptions } from "./ipc/connection";
import { daemonUrl } from "./ipc/endpoint";
import { TelemetrySink } from "./ipc/telemetry";
import { DeskStore, deskEvents } from "./store/desk";

/** A wired-up interface. */
export interface Desk {
  /** What the views render from. */
  readonly store: DeskStore;
  /** The connection to `prismd`. */
  readonly connection: Connection;
  /** Where telemetry goes, which is not into the store (§7). */
  readonly telemetry: TelemetrySink;
  /** Opens the connection and keeps it open. */
  start: () => void;
  /** Closes it and stops retrying. */
  stop: () => void;
}

/** How to build one. Everything has a default that works in a browser. */
export type DeskOptions = Partial<ConnectionOptions>;

/** Builds the interface's one store and its connection. */
export function createDesk(options: DeskOptions = {}): Desk {
  const store = new DeskStore();
  const telemetry = new TelemetrySink();

  const events = deskEvents(store, (reason) => {
    connection.resync(reason);
  });
  const connection = new Connection(
    { ...options, url: options.url ?? daemonUrl({ search: locationSearch() }) },
    {
      ...events,
      onStatus: (status) => {
        events.onStatus?.(status);
        if (status.kind !== "connected") {
          // A picture of the rig from a daemon that has stopped is exactly as
          // stale as a fader value from one.
          telemetry.clear();
        }
      },
      onTelemetry: telemetry.accept,
    },
  );

  store.attach((command) => connection.send(command));

  return {
    store,
    connection,
    telemetry,
    start: () => {
      connection.start();
    },
    stop: () => {
      connection.stop();
    },
  };
}

/** The query string, when there is a `window` to read it from. */
function locationSearch(): string {
  return typeof window === "undefined" ? "" : window.location.search;
}
