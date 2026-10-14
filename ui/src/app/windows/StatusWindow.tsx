/**
 * Example window: how the engine is doing.
 *
 * Read-only readings straight out of the store — no command involved.
 */

import { useHealth, useOutputs, useShowFile, useUnsavedChanges } from "../../core";

export function StatusWindow() {
  const health = useHealth();
  const outputs = useOutputs() ?? [];
  const showFile = useShowFile();
  const unsaved = useUnsavedChanges();

  return (
    <dl className="facts">
      <dt>Show</dt>
      <dd>
        {showFile?.path ?? "—"} {unsaved ? <span className="warn">(unsaved)</span> : null}
      </dd>
      <dt>Engine</dt>
      <dd>
        {health === null ? "—" : `${health.tickHz.toFixed(1)} Hz, ${String(health.missedTicks)} missed`}
      </dd>
      <dt>Outputs</dt>
      <dd>{outputs.length === 0 ? "none" : outputs.map((output) => `${output.name} (${output.health})`).join(", ")}</dd>
    </dl>
  );
}
