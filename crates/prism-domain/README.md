# `prism-domain` — the words everything else speaks

The vocabulary. Every fixture, cue, command, delta and query in PrismDMX is a
type declared here, once, and exported to TypeScript so the interface cannot
drift from the daemon. Nothing in this crate *does* anything: it has no threads,
no files, no sockets and no opinions about the show. It is the agreement the
other eight crates are written against.

Two things follow from that and are worth knowing before you edit it.

- **A change here is a change everywhere.** Adding a field is cheap; renaming
  one is a wire break, a show-file break and a TypeScript break in one edit.
  `docs/IPC_PROTOCOL.md` §4.2 carries the wire's own version and says when it
  has to move.
- **The TypeScript is generated, not written.** `export_bindings` writes
  `ui/src/bindings/` and is run by this crate's *test suite*, so
  `cargo test -p prism-domain` regenerates it and a stale binding cannot survive
  a green build. The directory is deliberately not committed.

## What it may not contain

**No platform code.** `#[cfg(target_os = …)]` is confined to four crates and
this is not one of them — `ARCHITECTURE_SPEC.md` §10.1 is the rule, and the
Linux and ARM64 CI jobs are what enforce it. The rule is stated there and not
repeated here on purpose: a second copy of it is a copy that can go out of date.

## Testing it

```bash
cargo test -p prism-domain
```

Runs on every platform, needs nothing plugged in, and writes `ui/src/bindings/`
as a side effect. `wire.rs` round-trips every exported type through JSON and
MessagePack; `every_exported_type_has_a_round_trip_property` is what stops a new
type being added without one.

## Where the depth is

| | |
|---|---|
| The model | `ARCHITECTURE_SPEC.md` §6 |
| The wire | [`docs/IPC_PROTOCOL.md`](../../docs/IPC_PROTOCOL.md) §§5–6 |
| Everything else | `cargo doc -p prism-domain --open` — the crate documentation is the specification of each type |

## Sessions

Built in **S1**. Extended by nearly every session since; the ones that changed
its *shape* are S28 (cue editing), S33 (the output patch), S38 (the binding
table), S45 (`PlaybackId`), S48 (cue tracking), S52 (an attribute key with an
occurrence in it) and S54 (`AttributeType::Raw`). `PROGRESS.md` §2 has the
record for each.
