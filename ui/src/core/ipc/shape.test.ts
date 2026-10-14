/**
 * The readers everything else is built out of, including the depth limit.
 */

import { describe, expect, it } from "vitest";

import {
  MAX_NESTING_DEPTH,
  ProtocolFault,
  asArray,
  asBoolean,
  asBytes,
  asInteger,
  asJsonValue,
  asNullable,
  asNumber,
  asRecord,
  asString,
  asVariant,
  describe as describeValue,
  field,
  overArrayBuffer,
} from "./shape";

describe("readers", () => {
  it("answer with what they promised", () => {
    expect(asString("a", "p")).toBe("a");
    expect(asNumber(1.5, "p")).toBe(1.5);
    expect(asInteger(-3, "p")).toBe(-3);
    expect(asBoolean(false, "p")).toBe(false);
    expect(asArray([1], "p")).toEqual([1]);
    expect(asRecord({ a: 1 }, "p")).toEqual({ a: 1 });
    expect(asRecord(Object.create(null) as object, "p")).toEqual({});
    expect(asBytes(new Uint8Array([1]), "p")).toEqual(new Uint8Array([1]));
    expect(asNullable(null, "p", asString)).toBeNull();
    expect(asNullable("a", "p", asString)).toBe("a");
    expect(asVariant("b", "p", ["a", "b"])).toBe("b");
  });

  it("refuse what they were not given, with the path", () => {
    const refusals: [() => unknown, string][] = [
      [() => asString(1, "p"), "a string"],
      [() => asNumber("1", "p"), "a finite number"],
      [() => asNumber(Number.NaN, "p"), "a finite number"],
      [() => asNumber(Number.POSITIVE_INFINITY, "p"), "a finite number"],
      [() => asInteger(1.5, "p"), "a whole number"],
      [() => asBoolean(0, "p"), "a boolean"],
      [() => asArray({}, "p"), "an array"],
      [() => asRecord([], "p"), "an object"],
      [() => asRecord(new Date(), "p"), "an object"],
      [() => asRecord(new Uint8Array(), "p"), "an object"],
      [() => asBytes([1], "p"), "a byte string"],
      [() => asVariant("c", "p", ["a", "b"]), "one of a, b"],
      [() => asVariant(3, "p", ["a"]), "one of a"],
    ];
    for (const [run, expected] of refusals) {
      expect(run).toThrow(ProtocolFault);
      expect(run).toThrow(new RegExp(`^p: expected ${expected.replaceAll("|", "\\|")}`));
    }
  });

  it("tell an absent member from one that is there and null", () => {
    expect(field({ a: null }, "a")).toBeNull();
    expect(field({ a: null }, "b")).toBeUndefined();
  });

  it("say what a value was, for the message", () => {
    expect(describeValue(null)).toBe("null");
    expect(describeValue([1])).toBe("an array");
    expect(describeValue(new Uint8Array())).toBe("a byte string");
    expect(describeValue("a")).toBe("string");
    expect(describeValue(undefined)).toBe("undefined");
  });
});

describe("a schema-free value", () => {
  it("takes anything JSON can hold", () => {
    const value = { a: [1, "two", null, true, { b: 1.5 }], c: {} };
    expect(asJsonValue(value, "p")).toEqual(value);
  });

  it("refuses what JSON cannot", () => {
    expect(() => asJsonValue(undefined, "p")).toThrow(ProtocolFault);
    expect(() => asJsonValue(new Uint8Array(), "p")).toThrow(ProtocolFault);
    expect(() => asJsonValue(Number.NaN, "p")).toThrow(ProtocolFault);
    expect(() => asJsonValue({ a: undefined }, "p.a")).toThrow(ProtocolFault);
  });

  it("stops at the nesting limit rather than walking whatever it was given", () => {
    // The same limit `prism-ipc`'s `scan.rs` applies, against the same attack:
    // `JsonValue` is recursive and MessagePack has no limit of its own.
    const deep = (levels: number) => {
      let value: unknown = 1;
      for (let level = 0; level < levels; level += 1) {
        value = [value];
      }
      return value;
    };
    expect(() => asJsonValue(deep(MAX_NESTING_DEPTH), "p")).not.toThrow();
    expect(() => asJsonValue(deep(MAX_NESTING_DEPTH + 2), "p")).toThrow(
      /nested at most 128 deep/,
    );
  });

  it("names the element that was wrong, however deep", () => {
    try {
      asJsonValue({ ops: [{ value: undefined }] }, "Delta");
    } catch (cause) {
      expect(cause).toBeInstanceOf(ProtocolFault);
      expect(cause instanceof ProtocolFault ? cause.path : "").toBe("Delta.ops[0].value");
    }
  });
});

describe("payloads", () => {
  it("keep the same bytes over an ArrayBuffer", () => {
    const source = new Uint8Array([1, 2, 3, 4, 5]);
    const view = source.subarray(1, 4);
    const payload = overArrayBuffer(view);
    expect(Array.from(payload)).toEqual([2, 3, 4]);
    // A view rather than a copy: nothing on the command path allocates twice.
    expect(payload.buffer).toBe(source.buffer);
  });
});
