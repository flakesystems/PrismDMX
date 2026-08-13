/**
 * The wire format of `docs/IPC_PROTOCOL.md` §3, from the browser's end.
 *
 * # What this end of the wire does and does not do
 *
 * §3's frame is a `u32` length prefix followed by a MessagePack payload — and
 * the prefix *belongs to the byte-stream transports*. A WebSocket message
 * already carries its own length, and a second copy of the same number inside
 * it would be two lengths that can disagree; `prism-ipc` made that decision in
 * S16 and the browser is on the WebSocket. So there is no framing here, only a
 * payload.
 *
 * What both ends share is the **limit**. `MAX_FRAME_BYTES` is checked before a
 * payload is decoded and before one is sent, so a message too big for the
 * daemon is refused here rather than closing the connection there.
 *
 * # Why a MessagePack library rather than a hand-written codec
 *
 * The daemon uses `rmp-serde`. Writing the other half by hand would put a
 * second implementation of a binary format in the project — in the one language
 * where a decoding mistake is silent — and there is nothing project-specific
 * about MessagePack. `@msgpack/msgpack` has no dependencies of its own and
 * decodes into `unknown`, which is exactly the shape `./shape.ts` wants: the
 * checking that matters is *is this a message*, not *is this MessagePack*.
 *
 * Its decoder is iterative rather than recursive — it walks with an explicit
 * stack — so a hostile nesting depth cannot overflow the JavaScript stack the
 * way it could overflow Rust's. The depth limit in `./shape.ts` is about what
 * happens afterwards, when the value is walked as a document.
 */

import { decode, encode } from "@msgpack/msgpack";

import type { ClientMessage, ServerMessage } from "./protocol";
import { readServerMessage } from "./protocol";
import type { Payload } from "./shape";
import { ProtocolFault, overArrayBuffer } from "./shape";

export type { Payload } from "./shape";
export { overArrayBuffer } from "./shape";

/** The maximum payload, matching `prism_ipc::MAX_FRAME_BYTES`. */
export const MAX_FRAME_BYTES = 1024 * 1024;

/**
 * A message the client sends, as MessagePack.
 *
 * `ignoreUndefined` is what makes an absent optional field absent rather than
 * `nil`: `Command::OpenWindow` skips its `params` when there are none, and a
 * `nil` in its place is a different message.
 *
 * @throws {ProtocolFault} if the message would be too big for the daemon to read.
 */
export function encodeClientMessage(message: ClientMessage): Payload {
  const payload = encode(message, { ignoreUndefined: true });
  if (payload.byteLength > MAX_FRAME_BYTES) {
    throw new ProtocolFault(
      "ClientMessage",
      `at most ${MAX_FRAME_BYTES} bytes, not ${payload.byteLength}`,
    );
  }
  return overArrayBuffer(payload);
}

/**
 * A payload from the daemon as a message.
 *
 * @throws {ProtocolFault} if it is too big, not MessagePack, or not a message
 * this build understands. None of those ends the connection by itself — §8 is
 * explicit that a payload the *daemon* cannot decode does not, and the same
 * reasoning holds in this direction.
 */
export function decodeServerMessage(payload: Uint8Array): ServerMessage {
  if (payload.byteLength > MAX_FRAME_BYTES) {
    throw new ProtocolFault(
      "ServerMessage",
      `at most ${MAX_FRAME_BYTES} bytes, not ${payload.byteLength}`,
    );
  }
  let value: unknown;
  try {
    value = decode(payload);
  } catch (cause) {
    throw new ProtocolFault(
      "ServerMessage",
      `MessagePack, not ${cause instanceof Error ? cause.message : "something unreadable"}`,
    );
  }
  return readServerMessage(value);
}
