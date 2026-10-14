/**
 * The bottom band: the encoder banks, what is selected, what is touched.
 *
 * The lit bank is the session's `encoderBank`, so pressing one here lights it
 * on the X-Touch too. The old band also drew the four encoders with their
 * values (`ui/legacy/src/desk/programmerband.tsx`); the helpers it used —
 * `bankReadings`, `encoderPage`, `valueText` — are exported from `core`.
 */

import {
  FEATURE_GROUP_VARIANTS,
  selectionSize,
  selectionText,
  touchedCount,
  useProgrammer,
  useSend,
  useSession,
} from "../../core";

export function ProgrammerBand() {
  const programmer = useProgrammer();
  const session = useSession();
  const send = useSend();
  const bank = session?.session.encoderBank ?? null;

  return (
    <section className="band programmer" data-testid="programmer">
      <nav className="banks" aria-label="Encoder banks">
        {FEATURE_GROUP_VARIANTS.map((group) => (
          <button
            key={group}
            type="button"
            className="key"
            aria-pressed={group === bank}
            onClick={() => send({ t: "SetEncoderBank", group })}
          >
            {group}
          </button>
        ))}
      </nav>
      <dl className="readings">
        <div>
          <dt>Selection ({selectionSize(programmer)})</dt>
          <dd data-testid="selection">{selectionText(programmer)}</dd>
        </div>
        <div>
          <dt>Touched values</dt>
          <dd>{touchedCount(programmer)}</dd>
        </div>
      </dl>
    </section>
  );
}
