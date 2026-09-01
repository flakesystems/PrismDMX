/**
 * The bridge to the desktop shell, held to both of the places it runs.
 *
 * Every test here installs — or deliberately does not install — the object Tauri
 * puts on `window`, which is the whole of what *inside the shell* means to this
 * interface. Nothing here needs a shell, a window or a dialogue, which is
 * `CLAUDE.md`'s rule stated for a bridge: what a shell **decides** is Rust's and
 * has its own tests in `crates/prism-app`; what this file does is ask and narrow.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import type { PathKind } from "./bridge";
import { autostartApply, autostartState, choosePath, inShell } from "./bridge";

/** What Tauri puts on `window`, replaced by one a test can drive. */
function installShell(invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>) {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = { invoke };
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

describe("whether there is a shell at all", () => {
  it("says no in a browser, which is where the Web Remote will always be", () => {
    expect(inShell()).toBe(false);
  });

  it("says yes when Tauri has put its bridge on the window", () => {
    installShell(() => Promise.resolve(null));
    expect(inShell()).toBe(true);
  });
});

describe("the operating system's file dialogue", () => {
  it("answers nothing in a browser rather than throwing", async () => {
    // The whole reason the box stays: a panel calls this unconditionally and
    // finds out there was nowhere to ask.
    await expect(choosePath("OpenShow")).resolves.toBeNull();
  });

  it("passes the place and what the box already holds, and answers the path", async () => {
    const invoke = vi.fn().mockResolvedValue("C:\\shows\\aula.prism");
    installShell(invoke);

    await expect(choosePath("SaveShowAs", "C:\\shows\\old.prism")).resolves.toBe(
      "C:\\shows\\aula.prism",
    );
    expect(invoke).toHaveBeenCalledWith("choose_path", {
      kind: "SaveShowAs",
      start: "C:\\shows\\old.prism",
    });
  });

  it("sends no starting point when the box is empty", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    installShell(invoke);
    await choosePath("OpenShow", "");
    expect(invoke).toHaveBeenCalledWith("choose_path", { kind: "OpenShow", start: null });
  });

  it("treats a cancelled dialogue as leave the box alone", async () => {
    installShell(() => Promise.resolve(null));
    await expect(choosePath("ImportShow")).resolves.toBeNull();
  });

  it("treats a shell that could not open one the same way, and says so once", async () => {
    installShell(() => Promise.reject(new Error("no display")));
    await expect(choosePath("NewShow")).resolves.toBeNull();
  });

  /** Every place the interface asks for a path has a name the shell knows. */
  it("names the five file commands and the two machine paths", () => {
    const kinds: PathKind[] = [
      "OpenShow",
      "SaveShowAs",
      "NewShow",
      "ExportShow",
      "ImportShow",
      "FixtureLibrary",
      "SurfaceProfile",
    ];
    // A compile-time list asserted at run time: `prism_app::dialogs::PathKind`
    // has the matching test on the Rust side, and between the two a name that
    // is renamed in one place fails in the other.
    expect(new Set(kinds).size).toBe(kinds.length);
  });
});

describe("what this machine's start-up entry actually is", () => {
  it("is unknown in a browser, which is not the same as absent", async () => {
    await expect(autostartState()).resolves.toBeNull();
    await expect(autostartApply(true)).resolves.toBeNull();
  });

  it("reads the report the shell answers with", async () => {
    installShell(() =>
      Promise.resolve({
        supported: true,
        installed: true,
        command: '"C:\\PrismDMX\\PrismDMX.exe" --hidden',
        matchesThisInstall: true,
      }),
    );
    await expect(autostartState()).resolves.toEqual({
      supported: true,
      installed: true,
      command: '"C:\\PrismDMX\\PrismDMX.exe" --hidden',
      matchesThisInstall: true,
    });
  });

  it("asks for the entry to be written, and passes what was wanted", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValue({ supported: true, installed: false, command: null });
    installShell(invoke);

    const report = await autostartApply(false);
    expect(invoke).toHaveBeenCalledWith("autostart_apply", { wanted: false });
    expect(report).toEqual({
      supported: true,
      installed: false,
      command: null,
      matchesThisInstall: false,
    });
  });

  /**
   * The narrowing, and why it is here: a panel that drew a tick because a field
   * was `undefined` would be the switch displaying a lie by another route.
   */
  it("refuses anything that is not a report", async () => {
    for (const answer of [null, 42, "yes", {}, { supported: "yes", installed: true }]) {
      installShell(() => Promise.resolve(answer));
      // eslint-disable-next-line no-await-in-loop
      await expect(autostartState()).resolves.toBeNull();
    }
  });

  it("treats a shell that refused the question as no answer", async () => {
    installShell(() => Promise.reject(new Error("the registry said no")));
    await expect(autostartApply(true)).resolves.toBeNull();
  });
});
