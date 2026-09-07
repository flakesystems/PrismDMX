# `prism-surface` — the X-Touch, as three layers

Mackie Control, decoded and encoded. Three layers kept apart so that protocol
detail, device abstraction and user configuration never mix:

1. **The codec** — MIDI bytes to logical control events, and feedback back to
   bytes. Running status, SysEx reassembly, counters. *No domain type appears at
   this layer at all*: `GlobalButton::Play` is note 94, and that pressing it
   starts an executor is layer 3's opinion.
2. **The surface model** — device-independent controls, two acceleration curves,
   touch bookkeeping, and the shadow model that makes outbound traffic a diff
   rather than a repaint.
3. **The binding table** — JSON, user-editable, mapping a control to a command.

`CLAUDE.md`'s rule is stated for this crate as: a malformed MIDI packet must
never propagate a failure. The codec discards and counts; nothing reaches the
engine thread.

## What it may not contain

**No platform code, and no port.** `#[cfg(target_os = …)]` is confined to four
crates and this is not one of them (`ARCHITECTURE_SPEC.md` §10.1) — and opening
a MIDI port is nothing *but* platform code, so it happens one crate along, in
[`prism-midi`](../prism-midi/README.md). This crate gained **no dependency at
all** when that backend arrived, which is the measurement that says the split
held.

## Testing it

```bash
cargo test -p prism-surface
```

Nothing here opens a port, on a machine with a real X-Touch attached. What a
real device did is in the repository instead: `tests/captures/` holds four
recordings taken in S20 from a Behringer X-Touch in MC mode over USB, firmware
V1.25, and `tests/hardware_capture.rs` replays them in the ordinary suite. That
is the pattern for any future device — **platform code and hardware in a tool,
evidence in a fixture.**

## Where the depth is

| | |
|---|---|
| Every note, CC and offset | [`docs/MCU_MAPPING.md`](../../docs/MCU_MAPPING.md) |
| What the surface can do without a client | `ARCHITECTURE_SPEC.md` §4 |
| Everything else | `cargo doc -p prism-surface --open` |

## Sessions

Built in **S19–S22**: the codec, the verification against real hardware, the
shadow model, and the bindings. Extended by **S38** (the table is editable from
the settings window) and S45. `PROGRESS.md` §2.
