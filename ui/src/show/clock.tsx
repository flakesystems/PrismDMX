/**
 * The Clock Viewer: the time, and what is running — S43.
 *
 * # What it shows, and what it deliberately does not
 *
 * `ARCHITECTURE_SPEC.md` §6 has called this window *clocks and timecode* since
 * the first draft, and there is no timecode in this build: no MIDI Timecode
 * input, no LTC, no clock object in the show. So this session built the half
 * that is real, and the owner agreed the shape — a wall clock an operator can
 * read from the back of the room, and a line per running playback saying **what**
 * is running rather than for how long.
 *
 * The times are missing because they are not on the wire, not because they were
 * forgotten. `Executor` carries `isActive` and `currentCueIndex` and no clock;
 * `DaemonHealth` carries the tick rate and no uptime. A client could *guess* at
 * a cue's elapsed time by watching the index change, and that guess would differ
 * on a second screen and start again after a reconnect — so the columns are left
 * out rather than filled with something that looks like a measurement. **S47**
 * is the session that gives them a source.
 *
 * # The clock is client-local, and it is the one honest exception
 *
 * §4.2 keeps per-screen state out of the session, and the time of day is exactly
 * that: it is the *viewer's* wall clock, not the desk's. It ticks in this
 * component with `setInterval` and sends nothing.
 *
 * A second is a slow enough beat to be React's: one commit per second per open
 * clock, against thirty frames of telemetry that must never be one
 * (`telemetry/render.test.tsx`). The interval is torn down with the window.
 */

import { useEffect, useState } from "react";

import type { JsonValue } from "../bindings";
import { pageStrips } from "../desk/session";

/** How often the face is redrawn. */
const TICK_MS = 1000;

/** The Clock Viewer. */
export function ClockViewer({
  show,
  session,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
}) {
  const now = useNow();
  return (
    <div className="clock" data-testid="clock-viewer">
      <p className="clock-face" data-testid="clock-time">
        {timeOf(now)}
      </p>
      <p className="clock-date" data-testid="clock-date">
        {dateOf(now)}
      </p>
      <Running show={show} session={session} />
    </div>
  );
}

/**
 * The wall clock, one commit a second.
 *
 * Aligned to the next whole second rather than started at an arbitrary offset,
 * so a clock opened at 12:00:00.9 does not read 12:00:00 for most of the next
 * second. After the first alignment it is a plain interval.
 */
function useNow(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    let interval: ReturnType<typeof setInterval> | null = null;
    const align = setTimeout(
      () => {
        setNow(new Date());
        interval = setInterval(() => {
          setNow(new Date());
        }, TICK_MS);
      },
      TICK_MS - (Date.now() % TICK_MS),
    );
    return () => {
      clearTimeout(align);
      if (interval !== null) {
        clearInterval(interval);
      }
    };
  }, []);
  return now;
}

/**
 * What is playing: one row per executor on this page that is running.
 *
 * The page rather than the whole show, because the strip an operator is looking
 * at is the page they are on (**D7**) — and because a rig with twelve pages of
 * chases would otherwise fill this window with rows nobody is watching.
 */
function Running({ show, session }: { readonly show: JsonValue; readonly session: JsonValue }) {
  const strips = pageStrips(session, show).filter((strip) => strip.isActive);
  if (strips.length === 0) {
    return (
      <p className="window-note" data-testid="clock-idle">
        Nothing is running on this page.
      </p>
    );
  }
  return (
    <table className="clock-running" data-testid="clock-running">
      <thead>
        <tr>
          <th scope="col">Exec</th>
          <th scope="col">Sequence</th>
          {/*
            The **position in the list**, which is what `currentCueIndex` is —
            not the cue's number, which an operator may have called `2.5`. A
            column headed *Cue* showing an index would be a number that does not
            match the one in the Sequence Sheet.
          */}
          <th scope="col">Step</th>
        </tr>
      </thead>
      <tbody>
        {strips.map((strip) => (
          <tr key={strip.executorId} data-testid={`clock-row-${String(strip.executorId)}`}>
            <td>{strip.executorId}</td>
            <td>{strip.name ?? "—"}</td>
            <td data-testid={`clock-step-${String(strip.executorId)}`}>
              {strip.currentCueIndex === null ? "—" : strip.currentCueIndex + 1}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** `14:32:07`, in the viewer's own locale-independent form. */
function timeOf(now: Date): string {
  return [now.getHours(), now.getMinutes(), now.getSeconds()]
    .map((part) => String(part).padStart(2, "0"))
    .join(":");
}

/** The date under the face, spelled so it cannot be read the American way. */
function dateOf(now: Date): string {
  const day = String(now.getDate()).padStart(2, "0");
  const month = MONTHS[now.getMonth()] ?? "";
  return `${day} ${month} ${String(now.getFullYear())}`;
}

/** Month names, so the date needs no locale and no library. */
const MONTHS: readonly string[] = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];
