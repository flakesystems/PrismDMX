/**
 * The read model: everything the interface renders, and nothing it invents.
 *
 * # Why there is no state-management library here
 *
 * `IMPLEMENTATION_PLAN.md` names Zustand and Immer for this session, and
 * neither earns its place once the shape of the problem is looked at.
 *
 * *Immer* exists to make a deep update read like a mutation. But the two
 * documents this store holds are patched by **RFC 6902 operations against a
 * document root** — `prism-core` decides what changes and the client applies
 * what it is told — so the update is `applyOps`, which already returns a new
 * document with everything untouched shared by reference. A draft proxy in
 * front of that would be a second immutability mechanism over data that is
 * already immutable.
 *
 * *Zustand* is a store with selector subscriptions, which React 19 has as
 * `useSyncExternalStore`. What is left is about forty lines, they are here, and
 * they are testable without a renderer. The interface ships inside a Tauri
 * bundle (S29), so a dependency here is a dependency an operator installs.
 *
 * # Disconnected means *disconnected*
 *
 * §8: on losing the daemon a client *shows a clear disconnected state* and
 * resynchronises through a fresh snapshot. So {@link DeskStore.disconnected}
 * drops the three documents. Not to be tidy — because the alternative is an
 * interface that goes on showing a fader at 63 % after the engine that knew
 * that has stopped, and in a room where somebody is running a show off those
 * numbers, a stale value is worse than no value. There is nothing to render
 * them from until the next snapshot arrives, which is checkable and is checked.
 */

import type { Answer, Command, Delta, NoticeLevel, ProgrammerState, Query } from "../bindings";
import type { ConnectionStatus, ConnectionEvents } from "../ipc/connection";
import type { DaemonHealth, OutputSnapshot, RejectReason, Snapshot } from "../ipc/protocol";
import { logger } from "../log/logger";
import type { Documents } from "../mirror/mirror";
import { applyDelta } from "../mirror/mirror";
import { MirrorFault } from "../mirror/patch";

const log = logger("store");

/** How many notices are kept. Older ones are dropped, oldest first. */
export const NOTICE_LIMIT = 50;

/** One line in the message list. */
export interface Notice {
  /** Distinguishes two identical messages in a list. */
  readonly id: number;
  /** Severity, matching the logger's levels. */
  readonly level: NoticeLevel;
  /** What to show. */
  readonly message: string;
  /** `Date.now()` when it arrived. */
  readonly at: number;
}

/**
 * Everything the interface may render.
 *
 * `documents` is `null` exactly when there is no daemon: it is the snapshot
 * made whole by deltas, and without a connection there is no such thing.
 */
export interface DeskState {
  /** Where the interface stands with respect to the engine. */
  readonly status: ConnectionStatus;
  /** The three documents, or `null` when not connected. */
  readonly documents: Documents | null;
  /** The configured DMX outputs, or `null` when not connected. */
  readonly outputs: readonly OutputSnapshot[] | null;
  /** How the daemon is doing, or `null` when not connected. */
  readonly health: DaemonHealth | null;
  /**
   * How many profiles this desk can embed, or `null` when not connected.
   *
   * A number and not the profiles (S44): the library is two thousand entries on
   * an installed desk, so a client **searches** it rather than holding it. What
   * this is for is saying *2 157 profiles* beside the search box, and knowing
   * whether there are any at all.
   */
  readonly fixtureLibrary: number | null;
  /** Whether the show has unsaved changes — the Save lamp. */
  readonly unsavedChanges: boolean;
  /** Messages for the operator, newest last. */
  readonly notices: readonly Notice[];
}

/** The state of a client that has never seen a daemon. */
export const INITIAL_STATE: DeskState = {
  status: { kind: "connecting", attempt: 0 },
  documents: null,
  outputs: null,
  health: null,
  fixtureLibrary: null,
  unsavedChanges: false,
  notices: [],
};

/** Something that wants to know when the state changed. */
export type Listener = () => void;

/** Sends a command to the daemon, answering with `null` if it could not. */
export type Dispatch = (command: Command) => number | null;

/** Sends a question, answering with `null` if it could not. */
export type Enquire = (query: Query) => number | null;

/** How long a question may go unanswered before the caller is told so. */
export const QUERY_TIMEOUT_MS = 5000;

/**
 * The store.
 *
 * Every method that changes anything replaces the whole state object, so a
 * subscriber comparing by identity sees exactly the changes there were —
 * which is what `useSyncExternalStore` needs and what `CLAUDE.md` asks for in
 * as many words.
 */
export class DeskStore {
  #state: DeskState = INITIAL_STATE;
  #listeners = new Set<Listener>();
  #nextNoticeId = 1;
  #dispatch: Dispatch = () => {
    log.warn("a command was dropped because no connection is attached");
    return null;
  };
  #enquire: Enquire = () => {
    log.warn("a query was dropped because no connection is attached");
    return null;
  };
  /** Questions asked and not yet answered, by the daemon's echo number. */
  #pending = new Map<number, (answer: Answer | null) => void>();

  /** The current state. Stable between changes, so it may be compared by identity. */
  getState = (): DeskState => this.#state;

  /** Subscribes, answering with the function that unsubscribes. */
  subscribe = (listener: Listener): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** Attaches the connection commands and questions go out through. */
  attach(dispatch: Dispatch, enquire?: Enquire): void {
    this.#dispatch = dispatch;
    if (enquire !== undefined) {
      this.#enquire = enquire;
    }
  }

  /**
   * Sends a command.
   *
   * The store does **not** apply it. D3: a client sends intent and receives
   * facts, and what changes this state is the delta that comes back — never
   * the command that went out, not even optimistically.
   */
  send: Dispatch = (command) => this.#dispatch(command);

  /**
   * Asks a question and waits for the answer.
   *
   * Answers `null` rather than throwing when there is no daemon, when the
   * connection goes before the answer does, or when nothing comes back within
   * {@link QUERY_TIMEOUT_MS}. A caller that got `null` shows what it last knew
   * or shows nothing; **it never guesses**, which is the whole reason the
   * question was asked of the daemon in the first place (D3).
   */
  ask = async (query: Query): Promise<Answer | null> => {
    const seq = this.#enquire(query);
    if (seq === null) {
      return null;
    }
    return new Promise<Answer | null>((resolve) => {
      const timer = setTimeout(() => {
        if (this.#pending.delete(seq)) {
          log.warn("a query went unanswered", { query: query.t, seq });
          resolve(null);
        }
      }, QUERY_TIMEOUT_MS);
      this.#pending.set(seq, (answer) => {
        clearTimeout(timer);
        resolve(answer);
      });
    });
  };

  /** The daemon answered a question. */
  answered(seq: number, answer: Answer): void {
    const waiting = this.#pending.get(seq);
    if (waiting === undefined) {
      // An answer to a question this client has stopped caring about — a
      // keystroke two keystrokes ago, or one that timed out. Nothing to do:
      // an answer changes no state by construction.
      log.debug("an answer arrived for a question nobody is waiting for", { seq });
      return;
    }
    this.#pending.delete(seq);
    waiting(answer);
  }

  /** The connection state changed. */
  setStatus(status: ConnectionStatus): void {
    if (this.#state.status === status) {
      return;
    }
    this.#set({ ...this.#state, status });
  }

  /** The world arrived. Everything from before it is replaced, not merged. */
  applySnapshot(snapshot: Snapshot): void {
    this.#set({
      ...this.#state,
      documents: {
        show: snapshot.show,
        session: snapshot.session,
        programmer: snapshot.programmer,
      },
      outputs: snapshot.outputs,
      health: snapshot.health,
      fixtureLibrary: snapshot.fixtureLibrary,
      unsavedChanges: snapshot.health.unsavedChanges,
    });
  }

  /**
   * The daemon is gone: the documents go with it.
   *
   * See the module documentation. The notices stay — a message about what went
   * wrong is the one thing that is still true after the connection is lost.
   */
  disconnected(): void {
    // Every question in flight is answered with `null` rather than left
    // hanging: a patch form waiting on a preview from a daemon that has gone
    // would wait for the timeout and then show an answer about a show nobody
    // is holding any more.
    const waiting = [...this.#pending.values()];
    this.#pending.clear();
    for (const resolve of waiting) {
      resolve(null);
    }
    if (this.#state.documents === null && this.#state.outputs === null) {
      return;
    }
    this.#set({
      ...this.#state,
      documents: null,
      outputs: null,
      health: null,
      fixtureLibrary: null,
      unsavedChanges: false,
    });
  }

  /**
   * Applies one delta.
   *
   * Answers `false` when the delta did not fit the documents, having left the
   * state alone. That is not an error to report and continue from: it means
   * this client and the daemon have already diverged, and the caller's answer
   * is to reconnect for a fresh snapshot.
   */
  applyDelta(delta: Delta): boolean {
    const state = this.#state;
    if (state.documents === null) {
      // A delta that arrives before the snapshot, or after the connection was
      // dropped. There is nothing to apply it to and nothing to fix: the next
      // snapshot carries it.
      log.debug("a delta arrived with no documents to apply it to", { delta: delta.t });
      return true;
    }

    let documents;
    try {
      documents = applyDelta(state.documents, delta);
    } catch (cause) {
      if (cause instanceof MirrorFault) {
        log.error("a delta did not fit the mirror", {
          delta: delta.t,
          fault: cause.detail.kind,
          message: cause.message,
        });
        return false;
      }
      throw cause;
    }

    const next: DeskState = documents === state.documents ? state : { ...state, documents };
    const withSideEffects = this.#sideEffects(next, delta);
    if (withSideEffects !== state) {
      this.#set(withSideEffects);
    }
    return true;
  }

  /** A command was refused. §5: it changed nothing, so only the operator is told. */
  refused(seq: number | null, reason: RejectReason, message: string): void {
    log.warn("the daemon refused something", { seq: seq ?? -1, reason, message });
    this.#set(this.#withNotice(this.#state, "Error", message));
  }

  /** Adds a message for the operator. */
  notice(level: NoticeLevel, message: string): void {
    this.#set(this.#withNotice(this.#state, level, message));
  }

  /**
   * Drops one message, because the operator has read it.
   *
   * Here rather than in the component that draws the close button, so that
   * *which messages exist* has one answer. A view holding its own set of
   * dismissed ids would be a second list, and the two diverge the moment
   * `NOTICE_LIMIT` evicts an id the view is still remembering.
   *
   * Notices are client-local either way — §4.2 — so this dismisses the message
   * on this screen and sends nothing.
   */
  dismissNotice(id: number): void {
    const notices = this.#state.notices.filter((notice) => notice.id !== id);
    if (notices.length === this.#state.notices.length) {
      return;
    }
    this.#set({ ...this.#state, notices });
  }

  /** The deltas that are not documents: the status panel, the save lamp, the messages. */
  #sideEffects(state: DeskState, delta: Delta): DeskState {
    switch (delta.t) {
      case "OutputHealth": {
        if (state.outputs === null) {
          return state;
        }
        return {
          ...state,
          outputs: state.outputs.map((output) =>
            output.id === delta.outputId ? { ...output, health: delta.health } : output,
          ),
        };
      }
      case "DirtyFlag":
        return { ...state, unsavedChanges: delta.unsavedChanges };
      case "Notice":
        return this.#withNotice(state, delta.level, delta.message);
      case "ShowPatch":
      case "SessionPatch":
      case "ProgrammerChanged":
      case "PlaybackState":
        return state;
    }
  }

  #withNotice(state: DeskState, level: NoticeLevel, message: string): DeskState {
    const notice: Notice = { id: this.#nextNoticeId, level, message, at: Date.now() };
    this.#nextNoticeId += 1;
    const notices = [...state.notices, notice];
    return { ...state, notices: notices.slice(-NOTICE_LIMIT) };
  }

  #set(state: DeskState): void {
    if (state === this.#state) {
      return;
    }
    this.#state = state;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

/**
 * The connection callbacks that drive a store.
 *
 * `resync` is called when a delta did not fit — see {@link DeskStore.applyDelta}.
 * Telemetry is deliberately absent: it must never reach reactive state (§7).
 */
export function deskEvents(store: DeskStore, resync: (reason: string) => void): ConnectionEvents {
  return {
    onStatus: (status) => {
      store.setStatus(status);
      if (status.kind !== "connected") {
        store.disconnected();
      }
    },
    onSnapshot: (snapshot) => {
      store.applySnapshot(snapshot);
    },
    onDelta: (delta) => {
      if (!store.applyDelta(delta)) {
        resync("a delta did not fit the mirror");
      }
    },
    onAnswer: (seq, answer) => {
      store.answered(seq, answer);
    },
    onRefused: (seq, reason, message) => {
      store.refused(seq, reason, message);
    },
  };
}

/** The programmer, or an empty one — for a view that would rather not branch. */
export function programmerOf(state: DeskState): ProgrammerState | null {
  return state.documents?.programmer ?? null;
}
