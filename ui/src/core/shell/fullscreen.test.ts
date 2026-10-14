/**
 * **B42** — the two keys, and which host does the work.
 *
 * The half that can be asserted in `jsdom` is the half that decides: which key
 * presses count, and which of the two mechanisms is used. Whether a *window*
 * actually loses its title bar is the operating system's answer and belongs in
 * the hand-check; whether the *key* reaches a mechanism at all is this file's,
 * and it is the half that was missing — B42 is an entry about two keys that did
 * nothing.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import { nullSink, setLogSink } from "../log/logger";
import { documentIsFullscreen, isFullscreenKey, setFullscreen } from "./fullscreen";

setLogSink(nullSink);

interface ShellWindow {
  __TAURI_INTERNALS__?: { invoke?: unknown };
}

/**
 * Gives `jsdom` the two calls it does not implement.
 *
 * `jsdom` has no Fullscreen API at all — not even the properties — which is the
 * same reason `documentIsFullscreen` reads its answer with `Boolean` rather
 * than by comparing with `null`. Defined rather than spied, because there is
 * nothing there to spy on.
 */
function withFullscreenApi(
  request: () => Promise<void>,
  exit: () => Promise<void> = () => Promise.resolve(),
): void {
  Object.defineProperty(document.documentElement, "requestFullscreen", {
    configurable: true,
    value: request,
  });
  Object.defineProperty(document, "exitFullscreen", { configurable: true, value: exit });
}

/** Installs a fake shell, the way `bridge.test.ts` does. */
function withShell(invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>) {
  (window as unknown as ShellWindow).__TAURI_INTERNALS__ = { invoke };
}

afterEach(() => {
  delete (window as unknown as ShellWindow).__TAURI_INTERNALS__;
  vi.restoreAllMocks();
});

/** A key press, with every modifier down unless it is named. */
function press(key: string, modifiers: Partial<Record<string, boolean>> = {}) {
  return {
    key,
    altKey: modifiers.altKey === true,
    ctrlKey: modifiers.ctrlKey === true,
    metaKey: modifiers.metaKey === true,
    shiftKey: modifiers.shiftKey === true,
    defaultPrevented: modifiers.defaultPrevented === true,
  };
}

describe("the two ways of asking for full screen", () => {
  it("takes F11 and Alt+Enter, which are the two the entry names", () => {
    expect(isFullscreenKey(press("F11"))).toBe(true);
    expect(isFullscreenKey(press("Enter", { altKey: true }))).toBe(true);
  });

  it("takes neither of them with a modifier the operator did not ask for", () => {
    // `Ctrl` + `F11` belongs to the browser and `Alt` + `F11` is not this;
    // a plain Enter is the command line's, which is the important one — the
    // desk would go full screen every time a line was run.
    expect(isFullscreenKey(press("Enter"))).toBe(false);
    expect(isFullscreenKey(press("F11", { ctrlKey: true }))).toBe(false);
    expect(isFullscreenKey(press("F11", { altKey: true }))).toBe(false);
    expect(isFullscreenKey(press("Enter", { altKey: true, shiftKey: true }))).toBe(false);
    expect(isFullscreenKey(press("Enter", { altKey: true, ctrlKey: true }))).toBe(false);
  });

  it("leaves alone a press something else has already handled", () => {
    expect(isFullscreenKey(press("F11", { defaultPrevented: true }))).toBe(false);
  });

  it("is not every other key", () => {
    for (const key of ["F10", "F12", "Escape", "a", "Tab"]) {
      expect(isFullscreenKey(press(key)), key).toBe(false);
      expect(isFullscreenKey(press(key, { altKey: true })), key).toBe(false);
    }
  });
});

describe("which host does it", () => {
  it("asks the shell to move the window, and reports what the window reached", async () => {
    const calls: { command: string; args?: Record<string, unknown> }[] = [];
    withShell((command, args) => {
      calls.push({ command, args });
      return Promise.resolve(true);
    });

    await expect(setFullscreen(true)).resolves.toBe(true);
    expect(calls).toEqual([{ command: "set_fullscreen", args: { on: true } }]);
  });

  it("reports false when the shell says the window did not get there", async () => {
    // The autostart switch's rule, one control along: what is drawn is what
    // happened, never what was asked for.
    withShell(() => Promise.resolve(false));
    await expect(setFullscreen(true)).resolves.toBe(false);
  });

  it("reports false rather than throwing when the shell refuses", async () => {
    withShell(() => Promise.reject(new Error("this shell has no desk window")));
    await expect(setFullscreen(true)).resolves.toBe(false);
  });

  it("uses the browser's own API when there is no shell", async () => {
    // `jsdom` implements neither call, so both are supplied — and what is being
    // asserted is that the *document element* is the one asked, because a
    // full-screen element with a margin shows the browser's backdrop as a frame.
    const request = vi.fn(() => Promise.resolve());
    withFullscreenApi(request);
    await setFullscreen(true);
    expect(request).toHaveBeenCalledTimes(1);
  });

  it("answers no on a browser that has no Fullscreen API at all", async () => {
    // `jsdom` is one, and so is any browser with the feature switched off. The
    // key must come back with *no* rather than throwing a `TypeError`.
    await expect(setFullscreen(true)).resolves.toBe(false);
  });

  it("swallows a browser that refuses, because a key handler may not throw", async () => {
    withFullscreenApi(() => Promise.reject(new Error("permissions check failed")));
    await expect(setFullscreen(true)).resolves.toBe(false);
  });

  it("leaves a full screen the browser is in", async () => {
    // The other half of the toggle, and the path `documentIsFullscreen` guards:
    // `jsdom` has no Fullscreen API, so both the property and the call are
    // supplied here.
    const exit = vi.fn(() => Promise.resolve());
    withFullscreenApi(() => Promise.resolve(), exit);
    Object.defineProperty(document, "fullscreenElement", {
      configurable: true,
      value: document.documentElement,
    });
    expect(documentIsFullscreen()).toBe(true);
    await setFullscreen(false);
    expect(exit).toHaveBeenCalledTimes(1);
    Object.defineProperty(document, "fullscreenElement", {
      configurable: true,
      value: null,
    });
  });

  it("does not ask a browser to leave a full screen it is not in", async () => {
    const exit = vi.fn(() => Promise.resolve());
    withFullscreenApi(() => Promise.resolve(), exit);
    expect(documentIsFullscreen()).toBe(false);
    await expect(setFullscreen(false)).resolves.toBe(false);
    expect(exit).not.toHaveBeenCalled();
  });
});
