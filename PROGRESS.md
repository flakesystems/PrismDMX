# PROGRESS.md — PrismDMX Status Tracker

**Last updated:** 2026-08-10
**Current phase:** Phase 1 — Domain and engine
**Current session:** S4 — `prism-engine` attribute-to-DMX encoding (not started; see §8 for the prompt that starts it)
**Last completed:** S3 — `prism-engine` HTP/LTP merge ✅
**Plan:** [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) · **Architecture:** [`ARCHITECTURE_SPEC.md`](ARCHITECTURE_SPEC.md)

> Update this file at the end of every session. Record what was *measured*, not what was intended. A session is `done` only when its exit criteria in the plan actually pass.

**Legend:** ☐ todo · ▶ in progress · ✅ done · ⛔ blocked · 🔌 needs hardware

---

## 1. Setup checklist

| # | Item | Status | Note |
|---|---|---|---|
| 1 | Git repository initialised | ✅ | `git init` done, `core.autocrlf=false`. No commit made yet |
| 2 | `.gitignore` | ✅ | Rust, Node, Tauri, SQLite show files, OS cruft |
| 3 | Rust toolchain | ✅ | rustup 1.29.0, **rustc 1.97.1** stable-x86_64-pc-windows-msvc |
| 4 | MSVC Build Tools / linker | ✅ | Visual Studio Build Tools 2022, **17.14.37516.0**. Installed manually by the user |
| 5 | Cargo workspace | ✅ | 8 crates, verified with `cargo check` (no linker required) |
| 6 | Workspace lints (clippy, rustfmt) | ✅ | Shared config in root `Cargo.toml` |
| 7 | Node / npm | ✅ | Node v24.11.0, npm 11.6.1 |
| 8 | `ui/` scaffold (Vite + React + TS) | ✅ | Vite 8.2.1, 69 packages, 0 vulnerabilities. `strict` added manually — see D-log |
| 9 | CI workflow | ✅ | `.github/workflows/ci.yml` — Windows full, Linux neutral tests, ARM64 cross-check, UI |
| 10 | First commit | ✅ | `chore(workspace)` — docs, workspace, UI scaffold, CI |
| 11 | Git remote | ✅ | `origin` → github.com/flakesystems/PrismDMX (**private**), `master` pushed and tracking |
| 12 | GitHub CLI | ✅ | `gh` 2.97.0, authenticated (scopes: repo, workflow, read:org, gist) |
| 13 | CI verified green | ✅ | Latest: run **31412972231** on `0f7b753` (S3). First verified: run **31346581991** — all four jobs: Windows 55 s, ARM64 check 20 s, UI 15 s, Linux neutral 14 s. No annotations |
| 14 | `loom` model checking | ✅ | `loom` 0.7.2, a `cfg(loom)`-only dependency of `prism-engine`. Not run by CI — see §3.1 for the command |

---

## 2. Session status

### Phase 0 — Foundation
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S0 | Toolchain and workspace | ✅ | 2026-08-10 | All local exit criteria verified — see §2.1. The one criterion not met is "CI green on a pushed branch": no remote exists, tracked as **B2**. Not treated as blocking S1 |

### Phase 1 — Domain and engine
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S1 | `prism-domain` — types | ✅ | 2026-08-10 | All exit criteria verified — see §2.2. 133 tests, coverage 99.8 % lines |
| S2 | `prism-engine` — tick loop, triple buffer | ✅ | 2026-08-10 | All exit criteria verified — see §2.3. 77 tests + 3 `loom` models, coverage 98.6 % lines |
| S3 | `prism-engine` — HTP/LTP merge | ✅ | 2026-08-10 | All exit criteria verified — see §2.4. 135 tests, coverage 99.1 % lines |
| S4 | `prism-engine` — DMX encoding | ☐ | | Consumes `MergeBody::values()` — attribute values, not bytes |
| S5 | `prism-engine` — executors, cues, fades | ☐ | | |
| S6 | `prism-engine` — programmer, masters, stress | ☐ | | Coverage gate > 95 % |

### Phase 2 — Protocols
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S7 | `DmxOutput` trait + Open DMX USB | ☐ | | |
| S8 | 🔌 Hardware bring-up SH-RS09B | ☐ | | Needs the adapter + a fixture |
| S9 | ArtNet | ☐ | | |
| S10 | sACN (E1.31) | ☐ | | |

### Phase 3 — Core state
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S11 | Show model, command application | ☐ | | |
| S12 | Session state (D11) | ☐ | | |
| S13 | Programmer state machine | ☐ | | |
| S14 | Oops journal | ☐ | | |
| S15 | SQLite persistence | ☐ | | |

### Phase 4 — IPC and daemon
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S16 | `prism-ipc` framing and transports | ☐ | | |
| S17 | `prismd` daemon binary | ☐ | | |
| S18 | D2 gate — resilience | ☐ | | Mandatory gate |

### Phase 5 — Surface
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S19 | MCU codec | ☐ | | Against unverified tables — see §5 |
| S20 | 🔌 Hardware verification X-Touch | ☐ | | Needs the console |
| S21 | Surface model and feedback | ☐ | | |
| S22 | Bindings + D11 gate | ☐ | | Mandatory gate |

### Phase 6 — User interface
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S23 | UI foundation | ☐ | | |
| S24 | Telemetry channel | ☐ | | |
| S25 | Canvas, windows, views | ☐ | | |
| S26 | Executor bar, encoder bar, console | ☐ | | |
| S27 | Patch and fixture sheet | ☐ | | |
| S28 | Sequences, cues, presets | ☐ | | |
| S29 | `prism-app` Tauri shell | ☐ | | Needs MSVC Build Tools |

### Phase 7 — Extended features
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S30 | 3D viewer | ☐ | | |
| S31 | Web Remote | ☐ | | |
| S32 | PSN / OSC — openfollow.app | ☐ | | |

**Done:** 4 / 33 · **In progress:** 0 · **Blocked:** 0

### 2.1 S0 verification record

Measured on 2026-08-10, all exit criteria from `IMPLEMENTATION_PLAN.md` S0:

| Check | Result |
|---|---|
| `cargo build --workspace` | ✅ exit 0, 8 crates |
| `cargo test --workspace` | ✅ exit 0 — 8 test binaries + 6 doc-test targets, 0 tests (expected at this stage) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| `cargo build --workspace --release` | ✅ exit 0 — confirms `panic = "unwind"`, `lto = "thin"`, `codegen-units = 1` are valid |
| `cargo run -p prismd` | ✅ exit 0 — daemon binary links and runs |
| `npx tsc -b --force` (ui, strict) | ✅ exit 0 |
| `npm run build` (ui) | ✅ exit 0 |
| CI green on a pushed branch | ✅ run 31346581991, all four jobs passed |

### 2.2 S1 verification record

Measured on 2026-08-10, all exit criteria from `IMPLEMENTATION_PLAN.md` S1:

| Check | Result |
|---|---|
| `cargo test -p prism-domain` | ✅ exit 0 — **133 tests**, 0 failed |
| `cargo test --workspace` | ✅ exit 0 — 14 test targets green |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| `npx tsc -b --force` in `ui/` (strict) | ✅ exit 0 — all 47 generated files in the build root set |
| `npm run build` in `ui/` | ✅ exit 0 |
| No `any` in the generated bindings | ✅ asserted per line by `export::tests::no_generated_binding_contains_any`; the detector itself is unit-tested |
| Round-trip property, every type | ✅ 46 proptest properties, JSON **and** MessagePack, plus a guard asserting every exported binding has one |
| CI green on the pushed commit | ✅ run **31384082300** on `1ab1fac` — all four jobs, zero non-success steps: Windows full 2 m 47 s, UI typecheck and build 1 m 20 s, Linux neutral 1 m 4 s, ARM64 cross-check 39 s |

**Delivered:** 47 exported types across 13 modules — 10 newtype IDs, the full
`ARCHITECTURE_SPEC.md` §6 model, and the `Command` (23 variants) and `Delta`
(7 variants) wire enums from `docs/IPC_PROTOCOL.md` §5–6. Bindings are generated
by `prism_domain::export_bindings`, which the test suite runs, so a stale binding
cannot survive a green test run.

### 2.3 S2 verification record

Measured on 2026-08-10, all exit criteria from `IMPLEMENTATION_PLAN.md` S2:

| Check | Result |
|---|---|
| `cargo test -p prism-engine` | ✅ exit 0 — **77 tests**, 0 failed, 4 `#[ignore]`d (see §3.1) |
| `cargo test --workspace` | ✅ exit 0 — 210 tests across 17 targets |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| 10-minute run: no missed tick | ✅ **26 401 of 26 401 ticks, 0 missed, 0 panics** |
| 10-minute run: p99.9 jitter < 2 ms | ✅ **p99.9 = 200 µs**, max 588 µs, p50 and p99 both in the first 100 µs bucket |
| Zero allocations in the tick after warm-up | ✅ **0 allocator calls** over 2 000 ticks at 64 universes, 4 subscribers, 4 000 commands — counted by the allocator itself, and again over 44 ticks on the real clock |
| Triple buffer: concurrent stress, no torn frames | ✅ 1 000 000 frames of 64 universes to 4 reader threads in 15.8 s, no torn frame and no sequence regression. Plus **3 `loom` models** covering the buffer's ownership protocol and the queue |
| Drift: < one tick period after 100 000 ticks | ✅ simulated clock with up to 900 µs of random overshoot per tick ends within one period of ideal, and the error does not grow with the tick count. Deadline arithmetic is exact to under 1 µs at 1, 100, 10 000, 100 000 and 1 000 000 ticks |

**Delivered:** `DmxFrame`/`FrameLayout`, a wait-free triple buffer, a wait-free
SPSC command queue, `TickCommand`, the `Clock` abstraction with a real and a
simulated implementation, a fixed-bucket jitter histogram, and the `Engine` tick
loop with `TickBody` as the seam S3-S6 plug into. No `unsafe`, no
`#[cfg(target_os = …)]`, no dependency beyond `prism-domain`.

**The `loom` models are known to work, not merely to pass.** Both were run against
deliberately weakened memory orderings (`Relaxed` in place of `AcqRel`/`Release`)
and both failed, as they should. That matters because CI runs tests only on
x86-64, whose hardware would hide a missing `Release` — and D10 puts a Raspberry
Pi on the roadmap.

**The ten-minute run needs the thread priority the specification already asks
for.** `ARCHITECTURE_SPEC.md` §3 gives `engine-tick` realtime or high priority.
At the shell's default priority the same run missed 45 ticks with a p99.9 of
54 ms; at high priority, nothing else changed, it missed none with a p99.9 of
200 µs. See the decision log and §3.1.

### 2.4 S3 verification record

Measured on 2026-08-10, all exit criteria from `IMPLEMENTATION_PLAN.md` S3:

| Check | Result |
|---|---|
| `cargo test -p prism-engine` | ✅ exit 0 — **135 lib tests** (64 of them new) + 7 integration tests, 0 failed, 4 `#[ignore]`d |
| `cargo test --workspace` | ✅ exit 0 — 268 tests across 17 targets |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| `DMX_MERGE.md` §6.1 — HTP commutative, associative, idempotent, monotone, identity | ✅ five `proptest` properties on `merge_htp`, plus `raising_one_source_never_lowers_an_htp_slot` for monotonicity of the whole layer |
| §6.2 — LTP order-dependent, asserted against a shuffled input | ✅ `ltp_is_the_value_of_the_most_recently_activated_source` rotates the slice with the counters preserved and demands the same answer |
| §6.2 — LTP **not** commutative, asserted | ✅ `ltp_is_not_commutative` **and** `ltp_is_not_a_maximum`, so turning LTP into a max fails two properties, not none |
| Deactivation fallback down to home | ✅ tested at all three levels: the pure function, the layer, and the §7 cascade |
| `DMX_MERGE.md` §7 worked example, literal | ✅ `the_worked_example_from_the_specification` — every number in the table, both quoted DMX byte pairs, and all four sentences of the closing paragraph |
| Zero allocations in the tick **with the merge active** | ✅ **0 allocator calls** over 1 000 ticks, 768 slots, 8 fully-loaded sources, executors going on and off, counted by the allocator itself |
| `loom` models still pass | ✅ 3 models, unchanged from S2 |
| Coverage on `prism-engine` | ✅ **99.14 % lines**, 99.29 % regions, 98.46 % functions — `merge.rs`, `playback.rs` and `body.rs` at 100 % lines |
| CI green on the pushed commit | ✅ run **31412972231** on `0f7b753` — all four jobs, zero non-success steps, no annotations: Windows full build and test 2 m 13 s, Linux platform-neutral 54 s, UI typecheck and build 51 s, ARM64 cross-check 25 s |

**Delivered:** four modules. `merge` is the arithmetic — `merge_htp`, `merge_ltp`,
`apply_master`, `merge_playbacks`, `merge_programmer` — pure, `const` where it
can be, with no state and no clock. `plan` is the home layer: `MergePlan`
flattens the patch into one `AttributeSlot` per fixture and attribute. `playback`
is the source set and the resolver. `body` joins them onto `TickBody`. One new
`TickCommand` variant, `SetExecutorActive`, drives the activation counter.

**The merge produces attribute values, not DMX bytes.** `MergeBody::render`
resolves into a `[u16]` and leaves the frame alone; the encoding is S4. Until
then a `prismd` built on `MergeBody` outputs a blackout, which is why S4 is the
next session rather than S5.

---

## 3. Coverage tracking

Targets from `CLAUDE.md`: ≥ 85 % global, > 95 % on engine, programmer and protocols. Record **measured** figures only — leave blank until a run produces a number.

Measured with `cargo llvm-cov` 0.8.7 (installed 2026-08-10, `llvm-tools-preview`).
Command: `cargo llvm-cov -p <crate> --summary-only`.

| Crate | Target | Measured | Date |
|---|---|---|---|
| `prism-domain` | ≥ 85 % | **99.77 % lines**, 97.86 % regions, 100 % functions | 2026-08-10 |
| `prism-engine` | **> 95 %** | **99.14 % lines**, 99.29 % regions, 98.46 % functions | 2026-08-10 (S3) |
| `prism-core` | **> 95 %** (programmer) | — | |
| `prism-protocols` | **> 95 %** | — | |
| `prism-surface` | **> 95 %** | — | |
| `prism-ipc` | ≥ 85 % | — | |
| `ui` | ≥ 85 % | — | |

### Performance gates

| Gate | Requirement | Measured | Date |
|---|---|---|---|
| Tick jitter | p99.9 < 2 ms, 64 universes, 10 min | **p99.9 = 200 µs**, p50 and p99 ≤ 100 µs, max 588 µs, **0 of 26 401 ticks missed** — at the thread priority `ARCHITECTURE_SPEC.md` §3 specifies. See the note below | 2026-08-10 |
| Tick jitter under 100 % CPU load | S6 stress gate — not this session | — | |
| Tick allocations | zero inside the tick after warm-up | **0 allocator calls** in 2 000 ticks, 64 universes, 4 subscribers; and **0** in 1 000 ticks with the merge active — 768 slots, 8 loaded sources, executors switching | 2026-08-10 |
| Tick drift | < one tick period after 100 000 ticks | within one period, and the error does not grow with the tick count | 2026-08-10 |
| Triple buffer integrity | no torn frame under concurrent load | 1 000 000 frames × 64 universes → 4 readers, clean; 3 `loom` models | 2026-08-10 |
| Open DMX frame rate | measure real rate on SH-RS09B | — | |
| Telemetry render | 64 universes @ 30 Hz, zero React re-renders | — | |

**Thread priority is part of the tick jitter figure.** The same ten-minute run at
the shell's default priority missed 45 ticks and had a p99.9 of 54 ms. The engine
is not the difference: a three-minute run with *no* subscriber threads at all
still stalled every twenty seconds or so, which is the Windows scheduler
preempting a normal-priority thread. `prism-engine` cannot set its own priority —
it is platform-neutral by rule — so **S17 must raise the tick thread's priority in
`prismd`**, and the figure above is measured in that configuration.

Measured on: Windows 11 26200, Rust 1.97.1 msvc, release profile, engine plus four
subscriber threads polling at 5 ms, machine otherwise idle but not quiesced.

### 3.1 Running the long tests

Four tests are `#[ignore]`d because they take minutes, not seconds. A criterion
nobody can re-run is not a measurement, so this is how.

Everything that is not `#[ignore]`d, which is what CI runs:

```bash
cargo test --workspace
```

The long ones — the ten-minute deadline run, the diagnostic without subscribers,
the full-size buffer stress and the real-clock drift run (that last is 38 minutes
on its own):

```bash
cargo test -p prism-engine --release -- --ignored --nocapture --test-threads=1
```

To reproduce the **recorded** jitter figures, the tick needs the priority §3 gives
it. Build first, then run the test binary in a high-priority process:

```powershell
cargo build -p prism-engine --release --tests
$exe = (Get-ChildItem target\release\deps\realtime-*.exe |
        Sort-Object LastWriteTime -Descending)[0].FullName
$p = Start-Process $exe -ArgumentList "--ignored","--nocapture","--test-threads=1",`
       "the_tick_holds_its_deadline_for_ten_minutes" -PassThru -NoNewWindow
$p.PriorityClass = "High"
```

The `loom` models replace the standard atomics with instrumented ones, so they are
a different build of the crate rather than a different test. Under `--cfg loom`
the ordinary unit tests are compiled out, so this runs the models and nothing else:

```powershell
$env:RUSTFLAGS = "--cfg loom"; cargo test -p prism-engine --release --lib
```

---

## 4. Blockers

*None. All setup blockers resolved.*

### B2 — CI run not yet verified ✅ RESOLVED 2026-08-10
The repository is private, so the unauthenticated GitHub API returned 404 and run status could not be read from the shell. Resolved by installing the `gh` CLI and the user authenticating it. The workflow was correct on its first run; the only change needed was bumping `actions/checkout` and `actions/setup-node` off the deprecated Node 20 runtime. Final verification: run **31346581991** on HEAD, all four jobs green — Windows full build and test (55 s), Linux ARM64 cross-compile check (20 s), UI typecheck and build (15 s), Linux platform-neutral crates (14 s), with no annotations.

### B1 — MSVC Build Tools missing ✅ RESOLVED 2026-08-10
Rust could be installed per-user via winget, but the MSVC linker could not — `link.exe` was absent and every link step failed. Resolved by the user installing Visual Studio Build Tools 2022 (17.14.37516.0) from an elevated shell. Verified: `cargo build`, `cargo test` and `cargo run -p prismd` all succeed.

---

## 5. Open verification items

Both are recorded as plain data so verification is a data update, not a refactor. See `ARCHITECTURE_SPEC.md` §14.

| Item | Blocks | Status |
|---|---|---|
| MCU note and CC numbers vs. real X-Touch | S20, and sign-off of S19 | ☐ unverified — banner in `docs/MCU_MAPPING.md` §2 |
| SH-RS09B USB VID/PID and real frame rate | S8 | ☐ unverified — estimate in `ARCHITECTURE_SPEC.md` §7.1 |

---

## 6. Decision log

Architectural decisions D1–D11 are in `ARCHITECTURE_SPEC.md` §1. This log records **changes and discoveries made during implementation** — things that turned out differently from the plan.

| Date | Session | Finding | Consequence |
|---|---|---|---|
| 2026-08-10 | S3 | **"LTP is order-dependent" and "LTP is not commutative" are two different claims, and only one of them is about the merge.** Ordering a source *set* by activation counter is, correctly, independent of the order of the list — shuffling the input must not change the answer. The non-commutativity `DMX_MERGE.md` §6.2 demands lives one level down, in the binary operation "the later one wins" | Split into two functions and two properties. `merge_ltp(a, b) = b` carries the non-commutativity and is asserted against it; `merge_playbacks` carries the ordering and is asserted to be shuffle-invariant. A second property, `ltp_is_not_a_maximum`, fails as well if anyone replaces the operation with `max` — the failure the specification is really trying to prevent |
| 2026-08-10 | S3 | **Equal activation counters would make LTP depend on slice order.** Counters are unique by construction, so the case cannot arise from the layer — but `merge_playbacks` is public and takes any slice, and "usually deterministic" is not a property worth having in the one function the whole product resolves through | The LTP key is `(activation, executor)`, a total order on any source set at all. Costs nothing, and makes shuffle-invariance a theorem rather than a convention |
| 2026-08-10 | S3 | **The resolver cannot be a per-slot fold without wasting the tick.** Asking every source about every slot costs `slots × sources` every tick whether anything is running or not — at 8 000 attributes and 64 executors, half a million operations per tick to produce a rig at home | Source-major instead: each source is asked only for the slots it touches, and the per-slot winner accumulates in a scratch buffer. The cost is what the active cues contain. The price is that the accumulator exists, so `PlaybackLayer::resolve` takes `&self` and the scratch is passed in — purity kept in the signature rather than in a comment — and a `proptest` holds the resolver to agreeing with the pure `merge_playbacks` on every slot |
| 2026-08-10 | S3 | **`Accumulator` needs a "covered" flag; a sentinel value will not do.** An HTP slot whose only source contributes 0 must resolve to 0, not to its home value. Without a third state the merge cannot tell "resolved to zero" from "nothing active", which is the difference between a dark fixture and a fixture at home — and for a moving head, between a centred pan and pan 0 | One bool per slot. Recorded because it is the sort of thing a later optimisation removes |
| 2026-08-10 | S3 | **The §7 worked example is an S3 exit criterion but its last two rows are the programmer, which the plan gives to S6.** Leaving them out would have meant closing the session on a partial test of a criterion that names the example explicitly | `merge_programmer(merged, Option<u16>)` implemented here: five lines, pure, no state. The programmer *state machine* — `ProgrammerValue`, the three-stage clear, store behaviour — is untouched and remains **S6**, as do all masters. The §7 test asserts the two DMX byte pairs the document quotes as arithmetic on the 16-bit value, without pre-empting S4's encoder |
| 2026-08-10 | S3 | An executor that is already active and is switched on again does **not** get a new activation stamp. §2.2 orders by when an executor "goes active", and one that was already on has not gone active | `PlaybackLayer::activate` is idempotent and returns `false` for a source already on. **S5 requirement:** if a retrigger is ever meant to move a playback to the top of the LTP order, that is a separate operation with its own name, not a side effect of pressing a button twice |
| 2026-08-10 | S3 | `MergeBody::render` writes **nothing** to the frame. Attribute values are 16-bit and the split into DMX bytes, the two levels of invert and the patch-time address validation are all S4 | Recorded rather than papered over with a provisional encoder: half an encoder that S4 then replaces is worse than none, and a body that silently wrote coarse bytes only would look like it worked. **S4 requirement:** the encoder consumes `MergeBody::values()` against the same `MergePlan`, whose slot order (`fixture`, then `attribute`) is stable and is part of its contract |
| 2026-08-10 | S3 | `TickCommand::SetGrandMaster` and `SetBlackout` reach the merge already but are masters, which `DMX_MERGE.md` §4 applies *after* the merge | Ignored by `MergeBody`, with a test that says so outright. A body that quietly swallowed a blackout would pass every other test in the crate. **S6 requirement:** the masters wrap the merged values, and only intensity attributes |
| 2026-08-10 | S2 | **A `prism_domain::Command` cannot enter the tick at all.** It owns `String`s, `Vec`s and `JsonValue`s, so *dropping* one inside the tick calls the allocator — which §3.1 forbids exactly as firmly as allocating does, and which is the easier mistake to make because nothing in the code looks like an allocation | The queue carries `TickCommand`: flat, `Copy`, no owned fields, encoded into a fixed 16-byte slot. The core thread resolves a `Command` against the show and pushes the flat form. **S17 requirement:** `prismd` owns that translation, and anything that cannot be expressed flatly does not belong in the engine. A `const` assertion fails the build if a later session's variant outgrows the slot |
| 2026-08-10 | S2 | **A triple buffer with one control word is single-consumer, not multi-consumer.** The plan says "one writer, many readers". With two readers swapping against the same word, one can be handed the slot the other has just given back — an *older* frame than it has already put on the wire. Making that safe needs a retry loop, and a retry loop is not wait-free | Each subscriber gets its own three-slot buffer and the publisher fans the frame out. Costs one copy per subscriber per tick (32 KB at 64 universes); buys that *every* buffer in the system is genuinely single-producer/single-consumer, which is a property `loom` can actually check. Measured cost recorded in §3.1 |
| 2026-08-10 | S2 | `unsafe_code` is denied workspace-wide, so a shared slot cannot be an `UnsafeCell` — and `prism-engine` is the last crate in which to start making exceptions | Slots are `[AtomicU64]`, and frames are packed in and out of them eight channel bytes at a time. The ownership protocol already guarantees exclusive access, so every payload access is `Relaxed` and the control-word swap carries the ordering. What the atomics buy is that the worst case of a protocol bug is a stale frame, never undefined behaviour |
| 2026-08-10 | S2 | **The `loom` models cannot run a real frame.** loom explores every interleaving of every atomic access, and a 64-universe frame is 4097 words per slot | The ownership protocol was extracted into `Slots::publish` and `Slots::take`, which the models drive **unchanged** against a two-word slot — two words being the smallest number that can tear. Validated by deliberately weakening the orderings: with `Relaxed` in place of `AcqRel` both models fail, so they are checking something. This matters for the Raspberry Pi target (D10): CI runs tests only on x86-64, whose hardware would hide a missing `Release` |
| 2026-08-10 | S2 | **1/44 s is not a whole number of nanoseconds.** A rounded 22 727 272 ns period loses 0.73 ns per tick — 73 µs over 100 000 ticks, which is small but is drift, and drift is the one thing this session exists to exclude | Deadlines are derived from the tick index as an exact rational (`index × 10⁹ / 44`), so the error is under a nanosecond at every tick and never accumulates. `TICK_PERIOD` survives as a nominal constant for budgets and display, and is documented as such |
| 2026-08-10 | S2 | **A p99.9 over a three-second run is just the worst sample.** The short CI timing test failed about one run in four on an idle machine — not because of a regression but because 130 samples cannot carry a 99.9th percentile, and one scheduler stall decided it | The CI run asserts on the median and p95, which is where a schedule that drifts or sleeps by period rather than to a deadline shows up, and prints the whole distribution. The tail gate lives in the ten-minute run, where 26 400 samples make a p99.9 mean something. A gate that fails for reasons unrelated to the code teaches people to ignore gates |
| 2026-08-10 | S2 | **The ten-minute criterion failed on the first run: 45 missed ticks, p99.9 jitter 54 ms.** The cause is not the engine. A three-minute run with *no* subscriber threads at all still missed 14 ticks with a 51 ms worst case, so nothing of ours was responsible; and the same run at high process priority missed none, with a p99.9 of 200 µs. It is the Windows scheduler preempting a normal-priority thread, roughly every twenty seconds | `ARCHITECTURE_SPEC.md` §3 already gives `engine-tick` realtime or high priority, and `prism-engine` cannot set it — platform-neutral by rule, and priority is a per-OS call. **S17 requirement:** `prismd` raises the tick thread's priority at start-up, and treats failing to do so as a degraded state worth reporting, not a silent one. The recorded figure is measured in that configuration and §3.1 says how to reproduce it. The diagnostic that answers "is it us or the operating system?" is kept as `the_tick_alone_holds_its_deadline_for_ten_minutes` |
| 2026-08-10 | S2 | The first harness had four subscriber threads polling every 500 µs — eight thousand wake-ups a second, contending with the tick for the same cores. That models nothing: a driver consumes a 44 Hz stream | Poll interval raised to 5 ms, about four times the frame rate. It removed the missed ticks entirely at normal priority (45 → 0 over the equivalent period), which is worth knowing in its own right: **S7 driver threads should wake at their own output cadence, not spin near the engine**. Tuning a test until it passes is a bad habit, so both figures are recorded above rather than only the better one |
| 2026-08-10 | S2 | A panic in the tick body leaves the frame half written, so containing the panic is not enough on its own | `Engine::tick` runs the body in `catch_unwind` and, on a panic, does **not** publish. Every output holds its last good frame for 23 ms, which is what a DMX receiver does with a stalled line anyway — far smaller a fault than one garbled frame reaching the fixtures. Counted in `TickStats::panics` |
| 2026-08-10 | S2 | `FramePublisher::subscribe` allocates, so an output driver cannot be attached while the tick is running | Documented on the method. **S7/S17 requirement:** drivers are attached during setup; hot-plugging an interface at run time needs a different mechanism, not a call into this one |
| 2026-08-10 | S0 | Rust installs cleanly per-user via winget, but the MSVC linker does not — they are separate installs | Setup split into an automatic part and a manual elevated part; recorded as B1 |
| 2026-08-10 | S0 | `cargo check` and `cargo clippy` do not link | Workspace scaffold can be verified before B1 is resolved; test-driven work cannot start until it is |
| 2026-08-10 | S0 | The first CI run passed but flagged `actions/checkout@v4` and `actions/setup-node@v4` as targeting the deprecated Node 20 runtime | Both bumped to `@v5` immediately, while the workflow was still trivial to re-verify |
| 2026-08-10 | S0 | The remote already carried an MIT `LICENSE` from repository creation, while the workspace manifest declared `license = "UNLICENSED"` | Rebased onto the existing commit instead of overwriting it; manifest corrected to `MIT` in a separate commit |
| 2026-08-10 | S0 | The CI workflow cannot be validated without a remote, so a green local build is not the same as a green pipeline | Tracked as B2 rather than silently assuming CI works. S0 closed with this criterion explicitly unmet |
| 2026-08-10 | S1 | **Non-finite floats had no safe wire representation.** JSON cannot encode NaN or infinity — `serde_json` silently writes `null`, and the file then fails to reopen. MessagePack encodes both faithfully, so an infinite fade time would survive the IPC wire intact and produce a cue that never completes | Every `f64` field is guarded by `crate::finite` in **both** directions: a non-finite value is refused when written and when read. A domain value now either round-trips exactly or fails loudly. `prism-ipc` and the show writer must treat serialisation as fallible |
| 2026-08-10 | S1 | `ExecutorId::from_page_and_slot` multiplied an unchecked `u32` page by 8. `SetExecutorPage` carries an arbitrary `u32` from any client, and a release build would have **wrapped silently** — addressing a low executor mid-show | Saturating arithmetic. An out-of-range page stays out of range instead of aliasing a real executor |
| 2026-08-10 | S1 | `JsonValue` is recursive, so a hostile payload can exhaust the stack while decoding — and a stack overflow aborts the process, taking DMX with it. It **cannot** be guarded in this crate: serde buffers the content of an internally tagged enum before any of our code runs, so the recursion has already happened by then | Documented on `JsonValue`. **S16 requirement:** `prism-ipc` must enforce a nesting depth limit on decode, alongside the maximum frame size. `serde_json` has one (128 levels); MessagePack does not |
| 2026-08-10 | S1 | Evaluated storing a fixture's DMX parameter channels in `PatchFixture`. **Rejected.** The channel layout follows from the `FixtureType` that `type_id` names; a client computing it and expecting the daemon to accept it is what D3 forbids, and a duplicated derived value drifts from the profile | The real risk behind the question — a profile library changing under a saved show — is addressed in the show model instead. **S11 requirement:** the show file embeds the fixture types it uses rather than referencing an external library, so a show is self-contained. Reasoning recorded on `Command::PatchFixture` |
| 2026-08-10 | S1 | An **integer-keyed map inside an internally tagged enum does not round-trip**. `Delta::ProgrammerChanged` buffers its content before deserialising (that is how `#[serde(tag = "t")]` works), and the buffered form no longer knows that a JSON object key `"1"` should be read back as a number. `ProgrammerState` alone round-tripped; inside the delta it failed | `ProgrammerState.values` stays a nested `BTreeMap` in Rust — the merge indexes it every tick — but travels as a flat `Vec<ProgrammerEntry>`. That is also what a TypeScript `Map` is constructed from, so §6's `Map<…>` shape is preserved client-side. Found by the round-trip property, not by review |
| 2026-08-10 | S1 | Consequence of the same discovery: a fixture mapped to an **empty** attribute map has no representation in the flat list, so it comes back pruned | Documented as an invariant on `ProgrammerState.values` and enforced by `set_value`/`clear_value`, which prune a fixture when its last attribute is cleared. S13 must go through them rather than touching the map directly |
| 2026-08-10 | S1 | **`serde_json`'s float parser is not correctly rounded.** It writes the correct shortest text for an `f64` — the standard library reads that text back exactly — but its own `from_str` returns a neighbouring value, one ULP off. MessagePack, being binary IEEE 754, is exact | Pinned by `wire::json_floats_are_only_exact_to_bounded_precision`, and the proptest strategies generate floats at millithousandth precision so the round-trip property stays an exact equality. **Consequence for S15:** a JSON-exported show must not be assumed bit-identical in its floats unless values are written at a bounded precision |
| 2026-08-10 | S1 | `SetAttribute { value }` is a plain `number` in `docs/IPC_PROTOCOL.md` §5, but the same command carries `relative: true` for encoder movement, and an encoder turns both ways | Typed as `i32`, not `u16`: absolute values are `0..=65535`, relative values are signed deltas. Documented on the variant |
| 2026-08-10 | S1 | `PatchFixture` is `{ /* … */ }` in the protocol spec — left open | Defined as `id`, `name`, `typeId`, `universe`, `address`, matching the `Fixture` fields an operator supplies at patch time. Geometry and inverts are edited afterwards, not at patch |
| 2026-08-10 | S1 | Internal tagging (`t`) also constrains the codec: MessagePack's default array encoding of structs cannot carry a tag | `prism-ipc` must use `rmp_serde::to_vec_named`. Recorded in the crate docs and asserted by every round-trip property |
| 2026-08-10 | S1 | `ui/src/bindings/` is gitignored (S0 decision), so the UI CI job had nothing to typecheck and would have passed vacuously | The `ui` job now installs Rust and runs `cargo test -p prism-domain` to generate the bindings before `tsc`. Keeping them out of git means drift is structurally impossible rather than merely checked |
| 2026-08-10 | S0 | The Vite `react-ts` template does **not** set `"strict": true` — the default is `false`, so the template ships non-strict TypeScript despite its name | `CLAUDE.md` violation caught before any code was written. `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride` and `noImplicitReturns` added to both `tsconfig.app.json` and `tsconfig.node.json`; typecheck and build re-verified green |

---

## 7. Next actions

The merge resolves. Begin **S4** (`prism-engine` — attribute to DMX encoding). Use the prompt in §8. The seam is `MergeBody::render`, which resolves values and leaves the frame untouched.

Carried into S4 and beyond:
- The encoder consumes `MergeBody::values()` — one 16-bit value per `MergePlan` slot, in slot order (`fixture`, then `attribute`). That order is stable and part of the plan's contract.
- `MergePlan` deliberately knows nothing about addresses: `Fixture.universe`, `Fixture.address`, `AttributeDef.coarse_offset`/`fine_offset`/`invert` and `Fixture.invert_pan`/`invert_tilt` are all S4's, as is rejecting a footprint that runs past channel 512.
- `PlaybackLayer::source_mut(..).set(slot, value)` and `.clear()` are how **S5** feeds evaluated cue values in; `activate`/`deactivate` maintain the LTP order. `activate` is idempotent — a retrigger that should move a playback to the top of the order needs its own operation.
- Masters and the programmer state machine are **S6**. `SetGrandMaster` and `SetBlackout` reach `MergeBody` today and are ignored on purpose.
- `prism-domain`'s optional `proptest` feature is already enabled in `prism-engine`'s `[dev-dependencies]`. Use `prism_domain::arb` rather than growing new generators.
- `TickCommand` is flat, `Copy` and encoded into 16 bytes; S3 added `SetExecutorActive` as tag 5. A new variant needs a new tag, a row in the round-trip test and, if it is wider, a raised `MAX_ENCODED` — a `const` assertion breaks the build otherwise.
- `prismd` (S17) owns the translation from `prism_domain::Command` to `TickCommand`, must attach every output driver during setup (`FramePublisher::subscribe` allocates), and **must raise the tick thread's priority** — without it the deadline is not held, see §3.
- `prism-protocols` (S7): a driver thread should wake at its own output cadence rather than spin near the engine — see the decision log.
- `prism-ipc` (S16) must serialise MessagePack with `to_vec_named`, must enforce a **nesting depth limit** on decode, and must treat serialisation as fallible — see the decision log.
- `prism-core` (S11) must embed the used fixture types in the show file — see the decision log.
- S15 must not assume bit-identical floats through a JSON export — see the decision log.

---

## 8. Follow-up prompt for the next session

> Rewritten at the close of every session, per `IMPLEMENTATION_PLAN.md`. Written to be **self-contained**: it assumes no loaded context, no memory of previous conversations and no knowledge of the project. Paste it into a fresh session to continue.

**Next up: S4 — `prism-engine`: Attribut-zu-DMX-Encoding**

```text
PrismDMX — Session S4: prism-engine, Attribut-zu-DMX-Encoding

Projektverzeichnis: C:\Users\Milan\Prismdmx

Bitte lies zuerst in dieser Reihenfolge, bevor du irgendetwas änderst:
1. CLAUDE.md                        — verbindliche Qualitäts-, Architektur- und Teststandards
2. PROGRESS.md                      — aktueller Stand, Decision Log, gemessene Zahlen
3. IMPLEMENTATION_PLAN.md           — Session-Protokoll und die Definition von S4
4. docs/DMX_MERGE.md §5             — die Spezifikation dieser Session; §1 für den Stack darüber
5. ARCHITECTURE_SPEC.md §3.1, §5    — die harten Tick-Regeln und die Pipeline-Reihenfolge
6. crates/prism-engine/src/lib.rs   — die Crate-Doku erklärt Aufbau, Tick-Vertrag und die Merge-Schichten
7. crates/prism-engine/src/plan.rs  — MergePlan: die Slots, die S4 in Kanäle übersetzt
8. crates/prism-engine/src/body.rs  — MergeBody: render() löst Werte auf und lässt den Frame in Ruhe
9. crates/prism-domain/src/attribute.rs und src/patch.rs — AttributeDef, FixtureType, Fixture

Aufgabe: Session S4 umsetzen — das Encoding. Aus den gemergten Attributwerten
(16 Bit, 0..=65535, einer pro MergePlan-Slot) werden DMX-Kanalbytes im DmxFrame.
Das ist der Schritt, nach dem die Kette Patch → Merge → Frame vollständig ist und
ein Daemon tatsächlich Licht machen könnte.

Stand nach S3 — nichts davon musst du neu bauen:
- Der Merge ist fertig und getestet: MergePlan (Home-Layer), PlaybackLayer
  (HTP/LTP, Aktivierungsordnung, Executor-Master), MergeBody als TickBody.
- `MergeBody::render` löst pro Tick auf und füllt `MergeBody::values()`:
  ein `[u16]`, ein Wert pro Slot, in Slot-Reihenfolge (Fixture, dann Attribut).
  Diese Reihenfolge ist stabil und Teil des Vertrags von MergePlan.
- Der Frame bleibt bisher unberührt. Genau diese Lücke schließt S4.
- MergePlan kennt bewusst keine Adressen. Universum, Startadresse, coarse/fine
  Offsets und beide Invert-Ebenen sind Sache dieser Session.

Umzusetzen (docs/DMX_MERGE.md §5):
- 8-Bit-Attribut (`fineOffset == null`): data[address + coarseOffset] = value >> 8
- 16-Bit-Attribut: coarse = value >> 8, fine = value & 0xFF, geschrieben an
  coarseOffset und fineOffset
- `AttributeDef.invert`: value = 65535 - value, VOR dem Aufteilen
- `Fixture.invertPan` / `invertTilt`: zusätzlich zum Attribut-Invert, damit ein
  über Kopf hängender Scheinwerfer ohne Änderung des Fixture-Typs stimmt
- Adressarithmetik wird beim Patchen validiert, nie im Tick: ein Fixture, dessen
  Footprint über Kanal 512 hinausginge, wird beim Patchen abgelehnt. Dafür gibt
  es bereits `Fixture::last_address(footprint)` in prism-domain.
- Die Anbindung an den Tick: `MergeBody::render` schreibt danach den Frame,
  nicht mehr nur die Werte.

Vorgehen strikt test-driven (CLAUDE.md): erst der fehlschlagende Test, dann die
Implementierung. Diese Crate trägt die strengste Coverage-Anforderung des
Projekts (> 95 %); nach S3 gemessen wurden 99,14 % Zeilen. Abgenommen wird die
Coverage in S6, aber sie darf hier nicht fallen.

Exit-Kriterien — die Session gilt erst als fertig, wenn diese wirklich zutreffen:
- Tabellentests für 8-Bit- und 16-Bit-Encoding, inklusive der Grenzwerte
  0, 1, 32767, 32768, 65534, 65535
- Invert-Komposition getestet: Attribut-Invert × Fixture-Invert in allen vier
  Kombinationen, und dass doppeltes Invertieren die Identität ist
- Ein Fixture, dessen Footprint über Kanal 512 hinausgeht, wird BEIM PATCHEN
  mit einer klaren Fehlermeldung abgelehnt — nicht im Tick
- Ein Fixture in einem Universum, das nicht im FrameLayout steht, wird ebenso
  beim Patchen abgelehnt
- Null Allokationen im Tick mit aktivem Merge UND Encoding
  (crates/prism-engine/tests/tick_allocations.rs erweitern, nicht ersetzen)
- Die Kette Patch → Merge → Frame als Integrationstest: der Home-Layer ergibt
  bestimmte Bytes, ein aktiver Executor ändert genau die erwarteten Kanäle
- cargo test -p prism-engine ist grün
- cargo clippy --workspace --all-targets -- -D warnings ist sauber
- cargo fmt --all --check ist sauber

Wichtige Randbedingungen:
- prism-engine ist plattformneutral und I/O-frei — kein #[cfg(target_os = ...)],
  keine UI-Abhängigkeit, keine Hardware. CI testet die Crate auch unter Linux.
- Im Tick gilt ARCHITECTURE_SPEC.md §3.1: keine Allokation, kein Lock, kein I/O,
  kein Logging. Alle Puffer werden vorab aus dem Patch dimensioniert. Zusätzlich
  sind clippy::indexing_slicing und clippy::integer_division im Produktionscode
  verboten — also `get`/`get_mut` und `div_euclid` statt `[]` und `/`.
- prism-domain hat ein optionales Feature `proptest` (`prism_domain::arb` und die
  `Arbitrary`-Impls). Es ist in den [dev-dependencies] von prism-engine bereits
  aktiviert — benutzen statt eigene Generatoren zu schreiben.
- Interne Testhelfer für Fixture-Typen stehen in crates/prism-engine/src/testkit.rs.
- Lange Tests laufen nicht in CI. Wie man sie ausführt, steht in PROGRESS.md §3.1
  und in der Crate-Doku von prism-engine.
- Toolchain ist eingerichtet und funktioniert (Rust 1.97.1 msvc, MSVC Build Tools 2022,
  Node 24.11). Coverage misst man mit `cargo llvm-cov -p prism-engine --summary-only`.
- Es ist kein Setup mehr nötig.

Zum Abschluss der Session:
- PROGRESS.md aktualisieren: S4-Status, gemessene Coverage, Decision Log bei
  Abweichungen vom Plan oder Funden, die spätere Sessions betreffen
- PROGRESS.md §8 mit einem neuen, ebenfalls kontextfreien Follow-up-Prompt für
  Session S5 (prism-engine: Executoren, Cues, Fades) überschreiben
- Mit Conventional-Commit-Message committen, z. B. feat(engine): …
```
