/**
 * What is inside a window.
 *
 * S25 builds the canvas, not the sheets: the fixture sheet is S27, the cue
 * viewer is S28 and the 3D viewer is S30. So most of these are one honest line
 * saying the window exists and is not built yet — an operator who opens a
 * `PhaserEditor` from an X-Touch F-key should find a window that says so
 * rather than an empty rectangle they will file a bug about.
 *
 * The three that are not placeholders are the three whose data is already in
 * the mirror or already on a canvas:
 *
 * - **DMX Sheet** is S24's level view, moved out of the column it was demoed in
 *   and into a window sized by the window. It is the reason `WindowType` grew a
 *   variant: none of `ARCHITECTURE_SPEC.md` §6's ten named the DMX output
 *   itself.
 * - **Patch** and **Fixture Sheet** list what the show has patched, read
 *   straight out of the show document by pointer.
 *
 * # Scrolling
 *
 * `CLAUDE.md` forbids scrolling *outside* the canvas, and this is inside one. A
 * window with more rows than it has room for scrolls **within its own body**,
 * which is what a window is for; the canvas itself never scrolls and neither
 * does the page.
 */

import type { JsonValue } from "../bindings";
import { isObject } from "../mirror/patch";
import { numberAt, stringAt, valueAt } from "../mirror/select";
import { TelemetryPanel } from "../telemetry/panel";
import type { CanvasWindow } from "./windows";
import { windowTitle } from "./windows";

/** Renders the body of one window. */
export function WindowContent({
  window: instance,
  show,
}: {
  readonly window: CanvasWindow;
  readonly show: JsonValue;
}) {
  switch (instance.type) {
    case "DmxSheet":
      return <TelemetryPanel />;
    case "Patch":
    case "FixtureSheet":
      return <PatchList show={show} />;
    case "Groups":
      return <NameList show={show} collection="groups" empty="No groups yet" />;
    case "SequenceSheet":
    case "CueViewer":
      return <NameList show={show} collection="sequences" empty="No sequences yet" />;
    case "PresetPool":
      return <NameList show={show} collection="presets" empty="No presets yet" />;
    case "Viewer3D":
    case "PhaserEditor":
    case "ClockViewer":
    case "Settings":
      return <NotBuiltYet window={instance} />;
  }
}

/** The patch: every fixture, its type, and where it lives. */
function PatchList({ show }: { readonly show: JsonValue }) {
  const fixtures = valueAt(show, "/fixtures");
  const rows = isObject(fixtures) ? Object.entries(fixtures) : [];
  if (rows.length === 0) {
    return <p className="window-note">Nothing is patched.</p>;
  }
  return (
    <table className="sheet">
      <thead>
        <tr>
          <th scope="col">Fx</th>
          <th scope="col">Name</th>
          <th scope="col">Type</th>
          <th scope="col">Univ</th>
          <th scope="col">Addr</th>
        </tr>
      </thead>
      <tbody>
        {rows.map(([id, fixture]) => (
          <tr key={id}>
            <td>{id}</td>
            <td>{stringAt(fixture, "/name") ?? "—"}</td>
            <td>{stringAt(fixture, "/typeId") ?? "—"}</td>
            <td>{numberAt(fixture, "/universe") ?? "—"}</td>
            <td>{numberAt(fixture, "/address") ?? "—"}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Whatever a collection of the show holds, by number and name. */
function NameList({
  show,
  collection,
  empty,
}: {
  readonly show: JsonValue;
  readonly collection: string;
  readonly empty: string;
}) {
  const value = valueAt(show, `/${collection}`);
  const rows = isObject(value) ? Object.entries(value) : [];
  if (rows.length === 0) {
    return <p className="window-note">{empty}</p>;
  }
  return (
    <ul className="pool">
      {rows.map(([id, entry]) => (
        <li key={id}>
          <span className="pool-number">{id}</span>
          <span className="pool-name">{stringAt(entry, "/name") ?? "—"}</span>
        </li>
      ))}
    </ul>
  );
}

/**
 * A window the daemon can hold and this build cannot yet draw.
 *
 * Said plainly rather than left blank: the session is authoritative, the
 * console can open any of these, and "nothing here yet" is a different
 * statement from "something went wrong".
 */
function NotBuiltYet({ window: instance }: { readonly window: CanvasWindow }) {
  return (
    <p className="window-note" data-testid={`unbuilt-${String(instance.instanceId)}`}>
      {windowTitle(instance.type)} is not built yet. The session is holding this window open, so
      it will be here — in this place — when it is.
    </p>
  );
}
