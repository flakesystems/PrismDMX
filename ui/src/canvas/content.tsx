/**
 * What is inside a window.
 *
 * S25 builds the canvas, not the sheets: the fixture sheet is S27, the cue
 * viewer is S28 and the 3D viewer is S30. So most of these are one honest line
 * saying the window exists and is not built yet — an operator who opens a
 * `PhaserEditor` from an X-Touch F-key should find a window that says so
 * rather than an empty rectangle they will file a bug about.
 *
 * The three of the family that are built are the three about light rather than
 * about looks, and S27 is where the difference between them was settled:
 *
 * - **Patch** is the *rig* — which fixtures exist and where their channels are.
 *   It is the one window in this interface that changes the show's shape.
 * - **Fixture Sheet** is the *state* — what the programmer holds and what is on
 *   the cable, per fixture, live.
 * - **DMX Sheet** is the *cable* — S24's level view, channel by channel, with no
 *   fixtures in it at all. It is the reason `WindowType` grew a variant in S25:
 *   none of `ARCHITECTURE_SPEC.md` §6's ten named the DMX output itself.
 *
 * # Scrolling
 *
 * `CLAUDE.md` forbids scrolling *outside* the canvas, and this is inside one. A
 * window with more rows than it has room for scrolls **within its own body**,
 * which is what a window is for; the canvas itself never scrolls and neither
 * does the page.
 */

import type { JsonValue, ProgrammerState } from "../bindings";
import { isObject } from "../mirror/patch";
import { stringAt, valueAt } from "../mirror/select";
import { PatchWindow } from "../patch/patchwindow";
import { FixtureSheet } from "../patch/sheet";
import { TelemetryPanel } from "../telemetry/panel";
import type { CanvasWindow } from "./windows";
import { windowTitle } from "./windows";

/** Renders the body of one window. */
export function WindowContent({
  window: instance,
  show,
  session,
  programmer,
}: {
  readonly window: CanvasWindow;
  readonly show: JsonValue;
  readonly session: JsonValue;
  readonly programmer: ProgrammerState | null;
}) {
  switch (instance.type) {
    case "DmxSheet":
      return <TelemetryPanel />;
    case "Patch":
      return <PatchWindow show={show} />;
    case "FixtureSheet":
      return <FixtureSheet show={show} session={session} programmer={programmer} />;
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
