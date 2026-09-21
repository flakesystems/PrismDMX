/**
 * The camera: where it stands, what it sees, and that every gesture moves it
 * without anything else — it is client-local, and nothing here is a command.
 */

import { describe, expect, it } from "vitest";

import {
  FURTHEST,
  NEAREST,
  STEEPEST,
  clipToNear,
  depthOf,
  eyeOf,
  frameAll,
  lensOf,
  look,
  newCamera,
  orbit,
  project,
  slide,
  zoom,
} from "./camera";
import { v3 } from "./space";

describe("a camera", () => {
  it("at heading nought stands downstage and sees +x on its right", () => {
    const camera = newCamera();
    look(camera, "front");
    const eye = eyeOf(camera);
    expect(eye.z).toBeLessThan(camera.target.z);
    const lens = lensOf(camera, 800, 600);
    const out = [0, 0];
    expect(project(lens, v3(3, camera.target.y, camera.target.z), out)).toBe(true);
    expect(out[0]).toBeGreaterThan(400);
    // Higher is further up the screen.
    expect(project(lens, v3(0, camera.target.y + 3, camera.target.z), out)).toBe(true);
    expect(out[1]).toBeLessThan(300);
    // The target is the middle of the picture.
    expect(project(lens, camera.target, out)).toBe(true);
    expect(out[0]).toBeCloseTo(400, 6);
    expect(out[1]).toBeCloseTo(300, 6);
  });

  it("does not draw what is behind it, and cuts a line where it passes the near plane", () => {
    const camera = newCamera();
    look(camera, "front");
    const lens = lensOf(camera, 800, 600);
    const behind = eyeOf({ ...camera, distance: camera.distance + 5 });
    expect(project(lens, behind, [0, 0])).toBe(false);
    const cut = clipToNear(lens, behind, camera.target);
    expect(depthOf(lens, cut)).toBeGreaterThan(0.05);
    expect(depthOf(lens, cut)).toBeLessThan(0.06);
  });

  it("counts every change, so a loop can tell it moved", () => {
    const camera = newCamera();
    const before = camera.version;
    look(camera, "top");
    orbit(camera, 10, 5);
    slide(camera, 30, -20, 600);
    zoom(camera, -1);
    frameAll(camera, []);
    expect(camera.version).toBe(before + 5);
  });

  it("orbits within the sky and wraps its heading", () => {
    const camera = newCamera();
    orbit(camera, 0, 10_000);
    expect(camera.elevation).toBe(STEEPEST);
    orbit(camera, 0, -100_000);
    expect(camera.elevation).toBe(-STEEPEST);
    orbit(camera, 500, 0);
    expect(camera.heading).toBeGreaterThan(-180);
    expect(camera.heading).toBeLessThanOrEqual(180);
  });

  it("zooms within its limits and slides its target across the screen", () => {
    const camera = newCamera();
    zoom(camera, -1000);
    expect(camera.distance).toBe(NEAREST);
    zoom(camera, 1000);
    expect(camera.distance).toBe(FURTHEST);
    const before = camera.target;
    look(camera, "front");
    slide(camera, -100, 0, 600);
    expect(camera.target.x).toBeGreaterThan(before.x);
  });

  it("frames a rig so every fixture is in the picture", () => {
    const camera = newCamera();
    const rig = [v3(-10, 6, 0), v3(10, 6, 0), v3(0, 0.2, 8)];
    frameAll(camera, rig);
    const lens = lensOf(camera, 800, 600);
    const out = [0, 0];
    for (const point of rig) {
      expect(project(lens, point, out)).toBe(true);
      expect(out[0]).toBeGreaterThanOrEqual(0);
      expect(out[0]).toBeLessThanOrEqual(800);
      expect(out[1]).toBeGreaterThanOrEqual(0);
      expect(out[1]).toBeLessThanOrEqual(600);
    }
  });
});
