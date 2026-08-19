/**
 * The encoder bar: five banks, the parameters of the one in force, and the
 * programmer they write into.
 *
 * # The order is a contract with the console
 *
 * The parameters come from `FEATURE_GROUP_ATTRIBUTES`, which `prism-domain`
 * generates from `FeatureGroup::attributes` — and `prismd::surface::parameter_of`
 * resolves the jog wheel through the **same** table. That is S22's open warning
 * answered by construction rather than by agreement: if the two lists could
 * differ, the wheel would turn one parameter while another was highlighted, and
 * nobody would think to blame a table. There is one list.
 *
 * The highlighted encoder is `/session/programmerParamIndex`, so `Zoom ◀▶` on
 * the X-Touch moves the highlight here, and the two arrows here move it on the
 * desk. Same command, same field, both directions.
 *
 * # An encoder is relative, and that is why there is nothing local
 *
 * A turn sends `SetAttribute { relative: true }`. There is no local value to
 * hold and none to drop: what an encoder reads is the programmer's value, and it
 * moves when `ProgrammerChanged` comes back. **D3** costs nothing here. What is
 * still local is the cadence — see `./valuedrag.ts`.
 *
 * # An untouched parameter reads as a dash, never as 0 %
 *
 * The programmer is sparse by specification: absent means *the playbacks
 * decide*, and zero means *black*. An encoder bar that displayed 0 % for an
 * untouched attribute would teach an operator that the two are the same thing.
 */

import { useEffect, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

import type { FeatureGroup, JsonValue, ProgrammerState } from "../bindings";
import { FEATURE_GROUP_VARIANTS } from "../bindings/variants";
import type { ParameterReading } from "./programmer";
import {
  bankReadings,
  clearStage,
  encoderPage,
  selectionText,
  selectionSize,
  sourceText,
  touchedBanks,
  touchedCount,
  valueText,
} from "./programmer";
import { encoderBank, programmerPage, programmerParamIndex } from "./session";
import { EncoderDrag } from "./valuedrag";

/** What the bar needs. */
export interface EncoderBarProps {
  /** The session document. */
  readonly session: JsonValue;
  /** The show document, for which bank an attribute is filed under. */
  readonly show: JsonValue;
  /** The programmer, which is what the encoders read and write. */
  readonly programmer: ProgrammerState | null;
  /** Sends a `SetEncoderBank`. */
  readonly onBank: (group: FeatureGroup) => void;
  /** Sends a `SelectProgrammerParam`, once per step. */
  readonly onParam: (direction: "Prev" | "Next") => void;
  /** Sends a `SetProgrammerPage`, which is absolute. */
  readonly onPage: (page: number) => void;
  /** Sends a `SetAttribute` with `relative` set. */
  readonly onTurn: (reading: ParameterReading, delta: number) => void;
  /** Sends a `ClearProgrammer`. */
  readonly onClear: () => void;
}

/** The bar. */
export function EncoderBar({
  session,
  show,
  programmer,
  onBank,
  onParam,
  onPage,
  onTurn,
  onClear,
}: EncoderBarProps) {
  const bank = encoderBank(session);
  const readings = bankReadings(programmer, show, bank);
  const selectedIndex = programmerParamIndex(session);
  const touched = touchedBanks(programmer, show);
  const stage = clearStage(programmer);
  // The page the *session* is asking for, clamped to what this bank has. Both
  // hands change the same field: `Zoom ▲▼` on the console and the two buttons
  // below send `SetProgrammerPage`, and neither of them moves anything until the
  // delta comes back.
  const paged = encoderPage(readings, programmerPage(session));

  return (
    <section className="encbar" data-testid="encoder-bar" aria-label="Encoders">
      <div className="encbar-banks">
        {FEATURE_GROUP_VARIANTS.map((group) => (
          <button
            key={group}
            type="button"
            className={`bank${group === bank ? " bank-active" : ""}${
              touched.includes(group) ? " bank-touched" : ""
            }`}
            data-testid={`bank-${group}`}
            data-active={group === bank ? "yes" : "no"}
            data-touched={touched.includes(group) ? "yes" : "no"}
            title={
              touched.includes(group)
                ? `${group} — the programmer is holding values on this bank`
                : group
            }
            onClick={() => {
              onBank(group);
            }}
          >
            {group}
          </button>
        ))}
      </div>

      <div className="encbar-encoders" data-testid="encoders">
        {paged.readings.map((reading) => (
          <Encoder
            key={reading.attribute}
            reading={reading}
            selected={reading.index === selectedIndex}
            selectable={selectionSize(programmer) > 0}
            onSelect={() => {
              step(selectedIndex, reading.index, onParam);
            }}
            onTurn={onTurn}
          />
        ))}
      </div>

      <div className="encbar-steps">
        <div className="encbar-param" data-testid="param-steps">
          <button
            type="button"
            className="page-step"
            data-testid="param-prev"
            aria-label="Previous parameter"
            disabled={selectedIndex === 0}
            onClick={() => {
              onParam("Prev");
            }}
          >
            ◀
          </button>
          <button
            type="button"
            className="page-step"
            data-testid="param-next"
            aria-label="Next parameter"
            // The daemon does not bound this — it cannot, because how many
            // parameters there are is the bank's question — so the bar does. An
            // index past the end leaves the jog wheel turning nothing at all.
            disabled={selectedIndex >= readings.length - 1}
            onClick={() => {
              onParam("Next");
            }}
          >
            ▶
          </button>
        </div>
        {/*
          The page control, which is `Zoom ▲▼` on the console (D8,
          `docs/MCU_MAPPING.md` §4.1). Both send `SetProgrammerPage` and neither
          moves anything until the delta comes back — so *paging from either end
          is the same act*, which is what S35 put this here for. A bank that fits
          on one page disables both, because there is nowhere to go.
        */}
        <div className="encbar-page" data-testid="page-steps">
          <button
            type="button"
            className="page-step"
            data-testid="encoder-page-up"
            aria-label="Previous parameter page"
            disabled={paged.page === 0}
            onClick={() => {
              onPage(paged.page - 1);
            }}
          >
            ▲
          </button>
          <span className="encbar-page-number" data-testid="programmer-page">
            {paged.page + 1}/{paged.pages}
          </span>
          <button
            type="button"
            className="page-step"
            data-testid="encoder-page-down"
            aria-label="Next parameter page"
            disabled={paged.page >= paged.pages - 1}
            onClick={() => {
              onPage(paged.page + 1);
            }}
          >
            ▼
          </button>
        </div>
      </div>

      <dl className="encbar-readout" data-testid="programmer-readout">
        <div className="reading">
          <dt>Sel</dt>
          <dd data-testid="selection">{selectionText(programmer)}</dd>
        </div>
        <div className="reading">
          <dt>Values</dt>
          <dd data-testid="touched">{touchedCount(programmer)}</dd>
        </div>
        <button
          type="button"
          className={`clear-button clear-stage-${String(stage)}`}
          data-testid="clear"
          data-stage={stage}
          title={CLEAR_TITLES[stage] ?? CLEAR_TITLES[0]}
          onClick={onClear}
        >
          Clear
        </button>
      </dl>
    </section>
  );
}

/** What the next press of Clear will do, by the stage it is at. */
const CLEAR_TITLES: readonly string[] = [
  "Clear the programmer values, keeping the selection",
  "Clear the selection as well",
  "Clear everything, including the encoder bank",
];

/**
 * Moves the highlight from `from` to `to`, one command per step.
 *
 * The protocol has `SelectProgrammerParam { direction }` and no absolute form —
 * because the console has no absolute form either: `Zoom ◀▶` steps. Rather than
 * add a thirteenth session command for a click, the bar composes the one that
 * exists. A bank has at most six parameters, so this is at most five commands
 * and the daemon applies them in order.
 */
function step(from: number, to: number, onParam: (direction: "Prev" | "Next") => void): void {
  const direction = to > from ? "Next" : "Prev";
  for (let at = 0; at < Math.abs(to - from); at += 1) {
    onParam(direction);
  }
}

/** One encoder. */
function Encoder({
  reading,
  selected,
  selectable,
  onSelect,
  onTurn,
}: {
  readonly reading: ParameterReading;
  readonly selected: boolean;
  readonly selectable: boolean;
  readonly onSelect: () => void;
  readonly onTurn: (reading: ParameterReading, delta: number) => void;
}) {
  const begin = useEncoder(reading, onTurn);
  return (
    <button
      type="button"
      className={`encoder${selected ? " encoder-selected" : ""}${
        reading.available === 0 ? " encoder-absent" : ""
      }`}
      data-testid={`encoder-${reading.attribute}`}
      data-selected={selected ? "yes" : "no"}
      data-index={reading.index}
      title={
        reading.available === 0
          ? `Nothing selected has ${reading.attribute}`
          : `${reading.attribute} — drag to change, Shift for fine`
      }
      aria-disabled={selectable ? undefined : true}
      onPointerDown={begin}
      onClick={onSelect}
    >
      <span className="encoder-name">{reading.attribute}</span>
      <span className="encoder-value" data-testid={`value-${reading.attribute}`}>
        {valueText(reading)}
      </span>
      <span className="encoder-foot">
        <span className="encoder-source" data-testid={`source-${reading.attribute}`}>
          {sourceText(reading)}
        </span>
        <span className="encoder-count" data-testid={`count-${reading.attribute}`}>
          {reading.available === 0 ? "—" : `${String(reading.held)}/${String(reading.available)}`}
        </span>
      </span>
    </button>
  );
}

/** The turn gesture: relative commands, paced, with nothing held locally. */
function useEncoder(
  reading: ParameterReading,
  onTurn: (reading: ParameterReading, delta: number) => void,
): (event: ReactPointerEvent) => void {
  const [drag, setDrag] = useState<EncoderDrag | null>(null);

  useEffect(() => {
    if (drag === null) {
      return;
    }
    const move = (event: PointerEvent): void => {
      drag.to(event.clientX, Date.now());
    };
    const finish = (): void => {
      drag.end(Date.now());
      setDrag(null);
    };
    globalThis.addEventListener("pointermove", move);
    globalThis.addEventListener("pointerup", finish);
    globalThis.addEventListener("pointercancel", finish);
    return () => {
      globalThis.removeEventListener("pointermove", move);
      globalThis.removeEventListener("pointerup", finish);
      globalThis.removeEventListener("pointercancel", finish);
    };
  }, [drag]);

  return (event: ReactPointerEvent): void => {
    if (event.button !== 0) {
      return;
    }
    setDrag(
      new EncoderDrag({
        from: event.clientX,
        fine: event.shiftKey,
        send: (delta) => {
          onTurn(reading, delta);
        },
      }),
    );
  };
}
