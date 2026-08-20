/**
 * What every test run starts from.
 *
 * Three things. Two of them are about not letting one test see another's
 * leavings: the rendered document is torn down after each test — Testing
 * Library only registers that by itself when the globals are on, and they are
 * not — and the logger writes nowhere, because a suite that printed every
 * reconnection to the console would bury its own failures.
 *
 * The third is React's flag for *this is a test*. Without it, `act` from `react`
 * warns that the environment does not support it and flushing is left to
 * chance — which matters here more than usual: S24's render counter is only
 * worth anything if every update React had queued has actually happened by the
 * time the count is read.
 *
 * And one shim, which is a gap in `jsdom` rather than a decision here: it does
 * not implement the **Pointer Capture** API at all. A strip's `Flash` key
 * captures the pointer so that a finger sliding off it still releases the flash
 * — a press with no release is a strip left at full for the rest of the show —
 * and without the shim that line throws in tests and works in every browser.
 * The shim only has to exist; there is nothing about capture semantics that a
 * test in `jsdom` could observe, because `jsdom` dispatches to the element the
 * event names either way.
 */

import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";

import { nullSink, setLogSink, setLogThreshold } from "../log/logger";

declare global {
  /** React's own switch for "this is a test environment". */
  var IS_REACT_ACT_ENVIRONMENT: boolean;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

// `in` narrows `Element.prototype` to `never` in the negative branch, because
// the DOM types say these three are always there — which is exactly the claim
// `jsdom` breaks. The names are therefore assigned through the prototype as an
// object, which is what it is.
const elements = Element.prototype as unknown as Record<string, unknown>;
if (typeof elements["setPointerCapture"] !== "function") {
  elements["setPointerCapture"] = () => {
    /* jsdom has no pointer capture; see the module documentation. */
  };
  elements["releasePointerCapture"] = () => {
    /* the same. */
  };
  elements["hasPointerCapture"] = () => false;
}

beforeEach(() => {
  setLogThreshold("info");
  setLogSink(nullSink);
});

afterEach(() => {
  cleanup();
});
