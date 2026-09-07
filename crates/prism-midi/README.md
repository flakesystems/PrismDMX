# `prism-midi` — the port, and only the port

The one crate in the workspace that opens a device. Bytes go in, bytes come out;
nothing here knows what a Mackie Control message *is*.

It exists because a backend needs a home before it can have an implementation.
Until S36 the only code in this repository that had ever opened a MIDI port
lived *outside* the workspace, in `tools/xtouch-probe/`, and the daemon's
`SurfacePort` seam had two implementations, neither of which touched a device.
This crate is that home, and it is deliberately as thin as a home can be:
enumeration, a name, a read, a write, and the cable coming and going.

## The two things it decides

- **A name is the configuration.** A settings window writes down *which* port,
  and the only handle an operating system offers that means anything to a person
  is the name — it is also the only one that survives a replug, because `midir`'s
  identifiers are positions in a list that renumbers itself. `naming.rs` takes
  the platform's decoration off, and that rule is why an ALSA client number is
  not part of what gets stored.
- **A desk that has merely gone quiet is never reopened.** A port that is *there*
  and silent is a surface somebody switched off, not a cable that was pulled, and
  reopening it would be the desk arguing with the operator. The backoff doubles
  to a five-second ceiling and applies only to a port that is genuinely absent.

## What it may not contain

This is **one of the four crates `ARCHITECTURE_SPEC.md` §10.1 allows
`#[cfg(target_os = …)]`**, and it is confined to `system.rs` alone: WinMM,
CoreMIDI or ALSA behind `midir`. Everything above that — the naming, the
reconnection, the `SurfacePort` implementation — is one code path on every
target.

The manifest is what keeps `midir` off the ARM64 cross-check, not a habit: it is
declared for Windows and macOS, and behind an **`alsa` feature** on Linux, whose
MIDI service is a `pkg-config` probe for `libasound2-dev`. So on
`aarch64-unknown-linux-gnu` this crate has no dependencies whatever, and a
Raspberry Pi build is `--features prism-midi/alsa` with that package installed.
A build with no backend enumerates an empty list and refuses to open — the same
answer a machine with nothing plugged in gives.

## Testing it

```bash
cargo test -p prism-midi
```

**Nothing in the suite opens a port.** Every test runs against a fake backend or
asks the real one to enumerate. The half that only a Pi can answer is a row in
`ARCHITECTURE_SPEC.md` §14 with a two-command recipe, not a claim made here.

## Where the depth is

| | |
|---|---|
| Why this crate exists | `ARCHITECTURE_SPEC.md` §10.1, fourth exception |
| What travels over it | [`docs/MCU_MAPPING.md`](../../docs/MCU_MAPPING.md) |
| Everything else | `cargo doc -p prism-midi --open` |

## Sessions

Built in **S36**, whole. `PROGRESS.md` §2.36.
