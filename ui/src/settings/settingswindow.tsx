/**
 * The settings window — S37, and the window `WindowType::Settings` has been
 * reserving since S25.
 *
 * # A settings window is a window, not a modal
 *
 * It lives on the canvas like any other: it can be moved, it can be open on a
 * second screen, it can be opened from an F-key (the shipped profile binds F4),
 * and the show carries on behind it. `CLAUDE.md` forbids a modal over the canvas
 * and this is why the rule is easy to keep — what it changes is machine or
 * session state that **every** client sees, so there is nothing to lock.
 *
 * # Every panel is a reader over daemon state
 *
 * Closing the window and opening it again shows the same thing, and a second
 * client sees the change, because no panel holds a truth of its own: the rig is
 * `state.outputs`, the surface is `state.surfacePort` and `Query::MidiPorts`,
 * the show file is `state.showFile` and the machine is `state.machine`. That is
 * **D3**, and `ui/src/mirror/` is the only way in.
 *
 * # Which panel is showing is client-local, and deliberately
 *
 * `ARCHITECTURE_SPEC.md` §4.2's own category — the same as a scroll position or
 * the row being typed into. Two operators looking at two panels of one window on
 * two screens is right; a tab that jumped under one of them because the other
 * clicked would be the thing §4.2 exists to prevent.
 *
 * # What scrolls, scrolls inside this window
 *
 * `CLAUDE.md` forbids scrolling outside the canvas. The panel body is the one
 * thing here with `overflow`, which is what a window is for — S27's patch sheet
 * and S28's cue sheet are the two that came before it, and all three are checked
 * in a browser rather than reviewed in a stylesheet.
 */

import { useState } from "react";

import { ControlsPanel } from "./controls";
import { DevicesPanel } from "./devices";
import { MachinePanel } from "./machine";
import { OutputsPanel } from "./outputs";
import type { Panel } from "./settings";
import { PANELS } from "./settings";
import { ShowFilesPanel } from "./showfiles";

/** The whole window. */
export function SettingsWindow() {
  const [panel, setPanel] = useState<Panel>("Outputs");
  return (
    <div className="settings" data-testid="settings">
      <nav className="settings-tabs" aria-label="Settings sections">
        {PANELS.map((name) => (
          <button
            key={name}
            type="button"
            className={name === panel ? "tab tab-active" : "tab"}
            aria-pressed={name === panel}
            data-testid={`settings-tab-${name.replace(/\s+/gu, "-").toLowerCase()}`}
            onClick={() => {
              setPanel(name);
            }}
          >
            {name}
          </button>
        ))}
      </nav>
      <div className="settings-body" data-testid="settings-body">
        {panel === "Outputs" ? <OutputsPanel /> : null}
        {panel === "Devices" ? <DevicesPanel /> : null}
        {panel === "Controls" ? <ControlsPanel /> : null}
        {panel === "Show files" ? <ShowFilesPanel /> : null}
        {panel === "This machine" ? <MachinePanel /> : null}
      </div>
    </div>
  );
}
