/**
 * The React binding: a component re-renders when *its* slice changed.
 *
 * That is the whole reason a selector exists, and it is worth a test because
 * the alternative — every component re-rendering on every delta — would work
 * perfectly and be unusable at 8 executors × 512 channels.
 */

import { render, screen } from "@testing-library/react";
import { act } from "react";
import { describe, expect, it } from "vitest";

import { snapshot } from "../testing/fake-daemon";
import { DeskProvider } from "./context";
import { useDesk, useDeskStore, useSend } from "./hooks";
import type { DeskState } from "./desk";
import { DeskStore } from "./desk";

const selectPage = (state: DeskState): number => {
  const session = state.documents?.session;
  if (session === null || session === undefined || typeof session !== "object") {
    return -1;
  }
  if (Array.isArray(session)) {
    return -1;
  }
  const inner = session["session"];
  if (inner === undefined || typeof inner !== "object" || inner === null || Array.isArray(inner)) {
    return -1;
  }
  const page = inner["executorPage"];
  return typeof page === "number" ? page : -1;
};

const selectNoticeCount = (state: DeskState): number => state.notices.length;

describe("the provider", () => {
  it("refuses to work outside one, rather than inventing a store", () => {
    function Orphan() {
      useDeskStore();
      return null;
    }
    // React logs the error itself; what matters is that it is thrown rather
    // than a second store appearing from nowhere.
    expect(() => render(<Orphan />)).toThrow(/outside a <DeskProvider>/);
  });

  it("sends through the store it was given", () => {
    const store = new DeskStore();
    const sent: string[] = [];
    store.attach((command) => {
      sent.push(command.t);
      return 1;
    });

    function Sender() {
      const send = useSend();
      return (
        <button type="button" onClick={() => send({ t: "SaveShow" })}>
          save
        </button>
      );
    }
    render(
      <DeskProvider store={store}>
        <Sender />
      </DeskProvider>,
    );
    act(() => {
      screen.getByText("save").click();
    });
    expect(sent).toEqual(["SaveShow"]);
  });
});

describe("selectors", () => {
  it("re-render only what changed", () => {
    const store = new DeskStore();
    let pageRenders = 0;
    let noticeRenders = 0;

    function Page() {
      pageRenders += 1;
      return <span data-testid="page">{useDesk(selectPage)}</span>;
    }
    function Notices() {
      noticeRenders += 1;
      return <span data-testid="count">{useDesk(selectNoticeCount)}</span>;
    }

    render(
      <DeskProvider store={store}>
        <Page />
        <Notices />
      </DeskProvider>,
    );
    const pageBefore = pageRenders;
    const noticeBefore = noticeRenders;

    act(() => {
      store.applySnapshot(snapshot());
    });
    expect(screen.getByTestId("page").textContent).toBe("3");
    expect(pageRenders).toBeGreaterThan(pageBefore);

    // A notice changes the notice count and nothing else. The page component
    // is subscribed to the same store and must not redraw for it.
    const pageAfterSnapshot = pageRenders;
    act(() => {
      store.notice("Info", "the show was saved");
    });
    expect(screen.getByTestId("count").textContent).toBe("1");
    expect(noticeRenders).toBeGreaterThan(noticeBefore);
    expect(pageRenders).toBe(pageAfterSnapshot);
  });

  it("unsubscribes when the component goes", () => {
    const store = new DeskStore();
    function Page() {
      return <span>{useDesk(selectPage)}</span>;
    }
    const view = render(
      <DeskProvider store={store}>
        <Page />
      </DeskProvider>,
    );
    view.unmount();
    // Nothing listening, and nothing throwing.
    act(() => {
      store.applySnapshot(snapshot());
    });
    expect(store.getState().documents).not.toBeNull();
  });
});
