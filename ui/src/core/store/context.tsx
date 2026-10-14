/**
 * The store, as React sees it.
 *
 * `useSyncExternalStore` is the whole of the binding: the store is an ordinary
 * object that lives outside React, React subscribes to it, and a selector
 * decides what a component actually depends on. That is the same contract a
 * store library would give, without one — see `./desk.ts` for why there is not
 * one.
 */

import type { ReactNode } from "react";

import type { DeskStore } from "./desk";
import { StoreContext } from "./store-context";

/** Makes one store available to everything below it. */
export function DeskProvider({
  store,
  children,
}: {
  readonly store: DeskStore;
  readonly children: ReactNode;
}) {
  return <StoreContext value={store}>{children}</StoreContext>;
}
