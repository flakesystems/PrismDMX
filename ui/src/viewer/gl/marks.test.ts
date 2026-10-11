/**
 * The stage marks as three.js objects - B70. Built again only when the marks
 * change, drawn as lines and one sprite each.
 */

import { Mesh, Sprite } from "three";
import { describe, expect, it } from "vitest";

import type { Mark } from "../../bindings";
import { MarkLayer } from "./marks";

const mark = (id: number, name: string, x: number, y: number, z: number): Mark => ({
  id,
  name,
  position: { x, y, z },
});

function boxes(layer: MarkLayer): Mesh[] {
  return layer.group.children.filter((child): child is Mesh => child instanceof Mesh);
}

describe("the mark layer", () => {
  it("is empty until it is given marks", () => {
    const layer = new MarkLayer();
    expect(layer.count).toBe(0);
    expect(layer.group.children).toHaveLength(0);
  });

  it("draws a cross and a label for a mark on the floor, in three.js axes", () => {
    const layer = new MarkLayer();
    layer.set([mark(1, "DS", 1, 0, -3)]);
    const sprites = layer.group.children.filter((child) => child instanceof Sprite);
    expect(sprites).toHaveLength(1);
    // Show space's z flips: upstage is three's -z, so -3 is +3.
    expect(sprites[0]?.position.z).toBe(3);
    // Two arms and no stem.
    expect(boxes(layer)).toHaveLength(2);
  });

  it("adds a stem for a mark above the floor", () => {
    const layer = new MarkLayer();
    layer.set([mark(1, "", 0, 0.8, 0)]);
    const meshes = boxes(layer);
    expect(meshes).toHaveLength(3);
    // The stem stands from the floor to the mark, half its height up.
    expect(meshes[2]?.position.y).toBeCloseTo(0.4, 6);
    expect(meshes[2]?.scale.y).toBeCloseTo(0.8, 6);
  });

  it("is not rebuilt for the marks it already draws, and is for others", () => {
    const layer = new MarkLayer();
    const marks = [mark(1, "a", 0, 0, 0)];
    layer.set(marks);
    const first = boxes(layer)[0];
    layer.set(marks);
    expect(boxes(layer)[0]).toBe(first);
    layer.set([mark(1, "a", 0, 0, 0), mark(2, "b", 1, 0, 0)]);
    expect(boxes(layer)[0]).not.toBe(first);
    expect(boxes(layer)).toHaveLength(4);
    expect(layer.count).toBe(2);
    layer.set([]);
    expect(layer.group.children).toHaveLength(0);
    layer.dispose();
  });
});
