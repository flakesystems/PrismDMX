# PROGRESS.md — PrismDMX Status Tracker

**Last updated:** 2026-08-12
**Current phase:** Phase 4 — IPC and daemon
**Current session:** S17 — `prismd` daemon binary (not started; see §8 for the prompt that starts it)
**Last completed:** S16 — `prism-ipc` framing and transports ✅ — **the daemon has a wire**
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
| 13 | CI verified green | ✅ | Latest: run **31607859145** on `2288ceb` (S16) — all four jobs: Windows 7 m 25 s (it now compiles `tokio`, `axum` and `hyper` as well), Linux neutral 2 m 17 s, ARM64 check 41 s (unchanged — none of S16's dependencies compiles C), UI 54 s. Three attempts: **31604980498** hung on a test rather than failing, **31607534093** failed on a clippy warning — both in the decision log. Before that: run **31583296669** on `24bbbc9` (S15) — all four jobs on the first attempt: Windows 4 m 32 s, Linux neutral 1 m 34 s, ARM64 check 1 m 15 s (now installing `gcc-aarch64-linux-gnu` for the bundled SQLite), UI 52 s. Before that: run **31550454514** on `5ad7d70` (S14) — all four jobs on the first attempt: Windows 3 m 35 s, Linux neutral 1 m 29 s, UI 45 s, ARM64 check 22 s. Before that: run **31546552629** on `d9a6564` (S13) — all four jobs on the first attempt: Windows 3 m 33 s, Linux neutral 1 m 43 s, UI 42 s, ARM64 check 22 s. Before that: run **31531687277** on `3db01dd` (S12) — all four jobs on the first attempt: Windows 3 m 56 s, Linux neutral 1 m 11 s, UI 44 s, ARM64 check 21 s. Before that: run **31522070040** on `3cc803a` (S11) — all four jobs on the first attempt: Windows 3 m 24 s, Linux neutral 1 m 10 s, UI 52 s, ARM64 check 25 s. Before that: run **31506271867** on `532cd1b` (S10, timing gate) — all four jobs on the first attempt: Windows 4 m 45 s, Linux neutral 1 m 12 s, UI 40 s, ARM64 check 23 s. S10's feature commit: run **31500810174** on `3711902`, green on a rerun of the Windows job, which failed on `prism-engine`'s short timing gate rather than on anything in the commit — see the decision log. First verified: run **31346581991** |
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
| S7 | `DmxOutput` trait + Open DMX USB | ✅ | 2026-08-11 | All exit criteria verified — see §2.8. 92 tests, coverage 99.7 % lines. No real FTDI backend yet, on purpose — see the decision log |
| S8 | 🔌 Hardware bring-up SH-RS09B | ✅ | 2026-08-11 | All exit criteria verified — see §2.9. **Found and fixed a real defect:** the break was landing inside the frame. 35.5 Hz measured, adapter verified `0403:6001` / `B0037HIY` |
| S9 | ArtNet | ✅ | 2026-08-11 | All exit criteria verified — see §2.10. 184 tests, coverage 97.9 % lines on the crate with `artnet.rs` at **100 %**. Packet asserted field by field *and* on a datagram received over loopback |
| S10 | sACN (E1.31) | ✅ | 2026-08-11 | All exit criteria verified — see §2.11. 231 tests, coverage 98.4 % lines on the crate with `sacn.rs` at **100 %**. Packet asserted field by field over all three layers *and* on a datagram received over loopback; the stream is ended rather than merely stopped |

### Phase 3 — Core state
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S11 | Show model, command application | ✅ | 2026-08-11 | All exit criteria verified — see §2.12. 88 tests, coverage 99.9 % lines. Two mutation checks confirm the two central tests are not vacuous |
| S12 | Session state (D11) | ✅ | 2026-08-11 | All exit criteria verified — see §2.13. 125 tests, coverage 99.94 % lines with `session.rs` and `file.rs` at **100 %**. Two mutation checks confirm the two central tests are not vacuous |
| S13 | Programmer state machine | ✅ | 2026-08-12 | All exit criteria verified — see §2.14. 166 tests, coverage 99.89 % lines. Three mutation checks confirm the three central tests are not vacuous |
| S14 | Oops journal | ✅ | 2026-08-12 | All exit criteria verified — see §2.15. 192 tests, coverage 99.88 % lines. Three mutation checks confirm the three central tests are not vacuous |
| S15 | SQLite persistence | ✅ | 2026-08-12 | All exit criteria verified — see §2.16. 225 tests, coverage 99.47 % lines. Five mutation checks confirm the five central tests are not vacuous. **Phase 3 is complete** |

### Phase 4 — IPC and daemon
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S16 | `prism-ipc` framing and transports | ✅ | 2026-08-12 | All exit criteria verified — see §2.17. 131 tests, coverage 98.43 % lines. Three mutation checks confirm the three central tests are not vacuous, and one of them **aborted the process** — which is exactly the failure the nesting-depth limit exists to prevent |
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

**Done:** 17 / 33 · **In progress:** 0 · **Blocked:** 0

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
| CI green on the pushed commit | ✅ run **31440601640** on `8bc5b97` — all four jobs, zero non-success steps: Windows full build and test 2 m 23 s, Linux platform-neutral 1 m 7 s, UI typecheck and build 41 s, ARM64 cross-check 19 s. It took three attempts: **31439289713** and **31439990860** both failed on Windows, on two separate findings about timing tests that only a shared two-core runner exposes. Both are in the decision log |

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

### 2.8 S7 verification record

Measured on 2026-08-11, all exit criteria from `IMPLEMENTATION_PLAN.md` S7 and
the session prompt:

| Check | Result |
|---|---|
| `cargo test -p prism-protocols` | ✅ exit 0 — **88 lib tests** + 4 integration tests, 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — 524 tests across 22 targets |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Mock backend asserts the exact call sequence: `SetBreakOn` → delay → `SetBreakOff` → delay → 513 bytes with start code `0x00` | ✅ `a_frame_is_break_mark_and_five_hundred_and_thirteen_bytes_in_that_order` asserts the whole list in one `assert_eq!`, waits included — the mock records them into the same log as the calls, which is what makes it one assertion rather than two interleaved ones. Plus a `proptest` over arbitrary 512-byte frames that every channel arrives unchanged behind a zero start code |
| Port setup: 250 000 baud, 8N2, no flow control, latency timer 1 | ✅ asserted twice — on `PortConfig::DMX512` and on what the cable was actually *told* during `connect`. A separate test pins the latency timer below the 16 ms FTDI default and demands a whole packet fit one USB transfer |
| Simulated disconnect mid-frame: `Disconnected`, retried with backoff, engine unaffected | ✅ at three levels. The driver: a write that fails after the break has gone out closes the link and refuses to send. The runner: the retries land at exactly 100, 300, 700 and 1500 ms after the loss, on a simulated clock. End to end in `tests/engine_to_wire.rs`: six seconds of ticks with the cable out, **264 of 264 ticks, 0 missed, 0 panics**, nothing on the wire, a handful of reconnection attempts rather than 240 — and when the cable comes back the driver sends the *current* look, not the one it was holding |
| Simulated panic in the driver: caught, output degraded, process alive | ✅ three paths, all contained: a panic while sending, one while connecting (backed off like a failed attempt, or a driver that panics every call would spin a core) and one while shutting down. Each counted permanently in `OutputStatus::panics`, each followed by the thread carrying on — asserted on a real thread as well as on the state machine |
| Coverage on `prism-protocols` | ✅ **99.72 % lines**, 99.67 % regions, **100 % functions**. `ftdi.rs` and `output.rs` at 100 % lines, `opendmx.rs` at 100 %, `device.rs` at 100 % |
| Builds on Linux and ARM64 | ✅ the only `#[cfg]` in the crate is `AccessPath::preferred`, and both of its branches are asserted. `prism-protocols` was added to the Linux CI job for this reason |
| CI green on the pushed commit | ✅ run **31451757315** on `9bf7573` — all four jobs, zero non-success steps: Windows full build and test 2 m 57 s, Linux platform-neutral 1 m 10 s (now including `prism-protocols`), UI typecheck and build 43 s, ARM64 cross-check 24 s. Green on the first attempt |

**Delivered:** five modules. `device` is the adapter as data — descriptor, port
parameters, break timing, expected rate and a `verified` flag that is still
`false`; `ftdi` is the cable behind `FtdiBackend`, with `MockFtdi` recording
every call and failing wherever a test asks; `output` is `DmxOutput` and a
`MockOutput` for the mock-output mode `ARCHITECTURE_SPEC.md` §12 wants;
`opendmx` is the DMX512 knowledge — port setup, break, mark-after-break, the
513-byte packet; `runner` is the thread — cadence, reconnect backoff,
`catch_unwind`, and an `OutputStatus` the UI can read from another thread.

**Not delivered, deliberately: a backend that talks to real hardware.** The
plan permits platform code in this crate and the exit criteria never ask for a
device. See the decision log — the seam is `FtdiBackend`, and S8 is where a
cable exists to check one against.

### 2.9 S8 verification record

Measured on 2026-08-11 against the adapter itself — DSD TECH SH-RS09B, serial
`B0037HIY` — with a Monacor WASH-42LED moving head on the line. All exit
criteria from `IMPLEMENTATION_PLAN.md` S8 and the session prompt:

| Check | Result |
|---|---|
| A real fixture responds correctly to a value ramp | ✅ **and it did not, at first.** With the driver as S7 left it the fixture strobed, went dark, or ran its own programme. The cause was a defect in the driver, not in the rig — see the decision log. With the frame timing corrected: a held value gives steady red, and a 12-second fade of the red channel is smooth. The visible staircase in a *slow* fade is 8-bit quantisation and was confirmed as such by running the same fade in 3 s, where it disappears |
| Frame rate measured over 60 s and written into `ARCHITECTURE_SPEC.md` §7.1 | ✅ **35.53 Hz** through D2XX (2 132 frames in 60.01 s) and **38.35 Hz** through the virtual COM port (2 302 in 60.02 s). §7.1's estimate of 30–40 Hz is replaced by the measurement, and so is `DeviceProfile::SH_RS09B` |
| USB VID/PID, serial and product string confirmed | ✅ `0403:6001`, serial `B0037HIY`, product string `FT232R USB UART`, device type FT232R. The product ID matched the guess. The strings stay out of the profile's matching rule — they are FTDI's, not the adapter's |
| Decision recorded: D2XX vs. VCP on this machine | ✅ **D2XX**, and not for the reason one would expect: the VCP measured *faster*. Both drive the fixture correctly; D2XX is preferred because the latency timer and the USB transfer sizes are reachable through it and **unreachable through any serial API** — the registry on this machine had the 16 ms default. Written up in `ARCHITECTURE_SPEC.md` §7.1 |
| Unplug during output: reconnect without restarting the daemon | ✅ with the cable physically pulled and replugged: the driver reported `Disconnected`, the process stayed up, and it **reconnected 4.3 s later** on its own — `unplugging_the_cable_reconnects_without_restarting_the_process` |
| `ARCHITECTURE_SPEC.md` §14 and `PROGRESS.md` §5 ticked | ✅ both rows for the adapter are closed. `DeviceProfile::SH_RS09B.verified` is now `true`, and the test that asserted the opposite has been rewritten onto the measurements |
| `cargo test -p prism-protocols` green **without** hardware | ✅ exit 0 — **119 lib tests** + 4 integration tests, 0 failed, **7 ignored** (the whole bring-up target). Nothing in the ordinary suite touches a device |
| `cargo test --workspace` | ✅ exit 0 — 552 tests across 22 targets |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-protocols` **> 95 %** | ✅ **96.94 % lines without the adapter** — which is the figure CI can reproduce — and **99.30 % with it attached**, running the same suite with `--include-ignored`. Both are recorded because the difference *is* the FFI: 70 lines of library calls that no build server can execute |
| Linux and ARM64 still build | ✅ the two backends are behind `cfg(windows)` and their crates are declared per target, so the Linux job and the ARM64 cross-check never see `libftd2xx` or `serialport` |
| CI green on the pushed commit | ✅ run **31456114786** on `176c674` — all four jobs, zero non-success steps: Windows full build and test 3 m 55 s (it now links a statically linked D2XX), Linux platform-neutral 1 m 13 s, UI typecheck and build 54 s, ARM64 cross-check 23 s. Green on the first attempt, which is the check that the two Windows-only crates really are behind `cfg(windows)` |

**Delivered:** three modules and a bring-up target. `d2xx` is FTDI's own driver,
statically linked; `vcp` is the serial-port fallback; `system` is the policy
that chooses between them — `FallbackFtdi`, which is generic over its two
backends and is therefore tested against two mocks with no cable in the
building. `tests/hardware.rs` is the only code in the repository that needs a
device, every test in it is `#[ignore]`d, and §3.1 carries the commands.

**The session's real product is a defect nobody would have found without the
cable.** Every one of S7's 92 tests passed against the mock, and the frame they
described was malformed on the wire. That is worth stating plainly: a mock
asserts the calls a driver makes, never the time between them.

### 2.10 S9 verification record

Measured on 2026-08-11, all exit criteria from `IMPLEMENTATION_PLAN.md` S9 and
the session prompt. No network device was involved in any of it: the captures
are datagrams received on `127.0.0.1`.

| Check | Result |
|---|---|
| Packet bytes asserted against the specification, field by field | ✅ `the_packet_is_the_specifications_packet_field_by_field` names every field at its offset — ID at 0–7, OpCode `0x5000` **low byte first** at 8–9, protocol version 14 **high byte first** at 10–11, Sequence at 12, Physical at 13, SubUni at 14, Net at 15, Length `0x0200` at 16–17, 512 channels from 18. Each literal is asserted a second time against the constant it must equal (`OP_DMX.to_le_bytes()`, `PROTOCOL_VERSION.to_be_bytes()`, `512u16.to_be_bytes()`), so a byte and its meaning cannot drift apart. ArtSync has the same treatment |
| **Including sequence number wraparound: 255 → 1, not 0** | ✅ `sequence_numbers_count_from_one_and_wrap_to_one` drives 300 changed frames and reads the sequence byte out of all 300 datagrams: element 0 is 1, 254 is 255, **255 is 1**, 256 is 2, and 0 appears nowhere in the list. Plus `every_universe_counts_its_own_sequence`, because the specification counts per port address and a shared counter would look like reordering to a node |
| Refresh timer: a static universe still emits at least every 800 ms | ✅ measured as a **gap between datagrams**, not as a call count, and at two levels. On the driver: ten seconds of a rig at rest on a `ManualClock`, longest gap ≤ 800 ms. Through the `OutputRunner` at the engine's own cadence (`tests/artnet_wire.rs`): 440 frames sent by the runner, **13 datagrams**, longest gap ≤ 800 ms and ≥ 700 ms — so it is a keep-alive rather than a stream. The naive implementation fails this: see the decision log |
| Broadcast is opt-in, never the default — asserted | ✅ three ways. `broadcast_is_never_the_default`: the default `Destination` is `Unicast(vec![])`, `is_broadcast()` is false, and the socket is **never asked for broadcast permission** — the flag `bind` receives is recorded and asserted false. `broadcast_has_to_be_asked_for_by_name`: only `Destination::Broadcast` produces `bind(_, true)`. And `an_output_that_is_never_told_where_to_send_stays_red`: an unfinished configuration is a disconnected output, not a broadcasting one |
| `cargo test -p prism-protocols` green without a network device | ✅ exit 0 — **174 lib tests** (55 of them new) + 10 integration tests, 0 failed, 7 ignored (the S8 hardware target). The only sockets involved are bound to `127.0.0.1`; nothing in the suite sends a datagram off the machine |
| `cargo test --workspace` | ✅ exit 0 — **616 tests** across 23 targets |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-protocols` **> 95 %** | ✅ **97.91 % lines**, 96.89 % regions, 97.29 % functions with no adapter attached — up from S8's comparable 96.94 %. `artnet.rs` at **100 % lines** and 99.88 % regions, `udp.rs` at 99.67 %. The whole remaining gap is the two FTDI backends, which is the FFI S8 already recorded as unmeasurable without hardware |
| CI green on the pushed commit | ✅ run **31495561058** on `4284ae1` — all four jobs, zero non-success steps: Windows full build and test 2 m 50 s, Linux platform-neutral 1 m 5 s, UI typecheck and build 48 s, ARM64 cross-check 17 s. Green on the first attempt, which is also the check that the new modules really are platform-neutral: the Linux job runs `prism-protocols`, and `udp.rs` and `artnet.rs` contain no `#[cfg]` at all. **The documentation commit behind it then went red** on a `prism-engine` timing test and passed on a rerun with no change — not this session's code, and dealt with rather than shrugged at: see the decision log |
| The bytes were checked on a datagram somebody received | ✅ `tests/artnet_wire.rs`: the engine's frame goes through the triple buffer, the runner and a real `UdpSocket`, and the 530 bytes are read back off a loopback socket and asserted — header, port address, sequence, and the patched channels listed literally. Two universes on one output arrive as two datagrams with port addresses 0 and 1 |

**Delivered:** two modules. `udp` is the seam — `UdpSender` with `SystemUdp` over
`std::net::UdpSocket`, `MockUdp` recording every datagram and failing wherever a
test asks, and a `classify` that turns an `io::ErrorKind` into "the network is
gone" or "this datagram was refused". `artnet` is the protocol — `PortAddress`,
the packet builders, `ArtNetConfig`/`Destination`, and `ArtNetOutput`, which is
generic over both the socket and the clock.

**Nothing above `DmxOutput` was touched.** The trait is the same five methods S7
defined, `OutputRunner` is unchanged, and an Art-Net output inherits the thread,
the reconnect backoff and the panic containment exactly as S7 intended — the
first evidence that the extra two methods paid for themselves. It carries
several universes where the cable carries one, and the runner already knew how
to do that.

**The rate is the point.** Open DMX tops out at 35.5 Hz (§2.9); this output runs
at `RunnerConfig::default()`, which is the engine's own 22.727 ms period, and
sends about thirteen datagrams a second per universe while nothing moves.

### 2.11 S10 verification record

Measured on 2026-08-11, all exit criteria from `IMPLEMENTATION_PLAN.md` S10 and
the session prompt. No network device was involved in any of it, and **nothing in
the suite sends a multicast datagram**: the captures are unicast datagrams
received on `127.0.0.1`.

| Check | Result |
|---|---|
| Packet bytes asserted against E1.31, field by field over all three layers | ✅ `the_packet_is_the_specifications_packet_field_by_field` names every field at its offset — **Root:** preamble `0x0010` at 0–1, post-amble at 2–3, `ASC-E1.17\0\0\0` at 4–15, flags/length `0x726E` at 16–17, vector 4 at 18–21, CID at 22–37. **Framing:** flags/length `0x7258` at 38–39, vector 2 at 40–43, source name at 44–107, priority at 108, sync address at 109–110, sequence at 111, options at 112, universe at 113–114. **DMP:** flags/length `0x720B` at 115–116, vector `0x02`, address/data type `0xA1`, first property address, increment, value count `0x0201`, start code `0x00` at 125, 512 channels from 126. Each literal is asserted a second time against the constant it must equal (`flags_and_length(ROOT_PDU_BYTES)`, `VECTOR_ROOT_E131_DATA.to_be_bytes()`, `513u16.to_be_bytes()`, `ACN_PACKET_IDENTIFIER`), so a byte and its meaning cannot drift apart |
| **Including CID stability across restarts** | ✅ `the_cid_is_the_same_after_a_restart` builds an output, sends, shuts it down, then builds a **new** output with a new socket from the same configuration and compares bytes 22–37 of the two datagrams. It holds by construction rather than by luck: there is no constructor anywhere in the crate that invents a CID. The other half is asserted too — `an_output_with_no_identity_refuses_to_connect`: the default CID is nil, a nil CID is a **disconnected** output, and the socket is never opened |
| Multicast group computed for universes 1 **and 63999** | ✅ `the_multicast_group_is_the_universe_in_the_bottom_two_octets`: universe 1 → `239.255.0.1`, universe 63999 → **`239.255.249.255`**. Plus the carry cases a function written for a desk's 64 universes would never meet — 255 → `239.255.0.255`, 256 → `239.255.1.0` — and a `proptest` over the **whole** range 1…63999 asserting that the top two octets are 239.255 and the bottom two are the universe. Out-of-range values are refused: `SacnUniverse::new(0)` and `new(64_000)` are `None` |
| Clean shutdown emits a stream-terminated packet | ✅ at both levels. On the driver: `shutting_down_ends_every_stream_it_started` — three packets per universe, options bit 6 set and no other bit, still this source's CID and this universe's priority, to the universe's own group. On the wire: `tests/sacn_wire.rs::stopping_the_driver_ends_the_stream_on_the_wire` reads them off a loopback socket after `OutputRunner::run` returns. **And each carries the next sequence number** — three identical ones would be discarded as duplicates by a conforming receiver, so only the first would end anything |
| `cargo test -p prism-protocols` green without a network device | ✅ exit 0 — **231 lib tests** (57 of them new) + 17 integration tests, 0 failed, 7 ignored (the S8 hardware target). The only sockets involved are bound to `127.0.0.1` |
| `cargo test --workspace` | ✅ exit 0 — **680 tests** across 24 targets, 12 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-protocols` **> 95 %** | ✅ **98.41 % lines**, 97.65 % regions, 97.75 % functions with no adapter attached — up from S9's comparable 97.91 %. `sacn.rs` at **100 % lines and 100 % functions**, `udp.rs` at 99.43 %, `artnet.rs` still 100 %. The whole remaining gap is the two FTDI backends, which is the FFI S8 recorded as unmeasurable without hardware |
| CI green on the pushed commit | ✅ run **31500810174** on `3711902` — all four jobs: Windows full build and test, Linux platform-neutral 1 m 16 s (it runs `prism-protocols`, which is the check that `sacn.rs` really is platform-neutral — it contains no `#[cfg]` at all), UI typecheck and build 51 s, ARM64 cross-check 26 s. **Green on the second attempt, and not on this session's code:** the Windows job's first attempt failed on `prism-engine`'s short timing gate — the same test that flaked two commits earlier — with the identical tree passing on a rerun with no change. See the decision log: this time the diagnostic says the runner was *starved*, and this commit adds a test binary to a suite `cargo test` runs in parallel |
| The bytes were checked on a datagram somebody received | ✅ `tests/sacn_wire.rs`: the engine's frame goes through the triple buffer, the runner and a real `UdpSocket`, and the 638 bytes are read back off a loopback socket — all three layers, the CID, the source name, the priority, the sequence and the patched channels listed literally. Two universes on one output arrive as two datagrams at two E1.31 universe numbers and two priorities |

**Delivered:** one module and one method. `sacn` is the protocol — `Cid`,
`Priority`, `SacnUniverse` with the multicast arithmetic, `SacnPort`,
`SacnConfig`/`SacnDestination`, the packet builder, and `SacnOutput`, generic over
both the socket and the clock. The method is `UdpSender::set_multicast_ttl`, the
one thing a multicast *sender* needs that the S9 seam did not have.

**The trait is untouched, for the third driver running.** `DmxOutput` is the same
five methods, `OutputRunner` is unchanged, and the termination packets go out
through `shutdown` — which S7 put in the trait for a different reason and which
turns out to be exactly where a protocol's goodbye belongs.

**What sACN has that Art-Net does not is an identity and an ending.** The CID is
configuration rather than a random number, so a desk is the same source after a
restart; and a stopped stream is *ended* rather than left to time out, which is
the difference between a rig releasing a universe at once and holding a dead
desk's look for two and a half seconds.

### 2.12 S11 verification record

Measured on 2026-08-11, all exit criteria from `IMPLEMENTATION_PLAN.md` S11 and
the session prompt. No hardware and no network: this crate is state and data.

| Check | Result |
|---|---|
| Every `Command` in the show group applies or rejects | ✅ `tests/command_application.rs`: `the_two_groups_together_are_the_whole_protocol` asserts the twelve show commands and the eleven §4.4 session commands are **23 in total** and that `Command::is_session_command` agrees with the split, so a command added to the protocol has to be given a home here. `every_show_command_is_decided_rather_than_ignored` applies all twelve and demands each produce a non-empty `effects` list, which is what a command that was quietly dropped would fail. The applier's `match` names all 23 variants — no wildcard — so a new variant is a **compile error** rather than a silent rejection |
| A rejection leaves the state **byte-identical**, as a test | ✅ taken literally: the show is serialised with `rmp_serde::to_vec_named` before and after and the two `Vec<u8>` compared, plus the two fields that are deliberately *not* serialised (`patch_revision`, the dirty flag), which a rejection could otherwise move invisibly. Twelve hand-written rejection cases, one per way a show command can be refused, each run against a clean **and** a dirty show; sixteen more on the direct-edit API S13/S15/S27 will call; and a `proptest` over `any::<Command>()` as the broad net. **Checked by mutation:** bumping the revision before validating turns four of the nine tests red |
| Delta generation verified: deltas applied to a copy reproduce the source exactly | ✅ the copy is `ShowMirror`, an RFC 6902 applier over `prism_domain::JsonValue` — not test scaffolding, it is what a Rust client mirrors with. `tests/delta_round_trip.rs` compares the mirror with the show after **every** operation of a scripted build-up and tear-down, again after the same thing driven through `Show::apply`, and again over up to 24 arbitrary edits per case. The scripted run finishes by deserialising the mirror back into a `Show` and comparing the two byte for byte. **Checked by mutation:** dropping the JSON Pointer escaping fails the property in one shrink step, on a profile key containing `/` |
| Patch conflicts detected and reported, not silently accepted | ✅ and **not refused**, which is the harder half: `Show::conflicts()` returns one `PatchConflict` per overlapping pair — universe, first and last shared channel, and which fixture **wins** — and `Show::apply` puts them in a `Delta::Notice` at `Warn` naming the winner. Asserted for a clone at the same address, a partial overlap, a narrow fixture inside a wide one, three fixtures on one address (three pairs), and the same address in another universe (none). `tests/show_to_engine.rs` asserts the winner the show *names* is the byte the **engine** puts on the wire, both ways round |
| `cargo test -p prism-core` | ✅ exit 0 — **70 lib tests** + 18 integration tests across three targets, 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **768 tests** across 27 targets, 12 ignored (the S8 hardware target and the long engine runs) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-core` **> 95 %** | ✅ **99.91 % lines**, 99.26 % regions, 99.49 % functions. `command.rs` and `conflict.rs` at **100 % lines, regions and functions**, `mirror.rs` and `desk.rs` at 100 % lines, `show.rs` at 99.74 %. The residue is one monomorphisation of the generic JSON projection — the same measurement artefact S4 recorded — and `--show-missing-lines` reports no uncovered source line at all |
| Platform-neutral | ✅ no `#[cfg]` of any kind in the crate. It is in the Linux job's list and in the ARM64 cross-check |
| CI green on the pushed commit | ✅ run **31522070040** on `3cc803a` — all four jobs, zero non-success steps, green on the first attempt: Windows full build and test 3 m 24 s, Linux platform-neutral 1 m 10 s (which is where "`prism-core` is platform-neutral" is actually checked — the crate contains no `#[cfg]` at all), ARM64 cross-check 25 s, UI typecheck and build 52 s |

**Delivered:** five modules. `show` is the model — the patch, the **embedded**
profiles, groups, presets, sequences and executors, with one validated
operation per edit and the `JsonPatchOp`s that describe what it changed.
`command` is `Show::apply`: validation, application, and an `Effect` list for
the work the show model has decided but cannot itself carry out. `conflict` is
what a patch sheet shows in red — overlapping addresses and dangling
references, found and reported rather than refused. `mirror` is `ShowMirror`,
the other end of the delta. `desk` is `MachineConfig`, which is where the sACN
CID now lives.

**The show model does not finish every command, and says so rather than
pretending.** Five of the twelve show commands are the programmer's, whose state
machine is S13; `Oops` and `Redo` are S14 and `SaveShow` is S15. All eight are
still *decided* here: the show validates the half only it can see — that fixture
12 is not patched, that preset 4 does not exist, that sequence 7 is not there to
store into — and answers with an effect naming who finishes the job. That is why
"applies or rejects" is a real criterion for all twelve rather than for four.

**`prism-engine` is a development dependency, and only that.** The show model
must not reach into the engine at run time — that wiring is S17's — but the two
share one rule, "what is a legal patch", and two copies of a rule drift apart.
`tests/show_to_engine.rs` holds them together: every patch the show accepts is
one `MergeBody::for_patch` accepts, over arbitrary patches.

### 2.13 S12 verification record

Measured on 2026-08-11, all exit criteria from `IMPLEMENTATION_PLAN.md` S12 and
the session prompt. No hardware and no network: this session is state.

| Check | Result |
|---|---|
| Every session command applies and emits a `SessionPatch` | ✅ `tests/session_commands.rs`: `every_session_command_applies_and_emits_a_session_patch` applies all eleven §4.4 commands against a session in which each of them has something to change, and demands a **non-empty** operation list from every one — an empty answer is what a command that was quietly dropped would give. `the_eleven_commands_of_section_4_4_are_the_session_group` pins the count at eleven and re-checks the 11 + 12 = 23 split against `Command::is_session_command`. The applier's `match` names all 23 variants without a wildcard, in the opposite direction to `Show::apply`, so a command added to the protocol is a **compile error in both** |
| A rejection leaves the state **byte-identical**, as a test | ✅ taken literally, as in S11: `rmp_serde::to_vec_named` before and after, plus the dirty flag, which is not part of the bytes. Five hand-written cases, one per way a session command can be refused, each run against a clean **and** a dirty session; the twelve show commands, each refused with `NotASessionCommand`; and a `proptest` over arbitrary command sequences as the broad net. **Checked by mutation:** moving the executor-page validation to *after* the write — the S11-shaped bug — turns the byte-identical test and the invariant property red |
| Delta generation verified: deltas applied to a copy reproduce the source | ✅ the copy is `SessionMirror`, the sibling `ShowMirror` needed and did not have. `tests/delta_round_trip.rs` compares the mirror with the session after **every** one of a scripted run through all eleven commands, then deserialises the mirror back into a `SessionState` and compares the two byte for byte; and again over up to 24 arbitrary commands per case, show commands included, which is what the daemon's router hands the wrong way round if the predicate is ever wrong. `the_two_mirrors_ignore_each_others_deltas` drives both mirrors from one delta stream. **Checked by mutation:** dropping the `focusedWindow` operation from the diff fails both session round-trip tests and neither show one |
| The session survives save and load | ✅ `the_session_survives_save_and_load`: a session driven through eight commands and a window placement, written as a `ShowFile` beside a populated show, read back, re-encoded and compared byte for byte — then **every field the criterion names asserted one by one** (view, windows, focus, executor page, selected executor, encoder bank, programmer page, parameter index, command line, the stored view's window count and the placed window's geometry), so a round trip that was equal for the wrong reason cannot pass. Through MessagePack and through JSON, which is S15's export path |
| Client-local state (§4.2) is provably absent | ✅ three ways. **By the type:** the session is `prism_domain::Session` plus a map of `View`, and neither has a field for a monitor, a scroll position, a hover, a drag, a camera or a zoom. **By the document:** `client_local_state_is_absent_from_the_session_document` asserts the exact eleven members of §4.1 and the two members of the document root, then walks **every object key anywhere** in a populated session — windows and stored views included — and fails on any that contains one of §4.2's words. **At the door:** `WindowInstance::params` is the one open bag, so `OpenWindow` refuses `monitor`, `screenIndex`, `scrollTop`, `hoverTarget`, `dragging`, `cameraPosition` and `uiZoom` with `ClientLocalParam`, while `pool` still goes through |
| `cargo test -p prism-core` | ✅ exit 0 — **95 lib tests** + 30 integration tests across four targets, 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **805 tests** across 28 targets, 12 ignored (the S8 hardware target and the long engine runs) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-core` **> 95 %** | ✅ **99.94 % lines**, 99.07 % regions, 99.65 % functions — up from S11's 99.91 %. `session.rs` and `file.rs` at **100 % lines**, `command.rs`, `conflict.rs` and `testkit.rs` at 100 % on all three, `mirror.rs` back to **100 % lines** after the split. The two remaining uncovered lines in the crate are S11's monomorphisation artefact in `show.rs` |
| Platform-neutral | ✅ still no `#[cfg]` of any kind in the crate |
| CI green on the pushed commit | ✅ run **31531687277** on `3db01dd` — all four jobs, zero non-success steps, green on the first attempt: Windows full build and test 3 m 56 s, Linux platform-neutral 1 m 11 s (which is where "`prism-core` is platform-neutral" is actually checked — the crate contains no `#[cfg]` at all), UI typecheck and build 44 s, ARM64 cross-check 21 s |

**Delivered:** two modules and a split. `session` is `SessionState` — the §4.1
state, the views it selects between, the eleven §4.4 commands, and a direct API
for the things a client does that no command carries. `file` is `ShowFile`: the
show and its session, saved together, plus the routing on
`Command::is_session_command` that D11 describes and S17 would otherwise write a
third copy of. `mirror` grew `SessionMirror` beside `ShowMirror`, both on one
`JsonMirror`, because two copies of an RFC 6902 applier is exactly the kind of
duplication that drifts.

**The session is now the second thing the daemon is authoritative about, and it
is deliberately not part of the first.** A show command and a session command are
validated by different appliers, travel as different deltas and are mirrored by
different clients' documents. What holds them together is `ShowFile`, which is
one file on disk and one Save LED.

**One edit in the module can write the session, and it is the one that also
encodes.** `SessionState::commit` takes the successor an edit built beside the
current state, diffs it field by field, encodes the operations, and only then
swaps it in — so S11's "validate and encode before you write" is a property of
the module rather than a rule each of the twelve edits has to remember, and
change detection comes with it.

### 2.14 S13 verification record

Measured on 2026-08-12, all exit criteria from `IMPLEMENTATION_PLAN.md` S13 and
the session prompt. No hardware and no network: this session is the operator's
own layer.

| Check | Result |
|---|---|
| Three-stage clear tested through **all** transitions, including the reset-to-0 rule on unrelated interaction | ✅ `tests/programmer.rs`: `the_three_stage_clear_runs_through_every_transition` walks 0 → 1 → 2 → 0 and **round again**, asserting at each step what survives and what goes — the selection outlives the first press, the active feature group outlives the second, and the third takes the feature group *and* the session's page state with it. `any_other_programmer_interaction_puts_the_clear_stage_back_to_zero` drives four interactions from **both** non-zero stages. The fifth interaction, `StoreCue`, is the exception that proves the rule and has a test of its own: the only way to a non-zero stage is a Clear, and the first Clear has already emptied the values, so a store from stage 1 or 2 is refused before the stage matters — `a_store_can_never_meet_a_non_zero_clear_stage` asserts that, and that the refusal does not move the stage either. **Checked by mutation:** taking the reset out of the one place that performs it turns the interaction test red |
| `ApplyPreset` records `presetRef` so cues stay live-updatable | ✅ asserted at both ends. `applying_a_preset_records_the_preset_reference` reads the value, its `Preset` source and its `presetRef` out of the programmer; `a_preset_reaches_the_cue_with_its_link_intact` stores it and reads the link out of the **cue** in the show, which is where "live-updatable" actually lives. The other half is asserted too — `a_manual_change_on_top_of_a_preset_breaks_the_link`, because a value the operator has since moved by hand must *not* follow a later edit of the preset. **Checked by mutation:** dropping `presetRef` when the cue part is built turns the cue test red and nothing else |
| The programmer is sparse: an untouched attribute is **absent**, not 0 | ✅ four tests and a property. `selecting_fixtures_creates_no_values_at_all` (a programmer that laid down zeros on selection would black the stage out); `an_untouched_attribute_is_absent_rather_than_zero`, which checks absence on a fixture that *is* selected and *does* have the attribute, on one that is not selected, and that the whole map is two fixtures with one attribute each; `an_attribute_the_fixture_does_not_have_is_not_written` (a PAR has no pan); and `setting_an_attribute_with_nothing_selected_changes_nothing`. The property `the_invariants_hold_however_the_programmer_is_driven` runs up to 24 arbitrary programmer commands and asserts after **every** one that no fixture maps to an empty attribute map (the S1 invariant), that nothing is selected twice or unpatched, and that every value the programmer holds is one the show can still resolve. **Checked by mutation:** letting a value be written for an attribute the profile does not define turns three of them red |
| `cargo test -p prism-core` | ✅ exit 0 — **109 lib tests** + 57 integration tests across five targets, 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **846 tests** across 29 targets, 12 ignored (the S8 hardware target and the long engine runs). Two earlier attempts failed `prism-engine`'s `the_tick_holds_its_deadline_for_a_few_seconds`, and the cause was the machine both times: a game and a video were running on the same four cores, which is what the test's own `probe thread turns: 0/s` reports. On a quiet machine the same tree passes all 846. See the decision log — the finding is about how such a failure is read, not about the engine |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-core` **> 95 %** | ✅ **99.89 % lines**, 98.98 % regions, 99.42 % functions. `command.rs`, `conflict.rs`, `desk.rs`, `file.rs`, `mirror.rs`, `session.rs` and `testkit.rs` at **100 % lines**, `programmer.rs` 99.43 %, `show.rs` 99.87 % — and `--show-missing-lines` reports **no uncovered source line at all**, so the residue is the monomorphisation artefact S4 measured and S11 recorded, now in two files instead of one. The first measurement read 99.81 %, and — for the fifth session running — none of *that* gap was a missing test of real behaviour either: it was two validation guards `Show::apply` reaches first, a preset value for an attribute a selected fixture does not have, and one error-display arm. All four now have tests, and the two guards matter to the direct callers S14, S17 and S27 will be |
| Delta generation, the third document | ✅ `tests/delta_round_trip.rs` grew a `FilePair`: one command stream through `ShowFile::apply`, **three** mirrors fed every delta, each taking its own and ignoring the rest. A scripted run through selection, absolute and relative attribute moves, a preset, a store and all three Clear presses, then `one_delta_stream_reproduces_all_three_documents` over up to 24 arbitrary commands, comparing all three after every one. There is deliberately no `ProgrammerMirror` in the crate: `ProgrammerChanged` carries the state whole, so applying it *is* an assignment — what needed asserting was the other half, that a client told nothing has missed nothing |
| Platform-neutral | ✅ still no `#[cfg]` of any kind in the crate |
| CI green on the pushed commit | ✅ run **31546552629** on `d9a6564` — all four jobs, zero non-success steps, green on the first attempt: Windows full build and test 3 m 33 s, Linux platform-neutral 1 m 43 s (where "`prism-core` is platform-neutral" is actually checked — the crate contains no `#[cfg]` at all), UI typecheck and build 42 s, ARM64 cross-check 22 s. The Windows job runs `cargo test --workspace`, including the timing test that had failed twice on a busy development machine |

**Delivered:** one module and the composition around it. `programmer` is
[`Programmer`]: the selection with its three modes, the sparse values, the
feature groups they fall under, the three-stage Clear, and `cue`, which is what
`StoreCue` puts into a sequence. `ShowFile` holds it beside the show and the
session and is where the five split commands are put back together —
`Effect::Programmer` is now carried out rather than handed on. `Show` grew one
query, `attribute_def`, which is the show's knowledge the programmer needs twice
for every value it writes.

**Five commands are decided by two models, and the seam is closed in one
place.** S11 left `SelectFixtures`, `SetAttribute`, `ApplyPreset`,
`ClearProgrammer` and `StoreCue` validated but unfinished, answering
`Effect::Programmer`. `ShowFile::apply` now finishes them, drops the effect and
merges the deltas, so a daemon applying a command through the file never learns
that the work was split — and the show applier is unchanged, so a caller with a
bare `Show` still gets the effect that names the finisher.

**The programmer is not in the file, and the Save LED is the argument.** A
programmer restored from disk would be an absolute override of every playback,
applied to a rig the moment a show opened. The structural half is that setting a
value is not an edit to the show (S11), so a persisted programmer would change
the file's bytes without the lamp ever lighting — `the_programmer_is_not_part_of_the_show_file`
asserts both: the bytes are identical with a full programmer and an empty one,
and `is_dirty()` stays false.

### 2.15 S14 verification record

Measured on 2026-08-12, all exit criteria from `IMPLEMENTATION_PLAN.md` S14 and
the session prompt. No hardware and no network: this session is the way back.

| Check | Result |
|---|---|
| Property test: apply *n* commands, undo *n* times, the state is the start state **byte for byte** | ✅ `tests/oops.rs`: `undoing_every_command_returns_the_state_it_started_in` applies up to 23 generated commands, presses Oops once per journal entry and compares `rmp_serde::to_vec_named` of the whole `ShowFile` — plus the programmer, which S13 deliberately keeps **out** of those bytes and which would otherwise be the half nobody checked. *n* is the number of entries rather than of commands, and that is the criterion rather than a weakening of it: a command that was refused, and one that was accepted and moved nothing, are not steps. The generator is four parts commands the show can actually apply to one part `any::<Command>()` filtered to the undoable ones — an arbitrary `PatchFixture` names a profile no show has, so a purely arbitrary run would journal almost nothing and assert almost nothing |
| Redo after undo returns to the post-command state | ✅ the same property continues: after the undos it presses Redo once per entry and compares against the state the commands left, then asserts the next Redo is refused. Scripted beside it in `a_redo_puts_back_exactly_what_the_oops_took_away`, where the cue count of the sequence is read at each of the three points, and in `a_command_after_an_undo_forgets_the_redo`, which is the other half of the rule: a new step forgets the branch the operator left |
| Playback and session commands are **excluded**, asserted with an executor Go followed by Oops | ✅ `a_running_executor_survives_an_oops`: a patch is journaled, then a master is moved to 32768, then a Go is issued and the engine's answer recorded — and the Oops takes back **the patch**, leaves `is_active`, `current_cue_index` and `master_level` exactly where they were, and produces no playback effect. `SetExecutorMaster` is the sharp case and the reason the record is a scope: an executor master **is** show state, written into the show by a command, so a journal that snapshotted the show would take the operator's fader back down with the patch. `a_session_command_is_never_taken_back` is the other half: all eleven §4.4 commands are applied, none is journaled, and the Oops leaves the session's bytes untouched |
| Ring overflow drops the oldest without corrupting the journal | ✅ `the_ring_drops_the_oldest_entry_and_stays_usable` applies **250** patch commands, asserts the journal holds exactly 200, walks all 200 back and lands byte-identically on the state after command 50 — then asserts the 201st Oops is refused, that the fifty dropped commands are still applied, and that 200 Redos return to the state after command 250. Beside it `journal::tests::the_ring_holds_two_hundred_and_drops_the_oldest` checks the ring itself. **Checked by mutation:** dropping from the back instead of the front turns the unit test red |
| `cargo test -p prism-core` | ✅ exit 0 — **116 lib tests** + 76 integration tests across six targets, 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **872 tests** across 30 targets, 12 ignored (the S8 hardware target and the long engine runs). Green on the first attempt, timing gate included |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-core` **> 95 %** | ✅ **99.88 % lines**, 98.94 % regions, 99.48 % functions. `command.rs`, `conflict.rs`, `journal.rs` and `testkit.rs` at **100 % lines, regions and functions**, `mirror.rs`, `desk.rs` and `session.rs` at 100 % lines, `file.rs` 99.75 %, `show.rs` 99.87 %, `programmer.rs` 99.43 % — and `--show-missing-lines` reports **no uncovered source line at all**, so the five counted lines are the monomorphisation artefact S4 measured and S11 recorded. The first measurement read 99.73 % and, for the sixth session running, the gap was not a missing test either — see the decision log |
| An undo produces deltas like any other command | ✅ `tests/delta_round_trip.rs` grew `an_undo_and_a_redo_reach_all_three_mirrors`: a run that moves the show, the programmer and the session is walked all the way back and all the way forward again through `FilePair`, which compares all three mirrors after **every** command. The arbitrary-command property in the same file now generates `Oops` and `Redo` too, because they are two of the twenty-three |
| An undo reaches the engine | ✅ `undoing_a_patch_reports_the_repatch_and_the_change` asserts `Effect::Repatch` and the `Remove` operation on `/fixtures/9`, and that `patch_revision` moved **forward** rather than back; `undoing_a_store_reloads_the_sequence` asserts `Effect::ReloadSequence` |
| A refusal leaves the state byte-identical | ✅ taken literally, as in S11, S12 and S13. `an_oops_with_an_empty_journal_changes_nothing` for the empty journal; `an_undo_that_is_refused_leaves_everything_where_it_was` and `a_redo_that_is_refused_leaves_everything_where_it_was` for the reachable failure — a cue part names a fixture, so unpatching that fixture makes the older cue list something `Show::store_sequence` refuses. Both assert the record is still in the journal afterwards and that the same press goes through once the fixture is back. **Checked by mutation:** dropping the record instead of putting it back turns the undo test red |
| Platform-neutral | ✅ still no `#[cfg]` of any kind in the crate |
| CI green on the pushed commit | ✅ run **31550454514** on `5ad7d70` — all four jobs, zero non-success steps, green on the first attempt: Windows full build and test 3 m 35 s, Linux platform-neutral 1 m 29 s (where "`prism-core` is platform-neutral" is actually checked — the crate contains no `#[cfg]` at all), UI typecheck and build 45 s, ARM64 cross-check 22 s |

**Delivered:** one module and the door it hangs on. `journal` is [`Journal`], the
200-entry ring of `ARCHITECTURE_SPEC.md` §6.1, and [`UndoRecord`] — a command, an
[`UndoScope`] and the state before and after it over exactly that scope.
`ShowFile::apply` files a step for every command `Command::is_undoable` admits
and carries out `Oops` and `Redo`, dropping `Effect::Undo` and `Effect::Redo`
from the answer exactly as it drops `Effect::Programmer`.

**Six commands are undoable, and the scope is what makes the other seventeen
safe.** A record covers one fixture's patch entry (`PatchFixture`), or one
sequence plus the programmer and its page state (`StoreCue`), or the programmer
and its page state (the four other programmer commands). Nothing else. That is
why an Oops cannot move an executor master — which is show state, and which a
journal of show snapshots would have taken back along with everything else.

**An undo is a command like any other, from the outside.** It emits `ShowPatch`,
`ProgrammerChanged` and `SessionPatch` deltas, answers `Repatch` and
`ReloadSequence`, moves `patch_revision` forward, and leaves the Save LED lit —
undoing back to the state that was last written does not put the file back on
disk, and saying otherwise would be a lie about the platter.

### 2.16 S15 verification record

Measured on 2026-08-12, all exit criteria from `IMPLEMENTATION_PLAN.md` S15 and
the session prompt. No hardware and no network — but for the first time in this
crate, the platter.

| Check | Result |
|---|---|
| Save/load round trip: the loaded show is **byte-identical** to the saved one, sessions included | ✅ `tests/persistence.rs`: `a_saved_show_comes_back_byte_identical` compares `rmp_serde::to_vec_named` of the whole `ShowFile` before the save and after the load, then asserts **every** field the criterion is about one by one — the 16-bit attribute's fine offset, a fixture's position and rotation and its two inverts, universe 64, a preset's colour, a looping sequence, cue `2.5`'s trigger time, a cue part's `presetRef`, an executor's master level and cue index, and all eleven §4.1 session fields plus a placed window's geometry. Beside it a `proptest`, `any_show_the_model_accepts_survives_the_platter`, over arbitrary fixtures and arbitrary cue lists. **The fixture is deliberately free of default values**, which is S14's finding taken as a rule: a show built out of zeros cannot tell "restored correctly" from "never touched". **Checked by mutation:** dropping the preset table from the writer turns it red |
| **Crash safety:** kill the process mid-write; the file still opens and holds the last committed state | ✅ `a_process_killed_mid_write_leaves_the_last_committed_state` re-invokes the test binary as a child that saves a **different** show in a loop, waits for it to say it has started, and kills it — `TerminateProcess`, no unwinding, no destructors, no `sqlite3_close`. Six kills at 17 to 211 ms. After each: `PRAGMA integrity_check`, then the file is asserted to be **exactly one of the two shows and never a mixture**, which is what a write without a transaction produces. The evidence that this is a crash rather than a tidy exit is read off the corpse: the child writes a marker before each save and after each commit, and the run reports `crash test: 6 kills, 5 inside a save, 6 after a commit, largest write-ahead log 3534992 bytes`. **Checked by mutation:** writing the same rows without a transaction turns it red on the second kill |
| Migration from a version-1 file to version 2, tested with a **fixture file** | ✅ `crates/prism-core/tests/fixtures/version-1.prism` is checked in and frozen — a migration checked against a file the current code has just written is a migration checked against itself. `a_version_one_file_migrates_to_version_two` copies it (opening it is what migrates it), asserts `user_version` moves 1 → 2, that all six show tables came across, and that the half version 1 never had opens as the session a fresh desk starts in. `store::tests::the_frozen_fixture_is_the_show_it_was_written_from` is the exhaustive half: the migrated show compared byte for byte against the fixture it was written from, so a table that quietly went missing fails there rather than in a spot check. **Checked by mutation:** a `migrate` that only ever builds a new file turns it red |
| The dirty flag drives the `DirtyFlag` deltas (the X-Touch Save LED) | ✅ `the_lamp_goes_out_when_the_write_succeeds_and_only_then`: a successful save answers with exactly one `Delta::DirtyFlag { unsaved_changes: false }`, a second save answers with **nothing** (an LED cannot be turned off twice), an edit lights it again, and `SaveShow` still answers `Effect::Save` — asking for a save is not the same as having saved. The other half is `store::tests::a_write_that_fails_leaves_the_lamp_lit_and_the_file_as_it_was`, where another writer holds the database: the save fails, the lamp stays lit, the file on disk is byte-identical, and the same save goes through once the lock is released. **Checked by mutation:** moving `mark_saved` to before the commit turns that test red. And S14's rule still holds: `an_undo_back_to_the_saved_state_still_leaves_the_lamp_lit` |
| `cargo test -p prism-core` | ✅ exit 0 — **133 lib tests** + 92 integration tests across seven targets, 0 failed, 2 ignored (the crash test's child and the fixture regenerator) |
| `cargo test --workspace` | ✅ exit 0 — **905 tests** across 31 targets, 14 ignored (the S8 hardware target, the long engine runs, and the two above). Green on the first attempt, timing gate included |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-core` **> 95 %** | ✅ **99.47 % lines**, 98.07 % regions, 98.51 % functions. `command.rs`, `conflict.rs`, `journal.rs` and `testkit.rs` at **100 % on all three**, `desk.rs`, `mirror.rs` and `session.rs` at 100 % lines, `show.rs` 99.88 %, `file.rs` 99.75 %, `programmer.rs` 99.43 %, `store.rs` 97.27 %. `--show-missing-lines` reports ten lines, and they are two kinds: **eight** are the `#[ignore]`d regenerator that rewrites the frozen fixture, and **two** are the error arms of a table count and of encoding one session — the same `?`-propagation residue S11 and S14 recorded. The first measurement read 96.93 %, and for the seventh session running the gap was not a missing test of real behaviour either — see the decision log |
| The programmer, the journal and the desk identity are **not** in the file | ✅ structurally (the schema has no table for any of them) and by test: `a_reopened_show_is_clean_and_has_nothing_to_undo` fills both and asserts that a load empties them, because a loader that replaces the show in place would otherwise leave an absolute override standing over a rig that has just changed. **Checked by mutation:** dropping `Journal::clear` and the programmer reset turns it red. `desk_id_is_not_show_content` (S11) is unchanged and still passes |
| A failed write leaves the state byte-identical and the file undamaged | ✅ as above, and the encoding — the last step that can fail, S11's finding — happens **before** the transaction is opened. Damage is reported rather than read past: a row that does not decode, a row filed under a key that is not its own, a session pointing at a view the file does not have or focusing a window that is not open, a database that belongs to another application, a file written by a newer PrismDMX, a path that is not there, and a file system that cannot carry a write-ahead log are seven separate refusals with seven separate messages |
| Platform-neutral | ✅ still no `#[cfg]` of any kind in the crate. The ARM64 cross-check now needs a C cross-compiler, which is a property of the *dependency* rather than of this crate — see the decision log |
| CI green on the pushed commit | ✅ run **31583296669** on `24bbbc9` — all four jobs, zero non-success steps, green on the first attempt: Windows full build and test 4 m 32 s (it now compiles SQLite's amalgamation as well), Linux platform-neutral 1 m 34 s (where "`prism-core` is platform-neutral" is actually checked — the crate still contains no `#[cfg]` at all), **ARM64 cross-check 1 m 15 s including the cross C toolchain**, UI typecheck and build 52 s. The ARM64 job is the one this session had to change, and it is the one that had been predicted to fail without the change |

**Delivered:** one module. `store` is the `.prism` file — [`ShowStore`], the
schema and its `user_version` migrations, the write-ahead log, the save and the
load; [`Autosave`], the thirty-second policy; and `export_json`/`import_json`,
the interchange format. `Show::from_parts` and `SessionState::from_parts` are
the two doors it builds through, and they are `pub(crate)`: a file is read into
the models directly rather than replayed through their edit operations.

**A load is not a replay, and the reason is a show that is legal to hold and
illegal to store.** S11 decided that a dangling reference is reported rather
than refused, so a cue list naming a fixture somebody unpatched afterwards is an
ordinary show — and `Show::store_sequence` would turn it down. A loader built
out of the edit API would refuse to open the file it had itself written. What
the file *is* checked for is what only a file can be wrong about.

**SQLite is here for the write, not for the queries.** A school's show is a few
hundred kilobytes and nothing queries it; what the dependency buys is one
transaction over a write-ahead log, which is the whole of the crash-safety
criterion. That is why the crash test asserts atomicity — one show or the other,
never a mixture — rather than merely that the file reopens.

### 2.17 S16 verification record

Measured on 2026-08-12, all exit criteria from `IMPLEMENTATION_PLAN.md` S16 and
the session prompt. No hardware and no platter — for the first time in this
project, a socket.

| Check | Result |
|---|---|
| Framing round-trip **property test over arbitrary messages** | ✅ `tests/framing.rs`: six properties at 256 cases each over generated `ClientMessage` and `ServerMessage`, which means all 23 `Command` variants, all 7 `Delta` variants, arbitrary `JsonValue` trees inside the patch deltas, arbitrary programmer states and arbitrary snapshots — from `prism_domain`'s `proptest` feature rather than from generators written here. The round trip goes through the **whole frame** and reads the length back out of the header, because encoding a payload and decoding the same payload leaves the one number the framing adds untested. Two further properties pin what the prefix is for: that it describes exactly the payload behind it, and that two frames back to back stay two frames. **Checked by mutation:** encoding with `to_vec` instead of `to_vec_named` turns `the_payload_is_a_map_carrying_the_tag_and_the_field_names` red |
| An oversized frame closes the connection **without a large allocation** | ✅ `tests/oversized_frame.rs`, with the counting allocator `prism-engine` established in S2, extended to record the **largest single request** — which is the number this criterion is actually about. Refusing a header announcing four gigabytes cost **1 allocator call and 0 bytes**; reading a legitimate frame of exactly `MAX_FRAME_BYTES` through the same reader cost **1 048 576 bytes**, and the test asserts both, because an absolute ceiling would be a claim about the runtime's own bookkeeping as much as about this crate's. Two guard tests prove the probe can see a four-megabyte allocation and reports nothing for an empty window. **Checked by mutation:** allocating the body before checking the length — which leaves every functional test passing, `an_oversized_frame_closes_the_connection` included — makes the probe read **4 294 967 295 bytes** and turns the test red |
| **Transport parity:** the same suite over both transports | ✅ `tests/transport_parity.rs`. Not two suites that resemble each other: **one** function, `the_full_suite`, called three times — named pipe, WebSocket, and the in-process duplex. It takes a `Desk`, which is the same type whichever transport built it, because every transport becomes a `Wire` and nothing above that line knows which. Seven scenarios: the handshake serving the world, a command applied and answered in that order, a refused command, a version mismatch, a delta reaching every client, telemetry arriving decoded, and a client leaving without taking anything with it. Beside it, "results must be identical" taken literally: one scripted session recorded over each transport as the **encoded bytes** of every message the client received, and the three recordings compared. **Checked by mutation:** truncating one byte in the WebSocket pump turns both the WebSocket run and the byte-identity test red |
| A nesting depth is limited on decode (S1's open security requirement) | ✅ `scan.rs` walks the payload with an **explicit stack** before `serde` sees it, so the check cannot itself overflow, and refuses past 128 levels — `serde_json`'s limit, against the same attack. Sixteen tests cover every MessagePack format family byte for byte, the limit exactly at 128 and at 129, and a hundred-thousand-level payload that costs a hundred thousand and one bytes to write. **Checked by mutation, and this is the one that matters:** decoding without the scan does not merely give the wrong answer — `a_hostile_delta_is_refused_rather_than_decoded` **aborts the test process with `STATUS_STACK_OVERFLOW`**. Twelve bytes from an unauthenticated socket, and in `prismd` the DMX output would go with it |
| `cargo test -p prism-ipc` | ✅ exit 0 — **113 lib tests** + 18 integration tests across four targets, 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **1 036 tests** across 35 targets, 14 ignored (unchanged: the S8 hardware target, the long engine runs, the crash test's child and the fixture regenerator). Green on the first attempt, timing gate included |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-ipc` **≥ 85 %** | ✅ **98.43 % lines**, 97.43 % regions, 99.45 % functions. `backpressure.rs`, `memory.rs` and `scan.rs` at **100 % lines**; `message.rs` 99.55 %, `server.rs` 99.50 %, `frame.rs` 99.51 %, `telemetry.rs` 99.48 %, `client.rs` 99.15 %, `transport/mod.rs` 99.09 %, `stream.rs` 97.27 %, `local.rs` 93.33 %, `websocket.rs` 92.23 %. `--show-missing-lines` reports 47 lines and they are four kinds: `?`-propagation arms, `panic!` arms inside tests that pass, the **`#[cfg(unix)]` half of `local.rs`** — which the Linux CI job covers and this machine cannot — and the client-side WebSocket pump's text and error arms. Raising the figure from 97.96 % found two behaviours nobody had asked about, and both were real; see the decision log |
| New dependencies pass the ARM64 cross-check | ✅ checked **before a line of the crate was written**, as the prompt asked: `cargo check -p prism-ipc --all-targets --target aarch64-unknown-linux-gnu` succeeds with `tokio`, `axum`, `tokio-tungstenite`, `futures-util`, `serde_bytes` and `rmp-serde`. None of them compiles C, so unlike S15's SQLite this needs no cross toolchain and the CI job is unchanged. That command compiles the `#[cfg(unix)]` branches of `local.rs`, which is what makes it a portability check rather than a formality |
| CI green on the pushed commit | ✅ run **31607859145** on `2288ceb` — all four jobs, zero non-success steps: Windows full build and test 7 m 25 s (the largest jump yet, and it is `tokio`, `axum` and `hyper` being compiled), Linux platform-neutral 2 m 17 s — **where the `#[cfg(unix)]` half of `transport/local.rs` is executed rather than merely compiled** — ARM64 cross-check 41 s with no toolchain change needed, UI typecheck and build 54 s. **It took three attempts, and the first two are both in the decision log:** run **31604980498** did not fail but *stopped*, on a test that waited for a message nothing had promised to send, and run **31607534093** failed on a clippy warning that a local check had silently skipped |

**Delivered:** eight modules. `frame` and `scan` are the wire format — the length
prefix, the size limit, the depth limit, and the three refusals in the order they
happen in. `message` is `docs/IPC_PROTOCOL.md` §4: two envelopes rather than one,
because a type that could carry either would let a client send a `Delta`.
`telemetry` is §7's channel — a 16-byte header and one 514-byte section per
universe, versioned so a later session can add to it. `backpressure` is §8 as a
plain state machine with no clock and no channel, so *which message is dropped
when* is a unit test rather than a race. `transport` is the three transports and
the one thing they all become; `server` and `client` are the two halves.

**The transport disappears into a value, not behind a trait.** S7 hid three DMX
drivers behind `DmxOutput`, and the same question here has a different answer:
the three things being hidden are two byte streams and a message stream, in an
async world where a trait with `async fn` is not object-safe. So each transport
module spawns a pump that owns its socket and hands back a `Wire` — payloads out
through one channel, payloads in through another. Parity is then not something to
check but something that holds by construction, and `transport/memory.rs` is a
real transport rather than a stub, which is what makes the handshake, the
backpressure policy and every error path testable with no socket and no port.

**The parity suite found a defect that only a named pipe has.** A reader that
stops only at end of stream keeps its half of the socket alive, and a socket
closes only when both halves are dropped — and shutting down the writing half of
a *split* Windows named pipe does not close the pipe. A client that disconnected
while the daemon happened to be saying nothing therefore stayed in the daemon's
client list for ever. It passed over the duplex and over WebSocket and failed
over the pipe, which is the whole argument for running one suite over three
transports rather than three suites over three transports.

---

## 3. Coverage tracking

Targets from `CLAUDE.md`: ≥ 85 % global, > 95 % on engine, programmer and protocols. Record **measured** figures only — leave blank until a run produces a number.

Measured with `cargo llvm-cov` 0.8.7 (installed 2026-08-10, `llvm-tools-preview`).
Command: `cargo llvm-cov -p <crate> --summary-only`.

| Crate | Target | Measured | Date |
|---|---|---|---|
| `prism-domain` | ≥ 85 % | **99.77 % lines**, 97.86 % regions, 100 % functions | 2026-08-10 |
| `prism-engine` | **> 95 %** | **99.61 % lines**, 99.53 % regions, 99.22 % functions | 2026-08-11 (S6) |
| `prism-core` | **> 95 %** (programmer) | **99.47 % lines**, 98.07 % regions, 98.51 % functions — `command.rs`, `conflict.rs`, `journal.rs` and `testkit.rs` at **100 % on all three**, `desk.rs`, `mirror.rs` and `session.rs` at 100 % lines, `show.rs` 99.88 %, `file.rs` 99.75 %, `programmer.rs` 99.43 %, `store.rs` 97.27 %. The ten uncovered lines are the `#[ignore]`d regenerator of the frozen migration fixture (eight) and two `?` arms that no test can reach — see §2.16 | 2026-08-12 (S15) |
| `prism-protocols` | **> 95 %** | **98.41 % lines**, 97.65 % regions, 97.75 % functions without the adapter (what CI reproduces) — `sacn.rs` **100 % lines and functions**, `artnet.rs` **100 %**, `ftdi.rs` and `output.rs` 100 %, `udp.rs` 99.43 %. With the adapter attached S8 measured 99.30 % via `-- --include-ignored`; that figure was not re-measured since and the code it covers is unchanged. The gap between the two is the FFI, which no build server can execute | 2026-08-11 (S10) |
| `prism-surface` | **> 95 %** | — | |
| `prism-ipc` | ≥ 85 % | **98.43 % lines**, 97.43 % regions, 99.45 % functions — `backpressure.rs`, `memory.rs` and `scan.rs` at **100 % lines**, `message.rs` 99.55 %, `frame.rs` 99.51 %, `server.rs` 99.50 %, `telemetry.rs` 99.48 %, `client.rs` 99.15 %, `stream.rs` 97.27 %, `local.rs` 93.33 %, `websocket.rs` 92.23 %. The 47 uncovered lines are `?` arms, `panic!` arms in tests that pass, the `#[cfg(unix)]` half of `local.rs` (which only the Linux job can reach) and the client WebSocket pump's error arms — see §2.17 | 2026-08-12 (S16) |
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
| Open DMX frame rate | measure real rate on SH-RS09B | **35.53 Hz** over 60 s through D2XX (2 132 frames), **38.35 Hz** through the virtual COM port. Both with the frame timing corrected; before the correction the same code reported 43.1 Hz, and that figure was the *symptom* — see the decision log. `DeviceProfile::SH_RS09B` now carries `verified: true` | 2026-08-11 (S8) |
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

### 3.2 Running the hardware tests (S8)

`crates/prism-protocols/tests/hardware.rs` is the only code in the repository
that needs a device. Every test in it is `#[ignore]`d, so `cargo test` never
touches an adapter; these are the commands that reproduce every number in §2.9.
All of them need the SH-RS09B plugged in, and the ramp needs a fixture on the
line. The tests take turns behind a mutex, so `--test-threads=1` is belt and
braces rather than a requirement.

Which adapter is attached, and what it calls itself:

```bash
cargo test -p prism-protocols --release --test hardware -- --ignored --nocapture the_adapter_on_this_machine
```

The sustained frame rate over sixty seconds, on both access paths:

```bash
cargo test -p prism-protocols --release --test hardware -- --ignored --nocapture --test-threads=1 the_sustained_frame_rate
```

Where the time in a frame goes — the diagnostic that found S8's defect. It
prints, per frame, the cost of the break transfers, of `FT_Write`, and of the
transmit queue that turned out to be no signal at all:

```bash
cargo test -p prism-protocols --release --test hardware -- --ignored --nocapture where_the_time_in_a_frame_actually_goes
```

Driving a fixture. `PRISM_DMX_CHANNELS` takes a channel (`7`), a range
(`1-16`), `all`, or `sweep` to walk one channel at a time; `PRISM_DMX_HOLD`
pins channels on top, which is how a wash light's master is held open while one
colour is ramped. **Hold the master and drive one channel** — blind-ramping
every channel walks a fixture into its own auto programme, which is exactly how
S8 lost an hour:

```powershell
$env:PRISM_DMX_CHANNELS = "7"; $env:PRISM_DMX_HOLD = "6=100"; $env:PRISM_DMX_STEP_MS = "10000"
cargo test -p prism-protocols --release --test hardware -- --ignored --nocapture a_fixture_follows_a_value_ramp
```

`PRISM_DMX_PATH` forces `d2xx` or `vcp` instead of the machine's preferred
path, `PRISM_DMX_BREAK_US` and `PRISM_DMX_MAB_US` override the break timing,
and `PRISM_DMX_SECONDS` shortens the rate runs.

Unplugging the cable mid-show needs a pair of hands, so it asks for them:

```powershell
$env:PRISM_DMX_INTERACTIVE = "1"
cargo test -p prism-protocols --release --test hardware -- --ignored --nocapture unplugging_the_cable
```

Coverage including everything the hardware reaches — the 99.30 % figure in §3.
Without the adapter this fails, which is the point of the `#[ignore]`s:

```bash
cargo llvm-cov -p prism-protocols --summary-only -- --include-ignored
```

### 3.3 The `loom` models

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
| sACN against a real receiver | nothing — S10 is complete without it | ☐ unverified, and **deliberately not blocking**. Everything a socket can answer is asserted, including the datagram as received and the group address over the whole 1…63999 range. What only a gateway and a real switch can answer is whether the **multicast path** works end to end — IGMP snooping on the switch, and whether a hop limit of 1 reaches the venue's nodes. Both are held as data (`SacnDestination`, `SacnConfig::multicast_ttl`), so verifying them is a configuration change. **No test sends multicast on purpose:** a suite that put sACN on the network it runs on is the same fault as one that broadcasts. `ARCHITECTURE_SPEC.md` §14 |
| ArtNet against a real node | nothing — S9 is complete without it | ☐ unverified, and **deliberately not blocking**. Everything a socket can answer is asserted, including the datagram as received. What only a node can answer is whether it agrees about the port-address mapping (0-based or 1-based on that front panel) and whether it wants ArtSync. Both are held as data — `PortAddress` per universe and `ArtNetConfig::sync` — so verifying them is a configuration change, not a code change. `ARCHITECTURE_SPEC.md` §14 |
| ~~SH-RS09B USB VID/PID and real frame rate~~ | — | ✅ **verified 2026-08-11 (S8)** — `0403:6001`, serial `B0037HIY`, `FT232R USB UART`, 35.5 Hz over 60 s. `DeviceProfile::SH_RS09B` carries `verified: true` and the tests assert the measurements. **Holding it as data paid for itself:** the whole verification was three fields and one test, with no code changed anywhere else — see `ARCHITECTURE_SPEC.md` §14 |

---

## 6. Decision log

Architectural decisions D1–D11 are in `ARCHITECTURE_SPEC.md` §1. This log records **changes and discoveries made during implementation** — things that turned out differently from the plan.

| Date | Session | Finding | Consequence |
|---|---|---|---|
| 2026-08-12 | S16 | **A local check that is skipped reports nothing, and `&&` is how it gets skipped.** The second CI attempt failed on one clippy warning — `unnecessary qualification`, from an import added while fixing the hang. It had been run locally as `cargo fmt --all --check && echo FMT_OK && cargo clippy … ; echo CLIPPY_OK`, and the formatting check failed on an import order, so the `&&` chain stopped before clippy — while the `;` after it printed `CLIPPY_OK` regardless | Read as "clippy passed", which it had not. The two checks are independent and are now run independently, each with its own exit code printed. Worth recording because it is not a Rust mistake or a CI mistake: a status line that can say a check succeeded without the check having run is worse than no status line, and it is the same failure mode as a test fixture of default values — something that cannot distinguish "passed" from "never happened" |
| 2026-08-12 | S16 | **A test that can hang is worse than a test that fails, and the first CI run proved it.** `telemetry_is_coalesced_and_the_counters_say_how_much_was_dropped` read a **fixed number of messages** off a client that is behind by construction — and how many get through such a client is precisely the quantity that test says nothing is guaranteed about. Nine arrived on this machine; on a two-core runner fewer did, and both the Windows and the Linux job sat in the `Test` step until they were cancelled by hand, 40 minutes in, with no output because a job's log is not readable until it ends | Every receive in the integration tests now has a deadline, so a message that never comes is a named failure in seconds rather than a job that stops. The telemetry test drains what actually arrived instead of counting to eight. **And the workflow grew `timeout-minutes`** — 20 for Windows, 15 for the rest — because the default is six hours and the next hang should cost twenty minutes, not a day. Two things are worth writing down beyond the fix: the local suite passed serialised, under load and at `--test-threads=1`, so *reproduce it locally* was not available; and cancelling the run is what made the log readable, which is how the failing test was identified in one minute after 40 of guessing |
| 2026-08-12 | S16 | **The nesting-depth limit is not a hardening measure, it is the difference between an error and a dead console — and that was measured rather than argued.** S1 left it as the one open security requirement: `JsonValue` is recursive, serde buffers the content of an internally tagged enum before any domain code runs, and MessagePack has no depth limit of its own. A payload of a hundred thousand nested arrays costs a hundred thousand and one bytes to write | `scan::depth_of` walks the payload with an **explicit stack** before `serde` sees it and refuses past 128 levels — `serde_json`'s limit, against the same attack. A wrapping `Deserializer` would not have worked: the recursion to be stopped happens *inside* serde's own buffering, one layer below anywhere a wrapper could count. **The mutation check is the finding.** With the scan removed, `a_hostile_delta_is_refused_rather_than_decoded` does not fail — it **aborts the process with `STATUS_STACK_OVERFLOW`**, which in `prismd` takes the DMX output with it. Twelve bytes from an unauthenticated socket. The scan doubles as a structural check, so a truncated frame, the reserved byte `0xc1` and trailing rubbish are all found before a value is built |
| 2026-08-12 | S16 | **`tokio` and `axum` are large dependencies, and the question was settled by the specification before it was settled by taste.** `ARCHITECTURE_SPEC.md` §3 names the runtime in the threading table — `core-main (async, tokio)` — and `docs/IPC_PROTOCOL.md` §2 names `axum` for the WebSocket listener. The alternatives were real: a synchronous, thread-based crate with `tungstenite` in blocking mode would have matched the rest of the workspace, and the daemon's other threads are threads | Taken as specified, for two reasons beyond the specification. `tokio`'s `net` feature carries **both** local transports — Windows named pipes and Unix domain sockets — so the platform split becomes a dependency feature rather than FFI we would have to write, and the workspace forbids `unsafe_code`. And `axum` hands back a `Router`, so S31's Web Remote adds routes to the listener that already exists instead of opening a second port to firewall. **The ARM64 cross-check was run before a line of the crate was written**, as the prompt asked: all five new dependencies check clean for `aarch64-unknown-linux-gnu`, and none of them compiles C, so the CI job needs no change — unlike S15's SQLite. The engine tick stays on its own OS thread at its own priority; nothing about §3.1 changes |
| 2026-08-12 | S16 | **`ARCHITECTURE_SPEC.md` §10.1 allows `#[cfg(target_os = …)]` in two crates and this is a third, so the exception is drawn as narrowly as it can be.** A named pipe under Windows beside a Unix domain socket under Linux is exactly the kind of split §10.1 exists to contain, and there is no way to have the transport §2 specifies without it | Two `#[cfg]` blocks, both in `transport/local.rs`, both selecting a type: `NamedPipeServer` or `UnixListener`, `ClientOptions::open` or `UnixStream::connect`. Both hand back the same `Wire` through the same `stream::spawn`, and the framing, the handshake, the backpressure policy, the server and the client are **one code path on every target**. The case is the same one §10.1 makes for the FTDI backend, and it is kept honest the same way: CI runs `cargo test -p prism-ipc` in the **Linux** job, so the Unix branches are compiled *and executed* on every commit while the Windows job does the same for the pipe. `ARCHITECTURE_SPEC.md` §10.1 has been amended to name the crate rather than leaving the rule quietly broken |
| 2026-08-12 | S16 | **The transport hides behind a value, not behind a trait, and that is the design question the session was set.** S7's answer for three DMX drivers was a five-method `DmxOutput`. It does not transfer: the three things being hidden here are two byte streams and a message stream, every operation is `async`, and a trait with `async fn` is not object-safe — so a `dyn Transport` would have meant `async-trait` and a boxed future per message, or an enum that has to be extended in three places for every new transport | `Wire`: a pair of channels and a pump task that owns the socket. Each transport module's whole job is to produce one. The parity criterion then stops being something to check and becomes something that holds by construction — `Server` and `Client` **cannot** tell which transport they are on, because the type does not carry the information. The second consequence is the one that pays: `transport::memory::pair()` is a `Wire` over `tokio::io::duplex`, a real transport rather than a stub, so the handshake, the backpressure policy and every error path are testable with no socket, no port and nothing left behind by a panicking test. That is S7's actual lesson — choose the abstraction so the tests can implement it too |
| 2026-08-12 | S16 | **The framing is not identical across the two transports, and §3's sentence had to be corrected rather than obeyed.** §2 says *both transports carry identical framing*; §3 defines framing as a `u32` little-endian length followed by MessagePack. Putting that prefix inside a WebSocket binary message would be a second copy of a number the WebSocket frame header already carries | The **payload** is identical, byte for byte, and `tests/transport_parity.rs` asserts exactly that by recording a scripted session over each transport and comparing the encodings. The prefix is not, because two lengths that can disagree is precisely the ambiguity a protocol whose purpose is that neither end guesses should not have. What is genuinely shared is the **limit**: `MAX_FRAME_BYTES` is checked against the length prefix on a stream and handed to the WebSocket implementation as `max_message_size` on a WebSocket — in both cases against what the peer *announced*, before a buffer for it exists. `docs/IPC_PROTOCOL.md` §2 and §3 now say this |
| 2026-08-12 | S16 | **The `Snapshot` grew a third document rather than being followed by a delta, and §9 is what decides it.** S13 left the choice open: §4.1's snapshot carries the show and the session and not the programmer, so a client connecting mid-programming sees an empty one. Either the snapshot grows, or the daemon sends a `ProgrammerChanged` straight after it | The snapshot grows. §9's *snapshot completeness* row asks that a fresh client's snapshot equal the state an existing client reached by accumulating deltas — and `ProgrammerChanged` **is** one of those deltas, so under the other reading that criterion is false for the programmer unless it is rewritten to exclude it. The world arrives in one message. The show and the session travel as **documents** rather than as models for a separate reason: `ShowPatch` and `SessionPatch` are RFC 6902 operations and an operation is only meaningful against a document root, which is the value `prism_core::ShowMirror` already applies them to — and it is also what keeps `prism-ipc` on `prism-domain` alone, rather than putting the whole show model behind the wire format |
| 2026-08-12 | S16 | **Telemetry travels inside the MessagePack envelope as an opaque byte string, which satisfies §7 and §3 at once.** §7 says telemetry is *binary, fixed layout — not MessagePack maps*; §3 says message types are distinguished by the tagged enum in the payload and **not** by a separate header byte. Read together they appear to ask for a channel discriminator that §3 forbids | `ServerMessage::Telemetry { data }`, where `data` is a MessagePack `bin` holding a fixed-layout frame. The envelope is the one tagged enum, the content is not a map, and the cost is about ten bytes per frame — against 32 KiB of levels at 64 universes. Without `serde_bytes` it would have been one MessagePack integer per channel, which is what the test asserts it is not. The layout is a 16-byte header and one 514-byte section per universe, **versioned**: a frame announcing a layout this build does not know is dropped rather than guessed at, which a droppable channel can afford. What is measured beside the levels is S17's to decide; S16 decided the channel |
| 2026-08-12 | S16 | **A command carries a sequence number and the answers echo it — an addition to §5, and the fader bank is the reason.** §4 lists `Ack` and `Reject` as reliable daemon-to-client messages and §5 says a command that cannot be applied yields a `Reject`. Neither says which command | `ClientMessage::Command { seq, command }`, echoed in `Ack { seq }` and `Reject { seq: Some(..) }`. The number is per connection, and the daemon stores it and never interprets it. Without it, a client with several commands in flight — the normal case for eight faders — learns only that *something* was refused. `Reject` carries `seq: None` when the refusal is about the connection rather than about a message, which is what distinguishes a rejected command from a rejected client |
| 2026-08-12 | S16 | **A payload the daemon cannot decode is not a reason to disconnect, and an oversized frame is — the line is about the framing rather than about how bad the message was.** The first implementation had a `FrameError::is_fatal` that called a too-deep or malformed payload fatal, which reads as severity and is the wrong question | `FrameError::loses_the_frame_boundary`, and only `TooLarge` does: it is read *from* the length prefix, so where the next frame begins is no longer known and every byte after it is rubbish of unknown length. Everything else concerns a payload the framing already delimited — the next frame starts exactly where it always would — so the peer is answered with `Reject { reason: Undecodable }` and the connection carries on. The case that decides it is an honest client of the wrong version: disconnecting it would hide the reason, and a hostile one gets a refusal per message rather than a way to be disconnected on purpose |
| 2026-08-12 | S16 | **A reader that stops only at end of stream leaks a Windows named pipe, and the transport parity suite is what found it.** A socket closes when **both** halves are dropped, and shutting down the writing half of a *split* named pipe does not close the pipe. So a client that disconnected while the daemon happened to be saying nothing left the daemon blocked on a handle that would never produce another byte — and in the client list for ever, which is a leak in the process §8 promises will *free the client's state and carry on* | The read loop also stops when the `Wire`'s receiving half has been dropped: nowhere left to deliver to, read half released, socket closed on both sides. It passed over the in-memory duplex and over WebSocket and failed **only** over the pipe, which is the argument for one suite run three times rather than three suites — a per-transport suite would have been written against the transport it was testing and would have had no reason to try this |
| 2026-08-12 | S16 | **The `Reject` that ends a connection is best-effort; the disconnection is not.** §8 disconnects a client whose control queue filled — and a client whose control queue filled is exactly the client that has stopped reading, so the message explaining why cannot reach it. Waiting for it to reach it is waiting for ever, in a task still holding a socket and a place in the client list | `ServerConfig::goodbye`, one second by default, bounds both the wait for room on the transport and the final flush. The connection then closes regardless. `tests/backpressure.rs` asserts the message *does* arrive for the case it exists for — a client that falls behind and then starts reading again — and `a_client_that_never_takes_its_rejection_is_dropped_anyway` asserts the connection ends for the client that never does. Found while writing the second of those, which without the deadline never returned |
| 2026-08-12 | S16 | **`to_vec_named` cannot be verified by a Rust round trip, and the first version of the test tried to.** S1's requirement is that MessagePack be written with named fields, because the compact array encoding of a struct has nowhere to put an internally tagged enum's `t`. The obvious test — encode compactly, fail to decode — **passes nothing**: `rmp-serde`'s deserialiser accepts the array form and reads it back correctly | Asserted on the bytes. The named payload is a map, and contains `t` and `executorId`; the compact one is an array, and contains neither. The reason the distinction matters is not visible from inside Rust at all: the other end of this wire is TypeScript reading `ui/src/bindings/`, where `Command` is an object with a `t`, and `["ExecutorOff", 3]` is not that object. **Checked by mutation:** switching `encode` to `to_vec` turns the byte assertion red and leaves every round-trip property green |
| 2026-08-12 | S16 | **Coverage was raised by testing behaviour nobody had asked about, for the eighth session running.** The first measurement read 97.96 % lines. Two of the gaps were reachable and real: a client sending an oversized frame *after* a successful handshake (the daemon's side of §3, which no test had exercised — the framing's own test asserts it from the wire's side), and a first message that is well-formed MessagePack and not a `ClientMessage` | Three tests added, 98.43 % lines, and one dead method deleted rather than excused — `Wire::shutdown_within` had no caller, so it went, which is S14's rule about removing a branch instead of covering it. What is left is `?` arms, `panic!` arms inside tests that pass, and the `#[cfg(unix)]` half of `local.rs`, which this machine cannot execute and the Linux job does. **The rule that keeps holding:** the uncovered line is where the defect is, and "it is only an error path" has now been wrong eight times |
| 2026-08-12 | S15 | **SQLite is C, and the ARM64 cross-check is where that stops being an implementation detail.** Three variants were weighed and two were tried on this machine before a line of the module was written, which is what the session prompt asked for. **Bundled** (`rusqlite` with the SQLite amalgamation compiled in) builds on Windows and Linux and fails `cargo check --workspace --target aarch64-unknown-linux-gnu` with `failed to find tool "aarch64-linux-gnu-gcc"` — a build script has to *compile* `sqlite3.c` for the target even though `cargo check` never links. **A preinstalled SQLite** removes the C compilation and replaces it with a system library that has to exist on every machine a show file is opened on, Windows included, which it does not. **Pure Rust** exists — `turso` 0.8.0-pre.4, `limbo_core` 0.0.22 — and is a pre-1.0 reimplementation of the write-ahead log this session's crash-safety criterion is entirely about | **Bundled, and the ARM64 job installs `gcc-aarch64-linux-gnu`.** The reasoning is that a file format a school's shows live in should be the same SQLite everywhere, pinned in the binary rather than supplied by a distribution — and that a cross-compile of a workspace containing C needs a cross C toolchain, which is a missing tool rather than the portability break this job exists to catch. The alternative — leaving the job to fail — would have retired the check that `prism-core` contains no `#[cfg]`. **`prism-core` is unchanged in the respect that matters:** no `#[cfg]` of any kind, and the Linux job still runs its tests. **S17/S29 requirement:** a Raspberry Pi build compiles this natively and needs no cross toolchain, but a *cross*-build for the Pi does, and the packaging session owns saying so |
| 2026-08-12 | S15 | **A load cannot be a replay of the edit operations, because a show that is legal to hold is not always legal to store.** The obvious loader rebuilds the show by calling `patch_fixture`, `store_preset`, `store_sequence` and the rest, and gets validation for free. It also refuses to open files it wrote itself: S11 decided that a dangling reference is *reported* rather than refused, so unpatching a fixture a cue part names leaves an ordinary show that `Show::store_sequence` turns down (S14 relied on exactly that to test a refused undo) | `Show::from_parts` and `SessionState::from_parts`, `pub(crate)`, building the collections directly. What the file is checked for instead is what only a file can be wrong about: a row that does not decode, a row filed under a key that is not the identifier inside it, a session naming a view the file does not have or focusing a window that is not open. Everything else is `Show::issues()`' job, exactly as it is for a show edited into that state in front of the operator. **S27 requirement:** the patch sheet shows those issues after a load as it does after an edit — a show that opens with a warning is better than one that will not open |
| 2026-08-12 | S15 | **One row per entity rather than one blob per file, and the argument is what a damaged sector costs.** A show is small enough that a single MessagePack blob in one row would have worked and been half the code | Eight tables keyed by the number the operator uses. A row that will not decode costs **one sequence**, and the error names it (`sequence row 5: …`); a single blob is a file that is either readable or not. The keys are the same keys `Show` holds its collections under, so a load is a walk rather than a rebuild — and the shape a later session needs to write only what changed is already there. **The documents are MessagePack and not JSON**, which is S1's finding applied to the platter: `serde_json`'s parser is not correctly rounded, so a fixture's position would come back a unit in the last place from where it was hung, and "byte-identical" would be false for every rig with a 3D view |
| 2026-08-12 | S15 | **A save that succeeded and a tidy-up that failed are two different things, and the Save LED is what makes the difference matter.** `ShowStore::save` discards the recovery copy after committing. The first version returned that failure as the save's failure — so a recovery copy that could not be removed (a permission, a file somebody had open) left the show *written* and the lamp *lit*, which is the one lie the lamp must never tell. Found by a test written for the coverage of an error arm, not by review | The save answers `Ok` with a `Delta::Notice` at `Warn` naming the copy that outlived its show, beside the `DirtyFlag` that goes out because the show really is on the platter. It is the S11 rule about silence in a new place: an autosave copy that survives its own show would be offered to the operator at the next start as unsaved work that is not unsaved |
| 2026-08-12 | S15 | **An autosave is not a save, and the timer starts at the edit rather than at start-up.** Two readings were available and both are wrong: writing the recovery copy every thirty seconds regardless leaves a desk writing files all evening while nothing changes, and starting the clock when the daemon did makes the first autosave land *immediately* after a long clean spell | `Autosave::poll(now, dirty)` — the interval runs from the moment the file became dirty, resets when it is saved, and answering yes spends the interval whether the caller acts on it or not (a disk that has just refused is not persuaded 22 ms later). It owns no clock and no thread, because `prism-core` owns neither; the daemon passes the time since it started, which is what makes "thirty seconds" assertable on a simulated clock instead of by waiting. **`write_recovery` deliberately does not call `mark_saved`:** the operator asked for *their* file to be written, and until it is, the lamp is telling the truth |
| 2026-08-12 | S15 | **`Effect::Save` is the one effect `ShowFile::apply` does not carry out, and that is a boundary rather than an omission.** S13 and S14 both absorbed their effect (`Programmer`, `Undo`, `Redo`) into the file, so the symmetric move would have been to give `ShowFile` a store and let `SaveShow` write | A show file has a path, a disk and a failure mode, and the model that decides what a show *is* deliberately holds none of the three — an `apply` that could block on a network drive would be a state machine that sometimes takes five seconds. `Effect::Save` reaches the daemon, which answers it with `ShowStore::save`, and that function returns the deltas so the flag transition still travels exactly once. **S17 requirement:** the daemon holds the `ShowStore`, carries out `Effect::Save`, and polls `Autosave` on the same timer it does everything else |
| 2026-08-12 | S15 | **A migration fixture written by the code under test is a migration tested against itself.** The plan asks for "a version-1 file, tested with a fixture file", and the cheap reading is to write one in the test's set-up | `crates/prism-core/tests/fixtures/version-1.prism` is checked in, frozen, and un-ignored explicitly in `.gitignore`. It is copied before it is opened, because **opening is what migrates it** — a test that left it at version 2 would pass exactly once. Its provenance is a function (`write_version_one`) that a second, non-frozen test also exercises, so the fixture is reproducible without being regenerated. The catch worth writing down for whoever adds version 3: the *documents* inside a frozen file are MessagePack of the domain types as they were, so a domain change that breaks decoding is a migration that has to re-encode rows, not merely add tables |
| 2026-08-12 | S15 | **Coverage was raised by testing behaviour nobody had asked about, for the seventh session running.** The first measurement read 96.93 % with `store.rs` at 88.71 %. Two of the gaps were the session invariants the loader checks and the write-ahead log guard — neither of which had a test, both of which are real: a file whose session names a view it does not have, and a file system that cannot carry a WAL (an in-memory database says so, and so does a network share, which is exactly where a school would put its shows) | Six tests added, and one of them found the recovery-copy defect above. 99.47 % lines. What is left is ten lines: eight in the `#[ignore]`d regenerator, and two `?` arms — a table count and the encoding of one session — that no input can reach, the same residue S11 and S14 recorded. **The rule that keeps holding:** the uncovered line is where the defect is, and "it is only an error path" has now been wrong seven times |
| 2026-08-12 | S14 | **A journal of show snapshots cannot honour both sentences of §6.1 at once, and `SetExecutorMaster` is where the two collide.** The obvious Oops journal is a stack of two hundred copies of the show. §6.1's first sentence asks for a *compact* record "holding the inverse and the affected scope"; its second excludes playback actions, "so undo during a running show does not change light the operator is currently driving". An executor master **is** show state — it lives in `Executor::masterLevel` and is written by a command — and it is excluded from the journal. Snapshot the show and undoing a patch takes the fader the operator moved after it back down with it | The record is a scope: one fixture's patch entry for `PatchFixture`, one sequence plus the programmer and its page state for `StoreCue`, the programmer and its page state for the four other programmer commands. Six commands, and nothing else in the show is ever touched by an Oops. Asserted rather than argued: `a_running_executor_survives_an_oops` moves a master to 32768, starts an executor, records the engine's answer, and demands that the Oops take back **the patch** and leave all three alone. **S17 requirement:** the daemon has no journal of its own — it dispatches through `ShowFile::apply`, which is where `Oops` and `Redo` are carried out |
| 2026-08-12 | S14 | **A record holds two images rather than one inverse, and that is what makes Redo cheap rather than clever.** An inverse alone answers Oops; Redo then needs the state *after* the command, which would have to be derived by inverting the inverse | Both images are to hand at the moment the command is applied — one read before it, one after — and they are the same shape, so `restore(before)` is Oops and `restore(after)` is Redo, one function. The cost is that a `StoreCue` record carries two copies of one cue list; the alternative was a second code path that has to agree with the first about what an inverse means |
| 2026-08-12 | S14 | **The session's programmer page is inside the scope of a programmer command, and that does not contradict the exclusion of the session commands.** §6.1 excludes the eleven §4.4 commands so that an undo does not pull windows out from under the operator. But S13 made two §4.1 fields — `programmerPage` and `programmerParamIndex` — move as a *side effect* of a programmer command: a new selection resets the jog wheel, the third Clear resets both. Leaving them out of the scope would make "the state is the start state byte for byte" false | They are in the scope, and the reasoning is the same one S13 used to reset them: the index is a cursor into the parameters of a *selection*, so an undo that restored the selection and left the wheel pointing into it would restore half a state. The eleven commands themselves remain unjournaled, which `a_session_command_is_never_taken_back` asserts by applying all of them and finding the journal still holding one entry |
| 2026-08-12 | S14 | **A test fixture whose fields are all zero cannot tell a restore from a no-op — and that nearly hid the finding above.** The first version of `tests/oops.rs` built its file with the programmer page and the jog wheel at 0. Every reset in the code sets them *to* 0, so a journal that never recorded them at all passed the whole property test: restoring 0 over 0 changes nothing | The fixture now starts at page 3, index 5, with the reason written above it. **Checked by mutation:** dropping the page image from the scope turns three tests red with the new fixture and **none** with the old one. The general rule is worth stating, because it is not specific to this session: a fixture built out of default values cannot distinguish "restored correctly" from "never touched" |
| 2026-08-12 | S14 | **`Command::is_undoable` is the definition and `ShowFile::image` is a second statement of it, so the two are held together by a test.** The scope function is the fourth exhaustive match over all twenty-three commands in this crate, in a fourth direction — so a command *added* to the protocol is a compile error here as it is in the three appliers. What a compiler cannot catch is a command that exists in both lists and is classified differently in each | `a_command_has_a_scope_exactly_when_it_is_undoable` asserts `!image(c).is_empty() == c.is_undoable()` for all twenty-three and pins the undoable count at six. A command given a scope but left out of `is_undoable` would be journaled against the protocol; one added there and forgotten here would file an empty record that takes nothing back |
| 2026-08-12 | S14 | **A command that was accepted and changed nothing is not a step.** The obvious rule is "one command, one record", and it puts the operator in front of an Oops button that does nothing the first time they press it — so they press it again and lose the edit they actually meant to take back | A record is filed only when its before-image and after-image differ (`UndoRecord::is_a_step`). A refused command files nothing either, which follows from the write happening after the validation. Both asserted in `nothing_is_journaled_for_a_command_that_changed_nothing`, including the case that looks like a change and is not: repatching a fixture exactly where it already is |
| 2026-08-12 | S14 | **A refused undo has to leave the state byte-identical, and the record has to stay in the journal.** The failure is reachable rather than hypothetical: a cue part names a fixture, so unpatching that fixture (S27's patch sheet, or a hand-edited file) makes an older cue list something `Show::store_sequence` refuses to store | The show images are restored in their own pass **before** the programmer and the session, so a refusal happens before anything has been written; and the record is put back where it came from, so the same Oops goes through once the fixture is patched again. Asserted in both directions (`an_undo_that_is_refused_...`, `a_redo_that_is_refused_...`). **Checked by mutation:** dropping the record instead of putting it back turns the undo test red. The pass ordering is belt and braces *today* — the only record with a show image is `StoreCue`'s, and its desk half never moves — and it is documented as being kept for the scopes S27 and S28 will add |
| 2026-08-12 | S14 | **An undo is a command like any other from the outside, and the three ways it could quietly not be are all live.** A client mirrors state through deltas only; the engine rebuilds the patch on `Effect::Repatch` and a cue list on `Effect::ReloadSequence`; and the Save LED is a claim about the disk | All three are asserted. `tests/delta_round_trip.rs` walks a run that moved all three documents all the way back and all the way forward with three mirrors following. `undoing_a_patch_reports_the_repatch_and_the_change` demands `Repatch` and a `patch_revision` that moved **forward** — a rebuild is needed whichever direction the patch changed in, and a revision that went backwards would let a stale queued programmer command look current. And undoing back to the state that was last written leaves the lamp lit: `undoing_back_to_the_saved_state_does_not_clean_the_file` |
| 2026-08-12 | S14 | **The journal is not in the file, and a reopened show has nothing to undo.** A record is an assertion that the state *was* something. Restored from disk beside a file that may have been edited by hand since, its inverse would describe a show that no longer exists, and the first Oops would write it back | `#[serde(skip)]`, asserted by `the_journal_is_not_part_of_the_show_file`. **S15 requirement:** a loader that deserialises a whole `ShowFile` gets an empty journal for free; one that replaces `file.show` and `file.session` **in place** must call `Journal::clear`. Everything on `Journal` that adds or takes an entry is `pub(crate)`, so the field can be public — a caller can read it and clear it, and cannot invent history |
| 2026-08-12 | S14 | **Coverage was raised by deleting code the design does not need, for the sixth session running.** The first measurement read 99.73 % with `file.rs` at 98.31 %, and every uncovered line was unreachable by construction rather than untested: two "skip this image if it already matches" guards (a record is only filed when its images moved, so a show image in a record always differs) and the arm that removes a sequence whose image is absent (no command in `docs/IPC_PROTOCOL.md` §5 creates or deletes a sequence, and `StoreCue` refuses a missing one outright) | The guards were deleted and `Image::Sequence` lost its `Option`, which removed the branch rather than excusing it. 99.88 % lines, `file.rs` at 99.75 %, `journal.rs` at 100 % on all three counts, and `--show-missing-lines` reporting no uncovered source line at all. **S28 requirement:** a command that creates or deletes a sequence is when `Image::Sequence` grows an absence of its own, exactly as `Image::Fixture` already has one |
| 2026-08-12 | S14 | **The exit criterion says "*n* random commands", and the honest reading is *n* random **undoable** commands.** A non-undoable command interleaved into the run changes state the journal is forbidden to reach, so "undo *n* times and the state is the start state" would be false by design rather than by defect | The property generates from the undoable commands only — four parts commands this show can actually apply to one part `any::<Command>()` filtered, because an arbitrary `PatchFixture` names a profile no show has and a purely arbitrary run would journal almost nothing. The excluded half is covered by the two tests that exist for it, which is where it belongs: an executor that goes on running, and a session that does not move |
| 2026-08-12 | S13 | **The programmer is a third model beside the show and the session, and it is deliberately not in the file.** Where it lives was left open on purpose, and both obvious answers are wrong: inside `Show` it would travel as a `ShowPatch` (the protocol gives it its own delta), and inside `SessionState` it would travel as a `SessionPatch` and be part of the document `/session` points into | `prism_core::Programmer`, held by `ShowFile` beside the other two, `#[serde(skip)]`. `ARCHITECTURE_SPEC.md` §4.4 puts it there in as many words — "V1 has exactly one session and the programmer belongs to it" — so a multi-session daemon gets one programmer per session, the same change as the session map. **Not persisted, and the Save LED is the argument rather than taste:** setting a value is not an edit to the show (S11 asserted that `SetAttribute` leaves it clean), so a programmer inside the file would change the file's bytes without the lamp ever lighting — and a programmer restored from disk is an absolute override of every playback, applied to a rig the moment the show opens and asked for by nobody. Both halves asserted: the bytes are identical with a full programmer and an empty one. **S15 requirement:** the `.prism` schema has no table for it |
| 2026-08-12 | S13 | **Five commands are decided by two models, and the composition belongs in one place or it will be written three times.** S11 left `SelectFixtures`, `SetAttribute`, `ApplyPreset`, `ClearProgrammer` and `StoreCue` validated-but-unfinished, answering `Effect::Programmer`. Two of them also reach into a *third* model: a new selection resets the jog wheel, and the third press of Clear resets the page state, both of which are session state (S12) | `ShowFile::apply` carries out `Effect::Programmer`, drops it from the answer, and merges the deltas — so a daemon applying a command through the file never learns the work was split, and `Show::apply` is unchanged for a caller holding a bare show. The order inside it is S11's rule one level up: **the fallible, writing step goes first**, so a `StoreCue` the show refuses leaves the programmer byte-identical. `Programmer::apply` is the third exhaustive match over all 23 commands, in the third direction, so a command added to the protocol is now a compile error in three places |
| 2026-08-12 | S13 | **A store *merges* into the cue that is there, and `StoreCue` cannot say otherwise.** The programmer is sparse by specification, so a store carries only what was touched this time; overwriting would delete every value in the cue the operator did not happen to touch — data loss the command has no field to ask for. The same reasoning keeps an existing cue's name, times and trigger: a store is about the look | Merged, asserted, and written on `Programmer::cue`. A new cue starts with **no name and no fade**, because a store is a snapshot rather than a statement about time. **S28 requirement, and it is a protocol change:** Merge / Overwrite / Remove is a real distinction on a console, and offering it means adding a mode to `StoreCue` in `docs/IPC_PROTOCOL.md` §5. **Storing an empty programmer is refused** (`NothingToStore`) rather than writing a cue that does nothing — the S11 rule about silence, applied to the one gesture where an operator would otherwise get a cue number that goes dark |
| 2026-08-12 | S13 | **A relative move needs a base and `prism-core` cannot see the one the operator is looking at.** An encoder turn on an attribute the programmer is not yet holding has to start somewhere, and the value on stage is whatever the *playbacks* resolved to — which lives in the engine, one process boundary away | It starts from the attribute's **home** value, which is the bottom of the merge stack and the only base this crate can know, and from the held value once there is one. Both asserted, along with saturation at either end for a wheel spun hard. **S17/S27 requirement:** "grab what the playback is doing" is a different gesture, it needs the engine's resolved value, and it already has a name in the domain — `ProgrammerValueSource::Recalled`, which is the one of the three sources nothing produces yet |
| 2026-08-12 | S13 | **`ApplyPreset` applies to the selection, and a preset value for an unselected fixture is not applied.** The other reading — apply every value the preset holds — is tempting because a preset already names its fixtures, and it would make the command work with nothing selected | `ARCHITECTURE_SPEC.md` §6 and `docs/IPC_PROTOCOL.md` §5 both define it as "apply a preset to the current selection", and a preset holding a value *per fixture* is what makes that the useful reading: the operator selects the heads they mean and recalls the look stored for them. A value for a fixture that is not selected, or for an attribute the selected fixture's profile does not define, is skipped rather than refused — the same rule as `SetAttribute`, because a preset outlives the rig it was recorded on. **A manual change on top of a preset value breaks the link:** it is no longer the preset's value, so a later edit of the preset must not move it, and `presetRef` is exactly the promise that it would |
| 2026-08-12 | S13 | **The Clear stage resets on every interaction, which makes one of the five reset paths unreachable — and that is worth a test rather than a comment.** The rule in `docs/DMX_MERGE.md` §3.1 is "the stage resets to 0 on any other programmer interaction". Written per command it is four copies; written once it turns out that a *store* can never observe a non-zero stage at all, because the only way to one is a Clear and the first Clear has already emptied the programmer | The reset lives in one place — the successor every non-Clear edit starts from — and `a_store_can_never_meet_a_non_zero_clear_stage` asserts the unreachability instead of leaving a plausible-looking branch untested: the store is refused with `NothingToStore` and the stage does not move either. The rule covers the **direct** API as well as the command path, so an S27 fixture sheet writing a value has the same effect on the button as an encoder does |
| 2026-08-12 | S13 | **`Delta::ProgrammerChanged` carries the whole state, so nothing tells the daemon *what* changed — and the engine is addressed per slot.** S6 fixed that the tick receives one `MergePlan` slot at a time; the delta the protocol defines is the opposite shape, deliberately ("sent whole: it is small and sparse"), and it is the shape a *client* wants | **S17 requirement:** the daemon keeps the state it last sent to the engine and diffs against `Programmer::state()` to build `TickCommand::SetProgrammerValue`/`ClearProgrammerValue`, or reloads the whole layer through `MergeBody::load_programmer` — which allocates and is therefore set-up work, not something to do per encoder turn. **S16 requirement:** `docs/IPC_PROTOCOL.md` §4.1's `Snapshot` carries the show and the session and **not** the programmer, so a client connecting mid-programming would see an empty one; either the snapshot grows a third document or the daemon sends a `ProgrammerChanged` immediately after it |
| 2026-08-12 | S13 | **A programmer value the show can no longer resolve is dropped at store time and *reported*, which is what S6 asked for by name.** A fixture can be unpatched, or its profile replaced by a mode without that attribute, while the operator's value sits in the programmer; `Show::store_cue` would refuse the whole cue over it | `Programmer::unresolved` lists them, `Programmer::cue` leaves them out, and `ShowFile::apply` puts them in a `Delta::Notice` at `Warn` naming each one. A `presetRef` to a preset that has since been deleted is dropped the same way and the **value kept** — which is exactly what S11's `Show::remove_preset` already does to the cue parts that referenced it. **S27 requirement:** the programmer window shows an unresolved value in red rather than hiding it, because an operator who cannot see it cannot understand why it does nothing |
| 2026-08-12 | S13 | **`activeFeatureGroup` and the session's `encoderBank` are two fields that look like one, and defining the first as a copy of the second would have been a bug factory.** §4.1 gives the session an encoder bank; §6 gives the programmer an active feature group. Kept in step they are duplicated state; left alone they need a meaning | The programmer's group is the group of the attribute it last **touched**, filed under the profile's `featureGroup` rather than the attribute's name (S6). The encoder bank stays what the encoders are showing and is never written from here. `Programmer::feature_groups` is the related query — which groups the programmer holds values for at all, in encoder-bank order — and **S26 requirement:** that is what the encoder bar marks, so an operator can see they have touched colour while looking at the position bank |
| 2026-08-12 | S13 | **The short timing gate went red twice on the development machine, and both times the machine was busy with something that had nothing to do with PrismDMX.** The first failure came beside a `cargo llvm-cov` build; the second looked worse — `cargo test --workspace` alone, 45 of 132 ticks missed, median jitter 4.6 ms — and the tempting conclusion was S10's prediction coming true ("every new test target makes this window noisier"; this session adds the 29th). It was not: a game and a video were running on the same four cores. **The test said so itself, in the line it prints for exactly this purpose: `probe thread turns: 0/s`.** On a quiet machine the same tree passes all 846 tests, and CI's Windows job — the identical command — was green on the first attempt | **No code changed, and that is the finding.** The diagnostic S6 built into these tests is enough to tell "the engine is too slow" from "the machine had no core", and it was believed too late twice in one session. **The rule, for anyone reading a red timing test:** read the probe line first, then re-run the target alone (`cargo test -p prism-engine --test realtime`), and only then look at the diff. S10's prediction about the number of test targets is *not* confirmed by this session and stands as an open concern rather than a demonstrated one — which is worth stating, because a false confirmation would have justified loosening a gate that turned out to be right |
| 2026-08-11 | S12 | **The views live in the session, not in the show, and the protocol says so twice before the argument even starts.** `ARCHITECTURE_SPEC.md` §4.1 lists `activeViewId` in the session but never says where the views themselves are kept, and a view is stored content that looks like show content — the school builds three layouts once and uses them all term | `SessionState` holds them, keyed by view number. Three things point the same way: `docs/IPC_PROTOCOL.md` §6 annotates `SessionPatch` with "**views**, windows, pages, selection", so a view stored into the show would travel as the wrong delta *and* contradict the exit criterion; `Command::is_undoable` already excludes every session command from the Oops journal (S14), so a view stored into the show would be a show edit undo deliberately cannot reach, which is the asymmetry that turns into a bug report; and §4.4's closing note gives the reason multi-session exists at all — "two operators can work with **independent views**". **S15 requirement:** the views are in the session half of the file, so a show imported into another session does not bring another operator's layouts with it |
| 2026-08-11 | S12 | **A `SessionPatch` needs a document to point into, and the session is not one object but two.** JSON Patch is only meaningful against a root, and §4.1's `Snapshot { show, session, ... }` already treats show and session as separate documents | The session document is `{ "session": …, "views": … }`: `/session/executorPage` is the §4.1 state and `/views/3` is one stored layout. Views are keyed by **number**, for exactly S11's reason — a pointer into an array names a position, so a list would silently renumber every view after a deleted one. `SessionMirror` is the applier on the other end, and `the_two_mirrors_ignore_each_others_deltas` asserts that one connection's delta stream leaves each mirror with only its own half: a `SessionPatch` applied to the show document would fail on `/session`, and a mirror that guessed would corrupt itself |
| 2026-08-11 | S12 | **`ShowMirror` had to grow a sibling, and two copies of RFC 6902 would have been the third mistake of this kind in the project.** The session needs the same applier over a different document and a different delta | The engine is `JsonMirror`, public because a client with a document of its own needs it too; `ShowMirror` and `SessionMirror` are that engine plus the knowledge of which delta belongs to which document. The split cost nine delegating lines each and no behaviour: S11's mirror tests are unchanged and still pass |
| 2026-08-11 | S12 | **One Save LED, two sources — and only one of the eleven session commands is authoring rather than operating.** Paging the fader bank, focusing a window or moving the encoder bank changes state that is saved with the show, so the naive reading makes every console keypress an unsaved change. A Save LED that lit because somebody paged the faders would teach an operator to ignore it, which is the S11 finding about executors advancing, one level up | `SessionState::is_dirty` is set by `store_view` **and nothing else**, and the lamp itself is `ShowFile::is_dirty` — the two flags together. `ShowFile::apply` is where `Delta::DirtyFlag` is decided: it drops the one `Show::apply` raises for its own half and emits one only when the **pair** transitions, so a show edit behind an already-stored view does not light the lamp twice. `mark_saved` clears both, because a save writes both |
| 2026-08-11 | S12 | **A command that changes nothing must not produce a delta, and the jog wheel is the ordinary case rather than the edge case.** `SelectProgrammerParam { Prev }` at parameter 0 saturates; `SetExecutorPage` to the page already shown changes nothing. An empty `SessionPatch` broadcast to every client is a message that says nothing, at whatever rate an operator turns a wheel | Change detection lives in one place: every edit builds its successor beside the current state and hands it to `SessionState::commit`, which diffs field by field, encodes, and only then swaps. That is also where S11's "validate and encode before you write" is enforced for the whole module rather than per edit — `place_window` with a NaN coordinate is refused with the session untouched, asserted on the bytes. **The rule for the exit criterion:** all eleven commands are tested from a state in which each has something to change, and the no-op cases are tested separately for emitting nothing |
| 2026-08-11 | S12 | **§4.2's absence is structural everywhere except one open bag, and that bag is `params`.** `WindowInstance.params` is `Map<string, unknown>` by design — it is how a preset pool window says which pool it shows — so a client could put its scroll position in there and it would become shared state, silently, past a type that has no field for it | Checked at the door: `OpenWindow` refuses a parameter key containing `monitor`, `screen`, `scroll`, `hover`, `drag`, `camera` or `zoom`, matched on the lower-cased key. It is a deny-list and therefore not a proof — the module says so in place — but it turns the one hole in the criterion from an invitation into a refusal that names the rule. Note that a phaser editor's time zoom and a 3D camera are **genuinely** §4.2 state, so refusing them is correct rather than merely defensive. **S25 requirement:** window-local view state stays in the client |
| 2026-08-11 | S12 | **No §4.4 command carries a window's geometry, and position and size are session state all the same.** §4.1 puts "type, position, size on the canvas" in the session; §4.4's eleven commands open, close and focus windows and never move one. Read literally, a window could only ever have the default geometry the daemon chose for it | `SessionState::place_window` is the direct API, validated and delta-emitting like every other edit, and `DEFAULT_WINDOW` is where a console-opened window lands. **S25 requirement, and it is a protocol change:** dragging a window on one screen has to move it on every other, so the canvas needs a command carrying geometry — an addition to `docs/IPC_PROTOCOL.md` §5, not something the UI can do locally without breaking D3 |
| 2026-08-11 | S12 | **The executor page is validated and the programmer page is not, and the asymmetry is the S1 finding rather than an oversight.** `SetExecutorPage` carries an arbitrary `u32` from any client and `ExecutorId::from_page_and_slot` multiplies it by eight **saturating** — so above `(u32::MAX − 7) / 8` all eight slots of a page fold onto one executor, which is a fader bank where every fader drives the same thing | Refused as `ExecutorPageOutOfRange`, written as a round trip — `from_page_and_slot(page, 7).page() != page` — so the check is the saturation itself rather than a second copy of the arithmetic. Both boundaries are asserted: 536 870 911 is accepted and its last slot is exactly `u32::MAX`, 536 870 912 is not. The **programmer** page is accepted unbounded, because nothing is derived from it by arithmetic that can fold two pages into one; what is on a page at all is the programmer's question, and that is S13 |
| 2026-08-11 | S12 | **`SelectExecutor` is deliberately not validated against the show, where S11's playback commands are.** The obvious symmetry — an executor the show does not have is a rejection — would break the ordinary way an operator assigns a sequence: select the empty slot, then store into it | Accepted for any executor number, and the direct API takes an `Option` so the daemon can *clear* a selection when the executor it names is deleted. The distinction is worth stating: a Go into an empty executor is a command that cannot do anything, and selecting one is a command that has already done everything it claims to |
| 2026-08-11 | S12 | **A window number is derived from the session's own content rather than counted, and one number is therefore reusable.** A serialised counter would have to be restored above the content it numbers or it hands out a number a loaded view already uses | The next number is one past the highest in use **anywhere** — the canvas and every stored view — so a number a view remembers is never reissued and loading a view can never collide with what is already open. A number no view remembers *is* reused once its window closes, which is a deliberate trade: the race it leaves is a stale `CloseWindow` closing the wrong window, which the operator sees and undoes by opening it again. `NoWindowNumberLeft` is the refusal when the space really is exhausted, rather than a second window quietly sharing a number |
| 2026-08-11 | S12 | **`CommandLineInput` carries the whole line, not a keystroke, and `SelectView` always reloads.** Both are readings the protocol leaves open, and the other reading is broken in each case: an append-only command line has no backspace and no clear, and a `SelectView` that skipped the reload when the view was already active would take away the gesture an operator uses to get their layout back after moving things around | Written on the two methods. `activeViewId` therefore names the view last **selected**, not a promise that the canvas still equals it — which is what `StoreView` exists to reconcile, and why storing a view does not change the active one. Two invariants come with it and are asserted as properties over arbitrary command sequences: the focused window is always one that is open (the focus follows a loaded layout, and falls to the next in front when the focused window closes), and the active view always exists — which is why a fresh session already has one |
| 2026-08-11 | S11 | **The sACN CID belongs to the machine, not to the show — and the question S10 left open has only one safe answer.** Storing it with the show fails at exactly the point that makes a second desk exist: copying a show *is* how a second machine comes to have one, on a stick, from a backup, or as the school's template for next term, and nothing in a show file can tell "this is the same desk" from "this is a copy". Two senders under one CID look to a receiver like one source contradicting itself, and no priority sorts that out | `prism_core::MachineConfig`, holding a `DeskId`, written beside the daemon's own settings and **never inside a `.prism` file**. A show arrives on a new machine with no identity in it and picks up the identity of the desk that opens it. Three rules follow and are written on the module: **S15 must not write it into the show and must not regenerate it on save** — `Show` has no field for it, which is the structural half, and `desk_id_is_not_show_content` is the asserted half; **S17 generates it once** on first start, because a UUID needs entropy and this crate is dependency-free by rule; and `DeskId::NIL` is **not** an identity, so `MachineConfig::is_configured` asks one layer up the same question `prism-protocols` already refuses to connect without |
| 2026-08-11 | S11 | **One edit wrote the show before the last thing that could fail had run, and coverage is what found it.** `store_cue` inserted the cue into the sequence, sorted the list, and *then* built the JSON Patch operation — and building it is fallible, because `prism-domain` refuses a non-finite float on the way out (S1). A cue with a NaN fade time would have been stored **and** reported as an error. The only uncovered line in the crate was that `?`, which is what pointed at it; no test had reached it because nothing else in the suite constructs a NaN | The list is built beside the old one and swapped in only once the operation describing it exists. **The general rule, now written on the module: validate and encode before you write, because the encoding is the last thing that can fail.** Every other operation already had this shape; this one did not, and the byte-identical criterion was true by accident everywhere else. Three tests now drive the NaN path through `embed_fixture_type`, `store_cue` and `patch_fixture` |
| 2026-08-11 | S11 | **`Command::ApplyPreset` carries a preset number and no pool, and `Preset.pool` files presets into pools where the number restarts.** Read literally, the two together make the command ambiguous: preset 4 in the colour pool and preset 4 in the position pool are different presets and the command cannot say which | **Preset numbers are unique across pools.** The pool is how a preset is filed and which encoder bank shows it; it is not part of its identity. That is the only reading under which `ApplyPreset` is well-defined, and it matches `PresetId` being a plain number on the wire. Written on `Show::store_preset`, which refuses a second preset with a number already used in another pool. **S28 requirement:** the preset pool UI numbers presets globally, not per pool |
| 2026-08-11 | S11 | **A JSON Pointer into an array names a position, so a show whose collections were lists would rename things when one was deleted.** `/fixtures/3` would be the fourth *element*: remove fixture 1 and every pointer after it silently means a different fixture, which is a class of bug that survives review because every pointer still resolves | Every collection is an object keyed by the identifier the operator uses, so `/fixtures/3` is fixture 3 whatever else happens. Two consequences that had to be checked rather than assumed: an integer-keyed map **does** round-trip through both codecs here (unlike S1's finding, where the map was inside an internally tagged enum — a patch operation carries its key inside the pointer string, so the problem does not arise), and a profile key is **operator text**, so the pointer has to escape `~` and `/` per RFC 6901. The property test generates profile keys containing both, and removing the escaping fails it in one shrink step |
| 2026-08-11 | S11 | **"Patch conflicts are detected and reported, not silently accepted" is not the same as "rejected", and reading it as rejection would have broken a technique in daily use.** S4 settled the engine half — a second fixture on the same address is how an operator clones one, and the higher fixture number wins the shared channel because its targets are written last | `Show::conflicts()` is a list the patch sheet (S27) can show: universe, first and last shared channel, both fixture numbers and which one wins. `Show::apply` puts the overlaps a patch *creates* into a `Delta::Notice` at `Warn`. `tests/show_to_engine.rs` asserts the winner the show names is the byte the engine actually writes, in both orders, so the two cannot disagree about the rule while agreeing about the words. `Show::issues()` reports the same way for references that have gone stale — a group member, a preset value, a cue part or an executor's sequence — which S5 and S6 both asked for: dropped silently, an operator cannot learn why something does nothing |
| 2026-08-11 | S11 | **Delta generation cannot be verified without an applier, so one exists.** "A client that has applied every delta since its snapshot holds state identical to the daemon's" is a claim about two pieces of code, and a test that only checks the generator checks nothing | `ShowMirror`: RFC 6902 over `prism_domain::JsonValue`, plus `Delta::ExecutorState`, which carries show state without being JSON Patch and which a mirror that ignored it would drift on. It is not test scaffolding — the Web Remote (S31) and any Rust client mirror the show exactly this way. **Deliberately not all-or-nothing:** clients apply deltas without validating them because the daemon has already decided, so an operation that does not fit means the two have *already* diverged; `apply_all` stops at the first failure, names it, and the client re-snapshots. Copying the whole show per delta to protect a case in which the mirror is already wrong would buy nothing |
| 2026-08-11 | S11 | **A repatch must not move a fixture back to the origin, and `PatchFixture` carries no geometry.** S1 defined the command as the five fields an operator supplies at patch time, with position, rotation and the inverts edited afterwards. Building a fresh `Fixture` from it — the obvious implementation — would un-hang a moving head from the ceiling and reset its place in the 3D view every time somebody corrected its address | `Show::apply` keeps the geometry and both inverts of the fixture already there and replaces only what the command carries. Asserted. **S27 requirement:** the patch sheet's address column edits the address, and must not round-trip a whole fixture through this command |
| 2026-08-11 | S11 | **`Show::apply` returns effects rather than performing them, and that is what lets five commands owned by later sessions still be *decided* now.** The show model has no engine, no journal and no file. A command it cannot finish could have been left to the daemon entirely — and then "every command applies or rejects" would have been a criterion about four commands | `Applied { deltas, effects }`. `Effect::Programmer` (S13), `Undo`/`Redo` (S14), `Save` (S15), `Repatch`, `ReloadGroups`, `ReloadSequence`, and the three playback ones. **S17 requirement, and S5 asked for it in advance:** an executor with a cue list is *played* through `MergeBody`, never poked into `PlaybackLayer` — `Effect::ExecutorGo`/`ExecutorOff` is the validated intent, not the mechanism. **`Effect::Repatch` is two jobs, not one:** rebuild the `MergeBody` *and* blank the publisher's frame buffers, because the encoder leaves unpatched channels alone (S4). `Show::patch_revision()` is the number that moves, and it is what a daemon compares against to know its queued programmer commands — addressed by merge-plan slot (S6) — are stale |
| 2026-08-11 | S11 | **`prism-engine` became a development dependency of `prism-core`, which is a smaller commitment than it looks and buys the one thing an argument could not.** The show validates a patch against `Fixture::last_address` and its own profile checks; the engine validates again when it builds the channel plan. Two copies of a rule drift, and a show that could be saved and then could not be played would be the worst way to find out | Dev-dependency only, so nothing at run time can reach the engine from here — the daemon still owns the wiring. `tests/show_to_engine.rs` asserts over arbitrary patches that everything the show accepts, `MergeBody::for_patch` accepts, and pins the three doors S17 will use: `FrameLayout` from `Show::universes()`, `for_patch` from `Show::patched()`, `load_groups` and `load_sequence` from the show's own pools |
| 2026-08-11 | S11 | **An executor the show does not have is a rejection, not a no-op.** Executors are stored, not a fixed grid, so a fresh show has none and a master move or a Go names something that does not exist. Silence is the tempting answer and the wrong one | `UnknownExecutor` and `ExecutorHasNoSequence`. "The Go did nothing" is a complaint an operator cannot diagnose; "executor 3 has no sequence" is one they can. **S21/S26 requirement:** the surface and the executor bar only send playback commands for executors that exist, and show an empty slot as empty rather than sending into it |
| 2026-08-11 | S11 | **The dirty flag travels when it changes, not on every edit.** `Delta::DirtyFlag` drives the console's Save LED, and an LED cannot be lit twice | `Show::apply` emits it only on the transition to unsaved, and `Show::mark_saved` returns whether it actually cleared anything. Deliberately **not** set by `record_executor_state`: an executor advancing a cue is not an unsaved edit, and a Save LED that lit because a cue followed on would teach an operator to ignore it. **S15 requirement:** `mark_saved` is called when the write has succeeded, not when it starts |
| 2026-08-11 | S10 | **The short timing gate failed a third time, and the diagnostic finally said why: `probe thread turns: 0/s`.** It went red on the Windows job of this session's push — p95 **210 ms**, 23 of 133 ticks missed, median still 100 µs, identical tree green on a rerun — on a commit that touches no code in `prism-engine`. The probe line is the part that matters: the machine had **no core to spare at all**, which is the precondition of the measurement rather than a property of the schedule. `cargo test` runs a workspace's test binaries **in parallel**, and this commit adds one more (`sacn_wire`) — so S6's finding that "two timing tests in one binary measure each other" holds one level up as well, across targets, where a mutex cannot reach. The previous session had already loosened this bound from 5 ms to one tick period; 210 ms went straight through it | **The tail assertion is gone from the three-second run**, and what remains is what the sample count supports: the **median** (a schedule that drifts, sleeps by period instead of to a deadline, or wakes late every time moves it, and by far more than the margin) and the **share of the grid that ran** (≥ ¾). The distribution is still printed, so a strange run can be read. The tail number that means something is unchanged and lives where its 26 401 samples are: p99.9 = 200 µs in the ten-minute run (§3). **Rule, now for the third time and hopefully the last:** a percentile is a gate only where the sample count supports it — p95 over 130 samples is the seventh-worst sample, and on a shared two-core runner the seventh-worst is whatever else the runner was doing. **For later sessions:** every new test target makes this window noisier, so a wall-clock assertion that has to be true on CI belongs in the ten-minute runs, not beside the suite. Verified green on the first attempt as run **31506271867** |
| 2026-08-11 | S10 | **A CID has to survive a restart, and there is nowhere yet to keep one — so the crate refuses to invent it.** E1.31 receivers track sources by CID. A desk that generates a fresh UUID at start-up is a *new source* at every start, and the old one goes on holding the universe until its 2.5 s network-data-loss timeout expires — two and a half seconds in which two equal-priority sources fight. Two desks *sharing* a CID is worse still. The tempting default, "generate one at start-up if none is configured", is exactly the failure | **No constructor in the crate produces a CID.** It is `SacnConfig::cid`, parsed from the canonical UUID text a configuration holds, and the default is nil — which is not an identity, so an output holding it **refuses to connect** rather than transmitting under the value every unconfigured desk would share. Both halves are asserted, and stability is by construction rather than by luck. **S11/S15 requirement, and it is a hard one:** the show file (or a machine-level configuration beside it) must carry the CID, and it must survive a show being copied to another machine *without* two machines ending up with the same one — a CID belongs to the **desk**, not to the show. Whichever of the two it is stored in, S15 must not regenerate it on save |
| 2026-08-11 | S10 | **A stopped stream and an ended stream are different things, and only one of them is instant.** A receiver that simply stops hearing from a source waits out 2.5 s before releasing the universe, so a desk that closes its socket leaves the rig holding a dead source's look. E1.31 has a goodbye and Art-Net has none — which is why `DmxOutput::shutdown` earning its place is a finding rather than a formality: S7 put it in the trait to close a cable, and it turns out to be where a protocol's ending belongs | Three `Stream_Terminated` packets per active universe on `shutdown`, **each with the next sequence number**. That last part is the whole point: a conforming receiver discards a packet whose sequence has not moved on, so three identical packets are one packet and two duplicates — the redundancy the standard asks for would be worth nothing. Asserted on the driver and again on datagrams received over loopback after `OutputRunner::run` returns |
| 2026-08-11 | S10 | **The terminated packet carries the last look, not a blackout, and that is a decision about who owns the question.** "The desk is stopping" and "the stage should go dark" are different statements, and a driver that blacked out on the way out would take the choice away from the level that has it | The data is unchanged and a receiver ignores it in a terminated packet anyway. `IMPLEMENTATION_PLAN.md` S17 already owns "shutdown with sACN termination and **configurable blackout-or-hold**": the daemon publishes a blackout frame *before* stopping the outputs if that is what the operator configured. **S17 requirement:** blackout is a frame, not a shutdown flag |
| 2026-08-11 | S10 | **Multicast is the sACN default where broadcast is the Art-Net exception, and that is not an inconsistency.** The obvious reading of §7.2 — "network floods are the danger, so make the many-recipient mode opt-in" — would have made sACN unicast by default and broken the protocol's ordinary deployment | An sACN group carries **one universe**, so a switch that does IGMP snooping delivers it only to ports that asked for that universe; Art-Net's broadcast carries everything to everyone regardless. The difference is the switch, not the packet count. `SacnDestination::Multicast` is the default, unicast is available for venues that forbid multicast, and **broadcast does not exist** — the socket is never asked for the permission, asserted for both destinations |
| 2026-08-11 | S10 | **A multicast *sender* needs one socket option and none of the rest.** The instinct at the seam was to add group membership — `join_multicast_v4` — and it would have been dead code: joining is how a **receiver** asks to be given datagrams, and this seam has no receive. What a sender does need is the hop limit, which defaults to 1 on every platform and silently confines the show to the local segment | `UdpSender::set_multicast_ttl`, the only addition to the S9 seam, plus `SacnConfig::multicast_ttl` (default 1) as data. A hop limit that could not be set is a **disconnected** output rather than a warning: the operator asked for a routed lighting network and would otherwise get datagrams that stop at the first router, under a green light. The outgoing interface needs nothing new — it is the `bind` address, which the seam already had |
| 2026-08-11 | S10 | **No test may send a multicast datagram, which costs the one thing a loopback capture cannot check.** `tests/artnet_wire.rs` proves its bytes on a received datagram; the same trick with sACN's actual destination would put lighting data on the network the test suite is running on, and on Linux the loopback interface is not even a multicast interface | The wire target unicasts to a loopback socket — E1.31 permits unicast, so it is a real configuration and not a test-only mode — and the **group address per universe** is asserted against a recording socket instead, over the whole 1…63999 range. Recorded in §5 as the open verification item a real switch answers: IGMP snooping and whether a hop limit of 1 reaches the venue's gateways |
| 2026-08-11 | S10 | **`last` should mean "what the receiver has", not "what the driver last built", and S9's version conflates them.** Art-Net records the look *before* the send and clears the timestamp if the send fails; that works, but it also means a look that failed and was then changed back is re-sent, and it left `last` holding data no receiver ever saw. The difference surfaced as a failing test of the **termination** packet, which must carry the look a receiver is actually holding | In `sacn.rs` nothing is written down until the datagram has gone out: `last` and `sent_at` describe the last datagram that really left. Suppression still cannot hide a failure — the record still shows the older look, which differs from the new frame — and a look that failed and reverted is correctly *not* re-sent. The sequence number is spent either way, which a receiver reads as a packet lost in the network, because it was. **Not back-ported to `artnet.rs`:** S9's behaviour is correct, merely coarser by one redundant datagram in a case that ends the same way, and reopening a closed session's driver to make two implementations rhyme is not worth the risk to a verified output |
| 2026-08-11 | S10 | **E1.31's sequence wraps 255 → 0 and Art-Net's wraps 255 → 1, and the two rules look like the same rule.** Copying S9's counter would have skipped 0 forever — harmless-looking, and wrong: in Art-Net 0 means "this sender does not number its packets", in E1.31 it is an ordinary value | Counted per universe, `wrapping_add(1)`, asserted over 300 frames with element 256 read back as 0. The two drivers' counters are deliberately different and each says why in place. **The generalisable part:** the second implementation of a family of protocols is where copied assumptions go unnoticed, because the code looks right |
| 2026-08-11 | S10 | **A 64-byte source name is a UTF-8 field, and truncating it by bytes can cut a character in half.** "Aula Bühnenlicht" and any German rig name is two bytes for the umlaut; a name whose 63rd byte lands mid-character would reach a receiver as a replacement character or be rejected outright | `source_name_field` truncates on a **character** boundary — 31 two-byte characters, not 63 bytes and a broken one — and always leaves the null terminator. Asserted with a name of 40 `ä`, decoded back as UTF-8 in the test so the assertion is about validity rather than about a byte count |
| 2026-08-11 | S10 | **The Sync Address field exists and is transmitted as zero, deliberately.** Offering it as configuration without implementing E1.31 synchronisation would be a setting whose only effect is a dark rig: a receiver given a non-zero sync address waits for synchronisation packets before it acts on data, and this source sends none — the same trap as Art-Net's ArtSync, one level worse because there is no way to notice | The field is built rather than hard-coded, so it is asserted at a non-zero value at the packet level, and the output always passes 0. `Force_Synchronization` is never set for the same reason. **A later session** that implements universe synchronisation turns the constant into configuration, and owns sending the sync packets in the same change |
| 2026-08-11 | S9 | **The flake S6 predicted in writing arrived, on a documentation-only commit.** `the_tick_holds_its_deadline_for_a_few_seconds` failed the Windows job of run **31495900648** with `p95 jitter was 16ms` against a 5 ms bound — 16 ms being one Windows scheduler quantum. The commit under test changed **one markdown file**; the identical tree was green in run 31495561058 twelve minutes earlier, the median was at its usual value, and a rerun of the same commit with no change at all passed. S6's note said this in advance: "S2's short run still carries a 2 % missed-tick budget and the same exposure; if it starts flapping this is the reason and this is the fix" | S6's remedy applied to S2's run. p95 over ~130 samples is the seventh-worst sample and on a shared two-core runner that is a scheduler stall, so the tail bound becomes **one tick period** — still failing a schedule that puts a twentieth of its ticks in the next slot, deaf to a tick that was late and whose frame went out anyway — and the 2 % missed-tick budget becomes **the share of the grid that ran** (≥ ¾), which is what an engine too slow for its own period actually destroys. The real tail number is unchanged and lives where the samples are: p99.9 = 200 µs over 26 401 ticks in the ten-minute run (§3). **Rule for later sessions:** a percentile is only a gate where the sample count supports it; a CI run of a hundred-odd samples can carry the body of a distribution and never its tail. Verified green as run **31497341333** |
| 2026-08-11 | S9 | **"Refresh at least every 800 ms" cannot be implemented as "refresh when 800 ms have passed", and the difference is a whole cadence.** A driver only gets to decide at its own wake-up: asking `elapsed >= 800 ms` puts the datagram at the first cadence *after* 800 ms, so the real gap is 800 ms + one period — 818 ms at 44 Hz, and worse on any slower output. The guarantee is a maximum, and the obvious implementation misses it every single time | `ArtNetConfig::refresh_margin`, default one engine tick, and the rule is `elapsed + margin >= interval`. The test is the criterion rather than a proxy for it: ten seconds of a static rig through the real `OutputRunner`, measuring the **gap between datagrams**, asserted ≤ 800 ms — and ≥ 700 ms, so a driver that "fixed" it by sending every cadence would fail as well. **S10 requirement:** sACN's keep-alive has the same shape and the same trap |
| 2026-08-11 | S9 | **ArtSync needs to know when a frame is finished, and `DmxOutput` has no such call.** The trait is five per-universe methods; the runner sends universe after universe and never says "that was the last one". Adding a sixth method was the obvious answer and was rejected — three drivers implement the trait, `OutputRunner` is written against it once, and S7 chose those five deliberately | The driver finds the end of a cycle itself, two ways: normally it is the **last universe in its own list**, and if that one never arrives — the engine does not publish it, so the runner skips it, which is exactly the `unmapped()` case S7 already reports — it is the moment a universe **comes round a second time**. Both paths are asserted. The trait is untouched, which was the constraint |
| 2026-08-11 | S9 | **Sending an unchanged universe every cadence is the broadcast flood wearing a smaller hat.** §7.2 makes broadcast opt-in because 530 bytes × 44 Hz × universes to every machine on the segment is a denial of service performed by the lighting desk — but the same arithmetic applies to unicast at 64 universes: 1.5 MB/s of "nothing has changed" | A universe goes out when its data changes and otherwise on the forced refresh, which is what `ARCHITECTURE_SPEC.md` §7.2's 800 ms is *for* rather than an extra. Measured: a static rig produces 13 datagrams in ten seconds where the runner sent 440 frames. **Consequence for the UI (S27):** `OutputStatus::frames_sent` counts frames the runner handed over, not packets — `ArtNetOutput::datagrams_sent` is the second number, and a status panel that showed only the first would be describing the wrong thing |
| 2026-08-11 | S9 | **A datagram that failed was not sent, and the suppression has to be told.** With change detection, a refused send that had already recorded "this look is on the wire" would leave the failed look sitting out the whole refresh interval — up to 800 ms of the wrong picture behind a green light, because the next frame is identical and therefore suppressed | A failed send clears that universe's record, so the next frame goes out whatever it contains. Same on reconnect, for the same reason one level up: a node that has been away must not be told "nothing has changed since a datagram you never received". Both asserted |
| 2026-08-11 | S9 | **`ARCHITECTURE_SPEC.md` §7.2 names the `artnet_protocol` crate, and no dependency was added.** ArtDmx is an 18-byte header and 512 bytes of data; the seam that would let an external crate's types be asserted against the specification is larger than the code it would replace, and the exit criterion is a **byte-for-byte** assertion against the document — which is much easier to trust when the bytes are written in one place beside the field names | The packet is built in `artnet.rs`, spelled out field by field, and every literal is asserted against the constant it must equal so the two cannot drift. §7.2 updated. The dependency count of the crate is unchanged: `prism-protocols` still pulls in nothing platform-neutral beyond the two workspace crates. **S10 note:** the same question arrives for `sacn`, and E1.31 has a root layer, a framing layer and a DMP layer plus a CID — bigger, but the same reasoning applies to the assertion, not automatically to the dependency |
| 2026-08-11 | S9 | **The clock had to go into the output, not just the runner.** The 800 ms refresh is the driver's decision and `send_frame` carries no time; the runner's clock is the runner's | `ArtNetOutput` is generic over `prism_engine::Clock` exactly as `OutputRunner` is, defaulting to `SystemClock`. In the end-to-end test the two share one clock, which `ManualClock` cannot do — it is a `Cell` and belongs to one owner — so `tests/artnet_wire.rs` defines a `SharedClock` over a mutex. Worth knowing before writing the same test again in S10 |
| 2026-08-11 | S9 | **PrismDMX numbers universes from 1 and Art-Net numbers port addresses from 0, and nodes disagree about which one their front panel means.** Guessing either way produces a rig where every universe is off by one, which looks like a patch error rather than an addressing one | Default mapping is universe N → port address N − 1, documented and asserted; the mapping is data (`ArtNetOutput::with_ports`) so a node that counts the other way is a configuration change. Recorded in `ARCHITECTURE_SPEC.md` §14 as an open verification item that blocks nothing, because it is data. **S27 requirement:** the output editor shows the port address as `net:sub:universe`, which is what a node's display shows |
| 2026-08-11 | S9 | **ArtSync is specified as a broadcast, and this output does not broadcast it.** One 14-byte packet tells every node on the network to display what it is holding, which is why the specification broadcasts it — but sending it that way would mean switching ArtSync on silently turned a unicast configuration into a broadcasting one | The sync goes to the addresses the data went to. §7.2's "unicast by default" is the stronger requirement of the two, and a controller unicasting to a known set of nodes is exactly the case where the sync can be unicast as well. Written on `ArtNetConfig::sync` and asserted |
| 2026-08-11 | S8 | **The break was landing inside the frame, and every one of S7's 92 tests passed while it did.** A fixture on the line strobed, went dark, or ran its own programme, and the driver looked perfect. The measurement that settled it: `FT_Write` returns after **20.0 ms** of a frame that takes **22.572 ms** to transmit, and `FT_GetStatus`'s transmit queue reads **zero on the first poll of every single frame** — it reports the *driver's* queue, not the chip's shift register, so the "wait for the queue to drain" written first was waiting for nothing. A DMX break is a USB **control** transfer and does not queue behind bulk data, so it was being asserted on top of the tail of the frame still going out | The backends wait out the **computed** transmission time — `bytes × 11 bits ÷ baud`, which is arithmetic and does not lie — plus a 2 ms margin, before returning from `write`. The contract is now written on `FtdiBackend::write`: *must not return before the bytes have left the port*. Both backends honour it, `transmission_time` is a tested pure function, and the fixture went from strobing to steady. **S9/S10 requirement:** ArtNet and sACN have no break and no such hazard, but the lesson generalises — a mock asserts the calls a driver makes, never the time between them |
| 2026-08-11 | S8 | **43 Hz was the symptom, not the good news.** Before the fix the driver measured 43.1 Hz, comfortably above `ARCHITECTURE_SPEC.md` §7.1's 30–40 Hz estimate, and that should have been the first clue: 513 bytes at 250 kBaud is 22.572 ms, so 23.2 ms per frame leaves 0.6 ms for a break sequence that measured **3 ms**. The data was still flowing *during* the break | The corrected driver runs at **35.5 Hz**, and the arithmetic closes: 3 ms of break transfers + 22.6 ms of data + 2 ms of margin = 27.6 ms. §7.1's estimate was right about the band and wrong about why. **Recorded as a rule:** on this hardware a frame rate above about 40 Hz means the frame is not being framed |
| 2026-08-11 | S8 | **D2XX is the preferred path, and the rate is not the reason.** Both paths drive the fixture correctly, and the virtual COM port measured *faster* — 38.4 Hz against 35.5 Hz, which is entirely the safety margin D2XX is given | D2XX wins because it can **configure the port at all**: the latency timer and the USB transfer sizes are reachable through it and unreachable through any serial API. On the bring-up machine the registry held the FTDI default of **16 ms**, which §7.1 calls fatal, and nothing in `serialport` can change it. A path that cannot set the settings that matter is a fallback, not a default. Written up in §7.1 |
| 2026-08-11 | S8 | **A fixture is a poor oracle, and an hour went into learning it.** Sweeping one channel at a time made the head "blink red twice"; ramping every channel made it light up. Neither was a response to the values — the manual (Monacor WASH-42LED, 13-channel mode) puts a **strobe band at 135–239 on the dimmer channel** and **auto programmes on channel 13**, so a ramp crosses the strobe band twice on its way up and back, and an all-channels ramp drives the fixture into its own programme | The bring-up target grew `PRISM_DMX_HOLD`, and the rule is written beside it: **hold the master, drive one channel**, never blind-ramp a whole universe. Get the fixture's channel table before drawing conclusions from it. **S27 requirement:** the patch UI should show the fixture type's channel table beside the fixture, because this is the operator's version of the same hour |
| 2026-08-11 | S8 | **A slow fade on an 8-bit channel is visibly stepped, and that is the protocol rather than a fault.** 256 values over 12 seconds is 47 ms a step | Confirmed rather than assumed, by running the same fade in 3 s where the steps vanish. It is also why `prism-engine` interpolates fades in 16 bit (S5) and why a 16-bit dimmer is worth patching where a fixture offers one. **S28 requirement:** a fade time long enough to expose 8-bit steps is a thing the cue editor could warn about |
| 2026-08-11 | S8 | **`LNK4098` is a rustc lint, and CI runs `-D warnings`.** FTDI's static D2XX library is built against the static C runtime while everything else uses the dynamic one, so the linker reports a conflict — and `linker_messages` is a *lint*, which means the Windows job would have failed on this commit rather than warned | `/NODEFAULTLIB:LIBCMT` in a new `.cargo/config.toml`, and repeated in the Windows CI job because `RUSTFLAGS` from the environment **replaces** the config's rustflags rather than adding to them. Static linking is kept deliberately: `ftd2xx.dll` ships with FTDI's driver, and a dynamically linked daemon on a machine without it would fail to start *at all* rather than falling back to the COM port — the import is resolved when the process loads, long before any of our code can decide anything. **S29 requirement:** `-C target-feature=+crt-static` would make the two runtimes agree *and* remove the VC++ redistributable from what a school has to install; that is a decision about the whole workspace and belongs with the shell |
| 2026-08-11 | S8 | **Four hardware tests failed inside a third of a second, racing for one cable.** `cargo test` runs a target's tests on several threads and there is exactly one adapter | A `static ADAPTER: Mutex<()>` held for the whole of every test that touches it — the same remedy S6 applied to the timing tests, for the same reason, and preferred over a note telling people to pass `--test-threads=1` |
| 2026-08-11 | S8 | Coverage cannot mean one number for a crate that is half FFI | Recorded as **two**: 96.94 % lines with no adapter attached, which is what CI reproduces, and 99.30 % with one, running the same suite under `--include-ignored`. The gap is 70 lines of library calls that no build server can execute, and hiding it behind an `--ignore-filename-regex` would have made the crate look better and mean less |
| 2026-08-11 | S8 | Linux still has no FTDI backend, and pretending otherwise would be worse than saying so | `FtdiError::Unsupported` and `UnsupportedBackend`: the crate builds, links and answers everywhere, and a Linux daemon says clearly why it cannot send rather than failing to compile or hitting a `todo!()`. It is deliberately **not** a lost link, so a runner does not spend its life reconnecting to a platform that will never have one. **A later session** writes the libftdi path, on the Raspberry Pi it is meant for (D10) — the same rule as S7: no backend without a device to verify it against |
| 2026-08-11 | S7 | **No real FTDI backend was written, and that is a decision rather than an omission.** The plan permits platform code in this crate and `ARCHITECTURE_SPEC.md` §7.1 names the crates — `libftd2xx` on Windows, `rusb`/libftdi on Linux. Neither can be exercised by anything: no test may require hardware (`CLAUDE.md`), no CI job has an adapter, and `cargo clippy --workspace --all-targets` does not build code behind a non-default feature. A backend written now would be untested code against a device with an **unverified product ID**, shipping under a green tick | S7 delivers the seam and everything above it; **S8 owns the first real backend**, with a cable on the bench to check it against, and starts by verifying the descriptor it is supposed to open. The confinement §10.1 asks for is already in place and already tested: `AccessPath::preferred()` is the only `#[cfg]` in the crate, both branches are asserted, and `prism-protocols` now runs its tests in the **Linux** CI job as well — which is what will fail on the commit that lets platform-specific logic leak above `FtdiBackend` |
| 2026-08-11 | S7 | **`ARCHITECTURE_SPEC.md` §7's trait is three methods and a driver thread needs five.** The same section requires "exponential reconnect backoff" from every driver, and a reconnect that only the concrete type knows how to perform cannot be expressed in a thread body written once for every kind of output. Nor can "send the frame this output is for" without asking which universes it carries | `DmxOutput` adds `connect`, `shutdown` and `universes`. The gain is that `OutputRunner` is one piece of code for Open DMX, ArtNet and sACN alike, so S9 and S10 inherit the backoff and the panic containment instead of each writing their own. **S9/S10 requirement:** a network output implements the same five, and the runner is not rewritten |
| 2026-08-11 | S7 | **The break delay had to become a backend call, and `thread::sleep` cannot implement it.** The break is 110 µs and the mark-after-break 16 µs; the Windows timer granularity is between 1 ms and 15.6 ms, so a sleeping driver would spend most of a frame period in the two delays and cap the output somewhere in the teens of hertz. It also has to be *observable*, or the exit criterion's "SetBreakOn → delay → SetBreakOff" is two assertions that cannot be interleaved | `FtdiBackend::wait` is part of the trait with `spin_wait` as its default: a spin under a millisecond, a sleep plus a spin above it (DMX512 permits a break of up to a second). `MockFtdi` overrides it to record, so the sequence is one `assert_eq!` on one list. **S8 requirement:** measure whether the spin is what limits the rate, or whether the two USB control transfers are |
| 2026-08-11 | S7 | **The backoff has to start when the cable is lost, not when the first reconnection fails.** Written the obvious way, a disconnect is followed immediately by an attempt that cannot succeed — the cable came out microseconds ago — and every delay in the series is then one attempt late | Losing the link sets the retry time; the schedule is asserted exactly, at 100, 300, 700 and 1500 ms after the loss, on a simulated clock |
| 2026-08-11 | S7 | **A driver thread must never sleep for the length of its own backoff.** The ceiling is five seconds, and a thread asleep for five seconds is a thread that takes five seconds to notice the show is being shut down | The runner always wakes at its output's cadence and compares the clock against the retry time. Shutdown is bounded by one cadence — 25 ms for Open DMX — rather than by the backoff |
| 2026-08-11 | S7 | **An output re-sends its last frame when the engine has published nothing new.** The tempting reading of "the triple buffer had nothing fresh" is "there is nothing to do", and it is wrong: a DMX receiver that stops being refreshed times out, and the engine publishing nothing means "hold this look" | The runner refreshes and sends either way. **S9 requirement:** ArtNet's forced full-frame refresh every 800 ms (`ARCHITECTURE_SPEC.md` §7.2) is the same rule with a network-shaped answer — there, sending *every* cadence is what floods a school network |
| 2026-08-11 | S7 | **A short write is a truncated frame, not a partial success.** 512 of 513 bytes is a packet with its last channel missing, and a driver that called that "sent" would show a green light over a fixture sitting at the wrong value | Reported as `OutputError::Faulted`, which degrades the output and keeps the link — the next frame is attempted. Distinguished throughout from a lost cable, which closes down and reconnects: `FtdiError::is_link_lost` is the single place that decides which is which |
| 2026-08-11 | S7 | **A failure to *release* the break is not special-cased, and the reason is worth writing down.** It leaves the line low, which is a dark universe, so the instinct is to close the device — but closing an FTDI device does not guarantee the pin comes back up either, and the next frame begins by asserting and releasing the break again | The ordinary retry *is* the recovery. Documented on `OpenDmxUsb::fault` and asserted: after a failed release, no packet goes out, the output is degraded, and the following frame is normal |
| 2026-08-11 | S7 | A universe an output carries that the engine does not publish is the same problem as S5's unresolved cue part and S6's dropped programmer value | Kept and reported as `OutputRunner::unmapped()` rather than silently skipped. **S17/S27 requirement:** surface it — an output configured for a universe that is not in the patch is a dark universe with a green light next to it |
| 2026-08-11 | S7 | **The coverage pattern held for a fourth session.** The first measurement read 98.37 %, and none of the gap was a missing test of real behaviour: six of the uncovered functions were a test double's unused methods, one was a `Default` nothing called, and one was the shutdown-panic path | All three are now exercised, the last by a fault worth having anyway — a driver that panics *while shutting down* must not turn a clean stop into a crash. 99.72 % lines, 100 % functions. **One** line in the crate is deliberately unreachable and says so in place: a frame position that came out of the layout cannot name a missing universe, and inventing a frame that disagreed with its own layout would test nothing real |
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

**The daemon has a wire.** `prism-ipc` is complete: length-prefixed
MessagePack with a size limit and a nesting-depth limit, a named pipe on Windows
beside a Unix domain socket elsewhere, a WebSocket over `axum`, the backpressure
policy of §8, and the server and client halves — all of it above one
transport-independent `Wire`, so the same suite runs identically over three
transports. What is missing is a process to run any of it in: there is a motor,
three outputs, a complete state and now a wire, and nothing that holds them
together. Begin **S17** (`prismd` — the daemon binary). Use the prompt in §8.

Carried out of S16:
- **`ServerHandler` is the whole of what S17 has to supply**: `snapshot`,
  `command`, and the two default hooks `connected` and `disconnected`. The
  server holds no show and no engine on purpose — a server with an opinion about
  whether a command is valid would be a second source of truth. Implement it
  over `prism_core::ShowFile` plus the engine's command queue.
- **`CommandOutcome::Applied { deltas }` is broadcast to *every* client and the
  `Ack` goes to one**, in that order: the fact before the receipt. `ShowFile::apply`
  already answers with exactly those deltas, so the two fit together without a
  translation layer.
- **The `Snapshot` carries three documents — show, session and programmer** — and
  the show and the session are `JsonValue` documents, because that is what a
  `ShowPatch` is an RFC 6902 operation *against*. S17 builds them from
  `prism_core`'s projections; `outputs` and `health` are the daemon's own.
- **`Endpoint` is what goes in the lock file of §2.2.** `LocalListener::endpoint()`
  and `WebSocketListener::endpoint()` produce it, and
  `Endpoint::needs_a_token()` answers whether §2.1 requires one — a listener not
  on loopback does. **Binding does not remove a stale socket file**, deliberately:
  deciding a previous daemon is dead is a PID liveness question and belongs to
  S17's takeover, not to a transport that would defeat the single-instance
  guarantee by sweeping up whatever it found.
- **`local::scratch_address(label)` exists for tests** — a pipe name or socket
  path nothing else is using, with the process id in it. S17's tests need it too.
  The real daemon does *not* use it: its endpoint has to be findable.
- **A client that is not reading cannot be told why it was disconnected.**
  `ServerConfig::goodbye` bounds both the wait for room and the final flush, and
  the connection closes regardless. S18 measures the mechanism; S16 built it and
  `crates/prism-ipc/tests/backpressure.rs` shows it working through a socket.
- **`TelemetryFrame` is the channel, not its contents.** Header, version, a
  reserved byte and one 514-byte section per universe. S17 decides what is
  measured beside the levels and fills it; `encode_into` takes a buffer so a
  frame at 30 Hz costs no allocation after the first.
- **The engine tick is unaffected by any of this.** `prism-ipc` is async and
  `tokio`-based; the tick is still an OS thread that S17 must raise to high
  priority (§3.1 and §3), and nothing in this crate runs on it.

Carried out of S15:
- **`ShowStore` is the only thing in the crate that touches a disk, and it holds
  no clock and no thread.** `Effect::Save` is deliberately *not* absorbed by
  `ShowFile::apply` the way `Programmer`, `Undo` and `Redo` are: a file has a
  path, a disk and a failure mode. **S17 holds the store**, answers
  `Effect::Save` with `ShowStore::save`, and polls `Autosave` on its own timer.
- **`save` returns the deltas**, so the `DirtyFlag` transition still travels
  exactly once, and a `Notice` comes with it when the recovery copy could not be
  cleared away. `mark_saved` is called when the commit has **returned**.
- **A load replaces the show and the session and empties the programmer and the
  journal.** Both describe the show that was open a moment ago. It answers with
  `Repatch`, `ReloadGroups` and one `ReloadSequence` per sequence, which is
  S17's to-do list after opening a file.
- **The `.prism` schema is eight tables and a `user_version`.** A new version is
  an entry appended to `MIGRATIONS`; nothing in that list is ever edited, and
  `FORMAT_VERSION` is its length so the two cannot drift. A file from a newer
  build is refused rather than guessed at.
- **The documents inside the rows are MessagePack**, which is S1's finding on
  the platter: a JSON export is not bit-exact for floats, so `export_json` is
  the interchange format and the `.prism` file is the authoritative one.
- **The migration fixture is frozen and is copied before it is opened**, because
  opening a file is what migrates it. **A domain change that breaks decoding of
  an old row is a migration that re-encodes**, not one that adds a table.
- **The workspace contains C now.** `rusqlite` with the bundled amalgamation;
  the ARM64 cross-check installs `gcc-aarch64-linux-gnu` for it. A native Pi
  build needs nothing extra, a cross-build for one does — **S29's packaging
  owns saying that out loud**.

Carried out of S14:
- **`ShowFile::apply` is the door for the journal too.** It files a step for
  every undoable command and carries out `Oops` and `Redo`, dropping
  `Effect::Undo` and `Effect::Redo` exactly as it drops `Effect::Programmer`.
  **S17 must not write a second journal** — it dispatches here.
- **A record is a scope, not a snapshot**: one fixture's patch entry, or one
  sequence plus the programmer and its page state, or the programmer and its
  page state. That is what keeps an Oops off an executor master, which is show
  state and is deliberately not undoable.
- **The journal is not in the file.** `#[serde(skip)]`, and S15 gave it no
  table: `ShowStore::load` replaces `file.show` and `file.session` in place and
  calls `Journal::clear` — asserted, and checked by mutation.
- **New *state* is not caught by the compiler, only new *commands* are.** The
  four exhaustive matches make a new `Command` a compile error in four places,
  including `ShowFile::image`. A new field that an existing command can move —
  S27's patch sheet, S28's cue editor — has to be added to that command's scope
  by hand, or the Oops will silently not reach it. The property test in
  `tests/oops.rs` is what would catch it.
- **S28: a command that creates or deletes a sequence** means `Image::Sequence`
  grows an absence of its own, as `Image::Fixture` already has one.
- **S26: `UndoRecord::command()` names the Oops on the console** — "Oops: Store
  Cue" tells an operator what is about to happen; "Oops" alone asks them to
  guess. `UndoRecord::scope()` is the rest of that label.
- **A test fixture built out of default values cannot tell a restore from a
  no-op.** `tests/oops.rs` starts its session with a non-zero programmer page
  for that reason; the version that started at zero passed the whole property
  test with the page missing from the journal entirely.

Carried out of S13:
- **`ShowFile` holds three models and composes them.** A programmer command is
  validated by the show, finished by the programmer, and may move the session as
  well; `Effect::Programmer` is carried out here and never handed on.
- **The programmer is not in the file** (`#[serde(skip)]`), and the Save LED is
  the reason: setting a value is not an edit to the show. **S15 has no table for
  it.**
- **`Programmer::restore` is S14's door.** Every programmer command is undoable,
  and the inverse of one is the state that was there before — small and sparse
  enough to journal whole, Clear stage included.
- **S17 must diff the programmer**, because `Delta::ProgrammerChanged` carries
  the whole state and the engine is addressed per `MergePlan` slot.
- **S16: the `Snapshot` of `docs/IPC_PROTOCOL.md` §4.1 carries no programmer.**
  Either it grows a third document or the daemon sends a `ProgrammerChanged`
  straight after it.
- **S28: `StoreCue` has no mode**, so a store merges. Merge / Overwrite / Remove
  is a protocol addition.
- **A red timing test is a question about the machine before it is a question
  about the code.** `the_tick_holds_its_deadline_for_a_few_seconds` failed twice
  in S13 and both times the desk was busy elsewhere — a coverage build once, a
  game and a video the other time. The test prints `probe thread turns: N/s`
  precisely so this can be told apart: read that line, re-run the target alone,
  then look at the diff.

Carried out of S12:
- **`ShowFile` is the routing D11 describes, and S17 should use it rather than
  matching on command variants a third time.** `Command::is_session_command`
  decides; both appliers answer with the same `Applied`.
- **The Save LED is `ShowFile::is_dirty`** — the show's flag or the session's,
  and only `StoreView` sets the session's. `ShowFile::apply` already keeps
  `Delta::DirtyFlag` to the transitions of the pair.
- **Views live in the session**, keyed by number, and travel as `SessionPatch`
  on `/views/<number>`. See the decision log for the three reasons.
- **S25 needs a protocol command for window geometry.** `place_window` is the
  state operation and no §4.4 command carries a position — dragging a window is
  session state, so it cannot stay in the client.
- **`SessionMirror` and `ShowMirror` sit on one `JsonMirror`.** A client's
  document is mirrored by whichever knows the delta; each ignores the other's.
- **A session edit that changes nothing emits nothing**, and `SessionState::commit`
  is the single place that decides both that and the encode-before-write order.
- **S13 resets the jog wheel through `set_programmer_param_index`** when the
  selection changes: the parameter index is session state, not programmer state,
  and a wheel left pointing at a parameter the new selection does not have is a
  console that lies.

Carried out of S11:
- **`Show::apply` answers with `deltas` and `effects`, and the effects are S17's
  to-do list.** `Repatch` means *rebuild the `MergeBody` and blank the
  publisher's frame buffers*; `Programmer` means S13 finishes this one; `Undo`,
  `Redo` and `Save` mean S14 and S15 do.
- **`Show::patch_revision()` is the number a daemon watches.** The programmer is
  addressed by merge-plan slot, so every queued programmer command is stale when
  it moves.
- **The sACN CID lives in `MachineConfig`, not in the show.** S15 wrote the file
  and gave it no table for a desk, and does not regenerate anything on save;
  S17 generates the identity once on first start. See the decision log.
- **Validate and encode before you write.** The JSON projection is the last thing
  that can fail in an edit, and one operation had it in the wrong order — see the
  decision log.
- **`ShowMirror` is the applier a client uses**, and the reason delta generation
  can be tested at all. `Delta::ExecutorState` has to be applied by hand: it is
  show state that does not travel as JSON Patch.
- **Preset numbers are unique across pools**, because `ApplyPreset` carries no
  pool.
- **Overlaps and dangling references are reported, never refused**:
  `Show::conflicts()` and `Show::issues()` are what the patch sheet (S27) shows in
  red, and the winner they name is the byte the engine writes.
- `tests/common/mod.rs` in this crate is the pattern for shared integration
  scenery: `#![allow(dead_code)]` at its head, because each target compiles its
  own copy and uses a subset.

Carried out of Phase 2 into the daemon sessions:
- **`DmxOutput` is five methods and it stayed that way for three drivers.** Open DMX, Art-Net and sACN all implement the same five and all inherit `OutputRunner`'s thread, reconnect backoff and panic containment unchanged. Two things that looked like they needed a sixth method — Art-Net's end-of-frame for ArtSync, sACN's goodbye — did not: the first is found in the driver, the second belongs in `shutdown`. If a fourth output does not fit, that is a decision-log entry.
- **The CID belongs to the desk, and the question is settled.** It is a UUID identifying *this machine*, stable across restarts, and it must **not** be duplicated by copying a show to a second machine — so it is `prism_core::MachineConfig` (S11) and the `.prism` file has no table for it (S15). `SacnConfig::cid` still defaults to nil and an output holding a nil CID refuses to connect; **S17 generates one on first start** and writes it beside the daemon's settings.
- **S17 owns blackout-or-hold on shutdown, and it is a frame rather than a flag.** The sACN driver terminates its streams carrying the last look; a daemon that wants a dark stage publishes a blackout frame *before* stopping the outputs.
- **S17 must not raise the process's priority, only the tick thread's** — the stress gate measured what happens otherwise (§3.1).
- **S27's output editor shows things that are data on purpose:** Art-Net's port address as `net:sub:universe`, sACN's E1.31 universe and per-universe priority, the source name, the CID, and `OutputRunner::unmapped()` — a universe an output carries that the patch does not publish is a dark universe with a green light beside it.
- **`OutputStatus::frames_sent` is not a packet count.** Both network outputs suppress an unchanged universe, so `datagrams_sent()` is the second number a status panel needs.
- **Assert the bytes of a received datagram, not the calls.** S8's lesson; `tests/artnet_wire.rs` and `tests/sacn_wire.rs` are the shape. A mock asserts the calls a driver makes, never the time between them.
- **A keep-alive interval is a maximum, so it fires early** — `refresh_margin` in both network configurations, and the reasoning is in the decision log.
- **`ManualClock` cannot be shared** — it is a `Cell` with one owner. Both wire targets define a mutex-backed `SharedClock` for the case where a runner and its output must read the same simulated time.
- **No test sends multicast or broadcast**, and that is a rule rather than an omission: a suite that put lighting data on the network it runs on is the fault §7.2 exists to prevent. What a real switch and gateway answer is in §5.
- Everything about the SH-RS09B is one constant, `DeviceProfile::SH_RS09B`, carrying measurements with `verified: true`. A profile for a device nobody has held should say so.
- The bring-up target (`tests/hardware.rs`) and §3.2's commands are the pattern for S20's X-Touch verification: `#[ignore]`d, documented, serialised behind a mutex, and driven by environment variables so a person can steer it.
- `prism-protocols` runs its tests in the **Linux** CI job as well; only the two FTDI backends are behind `cfg(windows)`. `udp.rs`, `artnet.rs` and `sacn.rs` contain no `#[cfg]` at all, and it must stay that way.
- A driver thread wakes at its output's own cadence, never sleeps longer than one cadence, and re-sends the last frame when the engine has published nothing new. For a network output "re-sends" means the forced refresh — see the decision log.

Carried from Phase 1:
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
- `prism-protocols`: a driver thread wakes at its own output cadence rather than spinning near the engine — see the decision log. S7 implemented this; S9 and S10 inherit it.
- `prism-ipc` (S16) must serialise MessagePack with `to_vec_named`, must enforce a **nesting depth limit** on decode, and must treat serialisation as fallible — see the decision log.
- `prism-core` (S11) embeds the used fixture types in the show file — done, see §2.12.
- A JSON export is **not** bit-identical in its floats (S1), which is why the `.prism` file holds MessagePack and `export_json` is documented as the interchange format — done, see §2.16.

---

## 8. Follow-up prompt for the next session

> Rewritten at the close of every session, per `IMPLEMENTATION_PLAN.md`. Written to be **self-contained**: it assumes no loaded context, no memory of previous conversations and no knowledge of the project. Paste it into a fresh session to continue.

**Next up: S17 — `prismd`: das Daemon-Binary**

```text
PrismDMX — Session S17: prismd, das Daemon-Binary

Projektverzeichnis: C:\Users\Milan\Prismdmx

Diese Session baut zum ersten Mal einen Prozess. Bis hierher gibt es einen
Motor, drei Ausgänge, einen vollständigen Zustand, eine Datei und eine Leitung —
acht Crates, die alles können und die niemand startet. `prismd` ist das
Programm, das sie hält.

Bitte lies zuerst in dieser Reihenfolge, bevor du irgendetwas änderst:
1. CLAUDE.md                              — verbindliche Qualitäts-, Architektur-
                                            und Teststandards
2. PROGRESS.md                            — Stand, Decision Log, gemessene Zahlen;
                                            besonders §2.17 (was S16 geliefert
                                            hat), §7 „Carried out of S16" und
                                            alle Decision-Log-Einträge, die eine
                                            „S17 requirement" nennen — davon gibt
                                            es viele, aus jeder Session seit S11,
                                            und sie sind ein Teil des Auftrags
3. IMPLEMENTATION_PLAN.md                 — Session-Protokoll und die Definition
                                            von S17
4. ARCHITECTURE_SPEC.md §2 (Systemüberblick), §3 (Threading-Modell, besonders
   §3.1 und §3.2), §5 (Pipeline), §7 (Ausgänge), §10.3 (Lebenszyklus,
   Single-Instance, Shutdown), §12
5. docs/IPC_PROTOCOL.md §2.2 (Discovery und Lock-File), §4 (Handshake und
   Snapshot), §8 (Backpressure und Ausfall), §9 (Tests)
6. crates/prismd/src/main.rs              — bisher nur ein Rumpf
7. crates/prism-ipc/src/server.rs         — `ServerHandler` ist genau das, was
                                            diese Session zu liefern hat
8. crates/prism-core/src/file.rs und store.rs — `ShowFile::apply` ist die Tür,
                                            `ShowStore` die Platte, `Autosave`
                                            die Politik
9. crates/prism-engine/src/lib.rs         — `Engine`, `TickCommand`,
                                            `FramePublisher`, `MergeBody`

Stand nach S16 — nichts davon musst du neu bauen:
- `prism-domain` (S1): alle Domänentypen samt Serialisierung und
  TypeScript-Bindings; 133 Tests.
- `prism-engine` (S2–S6) ist vollständig: 44 Hz bei 64 Universen unter Volllast,
  allokationsfrei im Tick.
- `prism-protocols` (S7–S10) ist vollständig: Open DMX USB (am echten Gerät
  verifiziert, 35,5 Hz), ArtNet und sACN (beide 44 Hz), jeweils hinter
  `DmxOutput` mit `OutputRunner` für Thread, Reconnect-Backoff und
  Panic-Eindämmung.
- `prism-core` (S11–S15) ist vollständig: `Show`, `SessionState`, `Programmer`,
  das Oops-Journal und die `.prism`-Datei. 225 Tests.
- `prism-ipc` (S16) ist vollständig: längenpräfigiertes MessagePack mit Größen-
  und Verschachtelungsgrenze, Named Pipe bzw. Unix-Domain-Socket, WebSocket über
  `axum`, Backpressure nach §8, Server- und Client-Hälfte — alles über einem
  transportunabhängigen `Wire`. 131 Tests, 98,43 % Zeilenabdeckung.
- Insgesamt 1036 Tests im Workspace, alle grün, CI vierfarbig grün.

Aufgabe: Session S17 umsetzen — `prismd`: das Daemon-Binary.

Umzusetzen (IMPLEMENTATION_PLAN.md S17):
- Verdrahtung von Engine, Core, Ausgängen und IPC
- Lock-File mit PID und Endpunkt; Single-Instance-Garantie; Übernahme eines
  veralteten Lock-Files
- CLI für den kopflosen Betrieb
- Shutdown mit sACN-Terminierung und konfigurierbarem Blackout-oder-Halten

Exit-Kriterien — die Session gilt erst als fertig, wenn diese wirklich zutreffen:
- Der Daemon startet, lädt eine Show, gibt DMX auf einen Mock-Treiber aus,
  **ohne dass sich jemals ein Client verbindet**
- Eine zweite Instanz erkennt die erste und weigert sich zu starten — als Test
  behauptet, nicht als Absicht beschrieben
- Ein veraltetes Lock-File (getöteter Prozess) wird erkannt und übernommen
- Der Handshake liefert einen `Snapshot` mit Show **und** Session
- `cargo test -p prismd` ist grün
- `cargo clippy --workspace --all-targets -- -D warnings` ist sauber
- `cargo fmt --all --check` ist sauber
- Abdeckung auf `prismd` gemessen mit `cargo llvm-cov -p prismd --summary-only`
  und in PROGRESS.md eingetragen

Wichtige Randbedingungen — alle stehen ausführlich im Decision Log:
- **Der Tick-*Thread* braucht hohe Priorität, nicht der Prozess** (S2/S6-Fund,
  gemessen): bei Standardpriorität verfehlte derselbe Zehn-Minuten-Lauf 45 Ticks
  mit p99.9 = 54 ms, mit hoher Priorität keinen einzigen mit p99.9 = 200 µs.
  `prism-engine` darf seine Priorität nicht selbst setzen — es ist per Regel
  plattformneutral —, **also gehört das hierher**. Und nur der Thread: eine
  Prioritätsklasse gilt für alle Threads des Prozesses, und PROGRESS.md §3.1
  beschreibt, was dann passiert.
- **`Effect::Save` ist der eine Effekt, den `ShowFile::apply` nicht ausführt**
  (S15). Der Daemon hält den `ShowStore`, beantwortet ihn mit `ShowStore::save`
  — was die Deltas zurückgibt, damit der `DirtyFlag`-Übergang genau einmal
  reist — und pollt `Autosave` auf demselben Timer wie alles andere.
- **Kein zweites Journal** (S14): der Daemon dispatcht durch `ShowFile::apply`,
  dort werden `Oops` und `Redo` ausgeführt.
- **Der Programmer muss gegen den zuletzt an die Engine geschickten Stand
  gediffed werden** (S13): `Delta::ProgrammerChanged` trägt den ganzen Zustand,
  die Engine wird pro `MergePlan`-Slot adressiert. `MergeBody::load_programmer`
  allokiert und ist deshalb Einrichtungsarbeit, nichts pro Encoder-Drehung.
- **`Show::patch_revision()` ist die Zahl, die der Daemon beobachtet** (S11):
  jedes gequeuete Programmer-Kommando ist veraltet, sobald sie sich bewegt. Und
  ein Austausch der `MergeBody` zur Laufzeit muss die Framepuffer des Publishers
  leeren, sonst bleiben ungepatchte Kanäle stehen.
- **Ein Laden ist kein Replay** (S15): `ShowStore::load` antwortet mit
  `Repatch`, `ReloadGroups` und einem `ReloadSequence` pro Sequenz — das ist die
  To-do-Liste nach dem Öffnen einer Datei.
- **Die sACN-CID gehört der Maschine, nicht der Show** (S10/S11):
  `prism_core::MachineConfig`, einmal beim ersten Start erzeugt und neben den
  Einstellungen des Daemons abgelegt. Ein Ausgang mit einer Null-CID weigert
  sich zu verbinden.
- **Blackout-oder-Halten beim Herunterfahren ist ein Frame, kein Flag** (S10):
  der sACN-Treiber beendet seine Streams mit dem letzten Look; ein Daemon, der
  eine dunkle Bühne will, veröffentlicht **vorher** ein Blackout-Frame und
  stoppt dann die Ausgänge.
- **`ServerHandler` ist die ganze Schnittstelle zu `prism-ipc`** (S16), und die
  Reihenfolge steht fest: Deltas an **alle** Clients, danach das `Ack` an einen.
  Der `Snapshot` trägt drei Dokumente — Show, Session und Programmer —, und Show
  und Session sind `JsonValue`-Dokumente, weil ein `ShowPatch` eine
  RFC-6902-Operation *gegen* sie ist.
- **Das Lock-File ist die Single-Instance-Garantie, und das Binden allein ist
  sie nicht** (S16): `LocalListener::bind` räumt eine übrig gebliebene
  Socket-Datei absichtlich **nicht** weg, weil „der vorige Daemon ist tot" eine
  PID-Lebendigkeitsfrage ist. Diese Session besitzt die Übernahme. Unter Windows
  verweigert `first_pipe_instance` zusätzlich die zweite Instanz auf
  Betriebssystemebene.
- **`local::scratch_address(label)` gibt es für Tests** — ein Pipe-Name bzw.
  Socket-Pfad, den nichts anderes benutzt. Der echte Daemon benutzt ihn nicht:
  sein Endpunkt muss auffindbar sein und steht im Lock-File (§2.2).
- **`prismd` darf `#[cfg(target_os = ...)]` nicht enthalten**, außer wo
  ARCHITECTURE_SPEC.md §10.1 es erlaubt — und es erlaubt es dort nicht. Die
  Thread-Priorität ist der Punkt, an dem das weh tut: sie ist plattformabhängig
  und muss trotzdem irgendwo hin. Wie das gelöst wird, ist ein
  Decision-Log-Eintrag. Die CI führt `cargo check --workspace --target
  aarch64-unknown-linux-gnu` aus, also muss es dort durchkommen; der ARM64-Job
  installiert für SQLite bereits `gcc-aarch64-linux-gnu`.
- **Ein Test-Fixture aus lauter Default-Werten kann „korrekt übertragen" nicht
  von „nie angefasst" unterscheiden** (S14-Fund, seit S15 Regel).
- **Validieren und kodieren, bevor geschrieben wird**, und **eine Ablehnung
  lässt den Zustand byte-identisch**: S11 bis S16 haben das jeweils wörtlich als
  Test.
- Test-Driven, wie CLAUDE.md es verlangt: erst der fehlschlagende Test, dann der
  Code. Wo ein Test schnell grün wird, lohnt eine Gegenprobe: S11 bis S16 haben
  ihre zentralen Tests jeweils durch absichtlich eingebaute Regressionen
  geprüft — S15 und S16 haben auf diesem Weg echte Fehler gefunden, und S16s
  Gegenprobe hat den Testprozess mit einem Stack-Overflow abgeschossen.
- **Ein roter Timing-Test ist zuerst eine Frage an die Maschine, nicht an den
  Code.** `the_tick_holds_its_deadline_for_a_few_seconds` fiel in S13 zweimal
  aus, beide Male weil der Rechner nebenher beschäftigt war. Der Test druckt
  dafür selbst `probe thread turns: N/s`: erst diese Zeile lesen, dann das
  Target allein laufen lassen (`cargo test -p prism-engine --test realtime`),
  dann den Diff verdächtigen. Auf einer ruhigen Maschine sind alle 1036 Tests
  grün.
- Toolchain ist eingerichtet (Rust 1.97.1 msvc, MSVC Build Tools 2022,
  Node 24.11). Es ist kein weiteres Setup nötig.

Zum Abschluss der Session:
- PROGRESS.md aktualisieren: S17-Status, gemessene Coverage, Decision Log bei
  Abweichungen vom Plan oder Funden, die spätere Sessions betreffen
- PROGRESS.md §8 mit einem neuen, ebenfalls kontextfreien Follow-up-Prompt für
  Session S18 (D2-Gate — Resilienz) überschreiben
- Mit Conventional-Commit-Message committen, z. B. feat(prismd): …
- Danach pushen, den CI-Lauf beobachten und das Ergebnis in PROGRESS.md
  eintragen (IMPLEMENTATION_PLAN.md, Session-Protokoll Punkt 6)
```
