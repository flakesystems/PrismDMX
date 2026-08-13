/**
 * The codec: the size limit, and what an unreadable payload does.
 *
 * That the bytes are the *daemon's* bytes is asserted in
 * `src/mirror/recording.test.ts`, against a recording of one. This file is
 * about the refusals, which have nothing to do with MessagePack.
 */

import { decode, encode } from "@msgpack/msgpack";
import { describe, expect, it } from "vitest";

import { MAX_FRAME_BYTES, decodeServerMessage, encodeClientMessage } from "./codec";
import { ProtocolFault, asRecord, field } from "./shape";

/** The member at `key`, checked rather than asserted. */
function member(value: unknown, key: string): unknown {
  return field(asRecord(value, key), key);
}

describe("encoding", () => {
  it("writes the envelope the daemon reads", () => {
    const payload = encodeClientMessage({
      t: "Command",
      seq: 3,
      command: { t: "CommandLineInput", text: "fixture 1 at full" },
    });
    expect(decode(payload)).toEqual({
      t: "Command",
      seq: 3,
      command: { t: "CommandLineInput", text: "fixture 1 at full" },
    });
  });

  it("leaves an absent optional field absent rather than sending nil", () => {
    // `Command::OpenWindow` skips its `params` when there are none, and a nil
    // in its place is a different message to `rmp-serde`.
    const payload = encodeClientMessage({
      t: "Command",
      seq: 0,
      command: { t: "OpenWindow", window: "Patch" },
    });
    const command = asRecord(member(decode(payload), "command"), "command");
    expect(command).toEqual({ t: "OpenWindow", window: "Patch" });
    expect(Object.hasOwn(command, "params")).toBe(false);
  });

  it("refuses a message too big for the daemon to read", () => {
    expect(() =>
      encodeClientMessage({
        t: "Command",
        seq: 0,
        command: { t: "CommandLineInput", text: "x".repeat(MAX_FRAME_BYTES + 1) },
      }),
    ).toThrow(ProtocolFault);
  });
});

describe("decoding", () => {
  it("refuses a payload that is too big before it decodes it", () => {
    expect(() => decodeServerMessage(new Uint8Array(MAX_FRAME_BYTES + 1))).toThrow(
      /at most 1048576 bytes/,
    );
  });

  it("refuses a payload that is not MessagePack", () => {
    // 0xc1 is the one byte MessagePack never assigns; the second is a header
    // announcing three elements with one behind it.
    expect(() => decodeServerMessage(new Uint8Array([0xc1]))).toThrow(ProtocolFault);
    expect(() => decodeServerMessage(new Uint8Array([0x93, 0x01]))).toThrow(ProtocolFault);
  });

  it("refuses MessagePack that is not a message", () => {
    expect(() => decodeServerMessage(encode({ t: "Wibble" }))).toThrow(ProtocolFault);
    expect(() => decodeServerMessage(encode([1, 2, 3]))).toThrow(ProtocolFault);
  });

  it("reads a message the daemon could have sent", () => {
    expect(decodeServerMessage(encode({ t: "Ack", seq: 12 }))).toEqual({ t: "Ack", seq: 12 });
  });
});
