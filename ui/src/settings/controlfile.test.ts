/**
 * The control map as a file: what is written, and what a wrong file is told.
 *
 * `controls.test.tsx` holds the panel's half — that an import becomes one
 * `SurfaceBinding` per control. This holds the **document**, and the claim that
 * matters most is the round trip: what this writes is what this reads, and both
 * are the shape `prism_surface::Bindings::parse` takes. A file an export
 * produced that the daemon then refused would be the worst outcome of the whole
 * feature, and it is the one thing a test on this side can rule out short of
 * running the daemon — which `crates/prism-surface/tests/bindings.rs` does.
 */

import { describe, expect, it } from "vitest";

import type { SurfaceAction, SurfaceControl } from "../bindings";
import { profileDocument, readProfile } from "./controlfile";

/** One row of a table, as the daemon sends it. */
function control(name: string, action: SurfaceAction | null): SurfaceControl {
  return {
    name,
    action,
    control: { t: "Global", button: "F1" },
    permanent: false,
    reserved: false,
  };
}

const TABLE: readonly SurfaceControl[] = [
  control("Global.F1", { t: "OpenWindow", window: "Patch" }),
  control("Global.F2", { t: "WriteCommandLine", line: "Go Executor 1", submit: true }),
  // **Bound to nothing, and still in the file** — see below.
  control("Main.Fader", null),
];

describe("writing a control map", () => {
  it("writes the profile shape the daemon reads", () => {
    const written: unknown = JSON.parse(profileDocument("behringer-x-touch", 1, TABLE));
    expect(written).toEqual({
      profileVersion: 1,
      device: "behringer-x-touch",
      documentation: expect.stringContaining("prism_surface::Bindings::parse"),
      bindings: [
        { control: "Global.F1", action: { t: "OpenWindow", window: "Patch" } },
        {
          control: "Global.F2",
          action: { t: "WriteCommandLine", line: "Go Executor 1", submit: true },
        },
        { control: "Main.Fader", action: null },
      ],
    });
  });

  /**
   * **The empty keys are in the file**, and that is the decision.
   *
   * A file that left them out would *merge* when it was read back — whatever the
   * receiving desk had bound would stay in the gaps — and the point of an export
   * is that reading it gives you the desk it came from. `Bindings::parse` starts
   * from an empty table for the same reason.
   */
  it("keeps the controls that are bound to nothing", () => {
    const written = profileDocument("behringer-x-touch", 1, TABLE);
    expect(written).toContain('"Main.Fader"');
  });

  /** A file an operator can open, and a diff can read. */
  it("is indented and ends in a newline", () => {
    const written = profileDocument("behringer-x-touch", 1, TABLE);
    expect(written.endsWith("}\n")).toBe(true);
    expect(written).toContain("\n  ");
  });
});

describe("reading one back", () => {
  it("round-trips a table it wrote", () => {
    const read = readProfile(profileDocument("behringer-x-touch", 1, TABLE), "behringer-x-touch", 1);
    expect(read).toBeInstanceOf(Map);
    if (!(read instanceof Map)) {
      return;
    }
    expect(read.size).toBe(3);
    expect(read.get("Global.F1")).toEqual({ t: "OpenWindow", window: "Patch" });
    expect(read.get("Global.F2")).toEqual({
      t: "WriteCommandLine",
      line: "Go Executor 1",
      submit: true,
    });
    // Present with a `null`, which is not the same as absent — see the module
    // documentation of `controlfile.ts`.
    expect(read.has("Main.Fader")).toBe(true);
    expect(read.get("Main.Fader")).toBeNull();
  });

  /**
   * Each refusal is a **sentence**, because the panel draws it: an operator who
   * picked the wrong file needs to be told which wrong thing it was, and
   * *nothing happened* is the worst of the possible answers.
   */
  it("says which wrong thing a file is", () => {
    expect(readProfile("not json at all", "behringer-x-touch", 1)).toContain("not JSON");
    expect(readProfile("[1, 2]", "behringer-x-touch", 1)).toContain("not an object");
    expect(
      readProfile(JSON.stringify({ profileVersion: 9, device: "behringer-x-touch" }), "behringer-x-touch", 1),
    ).toContain("version 9");
    expect(
      readProfile(JSON.stringify({ profileVersion: 1, device: "other-desk" }), "behringer-x-touch", 1),
    ).toContain("other-desk");
    expect(
      readProfile(JSON.stringify({ profileVersion: 1, device: "behringer-x-touch" }), "behringer-x-touch", 1),
    ).toContain("no bindings list");
    expect(
      readProfile(
        JSON.stringify({ profileVersion: 1, device: "behringer-x-touch", bindings: [] }),
        "behringer-x-touch",
        1,
      ),
    ).toContain("empty");
  });

  /**
   * A row that is not a row is skipped rather than refusing the file.
   *
   * The alternative is a whole control map thrown away over one line somebody
   * hand-edited — and the daemon is the gate that matters: every row this
   * answers with still goes through `MachineChange::SurfaceBinding`, which
   * refuses what it cannot use, in its own words.
   */
  it("skips a row that is not a binding and keeps the rest", () => {
    const read = readProfile(
      JSON.stringify({
        profileVersion: 1,
        device: "behringer-x-touch",
        bindings: [
          "nonsense",
          { control: 7, action: null },
          { control: "Global.F1", action: { t: "Oops" } },
          { control: "Global.F2", action: "not an action" },
        ],
      }),
      "behringer-x-touch",
      1,
    );
    expect(read).toBeInstanceOf(Map);
    if (!(read instanceof Map)) {
      return;
    }
    expect([...read.keys()]).toEqual(["Global.F1", "Global.F2"]);
    expect(read.get("Global.F1")).toEqual({ t: "Oops" });
    // An action that is not an object at all is *no action*, which the daemon
    // reads as an unbind rather than as a word it does not know.
    expect(read.get("Global.F2")).toBeNull();
  });
});
