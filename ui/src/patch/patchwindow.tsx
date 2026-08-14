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
 * # The conflict is shown before it is committed, and the daemon computes it
 *
 * Every change to the draft asks `Query::PatchPreview`, and the line under the
 * form is the answer: whether the daemon would take this patch, how wide the
 * fixture is, where it would end, and what it would overlap. None of that is
 * worked out here — see `patch.ts` for why not.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { FixtureType, JsonValue, PatchPreview } from "../bindings";
import { useAsk, useDesk, useSend } from "../store/hooks";
import type { DeskState } from "../store/desk";
import type { PatchRow, ProfileRow } from "./patch";
import { embeddedProfiles, nextFreeFixtureId, patchRows, profileLabel } from "./patch";
import { PreviewRequester, conflictedFixtures, conflictsOf, isAcceptable, previewText } from "./preview";

const selectLibrary = (state: DeskState): readonly FixtureType[] | null => state.fixtureLibrary;

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
}

/** The whole window. */
export function PatchWindow({ show }: { readonly show: JsonValue }) {
  const send = useSend();
  const ask = useAsk();
  const library = useDesk(selectLibrary);
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
    });
  }, []);

  const add = useCallback(() => {
    const first = profiles[0];
    setDraft({
      id: nextFreeFixtureId(rows),
      wasId: null,
      name: "",
      typeId: first?.id ?? "",
      universe: 1,
      address: 1,
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
      <PatchToolbar
        rows={rows}
        profiles={profiles}
        library={library}
        onAdd={add}
        onEmbed={embed}
      />
      {profiles.length === 0 ? (
        <p className="window-note" data-testid="patch-no-profiles">
          This show carries no fixture profiles, so nothing can be patched into it yet. Add one
          from the desk&rsquo;s library above; the show keeps its own copy of it from then on.
        </p>
      ) : null}
      <PatchTable rows={rows} conflicted={conflicted} editing={draft?.wasId ?? null} onEdit={edit} />
      {draft === null ? null : (
        <PatchForm
          draft={draft}
          profiles={profiles}
          preview={preview}
          onChange={setDraft}
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

/** What there is, and the two ways to add to it. */
function PatchToolbar({
  rows,
  profiles,
  library,
  onAdd,
  onEmbed,
}: {
  readonly rows: readonly PatchRow[];
  readonly profiles: readonly ProfileRow[];
  readonly library: readonly FixtureType[] | null;
  readonly onAdd: () => void;
  readonly onEmbed: (typeId: string) => void;
}) {
  const embedded = new Set(profiles.map((profile) => profile.id));
  const offered = (library ?? []).filter((profile) => !embedded.has(profile.id));
  return (
    <div className="patch-bar">
      <span className="patch-count" data-testid="patch-count">
        {rows.length} fixtures · {profiles.length} profiles
      </span>
      <button type="button" onClick={onAdd} disabled={profiles.length === 0}>
        Add fixture
      </button>
      <label className="patch-embed">
        Add profile
        <select
          data-testid="patch-library"
          value=""
          disabled={offered.length === 0}
          onChange={(event) => {
            if (event.target.value !== "") {
              onEmbed(event.target.value);
            }
          }}
        >
          <option value="">
            {offered.length === 0 ? "all of them are in this show" : "choose…"}
          </option>
          {offered.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profileLabel({
                id: profile.id,
                manufacturer: profile.manufacturer,
                name: profile.name,
                mode: profile.mode,
                footprint: profile.footprint,
              })}
            </option>
          ))}
        </select>
      </label>
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

/** The row being typed into, and what the daemon says it would do. */
function PatchForm({
  draft,
  profiles,
  preview,
  onChange,
  onApply,
  onRemove,
  onCancel,
}: {
  readonly draft: Draft;
  readonly profiles: readonly ProfileRow[];
  readonly preview: PatchPreview | null;
  readonly onChange: (draft: Draft) => void;
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
        <label>
          Type
          <select
            data-testid="draft-type"
            value={draft.typeId}
            onChange={(event) => {
              onChange({ ...draft, typeId: event.target.value });
            }}
          >
            {profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profileLabel(profile)}
              </option>
            ))}
          </select>
        </label>
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
