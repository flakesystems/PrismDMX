# `prism-protocols` — what leaves the building

The output drivers. Art-Net (with node discovery), sACN (E1.31) and Open DMX USB
over FTDI, each behind one trait so that the engine above them cannot tell which
is which, and each on its own thread with `catch_unwind` and reconnect backoff.

The design rule is the one that survives a real venue: **an adapter being
unplugged mid-show is the expected case, not an exception.** A driver that
cannot send does not panic, does not block the tick and does not stop; it says
so through its health and keeps trying.

Since S46 there is one thread here that is not an output at all: `NodeDiscovery`
sends `ArtPoll` to the nodes this desk *already sends to* — never a broadcast —
and parses what comes back, so a configured node that nothing is listening on
reads *stopped* rather than *OK*.

## What it may not contain

This is **one of the four crates `ARCHITECTURE_SPEC.md` §10.1 allows
`#[cfg(target_os = …)]`**, and the allowance is narrow: it selects the **FTDI
backend** (D2XX on Windows, libftdi elsewhere) and nothing else. Everything
above `FtdiBackend` — the DMX framing, the break timing, the runner, the health
model, both network drivers — is one code path on every target, which is why
this crate's whole suite runs on the Linux CI job. A `#[cfg]` anywhere else in
here is a bug, not a shortcut.

Two rules it shares with the rest of the workspace and that bite hardest here:

- **No test may need a device.** Every driver is exercised against `MockFtdi` or
  `MockUdp`, on a machine with nothing plugged in.
- **No test sends multicast.** A suite that put sACN on the network it runs on
  would be doing to a build server what this crate does to a venue.

## Testing it

```bash
cargo test -p prism-protocols
```

The wire tests do open a real `UdpSocket` — on loopback, to a mock node — because
a datagram asserted only as a byte array is a datagram nobody has sent. The
hardware bring-up target is separate and `#[ignore]`d; `PROGRESS.md` §3.2 has
the recipe, and it needs an adapter.

## Where the depth is

| | |
|---|---|
| The outputs, and the rig as data | `ARCHITECTURE_SPEC.md` §7 |
| What a venue has to configure | [`docs/manual/installer.de.md`](../../docs/manual/installer.de.md) |
| Everything else | `cargo doc -p prism-protocols --open` |

## Sessions

Built in **S7–S10**. Extended by S33 (several outputs at once, each carrying the
universes its row names) and **S46** (the receive path, and node discovery).
`PROGRESS.md` §2.
