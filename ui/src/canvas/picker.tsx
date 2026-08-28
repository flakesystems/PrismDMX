/**
 * The window chooser — S43, punch-list entry B9, widened for the owner's
 * rebuild.
 *
 * A right-click on an empty part of the canvas, a keyboard shortcut, or a key on
 * the X-Touch; a panel of every window type there is; a click opens one.
 *
 * # It replaces a dropdown, and the dropdown is gone rather than hidden
 *
 * The View Selector Bar used to carry a `<select>` labelled *Add window*. The
 * owner's word for it was *hässlich* and the objection is sound: it sat in the
 * header, where it had nothing to do with the views beside it, and it was the
 * one control on the screen that looked like a form field on a web page.
 *
 * # Keys side by side, not a column
 *
 * The rebuild's second note: *das Modal zum Fenster öffnen darf gerne auch
 * breiter sein und mehrere Knöpfe nebeneinander haben, anstelle einer langen
 * Liste*. It has been a grid since B9, but inside a panel narrow enough that
 * `auto-fill` only ever gave it two columns, so it read as a list with a gap
 * down the middle. The panel is the **wide** modal now and each key carries a
 * line of its own saying what the window is — which is the other half of a
 * chooser being a chooser: fourteen names an operator has to already know is a
 * menu they learn by opening windows at random.
 *
 * The sentences are in {@link windowNote} and not in `windowTitle`, because a
 * title is what a window frame draws and a note is what a chooser draws.
 *
 * # Why the *open* state is the session's and not this component's
 *
 * Because a surface key opens it, and a surface key is resolved by a daemon with
 * no screen. `Session::window_picker` is the fact; `Command::SetWindowPicker` is
 * how it is set, from either hand. That is the owner's rule for this desk written
 * into the protocol — **the X-Touch drives the interface too, and the two are
 * never out of step** — and it is the same category as the command line, which
 * has been session state since S23 for the same reason.
 *
 * What is client-local, and stays so, is *where the pointer was when the menu
 * opened*: the panel is centred rather than placed at a coordinate, precisely so
 * there is nothing per-screen to share.
 */

import type { WindowType } from "../bindings";
import { WINDOW_TYPE_VARIANTS } from "../bindings/variants";
import { Modal } from "../chrome/modal";
import { windowTitle } from "./windows";

/** What the chooser needs. */
export interface WindowPickerProps {
  /** Opens a window of this type and closes the chooser. */
  readonly onOpen: (type: WindowType) => void;
  /** Closes the chooser without opening anything. */
  readonly onClose: () => void;
}

/**
 * The chooser.
 *
 * The list comes from the generated table rather than from one written here —
 * S23's finding: a view that hand-writes one of these reintroduces exactly the
 * drift `bindings/variants.ts` removes. A window type added to `prism-domain`
 * appears here with nothing edited.
 */
export function WindowPicker({ onOpen, onClose }: WindowPickerProps) {
  return (
    <Modal title="Open a window" testId="window-picker" size="wide" onClose={onClose}>
      <div className="picker-grid">
        {WINDOW_TYPE_VARIANTS.map((type) => (
          <button
            key={type}
            type="button"
            className="picker-key"
            data-testid={`picker-${type}`}
            onClick={() => {
              onOpen(type);
            }}
          >
            <span className="picker-name">{windowTitle(type)}</span>
            <span className="picker-note">{windowNote(type)}</span>
          </button>
        ))}
      </div>
    </Modal>
  );
}

/**
 * What each window is, in one line.
 *
 * Written out rather than generated, because it is prose and there is nothing in
 * `prism-domain` to generate it from — the doc comments on `WindowType` are the
 * daemon's own and are not on the wire. The **switch is exhaustive** over the
 * generated union, so a window type added later fails to compile here rather
 * than appearing in the chooser with a blank line under its name.
 */
function windowNote(type: WindowType): string {
  switch (type) {
    case "Patch":
      return "What is rigged, and where it is addressed";
    case "FixtureSheet":
      return "Every fixture, selected by row";
    case "DmxSheet":
      return "The universes as they leave the desk";
    case "Groups":
      return "Fixture groups, pressed to select";
    case "PresetPool":
      return "Named looks, applied and stored";
    case "SequenceSheet":
      return "The cue lists, chosen and managed";
    case "CueViewer":
      return "What each cue of a list sets";
    case "Executors":
      return "The faders and their keys";
    case "CommandKeys":
      return "The keypad the command line reads";
    case "Status":
      return "Output, tick and programmer readings";
    case "ClockViewer":
      return "The time, read from the back of the room";
    case "Settings":
      return "Outputs, devices, controls and files";
    // The two the daemon knows and this build cannot draw yet. They are in
    // `WindowType` because the daemon places them (S43's B10) and a later
    // session fills them in; the note says so rather than leaving a name with
    // nothing under it, which is what an operator meets when they open one.
    case "Viewer3D":
      return "The rig in three dimensions — a later session";
    case "PhaserEditor":
      return "Effects and their shapes — a later session";
  }
}
