/**
 * Reading the store from a component.
 *
 * **Selectors must be stable and pure.** A selector defined at module scope, or
 * one built with `useCallback`, is compared by identity against the last one and
 * its result is cached per state object — so a component re-renders when *its*
 * slice changed and not merely when something did. A selector that built a new
 * object every call would defeat that; there is a test that says so.
 */

import { use, useCallback, useRef, useSyncExternalStore } from "react";

import type { Command } from "../bindings";
import type { DeskState, DeskStore } from "./desk";
import { StoreContext } from "./store-context";

/** The store this subtree is bound to. */
export function useDeskStore(): DeskStore {
  const store = use(StoreContext);
  if (store === null) {
    throw new Error("useDeskStore was called outside a <DeskProvider>");
  }
  return store;
}

/** One slice of the state, re-rendering only when that slice changes. */
export function useDesk<T>(selector: (state: DeskState) => T): T {
  const store = useDeskStore();
  const cache = useRef<{ state: DeskState; selector: (state: DeskState) => T; value: T } | null>(
    null,
  );

  const read = useCallback(() => {
    const state = store.getState();
    const held = cache.current;
    if (held === null || held.state !== state || held.selector !== selector) {
      cache.current = { state, selector, value: selector(state) };
      return cache.current.value;
    }
    return held.value;
  }, [store, selector]);

  return useSyncExternalStore(store.subscribe, read, read);
}

/** The function that sends a command to the daemon. */
export function useSend(): (command: Command) => number | null {
  return useDeskStore().send;
}
