/**
 * What this browser draws 3D with — **S30b**: whether it has WebGL at all, and
 * whether that WebGL is done **in software**.
 *
 * A machine with no graphics hardware still runs WebGL: Chrome falls back to
 * SwiftShader, Linux without a driver to llvmpipe, Windows in a virtual
 * machine to the *Microsoft Basic Render Driver*. The picture is the same and
 * the price is not — every beam is rasterised on the processor, on threads
 * the console's own page and the daemon need. The 64-universe end-to-end test
 * measured it: the DMX Sheet beside a medium-detail viewer fell from 30 to
 * under 20 frames a second on SwiftShader, however little the viewer drew.
 *
 * So a software renderer **starts** the viewer at the lowest detail. It is a
 * starting point and not a limit: the operator's own choice, once made, is
 * remembered and wins (`./detail.ts`).
 */

/** What the browser has. */
export interface Graphics {
  /** Whether a WebGL 2 context can be had. */
  readonly webgl: boolean;
  /** Whether it is drawn in software. */
  readonly software: boolean;
  /** What the browser calls its renderer, for the readout; empty when it will not say. */
  readonly renderer: string;
}

/** Whether a renderer's name is one of the software rasterisers. */
export function isSoftwareRenderer(name: string): boolean {
  return /swiftshader|llvmpipe|softpipe|software|basic render/i.test(name);
}

/** Asks a fresh canvas, and lets its context go again. */
export function probeGraphics(make: () => HTMLCanvasElement = () => document.createElement("canvas")): Graphics {
  try {
    const gl = make().getContext("webgl2");
    if (gl === null) {
      return { webgl: false, software: false, renderer: "" };
    }
    // Chrome answers the unmasked name to `RENDERER` itself since 2024; the
    // extension is for the browsers that still mask it.
    const debug = gl.getExtension("WEBGL_debug_renderer_info");
    const named: unknown = gl.getParameter(debug === null ? gl.RENDERER : debug.UNMASKED_RENDERER_WEBGL);
    const renderer = typeof named === "string" ? named : "";
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return { webgl: true, software: isSoftwareRenderer(renderer), renderer };
  } catch {
    return { webgl: false, software: false, renderer: "" };
  }
}

let known: Graphics | null = null;

/** What this browser has, asked once — a context is not free to make. */
export function graphics(): Graphics {
  known ??= probeGraphics();
  return known;
}
