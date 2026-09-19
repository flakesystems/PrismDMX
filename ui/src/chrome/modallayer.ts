import { createContext } from "react";

/**
 * The element a modal opened **inside a window** is drawn into — the canvas
 * (S57).
 *
 * A modal's backdrop is `position: absolute` in its nearest positioned
 * ancestor, which is what keeps it over the canvas and off the command line.
 * Opened from a component the canvas draws directly — the window chooser — that
 * ancestor *is* the canvas. Opened from inside a window, it was the **window**:
 * the patch editor came up the size of a Patch window and its list had no room
 * to be seen. So the canvas provides itself here, and a modal inside it is
 * portalled up to it. React events still travel the component tree, so the
 * window around it still hears a click and takes the focus.
 */
export const ModalLayer = createContext<HTMLElement | null>(null);
