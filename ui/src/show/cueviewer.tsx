/**
 * The Cue Viewer: what the cues of the sequence in force actually set.
 *
 * # Watched, not edited
 *
 * The Sequence Sheet is the *cue list* and this is the *cue*: fixture by
 * fixture, attribute by attribute, with the preset link on each row. Nothing
 * here is editable, and that is the same rule `Command::PatchFixture` follows in
 * carrying no channels — **what a cue sets comes from the programmer** through
 * `StoreCue`, and a client that wrote a value straight into a cue would be
 * authoring show content for the daemon to accept.
 *
 * # The value beside a link is already the preset's
 *
 * A part carries a value *and* a `presetRef`, and the value is always current:
 * the daemon rewrites every linked part when the preset is stored
 * (`prism_core::Show::relink`). So this window reads the value and never looks
 * the preset up — resolving it here would be a second answer to a question the
 * show has already answered, and the two would disagree for exactly as long as a
 * round trip. What the link column is for is making the link *visible*: an
 * operator can see that this cue will follow the preset, which is the difference
 * between a look that can be re-coloured everywhere and one that cannot.
 *
 * # Scrolling
 *
 * A sequence of forty cues is several hundred rows. It scrolls **inside its own
 * window body**, which is what a window is for; `CLAUDE.md` forbids scrolling
 * outside the canvas and `e2e/desk.spec.ts` checks it in a browser.
 */

import { useMemo } from "react";

import type { JsonValue } from "../bindings";
import { percentOfLevel } from "../desk/level";
import type { PresetRow } from "./looks";
import { executorInForce, presetRows, sequenceRow } from "./looks";

/** The whole window. */
export function CueViewer({
  show,
  session,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
}) {
  const inForce = useMemo(() => executorInForce(session, show), [session, show]);
  const sequence = useMemo(
    () => sequenceRow(show, inForce.sequenceId),
    [show, inForce.sequenceId],
  );
  const presets = useMemo(() => presetRows(show), [show]);

  if (sequence === null) {
    return (
      <p className="window-note" data-testid="cue-viewer-empty">
        No cue list is in force. The viewer follows the sequence on the selected executor, the
        same one the Sequence Sheet shows.
      </p>
    );
  }
  const rows = sequence.cues.flatMap((cue) =>
    cue.parts.map((part) => ({ cue, part })),
  );
  if (rows.length === 0) {
    return (
      <p className="window-note" data-testid="cue-viewer-empty">
        {sequence.name} holds no values yet.
      </p>
    );
  }
  return (
    <div className="looks" data-testid="cue-viewer">
      <div className="looks-bar">
        <span className="looks-count" data-testid="cue-viewer-count">
          {sequence.name} · {sequence.cues.length} cues · {rows.length} values
        </span>
      </div>
      <div className="sheet-scroll" data-testid="cue-viewer-scroll">
        <table className="sheet">
          <thead>
            <tr>
              <th scope="col">Q</th>
              <th scope="col">Fx</th>
              <th scope="col">Attribute</th>
              <th scope="col">Value</th>
              <th scope="col">Preset</th>
            </tr>
          </thead>
          <tbody>
            {rows.map(({ cue, part }) => (
              <tr
                key={`${cue.number}-${String(part.fixture)}-${part.attribute}`}
                data-testid={`part-${cue.number}-${String(part.fixture)}-${part.attribute}`}
                className={part.presetRef === null ? "" : "row-linked"}
              >
                <td>{cue.number}</td>
                <td>{part.fixture}</td>
                <td>{part.attribute}</td>
                <td>{`${String(percentOfLevel(part.value))}%`}</td>
                <td data-testid={`link-${cue.number}-${String(part.fixture)}-${part.attribute}`}>
                  {linkText(part.presetRef, presets)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/**
 * What the link column says.
 *
 * A dash for a value nobody linked, the preset's number and name for one that
 * is, and the **number alone** for a link to a preset that has gone —
 * `prism_core::Show::remove_preset` leaves the value and the link, and
 * `Show::issues` reports the dangling reference, so a viewer that drew nothing
 * there would be hiding exactly the state an operator has to go and fix.
 */
function linkText(presetRef: number | null, presets: readonly PresetRow[]): string {
  if (presetRef === null) {
    return "—";
  }
  const preset = presets.find((row) => row.id === presetRef);
  if (preset === undefined) {
    return `${String(presetRef)} (missing)`;
  }
  return preset.name === "" ? String(preset.id) : `${String(preset.id)} ${preset.name}`;
}
