/**
 * One encoder, and the gesture that turns it.
 *
 * Lifted out of the encoder bar in **S43**, when the bar became the programmer
 * band the owner's skeleton draws (`./programmerband.tsx`). Nothing about the
 * control changed; what changed is that it now lives in a file with no bar
 * around it, so the band and anything later that wants an encoder read the same
 * one.
 *
 * # An encoder is relative, and that is why there is nothing local
 *
 * A turn sends `SetAttribute { relative: true }`. There is no local value to
 * hold and none to drop: what an encoder reads is the programmer's value, and it
 * moves when `ProgrammerChanged` comes back. **D3** costs nothing here. What is
 * still local is the cadence — see `./valuedrag.ts`.
 *
 * # What the number is, and what the mark is — S43
 *
 * It used to read a dash when the programmer held nothing, on the argument that
 * **absent is not zero**: the programmer is sparse, absence means *the playbacks
 * decide*, and 0 % means *black*. The argument was right and the dash was the
 * wrong way to carry it — B1 gave every attribute a resting value worth reading
 * (a colour rests **open**), and an operator looking at a dash went hunting for
 * the value at the bottom of the range.
 *
 * So the number is the programmer's value when it holds one and the resting
 * value when it does not, and the difference an operator actually needs — *is
 * this attribute mine?* — is a **mark** instead: `data-overriding`, a bar down
 * the side, and the word in the tooltip. A marked encoder sends its value
 * whatever a playback says; an unmarked one is showing where the lamp rests.
 *
 * Clicking one takes it over at the value it is already showing. That is the
 * owner's own wording, and it is a turn of nought — see `App.tsx`'s `onTake`.
 *
 * # And a channel with named ranges says which one it is in — S51, B38
 *
 * An Open Fixture Library channel can say *0–9 open, 10–19 gobo 1, 20–29 gobo
 * 2*. Until S51 none of that was read anywhere, so a gobo wheel was a number an
 * operator had to know by heart. It is read now (`prism_domain::AttributeRange`),
 * and this encoder does the two things that follow from having it:
 *
 * - it **names** the range the value is standing in, under the number; and
 * - it **offers the list**, so a range can be picked instead of hunted for.
 *
 * The picker writes the **middle** of the range it was given
 * (`AttributeRange::middle`), which is the furthest any single value can be from
 * both edges — a fixture whose thresholds are a step out from its manual still
 * lands on the slot that was asked for. Nothing else changes: a gobo is still
 * one number, a cue still stores that number, and the engine has never heard of
 * a range. See `prism_domain::AttributeRange` for the line this session drew.
 */

import { useEffect, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

import type { AttributeRange } from "../bindings";
import type { ParameterReading } from "./programmer";
import { sourceText, valueText } from "./programmer";
import { EncoderDrag } from "./valuedrag";

/** One encoder. */
export function Encoder({
  reading,
  selected,
  selectable,
  onSelect,
  onTurn,
  onPickRange,
}: {
  readonly reading: ParameterReading;
  readonly selected: boolean;
  readonly selectable: boolean;
  readonly onSelect: () => void;
  readonly onTurn: (reading: ParameterReading, delta: number) => void;
  /** **B38.** Sets this attribute to the middle of a named range. */
  readonly onPickRange: (reading: ParameterReading, range: AttributeRange) => void;
}) {
  const begin = useEncoder(reading, onTurn);
  const [listing, setListing] = useState(false);
  const ranges = reading.available === 0 ? [] : reading.ranges;
  return (
    <div className="encoder-slot">
    <button
      type="button"
      className={`encoder${selected ? " encoder-selected" : ""}${
        reading.available === 0 ? " encoder-absent" : ""
      }${reading.overriding ? " encoder-overriding" : ""}`}
      data-testid={`encoder-${reading.attribute}`}
      data-selected={selected ? "yes" : "no"}
      data-overriding={reading.overriding ? "yes" : "no"}
      data-index={reading.index}
      title={
        reading.available === 0
          ? `Nothing selected has ${reading.attribute}`
          : reading.overriding
            ? `${reading.attribute} — overriding: this value goes out whatever the playbacks say. Drag to change, Shift for fine`
            : `${reading.attribute} — resting value; click to take it over. Drag to change, Shift for fine`
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
      {/*
        **B38.** The name of the range the encoder is standing in, and the way
        into the list. Drawn only where there is one, because a row of empty
        space under every continuous encoder would take height off the canvas
        for nothing — `CLAUDE.md`'s device-screen rule, and the band's height is
        fixed.
      */}
      {ranges.length === 0 ? null : (
        <button
          type="button"
          className="encoder-range"
          data-testid={`range-${reading.attribute}`}
          data-open={listing ? "yes" : "no"}
          title={`Pick a named range of ${reading.attribute}`}
          aria-expanded={listing}
          onClick={() => {
            setListing((open) => !open);
          }}
        >
          {reading.range ?? "—"}
        </button>
      )}
      {listing && ranges.length > 0 ? (
        <ul className="encoder-ranges" data-testid={`ranges-${reading.attribute}`}>
          {ranges.map((range) => (
            <li key={`${range.name}-${String(range.from)}`}>
              <button
                type="button"
                className={range.name === reading.range ? "linkish range-here" : "linkish"}
                data-testid={`range-${reading.attribute}-${range.name}`}
                onClick={() => {
                  setListing(false);
                  onPickRange(reading, range);
                }}
              >
                {range.name}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
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
