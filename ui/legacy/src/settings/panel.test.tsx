/**
 * The drawing of the desk — S59.
 *
 * What this file is about is the **join**: the drawing holds no picture of an
 * X-Touch, it draws what the daemon sent on each row, and every claim here is
 * one of the ways that can go wrong. A component that had the layout built into
 * it would pass a test written as *it draws sixty-four keys*, so nothing here is
 * written that way.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ControlBox, PanelLayout, SurfaceControl } from "../bindings";
import { DeskDrawing } from "./panel";

const PANEL: PanelLayout = { width: 1000, height: 520, strips: 8, stripPitch: 58 };

function box(x: number, y: number, shape: ControlBox["shape"] = "Key"): ControlBox {
  return { x, y, w: 46, h: 22, shape };
}

function row(
  name: string,
  control: SurfaceControl["control"],
  extra: Partial<SurfaceControl> = {},
): SurfaceControl {
  return {
    control,
    name,
    action: null,
    permanent: false,
    reserved: false,
    geometry: box(10, 20),
    ...extra,
  };
}

const CONTROLS: readonly SurfaceControl[] = [
  row(
    "Global.Save",
    { t: "Global", button: "Save" },
    { action: { t: "SaveShow" }, geometry: box(540, 16) },
  ),
  row("Global.F1", { t: "Global", button: "F1" }, { geometry: box(600, 16) }),
  row(
    "Global.SmpteBeats",
    { t: "Global", button: "SmpteBeats" },
    { reserved: true, geometry: box(660, 16) },
  ),
  row(
    "Strip[*].Button.Select",
    { t: "StripButton", button: "Select" },
    { geometry: box(16, 176) },
  ),
  row("Strip[*].Fader", { t: "StripFader" }, { geometry: box(26, 214, "Fader") }),
  row("Global.Jog", { t: "Jog" }, { geometry: box(540, 390, "Wheel") }),
];

describe("the drawing of the desk", () => {
  it("draws a key where the daemon says it is", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={[]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    const save = screen.getByTestId("desk-Global.Save");
    const rect = save.querySelector("rect");
    expect(rect?.getAttribute("x")).toBe("540");
    expect(rect?.getAttribute("y")).toBe("16");
  });

  /**
   * **The panel comes from the daemon too**, so a device nobody has drawn is a
   * list and no picture. Drawing something anyway would be drawing the wrong
   * desk, which is worse than drawing nothing.
   */
  it("draws nothing at all without a panel, and says why", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={null}
        lamps={[]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    expect(screen.queryByTestId("desk-drawing")).toBeNull();
    expect(screen.getByTestId("panel-undrawn")).toBeTruthy();
  });

  /** And a control with no box of its own is simply left out. */
  it("leaves out a control the profile gives no place", () => {
    render(
      <DeskDrawing
        controls={[row("Global.F2", { t: "Global", button: "F2" }, { geometry: null })]}
        panel={PANEL}
        lamps={[]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    expect(screen.queryByTestId("desk-Global.F2")).toBeNull();
  });

  /**
   * **One row, eight columns.** `Strip[*]` is a single entry of the binding
   * table because D7 says the faders are a bank — so the drawing repeats the
   * column rather than being handed eight boxes that would all say the same
   * thing.
   */
  it("repeats the strip column across the bank", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={[]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    const drawn = screen.getAllByTestId("desk-Strip[*].Button.Select");
    expect(drawn).toHaveLength(PANEL.strips);
    const first = drawn[0]?.querySelector("rect")?.getAttribute("x");
    const second = drawn[1]?.querySelector("rect")?.getAttribute("x");
    expect(first).toBe("16");
    // 16 + one pitch, which is the one arithmetic this component does.
    expect(second).toBe(String(16 + PANEL.stripPitch));
  });

  /** A lit key is marked, by the name the daemon sent. */
  it("lights the keys the daemon says are lit", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={["Global.Save"]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    expect(screen.getByTestId("desk-Global.Save").dataset.lit).toBe("yes");
    expect(screen.getByTestId("desk-Global.F1").dataset.lit).toBe("no");
  });

  /** A name this build has no row for lights nothing and throws nothing. */
  it("ignores a lit name it has no key for", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={["Global.Wibble"]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    for (const control of CONTROLS) {
      const drawn = screen.getAllByTestId(`desk-${control.name}`);
      for (const one of drawn) {
        expect(one.dataset.lit).toBe("no");
      }
    }
  });

  /**
   * Bound and free are told apart, which is the question the picture is for.
   */
  it("tells a bound key from a free one", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={[]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    expect(screen.getByTestId("desk-Global.Save").dataset.bound).toBe("yes");
    expect(screen.getByTestId("desk-Global.F1").dataset.bound).toBe("no");
  });

  it("hands the row back when a key is clicked", () => {
    const onPick = vi.fn();
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={[]}
        arming={null}
        onPick={onPick}
      />,
    );
    fireEvent.click(screen.getByTestId("desk-Global.Save"));
    expect(onPick).toHaveBeenCalledTimes(1);
    expect(onPick.mock.calls[0]?.[0]).toMatchObject({ name: "Global.Save" });
  });

  /** A knob and the wheel are circles; a key and a fader are not. */
  it("draws each control as the shape its box says", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={[]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    expect(screen.getByTestId("desk-Global.Jog").querySelector("circle")).toBeTruthy();
    expect(screen.getByTestId("desk-Global.Save").querySelector("rect")).toBeTruthy();
    expect(screen.getAllByTestId("desk-Strip[*].Fader")[0]?.querySelector("rect")).toBeTruthy();
  });

  /** The reserved control is drawn and marked rather than left off. */
  it("draws the reserved control, marked", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={[]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    const reserved = screen.getByTestId("desk-Global.SmpteBeats");
    expect(reserved.getAttribute("class")).toContain("desk-reserved");
  });

  /** The panel's own proportions are what the drawing is scaled by. */
  it("takes its view box from the panel the daemon sent", () => {
    render(
      <DeskDrawing
        controls={CONTROLS}
        panel={PANEL}
        lamps={[]}
        arming={null}
        onPick={vi.fn()}
      />,
    );
    expect(screen.getByTestId("desk-drawing").getAttribute("viewBox")).toBe("0 0 1000 520");
  });
});
