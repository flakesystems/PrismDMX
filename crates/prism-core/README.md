# `prism-core` — the show, and everything the daemon is right about

The single source of truth. The patch, the groups, presets, cue lists and
executors; the programmer the operator is editing into; the Oops journal; the
session state that lets the X-Touch operate the interface with no client
attached (decision **D11**); the command line's parser; and the `.prism` file
they are all written into.

If you are looking for *where a command actually happens*, it is here. There are
three appliers and a command belongs to exactly one of them:

| Applier | What it acts on | Example |
|---|---|---|
| the show | numbered objects a `.prism` file holds | `Store Cue 5` |
| the session | what this desk is looking at | `View 2`, a window moved |
| the machine | this building's cabling and this desk's identity | an Art-Net output added |

A fourth kind exists and has no applier: `Shutdown` acts on the *process*. That
distinction is the first question to ask of any new command.

## Two rules that are older than most of the code

- **The console does not read the show.** `prism_core::console` turns a typed
  line into commands and nothing else; `Copy Sequence 2 Sequence 6` means the
  same thing whether or not sequence 2 exists, and a line naming a fixture the
  show has not got parses perfectly and is refused by the show. That is decision
  **D3**. [`docs/COMMAND_LINE.md`](../../docs/COMMAND_LINE.md) §4 has it in full.
- **The parser never fails.** Every string there is answers with commands or
  with a sentence somebody can be shown. `tests/console.rs` runs ten thousand
  generated lines through it.

## What it may not contain

**No platform code.** `#[cfg(target_os = …)]` is confined to four crates and
this is not one of them (`ARCHITECTURE_SPEC.md` §10.1). It does carry a C
dependency — SQLite, bundled rather than linked, because a show file has to open
on a machine that has never heard of SQLite — and `Cargo.toml` has the reasoning
beside the dependency.

## Testing it

```bash
cargo test -p prism-core
```

Nothing here needs hardware or a network. The fixture-library corpus tests want
the Open Fixture Library installed and **skip themselves** without it, which is
why every CI job that runs them fetches it first:

```bash
tools/fetch-fixtures/fetch-fixtures.sh      # or .ps1 on Windows
```

## Where the depth is

| | |
|---|---|
| The model, and what state lives where | `ARCHITECTURE_SPEC.md` §§4 and 6 |
| The console line | [`docs/COMMAND_LINE.md`](../../docs/COMMAND_LINE.md) |
| Undo | `ARCHITECTURE_SPEC.md` §6.1 |
| Everything else | `cargo doc -p prism-core --open` |

## Sessions

Built in **S11–S15**. The ones that changed its shape since: S26 and S40 (the
command line), S28 (cue editing), S33 (the rig moves out of the show and into
the machine), S43 (the daemon places windows), S48 (tracking is derived),
**S49** (the parser moves here from the interface), S51, and S52–S54 (the
fixture-library reader: repeated parameters, the manufacturer's own names, and a
knob on every slot). `PROGRESS.md` §2.
