/**
 * **Viewer 3D** — the rig as it hangs, with the beams the cable is making.
 * S30, and the last thing the 0.9.3 release waited for.
 *
 * # Three parts, and only one of them renders more than once a gesture
 *
 * - **The picture** is a canvas drawn by `./driver.ts`, outside React, from
 *   the show document and the telemetry sink. It is the `DmxSheet`'s
 *   arrangement exactly: this component renders it once and the loop owns its
 *   pixels.
 * - **The toolbar** moves the camera, which is client-local (§4.2) and lives
 *   in a ref. Pressing *Top* is not a render either.
 * - **The place panel** is an ordinary form over the programmer's selection,
 *   and sends `PlaceFixtures` — one command per gesture, so one Oops.
 *
 * # Picking
 *
 * A click on a fixture does what a click on a Fixture Sheet row does
 * (`patch/sheet.tsx`): it offers `Fixture 5` to the line if the line is
 * waiting for one, and otherwise writes `+ Fixture 5`, which adds it to the
 * selection or takes it back out. A drag orbits, a drag with Shift (or the
 * right button) slides, the wheel zooms — and none of those is a command.
 */

import { useEffect, useMemo, useRef, useState } from "react";

import type { JsonValue, ProgrammerState } from "../bindings";
import { pick, useConsole } from "../desk/consoleshell";
import { useSend } from "../store/hooks";
import { useTelemetryChannel } from "../telemetry/context";
import { devicePixelRatio, resizeCanvas } from "../telemetry/painter";
import type { Camera, ViewName } from "./camera";
import { frameAll, look, newCamera, orbit, slide, zoom } from "./camera";
import type { ViewerDriver } from "./driver";
import { driveViewer, readoutText } from "./driver";
import type { PlaceFields } from "./place";
import { fieldsAreNumbers, fieldsOf, placeSet, placeSpread, readField, selectedFixtures } from "./place";
import type { RigFixture } from "./rig";
import { rigOf } from "./rig";
import type { ViewSurface } from "./surface";
import { canvasViewSurface } from "./surface";

/** How far a press may move and still be a click, in CSS pixels. */
const CLICK_SLOP = 4;

/** How near a fixture a click must land to pick it, in CSS pixels. */
const PICK_RADIUS = 18;

/** The toolbar's views, in the order they are offered. */
const VIEW_BUTTONS: readonly (readonly [ViewName, string])[] = [
  ["front", "Front"],
  ["top", "Top"],
  ["side", "Side"],
  ["perspective", "3D"],
];

/** The window. */
export function Viewer3D({
  show,
  programmer,
  surface: makeSurface = canvasViewSurface,
}: {
  readonly show: JsonValue;
  readonly programmer: ProgrammerState | null;
  /** How a canvas becomes a surface; a recording one in the tests. */
  readonly surface?: (canvas: HTMLCanvasElement) => ViewSurface | null;
}) {
  const channel = useTelemetryChannel();
  const rig = useMemo(() => rigOf(show), [show]);
  const selectionList = useMemo(() => programmer?.selection ?? [], [programmer]);
  const selection = useMemo(() => new Set(selectionList), [selectionList]);

  // The loop reads these each frame; they are refs so a new rig or selection
  // reaches it without restarting it.
  const rigRef = useRef(rig);
  const selectionRef = useRef<ReadonlySet<number>>(selection);
  useEffect(() => {
    rigRef.current = rig;
    selectionRef.current = selection;
  }, [rig, selection]);

  const [camera] = useState<Camera>(newCamera);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const readoutRef = useRef<HTMLParagraphElement>(null);
  const driverRef = useRef<ViewerDriver | null>(null);
  const framedRef = useRef(false);

  // The first rig this window sees is framed once, so a window opened on a
  // hung rig shows it rather than an empty corner of the stage. After that
  // the camera is the operator's.
  useEffect(() => {
    if (!framedRef.current && rig.length > 0) {
      framedRef.current = true;
      frameAll(camera, rig.map((fixture) => fixture.position));
    }
  }, [camera, rig]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const readoutNode = readoutRef.current;
    const surface = canvas === null ? null : makeSurface(canvas);
    const driver = driveViewer({
      sink: channel?.sink ?? null,
      surface,
      rig: () => rigRef.current,
      selection: () => selectionRef.current,
      camera,
      scale: devicePixelRatio,
      scheduler: channel?.scheduler,
      clock: channel?.clock,
      readout: (line) => {
        if (readoutNode === null) {
          return;
        }
        readoutNode.textContent = readoutText(line);
        readoutNode.dataset.fixtures = String(line.fixtures);
        readoutNode.dataset.lit = String(line.lit);
        readoutNode.dataset.unplaced = String(line.unplaced);
        readoutNode.dataset.painted = String(line.painted);
        readoutNode.dataset.median = line.median.toFixed(3);
        readoutNode.dataset.p99 = line.p99.toFixed(3);
      },
    });
    driverRef.current = driver;

    const measure = (): void => {
      if (resizeCanvas(canvas, devicePixelRatio())) {
        driver.invalidate();
      }
    };
    measure();
    const observer =
      canvas !== null && typeof ResizeObserver !== "undefined" ? new ResizeObserver(measure) : null;
    if (canvas !== null) {
      observer?.observe(canvas);
    }

    // The wheel is a native listener because React's is passive, and a wheel
    // that zoomed the view *and* scrolled the window body would do both.
    const onWheel = (event: WheelEvent): void => {
      event.preventDefault();
      zoom(camera, Math.sign(event.deltaY));
    };
    canvas?.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      canvas?.removeEventListener("wheel", onWheel);
      observer?.disconnect();
      driver.stop();
      driverRef.current = null;
    };
  }, [camera, channel, makeSurface]);

  const shell = useConsole();
  const { run } = shell;
  const press = useRef<{ x: number; y: number; moved: boolean; slide: boolean } | null>(null);

  return (
    <div className="viewer3d" data-testid="viewer3d">
      <div className="viewer-bar">
        {VIEW_BUTTONS.map(([view, label]) => (
          <button
            key={view}
            type="button"
            data-testid={`viewer-view-${view}`}
            onClick={() => {
              look(camera, view);
            }}
          >
            {label}
          </button>
        ))}
        <button
          type="button"
          data-testid="viewer-frame"
          title="Stand back far enough to see every fixture"
          onClick={() => {
            frameAll(camera, rigRef.current.map((fixture) => fixture.position));
          }}
        >
          Frame all
        </button>
        {/* Written by the loop and never by React: see `./driver.ts`. */}
        <p ref={readoutRef} className="viewer-readout" data-testid="viewer-stats" />
      </div>
      <div className="viewer-body">
        <canvas
          ref={canvasRef}
          className="viewer-canvas"
          data-testid="viewer-canvas"
          onContextMenu={(event) => {
            event.preventDefault();
          }}
          onPointerDown={(event) => {
            press.current = {
              x: event.clientX,
              y: event.clientY,
              moved: false,
              slide: event.shiftKey || event.button === 2,
            };
            event.currentTarget.setPointerCapture?.(event.pointerId);
          }}
          onPointerMove={(event) => {
            const held = press.current;
            if (held === null) {
              return;
            }
            const dx = event.clientX - held.x;
            const dy = event.clientY - held.y;
            if (!held.moved && Math.hypot(dx, dy) < CLICK_SLOP) {
              return;
            }
            held.moved = true;
            held.x = event.clientX;
            held.y = event.clientY;
            if (held.slide) {
              slide(camera, dx, dy, event.currentTarget.clientHeight);
            } else {
              orbit(camera, dx, dy);
            }
          }}
          onPointerUp={(event) => {
            const held = press.current;
            press.current = null;
            if (held === null || held.moved) {
              return;
            }
            const box = event.currentTarget.getBoundingClientRect();
            const ratio = devicePixelRatio();
            const id = driverRef.current?.pick(
              (event.clientX - box.left) * ratio,
              (event.clientY - box.top) * ratio,
              PICK_RADIUS * ratio,
            );
            if (id === null || id === undefined) {
              return;
            }
            pick(shell, `Fixture ${String(id)}`, () => {
              run(`+ Fixture ${String(id)}`);
            });
          }}
        />
        <PlacePanel rig={rig} selection={selectionList} />
      </div>
    </div>
  );
}

/** The form that hangs the selection. */
function PlacePanel({
  rig,
  selection,
}: {
  readonly rig: readonly RigFixture[];
  readonly selection: readonly number[];
}) {
  const send = useSend();
  const chosen = useMemo(() => selectedFixtures(rig, selection), [rig, selection]);
  const first = chosen[0];
  // The form starts from where the first selected fixture hangs, and starts
  // again whenever the selection or that fixture's place changes — a form
  // holding the numbers from before an Oops would send them back.
  const origin = useMemo(() => fieldsOf(first), [first]);
  const [fields, setFields] = useState<PlaceFields>(origin);
  const [shownFor, setShownFor] = useState(origin);
  if (shownFor !== origin) {
    setShownFor(origin);
    setFields(origin);
  }

  const valid = chosen.length > 0 && fieldsAreNumbers(fields);
  const field = (key: keyof PlaceFields, label: string, title: string) => (
    <label className="viewer-field" title={title}>
      <span>{label}</span>
      <input
        inputMode="decimal"
        data-testid={`viewer-place-${key}`}
        value={fields[key]}
        disabled={chosen.length === 0}
        aria-invalid={Number.isNaN(readField(fields[key]) ?? 0)}
        onChange={(event) => {
          setFields({ ...fields, [key]: event.target.value });
        }}
      />
    </label>
  );

  return (
    <aside className="viewer-place" data-testid="viewer-place">
      <p className="viewer-selection" data-testid="viewer-place-selection">
        {chosen.length === 0
          ? "Select fixtures to place them — click them here, in the Fixture Sheet or on the line."
          : chosen.length === 1
            ? `Fixture ${String(first?.id)}`
            : `${String(chosen.length)} fixtures: ${chosen
                .slice(0, 8)
                .map((fixture) => String(fixture.id))
                .join(", ")}${chosen.length > 8 ? " …" : ""}`}
      </p>
      <fieldset className="viewer-fields">
        <legend>Position, m</legend>
        {field("x", "X", "Across the stage; + is stage left as the audience sees it")}
        {field("y", "Y", "Height above the floor")}
        {field("z", "Z", "Depth; + is upstage, away from the audience")}
      </fieldset>
      <fieldset className="viewer-fields">
        <legend>Rotation, °</legend>
        {field("rx", "X", "Tips a hanging beam: 90 points it at the audience, 180 stands the fixture up")}
        {field("ry", "Y", "Turns it about the vertical, applied last")}
        {field("rz", "Z", "Rolls it about the depth axis, applied first")}
      </fieldset>
      <fieldset className="viewer-fields">
        <legend>Spread</legend>
        {field("spacing", "Gap", "How far apart Spread puts them, in metres (1 when blank)")}
      </fieldset>
      <div className="viewer-actions">
        <button
          type="button"
          data-testid="viewer-place-set"
          disabled={!valid}
          title="Every selected fixture at these numbers; a blank field keeps its own"
          onClick={() => {
            send({ t: "PlaceFixtures", placements: placeSet(chosen, fields) });
          }}
        >
          Set
        </button>
        <button
          type="button"
          data-testid="viewer-place-spread"
          disabled={!valid}
          title="Across the stage in selection order, centred on X and a gap apart"
          onClick={() => {
            send({ t: "PlaceFixtures", placements: placeSpread(chosen, fields) });
          }}
        >
          Spread
        </button>
      </div>
    </aside>
  );
}
