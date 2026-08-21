/**
 * **The Group Pool, key by key: every one of them writes a line.**
 *
 * `ARCHITECTURE_SPEC.md` §4.5 is the claim under test, so each assertion is
 * about *what the line said* rather than about what the pool then drew — the
 * pool draws the mirror, and the mirror only moves when the daemon says so.
 *
 * The store bar is the interesting half: it is where a number, a name and a mode
 * become one `Store Group` line, and where the two questions an operator asks of
 * a pool box — *is something already filed here* and *what is it called* — are
 * answered from the mirror rather than remembered.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import type { Command, JsonValue } from "../bindings";
import { nullSink, setLogSink } from "../log/logger";
import { DeskStore } from "../store/desk";
import { Shell } from "../testing/shell";
import { GroupPool } from "./grouppool";

/** Groups 1 and 3, so 2 is the free number the store bar should offer. */
const SHOW: JsonValue = {
  groups: {
    "1": { id: 1, name: "Front wash", fixtures: [1, 2, 3] },
    "3": { id: 3, name: "Back truss", fixtures: [4] },
  },
};

const SESSION: JsonValue = { session: { commandLine: "" }, views: {} };

/** The window, with a way to read the lines it sent. */
function pool(show: JsonValue = SHOW) {
  const store = new DeskStore();
  const sent: Command[] = [];
  store.attach(
    (command) => {
      sent.push(command);
      return sent.length;
    },
    () => null,
  );
  render(
    <Shell store={store} session={SESSION} show={show}>
      <GroupPool show={show} />
    </Shell>,
  );
  /** The whole traffic, and the half of it that is not the line being typed. */
  return {
    sent,
    acted: (): Command[] => sent.filter((command) => command.t !== "CommandLineInput"),
    /** The last line the console wrote, which is what a *write* key leaves. */
    line: (): string => {
      const written = sent.filter((command) => command.t === "CommandLineInput");
      const last = written.at(-1);
      return last === undefined || last.t !== "CommandLineInput" ? "" : last.text;
    },
  };
}

function field(testId: string): HTMLInputElement {
  const element = screen.getByTestId(testId);
  if (!(element instanceof HTMLInputElement)) {
    throw new Error(`${testId} is an input`);
  }
  return element;
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the pool draws the groups the show holds", () => {
  it("lists them in number order with their membership counts", () => {
    pool();
    expect(screen.getByTestId("group-count").textContent).toBe("2 groups");
    expect(screen.getByTestId("group-1").textContent).toContain("Front wash");
    expect(screen.getByTestId("group-1").textContent).toContain("3 fixtures");
    expect(screen.getByTestId("group-3").textContent).toContain("1 fixtures");
  });

  /**
   * A show with no groups says so in words, because an empty grid looks like a
   * window that failed to load rather than a pool nobody has filled.
   */
  it("says what to do when there are none", () => {
    pool({ groups: {} });
    expect(screen.getByTestId("group-empty")).toBeTruthy();
    expect(screen.queryByTestId("group-scroll")).toBeNull();
  });

  /** A group with no name draws a dash rather than nothing at all. */
  it("draws a dash for a group nobody named", () => {
    pool({ groups: { "5": { id: 5, name: "", fixtures: [] } } });
    expect(screen.getByTestId("group-5").textContent).toContain("—");
  });
});

describe("every key writes a line", () => {
  /** §4.5's third case: the pointer supplied the argument, so it submits. */
  it("selects a group by writing Group 3 and submitting it", () => {
    const { acted } = pool();
    fireEvent.click(screen.getByTestId("group-3"));
    expect(acted()).toEqual([{ t: "SelectGroup", groupId: 3, mode: "Set" }]);
  });

  /** The delete key is the same shape: a whole line, sent. */
  it("deletes a group by writing Delete Group 1", () => {
    const { acted } = pool();
    fireEvent.click(screen.getByTestId("group-delete-1"));
    expect(acted()).toEqual([{ t: "Delete", target: { t: "Group", groupId: 1 } }]);
  });

  /**
   * The label key is §4.5's **second** shape: the line is written and left
   * standing, because the name is the argument the operator still has to type.
   * Nothing is sent.
   */
  it("writes a label line and sends nothing", () => {
    const { acted, line } = pool();
    fireEvent.click(screen.getByTestId("group-label-3"));
    expect(acted()).toEqual([]);
    expect(line()).toBe('Label Group 3 "Back truss"');
  });
});

describe("the store bar", () => {
  /**
   * **The number it offers is the lowest free one**, so an operator who presses
   * Store without thinking about it does not overwrite a group they made a
   * minute ago.
   */
  it("offers the first free number and stores under it", () => {
    const { acted } = pool();
    expect(field("group-number").value).toBe("2");
    expect(screen.getByTestId("store-group").textContent).toBe("Store group 2");

    fireEvent.submit(screen.getByTestId("group-store"));
    expect(acted()).toEqual([
      { t: "StoreGroup", groupId: 2, name: "Group 2", mode: "Merge" },
    ]);
  });

  /**
   * Typing a number that **is** taken offers that group's own name and says
   * what the mode would do to it, which is the pool's half of *is this already
   * there* — the console asks the same question of the same mirror.
   */
  it("follows the number to the name of the group that is already there", () => {
    const { acted } = pool();
    fireEvent.change(field("group-number"), { target: { value: "3" } });
    expect(field("group-name").value).toBe("Back truss");
    expect(screen.getByTestId("store-group").textContent).toBe("Merge into group 3");

    fireEvent.change(screen.getByTestId("group-store-mode"), { target: { value: "Override" } });
    expect(screen.getByTestId("store-group").textContent).toBe("Override into group 3");

    fireEvent.submit(screen.getByTestId("group-store"));
    expect(acted()).toEqual([
      { t: "StoreGroup", groupId: 3, name: "Back truss", mode: "Override" },
    ]);
  });

  /** A name the operator typed wins, and survives the store's own reset. */
  it("stores the name that was typed", () => {
    const { acted } = pool();
    fireEvent.change(field("group-name"), { target: { value: "Side booms" } });
    fireEvent.submit(screen.getByTestId("group-store"));
    expect(acted()).toEqual([
      { t: "StoreGroup", groupId: 2, name: "Side booms", mode: "Merge" },
    ]);
    // And the bar goes back to offering the free number, because the store it
    // just sent has not come back yet and guessing that it worked would be the
    // client holding show state (**D3**).
    expect(field("group-number").value).toBe("2");
    expect(field("group-name").value).toBe("Group 2");
  });

  /** Anything that is not a group number is ignored rather than accepted. */
  it("keeps the number it had when the field is not a number", () => {
    pool();
    for (const typed of ["", "  ", "0", "-4", "2.5", "x"]) {
      fireEvent.change(field("group-number"), { target: { value: typed } });
      expect(field("group-number").value, typed).toBe("2");
    }
  });
});
