/**
 * A desk and its console, for a test that renders one window.
 *
 * Since S40 every key on the screen writes into the command line rather than
 * sending a command (`ARCHITECTURE_SPEC.md` §4.5), so a component under test
 * needs the shell above it as well as the store. This is those two providers in
 * one, so a test says what it is about rather than what it has to be wrapped in.
 *
 * Scenery, not a fixture: `src/testing/` is excluded from coverage for exactly
 * this reason (S28's note about `show-recording.ts`).
 */

import type { ReactNode } from "react";

import type { JsonValue } from "../bindings";
import { ConsoleProvider } from "../desk/shell";
import { DeskProvider } from "../store/context";
import type { DeskStore } from "../store/desk";

/** The two providers a window needs, in the order it needs them. */
export function Shell({
  store,
  session,
  show,
  children,
}: {
  readonly store: DeskStore;
  /** The session document, or nothing at all for a window that reads none. */
  readonly session?: JsonValue;
  /** The show document, for the console's *is this already there* question. */
  readonly show?: JsonValue;
  readonly children: ReactNode;
}) {
  return (
    <DeskProvider store={store}>
      <ConsoleProvider session={session ?? null} show={show ?? null}>
        {children}
      </ConsoleProvider>
    </DeskProvider>
  );
}
