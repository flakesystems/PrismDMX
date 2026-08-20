/**
 * The Sequence Sheet: the cue list of the sequence in force, edited in place.
 *
 * # What the three look windows are for
 *
 * S27 settled what the three *light* windows are (Patch, Fixture Sheet, DMX
 * Sheet); this is the same paragraph for the three **look** windows S28 fills
 * in, and it belongs beside `WindowType` in `ARCHITECTURE_SPEC.md` §6:
 *
 * - **Sequence Sheet** — the *cue list*. Which sequences there are, which one is
 *   in force, and its cues with their numbers, names, times and triggers. It is
 *   edited, and it is where a look is stored.
 * - **Cue Viewer** — the *cue*. What those cues actually set, fixture by fixture,
 *   with the preset links visible. Watched, not edited: what a cue sets comes
 *   from the programmer through `StoreCue`.
 * - **Preset Pool** — the *pools*. Named looks per feature group, applied and
 *   stored, with the colour the scribble strips use.
 *
 * # The sequence in force is the desk's, and the executor is the transport
 *
 * **S28's marked assumption is settled.** §4.1 had no *selected sequence* and
 * this window used the selected executor's, with the assumption written down
 * rather than made permanent; §4.4 named S39 as the session that would decide,
 * and S39 gave the session a field of its own. So the cue list this sheet shows
 * is `Session::selectedSequence` and choosing one is a `Command::SelectSequence`
 * — a command out, a `SessionPatch` back, and not a local selection two screens
 * could disagree about.
 *
 * What changed for an operator: a cue list can be written **before** anybody
 * decides which fader it goes on, and `Store Cue 5` means something with no
 * executor selected. The executor line below the pool is what it always was —
 * Go, Back, Off and which cue the playback is standing on — and it follows the
 * *executor*, so the two can legitimately be looking at different sequences.
 * That is the case a console has when one list is being programmed while
 * another is on stage.
 *
 * # Nothing on this window is state this interface holds
 *
 * The pool is `sequenceRows(show)`. The cue list is that sequence's cues. What
 * is running is `/executors/<id>/isActive`. The **one** local thing is the cell
 * being typed into and the number in the store box, which is
 * `ARCHITECTURE_SPEC.md` §4.2's own category — a half-finished edit is not
 * something a second operator's screen should follow. The moment it is
 * committed it goes to the daemon and the draft is dropped, not merged: the same
 * resolution `canvas/drag.ts` wrote down for a pointer and `patch/patchwindow.tsx`
 * for a form.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type {
  CueProperty,
  CueTrigger,
  JsonValue,
  ProgrammerState,
  StoreMode,
  StorePreview,
} from "../bindings";
import { CUE_TRIGGER_VARIANTS } from "../bindings/variants";
import { useAsk, useSend } from "../store/hooks";
import type { CueEditInForce, CueRow, SequenceRow } from "./looks";
import {
  cueEditInForce,
  executorInForce,
  nextCueNumber,
  nextFreeNumber,
  secondsText,
  sequenceInForce,
  sequenceRow,
  sequenceRows,
  sequencesDocument,
} from "./looks";
import { StoreRequester, isStorable, storeText } from "./store";
import { StoreModeChooser } from "./storemode";

/** Which cell of which cue is being typed into. Local, and dropped on commit. */
interface CellDraft {
  /** The cue, by the number it had when the edit began. */
  readonly cueNumber: string;
  /** Which column. */
  readonly field: "number" | "name" | "fadeIn" | "fadeOut" | "delay";
  /** What has been typed so far. */
  readonly text: string;
}

/** The whole window. */
export function SequenceSheet({
  show,
  session,
  programmer,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
  /**
   * What the programmer is holding.
   *
   * Not drawn — the sheet is about the cue list — but the **store preview**
   * depends on it: what a store would add and replace changes the instant an
   * encoder moves, and the programmer is a document of its own
   * (`Delta::ProgrammerChanged`), so a bar watching only the show would go on
   * offering the answer to a question about a programmer that had since
   * emptied.
   */
  readonly programmer: ProgrammerState | null;
}) {
  const send = useSend();
  const ask = useAsk();
  const sequences = useMemo(() => sequenceRows(show), [show]);
  const inForce = useMemo(() => executorInForce(session, show), [session, show]);
  const chosen = useMemo(() => sequenceInForce(session), [session]);
  const editing = useMemo(() => cueEditInForce(session), [session]);
  const sequence = useMemo(() => sequenceRow(show, chosen), [show, chosen]);
  // The transport line names the executor's **own** cue list, which since S39
  // need not be the one being edited: a strip that borrowed the sheet's name
  // would tell an operator that the fader under their hand plays a list it does
  // not play.
  const onExecutor = useMemo(
    () => sequenceRow(show, inForce.sequenceId),
    [show, inForce.sequenceId],
  );

  const create = useCallback(() => {
    const sequenceId = nextFreeNumber(sequences);
    send({ t: "CreateSequence", sequenceId, name: `Sequence ${String(sequenceId)}` });
    // And put it in force, so the sheet is looking at the list that was just
    // made. Two commands rather than one, and each is meaningful on its own —
    // making a cue list and choosing which one to edit are different acts, and
    // since S39 neither of them needs an executor.
    send({ t: "SelectSequence", sequenceId });
    // A fader as well, if one is selected: a cue list nobody can fire is a cue
    // list nobody can check. Refused where the slot is taken by another list,
    // which is the daemon's decision and not this window's.
    if (inForce.executorId !== null) {
      send({ t: "AssignExecutor", executorId: inForce.executorId, sequenceId });
    }
  }, [inForce.executorId, send, sequences]);

  const choose = useCallback(
    (sequenceId: number) => {
      // No executor needed since S39 — which is half the reason the field
      // exists. See the module documentation.
      send({ t: "SelectSequence", sequenceId });
    },
    [send],
  );

  return (
    <div className="looks" data-testid="sequence-sheet">
      <SequenceBar
        sequences={sequences}
        current={chosen}
        onChoose={choose}
        onCreate={create}
      />
      <ExecutorLine
        executorId={inForce.executorId}
        sequenceName={onExecutor?.name ?? null}
        isActive={inForce.isActive}
        currentCueIndex={inForce.currentCueIndex}
        onGo={(direction) => {
          if (inForce.executorId !== null) {
            send({ t: "ExecutorGo", executorId: inForce.executorId, direction });
          }
        }}
        onOff={() => {
          if (inForce.executorId !== null) {
            send({ t: "ExecutorOff", executorId: inForce.executorId });
          }
        }}
      />
      {sequence === null ? (
        <p className="window-note" data-testid="no-sequence">
          No cue list is in force. Choose one above, or make a new one — a cue
          list does not need a fader to be written.
        </p>
      ) : (
        <>
          <CueTable
            sequence={sequence}
            // The playback marker only where the two are looking at the same
            // list: an executor running sequence 3 says nothing about which row
            // of sequence 7 an operator is editing, and a marker drawn from it
            // would point at a cue nobody is on.
            currentCueIndex={
              inForce.isActive && inForce.sequenceId === sequence.id
                ? inForce.currentCueIndex
                : null
            }
            onSend={send}
          />
          <StoreBar
            sequence={sequence}
            sequencesDoc={sequencesDocument(show)}
            programmer={programmer}
            editing={editing}
            ask={ask}
            onSend={send}
          />
        </>
      )}
    </div>
  );
}

/** The sequence pool: what there is, and the way to add to it. */
function SequenceBar({
  sequences,
  current,
  onChoose,
  onCreate,
}: {
  readonly sequences: readonly SequenceRow[];
  readonly current: number | null;
  readonly onChoose: (sequenceId: number) => void;
  readonly onCreate: () => void;
}) {
  return (
    <div className="looks-bar">
      <span className="looks-count" data-testid="sequence-count">
        {sequences.length} sequences
      </span>
      {sequences.map((row) => (
        <button
          key={row.id}
          type="button"
          className={`pool-chip${row.id === current ? " pool-chip-current" : ""}`}
          data-testid={`sequence-${String(row.id)}`}
          data-current={row.id === current ? "yes" : "no"}
          title={`Edit ${row.name}`}
          onClick={() => {
            onChoose(row.id);
          }}
        >
          <span className="pool-number">{row.id}</span>
          <span className="pool-name">{row.name === "" ? "—" : row.name}</span>
          <span className="pool-note">{row.cues.length} cues</span>
        </button>
      ))}
      <button type="button" data-testid="new-sequence" onClick={onCreate}>
        New sequence
      </button>
    </div>
  );
}

/**
 * Which executor the sheet is following, and the three keys that fire it.
 *
 * **The cue number is the daemon's, at last.** `Executor::currentCueIndex` was
 * on the wire from S1 with nothing filling it, so S26 and S28 both drew a dash
 * here; S34's readback out of the tick fills it, and this line shows it. It is
 * still not a number this sheet may work out for itself — a follow cue advances
 * without anybody pressing anything.
 *
 * The three keys stay `ExecutorGo` and `ExecutorOff` rather than
 * `ExecutorButton`: this line is not a strip and has no button *positions* to
 * press. What it offers is *go*, *back* and *off* as themselves, which is what a
 * cue sheet's transport is, and `desk/executorbar.tsx` is where an executor's
 * own four keys live.
 */
function ExecutorLine({
  executorId,
  sequenceName,
  isActive,
  currentCueIndex,
  onGo,
  onOff,
}: {
  readonly executorId: number | null;
  readonly sequenceName: string | null;
  readonly isActive: boolean;
  readonly currentCueIndex: number | null;
  readonly onGo: (direction: "Next" | "Prev") => void;
  readonly onOff: () => void;
}) {
  const playable = executorId !== null && sequenceName !== null;
  return (
    <div className="looks-exec" data-testid="looks-executor" data-active={isActive ? "yes" : "no"}>
      <span className="looks-exec-name" data-testid="looks-executor-name">
        {executorId === null ? "No executor selected" : `Executor ${String(executorId)}`}
        {sequenceName === null ? "" : ` · ${sequenceName}`}
      </span>
      <span className="looks-exec-state" data-testid="looks-executor-state">
        {isActive ? "running" : "stopped"}
      </span>
      <span
        className="looks-exec-cue"
        data-testid="looks-executor-cue"
        title="Which cue the tick says this playback is on."
      >
        {currentCueIndex === null ? "cue —" : `cue ${String(currentCueIndex + 1)}`}
      </span>
      <div className="looks-exec-keys">
        <button
          type="button"
          data-testid="looks-go"
          disabled={!playable}
          onClick={() => {
            onGo("Next");
          }}
        >
          Go
        </button>
        <button
          type="button"
          data-testid="looks-back"
          disabled={!playable}
          onClick={() => {
            onGo("Prev");
          }}
        >
          Back
        </button>
        <button
          type="button"
          data-testid="looks-off"
          disabled={!playable}
          onClick={onOff}
        >
          Off
        </button>
      </div>
    </div>
  );
}

/** The cue list, with every field editable in its own cell. */
function CueTable({
  sequence,
  currentCueIndex,
  onSend,
}: {
  readonly sequence: SequenceRow;
  /**
   * Which row the playback is standing on, or nothing when it is stopped.
   *
   * An **index** rather than a number, because that is what the tick reports and
   * what the show carries; the row it names is `cues[index]` in the order the
   * daemon compiled them, which is the order this table draws.
   */
  readonly currentCueIndex: number | null;
  readonly onSend: ReturnType<typeof useSend>;
}) {
  const [draft, setDraft] = useState<CellDraft | null>(null);

  // A cue that has been renumbered or deleted under an open editor is one whose
  // draft names nothing; dropping it is what stops an edit landing on whatever
  // cue happens to hold that number now.
  useEffect(() => {
    if (draft !== null && !sequence.cues.some((cue) => cue.number === draft.cueNumber)) {
      setDraft(null);
    }
  }, [draft, sequence]);

  const commit = useCallback(
    (cue: CueRow, field: CellDraft["field"], text: string) => {
      setDraft(null);
      const property = propertyOf(field, text);
      if (property === null) {
        return;
      }
      onSend({
        t: "SetCueProperty",
        sequenceId: sequence.id,
        // The number the cue **started** at, not the one being typed: the trap
        // an operator meets by changing their mind halfway, which S27 met with
        // Unpatch.
        cueNumber: cue.number,
        property,
      });
    },
    [onSend, sequence.id],
  );

  if (sequence.cues.length === 0) {
    return (
      <p className="window-note" data-testid="no-cues">
        This sequence has no cues. Put a look in the programmer and store one below.
      </p>
    );
  }
  return (
    <div className="sheet-scroll" data-testid="cue-scroll">
      <table className="sheet">
        <thead>
          <tr>
            <th scope="col">Q</th>
            <th scope="col">Name</th>
            <th scope="col">In</th>
            <th scope="col">Out</th>
            <th scope="col">Delay</th>
            <th scope="col">Trigger</th>
            <th scope="col">Values</th>
            <th scope="col">
              <span className="visually-hidden">Edit and delete</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {sequence.cues.map((cue, index) => (
            <tr
              key={cue.number}
              data-testid={`cue-row-${cue.number}`}
              className={index === currentCueIndex ? "cue-running" : undefined}
              data-running={index === currentCueIndex ? "yes" : "no"}
            >
              <Cell
                cue={cue}
                field="number"
                shown={cue.number}
                draft={draft}
                onEdit={setDraft}
                onCommit={commit}
              />
              <Cell
                cue={cue}
                field="name"
                shown={cue.name === "" ? "—" : cue.name}
                draft={draft}
                onEdit={setDraft}
                onCommit={commit}
              />
              <Cell
                cue={cue}
                field="fadeIn"
                shown={secondsText(cue.fadeIn)}
                draft={draft}
                onEdit={setDraft}
                onCommit={commit}
              />
              <Cell
                cue={cue}
                field="fadeOut"
                shown={secondsText(cue.fadeOut)}
                draft={draft}
                onEdit={setDraft}
                onCommit={commit}
              />
              <Cell
                cue={cue}
                field="delay"
                shown={secondsText(cue.delay)}
                draft={draft}
                onEdit={setDraft}
                onCommit={commit}
              />
              <td>
                <TriggerCell
                  cue={cue}
                  onChange={(trigger, triggerTime) => {
                    onSend({
                      t: "SetCueProperty",
                      sequenceId: sequence.id,
                      cueNumber: cue.number,
                      property: { t: "Trigger", trigger, triggerTime },
                    });
                  }}
                />
              </td>
              <td data-testid={`cue-parts-${cue.number}`}>{cue.parts.length}</td>
              {/*
                Both keys in **one** cell rather than a column each. A cue sheet
                is a dense table on a screen that must not scroll sideways
                (`CLAUDE.md`), and an eighth column pushed the rows far enough
                that the sticky header started intercepting clicks on the first
                one — which CI found on a Linux runner and this machine did not.
              */}
              <td className="cue-keys">
                <button
                  type="button"
                  className="linkish"
                  data-testid={`cue-edit-${cue.number}`}
                  aria-label={`Edit cue ${cue.number}`}
                  title="Load this cue into the programmer"
                  onClick={() => {
                    // **The one gesture that fills the programmer** — S39. What
                    // comes back is the cue's every value with its preset links
                    // kept, and the desk remembers which cue it came from, which
                    // is what the Update key below acts on.
                    onSend({
                      t: "EditCue",
                      sequenceId: sequence.id,
                      cueNumber: cue.number,
                    });
                  }}
                >
                  Edit
                </button>
                <button
                  type="button"
                  className="linkish"
                  data-testid={`cue-delete-${cue.number}`}
                  aria-label={`Delete cue ${cue.number}`}
                  onClick={() => {
                    onSend({
                      t: "DeleteCue",
                      sequenceId: sequence.id,
                      cueNumber: cue.number,
                    });
                  }}
                >
                  ×
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/**
 * One editable cell: what the document says, or what is being typed into it.
 *
 * Committed on Enter and on blur, abandoned on Escape. The commit sends a
 * command and drops the draft **without waiting**: what the cue is comes back as
 * a `ShowPatch`, and a cell that held the typed text until the delta arrived
 * would be this interface having an opinion about the show for the length of a
 * round trip (D3).
 */
function Cell({
  cue,
  field,
  shown,
  draft,
  onEdit,
  onCommit,
}: {
  readonly cue: CueRow;
  readonly field: CellDraft["field"];
  readonly shown: string;
  readonly draft: CellDraft | null;
  readonly onEdit: (draft: CellDraft | null) => void;
  readonly onCommit: (cue: CueRow, field: CellDraft["field"], text: string) => void;
}) {
  const editing = draft !== null && draft.cueNumber === cue.number && draft.field === field;
  const testId = `cue-${field}-${cue.number}`;
  if (!editing) {
    return (
      <td>
        <button
          type="button"
          className="linkish cell"
          data-testid={testId}
          onClick={() => {
            onEdit({ cueNumber: cue.number, field, text: rawOf(cue, field) });
          }}
        >
          {shown}
        </button>
      </td>
    );
  }
  return (
    <td>
      <input
        // eslint-disable-next-line jsx-a11y/no-autofocus -- the cell was just
        // clicked; the focus is where the operator put it.
        autoFocus
        className="cell-input"
        data-testid={`${testId}-input`}
        value={draft.text}
        onChange={(event) => {
          onEdit({ ...draft, text: event.target.value });
        }}
        onBlur={() => {
          onCommit(cue, field, draft.text);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            onCommit(cue, field, draft.text);
          }
          if (event.key === "Escape") {
            onEdit(null);
          }
        }}
      />
    </td>
  );
}

/** The trigger, and the time a `Time` trigger needs beside it. */
function TriggerCell({
  cue,
  onChange,
}: {
  readonly cue: CueRow;
  readonly onChange: (trigger: CueTrigger, triggerTime: number | null) => void;
}) {
  return (
    <span className="trigger-cell">
      <select
        data-testid={`cue-trigger-${cue.number}`}
        value={cue.trigger}
        onChange={(event) => {
          const trigger = triggerFrom(event.target.value);
          // The two travel together, because a time means nothing for the other
          // three triggers — `CueProperty::Trigger` carries both for exactly
          // that reason. A cue that becomes `Time` keeps the time it had, or
          // starts at nothing and is refused until one is typed.
          onChange(trigger, trigger === "Time" ? (cue.triggerTime ?? 0) : null);
        }}
      >
        {CUE_TRIGGER_VARIANTS.map((trigger) => (
          <option key={trigger} value={trigger}>
            {trigger}
          </option>
        ))}
      </select>
      {cue.trigger === "Time" ? (
        <input
          className="cell-input cell-input-narrow"
          data-testid={`cue-trigger-time-${cue.number}`}
          inputMode="decimal"
          defaultValue={String(cue.triggerTime ?? 0)}
          onBlur={(event) => {
            const seconds = Number(event.target.value.trim());
            if (Number.isFinite(seconds)) {
              onChange("Time", seconds);
            }
          }}
        />
      ) : null}
    </span>
  );
}

/** The Store button, which says what it will do before it is pressed. */
function StoreBar({
  sequence,
  sequencesDoc,
  programmer,
  editing,
  ask,
  onSend,
}: {
  readonly sequence: SequenceRow;
  /**
   * The `/sequences` subtree, as the dependency of the question below.
   *
   * Not `sequence`, which is a fresh object every render: since S34 the show
   * document moves whenever a playback changes cue, so an effect keyed on it
   * would ask the daemon what a store would do **once per cue of a chase**.
   * This node's identity only changes when a sequence really does. See
   * `looks.ts::sequencesDocument`.
   */
  readonly sequencesDoc: JsonValue | null;
  readonly programmer: ProgrammerState | null;
  /**
   * The cue the programmer is editing, or `null` — S39's update state.
   *
   * The daemon's, not this window's: `Session::editingCue` is what every client
   * blinks the Update key on, so a second screen agrees without being told.
   */
  readonly editing: CueEditInForce | null;
  readonly ask: ReturnType<typeof useAsk>;
  readonly onSend: ReturnType<typeof useSend>;
}) {
  const [number, setNumber] = useState<string | null>(null);
  const [mode, setMode] = useState<StoreMode>("Merge");
  const [preview, setPreview] = useState<StorePreview | null>(null);
  const wanted = number ?? nextCueNumber(sequence.cues);

  const requester = useRef<StoreRequester | null>(null);
  useEffect(() => {
    const live = new StoreRequester(ask, setPreview);
    requester.current = live;
    return () => {
      live.stop();
      requester.current = null;
    };
  }, [ask]);
  // Asked again whenever the cue number, the cue **lists** or the programmer
  // move, which is exactly when the answer can have changed: a store into cue 3
  // means something different once somebody has stored cue 3, and something
  // different again once they have touched another encoder. **Not** whenever
  // the show moves — see `sequencesDoc`.
  useEffect(() => {
    requester.current?.request(
      {
        t: "Cue",
        sequenceId: sequence.id,
        cueNumber: wanted,
      },
      mode,
    );
    // `sequence` is deliberately absent: `sequence.id` and `sequencesDoc`
    // between them say everything a preview depends on, and the row object is
    // rebuilt on every render. `mode` is here because S39 made it part of the
    // question — the counts on the button are what *that* mode would cost.
  }, [mode, programmer, sequence.id, sequencesDoc, wanted]);

  return (
    <form
      className="store-bar"
      data-testid="cue-store"
      onSubmit={(event) => {
        event.preventDefault();
        onSend({ t: "StoreCue", sequenceId: sequence.id, cueNumber: wanted, mode });
        // Dropped, not kept: what the cue list is comes back as a `ShowPatch`,
        // and the box goes back to offering the next number of whatever the
        // daemon ends up holding.
        setNumber(null);
      }}
    >
      <label>
        Cue
        <input
          className="cell-input cell-input-narrow"
          data-testid="store-number"
          value={wanted}
          onChange={(event) => {
            setNumber(event.target.value);
          }}
        />
      </label>
      <StoreModeChooser mode={mode} onChoose={setMode} testId="cue-store-mode" />
      <button type="submit" data-testid="store-cue" disabled={!isStorable(preview)}>
        {storeText(preview, `cue ${wanted}`)}
      </button>
      {editing === null ? null : (
        <button
          type="button"
          // **The blink is the daemon's state and not a timer this window
          // keeps** — `Session::editingCue.modified`. Two screens looking at
          // one desk therefore blink together, and a client that had counted
          // its own keystrokes would drift the moment a console touched an
          // encoder.
          className={`update-key${editing.modified ? " update-blinking" : ""}`}
          data-testid="update-cue"
          data-modified={editing.modified ? "yes" : "no"}
          title="Store the programmer back into the cue it came from"
          onClick={() => {
            // It carries nothing: which cue, and that the mode is Override, are
            // both the desk's — see `Command::Update`.
            onSend({ t: "Update" });
          }}
        >
          Update cue {editing.cueNumber}
        </button>
      )}
    </form>
  );
}

/** The text a cell starts an edit with — the raw value, not the shown one. */
function rawOf(cue: CueRow, field: CellDraft["field"]): string {
  switch (field) {
    case "number":
      return cue.number;
    case "name":
      return cue.name;
    case "fadeIn":
      return String(cue.fadeIn);
    case "fadeOut":
      return String(cue.fadeOut);
    case "delay":
      return String(cue.delay);
  }
}

/**
 * The command a committed cell carries, or `null` when there is nothing to send.
 *
 * A time that is not a number is **not sent** rather than sent as zero: the
 * empty box is *no answer yet*, and a fade that silently became 0 s is a cue
 * that snaps in the middle of a show. What is *out of range* — a negative time,
 * a number already in use — is the daemon's to refuse, and it does.
 */
function propertyOf(field: CellDraft["field"], text: string): CueProperty | null {
  if (field === "number") {
    return { t: "Number", number: text };
  }
  if (field === "name") {
    return { t: "Name", name: text };
  }
  const seconds = Number(text.trim());
  if (text.trim() === "" || !Number.isFinite(seconds)) {
    return null;
  }
  if (field === "fadeIn") {
    return { t: "FadeIn", seconds };
  }
  if (field === "fadeOut") {
    return { t: "FadeOut", seconds };
  }
  return { t: "Delay", seconds };
}

/** A trigger this build knows, or the one a cue starts with. */
function triggerFrom(value: string): CueTrigger {
  return isCueTrigger(value) ? value : "Go";
}

/**
 * Whether a string is one of the triggers this build knows.
 *
 * A **type predicate** over the generated table, not an `as`: the value comes
 * off a `<select>` as a `string`, and claiming it is a `CueTrigger` would be a
 * claim rather than a check (S26's rule for every narrowing in this interface).
 */
function isCueTrigger(value: string): value is CueTrigger {
  return (CUE_TRIGGER_VARIANTS as readonly string[]).includes(value);
}
