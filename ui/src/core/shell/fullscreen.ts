/**
 * Full screen — **punch-list entry B42**, and which of the two mechanisms does
 * it.
 *
 * # Full screen belongs to the window, and the page is what asks for it
 *
 * There are two ways to make this interface fill a screen and they are not the
 * same thing:
 *
 * - **The window.** Tauri's `set_fullscreen` asks the window manager, which
 *   takes the title bar with it. `CLAUDE.md` asks for a device screen and a
 *   title bar is the last thing on this one that is not one, so inside the
 *   shell this is the mechanism — `prism_app::shell::set_fullscreen`.
 * - **The document.** A browser's Fullscreen API makes the *page* fill the
 *   screen inside whatever frame its host gives it. It is all a browser has.
 *
 * The decision is: **the shell uses the window and a browser uses the
 * document**, and neither build gets a key that does nothing. That is the whole
 * reason this file exists rather than a line in a key handler — a key bound to
 * one mechanism would be dead in the other host, and a key that is dead on one
 * of two paths is worse than no key (S37's *held by a flag* rule, and the same
 * argument `bridge.ts` makes about a *Browse…* button in a browser).
 *
 * The Web Remote (S31) is a browser and the end-to-end suite is a browser, so
 * the second path is not a courtesy: it is where this is exercised.
 *
 * # Nothing here remembers anything
 *
 * There is no stored *are we full screen* flag. Both hosts are asked, every
 * time: the shell answers with the state the window actually reached, and the
 * browser is read off `document.fullscreenElement`. A remembered flag would go
 * out of step the first time an operator pressed Escape, which every browser
 * treats as *leave full screen* without telling the page that asked.
 */

import { logger } from "../log/logger";

const log = logger("fullscreen");

/** The half of Tauri this file uses, so a test can supply one. */
interface Invoker {
  (command: string, args?: Record<string, unknown>): Promise<unknown>;
}

/** What Tauri puts on `window` when a page is running inside a shell. */
interface ShellWindow {
  __TAURI_INTERNALS__?: { invoke?: Invoker };
}

/**
 * The invoker this page has, or `null` in a browser.
 *
 * A copy of `bridge.ts`'s, deliberately: that file is *what the interface can
 * ask the shell for a path with*, and full screen is not a path. Sharing four
 * lines would have made one module import the other for no reason other than
 * that the four lines look alike.
 */
function invoker(): Invoker | null {
  if (typeof window === "undefined") {
    return null;
  }
  const internals = (window as unknown as ShellWindow).__TAURI_INTERNALS__;
  return typeof internals?.invoke === "function" ? internals.invoke : null;
}

/**
 * Whether this interface is full screen **now**.
 *
 * Read rather than remembered — see the module documentation. In the shell the
 * document is never full screen, because the shell moves the window instead, so
 * the reading there comes back from the last call to {@link setFullscreen} and
 * this answers what the document knows: `false`. That is why the caller keeps
 * the shell's answer and this is used only by the browser path.
 */
export function documentIsFullscreen(): boolean {
  // `Boolean` rather than a comparison with `null`: a host that implements none
  // of this leaves the property `undefined`, and `undefined !== null` is `true`
  // — which would have this answer *yes, full screen* on exactly the browsers
  // that cannot do it at all.
  return typeof document !== "undefined" && Boolean(document.fullscreenElement);
}

/**
 * Puts the interface into full screen, or takes it out, and answers with the
 * state that was actually reached.
 *
 * `false` is the honest answer to every way of not getting there: a browser
 * that refused the request, a shell that has no window, an API that is not
 * implemented. The caller draws what came back rather than what it asked for,
 * which is the same rule the autostart switch follows — a control that reports
 * its own intention is a control that displays a lie.
 */
export async function setFullscreen(on: boolean): Promise<boolean> {
  const invoke = invoker();
  if (invoke !== null) {
    try {
      return (await invoke("set_fullscreen", { on })) === true;
    } catch (error) {
      log.warn("the shell could not change the window", { on, error: String(error) });
      return false;
    }
  }
  if (typeof document === "undefined") {
    return false;
  }
  try {
    if (on) {
      // The **document element**, not the desk `<main>`: a full-screen element
      // is painted on the browser's own black backdrop, and an element with a
      // margin around it would show that backdrop as a frame.
      //
      // Reached through the property rather than called outright, because a
      // browser that has neither would otherwise throw a `TypeError` out of a
      // key handler rather than answer *no*.
      await document.documentElement.requestFullscreen?.();
    } else if (documentIsFullscreen()) {
      await document.exitFullscreen?.();
    }
  } catch (error) {
    // A browser refuses this outside a user gesture, and some refuse it
    // outright. Neither is a fault of this desk's, and neither may throw into a
    // key handler.
    log.warn("this browser would not change the screen", { on, error: String(error) });
  }
  return documentIsFullscreen();
}

/**
 * Whether a keyboard event is one of B42's two ways of asking.
 *
 * **`F11` and `Alt` + `Enter`**, which are the two the entry names and the two
 * every program on Windows answers to. Written as a predicate rather than as a
 * condition inside a handler so that the pair is asserted in one place: a key
 * that worked one way and not the other would be exactly the half-fix the entry
 * is about.
 *
 * A modifier the operator did not ask for disqualifies the press — `Ctrl` +
 * `F11` belongs to the browser, and `Ctrl` + `Alt` + `Enter` is not this. And a
 * press that is already being handled by something else (`defaultPrevented`) is
 * left alone.
 */
export function isFullscreenKey(event: {
  readonly key: string;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
  readonly defaultPrevented?: boolean;
}): boolean {
  if (event.defaultPrevented === true || event.ctrlKey || event.metaKey || event.shiftKey) {
    return false;
  }
  if (event.key === "F11") {
    return !event.altKey;
  }
  return event.key === "Enter" && event.altKey;
}
