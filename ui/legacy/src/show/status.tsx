/**
 * The Status window: the show, the session, the engine and the outputs — S43.
 *
 * These were a strip across the bottom of the shell until this session. The
 * owner's skeleton (`design/skeleton/main-layout.pdf`) has no strip: the screen
 * is a header, a canvas, the command line and the programmer, and nothing else.
 * So the readings moved into a window an operator opens when they want them.
 *
 * # One reading did not move, and that is the rule this window is drawn around
 *
 * *Is the engine answering* must be true at a glance and **without anybody
 * having opened anything** — it is the one fact that says whether the rest of
 * the screen means anything. It stayed in the header as a light beside the
 * title (`App.tsx`). Everything here is a number an operator goes looking for.
 *
 * The same argument is why `Settings` is a window and this one is too: a fact
 * that can be closed is a fact an operator has to remember to open, and only one
 * of these facts is worth that.
 */

import type { ReactNode } from "react";

import type { JsonValue, ProgrammerState } from "../bindings";
import { selectionText, touchedCount } from "../desk/programmer";
import { countAt, numberAt, stringAt } from "../mirror/select";
import type { OutputSnapshot } from "../ipc/protocol";
import type { DeskState } from "../store/desk";
import { useDesk } from "../store/hooks";

const selectHealth = (state: DeskState) => state.health;
const selectOutputs = (state: DeskState) => state.outputs;
const selectUnsaved = (state: DeskState): boolean => state.unsavedChanges;

/** The whole window. */
export function StatusWindow({
  show,
  session,
  programmer,
}: {
  readonly show: JsonValue;
  readonly session: JsonValue;
  readonly programmer: ProgrammerState | null;
}) {
  const health = useDesk(selectHealth);
  const outputs = useDesk(selectOutputs);
  const unsaved = useDesk(selectUnsaved);
  return (
    <div className="status-window" data-testid="status-window">
      <Section title="Show">
        <Reading label="Fixtures" value={text(countAt(show, "/fixtures"))} testId="fixtures" />
        <Reading label="Groups" value={text(countAt(show, "/groups"))} testId="groups" />
        <Reading label="Seqs" value={text(countAt(show, "/sequences"))} testId="sequences" />
        <Reading label="Execs" value={text(countAt(show, "/executors"))} testId="executors" />
        <Reading label="Show" value={unsaved ? "unsaved changes" : "saved"} testId="dirty-flag" />
      </Section>
      <Section title="Session">
        <Reading
          label="View"
          value={text(numberAt(session, "/session/activeViewId"))}
          testId="active-view"
        />
        <Reading
          label="Windows"
          value={text(countAt(session, "/session/openWindows"))}
          testId="open-windows"
        />
        <Reading
          label="Page"
          value={text(numberAt(session, "/session/executorPage"))}
          testId="executor-page"
        />
        <Reading
          label="Bank"
          value={text(stringAt(session, "/session/encoderBank"))}
          testId="encoder-bank"
        />
        <Reading
          label="Param"
          value={text(numberAt(session, "/session/programmerParamIndex"))}
          testId="param-index"
        />
      </Section>
      {/*
        **What the programmer is holding** — the two readings the encoder bar
        carried before S43 turned it into the programmer band. The band has the
        room for encoders and not for counters, and these are exactly the kind of
        fact this window exists for: worth going to look at, not worth a
        permanent strip. *Which* fixtures rather than how many, because a
        selection of three is a thing an operator checks by number.
      */}
      <Section title="Programmer">
        <Reading label="Selected" value={selectionText(programmer)} testId="selection" />
        <Reading label="Values" value={String(touchedCount(programmer))} testId="touched" />
      </Section>
      <Section title="Engine">
        {health === null ? (
          <Reading label="Engine" value="—" testId="protocol" />
        ) : (
          <>
            <Reading label="Protocol" value={String(health.protocolVersion)} testId="protocol" />
            <Reading label="Tick" value={`${health.tickHz.toFixed(1)} Hz`} testId="tick-hz" />
            <Reading
              label="Missed"
              value={String(health.missedTicks)}
              testId="missed-ticks"
            />
          </>
        )}
      </Section>
      <Outputs outputs={outputs} />
    </div>
  );
}

/**
 * The outputs, with the empty state said out loud.
 *
 * A desk with no output configured is a desk sending no DMX, and that is worth a
 * sentence rather than a blank space — it is also the state a fresh installation
 * is in until somebody opens Settings.
 */
function Outputs({ outputs }: { readonly outputs: readonly OutputSnapshot[] | null }) {
  if (outputs === null || outputs.length === 0) {
    return (
      <Section title="Outputs">
        <p className="window-note" data-testid="no-outputs">
          No output is configured, so nothing is going down a cable. Settings → Outputs.
        </p>
      </Section>
    );
  }
  return (
    <Section title="Outputs">
      {outputs.map((output) => (
        <Reading
          key={output.id}
          label={`${String(output.id)} ${output.name}`}
          value={output.health}
          testId={`output-${String(output.id)}`}
        />
      ))}
    </Section>
  );
}

/** One titled group of readings. */
function Section({
  title,
  children,
}: {
  readonly title: string;
  readonly children: ReactNode;
}) {
  return (
    <section className="status-section">
      <h3>{title}</h3>
      <dl className="status-readings">{children}</dl>
    </section>
  );
}

/** One label and one value. */
function Reading({
  label,
  value,
  testId,
}: {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
}) {
  return (
    <div className="reading">
      <dt>{label}</dt>
      <dd data-testid={testId}>{value}</dd>
    </div>
  );
}

/** A reading that is not in the document reads as an em dash, not as zero. */
function text(value: number | string | null): string {
  return value === null ? "—" : String(value);
}
