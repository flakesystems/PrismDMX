/**
 * A daemon that is not there: the sockets, the clock and the messages the unit
 * tests drive the connection with.
 *
 * `CLAUDE.md` requires every hardware and transport seam to be a trait so tests
 * run with nothing attached, and this is the other side of `Socket`,
 * `SocketFactory` and `Timer`. Nothing here is shipped — it is excluded from
 * coverage for the same reason `prism-core`'s `testkit` is a module of its own:
 * scenery that measured itself would flatter every figure in `PROGRESS.md`.
 *
 * The messages are encoded with the *real* MessagePack encoder, so what the
 * connection decodes in a unit test is a payload of exactly the shape a daemon
 * sends. What makes that shape right is checked elsewhere, against a recording
 * of a real one (`src/mirror/recording.test.ts`).
 */

import { encode } from "@msgpack/msgpack";

import type { Answer, Delta, FixtureType, ProgrammerState } from "../bindings";
import type { Socket, SocketFactory, SocketHandlers, Timer } from "../ipc/connection";
import type { DaemonHealth, RejectReason, Snapshot } from "../ipc/protocol";
import type { Payload } from "../ipc/shape";
import { overArrayBuffer } from "../ipc/shape";

/** One socket the test can drive from both ends. */
export class FakeSocket implements Socket {
  /** Everything the client sent, in order. */
  readonly sent: Payload[] = [];
  /** Whether the client closed it. */
  closed = false;
  /** What the connection wants to be told. */
  readonly handlers: SocketHandlers;
  /** Where it was opened. */
  readonly url: string;

  constructor(url: string, handlers: SocketHandlers) {
    this.url = url;
    this.handlers = handlers;
  }

  send = (payload: Payload): void => {
    this.sent.push(payload);
  };

  close = (): void => {
    this.closed = true;
  };

  /** The socket finished connecting. */
  open(): void {
    this.handlers.onOpen();
  }

  /** The daemon said something. */
  deliver(payload: Uint8Array): void {
    this.handlers.onMessage(overArrayBuffer(payload));
  }

  /** The connection went, for the reason given. */
  drop(reason = "the daemon stopped"): void {
    this.closed = true;
    this.handlers.onClose(reason);
  }
}

/** Every socket a connection opened, in order. */
export class FakeNetwork {
  /** Every socket, oldest first. */
  readonly sockets: FakeSocket[] = [];

  /** The factory to hand the connection. */
  readonly factory: SocketFactory = (url, handlers) => {
    const socket = new FakeSocket(url, handlers);
    this.sockets.push(socket);
    return socket;
  };

  /** The socket most recently opened. */
  get last(): FakeSocket {
    const socket = this.sockets.at(-1);
    if (socket === undefined) {
      throw new Error("no socket has been opened");
    }
    return socket;
  }

  /** Opens the newest socket and lets the handshake begin. */
  openAndHandshake(snapshot: Snapshot): FakeSocket {
    const socket = this.last;
    socket.open();
    socket.deliver(serverMessage({ t: "Snapshot", snapshot }));
    return socket;
  }
}

/** A clock the test advances by hand. */
export class ManualTimer {
  /** What is waiting, oldest first. */
  readonly pending: { run: () => void; delayMs: number; cancelled: boolean }[] = [];

  /** The timer to hand the connection. */
  readonly timer: Timer = (run, delayMs) => {
    const entry = { run, delayMs, cancelled: false };
    this.pending.push(entry);
    return () => {
      entry.cancelled = true;
    };
  };

  /** How long the next thing waiting was asked to wait. */
  get nextDelay(): number {
    const entry = this.pending.filter((waiting) => !waiting.cancelled).at(-1);
    if (entry === undefined) {
      throw new Error("nothing is waiting");
    }
    return entry.delayMs;
  }

  /** Runs the most recently armed timer, which is the live one. */
  fire(): void {
    const entry = this.pending.filter((waiting) => !waiting.cancelled).at(-1);
    if (entry === undefined) {
      throw new Error("nothing is waiting");
    }
    entry.cancelled = true;
    entry.run();
  }
}

/** A programmer with nothing in it. */
export function emptyProgrammer(): ProgrammerState {
  return { selection: [], activeFeatureGroup: "Dimmer", values: [], clearStage: 0 };
}

/**
 * The profiles the desk carries, in the shape `prism_core::library` serves.
 *
 * Two of them, because a menu with one entry cannot tell a component that reads
 * the list from one that draws the first thing it finds.
 */
export function fixtureLibrary(): FixtureType[] {
  return [
    {
      id: "generic.dimmer",
      manufacturer: "Generic",
      name: "Dimmer",
      mode: "1ch",
      footprint: 1,
      attributes: [
        {
          attribute: "Dimmer",
          featureGroup: "Dimmer",
          coarseOffset: 0,
          fineOffset: null,
          defaultValue: 0,
          mergeMode: "HTP",
          invert: false,
          physicalFrom: 0,
          physicalTo: 100,
        },
      ],
    },
    {
      id: "generic.rgbw.par",
      manufacturer: "Generic",
      name: "RGBW PAR",
      mode: "4ch",
      footprint: 4,
      attributes: (["Red", "Green", "Blue", "White"] as const).map((attribute, index) => ({
        attribute,
        featureGroup: "Color" as const,
        coarseOffset: index,
        fineOffset: null,
        defaultValue: 0,
        mergeMode: "LTP" as const,
        invert: false,
        physicalFrom: 0,
        physicalTo: 100,
      })),
    },
  ];
}

/** The daemon's health, at its defaults. */
export function health(overrides: Partial<DaemonHealth> = {}): DaemonHealth {
  return {
    protocolVersion: 1,
    tickHz: 44,
    missedTicks: 0,
    unsavedChanges: false,
    ...overrides,
  };
}

/**
 * A snapshot in the shape `prismd` serves: a show document, a session document
 * `{ session, views }`, and a programmer.
 *
 * Deliberately **not** at its defaults, for S14's reason: a fixture built out
 * of default values cannot tell "carried across" from "never filled in".
 */
export function snapshot(overrides: Partial<Snapshot> = {}): Snapshot {
  return {
    show: {
      fixtures: { "1": { name: "Front", universe: 1, address: 1 } },
      groups: {},
      sequences: {},
      executors: { "0": { isActive: false, currentCueIndex: null, masterLevel: 65535 } },
    },
    session: {
      session: {
        activeViewId: 1,
        executorPage: 3,
        encoderBank: "Dimmer",
        commandLine: "fixture 1 at full",
        // One window, because a canvas with nothing on it cannot tell a test
        // that draws windows from one that does not — and because the level
        // view lives in a `DmxSheet` from S25 onwards, so the telemetry tests
        // need one open to have anywhere to draw.
        openWindows: [
          { instanceId: 1, type: "DmxSheet", x: 0, y: 0, w: 640, h: 480, params: {} },
        ],
        focusedWindow: 1,
      },
      views: { "1": { id: 1, name: "View 1", windows: [] } },
    },
    programmer: emptyProgrammer(),
    outputs: [
      {
        id: 1,
        name: "Mock",
        health: "Ok",
        // The configured row, as S33 made a snapshot carry it: a status panel
        // draws the kind and the universes beside the light.
        output: {
          id: 1,
          name: "Mock",
          kind: { t: "Mock" },
          universes: [1],
          enabled: true,
        },
        framesSent: 0,
        lastError: null,
        lastErrorAgoMs: null,
      },
    ],
    health: health(),
    fixtureLibrary: fixtureLibrary().length,
    ...overrides,
  };
}

/** One message from the daemon, as bytes. */
export function serverMessage(
  message:
    | { t: "Snapshot"; snapshot: Snapshot }
    | { t: "Delta"; delta: Delta }
    | { t: "Telemetry"; data: Uint8Array }
    | { t: "Ack"; seq: number }
    | { t: "Answer"; seq: number; answer: Answer }
    | { t: "Reject"; seq: number | null; reason: RejectReason; message: string },
): Uint8Array {
  return encode(message, { ignoreUndefined: true });
}
