/**
 * A GDTF device as the visualiser reads it — **S30b** — out of the show the
 * daemon served (`crates/prismd/tests/ui_viewer.rs`): the head's geometry tree,
 * its channels' functions and its gobo wheel, as `prism-core` read them from a
 * `.gdtf` built byte by byte.
 */

import { describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";
import { finalShow } from "../testing/viewer-recording";
import { deviceOf, isUnder } from "./device";
import { rigOf } from "./rig";

function head(): NonNullable<ReturnType<typeof deviceOf>> {
  const device = rigOf(finalShow()).find((fixture) => fixture.id === 1)?.device ?? null;
  if (device === null) {
    throw new Error("the recorded head carries no device");
  }
  return device;
}

describe("the recorded head", () => {
  it("is a body, a yoke, a head and a beam, each hanging from the one before", () => {
    const { nodes, guid } = head();
    expect(guid).toBe("5A1E0000-0000-4000-8000-0000000030AA");
    expect(nodes.map((node) => [node.name, node.kind, node.parent])).toEqual([
      ["Body", "Geometry", null],
      ["Yoke", "Axis", 0],
      ["Head", "Axis", 1],
      ["Beam", "Beam", 2],
    ]);
    expect(nodes[0]?.model).toBe("viewer-head");
    expect(nodes[3]?.beam).toMatchObject({ beamAngle: 18, kelvin: 6500 });
    // The beam 0.2 m below the head's origin, in show axes: down is −y.
    expect(nodes[3]?.position.y).toBeCloseTo(-0.2, 9);
  });

  it("has every channel with its functions, sets and wheel", () => {
    const { channels, wheels } = head();
    expect(channels.map((channel) => [channel.attribute, channel.offset, channel.fine, channel.geometry])).toEqual([
      ["Pan", 0, 1, "Yoke"],
      ["Tilt", 2, 3, "Head"],
      ["Dimmer", 4, null, "Beam"],
      ["Zoom", 5, null, "Beam"],
      ["Gobo1", 6, null, "Head"],
      ["Shutter1", 7, null, "Beam"],
    ]);
    const gobo = channels[4]?.functions[0];
    expect(gobo?.wheel).toBe("Gobo1");
    expect(gobo?.sets.map((set) => [set.name, set.slot])).toEqual([
      ["Open", 1],
      ["Dots", 2],
      ["Star", 3],
    ]);
    const shutter = channels[5]?.functions ?? [];
    expect(shutter.map((fn) => fn.attribute)).toEqual(["Shutter1", "Shutter1Strobe"]);
    // The first function ends where the strobe starts: 64/1 on sixteen bits.
    expect(shutter[0]?.to).toBe(Math.floor((64 * 65535) / 255) - 1);
    expect(shutter[1]).toMatchObject({ physicalFrom: 1, physicalTo: 20 });
    expect(wheels.get("Gobo1")?.slots.map((slot) => slot.media)).toEqual([null, "gobo-dots", "gobo-star"]);
  });
});

describe("deviceOf", () => {
  it("is null for a profile with no geometry tree", () => {
    expect(deviceOf(null)).toBeNull();
    expect(deviceOf("physical" as JsonValue)).toBeNull();
    expect(deviceOf({ beams: [] })).toBeNull();
  });

  it("fills in what a node does not say and refuses a parent that is not before it", () => {
    const device = deviceOf({
      geometries: [
        { name: "A", parent: 1 },
        { parent: 0, beam: { beamAngle: 25 } },
        "not a node",
      ],
      wheels: [{ slots: [] }, { name: "Prism", slots: [{ facets: [{ x: 0.2 }, "no"] }] }],
      channels: [{ functions: [{ sets: [{}, 3] }] }],
    });
    expect(device?.nodes).toHaveLength(2);
    expect(device?.nodes[0]?.parent).toBeNull();
    expect(device?.nodes[1]).toMatchObject({ name: "Geometry 2", kind: "Geometry", parent: 0 });
    expect(device?.nodes[1]?.beam).toMatchObject({ type: "Wash", beamAngle: 25, ratio: 1 });
    expect(device?.nodes[0]?.zAxis).toEqual({ x: 0, y: 0, z: 1 });
    // A wheel with no name is no wheel; a facet with no y is at nought.
    expect([...(device?.wheels.keys() ?? [])]).toEqual(["Prism"]);
    expect(device?.wheels.get("Prism")?.slots[0]?.facets).toEqual([{ x: 0.2, y: 0 }]);
    expect(device?.channels[0]?.functions[0]).toMatchObject({ from: 0, to: 65535, physicalFrom: 0, physicalTo: 1, modeMaster: null });
    expect(device?.channels[0]?.functions[0]?.sets).toEqual([{ name: "", from: 0, to: 65535, slot: null }]);
  });
});

describe("isUnder", () => {
  it("answers whether a node is an ancestor or hangs under it", () => {
    const { nodes } = head();
    expect(isUnder(nodes, 3, 1)).toBe(true);
    expect(isUnder(nodes, 3, 3)).toBe(true);
    expect(isUnder(nodes, 1, 3)).toBe(false);
    expect(isUnder(nodes, 0, 2)).toBe(false);
  });
});
