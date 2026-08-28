/**
 * What is inside a window.
 *
 * S25 builds the canvas, not the sheets: the fixture sheet is S27, the cue
 * viewer is S28 and the 3D viewer is S30. So the ones that are left are one
 * honest line saying the window exists and is not built yet — an operator who
 * opens a `PhaserEditor` from an X-Touch F-key should find a window that says so
 * rather than an empty rectangle they will file a bug about.
 *
 * Twelve of the fourteen are built. Three are about **light**, and S27 settled the
 * difference between them:
 *
 * - **Patch** is the *rig* — which fixtures exist and where their channels are.
 *   It is the one window in this interface that changes the show's shape.
 * - **Fixture Sheet** is the *state* — what the programmer holds and what is on
 *   the cable, per fixture, live.
 * - **DMX Sheet** is the *cable* — S24's level view, channel by channel, with no
 *   fixtures in it at all. It is the reason `WindowType` grew a variant in S25:
 *   none of `ARCHITECTURE_SPEC.md` §6's ten named the DMX output itself.
 *
 * Four are about **looks** — three from S28 and the group pool from S40, which
 * is the window `Command::StoreGroup` needed:
 *
 * - **Sequence Sheet** is the *cue list* — which sequences there are, which is in
 *   force, and its cues with their numbers, names, times and triggers. It is the
 *   one window that stores a look.
 * - **Cue Viewer** is the *cue* — what those cues set, fixture by fixture, with
 *   the preset links visible. Watched, not edited.
 * - **Preset Pool** is the *pools* — named looks per feature group, applied and
 *   stored, with the colour the scribble strips use.
 * - **Group Pool** is the *groups* — lists of fixtures rather than looks, which
 *   is what makes `Group 3` a selection. It replaced a plain list of names in
 *   **S40**, when the pool finally had commands behind it.
 *
 * Three are about **the desk itself**, and all three were part of the shell
 * until **S43** moved them here — the owner's skeleton
 * (`design/skeleton/main-layout.pdf`) has a header, a canvas, the command line
 * and the programmer, and no band for anything else:
 *
 * - **Executors** is the strip: the eight of the current page, their faders and
 *   their four keys. **S45** is where what those keys *do* becomes editable,
 *   which is the punch list's B15 and the reason the strip had to become a
 *   window at all — a band with a fixed height has no room for an editor.
 * - **Command Keys** is the console's vocabulary as buttons (B12). They used to
 *   crowd the command line, which is *the* interface (§4.5) and looked like the
 *   smaller half beside nineteen buttons.
 * - **Status** is the readings that were a strip along the bottom. The one that
 *   must be true without anybody having opened anything — *is the engine
 *   answering* — stayed in the header instead.
 *
 * And one is about **the desk** rather than about the show at all — S37:
 *
 * - **Settings** is the *machine*: the output patch of S33, the MIDI ports of
 *   S36, the show file this daemon has open, and everything `prismd` used to be
 *   told on a command line. It is the first window in this interface that
 *   configures the desk rather than what the desk is playing, and it is a
 *   **window** and not a modal for the reason its own module documentation
 *   gives.
 *
 * # And one is a clock — S43
 *
 * **Clock Viewer** was one of the three that said *not built yet*. §6 has called
 * it *clocks and timecode* since the first draft and there is no timecode in
 * this build, so it draws the half that is real: the time, large enough to read
 * from the back of the room, and what is running. `show/clock.tsx` says which
 * columns are missing and why. That leaves **two** windows saying they are not
 * built, which is S43's exit criterion: `Viewer3D` is S30 and `PhaserEditor` is
 * an engine that does not exist.
 *
 * # Scrolling
 *
 * `CLAUDE.md` forbids scrolling *outside* the canvas, and this is inside one. A
 * window with more rows than it has room for scrolls **within its own body**,
 * which is what a window is for; the canvas itself never scrolls and neither
 * does the page.
 */

import type { JsonValue, ProgrammerState } from "../bindings";
import { ExecutorWindow } from "../desk/executorwindow";
import { Keypad } from "../desk/keypad";
import { PatchWindow } from "../patch/patchwindow";
import { FixtureSheet } from "../patch/sheet";
import { ClockViewer } from "../show/clock";
import { CueViewer } from "../show/cueviewer";
import { GroupPool } from "../show/grouppool";
import { PresetPool } from "../show/presetpool";
import { SequenceSheet } from "../show/sequencesheet";
import { StatusWindow } from "../show/status";
import { SettingsWindow } from "../settings/settingswindow";
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
      return <GroupPool show={show} programmer={programmer} />;
    case "SequenceSheet":
      return <SequenceSheet show={show} session={session} />;
    case "CueViewer":
      return <CueViewer show={show} session={session} programmer={programmer} />;
    case "PresetPool":
      return <PresetPool show={show} />;
    case "Settings":
      return <SettingsWindow />;
    case "Executors":
      return <ExecutorWindow show={show} session={session} />;
    case "CommandKeys":
      return <Keypad />;
    case "Status":
      return <StatusWindow show={show} session={session} programmer={programmer} />;
    case "ClockViewer":
      return <ClockViewer show={show} session={session} />;
    case "Viewer3D":
    case "PhaserEditor":
      return <NotBuiltYet window={instance} />;
  }
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
