/**
 * The shape of a beam's light — **S30b**: every gobo in it, the iris, the four
 * framing blades and the shaper, and how soft the whole is, drawn into one
 * square picture.
 *
 * The same picture is sampled by the beam's volume in the haze **and** thrown
 * onto the floor by the beam's spot light, so the shaft of light and the pool
 * it lands in always agree — a blade that cuts one cuts the other.
 *
 * Drawn on a 2D canvas and redrawn only when something in it changes: a gobo
 * turning continuously is a rotation of the picture, which the beam applies
 * itself, so a spinning gobo costs no redraw at all.
 *
 * # Gobo pictures
 *
 * GDTF ships a wheel slot's picture as a PNG whose **white is light and black
 * is metal**, round, on a transparent square (a Robe Robin T1's, read on
 * 2026-09-21). An Open Fixture Library gobo has no picture and gets one of six
 * built-in patterns (`"@1"`…`"@6"`), so an operator can still see that a gobo
 * is in and which.
 */

import { CanvasTexture, LinearFilter } from "three";

import type { BeamState } from "../state";

/** A picture loaded for drawing. */
export type Picture = CanvasImageSource & { readonly width: number; readonly height: number };

/** Something that gives a gobo's picture, or `null` while it has none (yet). */
export type Pictures = (media: string) => Picture | null;

/** The canvases one mask is drawn with. */
interface Canvases {
  readonly canvas: HTMLCanvasElement;
  readonly texture: CanvasTexture;
  /** The sharp picture, before focus and frost soften it. */
  readonly scratch: HTMLCanvasElement;
  /** One gobo at a time, before it is multiplied into the light. */
  readonly layer: HTMLCanvasElement;
}

function canvasOf(size: number): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  return canvas;
}

function canvasesOf(size: number): Canvases {
  const canvas = canvasOf(size);
  const texture = new CanvasTexture(canvas);
  // Row 0 is the top of the picture, as it is for the floor's atlas
  // (`./floor.ts`), so the haze and the pool show the gobo the same way up.
  texture.flipY = false;
  texture.minFilter = LinearFilter;
  texture.magFilter = LinearFilter;
  texture.generateMipmaps = false;
  return { canvas, texture, scratch: canvasOf(size), layer: canvasOf(size) };
}

/**
 * The open beam's mask, one per size for every beam there is. Most of a rig
 * is open most of the time — a wash, a PAR, a head with nothing in — and a
 * canvas and a texture of its own for each would be a gigabyte of nothing at
 * the highest detail on a big rig.
 */
const OPEN = new Map<number, Canvases>();

function openMask(size: number): Canvases {
  let held = OPEN.get(size);
  if (held === undefined) {
    held = canvasesOf(size);
    paint(held, fullBeam(), () => null);
    OPEN.set(size, held);
  }
  return held;
}

/**
 * A beam's mask and what it was drawn from: the shared open one until
 * something is put in the beam, and a picture of its own from then on.
 */
export class BeamMask {
  readonly #size: number;
  #own: Canvases | null = null;
  #open = true;
  #drawn: string;
  /** Moves every time the picture changes, so a copy of it knows it is stale. */
  version = 0;

  constructor(size: number) {
    this.#size = size;
    this.#drawn = OPEN_KEY;
  }

  /** The picture as it stands. */
  get canvas(): HTMLCanvasElement {
    return this.#open || this.#own === null ? openMask(this.#size).canvas : this.#own.canvas;
  }

  /** The picture as a texture for the volume. */
  get texture(): CanvasTexture {
    return this.#open || this.#own === null ? openMask(this.#size).texture : this.#own.texture;
  }

  /**
   * Redraws the mask for `beam` if anything in it changed. Answers whether it
   * did. Continuous gobo rotation is **not** in the mask — see the module
   * documentation.
   */
  update(beam: BeamState, pictures: Pictures): boolean {
    const key = maskKey(beam, pictures);
    if (key === this.#drawn) {
      return false;
    }
    this.#drawn = key;
    this.version += 1;
    this.#open = key === OPEN_KEY;
    if (!this.#open) {
      this.#own ??= canvasesOf(this.#size);
      paint(this.#own, beam, pictures);
      this.#own.texture.needsUpdate = true;
    }
    return true;
  }

  /** Frees its own texture; the open one is everybody's. */
  dispose(): void {
    this.#own?.texture.dispose();
  }
}

function paint(into: Canvases, beam: BeamState, pictures: Pictures): void {
  const size = into.canvas.width;
  const sharp = into.scratch.getContext("2d");
  const out = into.canvas.getContext("2d");
  const layer = into.layer.getContext("2d");
  if (sharp === null || out === null || layer === null) {
    return;
  }
  const centre = size / 2;
  const radius = size / 2 - 2;
  sharp.setTransform(1, 0, 0, 1, 0, 0);
  sharp.globalCompositeOperation = "source-over";
  sharp.fillStyle = "#000";
  sharp.fillRect(0, 0, size, size);
  // The light: the full circle, then everything that takes some of it away.
  sharp.fillStyle = "#fff";
  sharp.beginPath();
  sharp.arc(centre, centre, radius, 0, Math.PI * 2);
  sharp.fill();

  // Each gobo is drawn whole on its own layer — over black, so outside the
  // picture is metal too — and the layer multiplied into the light: two
  // gobos in one beam let through only what both let through.
  for (const gobo of beam.gobos) {
    layer.setTransform(1, 0, 0, 1, 0, 0);
    layer.globalCompositeOperation = "source-over";
    layer.fillStyle = "#000";
    layer.fillRect(0, 0, size, size);
    layer.translate(centre, centre);
    layer.rotate((gobo.rotation * Math.PI) / 180);
    const picture = gobo.media.startsWith("@") ? null : pictures(gobo.media);
    if (picture !== null) {
      // White is light, black is metal, transparent is outside the gobo.
      layer.drawImage(picture, -radius, -radius, radius * 2, radius * 2);
    } else {
      builtIn(layer, gobo.media, radius);
    }
    sharp.globalCompositeOperation = "multiply";
    sharp.drawImage(into.layer, 0, 0);
  }

  // The iris: everything outside its circle is dark.
  if (beam.iris < 0.999) {
    sharp.globalCompositeOperation = "destination-in";
    sharp.fillStyle = "#fff";
    sharp.beginPath();
    sharp.arc(centre, centre, radius * beam.iris, 0, Math.PI * 2);
    sharp.fill();
  }

  // The blades: four, a quarter turn apart, each a straight edge from its
  // `a` end to its `b` end, turned by its own rotation and the shaper's.
  sharp.globalCompositeOperation = "source-over";
  sharp.fillStyle = "#000";
  beam.blades.forEach((blade, index) => {
    if (blade.a <= 0.001 && blade.b <= 0.001) {
      return;
    }
    sharp.save();
    sharp.translate(centre, centre);
    sharp.rotate(((index * 90 + beam.shaper + blade.rotation) * Math.PI) / 180);
    // In this frame the blade comes in from +x: its edge runs from
    // (r − a·2r, −r) to (r − b·2r, +r), and everything beyond it is metal.
    const a = radius - blade.a * 2 * radius;
    const b = radius - blade.b * 2 * radius;
    sharp.beginPath();
    sharp.moveTo(a, -radius * 1.5);
    sharp.lineTo(b, radius * 1.5);
    sharp.lineTo(radius * 1.5, radius * 1.5);
    sharp.lineTo(radius * 1.5, -radius * 1.5);
    sharp.closePath();
    sharp.fill();
    sharp.restore();
  });

  // Soft by focus and by frost, drawn from the sharp picture.
  const soft = Math.min(1, beam.blur * 0.7 + beam.frost);
  out.setTransform(1, 0, 0, 1, 0, 0);
  out.globalCompositeOperation = "source-over";
  out.filter = soft > 0.01 ? `blur(${(soft * size * 0.06).toFixed(2)}px)` : "none";
  out.fillStyle = "#000";
  out.fillRect(0, 0, size, size);
  out.drawImage(into.scratch, 0, 0);
  out.filter = "none";
  // A frosted beam spreads its light: the edge is lifted rather than cut.
  if (beam.frost > 0.01) {
    out.globalCompositeOperation = "lighter";
    const glow = out.createRadialGradient(centre, centre, 0, centre, centre, radius);
    glow.addColorStop(0, `rgba(255,255,255,${(beam.frost * 0.35).toFixed(3)})`);
    glow.addColorStop(1, "rgba(255,255,255,0)");
    out.fillStyle = glow;
    out.fillRect(0, 0, size, size);
  }
}

/** An open beam: nothing in it. */
function fullBeam(): BeamState {
  return {
    intensity: 1,
    strobe: "none",
    hz: 0,
    color: [1, 1, 1],
    angle: null,
    iris: 1,
    blur: 0,
    frost: 0,
    gobos: [],
    prism: null,
    blades: [],
    shaper: 0,
  };
}

/** The key of an open beam: nothing in it. */
const OPEN_KEY = maskKey(fullBeam(), () => null);

/** Everything a mask is drawn from, as a string — and whether each picture has arrived. */
export function maskKey(beam: BeamState, pictures: Pictures): string {
  const round = (value: number): string => value.toFixed(3);
  const gobos = beam.gobos
    .map((gobo) => `${gobo.media}:${pictures(gobo.media) === null ? "-" : "+"}:${round(gobo.rotation)}`)
    .join(",");
  const blades = beam.blades.map((blade) => `${round(blade.a)}/${round(blade.b)}/${round(blade.rotation)}`).join(",");
  return [gobos, blades, round(beam.shaper), round(beam.iris), round(beam.blur), round(beam.frost)].join("|");
}

/** One of the six built-in gobos, centred on the origin. */
function builtIn(context: CanvasRenderingContext2D, media: string, radius: number): void {
  context.fillStyle = "#000";
  context.fillRect(-radius, -radius, radius * 2, radius * 2);
  context.fillStyle = "#fff";
  const pattern = Number(media.slice(1)) || 1;
  switch (pattern) {
    case 1: // dots
      for (let ring = 0; ring < 3; ring += 1) {
        const count = ring === 0 ? 1 : ring * 6;
        for (let dot = 0; dot < count; dot += 1) {
          const angle = (dot / count) * Math.PI * 2;
          context.beginPath();
          context.arc(Math.cos(angle) * ring * radius * 0.3, Math.sin(angle) * ring * radius * 0.3, radius * 0.1, 0, Math.PI * 2);
          context.fill();
        }
      }
      break;
    case 2: // breakup
      for (let blob = 0; blob < 14; blob += 1) {
        const angle = blob * 2.39996;
        const distance = Math.sqrt(blob / 14) * radius * 0.8;
        context.beginPath();
        context.ellipse(Math.cos(angle) * distance, Math.sin(angle) * distance, radius * 0.16, radius * 0.09, angle, 0, Math.PI * 2);
        context.fill();
      }
      break;
    case 3: // star
      context.beginPath();
      for (let point = 0; point < 10; point += 1) {
        const angle = (point / 10) * Math.PI * 2 - Math.PI / 2;
        const reach = point % 2 === 0 ? radius * 0.9 : radius * 0.38;
        context.lineTo(Math.cos(angle) * reach, Math.sin(angle) * reach);
      }
      context.closePath();
      context.fill();
      break;
    case 4: // bars
      for (let bar = -3; bar <= 3; bar += 2) {
        context.fillRect(bar * radius * 0.2 - radius * 0.08, -radius, radius * 0.16, radius * 2);
      }
      break;
    case 5: // ring
      context.beginPath();
      context.arc(0, 0, radius * 0.85, 0, Math.PI * 2);
      context.arc(0, 0, radius * 0.55, 0, Math.PI * 2, true);
      context.fill();
      break;
    default: // tri-spot
      for (let spot = 0; spot < 3; spot += 1) {
        const angle = (spot / 3) * Math.PI * 2;
        context.beginPath();
        context.arc(Math.cos(angle) * radius * 0.45, Math.sin(angle) * radius * 0.45, radius * 0.3, 0, Math.PI * 2);
        context.fill();
      }
  }
}
