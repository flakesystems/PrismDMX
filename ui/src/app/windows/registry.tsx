/**
 * Which component draws which kind of window.
 *
 * The kinds are the daemon's (`WindowType`, generated): the session opens a
 * window by kind, and the canvas asks this table what to put in it. **To build
 * a window, write a component taking `WindowProps` and put it in the table.**
 * The `Record` makes TypeScript complain if a kind is ever missing.
 */

import type { ComponentType } from "react";

import type { WindowInstance, WindowType } from "../../core";
import { GroupsWindow } from "./GroupsWindow";
import { Placeholder } from "./Placeholder";
import { StatusWindow } from "./StatusWindow";

/** What every window component is given. */
export interface WindowProps {
  /** Where it is, its number and its stored `params`. */
  readonly window: WindowInstance;
}

const WINDOWS: Record<WindowType, ComponentType<WindowProps>> = {
  FixtureSheet: Placeholder,
  DmxSheet: Placeholder,
  SequenceSheet: Placeholder,
  Groups: GroupsWindow,
  Viewer3D: Placeholder,
  PhaserEditor: Placeholder,
  ClockViewer: Placeholder,
  CueViewer: Placeholder,
  PresetPool: Placeholder,
  Patch: Placeholder,
  Settings: Placeholder,
  Executors: Placeholder,
  CommandKeys: Placeholder,
  Status: StatusWindow,
};

export function windowContent(type: WindowType): ComponentType<WindowProps> {
  return WINDOWS[type];
}
