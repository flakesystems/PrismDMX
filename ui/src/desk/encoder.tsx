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
 * # Every encoder here is one the selection has — S52
 *
 * The band used to draw a fixed table of knobs and write `—` under the ones
 * nothing selected had. It draws only what the fixtures have now
 * (`./programmer.ts`'s `bankParameters`), so there is no absent state left to
 * render and `encoder-absent` is gone with it. An encoder's name is its key's:
 * `Gobo` for the first of a kind and `Gobo 2` for the second, on a head that
 * has two.
 *
 * # And a channel with named ranges is picked from, not guessed at — S51, S52
 *
 * An Open Fixture Library channel can say *0–9 open, 10–19 gobo 1, 20–29 gobo
 * 2*. S51 read those (`prism_domain::AttributeRange`) and hung a small button
 * under the encoder to open the list. The owner's word on that button is that
 * it should not be one: **right-click the encoder** and the steps come up in a
 * window (`./rangepicker.tsx`), which is the same gesture the pools already use
 * for *manage this* and gives the band its row of height back.
 *
 * The name of the range the value is standing in stays — it is a **reading**,
 * not a control, so it sits inside the encoder under the percentage. An encoder
 * whose channel has no ranges has no window and no line, and right-clicking it
 * does nothing.
 */

import type { PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useState } from "react";

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
  onSteps,
}: {
  readonly reading: ParameterReading;
  readonly selected: boolean;
  readonly selectable: boolean;
  readonly onSelect: () => void;
  readonly onTurn: (reading: ParameterReading, delta: number) => void;
  /**
   * **S52.** Opens the steps window for this encoder.
   *
   * Called only where the channel has named ranges: an encoder with none has
   * nothing to show, and a window that opened empty would be a window an
   * operator learns to stop opening.
   */
  readonly onSteps: (reading: ParameterReading) => void;
}) {
  const begin = useEncoder(reading, onTurn);
  const hasSteps = reading.ranges.length > 0;
  // **The manufacturer's word where there is one** — S53. *Rotating Gobo*
  // rather than *Gobo 2*: the desk's own name is what an operator falls back
  // on, not what they are holding. The **key** is unchanged either way — see
  // `prism_domain::AttributeDef::label`.
  const shown = reading.name ?? reading.label;
  // **The test id is the key and not the attribute** — S52. A head with two
  // colour wheels draws two encoders, and a name that could not tell them apart
  // would be two elements answering to one id. The first of a kind keeps the
  // bare name, so every id written before a fixture could have two of a
  // parameter still names the same encoder.
  const id =
    reading.occurrence === 0
      ? reading.attribute
      : `${reading.attribute}-${String(reading.occurrence + 1)}`;
  return (
    <button
      type="button"
      className={`encoder${selected ? " encoder-selected" : ""}${
        reading.overriding ? " encoder-overriding" : ""
      }`}
      data-testid={`encoder-${id}`}
      data-selected={selected ? "yes" : "no"}
      data-overriding={reading.overriding ? "yes" : "no"}
      data-steps={hasSteps ? "yes" : "no"}
      data-index={reading.index}
      title={
        reading.overriding
          ? `${shown} — overriding: this value goes out whatever the playbacks say. Drag to change, Shift for fine${
              hasSteps ? ". Right-click for its steps" : ""
            }`
          : `${shown} — resting value; click to take it over. Drag to change, Shift for fine${
              hasSteps ? ". Right-click for its steps" : ""
            }`
      }
      aria-disabled={selectable ? undefined : true}
      onPointerDown={begin}
      onClick={onSelect}
      onContextMenu={(event) => {
        // A channel with no steps keeps the browser's own menu out of the way
        // all the same: this is a device screen, and a context menu over the
        // programmer band is never what was wanted.
        event.preventDefault();
        if (hasSteps) {
          onSteps(reading);
        }
      }}
    >
      <span className="encoder-name" title={shown}>
        {shown}
      </span>
      <span className="encoder-value" data-testid={`value-${id}`}>
        {valueText(reading)}
      </span>
      {/*
        **S51/S52.** The range the value is standing in, as a reading rather
        than a button — drawn only where the channel has ranges, because a line
        of empty space under every continuous encoder would take height off the
        canvas for nothing (`CLAUDE.md`'s device-screen rule).
      */}
      {hasSteps ? (
        <span className="encoder-step" data-testid={`range-${id}`}>
          {reading.range ?? "—"}
        </span>
      ) : null}
      <span className="encoder-foot">
        <span className="encoder-source" data-testid={`source-${id}`}>
          {sourceText(reading)}
        </span>
        <span className="encoder-count" data-testid={`count-${id}`}>
          {`${String(reading.held)}/${String(reading.available)}`}
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
