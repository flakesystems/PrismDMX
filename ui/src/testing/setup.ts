/**
 * What every test run starts from.
 *
 * Two things, and both of them are about not letting one test see another's
 * leavings: the rendered document is torn down after each test — Testing
 * Library only registers that by itself when the globals are on, and they are
 * not — and the logger writes nowhere, because a suite that printed every
 * reconnection to the console would bury its own failures.
 */

import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";

import { nullSink, setLogSink, setLogThreshold } from "../log/logger";

beforeEach(() => {
  setLogThreshold("info");
  setLogSink(nullSink);
});

afterEach(() => {
  cleanup();
});
