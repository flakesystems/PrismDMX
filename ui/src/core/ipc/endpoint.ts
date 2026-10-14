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
 * **The desktop shell has the file, and passes what it read as (1)** — S29, and
 * it is (1) rather than a fourth source of its own precisely so that nothing
 * about the interface is special-cased for the shell: what a shell supplies is
 * what an operator with two engines on a bench types into the address bar.
 *
 * The §2.1 token travels the same way, when there is one ({@link daemonToken}).
 * It is a secret in a query string and that is a smaller thing than it sounds:
 * the URL is `tauri://localhost/…`, it is never requested over a network and
 * never leaves the machine, and a page that can read `location.search` can
 * equally read any global the shell might have set instead. What actually keeps
 * it in is the content-security policy in `crates/prism-app/tauri.conf.json`,
 * which lets this page open a WebSocket to loopback and reach nothing else.
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
 * The §2.1 token, when the desktop shell found one in the discovery document.
 *
 * `null` in a browser and on a loopback listener, which is the ordinary case:
 * `docs/IPC_PROTOCOL.md` §2.1 asks for a token only when the listener can be
 * reached from another machine. A token in the address bar of a Web Remote is
 * the one an operator was given, and it travels the same way for the same
 * reason.
 */
export function daemonToken(sources: EndpointSources = {}): string | null {
  const asked = new URLSearchParams(sources.search ?? "").get("token");
  return asked === null || asked === "" ? null : asked;
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
