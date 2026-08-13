/**
 * **The first exit criterion of S24.** 64 universes at 30 Hz, sustained, with
 * *zero* React re-renders — counted, not judged by feel.
 *
 * `docs/IPC_PROTOCOL.md` §7's last paragraph is the rule: clients must not put
 * telemetry into reactive state. S23 proved half of it — a hundred frames cost
 * the store no notifications — and this is the other half, one layer up, where
 * it could still have gone wrong: a panel that held the frame in `useState`, a
 * context that published it, a hook that returned it.
 *
 * The counter is a `<Profiler>` round the whole interface. It fires on every
 * commit of the tree beneath it, so *no commit at all* between the handshake and
 * the three-hundredth frame is the strongest form of the claim available in
 * jsdom: not "the panel did not re-render", but **nothing did**.
 *
 * The frames are the recorded 64-universe one, which is 32 912 bytes of a rig a
 * real `prismd` was actually driving, sent through a real `Connection` over the
 * fake socket. Ten seconds of telemetry at §7's rate, decoded and drawn.
 */

import { render, screen } from "@testing-library/react";
import { Profiler, act } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "../App";
import { Connection } from "../ipc/connection";
import { TelemetrySink } from "../ipc/telemetry";
import { nullSink, setLogSink } from "../log/logger";
import { DeskProvider } from "../store/context";
import { DeskStore, deskEvents } from "../store/desk";
import {
  FakeNetwork,
  ManualTimer,
  serverMessage,
  snapshot as aSnapshot,
} from "../testing/fake-daemon";
import { malformedFrames, wideFrame } from "../testing/telemetry-frames";
import type { Scheduler } from "./driver";
import type { LevelSurface } from "./painter";
import { TelemetryProvider } from "./panel";

/** §7's rate: thirty frames a second. */
const FRAME_MS = 1000 / 30;

/** Ten seconds of it. */
const FRAMES = 300;

/** A scheduler the test steps by hand. */
class ManualFrames {
  #run: ((now: number) => void) | null = null;

  readonly scheduler: Scheduler = (run) => {
    this.#run = run;
    return () => {
      this.#run = null;
    };
  };

  frame(now: number): void {
    this.#run?.(now);
  }
}

/**
 * A surface that counts rather than records.
 *
 * `RecordingSurface` keeps a copy of every blit, and three hundred frames of 64
 * universes is forty megabytes of them. What this test needs to know is only
 * that the picture was drawn.
 */
class CountingSurface implements LevelSurface {
  readonly width = 1024;
  readonly height = 640;
  blits = 0;
  fills = 0;
  labels = 0;
  clears = 0;

  clear(): void {
    this.clears += 1;
  }
  fill(): void {
    this.fills += 1;
  }
  label(): void {
    this.labels += 1;
  }
  blit(): void {
    this.blits += 1;
  }
}

/** A rendered interface with a daemon, a telemetry channel and a commit counter. */
function desk() {
  const network = new FakeNetwork();
  const clock = new ManualTimer();
  const frames = new ManualFrames();
  const surface = new CountingSurface();
  const sink = new TelemetrySink();
  const store = new DeskStore();
  const events = deskEvents(store, (reason) => {
    connection.resync(reason);
  });
  const connection = new Connection(
    { url: "ws://127.0.0.1:7373/ipc", socketFactory: network.factory, timer: clock.timer },
    { ...events, onTelemetry: sink.accept },
  );
  store.attach((command) => connection.send(command));

  let commits = 0;
  // The channel object is built once: it is the effect's only dependency, and a
  // new one on every render would restart the loop rather than measure it.
  const channel = {
    sink,
    scheduler: frames.scheduler,
    clock: () => 0,
    surface: () => surface,
  };

  render(
    <Profiler
      id="desk"
      onRender={() => {
        commits += 1;
      }}
    >
      <DeskProvider store={store}>
        <TelemetryProvider channel={channel}>
          <App />
        </TelemetryProvider>
      </DeskProvider>
    </Profiler>,
  );
  act(() => {
    connection.start();
    network.last.open();
    network.last.deliver(serverMessage({ t: "Snapshot", snapshot: aSnapshot() }));
  });

  return {
    network,
    frames,
    surface,
    sink,
    store,
    commits: () => commits,
  };
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("64 universes at 30 Hz", () => {
  it("costs the interface not one render", () => {
    const { network, frames, surface, commits } = desk();
    expect(screen.getByTestId("telemetry")).not.toBeNull();
    const mounted = commits();
    expect(mounted).toBeGreaterThan(0);

    for (let frame = 0; frame < FRAMES; frame += 1) {
      act(() => {
        // Through the whole path: MessagePack envelope → `readServerMessage` →
        // the sink. A new payload each time, as a daemon sends.
        network.last.deliver(serverMessage({ t: "Telemetry", data: wideFrame }));
        frames.frame(frame * FRAME_MS);
      });
    }

    // **The criterion.** Ten seconds, three hundred frames, 9.8 megabytes of
    // levels — and React committed nothing.
    expect(commits()).toBe(mounted);
    // And it was not zero work: the picture was drawn for every frame.
    expect(surface.blits).toBe(FRAMES);
    // The chrome was drawn once, not three hundred times.
    expect(surface.clears).toBe(1);

    // One more animation frame, with nothing new in the sink, far enough after
    // the last for the readout's own quarter-second to have passed. It draws
    // nothing — the assertion above is what says so — and it is how the line
    // comes to name the last frame rather than the one four before it.
    act(() => {
      frames.frame(FRAMES * FRAME_MS + 250);
    });
    expect(surface.blits).toBe(FRAMES);

    // The readout says so, and it says so without a render — which is the same
    // claim from the other side: if React had re-rendered this element, its
    // text would be back to what the JSX says.
    const readout = screen.getByTestId("telemetry-stats").textContent ?? "";
    expect(readout).toContain("64 universes");
    expect(readout).toContain(`${FRAMES} frames`);
    expect(readout).not.toContain("waiting");
    expect(readout).not.toContain("not live");
  });

  it("keeps the control state exactly where it was when telemetry cannot be read", () => {
    const { network, frames, store, commits } = desk();
    expect(screen.getByTestId("executor-page").textContent).toBe("3");
    const before = commits();

    // Every way a frame can be wrong, thirty times over — the recorded ones, so
    // these are refusals `prism-ipc` itself would make.
    for (let round = 0; round < 30; round += 1) {
      for (const malformed of malformedFrames) {
        act(() => {
          network.last.deliver(serverMessage({ t: "Telemetry", data: malformed.bytes }));
          frames.frame(round * FRAME_MS);
        });
      }
    }

    // Nothing rendered, nothing was refused, nothing reconnected: telemetry the
    // interface cannot read is one missing picture and nothing else.
    expect(commits()).toBe(before);
    expect(screen.getByTestId("connection-status").textContent).toBe("Connected");
    expect(screen.queryByTestId("notices")).toBeNull();
    expect(store.getState().documents).not.toBeNull();

    // And the control channel is not merely intact but *working*: a delta after
    // the rubbish arrives and is applied.
    act(() => {
      network.last.deliver(
        serverMessage({
          t: "Delta",
          delta: { t: "SessionPatch", ops: [{ op: "replace", path: "/session/executorPage", value: 7 }] },
        }),
      );
    });
    expect(screen.getByTestId("executor-page").textContent).toBe("7");
    expect(commits()).toBeGreaterThan(before);

    const readout = screen.getByTestId("telemetry-stats").textContent ?? "";
    expect(readout).toContain("dropped");
  });

  it("takes the picture down with the daemon and puts nothing stale in its place", () => {
    const { network, frames, surface, sink } = desk();
    act(() => {
      network.last.deliver(serverMessage({ t: "Telemetry", data: wideFrame }));
      frames.frame(0);
    });
    expect(surface.blits).toBe(1);

    // The daemon goes. `createDesk` clears the sink on any status that is not
    // *connected*; this is that, without the wiring.
    act(() => {
      network.last.drop("the daemon stopped");
      sink.clear();
      frames.frame(FRAME_MS);
    });

    // The panel goes with the rest of the readouts, because the documents went.
    expect(screen.queryByTestId("telemetry")).toBeNull();
    expect(screen.queryByTestId("telemetry-stats")).toBeNull();
    expect(surface.blits).toBe(1);
  });
});
