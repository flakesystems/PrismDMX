/**
 * The detail setting and the haze — **S30b**: client-local, remembered by the
 * browser, and a browser that will not remember is not an error.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import {
  DEFAULT_DETAIL,
  DETAIL,
  DETAIL_LEVELS,
  isDetailLevel,
  rememberDetail,
  rememberHaze,
  rememberedDetail,
  rememberedHaze,
} from "./detail";

afterEach(() => {
  localStorage.clear();
  vi.restoreAllMocks();
});

describe("the levels", () => {
  it("draw more at every step up, and only the lowest goes without models", () => {
    for (let index = 1; index < DETAIL_LEVELS.length; index += 1) {
      const lower = DETAIL[DETAIL_LEVELS[index - 1] ?? "low"];
      const higher = DETAIL[DETAIL_LEVELS[index] ?? "low"];
      expect(higher.steps).toBeGreaterThanOrEqual(lower.steps);
      expect(higher.projectors).toBeGreaterThan(lower.projectors);
      expect(higher.mask).toBeGreaterThan(lower.mask);
      expect(higher.segments).toBeGreaterThan(lower.segments);
      expect(higher.pixelRatio).toBeGreaterThanOrEqual(lower.pixelRatio);
    }
    expect(DETAIL.low.models).toBe(false);
    expect(DETAIL.low.steps).toBe(0);
    expect(DETAIL.medium.models).toBe(true);
  });

  it("recognises its own names and nothing else", () => {
    expect(DETAIL_LEVELS.every(isDetailLevel)).toBe(true);
    expect(isDetailLevel("extreme")).toBe(false);
    expect(isDetailLevel(3)).toBe(false);
  });
});

describe("remembering", () => {
  it("starts at the default and keeps what was chosen", () => {
    expect(rememberedDetail()).toBe(DEFAULT_DETAIL);
    rememberDetail("ultra");
    expect(rememberedDetail()).toBe("ultra");
    localStorage.setItem("prismdmx.viewer.detail", "extreme");
    expect(rememberedDetail()).toBe(DEFAULT_DETAIL);
  });

  it("keeps the haze, and takes nothing out of range", () => {
    expect(rememberedHaze()).toBe(0.5);
    rememberHaze(0);
    expect(rememberedHaze()).toBe(0);
    rememberHaze(0.8);
    expect(rememberedHaze()).toBe(0.8);
    localStorage.setItem("prismdmx.viewer.haze", "7");
    expect(rememberedHaze()).toBe(0.5);
  });

  it("goes on without a browser that will store", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("private window");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("quota");
    });
    expect(() => rememberDetail("high")).not.toThrow();
    expect(() => rememberHaze(0.2)).not.toThrow();
    expect(rememberedDetail()).toBe(DEFAULT_DETAIL);
    expect(rememberedHaze()).toBe(0.5);
  });
});

describe("a starting level other than the default", () => {
  it("is used until the browser remembers a choice", () => {
    localStorage.clear();
    expect(rememberedDetail("low")).toBe("low");
    rememberDetail("high");
    expect(rememberedDetail("low")).toBe("high");
    localStorage.clear();
  });
});

describe("beams in the haze", () => {
  it("are capped higher at every level", () => {
    const caps = DETAIL_LEVELS.map((level) => DETAIL[level].volumes);
    expect(caps).toEqual([...caps].sort((a, b) => a - b));
    expect(new Set(caps).size).toBe(caps.length);
  });
});
