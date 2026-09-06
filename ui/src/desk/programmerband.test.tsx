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
import { parameterLabel } from "./programmer";
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
          // **S53.** What the manufacturer calls this channel. The *key* is
          // still `Gobo`, which is what a preset is filed under; this is the
          // word on the knob.
          label: "Static Gobo",
          ranges: [
            { name: "Open", from: 0, to: 9999 },
            { name: "Gobo 1", from: 10000, to: 19999 },
            { name: "Gobo 2", from: 20000, to: 65535 },
          ],
        },
        // **S52.** A second gobo wheel, which this desk used to drop. It is a
        // knob of its own beside the first, numbered.
        {
          attribute: "Gobo",
          occurrence: 1,
          featureGroup: "Gobo",
          defaultValue: 0,
          label: "Rotating Gobo",
          ranges: [
            { name: "Open", from: 0, to: 32767 },
            { name: "Breakup", from: 32768, to: 65535 },
          ],
        },
        // **S54.** Two slots this desk has no word for, the way the library
        // reader produces them: one the file named and one it did not. Both are
        // knobs on the Control bank; before S54 they were not knobs at all.
        {
          attribute: "Raw",
          occurrence: 0,
          featureGroup: "Control",
          defaultValue: 0,
          label: "Reserved 1",
          ranges: [],
        },
        {
          attribute: "Raw",
          occurrence: 1,
          featureGroup: "Control",
          defaultValue: 0,
          label: "Ch 9",
          ranges: [],
        },
      ],
    },
  },
};

/**
 * **A tube with a red per pixel** — the other shape S52 has to handle.
 *
 * Six cells is more than `INLINE_OCCURRENCES`, so the colour bank draws one
 * **part** at a time rather than six pages of things called *Red*.
 */
const TUBE: JsonValue = {
  fixtures: { "5": { typeId: "tube", softwareDimmer: false } },
  fixtureTypes: {
    tube: {
      attributes: Array.from({ length: 6 }, (_, cell) => ({
        attribute: "Red",
        occurrence: cell,
        featureGroup: "Color",
        defaultValue: 65535,
      })),
    },
  },
};

/** A session on one bank, and on one part of a repeated fixture. */
function session(bank: string, part = 0): JsonValue {
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
      programmerOccurrence: part,
    },
    views: {},
  };
}

/** A programmer selecting the head, holding whatever it is given. */
function programmer(values: [AttributeType, number, number?][] = []): ProgrammerState {
  return {
    selection: [5],
    selectedGroups: [],
    manualSelection: [],
    activeFeatureGroup: "Color",
    clearStage: 0,
    values: values.map(([attribute, value, occurrence]) => ({
      fixture: 5,
      attribute,
      occurrence: occurrence ?? 0,
      value: { value, source: "Manual", presetRef: null },
    })),
  };
}

/** The band, every attribute it asked to take over, and every range picked. */
function band(bank: string, state: ProgrammerState, show: JsonValue = SHOW, part = 0) {
  // **Labels rather than bare attribute names since S52**, so a test can tell
  // the second gobo wheel from the first.
  const taken: string[][] = [];
  const picked: { label: string; name: string; from: number; to: number }[] = [];
  const parts: number[] = [];
  render(
    <ProgrammerBand
      session={session(bank, part)}
      show={show}
      programmer={state}
      onBank={() => undefined}
      onParam={() => undefined}
      onPage={() => undefined}
      onTurn={() => undefined}
      onPart={(occurrence) => parts.push(occurrence)}
      onTake={(keys) => taken.push(keys.map(parameterLabel))}
      onPickRange={(reading, range) => picked.push({ label: reading.label, ...range })}
      onLine={() => undefined}
    />,
  );
  return { taken, picked, parts };
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

  it("draws no encoder at all for an attribute nothing selected has", () => {
    // **S52, and the owner's own ask.** The head has no Iris, and the Beam bank
    // used to draw one dimmed so an operator could see the bank had it. It does
    // not any more: a bank shows what the fixtures have, and the band says in a
    // sentence that this one has nothing here.
    band("Beam", programmer());
    expect(screen.queryByTestId("encoder-Iris")).toBeNull();
    expect(screen.getByTestId("no-parameters").textContent).toContain("Beam");
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

  /**
   * **Right-click opens the steps, and a click on the name does not** — S52,
   * the owner's own wording.
   *
   * S51 hung a button under each encoder that dropped a list down inside a band
   * of fixed height. The name of the range stays, as a reading; the window it
   * used to open comes off a right-click instead, which is the gesture the
   * pools already use for *manage this*.
   */
  it("opens the steps window on a right-click, and picking one asks for the middle", () => {
    const { picked } = band("Gobo", programmer([["Gobo", 0]]));
    // A left-click on the name is not a control: it selects the encoder, and
    // no window opens.
    fireEvent.click(screen.getByTestId("range-Gobo"));
    expect(screen.queryByTestId("range-picker")).toBeNull();

    fireEvent.contextMenu(screen.getByTestId("encoder-Gobo"));
    const list = screen.getByTestId("ranges-Gobo");
    expect(
      [...list.querySelectorAll("button")].map(
        (button) => button.querySelector(".range-step-name")?.textContent,
      ),
    ).toEqual(["Open", "Gobo 1", "Gobo 2"]);

    fireEvent.click(screen.getByTestId("range-Gobo-Gobo 2"));
    expect(picked).toEqual([{ label: "Gobo", name: "Gobo 2", from: 20000, to: 65535 }]);
    // The window puts itself away: the question it was asking has been answered.
    expect(screen.queryByTestId("range-picker")).toBeNull();
  });

  it("opens the second wheel's own steps, and not the first one's", () => {
    // **S52.** The two wheels are separate channels with separate lists, and
    // offering the first one's slots for the second would name the wrong gobo.
    const { picked } = band("Gobo", programmer());
    fireEvent.contextMenu(screen.getByTestId("encoder-Gobo-2"));
    expect(screen.getByTestId("range-picker")).toBeTruthy();
    fireEvent.click(screen.getByTestId("range-Gobo-Breakup"));
    expect(picked).toEqual([{ label: "Gobo 2", name: "Breakup", from: 32768, to: 65535 }]);
  });

  it("draws nothing at all for a channel that has no ranges", () => {
    // Every continuous encoder on the desk is one of these, and a row of empty
    // space under each would take height off the canvas for nothing. A
    // right-click on one opens no window either.
    band("Color", programmer());
    expect(screen.queryByTestId("range-Red")).toBeNull();
    fireEvent.contextMenu(screen.getByTestId("encoder-Red"));
    expect(screen.queryByTestId("range-picker")).toBeNull();
  });
});

/**
 * **The name the manufacturer gave the channel** — S53, `AttributeDef::label`.
 *
 * The encoder reads the fixture's own word where the profile carries one, so an
 * operator holding a data sheet reads the same thing off the screen. It is a
 * **label and not a key**: both of these knobs are `Gobo`, which is what the
 * gestures below still name.
 */
describe("the word on an encoder", () => {
  it("is the manufacturer's where the profile has one", () => {
    band("Gobo", programmer());
    expect(screen.getByTestId("encoder-Gobo").textContent).toContain("Static Gobo");
    expect(screen.getByTestId("encoder-Gobo-2").textContent).toContain("Rotating Gobo");
  });

  it("is the desk's own where it has none", () => {
    // Every continuous channel of this rig, and every profile a show embedded
    // before S53.
    band("Color", programmer());
    expect(screen.getByTestId("encoder-Red").textContent).toContain("Red");
  });

  it("does not change what a gesture names", () => {
    // The word moved; the key did not. `Gobo 2` is what the daemon is told,
    // which is why a preset made on this head plays back on another.
    const { taken } = band("Gobo", programmer());
    fireEvent.click(screen.getByTestId("encoder-Gobo-2"));
    expect(taken).toEqual([["Gobo 2"]]);
  });

  it("titles the steps window with it", () => {
    band("Gobo", programmer());
    fireEvent.contextMenu(screen.getByTestId("encoder-Gobo-2"));
    expect(screen.getByTestId("range-picker").textContent).toContain("Rotating Gobo");
  });
});

/**
 * **A channel this desk has no word for** — S54, the owner's own report.
 *
 * A slot the library reader could not place used to reach the desk as nothing
 * at all: no encoder, no line, no cue, while the fixture went on occupying the
 * channel. It is a knob on the **Control** bank now, and the only thing that
 * makes it usable is its name — the manufacturer's word where the profile has
 * one, and `Ch 9` where it does not.
 */
describe("a channel the desk has no word for", () => {
  it("is a knob on the control bank, under the name the profile gives it", () => {
    band("Control", programmer());
    expect(screen.getByTestId("encoder-Raw").textContent).toContain("Reserved 1");
    expect(screen.getByTestId("encoder-Raw-2").textContent).toContain("Ch 9");
  });

  it("is named by a gesture the way any other parameter is", () => {
    // The label moved; the key did not. `Raw 2` is what the daemon is told,
    // which is what makes `1 raw 2 at 50` mean the same thing on the line.
    const { taken } = band("Control", programmer());
    fireEvent.click(screen.getByTestId("encoder-Raw-2"));
    expect(taken).toEqual([["Raw 2"]]);
  });

  it("has no steps window, because there is nothing to read steps from", () => {
    band("Control", programmer());
    fireEvent.contextMenu(screen.getByTestId("encoder-Raw"));
    expect(screen.queryByTestId("range-picker")).toBeNull();
  });
});

/**
 * **A fixture with two of a parameter** — S52, the owner's own report.
 *
 * A head with two colour wheels used to keep the lower channel and drop the
 * higher one. Both are knobs now, and which one a gesture names is the pair
 * `(attribute, occurrence)` rather than the attribute alone.
 */
describe("a repeated parameter", () => {
  it("is a knob of its own, numbered from the second", () => {
    const { taken } = band("Gobo", programmer());
    expect(screen.getByTestId("encoder-Gobo").textContent).toContain("Gobo");
    expect(screen.getByTestId("encoder-Gobo-2")).toBeTruthy();
    fireEvent.click(screen.getByTestId("encoder-Gobo-2"));
    expect(taken).toEqual([["Gobo 2"]]);
  });

  it("takes the whole bank over including the repeats", () => {
    const { taken } = band("Color", programmer());
    fireEvent.contextMenu(screen.getByTestId("bank-Gobo"));
    expect(taken).toEqual([["Gobo", "Gobo 2"]]);
  });

  it("is held on its own: the first wheel is untouched when the second is set", () => {
    band("Gobo", programmer([["Gobo", 65535, 1]]));
    expect(screen.getByTestId("encoder-Gobo").dataset.overriding).toBe("no");
    expect(screen.getByTestId("encoder-Gobo-2").dataset.overriding).toBe("yes");
  });

  it("has no part stepper, because two knobs fit side by side", () => {
    // The stepper is for the case knobs cannot cover. A head with two colour
    // wheels has both on the bank, and a control that walked between them would
    // be a control that does nothing.
    band("Gobo", programmer());
    expect(screen.queryByTestId("part-steps")).toBeNull();
  });
});

/**
 * **A fixture with more repeats than fit side by side** — S52.
 *
 * Six cells of red is six pages of the colour bank if every one is a knob, and
 * the four an operator reaches for would be buried. So the bank draws one
 * **part** and the band grows a stepper; which part is session state, so the
 * X-Touch and a second screen follow it.
 */
describe("the part stepper", () => {
  const tube = (part = 0) =>
    band(
      "Color",
      { ...programmer(), selection: [5] },
      TUBE,
      part,
    );

  it("draws one part at a time and says which", () => {
    const { parts } = tube();
    expect(screen.getByTestId("programmer-part").textContent).toBe("Part 1/6");
    expect(screen.getByTestId("encoders").children).toHaveLength(1);
    expect(screen.getByTestId("encoder-Red")).toBeTruthy();
    // Nowhere back from the first part, somewhere forward from it.
    expect(screen.getByTestId("encoder-part-prev").hasAttribute("disabled")).toBe(true);
    expect(screen.getByTestId("encoder-part-next").hasAttribute("disabled")).toBe(false);
    // And nothing has been sent: the part it is on is the one asked for.
    expect(parts).toEqual([]);
  });

  it("walks a part at a time, and the command is absolute", () => {
    const { parts } = tube(2);
    expect(screen.getByTestId("programmer-part").textContent).toBe("Part 3/6");
    expect(screen.getByTestId("encoder-Red-3")).toBeTruthy();
    fireEvent.click(screen.getByTestId("encoder-part-next"));
    // **D3**: the part is the session's, so nothing has moved here — what was
    // sent is the number, not a step.
    expect(parts).toEqual([3]);
    expect(screen.getByTestId("programmer-part").textContent).toBe("Part 3/6");
  });

  it("clamps a part the fixture cannot honour, and says so on the wire", () => {
    // The upper bound is the client's: `prism-core` does not know how deep a
    // bank's repeats go for a selection, so the band clamps for the draw and
    // sends the correcting number — which is what stops the X-Touch holding a
    // part the screen is not on.
    const { parts } = tube(99);
    expect(screen.getByTestId("programmer-part").textContent).toBe("Part 6/6");
    expect(screen.getByTestId("encoder-part-next").hasAttribute("disabled")).toBe(true);
    expect(parts).toEqual([5]);
  });
});
