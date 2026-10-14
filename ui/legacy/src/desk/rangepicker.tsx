/**
 * The steps window — what a channel's named ranges are, and picking one.
 *
 * # What a step is, and where it comes from
 *
 * An Open Fixture Library channel is a list of **capabilities**: *0–7 closed,
 * 8–134 dimmer, 135–239 strobe*, or *0–9 open, 10–19 gobo 1, 20–29 gobo 2*.
 * S51 read them into `prism_domain::AttributeRange`, and the profile a show
 * carries is where they come from — not a table in this file. **Nothing here
 * invents a step.** A gobo wheel offers exactly the slots its own profile
 * names, in the profile's order, and a channel the file describes as one thing
 * over its whole travel has no steps and no window.
 *
 * Since S52 the names are the ones the manufacturer wrote: a wheel slot is
 * looked up in the fixture's `wheels` block rather than read off the
 * capability, so *Gobo 3* stops being *Slot 3*.
 *
 * # Why it is a window and not the little button it was
 *
 * S51 hung a button under each encoder that dropped a list down. The owner's
 * word is that a right-click on the attribute should bring the steps up
 * instead — the same gesture the pools use for *manage this*
 * (`chrome/menu.tsx`) — and that the button should go. A gobo wheel with twenty
 * slots wants room and columns, which a dropdown inside a fixed-height band
 * cannot give it, and the band gets a row of height back.
 *
 * So it is `chrome/modal.tsx`, the panel the window chooser and the fixture
 * library already use. It sits over the canvas: the show carries on behind it,
 * the DMX thread never sees it, and a Go from the X-Touch does not wait on it.
 *
 * # Picking one writes the middle of it
 *
 * `AttributeRange::middle`, which is the furthest any single value can be from
 * both edges — a fixture whose thresholds are a step out from what its manual
 * says still lands on the slot that was asked for. Nothing else changes: a gobo
 * is still one number, a cue still stores that number, and the engine has never
 * heard of a range.
 */

import type { AttributeRange } from "../bindings";
import { Modal } from "../chrome/modal";
import { percentOfLevel } from "./level";
import type { ParameterReading } from "./programmer";

/** What the steps window needs. */
export interface RangePickerProps {
  /** The encoder whose steps these are. */
  readonly reading: ParameterReading;
  /** Sets the attribute to the middle of one range. */
  readonly onPick: (reading: ParameterReading, range: AttributeRange) => void;
  /** Closes it without picking anything. */
  readonly onClose: () => void;
}

/** The window. */
export function RangePicker({ reading, onPick, onClose }: RangePickerProps) {
  return (
    <Modal
      title={`${reading.name ?? reading.label} — steps`}
      testId="range-picker"
      size="wide"
      onClose={onClose}
    >
      <p className="range-picker-note">
        The steps this fixture&apos;s own profile names. Picking one sets{" "}
        {reading.name ?? reading.label} to the middle of it.
      </p>
      <ul
        className="range-picker"
        data-testid={`ranges-${reading.attribute}${
          reading.occurrence === 0 ? "" : `-${String(reading.occurrence + 1)}`
        }`}
      >
        {reading.ranges.map((range) => {
          const here = range.name === reading.range;
          return (
            <li key={`${range.name}-${String(range.from)}`}>
              <button
                type="button"
                className={`range-step${here ? " range-here" : ""}`}
                data-here={here ? "yes" : "no"}
                data-testid={`range-${reading.attribute}-${range.name}`}
                onClick={() => {
                  onPick(reading, range);
                  onClose();
                }}
              >
                <span className="range-step-name">{range.name}</span>
                {/*
                  Where the step sits on the channel, as a percentage — the same
                  number the encoder above reads, so an operator can see which
                  way to turn if they would rather turn than pick.
                */}
                <span className="range-step-at">
                  {`${String(percentOfLevel(range.from))}–${String(percentOfLevel(range.to))} %`}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </Modal>
  );
}
