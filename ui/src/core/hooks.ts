/**
 * The hooks a view reads the desk with — one per thing there is to show.
 *
 * Each is `useDesk` with a selector defined **at module scope**, which is what
 * keeps a component from re-rendering when some other part of the state moved
 * (`./store/hooks.ts` explains the cache). A view that needs something not
 * listed here writes its own the same way:
 *
 * ```ts
 * const selectFixtureCount = (state: DeskState) =>
 *   Object.keys(showOf(state.documents)?.fixtures ?? {}).length;
 * const count = useDesk(selectFixtureCount);
 * ```
 *
 * **Nothing here changes state.** A view sends a command (`useSend`) or a line
 * (`useRunLine`) and the daemon answers with a delta that changes the store —
 * never the other way round, not even optimistically (decision D3).
 */

import { useCallback } from "react";

import type { CommandLineMode, MachineSettings, ProgrammerState, ShowFileInfo } from "../bindings";
import type { ConnectionStatus } from "./ipc/connection";
import type { DaemonHealth, OutputSnapshot } from "./ipc/protocol";
import type { SessionDocument, ShowDocument } from "./model/documents";
import { sessionOf, showOf } from "./model/documents";
import type { DeskState, Notice } from "./store/desk";
import { useDesk, useSend } from "./store/hooks";

const selectStatus = (state: DeskState): ConnectionStatus => state.status;
const selectConnected = (state: DeskState): boolean => state.status.kind === "connected";
const selectShow = (state: DeskState): ShowDocument | null => showOf(state.documents);
const selectSession = (state: DeskState): SessionDocument | null => sessionOf(state.documents);
const selectProgrammer = (state: DeskState): ProgrammerState | null =>
  state.documents?.programmer ?? null;
const selectNotices = (state: DeskState): readonly Notice[] => state.notices;
const selectOutputs = (state: DeskState): readonly OutputSnapshot[] | null => state.outputs;
const selectHealth = (state: DeskState): DaemonHealth | null => state.health;
const selectShowFile = (state: DeskState): ShowFileInfo | null => state.showFile;
const selectUnsaved = (state: DeskState): boolean => state.unsavedChanges;
const selectMachine = (state: DeskState): MachineSettings | null => state.machine;

/** Where the interface stands with respect to the engine. */
export function useConnectionStatus(): ConnectionStatus {
  return useDesk(selectStatus);
}

/** Whether there is a daemon answering. Everything else is `null` while there is not. */
export function useConnected(): boolean {
  return useDesk(selectConnected);
}

/** The show: patch, groups, presets, cue lists, executors. `null` while disconnected. */
export function useShow(): ShowDocument | null {
  return useDesk(selectShow);
}

/** The session: open windows, views, command line, encoder bank, …. `null` while disconnected. */
export function useSession(): SessionDocument | null {
  return useDesk(selectSession);
}

/** The programmer: selection, touched values, clear stage. `null` while disconnected. */
export function useProgrammer(): ProgrammerState | null {
  return useDesk(selectProgrammer);
}

/** Messages for the operator, newest last. */
export function useNotices(): readonly Notice[] {
  return useDesk(selectNotices);
}

/** The configured DMX outputs. `null` while disconnected. */
export function useOutputs(): readonly OutputSnapshot[] | null {
  return useDesk(selectOutputs);
}

/** How the daemon is doing (tick rate, missed ticks). `null` while disconnected. */
export function useHealth(): DaemonHealth | null {
  return useDesk(selectHealth);
}

/** Which show file is open and whether the autosave is keeping up. */
export function useShowFile(): ShowFileInfo | null {
  return useDesk(selectShowFile);
}

/** Whether the show has unsaved changes — the Save lamp. */
export function useUnsavedChanges(): boolean {
  return useDesk(selectUnsaved);
}

/** What this machine is set to (outputs, surface, listeners). `null` while disconnected. */
export function useMachine(): MachineSettings | null {
  return useDesk(selectMachine);
}

/**
 * Runs a command line, exactly as if the operator had typed it and pressed Enter.
 *
 * `run("Fixture 1 Thru 4")`, `run("Clear")`, `run("Store Cue 5")`. The daemon
 * parses it — the syntax lives in Rust, not here — so every key on the screen
 * can be a line, which is how the old interface worked (`ARCHITECTURE_SPEC.md`
 * §4.5). Answers whether the line went out.
 */
export function useRunLine(): (line: string, mode?: CommandLineMode | null) => boolean {
  const send = useSend();
  return useCallback(
    (line: string, mode: CommandLineMode | null = null) =>
      send({ t: "CommandLineInput", text: line, run: true, mode }) !== null,
    [send],
  );
}

/**
 * Writes into the shared command line without running it.
 *
 * The session's `commandLine` is shared by every client and by the X-Touch, so
 * what is typed is sent as it is typed and every screen shows the same line.
 */
export function useTypeLine(): (text: string) => boolean {
  const send = useSend();
  return useCallback(
    (text: string) => send({ t: "CommandLineInput", text, run: false, mode: null }) !== null,
    [send],
  );
}
