# `prism-engine` — the tick, and nothing that can stop it

The real-time half. It merges the show into one value per DMX slot forty-four
times a second and publishes the frame; it holds no files, opens no sockets and
knows nothing about an interface. This is the crate `CLAUDE.md`'s zero-crash
invariant is really about, and it carries the project's strictest coverage
requirement.

The shape, smallest first: pure merge arithmetic, a `MergePlan` built off the
tick, the playback / programmer / master layers resolved into it, and a triple
buffer that hands the finished frame to whichever output drivers are subscribed.
Nothing on that path takes a lock, allocates, or waits for anything else.

## The rule that decides every question here

**The tick makes no allocator call.** Not *few* — none, on every path, measured
rather than argued: `tests/tick_allocations.rs` counts what the allocator was
asked for on ten separate paths and asserts nought, with an eleventh test that
deliberately allocates so the probe cannot be silently broken. A new path
through the tick needs a new row in that file, or the claim stops being true
without anything going red.

The corollary an editor meets first: anything that needs a `Vec`, a `String` or
a lock belongs on the core thread, before the frame, not inside it.

## What it may not contain

**No platform code and no I/O.** `#[cfg(target_os = …)]` is confined to four
crates and this is not one of them (`ARCHITECTURE_SPEC.md` §10.1), which is why
its whole suite runs on the Linux CI job as well as the Windows one. A driver, a
socket or a file here would also be a blocking call on the tick, which §3.1
forbids for a different reason.

## Testing it

```bash
cargo test -p prism-engine
```

The long ones are `#[ignore]`d because they take minutes:

```bash
cargo test -p prism-engine --release --test realtime -- --ignored --nocapture
cargo test -p prism-engine --test triple_buffer_stress -- --ignored
RUSTFLAGS="--cfg loom" cargo test -p prism-engine --release --test triple_buffer_stress
```

`PROGRESS.md` §3.1 has the harness and the caveat that matters: a deadline
measurement taken on a busy machine says nothing, so the bare-tick control is
run beside it and the pair is the reading.

## Where the depth is

| | |
|---|---|
| The merge | [`docs/DMX_MERGE.md`](../../docs/DMX_MERGE.md) |
| The tick contract | `ARCHITECTURE_SPEC.md` §3.1 |
| The pipeline order | `ARCHITECTURE_SPEC.md` §5 |
| Everything else | `cargo doc -p prism-engine --open` |

## Sessions

Built in **S2–S6**. Changed since by S33 (`FrameEnrolment`: outputs come and go
without costing the tick a frame), S45 (a playback belongs to the cue list),
S48 (cue tracking, the ninth allocation-free path), S51 (the two crossfade
modes) and S52 (a three-part merge key, and the tenth path). `PROGRESS.md` §2.
