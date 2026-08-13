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
 */

import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";

import { nullSink, setLogSink, setLogThreshold } from "../log/logger";

declare global {
  /** React's own switch for "this is a test environment". */
  var IS_REACT_ACT_ENVIRONMENT: boolean;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

beforeEach(() => {
  setLogThreshold("info");
  setLogSink(nullSink);
});

afterEach(() => {
  cleanup();
});
