/**
 * A drawing of the desk, beside the list — S59.
 *
 * # Why a picture as well as a list
 *
 * The list is the right shape for the question an operator usually has: *I want
 * Go on the selected executor, which key is it on?* It is the wrong shape for
 * the other one, which comes up once per desk and then matters for a whole
 * evening: *which keys are still free, and what have I filled up so far?* The
 * owner asked for both (2026-09-20) — switchable, not instead of.
 *
 * # It knows nothing about what an X-Touch looks like
 *
 * Every box comes off the daemon, on the row it belongs to
 * (`SurfaceControl::geometry`), and the panel it is drawn on comes with the
 * table (`Answer::SurfaceBindings::panel`). That is the same rule `permanent`
 * and `reserved` already follow and it is here for a stronger reason: a browser
 * that held the X-Touch's layout would need a second one the day a second
 * surface is supported, and the second one would be in the wrong place. A
 * device brings its own picture.
 *
 * So a table with no panel draws **nothing** and says so. That is an ordinary
 * state — a surface nobody has drawn yet — and it is better than a picture of
 * the wrong desk.
 *
 * # The strips are one row and eight columns
 *
 * `Strip[*]` is one entry of the binding table, because D7 says the faders are
 * a bank and a strip's keys do the same thing on every column. The drawing
 * repeats that column `panel.strips` times at `panel.stripPitch`, so what an
 * operator sees is eight strips and what they click is one row.
 *
 * # It lights
 *
 * `surfaceLamps` is what the daemon last said is lit, and the drawing shows it.
 * The **list stays quiet** — twenty rows blinking is noise while somebody is
 * reading them — but a picture is glanced at, and the owner's reason for
 * wanting it is exactly this: somebody reworking the table on a rig can see
 * whether a condition they wrote is the one they meant without looking away.
 */

import { useMemo } from "react";

import type { BoundControl, ControlBox, PanelLayout, SurfaceControl } from "../bindings";
import { actionText } from "./actions";

/** What the drawing needs, which is less than the panel holds. */
export interface DeskDrawingProps {
  /** Every control, with its box where it has one. */
  readonly controls: readonly SurfaceControl[];
  /** The panel to draw on, or `null` for a device nobody has drawn. */
  readonly panel: PanelLayout | null;
  /** The names of the controls that are lit right now. */
  readonly lamps: readonly string[];
  /** The control the list has armed learn for, so the picture can agree. */
  readonly arming: string | null;
  /** Called when a key in the picture is clicked. */
  readonly onPick: (control: SurfaceControl) => void;
}

/** One key of the drawing, after the strips have been multiplied out. */
interface Placed {
  /** The row it belongs to. */
  readonly control: SurfaceControl;
  /** Where it goes. */
  readonly box: ControlBox;
  /** Which strip it is, for a strip control — used only for the label. */
  readonly strip: number | null;
  /** A key for React, unique across the repeated columns. */
  readonly key: string;
}

/** Whether a control is one of the eight-times-repeated strip ones. */
function isStrip(control: BoundControl): boolean {
  return (
    control.t === "StripFader" || control.t === "StripEncoder" || control.t === "StripButton"
  );
}

/**
 * Every control as it is drawn, with the strip column repeated.
 *
 * A control with no box is left out rather than drawn at the origin: an absent
 * box means the profile does not say where that control is, and guessing would
 * put a key on top of another one.
 */
function place(
  controls: readonly SurfaceControl[],
  panel: PanelLayout,
): readonly Placed[] {
  const placed: Placed[] = [];
  for (const control of controls) {
    const box = control.geometry;
    if (box === null) {
      continue;
    }
    if (isStrip(control.control)) {
      for (let strip = 0; strip < panel.strips; strip += 1) {
        placed.push({
          control,
          box: { ...box, x: box.x + strip * panel.stripPitch },
          strip,
          key: `${control.name}#${String(strip)}`,
        });
      }
      continue;
    }
    placed.push({ control, box, strip: null, key: control.name });
  }
  return placed;
}

/** What a key is called on the drawing: the last word of its control name. */
function labelOf(control: SurfaceControl): string {
  const dot = control.name.lastIndexOf(".");
  return dot < 0 ? control.name : control.name.slice(dot + 1);
}

/** The drawing. */
export function DeskDrawing({ controls, panel, lamps, arming, onPick }: DeskDrawingProps) {
  const lit = useMemo(() => new Set(lamps), [lamps]);
  const placed = useMemo(
    () => (panel === null ? [] : place(controls, panel)),
    [controls, panel],
  );

  if (panel === null || placed.length === 0) {
    return (
      <p className="settings-none" data-testid="panel-undrawn">
        This surface has no drawing. The list above is the whole table; a device whose profile
        carries a layout is drawn here instead.
      </p>
    );
  }

  return (
    <svg
      className="desk-drawing"
      data-testid="desk-drawing"
      viewBox={`0 0 ${String(panel.width)} ${String(panel.height)}`}
      role="group"
      aria-label="The surface, drawn to scale"
    >
      <rect
        className="desk-panel"
        x={0}
        y={0}
        width={panel.width}
        height={panel.height}
        rx={12}
      />
      {placed.map((entry) => (
        <DrawnControl
          key={entry.key}
          placed={entry}
          lit={lit.has(entry.control.name)}
          arming={arming === entry.control.name}
          onPick={onPick}
        />
      ))}
    </svg>
  );
}

/** One control, drawn as the shape its box says it is. */
function DrawnControl({
  placed,
  lit,
  arming,
  onPick,
}: {
  readonly placed: Placed;
  readonly lit: boolean;
  readonly arming: boolean;
  readonly onPick: (control: SurfaceControl) => void;
}) {
  const { control, box } = placed;
  const bound = control.action !== null;
  const classes = [
    "desk-control",
    `desk-${box.shape}`,
    bound ? "desk-bound" : "desk-free",
    lit ? "desk-lit" : "",
    control.reserved ? "desk-reserved" : "",
    arming ? "desk-arming" : "",
  ]
    .filter((name) => name !== "")
    .join(" ");
  // The title is what the list would say about this row, so the two windows
  // never describe one key differently.
  const title = `${control.name}${placed.strip === null ? "" : ` (strip ${String(placed.strip + 1)})`} — ${actionText(control.action)}`;
  const round = box.shape === "Knob" || box.shape === "Wheel";
  return (
    <g
      className={classes}
      data-testid={`desk-${control.name}`}
      data-lit={lit ? "yes" : "no"}
      data-bound={bound ? "yes" : "no"}
      role="button"
      tabIndex={0}
      aria-label={title}
      onClick={() => {
        onPick(control);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onPick(control);
        }
      }}
    >
      <title>{title}</title>
      {round ? (
        <circle cx={box.x + box.w / 2} cy={box.y + box.h / 2} r={box.w / 2} />
      ) : (
        <rect x={box.x} y={box.y} width={box.w} height={box.h} rx={box.shape === "Fader" ? 4 : 5} />
      )}
      {box.shape === "Key" ? (
        <text x={box.x + box.w / 2} y={box.y + box.h / 2 + 4} textAnchor="middle">
          {labelOf(control)}
        </text>
      ) : null}
    </g>
  );
}
