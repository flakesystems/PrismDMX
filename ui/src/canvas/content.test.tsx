/**
 * What each kind of window shows.
 *
 * Three of the eleven read the show document; the rest say plainly that they
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
import { TelemetryProvider } from "../telemetry/panel";
import { WindowContent } from "./content";

import recordingText from "../../tests/fixtures/session-recording.json?raw";

const recording = JSON.parse(recordingText) as { readonly initialSnapshot: string };

/** The show document the recorded daemon was holding. */
const show: JsonValue = (() => {
  const binary = atob(recording.initialSnapshot);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  const message = readServerMessage(decode(bytes));
  if (message.t !== "Snapshot") {
    throw new Error("the recording does not start with a snapshot");
  }
  return message.snapshot.show;
})();

/** Renders one window's body. */
function body(type: WindowType, document: JsonValue = show): string {
  const view = render(
    <TelemetryProvider channel={{ sink: new TelemetrySink(), surface: () => null }}>
      <WindowContent window={{ instanceId: 1, type, x: 0, y: 0, w: 640, h: 480 }} show={document} />
    </TelemetryProvider>,
  );
  const text = view.container.textContent ?? "";
  view.unmount();
  return text;
}

beforeEach(() => {
  setLogSink(nullSink);
});

describe("a window's body", () => {
  it("shows the patch in the patch and the fixture sheet", () => {
    for (const type of ["Patch", "FixtureSheet"] as const) {
      const text = body(type);
      expect(text, type).toContain("Fixture 1");
      expect(text, type).toContain("generic.dimmer");
      // Universe and address, which is what makes it a patch rather than a
      // list of names.
      expect(text, type).toContain("Addr");
    }
  });

  it("shows the pools the show has, by number and name", () => {
    expect(body("SequenceSheet")).toContain("Sequence 1");
    expect(body("CueViewer")).toContain("Sequence 1");
  });

  it("says what is missing rather than showing an empty box", () => {
    expect(body("Patch", {})).toContain("Nothing is patched");
    expect(body("Groups", {})).toContain("No groups yet");
    expect(body("SequenceSheet", {})).toContain("No sequences yet");
    expect(body("PresetPool", {})).toContain("No presets yet");
    // A collection that is there but is not a collection — a hand-edited show,
    // or a daemon that changed shape.
    expect(body("Groups", { groups: 7 })).toContain("No groups yet");
  });

  it("puts the level view in the DMX sheet", () => {
    render(
      <TelemetryProvider channel={{ sink: new TelemetrySink(), surface: () => null }}>
        <WindowContent
          window={{ instanceId: 1, type: "DmxSheet", x: 0, y: 0, w: 640, h: 480 }}
          show={show}
        />
      </TelemetryProvider>,
    );
    expect(screen.getByTestId("telemetry-canvas")).not.toBeNull();
  });

  it("says so, by name, for the windows that are not built yet", () => {
    for (const type of ["Viewer3D", "PhaserEditor", "ClockViewer", "Settings"] as const) {
      const text = body(type);
      expect(text, type).toContain("is not built yet");
      // Named, so an operator can tell which button they pressed.
      expect(text, type).toMatch(/Viewer 3D|Phaser Editor|Clock Viewer|Settings/);
    }
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
