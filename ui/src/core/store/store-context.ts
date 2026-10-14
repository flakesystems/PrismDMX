/**
 * The context object itself, in a module of its own.
 *
 * Split from the provider and the hooks so that neither file exports a mixture
 * of components and other things — which is what keeps fast refresh working
 * during development, and is the only reason this is three files.
 */

import { createContext } from "react";

import type { DeskStore } from "./desk";

/** The store this subtree is bound to, or `null` outside a provider. */
export const StoreContext = createContext<DeskStore | null>(null);
