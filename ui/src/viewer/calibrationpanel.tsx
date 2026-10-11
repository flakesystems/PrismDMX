/**
 * The calibration panel of the 3D viewer - **B70**. The logic is
 * `./calibration.ts`; the arithmetic is the daemon's. This is the form.
 *
 * What the operator does: select one moving head, put marks in the show (or use
 * the ones that are there), press **Aim** on a mark to drive the head at it from
 * the best estimate of where it hangs, trim pan and tilt with the encoders until
 * the real beam is on the real mark, press **Take**. After the third sighting
 * (the second, with the position held) the panel shows where the head really
 * hangs and how far to trust it; **Apply** hangs it there - one `PlaceFixtures`,
 * one Oops.
 *
 * The sightings are this window's: they are not show data and not the daemon's,
 * because a half-finished calibration that two clients could both extend would be
 * two operators aiming one head. The marks are the show's.
 */

import { useEffect, useMemo, useState } from "react";

import type { Mark, ProgrammerState } from "../bindings";
import { useAsk, useSend } from "../store/hooks";
import type { Estimate, Outcome, Sighting } from "./calibration";
import {
  aimOf,
  aimQuery,
  applyCommand,
  driveCommands,
  estimateOf,
  heldPanTilt,
  metres,
  noTargetText,
  outcomeOf,
  percent,
  placeOf,
  sightingsNeeded,
  solveQuery,
  targetOf,
  verdictOf,
} from "./calibration";
import type { MarkFields } from "./marks";
import {
  BLANK_MARK,
  MAX_MARK_NAME,
  fieldsOfMark,
  markLabel,
  markOfFields,
  nextMarkId,
  removeMarks,
  setMark,
} from "./marks";
import type { RigFixture } from "./rig";

/** The panel. */
export function CalibrationPanel({
  rig,
  marks,
  programmer,
  selection,
  visible,
}: {
  readonly rig: readonly RigFixture[];
  readonly marks: readonly Mark[];
  readonly programmer: ProgrammerState | null;
  readonly selection: readonly number[];
  readonly visible: boolean;
}) {
  const send = useSend();
  const ask = useAsk();
  const target = useMemo(() => targetOf(rig, selection), [rig, selection]);
  const targetId = target?.id ?? null;

  // The sightings belong to one fixture: a different head starts again. The
  // reset is made while rendering, as `PlacePanel` does for its form - an effect
  // would paint one frame of the old head's sightings on the new one.
  const [sightings, setSightings] = useState<readonly Sighting[]>([]);
  const [hold, setHold] = useState(false);
  const [workingOn, setWorkingOn] = useState<number | null>(null);
  if (workingOn !== targetId) {
    setWorkingOn(targetId);
    setSightings([]);
    setHold(false);
  }

  // What the daemon made of the sightings. Asked again whenever they, the marks
  // or the choice to hold change; an answer that arrives after the question has
  // moved on is dropped.
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const question = useMemo(
    () =>
      target === null ? null : solveQuery(target.id, placeOf(target), hold, sightings, marks),
    [target, hold, sightings, marks],
  );
  const questionKey = question === null ? "" : JSON.stringify(question);
  useEffect(() => {
    if (question === null || target === null) {
      setOutcome(null);
      return;
    }
    let current = true;
    void ask(question).then((answer) => {
      if (current) {
        setOutcome(outcomeOf(answer, target.id));
      }
    });
    return () => {
      current = false;
    };
    // The key stands for the question.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [questionKey]);

  const solved = outcome?.t === "Solved" ? outcome.solved : null;
  const held = target === null ? null : heldPanTilt(programmer, target.id);
  const need = sightingsNeeded(hold);
  // The sightings the daemon is asked about: those whose mark is still there.
  const usable = sightings.filter((each) => marks.some((mark) => mark.id === each.markId));

  // Where the next aim starts from: the best estimate there is.
  const estimate: Estimate | null =
    solved !== null ? estimateOf(solved) : target === null ? null : placeOf(target);

  const aim = (mark: Mark): void => {
    if (target === null || estimate === null) {
      return;
    }
    void ask(aimQuery(target.id, mark, estimate, held)).then((answer) => {
      const aimed = aimOf(answer, target.id);
      if (aimed !== null) {
        for (const command of driveCommands(aimed)) {
          send(command);
        }
      }
    });
  };
  const take = (mark: Mark): void => {
    if (held === null) {
      return;
    }
    setSightings([
      ...sightings.filter((each) => each.markId !== mark.id),
      { markId: mark.id, pan: held.pan, tilt: held.tilt },
    ]);
  };

  // The mark form: a new mark, or the one being edited.
  const [editing, setEditing] = useState<number | null>(null);
  const [fields, setFields] = useState<MarkFields>(BLANK_MARK);
  const formId = editing ?? nextMarkId(marks);
  const formMark = markOfFields(formId, fields);
  const field = (key: keyof MarkFields, label: string, title: string) => (
    <label className="viewer-field" title={title}>
      <span>{label}</span>
      <input
        data-testid={`calibration-field-${key}`}
        inputMode={key === "name" ? "text" : "decimal"}
        maxLength={key === "name" ? MAX_MARK_NAME : undefined}
        value={fields[key]}
        onChange={(event) => {
          setFields({ ...fields, [key]: event.target.value });
        }}
      />
    </label>
  );

  const verdict = solved === null ? null : verdictOf(solved, usable, marks, hold);

  return (
    <aside
      className="viewer-place viewer-calibration"
      data-testid="calibration-panel"
      hidden={!visible}
    >
      <p className="viewer-selection" data-testid="calibration-target">
        {target === null
          ? noTargetText(selection, rig)
          : `Fixture ${String(target.id)} - ${String(usable.length)} of ${String(need)} sightings` +
            (usable.length >= need ? "" : " needed")}
      </p>

      <label className="viewer-field viewer-check" title="The position is known - from the plan or a tape measure - so only the rotation is solved for. Two marks are then enough">
        <span>Position is known</span>
        <input
          type="checkbox"
          data-testid="calibration-hold"
          checked={hold}
          disabled={target === null}
          onChange={(event) => {
            setHold(event.target.checked);
          }}
        />
      </label>

      <ul className="calibration-marks" data-testid="calibration-marks">
        {marks.length === 0 ? (
          <li className="calibration-none">
            No marks yet. Add the spikes and corners you can point a beam at, with their places.
          </li>
        ) : null}
        {marks.map((mark) => {
          const sighting = sightings.find((each) => each.markId === mark.id);
          const fit =
            solved !== null && sighting !== undefined
              ? solved.fits[usable.indexOf(sighting)]
              : undefined;
          return (
            <li key={mark.id} data-testid={`calibration-mark-${String(mark.id)}`}>
              <span className="calibration-mark-name">
                {markLabel(mark)}{" "}
                <small>
                  {metres(mark.position.x)}, {metres(mark.position.y)}, {metres(mark.position.z)}
                </small>
              </span>
              {sighting === undefined ? null : (
                <span className="calibration-sighting" data-testid={`calibration-sighting-${String(mark.id)}`}>
                  pan {percent(sighting.pan)} · tilt {percent(sighting.tilt)}
                  {fit === undefined ? "" : ` · ${fit.residual.toFixed(2)}°`}
                </span>
              )}
              <span className="calibration-buttons">
                <button
                  type="button"
                  data-testid={`calibration-aim-${String(mark.id)}`}
                  disabled={target === null}
                  title="Drive the head at this mark from where it is thought to hang"
                  onClick={() => {
                    aim(mark);
                  }}
                >
                  Aim
                </button>
                <button
                  type="button"
                  data-testid={`calibration-take-${String(mark.id)}`}
                  disabled={target === null || held === null}
                  title={
                    held === null
                      ? "Aim the head first: there is no pan and tilt to take"
                      : "The beam is on this mark: take the pan and tilt it has now"
                  }
                  onClick={() => {
                    take(mark);
                  }}
                >
                  Take
                </button>
                {sighting === undefined ? null : (
                  <button
                    type="button"
                    data-testid={`calibration-forget-${String(mark.id)}`}
                    title="Forget this sighting"
                    onClick={() => {
                      setSightings(sightings.filter((each) => each.markId !== mark.id));
                    }}
                  >
                    Forget
                  </button>
                )}
                <button
                  type="button"
                  data-testid={`calibration-edit-${String(mark.id)}`}
                  title="Change this mark's name or place"
                  onClick={() => {
                    setEditing(mark.id);
                    setFields(fieldsOfMark(mark));
                  }}
                >
                  Edit
                </button>
                <button
                  type="button"
                  data-testid={`calibration-delete-${String(mark.id)}`}
                  title="Take this mark out of the show"
                  onClick={() => {
                    send(removeMarks([mark.id]));
                    setSightings(sightings.filter((each) => each.markId !== mark.id));
                    if (editing === mark.id) {
                      setEditing(null);
                      setFields(BLANK_MARK);
                    }
                  }}
                >
                  Delete
                </button>
              </span>
            </li>
          );
        })}
      </ul>

      <fieldset className="viewer-fields calibration-form" data-testid="calibration-form">
        <legend>{editing === null ? "New mark" : `Mark ${String(editing)}`}</legend>
        {field("name", "Name", "What the crew calls it: DS centre, Spike 3. May be blank")}
        {field("x", "X", "Across the stage, in metres")}
        {field("y", "Y", "Height above the floor, in metres - nought for a mark on the floor")}
        {field("z", "Z", "Depth, in metres; + is upstage")}
        <div className="viewer-actions calibration-wide">
          <button
            type="button"
            data-testid="calibration-save-mark"
            disabled={formMark === null}
            onClick={() => {
              if (formMark !== null) {
                send(setMark(formMark));
                setEditing(null);
                setFields(BLANK_MARK);
              }
            }}
          >
            {editing === null ? "Add mark" : "Save mark"}
          </button>
          {editing === null ? null : (
            <button
              type="button"
              data-testid="calibration-cancel-mark"
              onClick={() => {
                setEditing(null);
                setFields(BLANK_MARK);
              }}
            >
              Cancel
            </button>
          )}
        </div>
      </fieldset>

      <section
        className="calibration-result"
        data-testid="calibration-result"
        data-level={verdict?.level ?? ""}
      >
        {outcome?.t === "Refused" ? (
          <p>{outcome.reason}</p>
        ) : solved === null || verdict === null ? (
          <p>
            {target === null
              ? ""
              : `Take ${String(Math.max(need - usable.length, 0))} more sighting${need - usable.length === 1 ? "" : "s"} to see where this head hangs.`}
          </p>
        ) : (
          <>
            <p data-testid="calibration-pose">
              Hangs at {metres(solved.position.x)}, {metres(solved.position.y)},{" "}
              {metres(solved.position.z)} m · turned {metres(solved.rotation.x)}°,{" "}
              {metres(solved.rotation.y)}°, {metres(solved.rotation.z)}°
            </p>
            {verdict.lines.map((line) => (
              <p key={line} data-testid="calibration-verdict">
                {line}
              </p>
            ))}
          </>
        )}
      </section>

      <div className="viewer-actions">
        <button
          type="button"
          data-testid="calibration-apply"
          disabled={target === null || solved === null}
          title="Hang the head where the sightings say it hangs - one step, and Oops takes it back"
          onClick={() => {
            if (target !== null && solved !== null) {
              send(applyCommand(target, solved, hold));
              setSightings([]);
            }
          }}
        >
          Apply
        </button>
        <button
          type="button"
          data-testid="calibration-reset"
          disabled={usable.length === 0}
          title="Forget every sighting and start again"
          onClick={() => {
            setSightings([]);
          }}
        >
          Start over
        </button>
      </div>
    </aside>
  );
}
