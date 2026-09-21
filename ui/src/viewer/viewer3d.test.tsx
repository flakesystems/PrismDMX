/**
 * The Viewer 3D window: the picture renders once and the loop owns it, the
 * toolbar and the pointer move a client-local camera and send nothing, a click
 * on a fixture is the Fixture Sheet's click, and the place panel sends **one**
 * `PlaceFixtures` per gesture.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import type { Command, JsonValue, ProgrammerState } from "../bindings";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskStore } from "../store/desk";
import type { Scheduler } from "../telemetry/driver";
import { TelemetryProvider } from "../telemetry/panel";
import { attachDaemon } from "../testing/console";
import { emptyProgrammer } from "../testing/fake-daemon";
import { RecordingView } from "../testing/recording-view";
import { Shell } from "../testing/shell";
import { finalShow, litPayload } from "../testing/viewer-recording";
import { Viewer3D } from "./viewer3d";

const SESSION: JsonValue = { session: { commandLine: "" }, views: {} };

function manualFrames(): { scheduler: Scheduler; step: (elapsed?: number) => void } {
  let run: ((now: number) => void) | null = null;
  let now = 0;
  return {
    scheduler: (callback) => {
      run = callback;
      return () => {
        run = null;
      };
    },
    step: (elapsed = 16) => {
      now += elapsed;
      run?.(now);
    },
  };
}

function selecting(ids: readonly number[]): ProgrammerState {
  return { ...emptyProgrammer(), selection: [...ids] };
}

function viewer(options: { readonly programmer?: ProgrammerState | null; readonly show?: JsonValue } = {}) {
  const store = new DeskStore();
  const sent: Command[] = [];
  attachDaemon(store, sent);
  const sink = new TelemetrySink();
  const frames = manualFrames();
  const surface = new RecordingView(800, 500);
  const makeSurface = () => surface;
  const view = render(
    <Shell store={store} session={SESSION}>
      <TelemetryProvider channel={{ sink, scheduler: frames.scheduler }}>
        <Viewer3D show={options.show ?? finalShow()} programmer={options.programmer ?? null} surface={makeSurface} />
      </TelemetryProvider>
    </Shell>,
  );
  const stats = (): HTMLElement => screen.getByTestId("viewer-stats");
  const places = (): Command[] => sent.filter((command) => command.t === "PlaceFixtures");
  return { view, sent, sink, frames, surface, stats, places };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the picture", () => {
  it("is the rig, and says what is lit once a frame arrives", () => {
    const { frames, sink, stats } = viewer();
    frames.step();
    expect(stats().textContent).toContain("3 fixtures · 0 lit");
    sink.accept(litPayload());
    frames.step();
    expect(stats().dataset.lit).toBe("1");
    expect(stats().textContent).toContain("1 lit");
  });

  it("repaints when the toolbar or the pointer moves the camera, and sends nothing for it", () => {
    const { frames, stats, sent } = viewer();
    // A step longer than the readout's quarter second, so the count it
    // writes is the count after each step.
    const second = 300;
    frames.step(second);
    const painted = (): number => Number(stats().dataset.painted);
    const before = painted();
    for (const id of ["viewer-view-top", "viewer-view-front", "viewer-view-side", "viewer-view-perspective", "viewer-frame"]) {
      fireEvent.click(screen.getByTestId(id));
      frames.step(second);
    }
    const canvas = screen.getByTestId("viewer-canvas");
    fireEvent.pointerDown(canvas, { clientX: 100, clientY: 100, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 102, clientY: 101, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 160, clientY: 130, pointerId: 1 });
    fireEvent.pointerUp(canvas, { clientX: 160, clientY: 130, pointerId: 1 });
    frames.step(second);
    fireEvent.pointerDown(canvas, { clientX: 100, clientY: 100, shiftKey: true, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 140, clientY: 90, pointerId: 1 });
    fireEvent.pointerUp(canvas, { clientX: 140, clientY: 90, pointerId: 1 });
    frames.step(second);
    fireEvent.wheel(canvas, { deltaY: 100 });
    frames.step(second);
    fireEvent.contextMenu(canvas);
    fireEvent.pointerMove(canvas, { clientX: 1, clientY: 1, pointerId: 1 });
    expect(painted()).toBeGreaterThanOrEqual(before + 8);
    expect(sent).toEqual([]);
  });

  it("offers a clicked fixture to the line, as a Fixture Sheet row does", async () => {
    const { frames, surface, sent } = viewer();
    frames.step();
    const label = surface.labels.find((each) => each.text === "2");
    expect(label).toBeDefined();
    const canvas = screen.getByTestId("viewer-canvas");
    const x = label?.x ?? 0;
    const y = (label?.y ?? 0) + 8;
    fireEvent.pointerDown(canvas, { clientX: x, clientY: y, pointerId: 1 });
    fireEvent.pointerUp(canvas, { clientX: x, clientY: y, pointerId: 1 });
    await waitFor(() => {
      expect(
        sent.some((command) => command.t === "CommandLineInput" && command.text.includes("Fixture 2")),
      ).toBe(true);
    });
    // And a click on nothing is nothing.
    const count = sent.length;
    fireEvent.pointerDown(canvas, { clientX: 2, clientY: 2, pointerId: 1 });
    fireEvent.pointerUp(canvas, { clientX: 2, clientY: 2, pointerId: 1 });
    expect(sent.length).toBe(count);
  });

  it("stops drawing when the window goes", () => {
    const { frames, view, stats } = viewer();
    frames.step();
    const node = stats();
    const painted = node.dataset.painted;
    view.unmount();
    frames.step();
    expect(node.dataset.painted).toBe(painted);
  });
});

describe("the place panel", () => {
  it("asks for a selection and sends nothing without one", () => {
    const { places } = viewer();
    expect(screen.getByTestId("viewer-place-selection").textContent).toContain("Select fixtures");
    expect(screen.getByTestId("viewer-place-set")).toHaveProperty("disabled", true);
    expect(screen.getByTestId("viewer-place-spread")).toHaveProperty("disabled", true);
    expect(places()).toEqual([]);
  });

  it("starts from where the first selected fixture hangs, and Set is one command for all of them", () => {
    const { places } = viewer({ programmer: selecting([1, 2]) });
    expect(screen.getByTestId("viewer-place-selection").textContent).toBe("2 fixtures: 1, 2");
    expect(screen.getByTestId("viewer-place-x")).toHaveProperty("value", "-2");
    expect(screen.getByTestId("viewer-place-y")).toHaveProperty("value", "6");
    fireEvent.change(screen.getByTestId("viewer-place-x"), { target: { value: "" } });
    fireEvent.change(screen.getByTestId("viewer-place-y"), { target: { value: "7,5" } });
    fireEvent.click(screen.getByTestId("viewer-place-set"));
    expect(places()).toEqual([
      {
        t: "PlaceFixtures",
        placements: [
          { id: 1, position: { x: -2, y: 7.5, z: 1 }, rotation: { x: 0, y: 0, z: 0 } },
          { id: 2, position: { x: 2, y: 7.5, z: 1 }, rotation: { x: 0, y: 0, z: 0 } },
        ],
      },
    ]);
  });

  it("spreads the selection across the stage in one command", () => {
    const { places } = viewer({ programmer: selecting([10, 1, 2]) });
    fireEvent.change(screen.getByTestId("viewer-place-x"), { target: { value: "0" } });
    fireEvent.change(screen.getByTestId("viewer-place-spacing"), { target: { value: "3" } });
    fireEvent.click(screen.getByTestId("viewer-place-spread"));
    const [command] = places();
    expect(command?.t).toBe("PlaceFixtures");
    if (command?.t !== "PlaceFixtures") {
      return;
    }
    expect(command.placements.map((place) => [place.id, place.position.x])).toEqual([
      [10, -3],
      [1, 0],
      [2, 3],
    ]);
  });

  it("refuses a field that is not a number, and names one fixture as itself", () => {
    viewer({ programmer: selecting([2]) });
    expect(screen.getByTestId("viewer-place-selection").textContent).toBe("Fixture 2");
    fireEvent.change(screen.getByTestId("viewer-place-z"), { target: { value: "deep" } });
    expect(screen.getByTestId("viewer-place-z").getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByTestId("viewer-place-set")).toHaveProperty("disabled", true);
  });

  it("follows the selection: a new first fixture refills the form", () => {
    const { view } = viewer({ programmer: selecting([1]) });
    expect(screen.getByTestId("viewer-place-x")).toHaveProperty("value", "-2");
    view.rerender(
      <Shell store={new DeskStore()} session={SESSION}>
        <Viewer3D show={finalShow()} programmer={selecting([10])} surface={() => new RecordingView()} />
      </Shell>,
    );
    expect(screen.getByTestId("viewer-place-z")).toHaveProperty("value", "4");
  });

  it("lists at most eight numbers of a long selection", () => {
    const fixtures: Record<string, JsonValue> = {};
    for (let id = 1; id <= 12; id += 1) {
      fixtures[String(id)] = { typeId: "none", universe: 1, address: id };
    }
    viewer({ show: { fixtures }, programmer: selecting([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]) });
    expect(screen.getByTestId("viewer-place-selection").textContent).toBe("10 fixtures: 1, 2, 3, 4, 5, 6, 7, 8 …");
  });
});
