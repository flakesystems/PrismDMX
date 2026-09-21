/**
 * The canvas half of `ViewSurface`, held to what it tells a 2D context.
 *
 * `jsdom` has no 2D context, so the context here is a recording one: the test
 * is of the calls — additive light is `lighter`, a body is `source-over`, a
 * beam is a gradient from the lens outwards — and not of pixels, which the
 * end-to-end suite reads back out of a real Chromium.
 */

import { describe, expect, it, vi } from "vitest";

import { canvasViewSurface } from "./surface";

/** A 2D context that remembers what was done to it. */
function recordingContext() {
  const calls: string[] = [];
  const stops: [number, string][] = [];
  const context = {
    globalCompositeOperation: "source-over",
    globalAlpha: 1,
    fillStyle: "" as unknown,
    strokeStyle: "",
    lineWidth: 1,
    font: "",
    textAlign: "",
    textBaseline: "",
    fillRect: (x: number, y: number, w: number, h: number) => calls.push(`fillRect ${String([x, y, w, h])}`),
    beginPath: () => calls.push("beginPath"),
    moveTo: (x: number, y: number) => calls.push(`moveTo ${String(x)},${String(y)}`),
    lineTo: (x: number, y: number) => calls.push(`lineTo ${String(x)},${String(y)}`),
    closePath: () => calls.push("closePath"),
    fill: () => calls.push(`fill ${context.globalCompositeOperation} ${String(context.globalAlpha)}`),
    stroke: () => calls.push(`stroke ${context.strokeStyle}`),
    fillText: (text: string) => calls.push(`fillText ${text}`),
    createLinearGradient: () => ({
      addColorStop: (offset: number, colour: string) => stops.push([offset, colour]),
    }),
  };
  return { context, calls, stops };
}

function canvasWith(context: unknown): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = 640;
  canvas.height = 360;
  vi.spyOn(canvas, "getContext").mockReturnValue(context as RenderingContext);
  return canvas;
}

describe("a view surface over a canvas", () => {
  it("is null where there is no 2D context, rather than a throw", () => {
    expect(canvasViewSurface(canvasWith(null))).toBeNull();
  });

  it("clears the whole bitmap and reports its size", () => {
    const { context, calls } = recordingContext();
    const surface = canvasViewSurface(canvasWith(context));
    expect(surface?.width).toBe(640);
    expect(surface?.height).toBe(360);
    surface?.clear("#000");
    expect(calls).toEqual(["fillRect 0,0,640,360"]);
  });

  it("adds light for an additive polygon and paints over for a body, with its outline", () => {
    const { context, calls } = recordingContext();
    const surface = canvasViewSurface(canvasWith(context));
    surface?.polygon([0, 0, 10, 0, 10, 10], 3, "red", 0.5, "add");
    surface?.polygon([0, 0, 10, 0, 10, 10, 0, 10], 4, "grey", 1, "over", "#ffd479");
    // Two points are not a polygon.
    surface?.polygon([0, 0, 1, 1], 2, "red", 1, "over");
    expect(calls).toContain("fill lighter 0.5");
    expect(calls).toContain("fill source-over 1");
    expect(calls).toContain("stroke #ffd479");
    expect(calls.filter((call) => call.startsWith("fill "))).toHaveLength(2);
    expect(calls.filter((call) => call === "closePath")).toHaveLength(2);
  });

  it("draws a beam as light fading from the lens outwards", () => {
    const { context, calls, stops } = recordingContext();
    const surface = canvasViewSurface(canvasWith(context));
    surface?.beam([5, 5, 0, 50, 10, 50], 3, 5, 5, 5, 50, "255,0,0", 0.4, 0.05);
    surface?.beam([5, 5], 1, 0, 0, 0, 0, "0,0,0", 1, 1);
    expect(stops).toEqual([
      [0, "rgba(255,0,0,0.4)"],
      [1, "rgba(255,0,0,0.05)"],
    ]);
    expect(calls).toContain("fill lighter 1");
    expect(calls.filter((call) => call.startsWith("fill "))).toHaveLength(1);
  });

  it("draws lines and labels over what is there", () => {
    const { context, calls } = recordingContext();
    const surface = canvasViewSurface(canvasWith(context));
    surface?.line(0, 0, 5, 5, "#161d27", 2);
    surface?.label("12", 30, 40, "#8b98a8", 11);
    expect(calls).toEqual(["beginPath", "moveTo 0,0", "lineTo 5,5", "stroke #161d27", "fillText 12"]);
    expect(context.font).toContain("11px");
    expect(context.textAlign).toBe("center");
  });
});
