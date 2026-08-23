/**
 * The *Controls* panel: the binding table of `docs/MCU_MAPPING.md` §4, edited at
 * the desk — S38.
 *
 * # The table is asked for, and that is the decision rather than a detail
 *
 * What a desk's keys do is **derived**: §4.1's built-in defaults, or a profile
 * file read into this machine's own rows, or the rows an operator has typed
 * since. A client that layered those three for itself would be a second opinion
 * about something `prism_surface::Bindings` already decides — the trap
 * `PatchPreview` was built to avoid one panel along. So the panel asks
 * (`Query::SurfaceBindings`) when it opens and asks again whenever
 * `Delta::SurfaceBindingsChanged` says the table moved.
 *
 * That delta is also the whole of *two clients do not produce two tables*. Every
 * edit is one `MachineChange::SurfaceBinding` naming one control, so two
 * operators changing two keys cannot undo each other; both are told the same
 * revision; and both re-ask and draw the same rows.
 *
 * # Learn is the daemon's, not this panel's
 *
 * S20 established that the way to find out what a control sends is to press it
 * and read what arrives, never to ask the profile. Learn is that rule run in the
 * other direction, and it is armed on the **daemon** because there is one desk:
 * two editors must not both believe they have armed it, and the operator
 * standing at the console has no idea which browser asked. While it is armed the
 * control does not fire — an operator finding out what the Record key is called
 * must not clear their programmer to find out.
 *
 * # Two columns this panel draws and does not compute
 *
 * *In shared mode* and *reserved* are properties of the **device profile**
 * (§4.3), and this client holds none. Telling an operator that a key is always
 * in reach while the surface is showing the sound console is exactly the mistake
 * that section exists to prevent, so both arrive on the row.
 *
 * # What scrolls, scrolls inside this window
 *
 * Seventy-three rows is more than any window holds, and `CLAUDE.md` forbids
 * scrolling outside the canvas. They scroll in the window's own body, with
 * every other panel's rows — **one `overflow` per window**, so a settings window
 * never nests two scrollbars — and the document and the canvas read zero.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import type {
  Answer,
  BoundControl,
  ExecutorButtonFunction,
  FeatureGroup,
  SurfaceAction,
  SurfaceControl,
  WindowType,
} from "../bindings";
import {
  EXECUTOR_BUTTON_FUNCTION_VARIANTS,
  FEATURE_GROUP_VARIANTS,
  WINDOW_TYPE_VARIANTS,
} from "../bindings";
import { useAsk, useDesk, useSend } from "../store/hooks";
import type { DeskState } from "../store/desk";
import { ACTION_KINDS, actionOfKind, actionText, kindOf, sameControl, slotOf } from "./actions";
import type { ActionKind } from "./actions";
import { heldNote, isHeld } from "./settings";

const selectRevision = (state: DeskState): number => state.surfaceBindings;
const selectLearning = (state: DeskState): boolean => state.surfaceLearning;
const selectMachine = (state: DeskState) => state.machine;
const selectLearned = (state: DeskState): BoundControl | null => state.surfaceLearned;

/** What the daemon last said the table is. */
interface Table {
  readonly controls: readonly SurfaceControl[];
  readonly device: string;
  readonly profile: string | null;
  readonly revision: number;
  readonly learning: boolean;
}

const NOTHING: Table = {
  controls: [],
  device: "",
  profile: null,
  revision: 0,
  learning: false,
};

/** The answer, if it is the one that was asked for. */
function tableOf(answer: Answer | null): Table | null {
  return answer !== null && answer.t === "SurfaceBindings"
    ? {
        controls: answer.controls,
        device: answer.device,
        profile: answer.profile,
        revision: answer.revision,
        learning: answer.learning,
      }
    : null;
}

/** The groups a person looks for a control in, in panel order. */
const GROUPS: readonly { readonly title: string; readonly of: (name: string) => boolean }[] = [
  { title: "Channel strips", of: (name) => name.startsWith("Strip[*]") },
  { title: "Main fader and jog wheel", of: (name) => name === "Main.Fader" || name === "Global.Jog" },
  { title: "The panel", of: (name) => name.startsWith("Global.") && name !== "Global.Jog" },
];

/** The whole panel. */
export function ControlsPanel() {
  const ask = useAsk();
  const send = useSend();
  const revision = useDesk(selectRevision);
  const learning = useDesk(selectLearning);
  const machine = useDesk(selectMachine);
  const learned = useDesk(selectLearned);
  const [table, setTable] = useState<Table>(NOTHING);
  const [chosen, setChosen] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    void ask({ t: "SurfaceBindings" }).then((answer) => {
      const read = tableOf(answer);
      if (current && read !== null) {
        setTable(read);
      }
    });
    return () => {
      current = false;
    };
    // `revision` is what a change — anybody's — moves. Nothing else is a reason
    // to ask again, and asking on every render would be a query at frame rate.
  }, [ask, revision]);

  const held = isHeld(machine, "SurfaceProfile");

  const bind = useCallback(
    (control: BoundControl, action: SurfaceAction | null) => {
      send({ t: "ConfigureMachine", change: { t: "SurfaceBinding", control, action } });
    },
    [send],
  );

  // **Learn names a control; this is what the panel does about it.** The
  // operator pressed a key, so they are put in front of that key's row — and the
  // row is found by matching the control the daemon named rather than by this
  // file rendering a name of its own, because the name is the daemon's.
  useEffect(() => {
    if (learned === null) {
      return;
    }
    const row = table.controls.find((entry) => sameControl(entry.control, learned));
    if (row !== undefined) {
      setChosen(row.name);
    }
  }, [learned, table.controls]);

  const learn = useCallback(
    (on: boolean) => {
      send({ t: "ConfigureMachine", change: { t: "SurfaceLearn", learning: on } });
    },
    [send],
  );

  const selected = useMemo(
    () => table.controls.find((row) => row.name === chosen) ?? null,
    [table.controls, chosen],
  );

  const bound = table.controls.filter((row) => row.action !== null).length;

  return (
    <div className="settings-panel" data-testid="settings-controls">
      <div className="settings-bar">
        <span data-testid="controls-device">{table.device === "" ? "No surface" : table.device}</span>
        <span data-testid="controls-bound">{bound} bound</span>
        <span data-testid="controls-revision">rev {table.revision}</span>
        <button
          type="button"
          disabled={held}
          data-testid="controls-learn"
          aria-pressed={learning}
          onClick={() => {
            learn(!learning);
          }}
        >
          {learning ? "Press a control…" : "Learn"}
        </button>
      </div>
      {held ? (
        <p className="settings-hint" data-testid="controls-held">
          {heldNote(machine, "SurfaceProfile")}, so the table is that file&apos;s for this run and
          cannot be edited here. Start the daemon without that flag to edit the controls.
        </p>
      ) : (
        <p className="settings-hint" data-testid="controls-source">
          {table.profile === null
            ? "This desk's own table. It started from the built-in bindings."
            : `This desk's own table, last read from ${table.profile}. Naming a file again in Devices re-reads it, which replaces every row.`}
        </p>
      )}
      {learning ? (
        <p className="settings-warning" role="status" data-testid="controls-learning">
          Learn is armed: press a control on the desk and it will be named here rather than doing
          what it is bound to.
        </p>
      ) : null}
      <div className="settings-list" data-testid="controls-list">
        {GROUPS.map((group) => (
          <section key={group.title}>
            <h3>{group.title}</h3>
            <table className="sheet">
              <thead>
                <tr>
                  <th scope="col">Control</th>
                  <th scope="col">Does</th>
                  <th scope="col">In shared mode</th>
                  <th scope="col" />
                </tr>
              </thead>
              <tbody>
                {table.controls
                  .filter((row) => group.of(row.name))
                  .map((row) => (
                    <Row
                      key={row.name}
                      row={row}
                      chosen={row.name === chosen}
                      onChoose={() => {
                        setChosen(row.name === chosen ? null : row.name);
                      }}
                    />
                  ))}
              </tbody>
            </table>
          </section>
        ))}
      </div>
      {selected === null ? null : <Editor row={selected} held={held} onBind={bind} />}
    </div>
  );
}

/** One control, as the list draws it. */
function Row({
  row,
  chosen,
  onChoose,
}: {
  readonly row: SurfaceControl;
  readonly chosen: boolean;
  readonly onChoose: () => void;
}) {
  return (
    <tr
      className={chosen ? "row-editing" : row.reserved ? "row-conflict" : ""}
      data-testid={`control-${row.name}`}
    >
      <td>{row.name}</td>
      <td data-testid={`control-does-${row.name}`}>{actionText(row.action)}</td>
      {/* §4.3's ownership, and it is the daemon's answer rather than this
          file's: a client holds no device profile, and a key drawn as always in
          reach when it is not is the mistake that section exists to prevent. */}
      <td data-testid={`control-owned-${row.name}`}>
        {row.permanent ? "always ours" : "follows the switch"}
      </td>
      <td>
        <button
          type="button"
          disabled={row.reserved}
          data-testid={`control-choose-${row.name}`}
          onClick={onChoose}
          title={row.reserved ? "Reserved — it switches the desk between hosts" : undefined}
        >
          {row.reserved ? "Reserved" : chosen ? "Close" : "Change"}
        </button>
      </td>
    </tr>
  );
}

/** The chooser for one control, over the whole vocabulary of §4. */
function Editor({
  row,
  held,
  onBind,
}: {
  readonly row: SurfaceControl;
  readonly held: boolean;
  readonly onBind: (control: BoundControl, action: SurfaceAction | null) => void;
}) {
  const [kind, setKind] = useState<ActionKind>(kindOf(row.action));
  const [target, setTarget] = useState<"Strip" | "Selected">(
    row.action !== null && "target" in row.action ? row.action.target : "Selected",
  );
  const [detail, setDetail] = useState<string>("");

  // The chooser follows the row: picking a different control has to redraw it
  // with that control's action rather than with the last one's.
  const name = row.name;
  useEffect(() => {
    setKind(kindOf(row.action));
    setDetail("");
    // `name` is the identity of the row; `row.action` moves under it when a
    // second client edits the same control, and that should redraw this too.
  }, [name, row.action]);

  return (
    <form
      className="settings-form"
      data-testid="control-editor"
      onSubmit={(event) => {
        event.preventDefault();
        onBind(row.control, actionOfKind(kind, target, detail, slotOf(row.name)));
      }}
    >
      <fieldset disabled={held}>
        <legend data-testid="control-editor-name">{row.name}</legend>
        <label>
          Does
          <select
            data-testid="control-action"
            value={kind}
            onChange={(event) => {
              // Narrowed against the list the options were drawn from rather
              // than asserted into the type: `CLAUDE.md` forbids `any` and `as`
              // is a claim, not a check.
              const picked = ACTION_KINDS.find((option) => option === event.target.value);
              if (picked !== undefined) {
                setKind(picked);
                setDetail("");
              }
            }}
          >
            {ACTION_KINDS.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
        {NEEDS_TARGET.has(kind) ? (
          <label>
            On
            <select
              data-testid="control-target"
              value={target}
              onChange={(event) => {
                setTarget(event.target.value === "Strip" ? "Strip" : "Selected");
              }}
            >
              <option value="Strip">the executor under this strip</option>
              <option value="Selected">the selected executor</option>
            </select>
          </label>
        ) : null}
        <Detail kind={kind} detail={detail} onDetail={setDetail} />
      </fieldset>
      <div className="settings-actions">
        <button type="submit" data-testid="control-apply" disabled={held}>
          Bind it
        </button>
        <button
          type="button"
          data-testid="control-clear"
          disabled={held || row.action === null}
          onClick={() => {
            onBind(row.control, null);
          }}
        >
          Unbind
        </button>
      </div>
    </form>
  );
}

/** The kinds that act on an executor and therefore need to be told which. */
const NEEDS_TARGET: ReadonlySet<ActionKind> = new Set<ActionKind>([
  "Executor master",
  "Executor go +",
  "Executor go −",
  "Executor off",
  "Executor button",
  "Select executor",
]);

/** The one extra answer a kind needs, where it needs one. */
function Detail({
  kind,
  detail,
  onDetail,
}: {
  readonly kind: ActionKind;
  readonly detail: string;
  readonly onDetail: (value: string) => void;
}) {
  if (kind === "Executor button") {
    return (
      <label>
        Button
        <select
          data-testid="control-detail"
          value={detail}
          onChange={(event) => {
            onDetail(event.target.value);
          }}
        >
          <option value="">the key in this position (the executor decides)</option>
          {EXECUTOR_BUTTON_FUNCTION_VARIANTS.map((option: ExecutorButtonFunction) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </label>
    );
  }
  if (kind === "Open window") {
    return (
      <label>
        Window
        <select
          data-testid="control-detail"
          value={detail}
          onChange={(event) => {
            onDetail(event.target.value);
          }}
        >
          <option value="">choose one</option>
          {WINDOW_TYPE_VARIANTS.map((option: WindowType) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </label>
    );
  }
  if (kind === "Encoder bank") {
    return (
      <label>
        Bank
        <select
          data-testid="control-detail"
          value={detail}
          onChange={(event) => {
            onDetail(event.target.value);
          }}
        >
          <option value="">choose one</option>
          {FEATURE_GROUP_VARIANTS.map((option: FeatureGroup) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </label>
    );
  }
  if (kind === "Jump to view") {
    return (
      <label>
        View
        <input
          data-testid="control-detail"
          value={detail}
          inputMode="numeric"
          placeholder="1"
          onChange={(event) => {
            onDetail(event.target.value);
          }}
        />
      </label>
    );
  }
  return null;
}
