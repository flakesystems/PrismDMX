/**
 * Reading a document for display: a pointer that names nothing is an ordinary
 * answer here, not the divergence a delta that does not fit reports.
 */

import { describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";
import { booleanAt, countAt, numberAt, stringAt, valueAt } from "./select";

const document: JsonValue = {
  session: { executorPage: 3, commandLine: "fixture 1 at full", locked: false },
  fixtures: { "1": {}, "2": {} },
  windows: [{ type: "Patch" }],
  nothing: null,
};

describe("reading for display", () => {
  it("answers with what is there", () => {
    expect(numberAt(document, "/session/executorPage")).toBe(3);
    expect(stringAt(document, "/session/commandLine")).toBe("fixture 1 at full");
    expect(booleanAt(document, "/session/locked")).toBe(false);
    expect(countAt(document, "/fixtures")).toBe(2);
    expect(countAt(document, "/windows")).toBe(1);
    expect(valueAt(document, "/windows/0")).toEqual({ type: "Patch" });
  });

  it("answers null for a pointer that names nothing, rather than throwing", () => {
    // A window open on a fixture that has just been deleted is an ordinary
    // state, and a view is not the place to discover it.
    expect(numberAt(document, "/session/nothing")).toBeNull();
    expect(stringAt(document, "/nowhere/at/all")).toBeNull();
    expect(countAt(document, "/session/executorPage")).toBeNull();
    expect(valueAt(document, "/fixtures/9")).toBeNull();
    expect(valueAt(null, "/anything")).toBeNull();
    expect(countAt(null, "/fixtures")).toBeNull();
  });

  it("answers null for something of the wrong kind, rather than coercing it", () => {
    expect(numberAt(document, "/session/commandLine")).toBeNull();
    expect(stringAt(document, "/session/executorPage")).toBeNull();
    expect(booleanAt(document, "/session/executorPage")).toBeNull();
    expect(countAt(document, "/nothing")).toBeNull();
  });

  it("passes a malformed pointer on, because that is a mistake in the caller", () => {
    expect(() => valueAt(document, "session/executorPage")).toThrow();
  });
});
