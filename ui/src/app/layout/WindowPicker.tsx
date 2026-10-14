/**
 * The window chooser: one key per kind of window.
 *
 * Whether it is open is **session state** (`session.windowPicker`), not local
 * state — a key on the X-Touch opens it too, and the daemon that resolves that
 * key has no screen. So the `+` key asks the daemon to open it, and this draws
 * it whenever the session says it is open.
 */

import { WINDOW_TYPE_VARIANTS, useSend, useSession, windowTitle } from "../../core";

/** The `+` key in the header. */
export function WindowPickerKey() {
  const session = useSession();
  const send = useSend();
  const open = session?.session.windowPicker ?? false;
  return (
    <button
      type="button"
      className="key"
      aria-pressed={open}
      title="Open a window"
      onClick={() => send({ t: "SetWindowPicker", open: !open })}
    >
      + Window
    </button>
  );
}

/** The chooser itself, drawn over the canvas while the session says so. */
export function WindowPicker() {
  const session = useSession();
  const send = useSend();
  if (session === null || !session.session.windowPicker) {
    return null;
  }
  return (
    <div className="picker" data-testid="window-picker">
      {WINDOW_TYPE_VARIANTS.map((type) => (
        <button
          key={type}
          type="button"
          className="key"
          onClick={() => {
            send({ t: "OpenWindow", window: type });
            send({ t: "SetWindowPicker", open: false });
          }}
        >
          {windowTitle(type)}
        </button>
      ))}
    </div>
  );
}
