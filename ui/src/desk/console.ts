/**
 * The command line: what an operator types, and the commands it means.
 *
 * # It is a parser in the client, and that is not a contradiction
 *
 * `1 thru 3 at 50` is not a command. It is two of them — a `SelectFixtures` and
 * a `SetAttribute` — and turning one into the other is the client's job, in the
 * same way that dragging a window into a `PlaceWindow` is. **D3 is untouched**:
 * nothing here decides what the desk *is*, it decides what was *asked for*, and
 * every answer goes to the daemon to be validated, applied or refused. A line
 * naming a fixture the show has not got parses perfectly and is refused by the
 * daemon — that split is deliberate and is in the recording as a step of its
 * own.
 *
 * The session's `commandLine` is a different thing again: it is the **text**,
 * shared with every other client and with the console's display
 * (`ARCHITECTURE_SPEC.md` §4.1), and it travels as `CommandLineInput`.
 *
 * # It never throws
 *
 * {@link parseCommandLine} answers with commands or with a message, for every
 * string there is. A console that threw would take the interface down over a
 * typo in the middle of a show, and there is no input that is not a typo
 * somewhere. `console.test.ts` runs ten thousand generated strings through it
 * and asserts that none of them throws — which is the exit criterion, not a
 * precaution.
 *
 * # It does not read the show, and that is why the modes are questions
 *
 * S26 wrote the rule and S40 keeps it: a parser that consulted the patch could
 * be wrong about the daemon's state. So `Copy Sequence 2 Sequence 6` means the
 * same thing whether or not sequence 2 exists, and a line whose destination
 * might already hold something carries a {@link ModeQuestion} rather than a
 * decision. The **interface** looks at its mirror to decide whether to ask, and
 * the operator's answer travels in the command — see {@link applyMode}.
 *
 * # The grammar, in full
 *
 * ```text
 *   line     := select | level | verb | playback | word
 *
 *   select   := ["fixture"] fixtures        -- 1 · 1 thru 4 · 1 + 3 · fixture 12
 *   level    := [fixtures] [attribute] "at" percent
 *
 *   word     := "clear" | "full" | "oops" | "update"
 *   verb     := "store"  target
 *             | "edit"   cue
 *             | "goto"   [playback] cue     -- goto cue 5
 *             | "delete" object
 *             | "move"   object object
 *             | "copy"   object object
 *             | "label"  object [name]
 *             | "assign" "sequence" n "executor" n
 *             | "page"   n
 *   playback := ("on" | "off" | "go" | "go+" | "go-") [target]
 *
 *   object   := ("sequence" | "cue" | "group" | "preset" | "view" | "executor") n
 *   target   := object | n                  -- `go 3` is S26's, and is an executor
 *   fixtures := range (("+" | ",") range)*
 *   range    := number ["thru" number]
 * ```
 *
 * A bare `Group 3`, `Sequence 5`, `View 2`, `Executor 4` or `Preset 1` is the
 * **selecting** form of that word: a group selects its fixtures, a sequence
 * becomes the one a store goes into, a view is switched to, an executor is the
 * one the transport acts on, and a preset is applied to the selection. That is
 * what `ARCHITECTURE_SPEC.md` §4.5 means by an argument keyword: pressing
 * `Group` writes `Group `, the operator types `3`, and Enter is a selection.
 *
 * Case does not matter and neither does spacing: `1thru4` is a range, because a
 * console's keypad has no space bar worth reaching for. A name may be quoted —
 * `Label View 1 "House lights"` — and need not be.
 */

import type {
  AttributeType,
  Command,
  FeatureGroup,
  ObjectRef,
  OverwriteMode,
  PlaybackTarget,
  SequenceStoreMode,
  StoreMode,
} from "../bindings";
import { ATTRIBUTE_TYPE_VARIANTS, FEATURE_GROUP_VARIANTS } from "../bindings/variants";
import { levelFromPercent } from "./level";

/**
 * A line whose destination may already hold something.
 *
 * The parser cannot know whether it does — it does not read the show — so it
 * says *what would be written and which words the question takes*, and the
 * interface asks when its mirror shows something there. {@link applyMode} puts
 * the answer into the command.
 */
export interface ModeQuestion {
  /** Which set of words the prompt offers. */
  readonly kind: "store" | "sequence" | "overwrite";
  /** What the line would write to, in the words an operator typed. */
  readonly what: string;
  /** Where to look to see whether anything is already there. */
  readonly at: ObjectRef;
}

/** What a line turned out to be. */
export type ConsoleResult =
  /** A line with nothing in it. Enter on an empty console does nothing. */
  | { readonly kind: "empty" }
  /** Commands, in the order they have to be sent. */
  | {
      readonly kind: "commands";
      readonly commands: readonly Command[];
      /**
       * Set **only** when the line carries a mode somebody may have to choose.
       *
       * Absent rather than `null` for the lines that have nothing to ask, which
       * is most of them: it keeps the answer to `1 thru 4 at 50` exactly the
       * shape S26 gave it, and says the thing itself.
       */
      readonly question?: ModeQuestion;
    }
  /** A line that is not one, and what to tell the operator about it. */
  | { readonly kind: "error"; readonly message: string };

/** The word that makes a range. */
const THRU = "thru";

/** The words that name one of the six numbered things a desk has. */
const OBJECT_WORDS = [
  "sequence",
  "cue",
  "group",
  "preset",
  "view",
  "executor",
] as const;

/** Every word the line knows, for completion. */
export const CONSOLE_WORDS: readonly string[] = [
  "at",
  "assign",
  "clear",
  "copy",
  "cue",
  "delete",
  "edit",
  "executor",
  "fixture",
  "full",
  "go",
  "go+",
  "go-",
  "goto",
  "group",
  "label",
  "move",
  "off",
  "on",
  "oops",
  "page",
  "preset",
  "sequence",
  "store",
  "thru",
  "update",
  "view",
];

/** One word of a line, as typed and as matched. */
interface Token {
  /** Lower-cased, for matching. */
  readonly text: string;
  /** Exactly as the operator typed it, for names. */
  readonly raw: string;
}

/**
 * What to show under the input for a line as it stands.
 *
 * An empty line says nothing, a bad one says what is wrong with it, and a good
 * one says **what it will do** — so an operator can see before Enter that
 * `1 thru 3 at 50` is two commands and which two.
 */
export function readingText(reading: ConsoleResult): string {
  switch (reading.kind) {
    case "empty":
      return "";
    case "error":
      return reading.message;
    case "commands":
      return reading.commands.map(describe).join(" · ");
  }
}

/**
 * One command in words.
 *
 * Deliberately not the wire form: `{"t":"SetAttribute"}` is a thing to debug
 * with, not a thing to read at a desk in the dark.
 */
function describe(command: Command): string {
  switch (command.t) {
    case "SelectFixtures":
      return `select ${command.ids.join(" + ")}`;
    case "SelectGroup":
      return `select group ${String(command.groupId)}`;
    case "ApplyPreset":
      return `apply preset ${String(command.presetId)}`;
    case "SetAttribute":
      return `${command.attribute.toLowerCase()} → ${String(
        Math.round((command.value / 65535) * 1000) / 10,
      )}%`;
    case "ClearProgrammer":
      return "clear";
    case "Update":
      return "update the cue being edited";
    case "Oops":
      return "oops";
    case "StoreCue":
      return `store cue ${command.cueNumber}${sequenceSuffix(command.sequenceId)}`;
    case "StoreSequence":
      return `store sequence ${String(command.sequenceId)}`;
    case "StoreGroup":
      return `store group ${String(command.groupId)}`;
    case "StorePreset":
      return `store preset ${String(command.presetId)}`;
    case "StoreView":
      return `store view ${String(command.viewId)}`;
    case "EditCue":
      return `edit cue ${command.cueNumber}${sequenceSuffix(command.sequenceId)}`;
    case "Goto":
      return `goto cue ${command.cueNumber} on ${playbackText(command.target)}`;
    case "Delete":
      return `delete ${objectText(command.target)}`;
    case "Copy":
      return `copy ${objectText(command.from)} to ${objectText(command.to)}`;
    case "Move":
      return `move ${objectText(command.from)} to ${objectText(command.to)}`;
    case "Label":
      return `label ${objectText(command.target)} ${JSON.stringify(command.name)}`;
    case "AssignExecutor":
      return `assign ${
        command.sequenceId === null ? "nothing" : `sequence ${String(command.sequenceId)}`
      } to executor ${String(command.executorId)}`;
    case "ExecutorGo":
      return `${command.direction === "Next" ? "go" : "back"} on ${playbackText(command.target)}`;
    case "ExecutorOff":
      return `off on ${playbackText(command.target)}`;
    case "ExecutorOn":
      return `on ${playbackText(command.target)}`;
    case "SelectSequence":
      return `select sequence ${String(command.sequenceId)}`;
    case "SelectView":
      return `view ${String(command.viewId)}`;
    case "SelectExecutor":
      return `select executor ${String(command.executorId)}`;
    case "SetExecutorPage":
      return `page ${String(command.page)}`;
    default:
      // Every command this parser can produce is named above. The arm exists
      // because `Command` is the whole protocol, and a readout is not where a
      // command added to it should become a compile error.
      return command.t;
  }
}

/** ` of sequence 4`, or nothing at all for the selected one. */
function sequenceSuffix(sequenceId: number | null | undefined): string {
  return sequenceId === null || sequenceId === undefined
    ? ""
    : ` of sequence ${String(sequenceId)}`;
}

/** One object reference in the words an operator typed. */
export function objectText(target: ObjectRef): string {
  switch (target.t) {
    case "Sequence":
      return `sequence ${String(target.sequenceId)}`;
    case "Cue":
      return `cue ${target.cueNumber}${sequenceSuffix(target.sequenceId)}`;
    case "Group":
      return `group ${String(target.groupId)}`;
    case "Preset":
      return `preset ${String(target.presetId)}`;
    case "View":
      return `view ${String(target.viewId)}`;
    case "Executor":
      return `executor ${String(target.executorId)}`;
  }
}

/** One playback target in words. */
function playbackText(target: PlaybackTarget): string {
  switch (target.t) {
    case "Executor":
      return `executor ${String(target.executorId)}`;
    case "Sequence":
      return `sequence ${String(target.sequenceId)}`;
    case "Selected":
      return "the selected sequence";
  }
}

/**
 * Puts the operator's chosen mode into the command that was waiting for one.
 *
 * **The choice travels in the command** — S28's rule, S39's implementation, and
 * the reason this exists rather than the interface describing an outcome the
 * daemon might not produce. Exactly one command of a line carries a mode, so
 * this is a map rather than an index.
 */
export function applyMode(
  commands: readonly Command[],
  mode: StoreMode | SequenceStoreMode | OverwriteMode,
): Command[] {
  return commands.map((command) => {
    switch (command.t) {
      case "StoreCue":
      case "StorePreset":
        return isStoreMode(mode) ? { ...command, mode } : command;
      case "StoreSequence":
        return isSequenceStoreMode(mode) ? { ...command, mode } : command;
      case "StoreGroup":
      case "Copy":
      case "Move":
        return isOverwriteMode(mode) ? { ...command, mode } : command;
      default:
        return command;
    }
  });
}

/** Whether a chosen word is one of the cue-and-preset store modes. */
function isStoreMode(mode: string): mode is StoreMode {
  return mode === "Merge" || mode === "Override" || mode === "Remove";
}

/** Whether a chosen word is one of the sequence store modes. */
function isSequenceStoreMode(mode: string): mode is SequenceStoreMode {
  return mode === "Append" || mode === "Override" || mode === "Merge";
}

/** Whether a chosen word is one of the copy-and-move modes. */
function isOverwriteMode(mode: string): mode is OverwriteMode {
  return mode === "Merge" || mode === "Override";
}

/**
 * Reads a line.
 *
 * Never throws — see the module documentation.
 */
export function parseCommandLine(line: string): ConsoleResult {
  const words = tokenise(line);
  const head = words[0];
  if (head === undefined) {
    return { kind: "empty" };
  }
  switch (head.text) {
    case "clear":
      return only(words, "clear", [{ t: "ClearProgrammer" }]);
    case "full":
      // A whole command with no argument (§4.5): dimmer to full on whatever is
      // selected, which is what the key does on every console there is.
      return only(words, "full", [
        { t: "SetAttribute", attribute: "Dimmer", value: levelFromPercent(100), relative: false },
      ]);
    case "oops":
      return only(words, "oops", [{ t: "Oops" }]);
    case "update":
      return only(words, "update", [{ t: "Update" }]);
    case "store":
      return storeLine(words);
    case "edit":
      return editLine(words);
    case "goto":
      return gotoLine(words);
    case "delete":
      return deleteLine(words);
    case "move":
      return pairLine(words, "move");
    case "copy":
      return pairLine(words, "copy");
    case "label":
      return labelLine(words);
    case "assign":
      return assignLine(words);
    case "on":
      return playbackLine(words, "on");
    case "off":
      return playbackLine(words, "off");
    case "go":
      return playbackLine(words, "go");
    case "goback":
      return playbackLine(words, "goback");
    case "page":
      return pageCommand(words);
    default:
      return selectionLine(words);
  }
}

/** A word that takes nothing after it. */
function only(
  words: readonly Token[],
  keyword: string,
  commands: readonly Command[],
): ConsoleResult {
  return words.length === 1
    ? { kind: "commands", commands }
    : tooMuch(keyword, words);
}

/* -------------------------------------------------------------------------- */
/* Object references                                                          */
/* -------------------------------------------------------------------------- */

/** An object reference read off the front of `words`, and what is left. */
interface ObjectRead {
  readonly target: ObjectRef;
  readonly rest: readonly Token[];
}

/**
 * `sequence 4`, `cue 1.5`, `group 3`, `view 2`, `executor 1`, `preset 6`.
 *
 * A **cue** may name its sequence — `sequence 5 cue 3` — and when it does not,
 * `sequenceId` is `null` and the daemon resolves it against
 * `Session::selectedSequence`. A client that filled it in would be sending a
 * command whose meaning had already moved on a second screen.
 */
function readObject(words: readonly Token[]): ObjectRead | string {
  const head = words[0];
  if (head === undefined) {
    return "which one? Try a word like sequence, cue, group, preset, view or executor.";
  }
  if (!(OBJECT_WORDS as readonly string[]).includes(head.text)) {
    return `"${head.raw}" is not something to name. Try sequence, cue, group, preset, view or executor.`;
  }
  const numberWord = words[1];
  if (numberWord === undefined) {
    return `${head.text} which one? Try "${head.text} 1".`;
  }
  // `sequence 5 cue 3` — a cue that names the list it is in.
  if (head.text === "sequence" && words[2]?.text === "cue") {
    const sequenceId = wholeNumber(numberWord.text);
    if (sequenceId === null) {
      return `"${numberWord.raw}" is not a sequence number.`;
    }
    const cueWord = words[3];
    if (cueWord === undefined) {
      return 'cue which one? Try "cue 1".';
    }
    if (!isCueNumber(cueWord.text)) {
      return `"${cueWord.raw}" is not a cue number.`;
    }
    return {
      target: { t: "Cue", sequenceId, cueNumber: cueWord.raw },
      rest: words.slice(4),
    };
  }
  if (head.text === "cue") {
    if (!isCueNumber(numberWord.text)) {
      return `"${numberWord.raw}" is not a cue number.`;
    }
    return {
      target: { t: "Cue", sequenceId: null, cueNumber: numberWord.raw },
      rest: words.slice(2),
    };
  }
  const id = wholeNumber(numberWord.text);
  if (id === null) {
    return `"${numberWord.raw}" is not a ${head.text} number.`;
  }
  const rest = words.slice(2);
  switch (head.text) {
    case "sequence":
      return { target: { t: "Sequence", sequenceId: id }, rest };
    case "group":
      return { target: { t: "Group", groupId: id }, rest };
    case "preset":
      return { target: { t: "Preset", presetId: id }, rest };
    case "view":
      return { target: { t: "View", viewId: id }, rest };
    default:
      return { target: { t: "Executor", executorId: id }, rest };
  }
}

/* -------------------------------------------------------------------------- */
/* The verbs                                                                  */
/* -------------------------------------------------------------------------- */

/**
 * `store cue 5`, `store sequence 4`, `store preset 1`, `store group 3`,
 * `store view 2`.
 *
 * Each carries the mode that **cannot lose anything** and a
 * {@link ModeQuestion} beside it, so an interface that finds something already
 * there can ask before sending. A `store view` is the odd one out and has no
 * mode at all: `StoreView` has always overwritten the layout, because that is
 * what storing a view *is*.
 */
function storeLine(words: readonly Token[]): ConsoleResult {
  const read = readObject(words.slice(1));
  if (typeof read === "string") {
    return { kind: "error", message: read };
  }
  const { target, rest } = read;
  const name = joinName(rest);
  switch (target.t) {
    case "Cue":
      return {
        kind: "commands",
        commands: [
          {
            t: "StoreCue",
            sequenceId: target.sequenceId,
            cueNumber: target.cueNumber,
            mode: "Merge",
          },
        ],
        question: { kind: "store", what: objectText(target), at: target },
      };
    case "Sequence":
      return {
        kind: "commands",
        commands: [
          {
            t: "StoreSequence",
            sequenceId: target.sequenceId,
            name: name === "" ? `Sequence ${String(target.sequenceId)}` : name,
            mode: "Append",
          },
        ],
        question: { kind: "sequence", what: objectText(target), at: target },
      };
    case "Preset": {
      // `Store Preset 1 Color "Deep blue"` — the pool may be named, and when it
      // is not the desk's answer is `Session::encoderBank`, resolved by the
      // daemon (`Command::StorePreset`). A name that happens to *be* one of the
      // five bank words is quoted, which is what the quotes are for.
      const pool = poolIn(rest);
      return {
        kind: "commands",
        commands: [
          {
            t: "StorePreset",
            presetId: target.presetId,
            pool,
            name: pool === null ? name : joinName(rest.slice(1)),
            // **A colour the line does not carry is one the daemon keeps.**
            // There is no word for a colour and there should not be: a store
            // that dropped `Preset::color` would throw away something an
            // operator chose with a picker and nothing here can put back.
            color: null,
            mode: "Merge",
          },
        ],
        question: { kind: "store", what: objectText(target), at: target },
      };
    }
    case "Group":
      return {
        kind: "commands",
        commands: [{ t: "StoreGroup", groupId: target.groupId, name, mode: "Merge" }],
        question: { kind: "overwrite", what: objectText(target), at: target },
      };
    case "View":
      return {
        kind: "commands",
        commands: [
          {
            t: "StoreView",
            viewId: target.viewId,
            name: name === "" ? `View ${String(target.viewId)}` : name,
          },
        ],
      };
    case "Executor":
      return {
        kind: "error",
        message:
          'an executor is assigned rather than stored. Try "assign sequence 5 executor 1".',
      };
  }
}

/** `edit cue 3` · `edit sequence 5 cue 3`. */
function editLine(words: readonly Token[]): ConsoleResult {
  const read = readObject(words.slice(1));
  if (typeof read === "string") {
    return { kind: "error", message: read };
  }
  if (read.target.t !== "Cue") {
    return {
      kind: "error",
      message: `edit takes a cue: try "edit cue 3", or "edit sequence 5 cue 3".`,
    };
  }
  if (read.rest.length > 0) {
    return tooMuch("edit", words);
  }
  return {
    kind: "commands",
    commands: [
      {
        t: "EditCue",
        sequenceId: read.target.sequenceId,
        cueNumber: read.target.cueNumber,
      },
    ],
  };
}

/**
 * `goto cue 5` · `goto sequence 2 cue 5` · `goto executor 1 cue 5`.
 *
 * A cue with no cue list named is the **selected** sequence's, which is
 * `PlaybackTarget::Selected` — the daemon resolves it, because the selection is
 * session state and a client that read it would be sending a command whose
 * meaning had already moved.
 */
function gotoLine(words: readonly Token[]): ConsoleResult {
  const rest = words.slice(1);
  // `goto executor 1 cue 5` — an executor and then the cue.
  if (rest[0]?.text === "executor") {
    const executorId = wholeNumber(rest[1]?.text ?? "");
    if (executorId === null) {
      return { kind: "error", message: 'executor which one? Try "goto executor 1 cue 5".' };
    }
    const cue = readObject(rest.slice(2));
    if (typeof cue === "string") {
      return { kind: "error", message: cue };
    }
    if (cue.target.t !== "Cue") {
      return { kind: "error", message: 'goto takes a cue. Try "goto executor 1 cue 5".' };
    }
    return {
      kind: "commands",
      commands: [
        {
          t: "Goto",
          target: { t: "Executor", executorId },
          cueNumber: cue.target.cueNumber,
        },
      ],
    };
  }
  const read = readObject(rest);
  if (typeof read === "string") {
    return { kind: "error", message: read };
  }
  if (read.target.t !== "Cue") {
    return { kind: "error", message: 'goto takes a cue. Try "goto cue 5".' };
  }
  const { sequenceId, cueNumber } = read.target;
  return {
    kind: "commands",
    commands: [
      {
        t: "Goto",
        target: sequenceId === null ? { t: "Selected" } : { t: "Sequence", sequenceId },
        cueNumber,
      },
    ],
  };
}

/** `delete sequence 4` and its five siblings. */
function deleteLine(words: readonly Token[]): ConsoleResult {
  const read = readObject(words.slice(1));
  if (typeof read === "string") {
    return { kind: "error", message: read };
  }
  if (read.rest.length > 0) {
    return tooMuch("delete", words);
  }
  return { kind: "commands", commands: [{ t: "Delete", target: read.target }] };
}

/** `move executor 1 executor 5` · `copy sequence 2 sequence 6`. */
function pairLine(words: readonly Token[], keyword: "move" | "copy"): ConsoleResult {
  const first = readObject(words.slice(1));
  if (typeof first === "string") {
    return { kind: "error", message: first };
  }
  const second = readObject(first.rest);
  if (typeof second === "string") {
    return {
      kind: "error",
      message: `${keyword} ${objectText(first.target)} where? Try "${keyword} sequence 2 sequence 6".`,
    };
  }
  if (second.rest.length > 0) {
    return tooMuch(keyword, words);
  }
  if (first.target.t !== second.target.t) {
    return {
      kind: "error",
      message: `${objectText(first.target)} and ${objectText(second.target)} are not the same kind of thing.`,
    };
  }
  const command: Command =
    keyword === "move"
      ? { t: "Move", from: first.target, to: second.target, mode: "Merge" }
      : { t: "Copy", from: first.target, to: second.target, mode: "Merge" };
  // **An executor and a view swap rather than overwriting**, so there is
  // nothing to ask: nothing is lost either way. Both are places on the desk
  // whose *number* is the position rather than a name for the thing in it —
  // see `prism_core::objects` and `SessionState::move_view`.
  if (
    keyword === "move" &&
    (first.target.t === "Executor" || first.target.t === "View")
  ) {
    return { kind: "commands", commands: [command] };
  }
  return {
    kind: "commands",
    commands: [command],
    question: { kind: "overwrite", what: objectText(second.target), at: second.target },
  };
}

/** `label view 1 "Programmer"`, and the same for the other five. */
function labelLine(words: readonly Token[]): ConsoleResult {
  const read = readObject(words.slice(1));
  if (typeof read === "string") {
    return { kind: "error", message: read };
  }
  return {
    kind: "commands",
    commands: [{ t: "Label", target: read.target, name: joinName(read.rest) }],
  };
}

/** `assign sequence 5 executor 1`. */
function assignLine(words: readonly Token[]): ConsoleResult {
  const sequence = readObject(words.slice(1));
  if (typeof sequence === "string") {
    return { kind: "error", message: sequence };
  }
  if (sequence.target.t !== "Sequence") {
    return { kind: "error", message: 'assign takes a sequence. Try "assign sequence 5 executor 1".' };
  }
  const executor = readObject(sequence.rest);
  if (typeof executor === "string") {
    return { kind: "error", message: 'assign it where? Try "assign sequence 5 executor 1".' };
  }
  if (executor.target.t !== "Executor") {
    return { kind: "error", message: 'a sequence is assigned to an executor. Try "assign sequence 5 executor 1".' };
  }
  if (executor.rest.length > 0) {
    return tooMuch("assign", words);
  }
  return {
    kind: "commands",
    commands: [
      {
        t: "AssignExecutor",
        executorId: executor.target.executorId,
        sequenceId: sequence.target.sequenceId,
      },
    ],
  };
}

/**
 * `on`, `off`, `go`, `go+`, `go-`, each with an executor, a sequence or
 * nothing.
 *
 * **Nothing means the selected sequence** — `PlaybackTarget::Selected`, which
 * is what makes a bare `Go+` on a desk with a cue list chosen mean something.
 * S26's `go 3` is kept: a bare number after one of these words is an executor,
 * which is what it meant before there was anything else it could be.
 */
function playbackLine(
  words: readonly Token[],
  keyword: "on" | "off" | "go" | "goback",
): ConsoleResult {
  const rest = words.slice(1);
  const target = readPlayback(rest);
  if (typeof target === "string") {
    return { kind: "error", message: target };
  }
  const command: Command =
    keyword === "on"
      ? { t: "ExecutorOn", target }
      : keyword === "off"
        ? { t: "ExecutorOff", target }
        : { t: "ExecutorGo", target, direction: keyword === "go" ? "Next" : "Prev" };
  return { kind: "commands", commands: [command] };
}

/** The target of a playback word: an executor, a sequence, or the selection. */
function readPlayback(words: readonly Token[]): PlaybackTarget | string {
  const head = words[0];
  if (head === undefined) {
    return { t: "Selected" };
  }
  // S26's form, kept and unbroken: a bare number is an executor.
  const bare = wholeNumber(head.text);
  if (bare !== null) {
    return words.length > 1
      ? `"${words.map((word) => word.raw).join(" ")}" says more than a playback takes.`
      : { t: "Executor", executorId: bare };
  }
  const read = readObject(words);
  if (typeof read === "string") {
    return read;
  }
  if (read.rest.length > 0) {
    return `"${read.rest.map((word) => word.raw).join(" ")}" says more than a playback takes.`;
  }
  switch (read.target.t) {
    case "Executor":
      return { t: "Executor", executorId: read.target.executorId };
    case "Sequence":
      return { t: "Sequence", sequenceId: read.target.sequenceId };
    default:
      return `${objectText(read.target)} is not something that plays back. Try an executor or a sequence.`;
  }
}

/** `page 2` — the fader bank. */
function pageCommand(words: readonly Token[]): ConsoleResult {
  const rest = words.slice(1);
  const first = rest[0];
  if (first === undefined) {
    return { kind: "error", message: 'page which page? Try "page 1".' };
  }
  if (rest.length > 1) {
    return tooMuch("page", words);
  }
  const page = wholeNumber(first.text);
  if (page === null) {
    return { kind: "error", message: `"${first.raw}" is not a page number.` };
  }
  return {
    kind: "commands",
    commands: [{ t: "SetExecutorPage", page }],
  };
}

/* -------------------------------------------------------------------------- */
/* Selections and levels                                                      */
/* -------------------------------------------------------------------------- */

/**
 * Everything else: a keyword on its own, a selection, a level, or both.
 *
 * `fixture` at the front is optional noise an operator may type out of habit,
 * and is accepted for exactly that reason.
 */
function selectionLine(words: readonly Token[]): ConsoleResult {
  // A bare keyword and a number is the *selecting* form of that word — see the
  // module documentation.
  const keyword = keywordSelection(words);
  if (keyword !== null) {
    return keyword;
  }
  const rest =
    words[0]?.text === "fixture" || words[0]?.text === "fixtures" ? words.slice(1) : words;
  const at = rest.findIndex((word) => word.text === "at");
  const commands: Command[] = [];

  // The attribute may sit on either side of `at` — `5 pan at 25` is how a
  // console reads aloud and `5 at pan 25` is how one is often typed — so it is
  // taken out of the line before either half is read.
  const named = attributeIn(rest);
  const without = named === null ? rest : rest.filter((word) => word.text !== named.word);
  const atWithout =
    named === null ? at : without.findIndex((word) => word.text === "at");

  const selectionWords = atWithout === -1 ? without : without.slice(0, atWithout);
  if (selectionWords.length > 0) {
    const ids = readFixtures(selectionWords);
    if (typeof ids === "string") {
      return { kind: "error", message: ids };
    }
    commands.push({ t: "SelectFixtures", ids, mode: "Set" });
  }

  if (atWithout === -1) {
    if (commands.length === 0) {
      return {
        kind: "error",
        message: `"${words.map((word) => word.raw).join(" ")}" is not a command.`,
      };
    }
    return { kind: "commands", commands };
  }

  const level = readLevel(without.slice(atWithout + 1), named?.attribute ?? "Dimmer");
  if (typeof level === "string") {
    return { kind: "error", message: level };
  }
  commands.push(level);
  return { kind: "commands", commands };
}

/**
 * `Group 3`, `Sequence 5`, `View 2`, `Executor 4`, `Preset 1` — the selecting
 * form of an argument keyword.
 *
 * Answers `null` when the line is not one of these, which is when it is a
 * fixture selection or a level. A **cue** on its own is deliberately not here:
 * `Cue 5` could be a Goto or an Edit and the desk must not guess, so it says so.
 */
function keywordSelection(words: readonly Token[]): ConsoleResult | null {
  const head = words[0];
  if (head === undefined || head.text === "fixture" || head.text === "fixtures") {
    return null;
  }
  if (!(OBJECT_WORDS as readonly string[]).includes(head.text)) {
    return null;
  }
  if (head.text === "cue") {
    return {
      kind: "error",
      message: 'a cue on its own is ambiguous. Try "goto cue 5" or "edit cue 5".',
    };
  }
  const read = readObject(words);
  if (typeof read === "string") {
    return { kind: "error", message: read };
  }
  if (read.rest.length > 0) {
    return tooMuch(head.text, words);
  }
  switch (read.target.t) {
    case "Group":
      return {
        kind: "commands",
        commands: [{ t: "SelectGroup", groupId: read.target.groupId, mode: "Set" }],
      };
    case "Sequence":
      return {
        kind: "commands",
        commands: [{ t: "SelectSequence", sequenceId: read.target.sequenceId }],
      };
    case "View":
      return {
        kind: "commands",
        commands: [{ t: "SelectView", viewId: read.target.viewId }],
      };
    case "Executor":
      return {
        kind: "commands",
        commands: [{ t: "SelectExecutor", executorId: read.target.executorId }],
      };
    case "Preset":
      return {
        kind: "commands",
        commands: [{ t: "ApplyPreset", presetId: read.target.presetId }],
      };
    // A cue is turned away above, so nothing else reaches this.
    default:
      return null;
  }
}

/**
 * The `at` half of a line: a percentage, for the attribute the line named.
 *
 * The attribute defaults to `Dimmer`, because `1 at 50` means intensity on
 * every lighting desk there has ever been.
 */
function readLevel(words: readonly Token[], attribute: AttributeType): Command | string {
  const first = words[0];
  if (first === undefined) {
    return "at what? A level is a percentage: 0 to 100.";
  }
  if (words.length > 1) {
    return `"${words.map((word) => word.raw).join(" ")}" is more than a level. A level is a percentage: 0 to 100.`;
  }
  if (first.text === "full") {
    return { t: "SetAttribute", attribute, value: levelFromPercent(100), relative: false };
  }
  if (first.text === "out" || first.text === "zero") {
    return { t: "SetAttribute", attribute, value: 0, relative: false };
  }
  const percent = Number(first.text);
  if (!Number.isFinite(percent) || first.text === "") {
    return `"${first.raw}" is not a percentage. A level is 0 to 100, or "full".`;
  }
  if (percent < 0 || percent > 100) {
    return `${first.raw} % is outside 0 to 100.`;
  }
  return {
    t: "SetAttribute",
    attribute,
    value: levelFromPercent(percent),
    relative: false,
  };
}

/**
 * The feature group the first of these words names, if it names one.
 *
 * Only the **first**: `Store Preset 1 Color Deep blue` names the Colour pool and
 * is called *Deep blue*, and a name with a bank word later in it is a name.
 */
function poolIn(words: readonly Token[]): FeatureGroup | null {
  const head = words[0];
  if (head === undefined) {
    return null;
  }
  return (
    FEATURE_GROUP_VARIANTS.find((group) => group.toLowerCase() === head.text) ?? null
  );
}

/** The attribute one of these words names, if any of them does. */
function attributeIn(
  words: readonly Token[],
): { readonly word: string; readonly attribute: AttributeType } | null {
  for (const word of words) {
    for (const attribute of ATTRIBUTE_TYPE_VARIANTS) {
      if (attribute.toLowerCase() === word.text) {
        return { word: word.text, attribute };
      }
    }
  }
  return null;
}

/**
 * The fixture numbers a selection names.
 *
 * Answers a message rather than throwing on anything that is not one. A range
 * runs in the direction it is written — `4 thru 1` is 4, 3, 2, 1 — because
 * selection order is what an operator sees when they fan a value across it.
 */
function readFixtures(words: readonly Token[]): number[] | string {
  const ids: number[] = [];
  const seen = new Set<number>();
  let expecting: "range" | "separator" = "range";
  let index = 0;
  while (index < words.length) {
    const word = words[index]?.text ?? "";
    const raw = words[index]?.raw ?? "";
    if (expecting === "separator") {
      if (word !== "+") {
        return `"${raw}" is not a fixture. Separate fixtures with + and ranges with thru.`;
      }
      expecting = "range";
      index += 1;
      continue;
    }
    const from = wholeNumber(word);
    if (from === null) {
      return `"${raw}" is not a fixture number.`;
    }
    index += 1;
    if (words[index]?.text === THRU) {
      const toWord = words[index + 1];
      if (toWord === undefined) {
        return "thru what? A range is two numbers, as in 1 thru 4.";
      }
      const to = wholeNumber(toWord.text);
      if (to === null) {
        return `"${toWord.raw}" is not a fixture number.`;
      }
      if (Math.abs(to - from) > MAX_RANGE) {
        return `${String(from)} thru ${String(to)} is more than ${String(MAX_RANGE)} fixtures.`;
      }
      const step = to >= from ? 1 : -1;
      for (let id = from; step > 0 ? id <= to : id >= to; id += step) {
        push(ids, seen, id);
      }
      index += 2;
    } else {
      push(ids, seen, from);
    }
    expecting = "separator";
  }
  if (expecting === "range") {
    return "the line ends with a +, so something is missing after it.";
  }
  return ids;
}

/**
 * The most fixtures one range may name.
 *
 * A guard rather than a limit anybody will meet: `1 thru 999999999` is a typo,
 * and building that array is how a console stops answering. The daemon would
 * refuse the command anyway; the point is to say so before the browser has
 * spent a second on it.
 */
export const MAX_RANGE = 4096;

/** Appends a fixture number once. A selection holds no fixture twice. */
function push(ids: number[], seen: Set<number>, id: number): void {
  if (!seen.has(id)) {
    seen.add(id);
    ids.push(id);
  }
}

/** A whole number that is not negative, or `null`. */
function wholeNumber(word: string): number | null {
  if (!/^\d+$/.test(word)) {
    return null;
  }
  const value = Number(word);
  return Number.isSafeInteger(value) ? value : null;
}

/**
 * Whether a word is a cue number.
 *
 * Cue numbers are **strings** on the wire (`ARCHITECTURE_SPEC.md` §6) so that
 * `1.5` sorts between `1` and `2` without a float entering the show file, and
 * this is the same shape read back: digits, with at most one point in them.
 */
function isCueNumber(word: string): boolean {
  return /^\d+(\.\d+)?$/.test(word);
}

/** The name at the end of a line, as it was typed, with any quotes taken off. */
function joinName(words: readonly Token[]): string {
  return words
    .map((word) => word.raw)
    .join(" ")
    .trim()
    .replace(/^"(.*)"$/s, "$1");
}

/** The complaint for a line that says more than its first word allows. */
function tooMuch(keyword: string, words: readonly Token[]): ConsoleResult {
  return {
    kind: "error",
    message: `"${words.map((word) => spoken(word.raw)).join(" ")}" says more than ${keyword} takes.`,
  };
}

/**
 * A token as the operator typed it.
 *
 * `go-` becomes the token `goback` in {@link tokenise} so that the `+` and `-`
 * of `go+` and `go-` are not read as separators; a message about it has to say
 * the word that was typed.
 */
function spoken(word: string): string {
  return word === "goback" ? "go-" : word;
}

/**
 * A line as words, each with the spelling the operator used beside it.
 *
 * `+`, `,` and `thru` are separators as well as words, so `1+2thru4` reads the
 * same as `1 + 2 thru 4` — a console keypad has digits and a few keys, and an
 * operator should not have to hunt for a space bar. A comma reads as a `+`,
 * because both mean *and also*.
 *
 * **A quoted run is one token and keeps its spaces**, which is what makes
 * `Label View 1 "House lights"` a two-word name rather than two words.
 */
function tokenise(line: string): Token[] {
  const out: Token[] = [];
  for (const chunk of line.match(/"[^"]*"?|\S+/g) ?? []) {
    if (chunk.startsWith('"')) {
      const raw = chunk.replace(/^"/, "").replace(/"$/, "");
      out.push({ text: raw.toLowerCase(), raw });
      continue;
    }
    const pieces = chunk
      // `go+` and `go-` first, or the `+` below would split the first of them
      // into a keyword and a separator. `goback` is the token the second
      // becomes, so the two directions are words rather than punctuation.
      .replace(/go\+/gi, " go ")
      .replace(/go-/gi, " goback ")
      .replace(/,/g, " + ")
      .replace(/\+/g, " + ")
      .replace(/thru/gi, " thru ")
      .split(/\s+/);
    for (const piece of pieces) {
      if (piece !== "") {
        out.push({ text: piece.toLowerCase(), raw: piece });
      }
    }
  }
  return out;
}

/* -------------------------------------------------------------------------- */
/* Completion                                                                 */
/* -------------------------------------------------------------------------- */

/**
 * The words that are legal **at this point in the line**, longest prefix first.
 *
 * Client-local, like the history beside it (`ARCHITECTURE_SPEC.md` §4.2): what
 * one operator is half-way through typing is not something a second screen
 * should be completing.
 *
 * It is deliberately a *grammar* answer and not a *show* answer — it offers the
 * word `sequence`, never the sequences there are, for the same reason the parser
 * does not read the show. A list of what exists is what the pools on the canvas
 * are for.
 */
export function completions(line: string): readonly string[] {
  const words = tokenise(line);
  const trailing = /\s$/.test(line);
  const typed = trailing ? "" : (words.at(-1)?.text ?? "");
  const before = trailing ? words : words.slice(0, -1);
  const legal = legalWords(before);
  return legal.filter((word) => word.startsWith(typed) && word !== typed);
}

/** Which words may come next, given what is already there. */
function legalWords(before: readonly Token[]): readonly string[] {
  const head = before[0];
  if (head === undefined) {
    return CONSOLE_WORDS;
  }
  switch (head.text) {
    case "store":
    case "delete":
    case "move":
    case "copy":
    case "label":
    case "goto":
    case "edit":
    case "assign":
    case "on":
    case "off":
    case "go":
    case "goback":
      // After a verb, and after each of its arguments, the words that name a
      // thing. A number is not offered: there is nothing to complete about one.
      return OBJECT_WORDS;
    default:
      return before.length === 1 && (head.text === "at" || wholeNumber(head.text) !== null)
        ? ["at", "thru", "full"]
        : [];
  }
}
