/**
 * What the viewer draws on — a 2D canvas behind an interface, S24's pattern.
 *
 * # Why a 2D canvas and not WebGL
 *
 * `ARCHITECTURE_SPEC.md` named *react-three-fiber* for this in its first
 * draft, and S30 decided against it, for three reasons that are this
 * project's rather than a taste:
 *
 * - **It has to work on every machine the desk runs on.** A school's ageing
 *   laptop, a Raspberry Pi's webview (D10), a remote desktop, a CI runner with
 *   no GPU: WebGL is missing or software-emulated on each of them, and a window
 *   that says *no WebGL* is a window that failed. A 2D context is there on
 *   every one of them.
 * - **It has to be testable without a rasteriser**, like every other canvas in
 *   this interface. `jsdom` has no WebGL and no 2D context either; a surface
 *   behind an interface is what lets `scene.test.ts` assert what was drawn,
 *   the way `telemetry/painter.test.ts` does.
 * - **A rig is small.** Boxes, cones and a floor — a few thousand polygons at
 *   four hundred fixtures — is well inside what a 2D canvas paints in a frame,
 *   and the end-to-end suite measures that it does rather than asserting it.
 *
 * What it gives up is a depth buffer and the fixtures' own 3D models. Bodies
 * are sorted far to near, beams are added light over them, and a GDTF model is
 * S30's open item rather than a missing feature: the bytes do not reach a
 * client yet (`PROGRESS.md` §5), and when they do this interface is the seam a
 * WebGL surface would be put behind.
 */

/** How a shape is combined with what is under it. */
export type Blend = "over" | "add";

/** Where the 3D view is drawn. Implemented over a 2D context, and over a fake. */
export interface ViewSurface {
  /** Device pixels across. */
  readonly width: number;
  /** Device pixels down. */
  readonly height: number;
  /** Paints the whole surface one colour. */
  clear(colour: string): void;
  /**
   * Fills a polygon given as `x0, y0, x1, y1, …` over `count` points, and
   * outlines it when `outline` is given.
   */
  polygon(
    points: ArrayLike<number>,
    count: number,
    fill: string,
    alpha: number,
    blend: Blend,
    outline?: string,
  ): void;
  /**
   * Fills a polygon with light fading from `from` to `to` — a beam. `rgb` is
   * `r, g, b` at `0..=255`. Always added to what is under it.
   */
  beam(
    points: ArrayLike<number>,
    count: number,
    fromX: number,
    fromY: number,
    toX: number,
    toY: number,
    rgb: string,
    alphaFrom: number,
    alphaTo: number,
  ): void;
  /** A line. */
  line(x1: number, y1: number, x2: number, y2: number, colour: string, width: number): void;
  /** Text centred on `x`, with its baseline at `y`. */
  label(text: string, x: number, y: number, colour: string, size: number): void;
}

/**
 * A {@link ViewSurface} over a real canvas, or `null` where there is no 2D
 * context — which is every `jsdom` test. `null` rather than a throw for
 * `telemetry/painter.ts`'s reason: a view that cannot draw must not take the
 * command line with it.
 */
export function canvasViewSurface(canvas: HTMLCanvasElement): ViewSurface | null {
  const context = canvas.getContext("2d", { alpha: false });
  if (context === null) {
    return null;
  }
  const trace = (points: ArrayLike<number>, count: number): void => {
    context.beginPath();
    for (let index = 0; index < count; index += 1) {
      const x = points[index * 2] ?? 0;
      const y = points[index * 2 + 1] ?? 0;
      if (index === 0) {
        context.moveTo(x, y);
      } else {
        context.lineTo(x, y);
      }
    }
    context.closePath();
  };
  return {
    get width() {
      return canvas.width;
    },
    get height() {
      return canvas.height;
    },
    clear: (colour) => {
      context.globalCompositeOperation = "source-over";
      context.globalAlpha = 1;
      context.fillStyle = colour;
      context.fillRect(0, 0, canvas.width, canvas.height);
    },
    polygon: (points, count, fill, alpha, blend, outline) => {
      if (count < 3) {
        return;
      }
      context.globalCompositeOperation = blend === "add" ? "lighter" : "source-over";
      context.globalAlpha = alpha;
      trace(points, count);
      context.fillStyle = fill;
      context.fill();
      if (outline !== undefined) {
        context.globalAlpha = 1;
        context.strokeStyle = outline;
        context.lineWidth = Math.max(1, canvas.width / 900);
        context.stroke();
      }
    },
    beam: (points, count, fromX, fromY, toX, toY, rgb, alphaFrom, alphaTo) => {
      if (count < 3) {
        return;
      }
      const gradient = context.createLinearGradient(fromX, fromY, toX, toY);
      gradient.addColorStop(0, `rgba(${rgb},${String(alphaFrom)})`);
      gradient.addColorStop(1, `rgba(${rgb},${String(alphaTo)})`);
      context.globalCompositeOperation = "lighter";
      context.globalAlpha = 1;
      trace(points, count);
      context.fillStyle = gradient;
      context.fill();
    },
    line: (x1, y1, x2, y2, colour, width) => {
      context.globalCompositeOperation = "source-over";
      context.globalAlpha = 1;
      context.strokeStyle = colour;
      context.lineWidth = width;
      context.beginPath();
      context.moveTo(x1, y1);
      context.lineTo(x2, y2);
      context.stroke();
    },
    label: (text, x, y, colour, size) => {
      context.globalCompositeOperation = "source-over";
      context.globalAlpha = 1;
      context.fillStyle = colour;
      context.font = `${String(size)}px ui-monospace, "Cascadia Mono", Menlo, monospace`;
      context.textAlign = "center";
      context.textBaseline = "alphabetic";
      context.fillText(text, x, y);
    },
  };
}
