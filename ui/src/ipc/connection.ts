/**
 * The connection to `prismd`: handshake, deltas, and coming back afterwards.
 *
 * `docs/IPC_PROTOCOL.md` §8 is the specification for the interesting half of
 * this file. A daemon that dies is an **ordinary** event, not an error state:
 * the client shows a clear disconnected state, retries with backoff, and on
 * reconnect *resynchronises through a fresh snapshot*. What it must never do is
 * go on showing the values it had, as though they were still true.
 *
 * # It sends intent and receives facts, and the types say so
 *
 * {@link Connection.send} takes a `Command` and nothing else. What comes back
 * are snapshots, deltas and answers. There is no method here that lets the
 * interface tell the daemon what the state now is — that is **D3** expressed as
 * an API rather than as a rule to remember. It is also why nothing here is
 * optimistic: a client that guessed at the result of a command would be a
 * second source of truth for as long as the guess stood.
 *
 * # The socket is an argument
 *
 * Every hardware and transport seam in this project is a trait so tests need no
 * device (`CLAUDE.md`), and a WebSocket is this layer's device. {@link Socket}
 * is that seam: the browser passes {@link webSocketFactory}, and the tests pass
 * one they can drive. The same goes for the clock — a backoff nobody can
 * advance by hand is a test that waits five seconds.
 *
 * # Telemetry never touches this file's answers
 *
 * §7, last paragraph: *clients must not put telemetry into reactive state*. So
 * a telemetry payload goes to {@link ConnectionEvents.onTelemetry} and nowhere
 * else, and the store does not subscribe to it. 64 universes × 512 channels at
 * 30 Hz through React state is the interface this rule exists to prevent.
 */

import type { Command, Delta } from "../bindings";
import { logger } from "../log/logger";
import type { Payload } from "./codec";
import { decodeServerMessage, encodeClientMessage } from "./codec";
import type { ClientKind, RejectReason, ServerMessage, Snapshot } from "./protocol";
import { PROTOCOL_VERSION, closesTheConnection, describeServerMessage, hello } from "./protocol";

const log = logger("ipc");

/** The first retry delay. */
export const BACKOFF_START_MS = 100;

/** The longest retry delay, matching the reconnect backoff of the DMX drivers. */
export const BACKOFF_MAX_MS = 5000;

/** Where the interface stands with respect to the engine. */
export type ConnectionStatus =
  /** No connection yet, or a retry in flight. */
  | { readonly kind: "connecting"; readonly attempt: number }
  /** Connected, and the world has arrived. */
  | { readonly kind: "connected" }
  /** There is no daemon answering. Retrying in `retryInMs`. */
  | {
      readonly kind: "disconnected";
      readonly reason: string;
      readonly attempt: number;
      readonly retryInMs: number;
    }
  /**
   * The daemon is there and speaks another version of the protocol (§4.2).
   *
   * Kept apart from `disconnected` deliberately: "the connection was lost" and
   * "the interface and the engine are different builds" call for two different
   * things from whoever reads it, and one of them is not solved by waiting.
   */
  | {
      readonly kind: "incompatible";
      readonly ourVersion: number;
      readonly message: string;
      readonly retryInMs: number;
    };

/** What the daemon said. */
export interface ConnectionEvents {
  /** The connection state changed. */
  onStatus?: (status: ConnectionStatus) => void;
  /** The world, in answer to a hello. Every earlier value is void. */
  onSnapshot?: (snapshot: Snapshot) => void;
  /** A change that has already been applied. */
  onDelta?: (delta: Delta) => void;
  /** A telemetry payload. **Not** reactive state — see the module docs. */
  onTelemetry?: (payload: Payload) => void;
  /** A command was applied. */
  onAck?: (seq: number) => void;
  /** A command was refused, and it changed nothing. */
  onRefused?: (seq: number | null, reason: RejectReason, message: string) => void;
}

/** The half of a socket this module drives. */
export interface Socket {
  /** Sends one payload. */
  send: (payload: Payload) => void;
  /** Closes it. `onClose` still follows. */
  close: () => void;
}

/** What a socket tells this module. */
export interface SocketHandlers {
  /** The socket is open and a hello may be sent. */
  onOpen: () => void;
  /** One payload arrived. */
  onMessage: (payload: Payload) => void;
  /** The socket is gone, for the reason given. */
  onClose: (reason: string) => void;
}

/** Opens a socket to `url`. */
export type SocketFactory = (url: string, handlers: SocketHandlers) => Socket;

/** Schedules `run` after `delayMs`, answering with a function that cancels it. */
export type Timer = (run: () => void, delayMs: number) => () => void;

/** The browser's WebSocket, as a {@link SocketFactory}. */
export const webSocketFactory: SocketFactory = (url, handlers) => {
  const socket = new WebSocket(url);
  socket.binaryType = "arraybuffer";
  socket.onopen = () => {
    handlers.onOpen();
  };
  socket.onmessage = (event: MessageEvent<unknown>) => {
    const { data } = event;
    if (data instanceof ArrayBuffer) {
      handlers.onMessage(new Uint8Array(data));
      return;
    }
    // §2: this protocol carries binary frames only, in both directions.
    log.warn("a text frame arrived on a binary protocol");
  };
  socket.onclose = (event: CloseEvent) => {
    handlers.onClose(event.reason === "" ? `closed (${event.code})` : event.reason);
  };
  socket.onerror = () => {
    // `onclose` always follows, and it is the one that carries a reason.
    log.debug("the socket reported an error");
  };
  return {
    send: (payload) => {
      socket.send(payload);
    },
    close: () => {
      socket.close();
    },
  };
};

/** The default timer: `setTimeout`. */
export const realTimer: Timer = (run, delayMs) => {
  const handle = setTimeout(run, delayMs);
  return () => {
    clearTimeout(handle);
  };
};

/** How to build a connection. */
export interface ConnectionOptions {
  /** Where the daemon's WebSocket listener is. */
  readonly url: string;
  /** What kind of client this is. */
  readonly clientKind?: ClientKind;
  /** The token a listener off loopback requires (§2.1). */
  readonly token?: string | null;
  /** How to open a socket. The browser's WebSocket by default. */
  readonly socketFactory?: SocketFactory;
  /** How to wait. `setTimeout` by default. */
  readonly timer?: Timer;
}

/** Where the handshake has got to on the current socket. */
type Phase = "idle" | "opening" | "waiting-for-snapshot" | "ready";

/**
 * One connection to the daemon, reconnecting for as long as it is running.
 *
 * There is no "not yet synchronised" state to render: a client is either
 * without a daemon, or it has been given the world.
 */
export class Connection {
  readonly #url: string;
  readonly #clientKind: ClientKind;
  readonly #token: string | null;
  readonly #openSocket: SocketFactory;
  readonly #timer: Timer;
  readonly #events: ConnectionEvents;

  #socket: Socket | null = null;
  #phase: Phase = "idle";
  #running = false;
  #attempt = 0;
  #nextSeq = 0;
  #cancelRetry: (() => void) | null = null;
  #status: ConnectionStatus = { kind: "connecting", attempt: 0 };

  constructor(options: ConnectionOptions, events: ConnectionEvents = {}) {
    this.#url = options.url;
    this.#clientKind = options.clientKind ?? "Desktop";
    this.#token = options.token ?? null;
    this.#openSocket = options.socketFactory ?? webSocketFactory;
    this.#timer = options.timer ?? realTimer;
    this.#events = events;
  }

  /** Where the interface stands. */
  get status(): ConnectionStatus {
    return this.#status;
  }

  /** Whether a command can be sent right now. */
  get isReady(): boolean {
    return this.#phase === "ready";
  }

  /** Opens the connection and keeps it open. */
  start(): void {
    if (this.#running) {
      return;
    }
    this.#running = true;
    this.#connect();
  }

  /** Closes it and stops retrying. */
  stop(): void {
    this.#running = false;
    this.#cancelRetry?.();
    this.#cancelRetry = null;
    this.#phase = "idle";
    const socket = this.#socket;
    this.#socket = null;
    socket?.close();
  }

  /**
   * Sends a command, answering with the sequence number the daemon will echo.
   *
   * Answers `null` when there is no daemon to send to. Commands are **not**
   * queued for a later connection on purpose: a fader movement or a Go that
   * arrived four seconds late would be an instruction the operator has already
   * given up on, and §8 says a client that reconnects re-snapshots rather than
   * replaying what it meant to say.
   */
  send(command: Command): number | null {
    const socket = this.#socket;
    if (this.#phase !== "ready" || socket === null) {
      log.warn("a command was not sent because the daemon is not connected", {
        command: command.t,
      });
      return null;
    }
    const seq = this.#nextSeq;
    this.#nextSeq += 1;
    try {
      socket.send(encodeClientMessage({ t: "Command", seq, command }));
    } catch (cause) {
      log.error("a command could not be sent", {
        command: command.t,
        cause: cause instanceof Error ? cause.message : String(cause),
      });
      return null;
    }
    log.debug("sent a command", { command: command.t, seq });
    return seq;
  }

  /**
   * Drops the connection and asks for a fresh snapshot.
   *
   * The answer to a mirror that has diverged from the daemon (§6: a client
   * applies deltas without validating them, so an operation that does not fit
   * means the two disagree already). Reconnecting is not a heavy remedy — it is
   * the ordinary one, and it is exactly what the client does when a daemon
   * restarts.
   */
  resync(reason: string): void {
    log.warn("resynchronising", { reason });
    this.#dropSocket();
    this.#scheduleRetry(reason);
  }

  /* --------------------------------------------------------------------- */

  #connect(): void {
    this.#phase = "opening";
    this.#publish({ kind: "connecting", attempt: this.#attempt });
    const socket = this.#openSocket(this.#url, {
      onOpen: () => {
        this.#onOpen(socket);
      },
      onMessage: (payload) => {
        this.#onMessage(socket, payload);
      },
      onClose: (reason) => {
        this.#onClose(socket, reason);
      },
    });
    this.#socket = socket;
  }

  #onOpen(socket: Socket): void {
    if (socket !== this.#socket) {
      return;
    }
    this.#phase = "waiting-for-snapshot";
    socket.send(
      encodeClientMessage({ t: "Hello", hello: hello(this.#clientKind, this.#token) }),
    );
    log.debug("said hello", { version: PROTOCOL_VERSION });
  }

  #onMessage(socket: Socket, payload: Payload): void {
    if (socket !== this.#socket) {
      return;
    }
    let message;
    try {
      message = decodeServerMessage(payload);
    } catch (cause) {
      // A payload this build cannot read leaves the mirror unable to follow the
      // daemon, and a client that guessed would be worse than one that starts
      // again. §8 keeps the *daemon* connected in the same situation because
      // the frame boundary survived; here it is the meaning that did not.
      log.error("the daemon sent something this build cannot read", {
        cause: cause instanceof Error ? cause.message : String(cause),
      });
      this.resync("the daemon sent something this build cannot read");
      return;
    }

    if (this.#phase === "waiting-for-snapshot") {
      this.#onHandshakeAnswer(message);
      return;
    }

    switch (message.t) {
      case "Delta":
        this.#events.onDelta?.(message.delta);
        return;
      case "Telemetry":
        this.#events.onTelemetry?.(message.data);
        return;
      case "Ack":
        this.#events.onAck?.(message.seq);
        return;
      case "Reject":
        this.#events.onRefused?.(message.seq, message.reason, message.message);
        if (closesTheConnection(message.reason)) {
          // The daemon is about to close this connection itself (§8). Saying so
          // here rather than waiting for the socket is what puts the reason in
          // front of the operator instead of "closed (1006)".
          log.warn("the daemon ended the connection", {
            reason: message.reason,
            message: message.message,
          });
          this.#dropSocket();
          this.#scheduleRetry(message.message);
        }
        return;
      case "Snapshot":
        // A second snapshot is not part of this version of the protocol: a
        // client that needs one reconnects, which is what §8 says it does.
        log.error("a second snapshot arrived on an open connection");
        this.resync("the daemon sent a second snapshot");
        return;
    }
  }

  #onHandshakeAnswer(message: ServerMessage): void {
    if (message.t === "Snapshot") {
      this.#phase = "ready";
      this.#attempt = 0;
      this.#nextSeq = 0;
      log.info("connected to the daemon", {
        version: message.snapshot.health.protocolVersion,
        outputs: message.snapshot.outputs.length,
      });
      this.#events.onSnapshot?.(message.snapshot);
      this.#publish({ kind: "connected" });
      return;
    }
    if (message.t === "Reject") {
      this.#events.onRefused?.(message.seq, message.reason, message.message);
      this.#dropSocket();
      if (message.reason === "ProtocolVersion") {
        // Waiting will not help unless the other half is replaced, so this says
        // what is wrong rather than counting attempts at it. The retry stays,
        // at its longest interval: the usual way this is fixed is that the
        // daemon is restarted from a matching build.
        log.error("the interface and the engine are different versions", {
          ourVersion: PROTOCOL_VERSION,
          daemon: message.message,
        });
        this.#attempt = attemptsToMaximum();
        this.#arm(BACKOFF_MAX_MS);
        this.#publish({
          kind: "incompatible",
          ourVersion: PROTOCOL_VERSION,
          message: message.message,
          retryInMs: BACKOFF_MAX_MS,
        });
        return;
      }
      log.error("the daemon refused the connection", {
        reason: message.reason,
        message: message.message,
      });
      this.#scheduleRetry(message.message);
      return;
    }
    log.error("the daemon answered the handshake with something else", {
      answer: describeServerMessage(message),
    });
    this.#dropSocket();
    this.#scheduleRetry(`the daemon answered the handshake with ${describeServerMessage(message)}`);
  }

  #onClose(socket: Socket, reason: string): void {
    if (socket !== this.#socket) {
      return;
    }
    this.#socket = null;
    this.#phase = "idle";
    log.warn("the connection to the daemon is gone", { reason });
    this.#scheduleRetry(reason);
  }

  /** Closes the socket without letting its `onClose` schedule anything. */
  #dropSocket(): void {
    const socket = this.#socket;
    this.#socket = null;
    this.#phase = "idle";
    socket?.close();
  }

  #scheduleRetry(reason: string): void {
    if (!this.#running) {
      return;
    }
    const retryInMs = backoffMs(this.#attempt);
    this.#attempt += 1;
    this.#arm(retryInMs);
    this.#publish({ kind: "disconnected", reason, attempt: this.#attempt, retryInMs });
  }

  #arm(delayMs: number): void {
    this.#cancelRetry?.();
    this.#cancelRetry = this.#timer(() => {
      this.#cancelRetry = null;
      if (this.#running) {
        this.#connect();
      }
    }, delayMs);
  }

  #publish(status: ConnectionStatus): void {
    this.#status = status;
    this.#events.onStatus?.(status);
  }
}

/**
 * The delay before attempt number `attempt`: 100 ms doubling to 5 s.
 *
 * The same shape as the output drivers' reconnect backoff (S7), and for the
 * same reason: the first retry should be fast enough that a daemon restarting
 * is barely visible, and the tenth should not be hammering a machine that is
 * switched off.
 */
export function backoffMs(attempt: number): number {
  const delay = BACKOFF_START_MS * 2 ** Math.max(0, attempt);
  return Math.min(delay, BACKOFF_MAX_MS);
}

/** How many attempts it takes to reach {@link BACKOFF_MAX_MS}. */
function attemptsToMaximum(): number {
  let attempt = 0;
  while (backoffMs(attempt) < BACKOFF_MAX_MS) {
    attempt += 1;
  }
  return attempt;
}
