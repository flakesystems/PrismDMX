/**
 * The wiring, end to end without a browser: a store, a connection and a
 * telemetry sink that reach each other and nothing else.
 */

import { describe, expect, it } from "vitest";

import { createDesk } from "./desk";
import {
  FakeNetwork,
  ManualTimer,
  serverMessage,
  snapshot as aSnapshot,
} from "./testing/fake-daemon";

/** A desk over a network and a clock the test owns. */
function desk() {
  const network = new FakeNetwork();
  const clock = new ManualTimer();
  return {
    network,
    clock,
    desk: createDesk({
      url: "ws://127.0.0.1:7373/ipc",
      socketFactory: network.factory,
      timer: clock.timer,
    }),
  };
}

describe("the wired-up interface", () => {
  it("puts the world into the store and the pictures into the sink", () => {
    const { network, desk: wired } = desk();
    wired.start();
    network.openAndHandshake(aSnapshot());

    expect(wired.store.getState().status).toEqual({ kind: "connected" });
    expect(wired.store.getState().documents?.session).toEqual(aSnapshot().session);

    network.last.deliver(serverMessage({ t: "Telemetry", data: new Uint8Array(514) }));
    expect(wired.telemetry.received).toBe(1);
    // §7: it went to the sink and nowhere near the state.
    expect(wired.store.getState().documents?.show).toEqual(aSnapshot().show);
  });

  it("clears the picture as well as the values when the daemon goes", () => {
    const { network, desk: wired } = desk();
    wired.start();
    network.openAndHandshake(aSnapshot());
    network.last.deliver(serverMessage({ t: "Telemetry", data: new Uint8Array(514) }));

    network.last.drop();

    // A picture of the rig from a daemon that has stopped is exactly as stale
    // as a fader value from one.
    expect(wired.telemetry.latest).toBeNull();
    expect(wired.store.getState().documents).toBeNull();
    wired.stop();
  });

  it("sends commands through the connection the store was attached to", () => {
    const { network, desk: wired } = desk();
    wired.start();
    network.openAndHandshake(aSnapshot());
    expect(wired.store.send({ t: "SaveShow" })).toBe(0);
    expect(network.last.sent).toHaveLength(2);
  });

  it("resynchronises when a delta does not fit the mirror", () => {
    const { network, clock, desk: wired } = desk();
    wired.start();
    network.openAndHandshake(aSnapshot());

    network.last.deliver(
      serverMessage({
        t: "Delta",
        delta: { t: "ShowPatch", ops: [{ op: "remove", path: "/nothing" }] },
      }),
    );
    // The mirror and the daemon disagree, so the client asks for the world
    // again rather than carrying on with a document it knows is wrong.
    expect(wired.store.getState().documents).toBeNull();
    expect(network.last.closed).toBe(true);
    clock.fire();
    expect(network.sockets).toHaveLength(2);
    wired.stop();
  });

  it("defaults to the daemon's own listener when it is given no address", () => {
    const network = new FakeNetwork();
    const clock = new ManualTimer();
    const wired = createDesk({ socketFactory: network.factory, timer: clock.timer });
    wired.start();
    expect(network.last.url).toBe("ws://127.0.0.1:7373/ipc");
    wired.stop();
  });
});
