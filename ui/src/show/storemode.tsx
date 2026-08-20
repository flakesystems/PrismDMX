/**
 * The store-mode chooser — S39.
 *
 * # Why this is a control at all
 *
 * S28 shipped a Store button that said what it would do and offered no choice,
 * because there was nothing to choose: `prism_core::Programmer` merged
 * unconditionally, and a client carrying a mode the daemon did not honour would
 * have been describing an outcome that did not happen. S39 built Override and
 * Remove, so the choice exists and it is the **operator's** — which means it
 * travels in the command, and the daemon never guesses.
 *
 * # The two halves have to move together
 *
 * The word on the button stays the daemon's (`store.ts`): what this control
 * changes is the **question**. Choosing a mode asks `Query::StorePreview` again
 * with that mode on it, and the counts that come back are what *that* mode would
 * cost. A bar that changed the word without re-asking would put "Override" over
 * a Merge's numbers, which is worse than the hard-coded "Merge" S28 refused to
 * ship: it would be wrong about the thing the operator is deciding.
 *
 * # The list comes out of the bindings
 *
 * `STORE_MODE_VARIANTS` is generated from the Rust enum
 * (`prism_domain::export_bindings`), so a fourth mode appears here without this
 * file being edited, and a mode that is removed cannot be left on the screen.
 */

import type { StoreMode } from "../bindings";
import { STORE_MODE_VARIANTS } from "../bindings/variants";

/**
 * Whether a string is one of the store modes this build knows.
 *
 * Private: `ipc/protocol.ts` narrows the mode on the way *in* and has its own,
 * and a shared one exported from a file that also exports a component costs
 * React's fast refresh — which is what `oxlint`'s `only-export-components` is
 * about. Two three-line guards over one generated table is the cheaper of the
 * two, and the table is what stops them disagreeing.
 */
function isStoreMode(value: string): value is StoreMode {
  return (STORE_MODE_VARIANTS as readonly string[]).includes(value);
}

/**
 * A `<select>` over the store modes.
 *
 * Deliberately a select rather than three buttons: a store bar is one row of a
 * window that is mostly a table, and three buttons that are almost always on
 * their default would take the width the cue number needs. The default is
 * whatever the caller holds, which is `Merge` — the one mode that cannot lose
 * anything.
 */
export function StoreModeChooser({
  mode,
  onChoose,
  testId,
}: {
  readonly mode: StoreMode;
  readonly onChoose: (mode: StoreMode) => void;
  /** So a window with two store bars on it can be driven a bar at a time. */
  readonly testId: string;
}) {
  return (
    <label>
      Mode
      <select
        className="cell-input cell-input-narrow"
        data-testid={testId}
        value={mode}
        onChange={(event) => {
          const chosen = event.target.value;
          // Narrowed rather than asserted: `CLAUDE.md` forbids `any` and `as` is
          // a claim rather than a check. A value that is not a mode this build
          // knows cannot come out of the options above, and if it ever did, the
          // right answer is to leave the chooser where it was.
          if (isStoreMode(chosen)) {
            onChoose(chosen);
          }
        }}
      >
        {STORE_MODE_VARIANTS.map((value) => (
          <option key={value} value={value}>
            {value}
          </option>
        ))}
      </select>
    </label>
  );
}
