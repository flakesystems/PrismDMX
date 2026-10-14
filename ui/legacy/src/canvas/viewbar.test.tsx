/**
 * The View Selector Bar's own edges.
 *
 * `session.test.tsx` drives it through a whole interface, which is where the
 * ordinary behaviour is asserted. This is the three cases that need a session
 * document a daemon would not ordinarily produce.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import type { JsonValue } from "../bindings";
import { nullSink, setLogSink } from "../log/logger";
import { ViewBar } from "./viewbar";

/** A bar over one session document, and what it asked for. */
function bar(session: JsonValue) {
  const selected: number[] = [];
  const stored: { id: number; name: string }[] = [];
  const renamed: { id: number; name: string }[] = [];
  const deleted: number[] = [];
  const moved: { id: number; to: number }[] = [];
  const made: { id: number; name: string }[] = [];
  render(
    <ViewBar
      session={session}
      onSelectView={(viewId) => selected.push(viewId)}
      onStoreView={(viewId, name) => stored.push({ id: viewId, name })}
      onRenameView={(viewId, name) => renamed.push({ id: viewId, name })}
      onDeleteView={(viewId) => deleted.push(viewId)}
      onMoveView={(viewId, toViewId) => moved.push({ id: viewId, to: toViewId })}
      onNewView={(viewId, name) => made.push({ id: viewId, name })}
    />,
  );
  return { selected, stored, renamed, deleted, moved, made };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("the view bar", () => {
  it("stores nothing when the session has no active view to store into", () => {
    // Only a document that has lost `activeViewId` — which `prism-core` cannot
    // produce — reaches this. Pressing Store then does nothing, rather than
    // sending a command naming a view number the interface made up.
    const { stored } = bar({ session: {}, views: {} });
    fireEvent.click(screen.getByTestId("store-view"));
    expect(stored).toEqual([]);
  });

  it("names a view the session has not named after its number", () => {
    const { stored } = bar({ session: { activeViewId: 4 }, views: { "4": { id: 4 } } });
    fireEvent.click(screen.getByTestId("store-view"));
    expect(stored).toEqual([{ id: 4, name: "View 4" }]);
  });

  it("stores into the active view even when no view has been stored at all", () => {
    const { stored } = bar({ session: { activeViewId: 1 }, views: {} });
    fireEvent.click(screen.getByTestId("store-view"));
    expect(stored).toEqual([{ id: 1, name: "View 1" }]);
  });

  /**
   * **B11.** *New* used to send `StoreView`, so a new view arrived carrying the
   * windows of the one being left. The two buttons send two different commands
   * now, and this is what says so: pressing New must not store anything.
   */
  it("makes an empty view rather than storing the canvas as one", () => {
    const { stored, made } = bar({ session: { activeViewId: 1 }, views: {} });
    fireEvent.click(screen.getByTestId("new-view"));
    expect(made).toEqual([{ id: 1, name: "View 1" }]);
    expect(stored).toEqual([]);
  });

  /**
   * **B9.** The dropdown is gone, and gone rather than hidden: a window is
   * opened from the chooser now (`canvas/picker.tsx`), which a right-click, a
   * keyboard shortcut or an X-Touch key opens. The bar is about views.
   */
  it("has no window picker in it at all", () => {
    bar({ session: { activeViewId: 1 }, views: {} });
    expect(screen.queryByTestId("open-window")).toBeNull();
  });
});
