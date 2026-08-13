/**
 * The connection: the handshake, what a `Reject` means, and coming back.
 *
 * Everything here runs against {@link FakeNetwork} and {@link ManualTimer}, so
 * the backoff is asserted rather than waited for and a "daemon restart" is one
 * line. What the fake sends is encoded with the real MessagePack encoder; that
 * the *shape* is a daemon's shape is checked in `src/mirror/recording.test.ts`
 * against a recording of one.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { setLogSink, nullSink } from "../log/logger";
import {
  FakeNetwork,
  ManualTimer,
  serverMessage,
  snapshot as aSnapshot,
} from "../testing/fake-daemon";
import { BACKOFF_MAX_MS, BACKOFF_START_MS, Connection, backoffMs } from "./connection";
import type { ConnectionEvents, ConnectionStatus } from "./connection";
import { decodeServerMessage } from "./codec";
import type { ClientMessage } from "./protocol";
import { decode } from "@msgpack/msgpack";

beforeEach(() => {
  // The logger's default sink is the console, and a test suite that printed
  // every reconnection would bury its own failures.
  setLogSink(nullSink);
});

/** A connection over a network and a clock the test owns. */
function connected(events: ConnectionEvents = {}) {
  const network = new FakeNetwork();
  const clock = new ManualTimer();
  const statuses: ConnectionStatus[] = [];
  const connection = new Connection(
    { url: "ws://127.0.0.1:7373/ipc", socketFactory: network.factory, timer: clock.timer },
    {
      ...events,
      onStatus: (status) => {
        statuses.push(status);
        events.onStatus?.(status);
      },
    },
  );
  return { network, clock, connection, statuses };
}

/** What a client sent, decoded. */
function sentMessages(socket: { sent: Uint8Array[] }): ClientMessage[] {
  return socket.sent.map((payload) => decode(payload) as ClientMessage);
}

describe("the handshake", () => {
  it("says hello and is given the world", () => {
    const snapshots: unknown[] = [];
    const { network, connection, statuses } = connected({
      onSnapshot: (snapshot) => snapshots.push(snapshot),
    });
    connection.start();

    expect(network.sockets).toHaveLength(1);
    expect(network.last.url).toBe("ws://127.0.0.1:7373/ipc");
    expect(statuses[0]).toEqual({ kind: "connecting", attempt: 0 });

    network.last.open();
    const hello = sentMessages(network.last)[0];
    expect(hello).toEqual({
      t: "Hello",
      hello: { protocolVersion: 1, clientKind: "Desktop", token: null },
    });

    network.last.deliver(serverMessage({ t: "Snapshot", snapshot: aSnapshot() }));
    expect(connection.status).toEqual({ kind: "connected" });
    expect(connection.isReady).toBe(true);
    expect(snapshots).toHaveLength(1);
  });

  it("carries the token and the client kind it was built with", () => {
    const network = new FakeNetwork();
    const clock = new ManualTimer();
    const connection = new Connection({
      url: "ws://host/ipc",
      clientKind: "WebRemote",
      token: "hunter2",
      socketFactory: network.factory,
      timer: clock.timer,
    });
    connection.start();
    network.last.open();
    expect(sentMessages(network.last)[0]).toEqual({
      t: "Hello",
      hello: { protocolVersion: 1, clientKind: "WebRemote", token: "hunter2" },
    });
  });

  it("starting twice opens one socket", () => {
    const { network, connection } = connected();
    connection.start();
    connection.start();
    expect(network.sockets).toHaveLength(1);
  });
});

describe("a refusal", () => {
  it("distinguishes a different version from a lost connection", () => {
    const { network, connection, clock } = connected();
    connection.start();
    network.last.open();
    network.last.deliver(
      serverMessage({
        t: "Reject",
        seq: null,
        reason: "ProtocolVersion",
        message: "this daemon speaks 2, the client speaks 1",
      }),
    );

    // §4.2: the operator has to be told which half to update, and waiting is
    // not the remedy — so this is its own state and it retries slowly.
    expect(connection.status).toEqual({
      kind: "incompatible",
      ourVersion: 1,
      message: "this daemon speaks 2, the client speaks 1",
      retryInMs: BACKOFF_MAX_MS,
    });
    expect(clock.nextDelay).toBe(BACKOFF_MAX_MS);
    expect(network.last.closed).toBe(true);
  });

  it("retries after any other refused handshake", () => {
    const { network, connection } = connected();
    connection.start();
    network.last.open();
    network.last.deliver(
      serverMessage({
        t: "Reject",
        seq: null,
        reason: "Unauthorised",
        message: "this listener needs a token",
      }),
    );
    expect(connection.status).toEqual({
      kind: "disconnected",
      reason: "this listener needs a token",
      attempt: 1,
      retryInMs: BACKOFF_START_MS,
    });
  });

  it("refusing a command leaves the connection open", () => {
    const refusals: string[] = [];
    const { network, connection } = connected({
      onRefused: (_seq, _reason, message) => refusals.push(message),
    });
    connection.start();
    network.openAndHandshake(aSnapshot());
    network.last.deliver(
      serverMessage({
        t: "Reject",
        seq: 4,
        reason: "CommandRefused",
        message: "no executor 9",
      }),
    );
    // §5: the command changed nothing, and the connection is perfectly usable.
    expect(refusals).toEqual(["no executor 9"]);
    expect(connection.isReady).toBe(true);
    expect(network.sockets).toHaveLength(1);
  });

  it("a refusal that ends the connection is acted on before the socket closes", () => {
    const { network, connection } = connected();
    connection.start();
    network.openAndHandshake(aSnapshot());
    network.last.deliver(
      serverMessage({
        t: "Reject",
        seq: null,
        reason: "Backpressure",
        message: "this client stopped reading",
      }),
    );
    // §8: the daemon disconnects it. Saying so here is what puts the reason in
    // front of the operator instead of a WebSocket close code.
    expect(connection.status).toEqual({
      kind: "disconnected",
      reason: "this client stopped reading",
      attempt: 1,
      retryInMs: BACKOFF_START_MS,
    });
    expect(network.last.closed).toBe(true);
  });

  it("an answer to the handshake that is not a snapshot is not guessed at", () => {
    const { network, connection } = connected();
    connection.start();
    network.last.open();
    network.last.deliver(serverMessage({ t: "Ack", seq: 0 }));
    expect(connection.status.kind).toBe("disconnected");
    expect(connection.isReady).toBe(false);
  });
});

describe("what arrives on an open connection", () => {
  it("hands each message to the event for it", () => {
    const deltas: string[] = [];
    const telemetry: number[] = [];
    const acks: number[] = [];
    const { network, connection } = connected({
      onDelta: (delta) => deltas.push(delta.t),
      onTelemetry: (payload) => telemetry.push(payload.byteLength),
      onAck: (seq) => acks.push(seq),
    });
    connection.start();
    network.openAndHandshake(aSnapshot());

    network.last.deliver(
      serverMessage({ t: "Delta", delta: { t: "DirtyFlag", unsavedChanges: true } }),
    );
    network.last.deliver(serverMessage({ t: "Telemetry", data: new Uint8Array(514) }));
    network.last.deliver(serverMessage({ t: "Ack", seq: 7 }));

    expect(deltas).toEqual(["DirtyFlag"]);
    expect(telemetry).toEqual([514]);
    expect(acks).toEqual([7]);
  });

  it("a second snapshot is not part of this protocol", () => {
    const { network, connection } = connected();
    connection.start();
    network.openAndHandshake(aSnapshot());
    network.last.deliver(serverMessage({ t: "Snapshot", snapshot: aSnapshot() }));
    expect(connection.status.kind).toBe("disconnected");
    expect(network.sockets[0]?.closed).toBe(true);
  });

  it("a payload this build cannot read starts the connection again", () => {
    const { network, connection } = connected();
    connection.start();
    network.openAndHandshake(aSnapshot());
    network.last.deliver(new Uint8Array([0x93, 0x01, 0x02, 0x03]));
    // Not a message, so the mirror cannot be kept in step by carrying on.
    expect(connection.status.kind).toBe("disconnected");
    expect(connection.isReady).toBe(false);
  });
});

describe("sending", () => {
  it("numbers commands from zero and answers with the number", () => {
    const { network, connection } = connected();
    connection.start();
    network.openAndHandshake(aSnapshot());

    expect(connection.send({ t: "ClearProgrammer" })).toBe(0);
    expect(connection.send({ t: "Oops" })).toBe(1);
    expect(sentMessages(network.last).slice(1)).toEqual([
      { t: "Command", seq: 0, command: { t: "ClearProgrammer" } },
      { t: "Command", seq: 1, command: { t: "Oops" } },
    ]);
  });

  it("refuses to send when there is no daemon, rather than queueing", () => {
    const { network, connection } = connected();
    connection.start();
    expect(connection.send({ t: "Oops" })).toBeNull();
    network.openAndHandshake(aSnapshot());
    network.last.drop();
    // A Go that arrived four seconds late is an instruction the operator has
    // already given up on, so intent is dropped rather than replayed.
    expect(connection.send({ t: "Oops" })).toBeNull();
    expect(sentMessages(network.sockets[0] ?? { sent: [] }).filter((m) => m.t === "Command")).toEqual(
      [],
    );
  });

  it("reports a socket that throws rather than losing the connection", () => {
    const { network, connection } = connected();
    connection.start();
    network.openAndHandshake(aSnapshot());
    const socket = network.last;
    socket.send = () => {
      throw new Error("the socket is closing");
    };
    expect(connection.send({ t: "Oops" })).toBeNull();
  });
});

describe("the reconnect", () => {
  it("backs off 100 ms doubling to five seconds", () => {
    expect(backoffMs(0)).toBe(100);
    expect(backoffMs(1)).toBe(200);
    expect(backoffMs(2)).toBe(400);
    expect(backoffMs(6)).toBe(5000);
    expect(backoffMs(100)).toBe(BACKOFF_MAX_MS);
    expect(backoffMs(-1)).toBe(BACKOFF_START_MS);
  });

  it("waits longer each time and starts again from the top when it succeeds", () => {
    const { network, clock, connection, statuses } = connected();
    connection.start();
    network.openAndHandshake(aSnapshot());

    const delays: number[] = [];
    network.last.drop();
    delays.push(clock.nextDelay);
    for (let attempt = 0; attempt < 3; attempt += 1) {
      clock.fire();
      network.last.open();
      // Nothing answers: the daemon is still down.
      network.last.drop();
      delays.push(clock.nextDelay);
    }
    expect(delays).toEqual([100, 200, 400, 800]);

    clock.fire();
    network.last.open();
    network.last.deliver(serverMessage({ t: "Snapshot", snapshot: aSnapshot() }));
    expect(connection.status).toEqual({ kind: "connected" });

    // And the next failure starts from the beginning, because the attempt
    // counter is about *this* outage rather than about the session.
    network.last.drop();
    expect(clock.nextDelay).toBe(BACKOFF_START_MS);
    // Twice connected: once at the start and once after the outage. A client
    // that had silently stayed "connected" through four failed attempts would
    // read as one.
    expect(statuses.filter((status) => status.kind === "connected")).toHaveLength(2);
  });

  it("is given a fresh snapshot when it comes back", () => {
    const snapshots: string[] = [];
    const { network, clock, connection } = connected({
      onSnapshot: (snapshot) => {
        const line = snapshot.session;
        snapshots.push(JSON.stringify(line));
      },
    });
    connection.start();
    network.openAndHandshake(aSnapshot());
    network.last.drop();
    clock.fire();
    network.last.open();
    network.last.deliver(
      serverMessage({
        t: "Snapshot",
        snapshot: aSnapshot({ session: { session: { commandLine: "" }, views: {} } }),
      }),
    );
    expect(snapshots).toHaveLength(2);
    expect(snapshots[0]).toContain("fixture 1 at full");
    expect(snapshots[1]).not.toContain("fixture 1 at full");
  });

  it("stops when it is told to, and a late socket event changes nothing", () => {
    const { network, clock, connection } = connected();
    connection.start();
    network.openAndHandshake(aSnapshot());
    const socket = network.last;
    connection.stop();
    expect(socket.closed).toBe(true);

    socket.drop("too late");
    expect(clock.pending.filter((waiting) => !waiting.cancelled)).toHaveLength(0);
    expect(network.sockets).toHaveLength(1);
  });

  it("ignores a message from a socket it has already replaced", () => {
    const deltas: string[] = [];
    const { network, clock, connection } = connected({
      onDelta: (delta) => deltas.push(delta.t),
    });
    connection.start();
    network.openAndHandshake(aSnapshot());
    const stale = network.last;
    stale.drop();
    clock.fire();
    network.openAndHandshake(aSnapshot());

    stale.deliver(serverMessage({ t: "Delta", delta: { t: "DirtyFlag", unsavedChanges: true } }));
    stale.open();
    expect(deltas).toEqual([]);
  });

  it("resynchronises on demand", () => {
    const { network, clock, connection } = connected();
    connection.start();
    network.openAndHandshake(aSnapshot());
    connection.resync("a delta did not fit the mirror");
    expect(network.last.closed).toBe(true);
    expect(connection.status).toEqual({
      kind: "disconnected",
      reason: "a delta did not fit the mirror",
      attempt: 1,
      retryInMs: BACKOFF_START_MS,
    });
    clock.fire();
    expect(network.sockets).toHaveLength(2);
  });
});

describe("the browser's socket", () => {
  it("only accepts binary frames and reports a close with its reason", async () => {
    // The one part of the module that touches the platform. jsdom has no
    // WebSocket server, so the object is driven directly — which is all this
    // adapter is: four handlers and a `binaryType`.
    const { webSocketFactory } = await import("./connection");
    const events: string[] = [];
    class StubSocket {
      binaryType = "";
      onopen: (() => void) | null = null;
      onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
      onclose: ((event: CloseEvent) => void) | null = null;
      onerror: (() => void) | null = null;
      sent: unknown[] = [];
      closed = false;
      send(payload: unknown) {
        this.sent.push(payload);
      }
      close() {
        this.closed = true;
      }
    }
    const stub = new StubSocket();
    vi.stubGlobal(
      "WebSocket",
      function WebSocketStub(this: unknown) {
        return stub;
      } as unknown as typeof WebSocket,
    );

    const socket = webSocketFactory("ws://host/ipc", {
      onOpen: () => events.push("open"),
      onMessage: (payload) => events.push(`message:${payload.byteLength}`),
      onClose: (reason) => events.push(`close:${reason}`),
    });
    expect(stub.binaryType).toBe("arraybuffer");

    stub.onopen?.();
    stub.onmessage?.({ data: new ArrayBuffer(3) } as MessageEvent<unknown>);
    // A text frame is dropped rather than decoded: §2 carries binary only.
    stub.onmessage?.({ data: "hello" } as MessageEvent<unknown>);
    stub.onerror?.();
    stub.onclose?.({ code: 1006, reason: "" } as CloseEvent);
    stub.onclose?.({ code: 1000, reason: "shutting down" } as CloseEvent);

    socket.send(new Uint8Array([1, 2]));
    socket.close();
    expect(events).toEqual(["open", "message:3", "close:closed (1006)", "close:shutting down"]);
    expect(stub.sent).toHaveLength(1);
    expect(stub.closed).toBe(true);
    vi.unstubAllGlobals();
  });
});

describe("the real timer", () => {
  it("runs and cancels", async () => {
    const { realTimer } = await import("./connection");
    vi.useFakeTimers();
    let ran = 0;
    const cancel = realTimer(() => {
      ran += 1;
    }, 50);
    vi.advanceTimersByTime(60);
    expect(ran).toBe(1);

    const cancelled = realTimer(() => {
      ran += 1;
    }, 50);
    cancelled();
    vi.advanceTimersByTime(60);
    expect(ran).toBe(1);
    cancel();
    vi.useRealTimers();
  });
});

describe("decoding", () => {
  it("refuses a payload that is too big before it decodes it", () => {
    const enormous = new Uint8Array(1024 * 1024 + 1);
    expect(() => decodeServerMessage(enormous)).toThrow(/at most 1048576 bytes/);
  });
});
