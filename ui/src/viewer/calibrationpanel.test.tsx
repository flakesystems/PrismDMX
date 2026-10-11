/**
 * The calibration panel of the 3D viewer - B70.
 *
 * A fake daemon that answers the two calibration questions with whatever a test
 * says; what the answers *mean* is asserted where the arithmetic lives
 * (`crates/prism-domain/src/calibration.rs`) and end to end in
 * `ui/e2e/calibration.spec.ts`. What this holds is the form: what it asks, what
 * it sends, and what it shows of the answer.
 */

import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import type { Answer, Command, JsonValue, ProgrammerState, Query } from "../bindings";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskStore } from "../store/desk";
import { TelemetryProvider } from "../telemetry/panel";
import { emptyProgrammer } from "../testing/fake-daemon";
import { RecordingView } from "../testing/recording-view";
import { Shell } from "../testing/shell";
import type { Solved } from "./calibration";
import { Viewer3D } from "./viewer3d";

const SESSION: JsonValue = { session: { commandLine: "" }, views: {} };

const axis = (attribute: string, coarseOffset: number): JsonValue => ({
  attribute,
  occurrence: 0,
  coarseOffset,
  fineOffset: null,
  invert: false,
  physicalFrom: -270,
  physicalTo: 270,
  ranges: [],
});

/** Two heads and a PAR, three marks, one of them unnamed. */
function showWith(marks: JsonValue): JsonValue {
  const fixture = (id: number, typeId: string): JsonValue => ({
    id,
    name: `Fixture ${String(id)}`,
    typeId,
    universe: 1,
    address: id * 10,
    position: { x: id, y: 6, z: 2 },
    rotation: { x: 0, y: 30, z: 0 },
  });
  return {
    fixtureTypes: {
      "t.head": { id: "t.head", attributes: [axis("Pan", 0), axis("Tilt", 1)], physical: null },
      "t.par": { id: "t.par", attributes: [axis("Red", 0)], physical: null },
    },
    fixtures: { "1": fixture(1, "t.head"), "2": fixture(2, "t.par"), "3": fixture(3, "t.head") },
    marks,
  };
}

const MARKS: JsonValue = {
  "1": { id: 1, name: "DS centre", position: { x: 0, y: 0, z: -3 } },
  "2": { id: 2, name: "", position: { x: 3, y: 0, z: 1 } },
  "3": { id: 3, name: "Spike 3", position: { x: -2, y: 0, z: 4 } },
};

const SOLVED: Solved = {
  t: "Solved",
  position: { x: 1.5, y: 6.25, z: 2.5 },
  rotation: { x: 1, y: 31, z: -1 },
  fits: [
    { residual: 0.1, distance: 7 },
    { residual: 0.2, distance: 6 },
    { residual: 0.05, distance: 8 },
  ],
  rms: 0.12,
  worst: 0.2,
  positionSigma: 0.05,
  rotationSigma: 0.04,
  spare: 3,
  iterations: 8,
};

function holding(fixture: number, pan: number, tilt: number): ProgrammerState {
  return {
    ...emptyProgrammer(),
    selection: [fixture],
    values: [
      { fixture, attribute: "Pan", occurrence: 0, value: { value: pan, source: "Manual", presetRef: null } },
      { fixture, attribute: "Tilt", occurrence: 0, value: { value: tilt, source: "Manual", presetRef: null } },
    ],
  };
}

function selecting(...ids: number[]): ProgrammerState {
  return { ...emptyProgrammer(), selection: ids };
}

/** A viewer on a fake daemon that answers the calibration questions. */
function calibrating(
  options: {
    readonly programmer?: ProgrammerState;
    readonly marks?: JsonValue;
    readonly solve?: (query: Extract<Query, { t: "CalibrationSolve" }>) => Answer;
    readonly aim?: { pan: number; tilt: number; reached: boolean } | null;
  } = {},
) {
  const store = new DeskStore();
  const sent: Command[] = [];
  const asked: Query[] = [];
  let seq = 0;
  store.attach(
    (command) => {
      sent.push(command);
      seq += 1;
      return seq;
    },
    (query) => {
      asked.push(query);
      seq += 1;
      const id = seq;
      queueMicrotask(() => {
        if (query.t === "CalibrationAim") {
          store.answered(id, {
            t: "Calibration",
            fixture: query.fixture,
            answer: { t: "Aimed", aim: options.aim === undefined ? { pan: 40000, tilt: 20000, reached: true } : options.aim },
          });
        } else if (query.t === "CalibrationSolve") {
          store.answered(
            id,
            options.solve?.(query) ?? { t: "Calibration", fixture: query.fixture, answer: SOLVED },
          );
        }
      });
      return id;
    },
  );
  const surface = new RecordingView(800, 500);
  const element = (programmer: ProgrammerState, marks: JsonValue): ReactElement => (
    <Shell store={store} session={SESSION}>
      <TelemetryProvider channel={{ sink: new TelemetrySink(), scheduler: () => () => undefined }}>
        <Viewer3D show={showWith(marks)} programmer={programmer} surface={() => surface} />
      </TelemetryProvider>
    </Shell>
  );
  const view = render(element(options.programmer ?? selecting(1), options.marks ?? MARKS));
  fireEvent.click(screen.getByTestId("viewer-panel-calibrate"));
  return {
    sent,
    asked,
    again: (programmer: ProgrammerState, marks: JsonValue = options.marks ?? MARKS) => {
      view.rerender(element(programmer, marks));
    },
    commands: (kind: Command["t"]) => sent.filter((command) => command.t === kind),
    questions: (kind: Query["t"]) => asked.filter((query) => query.t === kind),
  };
}

const isDisabled = (id: string): boolean =>
  screen.getByTestId<HTMLButtonElement | HTMLInputElement>(id).disabled;

const click = (id: string): void => {
  fireEvent.click(screen.getByTestId(id));
};

const type = (id: string, value: string): void => {
  fireEvent.change(screen.getByTestId(id), { target: { value } });
};

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the panels", () => {
  it("show one at a time and keep the other's state", () => {
    calibrating();
    expect(screen.getByTestId("calibration-panel").hidden).toBe(false);
    expect(screen.getByTestId("viewer-place").hidden).toBe(true);
    click("viewer-panel-place");
    expect(screen.getByTestId("viewer-place").hidden).toBe(false);
    expect(screen.getByTestId("calibration-panel").hidden).toBe(true);
  });
});

describe("the target", () => {
  it("is the one head selected, and the panel says so", () => {
    calibrating();
    expect(screen.getByTestId("calibration-target").textContent).toContain("Fixture 1");
    expect(screen.getByTestId("calibration-target").textContent).toContain("0 of 3 sightings needed");
  });

  it("is refused in words for none, for several, and for a fixture that cannot move", () => {
    const { again } = calibrating({ programmer: selecting() });
    expect(screen.getByTestId("calibration-target").textContent).toMatch(/Select the moving head/);
    again(selecting(1, 3));
    expect(screen.getByTestId("calibration-target").textContent).toMatch(/one head at a time/);
    again(selecting(2));
    expect(screen.getByTestId("calibration-target").textContent).toMatch(/no pan and tilt/);
    // And nothing can be aimed at nothing.
    expect(isDisabled("calibration-aim-1")).toBe(true);
    expect(isDisabled("calibration-hold")).toBe(true);
  });
});

describe("the marks", () => {
  it("are listed with their places, the unnamed one by its number", () => {
    calibrating();
    const rows = screen.getByTestId("calibration-marks").querySelectorAll("li");
    expect(rows).toHaveLength(3);
    expect(rows[0]?.textContent).toContain("DS centre");
    expect(rows[0]?.textContent).toContain("0.00, 0.00, -3.00");
    expect(rows[1]?.textContent).toContain("Mark 2");
  });

  it("say how to start while there are none", () => {
    calibrating({ marks: {} });
    expect(screen.getByTestId("calibration-marks").textContent).toMatch(/No marks yet/);
  });

  it("are added with the next number, one command, once the form is whole", () => {
    const { commands } = calibrating();
    const add = screen.getByTestId<HTMLButtonElement>("calibration-save-mark");
    expect(add.disabled).toBe(true);
    type("calibration-field-x", "1,5");
    type("calibration-field-z", "-2");
    expect(add.disabled).toBe(true);
    type("calibration-field-y", "0");
    type("calibration-field-name", " Spike 4 ");
    expect(add.disabled).toBe(false);
    fireEvent.click(add);
    expect(commands("EditMarks")).toEqual([
      {
        t: "EditMarks",
        set: [{ id: 4, name: "Spike 4", position: { x: 1.5, y: 0, z: -2 } }],
        remove: [],
      },
    ]);
    // And the form is empty again for the next one.
    expect(screen.getByTestId<HTMLInputElement>("calibration-field-x").value).toBe("");
  });

  it("are changed through the form, under their own number", () => {
    const { commands } = calibrating();
    click("calibration-edit-3");
    expect(screen.getByTestId<HTMLInputElement>("calibration-field-name").value).toBe("Spike 3");
    expect(screen.getByTestId<HTMLInputElement>("calibration-field-z").value).toBe("4");
    expect(screen.getByTestId("calibration-form").textContent).toContain("Mark 3");
    type("calibration-field-z", "4.5");
    click("calibration-save-mark");
    expect(commands("EditMarks")).toEqual([
      {
        t: "EditMarks",
        set: [{ id: 3, name: "Spike 3", position: { x: -2, y: 0, z: 4.5 } }],
        remove: [],
      },
    ]);
  });

  it("can have an edit cancelled", () => {
    const { commands } = calibrating();
    click("calibration-edit-3");
    click("calibration-cancel-mark");
    expect(screen.getByTestId<HTMLInputElement>("calibration-field-name").value).toBe("");
    expect(screen.queryByTestId("calibration-cancel-mark")).toBeNull();
    expect(commands("EditMarks")).toEqual([]);
  });

  it("are taken out of the show by Delete", () => {
    const { commands } = calibrating();
    click("calibration-edit-2");
    click("calibration-delete-2");
    expect(commands("EditMarks")).toEqual([{ t: "EditMarks", set: [], remove: [2] }]);
    // The form that was editing it is empty, not left holding a mark that is gone.
    expect(screen.getByTestId<HTMLInputElement>("calibration-field-x").value).toBe("");
  });
});

describe("aiming", () => {
  it("asks where the head must point and drives the selected head there", async () => {
    const { questions, commands } = calibrating();
    click("calibration-aim-1");
    await waitFor(() => {
      expect(commands("SetAttribute")).toHaveLength(2);
    });
    expect(questions("CalibrationAim")).toEqual([
      {
        t: "CalibrationAim",
        fixture: 1,
        target: { x: 0, y: 0, z: -3 },
        // The head where the show has it: nothing has been solved yet.
        position: { x: 1, y: 6, z: 2 },
        rotation: { x: 0, y: 30, z: 0 },
        near: null,
      },
    ]);
    expect(commands("SetAttribute")).toEqual([
      { t: "SetAttribute", attribute: "Pan", occurrence: 0, value: 40000, relative: false },
      { t: "SetAttribute", attribute: "Tilt", occurrence: 0, value: 20000, relative: false },
    ]);
  });

  it("says the head where it points now, so the nearer way round is taken", async () => {
    const { questions } = calibrating({ programmer: holding(1, 111, 222) });
    click("calibration-aim-2");
    await waitFor(() => {
      expect(questions("CalibrationAim")).toHaveLength(1);
    });
    expect(questions("CalibrationAim")[0]).toMatchObject({ near: [111, 222] });
  });

  it("sends nothing for a point the head cannot be driven to", async () => {
    const { questions, commands } = calibrating({ aim: null });
    click("calibration-aim-1");
    await waitFor(() => {
      expect(questions("CalibrationAim")).toHaveLength(1);
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(commands("SetAttribute")).toEqual([]);
  });
});

describe("taking a sighting", () => {
  it("is not possible until the programmer holds a pan and a tilt", () => {
    calibrating();
    expect(isDisabled("calibration-take-1")).toBe(true);
  });

  it("files the pan and tilt the programmer holds under the mark", () => {
    calibrating({ programmer: holding(1, 32768, 16384) });
    click("calibration-take-1");
    expect(screen.getByTestId("calibration-sighting-1").textContent).toContain("pan 50.0%");
    expect(screen.getByTestId("calibration-sighting-1").textContent).toContain("tilt 25.0%");
    expect(screen.getByTestId("calibration-target").textContent).toContain("1 of 3");
  });

  it("is replaced by taking the same mark again, and forgotten by Forget", () => {
    const { again } = calibrating({ programmer: holding(1, 32768, 16384) });
    click("calibration-take-1");
    again(holding(1, 65535, 0));
    click("calibration-take-1");
    expect(screen.getByTestId("calibration-sighting-1").textContent).toContain("pan 100.0%");
    expect(screen.getByTestId("calibration-target").textContent).toContain("1 of 3");
    click("calibration-forget-1");
    expect(screen.queryByTestId("calibration-sighting-1")).toBeNull();
  });

  it("starts again for a different head", () => {
    const { again } = calibrating({ programmer: holding(1, 32768, 16384) });
    click("calibration-take-1");
    again(holding(3, 1, 1));
    expect(screen.getByTestId("calibration-target").textContent).toContain("Fixture 3");
    expect(screen.getByTestId("calibration-target").textContent).toContain("0 of 3");
    expect(screen.queryByTestId("calibration-sighting-1")).toBeNull();
  });
});

describe("the solution", () => {
  function takeThree(again: (programmer: ProgrammerState) => void): void {
    for (const [mark, pan] of [[1, 30000], [2, 31000], [3, 32000]] as const) {
      again(holding(1, pan, 20000));
      click(`calibration-take-${String(mark)}`);
    }
  }

  it("is asked for only when there are enough sightings, and shown with how to read it", async () => {
    const { again, questions } = calibrating({ programmer: holding(1, 30000, 20000) });
    click("calibration-take-1");
    expect(screen.getByTestId("calibration-result").textContent).toMatch(/Take 2 more sightings/);
    expect(questions("CalibrationSolve")).toEqual([]);
    takeThree(again);
    await waitFor(() => {
      expect(screen.queryByTestId("calibration-pose")).not.toBeNull();
    });
    expect(screen.getByTestId("calibration-pose").textContent).toContain("Hangs at 1.50, 6.25, 2.50 m");
    expect(screen.getByTestId("calibration-result").dataset.level).toBe("good");
    const asked = questions("CalibrationSolve");
    expect(asked.at(-1)).toMatchObject({
      t: "CalibrationSolve",
      fixture: 1,
      holdPosition: false,
      // The search starts from where the show has the head, however many
      // times it has been solved: the answer must not depend on the order of
      // the questions.
      position: { x: 1, y: 6, z: 2 },
      sightings: [
        { target: { x: 0, y: 0, z: -3 }, pan: 30000, tilt: 20000 },
        { target: { x: 3, y: 0, z: 1 }, pan: 31000, tilt: 20000 },
        { target: { x: -2, y: 0, z: 4 }, pan: 32000, tilt: 20000 },
      ],
    });
    // And every sighting shows how far out it is.
    expect(screen.getByTestId("calibration-sighting-2").textContent).toContain("0.20°");
  });

  it("aims the next mark from the solved pose and not from the show's", async () => {
    const { again, questions } = calibrating({ programmer: holding(1, 30000, 20000) });
    takeThree(again);
    await waitFor(() => {
      expect(screen.queryByTestId("calibration-pose")).not.toBeNull();
    });
    click("calibration-aim-3");
    await waitFor(() => {
      expect(questions("CalibrationAim")).toHaveLength(1);
    });
    expect(questions("CalibrationAim")[0]).toMatchObject({
      position: SOLVED.position,
      rotation: SOLVED.rotation,
    });
  });

  it("needs only two marks when the position is held", async () => {
    const { again, questions } = calibrating({ programmer: holding(1, 30000, 20000) });
    click("calibration-hold");
    click("calibration-take-1");
    again(holding(1, 31000, 20000));
    click("calibration-take-2");
    await waitFor(() => {
      expect(questions("CalibrationSolve")).toHaveLength(1);
    });
    expect(questions("CalibrationSolve")[0]).toMatchObject({ holdPosition: true });
    expect(screen.getByTestId("calibration-target").textContent).toContain("2 of 2");
  });

  it("warns, in the daemon's numbers, when nothing is left over to check", async () => {
    const { again } = calibrating({
      programmer: holding(1, 30000, 20000),
      solve: (query) => ({
        t: "Calibration",
        fixture: query.fixture,
        answer: { ...SOLVED, spare: 0 },
      }),
    });
    takeThree(again);
    await waitFor(() => {
      expect(screen.queryByTestId("calibration-verdict")).not.toBeNull();
    });
    expect(screen.getByTestId("calibration-result").dataset.level).toBe("warn");
    expect(screen.getByTestId("calibration-verdict").textContent).toMatch(/one more sighting/);
  });

  it("shows the daemon's own words when it refuses", async () => {
    const { again } = calibrating({
      programmer: holding(1, 30000, 20000),
      solve: (query) => ({
        t: "Calibration",
        fixture: query.fixture,
        answer: { t: "Refused", reason: "these sightings do not decide where the fixture is" },
      }),
    });
    takeThree(again);
    await waitFor(() => {
      expect(screen.getByTestId("calibration-result").textContent).toMatch(/do not decide/);
    });
    expect(isDisabled("calibration-apply")).toBe(true);
    expect(screen.queryByTestId("calibration-pose")).toBeNull();
  });

  it("is applied as one PlaceFixtures, and the sightings are spent", async () => {
    const { again, commands } = calibrating({ programmer: holding(1, 30000, 20000) });
    takeThree(again);
    await waitFor(() => {
      expect(isDisabled("calibration-apply")).toBe(false);
    });
    click("calibration-apply");
    expect(commands("PlaceFixtures")).toEqual([
      {
        t: "PlaceFixtures",
        placements: [{ id: 1, position: SOLVED.position, rotation: SOLVED.rotation }],
      },
    ]);
    expect(screen.getByTestId("calibration-target").textContent).toContain("0 of 3");
    expect(isDisabled("calibration-reset")).toBe(true);
  });

  it("is dropped when a mark it rests on is deleted", async () => {
    const { again } = calibrating({ programmer: holding(1, 30000, 20000) });
    takeThree(again);
    await waitFor(() => {
      expect(screen.queryByTestId("calibration-pose")).not.toBeNull();
    });
    // The show answers a Delete with a document without the mark.
    again(holding(1, 30000, 20000), { "1": (MARKS as Record<string, JsonValue>)["1"]!, "2": (MARKS as Record<string, JsonValue>)["2"]! });
    await waitFor(() => {
      expect(screen.queryByTestId("calibration-pose")).toBeNull();
    });
    expect(screen.getByTestId("calibration-result").textContent).toMatch(/Take 1 more sighting to see/);
  });

  it("starts over on request", () => {
    const { again } = calibrating({ programmer: holding(1, 30000, 20000) });
    takeThree(again);
    click("calibration-reset");
    expect(screen.getByTestId("calibration-target").textContent).toContain("0 of 3");
    expect(within(screen.getByTestId("calibration-marks")).queryByTestId("calibration-sighting-1")).toBeNull();
  });
});

describe("a click on the floor of the picture", () => {
  const press = (x: number, y: number): void => {
    const canvas = screen.getByTestId("viewer-canvas");
    canvas.getBoundingClientRect = () =>
      ({ left: 0, top: 0, right: 800, bottom: 500, width: 800, height: 500, x: 0, y: 0, toJSON: () => ({}) }) as DOMRect;
    fireEvent.pointerDown(canvas, { clientX: x, clientY: y, pointerId: 1 });
    fireEvent.pointerUp(canvas, { clientX: x, clientY: y, pointerId: 1 });
  };
  const value = (id: string): string => screen.getByTestId<HTMLInputElement>(id).value;

  it("fills the place of a new mark, at height nought, and sends nothing", () => {
    const { sent } = calibrating();
    type("calibration-field-name", "Spike 9");
    // Low in the picture is floor, whatever the view.
    press(400, 440);
    expect(Number.isFinite(Number(value("calibration-field-x")))).toBe(true);
    expect(value("calibration-field-x")).not.toBe("");
    expect(value("calibration-field-y")).toBe("0");
    expect(value("calibration-field-z")).not.toBe("");
    expect(value("calibration-field-name")).toBe("Spike 9");
    expect(sent).toEqual([]);
    // And the mark can then be added as it stands.
    expect(isDisabled("calibration-save-mark")).toBe(false);
  });

  it("takes an edit in progress back to a new mark", () => {
    calibrating();
    click("calibration-edit-3");
    press(400, 440);
    expect(screen.getByTestId("calibration-form").textContent).toContain("New mark");
  });

  it("is not a place while the other panel is showing", () => {
    calibrating();
    click("viewer-panel-place");
    press(400, 440);
    click("viewer-panel-calibrate");
    expect(value("calibration-field-x")).toBe("");
  });
});
