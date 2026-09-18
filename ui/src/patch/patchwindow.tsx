/**
 * The Patch window: where a rig is built.
 *
 * # What it is, and what the Fixture Sheet is
 *
 * The two window types have existed since `ARCHITECTURE_SPEC.md` §6 and S27 is
 * the session that had to say what the difference is. It is the difference every
 * console makes:
 *
 * - **Patch** is the *rig*. Which fixtures exist, what they are called, which
 *   profile each instantiates, and where its channels are. It is edited, and it
 *   is the only window in this interface that changes the show's shape.
 * - **Fixture Sheet** is the *state*. What the programmer holds and what is on
 *   the cable, per fixture, live. It is watched, not edited.
 *
 * (And `DmxSheet`, added in S25, is the third of the family: the cable itself,
 * channel by channel, with no fixtures in it at all.)
 *
 * # Nothing here is state this interface holds
 *
 * The table is `patchRows(show)`. What is local is exactly one thing — **the
 * row being typed into** — and that is `ARCHITECTURE_SPEC.md` §4.2's own
 * category: a half-finished form is not something a second operator's screen
 * should be following. The moment Patch is pressed it goes to the daemon and
 * what comes back is a `ShowPatch`; the draft is dropped, not merged.
 *
 * That is the same resolution `canvas/drag.ts` wrote down for a pointer and
 * `desk/valuedrag.ts` for a fader: **ownership daemon, cadence local, and the
 * local value dropped when the gesture ends.**
 *
 * # One panel: the library and the fixture — S57, punch-list B60
 *
 * The owner's eight points, and the reason they are one rebuild rather than
 * eight corrections: they are one window, and they lean on each other.
 *
 * 1. **A fixture is one row, and its mode is chosen beside it.** The library
 *    is listed a fixture at a time (`Query::BrowseLibrary`); the mode is a menu
 *    in the form. The key a show embeds is still per mode.
 * 2. **The whole row picks**, not the first cell.
 * 3. **The list loads as it is scrolled** — `library.ts`, a page at a time.
 * 4. **Add fixture opens the library**, and the settings are part of the same
 *    panel rather than a second form under the table. Opening a patched row
 *    opens the same panel, on that fixture.
 * 5. **An overlap names the next free address** — the daemon's
 *    `PatchPreview::nextFree`, and a key that moves the fixture there.
 * 6. **A new fixture starts at the next free address** in the universe last
 *    patched into, and follows it while the mode changes, until an address is
 *    typed.
 * 7. **A fixture with no name is named after its type** — the daemon does
 *    that; the field says so in its placeholder.
 * 8. **Several at once**: a count, placed by the daemon
 *    (`PatchPreview::placements`) and sent as one `PatchFixtures`, which is one
 *    Oops.
 *
 * # Choosing embeds nothing any more
 *
 * S43 embedded a profile the moment it was picked, every time (B1), because the
 * preview could only measure a profile the show carried. Browsing a library of
 * two thousand that way would leave a profile behind in the show for every row
 * clicked. So since S57 the daemon previews **the library's copy**, and
 * `PatchFixtures` embeds it in the same step as the fixtures — which is still
 * *use the library's copy*, B1's rule, taken at the moment it is used. Changing
 * the profile of a fixture that is already patched still embeds before it
 * repatches, because `PatchFixture` patches from the show's copy.
 *
 * # The conflict is shown before it is committed, and the daemon computes it
 *
 * Every change to the draft asks `Query::PatchPreview`, and the line under the
 * form is the answer: whether the daemon would take this patch, how wide the
 * fixture is, where it would end, what it would overlap, where it would fit, and
 * — for several — where each would go. None of that is worked out here.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { JsonValue, LibraryFixture, LibraryMode, PatchPlacement, PatchPreview, Query } from "../bindings";
import { Modal } from "../chrome/modal";
import { useAsk, useSend } from "../store/hooks";
import type { PatchRow, ProfileRow } from "./patch";
import { embeddedProfiles, nextFreeFixtureId, patchRows, profileLabel, wholeNumber } from "./patch";
import type { LibraryView } from "./library";
import { EMPTY_VIEW, LibraryPager, fixtureLabel, modeLabel, modesSummary } from "./library";
import { PreviewRequester, conflictedFixtures, conflictsOf, placeText, previewText } from "./preview";

/** How close to the end of the list, in pixels, the next page is asked for. */
const LOAD_MARGIN = 96;

/** The row being typed into. Local, and dropped when it is submitted. */
interface Draft {
    /**
     * The fixture number **as typed**, which is the key everything uses once it
     * is one.
     *
     * The number fields are text since B53: a box that only ever took a valid
     * number could never be empty, so the first digit of a number could not be
     * changed. {@link numbersOf} reads them where a number is needed.
     */
    readonly id: string;
    /** The number this row started at, so a change of number is a renumber. `null` for new ones. */
    readonly wasId: number | null;
    /** The profile it started on, so a change of profile embeds first. */
    readonly wasTypeId: string | null;
    /** The name. Empty is *named after its type*, which the daemon does. */
    readonly name: string;
    /** The profile key — one mode of {@link fixture}. */
    readonly typeId: string;
    /** The library fixture the key is a mode of, when the library knows it. */
    readonly fixture: LibraryFixture | null;
    /** How many new fixtures, as typed. Only a new row has one. */
    readonly count: string;
    /** The universe, as typed. */
    readonly universe: string;
    /** The start address, as typed. */
    readonly address: string;
    /**
     * Whether the place follows the daemon's next free address — S57.
     *
     * A new row starts so, and follows it while the mode changes, because a
     * different footprint fits somewhere else. Typing an address is the
     * operator taking the place over, and from then on it is theirs.
     */
    readonly followFree: boolean;
    /** Whether the desk supplies this fixture's intensity — S43. */
    readonly softwareDimmer: boolean;
}

/** The numbers of a draft, or the name of the first field that is not one. */
type DraftNumbers =
    | { readonly id: number; readonly universe: number; readonly address: number; readonly count: number }
    | { readonly missing: string };

/** Reads a draft's number fields — B53. Asked for the preview and at Patch, never per keystroke. */
function numbersOf(draft: Draft): DraftNumbers {
    const id = wholeNumber(draft.id);
    if (id === null) {
        return { missing: "Number" };
    }
    let count = 1;
    if (draft.wasId === null) {
        const typed = wholeNumber(draft.count);
        if (typed === null || typed < 1) {
            return { missing: "Count" };
        }
        count = typed;
    }
    const universe = wholeNumber(draft.universe);
    if (universe === null) {
        return { missing: "Universe" };
    }
    const address = wholeNumber(draft.address);
    if (address === null) {
        return { missing: "Address" };
    }
    return { id, universe, address, count };
}

/** The question a draft asks, or `null` while a field is not a number yet. */
function queryOf(draft: Draft): Query | null {
    const numbers = numbersOf(draft);
    if ("missing" in numbers || draft.typeId === "") {
        return null;
    }
    return {
        t: "PatchPreview",
        id: numbers.id,
        typeId: draft.typeId,
        universe: numbers.universe,
        address: numbers.address,
        adding: draft.wasId === null ? numbers.count : 0,
    };
}

/** Two questions are the same question. */
function sameQuery(left: Query | null, right: Query | null): boolean {
    return left !== null && right !== null && JSON.stringify(left) === JSON.stringify(right);
}

/** The mode of the draft's fixture it is set to, if the library knows it. */
function modeOf(draft: Draft): LibraryMode | undefined {
    return draft.fixture?.modes.find((mode) => mode.id === draft.typeId);
}

/** The whole window. */
export function PatchWindow({ show }: { readonly show: JsonValue }) {
    const send = useSend();
    const ask = useAsk();
    const rows = useMemo(() => patchRows(show), [show]);
    const profiles = useMemo(() => embeddedProfiles(show), [show]);
    const [draft, setDraft] = useState<Draft | null>(null);
    const [conflicted, setConflicted] = useState<ReadonlySet<number>>(new Set());
    // **The universe last patched into** — S57, the owner's sixth point. What
    // this window last sent, and before that the universe of the highest
    // numbered fixture, which is the nearest thing a show says about it.
    const [lastUniverse, setLastUniverse] = useState<number | null>(null);

    // The overlaps in the show **as it stands**, which is the daemon's answer and
    // not a second opinion. Asked again whenever the show document moves, which
    // is exactly when the answer can have changed.
    useEffect(() => {
        let current = true;
        void ask({ t: "PatchConflicts" }).then((answer) => {
            if (current) {
                setConflicted(conflictedFixtures(conflictsOf(answer)));
            }
        });
        return () => {
            current = false;
        };
    }, [ask, show]);

    const edit = useCallback((row: PatchRow) => {
        setDraft({
            id: String(row.id),
            wasId: row.id,
            wasTypeId: row.typeId,
            name: row.name,
            typeId: row.typeId,
            fixture: null,
            count: "1",
            universe: String(row.universe),
            address: String(row.address),
            followFree: false,
            softwareDimmer: row.softwareDimmer,
        });
    }, []);

    const add = useCallback(() => {
        const newest = rows.at(-1);
        setDraft({
            id: String(nextFreeFixtureId(rows)),
            wasId: null,
            wasTypeId: null,
            name: "",
            typeId: "",
            fixture: null,
            count: "1",
            universe: String(lastUniverse ?? newest?.universe ?? 1),
            address: "1",
            followFree: true,
            // On, which is what makes a colour-only fixture dark at home — S43.
            // An operator whose PAR is on a dimmer pack switches it off; nobody
            // should have to switch it on to stop the rig lighting itself.
            softwareDimmer: true,
        });
    }, [lastUniverse, rows]);

    const close = useCallback(() => {
        setDraft(null);
    }, []);

    const patched = useCallback(
        (universe: number) => {
            setLastUniverse(universe);
            // **Dropped, not kept.** What the fixture is comes back as a
            // `ShowPatch`; a draft held here until the delta arrived would be
            // this interface having an opinion about the show for as long as
            // the round trip took.
            setDraft(null);
        },
        [],
    );

    return (
        <div className="patch" data-testid="patch">
            <PatchToolbar rows={rows} profiles={profiles} onAdd={add} />
            <PatchTable rows={rows} conflicted={conflicted} editing={draft?.wasId ?? null} onEdit={edit} />
            {draft === null ? null : (
                <PatchEditor
                    draft={draft}
                    profiles={profiles}
                    onChange={setDraft}
                    onPatched={patched}
                    onRemove={(id) => {
                        send({ t: "UnpatchFixture", id });
                        setDraft(null);
                    }}
                    onClose={close}
                />
            )}
        </div>
    );
}

/** What there is, and the one way to add to it. */
function PatchToolbar({
    rows,
    profiles,
    onAdd,
}: {
    readonly rows: readonly PatchRow[];
    readonly profiles: readonly ProfileRow[];
    readonly onAdd: () => void;
}) {
    return (
        <div className="patch-bar">
            <span className="patch-count" data-testid="patch-count">
                {rows.length} fixtures · {profiles.length} profiles
            </span>
            <button type="button" data-testid="patch-add" onClick={onAdd}>
                Add fixture
            </button>
        </div>
    );
}

/** The patch itself. */
function PatchTable({
    rows,
    conflicted,
    editing,
    onEdit,
}: {
    readonly rows: readonly PatchRow[];
    readonly conflicted: ReadonlySet<number>;
    readonly editing: number | null;
    readonly onEdit: (row: PatchRow) => void;
}) {
    if (rows.length === 0) {
        return <p className="window-note">Nothing is patched.</p>;
    }
    return (
        <div className="sheet-scroll">
            <table className="sheet">
                <thead>
                    <tr>
                        <th scope="col">Fx</th>
                        <th scope="col">Name</th>
                        <th scope="col">Type</th>
                        <th scope="col">Univ</th>
                        <th scope="col">Addr</th>
                        <th scope="col">Ch</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.map((row) => (
                        <tr
                            key={row.id}
                            className={rowClass(conflicted.has(row.id), editing === row.id)}
                            data-testid={`patch-row-${String(row.id)}`}
                            onClick={() => {
                                onEdit(row);
                            }}
                        >
                            <td>
                                <button type="button" className="linkish" aria-label={`Edit fixture ${String(row.id)}`}>
                                    {row.id}
                                </button>
                            </td>
                            <td>{row.name === "" ? "—" : row.name}</td>
                            <td>{row.typeName}</td>
                            <td>{row.universe}</td>
                            <td>{row.address}</td>
                            <td>{row.footprint === 0 ? "—" : row.footprint}</td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
}

/** The classes one row carries. */
function rowClass(clashes: boolean, editing: boolean): string {
    return [clashes ? "row-conflict" : "", editing ? "row-editing" : ""].filter(Boolean).join(" ");
}

/**
 * The library and the fixture being patched, in one panel — S57.
 *
 * The left half is the library, the right half is what is being patched. A new
 * row opens here from *Add fixture*; a patched row opens here from the table,
 * on the fixture it is.
 */
function PatchEditor({
    draft,
    profiles,
    onChange,
    onPatched,
    onRemove,
    onClose,
}: {
    readonly draft: Draft;
    readonly profiles: readonly ProfileRow[];
    readonly onChange: React.Dispatch<React.SetStateAction<Draft | null>>;
    readonly onPatched: (universe: number) => void;
    readonly onRemove: (id: number) => void;
    readonly onClose: () => void;
}) {
    const send = useSend();
    const ask = useAsk();
    const [answered, setAnswered] = useState<{ readonly query: Query; readonly preview: PatchPreview | null } | null>(
        null,
    );

    // What the draft *would* do, asked per keystroke. The requester drops
    // answers to drafts that have been typed over, and says which question each
    // answer is to — so the placements sent back below are this draft's.
    const requester = useRef<PreviewRequester | null>(null);
    useEffect(() => {
        const live = new PreviewRequester(ask, (preview, query) => {
            setAnswered({ query, preview });
        });
        requester.current = live;
        return () => {
            live.stop();
            requester.current = null;
        };
    }, [ask]);
    const query = useMemo(() => queryOf(draft), [draft]);
    useEffect(() => {
        if (query !== null) {
            requester.current?.request(query);
        }
    }, [query]);
    const current = answered !== null && sameQuery(answered.query, query) ? answered.preview : null;
    // What the line says: the answer to this draft, or — while that is on its
    // way — the last one, so the line does not blink on every keystroke.
    const shown = query === null ? null : (current ?? answered?.preview ?? null);

    // **A new fixture follows the next free address** — S57. When the answer to
    // this very draft says the place is somewhere else, the place moves there;
    // the next answer then says it is free, and nothing moves again.
    useEffect(() => {
        if (current === null || current.nextFree === null) {
            return;
        }
        const free = current.nextFree;
        const universe = String(free.universe);
        const address = String(free.address);
        onChange((latest) =>
            latest !== null && latest.followFree && (latest.universe !== universe || latest.address !== address)
                ? { ...latest, universe, address }
                : latest,
        );
    }, [current, onChange]);

    // The fixture a patched row is, so its mode can be changed without a search.
    // Asked once per profile key, and put on the draft only if the draft is
    // still on that key when the answer comes.
    const lookUp = draft.fixture === null ? draft.typeId : "";
    useEffect(() => {
        if (lookUp === "") {
            return;
        }
        let live = true;
        void ask({ t: "FixtureOfMode", typeId: lookUp }).then((answer) => {
            if (!live || answer === null || answer.t !== "FixtureOfMode" || answer.fixture === null) {
                return;
            }
            const fixture = answer.fixture;
            onChange((latest) =>
                latest !== null && latest.fixture === null && latest.typeId === lookUp ? { ...latest, fixture } : latest,
            );
        });
        return () => {
            live = false;
        };
    }, [ask, lookUp, onChange]);

    const pick = useCallback(
        (fixture: LibraryFixture) => {
            const same = fixture.modes.some((mode) => mode.id === draft.typeId);
            const first = fixture.modes[0];
            if (first === undefined) {
                return;
            }
            onChange({
                ...draft,
                fixture,
                typeId: same ? draft.typeId : first.id,
                // A new footprint fits somewhere else: look again from the
                // start of the universe.
                address: draft.followFree ? "1" : draft.address,
            });
        },
        [draft, onChange],
    );

    const numbers = numbersOf(draft);
    const placements = placementsToSend(draft, numbers, current);
    // Refused by **this** draft's answer: a refusal of the address typed a
    // moment ago says nothing about the one typed since.
    const refused = current !== null && !current.accepted;
    const ready = draft.typeId !== "" && !("missing" in numbers) && placements !== null && !refused;

    const apply = useCallback(() => {
        if (!ready || "missing" in numbers || placements === null) {
            return;
        }
        if (draft.wasId === null) {
            // One gesture, one command, one Oops — and the profile comes with
            // it, out of the library, in the same step.
            send({
                t: "PatchFixtures",
                typeId: draft.typeId,
                name: draft.name,
                softwareDimmer: draft.softwareDimmer,
                placements: [...placements],
            });
        } else {
            // A change of number is its own command, and it goes first: the
            // number is the key the patch is filed under (S27). A change of
            // profile embeds the library's copy before the repatch stands on it.
            if (draft.wasId !== numbers.id) {
                send({ t: "RenumberFixture", id: draft.wasId, to: numbers.id });
            }
            if (draft.typeId !== draft.wasTypeId) {
                send({ t: "EmbedFixtureType", typeId: draft.typeId });
            }
            send({
                t: "PatchFixture",
                id: numbers.id,
                name: draft.name,
                typeId: draft.typeId,
                universe: numbers.universe,
                address: numbers.address,
                softwareDimmer: draft.softwareDimmer,
            });
        }
        onPatched(numbers.universe);
    }, [draft, numbers, onPatched, placements, ready, send]);

    const title = draft.wasId === null ? "Add fixtures" : `Fixture ${String(draft.wasId)}`;
    return (
        <Modal
            title={title}
            testId="patch-editor"
            size="full"
            onClose={onClose}
            footer={
                <>
                    <button type="submit" form="patch-form" disabled={!ready} data-testid="draft-apply">
                        {draft.wasId === null ? "Patch" : "Apply"}
                    </button>
                    {draft.wasId === null ? null : (
                        <button
                            type="button"
                            data-testid="draft-remove"
                            onClick={() => {
                                if (draft.wasId !== null) {
                                    onRemove(draft.wasId);
                                }
                            }}
                        >
                            Unpatch
                        </button>
                    )}
                </>
            }
        >
            <div className="patch-editor">
                <LibraryBrowser chosen={draft.fixture} typeId={draft.typeId} onPick={pick} />
                <PatchForm
                    draft={draft}
                    profiles={profiles}
                    numbers={numbers}
                    preview={shown}
                    onChange={onChange}
                    onApply={apply}
                />
            </div>
        </Modal>
    );
}

/**
 * The placements a Patch would send, or `null` while they are not known.
 *
 * One new fixture is the place typed, which is no arithmetic at all — and a form
 * that waited on the daemon for it would be a form a slow answer locks. Several
 * are **the daemon's**, and only its answer to this very draft will do: the
 * answer to the previous address would put them somewhere else.
 */
function placementsToSend(
    draft: Draft,
    numbers: DraftNumbers,
    current: PatchPreview | null,
): readonly PatchPlacement[] | null {
    if ("missing" in numbers) {
        return null;
    }
    if (draft.wasId !== null || numbers.count === 1) {
        return [{ id: numbers.id, universe: numbers.universe, address: numbers.address }];
    }
    if (current === null || current.placements.length !== numbers.count) {
        return null;
    }
    return current.placements;
}

/**
 * The library, one row per fixture, loaded as it is scrolled — S57.
 *
 * # What is local, and what is the daemon's
 *
 * The text in the box is local; **what matches is the daemon's answer**, a page
 * at a time, because the library is two thousand profiles and no client holds
 * it. See `library.ts`.
 *
 * # The whole row picks
 *
 * The owner's second point. The row is clickable end to end, and the first cell
 * still holds a `<button>`, because a row that is only clickable is a row the
 * keyboard cannot reach (S43). Its click is the row's click — it bubbles — so
 * nothing is handled twice.
 */
function LibraryBrowser({
    chosen,
    typeId,
    onPick,
}: {
    readonly chosen: LibraryFixture | null;
    readonly typeId: string;
    readonly onPick: (fixture: LibraryFixture) => void;
}) {
    const ask = useAsk();
    const [text, setText] = useState("");
    const [view, setView] = useState<LibraryView>(EMPTY_VIEW);
    const pager = useRef<LibraryPager | null>(null);
    const scroller = useRef<HTMLDivElement | null>(null);

    useEffect(() => {
        const live = new LibraryPager(ask, setView);
        pager.current = live;
        return () => {
            live.stop();
            pager.current = null;
        };
    }, [ask]);
    useEffect(() => {
        pager.current?.search(text);
    }, [text]);

    // A page that does not fill the list cannot be scrolled to its end, so the
    // next one is asked for straight away. Only where there is a size to
    // measure: a list with no height is not one anybody is looking at.
    useEffect(() => {
        const element = scroller.current;
        if (element === null || element.clientHeight === 0 || view.loading) {
            return;
        }
        if (element.scrollHeight <= element.clientHeight + LOAD_MARGIN) {
            pager.current?.more();
        }
    }, [view]);

    const onScroll = useCallback(() => {
        const element = scroller.current;
        if (element !== null && element.scrollTop + element.clientHeight >= element.scrollHeight - LOAD_MARGIN) {
            pager.current?.more();
        }
    }, []);

    const more = view.matched === null || view.fixtures.length < view.matched;
    return (
        <section className="library-pane" aria-label="Fixture library">
            <div className="library-bar">
                <label className="library-search-label">
                    Search
                    <input
                        data-testid="library-search"
                        value={text}
                        placeholder="manufacturer, fixture or mode"
                        onChange={(event) => {
                            setText(event.target.value);
                        }}
                    />
                </label>
                <span className="library-count" data-testid="library-count">
                    {view.matched === null ? "…" : `${String(view.matched)} of ${String(view.total ?? 0)} fixtures`}
                </span>
            </div>
            {view.matched === 0 ? (
                <p className="window-note" data-testid="library-empty">
                    Nothing in the library matches that.
                </p>
            ) : (
                <div className="library-scroll" data-testid="library-scroll" ref={scroller} onScroll={onScroll}>
                    <table className="sheet library-table" data-testid="library-matches">
                        <thead>
                            <tr>
                                <th scope="col">Manufacturer</th>
                                <th scope="col">Fixture</th>
                                <th scope="col">Modes</th>
                                <th scope="col">Source</th>
                            </tr>
                        </thead>
                        <tbody>
                            {view.fixtures.map((fixture) => {
                                const key = fixture.modes[0]?.id ?? fixtureLabel(fixture);
                                const selected =
                                    chosen === null
                                        ? fixture.modes.some((mode) => mode.id === typeId)
                                        : fixtureLabel(chosen) === fixtureLabel(fixture) && chosen.own === fixture.own;
                                return (
                                    <tr
                                        key={key}
                                        data-testid={`library-row-${key}`}
                                        className={selected ? "library-row row-selected" : "library-row"}
                                        onClick={() => {
                                            onPick(fixture);
                                        }}
                                    >
                                        <td>
                                            <button type="button" className="linkish" data-testid={`library-${key}`}>
                                                {fixture.manufacturer === "" ? "—" : fixture.manufacturer}
                                            </button>
                                        </td>
                                        <td>{fixture.name}</td>
                                        <td title={modesSummary(fixture)}>{modesSummary(fixture)}</td>
                                        {/*
                                          **B43.** Whose profile this is. A venue's own
                                          may deliberately carry a library key in order
                                          to *correct* one, so the key cannot answer it
                                          and the daemon says so on the fixture instead.
                                        */}
                                        <td
                                            className="library-source"
                                            data-testid={`library-source-${key}`}
                                            data-own={fixture.own ? "yes" : "no"}
                                        >
                                            {fixture.own ? "yours" : "library"}
                                        </td>
                                    </tr>
                                );
                            })}
                        </tbody>
                    </table>
                    {more ? (
                        <button
                            type="button"
                            className="library-more"
                            data-testid="library-more"
                            disabled={view.loading}
                            onClick={() => {
                                pager.current?.more();
                            }}
                        >
                            {view.loading ? "Loading…" : "More"}
                        </button>
                    ) : null}
                </div>
            )}
        </section>
    );
}

/** The fixture being patched, and what the daemon says it would do. */
function PatchForm({
    draft,
    profiles,
    numbers,
    preview,
    onChange,
    onApply,
}: {
    readonly draft: Draft;
    readonly profiles: readonly ProfileRow[];
    readonly numbers: DraftNumbers;
    readonly preview: PatchPreview | null;
    readonly onChange: (draft: Draft) => void;
    readonly onApply: () => void;
}) {
    const embedded = profiles.find((profile) => profile.id === draft.typeId);
    const mode = modeOf(draft);
    const typeName = draft.fixture?.name ?? embedded?.name ?? "";
    const hasIntensity = mode?.hasIntensity ?? embedded?.hasIntensity;
    const clashes = preview !== null && preview.accepted && preview.conflicts.length > 0;
    const adding = draft.wasId === null && !("missing" in numbers) ? numbers.count : 0;
    const free = preview?.nextFree ?? null;
    const moved =
        free !== null && (String(free.universe) !== draft.universe || String(free.address) !== draft.address);
    return (
        <form
            className="patch-form"
            id="patch-form"
            data-testid="patch-form"
            onSubmit={(event) => {
                event.preventDefault();
                onApply();
            }}
        >
            <p className="patch-type" data-testid="draft-type">
                {draft.fixture !== null
                    ? fixtureLabel(draft.fixture)
                    : embedded !== undefined
                      ? profileLabel(embedded)
                      : draft.typeId === ""
                        ? "Choose a fixture from the library"
                        : draft.typeId}
            </p>
            <div className="patch-fields">
                {draft.fixture === null ? null : (
                    <label>
                        Mode
                        <select
                            data-testid="draft-mode"
                            value={draft.typeId}
                            onChange={(event) => {
                                onChange({
                                    ...draft,
                                    typeId: event.target.value,
                                    address: draft.followFree ? "1" : draft.address,
                                });
                            }}
                        >
                            {draft.fixture.modes.map((option) => (
                                <option key={option.id} value={option.id}>
                                    {modeLabel(option)}
                                </option>
                            ))}
                        </select>
                    </label>
                )}
                {draft.wasId === null ? (
                    <NumberField
                        label="Count"
                        testId="draft-count"
                        value={draft.count}
                        onChange={(count) => {
                            onChange({ ...draft, count });
                        }}
                    />
                ) : null}
                <NumberField
                    label={draft.wasId === null && draft.count.trim() !== "1" ? "First number" : "Number"}
                    testId="draft-id"
                    value={draft.id}
                    onChange={(id) => {
                        onChange({ ...draft, id });
                    }}
                />
                <label>
                    Name
                    <input
                        data-testid="draft-name"
                        value={draft.name}
                        // **Named after its type** when left empty — B60's
                        // seventh point, and the daemon does the naming.
                        placeholder={typeName}
                        onChange={(event) => {
                            onChange({ ...draft, name: event.target.value });
                        }}
                    />
                </label>
                <NumberField
                    label="Universe"
                    testId="draft-universe"
                    value={draft.universe}
                    onChange={(universe) => {
                        // Still following the next free address, now in this
                        // universe: from its start.
                        onChange({ ...draft, universe, address: draft.followFree ? "1" : draft.address });
                    }}
                />
                <NumberField
                    label="Address"
                    testId="draft-address"
                    value={draft.address}
                    onChange={(address) => {
                        // Typed, so the place is the operator's from now on.
                        onChange({ ...draft, address, followFree: false });
                    }}
                />
            </div>
            <DimmerField
                hasIntensity={hasIntensity}
                on={draft.softwareDimmer}
                onChange={(softwareDimmer) => {
                    onChange({ ...draft, softwareDimmer });
                }}
            />
            <p
                className={`patch-preview ${"missing" in numbers ? "preview-refused" : previewClass(preview, clashes)}`}
                data-testid="patch-preview"
                role="status"
            >
                {"missing" in numbers
                    ? `${numbers.missing} has to be a whole number.`
                    : draft.typeId === ""
                      ? "Pick a fixture in the list."
                      : previewText(preview, numbers.id, adding)}
            </p>
            {moved && free !== null && preview?.accepted === true ? (
                <button
                    type="button"
                    className="patch-next-free"
                    data-testid="draft-next-free"
                    onClick={() => {
                        onChange({
                            ...draft,
                            universe: String(free.universe),
                            address: String(free.address),
                            followFree: false,
                        });
                    }}
                >
                    Move to {placeText(free)}
                </button>
            ) : null}
        </form>
    );
}

/**
 * The desk-supplied intensity, and the one row that decides whether it is drawn
 * — S43.
 *
 * Punch-list B1 gave colour channels a home value of **full**, so a fixture with
 * no dimmer — an RGBW PAR is four colour channels and nothing else — came up
 * white the moment the daemon started. The desk supplies the missing channel,
 * resting at nought, and the operator can switch it off for a PAR on a dimmer
 * pack. **Nothing is drawn for a profile with an intensity of its own**, or for
 * one this form cannot see yet: a checkbox that did nothing would be worse than
 * no checkbox.
 */
function DimmerField({
    hasIntensity,
    on,
    onChange,
}: {
    readonly hasIntensity: boolean | undefined;
    readonly on: boolean;
    readonly onChange: (on: boolean) => void;
}) {
    if (hasIntensity !== false) {
        return null;
    }
    return (
        <label className="patch-check">
            <input
                type="checkbox"
                data-testid="draft-software-dimmer"
                checked={on}
                onChange={(event) => {
                    onChange(event.target.checked);
                }}
            />
            <span>
                Desk dimmer
                <small className="patch-check-note">
                    This profile has no intensity channel. With this on, the desk gives it one that
                    scales its colour — so it is dark until something asks for it. Switch it off for a
                    fixture on a dimmer pack.
                </small>
            </span>
        </label>
    );
}

/** Which of the three things the preview line is saying. */
function previewClass(preview: PatchPreview | null, clashes: boolean): string {
    if (preview === null) {
        return "preview-waiting";
    }
    if (!preview.accepted) {
        return "preview-refused";
    }
    return clashes ? "preview-overlap" : "preview-clear";
}

/**
 * A whole number, typed.
 *
 * **Held as text, whatever is typed** — B53. What the text means is asked where
 * a number is needed — {@link numbersOf} — and a box that is not a number yet
 * is *no answer yet*, which the preview line says and the Patch key refuses.
 */
function NumberField({
    label,
    testId,
    value,
    onChange,
}: {
    readonly label: string;
    readonly testId: string;
    readonly value: string;
    readonly onChange: (value: string) => void;
}) {
    return (
        <label>
            {label}
            <input
                data-testid={testId}
                inputMode="numeric"
                value={value}
                onChange={(event) => {
                    onChange(event.target.value);
                }}
            />
        </label>
    );
}
