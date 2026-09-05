/**
 * The programmer band — the owner's `design/skeleton/programmer.pdf`, built.
 *
 * ```text
 *   ┌───────────────┐ ┌─┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌──────────────┐
 *   │ Dimmer Position│ │>│ │    │ │    │ │    │ │    │ │ Selected Seq │
 *   │ Gobo   Color   │ │ │ │ En │ │ En │ │ En │ │ En │ ├──────────────┤
 *   │ Beam   Focus   │ │<│ │    │ │    │ │    │ │    │ │   Cue list   │
 *   │ Control        │ └─┘ └────┘ └────┘ └────┘ └────┘ │              │
 *   └───────────────┘                                   └──────────────┘
 * ```
 *
 * The drawing states arrangement and not detail, so what follows is what this
 * session decided and why. `ARCHITECTURE_SPEC.md` §4.6 carries the summary; this
 * is the reasoning.
 *
 * # The seven keys are the feature groups, in two columns
 *
 * `FEATURE_GROUP_VARIANTS`, generated from `prism_domain::FeatureGroup`, so the
 * key set cannot drift from the banks the jog wheel walks (S26's rule). The
 * drawing's own labels are those seven names, which is what identified the box.
 *
 * # The four boxes are the four encoders, and that is a measurement rather than
 * a reading of the picture
 *
 * `ENCODERS_PER_PAGE` has been four since S35, for the band it then had; the
 * drawing has four boxes. The `>` / `<` control beside them is therefore the
 * **page** stepper: it is what moves between sets of four.
 *
 * ## What that displaced, and where it went
 *
 * The old bar had *two* two-button controls — one for the page and one for the
 * highlighted parameter (`Zoom ▲▼` and `Zoom ◀▶` on the X-Touch). The drawing
 * has one. The parameter highlight is still reachable with a pointer, because
 * **clicking an encoder selects it** (it always has), and still reachable from
 * the desk, because `Zoom ◀▶` sends `SelectProgrammerParam` whatever this screen
 * draws. So nothing was lost; a redundant pair of arrows was.
 *
 * ## And the page is clamped here, which closes an S35 loose end
 *
 * S35 recorded that `prism_surface::binding::step_page` saturates at zero and
 * has no upper bound — it cannot have one, because `prism-core` deliberately
 * does not know how many parameters a bank has (S13) — so holding `Zoom ▼` on a
 * short bank runs the session's number past the end and the bar shows its last
 * page until the number is walked back. S35 named this session and named the
 * fix: *the bar sends a correcting `SetProgrammerPage` when it clamps*. That is
 * {@link useClampedPage} below.
 *
 * # The right-hand column is the selected sequence and its cues
 *
 * `Session::selectedSequence` (S39) and the cue list under it, with the marker on
 * the cue the playback in force is in. It is a **reading**: the Sequence Sheet is
 * where a cue list is stored into and edited. Two windows drawing the same list
 * is the ordinary case on a console — one under your hands, one on the canvas —
 * and the guard S39 recorded applies here too: the marker is drawn only where the
 * executor in force is playing *this* list, or it would point at a row of a list
 * nobody is on.
 *
 * # Nothing here is state this band holds
 *
 * The lit key is `encoderBank`. The values are the programmer. The page is
 * `programmerPage`. The list is the show. Every gesture is a command out and a
 * delta back, which is D3 and is why a second screen and the X-Touch agree with
 * this one without anything being kept in step.
 */

import { useEffect } from "react";

import type {
  AttributeRange,
  AttributeType,
  FeatureGroup,
  JsonValue,
  ProgrammerState,
} from "../bindings";
import { FEATURE_GROUP_VARIANTS } from "../bindings/variants";
import type { SequenceRow } from "../show/looks";
import { executorInForce, sequenceInForce, sequenceRow } from "../show/looks";
import { Encoder } from "./encoder";
import type { ParameterReading } from "./programmer";
import { bankReadings, encoderPage, selectionSize, touchedBanks } from "./programmer";
import { encoderBank, programmerPage, programmerParamIndex } from "./session";

/** What the band needs. */
export interface ProgrammerBandProps {
  /** The session document. */
  readonly session: JsonValue;
  /** The show document. */
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
  /**
   * Takes attributes over at the value they already have — S43.
   *
   * A click on an encoder takes that one; a right-click on a bank key takes
   * every attribute on that bank. Never called for an attribute that is already
   * overridden: see `App.tsx`'s `onTake` for why that matters.
   */
  readonly onTake: (attributes: readonly AttributeType[]) => void;
  /**
   * **B38.** Sets an attribute to the middle of one of its named ranges.
   *
   * The one absolute gesture in this band — see `App.tsx`'s `onPickRange`.
   */
  readonly onPickRange: (reading: ParameterReading, range: AttributeRange) => void;
  /** Writes and submits a line — the cue list's rows are list picks (§4.5). */
  readonly onLine: (line: string) => void;
}

/** The band. */
export function ProgrammerBand({
  session,
  show,
  programmer,
  onBank,
  onParam,
  onPage,
  onTurn,
  onTake,
  onPickRange,
  onLine,
}: ProgrammerBandProps) {
  const bank = encoderBank(session);
  const readings = bankReadings(programmer, show, bank);
  const selectedIndex = programmerParamIndex(session);
  const touched = touchedBanks(programmer, show);
  const paged = encoderPage(readings, programmerPage(session));
  useClampedPage(paged.page, programmerPage(session), onPage);

  return (
    <section className="progband" data-testid="programmer-band" aria-label="Programmer">
      <div className="progband-groups" data-testid="feature-groups">
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
            title={`${
              touched.includes(group)
                ? `${group} — the programmer is holding values on this bank`
                : group
            } · right-click to take the whole bank over`}
            onClick={() => {
              onBank(group);
            }}
            // **Right-click takes the whole bank over** — S43, the owner's
            // fourth point. Every attribute on it that is not already overridden
            // is written at the value it already has, so *this bank is mine now*
            // is one gesture rather than one per encoder. The browser's own menu
            // is not wanted on a console.
            onContextMenu={(event) => {
              event.preventDefault();
              onTake(
                bankReadings(programmer, show, group)
                  .filter((reading) => !reading.overriding && reading.available > 0)
                  .map((reading) => reading.attribute),
              );
            }}
          >
            {group}
          </button>
        ))}
      </div>

      {/*
        The drawing's `>` over `<`: the page stepper, which is `Zoom ▲▼` on the
        console. Both send `SetProgrammerPage` and neither moves anything until
        the delta comes back, so paging from either hand is the same act (S35).
        A bank that fits on one page disables both, because there is nowhere to
        go — and says so on the button rather than by being mysteriously inert.
      */}
      <div className="progband-pager" data-testid="page-steps">
        <button
          type="button"
          className="page-step"
          data-testid="encoder-page-up"
          aria-label="Previous parameter page"
          title="Previous page of encoders"
          disabled={paged.page === 0}
          onClick={() => {
            onPage(paged.page - 1);
          }}
        >
          ▲
        </button>
        <span className="progband-page-number" data-testid="programmer-page">
          {paged.page + 1}/{paged.pages}
        </span>
        <button
          type="button"
          className="page-step"
          data-testid="encoder-page-down"
          aria-label="Next parameter page"
          title="Next page of encoders"
          disabled={paged.page >= paged.pages - 1}
          onClick={() => {
            onPage(paged.page + 1);
          }}
        >
          ▼
        </button>
      </div>

      <div className="progband-encoders" data-testid="encoders">
        {paged.readings.length === 0 ? (
          <p className="window-note" data-testid="no-parameters">
            {selectionSize(programmer) === 0
              ? "Nothing is selected. Pick a fixture, or type Fixture 1."
              : `Nothing selected has any ${bank} parameter.`}
          </p>
        ) : (
          paged.readings.map((reading) => (
            <Encoder
              key={reading.attribute}
              reading={reading}
              selected={reading.index === selectedIndex}
              selectable={selectionSize(programmer) > 0}
              onSelect={() => {
                step(selectedIndex, reading.index, onParam);
                // **A click takes it over.** Only when it is not already
                // overridden — otherwise a click meant to point the jog wheel at
                // an encoder would rewrite a preset's value as a manual one.
                if (!reading.overriding && reading.available > 0) {
                  onTake([reading.attribute]);
                }
              }}
              onTurn={onTurn}
              onPickRange={onPickRange}
            />
          ))
        )}
      </div>

      <SelectedSequence show={show} session={session} onLine={onLine} />
    </section>
  );
}

/**
 * The selected cue list, and where the playback is in it.
 *
 * A reading rather than an editor — see the module documentation. Each row is a
 * **list pick** (§4.5): clicking one writes the line that names that cue and
 * submits it, which is exactly the line an operator could have typed.
 */
function SelectedSequence({
  show,
  session,
  onLine,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
  readonly onLine: (line: string) => void;
}) {
  const sequenceId = sequenceInForce(session);
  const sequence = sequenceRow(show, sequenceId);
  const executor = executorInForce(session, show);
  // S39's guard: the marker is drawn only where the playback in force is on
  // *this* list, or it would point at a row of a list nobody is on.
  const playing = executor.sequenceId === sequenceId ? executor.currentCueIndex : null;

  return (
    <div className="progband-sequence" data-testid="selected-sequence">
      <h3 data-testid="selected-sequence-name">
        {sequence === null ? "No sequence selected" : `${String(sequence.id)} ${sequence.name}`}
      </h3>
      {sequence === null ? (
        <p className="window-note" data-testid="no-selected-sequence">
          Type <code>Sequence 1</code>, or pick one in the Sequence Sheet.
        </p>
      ) : (
        <CueList sequence={sequence} playing={playing} onLine={onLine} />
      )}
    </div>
  );
}

/** The cues of one list, in the order the show holds them. */
function CueList({
  sequence,
  playing,
  onLine,
}: {
  readonly sequence: SequenceRow;
  readonly playing: number | null;
  readonly onLine: (line: string) => void;
}) {
  const cues = sequence.cues;
  if (cues.length === 0) {
    return (
      <p className="window-note" data-testid="empty-cue-list">
        This list has no cues yet. <code>Store Cue 1</code> puts the programmer into one.
      </p>
    );
  }
  return (
    <div className="progband-cues" data-testid="cue-list">
      <table className="sheet">
        <tbody>
          {cues.map((cue, index) => (
            <tr
              key={cue.number}
              className={index === playing ? "row-playing" : ""}
              data-testid={`band-cue-${cue.number}`}
              data-playing={index === playing ? "yes" : "no"}
            >
              <td>
                <button
                  type="button"
                  className="linkish"
                  title={`Go to cue ${cue.number}`}
                  data-testid={`band-goto-${cue.number}`}
                  onClick={() => {
                    onLine(`Goto Sequence ${String(sequence.id)} Cue ${cue.number}`);
                  }}
                >
                  {cue.number}
                </button>
              </td>
              <td>{cue.name === "" ? "—" : cue.name}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/**
 * Puts the session's page back inside the bank when it has run past the end.
 *
 * **S35's loose end, closed here.** The console's `Zoom ▼` steps the page with
 * `prism_surface::binding::step_page`, which saturates at zero and has no upper
 * bound — it cannot have one, because the daemon deliberately does not know how
 * many parameters a bank has (S13). So the number can be run past the last page
 * of a short bank, and every screen then shows its last page while the session
 * says something else.
 *
 * The correction is a command rather than a local clamp, which is the whole
 * point: **D3** says the daemon holds the number, so a screen that quietly drew
 * a different page would be a second answer. Sent once, on the render that
 * notices, and only when the two disagree — so it converges in one round trip
 * and then says nothing.
 */
function useClampedPage(
  clamped: number,
  asked: number,
  onPage: (page: number) => void,
): void {
  useEffect(() => {
    if (clamped !== asked) {
      onPage(clamped);
    }
  }, [clamped, asked, onPage]);
}

/**
 * Moves the highlight from `from` to `to`, one command per step.
 *
 * The protocol has `SelectProgrammerParam { direction }` and no absolute form —
 * because the console has no absolute form either: `Zoom ◀▶` steps. Rather than
 * add a session command for a click, the band composes the one that exists. A
 * bank has at most six parameters, so this is at most five commands and the
 * daemon applies them in order.
 */
function step(from: number, to: number, onParam: (direction: "Prev" | "Next") => void): void {
  const direction = to > from ? "Next" : "Prev";
  for (let at = 0; at < Math.abs(to - from); at += 1) {
    onParam(direction);
  }
}
