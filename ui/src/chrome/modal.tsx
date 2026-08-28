/**
 * The modal panel — one implementation, wherever a choice is too big for a row.
 *
 * # Two of them, and why the desk did not want a third copy
 *
 * S43 built the window chooser (`canvas/picker.tsx`) as a backdrop with a panel
 * on it, and the owner's rebuild asks for a second: the fixture library, which
 * was a dropdown under a search box in the patch form and, in the owner's words,
 * *sehr unübersichtlich* — a single column of run-together text over the table
 * it was covering. A library of some two thousand profiles wants columns and
 * room, and the way to give it room is to stop drawing it inside a form field.
 *
 * So the backdrop, the escape key, the click-away, the focus and the heading are
 * here, and both callers bring only what is inside.
 *
 * # It is not a modal in the sense `CLAUDE.md` objects to
 *
 * The objection is to a console that stops being a console while a dialogue is
 * up. This does not: the show carries on behind it, the DMX thread never sees
 * it, every other client is untouched, and a Go from the X-Touch does not wait
 * on it. Those are the same three properties the command line's prompt bar has
 * (S40). What it does take is the keyboard, because it is a chooser and Escape
 * has to close it.
 *
 * It sits over the **canvas** and not over the whole desk, so the command line
 * and the programmer band stay reachable behind it — which is also why the
 * backdrop is `position: absolute` in a positioned ancestor rather than fixed.
 */

import { useCallback, useEffect, useRef } from "react";

/** What a modal needs. */
export interface ModalProps {
  /** The heading, and the screen-reader name. */
  readonly title: string;
  /** The test id — the contract this panel is found by. */
  readonly testId: string;
  /** Closes it without choosing anything. */
  readonly onClose: () => void;
  /**
   * How much room it takes.
   *
   * `wide` is for a panel with columns in it — the library, and the window
   * chooser now that its keys are a grid rather than a list.
   */
  readonly size?: "normal" | "wide";
  /** What is under the heading. */
  readonly children: React.ReactNode;
  /** What is along the bottom, beside the Cancel key. */
  readonly footer?: React.ReactNode;
}

/** The panel. */
export function Modal({ title, testId, onClose, size = "normal", children, footer }: ModalProps) {
  const panel = useRef<HTMLDivElement>(null);

  // Escape closes it, wherever the focus is. The listener is on the document
  // because the panel may not have the focus yet on the frame it appears.
  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    globalThis.addEventListener("keydown", onKey);
    return () => {
      globalThis.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  // The first control takes the focus, so the whole panel is reachable with the
  // keyboard alone — S43's *a full pass with the keyboard reaches every
  // control*, and what makes a keyboard shortcut for opening it worth having.
  useEffect(() => {
    const first = panel.current?.querySelector<HTMLElement>("input, button, select, textarea");
    first?.focus();
  }, []);

  const onBackdrop = useCallback(
    (event: React.MouseEvent) => {
      if (event.target === event.currentTarget) {
        onClose();
      }
    },
    [onClose],
  );

  return (
    <div
      className="modal-backdrop"
      data-testid={testId}
      onMouseDown={onBackdrop}
      onContextMenu={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <div
        className={size === "wide" ? "modal modal-wide" : "modal"}
        ref={panel}
        role="dialog"
        aria-label={title}
      >
        <h2>{title}</h2>
        <div className="modal-body">{children}</div>
        <div className="modal-foot">
          {footer}
          <button
            type="button"
            className="modal-cancel"
            data-testid={`${testId}-cancel`}
            onClick={onClose}
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
