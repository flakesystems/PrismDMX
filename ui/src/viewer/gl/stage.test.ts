/**
 * The one decision of the stage that needs no WebGL — **S30b**: when it has to
 * size its renderer again. The rest of `Stage` draws, and the end-to-end suite
 * looks at what it draws.
 */

import { describe, expect, it } from "vitest";

import { sizeChanged } from "./stage";

describe("sizeChanged", () => {
  it("sizes a new stage the first time, whatever the canvas already is", () => {
    // A stage built when the detail changes takes over a canvas the last one
    // sized; before this rule it compared with that and left the camera square.
    expect(sizeChanged(null, 800, 450, 1)).toBe(true);
  });

  it("sizes again when the window or the resolution changes, and only then", () => {
    const applied = { width: 800, height: 450, scale: 1 };
    expect(sizeChanged(applied, 800, 450, 1)).toBe(false);
    expect(sizeChanged(applied, 801, 450, 1)).toBe(true);
    expect(sizeChanged(applied, 800, 300, 1)).toBe(true);
    expect(sizeChanged(applied, 800, 450, 1.5)).toBe(true);
  });
});
