/**
 * What the channel reports about itself.
 *
 * Two claims matter here. The first is that **losing telemetry is counted, not
 * inferred**: the daemon coalesces and drops (§8), every frame carries a
 * sequence number, and a gap is arithmetic rather than a guess about timing.
 * The second is that the paint window is a window — the p99 the frame budget is
 * judged on has to be the worst frame in a few seconds, not the average of the
 * whole run, or a panel that stuttered once a minute would read as healthy.
 */

import { describe, expect, it } from "vitest";

import { PAINT_WINDOW, STALE_AFTER_MS, TelemetryStats, summaryText } from "./stats";

describe("counting the channel", () => {
  it("counts what arrived and what the sequence numbers say never did", () => {
    const stats = new TelemetryStats();
    stats.accept(10n, 2, 1000);
    stats.accept(11n, 2, 1033);
    // Three frames the daemon sent and this client never saw — dropped for a
    // slow reader, which §8 says is allowed and worth showing.
    stats.accept(15n, 2, 1133);

    const summary = stats.summary(1133);
    expect(summary.frames).toBe(3);
    expect(summary.lost).toBe(3);
    expect(summary.universes).toBe(2);
    expect(summary.dropped).toBe(0);
  });

  it("counts a sequence that has not moved as nothing lost", () => {
    const stats = new TelemetryStats();
    stats.accept(4n, 1, 0);
    stats.accept(5n, 1, 33);
    expect(stats.summary(33).lost).toBe(0);
  });

  it("does not call the first frame a gap, however large its number", () => {
    const stats = new TelemetryStats();
    // A client that connects to a daemon which has been running for a day.
    stats.accept(2_600_000n, 64, 0);
    expect(stats.summary(0).lost).toBe(0);
  });

  it("counts frames it could not read, by fault", () => {
    const stats = new TelemetryStats();
    stats.dropped({ kind: "unknown-version", found: 2, expected: 1 });
    stats.dropped({ kind: "unknown-version", found: 2, expected: 1 });
    stats.dropped({ kind: "not-telemetry" });

    expect(stats.faults("unknown-version")).toBe(2);
    expect(stats.faults("not-telemetry")).toBe(1);
    expect(stats.faults("truncated")).toBe(0);
    expect(stats.summary(0).dropped).toBe(3);
    // And none of it counts as a frame: a picture that could not be read is not
    // a picture.
    expect(stats.summary(0).frames).toBe(0);
  });

  it("works the rate out from the frames it actually saw", () => {
    const stats = new TelemetryStats();
    for (let frame = 0; frame < 31; frame += 1) {
      stats.accept(BigInt(frame + 1), 4, frame * (1000 / 30));
    }
    // Thirty intervals over a second: 30 Hz, not 31.
    expect(stats.summary(1000).hz).toBeCloseTo(30, 1);
  });

  it("has no rate to report from one frame", () => {
    const stats = new TelemetryStats();
    stats.accept(1n, 4, 500);
    expect(stats.summary(500).hz).toBe(0);
  });
});

describe("the paint window", () => {
  it("reports the median and the 99th percentile of what it holds", () => {
    const stats = new TelemetryStats();
    for (let sample = 1; sample <= 100; sample += 1) {
      stats.painted(sample / 10);
    }
    const summary = stats.summary(0);
    expect(summary.paintMedian).toBeCloseTo(5.0, 5);
    expect(summary.paintP99).toBeCloseTo(9.9, 5);
    expect(summary.paintWorst).toBeCloseTo(10.0, 5);
  });

  it("forgets what fell out of the window, but not the worst frame ever seen", () => {
    const stats = new TelemetryStats();
    stats.painted(40);
    for (let sample = 0; sample < PAINT_WINDOW; sample += 1) {
      stats.painted(1);
    }
    const summary = stats.summary(0);
    // The 40 ms frame is out of the window — the p99 is of the last four
    // seconds — but it happened, and a panel that hid it would be lying.
    expect(summary.paintP99).toBe(1);
    expect(summary.paintWorst).toBe(40);
  });

  it("has nothing to report before the first paint", () => {
    const summary = new TelemetryStats().summary(0);
    expect(summary.paintMedian).toBe(0);
    expect(summary.paintP99).toBe(0);
  });
});

describe("staleness", () => {
  it("calls a picture stale when nothing has arrived for long enough", () => {
    const stats = new TelemetryStats();
    expect(stats.summary(0).stale).toBe(true);

    stats.accept(1n, 8, 1000);
    expect(stats.summary(1000).stale).toBe(false);
    expect(stats.summary(1000 + STALE_AFTER_MS).stale).toBe(false);
    expect(stats.summary(1001 + STALE_AFTER_MS).stale).toBe(true);
    expect(stats.lastAt).toBe(1000);
  });

  it("forgets everything when the connection does", () => {
    const stats = new TelemetryStats();
    stats.accept(9n, 8, 1000);
    stats.painted(3);
    stats.dropped({ kind: "not-telemetry" });
    stats.reset();

    const summary = stats.summary(1000);
    expect(summary).toMatchObject({ frames: 0, lost: 0, dropped: 0, universes: 0, stale: true });
    expect(stats.frames).toBe(0);
    expect(stats.faults("not-telemetry")).toBe(0);
    // And a frame after a reset is a first frame, not a gap of nine.
    stats.accept(90n, 8, 2000);
    expect(stats.summary(2000).lost).toBe(0);
  });
});

describe("the readout", () => {
  it("says what is there before anything is", () => {
    const stats = new TelemetryStats();
    expect(summaryText(stats.summary(0))).toBe("waiting for telemetry");
    stats.dropped({ kind: "unknown-version", found: 2, expected: 1 });
    expect(summaryText(stats.summary(0))).toContain("1 frame dropped");
    stats.dropped({ kind: "unknown-version", found: 2, expected: 1 });
    expect(summaryText(stats.summary(0))).toContain("2 frames dropped");
  });

  it("names the universes, the rate and the paint, in that order", () => {
    const stats = new TelemetryStats();
    for (let frame = 0; frame < 31; frame += 1) {
      stats.accept(BigInt(frame + 1), 64, frame * (1000 / 30));
      stats.painted(1.25);
    }
    const text = summaryText(stats.summary(1000));
    expect(text).toContain("64 universes");
    expect(text).toContain("30.0 Hz");
    expect(text).toContain("paint 1.25 ms (p99 1.25 ms)");
    expect(text).toContain("31 frames");
    expect(text).not.toContain("lost");
    expect(text).not.toContain("not live");
  });

  it("says so, first, when what is on the canvas is not a picture of now", () => {
    const stats = new TelemetryStats();
    stats.accept(1n, 4, 0);
    stats.accept(5n, 4, 33);
    const text = summaryText(stats.summary(10_000));
    expect(text.startsWith("not live")).toBe(true);
    expect(text).toContain("3 lost");
  });
});
