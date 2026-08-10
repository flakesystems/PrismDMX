# PROGRESS.md — PrismDMX Status Tracker

**Last updated:** 2026-08-11
**Current phase:** Phase 2 — Protocols
**Current session:** S7 — `prism-protocols` `DmxOutput` trait and Open DMX USB (not started; see §8 for the prompt that starts it)
**Last completed:** S6 — `prism-engine` programmer, masters, stress ✅ — **Phase 1 complete**
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
| 13 | CI verified green | ✅ | Latest: run **31439768003** on `2ddd66e` (S6). First verified: run **31346581991** — all four jobs: Windows 55 s, ARM64 check 20 s, UI 15 s, Linux neutral 14 s. No annotations |
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
| S4 | `prism-engine` — DMX encoding | ✅ | 2026-08-10 | All exit criteria verified — see §2.5. 179 tests, coverage 99.3 % lines |
| S5 | `prism-engine` — executors, cues, fades | ✅ | 2026-08-10 | All exit criteria verified — see §2.6. 241 tests, coverage 99.6 % lines |
| S6 | `prism-engine` — programmer, masters, stress | ✅ | 2026-08-11 | All exit criteria verified — see §2.7. 299 tests, coverage 99.6 % lines. Stress gate passed under full CPU load |

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

**Done:** 7 / 33 · **In progress:** 0 · **Blocked:** 0

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

### 2.5 S4 verification record

Measured on 2026-08-10, all exit criteria from `IMPLEMENTATION_PLAN.md` S4 and
the session prompt:

| Check | Result |
|---|---|
| `cargo test -p prism-engine` | ✅ exit 0 — **163 lib tests** (28 of them new) + 16 integration tests, 0 failed, 4 `#[ignore]`d |
| `cargo test --workspace` | ✅ exit 0 — 312 tests across 18 targets |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Table tests, 8-bit and 16-bit, boundaries 0, 1, 32767, 32768, 65534, 65535 | ✅ one `BOUNDARIES` table drives both cases, plus a property that the 8-bit write **is** the coarse byte of the 16-bit one — so the two cannot drift into separate scalings |
| Invert composition, all four combinations | ✅ `the_attribute_invert_and_the_fixture_invert_compose`, with both-inverted asserted equal to neither-inverted; `inverting_twice_is_the_identity` as a property; and `the_fixture_inverts_reach_pan_and_tilt_and_nothing_else` |
| Footprint past channel 512 rejected **at patch time** | ✅ `PatchError::AddressOutOfRange`, asserted through `ChannelPlan::build` and again through `MergeBody::for_patch`, with the "fits exactly at 512" case beside it. Also address 0 and an empty footprint |
| Universe not in the `FrameLayout` rejected at patch time | ✅ `PatchError::UniverseNotPatched`, same two levels |
| Zero allocations in the tick with merge **and** encoding | ✅ **0 allocator calls** over 1 000 ticks — 768 16-bit slots, 128 fixtures over 4 universes, half of them inverted, 8 sources, executors switching, frame published to a driver each tick |
| `Patch → Merge → Frame` as an integration test | ✅ `tests/patch_to_frame.rs`: the home layer listed byte for byte, an active executor asserted to move **exactly** eight channels, deactivation byte-identical to the home frame, HTP-with-master and LTP both checked on the wire |
| `loom` models still pass | ✅ 3 models, unchanged since S2 |
| Coverage on `prism-engine` | ✅ **99.31 % lines**, 99.42 % regions, 98.47 % functions — `encode.rs` and `body.rs` both at **100 % lines**, and the only uncovered production lines in the crate are the four S2 files' pre-existing ones |
| CI green on the pushed commit | ✅ run **31418621481** on `174c27d` — all four jobs, zero non-success steps, no annotations: Windows full build and test 2 m 16 s, Linux platform-neutral 53 s, UI typecheck and build 50 s, ARM64 cross-check 20 s |

**Delivered:** one module, `encode`. `ChannelPlan::build` validates the patch —
universe, address, footprint, offsets, and that the fixture list agrees with the
`MergePlan` — and flattens it into one `ChannelTarget` per slot, carrying the
frame position of the coarse channel, of the fine channel if there is one, and
the two inverts already composed. `ChannelPlan::encode` is what runs on the tick:
a walk over that table, one or two byte writes each. `MergeBody::for_patch`
builds both plans from one patch, so they cannot describe different rigs.

### 2.6 S5 verification record

Measured on 2026-08-10, all exit criteria from `IMPLEMENTATION_PLAN.md` S5 and
the session prompt:

| Check | Result |
|---|---|
| `cargo test -p prism-engine` | ✅ exit 0 — **224 lib tests** (61 of them new) + 17 integration tests, 0 failed, 4 `#[ignore]`d |
| `cargo test --workspace` | ✅ exit 0 — 374 tests across 19 targets |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| A 10-second fade is at exactly 50 % at 5 s, ±1 tick, on a simulated clock | ✅ **32 767 at tick 220**, which is what "65535 × 0.5" gives in `docs/DMX_MERGE.md` §7. Asserted three ways: on the arithmetic, on the tick grid, and through `Engine` + `ManualClock` where the clock reads within one period of 5.000 s. Ticks 219 and 221 are asserted to be one step either side, so the criterion cannot pass on a fade that is merely near the right place |
| Fades interpolate in 16-bit even on 8-bit patched channels | ✅ `tests/cue_playback.rs`: over 440 ticks the coarse byte of an 8-bit patched dimmer **never steps by more than one** and passes through all 256 values. Plus the same fade run against an 8-bit and a 16-bit patch of the same dimmer, asserted byte-identical on the coarse channel at every one of the 441 ticks |
| Go during a running fade is deterministic and tested | ✅ the rule is stated in `player.rs`'s module documentation and asserted at both levels: the value at the instant of the Go **is** the value the running fade had reached (no jump), and the new fade then runs on the new cue's time base, not the old one. On the wire as well as in the merge |
| Cue numbers with decimals order correctly | ✅ `1`, `1.5`, `2`, `10` from a list given in the reverse order, through `Cue::compare_numbers`; an unparsable number sorts last; plus a `proptest` that compiling any list orders it by number |
| Zero allocations in the tick with running fades | ✅ **0 allocator calls** over 1 000 ticks with 8 cue lists loaded, each fading over all 768 slots, cues following on by themselves, Gos and on/off arriving over the queue — and asserted afterwards that values were still moving, so it is not a measurement of a rig at rest |
| `loom` models still pass | ✅ 3 models, unchanged since S2 |
| Coverage on `prism-engine` | ✅ **99.55 % lines**, 99.52 % regions, 99.07 % functions — up from S4's 99.31 %. `cue.rs`, `player.rs` and `body.rs` all at **100 % lines**; every uncovered line in the crate is a pre-existing one in the four S2 files |
| CI green on the pushed commit | ✅ run **31433415055** on `83dc6a1` — all four jobs, zero non-success steps, no annotations: Windows full build and test 2 m 20 s, Linux platform-neutral 1 m 4 s, UI typecheck and build 37 s, ARM64 cross-check 22 s |

**Delivered:** two modules. `cue` is the compiler — `SequencePlan::build` resolves
cue parts against the `MergePlan` into slot indices, converts seconds into whole
ticks, orders cues by `Cue::compare_numbers`, and flattens the result into three
flat tables. `player` is the state machine — `CuePlayer` runs one compiled cue
list on the tick index, and `CueLayer` keeps one player per executor in step with
the `PlaybackLayer`. `MergeBody::load_sequence` compiles a sequence against the
body's own patch, and `TickCommand::Go` now reaches the traversal.

**The playback rules are written down, not implied.** Four of them are choices
rather than deductions from `docs/DMX_MERGE.md`, and all four are in the decision
log below and in `player.rs`'s module documentation: cues track, a Go overtakes a
running fade, a release fades the light out and leaves the rig still, and at most
one cue starts per tick.

### 2.7 S6 verification record

Measured on 2026-08-11, all exit criteria from `IMPLEMENTATION_PLAN.md` S6 and
the session prompt:

| Check | Result |
|---|---|
| `cargo test -p prism-engine` | ✅ exit 0 — **275 lib tests** (51 of them new) + 24 integration tests, 0 failed, 5 `#[ignore]`d |
| `cargo test --workspace` | ✅ exit 0 — 432 tests across 20 targets |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| `DMX_MERGE.md` §6.3 — stack invariants, with `proptest` | ✅ four properties over arbitrary playback state, at body level and on the real pipeline: a programmer value always reaches the output whatever the playbacks do; a desk with nothing running resolves to home on every slot; the grand master at zero zeroes intensity and changes nothing else; the grand master at full changes nothing at all. Plus five more on the master layer alone |
| Grand master at 0 forces every intensity to 0 and leaves every other attribute untouched | ✅ asserted three ways: as a `proptest` over arbitrary values at both levels, as a table test per slot, and on the wire in `tests/pipeline.rs` where a blackout leaves both heads' pan bytes exactly where they were |
| **Stress gate:** 64 universes, 100 % CPU load, 10 min, p99.9 < 2 ms, no dropped frames | ✅ **26 401 of 26 401 ticks, 0 missed, 0 panics**, p99.9 = **200 µs**, p50 and p99 ≤ 100 µs, max 464 µs. The real pipeline: 32 640 slots, 5 440 intensity slots, 8 cue lists fading over every slot, 16 group masters, programmer values arriving and being cleared, 79 616 commands. Four drivers took 26 441–26 442 frames each. Load: eight CPU burners at normal priority, tick process at High — see the decision log for why that configuration and not the obvious one |
| Determinism: identical input → byte-identical frames across runs | ✅ `tests/pipeline.rs`: three runs of the same scripted hundred ticks, compared frame by frame on the bytes a driver receives, with 8 command changes in the script and a guard that the run produced more than 20 distinct frames so it cannot pass by being static |
| Zero allocations in the tick with the programmer and the masters | ✅ **0 allocator calls** over 1 000 ticks — 768 slots, 8 cue lists, 16 group masters moving, programmer values set and cleared on the tick, grand master moving, frame published to a driver. Added beside the four earlier measurements, not instead of them |
| `loom` models still pass | ✅ 3 models, unchanged since S2 |
| Coverage on `prism-engine` | ✅ **99.61 % lines**, 99.53 % regions, 99.22 % functions — up from S5's 99.55 %. `master.rs` at **100 % lines**, `programmer.rs` at 99.62 %, `body.rs` still at 100 % |
| CI green on the pushed commit | ✅ run **31439768003** on `2ddd66e` — all four jobs, zero non-success steps: Windows full build and test 2 m 54 s, Linux platform-neutral 59 s, UI typecheck and build 50 s, ARM64 cross-check 16 s. The first push (**31439289713**) failed on Windows and is recorded in the decision log — two timing tests measuring each other, found by CI and not locally |

**Delivered:** two modules and the wiring that closes the pipeline. `programmer`
is the operator's override — sparse by specification, addressed by merge-plan
slot, allocation-free to set, clear and apply. `master` is the top of the stack:
the grand master, the blackout and one fader per group, scaling intensity and
nothing else. `MergeBody::resolve` now runs `ARCHITECTURE_SPEC.md` §5 steps 4 to
6 in order, `MergeBody::load_groups` and `MergeBody::load_programmer` are the
set-up doors, and four new `TickCommand` variants carry programmer values and
group masters onto the tick. `TickCommand::SetGrandMaster` and `SetBlackout`,
ignored on purpose since S3, now do what they say.

**Phase 1 is complete.** `prism-engine` implements `docs/DMX_MERGE.md` end to
end and holds 44 Hz at 64 universes on a fully loaded machine.

---

## 3. Coverage tracking

Targets from `CLAUDE.md`: ≥ 85 % global, > 95 % on engine, programmer and protocols. Record **measured** figures only — leave blank until a run produces a number.

Measured with `cargo llvm-cov` 0.8.7 (installed 2026-08-10, `llvm-tools-preview`).
Command: `cargo llvm-cov -p <crate> --summary-only`.

| Crate | Target | Measured | Date |
|---|---|---|---|
| `prism-domain` | ≥ 85 % | **99.77 % lines**, 97.86 % regions, 100 % functions | 2026-08-10 |
| `prism-engine` | **> 95 %** | **99.61 % lines**, 99.53 % regions, 99.22 % functions | 2026-08-11 (S6) |
| `prism-core` | **> 95 %** (programmer) | — | |
| `prism-protocols` | **> 95 %** | — | |
| `prism-surface` | **> 95 %** | — | |
| `prism-ipc` | ≥ 85 % | — | |
| `ui` | ≥ 85 % | — | |

### Performance gates

| Gate | Requirement | Measured | Date |
|---|---|---|---|
| Tick jitter | p99.9 < 2 ms, 64 universes, 10 min | **p99.9 = 200 µs**, p50 and p99 ≤ 100 µs, max 588 µs, **0 of 26 401 ticks missed** — at the thread priority `ARCHITECTURE_SPEC.md` §3 specifies. See the note below | 2026-08-10 |
| Tick jitter under 100 % CPU load | p99.9 < 2 ms, 64 universes, 10 min | **p99.9 = 200 µs**, p50 and p99 ≤ 100 µs, max 464 µs, **0 of 26 401 ticks missed**, 0 panics — the whole pipeline at 32 640 slots with eight cue lists running, against eight CPU burners at normal priority. **The load has to come from outside the process** — see §3.1 and the decision log | 2026-08-11 |
| Tick allocations | zero inside the tick after warm-up | **0 allocator calls** in 2 000 ticks, 64 universes, 4 subscribers; **0** in 1 000 ticks with the merge active — 768 slots, 8 loaded sources, executors switching; **0** in 1 000 ticks with merge **and** encoding — the same 768 slots patched 16-bit over 4 universes, half the fixtures inverted, 1 536 channel writes per tick; **0** in 1 000 ticks with **8 cue lists running** — every cue touching all 768 slots, ten-second fades, cues following on by themselves, Gos and on/off over the queue; and **0** in 1 000 ticks with the **programmer and the masters** as well — values arriving and being cleared on the tick, 16 group masters and the grand master moving | 2026-08-11 |
| Tick drift | < one tick period after 100 000 ticks | within one period, and the error does not grow with the tick count | 2026-08-10 |
| Triple buffer integrity | no torn frame under concurrent load | 1 000 000 frames × 64 universes → 4 readers, clean; 3 `loom` models | 2026-08-10 |
| Frame determinism | identical input → byte-identical frames | three runs of the same 100-tick script compared byte for byte on the driver's frames, 8 command changes, > 20 distinct frames | 2026-08-11 |
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

Five tests are `#[ignore]`d because they take minutes, not seconds. A criterion
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

**The S6 stress gate needs the load applied from outside the process.** A Windows
priority class applies to every thread in a process, so spinning burner threads
inside the test process makes them exactly as important as the tick — and an
equal-priority thread that never blocks is not preempted until its quantum
expires. Measured that way the gate fails badly (7 484 of 26 455 ticks missed,
median jitter 15.5 ms, which *is* one quantum), and it fails for a reason that
has nothing to do with the engine. The product raises the tick **thread**, and
the load on a real machine is other work at ordinary priority:

```powershell
cargo build -p prism-engine --release --tests
$exe = (Get-ChildItem target\release\deps\realtime-*.exe |
        Sort-Object LastWriteTime -Descending)[0].FullName
$load = 1..([Environment]::ProcessorCount) | ForEach-Object {
    Start-Process powershell -ArgumentList "-NoProfile","-Command","while(1){}" `
        -PassThru -WindowStyle Hidden
}
$p = Start-Process $exe -ArgumentList "--ignored","--nocapture","--test-threads=1",`
       "the_whole_pipeline_holds_its_deadline_for_ten_minutes_under_full_cpu_load" `
       -PassThru -NoNewWindow
$p.PriorityClass = "High"
$p.WaitForExit(); $load | Stop-Process
```

The test measures whether the machine really was loaded rather than trusting the
harness: a probe thread that yields on every iteration gets about 5.5 million
turns a second on an idle machine and about 130 thousand on a saturated one, and
the run fails if it looks idle.

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
| 2026-08-11 | S6 | **The stress gate failed on its first run, and the cause was the harness rather than the engine: 7 484 of 26 455 ticks missed, median jitter 15.5 ms.** The run put one CPU-burning thread per core inside the test process. A Windows priority class applies to *every* thread in the process, so raising the tick's priority — which `ARCHITECTURE_SPEC.md` §3 requires and §3 of this document already records as decisive — raised the burners' priority too. An equal-priority thread that never blocks is not preempted until its quantum expires, and 15.5 ms is one quantum, measured rather than inferred | Load is applied from outside the process at ordinary priority, exactly as the priority is applied from outside; the run then holds every one of 26 401 deadlines with a p99.9 of 200 µs. **S17 requirement, and it is a sharp one:** `prismd` must raise the priority of the tick **thread**, never of the process. Raising the process would put the ArtNet, MIDI and IPC threads at the same priority as the tick and reproduce exactly this failure — and on a machine where the daemon is one process among many, it would also starve the rest of the desk |
| 2026-08-11 | S6 | **A missed-tick budget over 130 ticks measures the CI runner, not the code.** With the contention fixed, the short pipeline run still went red once: 7 of 125 ticks missed, from a single 68 ms scheduler stall on a shared two-core runner — 68 ms is three periods gone before the engine gets a core back. Its median jitter in that same run was 100 µs. Worse, the median cannot be the gate either: jitter is measured on the ticks that ran and the engine resynchronises after a miss, so the unoptimised 64-universe run had a median of 100 µs while missing 49 of 133 ticks | The short run asserts on the **share of the grid that ran** — at least three quarters of the expected ticks — which is what a pipeline too slow to fit actually destroys, and is deaf to isolated stalls. Verified by sizing the test up to 64 universes in debug on purpose: it fails with 68 of 133 ticks, so the gate is not vacuous. The median assertion stays, for the drifting-schedule regression it does catch. **Note for later sessions:** S2's short run still carries a 2 % missed-tick budget and the same exposure; it has been green for five sessions, but if it starts flapping this is the reason and this is the fix |
| 2026-08-11 | S6 | **CI found the same mistake a second time, in a different disguise: two timing tests in one binary measure each other.** `cargo test` runs the tests of a target on several threads, and the runner has two cores. The new pipeline run and S2's deadline run overlapped — and the pipeline run was also spinning its own load threads — so both reported a median jitter of one scheduler quantum and both failed. S2's test had been green for four sessions because it was the only one of its kind | A `static MEASURING: Mutex<()>` held for the whole of every measured run: timing runs take turns. The short pipeline run also lost its in-process load threads and its probe, for the reason the ten-minute gate already documents, and is now named for what it actually asserts — `the_whole_pipeline_fits_inside_a_tick_period`. **S7 requirement:** a driver timing test belongs behind the same mutex, or it will measure the engine's tests and be measured by them |
| 2026-08-11 | S6 | **The programmer reaches the tick as a slot number, not as a fixture and an attribute.** `prism_domain::ProgrammerState` is nested `BTreeMap`s and cannot cross into the tick at all — dropping one allocates, which §3.1 forbids as firmly as allocating does. A fixture number plus an attribute would also need a lookup table in the tick and would not fit beside a 16-bit value in a queue slot | `TickCommand::SetProgrammerValue`, `ClearProgrammerValue` and `ClearProgrammer` carry a `MergePlan` slot index; `MergeBody::load_programmer` resolves a whole `ProgrammerState` off the tick. **S11/S13/S17 requirement:** the core thread translates each programmer change against *the same plan the body was built from*, and a repatch makes every queued programmer command stale — an out-of-range slot is ignored, but an in-range one would land on a different attribute. Draining or versioning the queue across a repatch is the daemon's problem, not the engine's |
| 2026-08-11 | S6 | **"Intensity" had to be defined before the masters could be written, and the obvious definitions are both wrong.** `AttributeType::Dimmer` is a name, and a profile is free to wire a channel called Dimmer to something that is not intensity; `MergeMode::Htp` describes how an attribute *combines*, which is a different question from what it *is* | An attribute is intensity when its definition files it under `FeatureGroup::Dimmer` — `AttributeDef.featureGroup`, which a profile sets per attribute. `AttributeSlot` carries it and `AttributeSlot::is_intensity` is the single place that decides. Asserted both ways round: a dimmer filed under Beam is not scaled, and a shutter filed under Dimmer is |
| 2026-08-11 | S6 | **A fixture in two groups needed a rule, and `docs/DMX_MERGE.md` §4 does not give one.** Multiplying the masters together gives a quarter of the light for two faders at half — a number neither fader predicts — and makes the result depend on how many groups a fixture happens to belong to | The **lowest** group master applies. A group master is an inhibitive master, an answer to "how much of this fixture's light may pass", and where two constraints apply the tighter one binds. Asserted as a `proptest` against the arithmetic, not against a second implementation of the same walk |
| 2026-08-11 | S6 | Blackout could have been "the grand master, set to zero". It is a separate switch | Releasing blackout gives the operator their fader back where they left it, rather than at zero or at full. One line of state, and the difference is visible mid-show |
| 2026-08-11 | S6 | **An unoptimised build cannot run the full-size pipeline at 44 Hz.** At 64 universes the debug build missed 49 of 84 ticks; the release build of the same code misses none in ten minutes under full load. That is a fact about `-C opt-level=0`, not about the product, but it decides how the tests are sized | The CI-sized version of the stress gate runs 8 universes, with the reason written beside it, and every long run is documented as a release run. **S17 requirement:** never measure or ship a debug `prismd` — the frame rate it gives is not the product's |
| 2026-08-11 | S6 | **The order of the programmer and the masters is not interchangeable, and only one order is defensible.** With the masters below the programmer, a blackout would be a suggestion the operator could lose to their own programmer | `ARCHITECTURE_SPEC.md` §5 steps 5 then 6, implemented in that order and asserted: a grand master at zero blacks out an intensity the programmer is holding, and leaves the programmer's *position* exactly where it was |
| 2026-08-11 | S6 | A programmer value naming a fixture the patch no longer has is the same problem as S5's unresolved cue part | Dropped and counted; `MergeBody::load_programmer` returns the count. **S13/S27 requirement:** surface it, for the same reason as the cue-part count — dropped silently, an operator cannot learn why a value does nothing |
| 2026-08-11 | S6 | **Coverage was raised by removing unreachable branches for the third session running.** The first measurement read 99.50 %, below S5, and every uncovered line in the new code was a defensive `else` no input can reach: a group index that came from the group list, a slot position that came from the intensity list, a membership range built together with its offsets | Merged into single fallible expressions and `map_or` defaults, which removes the branches rather than excusing them. 99.61 % lines, above S5, with `master.rs` at 100 %. This is now a pattern worth stating: in this crate a coverage gap has never once been a missing test — it has always been code the design does not need |
| 2026-08-10 | S5 | **Cues track, and that is forced by a decision already taken rather than chosen here.** A cue could be read as a complete look — everything it does not name goes out — or as a delta, where an attribute it does not mention keeps whatever the playback was holding. The second is the only one available: `Command::StoreCue` stores the programmer, and `docs/DMX_MERGE.md` §3 makes the programmer **sparse** by specification, so a cue recorded after moving one head would contain only that head and would black the rest of the stage out | Implemented as tracking and asserted. **S13 requirement:** the store operation records the programmer's sparse contents as the cue's parts, which is what the playback then reads as a delta. **S28 requirement:** the cue editor shows what a cue *contains*, not what the stage looks like when it runs — those are different lists, and an editor that conflated them would teach operators the wrong model |
| 2026-08-10 | S5 | **"A Go during a running fade behaves deterministically" names a decision the specification does not make.** Three answers are defensible: complete the running fade first, start the new one from the old cue's target, or start it from wherever the fade actually is. The first two both put a visible step in the middle of a crossfade — the operator sees the light jump *because* they pressed Go | Every attribute re-bases from the value it is holding at that instant and moves to the new cue's target over the **new** cue's fade time; attributes the new cue does not name keep their targets and move on to the same new time base. One transition has one clock. Asserted at the value level and again on the wire: the byte at the instant of the Go is the byte the previous fade had reached |
| 2026-08-10 | S5 | **A fade-out cannot mean the same thing for an intensity and for a position.** Fading a pan to its home value on release would swing the head while it is still lit, and — worse — a released LTP source that went on holding a value would go on *winning* its slot for the whole fade, so what faded would be the winner of the merge rather than the value | On release, intensities fade to home over the current cue's fade-out time and every other attribute is **held where it is** until that is over; then the whole source leaves the merge at once and everything falls back together. Same asymmetry as `docs/DMX_MERGE.md` §2.3, and the reason `CueSlot` carries the merge mode |
| 2026-08-10 | S5 | S3 left an open question: whether a retrigger should move a playback to the top of the LTP order, and it required that if so it be a separate operation with its own name. **It needs no new operation.** A Go on a running playback advances its cue and leaves the order alone — stepping a cue list is not switching a playback on. A playback that was switched off and started again *does* go to the top, because its source genuinely left the merge and genuinely re-entered it | Both asserted. `PlaybackLayer::activate` stays idempotent and no `reactivate` was added: the case that motivated the requirement turns out to be the ordinary activate/deactivate pair |
| 2026-08-10 | S5 | **`SetExecutorActive` has to mean two different things, and pretending otherwise would break either playback or the existing tests.** On an executor with a cue list it must mean "play the sequence" and "stop it" (`ExecutorButtonFunction::On`/`Off`); on one without, S3's raw activation is what a host uses to drive values it wrote into the source by hand — which is exactly what S3, S4 and the allocation harness do | Dispatched on whether a sequence is loaded, and a player without one does not touch its source at all. **S11/S17 requirement:** a daemon translating `Command::ExecutorGo`/`ExecutorOff` must go through `MergeBody`, not reach into `PlaybackLayer` — an executor with a cue list on it is played, not poked |
| 2026-08-10 | S5 | **A cue naming a fixture that is no longer patched must not stop the cue list.** A show outlives the rig it was written on, and refusing to compile the sequence would take the whole show down to defend against one dead light | Unresolved parts are dropped and counted in `SequencePlan::unresolved()`. **S27/S28 requirement:** the UI surfaces that count — dropped silently, an operator has no way to learn why a cue does nothing |
| 2026-08-10 | S5 | **A follow chain with no times in it is a loop with no fuse.** Cue 2 follows cue 1 after zero seconds, cue 3 follows cue 2 after zero seconds; evaluated as "advance while the trigger fires", one tick would run the whole list, and a looping sequence would never return | At most **one** cue starts per tick, whatever the times say. A zero-time follow chain advances at 44 cues a second, which is fast enough to look instant and cannot hang the tick. Asserted for both a finite and a looping list |
| 2026-08-10 | S5 | **The order of the two halves of a tick decides whether a cue ever reaches its target.** Checking the follow trigger before evaluating the fades means the cue that ends on this tick hands over one step short of its target, and the next cue starts from a value the operator never saw | Fades are evaluated first, then the trigger is checked, then the new cue re-bases from the values just computed. The test asserts the completed value is what reaches the frame on the tick the follow fires |
| 2026-08-10 | S5 | **`Sound` is deferred and `Time` can arrive with no time.** The tempting default for an under-specified trigger is to fire it — treat a missing time as zero | Neither fires; both wait for a Go. A cue that runs away by itself mid-show is a worse fault than one that waits, and "the cue did not fire" is a diagnosable complaint in a way that "the show ran ahead" is not |
| 2026-08-10 | S5 | **Coverage was raised by deleting unreachable code, not by writing tests for impossible states.** The first measurement read 99.18 % — below S4 — and every uncovered line was an `Option` that cannot be `None`: a slot index that came from `index_of`, a cue index that came from `step`. Writing tests to reach them would have meant inventing states the API cannot produce | The double lookups were merged into one fallible expression and the nested `if let`s became a let-chain, which removed the branches rather than excusing them; the genuinely reachable defensive paths (an out-of-range cue index, a buffer shorter than its sequence) got a real test that asserts they degrade instead of panicking. 99.55 % lines, above S4. Same lesson as S4's generic `build`: a coverage gap in this crate has usually been a structural finding, not a missing test |
| 2026-08-10 | S4 | **A footprint that fits is not the same as an attribute that fits inside it.** `docs/DMX_MERGE.md` §5 only requires rejecting a fixture that runs past channel 512, and `Fixture::last_address` answers exactly that. But a profile is free to name a `coarseOffset` of 9 on a four-channel type, and the two checks together are what make the write provably in range: a four-channel fixture at address 509 would otherwise write channel 518, which is another fixture — or the next universe, since the frame is one flat slice | Offsets are validated against the footprint too, as `PatchError::AttributeOutsideFootprint`, for the fine channel as well as the coarse one. With both checks, every target is inside its own fixture's footprint — asserted as a `proptest` over arbitrary `prism_domain::Fixture` values, not just as an argument |
| 2026-08-10 | S4 | **Two fixtures at overlapping addresses are not an error.** The obvious next validation would be to reject them, and it would be wrong: patching a second fixture onto the first is how an operator clones one, and it is a technique in daily use. The real risk is the *accidental* overlap | Allowed, and made deterministic rather than merely tolerated: targets are written in slot order, so the higher fixture number wins the shared channel, and a test says so. **S27 requirement:** the patch UI warns about overlapping footprints — the engine is the wrong place to forbid them |
| 2026-08-10 | S4 | **The encoder does not blank the frame it is given.** It writes only the channels the patch covers, which is correct while the patch is fixed: every patched channel is rewritten every tick and nothing else ever writes one. It stops being correct the moment a patch can change at run time, because the triple buffer recycles frame buffers — a channel that was patched and no longer is would keep a stale value for as long as that buffer lives | Documented on `ChannelPlan::encode`. **S11/S17 requirement:** replacing a `MergeBody` at run time must blank every frame buffer in the publisher, not merely swap the plan. Blanking on every tick instead was rejected: 32 KB of memset per tick to defend against a case that does not exist yet |
| 2026-08-10 | S4 | **Both inverts compose to a single flag, resolved at patch time.** `AttributeDef.invert` and `Fixture.invertPan`/`invertTilt` are exclusive or — a head hung upside down in a rig whose profile already inverts pan comes out the right way round, because inverting twice is the identity | One `bool` per target instead of two, and no branch on the fixture in the tick. All four combinations are asserted, including the both-true case that must equal the neither-true case; the identity is a `proptest` in its own right |
| 2026-08-10 | S4 | **A generic `build` is measured once per caller.** `ChannelPlan::build` and `MergeBody::for_patch` take `IntoIterator`, so each call site gets its own copy of the whole function — and llvm-cov counts every copy's untaken error paths as uncovered lines. The crate's line coverage read 99.03 % with no uncovered production line anywhere in the new code | Both collect the iterator and delegate to one non-generic function, which is where the work lives. Coverage went to 99.31 % — above the S3 figure rather than below it — and `encode.rs` to 100 %. Worth recording because the first reading was a measurement artefact, not a gap, and the fix is a real one: a patch is validated by one copy of the code rather than one per caller |
| 2026-08-10 | S4 | `MergeBody::new` now takes both plans and can be handed two that describe different patches. A silent mismatch would encode the wrong slot into the right channel — the worst kind of wrong, because every channel would still look plausible | `PatchError::PlanMismatch` on differing slot counts, and `MergeBody::for_patch` as the constructor that cannot produce one. **S11/S17 requirement:** build the two plans through `for_patch`, or re-derive both together |
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

**Phase 1 is complete.** `ARCHITECTURE_SPEC.md` §5 runs end to end —
`Patch → Cue → Fade → Merge → Programmer → Masters → DMX bytes` — allocation-free
on a 44 Hz tick that holds its deadline at 64 universes on a fully loaded
machine. What the engine does not yet have is anywhere to send the frames.
Begin **S7** (`prism-protocols` — the `DmxOutput` trait and Open DMX USB). Use
the prompt in §8. The seam is `FramePublisher::subscribe`, which hands a driver
its own `FrameSubscriber`.

Carried into S7 and beyond:
- The whole pipeline is on `MergeBody`. `load_sequence`, `load_groups` and `load_programmer` are the set-up doors and all three allocate; everything else arrives as a `TickCommand`.
- The programmer is addressed by `MergePlan` slot. That number is a contract between the core thread and the engine, and a repatch invalidates every queued programmer command — see the decision log.
- **`prismd` must raise the tick *thread*'s priority, not the process's.** The stress gate measured what happens otherwise; see the decision log and §3.1.
- Masters scale intensity only, and "intensity" means `AttributeDef.featureGroup == Dimmer` — not the attribute's name and not its merge mode.
- Speed masters (`docs/DMX_MERGE.md` §4, item 3) are playback *rate* and belong to step 2 of the tick. Nothing implements them yet and no domain type carries one.
- The programmer *state machine* — the three-stage Clear, store behaviour, selection — is still **S13**. S6 built only the merge layer beneath it.
- `MergeBody::for_patch(&layout, fixtures, executors)` is the constructor: it builds the `MergePlan` and the `ChannelPlan` from one patch, so they cannot describe different rigs. `MergeBody::new` takes both separately and rejects a mismatch.
- The encoder consumes `MergeBody::values()` — one 16-bit value per `MergePlan` slot, in slot order (`fixture`, then `attribute`). That order is stable and part of the plan's contract, and `ChannelPlan` targets are sorted by it.
- The encoder leaves unpatched channels alone, so **replacing a `MergeBody` at run time must blank the publisher's frame buffers** — see the decision log.
- `MergeBody::load_sequence(executor, &Sequence)` compiles a cue list against the body's own patch and puts it on an executor. It allocates, so it is set-up work, not something to do while the tick runs.
- An executor **with** a sequence is owned by its `CuePlayer`: it is played through `Go` and `SetExecutorActive`, and writing into its `PlaybackSource` by hand will not survive the next tick. An executor **without** one still answers S3's raw activation, which is how a host drives values it wrote in itself.
- The four playback rules S5 decided — cues track, a Go overtakes a running fade, a release fades intensity and holds the rest, one cue per tick — are in the decision log and in `crates/prism-engine/src/player.rs`'s module documentation. They are the console's behaviour, not an implementation detail.
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

**Next up: S7 — `prism-protocols`: `DmxOutput` trait and Open DMX USB**

```text
PrismDMX — Session S7: prism-protocols, DmxOutput-Trait und Open DMX USB

Projektverzeichnis: C:\Users\Milan\Prismdmx

Bitte lies zuerst in dieser Reihenfolge, bevor du irgendetwas änderst:
1. CLAUDE.md                          — verbindliche Qualitäts-, Architektur- und Teststandards
2. PROGRESS.md                        — aktueller Stand, Decision Log, gemessene Zahlen
3. IMPLEMENTATION_PLAN.md             — Session-Protokoll und die Definition von S7
4. ARCHITECTURE_SPEC.md §3, §7, §7.1  — Threadmodell, das DmxOutput-Trait und die
                                        vollständige Tabelle zum SH-RS09B: Zugriffspfad,
                                        Portparameter, Break/MAB-Timing, Fehlerverhalten
5. ARCHITECTURE_SPEC.md §10.1         — was plattformneutral bleiben muss und was nicht
6. crates/prism-engine/src/frame.rs   — DmxFrame und FrameLayout: was ein Treiber bekommt
7. crates/prism-engine/src/triple_buffer.rs — FrameSubscriber: wie ein Treiber Frames abholt
8. crates/prism-protocols/src/lib.rs  — die Crate ist leer und wartet auf diese Session

Aufgabe: Session S7 umsetzen — die Ausgabeseite. Ein Trait, hinter dem jeder
DMX-Ausgang steckt, und der erste echte Treiber dahinter: Open DMX USB auf
einem DSD TECH SH-RS09B. Vollständig getestet, ohne die Hardware zu besitzen.

Stand nach S6 — nichts davon musst du neu bauen:
- prism-engine ist fertig: die Pipeline Patch → Cue → Fade → Merge → Programmer
  → Masters → DMX-Bytes läuft auf dem 44-Hz-Tick, hält absolute Deadlines,
  alloziert im Tick nicht und ist mit 99 % Zeilenabdeckung geprüft.
- `FramePublisher::subscribe()` liefert einen `FrameSubscriber`. Genau den
  bekommt ein Ausgabetreiber: `refresh()` holt den neuesten Frame ab, `frame()`
  gibt ihn her, `channels()` sind die Bytes. Ein Subscriber gehört genau einem
  Treiberthread — der Puffer dahinter ist single-producer/single-consumer.
- `subscribe()` alloziert, darf also nur beim Aufbau gerufen werden, nicht
  während der Tick läuft. Treiber werden im Setup angehängt.
- `DmxFrame::universe(index)` liefert die 512 Bytes eines Universums;
  `FrameLayout::universes()` sagt, welche `UniverseId` an welchem Index liegt.
- Der Tick läuft mit 44 Hz. Ein Treiber liest den *neuesten* Frame in seinem
  eigenen Takt — Open DMX USB schafft laut §7.1 nur etwa 30–40 Hz, und das ist
  kein Fehler, sondern die Eigenschaft der Hardware.

Umzusetzen (IMPLEMENTATION_PLAN.md S7):
- `DmxOutput`-Trait wie in ARCHITECTURE_SPEC.md §7, `OutputHealth`
  (`prism_domain::OutputHealth` existiert bereits), ein Mock-Treiber
- `FtdiBackend`-Trait mit einer Mock-Implementierung — der ganze Treiber muss
  ohne angeschlossene Hardware prüfbar sein
- `OpenDmxUsb`: Portaufbau, Break/MAB-Sequenz, 513-Byte-Frame mit Startcode 0x00
- Treiberthread-Hülle: `catch_unwind`, Reconnect-Backoff 100 ms → 5 s

Vorgehen strikt test-driven (CLAUDE.md): erst der fehlschlagende Test, dann die
Implementierung. `prism-protocols` trägt dieselbe Coverage-Anforderung wie die
Engine (> 95 %); sie ist ein Exit-Kriterium, kein Nebenprodukt.

Exit-Kriterien — die Session gilt erst als fertig, wenn diese wirklich zutreffen:
- Das Mock-Backend prüft die exakte Aufruffolge: `SetBreakOn` → Wartezeit →
  `SetBreakOff` → Wartezeit → Schreiben von 513 Bytes mit Startcode 0x00
- Portparameter geprüft: 250 000 Baud, 8N2, keine Flusskontrolle, Latency
  Timer 1 (die Voreinstellung von 16 ms wäre tödlich)
- Simulierter Verbindungsabbruch mitten im Frame: der Treiber meldet
  `Disconnected`, versucht es mit Backoff erneut, die Engine merkt nichts
- Simulierte Panik im Treiber: von `catch_unwind` aufgefangen, Ausgang als
  degraded markiert, der Prozess lebt weiter
- Coverage auf prism-protocols > 95 %, gemessen und in PROGRESS.md eingetragen
- cargo test -p prism-protocols ist grün
- cargo clippy --workspace --all-targets -- -D warnings ist sauber
- cargo fmt --all --check ist sauber

Wichtige Randbedingungen:
- Diese Crate darf Hardware anfassen, aber kein Test darf welche brauchen. Die
  echte FTDI-Anbindung liegt hinter `FtdiBackend`; die Tests fahren den Treiber
  gegen den Mock und prüfen die Aufrufe, nicht das Gerät.
- VID/PID und die tatsächliche Framerate des SH-RS09B sind **unverifiziert**
  (PROGRESS.md §5, ARCHITECTURE_SPEC.md §7.1). Sie gehören als Daten an eine
  Stelle, an der S8 sie durch Messwerte ersetzt — nicht verstreut in den Code.
- Ein Treiberthread soll in seinem eigenen Ausgabetakt aufwachen und nicht dicht
  neben der Engine pollen (Decision Log, S2): 5 ms Pollintervall hat dort
  gemessen die verpassten Ticks beseitigt.
- Plattformabhängiger Code ist hier erstmals erlaubt (D2XX unter Windows,
  libftdi unter Linux) — aber die Crate muss unter Linux weiter *bauen*, weil CI
  sie dort baut. Das heißt: hinter `#[cfg]` und hinter dem Backend-Trait, und
  die Logik selbst plattformneutral halten.
- prism-domain hat ein optionales Feature `proptest` (`prism_domain::arb` und
  die `Arbitrary`-Impls) — benutzen statt eigene Generatoren zu schreiben.
- Toolchain ist eingerichtet (Rust 1.97.1 msvc, MSVC Build Tools 2022,
  Node 24.11). Coverage misst man mit
  `cargo llvm-cov -p prism-protocols --summary-only`.
- Es ist kein Setup mehr nötig.

Zum Abschluss der Session:
- PROGRESS.md aktualisieren: S7-Status, gemessene Coverage, Decision Log bei
  Abweichungen vom Plan oder Funden, die spätere Sessions betreffen
- PROGRESS.md §8 mit einem neuen, ebenfalls kontextfreien Follow-up-Prompt für
  Session S8 (🔌 Hardware-Bring-up SH-RS09B) überschreiben
- Mit Conventional-Commit-Message committen, z. B. feat(protocols): …
- Danach pushen, den CI-Lauf beobachten und das Ergebnis in PROGRESS.md
  eintragen (IMPLEMENTATION_PLAN.md, Session-Protokoll Punkt 6)
```
