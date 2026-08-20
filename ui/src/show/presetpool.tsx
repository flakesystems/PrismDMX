/**
 * The Preset Pools: named looks per feature group, applied and stored.
 *
 * # A pool is a filter, not a namespace
 *
 * Preset numbers are unique **across** pools, not within them
 * (`prism_core::Show::store_preset`): `Command::ApplyPreset` carries a number
 * and no pool, so a number that meant one thing in Colour and another in
 * Position would make that command ambiguous. The pool is how a preset is filed
 * and which bank shows it; it is not part of its identity. So the tabs here are
 * a filter over one list, and a new preset takes the lowest number **nothing**
 * is filed under.
 *
 * # Which values a store takes is the daemon's answer
 *
 * A colour preset stores the colour values of the programmer and leaves the
 * position alone — and which bank an attribute is on is the *profile's* answer
 * (`AttributeDef::featureGroup`) rather than the attribute name's, which is why
 * `Command::StorePreset` carries the pool and the daemon does the filtering. The
 * count on the Store button is `Query::StorePreview`'s, for the same reason.
 *
 * # Nothing here is state this interface holds
 *
 * The boxes are `poolRows(show, pool)`. Applying is an `ApplyPreset` out and a
 * `ProgrammerChanged` back. What is local is the pool tab, the number and the
 * name being typed — which is `ARCHITECTURE_SPEC.md` §4.2's category: which of
 * five tabs one screen is looking at is not something a second screen should
 * follow, and the console reaches presets through the command line rather than
 * through this window.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { FeatureGroup, JsonValue, ProgrammerState, StorePreview } from "../bindings";
import { FEATURE_GROUP_VARIANTS } from "../bindings/variants";
import { useAsk, useSend } from "../store/hooks";
import type { PresetRow } from "./looks";
import { colorStyle, nextFreeNumber, poolRows, presetRows, presetsDocument } from "./looks";
import { StoreRequester, isStorable, storeText } from "./store";

/** The whole window. */
export function PresetPool({
  show,
  programmer,
}: {
  readonly show: JsonValue;
  /**
   * What the programmer is holding — see `SequenceSheet` for why a store
   * preview has to watch it as well as the show.
   */
  readonly programmer: ProgrammerState | null;
}) {
  const send = useSend();
  const ask = useAsk();
  const [pool, setPool] = useState<FeatureGroup>("Color");
  const all = useMemo(() => presetRows(show), [show]);
  const rows = useMemo(() => poolRows(show, pool), [show, pool]);

  const apply = useCallback(
    (presetId: number) => {
      send({ t: "ApplyPreset", presetId });
    },
    [send],
  );

  return (
    <div className="looks" data-testid="preset-pool">
      <div className="looks-bar">
        <span className="looks-count" data-testid="preset-count">
          {rows.length} of {all.length} presets
        </span>
        {FEATURE_GROUP_VARIANTS.map((group) => (
          <button
            key={group}
            type="button"
            className={`pool-tab${group === pool ? " pool-tab-current" : ""}`}
            data-testid={`pool-${group}`}
            data-current={group === pool ? "yes" : "no"}
            onClick={() => {
              setPool(group);
            }}
          >
            {group}
          </button>
        ))}
      </div>
      {rows.length === 0 ? (
        <p className="window-note" data-testid="pool-empty">
          The {pool} pool is empty. Put a look in the programmer and store one below; only the
          values of this pool go in.
        </p>
      ) : (
        <div className="sheet-scroll" data-testid="pool-scroll">
          <ul className="pool-grid">
            {rows.map((row) => (
              <PresetBox key={row.id} preset={row} onApply={apply} />
            ))}
          </ul>
        </div>
      )}
      <PresetStoreBar
        pool={pool}
        presets={all}
        presetsDoc={presetsDocument(show)}
        programmer={programmer}
        ask={ask}
        onSend={send}
      />
    </div>
  );
}

/** One box: the number, the name, and the colour the scribble strips use. */
function PresetBox({
  preset,
  onApply,
}: {
  readonly preset: PresetRow;
  readonly onApply: (presetId: number) => void;
}) {
  const swatch = colorStyle(preset.color);
  return (
    <li>
      <button
        type="button"
        className="preset-box"
        data-testid={`preset-${String(preset.id)}`}
        title={`Apply preset ${String(preset.id)} to the selection`}
        onClick={() => {
          onApply(preset.id);
        }}
      >
        <span
          className="preset-swatch"
          data-testid={`preset-swatch-${String(preset.id)}`}
          data-color={swatch ?? ""}
          style={swatch === null ? undefined : { background: swatch }}
        />
        <span className="pool-number">{preset.id}</span>
        <span className="pool-name">{preset.name === "" ? "—" : preset.name}</span>
        <span className="pool-note">{preset.values} values</span>
      </button>
    </li>
  );
}

/**
 * The Store button, which says what it will do before it is pressed.
 *
 * The number defaults to the lowest free one and the name to the pool's, and
 * both are typed over. Storing onto a number that already exists is the
 * overwrite the preview describes — including the case an empty programmer
 * makes, which is a **relabel**: `Command::StorePreset` carries the name and the
 * colour, so a store with nothing to store is accepted for a preset that exists
 * and refused for one that does not.
 */
function PresetStoreBar({
  pool,
  presets,
  presetsDoc,
  programmer,
  ask,
  onSend,
}: {
  readonly pool: FeatureGroup;
  readonly presets: readonly PresetRow[];
  /** The `/presets` subtree, as the dependency of the question below. */
  readonly presetsDoc: JsonValue | null;
  readonly programmer: ProgrammerState | null;
  readonly ask: ReturnType<typeof useAsk>;
  readonly onSend: ReturnType<typeof useSend>;
}) {
  const [number, setNumber] = useState<number | null>(null);
  const [name, setName] = useState<string | null>(null);
  const [preview, setPreview] = useState<StorePreview | null>(null);
  const presetId = number ?? nextFreeNumber(presets);
  const existing = presets.find((row) => row.id === presetId);
  const wantedName = name ?? existing?.name ?? `${pool} ${String(presetId)}`;

  const requester = useRef<StoreRequester | null>(null);
  useEffect(() => {
    const live = new StoreRequester(ask, setPreview);
    requester.current = live;
    return () => {
      live.stop();
      requester.current = null;
    };
  }, [ask]);
  // Asked again whenever the number, the pool, the **pools** or the programmer
  // move — which is exactly when the answer can have changed. Deliberately
  // *not* whenever the show moves: since S34 the show document changes whenever
  // a playback advances a cue, and an effect keyed on it would ask the daemon
  // what a store would do once per cue of a chase. `presetsDoc` is the subtree
  // a preview actually depends on, and structural sharing keeps its identity
  // still while executors run (`looks.ts::presetsDocument`).
  useEffect(() => {
    requester.current?.request({ t: "Preset", presetId, pool });
  }, [pool, presetId, presetsDoc, programmer]);

  return (
    <form
      className="store-bar"
      data-testid="preset-store"
      onSubmit={(event) => {
        event.preventDefault();
        onSend({
          t: "StorePreset",
          presetId,
          pool,
          name: wantedName,
          // A colour is a later gesture: `Preset::color` is what the scribble
          // strips show and there is nothing on this screen that picks one yet.
          // Carrying the one that is already there is what stops a relabel
          // throwing an operator's colour away.
          color: existing?.color ?? null,
        });
        setNumber(null);
        setName(null);
      }}
    >
      <label>
        Preset
        <input
          className="cell-input cell-input-narrow"
          data-testid="preset-number"
          inputMode="numeric"
          value={String(presetId)}
          onChange={(event) => {
            const typed = Number(event.target.value.trim());
            if (event.target.value.trim() !== "" && Number.isInteger(typed) && typed > 0) {
              setNumber(typed);
              // The name follows the number until somebody types one: moving to
              // a preset that exists should offer *its* name, not the last one.
              setName(null);
            }
          }}
        />
      </label>
      <label>
        Name
        <input
          className="cell-input"
          data-testid="preset-name"
          value={wantedName}
          onChange={(event) => {
            setName(event.target.value);
          }}
        />
      </label>
      <button type="submit" data-testid="store-preset" disabled={!isStorable(preview)}>
        {storeText(preview, `preset ${String(presetId)}`)}
      </button>
    </form>
  );
}
