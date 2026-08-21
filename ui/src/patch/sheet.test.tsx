
/**
 * The Fixture Sheet: two live columns, and only one of them in React.
 *
 * What is asserted here is the split S27's exit criterion is about. The
 * **programmer** column is a reader over `Delta::ProgrammerChanged` and renders
 * like everything else. The **output** column is telemetry, so it is drawn on a
 * canvas by `./live.ts` — and `telemetry/render.test.tsx`, which counts React
 * commits over 300 frames of 64 universes and must stay at zero, is what says
 * that in the strongest possible form. This file asserts the two columns show
 * what they claim to.
 *
 * The show is the recorded one a real `prismd` was holding.
 */

import { decode } from "@msgpack/msgpack";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import type { JsonValue, ProgrammerState } from "../bindings";
import { TelemetrySink } from "../ipc/telemetry";
import { readServerMessage } from "../ipc/protocol";
import { nullSink, setLogSink } from "../log/logger";
import { RecordingSurface } from "../testing/recording-surface";
import { Shell } from "../testing/shell";
import { narrowFrame } from "../testing/telemetry-frames";
import { DeskStore } from "../store/desk";
import type { Scheduler } from "../telemetry/driver";
import { TelemetryProvider } from "../telemetry/panel";
import { FixtureSheet } from "./sheet";

import recordingText from "../../tests/fixtures/patch-recording.json?raw";

const recording = JSON.parse(recordingText) as { readonly initialSnapshot: string };

/** The show the recorded daemon was holding: three fixtures, two profiles. */
const show: JsonValue = (() => {
  const binary = atob(recording.initialSnapshot);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  const message = readServerMessage(decode(bytes));
  if (message.t !== "Snapshot") {
    throw new Error("the recording does not start with a snapshot");
  }
  return message.snapshot.show;
})();

/** A session on one encoder bank. */
function sessionOn(bank: string): JsonValue {
  return { session: { encoderBank: bank } };
}

/** A programmer holding the values given. */
function programmer(
  values: readonly { fixture: number; attribute: string; value: number }[],
  selection: number[] = [],
): ProgrammerState {
  return {
    selection,
    activeFeatureGroup: "Dimmer",
    values: values.map((entry) => ({
      fixture: entry.fixture,
      attribute: entry.attribute as ProgrammerState["values"][number]["attribute"],
      value: { value: entry.value, source: "Manual", presetRef: null },
    })),
    clearStage: 0,
  };
}

/** A scheduler the test steps by hand. */
function manualFrames(): { scheduler: Scheduler; step: () => void } {
  let run: ((now: number) => void) | null = null;
  return {
    scheduler: (callback) => {
      run = callback;
      return () => {
        run = null;
      };
    },
    step: () => {
      run?.(0);
    },
  };
}

/** Renders the sheet with a telemetry channel the test drives. */
function sheet(options: {
  readonly bank?: string;
  readonly programmer?: ProgrammerState | null;
  readonly surface?: RecordingSurface | null;
  readonly show?: JsonValue;
}) {
  const sink = new TelemetrySink();
  const frames = manualFrames();
  const surface = options.surface === undefined ? new RecordingSurface(120, 200) : options.surface;
  const view = render(
    <Shell store={new DeskStore()} session={sessionOn(options.bank ?? "Dimmer")}>
      <TelemetryProvider
        channel={{ sink, surface: () => surface, scheduler: frames.scheduler }}
      >
        <FixtureSheet
          show={options.show ?? show}
          session={sessionOn(options.bank ?? "Dimmer")}
          programmer={options.programmer ?? null}
        />
      </TelemetryProvider>
    </Shell>,
  );
  return { view, sink, frames, surface };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the fixture sheet", () => {
  it("shows every patched fixture, with the parameters of the bank in force", () => {
    sheet({ bank: "Color" });
    expect(screen.getByTestId("sheet-bank").textContent).toContain("Color");
    for (const id of [1, 2, 5]) {
      expect(screen.getByTestId(`sheet-row-${String(id)}`)).not.toBeNull();
    }
    // The Colour bank's parameters, out of the generated table — the same one
    // the encoder bar and the jog wheel walk (S26).
    for (const attribute of ["Red", "Green", "Blue", "White", "Amber"]) {
      expect(screen.getAllByText(attribute).length).toBeGreaterThan(0);
    }
  });

  it("follows the bank, because the bank is the session's", () => {
    const { view } = sheet({ bank: "Position" });
    expect(screen.getByTestId("sheet-bank").textContent).toContain("Position");
    expect(screen.getAllByText("Pan").length).toBeGreaterThan(0);
    expect(screen.queryByText("Red")).toBeNull();
    view.unmount();
  });

  it("shows a dash for an untouched attribute and a blank for one the fixture has not got", () => {
    // **Absent is not zero.** The programmer is sparse, so an attribute nobody
    // has touched means *the playbacks decide* — which an interface showing
    // 0 % would teach an operator is the same thing. And a dimmer has no red
    // channel at all, which is a third fact and reads as nothing.
    sheet({
      bank: "Color",
      programmer: programmer([{ fixture: 5, attribute: "Red", value: 32767 }]),
    });
    expect(screen.getByTestId("prog-5-Red").textContent).toBe("50%");
    expect(screen.getByTestId("prog-5-Green").textContent).toBe("—");
    // Fixture 1 is a one-channel dimmer: it has no colour attributes.
    expect(screen.getByTestId("prog-1-Red").textContent).toBe("");
  });

  it("marks the fixtures the programmer has selected", () => {
    sheet({ programmer: programmer([], [2, 5]) });
    expect(screen.getByTestId("sheet-row-1").className).not.toContain("row-selected");
    expect(screen.getByTestId("sheet-row-2").className).toContain("row-selected");
    expect(screen.getByTestId("sheet-row-5").className).toContain("row-selected");
  });

  it("draws what is on the cable, and does it outside React", () => {
    // The output column is the picture the telemetry loop paints. It is asserted
    // here as *drawing happened for every row*; where each channel's bar went is
    // `live.test.ts`, per pixel.
    const { sink, frames, surface } = sheet({});
    expect(surface).not.toBeNull();
    sink.accept(narrowFrame(0));
    frames.step();
    expect(surface?.clears.length).toBe(1);
    // Three fixtures, so at least three rows were painted.
    expect(surface?.fills.length).toBeGreaterThanOrEqual(3);
  });

  it("is a window with nothing in it when nothing is patched", () => {
    sheet({ show: { fixtures: {}, fixtureTypes: {} } });
    expect(screen.getByText(/Nothing is patched/)).not.toBeNull();
    expect(screen.queryByTestId("sheet-canvas")).toBeNull();
  });

  it("renders without a canvas where there is no 2D context to be had", () => {
    // Every `jsdom` browser, and any real one that has run out of contexts. A
    // sheet is a *view* of something happening whether or not it can be drawn.
    const { sink, frames } = sheet({ surface: null });
    sink.accept(narrowFrame(0));
    frames.step();
    expect(screen.getByTestId("sheet-canvas")).not.toBeNull();
    expect(screen.getByTestId("sheet-row-1")).not.toBeNull();
  });
});
