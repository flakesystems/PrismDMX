/**
 * The binding vocabulary of `docs/MCU_MAPPING.md` §4, as a person chooses it —
 * S38.
 *
 * # Nothing here holds anything, and nothing here decides anything
 *
 * `settings.ts`'s rule one panel along. Every function takes what the daemon
 * said and answers; there is no parsed copy of the table, because a parsed copy
 * is a second model to keep in step.
 *
 * # Why there is a list of *kinds* at all
 *
 * `SurfaceAction` is nineteen variants and four of them carry a direction, so
 * the vocabulary an operator picks from is not the same shape as the one that
 * travels: *Go+* and *Go−* are one variant with a field, and a dropdown with a
 * second dropdown inside it for the sign would be a worse way to say the same
 * thing. {@link ACTION_KINDS} is therefore the flattened list, and
 * {@link actionOfKind} and {@link kindOf} are the two halves of one
 * correspondence — `actions.test.ts` walks every kind through both, so a kind
 * that cannot be built or an action that cannot be named fails a test rather
 * than drawing a blank row.
 *
 * The list is **not** generated from `SURFACE_ACTION`'s tag union, and that is
 * deliberate: the flattening is a choice about wording, and a generated list
 * would have to be un-flattened again to make it. What is generated is the thing
 * that would actually drift — the executor button functions, the window types
 * and the feature groups, which the panel reads out of `variants.ts`.
 *
 * # The list became the panel — S43, punch-list B3
 *
 * S38 drew the table the way `prism_surface::Bindings` stores it: a row per
 * *control*, and a chooser of actions inside each row. The owner's entry says
 * that is backwards, and it is — an operator does not arrive wanting to know
 * what F5 does, they arrive wanting **Go on the selected executor** and needing
 * a key for it. So the rows are the vocabulary now, {@link ACTION_GROUPS} is
 * the order they are read in, and the key comes from pressing it.
 *
 * The kinds did not have to change for that, which is the useful part: the same
 * flattening that made a dropdown readable makes a list of rows readable, and
 * {@link actionOfKind} still builds what travels.
 */

import type {
  BoundControl,
  FeatureGroup,
  StripButton,
  SurfaceAction,
  WindowType,
} from "../bindings";
import {
  FEATURE_GROUP_VARIANTS,
  WINDOW_TYPE_VARIANTS,
} from "../bindings";
import { FIXED_BUTTON_FUNCTIONS } from "../desk/functions";
import type { FixedButtonFunction } from "../desk/functions";

/**
 * Whether a string picked out of a `<select>` really is one of `list`.
 *
 * `CLAUDE.md` forbids `any` and `as` is a claim rather than a check, so a value
 * that came back out of the DOM is narrowed against the **generated** table
 * rather than asserted into the type. It cannot fail while the options are drawn
 * from that same table — which is the point: the day somebody draws them from
 * somewhere else, this answers `null` instead of sending the daemon a word it
 * does not know.
 */
function oneOf<T extends string>(value: string, list: readonly T[]): T | null {
  return list.find((entry) => entry === value) ?? null;
}

/** The vocabulary as an operator picks it, in the order the chooser lists it. */
export const ACTION_KINDS = [
  "Nothing",
  "Executor master",
  "Executor go +",
  "Executor go −",
  "Executor off",
  "Executor button",
  "Select executor",
  "Clear programmer",
  "Executor page −",
  "Executor page +",
  "Jump to view",
  "Previous view",
  "Next view",
  "Programmer page −",
  "Programmer page +",
  "Previous parameter",
  "Next parameter",
  "Adjust parameter",
  "Encoder bank",
  "Open window",
  "Choose a window",
  "Type a command",
  "Save show",
  "Oops",
  "Redo",
] as const;

/** One entry of {@link ACTION_KINDS}. */
export type ActionKind = (typeof ACTION_KINDS)[number];

/**
 * The kinds in the order and the grouping the panel lists them — S43, B3, and
 * re-grouped for the owner's rebuild.
 *
 * *Es sollte eingeteilt werden in Executorbereich, Programmerbereich, Andere
 * Interne Befehle und Custom Befehle.* The first three sections are these; the
 * fourth is {@link CUSTOM_KINDS}, and it is a different shape on purpose — see
 * there.
 *
 * *Nothing* is not here: it is the absence of a binding, and *unbind* is a
 * button on the key rather than an action to learn a key onto. Every other kind
 * appears exactly once **across the four tables**, which `actions.test.ts`
 * holds — a kind that fell out would be a piece of the vocabulary an operator
 * could no longer reach from the panel, and nothing else would notice.
 */
export const ACTION_GROUPS: readonly {
  readonly title: string;
  readonly kinds: readonly ActionKind[];
}[] = [
  {
    title: "Executors",
    kinds: [
      "Executor master",
      "Executor go +",
      "Executor go −",
      "Executor off",
      "Executor button",
      "Select executor",
      "Executor page −",
      "Executor page +",
    ],
  },
  {
    title: "The programmer",
    kinds: [
      "Clear programmer",
      "Adjust parameter",
      "Previous parameter",
      "Next parameter",
      "Programmer page −",
      "Programmer page +",
      "Encoder bank",
    ],
  },
  {
    title: "Other internal commands",
    kinds: ["Previous view", "Next view", "Choose a window", "Save show", "Oops", "Redo"],
  },
];

/**
 * The kinds an operator adds one at a time — the **custom** section.
 *
 * # Why these three are not rows like the rest
 *
 * The first three sections are the desk's fixed vocabulary: *Go on the selected
 * executor* is one thing, it is either bound or it is not, and the row is the
 * thing. These three are not. *Open window* is fourteen different bindings, and
 * *type a command* is as many as an operator can think of — so the row is not
 * the action, the **key** is, and each key carries its own answer.
 *
 * That is the shape the owner asked for: *in der Custom Befehle Sektion sollte
 * es einen Keybind hinzufügen (oder einfach +) Knopf geben. Dort kann dann der
 * Typ ausgewählt werden: Send Command / Open Window / Jump to View / Execute
 * Macro.*
 *
 * The fourth type, **Execute macro**, is not here and is drawn as *a later
 * session* by the panel: there is no `SurfaceAction` for it and no macro to run,
 * and a chooser entry that bound nothing would be worse than one that says why.
 * `IMPLEMENTATION_PLAN.md` has the session.
 */
export const CUSTOM_KINDS: readonly ActionKind[] = [
  "Type a command",
  "Open window",
  "Jump to view",
];

/** Whether a kind is one an operator adds a key at a time. */
export function isCustom(kind: ActionKind): boolean {
  return CUSTOM_KINDS.includes(kind);
}

/**
 * Which of a strip's five keys a **control** is, from zero — S43, B3.
 *
 * {@link slotOf}'s twin, and it exists because the two halves of the panel now
 * arrive from different directions: S38 knew the row's *name* and read the
 * position off it, and learn hands over a `BoundControl` instead. §2.1's order
 * is written down once, in {@link STRIP_KEYS}, so the two cannot drift.
 *
 * Anything that is not a strip key is 0, which is what a panel key bound to a
 * slot would mean anyway.
 */
export function slotOfControl(control: BoundControl): number {
  if (control.t !== "StripButton") {
    return 0;
  }
  const index = STRIP_KEYS.findIndex((entry) => entry === control.button);
  return index < 0 ? 0 : index;
}

/** §2.1's order of a strip's five keys. */
const STRIP_KEYS: readonly StripButton[] = ["Rec", "Solo", "Mute", "Select", "VPotPush"];

/** Which executor an action acts on. */
export type Target = "Strip" | "Selected";

/**
 * The detail an already-bound action carries, as the editor's boxes hold it.
 *
 * The inverse of the `detail` argument {@link actionOfKind} takes, and the
 * custom section needs it: a row there is a **key that is already bound**, and
 * an operator changing the line has to see the line that is on it. The fixed
 * sections do not — {@link kindOf} answers the kind and the panel re-asks,
 * because a chooser that pre-filled *Open window: Patch* and then sent
 * *FixtureSheet* because nobody touched the second box would be worse than one
 * that asks again.
 */
export function detailOf(action: SurfaceAction): string {
  switch (action.t) {
    case "WriteCommandLine":
      return action.line;
    case "OpenWindow":
      return action.window;
    case "SelectView":
      return String(action.view);
    case "SetEncoderBank":
      return action.group;
    case "ExecutorButton":
      // A profile row names one of the **eight fixed** functions or a hardware
      // position. S45's custom row is not on this list and does not need to be:
      // a key on the desk that sends a line is `WriteCommandLine`, which has
      // been the way to say it since S43 and carries the *send* option too.
      return action.button.t === "Function" && typeof action.button.function === "string"
        ? action.button.function
        : "";
    default:
      return "";
  }
}

/** Whether an already-bound line runs itself. */
export function submitOf(action: SurfaceAction): boolean {
  return action.t === "WriteCommandLine" && action.submit;
}

/**
 * Which kind an action is, or `"Nothing"` for a control that is not bound.
 *
 * The inverse of {@link actionOfKind} over the kinds that need no detail; for
 * the four that do, it answers the kind and the panel re-asks for the detail,
 * because a chooser that pre-filled *Open window: Patch* and then sent
 * *FixtureSheet* because nobody touched the second box would be worse than one
 * that asks again.
 */
export function kindOf(action: SurfaceAction | null): ActionKind {
  if (action === null) {
    return "Nothing";
  }
  switch (action.t) {
    case "ExecutorMaster":
      return "Executor master";
    case "ExecutorGo":
      return action.direction === "Next" ? "Executor go +" : "Executor go −";
    case "ExecutorOff":
      return "Executor off";
    case "ExecutorButton":
      return "Executor button";
    case "SelectExecutor":
      return "Select executor";
    case "ClearProgrammer":
      return "Clear programmer";
    case "ExecutorPage":
      return action.delta < 0 ? "Executor page −" : "Executor page +";
    case "SelectView":
      return "Jump to view";
    case "StepView":
      return action.direction === "Prev" ? "Previous view" : "Next view";
    case "ProgrammerPage":
      return action.delta < 0 ? "Programmer page −" : "Programmer page +";
    case "SelectProgrammerParam":
      return action.direction === "Prev" ? "Previous parameter" : "Next parameter";
    case "AdjustParameter":
      return "Adjust parameter";
    case "SetEncoderBank":
      return "Encoder bank";
    case "OpenWindow":
      return "Open window";
    case "OpenWindowPicker":
      return "Choose a window";
    case "WriteCommandLine":
      return "Type a command";
    case "SaveShow":
      return "Save show";
    case "Oops":
      return "Oops";
    case "Redo":
      return "Redo";
  }
}

/**
 * The action a kind means, or `null` for *nothing at all*.
 *
 * `detail` is the one extra answer four of the kinds need — a button function, a
 * window type, a feature group or a view number. An empty `detail` is a
 * legitimate answer for **Executor button**, where it means *the key in this
 * position, and the executor decides what that is* (`ExecutorButtonRef::Slot`,
 * which is the whole of **D3** for playback). For the other three an empty
 * detail cannot be sent, so the kind answers `null` and the daemon is not asked
 * to guess.
 *
 * The slot index for a strip key is **not** chosen here: it is the button's own
 * position, and the panel knows which row it is editing. `Strip[*].Button.Rec`
 * is slot 0 by §2.1's order, which is what {@link slotOf} reads off the name.
 */
export function actionOfKind(
  kind: ActionKind,
  target: Target,
  detail: string,
  slot = 0,
  /**
   * Whether a bound line runs itself — S43, and the owner's *Kästchen*.
   *
   * Only **Type a command** reads it. A key bound to `Go Executor 1` that needs
   * Enter afterwards is not a Go key; a key that writes `Store Cue ` for the
   * operator to finish is exactly right. Both are wanted, so the binding
   * carries the answer. See `SurfaceAction::WriteCommandLine` for who actually
   * runs it, which is not the daemon.
   */
  submit = false,
): SurfaceAction | null {
  switch (kind) {
    case "Nothing":
      return null;
    case "Executor master":
      return { t: "ExecutorMaster", target };
    case "Executor go +":
      return { t: "ExecutorGo", target, direction: "Next" };
    case "Executor go −":
      return { t: "ExecutorGo", target, direction: "Prev" };
    case "Executor off":
      return { t: "ExecutorOff", target };
    case "Executor button":
      if (detail === "") {
        // *The key in this position, and the executor decides what that is* —
        // `ExecutorButtonRef::Slot`, which is the whole of **D3** for playback.
        return { t: "ExecutorButton", target, button: { t: "Slot", index: slot } };
      }
      return withFunction(target, oneOf<FixedButtonFunction>(detail, FIXED_BUTTON_FUNCTIONS));
    case "Select executor":
      return { t: "SelectExecutor", target };
    case "Clear programmer":
      return { t: "ClearProgrammer" };
    case "Executor page −":
      return { t: "ExecutorPage", delta: -1 };
    case "Executor page +":
      return { t: "ExecutorPage", delta: 1 };
    case "Jump to view": {
      const view = Number(detail);
      return Number.isInteger(view) && view > 0 ? { t: "SelectView", view } : null;
    }
    case "Previous view":
      return { t: "StepView", direction: "Prev" };
    case "Next view":
      return { t: "StepView", direction: "Next" };
    case "Programmer page −":
      return { t: "ProgrammerPage", delta: -1 };
    case "Programmer page +":
      return { t: "ProgrammerPage", delta: 1 };
    case "Previous parameter":
      return { t: "SelectProgrammerParam", direction: "Prev" };
    case "Next parameter":
      return { t: "SelectProgrammerParam", direction: "Next" };
    case "Adjust parameter":
      return { t: "AdjustParameter" };
    case "Encoder bank": {
      const group = oneOf<FeatureGroup>(detail, FEATURE_GROUP_VARIANTS);
      return group === null ? null : { t: "SetEncoderBank", group };
    }
    case "Open window": {
      const window = oneOf<WindowType>(detail, WINDOW_TYPE_VARIANTS);
      return window === null ? null : { t: "OpenWindow", window };
    }
    case "Choose a window":
      return { t: "OpenWindowPicker" };
    // **The custom row** — S43, punch-list B4. The detail is the line itself, so
    // this is the one kind whose detail is free text rather than a name picked
    // from a generated table, and the one an operator adds as many of as they
    // need. An empty line is not a binding: a key that writes nothing into the
    // command line is a key that does nothing, said obscurely.
    case "Type a command":
      return detail.trim() === "" ? null : { t: "WriteCommandLine", line: detail, submit };
    case "Save show":
      return { t: "SaveShow" };
    case "Oops":
      return { t: "Oops" };
    case "Redo":
      return { t: "Redo" };
  }
}

/** An executor-button action naming a function outright, if the name is one. */
function withFunction(
  target: Target,
  chosen: FixedButtonFunction | null,
): SurfaceAction | null {
  return chosen === null
    ? null
    : { t: "ExecutorButton", target, button: { t: "Function", function: chosen } };
}

/**
 * A control's binding in a sentence, for the list.
 *
 * The **name** of what it does rather than a re-statement of the tag, because
 * the list is what teaches an operator the vocabulary — the same argument
 * `ARCHITECTURE_SPEC.md` §4.5 makes for the command line teaching itself.
 */
export function actionText(action: SurfaceAction | null): string {
  if (action === null) {
    return "—";
  }
  const on = (target: string): string =>
    target === "Strip" ? "this strip's executor" : "the selected executor";
  switch (action.t) {
    case "ExecutorMaster":
      return `master of ${on(action.target)}`;
    case "ExecutorGo":
      return `${action.direction === "Next" ? "go +" : "go −"} on ${on(action.target)}`;
    case "ExecutorOff":
      return `off on ${on(action.target)}`;
    case "ExecutorButton":
      return action.button.t === "Slot"
        ? `key ${action.button.index + 1} of ${on(action.target)}`
        : `${action.button.function} on ${on(action.target)}`;
    case "SelectExecutor":
      return `select ${on(action.target)}`;
    case "ClearProgrammer":
      return "clear the programmer";
    case "ExecutorPage":
      return action.delta < 0 ? "previous executor page" : "next executor page";
    case "SelectView":
      return `jump to view ${String(action.view)}`;
    case "StepView":
      return action.direction === "Prev" ? "previous view" : "next view";
    case "ProgrammerPage":
      return action.delta < 0 ? "previous programmer page" : "next programmer page";
    case "SelectProgrammerParam":
      return action.direction === "Prev" ? "previous parameter" : "next parameter";
    case "AdjustParameter":
      return "turn the selected parameter";
    case "SetEncoderBank":
      return `encoder bank: ${action.group}`;
    case "OpenWindow":
      return `open ${action.window}`;
    case "OpenWindowPicker":
      return "choose a window to open";
    case "WriteCommandLine":
      return action.submit
        ? `type ${JSON.stringify(action.line)} into the command line and send it`
        : `type ${JSON.stringify(action.line)} into the command line`;
    case "SaveShow":
      return "save the show";
    case "Oops":
      return "oops";
    case "Redo":
      return "redo";
  }
}

/**
 * Whether two controls are the same control.
 *
 * Structural, because a `BoundControl` arrives as a decoded object and two
 * decodings of one control are two objects. Written out rather than compared as
 * JSON: key order is not part of the identity, and a comparison that depended on
 * it would fail for a daemon that serialised the same fact differently.
 */
export function sameControl(left: BoundControl, right: BoundControl): boolean {
  if (left.t !== right.t) {
    return false;
  }
  if (left.t === "StripButton" && right.t === "StripButton") {
    return left.button === right.button;
  }
  if (left.t === "Global" && right.t === "Global") {
    return left.button === right.button;
  }
  return true;
}

/**
 * Which of a strip's five keys a control name is, from zero.
 *
 * §2.1's order — Rec, Solo, Mute, Select, V-Pot push — and it is read off the
 * name rather than sent, because the name **is** the position: a binding on
 * `Strip[*].Button.Mute` that resolved to slot 0 would put the wrong key on the
 * wrong executor function, which is the one mistake `ExecutorButtonRef::Slot`
 * exists to make impossible. Anything that is not a strip key is 0, which is
 * what a panel key bound to a slot would mean anyway.
 */
export function slotOf(name: string): number {
  // Compared rather than asserted into the type — the same rule `oneOf` above
  // follows. A name that is not a strip key answers -1 and therefore 0, which
  // is what the doc comment above promises.
  const key = name.replace("Strip[*].Button.", "");
  const index = STRIP_KEYS.findIndex((entry) => entry === key);
  return index < 0 ? 0 : index;
}
