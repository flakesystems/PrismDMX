/**
 * The settings readers — S37.
 *
 * Pure functions over what the daemon said, and every one of them exists
 * because the alternative was arithmetic somewhere it does not belong.
 */

import { describe, expect, it } from "vitest";

import type { MachineChange, MachineSettings, SurfaceStatus } from "../bindings";
import type { AutostartReport } from "../shell/bridge";
import { machine as machineFixture } from "../testing/fake-daemon";
import {
  EXIT_ACTIONS,
  LOG_LEVELS,
  PANELS,
  autostartEntryText,
  counterRows,
  exitText,
  fileName,
  flagOf,
  healthText,
  heldNote,
  isHeld,
  isLoopback,
  listenerText,
  needsRestart,
  readUniverses,
  writeUniverses,
} from "./settings";

function machine(overrides: Partial<MachineSettings> = {}): MachineSettings {
  return machineFixture(overrides);
}

describe("what waits for a restart", () => {
  /**
   * **The list is the domain's**, and this is the assertion that says so.
   *
   * `MachineChange::needs_restart` decides it in Rust and `needsRestart` mirrors
   * it here; the seven below are exactly the seven that crate's own test names.
   * A session that adds a change to one list and not the other turns this red,
   * which is the whole reason the table is a table rather than prose.
   */
  it("is the seven a running daemon cannot take back", () => {
    const waits: MachineChange[] = [
      { t: "Local", local: false },
      { t: "Websocket", address: null },
      { t: "Token", token: null },
      { t: "NewToken" },
      { t: "Universes", universes: 4 },
      { t: "FixtureLibrary", path: null },
      { t: "NewIdentity" },
    ];
    for (const change of waits) {
      expect(needsRestart(change), change.t).toBe(true);
    }
  });

  it("and the four a running daemon makes on the spot", () => {
    const live: MachineChange[] = [
      { t: "LogLevel", level: "Debug" },
      { t: "ExitAction", action: "Blackout" },
      { t: "Autostart", autostart: true },
      { t: "SurfaceProfile", path: null },
    ];
    for (const change of live) {
      expect(needsRestart(change), change.t).toBe(false);
    }
  });
});

describe("a row a command line is holding", () => {
  it("names the flag that is holding it", () => {
    const held = machine({ overrides: ["Universes", "Outputs"] });
    expect(isHeld(held, "Universes")).toBe(true);
    expect(isHeld(held, "Websocket")).toBe(false);
    expect(heldNote(held, "Universes")).toContain("--universes");
    expect(heldNote(held, "Websocket")).toBeNull();
  });

  it("has a flag for every override the daemon can send", () => {
    // Ten of them, and a name for each: an override with no flag would be a
    // greyed row an operator cannot act on, which is the state the whole
    // mechanism exists to avoid.
    for (const held of [
      "Outputs",
      "Surface",
      "Local",
      "Websocket",
      "Token",
      "LogLevel",
      "Universes",
      "ExitAction",
      "FixtureLibrary",
      "SurfaceProfile",
    ] as const) {
      expect(flagOf(held).startsWith("--"), held).toBe(true);
    }
  });

  it("is not held at all when there is no machine yet", () => {
    expect(isHeld(null, "Universes")).toBe(false);
    expect(heldNote(null, "Universes")).toBeNull();
  });
});

describe("what the listener is doing", () => {
  it("says so in one sentence, and the three states are all ordinary", () => {
    expect(listenerText(machine({ websocket: null, websocketOpen: null }))).toContain(
      "not listening",
    );
    expect(listenerText(machine())).toBe("listening on 127.0.0.1:7373");
    // **Configured here, listening nowhere** — the state S37 made ordinary by
    // letting a bind that fails be a warning rather than a daemon that will not
    // start.
    expect(
      listenerText(machine({ websocket: "127.0.0.1:7373", websocketOpen: null })),
    ).toContain("listening nowhere");
    // And the case a flag produces: the setting says one thing and the run is
    // doing another.
    expect(
      listenerText(machine({ websocket: "127.0.0.1:7373", websocketOpen: "127.0.0.1:9001" })),
    ).toContain("which is not the configured");
  });
});

describe("whether an address reaches other machines", () => {
  it("knows loopback in the spellings a person types", () => {
    for (const address of ["127.0.0.1:7373", "localhost:7373", "[::1]:7373", "127.0.0.5:80"]) {
      expect(isLoopback(address), address).toBe(true);
    }
    for (const address of ["0.0.0.0:7373", "192.168.1.20:7373", "[2001:db8::1]:7373"]) {
      expect(isLoopback(address), address).toBe(false);
    }
  });
});

describe("the words a panel writes", () => {
  it("has one for every log level and every exit action", () => {
    expect(LOG_LEVELS).toHaveLength(5);
    expect(EXIT_ACTIONS).toHaveLength(2);
    expect(exitText("Hold")).toContain("last look");
    expect(exitText("Blackout")).toContain("black");
  });

  it("has one for every surface health", () => {
    for (const health of ["Disconnected", "Connected", "Live", "Probing", "Unresponsive"] as const) {
      expect(healthText(health).length, health).toBeGreaterThan(0);
    }
    // The one whose remedy is *not* reconnect says what it is rather than what
    // to do — the advice is the daemon's words (S20's finding).
    expect(healthText("Unresponsive")).toBe("stopped sending");
  });

  it("lists the panels in the order the window draws them", () => {
    // Four from S37 and S38's `Controls` beside `Devices`: that panel names the
    // desk's port and the file its table was read from, and this one is what the
    // table says.
    expect(PANELS).toEqual(["Outputs", "Devices", "Controls", "Show files", "This machine"]);
  });
});

describe("the counters", () => {
  it("are all seven, in the order a person reads them", () => {
    const status: SurfaceStatus = {
      health: "Live",
      remedy: null,
      sent: 12,
      superseded: 3,
      touchSuppressed: 1,
      resyncs: 2,
      reserved: 0,
      probes: 4,
      reconnects: 5,
      profile: null,
      boundControls: 156,
    };
    const rows = counterRows(status);
    expect(rows).toHaveLength(7);
    expect(rows[0]).toEqual({ label: "Sent", value: 12 });
    expect(rows.at(-1)).toEqual({ label: "Reconnects", value: 5 });
  });
});

describe("a path", () => {
  it("gives up its file name on either kind of separator", () => {
    expect(fileName("D:/shows/aula.prism")).toBe("aula.prism");
    expect(fileName("D:\\shows\\aula.prism")).toBe("aula.prism");
    expect(fileName("aula.prism")).toBe("aula.prism");
  });
});

describe("a list of universes", () => {
  it("is read back as numbers, however it was typed", () => {
    expect(readUniverses("1, 2,3")).toEqual([1, 2, 3]);
    expect(readUniverses(" 5 ")).toEqual([5]);
    expect(readUniverses("")).toEqual([]);
  });

  it("is refused rather than sent as nonsense", () => {
    // A form that sent `[NaN]` would be refused by the daemon anyway, one round
    // trip later and with a message about the wrong thing.
    for (const text of ["one", "1.5", "0", "-2", "1,,x"]) {
      expect(readUniverses(text), text).toBeNull();
    }
  });

  it("round-trips through the way it is written out", () => {
    expect(readUniverses(writeUniverses([9, 10, 11, 12]))).toEqual([9, 10, 11, 12]);
  });
});

describe("what the autostart row says about this machine's start-up entry", () => {
  /** A report the shell would answer with. */
  const report = (over: Partial<AutostartReport> = {}): AutostartReport => ({
    supported: true,
    installed: false,
    command: null,
    matchesThisInstall: false,
    ...over,
  });

  it("says a browser cannot see one, which is not the same as there not being one", () => {
    // The Web Remote, `npm run dev`, and every one of the 46 end-to-end tests.
    expect(autostartEntryText(true, null)).toContain("browser");
  });

  it("says a platform that cannot write one stores the setting and acts on nothing", () => {
    expect(autostartEntryText(true, report({ supported: false }))).toContain("cannot write");
  });

  it("agrees with itself when the two agree", () => {
    expect(autostartEntryText(true, report({ installed: true, matchesThisInstall: true }))).toBe(
      "A start-up entry for this installation is in place.",
    );
    expect(autostartEntryText(false, report())).toBe(
      "There is no start-up entry, which is what the setting says.",
    );
  });

  /**
   * **The two readings the row exists for.** A tick beside a setting whose
   * entry was deleted in Task Manager is a switch displaying a lie, and so is
   * an empty box beside an entry that is still there.
   */
  it("says which way round the two disagree, and what to do about it", () => {
    expect(autostartEntryText(true, report({ installed: false }))).toContain(
      "there is no start-up entry",
    );
    expect(autostartEntryText(false, report({ installed: true }))).toContain(
      "still there",
    );
  });

  /** An update installed somewhere else leaves an entry naming the old copy. */
  it("names another installation's entry rather than quietly disagreeing with it", () => {
    const text = autostartEntryText(
      true,
      report({ installed: true, command: '"C:\Old\PrismDMX.exe" --hidden' }),
    );
    expect(text).toContain("another copy");
    expect(text).toContain("C:\Old\PrismDMX.exe");
  });
});
