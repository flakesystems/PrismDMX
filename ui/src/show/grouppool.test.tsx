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

import type { Command, JsonValue, ProgrammerState } from "../bindings";
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

/**
 * The same session with a line half typed.
 *
 * The line is the **daemon's** (§4.1), so a test that wants one puts it in the
 * session document rather than typing into a box — which is also what a second
 * screen, or the X-Touch, would have done to get it there.
 */
function withLine(line: string): JsonValue {
  return { session: { commandLine: line }, views: {} };
}

/**
 * A programmer holding these groups switched on — S43, B27.
 *
 * The pool lights a box from `ProgrammerState::selectedGroups` and never from a
 * click it remembers, so a test that wanted a lit box has to say so the way the
 * daemon does.
 */
function programmerWith(...on: number[]): ProgrammerState {
  return {
    selection: [],
    selectedGroups: on,
    manualSelection: [],
    activeFeatureGroup: "Dimmer",
    values: [],
    clearStage: 0,
  };
}

/** The window, with a way to read the lines it sent. */
function pool(
  show: JsonValue = SHOW,
  programmer: ProgrammerState | null = null,
  line = "",
) {
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
    <Shell store={store} session={line === "" ? SESSION : withLine(line)} show={show}>
      <GroupPool show={show} programmer={programmer} />
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
  it("switches a group with the same line, whichever way the switch is going", () => {
    // **The line has not changed and its meaning has** — S43, B27. `Toggle`
    // used to run every fixture of the group through the per-fixture toggle, so
    // a second group sharing a lamp with the first took that lamp back out; it
    // is the **group's** switch now, and the daemon works out what a switch-off
    // releases (`Programmer::select_group`). Nothing about that arithmetic is
    // here, which is the point: a client that did the subtraction would be
    // holding a second opinion about a selection.
    const { acted } = pool();
    fireEvent.click(screen.getByTestId("group-3"));
    expect(acted()).toEqual([{ t: "SelectGroup", groupId: 3, mode: "Toggle" }]);

    // And with the switch already down, the line is the same one: what it means
    // is the daemon's answer, not a second command this window picks.
    const off = pool(SHOW, programmerWith(3));
    fireEvent.click(screen.getAllByTestId("group-3").at(-1) as HTMLElement);
    expect(off.acted()).toEqual([{ t: "SelectGroup", groupId: 3, mode: "Toggle" }]);
  });

  /**
   * **Which switches are down is read, not remembered** — B27, and it is what
   * makes the X-Touch and this screen agree. A pool that lit the boxes it had
   * been clicked on would go dark the moment somebody pressed a group key on
   * the console.
   */
  it("lights the groups the programmer says are on, and nothing else", () => {
    pool(SHOW, programmerWith(3));
    expect(screen.getByTestId("group-3").dataset["on"]).toBe("yes");
    expect(screen.getByTestId("group-1").dataset["on"]).toBe("no");
    expect(screen.getByTestId("group-on").textContent).toBe("1 on");

    // Clicking does not light it: the programmer's answer arrives as a delta.
    fireEvent.click(screen.getByTestId("group-1"));
    expect(screen.getByTestId("group-1").dataset["on"]).toBe("no");
  });

  /**
   * **The smaller actions moved to a right-click** — S43, the owner's rebuild:
   * *alle kleineren Group bzw. Preset bezogenen Aktionen sollen über Rechtsklick
   * ausgeführt werden*. The lines are the same ones the two keys on the box
   * used to write; what changed is where they are reached from, and that a box
   * is now only the switch.
   */
  it("deletes and renames from the menu, with the lines every pool shares", () => {
    const { acted } = pool();
    expect(screen.queryByTestId("group-delete-1")).toBeNull();
    expect(screen.queryByTestId("group-label-3")).toBeNull();

    fireEvent.contextMenu(screen.getByTestId("group-1"));
    expect(screen.getByTestId("group-menu").dataset["subject"]).toBe("1");
    fireEvent.click(screen.getByTestId("group-delete"));
    expect(acted()).toEqual([{ t: "Delete", target: { t: "Group", groupId: 1 } }]);

    fireEvent.contextMenu(screen.getByTestId("group-3"));
    fireEvent.click(screen.getByTestId("group-rename"));
    fireEvent.change(screen.getByTestId("group-rename-input"), {
      target: { value: "Back truss" },
    });
    fireEvent.submit(screen.getByTestId("group-rename-input").closest("form") as HTMLFormElement);
    expect(acted().at(-1)).toEqual({
      t: "Label",
      target: { t: "Group", groupId: 3 },
      name: "Back truss",
    });
  });

  /**
   * A move is §4.5's **second** shape: the line is written and left standing,
   * because the destination is the argument the operator still has to type.
   * Nothing is sent.
   */
  it("writes a move line and sends nothing", () => {
    const { acted, line } = pool();
    fireEvent.contextMenu(screen.getByTestId("group-3"));
    fireEvent.click(screen.getByTestId("group-move"));
    expect(acted()).toEqual([]);
    expect(line()).toBe("Move Group 3 Group ");
  });
});

/**
 * **The store bar is gone, and this is what took its place** — S43, the owner's
 * second rebuild: *die Store Sektionen sollen entfernt werden, das soll nur
 * über die Command Line gemacht werden.*
 *
 * The four tests that were here are gone with it, and the claims they made are
 * worth saying once so that nobody looks for them: three were about the number
 * box (it offered the lowest free number, it followed a typed number to that
 * group's name, it kept its value when the typing was not a number) and one was
 * about the name box. All four described a panel that built one line out of
 * three fields — and the line is still there, still the same line, and now
 * reachable in a way the panel never was.
 */
describe("storing is the command line's", () => {
  it("has no store bar at all", () => {
    pool();
    for (const gone of ["group-store", "group-number", "group-name", "store-group"]) {
      expect(screen.queryByTestId(gone), gone).toBeNull();
    }
    // The empty pool says what to type instead of pointing at a bar that has
    // gone: a window that tells an operator to *store one below* when there is
    // no below is worse than one that says nothing.
    const { unmount } = render(
      <Shell store={new DeskStore()} session={SESSION} show={{ groups: {} }}>
        <GroupPool show={{ groups: {} }} programmer={null} />
      </Shell>,
    );
    expect(screen.getByTestId("group-empty").textContent).toContain("Store Group 1");
    unmount();
  });

  /**
   * **A pool is an argument keyboard** — `consoleshell.ts::pickOnto`, and the
   * gesture the owner described: *in der Command Line steht Store und ich klicke
   * auf Sequence 2, dann soll "Sequence 2" angehängt werden*.
   *
   * `Delete Group 3` is a whole command with nothing left to answer, so it goes
   * at once rather than waiting for an Enter the operator has already committed
   * to — which is the second half of what was asked for.
   */
  it("finishes a waiting line and sends it at once", () => {
    const { acted } = pool(SHOW, null, "Delete");
    fireEvent.click(screen.getByTestId("group-3"));
    expect(acted()).toEqual([{ t: "Delete", target: { t: "Group", groupId: 3 } }]);
    // And it did **not** switch the group: an operator who typed a verb was
    // asking for an argument, not for a selection.
    expect(acted().some((command) => command.t === "SelectGroup")).toBe(false);
  });

  /**
   * **A finished line still goes through the console's own question.**
   *
   * `Store Group 3` is complete, and group 3 exists — so what happens is what
   * happens when the line is typed: the console asks which mode before it
   * sends anything (S39, S40). *Sent at once* means *not waiting for Enter*, not
   * *skipping the question an overwrite raises*.
   */
  it("asks before it overwrites, exactly as the typed line does", () => {
    const { acted, line } = pool(SHOW, null, "Store");
    fireEvent.click(screen.getByTestId("group-3"));
    expect(line()).toBe("Store Group 3");
    expect(acted()).toEqual([]);
  });

  /**
   * **A line the group cannot go into leaves the box alone.**
   *
   * *Passend* is the owner's word for it and the parser is what decides: a
   * group is not a fixture, so `1 thru Group 3` is not a line and the click is
   * the switch it has always been.
   */
  it("does the box's own thing when the line cannot take a group", () => {
    const { acted } = pool(SHOW, null, "1 thru");
    fireEvent.click(screen.getByTestId("group-3"));
    expect(acted().at(-1)).toEqual({ t: "SelectGroup", groupId: 3, mode: "Toggle" });
  });

  /**
   * **A verb whose missing word means *take it away* is never finished by a
   * click** — `CLEARING_VERBS`. `Label Group 3` is a perfectly good command
   * that clears the name, so a pointer that sent it would wipe a name with one
   * click. It is appended and left standing for the operator to finish.
   */
  it("leaves a clearing verb standing rather than sending it", () => {
    const { acted, line } = pool(SHOW, null, "Label");
    fireEvent.click(screen.getByTestId("group-3"));
    expect(acted()).toEqual([]);
    expect(line()).toBe("Label Group 3 ");
  });
});
