/** A window kind that has not been built yet. */

import type { WindowProps } from "./registry";

export function Placeholder({ window }: WindowProps) {
  return (
    <p className="dim">
      <code>{window.type}</code> — not built yet. See <code>src/app/windows/registry.tsx</code>.
    </p>
  );
}
