/**
 * The telemetry sink, and the rule it exists to keep: **never reactive state**.
 *
 * The rule is `docs/IPC_PROTOCOL.md` §7's last paragraph, and S24 is the
 * session that renders this channel. What S23 owes is that receiving it costs
 * no render — asserted here by counting the store's notifications while a
 * hundred frames arrive, rather than by promising to be careful.
 */

import { describe, expect, it } from "vitest";

import { Connection } from "../ipc/connection";
import { DeskStore, deskEvents } from "../store/desk";
import {
  FakeNetwork,
  ManualTimer,
  serverMessage,
  snapshot as aSnapshot,
} from "../testing/fake-daemon";
import { TelemetrySink } from "./telemetry";

describe("the sink", () => {
  it("keeps the latest payload and counts what went through", () => {
    const sink = new TelemetrySink();
    expect(sink.latest).toBeNull();
    expect(sink.received).toBe(0);
    expect(sink.at).toBe(0);

    sink.accept(new Uint8Array(514));
    sink.accept(new Uint8Array([1, 2, 3]));

    // The latest, not a queue: a picture of the lights that is three frames old
    // has no value at all, which is why the daemon coalesces at its end too.
    expect(sink.latest).toEqual(new Uint8Array([1, 2, 3]));
    expect(sink.received).toBe(2);
    expect(sink.bytes).toBe(517);
    expect(sink.at).toBeGreaterThan(0);
  });

  it("forgets the picture when the daemon goes", () => {
    const sink = new TelemetrySink();
    sink.accept(new Uint8Array(4));
    sink.clear();
    expect(sink.latest).toBeNull();
    expect(sink.at).toBe(0);
    // What has been through it is a diagnostic, and stays.
    expect(sink.received).toBe(1);
  });
});

describe("the rule", () => {
  it("costs the store nothing, however much of it arrives", () => {
    const network = new FakeNetwork();
    const clock = new ManualTimer();
    const store = new DeskStore();
    const sink = new TelemetrySink();
    const events = deskEvents(store, () => {});
    const connection = new Connection(
      { url: "ws://host/ipc", socketFactory: network.factory, timer: clock.timer },
      { ...events, onTelemetry: sink.accept },
    );
    connection.start();
    network.openAndHandshake(aSnapshot());

    let notified = 0;
    store.subscribe(() => {
      notified += 1;
    });
    const before = store.getState();

    // A hundred frames — three seconds of §7's 30 Hz.
    for (let frame = 0; frame < 100; frame += 1) {
      network.last.deliver(serverMessage({ t: "Telemetry", data: new Uint8Array(514) }));
    }

    expect(sink.received).toBe(100);
    // Not one notification, and the state object itself is untouched — which is
    // what React compares. 64 universes × 512 channels at 30 Hz through here
    // is the interface this rule exists to prevent.
    expect(notified).toBe(0);
    expect(store.getState()).toBe(before);
  });
});
