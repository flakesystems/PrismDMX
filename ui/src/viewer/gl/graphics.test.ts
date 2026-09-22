/**
 * What this browser draws 3D with — **S30b**. A canvas is made up here, since
 * jsdom has no WebGL: the question is what the answers are read as.
 */

import { describe, expect, it } from "vitest";

import { isSoftwareRenderer, probeGraphics } from "./graphics";

/** A canvas whose WebGL answers `renderer`, or has none when it is `null`. */
function canvasSaying(renderer: string | null, masked = false): () => HTMLCanvasElement {
  const lost: string[] = [];
  const gl = {
    RENDERER: 0x1f01,
    getExtension: (name: string) => {
      if (name === "WEBGL_debug_renderer_info") {
        return masked ? { UNMASKED_RENDERER_WEBGL: 0x9246 } : null;
      }
      if (name === "WEBGL_lose_context") {
        return { loseContext: () => lost.push("lost") };
      }
      return null;
    },
    getParameter: (which: number) => (which === 0x9246 || which === 0x1f01 ? renderer : null),
  };
  return () =>
    ({
      getContext: (kind: string) => (kind === "webgl2" && renderer !== null ? gl : null),
    }) as unknown as HTMLCanvasElement;
}

describe("isSoftwareRenderer", () => {
  it("knows the software rasterisers by name, and a graphics card is not one", () => {
    expect(isSoftwareRenderer("ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)")).toBe(true);
    expect(isSoftwareRenderer("llvmpipe (LLVM 15.0.7, 256 bits)")).toBe(true);
    expect(isSoftwareRenderer("ANGLE (Microsoft, Microsoft Basic Render Driver Direct3D11)")).toBe(true);
    expect(isSoftwareRenderer("ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0)")).toBe(false);
    expect(isSoftwareRenderer("Apple M2")).toBe(false);
  });
});

describe("probeGraphics", () => {
  it("says a browser with no WebGL has none", () => {
    expect(probeGraphics(canvasSaying(null))).toEqual({ webgl: false, software: false, renderer: "" });
  });

  it("reads the renderer's name, masked or not, and lets the context go", () => {
    expect(probeGraphics(canvasSaying("llvmpipe (LLVM 15)"))).toEqual({ webgl: true, software: true, renderer: "llvmpipe (LLVM 15)" });
    expect(probeGraphics(canvasSaying("Apple M2", true))).toEqual({ webgl: true, software: false, renderer: "Apple M2" });
  });

  it("is no WebGL when asking throws", () => {
    const throwing = () =>
      ({
        getContext: () => {
          throw new Error("blocked");
        },
      }) as unknown as HTMLCanvasElement;
    expect(probeGraphics(throwing).webgl).toBe(false);
  });
});
