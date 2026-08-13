/**
 * Where the daemon is.
 *
 * `docs/IPC_PROTOCOL.md` §2.2 says a client finds `prismd` by reading the lock
 * file it writes into the user data directory. A browser cannot read a file, so
 * the WebSocket client needs the endpoint another way, and there are three:
 *
 * 1. `?daemon=ws://host:port/ipc` in the address, which is what an operator
 *    with two engines on a bench types and what the end-to-end tests use;
 * 2. `VITE_PRISMD_WS` at build time, for a Web Remote served from somewhere
 *    other than the daemon itself;
 * 3. the default listener address, `prismd`'s own `DEFAULT_WEBSOCKET`.
 *
 * The desktop shell (S29) has the file, and will pass what it read as (1).
 */

/** The path clients upgrade on, matching `prism_ipc::websocket::IPC_PATH`. */
export const IPC_PATH = "/ipc";

/** `prismd --websocket` with no address, matching `prismd::cli::DEFAULT_WEBSOCKET`. */
export const DEFAULT_WEBSOCKET = "127.0.0.1:7373";

/** The default endpoint. */
export const DEFAULT_URL = `ws://${DEFAULT_WEBSOCKET}${IPC_PATH}`;

/** What {@link daemonUrl} needs to know about where it is running. */
export interface EndpointSources {
  /** The query string, `window.location.search`. */
  readonly search?: string;
  /** The build-time override. */
  readonly configured?: string | undefined;
}

/**
 * The daemon's WebSocket URL.
 *
 * A `daemon` parameter that is not a WebSocket URL is ignored rather than
 * obeyed: it arrives from the address bar, and an interface that tried to open
 * `http://` — or something worse — because a query string said so would be
 * taking instructions from wherever the link came from.
 */
export function daemonUrl(sources: EndpointSources = {}): string {
  const asked = new URLSearchParams(sources.search ?? "").get("daemon") ?? sources.configured;
  if (asked !== undefined && asked !== null && isWebSocketUrl(asked)) {
    return asked;
  }
  return DEFAULT_URL;
}

/** Whether `text` is a `ws://` or `wss://` URL. */
export function isWebSocketUrl(text: string): boolean {
  try {
    const url = new URL(text);
    return url.protocol === "ws:" || url.protocol === "wss:";
  } catch {
    return false;
  }
}
