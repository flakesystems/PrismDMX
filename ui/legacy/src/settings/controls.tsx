/**
 * The *Controls* panel: the binding table of `docs/MCU_MAPPING.md` §4, edited at
 * the desk — S38, turned round in S43 and laid out again for the owner's
 * rebuild.
 *
 * # The rows are actions, and the key is learned onto them — punch-list B3
 *
 * S38 drew this panel the way `prism_surface::Bindings` stores the table: one
 * row per **control**, seventy-three of them, each with a chooser of what it
 * should do. The owner's entry says that is backwards, and it is. Nobody comes
 * to this panel wanting to know what F5 does. They come wanting *Go on the
 * selected executor*, and needing a key for it.
 *
 * So the list is the **vocabulary** (`actions.ts`), and each row carries the
 * keys that are bound to it. Learn is on the row: press *Learn*, press the key
 * you mean, and the key is bound to that row. That is S20's method rule — *find
 * out what a control is by pressing it, never by asking the profile* — pointed
 * at the operator's actual question.
 *
 * # Four sections, and the fourth is a different shape
 *
 * *Es sollte eingeteilt werden in Executorbereich, Programmerbereich, Andere
 * Interne Befehle und Custom Befehle.* The first three are the desk's fixed
 * vocabulary and are rows: an action, the keys on it, a Learn.
 *
 * The fourth is not, and `actions.ts::CUSTOM_KINDS` says why: *open window* is
 * fourteen bindings and *type a command* is as many as an operator can think of,
 * so there the **key** is the row, each carrying its own answer, and a `+` adds
 * one. That is the owner's own description of it.
 *
 * # Why the panel had no structure before, and what fixed it
 *
 * *Die Controls Seite in den Settings ist sehr unstrukturiert und unübersichtlich
 * und alles hat verschiedene Längen und Breiten.* Two causes, both real. The
 * sections were four `<table>`s each sizing its own columns, so the *Action*
 * column was one width under Playback and another under The show and the eye had
 * nothing to run down. And the controls inside the cells were the browser's:
 * un-styled `<select>`s and `<input>`s of whatever width their contents wanted.
 *
 * The fix is a **grid with named columns shared by every section** — one
 * `grid-template-columns` on the list, so a row is four cells wherever it is and
 * the four line up down the whole panel — and `App.css`'s control base, which
 * gives every key and box one look. Neither is a table any more, because a table
 * cannot share its column widths with the table above it.
 *
 * # Export and import
 *
 * *Die Controls sollen exportiert und importiert werden können.* Both are here,
 * and both are **files in the browser's own dialogue** rather than paths typed
 * for the daemon to resolve, because a control map is a file an operator carries
 * between desks: it goes to the machine they are sitting at.
 *
 * What is written is a **profile file** — the same shape `prism_surface::
 * Bindings::parse` reads and `profiles/surface/xtouch.json` is written in — so
 * an export can also be handed to the daemon with `--surface-profile` or named
 * in *Devices*. The `device` key and the `profileVersion` come off the daemon's
 * own answer rather than being written here; see `Answer::SurfaceBindings`.
 *
 * An import sends **one `SurfaceBinding` per control**, which is this protocol's
 * own rule (`MachineChange::SurfaceBinding`: one control at a time, so two
 * editors cannot each undo the other). Every control the surface has is sent,
 * bound or not, so the result is the file's table exactly rather than the file
 * merged over whatever was there.
 *
 * # Learn is the daemon's, not this panel's
 *
 * S20 established that the way to find out what a control sends is to press it
 * and read what arrives, never to ask the profile. Learn is that rule run in the
 * other direction, and it is armed on the **daemon** because there is one desk:
 * two editors must not both believe they have armed it, and the operator
 * standing at the console has no idea which browser asked. While it is armed the
 * control does not fire — an operator finding out what the Record key is called
 * must not clear their programmer to find out. It is one shot: the daemon
 * disarms itself the moment a control is named.
 *
 * What *is* local is which row armed it, and that is §4.2's category exactly —
 * a half-finished gesture, like the patch window's draft row. It is dropped when
 * the binding is made and when the panel goes away.
 *
 * # Two columns this panel draws and does not compute
 *
 * *In shared mode* and *reserved* are properties of the **device profile**
 * (§4.3), and this client holds none. Telling an operator that a key is always
 * in reach while the surface is showing the sound console is exactly the mistake
 * that section exists to prevent, so both arrive on the row — on the key chip,
 * which is where the fact belongs now that the key is not the row.
 *
 * # What scrolls, scrolls inside this window
 *
 * `CLAUDE.md` forbids scrolling outside the canvas. The rows scroll in the
 * window's own body, with every other panel's — **one `overflow` per window**,
 * so a settings window never nests two scrollbars — and the document and the
 * canvas read zero.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type {
  Answer,
  BoundControl,
  FeatureGroup,
  KeyShape,
  PanelLayout,
  SurfaceAction,
  SurfaceControl,
  WindowType,
} from "../bindings";
import {
  CONSOLE_KEYS,
  FEATURE_GROUP_VARIANTS,
  WINDOW_TYPE_VARIANTS,
} from "../bindings";
import { logger } from "../log/logger";
import { useAsk, useDesk, useSend } from "../store/hooks";
import type { DeskState } from "../store/desk";
import {
  ACTION_GROUPS,
  ACTION_KINDS,
  CUSTOM_KINDS,
  actionOfKind,
  actionText,
  detailOf,
  isCustom,
  kindOf,
  slotOfControl,
  submitOf,
} from "./actions";
import type { ActionKind, Target } from "./actions";
import { profileDocument, readProfile } from "./controlfile";
import { DeskDrawing } from "./panel";
import { heldNote, isHeld } from "./settings";
import { titleOf } from "../desk/keys";
import { FIXED_BUTTON_FUNCTIONS } from "../desk/functions";
import type { FixedButtonFunction } from "../desk/functions";

const log = logger("controls");

const selectRevision = (state: DeskState): number => state.surfaceBindings;
const selectLearning = (state: DeskState): boolean => state.surfaceLearning;
const selectMachine = (state: DeskState) => state.machine;
const selectLearned = (state: DeskState): BoundControl | null => state.surfaceLearned;
const selectLamps = (state: DeskState): readonly string[] => state.surfaceLamps;

/** What the daemon last said the table is. */
interface Table {
  readonly controls: readonly SurfaceControl[];
  /** The panel a drawing is drawn on, or `null` for a device nobody has drawn. */
  readonly panel: PanelLayout | null;
  readonly device: string;
  readonly deviceKey: string;
  readonly profileVersion: number;
  readonly profile: string | null;
  readonly revision: number;
  readonly learning: boolean;
}

const NOTHING: Table = {
  controls: [],
  panel: null,
  device: "",
  deviceKey: "",
  profileVersion: 0,
  profile: null,
  revision: 0,
  learning: false,
};

/** The answer, if it is the one that was asked for. */
function tableOf(answer: Answer | null): Table | null {
  return answer !== null && answer.t === "SurfaceBindings"
    ? {
        controls: answer.controls,
        panel: answer.panel,
        device: answer.device,
        deviceKey: answer.deviceKey,
        profileVersion: answer.profileVersion,
        profile: answer.profile,
        revision: answer.revision,
        learning: answer.learning,
      }
    : null;
}

/**
 * What a row is waiting for: the kind, and the answers a kind can need.
 *
 * Held per kind rather than per panel, so an operator who has set *Encoder bank*
 * to `Color` and then goes to set *Executor button* comes back to `Color` still
 * chosen. It is local and it is dropped with the panel — §4.2.
 */
interface RowState {
  readonly target: Target;
  readonly detail: string;
  /** Whether a bound line runs itself — only *Type a command* reads it. */
  readonly submit: boolean;
}

const FRESH: RowState = { target: "Selected", detail: "", submit: false };

/** Which row, which keypad word, or which custom draft has armed learn. */
type Arming =
  | { readonly t: "kind"; readonly kind: ActionKind }
  | { readonly t: "word"; readonly word: string }
  | { readonly t: "new" };

/** Whether two armings are the same row, so that a second press gives up. */
function sameArming(one: Arming, two: Arming): boolean {
  if (one.t !== two.t) {
    return false;
  }
  if (one.t === "kind" && two.t === "kind") {
    return one.kind === two.kind;
  }
  if (one.t === "word" && two.t === "word") {
    return one.word === two.word;
  }
  return true;
}

/** The whole panel. */
export function ControlsPanel() {
  const ask = useAsk();
  const send = useSend();
  const revision = useDesk(selectRevision);
  const learning = useDesk(selectLearning);
  const machine = useDesk(selectMachine);
  const learned = useDesk(selectLearned);
  const lamps = useDesk(selectLamps);
  const [drawing, setDrawing] = useState(false);
  const [table, setTable] = useState<Table>(NOTHING);
  const [rows, setRows] = useState<Readonly<Record<string, RowState>>>({});
  const [arming, setArming] = useState<Arming | null>(null);
  const [draft, setDraft] = useState<{ readonly kind: ActionKind } & RowState>({
    kind: "Type a command",
    ...FRESH,
  });
  const [note, setNote] = useState<string | null>(null);

  /**
   * **What learn had already named by the time this row armed it.**
   *
   * `surfaceLearned` is a *moment* — `store/desk.ts` says so in as many words
   * — but it is carried in the store as a value, so the control named by the
   * last learn is still standing there when somebody arms the next one. Without
   * this, arming a row would bind that stale key the instant it was armed,
   * before the operator had pressed anything, and from a screen that had
   * nothing to do with the press. Two editors are exactly where it bites: the
   * daemon broadcasts the named control to **every** client, so the second
   * screen is holding a key it never learned.
   *
   * The daemon does clear it — arming broadcasts `learning: true` with no
   * control — but that is a round trip away and the effect below runs on the
   * render that arms. So the value in force at the moment of arming is
   * remembered here and ignored when it comes round.
   */
  const alreadyNamed = useRef<BoundControl | null>(null);

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

  const learn = useCallback(
    (on: boolean) => {
      send({ t: "ConfigureMachine", change: { t: "SurfaceLearn", learning: on } });
    },
    [send],
  );

  /**
   * **Learn named a control; this is what the panel does about it.**
   *
   * The row that armed it decides what the key becomes, and the *key* decides
   * the one thing a row cannot know: which of a strip's five positions it is,
   * for the positional executor-button binding that is **D3** for playback.
   *
   * A control named while nothing is arming is left alone rather than bound to
   * whatever row was last touched: learn can be armed from a second client, and
   * this one has no business acting on it.
   */
  useEffect(() => {
    if (learned === null || arming === null || learned === alreadyNamed.current) {
      return;
    }
    // A keypad row's answer is its word and nothing else — there is no box on
    // it to fill in, which is what makes it a row rather than a chooser.
    const action =
      arming.t === "word"
        ? actionOfKind("Console key", "Selected", arming.word)
        : (() => {
            const state = arming.t === "new" ? draft : (rows[arming.kind] ?? FRESH);
            const kind = arming.t === "new" ? draft.kind : arming.kind;
            return actionOfKind(
              kind,
              targetOf(learned),
              state.detail,
              slotOfControl(learned),
              state.submit,
            );
          })();
    if (action !== null) {
      bind(learned, action);
    }
    setArming(null);
  }, [learned, arming, rows, draft, bind]);

  /**
   * Which keys are bound to each kind, out of the daemon's own answer.
   *
   * Built per render rather than stored: it is a reading of `table.controls`,
   * and a copy kept beside it would be a second table to keep in step — the
   * fault this panel's own documentation warns about.
   */
  const boundTo = useMemo(() => {
    const index = new Map<ActionKind, SurfaceControl[]>();
    for (const control of table.controls) {
      if (control.action === null) {
        continue;
      }
      const kind = kindOf(control.action);
      // **The keypad words are indexed by word, not by kind** — S59. They are
      // one `ActionKind` carrying twenty-three different details, so a single
      // bucket would collect every bound word into whichever row drew first.
      if (kind === "Console key") {
        continue;
      }
      const held_ = index.get(kind);
      if (held_ === undefined) {
        index.set(kind, [control]);
      } else {
        held_.push(control);
      }
    }
    return index;
  }, [table.controls]);

  /** Which keys are bound to each word of the keypad — S59. */
  const boundToWord = useMemo(() => {
    const index = new Map<string, SurfaceControl[]>();
    for (const control of table.controls) {
      if (control.action === null || control.action.t !== "ConsoleWord") {
        continue;
      }
      const held_ = index.get(control.action.word);
      if (held_ === undefined) {
        index.set(control.action.word, [control]);
      } else {
        held_.push(control);
      }
    }
    return index;
  }, [table.controls]);

  /**
   * The controls the advanced section owns, in the table's own order — S59.
   *
   * Read off `table.controls` rather than listed here, so a device whose
   * profile has no jog wheel simply has no jog row: this panel holds no model
   * of what a surface is made of, which is §4.3's rule and the reason
   * `SurfaceControl` carries `permanent` and `reserved` at all.
   */
  const advanced = useMemo(
    () => table.controls.filter((control) => isAdvanced(control.control)),
    [table.controls],
  );

  /** Every key bound to one of the custom kinds, in the table's own order. */
  const custom = useMemo(
    () =>
      table.controls.filter(
        (control) => control.action !== null && isCustom(kindOf(control.action)),
      ),
    [table.controls],
  );

  const bound = table.controls.filter((row) => row.action !== null).length;

  const arm = useCallback(
    (next: Arming) => {
      const same = arming !== null && sameArming(arming, next);
      if (same) {
        setArming(null);
        learn(false);
        return;
      }
      alreadyNamed.current = learned;
      setArming(next);
      learn(true);
    },
    [arming, learn, learned],
  );

  /** Writes the table out as a profile file, through the browser's own dialogue. */
  const onExport = useCallback(() => {
    const text = profileDocument(table.deviceKey, table.profileVersion, table.controls);
    const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `${table.deviceKey === "" ? "surface" : table.deviceKey}.json`;
    link.click();
    URL.revokeObjectURL(url);
    setNote(`${String(bound)} bindings written to ${link.download}.`);
  }, [bound, table]);

  /**
   * Reads a profile file back, one `SurfaceBinding` per control.
   *
   * **Every control the surface has**, not only the ones the file mentions: an
   * import is *this table becomes that table*, and a merge would leave whatever
   * the operator had bound in the gaps — which is the state neither file
   * describes.
   */
  const onImport = useCallback(
    (file: File) => {
      void file.text().then((text) => {
        const read = readProfile(text, table.deviceKey, table.profileVersion);
        if (typeof read === "string") {
          setNote(read);
          log.warn("a control map could not be read", { why: read });
          return;
        }
        for (const control of table.controls) {
          bind(control.control, read.get(control.name) ?? null);
        }
        setNote(`${String(read.size)} bindings read from ${file.name}.`);
      });
    },
    [bind, table],
  );

  return (
    <div className="settings-panel" data-testid="settings-controls">
      <div className="settings-bar">
        <span data-testid="controls-device">
          {table.device === "" ? "No surface" : table.device}
        </span>
        <span data-testid="controls-bound">{bound} bound</span>
        <span data-testid="controls-revision">rev {table.revision}</span>
        <ProfileFile held={held} onExport={onExport} onImport={onImport} />
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
      {note === null ? null : (
        <p className="settings-hint" role="status" data-testid="controls-note">
          {note}
        </p>
      )}
      {learning ? (
        <p className="settings-warning" role="status" data-testid="controls-learning">
          {arming === null
            ? "Learn is armed from another client: the next control pressed will be named there."
            : `Press the key you want for ${armingName(arming, draft.kind).toLowerCase()}. It will be named rather than doing what it is bound to.`}
        </p>
      ) : null}
      <div className="settings-bar">
        <button
          type="button"
          data-testid="controls-view-list"
          aria-pressed={!drawing}
          onClick={() => {
            setDrawing(false);
          }}
        >
          List
        </button>
        <button
          type="button"
          data-testid="controls-view-drawing"
          aria-pressed={drawing}
          onClick={() => {
            setDrawing(true);
          }}
        >
          The desk
        </button>
      </div>
      {drawing ? (
        <div className="settings-list" data-testid="controls-drawing">
          <p className="settings-hint">
            Every key of the surface, where it is. A key that is filled in is bound; one that
            glows is lit right now, which is what the desk itself is showing. Click a key to put
            the list in front of what it does.
          </p>
          <DeskDrawing
            controls={table.controls}
            panel={table.panel}
            lamps={lamps}
            arming={armingControl(arming, table.controls)}
            onPick={(control) => {
              // Picking a key in the drawing is *looking it up*, not binding
              // it: the list is where a binding is made, and a click that had
              // rebound a key would be a gesture with no Learn in front of it.
              setDrawing(false);
              setNote(`${control.name} \u2014 ${actionText(control.action)}.`);
            }}
          />
        </div>
      ) : (
      <div className="settings-list" data-testid="controls-list">
        {ACTION_GROUPS.map((group) => (
          <section key={group.title} className="control-section">
            <h3>{group.title}</h3>
            <ControlHeadings />
            {group.kinds.map((kind) => (
              <ActionRow
                key={kind}
                kind={kind}
                state={rows[kind] ?? FRESH}
                keys={boundTo.get(kind) ?? []}
                held={held}
                arming={arming?.t === "kind" && arming.kind === kind}
                onState={(next) => {
                  setRows((was) => ({ ...was, [kind]: next }));
                }}
                onLearn={() => {
                  arm({ t: "kind", kind });
                }}
                onUnbind={(control) => {
                  bind(control, null);
                }}
              />
            ))}
          </section>
        ))}

        <section className="control-section" data-testid="controls-keypad">
          <h3>Console keys</h3>
          <p className="settings-hint">
            The same keys as the <em>Command keys</em> window. A key writes its word into the
            command line; the line is finished on screen, by Enter or by clicking the thing it
            was waiting for. A bound key lights up while pressing it would lead somewhere.
          </p>
          <ControlHeadings />
          {CONSOLE_KEYS.map((key) => (
            <WordRow
              key={key.word}
              word={key.word}
              shape={key.shape}
              keys={boundToWord.get(key.word) ?? []}
              held={held}
              arming={arming?.t === "word" && arming.word === key.word}
              onLearn={() => {
                arm({ t: "word", word: key.word });
              }}
              onUnbind={(control) => {
                bind(control, null);
              }}
            />
          ))}
        </section>

        <section className="control-section" data-testid="controls-custom">
          <h3>Custom commands</h3>
          <ControlHeadings />
          {custom.length === 0 ? (
            <p className="settings-none" data-testid="custom-empty">
              No custom keys yet. Add one below — a key can write a line into the command line,
              open a window, or jump to a view.
            </p>
          ) : (
            custom.map((control) => (
              <CustomRow
                key={control.name}
                control={control}
                held={held}
                onRebind={(action) => {
                  bind(control.control, action);
                }}
                onUnbind={() => {
                  bind(control.control, null);
                }}
              />
            ))
          )}
          <NewCustom
            draft={draft}
            held={held}
            arming={arming?.t === "new"}
            onDraft={setDraft}
            onLearn={() => {
              arm({ t: "new" });
            }}
          />
        </section>

        <details className="control-section" data-testid="controls-advanced">
          <summary>Advanced: the strips, the main fader and the jog wheel</summary>
          <p className="settings-hint">
            These belong to the executors and to the programmer, and they are set up in the
            Executors window and on the encoder bar. They are here so that a desk that wants
            them elsewhere can say so — one row per control, and the executor a strip control
            acts on is that strip&apos;s.
          </p>
          <ControlHeadings />
          {advanced.map((control) => (
            <AdvancedRow
              key={control.name}
              control={control}
              state={rows[control.name] ?? FRESH}
              held={held}
              onState={(next) => {
                setRows((was) => ({ ...was, [control.name]: next }));
              }}
              onRebind={(action) => {
                bind(control.control, action);
              }}
            />
          ))}
        </details>
      </div>
      )}
    </div>
  );
}

/**
 * Which control the drawing should show as armed — S59.
 *
 * Learn is armed for a **row**, and a row is an action rather than a key, so
 * most of the time the answer is *none*: the operator has not pressed anything
 * yet and there is no key to mark. What the drawing can show is the case where
 * exactly one key is already bound to that row — which is the common one when
 * somebody is moving a binding rather than making it.
 */
function armingControl(
  arming: Arming | null,
  controls: readonly SurfaceControl[],
): string | null {
  if (arming === null || arming.t === "new") {
    return null;
  }
  const bound = controls.filter((control) =>
    control.action === null
      ? false
      : arming.t === "word"
        ? control.action.t === "ConsoleWord" && control.action.word === arming.word
        : kindOf(control.action) === arming.kind,
  );
  return bound.length === 1 ? (bound[0]?.name ?? null) : null;
}

/**
 * The column headings, drawn once per section.
 *
 * They are a row of the same grid rather than a `<thead>`, which is what makes
 * the four columns line up across four sections — see the module documentation.
 */
function ControlHeadings() {
  return (
    <div className="control-row control-headings" aria-hidden="true">
      <span>Action</span>
      <span>On</span>
      <span>Keys</span>
      <span />
    </div>
  );
}

/** One action, what it is set to, and every key that reaches it. */
function ActionRow({
  kind,
  state,
  keys,
  held,
  arming,
  onState,
  onLearn,
  onUnbind,
}: {
  readonly kind: ActionKind;
  readonly state: RowState;
  readonly keys: readonly SurfaceControl[];
  readonly held: boolean;
  readonly arming: boolean;
  readonly onState: (next: RowState) => void;
  readonly onLearn: () => void;
  readonly onUnbind: (control: BoundControl) => void;
}) {
  // A row whose detail has not been answered cannot be learned onto: the daemon
  // would be asked to guess which window or which bank was meant. The button
  // says so by being disabled, and the box beside it is what it is waiting for.
  const ready = actionOfKind(kind, state.target, state.detail, 0, state.submit) !== null;
  return (
    <div className={`control-row${arming ? " control-arming" : ""}`} data-testid={`action-${kind}`}>
      <span className="control-name">{kind}</span>
      <span className="control-detail">
        <Detail
          kind={kind}
          detail={state.detail}
          held={held}
          onDetail={(detail) => {
            onState({ ...state, detail });
          }}
        />
      </span>
      <span data-testid={`action-keys-${kind}`}>
        <KeyChips keys={keys} held={held} onUnbind={onUnbind} />
      </span>
      <span>
        <button
          type="button"
          disabled={held || !ready}
          data-testid={`action-learn-${kind}`}
          aria-pressed={arming}
          title={ready ? undefined : "Answer the box beside this row first"}
          onClick={onLearn}
        >
          {arming ? "Press a key…" : "Learn"}
        </button>
      </span>
    </div>
  );
}

/**
 * One key of the console keypad, and every surface key that writes it — S59.
 *
 * A row with no box on it, which is what tells it apart from {@link ActionRow}:
 * the word **is** the answer, so there is nothing to fill in before Learn can be
 * pressed. The shape is shown rather than asked, because it is the word's and
 * not the operator's — `ARCHITECTURE_SPEC.md` §4.5.
 */
function WordRow({
  word,
  shape,
  keys,
  held,
  arming,
  onLearn,
  onUnbind,
}: {
  readonly word: string;
  readonly shape: KeyShape;
  readonly keys: readonly SurfaceControl[];
  readonly held: boolean;
  readonly arming: boolean;
  readonly onLearn: () => void;
  readonly onUnbind: (control: BoundControl) => void;
}) {
  return (
    <div
      className={`control-row${arming ? " control-arming" : ""}`}
      data-testid={`word-${word.toLowerCase()}`}
    >
      <span className="control-name">{word}</span>
      <span className="control-detail">
        <span className={`command-key command-key-${shape}`} data-shape={shape}>
          {SHAPE_WORDS[shape]}
        </span>
      </span>
      <span data-testid={`word-keys-${word.toLowerCase()}`}>
        <KeyChips keys={keys} held={held} onUnbind={onUnbind} />
      </span>
      <span>
        <button
          type="button"
          disabled={held}
          data-testid={`word-learn-${word.toLowerCase()}`}
          aria-pressed={arming}
          title={titleOf(word)}
          onClick={onLearn}
        >
          {arming ? "Press a key\u2026" : "Learn"}
        </button>
      </span>
    </div>
  );
}

/** What each of §4.5's shapes does, in a word an operator reads on the row. */
const SHAPE_WORDS: Readonly<Record<KeyShape, string>> = {
  run: "runs at once",
  oops: "takes a word back",
  write: "writes and waits",
  append: "adds to the line",
};

/**
 * One hardware control of the advanced section — S59.
 *
 * S38's original panel, kept as the back door it should always have been: the
 * row is the **control** and the chooser is what it does. It is here rather than
 * in the list because a strip's fader, encoder and keys belong to the executor
 * standing under them, and the Executors window is where an operator sets that
 * up — but a desk that wants them somewhere else has to be able to say so.
 *
 * The executor a strip control acts on is not asked: it is that strip's, which
 * is what {@link targetOf} reads off the control itself.
 */
function AdvancedRow({
  control,
  state,
  held,
  onState,
  onRebind,
}: {
  readonly control: SurfaceControl;
  readonly state: RowState;
  readonly held: boolean;
  readonly onState: (next: RowState) => void;
  readonly onRebind: (action: SurfaceAction | null) => void;
}) {
  const kind = kindOf(control.action);
  const detail = control.action === null ? state.detail : detailOf(control.action);
  const rebind = (nextKind: ActionKind, nextDetail: string): void => {
    onRebind(
      actionOfKind(
        nextKind,
        targetOf(control.control),
        nextDetail,
        slotOfControl(control.control),
        state.submit,
      ),
    );
  };
  return (
    <div className="control-row" data-testid={`advanced-${control.name}`}>
      <span className="control-name">
        <span className={control.reserved ? "key-chip key-chip-reserved" : "key-chip"}>
          {control.name}
        </span>
      </span>
      <span className="control-detail">
        <select
          data-testid={`advanced-kind-${control.name}`}
          disabled={held || control.reserved}
          value={kind}
          onChange={(event) => {
            const next = ACTION_KINDS.find((candidate) => candidate === event.target.value);
            if (next !== undefined) {
              onState({ ...state, detail: "" });
              rebind(next, "");
            }
          }}
        >
          {ACTION_KINDS.map((candidate) => (
            <option key={candidate} value={candidate}>
              {candidate}
            </option>
          ))}
        </select>
        <Detail
          kind={kind}
          detail={detail}
          held={held || control.reserved}
          testId={`advanced-detail-${control.name}`}
          onDetail={(next) => {
            onState({ ...state, detail: next });
            rebind(kind, next);
          }}
        />
      </span>
      <span>{actionText(control.action)}</span>
      <span />
    </div>
  );
}

/** The keys bound to one action, each with its own unbind. */
function KeyChips({
  keys,
  held,
  onUnbind,
}: {
  readonly keys: readonly SurfaceControl[];
  readonly held: boolean;
  readonly onUnbind: (control: BoundControl) => void;
}) {
  if (keys.length === 0) {
    return <span className="settings-none">—</span>;
  }
  return (
    <ul className="key-chips">
      {keys.map((control) => (
        <li key={control.name}>
          <span
            className={control.reserved ? "key-chip key-chip-reserved" : "key-chip"}
            title={`${actionText(control.action)}${
              control.permanent ? " · always ours" : " · follows the shared-mode switch"
            }`}
          >
            {control.name}
            {control.permanent ? "" : " *"}
          </span>
          <button
            type="button"
            className="linkish"
            disabled={held || control.reserved}
            data-testid={`action-unbind-${control.name}`}
            aria-label={`Unbind ${control.name}`}
            onClick={() => {
              onUnbind(control.control);
            }}
          >
            ✕
          </button>
        </li>
      ))}
    </ul>
  );
}

/**
 * One custom key: the key it is, what it does, and the answer it carries.
 *
 * Editable in place, which is the half `ActionRow` cannot give a custom kind:
 * the row **is** the binding, so changing the line changes that key rather than
 * needing a fresh Learn. Every edit is a whole `SurfaceBinding` for that one
 * control, which is the protocol's own rule.
 */
function CustomRow({
  control,
  held,
  onRebind,
  onUnbind,
}: {
  readonly control: SurfaceControl;
  readonly held: boolean;
  readonly onRebind: (action: SurfaceAction) => void;
  readonly onUnbind: () => void;
}) {
  const action = control.action;
  if (action === null) {
    return null;
  }
  const kind = kindOf(action);
  const detail = detailOf(action);
  const submit = submitOf(action);
  const rebind = (nextDetail: string, nextSubmit: boolean): void => {
    const next = actionOfKind(kind, "Selected", nextDetail, slotOfControl(control.control), nextSubmit);
    if (next !== null) {
      onRebind(next);
    }
  };
  return (
    <div className="control-row" data-testid={`custom-${control.name}`}>
      <span className="control-name">
        <span className={control.reserved ? "key-chip key-chip-reserved" : "key-chip"}>
          {control.name}
          {control.permanent ? "" : " *"}
        </span>
      </span>
      <span className="control-detail">
        <span className="control-kind">{kind}</span>
        <Detail
          kind={kind}
          detail={detail}
          held={held}
          testId={`custom-detail-${control.name}`}
          onDetail={(next) => {
            rebind(next, submit);
          }}
        />
        {kind === "Type a command" ? (
          <label className="control-check">
            <input
              type="checkbox"
              data-testid={`custom-submit-${control.name}`}
              disabled={held}
              checked={submit}
              onChange={(event) => {
                rebind(detail, event.target.checked);
              }}
            />
            send it
          </label>
        ) : null}
      </span>
      <span>{actionText(action)}</span>
      <span>
        <button
          type="button"
          className="key-danger"
          disabled={held || control.reserved}
          data-testid={`custom-unbind-${control.name}`}
          onClick={onUnbind}
        >
          Remove
        </button>
      </span>
    </div>
  );
}

/** The `+` row: pick a type, answer it, press the key you want. */
function NewCustom({
  draft,
  held,
  arming,
  onDraft,
  onLearn,
}: {
  readonly draft: { readonly kind: ActionKind } & RowState;
  readonly held: boolean;
  readonly arming: boolean;
  readonly onDraft: (next: { readonly kind: ActionKind } & RowState) => void;
  readonly onLearn: () => void;
}) {
  const ready = actionOfKind(draft.kind, draft.target, draft.detail, 0, draft.submit) !== null;
  return (
    <div
      className={`control-row control-new${arming ? " control-arming" : ""}`}
      data-testid="custom-new"
    >
      <span className="control-name">+ Add a key</span>
      <span className="control-detail">
        <select
          data-testid="custom-kind"
          disabled={held}
          value={draft.kind}
          onChange={(event) => {
            const kind = CUSTOM_KINDS.find((one) => one === event.target.value);
            if (kind !== undefined) {
              // The detail belongs to the kind: a window type left in the box
              // after switching to *Jump to view* is not a view number, and
              // sending it would be asking the daemon to read one as the other.
              onDraft({ ...draft, kind, detail: "" });
            }
          }}
        >
          {CUSTOM_KINDS.map((kind) => (
            <option key={kind} value={kind}>
              {CUSTOM_LABELS[kind] ?? kind}
            </option>
          ))}
          {/*
            **Execute macro, as the owner listed it, and disabled.** There is no
            `SurfaceAction` for it and no macro to run; an entry that bound
            nothing would be worse than one that says when it will work.
          */}
          <option value="" disabled>
            Execute macro — a later session
          </option>
        </select>
        <Detail
          kind={draft.kind}
          detail={draft.detail}
          held={held}
          testId="custom-new-detail"
          onDetail={(detail) => {
            onDraft({ ...draft, detail });
          }}
        />
        {draft.kind === "Type a command" ? (
          <label className="control-check">
            <input
              type="checkbox"
              data-testid="custom-new-submit"
              disabled={held}
              checked={draft.submit}
              onChange={(event) => {
                onDraft({ ...draft, submit: event.target.checked });
              }}
            />
            send it
          </label>
        ) : null}
      </span>
      <span className="settings-none">
        {ready ? "press Learn, then the key" : "answer the box first"}
      </span>
      <span>
        <button
          type="button"
          disabled={held || !ready}
          data-testid="custom-learn"
          aria-pressed={arming}
          onClick={onLearn}
        >
          {arming ? "Press a key…" : "Learn"}
        </button>
      </span>
    </div>
  );
}

/**
 * What the message says the next key is about to become.
 *
 * **A desk with two people at it has to say what it is waiting for** — the
 * message is on every screen, and *press a key* on its own tells the operator
 * who did not arm it nothing about what they are about to change.
 */
function armingName(arming: Arming, draftKind: ActionKind): string {
  if (arming.t === "word") {
    return `the ${arming.word} key`;
  }
  const kind = arming.t === "new" ? draftKind : arming.kind;
  return CUSTOM_LABELS[kind] ?? kind;
}

/** What the chooser calls each custom kind, in the owner's own words. */
const CUSTOM_LABELS: Partial<Record<ActionKind, string>> = {
  "Type a command": "Send command",
  "Open window": "Open window",
  "Jump to view": "Jump to view",
};

/**
 * Which executor a control's action acts on — **S59, and it is no longer asked.**
 *
 * The panel had a *Strip / Selected* chooser on every executor row, and it had
 * to: one list held both *the executor under this strip* and *the selected one*.
 * The owner's decision of 2026-09-20 split them — the strip controls went to the
 * advanced section and the functions of the selected executor stayed in the list
 * — and once they are apart the answer is a property of the **control** rather
 * than a question for the operator.
 *
 * A strip's fader, encoder or key means that strip's executor; anything else
 * means the selected one. A chooser offering *the executor under this strip* on
 * a panel key would offer a binding that resolves to nothing
 * (`Bindings::command` answers `None` for an `ExecutorTarget::Strip` off a
 * strip), which is a question with a wrong answer in it.
 */
function targetOf(control: BoundControl): Target {
  return control.t === "StripFader" ||
    control.t === "StripEncoder" ||
    control.t === "StripButton"
    ? "Strip"
    : "Selected";
}

/** Whether a control is one the advanced section owns — S59. */
function isAdvanced(control: BoundControl): boolean {
  return (
    control.t === "StripFader" ||
    control.t === "StripEncoder" ||
    control.t === "StripButton" ||
    control.t === "MainFader" ||
    control.t === "Jog"
  );
}

/** The one extra answer a kind needs, where it needs one. */
function Detail({
  kind,
  detail,
  held,
  testId,
  onDetail,
}: {
  readonly kind: ActionKind;
  readonly detail: string;
  readonly held: boolean;
  /** Overridden by the custom rows, where the row is a key and not a kind. */
  readonly testId?: string;
  readonly onDetail: (value: string) => void;
}) {
  const id = testId ?? `action-detail-${kind}`;
  if (kind === "Executor button") {
    return (
      <select
        data-testid={id}
        disabled={held}
        value={detail}
        onChange={(event) => {
          onDetail(event.target.value);
        }}
      >
        <option value="">the key in this position (the executor decides)</option>
        {FIXED_BUTTON_FUNCTIONS.map((option: FixedButtonFunction) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    );
  }
  if (kind === "Open window") {
    return (
      <select
        data-testid={id}
        disabled={held}
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
    );
  }
  if (kind === "Encoder bank") {
    return (
      <select
        data-testid={id}
        disabled={held}
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
    );
  }
  if (kind === "Jump to view") {
    return (
      <input
        data-testid={id}
        disabled={held}
        value={detail}
        inputMode="numeric"
        placeholder="view number"
        onChange={(event) => {
          onDetail(event.target.value);
        }}
      />
    );
  }
  if (kind === "Type a command") {
    return (
      <input
        className="control-line"
        data-testid={id}
        disabled={held}
        value={detail}
        placeholder="Go Executor 1"
        onChange={(event) => {
          onDetail(event.target.value);
        }}
      />
    );
  }
  return null;
}

/** The two file keys, and the hidden input the second one drives. */
function ProfileFile({
  held,
  onExport,
  onImport,
}: {
  readonly held: boolean;
  readonly onExport: () => void;
  readonly onImport: (file: File) => void;
}) {
  const picker = useRef<HTMLInputElement>(null);
  return (
    <span className="settings-file">
      <button type="button" data-testid="controls-export" onClick={onExport}>
        Export…
      </button>
      <button
        type="button"
        disabled={held}
        data-testid="controls-import"
        onClick={() => {
          picker.current?.click();
        }}
      >
        Import…
      </button>
      {/*
        The browser's own dialogue, which is the point: a control map is a file
        an operator carries between desks, so it goes to the machine they are
        sitting at rather than to a path the daemon resolves. A show file is the
        other case and is still typed — see `settings/showfiles.tsx`.
      */}
      <input
        ref={picker}
        className="visually-hidden"
        type="file"
        accept="application/json,.json"
        data-testid="controls-file"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file !== undefined) {
            onImport(file);
          }
          // Cleared, so choosing the same file twice reads it twice.
          event.target.value = "";
        }}
      />
    </span>
  );
}
