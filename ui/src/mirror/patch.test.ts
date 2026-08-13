/**
 * The applier against RFC 6902 — and against `prism_core::mirror`'s own tests.
 *
 * Every case in the first half is the TypeScript of a test in
 * `crates/prism-core/src/mirror.rs`, deliberately: the two appliers have to
 * agree about the awkward parts (`-`, `01`, a `move` into its own source,
 * `replace` needing something to replace) or the promise in
 * `docs/IPC_PROTOCOL.md` §6 is only true for the easy deltas. The second half
 * is about immutability, which the Rust side does not need and this one does.
 */

import { describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";
import { MirrorFault, applyOp, applyOps, equals, getAt, indexOf, isInside, tokens } from "./patch";

/** The document `prism_core::mirror`'s tests use, to the character. */
function document(): JsonValue {
  return {
    fixtures: { "1": { name: "Front" } },
    cues: [1, 2],
  };
}

/** The fault an operation threw, as a value to compare. */
function faultOf(run: () => unknown) {
  try {
    run();
  } catch (cause) {
    if (cause instanceof MirrorFault) {
      return cause.detail;
    }
    throw cause;
  }
  throw new Error("the operation was expected to fail and did not");
}

describe("pointers", () => {
  it("splits and unescapes reference tokens", () => {
    expect(tokens("")).toEqual([]);
    expect(tokens("/a/b")).toEqual(["a", "b"]);
    // `~1` before `~0`, or an escaped `~1` becomes a separator.
    expect(tokens("/a~1b~0c")).toEqual(["a/b~c"]);
    expect(tokens("/")).toEqual([""]);
  });

  it("refuses something that is not a pointer", () => {
    expect(faultOf(() => tokens("fixtures/1"))).toEqual({
      kind: "MalformedPointer",
      path: "fixtures/1",
    });
  });

  it("takes an index and nothing that merely looks like one", () => {
    expect(indexOf("/cues/0", "0")).toBe(0);
    expect(indexOf("/cues/12", "12")).toBe(12);
    for (const token of ["a", "01", "+1", "-", "1.0", " 1", ""]) {
      expect(faultOf(() => indexOf("/cues/x", token))).toEqual({
        kind: "NotAnIndex",
        path: "/cues/x",
        token,
      });
    }
  });

  it("knows what is inside what", () => {
    expect(isInside("/fixtures", "/fixtures")).toBe(true);
    expect(isInside("/fixtures", "/fixtures/1")).toBe(true);
    // A prefix that is not a segment boundary is a different path.
    expect(isInside("/fixtures", "/fixturesheet")).toBe(false);
  });
});

describe("reading", () => {
  it("reads through objects and arrays, and the whole document", () => {
    const root = document();
    expect(getAt(root, "/cues/1")).toBe(2);
    expect(getAt(root, "/fixtures/1/name")).toBe("Front");
    expect(getAt(root, "")).toBe(root);
  });

  it("says what was wrong with a pointer that names nothing", () => {
    const root = document();
    expect(faultOf(() => getAt(root, "/cues/9"))).toEqual({
      kind: "NoSuchPath",
      path: "/cues/9",
    });
    expect(faultOf(() => getAt(root, "/cues/x"))).toEqual({
      kind: "NotAnIndex",
      path: "/cues/x",
      token: "x",
    });
    expect(faultOf(() => getAt(root, "/fixtures/1/name/deeper"))).toEqual({
      kind: "NoSuchPath",
      path: "/fixtures/1/name/deeper",
    });
  });
});

describe("operations", () => {
  it("adds an object member", () => {
    const next = applyOp(document(), { op: "add", path: "/fixtures/2", value: 7 });
    expect(getAt(next, "/fixtures/2")).toBe(7);
  });

  it("inserts into an array and appends with a dash", () => {
    let next = applyOp(document(), { op: "add", path: "/cues/0", value: 0 });
    next = applyOp(next, { op: "add", path: "/cues/-", value: 3 });
    expect(getAt(next, "/cues")).toEqual([0, 1, 2, 3]);
  });

  it("treats the empty pointer as the whole document", () => {
    expect(applyOp(document(), { op: "add", path: "", value: null })).toBeNull();
  });

  it("removes a member and an element", () => {
    let next = applyOp(document(), { op: "remove", path: "/fixtures/1" });
    expect(faultOf(() => applyOp(next, { op: "remove", path: "/fixtures/1" }))).toEqual({
      kind: "NoSuchPath",
      path: "/fixtures/1",
    });
    next = applyOp(next, { op: "remove", path: "/cues/0" });
    expect(getAt(next, "/cues")).toEqual([2]);
    // The whole document cannot be removed, only replaced.
    expect(faultOf(() => applyOp(document(), { op: "remove", path: "" }))).toEqual({
      kind: "NoSuchPath",
      path: "",
    });
  });

  it("replaces only what is already there", () => {
    expect(
      faultOf(() => applyOp(document(), { op: "replace", path: "/fixtures/9", value: null })),
    ).toEqual({ kind: "NoSuchPath", path: "/fixtures/9" });
    const next = applyOp(document(), {
      op: "replace",
      path: "/fixtures/1/name",
      value: "Back",
    });
    expect(getAt(next, "/fixtures/1/name")).toBe("Back");
  });

  it("moves and copies a value across", () => {
    let next = applyOp(document(), { op: "copy", from: "/fixtures/1", path: "/fixtures/2" });
    next = applyOp(next, { op: "move", from: "/fixtures/1", path: "/fixtures/3" });
    expect(getAt(next, "/fixtures/2/name")).toBe("Front");
    expect(getAt(next, "/fixtures/3/name")).toBe("Front");
    expect(faultOf(() => getAt(next, "/fixtures/1"))).toEqual({
      kind: "NoSuchPath",
      path: "/fixtures/1",
    });
  });

  it("refuses a move into its own source", () => {
    expect(
      faultOf(() => applyOp(document(), { op: "move", from: "/fixtures", path: "/fixtures/1" })),
    ).toEqual({ kind: "MoveIntoSelf", from: "/fixtures", path: "/fixtures/1" });
  });

  it("tests without writing", () => {
    const root = document();
    expect(applyOp(root, { op: "test", path: "/fixtures/1/name", value: "Front" })).toBe(root);
    expect(
      faultOf(() => applyOp(root, { op: "test", path: "/fixtures/1/name", value: "Back" })),
    ).toEqual({ kind: "TestFailed", path: "/fixtures/1/name" });
  });

  it("addresses an array by index and nothing else", () => {
    for (const token of ["a", "01", "+1"]) {
      expect(faultOf(() => applyOp(document(), { op: "remove", path: `/cues/${token}` }))).toEqual({
        kind: "NotAnIndex",
        path: `/cues/${token}`,
        token,
      });
    }
    expect(faultOf(() => applyOp(document(), { op: "remove", path: "/cues/9" }))).toEqual({
      kind: "IndexOutOfRange",
      path: "/cues/9",
      index: 9,
    });
    expect(
      faultOf(() => applyOp(document(), { op: "add", path: "/cues/9", value: null })),
    ).toEqual({ kind: "IndexOutOfRange", path: "/cues/9", index: 9 });
  });

  it("names nothing through a leaf", () => {
    for (const path of [
      "/fixtures/1/name/deeper/still",
      "/fixtures/1/name/deeper",
      "/cues/0/deeper",
      "/nothing/here",
      "/cues/7/here",
    ]) {
      expect(faultOf(() => applyOp(document(), { op: "add", path, value: null }))).toEqual({
        kind: "NoSuchPath",
        path,
      });
    }
  });

  it("keeps escaped tokens as themselves", () => {
    expect(applyOp({}, { op: "add", path: "/a~1b~0c", value: 1 })).toEqual({ "a/b~c": 1 });
  });

  it("stops a batch at the operation that does not fit", () => {
    expect(
      faultOf(() =>
        applyOps(document(), [
          { op: "add", path: "/fixtures/2", value: 2 },
          { op: "remove", path: "/nothing" },
          { op: "add", path: "/fixtures/3", value: 3 },
        ]),
      ),
    ).toEqual({ kind: "NoSuchPath", path: "/nothing" });
  });

  it("says something readable for every failure", () => {
    const faults = [
      () => tokens("x"),
      () => getAt(document(), "/x"),
      () => getAt(document(), "/cues/a"),
      () => applyOp(document(), { op: "remove", path: "/cues/9" }),
      () => applyOp(document(), { op: "test", path: "/cues/0", value: 9 }),
      () => applyOp(document(), { op: "move", from: "/cues", path: "/cues/0" }),
    ];
    for (const fault of faults) {
      expect(() => fault()).toThrow(MirrorFault);
      try {
        fault();
      } catch (cause) {
        expect(String(cause).length).toBeGreaterThan(20);
      }
    }
  });
});

describe("immutability", () => {
  it("leaves the document it was given alone", () => {
    const root = document();
    const before = structuredClone(root);
    applyOps(root, [
      { op: "add", path: "/fixtures/2", value: { name: "Back" } },
      { op: "replace", path: "/fixtures/1/name", value: "Side" },
      { op: "remove", path: "/cues/0" },
      { op: "add", path: "/cues/-", value: 9 },
    ]);
    expect(root).toEqual(before);
  });

  it("shares everything that did not change", () => {
    const root: JsonValue = {
      fixtures: { "1": { name: "Front" }, "2": { name: "Back" } },
      groups: { "1": { members: [1, 2] } },
    };
    const next = applyOp(root, { op: "replace", path: "/fixtures/1/name", value: "Side" });

    // The point of the exercise: a selector watching the groups, or fixture 2,
    // has nothing to redraw — and can tell by identity rather than by walking.
    const before = getAt(root, "/groups");
    const after = getAt(next, "/groups");
    expect(after).toBe(before);
    expect(getAt(next, "/fixtures/2")).toBe(getAt(root, "/fixtures/2"));
    // And what did change is a different object.
    expect(getAt(next, "/fixtures/1")).not.toBe(getAt(root, "/fixtures/1"));
    expect(next).not.toBe(root);
  });

  it("leaves the earlier operations applied when a batch fails", () => {
    // `prism_core::mirror`'s documented behaviour, and this is the half of it
    // that is visible from outside: the document that threw is not the one the
    // caller still holds, so the caller's own copy is untouched.
    const root = document();
    expect(() =>
      applyOps(root, [
        { op: "add", path: "/fixtures/2", value: 2 },
        { op: "remove", path: "/nothing" },
      ]),
    ).toThrow(MirrorFault);
    expect(root).toEqual(document());
  });
});

describe("equality", () => {
  it("compares structurally and ignores key order", () => {
    expect(equals({ a: 1, b: [1, { c: null }] }, { b: [1, { c: null }], a: 1 })).toBe(true);
    expect(equals({ a: 1 }, { a: 1, b: 2 })).toBe(false);
    expect(equals({ a: 1, b: 2 }, { a: 1, c: 2 })).toBe(false);
    expect(equals([1, 2], [1, 2, 3])).toBe(false);
    expect(equals([1, 2], [1, 3])).toBe(false);
    expect(equals(null, false)).toBe(false);
    expect(equals("a", "a")).toBe(true);
    expect(equals(1, "1")).toBe(false);
    expect(equals([], {})).toBe(false);
  });
});
