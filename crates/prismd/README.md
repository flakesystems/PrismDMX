# `prismd` — the desk

The engine daemon: the process that owns the show, drives DMX, holds the session
state and answers clients — and **keeps running whether or not anything is
looking at it**. That is decision **D2**, and it is the reason the desk survives
a browser crash, a shell restart and an operator closing a window mid-show.

The process is a library plus a nine-line binary. Everything is in the library
because a binary target has no tests, and every exit criterion this crate was
set is a statement about a *running* daemon: that it drives DMX with no client
connected, that a second instance refuses to start, that a stale lock is taken
over, that the handshake serves.

## One daemon, structurally

Two daemons driving one rig is the worst failure this program can have, so it is
prevented by the operating system rather than by convention. `prismd` holds an
**advisory lock** on an empty `prismd.guard` for its lifetime and writes a
`prismd.lock` beside it with its process id, its endpoints and its token. Two
files, because an exclusive lock on Windows stops other processes reading the
locked bytes and the whole point of the discovery file is that clients read it.

The liveness question is therefore *is anybody holding this*, not *is process
4711 alive* — the kernel releases the lock when the process ends, killed
included, so a stale file needs no heuristic and a recycled process id cannot
lie.

## What it may not contain

**No platform code.** `ARCHITECTURE_SPEC.md` §10.1 names four crates and this is
not one of them, which is not an accident but a constraint that was met four
times over: the user data directory comes from a dependency, the tick thread's
priority from another, the IPC endpoint from `prism-ipc`, and the MIDI port from
`prism-midi`. Its tests run on the Linux CI job over a Unix domain socket and on
the Windows job over a named pipe — which is what keeps the claim honest.

## Testing it

```bash
cargo test -p prismd
```

Nothing needs a device: `--mock-output` and `--mock-surface` are what the suite
drives, and `tests/resilience.rs` is decision D2 executed — a client attached,
killed, and the frames counted through the gap. The corpus and installer tests
want the fixture library present:

```bash
tools/fetch-fixtures/fetch-fixtures.sh      # or .ps1 on Windows
```

The recordings under `tests/` that the interface's own tests read back are
regenerated deliberately, never by accident:

```bash
cargo test -p prismd --test ui_programmer -- --ignored
```

## Running it

```bash
cargo run -p prismd -- --mock-output
prismd --help          # every flag, and every one of them is also a setting
```

## Where the depth is

| | |
|---|---|
| Every flag, and its setting | [`docs/manual/installer.de.md`](../../docs/manual/installer.de.md) |
| The protocol it serves | [`docs/IPC_PROTOCOL.md`](../../docs/IPC_PROTOCOL.md) |
| Lifecycle, autostart, the single-instance guarantee | `ARCHITECTURE_SPEC.md` §10.3 |
| Everything else | `cargo doc -p prismd --open` |

## Sessions

Built in **S17** and **S18** (the D2 gate). Extended by S22 (the surface with no
client), S33 (the rig), S37 (every flag becomes a setting), S46 (node
discovery) and **S49** (it runs the command line now). `PROGRESS.md` §2.
