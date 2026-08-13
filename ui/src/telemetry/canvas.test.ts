/**
 * The half of the painter that touches a real canvas.
 *
 * `LevelSurface` is the seam and everything above it is asserted against a
 * recording one — but the code that turns those five operations into canvas
 * calls is what actually runs in front of an operator, and it is where the
 * things that make a canvas *look* wrong live: a smoothed blit is a blurred
 * grid, a source rectangle in the wrong units is a picture of the top-left
 * corner of the rig, and an `ImageData` allocated per frame is four megabytes a
 * second of rubbish.
 *
 * `jsdom` has no rasteriser, so the context is replaced with one that records.
 * That is the one place in this session where a test asserts a type it built
 * itself — the same allowance S23's four assertions carry, and for the same
 * reason: a test may know what it has just constructed.
 */

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { canvasSurface } from "./painter";

/** Everything the surface asked a context to do. */
interface Recorded {
  readonly what: string;
  readonly with: readonly unknown[];
}

/** A 2D context that records rather than rasterises. */
class FakeContext {
  fillStyle = "";
  font = "";
  textBaseline = "";
  imageSmoothingEnabled = true;
  readonly calls: Recorded[] = [];
  /** Every `ImageData` this context was asked to make. */
  readonly images: ImageData[] = [];

  fillRect(...args: unknown[]): void {
    this.calls.push({ what: `fillRect ${this.fillStyle}`, with: args });
  }

  fillText(...args: unknown[]): void {
    this.calls.push({ what: `fillText ${this.font}`, with: args });
  }

  createImageData(width: number, height: number): ImageData {
    const image: ImageData = {
      data: new Uint8ClampedArray(width * height * 4),
      width,
      height,
      colorSpace: "srgb",
    };
    this.images.push(image);
    return image;
  }

  putImageData(...args: unknown[]): void {
    this.calls.push({ what: "putImageData", with: args });
  }

  drawImage(...args: unknown[]): void {
    this.calls.push({ what: "drawImage", with: args });
  }
}

/** The contexts handed out, oldest first: the display one, then the scratch. */
let contexts: FakeContext[] = [];
/** Whether `getContext` answers at all. */
let available = true;

const original = HTMLCanvasElement.prototype.getContext;

beforeEach(() => {
  contexts = [];
  available = true;
  Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
    configurable: true,
    value: (): CanvasRenderingContext2D | null => {
      if (!available) {
        return null;
      }
      const context = new FakeContext();
      contexts.push(context);
      // The one assertion in this session's shipped-code tests, and it is about
      // an object this file created three lines above.
      return context as unknown as CanvasRenderingContext2D;
    },
  });
});

afterEach(() => {
  Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
    configurable: true,
    value: original,
  });
});

/** A canvas of a known size, with the fake context behind it. */
function surfaceOf(width = 1024, height = 640) {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const surface = canvasSurface(canvas);
  if (surface === null) {
    throw new Error("the fake context did not produce a surface");
  }
  return { canvas, surface };
}

describe("a surface over a canvas", () => {
  it("is the canvas's own size, and follows it", () => {
    const { canvas, surface } = surfaceOf(800, 400);
    expect([surface.width, surface.height]).toEqual([800, 400]);
    // The bitmap is resized by the panel when the element's box changes, and
    // the surface must read the new size rather than the one it started with.
    canvas.width = 1600;
    canvas.height = 800;
    expect([surface.width, surface.height]).toEqual([1600, 800]);
  });

  it("answers with nothing where there is no context to draw on", () => {
    available = false;
    expect(canvasSurface(document.createElement("canvas"))).toBeNull();
  });

  it("answers with nothing when the scratch canvas is the one refused", () => {
    // A browser that has run out of contexts does so part-way through: the
    // display canvas gets one and the scratch canvas does not, and a surface
    // that returned itself here would blit into nothing on every frame.
    let handed = 0;
    Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
      configurable: true,
      value: (): CanvasRenderingContext2D | null => {
        handed += 1;
        return handed > 1 ? null : (new FakeContext() as unknown as CanvasRenderingContext2D);
      },
    });
    expect(canvasSurface(document.createElement("canvas"))).toBeNull();
    expect(handed).toBe(2);
  });

  it("turns smoothing off, and turns it off again after a clear", () => {
    const { surface } = surfaceOf();
    const display = contexts[0];
    expect(display?.imageSmoothingEnabled).toBe(false);

    // A resized canvas resets its context state. The clear is where that is
    // noticed, so this is where it is put back — a smoothed blit is a grid of
    // 512 channels blurred into a wash.
    if (display !== undefined) {
      display.imageSmoothingEnabled = true;
    }
    surface.clear("#000000");
    expect(display?.imageSmoothingEnabled).toBe(false);
    expect(display?.calls.at(-1)?.what).toBe("fillRect #000000");
    expect(display?.calls.at(-1)?.with).toEqual([0, 0, 1024, 640]);
  });

  it("paints a rectangle and a label where it was asked to", () => {
    const { surface } = surfaceOf();
    surface.fill("#123456", 1, 2, 3, 4);
    surface.label("64", 5, 6, "#abcdef", 9);
    const display = contexts[0];
    expect(display?.calls[0]).toEqual({ what: "fillRect #123456", with: [1, 2, 3, 4] });
    expect(display?.calls[1]?.what).toContain("9px");
    expect(display?.calls[1]?.with).toEqual(["64", 5, 6]);
    // Text hangs from its top-left corner, because every row is placed by the
    // geometry and a baseline would put the first universe off the top.
    expect(display?.textBaseline).toBe("top");
  });
});

describe("the blit", () => {
  it("puts one pixel per channel on a scratch canvas and scales it up", () => {
    const { surface } = surfaceOf();
    const pixels = new Uint8ClampedArray(512 * 4 * 4);
    pixels[0] = 200;
    surface.blit(pixels, 512, 4, 34, 16, 900, 400);

    const scratch = contexts[1];
    const display = contexts[0];
    expect(scratch?.calls.at(-1)?.what).toBe("putImageData");
    expect(scratch?.images.at(-1)?.width).toBe(512);
    expect(scratch?.images.at(-1)?.height).toBe(4);
    expect(scratch?.images.at(-1)?.data[0]).toBe(200);

    // The whole scratch canvas into the grid rectangle: source in channels and
    // universes, destination in device pixels.
    expect(display?.calls.at(-1)).toEqual({
      what: "drawImage",
      with: [expect.anything() as unknown, 0, 0, 512, 4, 34, 16, 900, 400],
    });
  });

  it("keeps its ImageData between frames and makes a new one when the size changes", () => {
    const { surface } = surfaceOf();
    const pixels = new Uint8ClampedArray(512 * 2 * 4);
    surface.blit(pixels, 512, 2, 0, 0, 512, 200);
    surface.blit(pixels, 512, 2, 0, 0, 512, 200);
    surface.blit(pixels, 512, 2, 0, 0, 512, 200);

    const scratch = contexts[1];
    // Three frames, one buffer. 131 kB thirty times a second is four megabytes
    // a second for the collector to deal with in the middle of a show.
    expect(scratch?.images).toHaveLength(1);

    const wider = new Uint8ClampedArray(512 * 64 * 4);
    surface.blit(wider, 512, 64, 0, 0, 512, 640);
    expect(scratch?.images).toHaveLength(2);
    expect(scratch?.images.at(-1)?.height).toBe(64);
  });
});
