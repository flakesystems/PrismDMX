/**
 * The levelled logger `CLAUDE.md` requires.
 *
 * *Do NOT use plain console.log in production code. Use a structured logger
 * with log levels.* So a record here is a value — level, module, message and a
 * flat bag of fields — and where it goes is a sink the process installs once.
 * The default sink writes to the browser console through the method that
 * matches the level, which is what makes a warning filterable in the devtools
 * rather than one more line in a stream.
 *
 * It is deliberately not reactive and deliberately not asynchronous: a log line
 * is a side effect, and a UI that re-rendered because something was logged
 * would be a UI whose render depended on its own diagnostics.
 */

/** The levels, quietest first. */
export const LOG_LEVELS = ["debug", "info", "warn", "error"] as const;

/** How much a record matters. */
export type LogLevel = (typeof LOG_LEVELS)[number];

/** The threshold, which may also silence the logger entirely. */
export type LogThreshold = LogLevel | "off";

/** What may be attached to a record. Structured, so a sink can index it. */
export type LogField = string | number | boolean | null;

/** One log record. */
export interface LogRecord {
  /** How much it matters. */
  readonly level: LogLevel;
  /** Which part of the interface said it — `ipc`, `mirror`, `store`. */
  readonly module: string;
  /** What happened, as a fixed phrase rather than an interpolated sentence. */
  readonly message: string;
  /** The variable part, so a sink can filter on it. */
  readonly fields: Readonly<Record<string, LogField>>;
  /** `Date.now()` when the record was made. */
  readonly at: number;
}

/** Where records go. */
export type LogSink = (record: LogRecord) => void;

/** Rank of a level, for comparing against the threshold. */
function rank(level: LogThreshold): number {
  return level === "off" ? LOG_LEVELS.length : LOG_LEVELS.indexOf(level);
}

/**
 * The sink installed by default: the console method that matches the level.
 *
 * `console.log` is never one of them — that is the rule this module exists to
 * keep — and the record is passed as an object rather than interpolated into a
 * string, so the fields survive into the devtools.
 */
export const consoleSink: LogSink = (record) => {
  const line = `[${record.module}] ${record.message}`;
  const { fields } = record;
  const hasFields = Object.keys(fields).length > 0;
  switch (record.level) {
    case "debug":
      if (hasFields) console.debug(line, fields);
      else console.debug(line);
      return;
    case "info":
      if (hasFields) console.info(line, fields);
      else console.info(line);
      return;
    case "warn":
      if (hasFields) console.warn(line, fields);
      else console.warn(line);
      return;
    case "error":
      if (hasFields) console.error(line, fields);
      else console.error(line);
      return;
  }
};

/** A sink that drops everything, for tests that only care that nothing threw. */
export const nullSink: LogSink = () => {};

let threshold: LogThreshold = "info";
let sink: LogSink = consoleSink;

/** Records below this level are dropped before the sink sees them. */
export function setLogThreshold(level: LogThreshold): void {
  threshold = level;
}

/** The current threshold. */
export function logThreshold(): LogThreshold {
  return threshold;
}

/** Installs a sink and answers with the one it replaced. */
export function setLogSink(next: LogSink): LogSink {
  const previous = sink;
  sink = next;
  return previous;
}

/** A logger bound to one module name. */
export interface Logger {
  /** Detail nobody needs until something is wrong. */
  debug(message: string, fields?: Readonly<Record<string, LogField>>): void;
  /** Something happened that an operator would recognise. */
  info(message: string, fields?: Readonly<Record<string, LogField>>): void;
  /** Something is not as it should be, and the interface carried on. */
  warn(message: string, fields?: Readonly<Record<string, LogField>>): void;
  /** Something failed. */
  error(message: string, fields?: Readonly<Record<string, LogField>>): void;
}

const NO_FIELDS: Readonly<Record<string, LogField>> = Object.freeze({});

/** A logger for one module. */
export function logger(module: string): Logger {
  const emit =
    (level: LogLevel) =>
    (message: string, fields?: Readonly<Record<string, LogField>>): void => {
      if (rank(level) < rank(threshold)) {
        return;
      }
      sink({
        level,
        module,
        message,
        fields: fields ?? NO_FIELDS,
        at: Date.now(),
      });
    };
  return {
    debug: emit("debug"),
    info: emit("info"),
    warn: emit("warn"),
    error: emit("error"),
  };
}
