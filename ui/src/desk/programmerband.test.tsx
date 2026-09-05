/**
 * **The two gestures that take an attribute over** — S43, the owner's fourth
 * point from the hand-testing round.
 *
 * > Es muss klarer markiert sein, welche Attribute grade überschrieben werden
 * > und welche nicht. Durch rechtsklicken auf eine Kategorie sollen alle ihre
 * > Attribute als überschreibend ausgewählt werden. Wird auf ein Attribut
 * > geklickt ohne es zu bewegen, soll es den default Wert behalten, aber
 * > überschreiben.
 *
 * *Overriding* is the operator's word for what `docs/DMX_MERGE.md` §3 calls the
 * programmer's absolute priority: an attribute the programmer holds goes out
 * whatever a playback says about it, and one it does not hold rests.
 *
 * # Why the commands are a turn of nought
 *
 * A relative `SetAttribute` starts from what the programmer holds, or from the
 * attribute's resting value when it holds nothing — so a delta of zero writes
 * exactly the value that was already going out, and *writing* it is what makes
 * it an override. There is no new command, and nothing in this interface works
 * out what the value is: `prism_core::Programmer::set_attribute` does, per
 * fixture, which is the only place that knows each one's profile.
 *
 * The band is driven directly here rather than through `<App />`: what is being
 * asserted is which commands two gestures produce, and a whole interface around
 * it would add a socket to a question that has none in it.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { AttributeType, JsonValue, ProgrammerState } from "../bindings";
import { nullSink, setLogSink } from "../log/logger";
import { ProgrammerBand } from "./programmerband";

setLogSink(nullSink);

/** A head with two colours and two position attributes, and where each rests. */
const SHOW: JsonValue = {
  fixtures: { "5": { typeId: "head" } },
  fixtureTypes: {
    head: {
      attributes: [
        { attribute: "Red", featureGroup: "Color", defaultValue: 65535 },
        { attribute: "Green", featureGroup: "Color", defaultValue: 65535 },
        { attribute: "Pan", featureGroup: "Position", defaultValue: 32768 },
        { attribute: "Tilt", featureGroup: "Position", defaultValue: 32768 },
        // **B38.** A gobo wheel with named ranges, the way an OFL profile
        // carries one. `to` is the top of each range and the next one starts a
        // step above it, so no value belongs to nothing.
        {
          attribute: "Gobo",
          featureGroup: "Gobo",
          defaultValue: 0,
          ranges: [
            { name: "Open", from: 0, to: 9999 },
            { name: "Gobo 1", from: 10000, to: 19999 },
            { name: "Gobo 2", from: 20000, to: 65535 },
          ],
        },
      ],
    },
  },
};

/** A session on one bank. */
function session(bank: string): JsonValue {
  return {
    session: {
      activeViewId: 1,
      openWindows: [],
      focusedWindow: null,
      executorPage: 0,
      encoderBank: bank,
      commandLine: "",
      programmerPage: 0,
      programmerParamIndex: 0,
    },
    views: {},
  };
}

/** A programmer selecting the head, holding whatever it is given. */
function programmer(values: [AttributeType, number][] = []): ProgrammerState {
  return {
    selection: [5],
    selectedGroups: [],
    manualSelection: [],
    activeFeatureGroup: "Color",
    clearStage: 0,
    values: values.map(([attribute, value]) => ({
      fixture: 5,
      attribute,
      value: { value, source: "Manual", presetRef: null },
    })),
  };
}

/** The band, every attribute it asked to take over, and every range picked. */
function band(bank: string, state: ProgrammerState) {
  const taken: AttributeType[][] = [];
  const picked: { attribute: AttributeType; name: string; from: number; to: number }[] = [];
  render(
    <ProgrammerBand
      session={session(bank)}
      show={SHOW}
      programmer={state}
      onBank={() => undefined}
      onParam={() => undefined}
      onPage={() => undefined}
      onTurn={() => undefined}
      onTake={(attributes) => taken.push([...attributes])}
      onPickRange={(reading, range) =>
        picked.push({ attribute: reading.attribute, ...range })
      }
      onLine={() => undefined}
    />,
  );
  return { taken, picked };
}

describe("taking one attribute over", () => {
  it("is a click, and it leaves the value where it was", () => {
    const { taken } = band("Color", programmer());
    fireEvent.click(screen.getByTestId("encoder-Red"));
    expect(taken).toEqual([["Red"]]);
  });

  it("does not do it twice to an attribute that is already overriding", () => {
    // The reason this matters is `presetRef`: a second write would set the
    // source to `Manual` and a value that came from a preset would silently lose
    // its link (S13) — for a click that was meant to change nothing at all.
    const { taken } = band("Color", programmer([["Red", 100]]));
    fireEvent.click(screen.getByTestId("encoder-Red"));
    expect(taken).toEqual([]);
  });

  it("does not do it to an attribute nothing selected has", () => {
    // The head has no Iris. The encoder is still drawn — dimmed, so an operator
    // can see the bank has one and that their selection has not got it — but
    // clicking it takes nothing over, because there is no fixture the daemon
    // could write it for.
    const { taken } = band("Beam", programmer());
    const iris = screen.getByTestId("encoder-Iris");
    expect(iris.className).toContain("encoder-absent");
    fireEvent.click(iris);
    expect(taken).toEqual([]);
  });
});

describe("taking a whole bank over", () => {
  it("is a right-click on the bank key, and it names every attribute on it", () => {
    const { taken } = band("Color", programmer());
    fireEvent.contextMenu(screen.getByTestId("bank-Position"));
    expect(taken).toEqual([["Pan", "Tilt"]]);
  });

  it("leaves out the ones that are already overriding", () => {
    const { taken } = band("Color", programmer([["Pan", 1000]]));
    fireEvent.contextMenu(screen.getByTestId("bank-Position"));
    expect(taken).toEqual([["Tilt"]]);
  });

  it("names nothing at all when the whole bank is already overriding", () => {
    const { taken } = band(
      "Color",
      programmer([
        ["Pan", 1000],
        ["Tilt", 2000],
      ]),
    );
    fireEvent.contextMenu(screen.getByTestId("bank-Position"));
    expect(taken).toEqual([[]]);
  });

  it("names nothing on a bank the selection has no attribute of", () => {
    const { taken } = band("Color", programmer());
    fireEvent.contextMenu(screen.getByTestId("bank-Beam"));
    expect(taken).toEqual([[]]);
  });
});

describe("the mark on an encoder", () => {
  /**
   * **The half that makes the number safe to show.**
   *
   * Both encoders read a percentage now — the programmer's value, or the
   * resting value when it holds none — so *is this mine?* has to be carried by
   * something other than the number, or an operator cannot tell an asserted
   * value from one that is merely where the lamp sits.
   */
  it("says which attributes are overriding and which are resting", () => {
    band("Color", programmer([["Red", 32767]]));
    expect(screen.getByTestId("encoder-Red").dataset.overriding).toBe("yes");
    expect(screen.getByTestId("encoder-Green").dataset.overriding).toBe("no");
    // And the resting one still shows a number: a colour rests open (B1), which
    // is what tells an operator to pull it down rather than push it up.
    expect(screen.getByTestId("value-Green").textContent).toBe("100%");
    expect(screen.getByTestId("value-Red").textContent).toBe("50%");
  });
});

/**
 * **A channel with named ranges** — S51, punch-list B38.
 *
 * The half of the entry that is about capabilities rather than about channels.
 * Until S51 an OFL channel's ranges were read by nothing at all, so a gobo wheel
 * was a number an operator had to know by heart; now the encoder names the range
 * it is standing in and offers the list.
 */
describe("a channel with named ranges", () => {
  it("names the range the value is standing in", () => {
    const { picked } = band("Gobo", programmer([["Gobo", 15000]]));
    expect(screen.getByTestId("range-Gobo").textContent).toBe("Gobo 1");
    expect(picked).toEqual([]);
  });

  it("names the range the resting value is in when the programmer holds nothing", () => {
    // The same rule the number follows since B1: what an encoder reads is the
    // programmer's value, or the profile's resting value when it holds none.
    band("Gobo", programmer());
    expect(screen.getByTestId("range-Gobo").textContent).toBe("Open");
  });

  it("offers the list, and picking one asks for the middle of it", () => {
    const { picked } = band("Gobo", programmer([["Gobo", 0]]));
    fireEvent.click(screen.getByTestId("range-Gobo"));
    const list = screen.getByTestId("ranges-Gobo");
    expect([...list.querySelectorAll("button")].map((button) => button.textContent)).toEqual([
      "Open",
      "Gobo 1",
      "Gobo 2",
    ]);

    fireEvent.click(screen.getByTestId("range-Gobo-Gobo 2"));
    expect(picked).toEqual([{ attribute: "Gobo", name: "Gobo 2", from: 20000, to: 65535 }]);
    // The list puts itself away: the question it was asking has been answered.
    expect(screen.queryByTestId("ranges-Gobo")).toBeNull();
  });

  it("draws nothing at all for a channel that has no ranges", () => {
    // Every continuous encoder on the desk is one of these, and a row of empty
    // space under each would take height off the canvas for nothing.
    band("Color", programmer());
    expect(screen.queryByTestId("range-Red")).toBeNull();
  });
});
