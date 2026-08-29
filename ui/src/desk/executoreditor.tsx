/**
 * The control editor: what an executor's fader, encoder and four keys do — S45,
 * punch-list entry **B15**.
 *
 * Until this session, what a fader and its keys did was decided by
 * `prism_core::Show::assign_executor`'s defaults the moment a cue list was put
 * on the slot, and could not be changed from anywhere at all — not from a
 * window, not from the line, not from the desk. The entry says it in one
 * sentence: *es passiert nichts wenn man einen Executor rechtsklickt.*
 *
 * # It writes lines, like every other key on this desk
 *
 * `ARCHITECTURE_SPEC.md` §4.5 makes the command line **the** interface and names
 * the exceptions — the things a line cannot say. This is not one of them,
 * because S45 gave the grammar the words: `Assign Executor 1 Fader Master`,
 * `Assign Executor 1 Button 2 Go+`, `Assign Executor 1 Encoder Speed`. So every
 * chooser here writes that line and sends it, exactly as a tile in a pool does
 * (§4.6, `consoleshell.ts::pickOnto`), and the three ways of saying it — this
 * window, a typed line, a bound X-Touch key — end in one `ConfigureExecutor`
 * rather than three code paths that have to be kept in step.
 *
 * The **strip** above it is still §4.5's exception and is untouched: a Go is a
 * gesture with timing in it and a fader is a stream of positions.
 *
 * # The lists come from Rust
 *
 * `EXECUTOR_FADER_FUNCTION_VARIANTS`, `EXECUTOR_ENCODER_FUNCTION_VARIANTS` and
 * the eight fixed button functions are generated from `prism_domain`, so a
 * function added there appears in these choosers without a second list being
 * edited — §4.6's rule for `WindowType`, applied to the three enums an executor
 * has. The **ninth** row is not in any of them and is not a variant somebody has
 * to add: *send this command line* is a row an operator fills in, which is the
 * owner's answer from S43's third round, because every desk needs a different
 * number of them.
 */

import { useEffect, useState } from "react";

import type { ExecutorFaderFunction, JsonValue } from "../bindings";
import {
  EXECUTOR_ENCODER_FUNCTION_VARIANTS,
  EXECUTOR_FADER_FUNCTION_VARIANTS,
} from "../bindings/variants";
import { useConsole } from "./consoleshell";
import {
  FIXED_BUTTON_FUNCTIONS,
  buttonName,
  commandLineOf,
  encoderName,
  faderName,
} from "./functions";
import type { FixedButtonFunction } from "./functions";
import { EXECUTOR_BUTTONS, pageStrips, selectedExecutor } from "./session";
import type { ExecutorStrip } from "./session";

/** The value the custom row carries in a `<select>`, which no enum spelling is. */
const CUSTOM = "Command";

/** The editor, under the strip. */
export function ExecutorEditor({
  show,
  session,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
}) {
  const chosen = selectedExecutor(session);
  const strip = pageStrips(session, show).find((row) => row.executorId === chosen) ?? null;

  return (
    <section className="exec-editor" data-testid="executor-editor" aria-label="Executor controls">
      {strip === null || !strip.assigned ? (
        <p className="exec-editor-empty" data-testid="editor-none">
          {chosen === null
            ? "Select an executor to say what its fader, encoder and keys do."
            : `Executor ${String(chosen)} is not on this page.`}
        </p>
      ) : (
        <Rows strip={strip} />
      )}
    </section>
  );
}

/** The six rows of one executor. */
function Rows({ strip }: { readonly strip: ExecutorStrip }) {
  const { run } = useConsole();
  const assign = (words: string): void => {
    run(`Assign Executor ${String(strip.executorId)} ${words}`);
  };

  return (
    <div className="exec-editor-rows" data-testid="editor-executor" data-executor={strip.executorId}>
      <header className="exec-editor-head">
        <span className="exec-editor-number">Executor {strip.executorId}</span>
        <span className="exec-editor-name">{strip.name ?? "—"}</span>
      </header>
      <Row label="Fader" testId="editor-fader">
        <select
          data-testid="editor-fader"
          value={strip.faderFunction ?? "Empty"}
          onChange={(event) => {
            assign(`Fader ${event.target.value}`);
          }}
        >
          {EXECUTOR_FADER_FUNCTION_VARIANTS.map((option: ExecutorFaderFunction) => (
            <option key={option} value={option}>
              {faderName(option)}
            </option>
          ))}
        </select>
      </Row>
      <Row label="Encoder" testId="editor-encoder">
        <select
          data-testid="editor-encoder"
          value={strip.encoderFunction ?? "Empty"}
          onChange={(event) => {
            assign(`Encoder ${event.target.value}`);
          }}
        >
          {EXECUTOR_ENCODER_FUNCTION_VARIANTS.map((option) => (
            <option key={option} value={option}>
              {encoderName(option)}
            </option>
          ))}
        </select>
      </Row>
      {Array.from({ length: EXECUTOR_BUTTONS }, (_unused, index) => (
        <ButtonRow key={index} index={index} strip={strip} assign={assign} />
      ))}
    </div>
  );
}

/**
 * One key, and the box the custom row needs.
 *
 * The four are named for the hardware they sit under — Rec, Solo, Mute, Select
 * (`docs/MCU_MAPPING.md` §2.1) — because that is what an operator is looking at
 * when they reach for one, and numbered beside it because that is what the
 * command line calls them.
 */
function ButtonRow({
  index,
  strip,
  assign,
}: {
  readonly index: number;
  readonly strip: ExecutorStrip;
  readonly assign: (words: string) => void;
}) {
  const held = strip.buttonFunctions[index] ?? "Empty";
  const line = commandLineOf(held);
  // **Two pieces of local state, and both are the operator mid-gesture.**
  // Choosing *Command line…* opens the box before there is a line to open it
  // with, so whether the row is open cannot be read off the show; and what is
  // being typed is not the show's until it is sent. `ARCHITECTURE_SPEC.md` §4.2
  // is the rule — a half-typed line is this screen's, like a scroll position.
  const [custom, setCustom] = useState(line !== null);
  const [draft, setDraft] = useState(line ?? "");
  // **The daemon's answer wins.** Once the show says something else — this
  // client's own send, or a second screen's — the row shows that. The same rule
  // `desk/executorbar.tsx` follows for a fader drag.
  useEffect(() => {
    setCustom(line !== null);
    setDraft(line ?? "");
  }, [line]);
  const chosen: FixedButtonFunction | typeof CUSTOM =
    custom || typeof held !== "string" ? CUSTOM : held;

  const send = (): void => {
    // Quoted, because a line has spaces in it and `joinName` takes the quotes
    // off again. An empty box sends nothing rather than a command that would be
    // refused.
    if (draft.trim() !== "") {
      assign(`Button ${String(index + 1)} Command ${JSON.stringify(draft)}`);
    }
  };

  return (
    <Row label={`${KEYS[index] ?? "Key"} · ${String(index + 1)}`} testId={`editor-button-${String(index)}`}>
      <select
        data-testid={`editor-button-${String(index)}`}
        value={chosen}
        onChange={(event) => {
          const value = event.target.value;
          if (value === CUSTOM) {
            // Choosing the row does not send anything: there is nothing to send
            // until the operator has written the line. The box appears, and the
            // key keeps what it had until they do.
            setCustom(true);
            return;
          }
          setCustom(false);
          assign(`Button ${String(index + 1)} ${value}`);
        }}
      >
        {FIXED_BUTTON_FUNCTIONS.map((option: FixedButtonFunction) => (
          <option key={option} value={option}>
            {buttonName(option)}
          </option>
        ))}
        <option value={CUSTOM}>Command line…</option>
      </select>
      {chosen === CUSTOM && (
        <span className="exec-editor-line">
          <input
            type="text"
            data-testid={`editor-line-${String(index)}`}
            aria-label={`Command line for key ${String(index + 1)} of executor ${String(strip.executorId)}`}
            placeholder="Go+ Sequence 3"
            value={draft}
            onChange={(event) => {
              setDraft(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                send();
              }
            }}
          />
          <button type="button" data-testid={`editor-send-${String(index)}`} onClick={send}>
            Set
          </button>
        </span>
      )}
    </Row>
  );
}

/** One labelled row. The label is a `label` so the control is reachable by name. */
function Row({
  label,
  testId,
  children,
}: {
  readonly label: string;
  readonly testId: string;
  readonly children: React.ReactNode;
}) {
  return (
    <label className="exec-editor-row" htmlFor={testId}>
      <span className="exec-editor-label">{label}</span>
      {children}
    </label>
  );
}

/** What the four keys are called on the hardware — `docs/MCU_MAPPING.md` §2.1. */
const KEYS = ["Rec", "Solo", "Mute", "Select"] as const;
