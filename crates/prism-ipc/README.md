# `prism-ipc` — what travels between the two halves

Length-prefixed MessagePack: over a named pipe or a Unix domain socket for the
shell on this machine, over a WebSocket for anything else. Both carry identical
messages, and the choice is invisible above this crate.

The protocol is deliberately asymmetric. **Clients send intent; the daemon sends
facts.** No client computes state and expects the daemon to accept it — that is
decision **D3**, and it is why a gesture in the interface is a command out and a
delta back rather than a local edit.

## The transport disappears at `Wire`

A named pipe, a Unix domain socket and a WebSocket have three different types
and three different error vocabularies, and exactly one thing in common: each
can carry byte payloads in both directions. `Wire` is that common thing, and
every transport module's whole job is to produce one.

The pay-off is `memory::pair` — a `Wire` over an in-process duplex — which makes
the handshake, the backpressure policy and every error path testable with no
socket, no port and no file system. That is the same move `prism-protocols`
makes for `DmxOutput`: choose the abstraction so the *tests* can implement it.

## A message from outside is an attack surface

Three refusals, in this order, before `serde` is allowed near a payload:

1. **Too long.** A peer announcing four gigabytes is disconnected after four
   bytes have been read, having cost four bytes of memory.
2. **Too deep.** The payload is walked with an explicit stack first, because a
   stack overflow *aborts the process* and this process is holding the DMX
   output.
3. **Not the message it claims to be.** Only then is it deserialised.

## What it may not contain

This is **one of the four crates `ARCHITECTURE_SPEC.md` §10.1 allows
`#[cfg(target_os = …)]`**, and it is confined to `transport/local.rs` alone — a
named pipe on Windows, a Unix domain socket elsewhere. The framing, the
handshake, the backpressure policy, the server and the client are one code path
on every target.

## Testing it

```bash
cargo test -p prism-ipc
```

`tests/transport_parity.rs` is the one that holds the claim above: the same
conversation over the local transport and over the WebSocket, asserted to be the
same conversation. `tests/oversized_frame.rs` counts what the allocator was
asked for.

## Where the depth is

| | |
|---|---|
| The protocol, message by message | [`docs/IPC_PROTOCOL.md`](../../docs/IPC_PROTOCOL.md) |
| The listener, tokens and the network | `ARCHITECTURE_SPEC.md` §2.1 |
| Everything else | `cargo doc -p prism-ipc --open` |

## Sessions

Built in **S16**. Extended by S23 (the WebSocket a browser actually reconnects
over) and S25 (telemetry's cadence). `PROGRESS.md` §2.
