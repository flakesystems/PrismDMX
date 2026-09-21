/**
 * The library pager — S57, punch-list **B60**.
 *
 * The rules worth holding on their own, without a renderer: a newer search
 * drops the answers to an older one, a page is asked for once, and nothing is
 * asked past the end.
 */

import { describe, expect, it } from "vitest";

import type { Answer, LibraryFixture, LibraryMode, Query } from "../bindings";
import type { LibraryView } from "./library";
import { LibraryPager, PAGE_SIZE, fixtureLabel, modeLabel, modesSummary, physicalSummary } from "./library";

/** A daemon whose answers are given by hand, in any order. */
function daemon(): {
  ask: (query: Query) => Promise<Answer | null>;
  asked: Query[];
  answer: (index: number, value: Answer | null) => void;
} {
  const resolvers: ((value: Answer | null) => void)[] = [];
  const asked: Query[] = [];
  return {
    asked,
    ask: (query) => {
      asked.push(query);
      return new Promise<Answer | null>((resolve) => resolvers.push(resolve));
    },
    answer: (index, value) => {
      resolvers[index]?.(value);
    },
  };
}

function fixture(name: string): LibraryFixture {
  return {
    manufacturer: "Maker",
    name,
    own: false,
    gdtf: false,
    modes: [{ id: `maker/${name}/1ch`, mode: "1ch", footprint: 1, hasIntensity: true, beams: 0 }],
  };
}

function page(names: readonly string[], matched: number): Answer {
  return { t: "LibraryFixtures", fixtures: names.map(fixture), matched, total: 99 };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("the library, a page at a time", () => {
  it("asks for the first page, then the next one from where the list stops", async () => {
    const remote = daemon();
    const views: LibraryView[] = [];
    const pager = new LibraryPager(remote.ask, (view) => views.push(view));
    pager.search("");
    expect(remote.asked).toEqual([{ t: "BrowseLibrary", text: "", offset: 0, limit: PAGE_SIZE }]);
    // Nothing more while the first is on its way.
    pager.more();
    expect(remote.asked).toHaveLength(1);

    remote.answer(0, page(["A", "B"], 3));
    await settle();
    expect(pager.view.fixtures.map((entry) => entry.name)).toEqual(["A", "B"]);
    expect(pager.hasMore).toBe(true);

    pager.more();
    expect(remote.asked.at(-1)).toEqual({ t: "BrowseLibrary", text: "", offset: 2, limit: PAGE_SIZE });
    remote.answer(1, page(["C"], 3));
    await settle();
    expect(pager.view.fixtures.map((entry) => entry.name)).toEqual(["A", "B", "C"]);
    // At the end: nothing further is asked.
    expect(pager.hasMore).toBe(false);
    pager.more();
    expect(remote.asked).toHaveLength(2);
  });

  it("drops the answer to a search that has been typed over", async () => {
    const remote = daemon();
    const pager = new LibraryPager(remote.ask, () => undefined);
    pager.search("rob");
    pager.search("robe");
    remote.answer(1, page(["Robe"], 1));
    remote.answer(0, page(["Rob", "Robert"], 2));
    await settle();
    expect(pager.view.fixtures.map((entry) => entry.name)).toEqual(["Robe"]);
    expect(pager.view.matched).toBe(1);
  });

  it("stops saying it is loading when the daemon does not answer, so it can ask again", async () => {
    const remote = daemon();
    const pager = new LibraryPager(remote.ask, () => undefined);
    pager.search("");
    remote.answer(0, null);
    await settle();
    expect(pager.view.loading).toBe(false);
    expect(pager.view.fixtures).toEqual([]);
  });

  it("hears nothing once it is stopped", async () => {
    const remote = daemon();
    const views: LibraryView[] = [];
    const pager = new LibraryPager(remote.ask, (view) => views.push(view));
    pager.search("");
    const seen = views.length;
    pager.stop();
    remote.answer(0, page(["A"], 1));
    await settle();
    expect(views).toHaveLength(seen);
  });
});

describe("how the library names things", () => {
  it("reads a fixture by its make and model, and a mode by its name and width", () => {
    const wash: LibraryFixture = {
      manufacturer: "Robe",
      name: "Wash 7Q5",
      own: false,
      gdtf: false,
      modes: [
        { id: "robe/wash-7q5/4ch", mode: "4ch", footprint: 4, hasIntensity: true, beams: 0 },
        { id: "robe/wash-7q5/x", mode: "", footprint: 2, hasIntensity: true, beams: 0 },
      ],
    };
    expect(fixtureLabel(wash)).toBe("Robe Wash 7Q5");
    expect(fixtureLabel({ ...wash, manufacturer: "" })).toBe("Wash 7Q5");
    expect(wash.modes.map(modeLabel)).toEqual(["4ch", "2 ch"]);
    expect(modeLabel({ id: "x", mode: "Extended", footprint: 16, hasIntensity: true, beams: 0 })).toBe(
      "Extended · 16 ch",
    );
    // A name that says a different width than the profile has is shown with both.
    expect(modeLabel({ id: "x", mode: "8ch", footprint: 9, hasIntensity: true, beams: 0 })).toBe("8ch · 9 ch");
    expect(modesSummary(wash)).toBe("4ch · 2 ch");
  });

  /** **S60.** What a mode carries besides its channels. */
  it("says what a GDTF profile carries, and says nothing at all for one that carries none", () => {
    const mode = (beams: number): LibraryMode => ({
      id: "x",
      mode: "",
      footprint: 4,
      hasIntensity: true,
      beams,
    });
    // Nought is an Open Fixture Library profile or one of the desk's four
    // generics — the line is not drawn at all rather than drawn saying *no
    // beams*, which would be noise on four fifths of the library.
    expect(physicalSummary(mode(0))).toBe("");
    expect(physicalSummary(undefined)).toBe("");
    expect(physicalSummary(mode(1))).toBe("3D model · 1 beam");
    expect(physicalSummary(mode(8))).toBe("3D model · 8 beams");
  });
});
