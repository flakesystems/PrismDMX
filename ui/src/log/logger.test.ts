/**
 * The logger: levels, structure, and never `console.log`.
 *
 * `CLAUDE.md` forbids plain `console.log` in production code and asks for a
 * structured logger with levels. The last test is that rule as a test rather
 * than as a habit.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import type { LogRecord } from "./logger";
import {
  LOG_LEVELS,
  consoleSink,
  logThreshold,
  logger,
  nullSink,
  setLogSink,
  setLogThreshold,
} from "./logger";

/** Collects records instead of writing them. */
function collector() {
  const records: LogRecord[] = [];
  setLogSink((record) => records.push(record));
  return records;
}

afterEach(() => {
  setLogThreshold("info");
  setLogSink(nullSink);
  vi.restoreAllMocks();
});

describe("levels", () => {
  it("drops what is below the threshold", () => {
    const records = collector();
    const log = logger("ipc");

    setLogThreshold("warn");
    log.debug("nobody wants this");
    log.info("nor this");
    log.warn("this");
    log.error("and this");
    expect(records.map((record) => record.level)).toEqual(["warn", "error"]);

    setLogThreshold("off");
    log.error("not even this");
    expect(records).toHaveLength(2);
    expect(logThreshold()).toBe("off");

    setLogThreshold("debug");
    log.debug("everything now");
    expect(records).toHaveLength(3);
  });

  it("is ordered quietest first", () => {
    expect(LOG_LEVELS).toEqual(["debug", "info", "warn", "error"]);
  });
});

describe("records", () => {
  it("carries the module, the message and the fields separately", () => {
    const records = collector();
    logger("mirror").warn("a delta did not fit", { delta: "ShowPatch", fault: "NoSuchPath" });

    expect(records).toHaveLength(1);
    const record = records[0];
    expect(record?.module).toBe("mirror");
    expect(record?.message).toBe("a delta did not fit");
    expect(record?.fields).toEqual({ delta: "ShowPatch", fault: "NoSuchPath" });
    expect(record?.at).toBeGreaterThan(0);
  });

  it("gives a record with no fields an empty bag rather than undefined", () => {
    const records = collector();
    logger("store").info("connected");
    expect(records[0]?.fields).toEqual({});
  });

  it("answers with the sink it replaced, so a caller can put it back", () => {
    const first = () => {};
    const previous = setLogSink(first);
    expect(setLogSink(previous)).toBe(first);
  });
});

describe("the console sink", () => {
  it("uses the method that matches the level, and never `log`", () => {
    const debug = vi.spyOn(console, "debug").mockImplementation(() => {});
    const info = vi.spyOn(console, "info").mockImplementation(() => {});
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const plain = vi.spyOn(console, "log").mockImplementation(() => {});

    for (const level of LOG_LEVELS) {
      consoleSink({ level, module: "ipc", message: "something", fields: {}, at: 0 });
      consoleSink({ level, module: "ipc", message: "something", fields: { seq: 1 }, at: 0 });
    }

    expect(debug).toHaveBeenCalledTimes(2);
    expect(info).toHaveBeenCalledTimes(2);
    expect(warn).toHaveBeenCalledTimes(2);
    expect(error).toHaveBeenCalledTimes(2);
    // The rule, as a test.
    expect(plain).not.toHaveBeenCalled();

    expect(debug).toHaveBeenCalledWith("[ipc] something");
    expect(debug).toHaveBeenCalledWith("[ipc] something", { seq: 1 });
  });

  it("throws nothing when it is the installed sink", () => {
    vi.spyOn(console, "info").mockImplementation(() => {});
    setLogSink(consoleSink);
    expect(() => logger("ipc").info("hello")).not.toThrow();
  });
});
