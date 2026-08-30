/**
 * What each kind of window shows.
 *
 * Seven of the eleven read the show document; the rest say plainly that they
 * are not built yet, which is a deliberate answer rather than a gap — the
 * session is authoritative, an X-Touch F-key can open any of them, and a window
 * that rendered nothing would look like a fault.
 *
 * The show these render is the one the recorded daemon was holding, so the rows
 * are a real patch rather than a fixture written to match the assertions.
 */

import { decode } from "@msgpack/msgpack";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import type { JsonValue, WindowType } from "../bindings";
import { WINDOW_TYPE_VARIANTS } from "../bindings/variants";
import { TelemetrySink } from "../ipc/telemetry";
import { readServerMessage } from "../ipc/protocol";
import { nullSink, setLogSink } from "../log/logger";
import { Shell } from "../testing/shell";
import { DeskStore } from "../store/desk";
import { TelemetryProvider } from "../telemetry/panel";
import { WindowContent } from "./content";

import recordingText from "../../tests/fixtures/session-recording.json?raw";

const recording = JSON.parse(recordingText) as { readonly initialSnapshot: string };

/** The two documents the recorded daemon was holding. */
const { show, session } = ((): { show: JsonValue; session: JsonValue } => {
  const binary = atob(recording.initialSnapshot);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  const message = readServerMessage(decode(bytes));
  if (message.t !== "Snapshot") {
    throw new Error("the recording does not start with a snapshot");
  }
  return { show: message.snapshot.show, session: message.snapshot.session };
})();

/** Renders one window's body, inside the two providers every window sits in. */
function body(type: WindowType, document: JsonValue = show): string {
  const view = render(
    <Shell store={new DeskStore()} session={session}>
      <TelemetryProvider channel={{ sink: new TelemetrySink(), surface: () => null }}>
        <WindowContent
          window={{ instanceId: 1, type, x: 0, y: 0, w: 640, h: 480 }}
          show={document}
          session={session}
          programmer={null}
        />
      </TelemetryProvider>
    </Shell>,
  );
  const text = view.container.textContent ?? "";
  view.unmount();
  return text;
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("a window's body", () => {
  it("shows the rig in the Patch and the state in the Fixture Sheet", () => {
    // The two are **not** the same window, which is S27's decision: the patch
    // is where a rig is built and the sheet is where it is watched.
    const patch = body("Patch");
    expect(patch).toContain("Fixture 1");
    expect(patch).toContain("Addr");
    expect(patch).toContain("Add fixture");

    const sheet = body("FixtureSheet");
    expect(sheet).toContain("Fixture 1");
    // The bank in force, and no address column at all: a sheet is about levels.
    expect(sheet).toContain("Dimmer");
    expect(sheet).not.toContain("Addr");
  });

  it("shows the sequence pool in one window and the cue list in the other", () => {
    // **S43 split the two.** The Sequence Sheet is the pool — which cue lists
    // there are — and the Cue Viewer is one list, its transport and its store
    // bar. The recorded session has no executor selected and no sequence
    // chosen, so the pool lists what there is and the viewer says so.
    expect(body("SequenceSheet")).toContain("Sequence 1");
    expect(body("SequenceSheet")).not.toContain("No executor selected");
    expect(body("CueViewer")).toContain("No executor selected");
    expect(body("CueViewer")).toContain("No cue list is being edited");
  });

  it("says what is missing rather than showing an empty box", () => {
    expect(body("Patch", {})).toContain("Nothing is patched");
    expect(body("FixtureSheet", {})).toContain("Nothing is patched");
    expect(body("Groups", {})).toContain("There are no groups");
    expect(body("SequenceSheet", {})).toContain("0 sequences");
    expect(body("SequenceSheet", {})).toContain("There are no cue lists");
    expect(body("CueViewer", {})).toContain("No cue list is being edited");
    expect(body("PresetPool", {})).toContain("pool is empty");
    // A collection that is there but is not a collection — a hand-edited show,
    // or a daemon that changed shape.
    expect(body("Groups", { groups: 7 })).toContain("There are no groups");
  });

  it("puts the level view in the DMX sheet", () => {
    render(
      <Shell store={new DeskStore()} session={session}>
        <TelemetryProvider channel={{ sink: new TelemetrySink(), surface: () => null }}>
          <WindowContent
            window={{ instanceId: 1, type: "DmxSheet", x: 0, y: 0, w: 640, h: 480 }}
            show={show}
            session={session}
            programmer={null}
          />
        </TelemetryProvider>
      </Shell>,
    );
    expect(screen.getByTestId("telemetry-canvas")).not.toBeNull();
  });

  it("says so, by name, for the windows that are not built yet", () => {
    // **Two, since S43 built the third.** `Settings` left this list in S37 and
    // `ClockViewer` in S43 — a clock is a morning's work and the session's exit
    // criterion says so in as many words. What is left is the two whose sessions
    // have not happened: `Viewer3D` is S30 and `PhaserEditor` is an engine that
    // does not exist yet, so both stay honest lines rather than empty windows.
    for (const type of ["Viewer3D", "PhaserEditor"] as const) {
      const text = body(type);
      expect(text, type).toContain("is not built yet");
      // Named, so an operator can tell which button they pressed.
      expect(text, type).toMatch(/Viewer 3D|Phaser Editor/);
    }
  });

  it("draws the clock, which S43 built", () => {
    // The owner's answer in S43's first round was *the clock and what there is*:
    // the time, and the running times of what is playing back. It is a reading
    // and it is drawn from the documents, so it belongs to no session of its own.
    const text = body("ClockViewer");
    expect(text).not.toContain("is not built yet");
  });

  it("draws the settings window, which S25 reserved and S37 built", () => {
    const text = body("Settings");
    expect(text).not.toContain("is not built yet");
    // The four panels S37 asks for, in the order the plan gives them.
    expect(text).toContain("Outputs");
    expect(text).toContain("Devices");
    expect(text).toContain("Show files");
    expect(text).toContain("This machine");
  });

  it("renders something for every window type there is", () => {
    // The generated table, not a list written here: a window type added to
    // `prism-domain` and not to `WindowContent` is a compile error in the
    // switch, and this is what says the switch is reached for all of them.
    for (const type of WINDOW_TYPE_VARIANTS) {
      expect(body(type).length, type).toBeGreaterThan(0);
    }
  });
});
