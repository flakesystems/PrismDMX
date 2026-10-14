/**
 * What is shown while there is no daemon.
 *
 * Deliberately not a greyed-out copy of the last state: the store dropped the
 * documents when the connection went, because a stale fader value in a room
 * running a show is worse than no value.
 */

import { statusText, useConnectionStatus } from "../../core";

export function NotConnected() {
  const status = useConnectionStatus();
  return (
    <section className="not-connected" data-testid="not-connected">
      <p className="not-connected-title">No engine</p>
      <p>{statusText(status)}</p>
      <p className="dim">
        Start it with <code>npm run daemon</code>, or everything at once with{" "}
        <code>npm run dev:all</code>.
      </p>
    </section>
  );
}
