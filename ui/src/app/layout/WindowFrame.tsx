/**
 * One window on the canvas: a title bar and whatever `windows/registry.tsx`
 * says goes inside. Only the body scrolls — never the page.
 */

import type { CSSProperties } from "react";

import type { WindowInstance } from "../../core";
import { useSend, windowTitle } from "../../core";
import { windowContent } from "../windows/registry";

interface WindowFrameProps {
  readonly window: WindowInstance;
  readonly focused: boolean;
  readonly style: CSSProperties;
}

export function WindowFrame({ window, focused, style }: WindowFrameProps) {
  const send = useSend();
  const Content = windowContent(window.type);
  return (
    <article
      className="window"
      data-testid={`window-${window.type}`}
      data-focused={focused ? "yes" : "no"}
      style={style}
      onPointerDown={() => {
        if (!focused) {
          send({ t: "FocusWindow", instanceId: window.instanceId });
        }
      }}
    >
      <header className="window-title">
        <span>{windowTitle(window.type)}</span>
        <button
          type="button"
          className="key key-small"
          aria-label="Close window"
          onClick={() => send({ t: "CloseWindow", instanceId: window.instanceId })}
        >
          ×
        </button>
      </header>
      <div className="window-body">
        <Content window={window} />
      </div>
    </article>
  );
}
