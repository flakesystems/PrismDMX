/**
 * The show's stage marks in the 3D picture - **B70**.
 *
 * A cross on the spot, a stem down to the floor when the mark is above it, and
 * its name on a sprite that always faces the camera. The cross is made of thin
 * boxes and not of lines: a line is one pixel wide on every screen and a mark
 * has to be seen across a stage. None of it is tone-mapped, so it is the colour
 * the 2D picture draws. Where the calibration
 * panel lists the marks with numbers, this is the quickest check of a result:
 * the beam of a calibrated head lands on the cross.
 *
 * Built again only when the marks change (the array is the show's, so identity
 * is the change test), and never touched by a frame: the stage does not walk its
 * scene (`matrixWorldAutoUpdate` is off), so the world matrices are brought up
 * to date once, here.
 */

import {
  BoxGeometry,
  CanvasTexture,
  Group,
  Mesh,
  MeshBasicMaterial,
  Sprite,
  SpriteMaterial,
} from "three";

import type { Mark } from "../../bindings";
import { MARK_ARM } from "../scene";
import { markLabel } from "../marks";
import { showToThree } from "./coords";

/** Amber, as the 2D picture draws them. */
const COLOUR = 0xe0a030;

/** How thick a cross arm and a stem are, in metres. */
const THICKNESS = 0.05;

/** How tall a label is in the world, in metres. */
const LABEL_HEIGHT = 0.22;

/** The marks as three.js objects. */
export class MarkLayer {
  readonly group = new Group();
  #paint = new MeshBasicMaterial({ color: COLOUR, toneMapped: false });
  #arm = new BoxGeometry(MARK_ARM * 2, THICKNESS / 2, THICKNESS);
  #stem = new BoxGeometry(THICKNESS, 1, THICKNESS);
  #held: readonly Mark[] = [];

  constructor() {
    this.group.matrixAutoUpdate = false;
  }

  /** Replaces what is drawn, if the marks are not the ones already drawn. */
  set(marks: readonly Mark[]): void {
    if (marks === this.#held) {
      return;
    }
    this.#held = marks;
    this.#clear();
    for (const mark of marks) {
      const [x, y, z] = showToThree(mark.position);
      const across = new Mesh(this.#arm, this.#paint);
      across.position.set(x, y, z);
      const depth = new Mesh(this.#arm, this.#paint);
      depth.position.set(x, y, z);
      depth.rotation.y = Math.PI / 2;
      for (const arm of [across, depth]) {
        arm.updateMatrix();
        arm.matrixAutoUpdate = false;
        this.group.add(arm);
      }
      if (y > 0.01) {
        const stem = new Mesh(this.#stem, this.#paint);
        stem.position.set(x, y / 2, z);
        stem.scale.y = y;
        stem.updateMatrix();
        stem.matrixAutoUpdate = false;
        this.group.add(stem);
      }
      this.group.add(label(markLabel(mark), x, y + LABEL_HEIGHT, z));
    }
    this.group.updateMatrixWorld(true);
  }

  /** How many marks are drawn. */
  get count(): number {
    return this.#held.length;
  }

  #clear(): void {
    for (const child of [...this.group.children]) {
      this.group.remove(child);
      // The boxes and their paint are shared and outlive a rebuild.
      if (child instanceof Sprite) {
        child.material.map?.dispose();
        child.material.dispose();
      }
    }
  }

  dispose(): void {
    this.#clear();
    this.#paint.dispose();
    this.#arm.dispose();
    this.#stem.dispose();
  }
}

/** A name on a sprite. */
function label(text: string, x: number, y: number, z: number): Sprite {
  const canvas = document.createElement("canvas");
  canvas.width = 256;
  canvas.height = 64;
  const context = canvas.getContext("2d");
  if (context !== null) {
    context.font = "600 34px system-ui, sans-serif";
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillStyle = "#e0a030";
    context.fillText(text, 128, 32, 248);
  }
  const material = new SpriteMaterial({
    map: new CanvasTexture(canvas),
    transparent: true,
    depthTest: false,
    toneMapped: false,
  });
  const sprite = new Sprite(material);
  sprite.position.set(x, y, z);
  sprite.scale.set(LABEL_HEIGHT * 4, LABEL_HEIGHT, 1);
  sprite.renderOrder = 10;
  return sprite;
}
