/**
 * The canvas: the windows the session says are open, where it says they are.
 *
 * Positions are in **canvas units** — a 1920 × 1080 grid, whatever the screen
 * — and `asStyle` turns them into percentages, so a layout stored on one
 * monitor looks the same on another. Moving and resizing are commands
 * (`PlaceWindow`), not local state; that is left for the redesign.
 */

import { asStyle, useSession } from "../../core";
import { WindowFrame } from "./WindowFrame";
import { WindowPicker } from "./WindowPicker";

export function Canvas() {
  const session = useSession();
  const windows = session?.session.openWindows ?? [];
  const focused = session?.session.focusedWindow ?? null;
  return (
    <section className="canvas" data-testid="canvas">
      {windows.length === 0 ? (
        <p className="canvas-empty dim">No windows open in this view — press + Window.</p>
      ) : null}
      {windows.map((window) => (
        <WindowFrame
          key={window.instanceId}
          window={window}
          focused={window.instanceId === focused}
          style={asStyle(window)}
        />
      ))}
      <WindowPicker />
    </section>
  );
}
