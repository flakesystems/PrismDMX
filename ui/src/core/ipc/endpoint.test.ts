/**
 * Where the daemon is, and what the address bar is not allowed to talk the
 * interface into.
 */

import { describe, expect, it } from "vitest";

import { DEFAULT_URL, daemonToken, daemonUrl, isWebSocketUrl } from "./endpoint";

describe("the endpoint", () => {
  it("is the daemon's own listener by default", () => {
    expect(daemonUrl()).toBe("ws://127.0.0.1:7373/ipc");
    expect(daemonUrl({ search: "" })).toBe(DEFAULT_URL);
  });

  it("takes one from the query string, which is what the shell will pass", () => {
    expect(daemonUrl({ search: "?daemon=ws://127.0.0.1:9001/ipc" })).toBe(
      "ws://127.0.0.1:9001/ipc",
    );
    expect(daemonUrl({ search: "?other=1&daemon=wss://desk.local/ipc" })).toBe(
      "wss://desk.local/ipc",
    );
  });

  it("falls back to what the build was configured with", () => {
    expect(daemonUrl({ configured: "ws://built-in/ipc" })).toBe("ws://built-in/ipc");
    // The address bar wins over the build, because a person typing one means it.
    expect(daemonUrl({ search: "?daemon=ws://typed/ipc", configured: "ws://built-in/ipc" })).toBe(
      "ws://typed/ipc",
    );
  });

  it("ignores anything that is not a WebSocket URL", () => {
    // It arrives from the address bar. An interface that opened whatever a
    // query string said would be taking instructions from wherever the link
    // came from.
    for (const asked of [
      "http://evil.example/ipc",
      "javascript:alert(1)",
      "file:///etc/passwd",
      "not a url",
      "",
    ]) {
      expect(daemonUrl({ search: `?daemon=${encodeURIComponent(asked)}` })).toBe(DEFAULT_URL);
    }
  });

  it("knows a WebSocket URL from anything else", () => {
    expect(isWebSocketUrl("ws://host/ipc")).toBe(true);
    expect(isWebSocketUrl("wss://host/ipc")).toBe(true);
    expect(isWebSocketUrl("https://host/ipc")).toBe(false);
    expect(isWebSocketUrl("host/ipc")).toBe(false);
  });
});

describe("the token that travels with the address", () => {
  it("is absent in a browser and on a loopback listener", () => {
    // `docs/IPC_PROTOCOL.md` §2.1 asks for one only where the listener can be
    // reached from another machine, which is the state a desk is never in
    // unless somebody moved it there.
    expect(daemonToken()).toBeNull();
    expect(daemonToken({ search: "?daemon=ws://127.0.0.1:7373/ipc" })).toBeNull();
    expect(daemonToken({ search: "?token=" })).toBeNull();
  });

  it("is what the desktop shell read out of the discovery document", () => {
    expect(daemonToken({ search: "?daemon=ws://0.0.0.0:7373/ipc&token=hunter2" })).toBe("hunter2");
  });
});
