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
 * The table is `patchRows(show)`. The profile menu is `embeddedProfiles(show)`
 * and the desk's own library out of the snapshot. What is local is exactly one
 * thing — **the row being typed into** — and that is `ARCHITECTURE_SPEC.md`
 * §4.2's own category: a half-finished form is not something a second operator's
 * screen should be following. The moment Apply is pressed it goes to the daemon
 * and what comes back is a `ShowPatch`; the draft is dropped, not merged.
 *
 * That is the same resolution `canvas/drag.ts` wrote down for a pointer and
 * `desk/valuedrag.ts` for a fader: **ownership daemon, cadence local, and the
 * local value dropped when the gesture ends.**
 *
 * # One list of profiles, not two — S43, punch-list B19
 *
 * There used to be two: a search over the desk's library, which *embedded* a
 * profile into the show, and a menu of what had been embedded, which was the
 * only thing the Type field would offer. So patching a lamp was two gestures in
 * two places, and the first one had no visible effect except to unlock the
 * second. The owner's entry says it in one line: a fixture should be patchable
 * straight out of the library.
 *
 * It is now **one field**, and it is in the form, where the profile is actually
 * being chosen. It searches the whole library; what the show already carries is
 * marked rather than hidden, because a profile an operator has just used is the
 * one they are most likely to want again.
 *
 * **Choosing embeds — every time, not only the first time.** `EmbedFixtureType`
 * goes the moment a profile is picked, not when Apply is pressed, and it goes
 * even when the show already carries that key.
 *
 * Going at all is a decision rather than a shortcut: the line under the form is
 * `Query::PatchPreview`, the daemon's own answer about the *show*, and a profile
 * the show does not carry previews as a refusal with a footprint of zero.
 * Waiting until Apply would mean either showing that refusal for a patch that is
 * going to work, or having this client decide the refusal does not count — which
 * is **D3** exactly.
 *
 * Going *every* time is the correction to the first attempt, and the owner found
 * it: after B1 gave colour channels a home value of full, a show patched before
 * that fix went on showing its colours at nought, and re-patching did nothing at
 * all. The reason was here — the pick skipped the embed when the key was already
 * present, so the show's **stale copy of the profile stayed for ever** and no
 * gesture in this interface could replace it. A show embeds its profiles (S11)
 * so that it opens the same on a desk with a different library; what it must not
 * do is make the copy unreachable.
 *
 * So picking a profile out of the library means *use the library's copy of it*,
 * which is what the words say and what an operator expects. The daemon refuses
 * the replacement if it would break a patch that is already standing on it
 * (`ShowError::TypeChangeBreaksPatch`), which is the guard that makes it safe.
 *
 * What it costs is an orphan: cancel the form after picking a profile and the
 * show keeps a profile nothing is patched to. That is one Oops away, it is
 * invisible everywhere except the count, and it weighs nothing in a file — a
 * fair price for a preview that is the daemon's and not a guess.
 *
 * # The conflict is shown before it is committed, and the daemon computes it
 *
 * Every change to the draft asks `Query::PatchPreview`, and the line under the
 * form is the answer: whether the daemon would take this patch, how wide the
 * fixture is, where it would end, and what it would overlap. None of that is
 * worked out here — see `patch.ts` for why not.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { JsonValue, LibraryEntry, PatchPreview } from "../bindings";
import { Modal } from "../chrome/modal";
import { useAsk, useDesk, useSend } from "../store/hooks";
import type { DeskState } from "../store/desk";
import type { PatchRow, ProfileRow } from "./patch";
import { embeddedProfiles, nextFreeFixtureId, patchRows, profileLabel } from "./patch";
import { PreviewRequester, conflictedFixtures, conflictsOf, isAcceptable, previewText } from "./preview";

const selectLibrarySize = (state: DeskState): number | null => state.fixtureLibrary;

/**
 * How many library matches to ask for.
 *
 * **Sixty since the library became a panel** — S43, B23. It was twenty-five,
 * which is what a dropdown four rows high can be scrolled through; a table with
 * room shows twenty at a time and the point of the number is to keep a search
 * that matches half the library from being a dump.
 */
const SEARCH_LIMIT = 60;

/** The row being typed into. Local, and dropped when it is submitted. */
interface Draft {
    /** The fixture number as it stands now, which is the key everything uses. */
    readonly id: number;
    /** The number this row started at, so a change of number is a renumber. */
    readonly wasId: number | null;
    /** The name. */
    readonly name: string;
    /** The profile key. */
    readonly typeId: string;
    /** The universe. */
    readonly universe: number;
    /** The start address. */
    readonly address: number;
    /** Whether the desk supplies this fixture's intensity — S43. */
    readonly softwareDimmer: boolean;
}

/** The whole window. */
export function PatchWindow({ show }: { readonly show: JsonValue }) {
    const send = useSend();
    const ask = useAsk();
    const librarySize = useDesk(selectLibrarySize);
    const rows = useMemo(() => patchRows(show), [show]);
    const profiles = useMemo(() => embeddedProfiles(show), [show]);
    const [draft, setDraft] = useState<Draft | null>(null);
    const [preview, setPreview] = useState<PatchPreview | null>(null);
    const [conflicted, setConflicted] = useState<ReadonlySet<number>>(new Set());

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

    // And what the row being typed into *would* do, asked per keystroke. The
    // requester drops answers to drafts that have been typed over.
    const requester = useRef<PreviewRequester | null>(null);
    useEffect(() => {
        const live = new PreviewRequester(ask, setPreview);
        requester.current = live;
        return () => {
            live.stop();
            requester.current = null;
        };
    }, [ask]);
    useEffect(() => {
        if (draft === null) {
            setPreview(null);
            return;
        }
        requester.current?.request({
            t: "PatchPreview",
            id: draft.id,
            typeId: draft.typeId,
            universe: draft.universe,
            address: draft.address,
        });
    }, [draft]);

    const edit = useCallback((row: PatchRow) => {
        setDraft({
            id: row.id,
            wasId: row.id,
            name: row.name,
            typeId: row.typeId,
            universe: row.universe,
            address: row.address,
            softwareDimmer: row.softwareDimmer,
        });
    }, []);

    // The profile the show used last is the one the new row starts on, and an
    // empty show starts on none: the field is a search, so *nothing chosen yet*
    // is an ordinary state now rather than a window that cannot be used.
    const add = useCallback(() => {
        const first = profiles[0];
        setDraft({
            id: nextFreeFixtureId(rows),
            wasId: null,
            name: "",
            typeId: first?.id ?? "",
            universe: 1,
            address: 1,
            // On, which is what makes a colour-only fixture dark at home — S43.
            // An operator whose PAR is on a dimmer pack switches it off; nobody
            // should have to switch it on to stop the rig lighting itself.
            softwareDimmer: true,
        });
    }, [profiles, rows]);

    const apply = useCallback(() => {
        if (draft === null) {
            return;
        }
        // A change of number is its own command, and it goes first: the number is
        // the key the patch is filed under, so an unpatch-and-patch pair would
        // leave the rig without that fixture in between — which is why
        // `RenumberFixture` exists at all. Both travel on one ordered channel.
        if (draft.wasId !== null && draft.wasId !== draft.id) {
            send({ t: "RenumberFixture", id: draft.wasId, to: draft.id });
        }
        send({
            t: "PatchFixture",
            id: draft.id,
            name: draft.name,
            typeId: draft.typeId,
            universe: draft.universe,
            address: draft.address,
            softwareDimmer: draft.softwareDimmer,
        });
        // **Dropped, not kept.** What the fixture is comes back as a `ShowPatch`;
        // a draft held here until the delta arrived would be this interface having
        // an opinion about the show for as long as the round trip took.
        setDraft(null);
    }, [draft, send]);

    const remove = useCallback(
        (id: number) => {
            send({ t: "UnpatchFixture", id });
            setDraft(null);
        },
        [send],
    );

    const embed = useCallback(
        (typeId: string) => {
            send({ t: "EmbedFixtureType", typeId });
        },
        [send],
    );

    return (
        <div className="patch" data-testid="patch">
            <PatchToolbar rows={rows} profiles={profiles} onAdd={add} />
            <PatchTable rows={rows} conflicted={conflicted} editing={draft?.wasId ?? null} onEdit={edit} />
            {draft === null ? null : (
                <PatchForm
                    draft={draft}
                    profiles={profiles}
                    librarySize={librarySize}
                    preview={preview}
                    onChange={setDraft}
                    onEmbed={embed}
                    onApply={apply}
                    onRemove={remove}
                    onCancel={() => {
                        setDraft(null);
                    }}
                />
            )}
        </div>
    );
}

/**
 * What there is, and the one way to add to it.
 *
 * **The library search left this bar in S43** (B19) for the form, where the
 * profile is chosen. What is left is the count and the button, and the button is
 * no longer disabled on a show with no profiles: a rig now starts by opening a
 * row and searching, rather than by embedding something first and only then
 * being allowed to open one.
 */
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
            <button type="button" onClick={onAdd}>
                Add fixture
            </button>
        </div>
    );
}

/**
 * The Type field: what the row is set to, and the key that opens the library.
 *
 * **S43, B19 and then B23.** This was `LibrarySearch` in the toolbar, and it
 * *embedded* a profile; beside it, in the form, a `<select>` offered what had
 * been embedded. B19 made it one field. B23 is the owner's next reading of that
 * field: *das Fixture Auswahl Feld im Patch ist sehr unübersichtlich* — and it
 * was. What B19 left was a text box with a dropdown under it, four rows of
 * `Manufacturer · Name · Mode · n ch` run together on one line each, floating
 * over the patch table it was covering.
 *
 * A library of two thousand profiles is a **table**, and a table needs room. So
 * the field is now a reading and a key, and the key opens
 * {@link LibraryPicker} — a panel with columns, which is the whole of the
 * owner's request.
 */
function TypeField({
    typeId,
    profiles,
    librarySize,
    onPick,
}: {
    readonly typeId: string;
    readonly profiles: readonly ProfileRow[];
    readonly librarySize: number | null;
    readonly onPick: (entry: { readonly id: string }) => void;
}) {
    const [open, setOpen] = useState(false);
    const chosen = profiles.find((profile) => profile.id === typeId);
    return (
        <div className="patch-type">
            <span className="patch-type-label">Type</span>
            {/* What is chosen, in words. Full ink against the dimmed label
                beside it, because it is an answer and not a prompt. */}
            <p className="patch-type-chosen" data-testid="draft-type">
                {chosen === undefined ? (typeId === "" ? "No profile chosen" : typeId) : profileLabel(chosen)}
            </p>
            <button
                type="button"
                data-testid="library-open"
                onClick={() => {
                    setOpen(true);
                }}
            >
                {typeId === "" ? "Choose a profile…" : "Change…"}
            </button>
            {open ? (
                <LibraryPicker
                    profiles={profiles}
                    librarySize={librarySize}
                    chosen={typeId}
                    onClose={() => {
                        setOpen(false);
                    }}
                    onPick={(entry) => {
                        onPick(entry);
                        setOpen(false);
                    }}
                />
            ) : null}
        </div>
    );
}

/**
 * The library, as a table with columns — S43, B23.
 *
 * # What is local, and what is the daemon's
 *
 * The text in the box is local; **what matches is the daemon's answer**, asked
 * for again on every keystroke, because the library is two thousand profiles and
 * no client holds it (`LibraryEntry` says so in its own documentation). A
 * profile already in the show is marked rather than hidden — being in the show
 * is the normal case for the second lamp of a rig, not a reason to grey the row
 * out — and picking it **re-reads it from the library**, which is how a show
 * patched before a library fix is brought up to date. The last column says so,
 * because *already in this show* on its own read as *nothing will happen*, and
 * that was true and was the fault B1 finally turned out to be.
 *
 * # Why the columns are what they are
 *
 * Manufacturer, fixture, mode and channel count, in that order, because that is
 * the order an operator narrows: they know the make, then the model, then which
 * of its modes the lamp is switched to — and the channel count is how they
 * check they picked the right one, since a 8-channel and a 15-channel mode of
 * the same fixture look identical in a run-together line and are not
 * interchangeable in a rig.
 *
 * The rows are `<tr>` with a `<button>` in the first cell rather than a clickable
 * row: a row that is only clickable is a row the keyboard cannot reach, and S43
 * asks that a full pass with the keyboard reaches every control.
 */
function LibraryPicker({
    profiles,
    librarySize,
    chosen,
    onPick,
    onClose,
}: {
    readonly profiles: readonly ProfileRow[];
    readonly librarySize: number | null;
    /** What the row is set to now, marked in the list. */
    readonly chosen: string;
    readonly onPick: (entry: { readonly id: string }) => void;
    readonly onClose: () => void;
}) {
    const ask = useAsk();
    const [text, setText] = useState("");
    const [matches, setMatches] = useState<readonly LibraryEntry[]>([]);
    const [asked, setAsked] = useState(false);
    const embedded = useMemo(() => new Set(profiles.map((profile) => profile.id)), [profiles]);

    useEffect(() => {
        let current = true;
        void ask({ t: "SearchLibrary", text, limit: SEARCH_LIMIT }).then((answer) => {
            if (current && answer !== null && answer.t === "LibraryMatches") {
                setMatches(answer.matches);
                setAsked(true);
            }
        });
        return () => {
            // An answer to a search that has been typed over is dropped, exactly
            // as a preview's is: drawn, it would be a list of the *previous*
            // word.
            current = false;
        };
    }, [ask, text]);

    return (
        <Modal title="Fixture library" testId="library-modal" size="wide" onClose={onClose}>
            <div className="library-bar">
                <label className="library-search-label">
                    Search
                    <input
                        data-testid="library-search"
                        value={text}
                        placeholder="manufacturer, name or mode"
                        onChange={(event) => {
                            setText(event.target.value);
                        }}
                    />
                </label>
                <span className="library-count" data-testid="library-count">
                    {matches.length} shown
                    {librarySize === null ? "" : ` of ${String(librarySize)} profiles`}
                </span>
            </div>
            {asked && matches.length === 0 ? (
                <p className="window-note" data-testid="library-empty">
                    Nothing in the library matches that.
                </p>
            ) : (
                <table className="sheet library-table" data-testid="library-matches">
                    <thead>
                        <tr>
                            <th scope="col">Manufacturer</th>
                            <th scope="col">Fixture</th>
                            <th scope="col">Mode</th>
                            <th scope="col">Ch</th>
                            <th scope="col">In this show</th>
                        </tr>
                    </thead>
                    <tbody>
                        {matches.map((entry) => (
                            <tr
                                key={entry.id}
                                data-testid={`library-row-${entry.id}`}
                                className={entry.id === chosen ? "row-selected" : undefined}
                            >
                                <td>
                                    <button
                                        type="button"
                                        className="linkish"
                                        data-testid={`library-${entry.id}`}
                                        onClick={() => {
                                            onPick(entry);
                                        }}
                                    >
                                        {entry.manufacturer === "" ? "—" : entry.manufacturer}
                                    </button>
                                </td>
                                <td>{entry.name === "" ? entry.id : entry.name}</td>
                                <td>{entry.mode === "" ? "—" : entry.mode}</td>
                                <td>{entry.footprint}</td>
                                <td className="library-held">
                                    {embedded.has(entry.id) ? "yes — picking re-reads it" : ""}
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </Modal>
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

/** The row being typed into, and what the daemon says it would do. */
function PatchForm({
    draft,
    profiles,
    librarySize,
    preview,
    onChange,
    onEmbed,
    onApply,
    onRemove,
    onCancel,
}: {
    readonly draft: Draft;
    readonly profiles: readonly ProfileRow[];
    readonly librarySize: number | null;
    readonly preview: PatchPreview | null;
    readonly onChange: (draft: Draft) => void;
    readonly onEmbed: (typeId: string) => void;
    readonly onApply: () => void;
    readonly onRemove: (id: number) => void;
    readonly onCancel: () => void;
}) {
    const clashes = preview !== null && preview.accepted && preview.conflicts.length > 0;
    return (
        <form
            className="patch-form"
            data-testid="patch-form"
            onSubmit={(event) => {
                event.preventDefault();
                onApply();
            }}
        >
            <fieldset>
                <legend>{draft.wasId === null ? "New fixture" : `Fixture ${String(draft.wasId)}`}</legend>
                <NumberField
                    label="Number"
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
                        onChange={(event) => {
                            onChange({ ...draft, name: event.target.value });
                        }}
                    />
                </label>
                <TypeField
                    typeId={draft.typeId}
                    profiles={profiles}
                    librarySize={librarySize}
                    onPick={(entry) => {
                        // The show takes the library's copy at the moment the
                        // profile is chosen — **every time**, so a copy embedded
                        // before a library fix is replaced rather than kept for
                        // ever. See the module documentation. Both commands go on
                        // one ordered channel, so the preview that follows is
                        // asked of a show that already carries it.
                        onEmbed(entry.id);
                        onChange({ ...draft, typeId: entry.id });
                    }}
                />
                <NumberField
                    label="Universe"
                    testId="draft-universe"
                    value={draft.universe}
                    onChange={(universe) => {
                        onChange({ ...draft, universe });
                    }}
                />
                <NumberField
                    label="Address"
                    testId="draft-address"
                    value={draft.address}
                    onChange={(address) => {
                        onChange({ ...draft, address });
                    }}
                />
                <DimmerField
                    profile={profiles.find((profile) => profile.id === draft.typeId)}
                    on={draft.softwareDimmer}
                    onChange={(softwareDimmer) => {
                        onChange({ ...draft, softwareDimmer });
                    }}
                />
            </fieldset>
            <p
                className={`patch-preview ${previewClass(preview, clashes)}`}
                data-testid="patch-preview"
                role="status"
            >
                {previewText(preview, draft.id)}
            </p>
            <div className="patch-actions">
                <button type="submit" disabled={!isAcceptable(preview)} data-testid="draft-apply">
                    {draft.wasId === null ? "Patch" : "Apply"}
                </button>
                {draft.wasId === null ? null : (
                    <button
                        type="button"
                        data-testid="draft-remove"
                        onClick={() => {
                            onRemove(draft.wasId ?? draft.id);
                        }}
                    >
                        Unpatch
                    </button>
                )}
                <button type="button" onClick={onCancel} data-testid="draft-cancel">
                    Cancel
                </button>
            </div>
        </form>
    );
}

/**
 * The desk-supplied intensity, and the one row that decides whether it is drawn
 * — S43.
 *
 * # Why the switch exists, and why it is here
 *
 * Punch-list B1 gave colour channels a home value of **full**, because a colour
 * starts open on every desk the owner has used and the dimmer decides whether
 * any of it is seen. A fixture with no dimmer has no such decision to make: an
 * RGBW PAR is four colour channels and nothing else, so *colour open* and *lamp
 * at full* are the same eight bits, and a rig of them came up white the moment
 * the daemon started.
 *
 * So the desk supplies the missing channel: an intensity that exists in the
 * merge and on the encoders, rests at nought, and scales the fixture's colour on
 * the way out. And the operator can switch it off, because a PAR on a dimmer
 * pack wants its channels written through untouched and the desk cannot know
 * which one this is.
 *
 * **Nothing is drawn for a fixture whose profile has an intensity of its own.**
 * A checkbox that did nothing would be worse than no checkbox — and worse than
 * that, it would read as an offer to take the fixture's real dimmer away.
 */
function DimmerField({
    profile,
    on,
    onChange,
}: {
    readonly profile: ProfileRow | undefined;
    readonly on: boolean;
    readonly onChange: (on: boolean) => void;
}) {
    if (profile === undefined || profile.hasIntensity) {
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
 * Held as text while it is being typed and reported as a number, so an operator
 * clearing the box to type a new number does not get a 0 sent under them — the
 * empty string is *no answer yet*, not zero. The last number typed stands until
 * a new one is.
 */
function NumberField({
    label,
    testId,
    value,
    onChange,
}: {
    readonly label: string;
    readonly testId: string;
    readonly value: number;
    readonly onChange: (value: number) => void;
}) {
    return (
        <label>
            {label}
            <input
                data-testid={testId}
                inputMode="numeric"
                value={String(value)}
                onChange={(event) => {
                    const typed = Number(event.target.value.trim());
                    if (event.target.value.trim() !== "" && Number.isInteger(typed) && typed >= 0) {
                        onChange(typed);
                    }
                }}
            />
        </label>
    );
}
