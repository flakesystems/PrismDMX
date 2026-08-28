/**
 * The Cue Viewer: what the cues of the sequence in force actually set.
 *
 * # One row per cue, one column per attribute — S43, the owner's rebuild
 *
 * It used to be one row per **value**: cue, fixture, attribute, value, link. A
 * sequence of forty cues over a rig of twenty lamps is several thousand rows of
 * that, and the question an operator actually brings to this window — *what does
 * cue 7 change, and what does it leave alone?* — took scrolling through a
 * hundred lines to answer.
 *
 * So the shape is the owner's: *nur eine Zeile pro Cue und für jedes Attribut
 * eine Spalte*. A cue is a row, an attribute is a column, and a cell says what
 * that cue does to that attribute across the fixtures it touches. Read down a
 * column and you can see where a value is set and where it is left to track —
 * which is the reading S45 needs, because that session makes tracking
 * deterministic and this is the window it will be read in.
 *
 * # What a cell can say, and why a range is honest
 *
 * A cue sets one value per **fixture and** attribute, so a column holds as many
 * values as the cue has fixtures. Three cases, and each is drawn as itself:
 *
 * - **untouched** — a dash. The cue does not mention this attribute at all, so
 *   whatever a previous cue left stands. This is the reading the whole rebuild
 *   is for.
 * - **one value** — the percentage, whether one fixture carries it or twenty
 *   agree on it. The fixture count is in the title.
 * - **several** — the range, `10–80%`, with the count. Not an average, which
 *   would be a number no fixture is at; not the first, which would be a lie
 *   about the other nineteen. The title lists them fixture by fixture, which is
 *   where the detail went rather than where it was lost.
 *
 * # Every cue edit is here, and what a cue *sets* still is not
 *
 * The Sequence Sheet is the *pool* and this is the *list*. The rebuild moved the
 * editing: *alle Cue Editierungen sollen im Cue Viewer gemacht werden*. So this
 * window carries the cue's own properties — number, name, times and trigger,
 * the six columns before the attributes — the executor transport, and the store
 * bar, all three of which used to be stacked under the sequence pool in the
 * other window.
 *
 * What a cue **sets** is still not editable here, and that is the same rule
 * `Command::PatchFixture` follows in carrying no channels: values come from the
 * programmer through `StoreCue`, and a client that wrote one straight into a cue
 * would be authoring show content for the daemon to accept.
 *
 * # Which list, and which fader — two questions with two answers
 *
 * The list is `Session::selectedSequence` (S39), which the Sequence Sheet sets.
 * The transport follows the **selected executor**, which need not be running the
 * same list: that is the case a console has when one list is being programmed
 * while another is on stage, and a strip that borrowed this window's name would
 * tell an operator that the fader under their hand plays a list it does not
 * play.
 *
 * # The value beside a link is already the preset's
 *
 * A part carries a value *and* a `presetRef`, and the value is always current:
 * the daemon rewrites every linked part when the preset is stored
 * (`prism_core::Show::relink`). So this window reads the value and never looks
 * the preset up — resolving it here would be a second answer to a question the
 * show has already answered, and the two would disagree for exactly as long as a
 * round trip. What the link is for is being *visible*: a linked cell is marked,
 * because that is the difference between a look that can be re-coloured
 * everywhere and one that cannot.
 *
 * # Scrolling
 *
 * A rig of any size has more attributes than a window has width. The table
 * scrolls **inside its own window body**, in both directions, which is what a
 * window is for; `CLAUDE.md` forbids scrolling outside the canvas and
 * `e2e/desk.spec.ts` checks it in a browser at 1280x720 and at 4K.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type {
  AttributeType,
  CueProperty,
  CueTrigger,
  JsonValue,
  ProgrammerState,
  StoreMode,
  StorePreview,
} from "../bindings";
import { ATTRIBUTE_TYPE_VARIANTS, CUE_TRIGGER_VARIANTS } from "../bindings/variants";
import { objectLine, pick, useConsole } from "../desk/consoleshell";
import { percentOfLevel } from "../desk/level";
import { useAsk, useSend } from "../store/hooks";
import type { CueEditInForce, CuePartRow, CueRow, PresetRow, SequenceRow } from "./looks";
import {
  cueEditInForce,
  executorInForce,
  nextCueNumber,
  presetRows,
  secondsText,
  sequenceInForce,
  sequenceRow,
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
export function CueViewer({
  show,
  session,
  programmer,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
  /**
   * What the programmer is holding.
   *
   * Not drawn — this window is about the cue list — but the **store preview**
   * depends on it: what a store would add and replace changes the instant an
   * encoder moves, and the programmer is a document of its own
   * (`Delta::ProgrammerChanged`), so a bar watching only the show would go on
   * offering the answer to a question about a programmer that had since
   * emptied.
   */
  readonly programmer: ProgrammerState | null;
}) {
  const ask = useAsk();
  const { run } = useConsole();
  const inForce = useMemo(() => executorInForce(session, show), [session, show]);
  const chosen = useMemo(() => sequenceInForce(session), [session]);
  const editing = useMemo(() => cueEditInForce(session), [session]);
  const sequence = useMemo(() => sequenceRow(show, chosen), [show, chosen]);
  // The transport line names the executor's **own** cue list, which since S39
  // need not be the one being edited.
  const onExecutor = useMemo(
    () => sequenceRow(show, inForce.sequenceId),
    [show, inForce.sequenceId],
  );
  const presets = useMemo(() => presetRows(show), [show]);
  // The attributes this cue list touches, in the one order the desk has —
  // `AttributeType::ALL`, which is also the order the encoder banks walk. A
  // column order invented here would put pan beside dimmer on one screen and
  // not on another.
  const columns = useMemo(() => touchedAttributes(sequence), [sequence]);

  const transport = (
    <ExecutorLine
      executorId={inForce.executorId}
      sequenceName={onExecutor?.name ?? null}
      isActive={inForce.isActive}
      currentCueIndex={inForce.currentCueIndex}
      // The transport is a line as well, and can be: it names an executor
      // rather than being one. The **executor window's** own keys are the
      // exception §4.5 names, because a Go there is a gesture with timing in it
      // (§4.3) — this is a window, and a window is not a fader bank.
      onGo={(direction) => {
        if (inForce.executorId !== null) {
          run(`${direction === "Next" ? "Go+" : "Go-"} Executor ${String(inForce.executorId)}`);
        }
      }}
      onOff={() => {
        if (inForce.executorId !== null) {
          run(`Off Executor ${String(inForce.executorId)}`);
        }
      }}
    />
  );

  if (sequence === null) {
    return (
      <div className="looks" data-testid="cue-viewer">
        {transport}
        <p className="window-note" data-testid="cue-viewer-empty">
          No cue list is being edited. Choose one in the Sequence Sheet — a cue list does not need
          a fader to be written.
        </p>
      </div>
    );
  }
  const values = sequence.cues.reduce((count, cue) => count + cue.parts.length, 0);
  return (
    <div className="looks" data-testid="cue-viewer">
      <div className="looks-bar">
        <span className="looks-count" data-testid="cue-viewer-count">
          {sequence.name} · {sequence.cues.length} cues · {columns.length} attributes · {values}{" "}
          values
        </span>
      </div>
      {transport}
      {sequence.cues.length === 0 ? (
        <p className="window-note" data-testid="no-cues">
          {sequence.name} has no cues. Put a look in the programmer and store one below.
        </p>
      ) : (
        <div className="sheet-scroll" data-testid="cue-viewer-scroll">
          <CueGrid
            sequence={sequence}
            columns={columns}
            presets={presets}
            currentCueIndex={
              // The playback marker only where the two are looking at the same
              // list: an executor running sequence 3 says nothing about which
              // row of sequence 7 an operator is editing, and a marker drawn
              // from it would point at a cue nobody is on.
              inForce.isActive && inForce.sequenceId === sequence.id
                ? inForce.currentCueIndex
                : null
            }
          />
        </div>
      )}
      <StoreBar
        sequence={sequence}
        sequencesDoc={sequencesDocument(show)}
        programmer={programmer}
        editing={editing}
        ask={ask}
      />
    </div>
  );
}

/**
 * Which executor the window is following, and the three keys that fire it.
 *
 * **The cue number is the daemon's.** `Executor::currentCueIndex` was on the
 * wire from S1 with nothing filling it, so S26 and S28 both drew a dash here;
 * S34's readback out of the tick fills it. It is still not a number this window
 * may work out for itself — a follow cue advances without anybody pressing
 * anything.
 *
 * The three keys stay `ExecutorGo` and `ExecutorOff` rather than
 * `ExecutorButton`: this line is not a strip and has no button *positions* to
 * press. What it offers is *go*, *back* and *off* as themselves, which is what a
 * cue sheet's transport is.
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
        <button type="button" data-testid="looks-go" disabled={!playable} onClick={() => { onGo("Next"); }}>
          Go
        </button>
        <button type="button" data-testid="looks-back" disabled={!playable} onClick={() => { onGo("Prev"); }}>
          Back
        </button>
        <button type="button" data-testid="looks-off" disabled={!playable} onClick={onOff}>
          Off
        </button>
      </div>
    </div>
  );
}

/** The Store button, which says what it will do before it is pressed. */
function StoreBar({
  sequence,
  sequencesDoc,
  programmer,
  editing,
  ask,
}: {
  readonly sequence: SequenceRow;
  /**
   * The `/sequences` subtree, as the dependency of the question below.
   *
   * Not `sequence`, which is a fresh object every render: since S34 the show
   * document moves whenever a playback changes cue, so an effect keyed on it
   * would ask the daemon what a store would do **once per cue of a chase**.
   * This node's identity only changes when a sequence really does.
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
}) {
  const [number, setNumber] = useState<string | null>(null);
  const [mode, setMode] = useState<StoreMode>("Merge");
  const [preview, setPreview] = useState<StorePreview | null>(null);
  const wanted = number ?? nextCueNumber(sequence.cues);
  const { run, runWithMode } = useConsole();

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
    requester.current?.request({ t: "Cue", sequenceId: sequence.id, cueNumber: wanted }, mode);
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
        // **The store is a line too**, and the mode goes with it: the chooser
        // beside the button is where an operator picks one *before* pressing,
        // and the console's own prompt is what they get when they type the line
        // instead. Both end in a `StoreCue` carrying the mode.
        runWithMode(`Store Sequence ${String(sequence.id)} Cue ${wanted}`, mode);
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
          // keeps** — `Session::editingCue.modified`. Two screens looking at one
          // desk therefore blink together, and a client that had counted its
          // own keystrokes would drift the moment a console touched an encoder.
          className={`update-key${editing.modified ? " update-blinking" : ""}`}
          data-testid="update-cue"
          data-modified={editing.modified ? "yes" : "no"}
          title="Store the programmer back into the cue it came from"
          onClick={() => {
            // It carries nothing: which cue, and that the mode is Override, are
            // both the desk's — see `Command::Update`. A whole command with no
            // argument, so it runs at once (§4.5).
            run("Update");
          }}
        >
          Update cue {editing.cueNumber}
        </button>
      )}
    </form>
  );
}

/** The grid: cues down, attributes across. */
function CueGrid({
  sequence,
  columns,
  presets,
  currentCueIndex,
}: {
  readonly sequence: SequenceRow;
  readonly columns: readonly AttributeType[];
  readonly presets: readonly PresetRow[];
  /**
   * Which row the playback is standing on, or nothing when it is stopped.
   *
   * An **index** rather than a number, because that is what the tick reports and
   * what the show carries; the row it names is `cues[index]` in the order the
   * daemon compiled them, which is the order this table draws.
   */
  readonly currentCueIndex: number | null;
}) {
  const send = useSend();
  const shell = useConsole();
  const { run } = shell;
  const [draft, setDraft] = useState<CellDraft | null>(null);

  /**
   * **The cue number is an argument when the line is waiting for one** — S43's
   * second rebuild, `consoleshell.ts::pickOnto`.
   *
   * `Goto` and a click on cue 3 is `Goto Sequence 1 Cue 3`, and it goes; `Copy`
   * and two clicks is a copy. With nothing typed the cell is what it always was
   * — the editor for that cue's number — because a cue number is the one
   * reading in this window an operator changes by typing over it.
   *
   * The **full** form, `Sequence 1 Cue 3` rather than `Cue 3`: it names the list
   * this window is editing rather than whichever one is selected, which is the
   * same choice every other key in here makes.
   */
  const onPickCue = useCallback(
    (cue: CueRow, own: () => void) => {
      pick(
        shell,
        objectLine({ t: "Cue", sequenceId: sequence.id, cueNumber: cue.number }),
        own,
      );
    },
    [shell, sequence.id],
  );

  const commit = useCallback(
    (cue: CueRow, field: CellDraft["field"], text: string) => {
      setDraft(null);
      // **The number and the name are verbs of their own since S40.**
      // `CueProperty` lost both: renumbering a cue is `Move` and naming one is
      // `Label`, because those are the same act on a cue as on a sequence, a
      // group, a preset, a view and an executor. The number the cue **started**
      // at is what the line names — the trap an operator meets by changing
      // their mind halfway, which S27 met with Unpatch.
      if (field === "number") {
        const wanted = text.trim();
        if (wanted !== "" && wanted !== cue.number) {
          run(
            `Move Sequence ${String(sequence.id)} Cue ${cue.number} Sequence ${String(sequence.id)} Cue ${wanted}`,
          );
        }
        return;
      }
      if (field === "name") {
        run(`Label Sequence ${String(sequence.id)} Cue ${cue.number} ${JSON.stringify(text)}`);
        return;
      }
      // The three times stay `SetCueProperty`. They carry a *value* rather than
      // naming a place, and S40's vocabulary has no word for a fade — inventing
      // one would be a second grammar for something no console types.
      const property = propertyOf(field, text);
      if (property === null) {
        return;
      }
      send({
        t: "SetCueProperty",
        sequenceId: sequence.id,
        cueNumber: cue.number,
        property,
      });
    },
    [run, send, sequence.id],
  );

  return (
    <table className="sheet cue-grid">
      <thead>
        <tr>
          <th scope="col">Q</th>
          <th scope="col">Name</th>
          <th scope="col">In</th>
          <th scope="col">Out</th>
          <th scope="col">Delay</th>
          <th scope="col">Trigger</th>
          {columns.map((attribute) => (
            <th scope="col" key={attribute} data-testid={`cue-column-${attribute}`}>
              {attribute}
            </th>
          ))}
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
              onPick={onPickCue}
            />
            <Cell
              cue={cue}
              field="name"
              shown={cue.name === "" ? "—" : cue.name}
              draft={draft}
              onEdit={setDraft}
              onCommit={commit}
            />
            <Cell cue={cue} field="fadeIn" shown={secondsText(cue.fadeIn)} draft={draft} onEdit={setDraft} onCommit={commit} />
            <Cell cue={cue} field="fadeOut" shown={secondsText(cue.fadeOut)} draft={draft} onEdit={setDraft} onCommit={commit} />
            <Cell cue={cue} field="delay" shown={secondsText(cue.delay)} draft={draft} onEdit={setDraft} onCommit={commit} />
            <td>
              <TriggerCell
                cue={cue}
                onChange={(trigger, triggerTime) => {
                  send({
                    t: "SetCueProperty",
                    sequenceId: sequence.id,
                    cueNumber: cue.number,
                    property: { t: "Trigger", trigger, triggerTime },
                  });
                }}
              />
            </td>
            {columns.map((attribute) => (
              <ValueCell
                key={attribute}
                cue={cue}
                attribute={attribute}
                presets={presets}
              />
            ))}
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
                  // kept, and the desk remembers which cue it came from.
                  run(`Edit Sequence ${String(sequence.id)} Cue ${cue.number}`);
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
                  run(`Delete Sequence ${String(sequence.id)} Cue ${cue.number}`);
                }}
              >
                ×
              </button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** One attribute of one cue, across every fixture the cue touches. */
function ValueCell({
  cue,
  attribute,
  presets,
}: {
  readonly cue: CueRow;
  readonly attribute: AttributeType;
  readonly presets: readonly PresetRow[];
}) {
  const parts = cue.parts.filter((part) => part.attribute === attribute);
  const testId = `cue-${cue.number}-${attribute}`;
  if (parts.length === 0) {
    // **The reading the rebuild is for.** This cue says nothing about this
    // attribute, so whatever came before stands — which is what tracking is,
    // and what S45 makes deterministic.
    return (
      <td className="cue-untouched" data-testid={testId} data-set="no" title="not set by this cue">
        —
      </td>
    );
  }
  const linked = parts.some((part) => part.presetRef !== null);
  return (
    <td
      className={linked ? "cue-value cue-linked" : "cue-value"}
      data-testid={testId}
      data-set="yes"
      data-linked={linked ? "yes" : "no"}
      title={cellTitle(parts, presets)}
    >
      {cellText(parts)}
    </td>
  );
}

/**
 * What a cell says: one percentage, or the range the fixtures span.
 *
 * Not an average — that is a number no fixture is at — and not the first, which
 * would be a claim about the other nineteen. See the module documentation.
 */
function cellText(parts: readonly CuePartRow[]): string {
  const percents = parts.map((part) => percentOfLevel(part.value));
  const low = Math.min(...percents);
  const high = Math.max(...percents);
  return low === high ? `${String(low)}%` : `${String(low)}–${String(high)}%`;
}

/** The detail the cell has no room for: every fixture, and every link. */
function cellTitle(parts: readonly CuePartRow[], presets: readonly PresetRow[]): string {
  return parts
    .map((part) => {
      const link = linkText(part.presetRef, presets);
      return `Fixture ${String(part.fixture)}: ${String(percentOfLevel(part.value))}%${
        link === "" ? "" : ` · ${link}`
      }`;
    })
    .join("\n");
}

/**
 * Which attributes a cue list touches, in `AttributeType::ALL`'s order.
 *
 * The order is the generated one and not the order values happen to appear in,
 * for the reason `FeatureGroup::attributes` gives: the desk has **one** order
 * for attributes, and a second one invented in a view would put the columns of
 * two screens in different places.
 */
function touchedAttributes(sequence: SequenceRow | null): readonly AttributeType[] {
  if (sequence === null) {
    return [];
  }
  const touched = new Set<AttributeType>();
  for (const cue of sequence.cues) {
    for (const part of cue.parts) {
      touched.add(part.attribute);
    }
  }
  return ATTRIBUTE_TYPE_VARIANTS.filter((attribute) => touched.has(attribute));
}

/**
 * What a link says, or an empty string for a value nobody linked.
 *
 * The preset's number and name for one that is linked, and the **number alone**
 * for a link to a preset that has gone — `prism_core::Show::remove_preset`
 * leaves the value and the link, and `Show::issues` reports the dangling
 * reference, so a viewer that drew nothing there would be hiding exactly the
 * state an operator has to go and fix.
 */
function linkText(presetRef: number | null, presets: readonly PresetRow[]): string {
  if (presetRef === null) {
    return "";
  }
  const preset = presets.find((row) => row.id === presetRef);
  if (preset === undefined) {
    return `preset ${String(presetRef)} (missing)`;
  }
  return preset.name === ""
    ? `preset ${String(preset.id)}`
    : `preset ${String(preset.id)} ${preset.name}`;
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
  onPick,
}: {
  readonly cue: CueRow;
  readonly field: CellDraft["field"];
  readonly shown: string;
  readonly draft: CellDraft | null;
  readonly onEdit: (draft: CellDraft | null) => void;
  readonly onCommit: (cue: CueRow, field: CellDraft["field"], text: string) => void;
  /**
   * Offers the cell to the command line first — the **number** column only.
   *
   * A name, a fade or a delay is not a word the grammar has, so those cells are
   * editors whatever the line says. The number is a cue reference, which is the
   * one thing in this table a half-typed verb can be waiting for.
   */
  readonly onPick?: (cue: CueRow, own: () => void) => void;
}) {
  const editing = draft !== null && draft.cueNumber === cue.number && draft.field === field;
  const testId = `cue-${field}-${cue.number}`;
  if (!editing) {
    const edit = (): void => {
      onEdit({ cueNumber: cue.number, field, text: rawOf(cue, field) });
    };
    return (
      <td>
        <button
          type="button"
          className="linkish cell"
          data-testid={testId}
          onClick={() => {
            if (onPick === undefined) {
              edit();
              return;
            }
            onPick(cue, edit);
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
  if (field === "number" || field === "name") {
    // Both are lines — see `commit`.
    return null;
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
