# PROGRESS.md — PrismDMX Status Tracker

**Last updated:** 2026-08-29
**Current phase:** Phase 7 — Outputs, devices and the machine
**Current session:** S48 — tracking, and a cue list that lands in the same place twice (not started — see §8 for the prompt that starts it)
**Last completed:** S46 — `prism-protocols` + `prismd` + `ui`, Art-Net node discovery ✅ (CI green on run **33272829395**) — **an Art-Net output knows whether anything is listening.** Punch-list B6: `Health::Ok` meant *the socket took the datagram* and UDP always takes it, so a configured node read *OK* over an empty rack. `prism-protocols` has a **receive path** for the first time — `ArtPoll` out, `ArtPollReply` parsed field by field out of §6 of the specification, on a seam of its own so no driver thread can be handed a call that blocks. A node is *answering*, *never answered* or *stopped* with the age of its last reply beside it, and the daemon folds that into the health every client is told, so an output whose nodes are silent reads **Degraded** in the snapshot, in the delta and in the answer. Nothing broadcasts: the poll goes only where this desk already sends, and what that costs is written down rather than hidden. A reply is an input from outside — the parser allocates **nothing**, the table is bounded, and a quarter of a million random bytes produce no panic and no allocation. Before that: S45 — `prism-core` + `prism-engine` + `ui`, the executor window and one sequence one playback ✅ (CI green on run **33261331368**, all five jobs on the first attempt) — **an executor is a handle on a cue list's playback, and which handle it is, is editable.** Punch-list B18: a playback belongs to the *list*, so `Go` on either of two executors carrying it advances one cue pointer and two `Master` faders on it are two handles on one number — asserted on the frames a mock output received, which is the only place the difference is real. Punch-list B15: `Command::ConfigureExecutor` says what a fader, an encoder or one of four keys does, with a **custom row** that sends a line the operator wrote; the editor writes that line rather than sending a command, so the window, the console and a bound X-Touch key are one path. The decision the session existed to make is recorded with its reason — a `Command`, not a `MachineChange`, because an executor is a numbered show object rather than a control of this building. Before that: S33 — the output patch ✅ (CI green on run **32539636281**) — **a venue's rig is data now, and the data belongs to the building.** Twelve universes across five outputs of three kinds, built entirely from commands and asserted on the frames each driver was handed: every one got exactly the universes its row named and no others. An output added, removed or re-addressed while the show runs costs the outputs that did not change **no frame and no tick** — `prism_engine::FrameEnrolment` hands a subscriber over behind one atomic flag, and the tick that takes on four and gives up four makes **zero allocator calls**. Where the rig lives is the decision: `prism_core::MachineConfig`, beside the desk identity, so a show carried to another hall on a stick arrives with no cabling in it — asserted on the show file's own bytes

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
| 13 | CI verified green | ✅ | Latest: run **33272829395** on `f453c1b` (S46) — **all five jobs on the first attempt**: Windows full build and test 13 m 52 s, Linux platform-neutral 4 m 46 s, UI end-to-end against a daemon 3 m 34 s, UI typecheck/lint/test/build 2 m 14 s, ARM64 cross-check 41 s. **Forty-four end-to-end tests green on the runner**, one of them this session's: the node list never appears without saying whether this desk is listening. The Linux job is the one worth reading for this session — `prism-protocols`' whole suite runs there, so the ArtPoll that leaves a real socket and the ArtPollReply that comes back are asserted on a machine that is not this one, and the fuzz harness's zero allocator calls are measured there too. Before that: run **33261331368** on `0566bc1` (S45) — **all five jobs on the first attempt**: Windows full build and test 12 m 53 s, UI end-to-end against a daemon 3 m 37 s, Linux platform-neutral 4 m 48 s, UI typecheck/lint/test/build 2 m 04 s, ARM64 cross-check 35 s. **Forty-three end-to-end tests green on the runner**, two of them this session's: punch-list B15 driven in a browser — an executor selected, its fader given a master, a key given a Go and a fourth key given a line, all of it surviving a reload — and B18, where a second `Master` fader put on cue list 1 from the **command line** reads the level the first one holds and pulling either moves both. Before that: run **33225077470** on `959909c` — **all five jobs on the first attempt**: Windows full build and test 13 m 35 s, Linux platform-neutral 4 m 54 s, UI end-to-end against a daemon 3 m 21 s, UI typecheck/lint/test/build 2 m 17 s, ARM64 cross-check 47 s. Before that: run **33223231681** on `94ae402` — all five on the first attempt: Windows 12 m 35 s, Linux neutral 4 m 38 s, UI end-to-end 3 m 22 s, UI typecheck/lint/test/build 2 m 19 s, ARM64 53 s. **Forty-one end-to-end tests green on the runner**, including the device-screen test at 4K that had been passing without opening anything. **S43 took six runs and four of them are worth reading — §2.41.** Run **33209652373** on `fcb884d` (the feature commit) was green on the first attempt, 13 m 02 s. Run **33220786773** on `9c9ed4b`, a commit that changed **no Rust at all**, went red on `surface_gate.rs`'s real-time assertion and was green on a re-run with the identical tree — the same test, line and symptom S37 recorded, so this session built the control S37 had specified and left open. Run **33222187315** on `927955c` then went red on a **different** thing: `desk.spec.ts`'s device-screen test at 4K read the window chooser one React render too early and saw no window types at all — a race the same test had been hiding for a second reason, an `if (count > 0)` around the click that turned a chooser which had not rendered into a silent skip. Both are fixed in `openPicker`, and the 4K case went from 1.1 s to 7.7 s because it is now opening the fourteen windows it claimed to. Run **33224149782** on `648aba6`, two lines of this file, then went red on a **third** shape of the same sentence: `closeWindows` clicked a snapshot count of close keys without waiting for any of them to land, and one window survived. Closing a window is a command out and a delta back — **D3 applies to the suite** — so the loop waits for the count to drop. **The lesson is written into §7:** `evaluateAll` does not wait, and a guard that turns a missing element into a skip turns a red test green rather than a broken interface visible. Before all of that: run **32326458934** on `124bb2f` (S34's follow-up fix) — all five jobs on the first attempt: Windows 9 m 30 s, Linux neutral 2 m 26 s, UI end-to-end 2 m 29 s, UI typecheck/lint/test/build 1 m 48 s, ARM64 check 52 s. Before that: run **32325628021** on `124bb2f` (S34's follow-up fix) — all five jobs on the first attempt: Windows 9 m 30 s, Linux neutral 2 m 26 s, UI end-to-end 2 m 29 s, UI typecheck/lint/test/build 1 m 48 s, ARM64 check 52 s. Before that: run **32325628021** on `1a26d78` (S34) — **all five jobs on the first attempt**: Windows 9 m 47 s, Linux neutral 3 m 11 s, UI end-to-end 2 m 14 s, UI typecheck/lint/test/build 2 m 05 s, ARM64 check 31 s. **Twenty-three end-to-end tests, all green**, two of them this session's: a `Flash` held in a browser against a real daemon — the light on the canvas, the stored master untouched at `0%` through the whole gesture and after a reload — and a `Toggle` latching, with the cue number arriving from the tick and the second half of it driven from a `--mock-surface` console. Before that: run **31847326456** on `cce51e3` (S44) — **all five jobs**: Windows 11 m 11 s, Linux neutral 3 m 04 s, UI end-to-end 2 m 50 s, UI typecheck/lint/test/build 1 m 20 s, ARM64 check 54 s. The library is **installed on the runner** rather than committed, so the 634-fixture corpus tests run there and the end-to-end suite searches a real fixture by name and patches it. Fifteen end-to-end tests, all green. Before that: run **31828565272** on `f3356f5` (S27) — **all five jobs on the first attempt**: Windows 7 m 31 s, Linux neutral 3 m 02 s, UI end-to-end 2 m 56 s, UI typecheck/lint/test/build 1 m 42 s, ARM64 check 41 s. **Fourteen end-to-end tests, all green**, three of them this session's: a rig built from an empty show in a browser, an address conflict named by the daemon before the command was sent, and a fixture sheet whose two live columns were counted off the pixels. Before that: run **31807825063** on `2c09ba6` (S26) — **all five jobs on the first attempt**: Windows 8 m 28 s, Linux neutral 2 m 22 s, ARM64 check 41 s, UI typecheck/lint/test/build 1 m 35 s, UI end-to-end against a daemon 1 m 48 s. **Eleven end-to-end tests, all green**, five of them this session's: paging agreeing between a `--mock-surface` console and a real Chromium, the jog wheel turning the parameter the encoder bar highlights, a console line reaching the telemetry picture (**`programmer 88 ms · output 122 ms`** on the runner), a syntax error that is a message rather than a throw, and eight zeros of overflow with both new bars on the screen. The telemetry budget was re-measured through them: **`64 universes · 30.4 Hz · paint 0.20 ms (p99 0.80 ms) · 156 frames · 1 lost`**. Before that: run **31798437277** on `e698b57` (S25) — **all five jobs on the first attempt**: Windows 9 m 37 s, Linux neutral 2 m 26 s, ARM64 check 51 s, UI typecheck/lint/test/build 1 m 13 s, UI end-to-end against a daemon 1 m 45 s. The end-to-end job is again the interesting one: **six tests, all green**, and three of them are S25's — a canvas that survives a page reload against an untouched daemon, a screen with no scrollbar outside the canvas, and **D11 watched happening**: a real `prismd` with `--mock-surface`, a real Chromium doing nothing, and three MIDI bytes written to a file by neither of them. That is decision D11 verified on a build server rather than on one developer's machine. The telemetry budget was re-measured through the new window system in the same run: `64 universes · 30.4 Hz · paint 0.20 ms (p99 1.10 ms) · 155 frames`. Before that: run **31752194635** on `c5dd711` (S24) — **all five jobs on the first attempt**: Windows 6 m 52 s, Linux neutral 2 m 15 s, ARM64 check 40 s, UI typecheck/lint/test/build 1 m 13 s, UI end-to-end against a daemon 1 m 52 s. The end-to-end job is the one worth reading this time: it started a real `prismd` on the committed 64-universe rig and measured the interface drawing it — **`64 universes · 30.3 Hz · paint 0.20 ms (p99 0.40 ms) · 164 frames`**, no frame lost and none dropped, on a Linux runner with a software rasteriser and a debug daemon. S24's frame budget is therefore a figure two machines agree on rather than one this machine reported. Before that: Latest: run **31737443279** on `c0ce1e0` (S23) — **all five jobs on the first attempt**, and there are five because this session added one: Windows 6 m 35 s, Linux neutral 2 m 31 s, ARM64 check 54 s, UI typecheck/lint/test/build 1 m 16 s, **UI end-to-end against a daemon 2 m 20 s**. The new job is the one worth watching: it compiles `prismd`, installs Chromium, serves the production build and then **kills and restarts the daemon** under the browser, which is S23's second exit criterion executed rather than argued — on Linux, over a WebSocket. The UI job also runs `npm run lint` and `npm run test` for the first time, and typechecks with `tsc -b --force` rather than `tsc --noEmit`, which on a solution file with no files of its own checked nothing at all. Before that: run **31728064334** on `cbde96e` (S22) — all four jobs on the first attempt: Windows 8 m 19 s, Linux neutral 2 m 28 s, ARM64 47 s, UI 51 s. Before that: run **31700591538** on `e2fe45b` (S20) — all four jobs on the **first attempt**: Windows 6 m 19 s, Linux neutral 2 m 08 s, ARM64 check 48 s, UI 50 s. The Linux job is the one that matters for this session: `prism-surface`'s whole suite runs there, **including the new `hardware_capture` target**, because a recording of a device is a platform-neutral fixture — a claim about a specific Behringer X-Touch is now checked on a Linux build server with nothing plugged in. The ARM64 check is unchanged in substance and that is the point: the MIDI port lives in `tools/xtouch-probe`, outside the workspace, so no job compiles `midir`. Before that: run **31655311363** on `8aefe2a` (S19) — all four jobs on the first attempt: Windows 9 m 51 s, Linux neutral 2 m 21 s, ARM64 check 49 s, UI 47 s. `prism-surface` is platform-neutral, so its whole suite runs in the Linux job as well. Before that: run **31638518112** on `23cd22e` (S18) — all four jobs. **S18 took three runs:** **31635842272** on `5bff4e7` was green, and then **31636431800**, on a commit that changed nothing but this file, failed on two different flaky *tests* on two different platforms — both fixed here, both in the decision log, neither a defect in the daemon. In a green run the Linux job is the interesting one: **`tests/resilience.rs` runs the whole D2 gate there over a Unix domain socket — 6 passed in 4.01 s**. Before that: run **31630334754** on `4a8ef6c` (S17) — all four jobs on the first attempt: Windows 9 m 11 s (it now compiles the `windows` crate as well, which `thread-priority` brings in), Linux neutral 2 m 15 s (**`prismd`'s tests run there from this session, over a Unix domain socket**), ARM64 check 44 s, UI 53 s. Before that: run **31607859145** on `2288ceb` (S16) — all four jobs: Windows 7 m 25 s (it now compiles `tokio`, `axum` and `hyper` as well), Linux neutral 2 m 17 s, ARM64 check 41 s (unchanged — none of S16's dependencies compiles C), UI 54 s. Three attempts: **31604980498** hung on a test rather than failing, **31607534093** failed on a clippy warning — both in the decision log. Before that: run **31583296669** on `24bbbc9` (S15) — all four jobs on the first attempt: Windows 4 m 32 s, Linux neutral 1 m 34 s, ARM64 check 1 m 15 s (now installing `gcc-aarch64-linux-gnu` for the bundled SQLite), UI 52 s. Before that: run **31550454514** on `5ad7d70` (S14) — all four jobs on the first attempt: Windows 3 m 35 s, Linux neutral 1 m 29 s, UI 45 s, ARM64 check 22 s. Before that: run **31546552629** on `d9a6564` (S13) — all four jobs on the first attempt: Windows 3 m 33 s, Linux neutral 1 m 43 s, UI 42 s, ARM64 check 22 s. Before that: run **31531687277** on `3db01dd` (S12) — all four jobs on the first attempt: Windows 3 m 56 s, Linux neutral 1 m 11 s, UI 44 s, ARM64 check 21 s. Before that: run **31522070040** on `3cc803a` (S11) — all four jobs on the first attempt: Windows 3 m 24 s, Linux neutral 1 m 10 s, UI 52 s, ARM64 check 25 s. Before that: run **31506271867** on `532cd1b` (S10, timing gate) — all four jobs on the first attempt: Windows 4 m 45 s, Linux neutral 1 m 12 s, UI 40 s, ARM64 check 23 s. S10's feature commit: run **31500810174** on `3711902`, green on a rerun of the Windows job, which failed on `prism-engine`'s short timing gate rather than on anything in the commit — see the decision log. First verified: run **31346581991** |
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
| S17 | `prismd` daemon binary | ✅ | 2026-08-12 | All exit criteria verified — see §2.18. 95 tests, coverage 93.84 % lines. Five mutation checks; a sixth changed nothing, which is itself in the decision log. **Phase 4's daemon exists: there is a process** |
| S18 | D2 gate — resilience | ✅ | 2026-08-12 | All exit criteria verified — see §2.19. **The mandatory gate for D2 passes.** 6 new tests, coverage 94.73 % lines on `prismd`. Four mutation checks; the first of them is the reason "no gap" is two claims rather than one |

### Phase 5 — Surface
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S19 | MCU codec | ✅ | 2026-08-13 | All exit criteria verified — see §2.20. 112 tests, coverage **99.24 % lines**. Built against the **unverified** tables of §5, and held as data so S20 is an edit rather than a refactor. Seven mutation checks; two of them are only visible to the allocator |
| S20 | 🔌 Hardware verification X-Touch | ✅ | 2026-08-13 | All exit criteria verified — see §2.21. **Every number in `docs/MCU_MAPPING.md` §2 confirmed at the device; none was wrong.** Six things no source had stated were corrected, and one real fault found: the surface can stop transmitting while still receiving. 13 new tests, 1 265 in the workspace |
| S21 | Surface model and feedback | ✅ | 2026-08-13 | All exit criteria verified — see §2.22. **The four rules of `docs/MCU_MAPPING.md` §5 hold, and three of S20's findings are designed around rather than noted.** 211 tests in the crate (52 new), coverage **99.20 % lines**, zero allocations on the outbound path as well as the inbound one. Eight mutation checks; one of them turns exactly one test red, which is why that test exists |
| S22 | Bindings + D11 gate | ✅ | 2026-08-13 | All exit criteria verified — see §2.23. **The mandatory gate for D11 passes:** a console changes a view and opens a window with no client connected, and the client that arrives afterwards finds both in its `Snapshot`. Every row of §4.1 transcribed by hand; the shipped profile *is* the built-in defaults; 1 419 tests in the workspace, coverage **99.30 % lines** on `prism-surface`. Six mutation checks; one of them turns the exit criterion red end to end. **Phase 5 is complete** |

### Phase 6 — User interface
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S23 | UI foundation | ✅ | 2026-08-13 | All exit criteria verified — see §2.24. **A delta stream recorded off a running daemon, replayed through the TypeScript mirror, reaches the daemon's own snapshot** — twelve cases, 93 deltas; and a real `prismd` killed under Chromium leaves no value on the screen. 158 UI tests, coverage **98.60 % lines**, zero `any`, no state-management dependency |
| S24 | Telemetry channel | ✅ | 2026-08-14 | All exit criteria verified — see §2.25. **Zero React commits over 300 frames of 64 universes, counted with a `<Profiler>`; 0.30 ms median and 1.10 ms p99 for decode *and* paint, measured in Chromium against a real `prismd` publishing 64 real universes.** The decoder is held to `TelemetryFrame::decode`'s own answers on recorded frames; 77 new UI tests, coverage **98.91 % lines** on `ui/src`. Four mutation checks; one of them is what says the render counter counts |
| S25 | Canvas, windows, views | ✅ | 2026-08-14 | All exit criteria verified — see §2.26. **D11 observed rather than argued: a real `prismd`, a real Chromium, and three MIDI bytes appended to a file by neither of them — the view switches and the window opens in the browser.** The layout survives `page.reload()` against a daemon that is never told. Two protocol findings, both taken: `Command::PlaceWindow` and `WindowType::DmxSheet`. 298 UI tests, coverage **98.82 % lines**; 1 449 in the workspace |
| S26 | Executor bar, encoder bar, console | ✅ | 2026-08-14 | All exit criteria verified — see §2.27. **Paging observed from both ends, the jog wheel turning what the bar highlights, and a console line reaching the picture in 113 ms.** Two protocol findings recorded rather than invented (`ExecutorButton`, the cue-index readback); one table added to `prism-domain` so the encoder bar and the wheel cannot disagree. 371 UI tests, coverage **99.07 % lines**; 1 441 in the workspace |
| S27 | Patch and fixture sheet | ✅ | 2026-08-14 | All exit criteria verified — see §2.28. **A rig is built from an empty show entirely in a browser — profile, fixture, address, name, number, unpatch — and an address conflict is named by the daemon before the command is sent.** The protocol grew a third shape for it: `Query`/`Answer`, which changes nothing and is answered to the one client that asked; plus `UnpatchFixture`, `RenumberFixture` and `EmbedFixtureType`, and the desk's own profile library in the snapshot. The Fixture Sheet shows the programmer in React and the cable on a canvas, and S24's zero-commit count still reads zero. 440 UI tests, coverage **99.07 % lines**; 1 472 in the workspace |
| S44 | The Open Fixture Library | ✅ | 2026-08-15 | All exit criteria verified — see §2.29. **A real moving head is searched by name, embedded and patched in a browser, and its channels are the manufacturer's.** The library is *downloaded at install time* rather than committed — 634 fixtures, 2 157 profiles, 0 files rejected — and it therefore left the snapshot for `Query::SearchLibrary`, which is the query channel S27 added earning its keep. A `/` in a profile key turned out to have been silently unreadable by every view since S26. 441 UI tests, coverage **99.07 % lines**; 1 504 in the workspace |
| S35 | Desk layout, encoder pages, view management | ✅ | 2026-08-19 | All exit criteria verified, CI green on run **32252325489** — see §2.30. **Both bars in one band, `programmerPage` paging four encoders at a time with the console's `Zoom ▲▼` and the browser on one number, and views that can be renamed, deleted and moved.** Three new session commands through `prism-domain`, `prism-core` and `prism-ipc`; the ordering decision — the number **is** the order — made, argued and asserted from three directions. The review that opened the session found a shipped `console.log`, a notices panel keeping a second list, and `.strip` defined twice so the executor strips had never used their own grid. 470 UI tests, coverage **99.11 % lines**; 1 517 in the workspace |
| S28 | Sequences, cues, presets | ✅ | 2026-08-19 | All exit criteria verified — see §2.31. **A show is written from a rig that had none: a cue list made, put on an executor, stored into, corrected in place, fired, and a preset whose edit changes the light a cue puts out** — counted off the telemetry canvas in Chromium. Five commands and a sixth question through `prism-domain`, `prism-core`, `prism-ipc` and `prismd`; the store-mode question is *asked on the button* rather than answered, because the mode is a value on the daemon’s reply and no word in the client. `Show::relink` makes `presetRef` mean what `prism-domain` has claimed since S1. 533 UI tests, coverage **99.05 % lines**; 1 562 in the workspace. CI green on run **32292211176** |
| S34 | Executor functions and the tick readback | ✅ | 2026-08-20 | All exit criteria verified, CI green on run **32325628021** — see §2.32. **Every one of the eight `ExecutorButtonFunction` values does what its name says, checked on the frames a mock output received; and the tick answers back, so the desk shows a cue number for the first time.** `Command::ExecutorButton` carries *which button* and the executor decides; `Flash` is a layer over the master and never a write into it; `Toggle` is resolved against `isActive` by the daemon and by nobody else. Speed masters and the tap that learns one exist at last — `docs/DMX_MERGE.md` §4 item 3 had named them since S3. `docs/MCU_MAPPING.md` §4.1's three unbindable rows are bound as written and the shipped profile's `deviationsFromSection41` block is **empty** |
| S39 | Store modes, cue editing, the update state | ✅ | 2026-08-20 | All exit criteria verified, CI green on run **32412552879** — see §2.33. **A store on top of something that is already there now does what the operator asked for**: Merge, Override and Remove on `StoreCue` and `StorePreset`, Append / Override / Merge on the new `StoreSequence`, each asserted on the **stored cue** rather than on the command being accepted. `EditCue` loads a cue back into the programmer with every `presetRef` kept and `Update` puts it down byte-identically; `Session::editingCue` is what makes an Update key blink and it clears on a Clear, a delete and another load. The selected sequence was **decided** and it went the other way from S28's assumption: `Session::selectedSequence` is a field of its own, because `Store Cue 5` with no executor selected has to mean something. The two tests written to go red did, and both are turned round. 542 UI tests, coverage **99.05 % lines**; 1 645 in the workspace |
| S40 | The console shell | ✅ | 2026-08-21 | All exit criteria verified, CI green on run **32510091387** — see §2.34. **The command line is the interface** (`ARCHITECTURE_SPEC.md` §4.5): every key on the desk writes a word into `Session::commandLine` and none of them acts on its own, so what a gesture means is what the line means. The whole S40 vocabulary is built and each line is held to the commands a real daemon accepted for it. It was a protocol session as well: `ObjectRef` and four generic verbs (`Delete`, `Copy`, `Move`, `Label`) replaced seven narrower commands, `Goto` and `ExecutorOn` arrived, and `PlaybackTarget` made a playback addressable by **sequence** — so a cue list nobody has put on a fader can be played. 43 command variants, 28 e2e tests green |
| S43 | The punch list, the skeleton, and the first pre-release | ▶ | | **In progress — see §2.41.** Built and green on CI run **33209652373** (`fcb884d`): the interface rebuilt along `design/skeleton/`, three rounds of the owner's hand-testing answered as `docs/PRERELEASE_PUNCHLIST.md` B1–B34, 1 967 Rust tests, 775 in jsdom, 41 end-to-end. **Open**: the owner has not yet called it fit for a pre-release, and the §7 *carried out of* lists have not been gone through. Added 2026-08-14; **grew on 2026-08-24** from *interface cleanup and polish*. The §7 *carried out of* lists gone through one entry at a time, **plus** the owner's own fault list (`docs/PRERELEASE_PUNCHLIST.md`), the interface rebuilt along the owner's drawing (`design/skeleton/`, a Penpot export of flow and arrangement), and the windows that are a morning's work — `ClockViewer`. The **first interactive session**: agreed before it is built, driven by hand before it is called done, and anything too big for an afternoon becomes a session of its own rather than a dropped line |
| S45 | The executor window, and one sequence one playback | ✅ | 2026-08-29 | All exit criteria verified — see §2.42. **An executor is a handle on a cue list's playback, and which handle it is, is editable.** Punch-list **B18**: a playback is the *list's* now, so `Go` on either of two executors carrying it advances one cue pointer and two `Master` faders on it are two handles on one number — both asserted on the frames a mock output received, which is the only place the difference is real. Punch-list **B15**: `Command::ConfigureExecutor` says what a fader, an encoder or one of four keys does, with a **custom row** that sends a line the operator wrote — and the editor *writes that line* rather than sending a command, so the window, the console and a bound X-Touch key are one path. The decision the session existed to make is recorded with its reason: a **`Command`**, not a `MachineChange`, because an executor is a numbered show object rather than a control of this building (`docs/IPC_PROTOCOL.md` §5). 1 980 Rust tests, 797 in jsdom, 43 end-to-end |
| S29 | `prism-app` Tauri shell | ☐ | | Needs MSVC Build Tools |

*Listed in running order; the numbers are identity rather than sequence — see `IMPLEMENTATION_PLAN.md`, Conventions.*

### Phase 7 — Outputs, devices and the machine
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S33 | The output patch: many outputs, many kinds | ✅ | 2026-08-22 | CI green on run **32539636281**. `OutputInstance` in `prism_core::MachineConfig` — twelve universes across five outputs of three kinds, hot-reconfigurable without a missed tick, and never in the show file. See §2.35 |
| S36 | `prism-midi` — the real MIDI port | ✅ | 2026-08-22 | CI green on run **32581376059**. The backend has a home: `prism-midi`, the fourth exception in §10.1, with `midir` target-gated so the ARM64 cross-check never sees it. Port enumeration, a name that survives a replug, hot-plug both ways, `--surface` and `Command::SetSurfacePort` in `MachineConfig`. **Nothing in the suite opens a port**, on a machine with a real X-Touch attached. The 🔌 half — the gate over a *real* port — is `ARCHITECTURE_SPEC.md` §14. See §2.36 |
| S46 | Art-Net node discovery | ✅ | 2026-08-29 | CI green on run **33272829395**. The first **receive** path in `prism-protocols`: `ArtPoll` and `ArtPollReply` (`artpoll.rs`), a discovery thread that keeps a table (`discovery.rs`, `prismd::discovery`), and `NodeHealth` — *answering*, *never answered*, *stopped* with the age of the last reply. Punch-list **B6** closed: an Art-Net output whose nodes are silent reads `Degraded` rather than `Ok`, in the snapshot, in the delta and in `Query::OutputStatus`. The poll is **unicast to where the rig already sends** and nothing broadcasts. The settings panel draws both lists and where they disagree, and a discovered node is one click from being an output. See §2.43 |

### Phase 8 — Settings and the control editor
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S37 | The settings window | ✅ | 2026-08-23 | Outputs, devices, show files, this machine — the window `WindowType::Settings` had reserved since S25. **Every operational `prismd` flag is a setting now**: the network exposure and its §2.1 token, the log level, the universe count, the exit action, the autostart flag, the fixture library and the surface profile all live in `prism_core::MachineConfig`, and the WebSocket listener is **on by default**. Five commands the show file needed (`SaveShowAs`, `OpenShow`, `NewShow`, `ExportShow`, `ImportShow`) and one for the machine (`ConfigureMachine`). See §2.39 |
| S38 | The interactive control editor | ✅ | 2026-08-23 | The binding table of `docs/MCU_MAPPING.md` §4 edited at the desk. A fifth settings panel draws all **73 controls**, what each does, and which stay PrismDMX's in the combined Xctl+MC mode; **Learn** names the control an operator presses and does not fire it. The table lives in `prism_core::MachineConfig` beside the rig and the port — a **profile file is an import**, so a desk starts with the keys it was left with. Two new `MachineChange` variants and **no new command**. See §2.40 |

### Phase 9 — Extended features
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S30 | 3D viewer | ☐ | | |
| S31 | Web Remote | ☐ | | The settings window travels there; the **machine** panel does not, and the daemon is what refuses it |
| S32 | PSN / OSC — openfollow.app | ☐ | | Gains its settings panel, and OSC as a *surface* in the control editor rather than a second mapping system |

### Phase 10 — Documentation and release
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S41 | The manual, and a README in every crate | ☐ | | Added 2026-08-14. Checked by tests: every crate has one, and the manual's lists match the code |
| S42 | prismdmx.de | ☐ | | Added 2026-08-14. Built from this repository, so the documentation cannot drift from a release |

**Done:** 30 / 44 · **In progress:** 0 · **Blocked:** 0

**Eleven sessions were added on 2026-08-14** — S33–S43 — for the output patch,
the real MIDI port, the settings window, the control editor, the desk-layout
rework, view management, the console shell, the executor-function gap S26
recorded, the store modes, the documentation and the cleanup pass. Existing
sessions kept their numbers, because `prism-core`, `prism-surface` and this
file's decision log all reference them by number; the **running order** is in
`IMPLEMENTATION_PLAN.md`.

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

### 2.18 S17 verification record

Measured on 2026-08-12, all exit criteria from `IMPLEMENTATION_PLAN.md` S17 and
the session prompt. The first session that produces a **process** rather than a
library.

| Check | Result |
|---|---|
| The daemon starts, loads a show, outputs DMX to a mock driver, **with no client ever connecting** | ✅ `tests/daemon.rs::a_daemon_loads_a_show_and_drives_dmx_with_no_client_connected`. The show on the platter is the show in memory — three fixtures, not an empty one — and the assertion is on the **frames the driver was given**, because a daemon that held a show and published nothing would pass every other test in the file. It then runs for a second under its own timers with nobody attached and is asked what the tick managed: **44 Hz, within a window of 30 to 55**. A second rather than the fifty milliseconds it takes to see a frame, and that is S10's rule about percentiles applied to a mean — `tick_hz` is ticks over uptime, and over a short window the milliseconds between the engine starting and the clock starting are a measurable share of it |
| A second instance detects the first and refuses to start — **asserted** | ✅ twice, at both levels. `lock.rs::a_second_daemon_is_refused_and_told_where_the_first_one_is` asserts the lock itself; `tests/daemon.rs::a_second_daemon_refuses_to_start_and_says_where_the_first_one_is` starts two real daemons on one data directory and asserts the refusal **names the process that has it** and, more to the point, that **the first daemon goes on driving the rig** while the second is turned away — two daemons on one output is the failure being prevented, so the test says what did *not* happen as well as what did. **Checked by mutation:** removing the refusal on `WouldBlock` turns it red |
| A stale lock file (killed process) is detected and taken over | ✅ `tests/daemon.rs::a_lock_file_from_a_killed_daemon_is_taken_over`. The kill is simulated the only way one process can: the file system is put in exactly the state the operating system leaves it in when it reaps a daemon — the discovery document still there naming process 4711, a Unix socket file beside it, and **nothing holding the guard**. The new daemon takes it, rewrites the document with its own process id, and removes the socket file `LocalListener::bind` deliberately does not (S16 left that here). **Checked by mutation:** leaving the socket file turns it red |
| The handshake serves a `Snapshot` with show **and** session | ✅ `tests/daemon.rs::a_client_connects_and_is_served_the_show_and_the_session`, over a real named pipe, with `prism_ipc::Client` doing the handshake — the client finds the daemon by **reading the lock file**, which is §2.2 exercised rather than described. Both documents are read the way a client reads them, through `prism_core::JsonMirror`. The session is deliberately **not** at its defaults (page 3, a command line with text in it), because a fixture built out of default values cannot tell "carried correctly" from "never touched" (S14). The programmer is there too, which is S16's third document. And the connection is a working one: a command reaches the show, the `ProgrammerChanged` delta arrives **before** the `Ack`, and the daemon goes on driving the rig after the client disconnects |
| Shutdown with sACN termination and **configurable blackout-or-hold** | ✅ `tests/daemon.rs::a_daemon_told_to_black_out_publishes_a_blackout_before_it_stops` runs the same daemon twice and asserts the **last frame the output was actually given**: 255 with `--hold-on-exit`, 0 with `--blackout-on-exit`. Blackout is a frame and not a flag (S10), so the daemon publishes one and waits for the drivers to send it before stopping them; the sACN termination is `OutputRunner`'s, which runs `DmxOutput::shutdown` and is what carries the last look. **Checked by mutation:** stopping the outputs without giving the frame time to leave turns it red |
| The desk identity is generated once and then kept (S10/S11's requirement) | ✅ `a_desk_identity_is_made_once_and_then_kept` starts and stops two daemons over one data directory and asserts the file is **byte-identical** — a fresh CID at every start is a new sACN source at every start, with the old one holding the universe for 2.5 s while the two fight. `tests/wiring.rs::an_sacn_output_unicast_carries_this_desks_identity` asserts the sixteen bytes in the E1.31 root layer **are** the identity in `machine.json` |
| `cargo test -p prismd` | ✅ exit 0 — **80 lib tests** + 15 integration tests across two targets, 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **1 132 tests** across 37 targets, 14 ignored (unchanged: the S8 hardware target, the long engine runs, the crash test's child and the fixture regenerator) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 — and it took a fix to eight *pre-existing* lines in three other crates, because `stable` has moved since S16 and `manual_is_multiple_of` is new. See the decision log: a check that passed on Tuesday is not a check that passes on Thursday |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prismd` **≥ 85 %** | ✅ **94.80 % lines**, 94.83 % regions, 96.35 % functions. `main.rs` is 0 % and is 40 of the 144 uncovered lines — it is the process entry point and a binary target has no tests, which is why the daemon is a library with a binary on top; without it the crate reads **96.19 %**. The rest is four kinds, and each is a rule rather than an omission: the **Open DMX arm** (`CLAUDE.md`: no test may need a device, and this machine has the adapter S8 measured), the **sACN multicast destination** (S10: no test may put lighting data on the network it runs on — so the sACN output *is* tested, unicast to a loopback socket, which §7.2 names as the configuration for a venue that forbids multicast), error arms no input can reach, and the `Err` half of raising the tick thread's priority |
| The daemon contains no `#[cfg(target_os = …)]` | ✅ and CI now runs `cargo test -p prismd` in the **Linux** job, where the same tests run over a Unix domain socket rather than a named pipe. The three places it would have needed one — the user data directory, the tick thread's priority, the IPC endpoint — are each in the decision log |
| New dependencies pass the ARM64 cross-check | ✅ checked **before a line of the crate was written**, as the prompt asked: `thread-priority` 3.1 and `getrandom` 0.4 both compile for `aarch64-unknown-linux-gnu` and neither compiles C, so the CI job is unchanged. (The workspace as a whole cannot be cross-checked on this machine — `rusqlite`'s amalgamation wants `aarch64-linux-gnu-gcc`, which is what the ARM64 job installs — so the two new crates were checked on their own) |
| CI green on the pushed commit | ✅ run **31630334754** on `4a8ef6c` — all four jobs, zero non-success steps, **on the first attempt**: Windows full build and test 9 m 11 s (the `windows` crate arrives with `thread-priority`, which is most of the increase), Linux platform-neutral 2 m 15 s — **where `prismd`'s own tests run for the first time, over a Unix domain socket rather than a named pipe** — ARM64 cross-check 44 s with no toolchain change needed, UI typecheck and build 53 s. The Format and Clippy steps passing there is the second half of the `manual_is_multiple_of` story: the fixes were made against this machine's toolchain and CI's `stable` agreed |

**Delivered:** ten modules and a thirty-line binary. `lock` is the
single-instance guarantee and §2.2's discovery file; `machine` is the desk
identity; `paths` is where a daemon keeps its files, resolved from the
environment rather than from a `#[cfg]`; `engine` is the tick thread, its
priority and the hand-off of a rebuilt merge body; `core` is `ShowFile` plus
`ShowStore` plus every effect that follows from a command; `server` is the one
`ServerHandler` S16 asked for; `daemon` is the assembly, the timers, the
telemetry channel and the shutdown; `cli` is sixteen options and a `--help` that
is the documentation; `log` is the levelled logger `CLAUDE.md` requires.

**The tick thread is the only thread in the process with a deadline, and it is
the only one raised.** S6 measured both halves of that sentence — 45 missed
ticks at ordinary priority, and 7 484 missed when the whole *process* was raised
and the load came from inside it — so `prismd` raises the thread from inside
itself and leaves the runtime, the driver threads and the surface alone.

**Everything that changes what the engine is happens on the core thread.** A
repatch, a group edit, a stored cue and a load all rebuild the whole
`MergeBody`; the tick reads one atomic per tick to notice, takes the new body
with a `try_lock` that never blocks, and hands the old one back to be freed
somewhere that is allowed to free things. The frame is blanked on the tick the
new body arrives, which is the half of `Effect::Repatch` S11 named and the easy
one to forget.

**Five mutation checks turned tests red and a sixth turned nothing red**, which
is in the decision log because it is a fact about the design: diffing the
programmer before the rebuild rather than after is belt and braces, and what
actually holds is `MergeBody::load_programmer` in `build_body`.

### 2.19 S18 verification record

Measured on 2026-08-12, all exit criteria from `IMPLEMENTATION_PLAN.md` S18 and
the session prompt. The first session whose product is a **proof** rather than a
feature: `ARCHITECTURE_SPEC.md` §12's mandatory gate for D2.

| Check | Result |
|---|---|
| A client is **killed mid-show** and the frame sequence has no gap across the whole run | ✅ `tests/resilience.rs::a_client_killed_mid_show_costs_the_rig_nothing`, on the frames the mock driver was given rather than on anything watched. The show is genuinely running when the client dies — the client's own `Go` started the sequence on executor 0, so the light being asserted afterwards is *the light the dead client asked for*. The kill is an `abort` of the task holding the connection, with messages already queued for it: the socket goes without a goodbye, which is what `Client::disconnect` deliberately is not. **"No gap" is two claims and the decision log says why:** no silence longer than 250 ms between consecutive frames for a universe (**measured 24–51 ms over three runs — one output cadence**), and the look never changing by itself from the frame it is first complete on. 76 of the 78 recorded frames are after the kill, and the test asserts the kill fell inside the recorded window, because otherwise every other assertion is about a run that never met the failure |
| The client comes back and re-snapshots onto a state identical to the daemon's | ✅ `a_killed_client_comes_back_to_the_state_it_had_accumulated`, which is also §9's *snapshot completeness* row — the same claim from both ends. A client accumulates a nine-command script into `ShowMirror`, `SessionMirror` and the programmer **by deltas alone**, is killed holding that state, and a fresh client's snapshot is compared against all three documents. The script moves every asserted value off its starting one (S14's rule), and includes a `PatchFixture`, which rebuilds the whole engine under a watching client |
| A protocol version mismatch produces an explicit `Reject`, never undefined behaviour | ✅ `a_client_of_the_wrong_version_is_refused_in_words_and_the_show_goes_on`, over the daemon's real listener rather than through a duplex. **Both directions** — an interface built against a newer engine and one built against an older — because a half-updated installation is the ordinary case, and the refusal has to name **both** version numbers or an operator cannot tell which half to update. The daemon is then asserted to be unaffected: nobody was ever connected, the rig went on being driven, and a client of the right version is served immediately afterwards |
| Backpressure: a slow client loses telemetry, loses **no** command, and does not affect other clients | ✅ `a_slow_client_loses_telemetry_and_no_commands_and_nobody_else_notices`, against a running daemon at §7's real 30 Hz over a rig of 24 patched universes — a telemetry frame is 514 bytes per universe, so a client that stops reading fills a socket buffer in a fraction of a second instead of ten. Three measurements, because the three claims fail separately: the slow client **loses more telemetry frames than it is given** (21 dropped against 17 delivered) and is never disconnected over it; the fast client's **40 command round trips complete in 27–59 ms** while the slow one reads nothing; and when the slow client starts reading again it receives **all 40 control deltas, in order and complete**, each carrying a distinguishable value so the check is not a count |
| `cargo test -p prismd` | ✅ exit 0 — **80 lib tests** + 21 integration tests across three targets (7 + 6 + 8), 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **1 140 tests** across 40 targets, 14 ignored (unchanged: the S8 hardware target, the long engine runs, the crash test's child and the fixture regenerator) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 — and `stable` has not moved since S17, unlike between S16 and S17. The one thing it did catch is worth keeping: `print_stdout` is denied workspace-wide, and this suite *measures*, so it carries the same allowance `prism-engine`'s timing tests do — a figure quoted in this file has to be reproducible with `--nocapture` |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prismd` **≥ 85 %** | ✅ **94.73 % lines**, 94.81 % regions, 96.36 % functions — the same figure as S17 to within a rounding of the test bodies (94.80 % then), and for the same reason: `main.rs` is 0 % and is 40 of the 146 uncovered lines. Without it the crate reads **96.16 %**. `prism-protocols` was re-measured because `MockOutput` changed — **98.43 % lines** with `output.rs` at **100 % on all three** — and `prism-ipc` because `ServerHandle` grew a method: **98.46 % lines**, `server.rs` up from 99.50 % to 99.53 % |
| CI green on the pushed commit | ✅ run **31638518112** on `23cd22e`. **It took three runs and the middle one is the finding.** Run **31635842272** on `5bff4e7` was green on all four jobs; the very next run, **31636431800** on a commit that changed *nothing but PROGRESS.md*, failed on **two different tests on two different platforms** — the backpressure gate on Linux and S17's `an_output_that_falls_over_is_reported_to_every_client` on Windows. Both were real test defects and both are in the decision log; neither was a defect in the daemon. Both were then **reproduced locally under six CPU burners**, fixed, and the suite run three times under the same load before this commit. **The Linux job is the interesting half of a green run:** the whole gate runs there over a Unix domain socket rather than a named pipe — `tests/resilience.rs`, 6 passed in 4.01 s — which is what makes the 250 ms silence bound and the backpressure figures claims about the daemon rather than about how big a Windows pipe buffer is |

**Delivered: one test target and three affordances, and that ratio is the
point.** `crates/prismd/tests/resilience.rs` is 700 lines of assertions; what it
needed from the rest of the workspace was `MockOutputHandle::timeline` (every
frame **with the time it arrived**), `ServerHandle::clients` (which connections
exist, so a counter can be asked for by name) and `Daemon::server`. Nothing in
the daemon had to change for the gate to pass, which is the result S17 was
hoping for and could not claim.

**The gate was checked by four deliberate regressions, and the first one is why
the criterion has two halves.** Blacking the stage out when the last client
disconnects — three words in `DeskHandler::disconnected` — leaves the frame
timing *perfect*: 79 frames, longest gap 24 ms, no silence anywhere, and a dark
stage. A gate written only about timing would have passed it. The other three:
stopping the outputs once the last client has gone turns the gate red on the
frames that never arrive; swallowing the `ExecutorState` delta turns the
reconnect test red with `isActive: false` against `true`, which is a client
silently drifting from the daemon; and making telemetry non-droppable turns the
backpressure test red, because the counter that says a picture was dropped never
moves and the client is disconnected on a full control queue instead.

### 2.20 S19 verification record

Measured on 2026-08-13, all exit criteria from `IMPLEMENTATION_PLAN.md` S19 and
the session prompt. The first session of Phase 5, and the first line of code in
`prism-surface`: **layer 1 of `docs/MCU_MAPPING.md` §1, and only layer 1.**

| Check | Result |
|---|---|
| Table-driven round trip: bytes → event → bytes, **byte-equal both ways** | ✅ `tests/round_trip.rs`. 36 inbound rows and 18 outbound rows, each holding a **literal byte array and a literal note number transcribed from `docs/MCU_MAPPING.md` §2 by hand** rather than derived from the profile — which is the whole point, because a round trip computed from the table it is testing passes with every number in it shifted by one. The mutation check below proves that is not a hypothetical. Three rows that are *not* byte-equal are separated out as **aliases** with the canonical form they normalise onto (a real Note Off, a press at any non-zero velocity, and the V-Pot's minus zero), so the difference is asserted instead of being quietly left out of the table. Two property tests say the same thing about the whole space — every valid event and every valid feedback message, including all 16 384 fader positions and every scribble strip offset and length |
| Running status decoded identically to an explicit status byte | ✅ twice. `midi.rs`'s own test on a hand-built pair, and `the_same_burst_with_the_status_bytes_left_out_decodes_identically`, which takes the **whole 36-row inbound table**, drops every status byte that repeats the previous one, and asserts the two streams decode to the same events — and asserts the stripped stream is actually shorter, so a test that omitted nothing could not pass. The MIDI rules around it are separate claims: a real-time byte does **not** disturb running status, a system common message **cancels** it and takes its data bytes with it, and a SysEx cancels it too |
| Fuzz with truncated and out-of-range messages: no panic, no allocation growth, correct discard counters | ✅ `tests/fuzz.rs` and `tests/codec_allocations.rs`. **No panic:** a quarter of a million pseudo-random bytes per run from a seeded xorshift, plus `proptest`. **Correct counters** is an *identity* rather than a number, because for random bytes nobody can say what the counters should read: every byte pushed is counted, and every complete message is either delivered or counted as unmapped or ignored — a codec that swallowed a message breaks it one way and one that invented a message breaks it the other. Against the deterministic hostile stream the counts are asserted **exactly**, per fault class, per round. **No allocation growth: 0 allocator calls**, measured with a counting global allocator over 250 kB of hostile input decoded twelve times and over 99 kB of every kind of outbound message |
| SysEx split across packets reassembles; an unterminated one times out and is dropped | ✅ a whole 63-byte scribble strip message cut into **four-byte packets**, the size a USB MIDI packet actually is, arriving as one event. A real-time byte landing inside the payload does not join it. An oversized message is discarded **whole** rather than truncated to fit — a scribble strip built from the first 128 bytes of a 200-byte message is a display full of plausible nonsense. And the timeout is asserted from **both** sides of its deadline, at no cost in wall-clock time: the arrival instant is an argument to `push`, so there is no clock to simulate and nothing to wait for |
| Every note number, CC number and channel is **data** | ✅ `profile::X_TOUCH`, one constant, `verified: false`. Nothing in the codec matches on a literal. The two lists that describe the 64 global buttons — the enum and the note table — are asserted to be a bijection, the table to be sorted and hole-free over 40…103, the five strip button rows to tile 0…39 without overlapping, and the two tables to be disjoint over all 128 notes. `verified` is checked in a `const` block, so the day somebody sets it without doing S20's work the **build** stops rather than one test |
| `cargo test -p prism-surface` | ✅ exit 0 — **87 lib tests** + 25 integration tests across three targets (12 + 9 + 4), 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **1 266 tests** across 43 targets, 14 ignored (unchanged) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0. `stable` brought two lints this crate had to meet on its first day — `byte_char_slices` and `cloned_ref_to_slice_refs` — both in test code, both fixed here. The allocation target carries the `print_stdout` allowance the other measuring suites do |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-surface` **> 95 %** | ✅ **99.24 % lines**, 98.58 % regions, 97.94 % functions — `control.rs` and `midi.rs` at **100 % lines**, `profile.rs` 98.79 %, `codec.rs` 98.71 %, `feedback.rs` 98.26 %. The 17 uncovered lines are `panic!` arms in tests that pass and derived implementations. Two genuinely unreachable branches found while reading the report were **removed** rather than covered: `RingMode::from_bits` returns a mode instead of an `Option`, because all four two-bit codes are defined, and the SysEx header is read with `first`/`get` instead of a slice pattern with a dead `else` |
| `cargo check -p prism-surface --all-targets --target aarch64-unknown-linux-gnu` | ✅ exit 0. The one new dependency is `proptest`, already in the workspace and pure Rust; nothing new compiles C |
| CI green on the pushed commit | ✅ run **31655311363** on `8aefe2a` — all four jobs on the **first attempt**: Windows 9 m 51 s, Linux neutral 2 m 21 s, ARM64 check 49 s, UI 47 s. The Linux job is the one that matters for this crate: `prism-surface` is platform-neutral and its whole suite — the allocator target included — runs there as well as on Windows |

**The crate depends on nothing.** `prism-domain` is in its manifest and layer 1
does not yet use a single type from
it — deliberately: `GlobalButton::Play` is note 94 in this crate, and that
pressing it starts an executor is layer 3's opinion (S22). There is no
`Command`, no show, no session, and no MIDI port either: binding to a device is
platform code and belongs to the surface thread, which is what lets the whole
suite run in the Linux CI job with nothing plugged in.

**Seven deliberate regressions, and the two most useful are the ones only the
allocator can see.** Swapping the two halves of the 14-bit fader split *in both
directions* leaves every property test green — an encode/decode pair still
agrees with itself — and turns the hand-written byte tables red; that is
`ARCHITECTURE_SPEC.md` §12's row, and the reason the tables are transcribed
rather than generated. Reading the V-Pots as two's complement, symmetrically,
does the same. Forgetting running status after each message turns three tests
red. Truncating an oversized SysEx instead of discarding it turns three red.
Checking the SysEx timeout only on an explicit poll turns one red. And **two
mutations that keep every functional test green**: reassembling into a `Vec`
rather than into the fixed array, and assembling an outbound SysEx into a `Vec`
before copying it — 87 lib tests, 9 fuzz tests and 12 round-trip tests all pass,
and only `tests/codec_allocations.rs` says a word. That is S16's finding
reproduced: a functionally correct implementation can be the wrong one, and only
the counter sees the difference.

---

### 2.21 S20 verification record

Measured on 2026-08-13 against the console itself — **Behringer X-Touch, MC mode
over USB, firmware V1.25, serial `0156406`** (USB `1397:00B1`). All exit criteria
from `IMPLEMENTATION_PLAN.md` S20 and the session prompt.

The headline is short: **`docs/MCU_MAPPING.md` §2 was right. Not one note number,
CC number, MIDI channel or buffer offset had to change.** S19's bet — hold the
whole protocol table as one constant so that verification is a data edit — paid
out, and it paid more cleanly than S8's did, where the guessed frame rate turned
out to be the symptom of a real defect.

| Check | Result |
|---|---|
| Every item in `docs/MCU_MAPPING.md` §7 ticked | ✅ all fifteen, plus the three S19 added. Two are marked ◻ **untested** rather than ticked, and both say why: the foot switches (notes 102/103) need a pedal that is not plugged in, and the cable-pull case belongs to S21's connection handling. An absent pedal is not a result |
| Mode and connection recorded | ✅ **MC over USB**, read off the front panel with channel 1 SELECT held at power-up *and* confirmed independently by the device ID the surface answers SysEx on (`0x14`) |
| Firmware version recorded | ✅ **V1.25** — so ≥ 1.22 and the colour extension exists. Found by SysEx rather than by reading a boot screen: `F0 00 00 66 14 13 F7` is answered with `…14 "V1.25"`, an **undocumented firmware request** now written into §2.3 |
| The 40 strip-button notes | ✅ pressed strip by strip; notes 0–7 / 8–15 / 16–23 / 24–31 / 32–39 exactly as tabulated |
| The 64 panel notes | ✅ **60 verified in both directions at once** — the host lit one LED and the button that lit was pressed, so the outbound map and the inbound map are checked by the same act. Notes 52 and 53 pressed blind (no LED — see below). Notes 102/103 untested |
| Fader channels and 14-bit byte order | ✅ pitch-bend channels 1–8 left to right, 9 for the main fader; **LSB first**, proved by the top-of-travel message `E0 7C 7F` = 124 + 128 × 127 |
| Fader touch notes | ✅ 104–111 in order, 112 for the main fader |
| V-Pot acceleration — the first thing §7 asked | ✅ **the V-Pots accelerate (magnitudes 1…8) and the jog wheel does not (±1 in 404 messages, however hard it is spun).** Two controls the document described identically are not the same |
| Scribble strip offsets and the 56th character | ✅ strip *n* owns 7 characters at `7n`; **all 56 of a line arrive** (Ardour's 55 is caution, not a limit); the buffer is one continuous 112 characters — a write at offset 55 spills onto the lower line — and a single 112-byte message fills both |
| Colours — all four open questions | ✅ the eight values in the documented additive order; **no inverted variant in MC mode** (bits 3/4/6 are masked away); **a message without exactly eight colour bytes is ignored** (0, 4 and 9 all tried), so there is no per-strip form; **text does not reset the colour** |
| Meter format, overload flag, decay | ✅ the `(strip << 4)` + level split confirmed, and `0xE`/`0xF` set and clear the marker — **but the decay is under a second**, not the ~300 ms per division the sources claim, which would be 2–4 s. The LCD meter-mode SysEx does nothing at all here, and the overload marker works regardless of it |
| 7-segment addressing, encoding and channel | ✅ digit 0 is the **rightmost** (`0…9 A B` to digits 0…11 reads `BA9876543210`); CC 76 ignored; **the display is accepted on MIDI channel 16 as well as channel 1**, which is the measurement that justifies the codec taking both |
| What a 7-segment `0` draws | ✅ **a blank**, and so does 32. The contradiction is settled against the stripping rule: `'@'` cannot be shown at all. `SegmentChar::from_ascii` now **refuses** it and `to_ascii(0)` answers a space |
| A strip index above 7 | ✅ **ignored, never wrapped** — meters at index 8 and 15, ring LEDs on CC 56/63, note 120, pitch bend on channel 10, CC 76, and the extender device ID `0x15` all did nothing |
| Whether back-to-back messages are lost | ✅ measured, and the answer is worse than the reports — see the decision log. Small bursts are lossless; large bursts alone are harmless; **saturating both directions at once loses replies and can stop the surface transmitting until it is power-cycled** |
| Round-trip latency | ✅ **median 0.71 ms**, min 0.612, max 1.023 over 60 exchanges, host → surface → host. The figure that actually bounds `ARCHITECTURE_SPEC.md` §4.3 is different and also measured: a moving fader is reported every **19.8 ms** |
| §2 and `profiles/surface/xtouch.json` updated, banner removed | ✅ §2 carries the measurements, the UNVERIFIED banner is gone, §2.7 is the measurement and §7 is closed. The JSON is written — as **layer 3**, the binding table, plus the verification record. It deliberately does **not** copy the note map; see the decision log |
| `X_TOUCH.verified` is `true`, §14 ticked | ✅ and the `const` assertion S19 planted was rewritten in the same edit, which is exactly what it was for. `ARCHITECTURE_SPEC.md` §14's row is closed |
| `cargo test --workspace` | ✅ exit 0 — **1 265 tests across 44 targets**, 14 ignored. Also run with the desk plugged in, before any of this session's edits: same result, and nothing in the ordinary suite touches a device |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0. One lint met on the way: `assertions_on_constants` on `assert!(X_TOUCH.verified)` in the new target, which is now a `const` block like the one in `profile.rs` |
| `cargo fmt --all --check` | ✅ exit 0 |
| `cargo check -p prism-surface --all-targets --target aarch64-unknown-linux-gnu` | ✅ exit 0. **`prism-surface` gained no dependency at all** — the MIDI port lives in a crate outside the workspace, so nothing new compiles for ARM64 or for Linux |
| CI green on the pushed commit | ✅ run **31700591538** on `e2fe45b` — all four jobs on the **first attempt**: Windows 6 m 19 s, Linux neutral 2 m 08 s, ARM64 check 48 s, UI 50 s. The Linux job is the interesting one: the new `hardware_capture` target runs there too, so the X-Touch measurements are checked on a machine that has never seen an X-Touch |

**What the session produced, in three parts.**

**1. A tool, outside the workspace on purpose.** `tools/xtouch-probe` is the only
code in the repository that opens a MIDI port. `ARCHITECTURE_SPEC.md` §10.1 allows
`prism-surface` no platform code, so rather than bend the rule the tool is its own
crate with its own `[workspace]` table: `cargo test --workspace`, clippy and the
ARM64 cross-check never see `midir`. It depends on `prism-surface` by path, which
is the point — **every byte it sent was produced by `Feedback::encode_into` and
every byte it read was decoded by `McuCodec`**, so what was verified is the
shipping codec and not a transcription of it. The exceptions are marked in its own
output: the `raw` command, and the steps that deliberately send what the codec
*refuses* to encode — which is how "what does the surface do with a strip index of
8?" gets asked at all.

**2. The verification came back into the workspace as evidence rather than as
prose.** `crates/prism-surface/tests/captures/` holds four recordings of what the
desk sent — every strip button, the whole panel, all nine faders, all nine
relative controls, about 2 500 messages — and `tests/hardware_capture.rs` replays
them in the ordinary suite. A device-specific claim is now checked on a build
server with nothing plugged in, for ever. `verified: true` is a sentence about one
evening; the captures are what keeps it true.

**3. Six corrections, none of them a note number.** Every one is something no
source stated:

- **The faders are 12-bit.** All 576 captured positions are multiples of **4**, the
  LSB only ever takes the 32 values `00,04,…,7C`, and the top of travel is
  **16380** — the surface never sends 16383. Held as `McuProfile::fader_step`,
  because a percentage computed against 16383 gives 99.98 % for a fader against
  its end stop, and an executor master that cannot reach full is wrong.
- **The V-Pots accelerate and the jog wheel does not**, on the same encoding.
  Layer 2 needs two curves, not one.
- **Two panel buttons have no LED**: Name/Value (52) and SMPTE/Beats (53) send
  their notes and stay dark at any velocity. `McuProfile::unlit_buttons` says so as
  data.
- **The encoders have no lamp under them**, so ring bit 6 has nothing to light.
- **A 7-segment `0` blanks the digit**, which cost `SegmentChar` a corrected
  mapping — the only change to shipping behaviour in the whole session.
- **The meters decay in under a second**, several times faster than documented.

**And one fault, which is the session's real product.** Flooding the surface with
large SysEx writes *while replies are outstanding* can stop its MIDI transmitter
dead: no buttons, no faders, no replies — while it goes on **receiving perfectly**,
displaying text written to it in that state. Reopening the port does not help and
neither does a fresh process; only a power cycle. It happened twice, reproducibly,
and each half of the recipe is harmless on its own. That is S8's lesson in a second
setting: **the thing worth having the hardware for is the failure a mock cannot
contain.** The three consequences for S21 are in `docs/MCU_MAPPING.md` §2.7 and
§5.3.

**A note on method, because the first attempt at the panel was wrong.** The obvious
approach — print a press order, compare the capture against it — produced 71
"mismatches" that were nothing of the kind: the walk had been done strip by strip
where the printed list ran row by row. An order a person has to follow exactly
makes the *person* the thing under test. Lighting one LED and asking for whichever
button lit removes the order from the experiment altogether and verifies the
outbound map in the same pass. That is the method to reuse for the next surface.

### 2.22 S21 verification record

Measured on 2026-08-13, all exit criteria from `IMPLEMENTATION_PLAN.md` S21 and
the session prompt. **Layer 2 of `docs/MCU_MAPPING.md` §1 — the shadow model —
and the four rules of §5.** No hardware: everything here is asserted against a
mock, which is what §6 of that document has always required and what makes the
whole of §5 testable with arithmetic rather than with a wait.

| Check | Result |
|---|---|
| Touch suppression: **no** pitch bend while touched, **exactly one** resync 150 ms after release — assured, not observed | ✅ `tests/feedback_rules.rs`. Half a second of the show driving a touched fader as hard as it can produces **zero** messages to it; the 149 ms after release produce zero; then exactly one, and nothing for the three seconds after that. The one message's bytes are `E0 7E 3F`, **worked out by hand**: level 0x8000 of 0xFFFF against a top of travel of 16380 is 8190, and 8190 is 63 × 128 + 126, LSB first. **"Exactly one" is a guarantee because a touch invalidates the shadow**, so the resync happens whether or not the value moved — `a_fader_nobody_moved_is_still_resynchronised_after_a_release` is that case, and the mutation below is why it is a separate test. A hand on one fader suppresses **only** that fader, asserted on the other eight |
| Coalescing: 1000 changes in 100 ms → at most 3 messages for that control | ✅ 1000 changes at 100 µs intervals, pumped after every one. **At most 3**, and the value that survives is the **last** one rather than whichever frame boundary caught a stale reading. Repeated for eight faders, eight meters and eight rings changing together, so the bound is per control rather than per port |
| Priority under bandwidth pressure, in the documented order | ✅ every control on the surface different at once — 156 messages — drained one per minimum gap. The class sequence is **sorted**: the first nine out are the motor faders before a single LED, the last eight are the meters. A separate test gives the port one message per frame with meters changing every frame, and the LED still goes first while the meters starve — which is §5.2's *dropped first* and is correct, because a dropped meter falls rather than freezing (§2.7). **A message is classified by a `match` on its status byte transcribed from §2.2 by hand**, not by `Priority::of`, which is the code under test |
| The device's disappearance leaves the engine untouched | ✅ the port going away is not reported anywhere: the picture goes on being maintained, 200 further state changes are accepted, **nothing** is written, and a reconnect redraws the whole surface once. A hand that was on a fader when the cable went is not still on it afterwards, or that fader would be suppressed for ever. Also asserted from the other side: a controller with no surface attached at all accepts state and sends nothing |
| `cargo test -p prism-surface` | ✅ exit 0 — **161 lib tests** + 50 integration tests across six targets (12 round trip, 12 feedback rules, 9 fuzz, 9 hardware capture, 4 + 4 allocations), 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **1 351 tests across 46 targets**, 14 ignored (unchanged) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0. `stable` has not moved since S20; the one lint met on the way was `manual_is_multiple_of` in the new allocation target, which the new measuring suite also carries the `print_stdout` allowance for |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-surface` **> 95 %** | ✅ **99.20 % lines**, 98.66 % regions, 98.28 % functions — `accel.rs`, `model.rs`, `control.rs` and `midi.rs` at **100 % lines**, `color.rs` 99.32 %, `profile.rs` 99.27 %, `feedback.rs` 99.01 %, `codec.rs` 98.71 %, `surface.rs` 98.39 %. Three unreachable branches found while reading the report were **removed** rather than covered: the send loop's two conditions became one (so its `break` is the ordinary end of the queue), `probe_due`'s two early returns became one, and the fader loop now walks `ALL_FADERS` instead of converting an index that cannot be wrong. What is left uncovered in `surface.rs` is the *cannot happen* arm of each `emit` branch — `let Some(...) else { return false }` on an array the diff has already bounded — which a crate that denies `panic!` has to write and no test can reach |
| `cargo check -p prism-surface --all-targets --target aarch64-unknown-linux-gnu` | ✅ exit 0. **No new dependency**: layer 2 needed nothing that was not already in the workspace, and the one thing it added to the crate's own graph is `prism-domain`, which was in the manifest **unused** since S19 and is now used for exactly one type — `RgbColor`, on the way into the colour quantiser |
| CI green on the pushed commit | ✅ run **32581376059** on `a47e430` — all **five** jobs: Windows full build and test 12 m 49 s, Linux platform-neutral crates 3 m 12 s, Linux ARM64 cross-compile check **40 s**, UI typecheck/lint/test/build 1 m 40 s, UI end-to-end against a daemon 2 m 28 s. **The ARM64 job is the one that matters this session** and it is unchanged at 40 seconds, which is the measurement: a crate that holds `midir` was added to the workspace and that job compiles none of it. It took three attempts and none of the three was S36 code — see below.
**The two documentation commits on top of `a47e430` have no run of their own**, and not because anything failed: run **32582085371** on `7b46bcf` was declined by GitHub Actions before a single job started — five jobs "failed" in five seconds with no step and no log — for a billing reason on the account rather than anything in the repository. Every line of code in this session is covered by the green run above; what is not covered is two edited sentences in this file and one in `IMPLEMENTATION_PLAN.md` |

**Three things CI found that this machine could not**, and each is a class
rather than a typo. Two of the three are **latent flakes in S33's tests that
this session's run happened to expose** rather than anything S36 changed, and
both are recorded here because the next session should not rediscover them:

- **A test that assumed a MIDI backend is a test that only runs on Windows.**
  `a_real_port_that_is_not_there_is_attached_all_the_same_and_says_why` asserted
  that the reason mentions the port name — which is `PortError::NotFound`'s
  wording. The Linux job has **no backend compiled into it**, so it meets
  `NoBackend` instead, and the assertion was red there and green here. It now
  asserts the reason is *one of the two*, which is the honest claim and is
  better than the original: from the caller's side *this port is not plugged in*
  and *this build cannot look* are the same fact, and both produce a port that
  is attached, not open, and not a failure to start. **The no-backend build is
  not a lesser build to be tolerated in CI — it is the machine-with-nothing-
  plugged-in case, exercised for real.**
- **A pointer freed is a pointer the allocator may hand back.**
  `outputs.rs::a_rename_keeps_the_thread_and_a_re_addressing_replaces_it` is
  S33's, and it asserted that a re-addressed output has a *different*
  `Arc<OutputStatus>` by comparing `Arc::as_ptr` across the restart — after
  dropping the original. On run **32580777886** the replacement landed at the
  address the old one had just vacated, and `assert_ne!` compared
  `0x1e635c4fc90` against itself. The rule the test is about was kept perfectly;
  the test could not tell. It holds the old `Arc` across the reconcile now, so
  the two allocations are genuinely distinct and pointer identity means what the
  assertion says. **The general shape: an identity claim over a value that has
  been dropped is not a claim at all** — keep the thing alive, or compare
  something other than an address.
- **An absolute `missed_ticks == 0` is a claim about a runner, not about a
  fault.** `tests/outputs.rs::an_output_whose_device_disappears_degrades_alone`
  is S33's and nothing this session went near it, but it read **1** on run
  32579984199 where every run before it had read 0. `missed()` counts from
  start-up, and a cold two-core runner may perfectly well drop one 22.7 ms
  deadline while a twelve-universe show and five driver threads are still coming
  up. The assertion now compares the count **across the panic**, which is what
  its own message always said — *and it did not miss a tick over it*. The other
  two places this session asserts the same thing were already deltas.

**What was built, and the shape of it.** One object: `SurfaceController` holds the
codec, two `SurfaceState`s — what the show wants shown and what the desk was last
told — and the rules between them. It owns no port, no thread and **no clock**;
`push(bytes, now, sink)` and `pump(now, sink)` are handed the instant, which is
S19's rule one layer up. Outbound traffic is the difference between the two
pictures, walked in §5.2's order, and the shadow moves **only when a message
actually goes out** — so coalescing and dropping under pressure are the same
mechanism, and a frame that could not be finished is finished by the next one
rather than lost.

**Three of S20's findings are designed around rather than noted.**

- **Faders scale against `max_reported_position()`** — 16380 — in *both*
  directions, so a master at full parks the fader exactly where the surface
  itself reports full. A mutation that scaled against `FADER_MAX` turns two tests
  red.
- **Two acceleration curves that read different things.** The V-Pot curve reads
  the magnitude the desk measured (1…8 → 1, 3, 6, 10, 15, 21, 28, 36); the jog
  curve reads the **interval since the last message**, because the wheel raises
  its rate and never its magnitude. Two curves rather than two tables: a shared
  one would make the wheel a control that cannot be hurried, which is what an
  operator reaches for it to do.
- **Silence is a fault state with words for it.** A desk that *was* talking and
  goes quiet is asked **once** — the device query, and only when the send queue
  is empty, which is the opposite of the condition that caused the fault — and if
  that goes unanswered the health is `Unresponsive`, whose remedy text says
  *power-cycle it. Reopening the port or restarting will not bring it back*. A
  desk nobody has touched yet is **not** called dead, which is why `Connected`
  and `Live` are two states: an X-Touch speaks only when it is touched.

**And the pacing is a floor on the wire, not a counter.** A minimum of 1 ms
between outbound messages, enforced against the caller's clock, so a controller
pumped more often does not send faster. A full resync burst is 156 messages and
therefore about 156 ms, spread over five frames — nowhere near the traffic that
killed the surface's transmitter in S20.

**Eight deliberate regressions, and the first one is the reason a test exists.**
Removing the shadow invalidation on touch leaves
`releasing_a_fader_resynchronises_it_exactly_once` **green** — the value had
changed, so a message goes out anyway — and turns exactly one test red:
`a_fader_nobody_moved_is_still_resynchronised_after_a_release`. That is the
difference between a guarantee and a coincidence, and without the second test the
session would have claimed the first. The other seven: diffing every pump instead
of every frame turns both coalescing tests red; queueing meters before faders
turns four red; removing the minimum gap turns two red; scaling against
`FADER_MAX` turns two red; letting the handshake be polled turns three red;
driving the two lampless buttons turns four red (the burst becomes 158); and using
the V-Pot curve for the jog wheel turns one red.

**Zero allocations, on the outbound path as well.** `tests/surface_allocations.rs`
is S19's counting allocator pointed at layer 2: a simulated minute of a running
show — every fader, meter, ring, colour, name and LED changing thirty times a
second — plus twenty disconnect-and-reconnect cycles and a busy inbound stream,
and **0 allocator calls** in every window. Nothing required this: §2.4's promise
was about the codec. It is measured because the obvious implementation of a diff
is a `Vec` of messages built thirty times a second on the thread next to a 44 Hz
tick, and S16's finding is that only the counter sees the difference.

### 2.23 S22 verification record

Measured on 2026-08-13, all exit criteria from `IMPLEMENTATION_PLAN.md` S22 and
the session prompt. **Layer 3 of `docs/MCU_MAPPING.md` §1 — the binding table —
and the mandatory gate for D11.** No hardware: the surface is a mock MIDI port,
and the two note numbers pressed are transcribed from §2.1 by hand.

| Check | Result |
|---|---|
| The default profile reproduces **every row** of `docs/MCU_MAPPING.md` §4.1 | ✅ `tests/bindings.rs`, one test per row, **expectations copied from the document by a person**. Also the complement, which is the half a table of defaults can get wrong quietly: twenty-six panel buttons are bound and the other thirty-eight are not, asserted button by button. Three rows name something the command vocabulary has not got — `XFade`, `On`, and the executor-button list — and all three are recorded rather than invented; see the decision log |
| The shipped `profiles/surface/xtouch.json` **is** the built-in default table | ✅ asserted by `the_shipped_profile_is_the_built_in_default_table`, with the file compiled in by `include_str!`, so a profile that stopped parsing fails the build rather than one desk on one evening. The two must not drift: if they did, a malformed profile would silently *change* what the desk does, which is the opposite of what falling back is for |
| A malformed profile falls back with a warning and **never** blocks startup — assured, not observed | ✅ twice. `Bindings::load` **has no error path**: it answers with a table whatever it is given, which is the criterion as a type rather than as a habit, and twelve deliberately broken texts are asserted to produce the working default table. And end to end, through a real file and a real process: `crates/prismd/tests/surface_gate.rs` starts a daemon pointed at a profile that binds the reserved button, and the daemon starts, warns, and the desk works |
| **D11 gate** — `Channel ▶` and F1 with no client connected; a client then finds both in its `Snapshot` | ✅ **passed.** The precondition is asserted rather than described: `client_count()` is **0** when the two buttons are pressed and still 0 when the session has changed; only then does a client connect and ask for the world. Its snapshot carries `activeViewId = 2` and `openWindows[0].type = "FixtureSheet"`. The notes are 49 and 54, written out from §2.1 by hand — asking the profile which note to send would be asking the code under test what to press |
| A profile that binds SMPTE/Beats is refused, with a message that says why | ✅ `ProfileError::ReservedControl`, whose text names the button, the combined Xctl+MC mode, *the button that switches the desk between the two hosts* and *leave it unbound*. Belt and braces: layer 2 drops its presses and counts them, so the binding could never have fired — the refusal is for the person who wrote the profile. Asserted on the wording, not merely on the variant |
| `cargo test -p prism-surface` | ✅ exit 0 — **179 lib tests** + 71 integration tests across seven targets (21 bindings, 12 round trip, 12 feedback rules, 9 fuzz, 9 hardware capture, 4 + 4 allocations), 0 failed, 0 ignored |
| `cargo test --workspace` | ✅ exit 0 — **1 419 tests across 48 targets**, 14 ignored (unchanged). **53 of them are this session's**: 39 in `prism-surface` (211 → 250), 8 in `prismd`'s library and 6 in its new gate target (102 → 116). The two new targets are `prism-surface/tests/bindings.rs` and `prismd/tests/surface_gate.rs`. *The workspace total therefore moves by 68 against §2.22's figure of 1 351, and the extra 15 are not new tests* — summing the unchanged crates today gives 1 366 for S21's tree, so that record undercounted. Recorded rather than quietly corrected, because a number in this file that nobody can reproduce is worse than one that is wrong |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0. One lint met on the way and it was in a test: `useless_format` on a `format!` with nothing to interpolate |
| `cargo fmt --all --check` | ✅ exit 0 |
| Coverage on `prism-surface` **> 95 %** | ✅ **99.30 % lines**, 98.68 % regions, 98.46 % functions. `accel.rs`, `midi.rs`, `model.rs` and `control.rs` at **100 % lines**; the new `binding.rs` at **99.85 % lines** / 98.72 % regions, `profile.rs` 99.39 %, `color.rs` 99.32 %, `feedback.rs` 99.01 %, `codec.rs` 98.71 %, `surface.rs` 98.39 %. Three uncovered lines in `binding.rs` were found by reading the report — the `action()` arms for the two faders and the wheel, which every test reached through `command()` instead — and are now a test rather than an exception |
| Coverage on `prismd`, which grew a module | ✅ **95.03 % lines** (94.73 % at S18, so the new module is above the crate's own average rather than below it): `surface.rs` **96.94 % lines**, 97.32 % regions. What is left uncovered there is `main.rs`, which has no test target, and the arms a defensive `let … else` needs on a buffer the caller has already bounded |
| `cargo check -p prism-surface --all-targets --target aarch64-unknown-linux-gnu` | ✅ exit 0. Two new dependencies, both already in the workspace and both pure Rust: `serde` and `serde_json`, for layer 3 only. Layers 1 and 2 use neither |
| CI green on the pushed commit | ✅ run **31728064334** on `cbde96e` — all four jobs on the **first attempt**: Windows full build and test 8 m 19 s, Linux neutral 2 m 28 s, ARM64 cross-check 47 s, UI 51 s. The Linux job is again the interesting one: `surface_gate` runs there too, over a **Unix domain socket** rather than a named pipe, so the D11 gate is asserted on a machine that has never seen an X-Touch *and* over the other transport |

**What was built, in two halves.**

**1. `prism_surface::Bindings` — layer 3, and it does no arithmetic.** A fixed-size
`Copy` table: one action per strip control, one per panel button, one for the
wheel. `SurfaceAction` is a vocabulary of its own rather than a `Command` with
holes in it, because a binding is written before there is an event and half of
`Command`'s fields are answers only an event and a session have. Those answers
arrive as `SurfaceContext` — executor page, selected executor, the two
neighbouring views, the programmer page, the attribute under the jog wheel —
which is how a crate that holds **no show and no session** can produce
`SetExecutorMaster { executor_id, level }`. Everything that needed a measurement
was done one layer down, exactly as S21 promised it would be: layer 3 is a table
lookup and a `match`.

**2. `prismd::surface` — the half that has a port, a clock and the daemon's
state.** A `SurfacePort` trait (the seam `CLAUDE.md` requires), a `SurfaceLink`
that reads, applies, repaints and pumps once every millisecond, and
`load_profile`, which is the filesystem `prism-surface` deliberately has not got.
A press becomes a `Command` and the command goes through **`Desk::command` — the
same door every client uses**, so what a snapshot carries is the daemon's own
state and there is no second command path to keep in step. That sentence is D11,
and `tests/surface_gate.rs` is that sentence as a test.

**No MIDI backend was written, and that is deliberate.** Nothing in the
repository opens a MIDI port except S20's probe, which lives outside the
workspace. The trait is where a backend will go; choosing one is a question about
`midir` and about which crate may hold platform code (§10.1), and answering it
was not needed to pass this gate.

**The three rows §4.1 asks for and the protocol cannot answer.** `XFade` on the
main fader, `On` on Play, and the strip buttons' configurable list
(`LearnSpeed`, `Flash`, `Toggle`, …) are all `ExecutorFaderFunction` and
`ExecutorButtonFunction` values — *show data on the executor* — and there is no
command that presses an executor's button and lets the executor decide what that
means. Rather than invent one in a user-editable file, the bindings resolve to
the commands that exist and the gap is written down in three places:
`docs/MCU_MAPPING.md` §4.2.1, the shipped profile's `deviationsFromSection41`
block, and the decision log below. It is **one** gap, not three, and the session
that adds an executor-button command closes all of it — which is also where a tap
for speed lands.

**Six deliberate regressions, and one of them turns the exit criterion red.**
Making `Bindings::load` fall back to an *empty* table instead of the defaults
leaves every unit test green and turns two red: the property test over twelve
broken profiles, and — the one that matters — `prismd`'s
`a_malformed_profile_leaves_the_desk_working_on_the_built_in_bindings`, which is
the exit criterion measured against a running daemon. The other five: dropping
the reserved-button refusal turns two red; resolving a strip against page 0
instead of the current page turns one red; swapping `StepView`'s two directions
turns two red, one of them in the §4.1 transcription; never painting the show
onto the surface turns the drawing test red (both faders would read 0, and the
test asserts two *different* positions for two different masters); and taking the
surface out of the daemon's run loop turns **all six** gate tests red, each by
name and each in under ten seconds.

**And the picture goes the other way too.** The daemon paints the current page's
executors onto the faders and the scribble strips, the active ones onto the
Select LEDs and the unsaved-changes flag onto the Save lamp (§4.1's last row).
The gate target asserts it end to end on the bytes: the first nine messages out
are the nine motor faders **before a single LED** — §5.2's order, classified by
status byte transcribed by hand — strip 0 parks at `E0 7C 7F`, which is 16380
worked out by hand from a master of 65535, and the sequence's name arrives inside
a SysEx as `SEQUENC`, folded to upper case the way the hardware folds it.

### 2.24 S23 verification record

Measured on 2026-08-13, all exit criteria from `IMPLEMENTATION_PLAN.md` S23 and
the session prompt. The first session whose product runs in a **browser**, and
the first line of `ui/src` that is not the Vite template.

| Check | Result |
|---|---|
| **Deltas applied to the mirror reproduce the daemon's state — property-tested against a recorded delta stream** | ✅ `ui/src/mirror/recording.test.ts`, over `ui/tests/fixtures/daemon-recording.json`: **twelve scripted sequences, 93 deltas**, recorded off a running `prismd` over a real socket by `crates/prismd/tests/ui_recording.rs`. Each case is a snapshot, the deltas a randomised command script produced, and the snapshot a **second client** was served afterwards — `docs/IPC_PROTOCOL.md` §9's *snapshot completeness* row, from the browser's end. **Both halves of the comparison come from the daemon**: nothing in TypeScript computes what the answer should be, which is the trap S19–S22 each found in their own layer. What the test exercises is the whole path — base64 → MessagePack → `readServerMessage` → `applyDelta` → a document compared against one `prism-core` serialised |
| The recording is not vacuous, and cannot go stale quietly | ✅ three guards. In TypeScript: every case has deltas, all three documents move across the file, and all four document-touching delta kinds appear (`ShowPatch`, `SessionPatch`, `ProgrammerChanged`, `ExecutorState`). In Rust, on **every** `cargo test`: `the_recording_is_a_delta_stream_this_build_could_have_sent` decodes every payload with this build's own types and replays it through `prism_core::ShowMirror`/`SessionMirror` — so a wire format that moved stops decoding *there*, next to the daemon, rather than going stale in `ui/`. The regenerator itself is `#[ignore]`d, the way `prism-core`'s frozen migration fixture is: rewriting a committed recording is a deliberate act |
| **The client's encoder is what the daemon reads — asserted on bytes** | ✅ the recording also carries **thirteen client messages** encoded by `rmp-serde`, and the browser encodes the same thirteen and compares **byte for byte**: the tag first, fields in declaration order, the narrowest integer that fits, `nil` for an absent token and *no member at all* for an absent `params`. That is the only way to check an encoder against a decoder that is not in the process — and it passed on the first run, which is worth recording because it is a claim about two libraries agreeing (`@msgpack/msgpack` and `rmp-serde`) rather than about this code |
| **Daemon restart: the interface shows disconnected, reconnects, re-snapshots, and no value from before is left standing** | ✅ **twice, at two levels.** In jsdom (`src/App.test.tsx`): the socket drops, the status reads *Disconnected — retrying in 0.1 s*, and the assertion is on the **document** — `command-line` and `executor-page` have gone from the page altogether and the old text is nowhere in `document.body.textContent`. Then the clock is advanced, a fresh snapshot arrives with a different session, and the old value is still nowhere. And against a real daemon (`e2e/reconnect.spec.ts`, Playwright + Chromium): `prismd --mock-output --websocket` is **killed**, the page says so, and the daemon is restarted on the same data directory — where the command line the interface was showing no longer exists, because it was never saved into the show. **2 passed in 40.6 s** |
| The interface is not merely green when it reconnects | ✅ the end-to-end test types a second command into the reconnected daemon and asserts the readout follows. A status pill can be wrong on its own; a `SessionPatch` coming back cannot |
| A version mismatch is not called a lost connection | ✅ `RejectReason::ProtocolVersion` produces its own status — *the interface and the engine are different versions* — with its own explanation and a retry at the **longest** backoff, because waiting is not the remedy. Everything else that ends a connection reads *Disconnected*. The distinction is asserted in `connection.test.ts` and in `App.test.tsx`, and the two refusals that do **not** end a connection (§8, last paragraph) are asserted to leave it open |
| **The transcription of `prism-ipc`'s envelope is checked from Rust** | ✅ `crates/prism-ipc/tests/interface_protocol.rs` `include_str!`s `ui/src/ipc/protocol.ts` and asserts the protocol version, every `RejectReason` **and the count** (so a variant removed here cannot linger there), every `ClientKind`, every message tag in both directions, `Hello`'s three fields, the snapshot's five, and the exact text of `closesTheConnection` against `RejectReason::closes_the_connection`. The same shape as S22's `the_shipped_profile_is_the_built_in_default_table` |
| `npx tsc -b --force` clean with `strict: true`, **no `any` anywhere** | ✅ exit 0. `strict`, `noUncheckedIndexedAccess`, `noImplicitReturns`, `noUnusedLocals`, `erasableSyntaxOnly` — the S0 settings, unchanged. **`any` appears nowhere in `ui/src` or `ui/e2e`**, and **no shipped module contains a type assertion**: a decoded payload is `unknown` and every narrowing is a reader in `src/ipc/shape.ts` that answers with the type or throws a `ProtocolFault` naming the field. Four assertions exist and all four are in *test* files — a synthetic `CloseEvent` for the WebSocket adapter, the parsed recording, and the client's own encoded message read back — because a test is allowed to know what it just constructed. The `as const` in `protocol.ts` are const assertions on literal tuples, which is the opposite of a cast |
| `npm run build` and `npm run lint` clean | ✅ both exit 0. The build is 235 kB (73 kB gzipped) — React, `@msgpack/msgpack` and this session's code. `oxlint` reports **nothing**, which took splitting three files: a module that exports a component and a function together is a fast-refresh warning, and `statusText`, the hooks and the context object each moved to a file of their own |
| `cargo test --workspace` | ✅ exit 0 — **1 432 tests across 50 targets**, 15 ignored (14 as before, plus the recording's regenerator). **13 of them are this session's**: 5 in the new `prism-ipc/tests/interface_protocol.rs`, 5 in the new `prismd/tests/ui_recording.rs` (one of them the ignored regenerator), and 3 in `prism-domain`'s export module |
| `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` | ✅ both exit 0 |
| **Coverage on what this session wrote** | ✅ **98.60 % lines**, 96.06 % branches, **100 % functions** over `ui/src` (158 tests in 15 files, `vitest` + Testing Library). At **100 % lines**: `log/logger.ts`, `ipc/protocol.ts`, `ipc/endpoint.ts`, `ipc/telemetry.ts`, `ipc/codec.ts`, `mirror/mirror.ts`, `mirror/select.ts`, `store/hooks.ts`, `store/context.tsx`, `status.ts` and `desk.ts`. Then `connection.ts` 99.35 %, `mirror/patch.ts` 99.20 %, `App.tsx` 97.05 %, `store/desk.ts` 95.52 %, `ipc/shape.ts` 95.00 %. The nine uncovered lines were read rather than counted, and each is a *cannot happen* arm: the `SharedArrayBuffer` branch of `overArrayBuffer` (nothing in this interface makes one), a re-throw for a fault that is not a `MirrorFault`, a retry scheduled on a stopped connection, the store being set to the state it already holds, and a `return null` in a panel that only renders when the documents exist |
| Coverage on `prism-domain`, which grew the variant generator | ✅ **99.52 % lines**, 97.46 % regions, 98.31 % functions (99.77 % at S1; `export.rs` is now 97.07 % lines / 91.71 % regions). The seven uncovered lines are two `?` arms on file I/O, one acronym branch in the name converter that no type name reaches, and a `panic!` formatting inside a test that passes |
| A test never touches a device | ✅ the unit suite has no socket at all — `FakeNetwork` and `ManualTimer` are the seam, so a five-second backoff is asserted rather than waited for — and the end-to-end suite starts `prismd --mock-output`, which is the daemon's headless mode |
| CI green on the pushed commit | ✅ run **31737443279** on `c0ce1e0` — **all five jobs on the first attempt**: Windows full build and test 6 m 35 s, Linux neutral 2 m 31 s, ARM64 cross-check 54 s, UI typecheck/lint/test/build 1 m 16 s, and the new **UI end-to-end against a daemon 2 m 20 s**. The last is the interesting one: a Linux runner compiled `prismd`, downloaded Chromium, served the production build, and killed and restarted a real daemon under a real browser — so the reconnect criterion is asserted on a machine that has never run this interface by hand, and over a WebSocket rather than a named pipe |

**What was built, in five layers.**

**1. `ipc/` — the client.** `shape.ts` turns `unknown` into types: readers with a
path in every error, and an explicit 128-level depth limit for the reason
`prism-ipc`'s `scan.rs` has one. `protocol.ts` is `docs/IPC_PROTOCOL.md` §4
transcribed and checked from Rust. `codec.ts` is MessagePack and the 1 MiB
limit. `connection.ts` is the handshake, the backoff — 100 ms doubling to 5 s,
the same shape as the output drivers' — and the state machine that tells a
version mismatch from a lost socket. `telemetry.ts` is a sink with **no way to
notify anybody**.

**2. `mirror/` — RFC 6902 in the browser.** `patch.ts` is
`prism_core::JsonMirror` rule for rule — `-` at the end of an array, `01` not
being an index, a `move` into its own source, `replace` needing something to
replace — with the cases in its test file taken from `mirror.rs`'s own tests. It
is **immutable by path copying**: a patch that changes one fixture leaves every
other fixture object identical by reference, which is what lets a selector
decide it has nothing to redraw, and there is a test that asserts exactly that.

**3. `store/` — the read model.** A `DeskStore` of about a hundred lines and
`useSyncExternalStore`. Losing the daemon **drops the three documents**, which is
the second exit criterion made checkable rather than promised.

**4. `App.tsx` — enough interface to test the foundation through.** The
connection state said plainly, the three documents read out by pointer, nothing
at all when there is no daemon, and a command line — which is the smallest
honest illustration of **D3**: what is typed is *local input*, what is displayed
underneath is the **daemon's** command line, and the second only ever moves
because a `SessionPatch` said so.

**5. The test infrastructure `ARCHITECTURE_SPEC.md` §12 asks for.** `vitest` plus
Testing Library for units, Playwright against a daemon in mock-output mode for
end to end, and a CI job of its own for the second, because it compiles a daemon
and downloads a browser.

**Two dependencies were considered and one was taken.** The plan named Zustand
and Immer; neither is here, and `@msgpack/msgpack` is. The reasoning is in the
decision log — briefly: the documents are patched by RFC 6902 operations against
a document root, so the applier already returns a new document with everything
untouched shared by reference, and a draft proxy in front of that would be a
second immutability mechanism over data that is already immutable; the store is
forty lines of `useSyncExternalStore`; and a hand-written MessagePack decoder
would be a second implementation of a binary format, in the one language where a
decoding mistake is silent.

**What S24 inherits and what it must not undo.** The telemetry channel is
already received — `TelemetrySink.accept` — and it is asserted that a hundred
frames through it cost the store **zero notifications** and leave the state
object identical. S24 decodes the fixed-layout frame and renders it on a canvas;
the sink is where that starts, and `deskEvents` deliberately has no
`onTelemetry`, so the store cannot be wired to it by accident.

### 2.25 S24 verification record

Measured on 2026-08-14, all exit criteria from `IMPLEMENTATION_PLAN.md` S24 and
the session prompt. The session that makes the second channel worth having, and
the first one whose central claim is a number that has to be *counted* rather
than argued.

| Check | Result |
|---|---|
| **64 universes at 30 Hz sustained with zero React re-renders — asserted with a render counter** | ✅ `ui/src/telemetry/render.test.tsx`: a `<Profiler>` round the **whole interface**, a real `Connection` over the fake socket, and **300 frames of the recorded 64-universe frame** — ten seconds at §7's rate, 9.8 MB of levels through the MessagePack envelope. The commit count after the three-hundredth frame is **the same number it was after the handshake**. Not "the panel did not re-render": *nothing did*. And it was not zero work — the surface recorded 300 blits, one per frame, and the chrome was drawn **once** |
| The counter counts | ✅ mutation: a `useState` setter called from the readout callback — the smallest realistic way to get telemetry into reactive state — turns **two** of the three tests in that file red. Three more mutations are recorded below |
| **Frame budget: the canvas render stays under 8 ms at 64 universes** | ✅ **0.30 ms median, 1.10 ms p99**, over 163 frames — and 0.20 ms / 1.00 ms on the run before it. Measured in **Chromium against a real `prismd`** publishing **64 real universes** at 30.2 Hz over a WebSocket (`ui/e2e/telemetry.spec.ts`), with `performance.now()` round the whole per-frame cost: reading the 32 912-byte frame *and* drawing it. The rig is `ui/tests/fixtures/wide-rig.prism`, a show patched across all 64 universes — the daemon filters telemetry down to the universes a show actually patches, so a wide *layout* over a narrow show would have measured nothing. The assertion is on the p99, not the mean: the worst frame in four seconds is what an 8 ms budget is about |
| The picture is a picture, not a readout that agrees with itself | ✅ the same spec reads the canvas bitmap back with `getImageData` and counts lit pixels. A readout can be right about a blank canvas |
| **Dropped telemetry degrades smoothly and never desynchronises control state** | ✅ three ways. **Structurally:** the decode happens on the *paint* side of the sink, in a loop whose only output is a canvas, so a malformed frame cannot reach the store, the connection or the mirror. **Asserted:** 180 malformed frames — every case `prism-ipc` itself refuses — through a real connection leave the commit count unchanged, the status *Connected*, the notices empty and the documents intact; the delta sent afterwards arrives and moves the readout. **Visibly:** the last readable picture stays on the canvas, the fault is counted by kind, and the log says so **once** per kind rather than thirty times a second |
| A frame is a picture of now | ✅ the sink keeps the latest payload and no queue, and the loop draws a payload once: three frames arriving between two animation frames cost **one** paint, and the two that were skipped are counted as *lost* from their sequence numbers. A picture that has stopped arriving is **dimmed** after 600 ms and says *not live*; a picture from a daemon that has gone is taken off the canvas altogether, because the sink was cleared with the connection |
| **The decoder is checked against `prism_ipc::TelemetryFrame::encode`, not against itself** | ✅ `ui/tests/fixtures/telemetry-recording.json`, written by the new `crates/prismd/tests/ui_telemetry.rs` off **two running daemons**: four frames from the three-dimmer rig with all 512 levels of each universe written out, one frame from the 64-universe rig described by an FNV-1a digest per universe, and six malformed frames. **Every expectation in the file is `TelemetryFrame::decode`'s own answer.** There is no encoder in `ui/src/telemetry` and there is not meant to be: nothing in a client ever sends telemetry, so an encoder could only exist to feed the decoder its own idea of the format |
| The recording cannot go stale quietly, and is not vacuous | ✅ two guards in Rust, on every `cargo test`: every payload is decoded with this build's layout, compared against the recorded expectation **and re-encoded back to the same bytes**; and a second test says the frames are of a rig that is lit — 64 universes in order, none of them dark, three distinct levels across the narrow frames, a sequence number that moves, and levels past channel 256, which is where a stride error hides. The regenerator is `#[ignore]`d, like `prism-core`'s frozen migration fixture |
| **The layout constants are checked from Rust** | ✅ `crates/prism-ipc/tests/interface_telemetry.rs` `include_str!`s `ui/src/telemetry/frame.ts` and asserts `TELEMETRY_VERSION`, `TELEMETRY_HEADER_BYTES`, `CHANNELS_PER_UNIVERSE`, the section stride as *the number plus its levels*, the magic, `UniverseId::MAX`, all three fault names and the fact that the decoder answers with a fault rather than a frame. The same shape as S23's `interface_protocol.rs` |
| A layout version this build does not know is **dropped**, not guessed at | ✅ from a recorded frame with its version byte raised: the fault is `unknown-version`, naming both versions, and the view keeps the last frame it could read. Asserted against the fault `prism_ipc` gave for those exact bytes |
| `npx tsc -b --force` clean with `strict: true`, **no `any` anywhere** | ✅ exit 0. `any` appears **nowhere** in `ui/src` or `ui/e2e`. This session's shipped code contains **no type assertion at all**; the one `as` it adds is in `canvas.test.ts`, on an object that file constructed three lines above — the allowance S23's four test assertions carry. `tsconfig.node.json` gained the `DOM` library for one reason, written down in the file: the bodies of `page.evaluate` run in the browser |
| `npm run build`, `npm run lint`, `npm run test` clean | ✅ all three exit 0. The build is 244 kB (77 kB gzipped), up 9 kB on S23 — this session adds no dependency, and a canvas needs no library. `oxlint` reports nothing. **235 tests in 22 files**, up from 158 in 15 |
| `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ all three exit 0 — **1 439 tests across 52 targets**, 16 ignored. Seven of them are this session's: 3 in the new `prism-ipc/tests/interface_telemetry.rs` and 4 in the new `prismd/tests/ui_telemetry.rs` (one of them the ignored regenerator) |
| **Coverage on what this session wrote** | ✅ **98.91 % lines**, 94.08 % branches, **100 % functions** over `ui/src` (235 tests, `vitest` + Testing Library), up from S23's 98.60 %. The new module reads **99.45 % lines, 100 % functions**: `driver.ts` and `context.ts` at **100 % on every column**, `frame.ts`, `painter.ts` and `stats.ts` at **100 % lines**, `panel.tsx` 95.45 %. The two uncovered lines in `panel.tsx` were read rather than counted: a canvas ref that is `null` at the moment the effect runs, and the `typeof window === "undefined"` arm of the resize fallback — neither reachable from inside a browser or a jsdom test |
| A test never touches a device | ✅ the unit suite has no canvas either: `LevelSurface` is the seam, and `RecordingSurface` keeps the pixels, so the raster is asserted *per channel* rather than looked at. The end-to-end suite starts `prismd --mock-output`, the daemon's headless mode |
| CI green on the pushed commit | ✅ run **31752194635** on `c5dd711` — **all five jobs on the first attempt**: Windows full build and test 6 m 52 s, Linux neutral 2 m 15 s, ARM64 cross-check 40 s, UI typecheck/lint/test/build 1 m 13 s, UI end-to-end against a daemon 1 m 52 s. The end-to-end job took the frame-budget measurement again on a machine that has never run this interface by hand, on Linux, with a software rasteriser and a debug daemon: **`64 universes · 30.3 Hz · paint 0.20 ms (p99 0.40 ms) · 164 frames`**, nothing lost and nothing dropped. A number measured on one developer's machine is a claim; the same number on a build server is a measurement |

**What was built, in four layers.**

**1. `telemetry/frame.ts` — the layout, read in place.** A view rather than a
value: it keeps the payload and an index of where each universe's levels begin,
and reads the levels where they lie. Nothing is copied, nothing is parsed into
objects, and the only allocation per frame is the `DataView` over the header. The
sequence is a `bigint`, because the field is a `u64` and a `number` is exact only
to 2^53 — the range is not needed, but a decoder that quietly narrowed a wire
field is the kind of thing that is right until it is not.

**2. `telemetry/painter.ts` — one pixel per channel.** The levels become an RGBA
block 512 wide and one row per universe — 32 768 writes through a 256-entry
palette — and that block is blitted into the grid, scaled, with smoothing off.
The alternative is 32 768 `fillRect` calls for the same picture. The palette's
byte order is worked out at run time rather than assumed. Beside the grid: a peak
meter per universe, universe numbers in the gutter and a channel ruler, all drawn
**only when they change**.

**3. `telemetry/driver.ts` — the loop, and where the rule is kept.** The sink
holds the latest payload; an animation frame decodes *that one payload* and draws
it. No state is set, no context is published, no component is told anything. One
line of text reaches the DOM, written with `textContent` by hand — which is also
what the end-to-end suite reads the frame budget out of.

**4. `telemetry/panel.tsx` — a canvas, a line, and no state.** The channel comes
down the tree as a **device** — a sink, and the defaults for how to schedule and
where to draw — rather than as state, so there is nothing in the context to
subscribe to. The canvas's size is client-local (`ARCHITECTURE_SPEC.md` §4.2)
and lives in the element and a ref; no command is sent about it.

**Four mutation checks.** A `useState` setter in the readout callback turns two
render tests red — that is what says the counter counts. Shifting the raster by
one channel turns the two pixel tests red. Redrawing on every animation frame
rather than on a new payload turns the coalescing test, the staleness test *and*
the render counter red. Counting a sequence gap as the whole difference rather
than what was missed turns two stats tests red.

**Two fixtures were added and both are written by Rust.**
`ui/tests/fixtures/telemetry-recording.json` is the frames and their meanings;
`ui/tests/fixtures/wide-rig.prism` is a show patched across all 64 universes, and
it is the only honest way to have 64 of them in a browser. Both are regenerated
by `cargo test -p prismd --test ui_telemetry -- --ignored`, and both are opened
by non-ignored tests on every commit — a show format that moved would fail in
Rust rather than in Chromium months later.

### 2.26 S25 verification record

Measured on 2026-08-14, all exit criteria from `IMPLEMENTATION_PLAN.md` S25 and
the session prompt. The session where the interface stops being a readout and
starts being a desk — and the first one whose central claim can only be checked
by *watching* rather than by asserting.

| Check | Result |
|---|---|
| **Opening, moving, resizing and closing a window issues a session command — the interface does not hold this state** | ✅ four gestures, four commands, and the assertion is on what happens *before* the delta: `ui/src/canvas/session.test.tsx` drives the whole interface over a socket and, after each gesture, reads the bytes that went out **and** checks the canvas has not moved. Opening sends `OpenWindow` and the canvas stays empty; closing sends `CloseWindow` and the window stays; dragging sends `PlaceWindow` and the window is back at the daemon's rectangle the moment the button comes up. There is nowhere for the answer to be kept: `Canvas` renders `openWindows(session)` and holds no list, no positions and no stacking order |
| **The stacking order is the daemon's too** | ✅ the trap in the sentence above. `openWindows` deliberately does **not** sort — `prism-core` moves a focused window to the end of the list, so array order *is* stacking order, and the stylesheet has no `z-index` anywhere so document order is the only thing that can express it. `canvas.test.tsx` asserts the drawn order is `[2, 1]` after `FocusWindow(1)` and that this is **not** the sorted order. **Checked by mutation:** sorting the readers' answer by number turns five tests red across three files |
| **A view switched at the X-Touch appears at once — D11, observed end to end** | ✅ `ui/e2e/session.spec.ts`, in Chromium against a real `prismd`: a layout is stored as view 2 and the canvas is then emptied, and **three MIDI bytes are appended to a file by the test process** — `90 31 7F`, `Channel ▶` on channel 1, written out from `docs/MCU_MAPPING.md` §2.1 by hand. The browser is doing nothing at that moment. The view lights up, `activeViewId` reads 2 and the window comes back; then F1 (note 54) opens a Fixture Sheet, and it appears. Nothing was polled and nothing was asked for. The daemon reads the file through `--mock-surface`, which is the console's `--mock-output`: the same three layers of `prism-surface`, the same binding table, no device |
| The same claim in Rust, on the wire | ✅ `crates/prismd/tests/surface_gate.rs` gained `a_console_press_reaches_a_connected_client_as_a_delta`: a client connects, its snapshot says view 1, a press is appended to the mock surface file, and the client is **sent** `replace /session/activeViewId → 2`. The existing gate is the other half — the same press with *nobody* connected. Between them, D11 holds whether or not an interface is running |
| **The layout survives a restart of the interface, because it lives in the session** | ✅ twice. In Chromium: two windows are opened and one is dragged, `page.reload()` destroys every scrap of state in the tab, and the windows come back **at the same percentages** — with the daemon neither restarted nor saved nor told anything at all. In jsdom: a *second* interface, which has never sent a command or seen a delta, is handed the snapshot and draws the same canvas — which is the stronger form, because it holds for a client that took no part in building the layout |
| **Where the layout lives, and how a drag can still be smooth** | ✅ the question the prompt calls the decisive one, and the answer is **cadence, not ownership** — written down in `ui/src/canvas/drag.ts` and asserted in `drag.test.ts` and `canvas.test.tsx`. Ownership never moves: the canvas renders `openWindows` and nothing else. What is local is *how often the daemon is told* (at most one `PlaceWindow` every 33 ms, plus one when the button comes up) and *what the screen shows in between* (the rectangle the pointer describes — `ARCHITECTURE_SPEC.md` §4.2's own "drag state"). **The moment the button comes up the local rectangle is dropped**, so a drag against a daemon that never answers leaves the window exactly where it started. That is the test that would fail for an implementation that held the position and "synced up" afterwards, and it is `sends where the pointer went and holds nothing when it gets there`. **Checked by mutation:** keeping the drag rectangle after pointer-up turns four tests red |
| **The protocol had no command for dragging a window. It has one now** | ✅ `Command::PlaceWindow { instanceId, x, y, w, h }` in `prism-domain`, routed in `prism_core::SessionState::apply` to the `place_window` that S12 had already written and left a note on. §4.4 lists eleven commands because it lists what the **console** issues, and a console never drags a window; §4.1 puts the geometry in the session all the same. The alternative was a position the interface kept to itself, which fails all three exit criteria. `docs/IPC_PROTOCOL.md` §5 and `ARCHITECTURE_SPEC.md` §4.4 both say so now, and the decision log has the reasoning. **Checked by mutation:** removing it from `is_session_command` turns `prism-core` and `prism-domain` red |
| **The level view had no window type. It has one now** | ✅ `WindowType::DmxSheet` — the DMX output itself, channel by channel, which is what S24 built and what none of `ARCHITECTURE_SPEC.md` §6's ten window types named. A `FixtureSheet` shows what the *fixtures* are set to; this shows what is on the *cable*, and those two can disagree. The telemetry panel now lives in one, sized by the window rather than by `vh` — which is what §7's *carried out of S24* asked for |
| **The four coordinates cannot be NaN, in either direction** | ✅ the same `crate::finite` guard `WindowInstance` carries. MessagePack encodes NaN and infinity faithfully, so without it a corrupt or hostile frame could put one into a session — where it compares unequal to itself and stops the show file saving. Asserted on both wire formats in `prism-domain` |
| **The interface's encoder is what the daemon reads — still asserted on bytes, and a difference was found** | ✅ and this one is worth reading. `rmp-serde` writes an `f64` as a float64 whatever its value; `@msgpack/msgpack` writes the JavaScript number 240 as a **uint8**, because JavaScript has one kind of number. Both are valid MessagePack for the same value. So `crates/prismd/tests/ui_session.rs` records **both** forms of every `PlaceWindow` — its own, and the integer form built by hand — and asserts in Rust, against `prism_ipc::decode`, that they are the same command. The browser then compares its bytes against the integer form. Both sides still come from Rust; what changed is that the equivalence is now a checked claim about the daemon rather than a hope |
| **The canvas is held to the daemon's answers, not to a second opinion in TypeScript** | ✅ `ui/tests/fixtures/session-recording.json`, written by the new `crates/prismd/tests/ui_session.rs` off a running `prismd`: **twelve commands of a canvas being used** — two windows opened, one dragged, one resized, one brought to the front, a view stored, a window closed, two commands refused, a view selected back — and after each one, **which windows are open, in what order, at what coordinates, which has the focus and what views exist**, taken from a *fresh client's* snapshot. `ui/src/canvas/windows.test.ts` applies the recorded deltas through the interface's own mirror and compares its readers' answers with those. Nothing in TypeScript decides what the answer should be |
| The recording cannot go stale quietly, and is not vacuous | ✅ three guards in Rust, on every `cargo test`. `the_recorded_windows_are_what_the_session_document_says` replays the deltas through `prism_core::SessionMirror` and checks every step's recorded answers against the mirrored document, then checks the whole thing against the final snapshot. `the_integer_form_of_a_command_is_the_same_command` is above. `the_recording_is_of_a_canvas_being_used` demands the file actually contain the interesting cases: a window at the daemon's default, one that moved, one that was resized, a **reordering with the same set of windows** (which a canvas that sorted would get wrong), exactly two refusals that changed nothing, a view stored, and at least three kinds of window. The regenerator is `#[ignore]`d, like the other two |
| **A refusal moves nothing** | ✅ two of the twelve steps are refused on purpose — dragging a window that has just been closed, and selecting a view that was never stored. The recording carries `refused: true`, **no deltas at all** (not an empty one), and identical answers to the step before; the browser asserts the same |
| **No scrolling outside the canvas** | ✅ `CLAUDE.md`'s rule, checked in a browser rather than reviewed in a stylesheet: with four windows open, `e2e/session.spec.ts` reads `scrollWidth − clientWidth` and `scrollHeight − clientHeight` off **both** the document element and the canvas, and all four are 0. `index.css` — the Vite template until this session — is now `html, body, #root { height: 100% }` with `overflow: hidden`, so a layout mistake is clipped and gets fixed rather than quietly adding a scrollbar to a console. The one place that scrolls is inside a window, which is what a window is for |
| A window this build cannot draw is left out, not drawn empty | ✅ `type` is narrowed against the generated `WINDOW_TYPE_VARIANTS`, so a daemon newer than the interface produces a canvas with the windows it understands on it — asserted against a session holding a `Hologram`, a window with no rectangle, a window with no number, and a string where a window should be. The four windows that are genuinely not built yet (`Viewer3D`, `PhaserEditor`, `ClockViewer`, `Settings`) say so **by name**, because an operator who pressed an F-key should find an answer rather than an empty rectangle |
| `npx tsc -b --force` clean with `strict: true`, **no `any` anywhere** | ✅ exit 0. `any` appears **nowhere** in `ui/src` or `ui/e2e`. This session's shipped code contains **no type assertion at all**; the four in its test files are the allowance S23 set — a parsed recording and a decoded message the file itself constructed. Base64 is `atob`, not `Buffer`, so the test files stay under the browser configuration |
| `npm run build`, `npm run lint`, `npm run test` clean | ✅ all three exit 0. The build is **252 kB (79 kB gzipped)**, up 8 kB on S24 — **and this session adds no dependency**. A window system is the classic place a drag-and-drop library arrives; the whole of this one is three pure functions, a class with a clock argument, and two pointer listeners, and none of the libraries on offer would have made the *cadence* decision for us. `oxlint` reports nothing. **298 tests in 29 files**, up from 235 in 22 |
| `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ all three exit 0 — **1 449 tests across 53 targets**, 17 ignored. Nine of them are this session's: 3 in the new `prismd/tests/ui_session.rs` (plus its ignored regenerator), 1 in `surface_gate.rs`, 3 in `prismd`'s library (the file surface and the new flag) and 2 in `prism-domain` |
| **Coverage on what this session wrote** | ✅ **98.82 % lines**, 94.06 % branches, **100 % functions**, 98.86 % statements over `ui/src` (298 tests, `vitest` + Testing Library), against S24's 98.91 %. The new `canvas/` module reads **99.39 % lines, 95.49 % branches, 100 % functions**: `geometry.ts`, `drag.ts`, `windows.ts`, `window.tsx`, `content.tsx` and `viewbar.tsx` at **100 % lines**, `canvas.tsx` 91.66 %. The uncovered lines across the whole tree were read rather than counted, and every one is an arm this interface cannot reach: the canvas element being `null` when a drag asks for its box, three `documents === null` guards under a connection that is already `connected`, and the `?? "—"` fallbacks for a patched fixture with no name |
| Coverage on `prismd`, which grew a port and a flag | ✅ **94.98 % lines**, 94.97 % regions, 96.37 % functions — `surface.rs` **97.30 %** (up from 96.94 with `FileSurfacePort` in it) and `cli.rs` **99.36 %**. `main.rs` is still 0 % and still the honest part of the figure; without it the crate reads 96.2 % |
| **The two telemetry numbers from S24 are still true** | ✅ both, and the window system was made to prove it rather than trusted. *Zero React commits over 300 frames of 64 universes* is unchanged — `telemetry/render.test.tsx` now renders the panel **inside a window**, because the snapshot the fake daemon serves has a `DmxSheet` open, so the count is of an interface with a canvas and a window frame in it. *Under 8 ms* was re-measured in Chromium against a real `prismd` on the 64-universe rig, **with the window opened through the interface**: **`64 universes · 30.2 Hz · paint 0.20 ms (p99 0.50 ms) · 155 frames`**, nothing lost and nothing dropped. The measurement now goes through the window system as well as the telemetry channel — an `OpenWindow` out and a `SessionPatch` back before the first frame is drawn |
| A test never touches a device | ✅ the console is a **file**. `--mock-surface` is the console's `--mock-output`: MIDI bytes appended to a path, read by the same three layers, with the feedback dropped because a file has no motor faders. The output is `--mock-output`. Nothing is plugged into anything, on either the developer's machine or the build server |
| The end-to-end suite | ✅ **6 tests, all green** (Playwright + Chromium, against real daemons): S23's two, S24's one, and this session's three |
| CI green on the pushed commit | ✅ run **31798437277** on `e698b57` — **all five jobs on the first attempt**: Windows full build and test 9 m 37 s, Linux neutral 2 m 26 s, ARM64 cross-check 51 s, UI typecheck/lint/test/build 1 m 13 s, UI end-to-end against a daemon 1 m 45 s. The last is the one worth reading: **all six end-to-end tests passed on a Linux runner**, which means D11 was observed on a machine that has never run this interface by hand — a `prismd` compiled there, a Chromium downloaded there, and three MIDI bytes appended to a file. The frame budget was re-measured through the window system at the same time: **`64 universes · 30.4 Hz · paint 0.20 ms (p99 1.10 ms) · 155 frames · 1 lost`** |

**What was built, in five pieces.**

**1. `canvas/geometry.ts` — the coordinate space.** A fixed 1920 × 1080 grid of
*canvas units* that each client stretches over its own element, because §4.1's
`x`, `y`, `w` and `h` are shared by clients that do not share a screen. The
number is not free: the daemon opens a window at 640 × 480, so a grid on which
that is a third of the width is the reason 1920 was chosen, and there is a test
that says so. Windows are positioned in **percentages**, so a screen that
changes size moves every window without a single command — a screen's size is
client-local (§4.2).

**2. `canvas/drag.ts` — the cadence.** Sixty lines, a clock passed in as an
argument, and the rule in the module documentation. See the row above.

**3. `canvas/windows.ts` — readers, not a model.** S23's rule taken literally:
`openWindows`, `focusedWindow`, `activeViewId`, `storedViews` each walk the
document they are given and answer. Nothing is parsed and kept, because a
parsed copy is a second model to keep in step and RFC 6902 operations only mean
anything against a root.

**4. `canvas/canvas.tsx`, `window.tsx`, `content.tsx`, `viewbar.tsx` — the
screen.** A window frame with a title bar, a close button and a corner; a View
Selector Bar whose lit button is `activeViewId` and whose buttons issue the
same `SelectView` the X-Touch's `Channel ◀▶` does (**D8**); and a body per
window type — the level view, the patch, the pools, and an honest sentence for
the four that are S27, S28 and S30's.

**5. `prismd::surface::FileSurfacePort` — a console with no console.** The
missing half of the test rig. `MockSurfacePort` lets a *Rust test* press a
button; this lets a **separate process** press one, which is what D11 needed to
be watched rather than reconstructed. It is production code with a flag on it,
in the same sense `--mock-output` is, and a person can use it to try a binding
table with nothing plugged in.

**Four mutation checks.** Keeping the drag rectangle after the button comes up
— the optimistic implementation — turns four tests red. Sorting `openWindows`
by number turns five red across three files. Sending on every pointer event
rather than on the interval turns three red. Removing `PlaceWindow` from
`is_session_command` turns `prism-core` and `prism-domain` red.

**One fixture was added and it is written by Rust.**
`ui/tests/fixtures/session-recording.json`, by
`cargo test -p prismd --test ui_session -- --ignored`, and opened by three
non-ignored Rust tests on every commit. A session shape that moved would fail
in Rust rather than in Chromium months later.


### 2.27 S26 verification record

Measured on 2026-08-14, all exit criteria from `IMPLEMENTATION_PLAN.md` S26 and
the session prompt. The session in which the interface stops being a picture of
the desk and starts being one: the first gesture in a browser that puts a level
on a rig.

| Check | Result |
|---|---|
| **The executor bar shows exactly the current page** | ✅ **eight strips, always**, numbered `page * 8 + slot` — D7's arithmetic, in `desk/session.ts` and nowhere else. A page with nothing assigned is eight *empty* strips rather than a bar that has vanished, because that is what an operator paging past their executors sees on a console; the recorded script walks pages 0, 1 and 2 for exactly that reason, and page 2 has nothing on it. Held to the daemon: `ui/src/desk/session.test.ts` replays the recorded deltas through this interface's mirror and compares its readers' answers — the eight strips, field by field — against what a **fresh client's snapshot** said after each of twenty-three commands |
| **Paging from the X-Touch and from the interface agree** — observed, not asserted | ✅ `ui/e2e/desk.spec.ts`, in Chromium against a real `prismd` with `--mock-surface`: three MIDI bytes for `Faderbank ▶` (note 47, written out from `docs/MCU_MAPPING.md` §2.1 by hand) are appended to a file **by neither the browser nor the daemon**, and the bar moves from page 0 to page 1 — strip 0 becomes executor 8, the names change, and the one assigned slot on that page is executor 9. Then the *browser* pages to 2, and the console's next two presses of `Faderbank ◀` bring it back through 1 to 0. **One page number, two hands on it**, which is the criterion rather than "both send `SetExecutorPage`" |
| **An encoder change reaches the programmer and appears in the output** | ✅ **measured, in a browser, against a running daemon: `programmer 69 ms · output 113 ms`** (`ui/e2e/desk.spec.ts`, from the Enter key). Three things in order: the command goes out, `ProgrammerChanged` comes back and the readout reads three values on three fixtures, and the level is **in the picture** — counted off the telemetry canvas as the exact colour `LevelPainter` draws a channel at 127 in, which is what the engine encodes 32 767 to. Not a readout that agrees with itself: `getImageData`, and the count is zero before and greater than zero after. Clearing the programmer takes it off the canvas again |
| The rig is dark, so a level in the picture is one the test put there | ✅ `ui/tests/fixtures/desk-rig.prism` is written by `crates/prismd/tests/ui_programmer.rs` with **every** attribute's home value at 0, and a Rust test that runs on every commit opens the committed file and asserts it. `common::show_file`'s first dimmer sits at full, which would have made this measurement meaningless |
| **The command line parses and reports syntax errors without throwing** | ✅ and the exit criterion is checked over **ten thousand generated lines** rather than the six a person would think of (`console.test.ts`): a seeded xorshift over an alphabet of keywords, digits, separators, `999999999999999999999`, `1e400`, `0x10`, an emoji and an empty word, and every answer is one of `empty`, `commands` or `error` with a sentence that can be shown to somebody. A line that will not parse is a **message under the input**, shown as it is typed, and Enter sends nothing |
| **The parser's answers come from the daemon** | ✅ every typed line in `ui/tests/fixtures/desk-recording.json` sits beside the commands a real `prismd` accepted for it, and `console.test.ts` decodes those payloads and compares. Fourteen lines, grouped by a recorded line **number** rather than by equal text — `clear` is pressed three times in a row and those are three lines, not one line with three commands. One of them, `1 thru 3 at 50`, produces **two** commands, which is the case a parser answering with a single command could never have passed |
| A parser must not consult the show, and this one does not | ✅ `9` on a rig with no fixture 9 parses perfectly and the **daemon** refuses it — a step of its own in the recording, with `refused: true`, no deltas at all, and answers identical to the step before. That split is D3: the client decides what was asked for, the daemon decides what is |
| **The encoder bar and the jog wheel walk one list** | ✅ the finding S22 left open, closed by having one table rather than by keeping two in step. `FeatureGroup::attributes()` is new in `prism-domain` and asserted to be exactly the filter over `AttributeType::ALL`; `prismd::surface::parameter_of` is now `group.parameter(index)`; and `export_bindings` emits the same table into `ui/src/bindings/variants.ts` as `FEATURE_GROUP_ATTRIBUTES`, with the spellings still read back out of the unions `ts-rs` wrote. **Then it is watched:** the console presses Encoder Assign / Pan (note 42) and the Colour bank lights in the browser; it presses `Zoom ▶` (note 99) and the highlight moves from Red to Green; the jog wheel turns ten detents (`B0 3C 01`) and **Green moves while Red does not** |
| **Nothing on these bars is state the interface holds** | ✅ asserted on what happens *before* the delta, gesture by gesture, in `ui/src/desk/desk.test.tsx` — which drives the whole `<App />` over a socket. Paging sends `SetExecutorPage` and the page number does not move. Selecting sends `SelectExecutor` and no strip lights. Go, Off, the bank buttons, the parameter arrows and Clear are the same shape. There is nowhere for the answer to be kept: every bar renders readers over the two documents and holds nothing |
| **A fader is `canvas/drag.ts` one layer down, and the local value is dropped** | ✅ `desk/valuedrag.ts`: the daemon owns the level, the screen may show the pointer's while the button is down, a command goes out at most every 33 ms plus one on release, and **the local value is dropped the instant the button comes up** — so a fader pulled against a daemon that never answers springs back. Asserted through the component: after a drag with no delta, the fader reads the session's level again. An **encoder** is the easier half of the same contract — a turn is relative, so there is no local value at all, and each command carries *what has not been sent yet* rather than the total |
| **The four button functions the protocol cannot press are drawn and say so** | ✅ S22's gap, met from the interface. A strip draws every button its executor assigns; `Go+`, `Go-` and `Off` are pressed, and `On`, `Flash`, `Toggle` and `LearnSpeed` are **disabled with the reason on the button**. The recorded rig puts all four of them on one strip on purpose, so the test meets them. **Checked by mutation:** resolving `Toggle` to a Go — the tempting reading of `isActive`, and a client deciding what a show's own setting means — turns that test red |
| **The cue number is a dash, and the absence is asserted** | ✅ `Executor::currentCueIndex` is in the domain and on the wire and **nothing ever fills it**: `prismd::core::record_executor` reads it back out of the show, and what cue a playback is on lives on the tick thread with no channel back. So the bar shows a dash rather than a number it invented, and `the_recording_is_of_a_desk_being_used` demands that every recorded strip's is `null` — with a message telling whoever builds that channel to regenerate the recording and give the bar a cue number. A test that goes red when a gap is *closed* is the only kind of note that cannot be forgotten |
| **The readers are held to the daemon's answers, not to a second opinion** | ✅ `crates/prismd/tests/ui_programmer.rs` is the fourth frozen recording: twenty-three commands of a **desk being operated**, and after each one the eight strips, the programmer (selection, values, clear stage, and the banks `prism_core::Programmer::feature_groups` says are touched) and the six session fields the bars read — all taken from a fresh client's snapshot. `the_recorded_answers_are_what_the_documents_say` replays the deltas through `prism_core`'s own `ShowMirror`, `SessionMirror` and the programmer on every commit, then checks the lot against the final snapshot. The browser compares its readers with the same file |
| The recording is not vacuous | ✅ `the_recording_is_of_a_desk_being_used` demands the interesting cases be *in* it: three different pages including an empty one, `page * 8 + slot` on every strip of every step, three distinct master levels, an executor that starts and stops, all four unpressable button functions **and** the three pressable ones, values on more than one bank, all three Clear stages, an empty programmer and a non-empty one, the encoder bank moving, the parameter index moving, an executor selected, a command line written to, and **three refusals that change nothing at all**. `the_typed_lines_are_the_console_being_used` demands the line numbers be consecutive, unreused, and cover the six shapes the grammar has |
| **The two telemetry numbers from S24 are still true** | ✅ both, with two more bands on the screen. *Zero React commits over 300 frames of 64 universes* is unchanged. *Under 8 ms* was re-measured in Chromium against a real `prismd` on the 64-universe rig with the executor and encoder bars rendering above it: **`64 universes · 30.4 Hz · paint 0.20 ms (p99 0.40 ms) · 156 frames`**, nothing lost and nothing dropped |
| **The three S25 criteria are still true** | ✅ all eleven end-to-end tests pass, S25's three among them: windows open, move and close by command and survive a `page.reload()`; **D11 observed** — `Channel ▶` and F1 from a file, a browser that was doing nothing; and no scrolling outside the canvas. The last was re-checked *with both new bars on the screen* by this session's own spec, which reads `scrollWidth − clientWidth` and `scrollHeight − clientHeight` off the document element, the canvas **and both bars** with four windows open, and demands eight zeros |
| `npx tsc -b --force` clean with `strict: true`, **no `any` anywhere** | ✅ exit 0. `any` appears **nowhere** in `ui/src` or `ui/e2e`. Every narrowing in this session's shipped code is a **type predicate** against a generated table — `isFeatureGroup`, `isFaderFunction`, `isButtonFunction` — and the only `as` in it is the one S25's `canvas/windows.ts` already carries: `(TABLE as readonly string[]).includes(value)`, which **widens** the table so a string can be looked up in it and claims nothing about the string. The narrowing is the predicate's return type, which the compiler checks. The three assertions in the test files are the allowance S23 set: a parsed recording and a decoded message the file itself constructed |
| `npm run build`, `npm run lint`, `npm run test` clean | ✅ all three exit 0. The build is **269 kB (84 kB gzipped)**, up 17 kB on S25 — **and this session adds no dependency either.** A command-line parser is the classic place one arrives; this one is a tokeniser of four lines and about two hundred of grammar, and a parser-combinator library would have made the *messages* worse, which are the part an operator reads. `oxlint` reports nothing. **371 tests in 34 files**, up from 298 in 29 |
| `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ all three exit 0 — **1 441 passed and 18 ignored across 54 targets**. Nine of the passing ones are this session's: 4 in `prism-domain` (two on the new bank table, two on the generator), 1 in `prismd`'s library, and 4 in the new `prismd/tests/ui_programmer.rs` (whose regenerator is the eighteenth ignored test). *§2.26 recorded "1 449 tests across 53 targets, 17 ignored" for S25; that figure was passed **plus** ignored, so the comparable number here is 1 459, and 1 459 − 1 449 = 10 = nine new tests and one new regenerator.* Recorded rather than quietly corrected, because a number in this file that nobody can reproduce is worse than one that is wrong |
| **Coverage on what this session wrote** | ✅ **99.07 % lines**, 94.74 % branches, **100 % functions**, 99.10 % statements over `ui/src` (371 tests, `vitest` + Testing Library), up from S25's 98.82 %. The new `desk/` module reads **99.75 % lines**, 96.28 % branches, **100 % functions**: `level.ts`, `session.ts` and `valuedrag.ts` at **100 % on every column**, `console.ts`, `programmer.ts`, `encoderbar.tsx` and `executorbar.tsx` at **100 % lines**, `commandline.tsx` 97.56 %. The one uncovered line is the mirror's clear-the-timer-on-unmount arm reached with nothing pending; the uncovered *branches* were read rather than counted, and each is an unreachable arm — a `?? CLEAR_TITLES[0]` for a stage outside 0…2, an `aria-disabled` for a bar with no session, and the `position()` miss in a search over the array being searched |
| Coverage on `prism-domain` and `prismd`, which both changed | ✅ `prism-domain` **97.52 % lines**, 96.18 % regions, 95.50 % functions. The uncovered lines are three in `attribute.rs` that `llvm-cov` maps onto the `MergeMode` declaration (`proptest_derive` code attributed to the line it was generated from — the same effect §3 already records for this crate), and fourteen in `export.rs`: the string-literal lines of a multi-line `String::from`, and the `position()` miss above. `prismd` **95.06 % lines**, 95.02 % regions, 96.37 % functions — `surface.rs` **97.34 %**, and `main.rs` still 0 % and still the honest part of the figure |
| A test never touches a device | ✅ the console is a **file** and the output is `--mock-output`. The unit suite has no socket, no canvas and no clock of its own: `FakeNetwork`, `ManualTimer` and a `now` passed as an argument, so *at most one command every 33 ms* is asserted rather than waited for |
| CI green on the pushed commit | ✅ run **31807825063** on `2c09ba6` — **all five jobs on the first attempt**: Windows full build and test 8 m 28 s, Linux neutral 2 m 22 s, ARM64 cross-check 41 s, UI typecheck/lint/test/build 1 m 35 s, UI end-to-end against a daemon 1 m 48 s. The last is the one worth reading: **all eleven end-to-end tests passed on a Linux runner**, so paging from a console and the jog wheel turning what the encoder bar highlights were both observed on a machine that has never run this interface by hand — a `prismd` compiled there, a Chromium downloaded there, and MIDI bytes appended to a file by neither. This session's own measurement came out **`programmer 88 ms · output 122 ms`** there against 106 ms here, and S24's frame budget was re-measured through both new bars at the same time: **`64 universes · 30.4 Hz · paint 0.20 ms (p99 0.80 ms) · 156 frames · 1 lost`** |

**What was built, in five pieces.**

**1. `desk/session.ts` and `desk/programmer.ts` — readers, not a model.** S23's
rule taken a third time: `executorPage`, `selectedExecutor`, `encoderBank`,
`programmerPage`, `programmerParamIndex`, `commandLine`, `pageStrips` — each
walks the document it is given and answers, and nothing is parsed and kept.
Beside them the programmer's: what one encoder reads across a selection
(`mixed` rather than an average, a dash rather than 0 % — because absent is not
zero), and which banks hold values, which is the **show's** grouping and not the
attribute name's.

**2. `desk/valuedrag.ts` — the cadence, again.** `ValueDrag` is
`canvas/drag.ts` with one axis, and `EncoderDrag` is the easier half: a turn is
relative, so there is no local value to drop and each command carries what has
not been sent yet. Both take `now` as an argument.

**3. `desk/console.ts` — the parser, and the grammar in the module
documentation.** `[fixtures] [attribute] at percent`, `fixtures`, `clear`,
`go`/`go-`/`off`, `page`. `thru`, `+` and `,` are separators as well as words,
so `1THRU3,5` reads as a range and a fixture. What is deliberately **not** in it:
storing cues (S28 owns the mode question in front of `StoreCue`), groups and
presets, and anything that would have to read the show in order to decide what a
line meant.

**4. `desk/executorbar.tsx` and `desk/encoderbar.tsx` — the two bands.**
Eight strips of the current page with a fader, a select head, the show's own
buttons and a cue readout; five bank buttons, the parameters of the bank in force
with the highlighted one the jog wheel's, two arrows that step it, and the
three-stage Clear saying which stage the next press is.

**5. `desk/commandline.tsx` — three things, and D3 is the boundary between the
first two.** What has been typed (local), what the daemon's console line says
(`SessionPatch`, and it moves when the daemon says so), and what the line *would
do*, shown before Enter so `1 thru 3 at 50` is visibly two commands.

**Five mutation checks.** Reading the strips off page 0 rather than the
session's page turns two tests red across two files. Sorting a bank's parameters
alphabetically turns the bank-order test red — the one that says the wheel and
the bar walk one list. Dropping the `SelectFixtures` from a line that also sets
a level turns four red. Resolving `Toggle` to a Go turns the *drawn but not
pressable* test red. Keeping the fader's local value after the button comes up —
the optimistic implementation — turns the fader test red.

**One fixture pair was added and both are written by Rust.**
`ui/tests/fixtures/desk-recording.json` and `ui/tests/fixtures/desk-rig.prism`,
by `cargo test -p prismd --test ui_programmer -- --ignored`, and opened by four
non-ignored Rust tests on every commit. No `.gitignore` change was needed: S24's
`!ui/tests/fixtures/*.prism` already covers a second show file in that
directory, which is what a rule written for a directory rather than for a file
is for.


### 2.28 S27 verification record

Measured on 2026-08-14, all exit criteria from `IMPLEMENTATION_PLAN.md` S27 and
the session prompt. The session in which the interface stops needing a show
somebody else wrote: a rig is built in a browser, from nothing.

| Check | Result |
|---|---|
| **Fixtures can be patched, addressed and edited entirely from the interface** | ✅ and *entirely* is taken literally: `ui/e2e/patch.spec.ts` starts a real `prismd` on an **empty show** — no fixtures and **no profiles at all** — and builds a rig in Chromium. A profile out of the desk's library, two fixtures, a name, an address, a repatch that moves one, a **renumber**, and an unpatch. Every one of the five fields of `PatchFixture` is typed into the form, and the status strip's fixture count follows the daemon. Then `page.reload()`, and the rig is still there, because it was never here |
| **Address conflicts are shown before they are committed** | ✅ **before**, and in the daemon's own words. Typing 3 into the address of a fixture that sits at 10 draws *`4 channels, ending at 6. Overlaps 3–4 with fixture 1 — the higher fixture number wins.`* while the form is still open, the command unsent and the row still reading 10. Typing 510 draws the daemon's refusal and **Apply goes dead**. Typing 3 again brings it back, because an overlap is *not* a refusal: cloning a fixture onto another is a technique in daily use (`prism_core::conflict`), and the interface has to be able to say both things at once |
| **The daemon computes it, and the protocol grew the way to ask** | ✅ this is the session's finding, and it was taken rather than worked around. A command expresses intent and a delta describes a change that has happened; neither can answer *what would happen if*. So `docs/IPC_PROTOCOL.md` §5.2 is new: `ClientMessage::Query` and `ServerMessage::Answer`, with `Query::PatchConflicts` and `Query::PatchPreview`, answered by `prism_core::Show::preview_patch` — which takes its refusal from the **same** `check_patch` the edit runs, so a preview and the patch after it cannot disagree. Nothing in TypeScript intersects an address span. **Four rules, three of them asserted:** a query changes nothing; the answer goes to the client that asked (not a broadcast — what one operator is typing is nobody else's business, §4.2); and there is no `Query::Show`, because a question that returned state a client already mirrors would be a second path to the same fact |
| A question changes nothing, asserted on a recording | ✅ `crates/prismd/tests/ui_patch.rs::a_question_changes_nothing`: every one of the seven query steps carries an answer, **no deltas at all** (not an empty list — none), no refusal, and a patch and profile list identical to the step before it. That is the property that makes it safe to ask one per keystroke on a desk that is running a show |
| **A preview is what the patch that follows it does** | ✅ the claim the exit criterion rests on, asserted twice in two languages. In `prism-core`: four addresses, preview then patch, with `accepted` compared against what `patch_fixture` answered and `conflicts` against what `Show::conflicts` reports afterwards. In `prismd`'s recording: step 1 previews a PAR at 30 (*free, ends at 33*) and step 2 patches it — ending at 33; step 3 previews one at 32 (*overlaps 32–33 with fixture 6*) and step 4 patches it anyway — and step 5's `PatchConflicts` reports **exactly that pair** |
| **A brand-new show could not be patched at all, and now can** | ✅ the hole this session found in its own centre. `PatchFixture` names a profile the show must already carry, `Show::embed_fixture_type` had no command in front of it, and a fresh show carries none — so before S27 the patch window of a new show was a form with an empty menu and no way out of it. `Command::EmbedFixtureType { typeId }` carries **a key and nothing else**, resolved by the daemon against the new `prism_core::library` (a dimmer, two PARs and an eleven-channel moving head with 16-bit pan and tilt). A client that sent a whole `FixtureType` would be authoring show content — the same rule that keeps channels out of `PatchFixture`. The library rides in the `Snapshot`, because a client needs it in order to *offer* the list and it never changes while the daemon runs |
| **The number can be changed, and it is one command** | ✅ `Command::RenumberFixture { id, to }`. Not an unpatch and a patch: the number is the key the patch is filed under, so two commands would leave the rig without that fixture in between, and leave it *deleted* if the second were refused. The journal images **both** numbers, which is what makes the undo right rather than lucky (`a_renumber_is_journalled_over_both_numbers`), and renumbering to the number it already has is accepted, changes nothing and is **not a step** — an operator pressing Oops after one would otherwise watch nothing happen and press it again, losing the edit they meant to take back |
| The three new commands are undoable, byte for byte | ✅ `the_patch_edits_of_s27_are_undone_and_redone_byte_for_byte`: each is applied to a file whose fixture carries a position, a rotation and an invert, taken back, and compared on the **serialised bytes** — so a restore that rebuilt the fixture from the command rather than from the image would fail here. S14's property test is unchanged and still green |
| **The fixture sheet shows live values, and the two are drawn two different ways** | ✅ and they are allowed to disagree, which is the reason both are there. The **programmer** column is a reader over `Delta::ProgrammerChanged` and renders in React like every other reader — a dash for absent, because *absent is not zero*, and a blank where the fixture's profile has no such attribute at all. The **output** column is telemetry and is drawn on a canvas by `patch/live.ts`, outside React entirely. In Chromium: `1 red at 100` typed into the console line, the programmer cell reads `100%`, and the output column's pixels are **counted** — zero before, greater than zero after, and zero again after Clear |
| The output column is `LevelSurface` and `driveTelemetry` one layer along | ✅ deliberately the same bricks. `drawFixtureLevels` paints through S24's five-operation seam, so all of it is asserted per bar in a runtime with no rasteriser; `driveFixtureLevels` is `telemetry/driver.ts` with a different picture in it. **Only the rows on the screen are drawn**: the canvas is the size of the window and the loop reads `scrollTop` off the scroll container each frame — a scroll offset is client-local (§4.2), so it is read where it lives and no command is sent about it. Four hundred rows cost twenty |
| A fixture whose universe is not on the cable is **absent**, not dark | ✅ *no output* and *output at zero* are different facts, and a sheet that drew them alike would tell an operator their rig was dark when it is in fact unpatched. Drawn as one block rather than as channels sitting at zero, and asserted. That is also what a universe with no output configured looks like — **S33**'s to fix and this session's only to make visible |
| **Patch and Fixture Sheet are two different windows, and the difference is now written down** | ✅ the decision the prompt asked for, in `ARCHITECTURE_SPEC.md` §6 beside `WindowType`: **Patch** is the *rig*, and the one window in this interface that changes the show's shape; **Fixture Sheet** is the *state*, watched rather than edited; **DmxSheet** (S25) is the *cable*, channel by channel, with no fixtures in it at all |
| **Nothing on either window is state the interface holds** | ✅ asserted on what happens *before* the delta, gesture by gesture, in `patchwindow.test.tsx`, which drives the whole `<App />` over a socket. Apply sends `PatchFixture` **and the row still reads what it read**; the draft is dropped on submit rather than merged. A number change sends `RenumberFixture` *then* `PatchFixture`, in that order. Unpatch names the number the row **started** at, not the one being typed — the trap an operator meets by changing their mind halfway. Cancel sends nothing at all and moves nothing. The one thing that is local is the row being typed into, which is §4.2's own category |
| The readers are held to the daemon's answers | ✅ `ui/tests/fixtures/patch-recording.json` is the **fifth** frozen recording, written by `crates/prismd/tests/ui_patch.rs` off a running `prismd`: nineteen steps of a rig being built, corrected and taken apart, with the rows and the profiles after each one taken from a *fresh client's* snapshot, and the daemon's answer beside every question. `patch.test.ts` replays the deltas through this interface's own mirror and compares its readers field by field; `preview.test.ts` decodes the answers and compares the sentences. Five Rust guards run on every commit |
| The recording is not vacuous | ✅ `the_recording_is_of_a_rig_being_built` demands the interesting cases be *in* it: a patch that grew and shrank, a fixture that changed its **number**, one corrected in place, a profile embedded, three refusals that change nothing at all, and an Oops at the end that takes the edit back. `a_preview_says_what_the_patch_that_follows_it_does` demands the three kinds of answer — free, overlapping and refused — and the pair either side of an `EmbedFixtureType` |
| **What is deliberately not computed in the client** | ✅ two things, and both are one rule. *Whether an address clashes* — asked, never intersected here. And *which channel a fixture ends on*: `address + footprint − 1` looks harmless and is the same arithmetic the overlap search runs, so the sheet shows the start address and the footprint, and the **span** is only ever shown from a `PatchPreview`, where the daemon computed it. One less place for two answers to exist |
| **The two telemetry numbers from S24 are still true** | ✅ both, with a second canvas on the screen. *Zero React commits over 300 frames of 64 universes* is unchanged — and it is what says the output column is outside React, because a sheet that put a level into a `useState` would turn it red. *Under 8 ms* was re-measured in Chromium against a real `prismd` on the 64-universe rig: **`64 universes · 30.0 Hz · paint 0.20 ms (p99 0.40 ms) · 160 frames · 2 lost`**, nothing dropped |
| **The three S25 criteria and the three S26 criteria still hold** | ✅ all **fourteen** end-to-end tests pass, the eleven from before among them: windows open, move and close by command and survive a reload; **D11 observed** from a file by a browser that was doing nothing; paging agrees from both ends; the wheel turns what the bar highlights; and an encoder change reaches the programmer and the output — **measured this run at `programmer 57 ms · output 100 ms`** |
| **No scrolling outside the canvas** | ✅ re-checked with forty fixtures patched from the browser and both sheets open: the document, the canvas and both bars all read zero on width and height, and the fixture sheet's own body reads **greater than zero** — because a long list scrolling inside its window is what a window is for, and a check that only demanded zeros everywhere would pass for a sheet that had quietly clipped its rows |
| `npx tsc -b --force` clean with `strict: true`, **no `any` anywhere** | ✅ exit 0. `any` appears **nowhere** in `ui/src` or `ui/e2e`. This session's shipped code contains no type assertion at all beyond the widening S23 allowed; every narrowing is a reader in `ipc/protocol.ts` that checks a field and names it if it is wrong — including the new `readAnswer`, which refuses an answer this build does not know rather than guessing at it |
| `npm run build`, `npm run lint`, `npm run test` clean | ✅ all three exit 0. The build is **283 kB (88 kB gzipped)**, up 14 kB on S26 — **and this session adds no dependency either.** A patch sheet is the classic place a table or a form library arrives; this one is a `<table>`, five `<input>`s and a `<select>`, and neither kind of library would have made the *cadence* decision, which is the only hard part of it and is `canvas/drag.ts`'s, taken a third time. `oxlint` reports nothing. **440 tests in 39 files**, up from 371 in 34 |
| `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ all three exit 0 — **1 472 passed and 19 ignored across 55 targets**, up from 1 441 and 18 across 54. The new ones are the query types and the three commands in `prism-domain`, the profile library and the preview in `prism-core`, the journal's two new images, the envelope in `prism-ipc`, `Desk::query` in `prismd`, and the five guards of the new `prismd/tests/ui_patch.rs` — whose regenerator is the nineteenth ignored test |
| **Coverage on what this session wrote** | ✅ **99.07 % lines**, 93.96 % branches, **99.79 % functions**, 99.09 % statements over `ui/src` (440 tests, `vitest` + Testing Library) — level with S26's 99.07 %. The new `patch/` module reads **98.80 % lines**, 87.36 % branches and **100 % functions**: `patch.ts` and `preview.ts` at **100 % lines**, `live.ts` 98.64 %, `patchwindow.tsx` 98.73 %, `sheet.tsx` 97.05 %. The uncovered lines were read rather than counted, and each is an arm this interface cannot reach: an `apply` with no draft (the form only renders when there is one), a row index past the end of the list being drawn, and a telemetry channel that is `null` inside a provider that supplies one |
| Coverage on the four crates that changed | ✅ `prism-core` **99.39 % lines**, 98.03 % regions, 98.40 % functions — the new `library.rs` at **99.34 %** and `conflict.rs` at **99.58 %**, both above the crate's own average, which is what the row is for. `prism-domain` **98.02 % lines** (up from 97.52 %), with the new `query.rs` at **100 % on every column**. `prism-ipc` **98.14 % lines**, 99.20 % functions. `prismd` **94.94 % lines**, `server.rs` 93.26 % — the query path has unit tests of its own in that crate rather than only the integration target's, because the target's non-ignored half reads a file |
| A test never touches a device | ✅ the output is `--mock-output`, and there is no console in this session's end-to-end spec at all. The unit suite has no socket, no canvas and no clock of its own: `FakeNetwork`, `ManualTimer`, a hand-stepped animation scheduler and `RecordingSurface`, so *the newest answer is drawn and the older one dropped* is asserted rather than waited for |
| CI green on the pushed commit | ✅ run **31828565272** on `f3356f5` — **all five jobs on the first attempt**: Windows full build and test 7 m 31 s, Linux neutral 3 m 02 s, UI end-to-end against a daemon 2 m 56 s, UI typecheck/lint/test/build 1 m 42 s, ARM64 cross-check 41 s. **All fourteen end-to-end tests passed on a Linux runner**, so a rig was built from an empty show — a profile out of the library, fixtures, a conflict named before it was made, a renumber and an unpatch — on a machine that has never run this interface by hand. Both S24 gates were re-measured there at the same time: **`64 universes · 30.3 Hz · paint 0.20 ms (p99 0.40 ms) · 155 frames · 1 lost`**, and S26's round trip came out **`programmer 113 ms · output 168 ms`** on a debug daemon under a software rasteriser |

**What was built, in six pieces.**

**1. `prism-domain::query` — the protocol's third shape.** `Query`, `Answer`,
`PatchPreview`, and `PatchConflict` moved here from `prism-core` so that it can
travel. The module documentation is the argument rather than a description: a
command expresses intent, a delta describes what has happened, and neither
answers *what would happen if*. The two alternatives are written down beside it,
because both are tempting and both are worse — a client that intersected the
spans itself is the duplication D3 exists to prevent, and a command that patched
and offered an undo would show the operator the conflict by *making* it.

**2. `prism_core::library` — the profiles the desk knows.** Four generic ones,
and the reason they exist: a show **embeds** its profiles (S11), which is right,
and leaves open where the *first* one comes from. `EmbedFixtureType` copies one
in, and after that the show owns it — a desk with a different library opens that
show unchanged. It is explicitly **not** a fixture library in the sense a venue
means: no import, no GDTF, no operator-authored profiles. That is the largest
thing this session leaves for a later one, and §7 carries it.

**3. `Show::preview_patch` — the daemon's arithmetic, without the patch.** Takes
its refusal from the same `check_patch` the edit runs, excludes the fixture from
its own overlap search (a repatch of fixture 3 where fixture 3 already is is not
a conflict, and reporting one would put a red line under every row an operator
opened and did not change), and writes nothing — asserted on the serialised
bytes and on the revision counter.

**4. `ui/src/patch/patch.ts` and `preview.ts` — readers, and the cadence.** The
third file of the kind `canvas/windows.ts` started, and a `PreviewRequester` that
drops the answer to a draft that has since been typed over — the case that would
otherwise tell an operator that the address in front of them clashes, when it is
the one they have already replaced that did.

**5. `patch/patchwindow.tsx` — the rig, edited.** A table, a form, a preview line
in three colours and a profile menu. What is local is the row being typed into,
and nothing else.

**6. `patch/live.ts` and `patch/sheet.tsx` — the two live columns.** The
programmer in React, the cable on a canvas, and only the visible rows drawn.

**Five mutation checks.** Computing the overlap in TypeScript instead of asking —
the tempting shortcut, and the one D3 forbids — cannot be done at all without
deleting the query, which turns eleven tests red across three files. Drawing the
newest answer *and* the older one turns the requester's ordering test red.
Sending `PatchFixture` before `RenumberFixture` turns the renumber test red.
Unpatching by the number in the form rather than the number the row started at
turns one test red, and it is the one an operator would meet. Putting the output
level into a `useState` turns `telemetry/render.test.tsx` red, which is the guard
S24 built for exactly this.

**One fixture pair was added and both are written by Rust.**
`ui/tests/fixtures/patch-recording.json` and `patch-rig.prism`, by
`cargo test -p prismd --test ui_patch -- --ignored`, opened by five non-ignored
Rust tests on every commit. **The other four recordings were regenerated**,
because the snapshot grew a field: `fixtureLibrary` is in every `Snapshot`, so
every recorded snapshot payload had to be rewritten. That is the diff being the
change, which §7 has said since S24.


### 2.29 S44 verification record

Measured on 2026-08-15, all exit criteria from `IMPLEMENTATION_PLAN.md` S44 —
the extension of S27 asked for on 2026-08-14. The session in which the desk
stops offering four approximations and starts knowing real fixtures.

| Check | Result |
|---|---|
| **A real fixture is searched, embedded and patched from the interface** | ✅ `ui/e2e/patch.spec.ts`, in Chromium against a real `prismd`: *stage wash 7x10* typed into the library box, the Stage Right 9-channel mode chosen out of the daemon's answer, and the fixture patched — with the preview saying **9 channels, ending at 9** before the command was sent, out of a profile read from a JSON file **this repository does not contain**. The row then shows *Stage Wash* and a footprint of 9 |
| **The channels are the manufacturer's** | ✅ asserted against the file rather than against the reader: `crates/prism-core/tests/fixture_library.rs` reads the installed tree and checks that `stage-right/stage-wash-7x10w-led-moving-head/9ch` puts Pan, Tilt, Dimmer, Red, Green, Blue and White on channels 0…6 — which is the order that fixture's `modes[0].channels` gives — and that pan carries the 540° the file states, centred on zero as `ARCHITECTURE_SPEC.md` §6 writes a pan range |
| **The library is downloaded at install time, not committed** | ✅ `tools/fetch-fixtures/fetch-fixtures.sh` and `.ps1`, using nothing but `curl`/`Invoke-WebRequest` and `tar` — all of which ship with Windows 10 and later, with every Linux this runs on and with macOS, because a downloader with a dependency is a dependency somebody has to install before they can install anything. The revision is **pinned**, not `master`, so two machines that install a week apart get the same profiles and a show patched on one opens the same way on the other. `.gitignore` keeps everything but `SOURCE.md` out of git; **8.5 MB of somebody else's data stays where it has an upstream** |
| Every one of the 634 files parses | ✅ **0 files rejected** over the whole installed tree, and that number is asserted rather than reported. Getting there found something: seven files are not fixtures at all but **redirects**, OFL's way of keeping an old key working when a fixture is renamed or turns out to be another brand's — and one of the seven is the Lixada Mini Moving Head RGBW the request linked, which points at the Stage Right wash. They are **followed** rather than skipped: the alias carries the name on the box and the channels of the fixture it points at, so an operator holding a Lixada searches for *Lixada* and finds it. 73 of the 2 157 profiles are aliases |
| **The conversion is lossy, and the loss is measured** | ✅ printed by the corpus test on every run and shaped by assertions rather than pinned to numbers a re-import would break: **627 fixtures, 2 084 modes converted, 2 157 profiles**; 714 modes skipped because their channel list contains a matrix insert or a switching channel, which makes the layout depend on state and a footprint does not; 8 388 attributes mapped; 5 037 channels carrying a capability this model has no `AttributeType` for; 1 410 dropped as duplicates of an attribute a lower channel already claimed; 363 channel names that resolve to nothing. What is asserted is the *shape* — at least 65 % of modes convert, at most 20 % control nothing, and unresolved names stay under a tenth of the attributes mapped |
| **Every profile the reader produces is one the show model accepts** | ✅ the strongest thing that can be said about a converter whose output goes to a validator: all 2 157 are run through `embed_fixture_type`, the same door `Command::EmbedFixtureType` uses. An attribute outside its footprint, a duplicate, an empty footprint — any of them, in any of them, fails there. Every coarse *and* fine offset is separately asserted to be inside its own footprint |
| A fixture the desk cannot fully express is **honest about it** | ✅ UV, Cyan, Yellow, Magenta, Lime and Indigo have no `AttributeType`, so a CMY fixture patches with its colour mixing missing rather than with it guessed at. Counted, documented at the top of `library/ofl.rs`, and recorded in the decision log as wanting `AttributeType` widened — which is a domain change and a session of its own |
| **The library had to leave the snapshot** | ✅ and this is the finding that changed what S27 built rather than adding to it. 2 157 profiles is several megabytes; `docs/IPC_PROTOCOL.md` §3 caps a frame at 1 MiB. So `Snapshot.fixtureLibrary` is now a **count**, and the list is asked for — `Query::SearchLibrary` → `Answer::LibraryMatches`, carrying a `LibraryEntry` (key, manufacturer, name, mode, footprint) rather than a `FixtureType`. **The profile itself never leaves the daemon.** That is the second caller of the query channel S27 added and the first that *needed* it |
| The answer fits in a frame, asserted | ✅ `a_search_over_the_whole_library_fits_in_a_frame` encodes the answer to seven queries — including the empty one and the single letter `e`, which are the two worst cases the corpus admits — and requires every one under 256 kB against a 1 MiB limit. The limit is clamped **by the daemon** (`MAX_SEARCH_LIMIT`), so a client that asked for two thousand does not get them |
| **A search finds what a person would type** | ✅ words in any order, matched against the manufacturer, the name, the mode and the key: *robe 600* and *600 robe* both find the Robe MMX Spot 600. Ranked so the answer is useful when it is cut off — a hit in the **name** beats one only in the key, an earlier hit beats a later one, and one fixture's modes come out smallest first. A word nothing has answers with **nothing**, not with everything, which is what a search that ignored an unmatched word would do and what an operator would read as *the library is broken* |
| **The operator's own fixtures are read automatically, and win** | ✅ `<data-dir>/fixtures/`, in the same OFL format, read at start-up and **before** the installed tree — `FixtureLibrary` keeps the first profile it is given for a key, so a correction in the data directory beats the installed one and survives the next install, which replaces `profiles/fixtures/` wholesale. Asserted with two trees holding the same key and different names |
| A corrupt profile never stops the desk | ✅ a file that is not JSON, a fixture with no modes, a mode naming a channel nothing defines, a directory that is not there — each is skipped and counted, and none reaches a caller as an error. `nothing_here_trusts_the_file` walks ten malformed shapes. A desk with no library at all starts on the four built-in profiles and **says so in the log**, because a lighting desk that would not start over a missing directory is a worse answer than one with four profiles in it |
| **A `/` in a profile key broke every reader, and that was found here** | ✅ the session's other finding, and a real defect in shipped code. An OFL key is `manufacturer/fixture/mode`, and a JSON Pointer built by pasting one in names three levels of a document that has one — so `patchRows`, `liveFixtures` and **S26's `groupOf`** all answered `null` for every OFL fixture: a fixture sheet with no attributes, an encoder bar with no banks, a patch row with no footprint. `prism_core::show::escape` had done RFC 6901 §3 correctly since S11 and both mirrors unescape; what was missing was the third side, for a *view*. `mirror/select.ts::pointerToken` is it, and the recording's guard is what caught it |
| The recording does not depend on what is installed | ✅ `crates/prismd/tests/ui_patch.rs` writes **its own two-fixture library** into a temp directory and starts the daemon with `--fixtures` pointing at it. A recording made against whatever the developer happened to have installed would fail on a fresh clone and change whenever upstream did. The recorded answers are compared against what *this build's* reader and search make of the same two files |
| The recorded script exercises the search | ✅ three `SearchLibrary` steps: one that finds both modes of a fixture (smallest first), one that finds **nothing**, and an empty one clamped to three. Then the key that was answered with is embedded and patched, which is the whole loop — search, choose, embed, patch — and the row that results carries the manufacturer's footprint |
| `cargo test --workspace`, `clippy -D warnings`, `fmt --check` | ✅ all three exit 0 — **1 504 passed and 19 ignored**, up from 1 472. The new ones are the OFL reader's fifteen, the library's fourteen, the corpus target's six and the daemon's |
| **Coverage** | ✅ `prism-core` **99.13 % lines**, 97.97 % regions, 97.72 % functions — the new `library/mod.rs` at **98.61 %** and `library/ofl.rs` at **97.24 %**. `ui/src` **99.07 % lines**, 94.01 % branches, **99.80 % functions**; the `patch/` module **98.88 % lines**. *A caveat worth recording: `cargo llvm-cov` invoked twice in one shell merges the two runs' profile data and reports figures tens of points low — the first reading of `prism-core` here was 92 %, and `cargo llvm-cov clean --workspace` between runs is what makes the number reproducible* |
| **S27's three criteria still hold, and so do S24's and S26's** | ✅ all **fifteen** end-to-end tests pass, the fourteen from before among them. A rig is still built from an empty show; a conflict is still named before the command is sent; the fixture sheet still shows two live columns. **`64 universes · 30.3 Hz · paint 0.20 ms (p99 0.50 ms) · 154 frames`**, and the encoder round trip **`programmer 78 ms · output 131 ms`** |
| `npx tsc -b --force`, `npm run build`, `lint`, `test` | ✅ all clean, **441 tests in 39 files**. `any` appears nowhere. No new npm dependency — and none in Rust either: the reader is `serde_json`, which `prism-core` already had for the show projection |
| **Loading the library is measured, and it is off the critical path** | ✅ the exit criterion, and it was not met when this record was first written — it said the cost would be measured and the number was never taken. It is **395 ms in a release build and 1.49 s in a debug one** for 634 files, printed by `reading_the_whole_library_is_timed`. That is far too much to put in front of the engine, so the read happens on a thread of its own: started before `EngineThread::start` and joined after it, so **DMX begins at the same instant it always did** and what waits is a client connecting. Measured end to end, release: 1 348 ms without a library, 1 674 ms with one |
| A test never touches a device | ✅ unchanged. The one thing this session adds that reaches outside the machine is the **installer**, which is a script a person runs and not something a test does |
| CI green on the pushed commit | ✅ run **31847326456** on `cce51e3` — **all five jobs**: Windows full build and test 11 m 11 s, Linux neutral 3 m 04 s, UI end-to-end against a daemon 2 m 50 s, UI typecheck/lint/test/build 1 m 20 s, ARM64 cross-check 54 s. **The library was installed on the runners** — `installed 634 fixtures` in three jobs, on Windows and on Linux — so the corpus tests ran rather than skipping and the end-to-end suite searched a real one: **all fifteen passed**, including the Stage Right wash found by name and patched in a Chromium that had never seen it. The two S24 gates were re-measured there: **`64 universes · 30.4 Hz · paint 0.10 ms (p99 0.30 ms) · 156 frames · 1 lost`** and **`programmer 84 ms · output 131 ms`**. The run that made this record green is **31880588557** on `2c0e4b8`, after the three fixes below: Windows 8 m 31 s, Linux neutral 2 m 58 s, UI end-to-end 2 m 05 s, UI typecheck/lint/test/build 1 m 46 s, ARM64 37 s, with **`64 universes · 30.4 Hz · paint 0.10 ms (p99 0.30 ms) · 155 frames`** and **`programmer 87 ms · output 121 ms`**. *Getting there took three attempts and each one found something real. Run 31846688901 on `1197bcd` failed the two Linux jobs with exit 126: Git on Windows does not set the executable bit, so the installer would not run. The Windows job passed on that attempt, which is what said the PowerShell half and the library were fine. The **docs-only** push afterwards then failed the end-to-end job on a forty-fixture loop that raced against the round trip — a flake that had passed here three runs out of five, fixed by typing the number rather than accepting the form's suggestion. And run 31879211689 on `19d0d8e` then failed **Windows** on `wiring.rs` — a client shown a telemetry frame of all zeros for a rig at full — which turned out to be three things at once: the library parsing ahead of the engine, a daemon that published a frame the engine had never produced, and a tick rate divided by the wrong clock. All three are in the decision log. The lesson is the plain one: **a push whose diff cannot break CI can still be the push that reveals what is broken**, so watch it.* |

**What was built, in five pieces.**

**1. `tools/fetch-fixtures` — the install step.** Two scripts, no dependencies, a
pinned revision. The library is 8.5 MB with an upstream and a release cadence; a
copy in this repository would be the second copy of somebody else's data, and
the one that is out of date.

**2. `prism_core::library::ofl` — the reader.** One `FixtureType` per OFL
**mode**, because a mode is what a patched fixture *is*. Its module
documentation is the conversion table and the list of what is lost, and every
loss is counted rather than described.

**3. `prism_core::FixtureLibrary` — three sources, one key space.** The
operator's folder, the installed tree, the built-in four, in that order, first
key wins. Plus the search, which is what a library this size is used through.

**4. The query channel's second caller.** `Query::SearchLibrary` and a
`LibraryEntry` that is deliberately not a profile. The snapshot's field became a
number, which is the one thing S27 got wrong that only scale could reveal.

**5. `mirror/select.ts::pointerToken`.** Three lines, one real defect: a key with
a `/` in it had been silently unreadable by every view since S26.

**Three mutation checks.** Dropping the pointer escaping puts the fixture sheet,
the encoder bar and the patch row's footprint back to blank for every OFL
fixture — and turns the recording's guard red, which is where it was found.
Taking `Intensity` off the front of the capability rule puts every combined
`Dimmer / Strobe` channel on the shutter bank, where nobody would look for it,
and turns two tests red. Skipping redirects rather than following them loses the
Lixada alias and turns one test red.

---

### 2.30 S35 verification record

Measured on 2026-08-19, all exit criteria from `IMPLEMENTATION_PLAN.md` S35 and
the session prompt. The session that corrects what S26 shipped before anything
else is built on top of it: one band instead of two, a page number that finally
pages something, and a view library an operator can order rather than only add
to.

| Check | Result |
|---|---|
| **Both bars sit in one band, and the overflow check still reads zero** | ✅ `.deskband` is **6.8 rem** and holds both — the encoders at a fixed 27 rem on the left, the eight strips taking what is left — against S26's 3.1 rem + 6.4 rem in two full-width bands, so the canvas gets **2.7 rem back** and the encoders get the room a real encoder needs. The height is still fixed rather than taken from the contents, which is S26's rule and the important one: a band that grew when an executor was assigned would move every window on the screen. `ui/e2e/desk.spec.ts` reads `scrollWidth − clientWidth` and `scrollHeight − clientHeight` off the document, the canvas **and both bars** with four windows open and demands **eight zeros**, and gets them |
| **The encoder bar shows four parameters at a time; paging from the interface and from `Zoom ▲▼` agree** — observed, not asserted | ✅ `ui/e2e/desk.spec.ts`, in Chromium against a real `prismd` with `--mock-surface`: three MIDI bytes for Encoder Assign / Plug-in (note 43, written out from `docs/MCU_MAPPING.md` §2.1 by hand) select the Beam bank, four of its six parameters are drawn and the readout says `1/2`; `Zoom ▼` (note 97) is appended to a file **by neither the browser nor the daemon** and the bar shows `2/2` with Shutter and Control. Then the *browser* pages back and the console's next press carries on from where it left it. **One page number, two hands on it** |
| **A bank with fewer parameters than a page shows what it has and disables the page control; a bank with more than one page is asserted to exist** | ✅ Dimmer (one parameter) and Position (two) read `1/1` with both page buttons dead; Beam reads `1/2`. That Beam *has* six is asserted rather than assumed — `FEATURE_GROUP_ATTRIBUTES.Beam.length` is checked against `ENCODERS_PER_PAGE` in `desk.test.tsx`, and the table is generated from `FeatureGroup::attributes`, so a later session moving an attribute off Beam turns that red instead of quietly leaving the paging untested. A session page **past** the end shows the last page rather than an empty bar |
| **A view can be renamed, deleted and moved from the interface, each as a command whose answer arrives as a delta** | ✅ a context menu on a view button — rename, store here, store as new, move left, move right, delete — and every item sends a command and closes. Asserted on what happens *before* the delta, the way S25 asserts the canvas: the rename field still reads the daemon's name after Enter, the bar's order does not move when Move is pressed, and the view is still there after Delete. **The interface holds no view list**: `ViewBar` renders `storedViews(session)` and holds no order, no names and no numbers. **And no dependency was added** — a context menu is the classic place one arrives, and this one is about 180 lines of React with two document listeners for Escape and click-away |
| **Deleting the active view leaves the canvas in a state the *daemon* defines** | ✅ `SessionState::delete_view` selects the neighbour **before** it, or the one after it when there is none, exactly as `select_view` would — so the canvas is a *stored layout* and never a leftover. There is no successor-picking code in TypeScript at all. `ui_session.rs` asserts on the recording that `activeViewId` names a view that still exists and that the canvas is **that view's layout**; `e2e/session.spec.ts` watches it happen against a real daemon. The **last** view cannot be deleted (`SessionError::LastView`) and the menu item is disabled as well, so the refusal is not the first an operator hears of it |
| **After a move, `Channel ▶` steps to the view that is drawn next** | ✅ `ui/e2e/session.spec.ts`, with a real console: three views, `Channel ▶` from view 1 reaches the Groups layout; the *browser* moves view 3 left; `Channel ▶` from view 1 now reaches the **Patch** layout, because that is what is drawn second. It is structural rather than maintained — see the decision log: the number **is** the order, `MoveView` exchanges two numbers, and `SessionState::neighbour` is the one place the order is expressed, which `delete_view`, `move_view` and `prismd::surface::context_of` all resolve through |
| The three commands are a protocol change with tests **in each crate** | ✅ in the shape S25 used for `PlaceWindow`. `prism-domain`: three variants, `is_session_command` grown to fifteen, both protocol lists extended and their counts asserted (30 commands, 15 of them session). `prism-core`: `rename_view`, `delete_view`, `move_view`, `SessionError::LastView`, and **twelve** new unit tests — including that a rename leaves the windows alone, that a move steps to the next *stored* number rather than `id ± 1`, and that the active view follows the view rather than the number in **both** directions. `prism-ipc`: all three through the client envelope, and `any::<Command>()` carries them into the framing property test by construction. `common::session_commands` now holds all fifteen, so every property in `session_commands.rs` covers them — and `the_session_invariants_survive_any_command_sequence` generates arbitrary sequences and asserts `session.view(active_view_id).is_some()` throughout, which is what says `DeleteView` cannot break the invariant |
| **The expectations come from the daemon, not from a second opinion in TypeScript** | ✅ `crates/prismd/tests/ui_session.rs`'s script grew **eight steps** — a view stored at 5 to leave a gap, a rename, a refused rename, a move, a move that cannot happen, a select, a delete of the active view, a refused delete — and the recording was regenerated by driving a real `prismd`. Three non-ignored Rust tests read it on every commit, and this session added guard assertions to them for each of the claims above. The browser compares its readers against the same file. Steps are found by their `what` text rather than by index (S44's finding), so the eight were appended without moving anything |
| **The nine executor-bar tests S26 wrote are running again** | ✅ they had been commented out, and they *are* S26's first and third exit criteria — paging by command, the four unpressable buttons, the fader's cadence-and-drop. A criterion whose test is commented out is not a criterion, and S35's own criteria require S26's to still hold. Restored and green against the new layout, together with the `select-N` target they use: the select click had been moved onto the strip **container**, where a pointer up on the fader synthesises a click that bubbles, so every master move and every Go was also sending a `SelectExecutor` |
| `npx tsc -b --force` clean with `strict: true`, **no `any` anywhere** | ✅ exit 0. `any` appears **nowhere** in `ui/src` or `ui/e2e`. The one new narrowing is `ProgrammerValueSource`, which is a generated union and needs none |
| `npm run build`, `npm run lint`, `npm run test` clean | ✅ all three exit 0. The build is **289 kB (89 kB gzipped)**, up 5 kB on S44 — **and this session adds no dependency either.** `oxlint` reports nothing, and it now has `eslint/no-console` set to error, with `src/log/logger.ts` — the structured sink `CLAUDE.md` requires — and the test trees exempted. That rule found a `console.log` on every row of the fixture sheet, which is what a prohibition nobody enforces is worth. **470 tests in 39 files**, up from 441 |
| `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ all three exit 0 — **1 517 passed and 19 ignored**, up from 1 504 and 19. Thirteen new: twelve in `prism-core`'s session module and one in `prism-ipc`'s envelope; the domain's and `prismd`'s grew inside existing tests. Two hard-coded protocol counts had to move and both are guards doing their job — `session_commands.rs`'s and `command_application.rs`'s, which exist precisely so a new command variant cannot be added without being given a home |
| **Coverage on what this session wrote** | ✅ **99.11 % lines**, 94.09 % branches, **99.81 % functions**, 99.14 % statements over `ui/src` (470 tests), up from S44's 99.07 %. Every file this session touched is at **100 % lines**: `canvas/viewbar.tsx`, `desk/encoderbar.tsx`, `desk/executorbar.tsx` and `desk/programmer.ts`. `prism-domain` **97.96 % lines**, 99.72 % regions, 100 % functions; `prism-core` **97.98 % lines** with `session.rs` at **98.17 %**; `prism-ipc` **97.15 % lines** with `message.rs` at **99.11 %** |
| **The measured numbers from S24–S27 are still true** | ✅ *Zero React commits over 300 frames of 64 universes* is unchanged, with the band rebuilt above the canvas. The frame budget was re-measured in Chromium against a real `prismd` on the 64-universe rig: **`64 universes · 30.2 Hz · paint 0.20 ms (p99 1.50 ms) · 154 frames · 1 lost`**, and three consecutive runs of the gate alone read 30.4 Hz with **nothing lost**. **All eighteen end-to-end tests pass** — S25's three, S26's three and S27's three among them |
| A test never touches a device | ✅ the console is a **file** and the output is `--mock-output`. The unit suite has no socket, no canvas and no clock of its own |
| CI green on the pushed commit | ✅ run **32252325489** on `2182366` — **all five jobs on one run**: Windows full build and test 6 m 39 s, Linux neutral 2 m 56 s, ARM64 cross-check 2 m 14 s, UI typecheck/lint/test/build 1 m 36 s, UI end-to-end **2 m 1 s with all eighteen tests passing**. The session's own code was green from `d822c02` (run **32249620556**); the three commits after it are CI hardening, not product changes. That run's figures: Windows full build and test 8 m 53 s, Linux neutral 3 m 6 s, ARM64 cross-check 41 s, UI typecheck/lint/test/build 1 m 43 s, UI end-to-end against a daemon 2 m 16 s. **All eighteen end-to-end tests on a Linux runner**, so the console paging the encoder bar and `Channel ▶` following a moved view were both observed on a machine that has never run this interface by hand — a `prismd` compiled there, a Chromium downloaded there, and MIDI bytes appended to a file by neither. The telemetry gate came out **`64 universes · 30.3 Hz · paint 0.20 ms (p99 1.00 ms) · 156 frames · 1 lost`** there against 30.2 Hz here, and this session's own console measurement **`programmer 108 ms · output 211 ms`**. **Three CI faults were found and fixed on the way, none of them the commit's**: `--with-deps` made the end-to-end job depend on Ubuntu mirrors that went down mid-session; the same outage then killed the ARM64 job at its own fifteen-minute timeout, in a job that normally takes 41 s; and an S44 test was skipping itself while claiming a library was missing that was in fact installed. All three are in the decision log |

**What was built, in four pieces.**

**1. One band, two halves.** `.deskband` holds `EncoderBar` and `ExecutorBar` side
by side. What the encoders bought with the room: four parameters at a time with
the name, the value, **where the value came from** (`ProgrammerValueSource` —
`man`, `preset`, `cue`, or `~` when the selection disagrees) and how much of the
selection each covers, plus a page control and the bank buttons on a row of their
own.

**2. `programmerPage` reads something at last.** `ENCODERS_PER_PAGE` is four and
it is the *interface's* decision; so is the upper bound, which is the split
`SelectProgrammerParam` already had — `prism-core` deliberately does not know how
many parameters a bank has (S13), so `encoderPage` clamps and the buttons go dead
at the ends.

**3. Three commands and a menu.** `RenameView`, `DeleteView`, `MoveView` through
`prism-domain`, `prism-core` and `prism-ipc`, and a context menu that sends them
and closes. The ordering decision — the number **is** the order — is in the
decision log, in `Command::MoveView`'s own documentation, and asserted from three
directions.

**4. What the review found on the way in.** A `console.log` shipping on every
fixture-sheet row and no lint rule to catch it; a notices panel keeping a second
list beside the store's ring buffer, which left an empty frame nobody could
dismiss; a disconnect message that had lost the sentence telling an operator the
rig is unaffected; nine commented-out tests that were S26's exit criteria; and
`.strip` defined **twice** in the stylesheet, so the eight executor strips had
never once used the five-row grid their own rule sets up. All five are fixed and
the last four are in the decision log.

**Four mutation checks.** Recording an order beside the view map — the rejected
half of the ordering decision — leaves `context_of` stepping numbers while the
bar draws the order, and the end-to-end `Channel ▶` test is what goes red.
Picking the successor of a deleted active view in the client turns the
`ui_session.rs` guard red, because the recording says what the daemon chose.
Letting the encoder bar page past the end of a bank empties the screen and turns
two tests red. Putting the strip's select click back on the container turns the
restored executor-bar tests red, because the fader drag then sends a second
command.


### 2.31 S28 verification record

Measured on 2026-08-19, all exit criteria from `IMPLEMENTATION_PLAN.md` S28 and
the session prompt. The session in which the interface stops needing a show
somebody else wrote: a look is put in the programmer and **kept**.

| Check | Result |
|---|---|
| **Cues can be stored, edited and fired from the interface** | ✅ and *from the interface* is taken literally: `ui/e2e/looks.spec.ts` starts a real `prismd` on `show-rig.prism` — four fixtures and **nothing stored**, no sequence, no cue, no executor — and writes a show in Chromium. An executor is selected, a cue list is made and put on it, a look is stored into cue 1, the same cue is stored into again, its **name**, its **fade** and its **number** are corrected in place, a second cue is renumbered to `0.5` and moves to the top of the list, the list is fired with Go and stopped with Off, and a cue is deleted without the numbers left closing up. Then `page.reload()`, and the show is still there, because it was never here |
| **A store that would overwrite says what it will do before it does it** | ✅ **before**, in the daemon's own words and with the command unsent. The Store button does not say *Store*: it says `Merge into cue 1 — Opening: 2 added, 3 replaced` — and it said `Nothing is there yet` a moment earlier over the same number. The three counts are `Query::StorePreview`'s and so is the word **Merge**: `StoreMode` is a value on the answer rather than a string in the client, because `prism_core::Programmer` merges unconditionally today and **S39** adds Override and Remove, at which point a spelled-out "Merge" here would go on looking right and be wrong. `store.test.ts` asserts every recorded preview carries `Merge`, which is the assertion that goes red when S39 lands |
| The daemon computes it, and the protocol grew the way to ask | ✅ S27's `Query`/`Answer` shape earning its keep a second time, which is what its own §7 note asked for: *the next session to want a derived answer should add a third variant rather than compute one*. `Query::StorePreview { target }` is answered by `prism_core::ShowFile::preview_store`, which is on the **file** rather than on the show because it needs all three things only that type holds together — what the programmer is holding, what is already filed under that number, and which store mode this build has. Nothing in TypeScript counts an overlap |
| **A preview is what the store that follows it does** | ✅ the claim the criterion rests on, asserted twice in two languages, and it took a correction: the first implementation counted the **merged result**, which reported every value a Merge left alone as one it had replaced. The counts come from `Programmer::stored_keys` — the same filter the store itself runs — so *added + replaced + kept* is the cue afterwards. In `prism-core`: previewed then stored, with the sum compared against the stored cue's parts. In `prismd`'s recording: a create says `(3, 0, 0)` and the cue that follows has three parts; an overwrite says `(2, 3, 0)` and the merged cue has five; a preset edit says `(0, 3, 3)` — **three kept**, which is exactly what an Override would have thrown away |
| **Preset pools apply and store** | ✅ five pool tabs over one numbering (`ApplyPreset` carries a number and no pool, so numbers are unique *across* pools and the tabs are a filter), the colour `Preset::color` carries drawn as the swatch the scribble strips use, a click applying to the selection, and a Store bar with the preview on it. Which values go into a pool is the **daemon's** filter and not the client's — a colour preset takes the colour values, and the bank an attribute is on is `AttributeDef::featureGroup`, the profile's answer rather than the attribute name's. Asserted on the recording: the preset the script stored holds **six** values where an interface counting blues would have guessed three |
| **Preset links remain live: editing a preset changes the cues that reference it** | ✅ the session's hardest claim, and it is now true rather than documented. `prism_domain::preset`'s first paragraph has said since S1 that *a cue part that carries a `presetRef` follows later edits of the preset*, and until this session nothing did it: the cue kept the value the preset had when it was stored. `prism_core::Show::relink` rewrites every linked part when a preset is stored, in the **show** rather than in the engine, because the tick may not resolve anything (§3.1) — so the value in the cue is always the value that will be output and the link is what keeps it current. Seen three ways: in `prism-core` on the stored cue; in the recording, where cue 3's blues change and the three **whites the store never mentioned do not**; and in Chromium, where the same Go puts a different level on the rig, counted as pixels on the telemetry canvas |
| A link the store does not mention keeps **both** its value and its link | ✅ dropping the value would change light nobody asked to change, and dropping the link would mean a preset that regained the value could never reach the cue again — the view `Show::remove_preset` already takes of a preset that has gone altogether. And an Oops over a preset edit puts the **cues** back as well: `Image::Preset` images the sequences the preset reaches, because restoring the pool alone would take the edit back in one place and leave it standing in every cue |
| **Nothing about a cue, a sequence or a preset is held in the interface** | ✅ asserted on what happens *before* the delta, gesture by gesture, in `sequencesheet.test.tsx` and `presetpool.test.tsx`, which drive the whole `<App />` over a socket. Storing sends `StoreCue` **and the table still reads what it read**. A cell edit sends `SetCueProperty` and the cell goes back to the daemon's text; the draft is dropped, not merged. A renumber names the cue by the number the row **started** at. Choosing a sequence sends `AssignExecutor` — there is no *selected sequence* anywhere in this client, which is the marked assumption below. Deleting a cue leaves the list as it was until the daemon says otherwise. The three readers hold nothing: `sequenceRows`, `presetRows` and `executorInForce` walk the documents and answer |
| The readers are held to the daemon's answers | ✅ `ui/tests/fixtures/show-recording.json` is the **sixth** frozen recording, written by `crates/prismd/tests/ui_show.rs` off a running `prismd`: 43 steps of a show being written, corrected and played, with the sequences, the presets and the executor grid after each one taken from a *fresh client's* snapshot — and every cue's **values**, not a count of them, because two cues with the same number of parts and different values are exactly what a preset edit produces. `looks.test.ts` replays the deltas through this interface's own mirror and compares field by field; `store.test.ts` decodes the answers and compares the sentences. **Eight** Rust guards run on every commit |
| The recording is not vacuous | ✅ `the_recording_is_of_a_show_being_written` demands the interesting cases be *in* it: a cue list that grew, was corrected and shrank; a cue that changed its **number**; a cue carrying a name, a fade and a trigger time together; five refusals that change nothing at all; an executor that gained a sequence; and an Oops at the end that takes the last edit back. `a_store_preview_changes_nothing` demands every question broadcast **no** deltas — not an empty list, none — and left the sequences, the presets *and* the executors exactly as the step before it did |
| **The store-mode question is raised and not answered** | ✅ exactly as `IMPLEMENTATION_PLAN.md` asks. No command carries a mode, because `prism_core::Programmer::cue` merges unconditionally and a client carrying a mode the daemon did not honour would be describing an outcome that did not happen. What the session ships instead is the *sentence*: `StoreMode` is a one-variant enum in `prism-domain` with the argument in its own documentation, the daemon answers with it, and the interface renders the word it is given. `merge_is_the_only_store_mode_this_build_has` is the test that turns red the day S39 adds the second |
| **The selected sequence is marked, not made permanent** | ✅ `ARCHITECTURE_SPEC.md` §4.1 gained **no field**. The sheets follow the sequence on `selectedExecutor`, which is the reading that needs nothing invented — every part of it is already in the two documents — and choosing one is an `AssignExecutor`, so two screens cannot disagree about which is in force. The assumption is written down in three places a later session will meet: `show/looks.ts::executorInForce`, the Sequence Sheet's module documentation, and a new paragraph in §4.4 naming S39 as the session that decides |
| **The cue number is still a dash, and the absence is asserted from both ends** | ✅ `Executor::currentCueIndex` is on the wire and **nothing fills it** — what cue a playback is on lives on the tick thread with no channel back (S26's finding). So the sheet says *which* executor is running (`isActive` does arrive) and writes `cue —` rather than a number it invented. `the_cue_index_is_still_a_dash` demands every recorded executor's index be `null` *and* that something was running at some point, so it cannot pass over a script in which nothing ever played; `looks.test.ts` makes the same demand in the browser. **Both go red the day S34 builds the channel**, which is the only kind of note nobody can overlook |
| Only the three button functions the protocol has are offered | ✅ S26's rule, followed rather than re-litigated: the sheet's transport is Go, Back and Off, and the five functions with no command are still drawn disabled on the executor bar with the reason on them |
| **The five commands are a protocol change with tests in each crate** | ✅ in the shape S25 and S27 used. `prism-domain`: five variants and `CueProperty`, both protocol lists extended and their counts asserted (**35** commands, 15 of them session), plus `StoreTarget`, `StoreMode` and `StorePreview` on the query shape, each with a round-trip property and a wire-shape test. `prism-core`: `create_sequence`, `set_cue_property`, `remove_cue`, `assign_executor`, `remove_executor`, `relink`, `sequences_using_preset`, `Programmer::preset`, `Programmer::stored_keys` and `ShowFile::preview_store`, with **24** new tests in a new `tests/looks.rs`. `prism-ipc`: the default handler's new arm, and `any::<Command>()` carries the five into the framing property test by construction. `prismd`: the `StorePreview` arm of `Desk::query`. `common::show_commands` now holds all twenty, so every property in `command_application.rs` and `session_commands.rs` covers them — including `undoing_every_command_returns_the_state_it_started_in`, which is what found the one real defect in the batch |
| Every new command is undoable, byte for byte | ✅ S14's property test is unchanged and still green over all thirty-five commands, and it is what caught the defect: `AssignExecutor { sequenceId: null }` on an **empty** slot created an executor row the undo could not remove, because `Show` had no way to take one out. The fix is two rules rather than a special case — clearing a slot that has nothing on it changes nothing, and `Show::remove_executor` exists so an Oops can put the grid back exactly as it was. `Image::Sequence` and `Image::Preset` grew the absence `Image`'s own documentation predicted in S14: *"S28's editor will create or delete a sequence, and that is when this grows an absence of its own"* |
| **An edit that would change nothing is not a delta and not a step** | ✅ S27's `RenumberFixture` rule, applied to every field: setting a cue's name to what it already says answers with no operations, raises no dirty flag and files no journal entry — because an operator who pressed Oops after one would otherwise watch nothing happen and press it again, losing the edit they meant to take back |
| **No scrolling outside the canvas** | ✅ re-checked in Chromium with a cue list of thirty cues on the screen: the document and the canvas both read zero on width and height, and the sheet's own body reads **greater than zero** — because a long list scrolling inside its window is what a window is for, and a check that only demanded zeros would pass for a sheet that had quietly clipped its rows |
| `npx tsc -b --force` clean with `strict: true`, **no `any` anywhere** | ✅ exit 0. `any` appears **nowhere** in `ui/src` or `ui/e2e`. This session's shipped code contains one type assertion, and it is the widening S23 allowed — `(TABLE as readonly string[]).includes(value)` inside a type predicate, which claims nothing about the string. The new decoder arm, `readStorePreview`, refuses a `StoreMode` this build does not know rather than rendering a word nobody chose |
| `npm run build`, `npm run lint`, `npm run test` clean | ✅ all three exit 0. The build is **305 kB (93 kB gzipped)**, up 16 kB on S35 — **and this session adds no dependency either.** A cue sheet with editable cells is the classic place a table or a grid library arrives; this one is a `<table>`, a `<select>` and an `<input>` that replaces the cell it was clicked on, and the only hard part is the *cadence*, which is `canvas/drag.ts`'s decision taken for the fourth time. `oxlint` reports nothing. **533 tests in 43 files**, up from 470 in 39 |
| `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ all three exit 0 — **1 562 passed and 20 ignored**, up from 1 517 and 19. The new ones are the five commands and three query types in `prism-domain`, twenty-four in `prism-core`'s new `tests/looks.rs`, three in its journal module, and the eight guards of the new `prismd/tests/ui_show.rs` — whose regenerator is the twentieth ignored test. Two hard-coded protocol counts had to move and both are guards doing their job |
| **Coverage on what this session wrote** | ✅ **99.05 % lines**, 93.68 % branches, **99.68 % functions**, 99.08 % statements over `ui/src` (533 tests), against S35's 99.11 %. The new `show/` module reads **99.64 % lines**, 92.73 % branches and **100 % functions**: `looks.ts`, `store.ts`, `cueviewer.tsx` and `presetpool.tsx` at **100 % lines**, `sequencesheet.tsx` 98.98 %. **One line in the module is uncovered** and it was read rather than counted: the guard in `choose` for a sequence chosen with no executor selected, which the interface cannot reach because the chips are disabled and which exists so the command cannot be built without one |
| Coverage on the four crates that changed | ✅ `prism-core` **98.96 % lines**, 97.71 % regions, 97.82 % functions — `command.rs` and `mirror.rs` at **100 % lines**, `programmer.rs` **99.67 %**, `journal.rs` 99.49 %, `file.rs` 99.20 %, `show.rs` 98.53 %. `prism-domain` **99.73 % lines**, 98.05 % regions, **100 % functions**, with the new `query.rs` and `sequence.rs` at **100 % on every column**. `prism-ipc` **97.69 % lines**, 97.13 % regions, 99.20 % functions. `prismd` **94.55 % lines**, 94.54 % regions, 96.00 % functions, with `main.rs` still 0 % and still the honest part of the figure |
| **The measured numbers from S24–S27 and S35 are still true** | ✅ *Zero React commits over 300 frames of 64 universes* is unchanged, with three more windows able to be on the screen — and it is what says the store preview is not telemetry. The frame budget was re-measured in Chromium against a real `prismd` on the 64-universe rig: **`64 universes · 30.3 Hz · paint 0.20 ms (p99 0.60 ms) · 158 frames`**, nothing lost. **All twenty-one end-to-end tests pass** — S25's, S26's, S27's, S44's and S35's among them, with S26's round trip measured this run at **`programmer 84 ms · output 151 ms`** |
| A test never touches a device | ✅ the output is `--mock-output` and there is no console in this session's end-to-end spec at all. The unit suite has no socket, no canvas and no clock of its own: `FakeNetwork`, `ManualTimer` and a hand-stepped scheduler, so *the newest answer is drawn and the older one dropped* is asserted rather than waited for |
| CI green on the pushed commit | ✅ run **32292211176** on `d98e948` — **all five jobs on the first attempt**: Windows full build and test 10 m 13 s, Linux neutral 3 m 12 s, UI end-to-end against a daemon 2 m 30 s, UI typecheck/lint/test/build 1 m 49 s, ARM64 cross-check 1 m 11 s. **All twenty-one end-to-end tests passed on a Linux runner in 58 s**, so a show was written from a rig that had none — a cue list made and put on an executor, a look stored, a cue corrected in place, the list fired, and a preset edited so that the *same Go* put a different level on the rig — on a machine that has never run this interface by hand: a `prismd` compiled there and a Chromium downloaded there. Both S24 gates were re-measured at the same time: **`64 universes · 30.0 Hz · paint 0.20 ms (p99 0.40 ms) · 154 frames · 2 lost`**, and S26's round trip came out **`programmer 117 ms · output 165 ms`** there against 84 ms / 151 ms here, on a debug daemon under a software rasteriser |

**What was built, in six pieces.**

**1. `CueProperty` — one field, not a whole cue.** The alternative is a command
carrying every editable field, which makes a client read the cue, change one
member and send the rest back: a read-modify-write over state the daemon owns,
and two operators editing two different columns would each undo the other. What
a cue *sets* is deliberately not among the fields, for the reason
`PatchFixture` carries no channels.

**2. `Show::relink` — the sentence in `prism-domain::preset`, made true.**
Storing a preset rewrites every cue part linked to it. In the show rather than
in the engine, because the tick may not resolve anything (§3.1) — so the value
in a cue is always the value that will be output, and the link is what keeps it
current. A part the new preset does not mention keeps both its value and its
link.

**3. `Query::StorePreview` — the third message shape, second caller.** What a
store would add, replace and **keep**, and which mode it would use. All four
from `ShowFile`, which is the only type holding the show, the programmer and the
store rule together. The counts come from the same builder the store runs, which
is the correction the first implementation needed.

**4. `show/looks.ts` — readers, and the fourth file of the kind.** After
`canvas/windows.ts`, `desk/session.ts` and `patch/patch.ts`. Its header carries
the two things that are deliberately *not* worked out here: what a store would
do, and which value a preset link resolves to — the second because the daemon
has already resolved it.

**5. Three windows, and what each is for.** Sequence Sheet is the *cue list* and
the one window that stores a look; Cue Viewer is the *cue*, watched rather than
edited; Preset Pool is the *pools*. Written into `ARCHITECTURE_SPEC.md` §6
beside S27's three, because the difference between them had never been written
down either.

**6. `AssignExecutor` and `CreateSequence` — the two commands that make the rest
reachable.** Before them a show could only be written by hand in a file
somewhere else: `StoreCue` needs a sequence to store into, and `ExecutorGo`
needs one on an executor to reach. Both are refused where a *create* would
destroy something, which is the rule that keeps them safe on a running desk.

**Five mutation checks.** Counting the merged cue rather than what the store
writes — the implementation this session actually had first — turns
`a_preset_preview_counts_only_the_values_of_its_pool` red, and it is the case an
operator would have been misled by. Spelling `"Merge"` in the client instead of
rendering `preview.mode` turns nothing red today and is what
`merge_is_the_only_store_mode_this_build_has` exists to catch tomorrow. Removing
`Show::relink` turns four tests red across two languages and takes the
end-to-end light test with them. Sending `SetCueProperty` under the number being
*typed* rather than the number the row started at turns the renumber test red.
Letting `assign_executor(id, None)` create a slot turns S14's property test red,
which is how the defect was found in the first place.

**One fixture pair was added and both are written by Rust.**
`ui/tests/fixtures/show-recording.json` and `show-rig.prism`, by
`cargo test -p prismd --test ui_show -- --ignored`, opened by eight non-ignored
Rust tests on every commit. **The other five recordings were regenerated and the
diffs read**: the only difference is the `tickHz` a snapshot carries, which is a
live measurement rather than a shape, so they were left as they were. No message
this session added changes an existing encoding — the five commands and the
query variant are new arms of internally tagged enums.


### 2.32 S34 verification record

Measured on 2026-08-20, all exit criteria from `IMPLEMENTATION_PLAN.md` S34 and
the session prompt. The session that closes the two gaps S26 wrote down rather
than invented: an executor's buttons do what the show says they do, and the tick
says which cue is running.

| Check | Result |
|---|---|
| **Every one of the eight `ExecutorButtonFunction` values does what its name says** — on **frames**, not on state | ✅ `prismd::core`'s two frame tests, against a real engine thread and a `MockOutput` whose last frame is read channel by channel. `On` starts the list at cue 1 (channel 5 → 255) and **a second `On` does not restart it** — the test steps to cue 2 first and watches 78 stay put; `Go+` and `Go-` walk it; `Off` stops it and the light goes; `Flash` held is 255 and released is 0; `Toggle` is on, then off, from the same key; `LearnSpeed` is accepted and moves no value; `Empty` produces nothing at all and is **not** a refusal. A release of any of the seven non-momentary functions produces nothing. The rig is `prismd::testkit`'s, whose channel 5 is **dark at home**, so a level on it is one the test put there |
| **A `Flash` leaves the stored master byte-identical, and one held across a `SetExecutorMaster` does not lose the new value** | ✅ asserted at three levels, because they are three different claims. In the engine (`body.rs`): a quarter master reads 63 on the wire, the flash reads 255, the stored master is still `16_383`, a `SetExecutorLevel` to `49_151` arrives **during** the flash and the light does not move, and the release reads **191** — the new level, not the old. In the daemon (`core.rs`): the same over the whole `Executor` the show holds, compared as a value before and after. In a browser (`ui/e2e/executors.spec.ts`): the strip's fader reads `0%` before, during and after, and a `page.reload()` asks the daemon and gets `0%` again. **A flash that wrote into the master would pass the light half of every one of these and fail the master half** |
| **`Toggle` on an executor a second client has just started stops it** | ✅ `a_toggle_stops_an_executor_a_second_client_started`: the other client's `ExecutorGo` is applied through the same `Core` — which is exactly what a second WebSocket produces — the light comes up, the desk hears from the tick that it is running, and the **first** press of the toggle takes it down again. A client that resolved `Toggle` for itself would have sent a start, because it had never pressed anything. S26's mutation check (resolving `Toggle` to a Go in the interface) is still in `desk.test.tsx` and still red under that mutation |
| **`currentCueIndex` is filled while a sequence runs, and the two absence assertions are inverted** | ✅ `prism_engine::PlaybackReport` is the channel, and the claim asserted is stronger than *a number appears*: **a strip that is running is on a cue, one that is not is on none, and the number moves.** `ui_show.rs::the_cue_index_is_a_number_now` and `ui_programmer.rs` say it over the regenerated recordings; `looks.test.ts` says it in the browser; `core.rs` watches it arrive and go away against a live tick. All three were written by earlier sessions to demand the opposite, with a message telling whoever built the channel to turn them round |
| **§4.1's three rows resolve to real commands, and the deviations block is empty** | ✅ `deviationsFromSection41` in `profiles/surface/xtouch.json` is `[]`, and what it used to say is in `deviationsClosedInS34` beside it. The strip buttons bind the **position** (`Slot 0..3`) and the executor decides — which is what §4.1's *configurable* column always meant, and all eight functions are now reachable through it. Play sends `On` and Stop sends `Off` as named functions, which is what the profile is allowed to do. The main fader is still one `SetExecutorMaster` and the daemon routes it through `faderFunction`, so `XFade` crossfades. §4.2.1 is rewritten as history, with the three-way table of what each function needed and where it lives |
| **Zero allocations in the tick, re-measured** | ✅ **0 allocator calls** in 1 000 ticks with the readback publishing and all four of S34's commands in rotation — flash, speed, tap and crossfade — on top of 8 cue lists fading over 768 slots, the programmer filling and clearing, and the grand master moving (`tick_allocations.rs`, a sixth measurement added beside the five that were there). Asserted afterwards that eight playbacks were reported, every one on a cue, and the frame was not the home layer — so it is not a measurement of a rig at rest. The other five are unchanged and still zero |
| **The 44 Hz stability from S6 holds** | ✅ **26 401 of 26 401 ticks, 0 missed, 0 panics**, p99.9 = **600 µs**, p50 100 µs, p99 200 µs, max 3.55 ms, over 600.02 s at 64 universes and 79 616 commands, with the accumulated clock, the flash layer and the readback all in the pipeline. The gate is p99.9 < 2 ms. **The first run of this failed and the reason was the harness, not the code:** `cargo test` gives the process no priority elevation, and the machine was compiling and running other suites at the time — 816 ticks missed, p99.9 187 ms. Re-run in the configuration `realtime.rs` documents (release exe at High priority, one normal-priority burner per core, nothing else on the machine) it is the figure above. §3's warning about reading a busy machine as a code regression earned its place again |
| **The two telemetry numbers from S24 are still true** | ✅ both. *Zero React commits over 300 frames of 64 universes* is unchanged and runs on every commit. *Under 8 ms* was re-measured in Chromium against a real `prismd` on the 64-universe rig: **`64 universes · 30.4 Hz · paint 0.20 ms (p99 0.60 ms) · 164 frames`**, nothing lost and nothing dropped |
| **All the end-to-end tests are green** | ✅ **23 of 23**, S28's twenty-one among them, plus this session's two: a flash held and released in a browser against a real daemon, and a toggle latching with the cue number arriving from the tick — the second half of that one driven **from the console**, three MIDI bytes appended to a file by neither the browser nor the daemon, so the number cannot be a count of anything a client sent |
| `cargo test --workspace` | ✅ **1 604 passed, 20 ignored**, up from S28's 1 562 — 42 new tests. 11 in `prism-engine`'s player (speed, the tap, the crossfade, the flash), 6 in its body and readback, 1 in `tick_allocations`, 5 in `prism-core`'s command application, 2 in `prism-domain`, 2 in `prism-surface`, 5 in `prismd`, and 9 in the browser. The forty-second came after CI was green: reading the readback back showed that the report's length was published after its entries in *both* directions, which leaves a reader one poll in which a shrinking executor grid still looks longer than it is — see §7 |
| `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ both exit 0 |
| **Coverage on `prism-engine`** | ✅ **99.42 % lines**, 99.40 % regions, 98.87 % functions — the crate the *> 95 %* rule is really about, and it is up on itself in the files this session wrote: `readback.rs` **100 % lines**, `player.rs` 99.57 %, `body.rs` 99.47 %, `playback.rs` 99.35 %, `command.rs` 97.89 %. `encode.rs`, `merge.rs`, `clock.rs`, `stats.rs` and `cue.rs` still at **100 % lines**. Against S6's 99.61 % on a crate two thousand lines smaller |
| **Coverage on `prism-core`** | ✅ **98.97 % lines**, 97.73 % regions, 97.83 % functions. `command.rs` — where the eight functions and the four fader functions are resolved — reads **100 % lines**, and it took a change to get there: the `Empty` arm of the button match was dead, because an earlier `if` had already returned. Removed rather than covered, which is what S19 and S21 did with the same finding; the match now handles `Empty` itself and `require_playable` runs after it, so *a key with nothing on it* is an answer that does not depend on whether the executor has a sequence |
| Coverage on the other crates, all of which changed | ✅ `prism-domain` **99.74 % lines**, 98.08 % regions, **100 % functions** — `executor.rs` at **100 % on every column**, which needed one more test: an executor document written **before** `speed` existed, read back at unity. `prism-surface` **99.24 % lines** with `binding.rs` at 99.41 %. `prism-ipc` **97.66 % lines**. `prismd` **94.96 % lines**, `core.rs` 97.06 % with the readback poll in it, and `main.rs` still 0 % and still the honest part of the figure |
| **Coverage on `ui`** | ✅ **99.05 % lines**, 93.80 % branches, 99.68 % functions, 99.08 % statements (`vitest run --coverage`; **535 tests in 43 files**). `src/desk` **99.76 % lines** with the rebuilt executor bar in it, `src/show` **99.64 %**. Unchanged to two decimal places against S28, on nine more tests |
| **`Query::StorePreview` is not asked per frame** | ✅ the warning S28 left, checked before the session closed — and it was real. The readback patches `/executors/<id>/currentCueIndex`, so the show document moves whenever a playback changes cue, and both store bars depended on the show *root*: a chase of instantaneous cues would have asked the daemon what a store would do forty-four times a second. They depend on the **subtree** a preview reads now (`looks.ts::sequencesDocument`, `presetsDocument`), which structural sharing keeps still. Asserted rather than argued: *does not ask again when a playback advances a cue* fires the list, steps it, and demands the same question count |
| The six recordings are a set, and all six were regenerated | ✅ and the diffs were read. Two shapes moved and both were meant to: every executor document gained `speed` (1 024), and `Delta::ExecutorState` now carries a cue index instead of `null`. Nothing else changed but `tickHz`, which is a measurement. **One finding came out of reading them:** three recorders stopped at the `Ack` and the readback arrives a poll later, so an `ExecutorState` landed in the *next* step while the snapshot beside it already had it — `ui_recording`'s replay caught it. All three settle now, and only a **delta** resets the window, because the daemon publishes telemetry thirty times a second and the first implementation waited for silence on the socket for ever |
| A test never touches a device | ✅ the output is `--mock-output` and the console is a file. The two new end-to-end tests use `--mock-surface`; nothing in this session opens a port |
| CI green on the pushed commit | ✅ run **32325628021** on `1a26d78` — **all five jobs on the first attempt**: Windows full build and test 9 m 47 s, Linux neutral 3 m 11 s, UI end-to-end against a daemon 2 m 14 s, UI typecheck/lint/test/build 2 m 05 s, ARM64 cross-check 31 s. The end-to-end job is the one worth reading: **23 tests, all green on a Linux runner**, and two of them are this session's — a flash held and released against a real daemon, and a toggle latching with the cue number arriving from the tick, the second half of it driven from a `--mock-surface` console. This session's own console measurement came out **`programmer 96 ms · output 146 ms`** there, and S24's frame budget was re-measured through it at the same time: **`64 universes · 30.3 Hz · paint 0.20 ms (p99 0.40 ms) · 155 frames · 1 lost`**. The one-comparison fix in §7 — the readback's length on the shrinking side — went out afterwards as `124bb2f` and is green on run **32326458934**, again all five jobs on the first attempt: Windows 9 m 30 s, Linux neutral 2 m 26 s, UI end-to-end 2 m 29 s, UI typecheck/lint/test/build 1 m 48 s, ARM64 check 52 s |

**What was built, in six pieces.**

**1. `prism_domain::ExecutorButtonRef` and `Command::ExecutorButton` — the
command that presses a button without deciding what it means.** A `Slot` is a
hardware position and the executor's `buttonFunctions` decide; a `Function` is a
row of the *profile*, which is the desk's own configuration written by a person
rather than a client's run-time reading. `pressed` carries both edges, because
`Flash` is momentary. The distinction between the two shapes is the session's
one real design question and it is answered in `docs/MCU_MAPPING.md` §4.2.1.

**2. `prism_core::Show::apply_executor_button` — the one place a function
becomes an effect.** Eight arms, and `Toggle` is the line the session exists
for: `is_active` is read *here*, where it is owned. Five new `Effect`s carry the
answers out — `ExecutorOn`, `ExecutorFlash`, `ExecutorSpeed`, `ExecutorTapSpeed`
and `ExecutorXFade`.

**3. `PlaybackSource::flash` — a layer, not a write.** The held level sits beside
the stored one and `docs/DMX_MERGE.md` §2.1's master term is the one in force.
`set_master` writes the stored level whatever is happening above it, which is
what makes a fader move during a flash survive the release.

**4. The speed master, and the clock that carries it.** `Executor::speed` in
units of `prism_domain::SPEED_UNITY`; `CuePlayer` accumulates a fraction of a
tick per tick slot and carries the remainder, so unity is exactly the arithmetic
that was there before and **all 280 of `prism-engine`'s existing tests passed
unchanged**. `LearnSpeed` taps it: two taps mean *this transition should take
that long*. `ExecutorFaderFunction::XFade` is the same transition with the fader
for a clock.

**5. `prism_engine::PlaybackReport` — the channel back out of the tick.** A fixed
table of atomic words, sized once, published at the end of every tick with a
relaxed store per entry and only where the entry changed. `prismd` samples it at
25 ms and broadcasts `Delta::ExecutorState` **only on a change**. It is now the
**only** author of `is_active` and `currentCueIndex`.

**6. The interface.** Four buttons that were drawn disabled with the reason on
them are pressed like the other four, with pointer capture so a finger sliding
off a `Flash` still releases it. The strip shows `Q1`; the cue sheet shows which
row the playback is standing on.

**Five mutation checks.** Writing the flash into the stored master instead of
beside it turns the byte-identical assertion red in three files. Resolving
`Toggle` against `is_active` in the interface turns S26's check red, still.
Advancing the player's clock by one per *call* rather than by the tick slots
that passed turns `a_missed_tick_moves_the_fade_by_the_time_it_really_took` red.
Publishing the report's length before its entries turns
`a_report_answers_what_was_published_and_nothing_beyond_it` red. Keeping the
daemon's optimistic `is_active` write beside the readback turns the cue-index
test red — which is how the second-author race was found in the first place.

### 2.33 S39 verification record

Measured on 2026-08-20, all exit criteria from `IMPLEMENTATION_PLAN.md` S39 and
the session prompt. The session that answers the question S28 raised and
deliberately did not answer: **what happens when you store on top of something
that is already there.**

| Check | Result |
|---|---|
| **Each store mode does what its name says against a cue that already exists** — asserted on the **stored cue** | ✅ `crates/prism-core/tests/store_modes.rs`, twenty-five tests, and every one of them reads the cue back out of the show rather than checking that the command was accepted. Against a cue holding a red on fixture 1 and a green on fixture 2: **Merge** replaces the red, adds a blue and leaves the green (`(1,Red) (1,Blue) (2,Green)`); **Override** leaves the cue holding `(1,Red) (1,Blue)` and nothing else; **Remove** leaves `(2,Green)` and does **not** use the level it was given. All three keep the cue's name and its fade, because a store is about the look. A Remove that empties a cue leaves an **empty cue** — deleting one is `DeleteCue`'s, and a store that took a number off a running order as a side effect would be a surprise |
| A refused store writes nothing | ✅ a `Remove` against a cue that is not there, and one that would remove nothing, are both refused and the show is **byte-identical** afterwards (`rmp_serde::to_vec_named` before and after). This is where the session's one real defect was: see below |
| **A preview of each mode is the store that follows it** | ✅ S28's rule extended to three modes, asserted twice. In `prism-core`: for each mode, the four counts are checked to account for every value filed under that number (`kept + replaced + removed` is the cue that is there) and the cue afterwards is compared against `added + replaced + kept`. In `prismd`'s recording: the same store previewed in all three modes back to back — Merge `(2,3,0,0)`, Override `(2,3,0,0)`, Remove `(0,0,0,3)` — which is the case where Merge and Override *coincide*, and then the pair where they do not: an Override that would throw **four** values away, and a Remove that takes one out and leaves four |
| **`StoreSequence { sequenceId, mode }` with Append, Override and Merge, and Append adds a cue at the highest number** | ✅ and *highest* is one past the highest **whole** number: a list reading `1`, `1.5` gains a `2`, so an operator pressing Append repeatedly gets `1`, `2`, `3` rather than a running order nobody would write by hand. Append into an empty list starts at `1`. Override leaves the sequence holding **one** cue, numbered `1`, with the sequence's name and loop flag intact. Merge writes the look into **every** cue and adds none, asserted cue by cue on the recording as well. A Merge into a cue list with no cues is refused rather than quietly appending, because appending would be a different command than the one that was sent |
| **The selected sequence is decided** | ✅ **decided the other way from S28's assumption**: `Session::selectedSequence` is a field of its own, with `Command::SelectSequence` — the sixteenth session command and the one of the four added since S12 that a *console* can issue (`Sequence 5`, S40's). The two reasons the executor reading cannot meet are the criterion's own: `Store Cue 5` typed with **no executor selected** has to mean something, and a cue list nobody has put on a fader has to be editable without occupying a playback slot to reach it. It is deliberately **not** coupled to `selectedExecutor` — an operator programming cue list 7 while executor 3 plays the show is the ordinary case — and the recording asserts that nothing but a `SelectSequence` ever moves it. `show/looks.ts::executorInForce` is the one reader that changed, exactly as S28 predicted, and it kept the executor half: the sheet follows the sequence, the transport line follows the executor, and the playback marker is drawn only where the two are looking at the same list |
| **`EditCue` loads every attribute and keeps every `presetRef`** | ✅ the session's easiest thing to break by accident, and it is asserted from both ends. In `prism-core`: a cue holding a value applied from preset 4 and one applied from nothing loads with the link on the first and no link on the second, both as `ProgrammerValueSource::Recalled` — which is what that variant has been waiting for since S1 — and the cue's fixtures become the selection so an encoder reaches them. In the recording: cue 3 was stored out of an applied preset, so **all six** of its parts carry a link (the whites included, because `White` is a *colour* attribute on an RGBW profile — S28's finding), and every one of them survives the round trip |
| **A cue loaded and updated with nothing changed is byte-identical** | ✅ taken literally: the cue is serialised with `rmp_serde::to_vec_named` before and after and the two `Vec<u8>` compared, which is the same standard `command_application.rs` holds a refusal to. Asserted in `prism-core` on a cue carrying a name and a fade as well as its values, and in `prismd`'s recording on the cue the *daemon* ended up holding |
| **The update state, and `Command::Update`** | ✅ `Session::editingCue` is `{ sequenceId, cueNumber, modified }` — which cue the programmer is editing and whether it has moved since, which is exactly what an Update key blinks on. `Update` carries **nothing at all**, because both of the things it needs are the desk's: which cue, and that the mode is Override. An Update that merged could never take a value *out* of the cue it is updating, which is why an operator loads one in the first place — asserted by loading a cue, dropping one of its values from the programmer, and watching the stored cue lose it |
| **It clears on a Clear, on a delete and on another load** — all three | ✅ `the_update_state_clears_on_a_clear_a_delete_and_another_load`, one test with the three cases in it, plus the two neighbours that make each of them mean something: deleting a *different* cue leaves it standing, and loading another cue **replaces** the edit and starts it unmodified. A fourth rule was found rather than asked for and it is the one that matters: **a renumber of the cue being edited carries it**, because an `Update` stores in Override mode into the number it remembers — so clearing there would let an operator correct `1` to `1.5`, press Update, and end up with both |
| **Oops takes back a store and an update, and S14's property test still holds** | ✅ unchanged and still green over all **forty** commands, which is what caught the defect below. An Override store, an `EditCue` and an `Update` are each taken back on the show's bytes; the `EditCue`'s undo also puts `editingCue` back to `null`, because an Oops that restored the programmer and left the desk claiming to be editing cue 3 would restore half a state — the same argument S13 made for the jog wheel's parameter index, one field along. A `DeleteCue` that cleared the update state puts it back when it is undone |
| **Coverage on `prism-core`** | ✅ **98.93 % lines**, 97.70 % regions, 97.77 % functions — against S34's 98.97 % on a crate 160 lines smaller. `command.rs`, `mirror.rs` and `testkit.rs` at **100 % lines**; `file.rs` **99.47 %**, `session.rs` 99.46 %, `journal.rs` 99.54 %, `programmer.rs` **98.81 %**, `show.rs` 98.56 % |
| Coverage on the other crates that changed | ✅ `prism-domain` **99.61 % lines**, 97.99 % regions, 99.53 % functions, with the new `query.rs`, `command.rs` and `session.rs` at **100 % on every column**. `prism-ipc` **97.63 % lines**. `prismd` **94.90 % lines**, with `main.rs` still 0 % and still the honest part of the figure |
| **Coverage on `ui`** | ✅ **99.05 % lines**, 93.78 % branches, **99.68 % functions**, 99.09 % statements (`vitest run --coverage`; **542 tests in 43 files**, up from 535). `src/show` reads **99.68 % lines** with `cueviewer.tsx`, `presetpool.tsx`, `sequencesheet.tsx` and the new `storemode.tsx` at **100 % lines** |
| `cargo test --workspace` | ✅ **1 645 passed, 20 ignored**, up from S34's 1 604 — 41 new tests. 25 in `prism-core`'s new `tests/store_modes.rs`, 4 in its programmer module, 1 in its journal, 4 in `prism-domain`, 5 in `prismd`'s `ui_show.rs`, and 2 in the browser beside them |
| `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ both exit 0 |
| `npx tsc -b --force` with `strict: true`, **no `any`** | ✅ exit 0, and `any` appears nowhere in `ui/src` or `ui/e2e`. The new chooser narrows the string it reads off the `<select>` against the generated `STORE_MODE_VARIANTS` rather than asserting it, and `readStorePreview` still refuses a `StoreMode` this build does not know |
| `npm run build`, `npm run lint`, `npm run test` | ✅ all three exit 0. **306 kB (93 kB gzipped)**, up 1 kB on S28 — a `<select>`, a button per cue row and one more button on the store bar — and this session adds no dependency either. `oxlint` reports nothing — it did report one thing, and it was right: `isStoreMode` exported from a file that also exports a component costs React's fast refresh, so it is private there and `ipc/protocol.ts` keeps its own. **542 tests in 43 files** |
| **The two tests written to go red are turned round** | ✅ both, and neither was deleted. `prism-domain`'s `merge_is_the_only_store_mode_this_build_has` is now `the_three_store_modes_are_on_the_wire`, asserting all three spellings and that Merge is still the **default** — which has to be the mode that cannot lose anything. The browser's *every recorded preview is a Merge* now demands that the recording asked in **all three**, that a Remove's counts are about taking away rather than writing, and that no Merge in it reports anything removed. Each carries the paragraph saying what it used to say and why it was written that way |
| **The Store button says the word the operator chose** | ✅ and it is still `preview.mode` rather than a word the client spells, which matters *more* now rather than less: the chooser moves the **question**, the answer echoes the mode it was asked about, and the counts beside the word are what *that* mode would cost. Asserted by answering an Override question while the chooser reads Merge and demanding the button say **Override** — which is the state a client is in for one round trip after every change, and the one way this could still mislead |
| **`Query::StorePreview` is still not asked per frame** | ✅ S34's check, re-run with the mode as a fourth dependency: *does not ask again when a playback advances a cue* fires a cue list, steps it, and demands the same question count. The mode is a dependency of the effect, so choosing one asks again — which is the point — and nothing else was added to the list |
| **The measured numbers from S24–S28 and S34 are still true** | ✅ all four. *Zero React commits over 300 frames of 64 universes* is unchanged and runs on every commit. **Zero allocator calls** in the tick, re-run over all eight `tick_allocations.rs` measurements. The frame budget was re-measured in Chromium against a real `prismd` on the 64-universe rig: **`64 universes · 30.1 Hz · paint 0.20 ms (p99 0.70 ms) · 153 frames · 1 lost`**. And **all twenty-three end-to-end tests pass**, plus this session's twenty-fourth |
| A test never touches a device | ✅ the output is `--mock-output` and there is no console in this session's end-to-end spec at all. The `prism-core` suite has no socket, no clock and no disk |
| The six recordings are a set, and all six were regenerated | ✅ and the diffs were read. Two shapes moved and both were meant to: every session document gained `selectedSequence` and `editingCue`, and `StoreCue` / `StorePreset` gained a `mode`. Compared structurally rather than as base64: `desk-recording`, `patch-recording` and `session-recording` differ **only** in their two snapshot payloads, `telemetry-recording` only in its frame payloads, and `daemon-recording` in its snapshots, its commands and the deltas of the three cases that send a store — which is the whole of what changed and nothing else. `show-recording` is regenerated from a script that grew from 43 steps to **59** |
| CI green on the pushed commit | **The first push was red, in three places, and none of them was the session's core.** Run **32408327112** on `f8fa3f8`: (1) Rust 1.98.0 landed on CI's `stable` between S34 and this push and brought `clippy::chunks_exact_to_as_chunks`, which failed the Windows job on six sites in `prism-engine` untouched since S2; (2) `a_process_killed_mid_write_leaves_the_last_committed_state` took all six of its fixed kill delays *between* saves and asserted, correctly, that it had proved nothing; (3) **one real regression from this session** — the store bar gained a mode chooser, wrapped at a Linux runner's font metrics, squeezed `.sheet-scroll` to nothing and left cue 1 under the sticky header, where `looks.spec.ts` could not click it. All three are fixed and all three are in the decision log; the third was reproduced at 1280 × 560 and mutation-checked |
| **CI green** | ✅ run **32412552879** on `191ce14` — **all five jobs on the first attempt**: Windows full build and test 12 m 50 s, Linux neutral 3 m 25 s, UI end-to-end against a daemon 3 m 26 s, UI typecheck/lint/test/build 2 m 03 s, ARM64 cross-check 41 s. The end-to-end job is the one worth reading: **24 tests, all green on a Linux runner**, and one of them is this session's — an operator choosing Merge, then Override, then Remove over one cue number, reading a different sentence off the button each time in the daemon's own words, and then loading a cue back into the programmer and putting it down with the Update key. S26's console round trip came out **`programmer 95 ms · output 147 ms`** there, and S24's frame budget was re-measured through it at the same time: **`64 universes · 30.2 Hz · paint 0.20 ms (p99 0.60 ms) · 154 frames · 1 lost`** |

**What was built, in six pieces.**

**1. `StoreMode` grew its other two values, and the mode moved into the
command.** S28 shipped a one-variant enum with the argument in its own
documentation: the daemon answered with the mode it would actually use, because
`prism_core::Programmer` merged unconditionally and a client carrying a mode the
daemon did not honour would be describing an outcome that did not happen. Now
the operator chooses, so the choice travels in `StoreCue`, in `StorePreset` and
in `Query::StorePreview` — and the answer **echoes** the mode it was asked
about, which is what keeps a bar from drawing one mode's word over another
mode's numbers.

**2. `StorePreview::removed` — the fourth count, and the one that is a
warning.** The four counts now partition every value on both sides for every
mode, which is written as a table in the type's own documentation and asserted
as a sum in three places. `removed` is zero under Merge, which is why S28 could
ship without it; under Override it is what nobody asked to lose.

**3. `Command::StoreSequence` — the same three names one level up.** A cue store
is about values and a sequence store is about cues, so it is a different command
with a different enum: Append, Override, Merge. It has **no preview**, and that
is a decision rather than an omission — the three counts are about the values of
one cue, and an honest answer about a cue *list* needs a different shape. See
§7.

**4. `Command::EditCue` and `Command::Update` — the round trip.** `EditCue`
fills the programmer from a stored cue with every `presetRef` kept; `Update`
puts it back in Override mode and carries nothing, because both of the things it
needs are the desk's. `Update` is *resolved into* a `StoreCue` inside
`ShowFile::finish_programmer` rather than implemented twice, which is also what
makes the update state's one rule — an Override store into the cue being edited
stops the key blinking — true of both of them without being written down twice.

**5. `Session::selectedSequence` and `Session::editingCue` — §4.1 gained two
fields.** The first is S28's marked assumption, settled the other way; the
second is the update state, and it is session state because every attached
client has to blink the same key. Both are `#[serde(default)]`, which is how an
opaque-document schema grows (S34's finding, met again within a minute).

**6. The interface: a mode chooser, an Edit key per cue and a blinking
Update.** The chooser is a `<select>` over the generated `STORE_MODE_VARIANTS`,
so a fourth mode would appear without the file being edited. Choosing one asks
the question again — the two halves have to move together. The blink is
`editingCue.modified` and not a timer this window keeps, so two screens blink
together, and `prefers-reduced-motion` gets the colour without the flash.

**Five mutation checks.** Spelling the chosen mode on the button instead of
rendering `preview.mode` turns *puts the daemon's word on the button* red.
Leaving the mode out of the store bar's effect dependencies turns *asks again in
the mode the operator chose* red. Clearing the update state on a renumber
instead of following it turns `renumbering_the_cue_being_edited_follows_it` red,
with the cue recreated at its old number. Dropping `preset_ref` in
`Programmer::load_cue` turns four assertions red across two languages. And
re-deriving a store through `Programmer::apply` after the show has been written
turns the recording's replay red — which is how the one real defect in the batch
was found, and it is the next paragraph.

**The defect, and what found it.** A `StoreMode::Remove` was **applied to the
show and then refused**. `ShowFile::finish_programmer` writes the cue first and
then routes the command through `Programmer::apply`, which re-derives the store
as a guard for a direct caller — and until this session that cost nothing,
because merging twice answers the same both times. A Remove that has been
applied has taken its values out, so the second derivation answers *there is
nothing to remove*, the command is refused, and the show has already moved. It
was found by `crates/prismd/tests/ui_show.rs`, whose replay of the recorded
deltas and whose per-step snapshot disagreed about a cue — neither of which
would have noticed a store that merely returned the wrong thing. The fix is
`Programmer::stored`, which does the Clear-stage half and nothing else, and the
guard in `apply` stays for the direct caller it was written for.

### 2.34 S40 verification record

Measured on 2026-08-21, all exit criteria from `IMPLEMENTATION_PLAN.md` S40 and
the session prompt. The session that decided **what the interface is**: not a
screen with a command line on it, but a command line with a screen in front of
it.

| Check | Result |
|---|---|
| **A key on the desk writes a word into the line and does not act** | ✅ the decision, and it is asserted three ways. `ui/src/desk/shell.tsx` is the one place a gesture becomes a command, and it has three shapes and no fourth: `run` writes and executes, `write` writes and waits, `append` adds a word to the line as it stands. `shell.test.tsx` presses one of each and reads what went out on the socket — `Store` sends **nothing at all** and leaves `Store ` in the input. `console.spec.ts` says it against a real daemon and on the daemon's own field: pressing `Fixture` leaves `Fixture ` in `Session::commandLine`, which is what a second screen would draw, and no `Command` follows it. Every window's keys go through the same three functions: the group pool, the preset pool, the sequence sheet, the cue viewer and the view bar |
| **Every line in the S40 vocabulary works against a real daemon** | ✅ `ui/e2e/console.spec.ts` builds a show by typing and nothing else: `1 thru 3`, `Store Group 1 "Front wash"`, `Label Group 1`, `Copy Group 1 Group 2`, `Delete Group 2`, `Group 1`, `red at 100`, `Store Sequence 1 "Act 1"`, `Sequence 1`, `Store Cue 1`, `Label Cue 1 "Opening"`, `Move Cue 1 Cue 5`, `Copy Cue 5 Cue 6`, `Delete Cue 6`, `Assign Sequence 1 Executor 0`, `Go+ Executor 0`, `Off Executor 0`, `Move Executor 0 Executor 3`, `Delete Executor 3` — each one read back off the window that draws the document it changed, not off the interface's own state |
| **Every typed line produces the commands a daemon accepted for it** | ✅ `ui/tests/fixtures/desk-recording.json` carries **44 typed lines** beside the payloads a real `prismd` answered, and `crates/prismd/tests/ui_programmer.rs` asserts on every commit that those payloads are the commands the script names. `console.test.ts` decodes the recorded bytes and compares the **parser's** answer against them, so nothing in TypeScript decides what a line ought to mean |
| **The parser never throws** | ✅ S26's rule kept over a generated corpus, and 30 named refusals beside it: every bad line is answered with the word that was wrong and a line that would work (`"x" is not a sequence number.`, `assign it where? Try "assign sequence 5 executor 1".`). `console.ts` reads **99.72 % lines**, and the one uncovered line is the `default` arm of a match the line above has already made unreachable |
| **A prompt does not block the desk** | ✅ and it is asserted as a gesture rather than as a claim. The question lives in the footer beside the line it came from — **not a modal, and nothing over the canvas** — so `console.spec.ts` opens a window on the canvas *while it stands*, draws the show in it, and finds the question still there. Escape cancels and the cue is what it was: `cue-parts-1` reads `3` before the question, `3` while it stands, and `3` after the cancel; answering `Override` makes it `1` |
| **The mode travels in the command** | ✅ S39's rule kept through a new path. `applyMode` puts the operator's choice into the one command of the line that carries one, and it takes only the words that command's type accepts: a `StoreCue` takes `Remove`, a `StoreSequence` takes `Append`, a `Copy` takes `Merge` and `Override`, and a mode that does not belong is left alone rather than forced in |
| **The blinking Update is `Session::editingCue`** | ✅ unchanged from S39 and still the daemon's answer rather than a derivation — the keypad's `Update` key is one `run("Update")`, so it is a line like everything else |
| **Completion and history are the client's, the line is not** | ✅ `Session::commandLine` is §4.1 state and travels; `desk/history.ts` is a ring of what *this* operator typed and never leaves the browser. The split is the same one §4.2 makes for a canvas: what an operator is part-way through saying is shared, what they said an hour ago is theirs |
| **Every new command is undoable, or deliberately not** | ✅ and S14's property test still holds over all **43** variants. `Delete`, `Copy`, `Move` and `Label` are undoable when they name a show object and not when they name a **view** — the same §6.1 rule applied through a payload instead of a variant, held on the show's bytes by `crates/prism-core/tests/objects.rs`. `Goto` and `ExecutorOn` are deliberately not undoable, for the reason the playback commands beside them are not: they change light somebody is driving. Their **scope** is wider than the two objects they name — a moved sequence images every executor that played it, a moved preset images every cue that linked to it |
| **A refusal changes nothing**, taken literally | ✅ `rmp_serde::to_vec_named` before and after, which is the S11 standard. It caught the session's one real defect: `StoreSequence` created the cue list and *then* stored into it, so a refused store left a half-made list behind. It builds the sequence beside the show and writes once |
| **A readme explaining the command structure** | ✅ [`docs/COMMAND_LINE.md`](docs/COMMAND_LINE.md): the grammar, the three shapes, every line in a table, what the prompt asks and why it is not a modal |
| `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ all four clean. **46 test files, 615 tests**, 0 failed (S39: 533). The bundle is **326.25 kB / 98.11 kB gzipped** |
| `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ all three exit 0 — **1 721 passed, 0 failed, 20 ignored across 60 targets** (S39: 1 666). Clippy found one thing worth recording: an `unused_qualifications` in `prism-engine/src/player.rs` that only appears when the crate is rebuilt from cold, which is why a green *incremental* run is not the gate |
| **All 24 e2e tests still green, plus the new ones** | ✅ **28 passed** in 2.0 min: the 24 that existed and `console.spec.ts`'s four. Two of the 24 had to change and both changes are the S40 decision showing up where it should — the sheet's *New sequence* key writes **one line** now (`Store Sequence 1`) where it used to send three commands, so putting the list on a fader is `Assign Sequence 1 Executor 3`, a line an operator could type |
| **Coverage on `prism-core`** | ✅ **98.73 % lines**, 97.43 % regions, 97.97 % functions. The new `objects.rs` reads **98.03 % lines**, 96.59 % regions and **100 % functions**; `command.rs` — where the four verbs and the two playback targets are resolved — **99.50 % lines**; `show.rs` 99.03 %, `file.rs` 99.41 %, `session.rs` 98.86 %, `journal.rs` 98.67 % |
| **Coverage on `prism-domain` and `prism-engine`** | ✅ `prism-domain` **99.59 % lines**, 98.23 % regions, **100 % functions**, with the new `playback.rs` at **100 % on every column** and `command.rs` at 99.17 %. `prism-engine` **99.31 % lines**, 99.31 % regions, 98.89 % functions — the crate the *> 95 %* rule is really about — with `playback.rs`, `player.rs`, `cue.rs`, `merge.rs`, `encode.rs`, `master.rs`, `readback.rs`, `clock.rs` and `stats.rs` all at or above 99.3 % |
| **Coverage on `ui`** | ✅ **98.95 % lines**, 93.98 % branches, **99.72 % functions**, 98.98 % statements over `ui/src`. The new files: `exists.ts` and `grouppool.tsx` at **100 % lines**, `console.ts` 99.72 %, `shell.tsx` 97.77 %, `consoleshell.ts` 93.33 %. The uncovered lines were read rather than counted and each is a guard the interface cannot reach: `useConsole` outside its provider, `runWithMode` on a line that does not parse, and answering a question that is not being asked |
| **Coverage, whole workspace** | ✅ **98.34 % lines**, 97.71 % regions, 98.17 % functions across all eight crates; `prism-ipc` 97.66 %, `prism-surface` 99.27 %, `prismd` 94.74 % with `main.rs` still 0 % and still the honest part of the figure |
| **Zero allocations inside the tick** | ✅ re-measured after the engine was rekeyed from `ExecutorId` to `PlaybackId`, which is the change that could have cost it: **0 allocator calls** on all seven paths — a warm tick over 64 universes and 4 subscribers, the merge over 8 sources, the encoder over 768 slots, 8 cue lists fading, the programmer and the masters, the readback with S34's commands, and 44 ticks on the real system clock |
| **The tick still holds its deadline** | ✅ 64 universes, 4 drivers, 3 s: **133 ticks of 133 expected, 0 missed, 0 panics**; jitter p50 100 µs, p99 800 µs, p99.9 1.3 ms, max **1.244 ms**. Whole pipeline, 8 universes: p99.9 100 µs, max **22.6 µs**. Machine at 22–30 % background load, checked before the run |
| **The interface still costs nothing to draw** | ✅ **0 React commits** over the telemetry stream, and in the browser: 64 universes at **30.3 Hz**, paint **0.10 ms** mean and **0.30 ms** p99 over 155 frames — the same order as S24's measurement on a canvas with less on it |
| **The six recordings are a set** | ✅ all six regenerated and the diffs read. `patch-recording.json` and `telemetry-recording.json` differ **only in `tickHz`**, which is what a pure regeneration looks like; `daemon-recording.json` shows the protocol change and nothing else (`ExecutorGo` now carries a `target`); `show-recording.json` gained the `selectedSequence` deltas the new store writes; `desk-recording.json` grew from 23 steps to **54**, of which 44 are typed lines and 4 are refusals |
| **No test touches a device** | ✅ every daemon in the suite runs `--mock-output --mock-surface`, unchanged |
| **CI** | ✅ all five jobs green on run **32510091387**: UI typecheck/lint/test/build, Linux platform-neutral crates, UI end-to-end against a daemon, Linux ARM64 cross-compile, and Windows full build and test. The run before it (**32508350504**) failed three e2e tests and found the defect in the row above — recorded rather than amended away, because a CI-only failure that this machine cannot reproduce is exactly the kind worth being able to look up |
| **The one defect CI found and this machine could not** | ✅ fixed, with a regression test that goes red without the fix. `Session::commandLine` is §4.1 state, so every keystroke is mirrored to the daemon and comes back as a session delta — and by the time it comes back the operator has typed more. The input adopted that echo and went back to a **prefix** of the line. Locally the round trip lands between keystrokes and it is never seen; on a loaded CI runner a whole level line disappeared and the store after it was refused for an empty programmer, in three e2e tests at once. Every line this client sends is now held until its echo returns, and while one is outstanding the session's line is an older opinion about a line we are still writing |

**The two structural questions this session had to answer** were put to the
operator rather than decided quietly, and both are recorded here because the
reasons outlive the session.

**Where does a playback addressed to a *sequence* live?** In the **engine**, as a
playback of its own. `PlaybackId` is either an executor or a sequence, and
`Show::playback_of` resolves a `PlaybackTarget` to exactly one of them —
preferring the executor when one holds the list, so the two are never both live.
Two players of one cue list would fight over the same slots in the merge and the
operator would see a cue that would not fade. The alternative — having the daemon
assign a free executor behind the operator's back — was rejected because it
spends a numbered thing the operator can see, to answer a question they did not
ask.

**One command per verb, or one per pool?** One per verb, over an `ObjectRef`.
`Delete`, `Copy`, `Move` and `Label` across six pools is twenty-four commands
written the other way, and the narrow commands that already existed
(`DeleteView`, `RenameView`, `MoveView`, `DeleteCue`, `CreateSequence`) were
folded into them rather than left beside them — two vocabularies for one act is
what **D11** forbids. The cost is that *which applier owns a command* is now a
property of its payload: `Command::is_session_command` reads the target, because
a view is session state and the other five are the show's.

**The third question was answered inside the session** and is here for the same
reason: **where does `Store Preset 1` get its pool?** From the encoder bank the
operator is looking at, which is `Session::encoderBank` and therefore travels —
and the grammar takes an optional pool word beside it (`Store Preset 2 Beam
"Tight"`) for a line typed with the encoders showing something else. What the
pool tab of the Preset Pool window is remains **client-local**: §4.2 says a
window's own scroll position and tab legitimately differ per screen, and a second
operator reading colours while the first reads beams is the ordinary case.

### 2.35 S33 verification record

Measured on 2026-08-22, all exit criteria from `IMPLEMENTATION_PLAN.md` S33 and
the session prompt. The session that turned the rig from **command-line flags
into data**, and put that data where a show cannot carry it away.

| Check | Result |
|---|---|
| **The worked example, configured and asserted** | ✅ `crates/prismd/tests/outputs.rs::twelve_universes_across_five_outputs_each_get_exactly_their_own`. Twelve universes across five outputs of three kinds — two Open DMX adapters on 1 and 2, an sACN gateway on 3 and 4, an Art-Net node on 5–8 and another on 9–12 — built **entirely from commands** against a running daemon, with no flag anywhere in it. Each of the five is asked what it was handed, off `MockOutputHandle::frames`, and the answer is exactly `[1]`, `[2]`, `[3, 4]`, `[5, 6, 7, 8]`, `[9, 10, 11, 12]`. Not one universe reached an output it was not given |
| **A universe sent to two outputs arrives byte-identical at both** | ✅ two mock outputs on universe 3, four frames each, and the last 512 bytes compared byte for byte — 255 in channel 1, and the two vectors equal. The publisher fans the frame out rather than sharing it, so a driver that mutated one would be visible here |
| **Added mid-show, removed mid-show, and neither costs a tick** | ✅ `adding_and_removing_an_output_mid_show_costs_the_others_nothing`, asserted the way S18 asserts the D2 gate: on the recorded frame sequence rather than by watching. The output that did not change has **44+ frames across the whole reconfiguration and no gap over 250 ms** — S18's threshold, against a 23 ms cadence. The arriving output sends within a refresh interval; the removed one's count does not move again after 200 ms |
| **An output whose device disappears degrades alone** | ✅ `an_output_whose_device_disappears_degrades_alone`. Output 3 of five is told to fail twenty sends **and then to panic three times** — the fault `CLAUDE.md`'s zero-crash invariant is really about. Over the next 300 ms the other four all sent more frames than before, the engine's tick count moved, and **`missed()` is 0**: the engine never heard about it |
| **A patched universe with no output is reported** | ✅ `prism_core::ShowIssue::UniverseNotOutput`, and it is a report rather than a refusal at three levels: the show still patches it, the daemon still starts, and the operator is told. A twelve-universe show on a desk with no rig names all twelve; the worked example silences all twelve; disabling the sACN gateway names **3 and 4** again and answers the `SetOutputEnabled` with two `Delta::Notice`s naming them |
| **The configuration survives a restart** | ✅ the rig is written to `machine.json` when a command changes it, and a second `Daemon::start` over the same data directory finds five outputs, `Bridge node` at index 3 still carrying `[5, 6, 7, 8]`, and all five drivers running |
| **A show copied to a second machine arrives with no output configuration** | ✅ asserted on **bytes**, like `desk_id_is_not_show_content`. The `.prism` file is read back raw and must not contain `Stage left dimmers`, `Hall gateway`, `Bridge node`, `127.0.0.1` or `B0037HIY`; `machine.json` beside it contains all of them. `crates/prism-core/tests/outputs.rs` says it again one layer down, over two saves, and reads the reopened show's MessagePack to be sure the second save is not the one that leaks |
| **Coverage on the new code > 95 %** | ✅ measured after `cargo llvm-cov clean --workspace` **before every crate**, one at a time, because two runs sharing a target directory measure each other — the first attempt at this read `prism-engine` at 21 % for exactly that reason. The four new files: `prism-domain/output.rs` **100 % on lines, regions and functions**, `prism-domain/socket.rs` **100 % lines**, `prism-core/outputs.rs` **99.80 % lines / 99.85 % regions / 100 % functions**, `prismd/outputs.rs` **97.72 % lines / 98.20 % regions**. The one file S33 rewrote from the inside, `prism-engine/triple_buffer.rs`, reads **98.28 % lines**. §3 has the crate table |

**The rig is data, and the data belongs to the building.** `OutputInstance` is a
number, a name, a kind with its parameters, the universes it carries and whether
it is enabled; it lives in `prism_core::MachineConfig` beside the desk identity
and **never in a `.prism` file**. That is `prism_core::desk`'s argument for the
sACN CID with one more beside it — a rig is a property of the *building*, and the
school's template for next term names an Art-Net node the hall it is opened in
has never heard of. `MachineConfig` predicted this in S11 in as many words
("the question it answers comes up again for output configuration"), and S33 is
the session that answered it.

**A third applier, and the price paid where S40 paid it.** The four commands are
neither the show's nor the session's, so `Command::is_machine_command` is a third
predicate and `MachineConfig::apply` is a third applier. Both of the other two
refuse them by name, so a command added to the protocol is still a compile error
in every applier. The three example lists in `prism-core`'s tests are three
groups now — 52 forms across them — and every command is in exactly one.

**None of the four is undoable, and that follows rather than being decided.** The
Oops journal is the *show's*: it is cleared when a show is loaded and it images
show and session scopes, so a record of a rig change would be a record of
something the show it is filed against knows nothing about. It is also §6.1's
playback rule met by another road — an undo that re-addressed a node would move
light on a stage while somebody was driving it.

#### Hot reconfiguration, and what it cost the tick

The hard half of the session. Until S33 the set of frame subscribers was fixed
before the tick started, because `FramePublisher::subscribe` allocates and §3.1
forbids the tick the allocator — so an output could not join a running engine at
all.

`prism_engine::FrameEnrolment` is the channel, and it is deliberately the shape
`prismd`'s `BodySwap` already established: the arriving subscriber's buffer is
allocated by whoever asked for it, on their own thread, and left behind **one
atomic flag**. The tick reads that flag once per publish — false almost always —
and only when it is set does it take the slot with a `try_lock` that never
blocks. The links it gives up go back the same way, so a departing output's
buffer is freed on the caller's thread and not on the tick's.

| Measured | Result |
|---|---|
| Allocator calls in the tick that takes on four subscribers and gives up four, over 100 ticks at 64 universes | **0** — `crates/prism-engine/tests/tick_allocations.rs::taking_a_subscriber_on_and_giving_one_up_costs_the_tick_no_allocation`, the **eighth** path in that target |
| What the first attempt cost | **1 allocation**, and it was the retired-links vector's first `push`. `Vec::with_capacity(MAX_SUBSCRIBERS)` at construction is the fix, and the same reasoning is why `links` is `reserve_exact`ed: a push into spare capacity does not call the allocator, and a push into a full vector does |
| The bound | `MAX_SUBSCRIBERS = 64`, enforced *before* the caller allocates, so a rig that asks too often costs nothing. A rig reconfigured 80 times in a row holds exactly one place throughout — `the_places_an_output_took_come_back_when_it_goes` |

Two edges are asserted rather than assumed: a subscriber that **joins and leaves
between two ticks is never installed** — otherwise the vector would momentarily
hold one more than the enrolment has places for — and a subscriber **number is
never handed out twice**, because a stale removal that came round again would
take a live output's frames away.

**What costs a restart is deliberately narrow**: the kind, the universes, the
number and the enabled flag. **A rename costs nothing at all** — asserted by
comparing the `Arc` the status is published through before and after — because
an operator who typed a better name should not watch their rig blink, and for
sACN a restart is a stream terminated and a stream begun.

#### The two flags, and the rule between them

`--mock-devices` is new and is what makes the worked example testable:
**every configured output is opened with a recording double instead of the real
driver**, so a rig of two Open DMX adapters and three network nodes is driven
frame by frame with nothing plugged in and nothing on the network. This machine
has a real SH-RS09B attached and the suite does not open it.

`--mock-output` and the other four output flags kept their meaning and gained a
rule: **outputs named on the command line are the whole rig for that run.** The
stored configuration is neither read nor written, and the four commands are
refused with `MachineError::ConfiguredOnTheCommandLine` — asserted, along with
the fact that `machine.json` still holds no `Mock output` afterwards. Every
daemon target in this repository starts with `--mock-output`, and a run like that
must neither inherit a venue's cabling nor overwrite it with a test's.

A flag has nowhere to put a list of universes, so an output built from one
carries none until the show is open and `cli::fill_universes` fills them in;
`--open-dmx 3` is the exception, because it *is* a universe.

#### What a settings panel can now draw

`OutputSnapshot` went from three fields to seven: the configured row, the frames
sent, the last error and how long ago it was. The age is **relative** and that is
the decision rather than a convenience — the daemon and a browser have no shared
clock, and an `Instant` is not a thing that goes on a wire. It is stored on the
driver thread as two atomic words (`OutputStatus::record_fault`), never a string
behind a lock, and every one of the three `OutputError` variants is asserted to
survive the trip. A panic records `Faulted`: a status panel that showed nothing
for the one fault the zero-crash invariant is really about would be the least
useful it could be.

#### Two findings worth the decision log

**`rmp-serde` is not codec-symmetric about `SocketAddr`.** Its serialiser reports
itself as not human-readable and writes the address as a tagged map of octets;
its deserialiser reports itself as human-readable and asks for a string. A value
written with `to_vec_named` therefore cannot be read back with `from_slice`, and
`wire.rs`'s round-trip property found it within a minute of the type existing.
The fix is `prism_domain::socket`, which writes the address as **text on both
codecs** — which is also what a technician reading `machine.json` expects and
what `ts-rs` renders the field as, so the three agree by construction. It is the
same shape as `finite`, and for the same class of reason: a codec that is nearly
right about a type is a bug that appears on one wire only.

**A serial from a configuration cannot go into a `&'static str`.**
`DeviceDescriptor::with_serial` was built in S8 for exactly this — pinning one
cable when there are two — but its field was `Option<&'static str>`, which a
`machine.json` cannot fill. `Cow<'static, str>` is the change; `SH_RS09B` stays a
`const` with a borrowed string in it, `DeviceDescriptor` and `DeviceProfile` lose
`Copy`, and the ripple was two `clone()`s. The serial now reaches the bus:
`opendmx.rs::a_serial_from_the_configuration_is_what_the_bus_is_asked_for`
asserts the descriptor the driver hands the backend. Whether two adapters open
the two cables an installer expects is a 🔌 row in `ARCHITECTURE_SPEC.md` §14,
because only two cables can answer it.

#### The six recordings, and every difference in them

Regenerated as a set (`cargo test -p prismd --test ui_* -- --ignored`) and the
diffs read structurally rather than as base64. **Every field that differs across
all six is one of three things**, and `telemetry-recording.json` did not change
at all:

| What differs | Occurrences | Why |
|---|---|---|
| `snapshot.health.tickHz` | 32 | A measured rate. Expected to differ between runs |
| `snapshot.outputs[].output`, `.framesSent`, `.lastError`, `.lastErrorAgoMs` | 32 each | **S33's four new fields**, absent before and present now |
| Where a `PlaybackState` lands in a step's delta list, and the `isActive` / `currentCueIndex` that follow it | 4 steps, 2 snapshots | **S34's asynchronous readback**, recorded in this file's decision log when it was built: the recorder stops at the `Ack` and the tick's answer arrives a poll later, so it falls in one step's list or the next. The set of deltas and their order is otherwise identical |

Nothing else moved.

#### The numbers that had to stay where they were

| Gate | Result |
|---|---|
| Zero allocations in the tick | ✅ **0 on all eight paths** — the seven S40 measured, plus the new subscriber hand-over |
| Tick deadline | ✅ **0 missed ticks** over the reconfiguration tests, asserted inside `an_output_whose_device_disappears_degrades_alone` rather than measured beside it |
| Telemetry frame budget | ✅ **0.10 ms median, 0.50 ms p99** for decode and paint together, 64 universes at 30.4 Hz over 155 frames, against a budget of 8 ms |
| Zero React commits | ✅ the `<Profiler>` gate is one of the 28 and it passed |
| `cargo test --workspace` | ✅ **1 801 tests**, 62 targets, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ — and it caught three `unfulfilled_lint_expectations` on the way, which is the lint that only fires when an `#[expect]` was not needed |
| `cargo fmt --all --check` | ✅ |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **618 tests in 46 files**; the build is 327 KB, 98 KB gzipped |
| Playwright | ✅ **28 tests** |
| CI green on the pushed commit | ✅ run **32539636281** on `6e938f9` — all **five** jobs, zero non-success steps: Windows full build and test 8 m 19 s, Linux platform-neutral crates 4 m 16 s, Linux ARM64 cross-compile check 40 s, UI typecheck/lint/test/build 1 m 59 s, UI end-to-end against a daemon 2 m 40 s. The ARM64 job is what says `#[cfg(target_os = …)]` did not spread: S33 added none |

### 2.36 S36 verification record

Measured on 2026-08-22, all exit criteria from `IMPLEMENTATION_PLAN.md` S36 and
the session prompt. The other half of S33's sentence: **the desk in the rack is
a device, not a mock** — and, like the cabling, it belongs to the building.

| Check | Result |
|---|---|
| **Enumeration answers on a machine with no MIDI device, with an empty list rather than an error** | ✅ at three levels, and it is the criterion CI runs on two platforms that have never seen an X-Touch. `prism_midi::ports()` answers a `PortList` and cannot fail; `prismd --midi-ports` prints the backend and `no MIDI ports` and exits **successfully**; and `Query::MidiPorts` against a running daemon answers `{ ports: [], configured: null, open: null }`. The Linux jobs exercise the *empty* case for real, because that build has no backend compiled into it at all. The tests cannot assert emptiness on **this** machine — a real X-Touch is attached — so they assert what is true either way: it answers, every name is a name, and **nothing is opened** |
| **The whole `prism-surface` suite still runs with nothing plugged in, and so does the D11 gate** | ✅ `cargo test -p prism-surface` — **179 lib tests** + 72 across seven integration targets, 0 failed, 0 ignored, unchanged in every number. The crate gained **no dependency at all**, which is the same sentence S20 wrote about the probe. `crates/prismd/tests/surface_gate.rs` is **7 tests** (6 before), all green over a `MockSurfacePort` |
| **No `#[cfg(target_os = …)]` outside the new crate** | ✅ and the ARM64 cross-check is what says so, structurally rather than by inspection: on `aarch64-unknown-linux-gnu`, `cargo tree -p prismd` shows `prism-midi` with **no dependencies under it whatever**. `grep -rn "cfg(target_os" crates/` finds seven hits outside `prism-midi`, `prism-protocols` and `prism-ipc/src/transport/local.rs`, and **every one of them is a comment** saying where platform code may live. `prism-surface` compiles for ARM64 exactly as it did before |
| **A configured port that is not there is a warning and a daemon that starts** | ✅ `crates/prismd/tests/surface_port.rs::a_configured_port_that_is_not_there_is_a_warning_and_a_daemon_that_starts`. The port is named through `SetSurfacePort`, the daemon takes it up within a poll, and it is **attached and absent at the same time** — `SurfaceHealth::Disconnected`, `open_name()` is `None`, and the rig sends more frames than it had. Then the daemon is stopped and started again over the same data directory: the port is in `machine.json`, so this time it is configured *before* anything runs, and it still starts |
| 🔌 **The D11 gate over a real port** | ☐ **built, not verified** — see `ARCHITECTURE_SPEC.md` §14, which carries it as a row with the recipe. Everything a *port* can answer is asserted with nothing plugged in; what only the desk can answer is whether `midir` produces exactly one disappearance and one reappearance for one real unplug and replug. That is `docs/MCU_MAPPING.md` §7's last ◻ row, and S36 closed the half of it S21 could not: the backend is chosen, named and wired |
| `cargo test -p prism-midi` | ✅ **21 tests**, 0 failed, 0 ignored — and not one of them opens a port. The two that touch the real backend ask it to **enumerate** and to open a name **nothing can be called** |
| `cargo test --workspace` | ✅ exit 0 — **1 839 tests across 65 targets**, 20 ignored. **38 of them are this session's**: 21 in the new `prism-midi`, 11 in `prismd` (5 in the library, 6 in the new `tests/surface_port.rs` target) and 6 in `prism-domain`, `prism-core` and the gate |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0, and **measured cold**: `cargo clean -p` on all five changed crates first, because S40's `unused_qualifications` and S33's three `unfulfilled_lint_expectations` only appeared on a fresh compile. Two lints were met on the way and both were in the new crate — an `unused_qualifications` on `self::midir_backend::Midir`, and `items_after_test_module` on a `const` the test module used |
| `cargo fmt --all --check` | ✅ exit 0 |
| `cargo check --workspace --target aarch64-unknown-linux-gnu` | ✅ for every crate this session touched. The full workspace check stops locally at `libsqlite3-sys`, which needs `gcc-aarch64-linux-gnu` — a missing cross toolchain rather than a portability break, and CI installs it. `cargo check -p prism-midi -p prism-surface --all-targets --target aarch64-unknown-linux-gnu` is exit 0 |
| Coverage on the new crate | ✅ **100 % lines on everything that is not the platform module** — `lib.rs`, `naming.rs` and `port.rs` all at **100 % lines**, 99.58 % / 99.26 % regions and **100 % functions**. `system.rs` reads 51.49 %, and what is uncovered there is the open-a-real-device path and the three `MidirPort` methods, which is the same shape `prism-protocols` reports for its two FTDI backends. The crate as a whole is 90.52 % lines. Two unreachable branches found while reading the report were **removed** rather than covered, which is S21's rule |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **619 tests in 46 files** (618 before); the build is 327 KB, 98 KB gzipped |
| Playwright | ✅ **28 tests** |
| CI green on the pushed commit | ☐ recorded below after the push, per `IMPLEMENTATION_PLAN.md`'s session protocol |

**What was built, and where the line is.** `prism-midi` is a workspace member
with one job: hold the platform split. Above the split there is one code path on
every target — the port naming, the reconnection schedule, the framing — and
below it there is `midir`, in `system.rs` and nowhere else. `prismd` gained one
path dependency and four forwarding methods (`surface::RealSurfacePort`), and
`prism-surface` gained nothing at all.

#### Why it is a crate and not a second tool

S20's probe is outside the workspace and stays there, and the reasoning is worth
separating because the two look the same from a distance. The rule §10.1 states
is that *platform code is confined*; the rule `CLAUDE.md` states is that *no test
may need hardware*. The probe breaks neither by being outside — what it does
genuinely needs a device, and one of the things it does is flood a surface until
it stops answering, which is not something to run against a desk somebody is
about to use.

A shipping backend cannot live there, for two reasons that are not about taste:

- **`prismd` has to enumerate ports over the protocol.** S37's Devices panel
  offers a list and the operator picks one, so the daemon must be able to ask
  the operating system what is plugged in. A separate process cannot answer that
  for it.
- **The precedent is `thread-priority`, and it is exact.** S17 took on a
  dependency whose entire purpose was to keep `SetThreadPriority` and
  `pthread_setschedparam` out of a crate that may not have them. This is the same
  move one device along, and `prismd`'s own crate documentation now lists four
  places platform code would otherwise have been rather than three.

The cost was one decision recorded in §10.1, and the ARM64 job is what audits it.

#### The manifest is the split, not a habit

The dependency is declared twice and the difference is what keeps the
cross-check honest:

```toml
[target.'cfg(not(target_os = "linux"))'.dependencies]
midir = "0.11"

[target.'cfg(target_os = "linux")'.dependencies]
midir = { version = "0.11", optional = true }

[features]
alsa = ["dep:midir"]
```

Windows and macOS have a MIDI service in the operating system, so `midir` builds
against nothing that has to be installed first. Linux reaches ALSA through
`alsa-sys`, which is a `pkg-config` probe for `libasound2-dev` — a package the
ARM64 job does not have and **must not need**, because that job exists to catch
portability breaks in *our* code rather than to prove a C library can be
cross-installed. The measured consequence: on `aarch64-unknown-linux-gnu`,
`prism-midi` has no dependencies at all.

What a build with no backend gets is not a degraded mode to apologise for.
`NoBackend` enumerates an empty list and refuses to open, which is exactly the
answer a machine with an empty USB bus gives — the same two facts from the
caller's side. It is what both Linux CI jobs run against, so the *empty
enumeration* half of S36's exit criterion is exercised for real rather than
simulated. **The cost is named rather than hidden**: the Linux MIDI path is
compiled by nobody today, and `ARCHITECTURE_SPEC.md` §14 carries it as a 🔌 row
for the Raspberry Pi (§10.2) — a build with one feature and one test.

#### A name is the configuration, and it has to survive a cable

A port index renumbers itself when somebody moves a plug, so a configuration
holding one points at a different device afterwards. The configuration therefore
holds the **name**, and `prism_midi::selects` compares two names after taking
the platform's decoration off:

| Platform | What the surface is called | What comes off |
|---|---|---|
| Windows (WinMM) | `X-Touch`, `2- X-Touch`, `MIDIIN2 (X-Touch)` | the `2- ` driver-instance prefix, which changes with the socket the cable is in |
| Linux (ALSA) | `X-Touch:X-Touch MIDI 1 24:0` | the ` 24:0` client address, which changes at every boot |
| macOS (CoreMIDI) | `X-Touch` | nothing |

Three rules in order — the name as it stands, the name undecorated, the name as
a case-insensitive fragment — and the **order is the design**: looser rules come
last so a venue with two X-Touches can write both names out in full and neither
row can drift onto the other. `an_exact_name_wins_wherever_it_is_in_the_list` is
the case S20 measured: the X-Touch enumerates twice and only the first port is
the control surface, so a configuration that has to reach the second says so
exactly. An **empty** name selects nothing at all, because a desk that attached
itself to whatever was plugged in would be a desk that moves somebody else's
faders.

One thing was found by reading the coverage report and is worth writing down:
`selects` had a guard against `normalise` returning an empty string, and the
guard is unreachable — `normalise` only ever strips a prefix that something
follows and a suffix that something precedes. S21's rule is that an unreachable
branch is **removed** rather than covered, so it is gone, and
`normalising_never_empties_a_name` stands in its place over twelve awkward
inputs. It is load-bearing rather than tidy: an empty needle is contained in
every string, so the failure it prevents is a configuration that selects every
port on the machine.

#### The two ways a port can go, and the one way it cannot

```text
       ┌───────────── refresh(now), after the backoff ─────────────┐
       ▼                                                           │
   [ closed ] ── open succeeds ──▶ [ open ] ── gone from the list ──┤
    error says                             └── a write failed ──────┘
    why, and it
    is a warning
```

| Rule | Measured |
|---|---|
| A port that is not there is a **reason**, never a failure | `MidiSurfacePort::attach` cannot fail. `a_port_that_is_not_there_is_a_reason_rather_than_a_failure` |
| The retry backoff doubles and stops | **250 · 500 · 1000 · 2000 · 4000 · 5000 ms and 5000 thereafter**, read off the *attempts* the fake backend counted rather than described. Both halves matter: without the doubling a desk nobody will ever plug in costs a system call every 250 ms for the length of a show; without the ceiling a desk plugged in after an hour waits most of another one |
| An open port is checked against the enumeration **once a second, not once a millisecond** | 999 refreshes inside the interval ask the backend **nothing**; the thousandth asks once. The surface thread polls at 1 ms (`SURFACE_PERIOD`), and a system call at that rate to learn something that changes when a person moves a plug would be a measurable waste |
| A cable pulled and put back | Asserted both ways: the port closes and `reconnects()` reads 1 after the replug, which happens on the **next** refresh rather than after the backoff — a plug being moved is somebody standing at the desk |
| A write the operating system refuses **is** a cable coming out | `a_write_the_system_refuses_is_a_cable_that_has_gone` |
| **Silence is not** | `a_desk_that_has_gone_quiet_is_not_reopened`: ten seconds of nothing inbound with writes still landing, and the port is opened **once** and left alone |

That last row is S20's finding kept as a rule one layer below the health state
that reports it. The X-Touch can stop transmitting while receiving perfectly —
text written to it still appears on the display — and **only a power cycle
recovers it**; reopening the port does not, and neither does a fresh process
(`docs/MCU_MAPPING.md` §2.7). A port layer that reconnected on silence would
churn the one device state that cannot be recovered that way *and* take
`SurfaceHealth::Unresponsive` away from the operator who has to read it. So the
only two things that close a port are the port disappearing from the enumeration
and a write that failed.

#### The hot-plug edge, asserted with nothing plugged in

`SurfaceLink` learns about a cable through `SurfacePort::connected` and nothing
else, so a mock that answers both ways walks exactly the path a real one does.
`MockSurfacePort` gained `replug` beside S21's `unplug`, and that is the whole of
the test seam.

| Measured | Result |
|---|---|
| A cable pulled mid-show | `SurfaceHealth::Disconnected`, one `Delta::Notice` at `Warn` saying *no surface*, and **nothing** written to a port that is not there for the next hundred pumps |
| A cable put back | `connected` invalidates the shadow model, so §5.3's resync burst is the ordinary diff rather than a special path. The redraw is asserted to be **more than a hundred messages** — the whole surface — rather than the one fader that changed while it was away |
| The engine across both edges | **0 missed ticks** and the frame count still moving, asserted inside `surface_gate.rs` rather than beside it |
| A port that was **never** there | Starts `Disconnected` and says *nothing* about having gone. Attaching a surface used to mark it connected unconditionally; a configured port whose desk is switched off would then have produced *the surface has gone* about a surface that was never there |
| Changing the port while the show runs | `changing_the_port_while_the_show_runs_costs_the_rig_nothing`: one port down, another up, and the rig's frame count moves while `missed_ticks` does not |

#### A fifth machine command, and the flag that refuses it

`Command::SetSurfacePort { port: Option<String> }` is the 48th command and the
fifth machine command, and it needed no new argument: S33 carried out of itself,
in as many words, that *a later session that wants anything else about this
building — a MIDI port name (S36), a network interface, an autostart flag (S29) —
puts it here*. So it lands in `prism_core::MachineConfig` beside the desk
identity and the rig, both of the other appliers refuse it by name, and it is not
undoable for the reason the four output commands are not.

`the_surface_port_is_not_show_content` is the asserted half, written beside
`desk_id_is_not_show_content` and `outputs_are_not_show_content` and saying the
same thing on the same evidence: a show has nowhere to put a port name, and the
`.prism` file is read back **as bytes** and must not contain one.

The three example lists in `crates/prism-core/tests/common/mod.rs` are **53
forms** across the three groups now, and every command is in exactly one.

**`--surface <port>` is the surface for the run, and `SetSurfacePort` is then
refused** — S33's rule for `--mock-output`, one device along. It is a **second**
flag rather than a second meaning for the first, because the two are genuinely
independent: a daemon may take its rig from `machine.json` and its surface from a
command line while somebody is trying a desk out, and an operator refused an
`AddOutput` for a reason that is not true would go looking in the wrong place.
`MachineError::SurfaceOnTheCommandLine` names `--surface`, and the test asserts
that the rig is still the configuration's while the surface is not.

One consequence is worth stating because it caught this session's own tests: a
daemon whose **outputs** came from a command line refuses `SetSurfacePort` too,
with `ConfiguredOnTheCommandLine`. That is right rather than incidental — the
reason that rule exists is that *nothing is written down this run*, which applies
to a port name exactly as it applies to a rig. `tests/surface_port.rs` therefore
starts its daemon the way S37's settings window will: no output flags,
`--mock-devices`, and the rig built with `AddOutput`.

#### What a settings window can now ask, and why it is a question

`Query::MidiPorts` answers `{ ports, configured, open }`. It is a query rather
than a field of the snapshot for a reason S27 and S44 already established twice:
what is plugged in is **not state the daemon owns**. It changes when a person
moves a plug, no command causes it, and a client that mirrored it would be
holding the operating system's opinion from whenever it last connected — which
is also why a settings window needs a *rescan* button at all. Enumerating is a
system call, which is the second reason.

The answer carries the configuration as well as the enumeration, and that is the
decision rather than a convenience: a list of ports with no mark against the
chosen one is a list an operator cannot act on. `configured` need not appear in
`ports` — a desk that is switched off is **named and absent at the same time**,
which is exactly the state a panel has to draw — and `open` is what is really
there.

`Delta::SurfaceChanged` is the third piece: one name or none, sent whole because
it *is* whole, ignored by both mirrors for the reason `OutputsChanged` is, and
what tells a settings window that asking again is worth it.

**`open` is refreshed on the housekeeping tick and not the surface's**, which is
a cost decision written down: the surface polls every millisecond and
`open_name()` allocates a `String`, so a thousand of those a second to keep a
status panel half a second fresher would be the wrong trade. Half a second is
the right freshness for a fact that changes when somebody moves a plug.

#### Two findings worth the decision log

**A `SurfacePort` needed a fourth method, and it is not `is_connected`.**
`refresh(now)` is *give the port a chance to notice a cable*, and it exists
because the two things that close a port — an enumeration that no longer lists it
and a write that failed — cannot both be discovered by asking `connected()`. It
takes `now` for S21's reason, kept one layer further down: the clock is an
argument all the way, and the surface thread in `prismd` reads the only real
instant in the stack. That is what lets the whole reconnection schedule be walked
in a test in milliseconds.

The ordering in `SurfaceLink::poll` is deliberate and would be wrong the other
way: the port is asked **after** the send, because a write is one of the two
things that discovers a cable has come out, so asking first would report a port
gone one poll later than it went.

**The framing was the one part of a real port that is arithmetic, and it was in
the wrong place.** `MidirPort::read` originally held the queue that turns whole
MIDI messages into however many bytes the caller had room for — with the tail
kept for the next call, because the layer above is a stream decoder with SysEx
reassembly (S19) and truncating would corrupt a scribble-strip reply into a
different message the codec would *count* rather than notice. It sat inside the
platform module, where no test can reach it. It is `prism_midi::Inbox` now, and
a 63-byte SysEx read four bytes at a time comes back byte-identical. Finding it
was a coverage report read line by line, which is the third session running that
this has paid for.

#### The six recordings, and every difference in them

Regenerated as a set (`cargo test -p prismd --test ui_* -- --ignored`) and the
diffs read **structurally** — a script that base64-decodes each blob, unpacks the
MessagePack inside it and walks the two documents in parallel, rather than
comparing the text. Every field that differs across all six is one of four
things, and three of them are classes S33 already named:

| What differs | Occurrences | Why |
|---|---|---|
| `snapshot.health.tickHz` | 32 | A measured rate. Expected to differ between runs |
| `snapshot.outputs[].framesSent` | 31 | A driver's own counter over wall-clock time. Expected for the same reason |
| Where a `PlaybackState` lands in a step's delta list | 4 steps in `daemon-recording.json` | **S34's asynchronous readback**, the class S33 recorded: the recorder stops at the `Ack` and the tick's answer arrives a poll later, so it falls in one step's list or the next. In one case two of them arrived *inside* the window this time where they had not before, which is the same fact. The set of other deltas and their order is **identical in every case** |
| `telemetry-recording.json`'s frame sequence | 1 | The frame counter, and the payload differs in **exactly one byte, at offset 8** — asserted byte by byte. All 64 universes' sums, fingerprints and samples are identical |

Nothing about S36 appears in any of them, which is correct: no recording issues a
`SetSurfacePort`, and a new `Command`, `Delta` or `Query` variant changes no
message that was already being sent.

#### The numbers that had to stay where they were

| Gate | Result |
|---|---|
| Zero allocations in the tick | ✅ **0 on all eight paths** — `crates/prism-engine/tests/tick_allocations.rs`, 9 tests, unchanged. Nothing this session went near the tick, which is the point: the engine is never told about a cable |
| Tick deadline | ✅ **0 missed ticks** across a surface going away, coming back, and the configured port changing under a running show — asserted inside the tests rather than measured beside them |
| Telemetry frame budget | ✅ **0.20 ms median, 1.10 ms p99** for decode and paint together, 64 universes at 29.8 Hz over 154 frames, against a budget of 8 ms. Two frames lost, which is §8's coalescing working; zero **dropped**, which is what is asserted |
| Zero React commits | ✅ the `<Profiler>` gate is one of the 619 and it passed |
| `cargo test --workspace` | ✅ **1 839 tests**, 65 targets, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅, measured **cold** |
| `cargo fmt --all --check` | ✅ |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **619 tests in 46 files**; 327 KB, 98 KB gzipped |
| Playwright | ✅ **28 tests** |

**One measurement was thrown away and re-taken**, which is worth recording
because it is the trap `PROGRESS.md` already warns about: the first full `npm run
test` reported one failure — `App.test.tsx`'s command-line test timing out at
5 s — while a `cargo llvm-cov` run was using half the machine. Alone it takes
2.3 s, and the whole suite passes on an idle machine. A busy machine looks
exactly like a regression.

### 2.37 `Color` — sequences take a colour, out of band

A small feature asked for between sessions, built the same way the ones around
it were, and recorded here because two of its consequences are structural.

**`Command::Color { target, color }`** is `Label`'s mirror: the same
`ObjectRef`, the same executor indirection, and a narrower reach. It is the
**49th** variant and the **54th** entry across the three example lists. A colour
is drawn on a scribble strip and only a cue list is drawn on one, so it reaches a
sequence and — through the list standing on it — an executor; a cue, a group, a
preset and a view are refused with `ShowError::NotColourable` naming the noun
that was typed.

**Where it lives is the decision.** `Sequence::color`, beside the name, and not
on the executor: a list moved to another fader takes what is written on it along,
which is the same argument the name already made. `#[serde(default)]`, so a
`.prism` file written before this opens without one. `None` is *no colour
chosen* rather than black, and the two are different things — an uncoloured
strip is lit **white**, because the text on an unlit one cannot be read
(`docs/MCU_MAPPING.md` §2.3).

**The command line**: `color sequence 4 red`, `color executor 1 #ff8800`,
`color sequence 4` or `… none` to take it off. Seven colour words — the seven a
strip can light — plus a hex triplet in either the three- or six-digit form. The
words are the ones whose result an operator can predict exactly; anything else is
kept in full and quantised hue-first for the surface, which is what
`prism_surface::color` was already written to do.

**A latent fault in S40 came out with it.** `Label Executor 1 "…"` imaged the
**executor** for the Oops journal, and the command writes to the *sequence* — so
the record's before and after were equal, no record was written, the rename
applied and an Oops answered `NothingToUndo`. Both verbs image the cue list now
(`ShowFile::colored_image`), and `oops.rs` asserts it for both rather than only
for the new one. Found by giving `Color` the same indirection and checking what
its undo image ought to be.

| Gate | Result |
|---|---|
| `cargo test --workspace` | ✅ **1 845 tests**, 66 targets, 20 ignored, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅, measured **cold** — `cargo clean -p` on the five changed crates first |
| `cargo fmt --all --check` | ✅ |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **625 tests in 46 files**; 329 KB, 99 KB gzipped |
| Playwright | ✅ **28 tests**, telemetry **0.10 ms median, 0.30 ms p99** over 154 frames at 30.3 Hz |
| Recordings | ✅ all six regenerated and diffed structurally. Four classes, all explained: the new `color` key wherever a sequence is serialised; `tickHz` and `framesSent`; S34's async playback readback moving between steps; and the two steps appended to the script |

The same load trap as S36 appeared again and was handled the same way: the first
full `npm run test` reported `App.test.tsx` timing out at 5 s with the machine at
36 % before the run started. Alone the file takes 3.6 s and the whole suite
passes idle.

### 2.38 A Go comes round, and `loop` stops meaning two things

Asked for beside §2.37 and worth its own entry, because the fix is a **split**
rather than a flag flip.

`SequencePlan::step` held at the end of a list that did not loop — `Go+` on the
last cue stayed on the last cue. Holding is the wrong answer to a key somebody
pressed: an operator at the bottom of a busking list who presses Go wants the top
of it, and a desk that does nothing looks broken in the dark. **A Go now always
comes round**, forward and back, on every list. Nothing goes to black on the way;
the wrap enters cue 1 with cue 1's own fade, exactly as a Go into it from cue 0
would, which `cue_playback.rs` asserts on the encoded DMX byte rather than on the
index.

**The naïve fix would have been wrong.** `Sequence::loop` gated *both* the manual
Go and the automatic `Follow`/`Time` chain, so simply removing the gate would
have turned every follow chain in every show into an endless chase — a behaviour
change nobody asked for and one that only shows up on stage. So the two questions
are two functions now:

| | Wraps at the end? |
|---|---|
| `SequencePlan::step` — a **Go**, a hand on a key | always |
| `SequencePlan::follow_step` — an **automatic** `Follow` or `Time` | only when `Sequence::loop` |

`follow_step` returns `None` where the chain ends, so the `next != index ||
plan.looping()` guard the player used to carry is the return type instead — and
the one case that guard would have got wrong if written the obvious way, a
**one-cue looping list following itself**, is a test of its own.

`ARCHITECTURE_SPEC.md` §6 and `Sequence::looping`'s own documentation now say
which of the two the flag is, since both said "the last cue wraps back to the
first" and that is no longer what it buys.

| Gate | Result |
|---|---|
| `cargo test --workspace` | ✅ **1 847 tests**, 66 targets, 20 ignored, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` | ✅ |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **625 tests in 46 files**; 329 KB, 99 KB gzipped |
| Playwright | ✅ **28 tests**, telemetry **0.20 ms median, 0.50 ms p99** over 159 frames at 30.5 Hz |
| Recordings | ✅ unchanged — no wire type moved, and the recorded script has no Go at the end of a list |

### 2.39 S37 verification record

Measured on 2026-08-23, all exit criteria from `IMPLEMENTATION_PLAN.md` S37 and
the session prompt. The window `WindowType::Settings` had been reserving since
S25, and the session in which **the command line stops being the only way to say
what this machine is**.

| Check | Result |
|---|---|
| **S33's worked example is built entirely from the interface, and the frames arrive** | ✅ `ui/e2e/settings.spec.ts::a venue's rig is built entirely from the settings window`, in Chromium against a real `prismd`. Five outputs of three kinds — two Open DMX adapters with serials, an sACN gateway on 3 and 4, an Art-Net node on 5–8 and another on 9–12 — typed into the window with **no flag anywhere in it**, and each row's universes read back off the table: `1`, `3, 4`, `5, 6, 7, 8`, `9, 10, 11, 12`. A browser cannot look at a wire, so *the frames arrive* is the per-driver counter, polled by the panel and asserted to pass 3 for **all five**. The daemon is the one a venue runs: no output flags, `--mock-devices`, so nothing is opened and nothing goes on the network |
| **A show is saved, saved under a new name and reopened without a command line** | ✅ twice, in two languages. In Chromium: `default.prism` → *Save as* `panto.prism` → the panel reads `panto.prism` and *saved* → the recent list is clicked and `default.prism` is back → *Open* by name and `panto.prism` is back. In Rust, where the bytes can be read: `crates/prismd/tests/settings.rs::a_show_is_saved_under_a_new_name_and_opened_again` unpatches a fixture into the new file and finds **two** fixtures there and **three** in the old one |
| **Changing one output does not interrupt the others** | ✅ in the same browser test: output 4 is re-addressed while 1, 2, 3 and 5 are watched, and every one of their counters is **greater than it was** afterwards. Then output 4's own counter moves again by itself. S33 asserts the same thing on the recorded frame sequence in Rust; this is it watched from outside the process, which is what the exit criterion asks for |
| **Every panel is a reader over daemon state: closing and reopening shows the same thing, and a second client sees the change** | ✅ three ways. A **second tab** with no window of its own — because which windows are open is session state, so the first tab's settings window is already on its canvas — watches the universe count go 64 → 12 and the exit action change, without being told. Closing the window and opening another shows 12 again. In jsdom, the stronger form S25 used: a **second interface** that has never sent a command or seen a delta draws the same panels from the same snapshot |
| **A panel with more rows than it has room for scrolls inside its window** | ✅ `ui/e2e/settings.spec.ts`, and the *inside* half is the one a check of zeros everywhere would miss: the window is dragged small by its corner, twelve outputs are added, and `scrollHeight − clientHeight` on the panel body reads **greater than zero** while the document element and the canvas read **0 on both axes** |
| `cargo test --workspace` | ✅ exit 0 — **1 894 tests across 68 targets**, 20 ignored. **55 of them are this session's**: 14 in the new `crates/prism-core/tests/settings.rs`, 14 in the new `crates/prismd/tests/settings.rs`, and the rest in `prism-domain`'s round trips, `prism-core`'s three example lists and `prismd`'s two conversion round trips |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0, and **measured cold** — `cargo clean -p` on all six changed crates first, because S40's `unused_qualifications` and S33's three `unfulfilled_lint_expectations` only appear on a fresh compile. Three lints were met on the way: two `collapsible_if` in `prism-core::desk` and a `duplicate_macro_attributes` where a doc comment had been written between a `#[test]` and its function |
| `cargo fmt --all --check` | ✅ exit 0 |
| `cargo check -p prism-domain --all-targets --target aarch64-unknown-linux-gnu` | ✅ exit 0. The wider check stops at `libsqlite3-sys`, which needs `gcc-aarch64-linux-gnu` — a missing cross toolchain rather than a portability break, and CI installs it. **`grep -rn "cfg(target_os" crates/` finds seven hits outside the four permitted places and every one of them is a comment**, unchanged from S36 |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **688 tests in 48 files** (675 before this session's last two files, 619 at S36); the build is **359 KB, 107 KB gzipped**, up 32 KB on S36 — and **this session adds no dependency** |
| Playwright | ✅ **34 tests** (28 before), all green |
| Coverage on what this session wrote | ✅ **99.28 % lines on the new `ui/src/settings/`** — `settings.ts` and `settingswindow.tsx` at **100 % on every column**, `devices.tsx` and `machine.tsx` at 100 % lines, `outputs.tsx` 99.03 %, `showfiles.tsx` 96.77 %. The two lines that are left were read rather than counted and are both a guard this interface cannot reach: an apply with no draft, and a submit with nothing typed. `prism-domain`'s new `machine.rs` is **100 % on every column** |
| CI green on the pushed commit | ✅ run **32611422650** on `dd5d3cd` — all five jobs, **and the Windows one on its second attempt**: Windows full build and test 12 m 01 s, Linux platform-neutral 4 m 25 s, UI end-to-end against a daemon 2 m 57 s, UI typecheck/lint/test/build 2 m 07 s, ARM64 cross-check 41 s. **The six settings end-to-end tests passed on a Linux runner**, so a venue's rig was built from the window, a show was saved under a new name and reopened, and a second tab followed the first — on a machine that has never run this interface by hand. The first attempt is written up below rather than re-run and forgotten |

**What was built, and where the line is.** Four panels in one window, and the
window is a **window**: it lives on the canvas, it can be dragged, it can be open
on a second screen, the shipped profile has bound F4 to it since S22, and the
show carries on behind it. `CLAUDE.md` forbids a modal over the canvas, and this
is the easiest rule in the file to keep — what a settings window changes is
machine or session state that **every** client sees, so there is nothing to lock.

#### Three of the four panels needed no protocol change, and the fourth needed five commands

S33's four output commands, S36's `SetSurfacePort` and `Query::MidiPorts`, and
`MachineConfig` were already there; the *Show files* panel had `SaveShow` and
nothing else. So the file an operator was working in was whatever `--show` had
named at start-up, and renaming it, opening another or making a new one meant
stopping the daemon.

| Command | What it is careful about |
|---|---|
| `SaveShowAs` | The **new file first, the switch afterwards**: a daemon that adopted the path and then failed to write it would hold a show whose file does not exist, with the Save lamp lit over a name nothing is behind |
| `OpenShow` | **Never creates.** `ShowStore::open` makes a file that is not there, which is right for a daemon starting up and wrong for an operator who mistyped — the show they meant would be on the disk beside the empty one they got |
| `NewShow` | **Never replaces.** A *new* show over an existing one would be the most destructive command in `docs/IPC_PROTOCOL.md` §5 and would look like the least |
| `ExportShow` | Changes nothing at all, the open file included |
| `ImportShow` | Does **not** save. `Show::mark_dirty` exists for this one case, because a document read out of a JSON export arrives clean — and an import somebody did not mean to do must be one they can walk away from |

A path is validated in **two places and they are different questions**.
`prism_core::file::show_path` decides what a string can decide — not empty, names
a file rather than a bare extension, and carries the extension of the format this
command actually writes. Whether the file is *there* needs a disk and is
`prismd`'s. The extension check is not cosmetic: a `.prism` is a SQLite database
and a `.json` export is text.

**None of the five is undoable, and three of them are why the other two are
not.** `OpenShow`, `NewShow` and `ImportShow` replace the show, which empties the
Oops journal — a record is an assertion about the show that was open a moment ago
— and the other two change no show state at all.

#### Every operational flag is a setting, and the interface says which ones a flag is holding

`prism_core::Settings` is the third thing S33 predicted would land in
`MachineConfig`, and the last of them: the local transport, the WebSocket
address, the §2.1 token, the log level, the universe count, the exit action, the
autostart flag (S29's), the fixture library's path and the surface binding
profile. `Command::ConfigureMachine` is the sixth machine command and carries
**one field**, which is `ConfigureOutput`'s rule.

The half that is easy to leave out is the other one. A flag is still *the value
for that run* — S33's rule for `--mock-output` and S36's for `--surface` — so
`prismd::cli::resolve` puts the command line and the configuration together
**once** and answers with both the value in force and the list of settings the
command line is holding. `MachineSettings::overrides` carries that list to every
client, and the panel draws a held row disabled with the flag named beside it.
A box an operator can type into that does nothing is worse than a box that is not
there, and the two answers cannot drift because one function produces both.

**`MachineChange::needs_restart` is the domain's answer to what a running daemon
cannot take back**, not a list in an interface. Four are made on the spot — the
log level, the exit action, the autostart flag and the binding profile — and
seven wait. **A new desk identity is deliberately among the seven**: an sACN
source that changed its CID mid-show would be a *new* source fighting the old one
until its 2.5 s network-data-loss timeout expires, so the right moment for it is a
start. `ui/src/settings/settings.ts::needsRestart` mirrors the same seven and
`settings.test.ts` names all eleven changes to hold the two together.

#### The WebSocket listener is on by default, and a listener that cannot bind is a warning

Until S37 it was opt-in, which meant a browser could not reach a desk nobody had
passed a flag to — and the settings window, the Web Remote (S31) and the whole
end-to-end suite all speak WebSocket. Loopback is what makes having it on safe:
§2.1's rule is about reaching **off** the machine, and that still needs both an
address and a token.

What it costs is a rule, and it is S36's for a MIDI port one device along: **a
listener that cannot bind is a warning and a daemon that starts.** Two daemons on
one machine both want 7373, and refusing to start over it would let one stray
process make a desk unstartable half an hour before a show. What an operator gets
instead is the state a panel can draw — `MachineSettings::websocket` names the
address and `websocketOpen` is `null`, which is *configured here, listening
nowhere*. `crates/prismd/tests/settings.rs` produces the case rather than
contriving it: a real socket is bound to the address first, and the daemon is
then asserted to start, to draw that pair, and to drive the rig throughout.

**It cost the test suite something too, and the cost is written into the
targets.** Every daemon target in this repository now says `websocket:
Listen::Off` out loud, because a target that said nothing would bind
127.0.0.1:7373 — and the several that `cargo test` runs at once would each be
asking for it. A suite must not open a socket it does not use.

#### §2.1 is a refusal the daemon makes, and the token is one it generates

A WebSocket address that is not loopback is refused unless a token is set
(`MachineError::NoTokenForNetwork`), and it is refused in `prism-core` rather
than discouraged in an interface: a second client could otherwise put an
unauthenticated lighting console on a school's network, which is the exact thing
§2.1 exists to prevent. **Clearing the token closes the same door from the other
side** — the listener goes back to loopback on the port it was on, because a
configuration with a public listener and no token is a state this desk will not
be in, and a rule stated once would have missed that half.

The token and the desk identity are the two changes that carry **no value**. Both
need entropy, which `prism-core` is not allowed to have; and a *client* must not
supply either, because one would be choosing this desk's password and the other
would be able to give two desks one sACN CID. So `MachineConfig::apply` answers
with an effect, `prismd::machine` makes the value, and §2.1's own sentence — *the
daemon generates it and the UI displays it* — is what the protocol does rather
than what it hopes. The token is twenty-six characters of Crockford base32, whose
alphabet has no `I`, `L`, `O` or `U`: it has to be read off a screen and typed
into a phone in an auditorium.

#### Two questions the panels ask, and one of them closes S33's last gap

S33 carried out of itself that there was **no channel for a live frame counter
between snapshots** — the counter moves at 44 Hz and a delta per frame is out of
the question — and left S37 to either ask or accept a number as old as the
connection. Accepting was the wrong half: `OutputSnapshot` carries the counter
with the world, so an output **added while a client is connected** would read zero
frames for ever, and the row an operator has just made is exactly the row they are
watching to see whether it works. `Query::OutputStatus` is the answer, asked once
a second while the panel is open and never when it is closed. It carries the
health, the counter and the last error with its **age**, and deliberately not the
configuration: that arrives whole in `Delta::OutputsChanged` whenever it moves,
and a rig on the wire once a second to carry a counter would be the wrong trade
twice over.

`Query::DarkUniverses` is the other, and it is S27's rule kept rather than S33's
gap closed: which patched universes no output carries is **derived** from the
patch and the rig together, and a client that intersected the two would be a
second opinion about something `prism_core::dark_universes` already decides. The
`Delta::Notice` S33 says it with is right for *the rig just changed and here is
what that cost*; a settings window needs the same fact **standing**, because an
operator opening the panel has to see that universe 7 goes nowhere without having
changed anything to be told.

`Answer::MidiPorts` grew a fourth field for the same reason one layer along:
`SurfaceStatus` is the health, the six feedback counters, the reconnection count
and the binding table, refreshed on the housekeeping tick and never on the
surface's. **`null` is *no surface at all*, which is not the same fact as one that
is attached and `Disconnected`** — a laptop with no port configured has nothing to
report, and a desk that is switched off has a health and counters that happen to
be zero. And the **remedy travels as words**, which is the one field worth arguing
about: the obvious advice for a desk that has stopped sending is *reconnect*, and
S20 established that reconnecting is the one thing that cannot recover it. A
client that wrote its own sentence would eventually write that one.

#### Two decoders were missing and nothing had noticed

Reading the coverage report is not what found these; building the first panel
that uses them is. `ui/src/ipc/protocol.ts`'s `readDelta` had no arm for
`Delta::OutputsChanged` or `SurfaceChanged` — S33's and S36's — and `readAnswer`
had none for `Answer::MidiPorts`. The store has handled `OutputsChanged` since
S33; what it never had was a way through the door, so a daemon whose rig changed
would have thrown a `ProtocolFault` and resynchronised the connection. Nothing
had noticed because **nothing had ever changed a rig from a browser**, which is
the sentence S37 exists to make false.

The lesson is worth keeping: a delta or an answer with no *caller* is a decoder
arm nobody writes, and the round-trip properties in `prism-domain` do not reach
TypeScript. What would have caught it is a recording that contains one of those
messages — and none of the six does, because until now nothing sent them.

#### Naming the binding profile again is the reload

There is no second command for it, because a second way of saying one thing is a
second thing to keep in step. `MachineChange::SurfaceProfile` re-reads the file
and the daemon swaps the table under the attached surface — **round the same
port**, because a port holds an open connection and a count of how many times the
cable has come out, and because reopening is the one thing S20 established cannot
recover a desk that has gone quiet. The shadow model *is* thrown away, which is
right: a new table can mean a different scribble strip on every strip, so §5.3's
resync burst is the ordinary diff.

A profile that is missing or malformed is reported and the **built-in table
stands** — S22's rule — and `crates/prismd/tests/settings.rs` asserts it on the
one control the file names rather than on a count, because a profile *overlays*
the defaults and a count would be the same either way.

#### The `Command` value tree hit the stack again, and now there is a budget

`prism-core`'s `tests/oops.rs` overflowed a debug test thread for the third time
in four sessions — S34 at the thirty-sixth variant, S40 at the forty-second, S37
at the fifty-fourth. The remedy is the one those sessions wrote down and is
`crate::arb::boxed`: boxing a **field's** strategy replaces its whole subtree with
a pointer. What was missing was a way to find out before CI did.

`crates/prism-domain/src/wire.rs::a_generated_command_fits_in_a_test_thread` is
that: a 32 KiB budget on the size of one generated value tree, with both numbers
measured rather than guessed. With `Copy`'s and `Move`'s `OverwriteMode`,
`ExecutorGo`'s `GoDirection`, `SetSurfacePort`'s `Option<String>` and the five
`Option<SequenceId>` fields unboxed the tree was **36 160 bytes** and `oops.rs`
overflowed; with them boxed it is **30 992** and it does not. A session that fails
that test boxes a field, and `RUST_MIN_STACK` moves the cliff in one crate rather
than in the four that generate commands.

#### `rmp-serde` is still not symmetric about a socket address

S33's finding, met again within a minute of `MachineChange::Websocket` existing,
and found by the same thing: `wire.rs`'s round-trip property. `prism_domain::socket`
gained an `option` module for `Option<SocketAddr>`, and the two shrunk cases are
in `proptest-regressions/wire.txt` — which is the shape of a finding that has now
paid for the module twice.

#### The six recordings, and every difference in them

Regenerated as a set (`cargo test -p prismd --test ui_* -- --ignored`) and the
diffs read **structurally** — a script that base64-decodes each blob, unpacks the
MessagePack inside it and walks the two documents in parallel. **161 differing
paths, and every one is one of five things**, three of which are classes S33 and
S36 already named:

| What differs | Occurrences | Why |
|---|---|---|
| `snapshot.machine` and `snapshot.showFile` | 64 | **S37's two new snapshot fields**, absent before and present now. The class S33 recorded when `OutputSnapshot` grew four |
| `snapshot.health.tickHz` | 32 | A measured rate. Expected to differ between runs |
| Where a `PlaybackState` lands in a step's delta list | 32 paths over **2** steps of `daemon-recording.json` | **S34's asynchronous readback**: the recorder stops at the `Ack` and the tick's answer arrives a poll later, so it falls in one step's list or the next. Verified as a **permutation** rather than assumed — the multiset of delta tags per step is identical, and the two steps are the only ones whose order moved |
| `snapshot.outputs[].framesSent` | 31 | A driver's own counter over wall-clock time |
| `telemetry-recording.json`'s frame sequence | 2 | The frame counter, and the payload differs in **exactly one byte, at offset 8** — asserted byte by byte, as in S36 |

#### One measurement was thrown away, and this time the fix was a number

`ui/src/App.test.tsx`'s command-line test timed out at the 5 s default under
`--coverage`, twice, at 60 % and 93 % machine load. S36 recorded the same test
hitting the same wall while `cargo llvm-cov` used half the machine and re-took the
measurement; that was the right answer then and is not one now, because the suite
is forty-eight files rather than forty-six and v8 instruments all of them. The
test takes about two seconds idle and seven under coverage.

So it has a **20 s timeout of its own**, with the reason beside it: nothing in it
waits for anything — every step is a keystroke, a delivered message or an
assertion — so a deadline there is measuring the machine rather than the desk, and
a real regression would hang rather than take nineteen seconds.

#### The Windows job failed once, and it is S36's real-time assertion

`crates/prismd/tests/surface_gate.rs::a_surface_that_goes_away_is_reported_and_the_show_carries_on`
failed on the first attempt with **`missed()` reading 1 where it read 0**, and
passed on the second. It is not one of this session's tests and S37 changed
nothing in it but the types of four `Options` fields.

What it asserts is §5.3's rule — *the engine is never told about a cable* — in
the form `missed == missed_before` **exactly**, over a window in which a replugged
desk is redrawn with 156 messages. That is the right claim and an unlucky way to
state it on a shared runner: a two-core GitHub VM can lose a tick for reasons that
have nothing to do with a cable, and the *tick jitter* gate in §3 is where the
real-time claim actually lives — ten minutes, 64 universes, under 100 % CPU load,
0 of 26 401 ticks missed.

Two things were done about it and a third is left open, deliberately:

- **The housekeeping tick now takes the `Core` lock once instead of three
  times.** S37 had added two takes to that arm (`take_profile_change` and
  `take_exit_change`) beside S36's `take_surface_change`; they are one guard now.
  That is a real reduction in contention with the surface's millisecond poll and
  it costs nothing — but it is **not** offered as the cause, because nothing in
  this session runs on the engine thread at all, and the engine is what missed
  the tick.
- **It was run three times locally on a quiet machine** before the re-run and
  passed every time, in 6.3, 6.6 and 7.2 s.
- **The assertion was left strict.** Weakening somebody else's gate on one
  observation is how a gate stops meaning anything, and the honest fix is not a
  tolerance but a **control**: compare the missed ticks across the replug against
  the missed ticks across an equally long window with no replug, which is the
  shape S18 used for the D2 gate (*either claim alone is passed by a daemon
  broken in the other way*). That is a change to S36's test and belongs to
  whoever next has reason to open it. **S43 had the reason** — the same line
  failed a second time on a documentation commit that changed no Rust at all —
  and built the control. See §2.41.

#### The numbers that had to stay where they were

| Gate | Result |
|---|---|
| Zero allocations in the tick | ✅ **0 on all eight paths** — `crates/prism-engine/tests/tick_allocations.rs`, unchanged. Nothing this session went near the tick, which is the point: the engine is never told what this machine is set to |
| Tick deadline | ✅ **0 missed ticks** across the rig being built, re-addressed and torn down from a browser — asserted inside `crates/prismd/tests/settings.rs` and `outputs.rs` rather than measured beside them |
| Telemetry frame budget | ✅ **0.20 ms median, 1.00 ms p99** for decode and paint together, 64 universes at 30.4 Hz over 155 frames, against a budget of 8 ms |
| Zero React commits | ✅ the `<Profiler>` gate is one of the 688 and it passed — with a settings window in the tree, which is the point of re-running it |
| `cargo test --workspace` | ✅ **1 894 tests**, 68 targets, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅, measured **cold** |
| `cargo fmt --all --check` | ✅ |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **688 tests in 48 files**; 359 KB, 107 KB gzipped |
| Playwright | ✅ **34 tests** |
| CI green on the pushed commit | ✅ run **32611422650** on `dd5d3cd`, the Windows job on its second attempt — see above and the paragraph below |

---

### 2.40 S38 verification record

Measured on 2026-08-23, all exit criteria from `IMPLEMENTATION_PLAN.md` S38 and
the session prompt. Layer 3 of `docs/MCU_MAPPING.md` §1 — the binding table S22
built and could only ever **read** — becomes something an operator edits at the
desk, and the session in which **the last thing that could only be said in a file
stops needing one**.

| Check | Result |
|---|---|
| **A binding changed in the interface takes effect without restarting the daemon** | ✅ twice, and the claim is *the key does something else now* rather than *the table changed* — a readout would satisfy the second for a daemon that never told the desk. In Chromium against a real `prismd`: F1 is pressed at a `--mock-surface` console and opens a Fixture Sheet, F5 is rebound to *open Patch* in the window, and the same three bytes on F5 then open a Patch window. In Rust, `crates/prismd/tests/controls.rs::a_binding_changed_over_the_protocol_takes_effect_without_a_restart` presses **note 54 before and after** and asserts both windows |
| **The shipped profile still *is* the built-in defaults after a round trip through the editor** | ✅ S22's assertion is untouched (`the_shipped_profile_is_the_built_in_default_table`), and S38 adds the round trip it did not have: all **73 rows** read out of the daemon over `Query::SurfaceBindings`, written back one at a time through `MachineChange::SurfaceBinding`, and the table compared with `Bindings::defaults()` — then the **stored** rows read back through `Bindings::from_rows` and compared again, because that is what the next start reads. The unbound rows are written back too; a round trip that skipped the strip encoder and F5–F8 would not be one |
| **A table binding the reserved control cannot be saved, and the message names the button and says why** | ✅ asserted on the **wording** rather than on the variant, as S22 did: `Global.SmpteBeats`, `Xctl+MC`, *switches the desk between the two hosts*, *Leave it unbound*. Refused in three places now — a profile file (S22), a `MachineChange::SurfaceBinding` (`prism_core::MachineConfig::configure`), and a **stored** table (`Bindings::from_rows`, which falls back to the defaults rather than blocking a desk) — and the sentence is `prism_domain::RESERVED_REASON` **once**. Nothing is written after a refusal, asserted on the serialised bytes. Clearing it is **allowed**, because unbound is the state §4.3 wants it in |
| **Learn names the right control for every group of §2.1** | ✅ `crates/prismd/tests/controls.rs`, driven from `--mock-surface`, for one control of every shape the surface has: a panel button (note 94), a strip button (note 8), a strip encoder (CC 16) and the jog wheel (CC 60) — **every number written out by hand from §2.1**, which matters more here than anywhere, because the thing under test *is* the table a profile would have been asked. The name is read off the **delta**, through a connected client, because that is where an editor reads it. And the half that is worth the code: **the press does not fire** — F1 opens a window when it is obeyed and the canvas is asserted empty when it is learned, in Rust and again in Chromium |
| **Two clients with the editor open do not produce two tables** | ✅ structural rather than raced: every edit is **one control**, so two operators changing two keys cannot undo each other. In Chromium, two tabs: the first binds F5, the second is *told* and then binds F6, and both end on the same revision with both edits standing and F1 still what §4.1 says. In Rust the same claim plus the one a browser cannot make — the **stored** rows rebuild into exactly the table in force, so a second daemon over that data directory reads the same one |
| **Ownership is shown** (§4.3) | ✅ every row says *always ours* or *follows the switch*, and it is the daemon's answer rather than the client's — a client holds no device profile. `crates/prismd/tests/controls.rs::every_control_says_whether_it_is_ours_in_the_shared_mode` transcribes §4.3's permanent set **by hand** (Rewind, Forward, Stop, Play, Record, jog wheel) and checks all 73 rows against it, because a test that asked `McuProfile::permanent` would pass for a constant claiming the whole panel is always in reach |
| **A desk starts with the keys it was left with** | ✅ `a_desk_starts_with_the_keys_it_was_left_with`: F5 is bound, the daemon is dropped, and a second daemon over the same data directory starts with F5 bound. This is S38's storage decision as a test |
| `cargo test --workspace` | ✅ exit 0 — **1 927 tests across 69 targets**, 20 ignored. **26 of them are this session's**: 7 in the new `crates/prismd/tests/controls.rs`, 5 in `crates/prism-core/tests/settings.rs`, 6 in `crates/prism-surface/tests/bindings.rs` and 8 in `prism-domain`'s new `surface` module |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0, and **measured cold** — `cargo clean -p` on every changed crate first. Three lints were met that way and every one of them was invisible warm: an **`empty_line_after_doc_comment`** where removing a constant left its documentation with nothing to document, a **`doc_lazy_continuation`** where a dash starting a line read as a list item, and an `unused_qualifications` in a test |
| `cargo fmt --all --check` | ✅ exit 0 |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **715 tests in 50 files** (688 in 48 at S37); the build is **374 KB, 110.5 KB gzipped**, up 15 KB on S37 — and **this session adds no dependency** |
| Playwright | ✅ **39 tests** (34 before), all green |
| Coverage on what this session wrote | ✅ `ui/src/settings/` at **99.54 % lines**, with `controls.tsx`, `actions.ts` and `settings.ts` all at **100 % lines, statements and functions**. `prism_domain::surface` at **100 % on every column**. `prism-domain` **99.67 % lines** — *higher* than S37's 99.63 %, which took a detour worth recording below |
| CI green on the pushed commit | ✅ run **32649175306** on `863ac04` — all five jobs on the **first attempt**: Windows full build and test 12 m 58 s, Linux platform-neutral 4 m 46 s, UI end-to-end against a daemon 2 m 51 s, UI typecheck/lint/test/build 2 m 24 s, ARM64 cross-check 55 s. The Linux job is the interesting one again: `controls.rs` runs there too, over a **Unix domain socket** rather than a named pipe, so a key is rebound and learned on a machine that has never seen an X-Touch — and the five control-editor end-to-end tests passed on a Linux runner, which is a browser rebinding a key at a console nobody has ever plugged in |

**What was built, and where the line is.** A fifth panel in the settings window,
beside *Devices* on purpose: that panel names the desk's MIDI port and the file
its table was read from, and this one is what the table **says**. It draws all
seventy-three controls the surface has, what each does now, the whole vocabulary
of §4 for what it should do, whether the control stays PrismDMX's in the combined
Xctl+MC mode, and which one may never be bound at all.

#### Where the table lives, and why it is not the file

This was the decision the session prompt left open, and `docs/MCU_MAPPING.md`
§4.4 now carries the argument. It is **`prism_core::MachineConfig`**, for three
reasons in descending order of force: it is the same kind of fact as the rig
(S33) and the port (S36), so a show carried to another hall must not bring the
last hall's F-keys; it is written by one thread under one lock, which is what
makes *two editors, one table* structural rather than hopeful; and the shipped
`profiles/surface/xtouch.json` stays what a test has said it is since S22 — the
built-in defaults — which an editor writing to it would not, in a directory a
school's account often cannot write to anyway.

**A profile file is therefore an import.** Naming one replaces the stored table
and `Settings::surfaceProfile` becomes the record of where the table came from
rather than where it lives. The alternative — the file winning at every start —
has one consequence that decides it: an operator who rebound a key at the desk
would find it back the way it was the next morning.

#### S22's rule split in two, and the test that noticed

*A malformed profile never blocks anything* was one sentence while the built-in
defaults were the only other thing to fall back on. Once a desk has a table of
its own it becomes two: **the table in force stands**, which for a fresh desk is
the defaults and for an edited one is its own. Overwriting an operator's work
because of a typo in a file would be the one outcome worse than ignoring the
file, so `prismd::surface::read_profile` answers `None` where `load_profile`
answers with the defaults, and the caller decides which it needs.

The test that caught it is S37's own, and its **wording was already the stronger
claim**: *a broken profile leaves the desk with the keys it had*. It asserted the
built-in table, because until S38 those were the same thing. It asserts the keys
it had now, and a second case — a file that will not parse — was added beside the
one that is missing.

#### Two new things to say, and no new command

`MachineChange::SurfaceBinding` and `MachineChange::SurfaceLearn` are variants of
a *change* rather than commands of their own, and both halves of that are worth
recording. Conceptually they belong there: what this building's desk does is one
of this machine's settings, and `SurfaceProfile` — the file the table is read
from — has been in that enum since S37. Structurally they had to be there, and
that is the measurement below: two `Command` variants would have cost 1 120 bytes
of a budget four sessions have now been surprised by, while `MachineChange`
travels boxed inside `ConfigureMachine` and costs it nothing.

`is_machine_command` is therefore still six, `ShowFile::apply` and
`SessionState::apply` needed no new arm, and neither did the three example lists.

#### The value-tree budget was wrong in two ways, and both were measured

S37 made the stack budget a test. S38 found both of its blind spots the hard way,
and the numbers are all measured rather than reasoned:

- **It watched two of the four wire enums.** `Answer` went over the cliff while
  the test was green, because `Answer::SurfaceBindings` carries a
  `Vec<SurfaceControl>`. It watches all four now.
- **The budget sat above the largest tree ever shown to be safe.** S37 measured
  36 160 (overflows) and 30 992 (does not) and set 32 KiB between them; S38
  landed at **32 144**, in the untested gap — this test green,
  `prism-core`'s suite overflowing. It is **30 992** now.
- **And the remedy had run out.** Two bare **unit** variants added to `Command`
  cost **1 120 bytes** — 33 264 against 32 144 — so the tree grows at ~560 bytes
  per variant *whatever the variant carries*, and boxing a field cannot help once
  every fat field is a pointer. What is actually expensive is a **wide unit
  enum**: `GlobalButton` is sixty-four buttons carrying nothing and its derived
  value tree was **36 064 bytes**, larger than the whole of `Command`.
  `crate::arb::boxed` cannot reach it, because boxing replaces the subtree with a
  pointer and the subtree is still *built* as a local before the box takes it.

`crate::arb::arbitrary_from_list!` is what reaches it: a map over a range onto the
enum's own `ALL`. Measured, before and after:

| Type | Derived | With the list strategy |
|---|---|---|
| `GlobalButton` | 36 064 | **16** |
| `AttributeType` | 8 464 | **16** |
| `WindowType` | 6 224 | **16** |
| `ExecutorButtonFunction` | 4 512 | **16** |
| `FeatureGroup` | 2 832 | **16** |
| `BoundControl` (inherits `GlobalButton`) | 41 200 | **3 392** |
| `ExecutorButtonRef` | 5 120 | **1 152** |
| `MachineChange` | 81 440 | **12 064** |

It is also the **better generator**, which is worth saying so it does not read as
a workaround: the derive weights variants equally by construction and so does
this, and a variant added to the enum joins the strategy by joining `ALL`.
`WindowType` and `ExecutorButtonFunction` gained an `ALL` in the process, which
the control editor's chooser needed anyway.

One narrowing was still necessary and is marked as such: `MachineChange::SurfaceBinding`'s
two fields use `crate::arb::a_control` and `crate::arb::an_action`, which carry
**every** control and **every** action variant with one payload each. What is
narrowed is the combinations rather than the vocabulary, and `wire.rs`'s own
`bound_control` and `surface_action` properties still walk the full space with
nothing else on the frame.

#### The vocabulary moved and the note map did not

`BoundControl`, `SurfaceAction`, `ExecutorTarget`, `Step`, `GlobalButton` and
`StripButton` are `prism-domain`'s now and are re-exported from `prism-surface`.
They travel: a command carries one, an editor draws one, and the interface's copy
of each is generated from the domain. `IMPLEMENTATION_PLAN.md` S32 predicted the
split from the other side — an OSC control that fires a command belongs in the
same editor rather than in a second mapping system.

**§2.1's note and CC numbers did not move and must not**, which is the promise
that section makes in as many words. Nor did the codec, the shadow model, or the
resolution of an action into a `Command`: that needs an event and a session, and
the domain has neither. `BoundControl` gained struct variants in the move
(`Global { button }` rather than `Global(button)`) because serde cannot
internally tag a newtype variant whose payload is a string, and the whole
protocol is internally tagged.

The one place the move showed up as a *number* is coverage: measured per crate,
`prism-domain`'s new module read **12.37 %**, because its types are exercised from
`prism-surface` and `prismd`. A type's own crate should test it, so `surface.rs`
gained eight tests of its own and reads **100 % on every column**.

#### Learn, and the two things it has to be careful about

While learn is armed the control **does not fire**. An operator finding out what
Record is called would otherwise clear their programmer to find out, and one
learning a transport key would start a cue on a stage.

The release goes with the press. Learn is one shot, so the press names the
control and disarms — and the button's release arrives on a later poll with learn
already off. A release is not nothing: `SurfaceAction::is_momentary` forwards both
edges of an `ExecutorButton`, so letting it through would release a `Flash` that
was never held. `SurfaceLink::swallow` is that one button, remembered until its
release has been eaten.

Touch is deliberately not learnable: a hand landing on a fader produces no command
of its own, and the `Moved` that follows names the same fader.

#### Three bugs this session made, and what found each

Worth recording together, because none was found by reading:

- **A rebinding took up to half a second to reach the desk.** The new table was
  handed over on the housekeeping tick; the browser test pressed the key and
  nothing happened. It is taken on the **surface** tick now, beside
  `take_surface_change`, which is where a port change has always been picked up —
  so a key does the new thing within a millisecond of *Bind it*. Found by
  `ui/e2e/controls.spec.ts`.
- **Adopting the table at start-up reset the surface's counters.** A desk that has
  never been told writes down what it started with, and that went through
  `replace_bindings`, which hands the table to the surface — rebuilding the
  `SurfaceLink` and throwing away the shadow model **and the six feedback
  counters** a settings panel draws. `Core::adopt_bindings` records without
  handing over. Found by S22's
  `surface_gate.rs::pressing_smpte_beats_does_nothing_at_all_and_is_counted`,
  which read a `reserved` count of 0 where it had pressed the button twice — and
  found it **flakily**, one run in four, because it depended on where the
  housekeeping tick fell.
- **A dead accessor.** `MachineConfig::clear_surface_bindings` was written and
  never called. Found by reading the coverage report, which is the one thing in
  this list that a person did.

#### The six recordings, and every difference in them

Regenerated as a set (`cargo test -p prismd --test ui_* -- --ignored`) and the
diffs read **structurally** — a script that base64-decodes each blob, unpacks the
MessagePack inside it and walks the two documents in parallel. **192 differing
paths, and every one is one of six things.** Nothing structural changed in any of
the six, which is the claim that matters: the protocol grew, and no existing
message did.

| What differs | Occurrences | Why |
|---|---|---|
| `snapshot.health.tickHz` | 32 | A measured rate. Expected to differ between runs |
| `snapshot.outputs[].framesSent` | 31 | A driver's own counter over wall-clock time |
| `snapshot.machine.dataDir` | 5 | **A per-run value, and S38 is the first regeneration since S37 added the field**: every recording runs in a fresh temporary directory |
| `snapshot.machine.deskId` | 5 | The same, one field along: a fresh data directory has no identity, so the daemon generates one |
| `snapshot.showFile.path` | 5 | The same again — the show lives in that directory |
| Where a `PlaybackState` lands in a step's delta list | 4 paths over 2 steps of `daemon-recording.json` | **S34's asynchronous readback**, the class S37 also recorded: the recorder stops at the `Ack` and the tick's answer arrives a poll later |
| `telemetry-recording.json`'s frame sequence | 2 | The frame counter, and the payload differs in **exactly one byte, at offset 8** |

The three middle rows are a new *class* rather than a new kind of surprise: S37
added `snapshot.machine` and `snapshot.showFile` and recorded them as *absent
before, present now*; this is the first regeneration since, so S38 is the session
that finds out three of their fields are per-run values.

#### The decoder gap S37 named is closed by a test, not by a recording

S37 carried out of itself that *a delta or an answer with no caller is a decoder
arm nobody writes*, and asked that a recording send any new message. None of the
six can: every one runs a daemon whose rig came from its command line, and such a
daemon refuses **every** machine command by design (`MachineError::ConfiguredOnTheCommandLine`).

What closes it instead runs on every commit. `ui/src/settings/controls.test.tsx`
delivers both new deltas and the new answer through a fake socket that encodes
**genuine MessagePack**, so `readDelta`'s two new arms and `readAnswer`'s one are
driven by the real decoder rather than constructed in TypeScript. That is the
property S37 wanted; the recording was one way to get it and not the only one.

#### The numbers that had to stay where they were

| Gate | Result |
|---|---|
| Zero allocations in the tick | ✅ **0 on all eight paths** — `crates/prism-engine/tests/tick_allocations.rs`, unchanged. Nothing this session went near the tick, which is the point: the engine is never told what a desk's keys do |
| Tick deadline | ✅ **0 missed ticks** across every binding change, learn and profile reload in `crates/prismd/tests/controls.rs` and `settings.rs` — asserted inside them rather than measured beside them |
| Telemetry frame budget | ✅ **0.20 ms median, 0.70 ms p99** for decode and paint together, 64 universes at 29.9 Hz over 153 frames, against a budget of 8 ms |
| Zero React commits | ✅ the `<Profiler>` gate is one of the 715 and it passed — with a control editor in the tree, which is the point of re-running it |
| `cargo test --workspace` | ✅ **1 927 tests**, 69 targets, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅, measured **cold** |
| `cargo fmt --all --check` | ✅ |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ **715 tests in 50 files**; 374 KB, 110.5 KB gzipped |
| Playwright | ✅ **39 tests** |
| CI green on the pushed commit | ✅ run **32649175306** on `863ac04`, all five jobs on the **first attempt** |

---

### 2.41 S43 verification record

Measured on 2026-08-28. **This record is open**: S43 is the first *interactive*
session and its third movement ends when the owner says the interface is fit for
a pre-release, which they have not yet said. What is written here is what has
been built and measured; the exit criteria that depend on the owner's own
judgement are marked as such rather than ticked.

The session has three movements and a fourth still to come: the questions before
building, the building, the owner driving it by hand, and the tests brought back
to `CLAUDE.md` standard. Three rounds of hand-testing have happened and every
item of all three is answered — `docs/PRERELEASE_PUNCHLIST.md` B1–B34 is the
list, one entry per fault, each with what was wrong, how to see it, and what was
done.

| Check | Result |
|---|---|
| **The interface follows the owner's drawing** (`design/skeleton/`) | ✅ built, with every departure written down and reasoned — `ARCHITECTURE_SPEC.md` §4.6. The bands are gone: the executor strip, the console's keys and the status readings are windows, and what is left outside the canvas is a header, the command line and a programmer band of `clamp(9rem, 20vh, 13rem)` |
| **Nothing scrolls outside the canvas** | ✅ measured in Chromium at 1280 × 720 **and** at 4K, on a rig of forty fixtures and a cue list of four hundred rows: `document.documentElement`, `[data-testid="canvas"]` and `[data-testid="programmer-band"]` all report `[0, 0]` of overflow, and the sheet's own body reports more than zero — a check of zeros everywhere would pass for a sheet that had clipped its rows |
| **Zero React commits under telemetry** | ✅ the `<Profiler>` gate is one of the 784 and it passed over the rebuilt tree |
| **The canvas lays itself out** | ✅ `prism_core::layout` — a new window takes the first free place and shrinks before it is refused; a move is accepted only when it overlaps no more windows than the one it replaces, so a layout stacked at the origin by an older build can still be taken apart and a clean one cannot be spoiled |
| **The window chooser is reachable three ways** | ✅ `Insert` (ignored inside a text field, deliberately), a right-click on the canvas, and an X-Touch key. Measured live at 1152 px: six columns, one key per `WindowType`, each with a sentence saying what the window is for |
| **The fixture library is a table** | ✅ B23. Measured live against the installed library: `14 shown of 2161 profiles`, five columns — manufacturer, fixture, mode, channel count, and whether the show already carries it |
| **Group selection adds rather than toggles** | ✅ B27, and the provenance is the daemon's (`selectedGroups`, `manualSelection`). Measured live: `2` → `2+3` → `2+3+1` → back to `2+3`, with fixture 1 released and everything two groups still hold kept |
| **The pools append to the command line** | ✅ B33, `consoleshell.ts::pickOnto`. Measured live over all six sources: `Store` plus a group tile gives `Store Group 2` and the console's own mode question; `Copy` is two clicks; `Assign Sequence 1` plus an executor strip puts *Act one* on executor 2; `Delete` plus a cue row removes the cue; `Label` plus a group is **left standing**, because a click that deletes a name is the worst kind of shortcut |
| **The store bars are gone** | ✅ B32, both pools, and nothing was lost: the bar built a line and sent it, and could do **less** than the line it built |
| **A rig comes up dark** | ✅ B34, and this one is a fault the tests found and the eye had missed. Read off the telemetry canvas rather than argued: three RGBW PARs stood at 255 on twelve channels at rest, 1672 pixels of meter at full, unchanged by every programmer command. B1 had given colour channels a home value of full, and a fixture with no dimmer has nothing to hold them down. The desk supplies the missing channel — `docs/DMX_MERGE.md` §5.1 |
| `cargo test --workspace` | ✅ exit 0 — **1 967 tests**, 0 failed (1 927 at S38) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test` | ✅ **784 tests in 54 files** (715 in 50 at S38) |
| Playwright | ✅ **41 tests** (39 at S38), all green against a real `prismd` in mock-output mode |
| Coverage | ✅ **97.47 % lines** over the workspace (target ≥ 85 % global) and **97.66 %** on `ui/src`; engine 99.29 %, programmer 97.57 %, protocols 97.49 %, all three above `CLAUDE.md`'s stricter target — §3. Two files of this session's own came in under the house norm and were closed rather than recorded: the preset pool's context menu had no test at all (59.25 %) and the Clock Viewer had no test file (78.57 %); both read **100 % lines** now |
| CI green on the pushed commit | ✅ run **33225077470** on `959909c` — **all five jobs on the first attempt**: Windows 13 m 35 s, Linux neutral 4 m 54 s, UI end-to-end 3 m 21 s, UI typecheck/lint/test/build 2 m 17 s, ARM64 check 47 s. The feature commit itself was run **33209652373** on `fcb884d`, 13 m 02 s, also green on the first attempt; the two red runs between them are written up below and neither was in the code that commit shipped |
| **The owner calls it fit for a pre-release** | ☐ **not yet given.** Movement 3 is not finished |
| **The §7 *carried out of* lists, one entry at a time** | ☐ **outstanding** — S23–S40, each entry needing one of the three outcomes |

#### What the hand-testing found that the building did not

Three rounds, and their value is in what they caught rather than in what they
confirmed.

**Round one** was nine items and most were what they looked like: a fixture field
that was *sehr unübersichtlich*, a window chooser that was one long column, sixty
-odd unstyled buttons, a Controls page where nothing lined up. Two were more than
appearance. The Cue Viewer became a cue per row and an attribute per column, and
the three pools became grids with one shared context menu — which changes what
the windows **are**: the Sequence Sheet is a pure selection surface now and every
cue edit happens in the Cue Viewer.

**Round two** was two sentences and it finished `ARCHITECTURE_SPEC.md` §4.5
rather than extending it. The store bars went, and every data source now offers
its click to the command line first. The rule had already been *every key writes
a line*; what was missing is that a pool is a key.

**Round three** was the owner's answer to a question this session had to ask —
below.

#### The fault the eye could not see

The last two end-to-end failures did not look like a fault at all. A test that
counted pixels of *full* on the telemetry canvas and expected zero was getting
1672, and the count did not move when the programmer did. Reading the canvas
directly — a throwaway Playwright spec that dumped the colour histogram and the
lit columns — said the picture was **identical before and after `red at 100`**,
because those channels were already at full.

Twelve channels: three RGBW PARs of four. Punch-list **B1**, from round one, had
given colour channels a home value of full on the owner's own argument that a
colour starts open and the dimmer decides whether any of it is seen — and an RGBW
PAR has no dimmer, so *colour open* and *lamp at full* are the same eight bits. A
rig of them came up white the moment the daemon started.

`design/skeleton/README.md` says in as many words that a mismatch of this kind is
a question and not a decision, so it was asked. The owner's answer: **fixtures
without a dimmer get a software-controlled one, switchable off per fixture in the
patch.** Built as a merge slot with no DMX channel that rests at nought and
scales the fixture's colour on the way out — which, for a fixture whose only
output is its colour, *is* its intensity.

Two things fell out of it worth recording. A profile that files its dimmer on the
**colour** bank still has one, and a desk that supplied it a second would name
the same attribute twice — `MergeError::DuplicateAttribute`, which is a rig that
will not patch at all. A unit test carrying exactly that profile caught it, and
the predicate now asks about the attribute rather than about the bank. And the
price is real and is the right one: on a colour-only fixture `1 red at 100`
colours and `1 at 100` makes light, exactly as on a fixture with a dimmer of its
own. Three end-to-end tests were bringing no dimmer up and have been pulled
along.

#### And a bug the end-to-end suite found in the control editor

`surfaceLearned` is described in the store as *a moment rather than a state* and
was kept as a value. The daemon broadcasts the control learn named to **every**
client, so it was still standing there when the next row armed learn — and the
arming bound it, before anybody had pressed anything, from a screen that had
nothing to do with the press. Two operators are where it bites: the second one's
Learn silently takes the first one's key. Found by
`ui/e2e/controls.spec.ts::two editors change two keys and there is one table`,
which is the test written for exactly that pair and had been passing for a
different reason. `ControlsPanel` now remembers what was in force when it armed
and ignores it; the regression test is `does not bind the key the last learn
named when a row arms`.

#### The Windows job went red on a documentation commit, and it is S37's open item

The commit after the feature one changed **no Rust at all** — markdown and two
UI test files — and `crates/prismd/tests/surface_gate.rs::a_surface_that_goes_away_is_reported_and_the_show_carries_on`
failed on it, with `missed()` reading one where the test demanded the number it
had before. That is the **same test, the same line and the same symptom** S37
recorded in §2.39, and this is its second red run.

S37 wrote down what to do about it and deliberately did not do it: *the honest
fix is not a tolerance but a control — compare the missed ticks across the replug
against the missed ticks across an equally long window with no replug, which is
the shape S18 used for the D2 gate. That is a change to S36's test and belongs to
whoever next has reason to open it.* S43 had the reason.

**The runner measures itself first.** A control window of 400 ms with no cable in
it is what this machine costs when nothing at all is happening; the replug window
has to be no worse than that. The one tick of slack on top is the isolated hiccup
both failures were — and what carries the claim is not the slack but the
**difference in kind**: a surface that had got onto the tick thread would lose
the redraw's worth of ticks and not one of them. A second assertion stands beside
it, because either alone is passed by a daemon broken in the other way: the tick
count must have *moved*, or a daemon that lost no ticks by not ticking would
pass.

Two things this does **not** claim. It is not a real-time gate — that lives in
§3, ten minutes at 64 universes under 100 % CPU load with 0 of 26 401 ticks
missed — and it never had much power to catch a regression, because a surface
poll that blocked the tick for five milliseconds would fit inside the 22.7 ms
budget and miss nothing. What it is, and now reliably is, is a canary for gross
blocking.

#### And then the end-to-end job went red on something else, which was worth finding

The next run failed on `desk.spec.ts`'s *the screen is a device screen at 4K,
with every window open* — `expect(types.length).toBeGreaterThan(10)` receiving
**0**. The 1280 × 720 case beside it passed.

It is a race this session's own reading of the test should have caught. The
window chooser is a React render away from the `Insert` that opens it, and the
line underneath read the DOM straight afterwards with `evaluateAll` — which,
unlike a locator action or an assertion, **does not wait**. On a two-core runner
under load the read won, the list came back empty, and the test reported that a
build generating its chooser from `WINDOW_TYPE_VARIANTS` has no window types.

**The second fault is the one worth the entry**, because it was making the test
weaker every time it passed. The loop underneath guarded its click with
`if ((await key.count()) > 0)`. The chooser lists every type, every time, so that
guard can only be false when the chooser is *not there* — a render that had not
happened, or an `Insert` swallowed because a window had taken the focus. Under it
the test opened nothing, found nothing outside the canvas scrolling, and went
green having checked one window instead of fourteen. It is the same shape as the
overflow probes that were measuring `null`: **a vacuous green is the expensive
kind**, and this one had a guard written round it on purpose.

`openPicker` in `e2e/daemon.ts` now leaves the text field, presses `Insert` and
**waits for the chooser**, once, for every caller; the click is unconditional.
The measurement that says it was worth doing: the 4K case went from 1.1 s to
7.7 s, which is the fourteen windows it had been claiming to open.

#### And a third red run, which is the same sentence a third time

The commit recording the green run — two lines of this file — went red on
`looks.spec.ts`'s preset test at `closeWindows`:
`expect(page.locator("[data-window-type]")).toHaveCount(0)` receiving **1**.

The helper took `count()` once and clicked `first()` that many times. **Closing a
window is a command out and a `SessionPatch` back**: the canvas is the daemon's,
so the key going down proves nothing about the window going away. On a fast
machine each delta arrived before the next click; on the runner one did not, the
click landed on a window already on its way out, and the last one stayed open.
That is **D3 turning up in the test suite** — the same rule that says no client
has a second opinion says no client may assume its gesture has landed.

It now asks for the count to *drop* before clicking again, which is what
`toHaveCount` waits for.

**And a sweep rather than another single fix.** Every non-waiting read in the
end-to-end suite was walked — `count`, `evaluate`, `evaluateAll`, `textContent`,
`innerText`, `getAttribute` — against what precedes it. All but one were already
preceded by a waiting assertion, which is the shape that makes them safe;
`settings.spec.ts`'s output loop ends each row with `toBeVisible`, and
`controls.spec.ts`'s two counts are a negative assertion *after* a positive wait,
which is right. The one that was not is `desk.spec.ts`'s device-screen probe: it
waited for the **first** window and then measured all of them. That would never
have gone red — it would have measured however many had landed, which is the
quiet kind of wrong this session has now found three times. It waits for the
count to stop moving.

#### Two stop-gaps, and both say so

**The send-it checkbox** (B25). A custom key can write a line or write and send
it. Sending it means *running* it, and the parser lives in the interface — so the
daemon bumps `Session::commandLineRun` and the client holding the keyboard focus
runs what is already in `commandLine`. A counter and not a flag, because two
Enters on the same text are two commands. **S49** exists in
`IMPLEMENTATION_PLAN.md` to move the parser into the daemon and take the field
away.

**The OS file dialogues** (B31). Moved to **S29** on the owner's own stated
condition, and the reason is not convenience: `showOpenFilePicker` hands a
browser a *handle*, never a path, and the daemon is what opens the file. The
control map is the exception and is done (B26) — a key table belongs to the
machine the operator is sitting at, not to the one running the show.

### 2.42 S45 verification record

Measured on 2026-08-29. Two punch-list entries that land on the same model, and
the sentence that came out of putting them together: **an executor is a handle on
a cue list's playback, and which handle it is, is editable.**

| Check | Result |
|---|---|
| **B18 — one sequence, one playback, asserted on frames** | ✅ `crates/prismd/src/core.rs`. Two executors on cue list 7, both `Master`: executor 3's Go takes channel 5 to **255**, executor 4's fader to 32 768 takes it to **128**, executor 3's fader back to full returns it to **255** — one number, two handles. Before this session those were two `PlaybackSource`s and the second fader moved nothing an operator could see |
| **B18 — different fader functions stay independent** | ✅ a `Master` and an `XFade` on the same list: the crossfade takes the transition's clock off the engine and the light moves, the show's `masterLevel` is **untouched at 65 535**, driving the crossfade to its far end completes the cue back to **255**, and the master then scales *that* to **128**. The two compose rather than one being the other |
| **B18 — `Go` on either handle advances one cue pointer** | ✅ On from executor 3 puts the list on cue index **0**; `Go+` from executor **4** moves the same pointer to **1** and the light to **78** (cue 2 is 20 000/65 535 of full). A second playback would have entered cue 1 again and held channel 5 at 255. Both executors' rows say `1`, because there is one row and both read it |
| **B15 — every function is assignable, three ways, one state** | ✅ and it is one path rather than three kept in step. The window writes `Assign Executor 1 Fader Master` and sends it (`ui/src/desk/executoreditor.test.tsx`); the line parses to one `ConfigureExecutor` (`ui/src/desk/console.test.ts`); a bound X-Touch key carries the same line unchanged (`crates/prism-surface/tests/bindings.rs`). **No `SurfaceAction` was added**: a key that wants to say something the grammar can say already has a way to say it |
| **B15 — the custom row survives a save and a restart** | ✅ `a_custom_row_survives_a_save_and_a_restart`: a key given `Go+ Sequence 7`, saved, and read back off the platter — the line is there, the fixed function on key 1 is still `On`, and the fader is still `Speed`. The eight fixed functions are bare strings in the MessagePack document and the ninth is externally tagged, which is why it was checked rather than assumed |
| **B15 — firing it produces what typing produces** | ✅ `a_key_with_a_line_on_it_fires_the_line`: pressing the key leaves `Session::commandLine` at `Go+ Sequence 7` and `commandLineRun` one higher — **byte for byte what `Command::CommandLineInput { run: true }` leaves**, which is what a keyboard sends. It is `SurfaceAction::WriteCommandLine`'s stop-gap reused rather than a second mechanism, and S49 removes it for both at once |
| **A `.prism` file written before S45 opens and means the same thing** | ✅ the frozen version-1 fixture round-trips, and the level its executor carried is on the cue list: `sequence(5).masterLevel == 40 000`. `ShowStore::carry_levels_onto_their_cue_lists` reads the executor row a second time as `LegacyExecutorLevel` — the half a `#[serde(default)]` on the sequence cannot do. What is *not* migrated is `isActive` and `currentCueIndex`: a show reopens with nothing running, which is what `Sequence::is_active` has said since S40 |
| **The X-Touch follows** | ✅ the strip's lower line is the legend — `><x- M` for *Go, Back, Off, nothing, master fader* — so a reassigned key relabels the strip on the next diff, at most 33.3 ms later and inside S26's frame budget. The motor fader stands at the number its own function names, and because that number is the cue list's, a level moved from one executor moves the other's. The departure from `docs/MCU_MAPPING.md` §4.1's *value* column is written down there with its reason: the percentage was what the motor fader under it already showed |
| **Nothing scrolls outside the canvas** | ✅ `session.spec.ts`'s device-screen test at 1280 × 720 **and** at 4K, with the `Executors` window among the fourteen it opens: `document.documentElement`, the canvas and the programmer band all report `[0, 0]` of overflow. The window's own body is the one scroller — `.executor-window { overflow: auto }`, with the editor capped at 40 % of it |
| `cargo test --workspace` | ✅ exit 0 — **1 980 tests**, 0 failed (1 967 at S43) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| `ui`: `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build` | ✅ all four. **797 tests in 56 files** (784 in 54 at S43); the build is 400.68 kB of JS, 29.27 kB of CSS |
| Playwright | ✅ **43 tests** (41 at S43), all green against a real `prismd` in mock-output mode. The two new ones are B15 and B18 in a browser |
| No allocator call in the tick | ✅ **the eight paths**, unchanged — `crates/prism-engine/tests/tick_allocations.rs`, 9 tests green. The session touched the tick in two places and neither allocates: `TickCommand` lost the kind byte it grew in S40, and the playback set is built per cue list rather than per executor at *rebuild* time, which is a core-thread job |
| Coverage on the new `prism-core` and `prism-engine` code | ✅ engine **99.29 % lines** and core **97.32 %**, both above 95 %; the programmer `CLAUDE.md` names by name reads **97.03 %**. `store.rs` came out at 93.71 % and was read line by line rather than rounded up: two branches of the new migration — the *two executors on one list* tie-break and a legacy row carrying `speed` — were unreachable from any test, and now are not (94.30 %). Workspace **97.47 % lines** over 65 173, unchanged to two decimal places; `ui/src` **97.60 %** over 797 tests, with both new files at **100 % lines**. Measured with `cargo llvm-cov clean --workspace` between crates, per §3's caveat |
| CI green on the pushed commit | ✅ run **33261331368** on `0566bc1` — **all five jobs on the first attempt**: Windows full build and test 12 m 53 s, UI end-to-end against a daemon 3 m 37 s, Linux platform-neutral 4 m 48 s, UI typecheck/lint/test/build 2 m 04 s, ARM64 cross-check 35 s. The Linux job is the one worth reading for this session: `prism-core`'s and `prism-engine`'s whole suites run there, so *one sequence, one playback* is asserted on frames on a machine that is not this one |
| The wire budget | ✅ `a_generated_wire_value_fits_in_a_test_thread` green at **30 992 bytes**. `ConfigureExecutor` is one variant with a boxed field, and collapsing `PlaybackId` from a two-arm enum to a newtype gave back more than the variant cost |

#### The decision this session existed to make

**`Command::ConfigureExecutor`, not a `MachineChange`** — and both precedents
were real, which is why the plan called it a decision rather than a derivation.
What an executor *carries* is show content (S11: a show survives the hall it was
written in). What a desk's keys *do* is not (S38: a show carried to another hall
must not bring the old one's F-keys). An executor is both at once.

What settles it: **an executor is not a control of this building.** It is one of
the six numbered things `ObjectRef` names; it is copied, moved, labelled and
coloured by show verbs; it has been written into the `.prism` file since S1; and
a Web Remote with no X-Touch anywhere near it still has eight of them with four
keys each. S38's table binds *hardware* — this strip's third key — and
`ExecutorButtonRef` has kept that apart from *what the third key does* since S34:
a profile row names a **position**, and the executor decides what a press at that
position means. S45 made the second half editable and left the first where S38
put it.

The consequence is what makes the choice testable rather than tidy: an executor's
assignment **travels with the show**. Carry a show to another hall and its Go
keys are still Go keys; open it on a desk with no X-Touch and they still are. The
argument is written out in `docs/IPC_PROTOCOL.md` §5 and summarised in
`ARCHITECTURE_SPEC.md` §4.4.

#### Four things the session found

**A playback keyed by executor could not be fixed by rejecting the second one.**
The obvious repair for B18 is to refuse a second executor on a sequence. It is
wrong twice: an operator who wants a Master and an XFade on one list is asking
for something reasonable, and the entry says so in as many words; and refusing
would leave `On Sequence 1` and a Go on the fader as two different playbacks
still, which is the half S40 had already had to work around with *the two are
never both live for one list*. Moving the state is what makes both sentences one
sentence.

**Taking a cue list off a fader must not stop it, and that fell out rather than
being decided.** `AssignExecutor` answered `Effect::ExecutorOff` when the slot was
cleared, because the executor *was* the playback. With the playback on the list,
the set of them does not mention slots — so `AssignExecutor` reaches the engine
**not at all**, and neither does an Oops that puts an executor row back. That is
§6.1's rule arrived at from the other side: an undo must not stop a list somebody
is watching.

**A dependency that was narrow enough stopped being narrow enough.** The cue
viewer asked `Query::StorePreview` again whenever `/sequences` moved, which S34
had made safe by putting the playback state on the executor. Putting it back on
the sequence made a cue advance rewrite that subtree — one question per cue of a
chase, which is exactly the warning S28 left. The dependency is that list's
`cues` node now (`looks.ts::cuesDocument`). **The general shape: a React
dependency on a document subtree is a claim about what else writes into it, and
moving a field can invalidate it silently.** The test that caught it was S39's
own — *does not ask again when a playback advances a cue*.

**A generated table stopped being generated, and the fix was in the generator.**
`ExecutorButtonFunction` gained a variant with a field, so it is no longer a pure
union of string literals and `prism_domain::export::string_union` dropped
`EXECUTOR_BUTTON_FUNCTION_VARIANTS` — which four TypeScript files import to
*check* a string they were given. Writing the eight out by hand would have been a
second list. `string_union` **skips an object arm** instead: the literals are
still a table and the object is not a choice. A union of only objects still comes
back `None`, so `Command`, `PlaybackTarget` and `ExecutorButtonRef` are unchanged.

---

### 2.43 S46 verification record

Measured on 2026-08-29. A session about one sentence in a settings panel that was
not true, and the receive path it took to make it true: **an Art-Net output now
knows whether anything is listening.**

| Check | Result |
|---|---|
| **B6 — a configured node that never answers never reads OK**, over the protocol | ✅ `crates/prismd/tests/artnet_nodes.rs::a_configured_node_that_never_answers_never_reads_ok`. An Art-Net output on universes 1 and 2, addressed to a node that says nothing: the driver reports `Ok` — the socket takes every datagram, which is the whole of the entry — and `Query::OutputStatus` answers **`Degraded`** with one node row reading `NeverAnswered`, no name and no age. The snapshot every client is handed says `Degraded` too, so a delta and an answer cannot disagree about one output |
| **B6 — the same claim where it is decided** | ✅ `crates/prismd/src/outputs.rs::an_art_net_output_whose_node_never_answers_is_not_reported_ok`. The driver's health is `Ok` and `reported_health` is `Degraded`; the `Mock` output beside it is untouched at `Ok` with an empty node list, because nothing else this desk drives has an answer-back. When the node answers, the row goes green in the same test |
| **B6 — and not out of ignorance** | ✅ `a_desk_that_is_not_listening_reports_the_drivers_own_health`. A supervisor with no discovery reports the driver's `Ok` and says **nothing at all** about the nodes: reporting `Degraded` because this desk cannot hear is B6 pointed the other way. `a_disconnected_output_stays_disconnected_whatever_the_nodes_say` holds the other end |
| **A mock node that answers is discovered, named and shown** | ✅ three times over, at three depths. Against a double: `discovery.rs::a_node_that_answers_a_poll_is_discovered_and_named` — one poll to the one configured node, fourteen bytes, opcode `0x2000`; the reply comes back and the node has a name, a MAC and a port-address table of `0:0:0` and `0:0:1`. Over a **real loopback socket**: `artpoll_wire.rs::a_node_on_a_real_socket_is_polled_answered_and_discovered`, which reads the poll off the wire field by field and the reply back through the operating system's stack. Over the **protocol**: `artnet_nodes.rs::a_node_that_answers_is_discovered_named_and_shown` |
| **One that stops answering is *stopped*, with the time, inside one poll interval** | ✅ `a_node_that_stops_answering_is_stopped_within_one_poll_interval`, on `ManualClock`. Answering at the timeout (3 500 ms) and **`Stopped` one millisecond later**, with `lastReplyAgoMs` = 3 501 — the age, from the daemon. The poll interval is 3 000 ms, so the report is inside one. The node keeps the name it gave, because *there was a node here and it stopped* is the fact an installer is chasing |
| **Discovered is not configured, and where the two disagree** | ✅ `artnet_nodes.rs`. A node outputting port addresses 0, 1 and 2 while this desk sends it only 0 and 1 answers with `unaddressedPorts: [2]` and `missingPorts: []`; a desk that re-addresses universe 1 to port 9 on a node that has only port 0 answers `missingPorts: [9]`, `unaddressedPorts: [0]`. Both are the **daemon's** arithmetic — a client that intersected the rig with the table would be `PatchPreview`'s trap one panel along |
| **A node nobody typed an address for** | ✅ `a_node_nothing_is_addressed_to_is_listed_and_marked_unconfigured`, and `an_unsolicited_reply_on_a_real_socket_discovers_a_node_nobody_configured` over a real socket: the announcement a node sends at power-up is heard because the socket is bound to Art-Net's own port, and **no poll went out at all** (`polls_sent == 0`) |
| **One click makes it an output** | ✅ `ui/src/settings/settingswindow.test.tsx::lists what is out there, where it disagrees, and adds one in a click`. The click fills the **form** and sends no command — a panel does not change the rig of a running show on somebody's behalf — with the address, the name and the universes already in it. The universes are `suggestedUniverses`, which is the daemon's: the default mapping run backwards (`ArtNetNodeInfo::universe_for_port`), because spelling `port + 1` in TypeScript would be a third copy of a rule `ARCHITECTURE_SPEC.md` §7.0 states and `prism-domain` implements |
| **Replies are parsed under `prism-surface`'s harness: no panic** | ✅ `crates/prism-protocols/tests/artpoll_fuzz.rs`. **250 000+ random bytes** in 2 000 datagrams: **0** were read as replies, which is the interesting number — and 2 000 more with a valid eight-byte header and a random body, every one of which was read and none of which panicked. Plus a `proptest` generator next door in `artpoll.rs` over 512 cases of anything at all |
| **…no allocation storm** | ✅ measured, not asserted, with `prism-surface`'s counting global allocator. 64 well-formed replies → **0 allocator calls**; **4 096 hostile datagrams → 0**. `ArtPollReply` is plain data: the three name fields stay as the byte arrays they arrived in and are handed out as `&str`. The guard at the end proves the probe can still see an allocation. A **settled table** costs nothing either: 256 idle service passes → **0**, and 256 repeat replies from one node → at most one call apiece, which is the mock releasing the buffer it was handed |
| **…and a malformed reply is dropped rather than believed** | ✅ the accounting identity, `prism-surface`'s shape: over **2 048 datagrams** spread across 32 source addresses into a table bounded at 8, `replies + malformed == delivered` exactly, the bound was reached, and no datagram was silently swallowed. The truncation boundary is checked at **every prefix length** from 0 to 239: below 207 dropped, at or above it read |
| **No test opens a real socket on a real network** | ✅ every socket in this session is `127.0.0.1` with a port the operating system chose, or a `MockUdpNode`, which is a queue. **Nothing binds `0.0.0.0`** and **nothing broadcasts** — `nothing_here_asks_for_broadcast_permission` asserts the flag at the socket rather than in a comment. One test found the exception by failing: see the decision log |
| **The tick is untouched, asserted on the frame sequence** | ✅ the discovery is a thread of its own and the tick's eight allocation paths are unchanged — `crates/prism-engine/tests/tick_allocations.rs`, **9 tests green, 0 allocator calls**. Nothing in `prism-engine` changed this session, and no `DmxOutput` implementation did either: the receive seam is a second trait, so `ArtNetOutput`, `SacnOutput` and `OpenDmxUsb` compile unchanged |
| **A socket that will not bind degrades alone** | ✅ at both layers. `discovery.rs::a_socket_that_will_not_bind_degrades_alone`: every pass afterwards is a no-op rather than an error a second, the counters stay at zero, and it can be opened later. `prismd::discovery::a_socket_that_will_not_bind_leaves_a_reason_and_no_thread_that_pretends`: the table says `listening: false` with the reason in the daemon's own words, and the daemon goes on running |
| **Coverage on the new protocol code** | ✅ **`artpoll.rs` 100 % lines**, 99.57 % regions, **100 % functions**; **`discovery.rs` 99.41 % lines**, 99.70 % regions, **100 % functions**. `udp.rs` first read **92.06 %** — the whole node seam was exercised only from `prismd` — so it gained nine tests of its own and reads **99.25 % lines**, which is S38's rule kept: a type's own crate tests it. `prism-protocols` **98.38 % lines** overall, 97.81 % regions, 97.49 % functions |
| **Coverage elsewhere** | ✅ `prism-domain` **99.58 % lines**, 98.58 % regions, 99.53 % functions — `output.rs` first read 97.38 % for the same reason and is now **100 % on every column**. `prismd` 94.65 % lines with `discovery.rs` at **96.71 %** and `outputs.rs` at **97.92 %**; the fifteen lines left in `discovery.rs` are one branch — the operating system refusing to start a thread — and they are named rather than rounded up. Workspace **97.98 % lines**, 97.53 % regions, 98.01 % functions over 43 254 lines, measured in one run after `cargo llvm-cov clean --workspace`. `ui/src` **97.62 % lines**, 91.26 % branches, 98.31 % functions over **805 tests in 56 files**, with `outputs.tsx` at **99.24 %** |
| **The gates** | ✅ `cargo test --workspace` **2 062 tests, 0 failed** across 72 test binaries; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all --check` clean; in `ui/`: `npx tsc -b --force`, `npm run lint`, `npm run test` (805) and `npm run build` all clean |
| **Playwright** | ✅ **44 tests** (43 at S45), all green against a real `prismd` in mock-output mode. The new one is the half of B6 a browser can answer: the node list never appears without saying whether this desk is listening, and the reason is the **daemon's own words**. The telemetry budget was re-measured through the same run: **`64 universes · 30.3 Hz · paint 0.20 ms (p99 0.90 ms) · 155 frames`** |
| **CI green on the pushed commit** | ✅ run **33272829395** on `f453c1b` — **all five jobs on the first attempt**: Windows full build and test 13 m 52 s, Linux platform-neutral 4 m 46 s, UI end-to-end against a daemon 3 m 34 s, UI typecheck/lint/test/build 2 m 14 s, ARM64 cross-check 41 s. The Linux job is the one worth reading for this session: `prism-protocols`' whole suite runs there, so the poll that leaves a real socket, the reply that comes back and the fuzz harness's **zero** allocator calls are all measured on a machine that is not this one |
| **The wire budget** | ✅ `a_generated_wire_value_fits_in_a_test_thread` green at **30 992 bytes**, unchanged. `Query::ArtNetNodes` is a unit variant and `NodeHealth` is a three-value unit enum, so neither adds a slot worth measuring; `Answer::ArtNetNodes`'s three fields are behind `crate::arb::small_vec(2)` |

#### The two decisions this session existed to make

**Where a discovered node lives.** The prompt put it as a choice between two
precedents and both were real. The rig is `prism_core::MachineConfig` (S33)
because it is a decision somebody made and a file has to remember it; the MIDI
ports are a `Query` (S36) because they change when a person moves a plug. A
discovered node is the second shape and unambiguously so: it changes while
nobody does anything, no command causes it, and it is gone at the next start.
Writing it into the configuration would mean a `machine.json` that changes
because somebody switched a node off.

So it is **`Query::ArtNetNodes`**. One thing differs from `MidiPorts` and is why
this could not simply be *enumerate on the asking thread*: **there is no call
that answers *what is on this network***. Discovery is a conversation over time —
poll, wait, hear — so somebody has to be listening before the question is asked.
A query that started a conversation and waited for it would hold the IPC thread
for a poll interval and would still answer *nothing* the first time, which is the
one answer that must never be wrong. The daemon therefore listens continuously
and the question **reads** the table, which leaves §5.2's first rule intact: it
sends nothing and changes nothing.

**What health means when it has three values.** The plan asked for *answering*,
*never answered* and *stopped at 20:14*, and called it a protocol change to a
type three layers read. It is — but not to `OutputHealth`. An Open DMX cable and
an sACN stream have **no answer-back at all**: nothing on the far end of a DMX
line ever says anything, so `NeverAnswered` would be a state they could enter and
never leave. What answers is a **node**, so the word belongs to a node:
`NodeHealth`, and `OutputStatusInfo::nodes` is one row per configured node of an
Art-Net output, empty for every other kind.

What an output *reports* goes on being `OutputHealth`, folded down by
`prismd::outputs::reported_health`: an output that is sending and not sending
cleanly is **`Degraded`**, which is what that word has meant since S7. The fold
happens in the snapshot, in `Delta::OutputHealth` **and** in the answer, so no two
of them can say different things about one output — and it happens **only while
the discovery is listening**, because a desk that cannot hear must not report a
fault it has not observed.

The third state's **time is the daemon's**: `lastReplyAgoMs` is an age in
milliseconds beside the value, never a timestamp inside it, because the daemon
and a browser share no clock — S33's rule for `lastErrorAgoMs` one field along.
Every age in one answer is measured against one `Instant::now()` on the daemon,
and the panel turns *90 000 ms ago* into *20:14* against its own.

#### Where the poll goes, and what that costs

**Nothing broadcasts.** The poll is unicast to the addresses the rig's Art-Net
outputs already send to, and nowhere else: an `ArtPoll` to an address this desk is
already sending 530 bytes to forty-four times a second needs no new permission and
can reach nothing new. That keeps two rules S9 and S10 wrote down — broadcast
Art-Net is a denial of service performed by the lighting desk, and no test may put
a broadcast on the network it runs on — and it is asserted at the socket rather
than stated: `nothing_here_asks_for_broadcast_permission`.

The price is named rather than hidden. A node at an address nobody has typed is
found only when it **announces itself**, which nodes do at power-up and on a
configuration change; the listening socket is bound to Art-Net's own port, so those
announcements are heard, and that is the case the *one-click output* deliverable is
about. A node that neither answers where this desk sends nor announces itself is
not discovered, and finding one would need the broadcast this desk deliberately
does not send. If a venue's nodes turn out to answer only broadcasts, the change is
one address and one permission flag in `DiscoveryConfig` — §5's 🔌 row says so.

#### One test found the flag this session needed

`wiring.rs`'s Art-Net test binds a loopback receiver and points an output at it.
With discovery on, that receiver got a fourteen-byte `ArtPoll` before the 530-byte
`ArtDmx` and the length assertion failed — which is *poll where you already send*
behaving correctly, met by a socket standing in for a node without being one. The
larger half is that the same daemon bound `0.0.0.0:6454`, which `udp.rs`'s own
tests have forbidden since S9: a suite has no business listening on every
interface of the machine it runs on.

`--no-artnet-discovery` is the answer, on by default and off in the two daemon
targets that put a real Art-Net output on loopback — the same sentence
`websocket: Listen::Off` has carried in those files since S37, and the same
fixed-port argument underneath it: two `prismd` processes on one machine cannot
both hold 6454.

---

## 3. Coverage tracking

Targets from `CLAUDE.md`: ≥ 85 % global, > 95 % on engine, programmer and protocols. Record **measured** figures only — leave blank until a run produces a number.

Measured with `cargo llvm-cov` 0.8.7 (installed 2026-08-10, `llvm-tools-preview`).
Command: `cargo llvm-cov -p <crate> --summary-only`.

| Crate | Target | Measured | Date |
|---|---|---|---|
| `prism-domain` | ≥ 85 % | **99.58 % lines**, 98.58 % regions, 99.53 % functions (S46, which added `NodeHealth`, `NodeReach`, `ArtNetNodeInfo`, one query, one answer and one field on `OutputStatusInfo`) — `output.rs` **100 % on every column**, and it first read **97.38 %** because the new types were exercised from `prism-protocols` and `prismd`: S38's rule kept, a type's own crate tests it. `query.rs`, `delta.rs`, `machine.rs`, `midi.rs`, `patch.rs`, `playback.rs`, `preset.rs`, `sequence.rs`, `session.rs`, `surface.rs` and `json.rs` all at 100 % lines. **99.67 % lines**, 98.52 % regions, 99.70 % functions (S38, which moved the binding vocabulary in — `BoundControl`, `SurfaceAction`, `ExecutorTarget`, `Step`, `GlobalButton` and `StripButton` — and added two `MachineChange` variants, one query, one answer and two deltas) — the new `surface.rs` at **100 % on every column**, `machine.rs` 100 %, `command.rs` 99.28 %. **Measured per crate, `surface.rs` first read 12.37 %**, because the moved types are exercised from `prism-surface` and `prismd`: a type's own crate should test it, so it gained eight tests of its own and the crate came out *above* where S37 left it. **99.63 % lines**, 98.36 % regions, 99.66 % functions (S37, which added `MachineChange`, `MachineSettings`, `MachineOverride`, `LogLevel`, `ExitAction`, `ShowFileInfo`, `SurfaceHealth`, `SurfaceStatus`, `OutputStatusInfo`, six commands, two deltas and two queries) — the new `machine.rs` at **100 % on every column**, and `midi.rs`, `output.rs`, `query.rs`, `delta.rs`, `playback.rs`, `sequence.rs`, `session.rs`, `patch.rs` and `json.rs` all at 100 % lines; `command.rs` 99.28 %, `socket.rs` 98.75 % with its new `option` module. **99.65 % lines**, 98.37 % regions, **100 % functions** (S36, which added `MidiPortInfo`, `Command::SetSurfacePort`, `Delta::SurfaceChanged` and `Query`/`Answer::MidiPorts`) — the new `midi.rs` at **100 % on every column**, and `delta.rs`, `query.rs`, `output.rs`, `playback.rs`, `sequence.rs`, `session.rs` and `patch.rs` all at 100 % lines. **99.65 % lines**, 98.35 % regions, **100 % functions** (S33, which added `OutputInstance`, `OutputKind`, `ArtNetPort`, `SacnPort`, `OutputChange`, four commands, one delta and the `socket` module) — the new `output.rs` at **100 % on every column** and `socket.rs` at **100 % lines**; `delta.rs`, `playback.rs`, `sequence.rs`, `session.rs`, `query.rs` and `patch.rs` all at 100 % lines, `command.rs` 99.26 %. **99.59 % lines**, 98.23 % regions, **100 % functions** (S40, which added `PlaybackId`, `PlaybackTarget`, `ObjectRef`, `OverwriteMode`, eight commands and two `Sequence` fields, and removed five narrower commands) — the new `playback.rs` at **100 % on every column**, `command.rs` 99.17 %, `sequence.rs`, `session.rs`, `query.rs` and `delta.rs` all at 100 % lines. **99.61 % lines**, 97.99 % regions, 99.53 % functions (S39, which added `SequenceStoreMode`, `CueEdit`, two `Session` fields, four commands and two more of `StoreMode`) — `command.rs`, `query.rs` and `session.rs` at **100 % on every column**, `sequence.rs` 97.14 % before `CueEdit::is_of` got a test of its own and 100 % after. S34's measurement: **99.74 % lines**, 98.08 % regions, **100 % functions** (S34, which added `ExecutorButtonRef`, `Command::ExecutorButton`, `Executor::speed` and `SPEED_UNITY`); `executor.rs` reads **100 % on every column**, which needed one more test — an executor document written *before* `speed` existed, read back at unity. S28's measurement: **99.73 % lines**, 98.05 % regions, **100 % functions** (S28, which added five commands, `CueProperty`, `StoreTarget`, `StoreMode` and `StorePreview`); the new `query.rs` and `sequence.rs` read **100 % on every column**. S35's measurement: **97.96 % lines**, 99.72 % regions, **100 % functions** (S35, which added `RenameView`, `DeleteView` and `MoveView`); `session.rs` and `command.rs`'s own modules read 100 %. The half-point against S27 is the measurement artefact this row already records in both directions — three new enum variants change which `proptest_derive` lines a 64-case run reaches. S27's measurement: **98.02 % lines**, 96.80 % regions, 96.59 % functions (S27, which added the `Query`/`Answer` shape and three patch commands). The new `query.rs` reads **100 % on every column**; the figure went *up* half a point, and the reason is the same measurement artefact this row already records in the other direction — a new enum changes which `proptest_derive` lines a 64-case run reaches. S26's measurement: **97.52 % lines**, 96.18 % regions, 95.50 % functions (S26, which added `FeatureGroup::attributes` and the `FEATURE_GROUP_ATTRIBUTES` generator). The seventeen uncovered lines were read: three in `attribute.rs` that `llvm-cov` attributes to the `MergeMode` declaration — `proptest_derive` code mapped onto the line it was generated from, the same effect this row already records below — and fourteen in `export.rs`, which are the string-literal lines of a multi-line `String::from` and the `position()` miss in a search over the array being searched. No shipped path is uncovered. S25's measurement: **97.95 % lines**, 96.66 % regions, 96.72 % functions (S25, which added `Command::PlaceWindow` and `WindowType::DmxSheet`). **The figure went down and the reason was chased rather than assumed**: the same measurement at the previous commit reads 99.45 %, and the twenty-four newly uncovered lines are all *attribute and field lines* of `Command::PlaceWindow` and of `Session` — that is, code `proptest_derive::Arbitrary` generates and `llvm-cov` maps onto the declaration it came from. Adding a variant to an enum changes the shape of the generated union and therefore which of those lines a 64-case property run reaches. **No shipped path is uncovered**: both new items are serialised, deserialised and round-tripped by hand-written tests (`the_dmx_sheet_is_a_window_like_any_other`, `a_window_cannot_be_placed_at_a_coordinate_that_is_not_a_number`, `every_command_from_the_protocol_specification_exists`), and `export.rs`'s five are S23's known ones. A later session that wants the point and a half back should look at the case count in `wire.rs`'s `round_trip!` macro rather than at the domain types. S23's measurement: **99.52 % lines**, 97.46 % regions, 98.31 % functions (S23, re-measured because `export.rs` grew the run-time variant tables the interface narrows a decoded string with; `export.rs` reads 97.07 % lines / 91.71 % regions, and the seven uncovered lines are two `?` arms on file I/O, one acronym branch in the name converter that no type name reaches, and a `panic!` formatting inside a test that passes). S1's measurement: **99.77 % lines**, 97.86 % regions, 100 % functions | 2026-08-20 (S34) |
| `prism-engine` | **> 95 %** | **99.29 % lines**, 98.95 % regions, 99.29 % functions (S45, which rekeyed every playback from the executor to the cue list and took a byte back off `TickCommand`) — `merge.rs`, `clock.rs`, `stats.rs`, `master.rs`, `cue.rs`, `readback.rs` and `testkit.rs` at **100 % lines**; `player.rs` 99.39 %, `playback.rs` 99.38 %, `body.rs` 98.85 %, `command.rs` 98.17 %. The change is a **removal** — the kind byte and the executor arm both went — so the figure is where S33 left it on a slightly smaller crate. **99.31 % lines**, 99.29 % regions, 99.07 % functions (S33, which gave `FramePublisher` a way to take a subscriber on and give one up while the tick runs) — `triple_buffer.rs` **98.28 %**, which is where all of that went; `merge.rs`, `encode.rs`, `clock.rs`, `stats.rs`, `cue.rs`, `master.rs` and `readback.rs` at **100 % lines**. **99.31 % lines**, 99.31 % regions, 98.89 % functions (S40, which rekeyed every playback from `ExecutorId` to `PlaybackId` and gave `CuePlayer` a `goto`) — `playback.rs` 99.35 %, `player.rs` 99.53 %, `cue.rs`, `merge.rs`, `encode.rs`, `master.rs`, `readback.rs`, `clock.rs` and `stats.rs` at **100 % lines**. **99.42 % lines**, 99.40 % regions, 98.87 % functions (S34, which added `readback.rs`, the flash layer, the accumulated player clock and four `TickCommand` variants) — `readback.rs` at **100 % lines**, `player.rs` 99.57 %, `body.rs` 99.47 %, `playback.rs` 99.35 %; `encode.rs`, `merge.rs`, `clock.rs`, `stats.rs` and `cue.rs` still at **100 % lines**. The two hundredths against S6's 99.61 % are on a crate two thousand lines larger. Measured with `cargo llvm-cov clean --workspace` between crates, per the caveat below. S6's measurement: **99.61 % lines**, 99.53 % regions, 99.22 % functions | 2026-08-20 (S34) |
| `prism-core` | **> 95 %** (programmer) | **97.32 % lines**, 98.16 % regions, 98.41 % functions (S45, which moved the playback state onto the sequence, added `Show::configure_executor` and gave `ShowStore` a migration) — `programmer.rs` **97.03 %**, which is the file `CLAUDE.md` names, and `command.rs` 98.70 %, `show.rs` 97.34 %, `mirror.rs` 98.95 %, `file.rs` 97.29 %. `store.rs` came out at **93.71 %** and was read line by line rather than rounded up: two branches of the new migration were unreachable from any test — the *two executors on one list* tie-break and a legacy row carrying `speed`. Both have a test now (`a_pre_s45_executor_puts_its_level_and_its_rate_on_the_cue_list`, which writes the pre-S45 rows **by hand**, because a fixture built by the code under test cannot say anything about a file that code never wrote) and the file reads **94.30 %**. **98.42 % lines**, 97.29 % regions, 98.04 % functions (S38, which added the stored binding table and the two new `MachineChange` arms) — `desk.rs` **99.76 % lines**, `command.rs` 99.52 %, `outputs.rs` 94.08 %. `desk.rs` first read 96.45 % and a dead accessor was found in the report and deleted. **98.61 % lines**, 97.40 % regions, 98.04 % functions (S37, which added `Settings`, `RecentShows`, the path validators and the sixth machine command) — `desk.rs` **99.75 % lines** (up from 99.66), `file.rs` 99.53 %, `command.rs` 99.52 %, `mirror.rs` 99.70 %, `conflict.rs` 99.65 %, `outputs.rs` 97.20 %. The one uncovered line in `outputs.rs` was read rather than counted and is now gone: it was the sACN twin of the duplicate-port refusal the Art-Net branch already covered. **98.79 % lines**, 97.55 % regions, 98.10 % functions (S36, which put the surface port on `MachineConfig` and gave the machine applier a fifth command) — `outputs.rs` **99.82 % lines**, `desk.rs` 99.66 %, `mirror.rs` 99.70 %, `conflict.rs` 99.65 %, `command.rs` 99.50 %, `file.rs` 99.41 %. **98.78 % lines**, 97.51 % regions, 98.08 % functions (S33, which added `outputs.rs` — the third applier — and the rig on `MachineConfig`) — the new `outputs.rs` at **99.80 % lines**, 99.85 % regions and **100 % functions**; `desk.rs` 99.60 %, `conflict.rs` 99.65 %, `command.rs` 99.50 %, `mirror.rs` 99.70 %, `file.rs` 99.41 %, `programmer.rs` 97.65 %. **98.73 % lines**, 97.43 % regions, 97.97 % functions (S40, which added `objects.rs` and the four generic verbs) — the new `objects.rs` at **98.03 % lines**, 96.59 % regions and **100 % functions**; `command.rs` **99.50 %**, `file.rs` 99.41 %, `show.rs` 99.03 %, `session.rs` 98.86 %, `journal.rs` 98.67 %, `programmer.rs` 97.65 %. **98.93 % lines**, 97.70 % regions, 97.77 % functions (S39, which added the three store modes, `Programmer::sequence`, `Programmer::load_cue`, `Programmer::stored`, `ShowFile::follow_cue_edit` and two session fields) — `command.rs`, `mirror.rs` and `testkit.rs` at **100 % lines**, `file.rs` 99.47 %, `journal.rs` 99.54 %, `session.rs` 99.46 %, `programmer.rs` 98.81 %, `show.rs` 98.56 %. The first measurement of the session read 98.50 % and the gap was all in the paths `ShowFile` no longer takes — the store guards in `Programmer::apply`, which exist for a **direct** caller and now have a direct caller's tests. S34's measurement: **98.97 % lines**, 97.73 % regions, 97.83 % functions (S34, which added `apply_executor_button`, `apply_executor_fader` and `set_executor_speed`) — `command.rs` at **100 % lines**, and it took a change to get there: the `Empty` arm of the button match was dead, because an earlier `if` had already returned. **Removed rather than covered**, which is what S19 and S21 did with the same finding. `mirror.rs`, `desk.rs` and `testkit.rs` also at 100 % lines; `programmer.rs` 99.67 %. S28's measurement: **98.96 % lines**, 97.71 % regions, 97.82 % functions (S28, which added `Show::relink`, `create_sequence`, `set_cue_property`, `remove_cue`, `assign_executor`, `remove_executor`, `Programmer::preset`, `Programmer::stored_keys` and `ShowFile::preview_store`) — `command.rs` and `mirror.rs` at **100 % lines**, `programmer.rs` **99.67 %**, `journal.rs` 99.49 %, `file.rs` 99.20 %, `show.rs` 98.53 %. Measured with `cargo llvm-cov clean --workspace` between crates, per the caveat below. S35's measurement: **97.98 % lines**, 99.09 % regions, 97.81 % functions (S35, which added `rename_view`, `delete_view`, `move_view`, `neighbour` and `SessionError::LastView`) — `session.rs` **98.17 %**, above the crate's own average, which is what this row is for. Measured with `cargo llvm-cov clean --workspace` between crates, per the caveat below. S44's measurement: **99.13 % lines**, 97.97 % regions, 97.72 % functions (S44, which added the Open Fixture Library reader and the library itself) — `library/mod.rs` **98.61 %**, `library/ofl.rs` **97.24 %**, `command.rs`, `journal.rs` and `testkit.rs` at **100 % lines**. **A caveat that nearly went into this file as a regression:** `cargo llvm-cov` invoked twice in one shell merges the two runs' profile data and reported 92.32 % for this crate, with `show.rs` at 66 % and `session.rs` at 73 %. `cargo llvm-cov clean --workspace` between crates is what makes the figure reproducible. S27's measurement: **99.39 % lines**, 98.03 % regions, 98.40 % functions (S27, which added `library.rs`, `Show::preview_patch`, `Show::renumber_fixture`, two journal images and three command arms). The two files this session touched most read above the crate's own average — `library.rs` **99.34 %** and `conflict.rs` **99.58 %** — which is what this row is for. `command.rs` and `journal.rs` at **100 % lines**. S25's measurement: **99.47 % lines**, 98.07 % regions, 98.51 % functions (re-measured at S25, which routed `Command::PlaceWindow` into `SessionState::apply`; unchanged to two decimal places). S15's measurement and its reasoning: **99.47 % lines**, 98.07 % regions, 98.51 % functions — `command.rs`, `conflict.rs`, `journal.rs` and `testkit.rs` at **100 % on all three**, `desk.rs`, `mirror.rs` and `session.rs` at 100 % lines, `show.rs` 99.88 %, `file.rs` 99.75 %, `programmer.rs` 99.43 %, `store.rs` 97.27 %. The ten uncovered lines are the `#[ignore]`d regenerator of the frozen migration fixture (eight) and two `?` arms that no test can reach — see §2.16 | 2026-08-20 (S34) |
| `prism-protocols` | **> 95 %** | **98.38 % lines**, 97.81 % regions, 97.49 % functions (S46, the first **receive** path in the crate: `artpoll.rs`, `discovery.rs` and the `UdpNode` seam) — the new `artpoll.rs` at **100 % lines**, 99.57 % regions and **100 % functions**, and `discovery.rs` at **99.41 % lines**, 99.70 % regions and **100 % functions**. `udp.rs` first read **92.06 %**, because the whole node seam was exercised only from `prismd`; nine tests of its own took it to **99.25 %**. What is left uncovered is what it has been since S8: `d2xx.rs` 85.29 % and `vcp.rs` 78.02 %, which are the open-a-real-cable paths `CLAUDE.md` forbids a test to reach. **98.17 % lines**, 97.49 % regions, 97.35 % functions (S33, which gave `OutputStatus` a last-error record and made `DeviceDescriptor`'s serial owned) — and **99.77 % lines with the two Windows-only FTDI backends set aside**, which is what CI measures: `d2xx.rs` and `vcp.rs` are 470 of the 5 297 lines and 86 of the 97 missed, and neither is reachable without a cable. `output.rs`, `artnet.rs`, `sacn.rs`, `opendmx.rs`, `device.rs` and `ftdi.rs` are all at **100 % lines**; `runner.rs` 99.32 %. **98.43 % lines** (S18, re-measured because `MockOutput` grew a timestamped recording — `output.rs` is at **100 % lines, regions and functions**). S10's measurement, whose reasoning still holds: **98.41 % lines**, 97.65 % regions, 97.75 % functions without the adapter (what CI reproduces) — `sacn.rs` **100 % lines and functions**, `artnet.rs` **100 %**, `ftdi.rs` and `output.rs` 100 %, `udp.rs` 99.43 %. With the adapter attached S8 measured 99.30 % via `-- --include-ignored`; that figure was not re-measured since and the code it covers is unchanged. The gap between the two is the FFI, which no build server can execute | 2026-08-11 (S10) |
| `prism-surface` | **> 95 %** | **99.24 % lines**, 98.59 % regions, 98.45 % functions (S38, which took the vocabulary out into `prism-domain` and added `Bindings::bind`, `rows` and `from_rows`) — `binding.rs` **99.44 % lines**, `profile.rs` 99.25 %. **99.24 % lines**, 98.61 % regions, 98.47 % functions (S33, re-measured; the crate did not change). **99.27 % lines**, 98.62 % regions, 98.47 % functions (S40, re-measured because `binding.rs` builds `PlaybackTarget` now). **99.24 % lines**, 98.62 % regions, 98.47 % functions (S34, with `SurfaceAction::ExecutorButton` and the release edge) — `binding.rs` **99.41 %**. S22's measurement: **99.30 % lines**, 98.68 % regions, 98.46 % functions (S22, with layer 3 and the bindings target) — `accel.rs`, `midi.rs`, `model.rs` and `control.rs` at **100 % lines**, `binding.rs` **99.85 %** / 98.72 % regions, `profile.rs` 99.39 %, `color.rs` 99.32 %, `feedback.rs` 99.01 %, `codec.rs` 98.71 %, `surface.rs` 98.39 %. Three uncovered lines in the new module were found by reading the report — the `action()` arms for the two faders and the wheel, which every test had reached through `command()` instead — and became a test rather than an exception. S21's measurement: **99.20 % lines**, 98.66 % regions, 98.28 % functions (with layer 2 and two new targets) — `accel.rs`, `model.rs`, `control.rs` and `midi.rs` at **100 % lines**, `color.rs` 99.32 %, `profile.rs` 99.27 %, `feedback.rs` 99.01 %, `codec.rs` 98.71 %, `surface.rs` 98.39 %. The crate grew by about 1 500 lines and the figure moved by six hundredths of a point, which is the point of measuring it. The 33 uncovered lines are the *cannot happen* arms a crate that denies `panic!` has to write — `let Some(...) else { return … }` on an array the diff has already bounded — plus `panic!` arms in tests that pass; three genuinely unreachable branches found while reading the report were **removed** rather than covered (§2.22). S20's measurement: **99.26 % lines**, 98.62 % regions, 98.01 % functions (with the recorded-capture target added) — `control.rs` and `midi.rs` at **100 % lines**, `profile.rs` 98.94 %, `codec.rs` 98.71 %, `feedback.rs` 98.30 %. The 17 uncovered lines are `panic!` arms in tests that pass and derived implementations. S19 measured **99.24 % lines**, 98.58 % regions, 97.94 % functions; two unreachable branches found while reading that report were removed rather than covered — see §2.20. **The figure does not include `tools/xtouch-probe`**, which is not a workspace member and has no tests: it is the instrument, not the product | 2026-08-20 (S34) |
| `prism-midi` | ≥ 85 % | **100 % lines on everything that is not the platform module** (S36, the crate itself) — `lib.rs`, `naming.rs` and `port.rs` at **100 % lines**, 99.58 % / 99.26 % regions and **100 % functions**; `system.rs`, which is the `#[cfg(target_os = …)]` and the `midir` calls under it, reads 51.49 % and the crate as a whole 90.52 %. That is the shape `prism-protocols` reports for its two FTDI backends and it is the same fact: **what is uncovered is the open-a-real-device path**, and `CLAUDE.md` forbids a test from reaching it. Two unreachable branches found while reading the report were removed rather than covered (S21's rule), and one thing that *was* only reachable through a device — the framing that delivers a message longer than the caller's buffer in pieces — was moved out of the platform module into `Inbox`, where a 63-byte SysEx read four bytes at a time is asserted byte for byte | 2026-08-22 |
| `prism-ipc` | ≥ 85 % | **97.33 % lines**, 97.02 % regions, 99.20 % functions (S37, re-measured because the `Snapshot` grew `machine` and `showFile` and the default `ServerHandler::query` answers two more questions) — `message.rs` 99.69 %. **97.49 % lines**, 97.05 % regions, 99.20 % functions (S36, re-measured because the default `ServerHandler::query` answers a fifth question). **97.61 % lines**, 97.07 % regions, 99.20 % functions (S33, re-measured because `OutputSnapshot` grew from three fields to seven). **97.66 % lines**, 97.11 % regions, 99.20 % functions (S40, re-measured because the command set changed under it). **97.63 % lines**, 97.09 % regions, 99.20 % functions (S39, re-measured because the default `ServerHandler::query` echoes the mode it was asked about). S34's measurement: **97.66 % lines**, 97.11 % regions, 99.20 % functions (S34, re-measured because the command envelope grew a variant) — `message.rs` **99.65 %**. S28's measurement: **97.69 % lines**, 97.13 % regions, 99.20 % functions (S28, re-measured because the default `ServerHandler::query` grew an arm). S35's measurement: **97.15 % lines**, 98.02 % regions, 99.20 % functions (S35, re-measured because the command envelope grew three variants) — `message.rs` **99.11 %**. S27's measurement: **98.14 % lines**, 97.18 % regions, 99.20 % functions (S27, re-measured because the two envelopes grew `Query` and `Answer` and the snapshot grew `fixtureLibrary`) — `message.rs` **99.63 %**, `client.rs` **99.01 %**, `server.rs` 98.25 %. S18's measurement: **98.46 % lines**, 97.51 % regions, 99.46 % functions (S18, re-measured because `ServerHandle` grew `clients()`; `server.rs` 99.50 % → 99.53 %). S16's measurement: **98.43 % lines**, 97.43 % regions, 99.45 % functions — `backpressure.rs`, `memory.rs` and `scan.rs` at **100 % lines**, `message.rs` 99.55 %, `frame.rs` 99.51 %, `server.rs` 99.50 %, `telemetry.rs` 99.48 %, `client.rs` 99.15 %, `stream.rs` 97.27 %, `local.rs` 93.33 %, `websocket.rs` 92.23 %. The 47 uncovered lines are `?` arms, `panic!` arms in tests that pass, the `#[cfg(unix)]` half of `local.rs` (which only the Linux job can reach) and the client WebSocket pump's error arms — see §2.17 | 2026-08-20 (S34) |
| `prismd` | ≥ 85 % | **94.65 % lines**, 95.03 % regions, 97.14 % functions (S46, which added the discovery thread, the health fold and a seventh query) — the new `discovery.rs` at **96.71 % lines**, 97.96 % regions and 98.11 % functions, and `outputs.rs` at **97.92 % lines** with **100 % functions**. The fifteen lines left in `discovery.rs` are **one branch**: the operating system refusing to start a thread, which is named rather than rounded up. **94.16 % lines**, 94.54 % regions, 96.68 % functions (S38, which put the table in force on `Core`, added learn to the surface poll and answered a seventh query) — `surface.rs` **97.62 % lines**, `core.rs` 92.97 %, `daemon.rs` 91.27 %, `server.rs` 89.47 %. **94.40 % lines**, 94.65 % regions, 96.60 % functions (S37, which added `cli::resolve` and `Listen`, the five file operations on `Core`, the machine settings and the two new answers) — `surface.rs` **98.31 % lines**, `cli.rs` 98.75 %, `outputs.rs` 97.72 %, `paths.rs` 99.21 %, `lock.rs` 98.48 %, `log.rs` 94.39 %, `machine.rs` 95.93 %, `core.rs` 93.38 %, `daemon.rs` 91.62 %, `server.rs` 89.05 %. **The crate reads 95.1 % with `main.rs` set aside**, which is still 0 % and still the honest part of the figure; what is left in `server.rs` is the `DeskHandler::query` forwarder, which only a socket reaches and which `ui/e2e/settings.spec.ts` is what exercises. **94.85 % lines**, 95.06 % regions, 97.33 % functions (S36, which added `RealSurfacePort`, the hot-plug edge, `--surface`, `--midi-ports` and the surface-change effect) — `surface.rs` **98.38 % lines** and 97.57 % regions, up from 96.94 % at S22; `cli.rs` 99.37 %, `outputs.rs` 97.72 %, `paths.rs` 99.21 %, `lock.rs` 98.48 %, `core.rs` 94.39 %, `daemon.rs` 91.91 %. **The crate reads 0.15 points below S33's 95.00 % and `main.rs` is the whole of it**: it has no test target, it is 42 uncovered lines, and `--midi-ports` added two of them — which is why the *listing itself* was moved into `cli::midi_port_report`, where it has a test, leaving the binary the printing. **95.00 % lines**, 95.24 % regions, 96.79 % functions (S33, which added `outputs.rs` — the supervisor and the driver factory — and rewrote how the daemon opens an output) — the new `outputs.rs` at **97.72 % lines** and 98.20 % regions, `cli.rs` **99.52 %**, `paths.rs` 99.21 %, `lock.rs` 98.48 %, `server.rs` 92.28 %, `daemon.rs` 92.03 %. `main.rs` is still 0 % and still 40 of the 229 uncovered lines; most of the rest are `else { panic!(…) }` arms in the test module that a passing test never takes. **94.74 % lines**, 94.86 % regions, 96.63 % functions (S40, re-measured because `Core::build_body` and `poll_playback` were rewritten around `PlaybackId`) — `main.rs` is still 0 % and still 40 of the uncovered lines. **94.90 % lines**, 94.95 % regions, 96.60 % functions (S39, re-measured because `Desk::query` passes the mode through). S34's measurement: **94.96 % lines**, 95.00 % regions, 96.60 % functions (S34, with `Core::poll_playback`, the five new effects and the `PlaybackReport` wiring) — `core.rs` **97.06 %**, `surface.rs` 97.34 %, `testkit.rs` 100 %, and `main.rs` still 0 % and still the honest part of the figure. S28's measurement: **94.55 % lines**, 94.54 % regions, 96.00 % functions (S28, with the `StorePreview` arm of `Desk::query`) — `surface.rs` **97.34 %**, `paths.rs` 99.21 %, `server.rs` 92.86 %, and `main.rs` still 0 % and still the honest part of the figure. S27's measurement: **94.94 % lines**, 95.03 % regions, 96.15 % functions (S27, with `Desk::query` and the profile library in the snapshot) — `server.rs` 93.26 %, and `main.rs` still 0 % and still the honest part of the figure. The query path is unit tested **in this crate** rather than only through `tests/ui_patch.rs`, whose non-ignored half reads a committed file and never starts a daemon. S26's measurement: **95.06 % lines**, 95.02 % regions, 96.37 % functions (S26, re-measured because `parameter_of` now resolves through `prism-domain`'s one table) — `surface.rs` **97.34 %**, and `main.rs` still 0 %, which is still the honest part of the figure. S25's measurement: **94.98 % lines**, 94.97 % regions, 96.37 % functions (S25, with `FileSurfacePort` and `--mock-surface`) — `paths.rs` and `testkit.rs` at **100 %**, `cli.rs` **99.36 %**, `lock.rs` 98.48 %, `surface.rs` **97.30 %** (up from 96.94 % with a second port in it), `core.rs` 95.72 %, `machine.rs` 95.88 %, `engine.rs` 94.87 %, `server.rs` 94.00 %, `daemon.rs` 93.85 %, `log.rs` 93.45 %, and **`main.rs` at 0 %**. Without `main.rs` the crate reads **95.6 %**. The five hundredths of a point below S22 are `daemon.rs`'s new *the surface file would not open* arm, which no test can reach without taking a directory away from the process. S22's measurement: **95.03 % lines**, 95.04 % regions, 96.31 % functions (S22, with the `surface` module and its gate target) — `paths.rs` and `testkit.rs` at **100 %**, `cli.rs` 99.34 %, `lock.rs` 98.48 %, `surface.rs` **96.94 %**, `core.rs` 95.72 %, `machine.rs` 95.88 %, `daemon.rs` 95.24 %, `engine.rs` 94.87 %, `log.rs` 93.45 %, `server.rs` 92.50 %, and **`main.rs` at 0 %**. The new module is above the crate's own average rather than below it, which is what the coverage row is for. S18's measurement: **94.73 % lines**, 94.81 % regions, 96.36 % functions — `paths.rs` and `testkit.rs` at **100 %**, `cli.rs` 99.33 %, `lock.rs` 98.48 %, `core.rs` 95.58 %, `machine.rs` 95.88 %, `daemon.rs` 95.18 %, `engine.rs` 94.87 %, `log.rs` 93.45 %, `server.rs` 92.50 %, and **`main.rs` at 0 %**. Unchanged in substance from S17's figure below — 146 uncovered lines against 144, on six more lines of code, and the movement is in test bodies rather than in the crate. **Without `main.rs` the crate reads 96.16 %.** S17's measurement and the reasoning behind every uncovered line: **94.80 % lines**, 94.83 % regions, 96.35 % functions — `paths.rs` and `testkit.rs` at **100 %**, `cli.rs` 99.33 %, `lock.rs` 98.48 %, `core.rs` 95.58 %, `daemon.rs` 95.15 %, `machine.rs` 95.88 %, `engine.rs` 94.87 %, `server.rs` 93.50 %, `log.rs` 93.45 %, and **`main.rs` at 0 %**. The last is the honest part of the figure rather than a hole in it: `main.rs` is the process entry point — `--help`, `--version`, the two messages a person sees when a daemon will not start, and `ctrl_c` — and a binary target has no tests, which is why the daemon is a library. **Without it the crate reads 96.19 % lines.** What else is uncovered is four kinds: the Open DMX arm (no test may open a real adapter — `CLAUDE.md`), the sACN multicast destination (no test may send multicast — S10), error arms no input can reach, and the `Err` half of raising the tick thread's priority, which this machine does not take. See §2.18 and §2.19 | 2026-08-20 (S34) |
| `ui` | ≥ 85 % | **97.62 % lines**, 91.26 % branches, 98.31 % functions, 97.71 % statements (S46, **805 tests in 56 files**) — `settings/outputs.tsx` at **99.24 % lines** with its one uncovered line a guard the form cannot reach, and `src/settings` **96.83 %** overall. **97.60 % lines**, 91.39 % branches, 98.28 % functions, 97.68 % statements (S45, **797 tests in 56 files**) — the new `desk/executoreditor.tsx` and `desk/functions.ts` at **100 % lines**, `desk/executorbar.tsx` and `desk/session.ts` at **100 % lines** with the fader reading and the custom row in them, `desk/console.ts` 97.83 % with the second `Assign` sentence. `functions.ts` first read 95.65 % and the gap was an **exported helper nothing called** — deleted rather than covered, which is S19's and S21's rule. **97.66 % lines**, 91.56 % branches, 98.23 % functions, 97.74 % statements (S43, **784 tests in 54 files**) — the session rebuilt most of the interface, so the figure is a whole-suite one rather than a delta. Two files of this session's own came in under the house norm and were **closed rather than recorded**: `show/presetpool.tsx` read 59.25 % lines because the context menu the owner asked for had no test at all, and `show/clock.tsx` 78.57 % because the window had no test file; both read **100 % lines** now, and the nine tests that did it are the menu's five lines and the clock's alignment, teardown and page-following. `desk/consoleshell.ts`, `chrome/`, `settings/controlfile.ts` and `canvas/geometry.ts` — the session's other new modules — were written with their tests. **98.83 % lines**, 92.95 % branches, **99.78 % functions**, 98.87 % statements (S38, 715 tests in 50 files) — the new `settings/controls.tsx` and `settings/actions.ts` at **100 % lines, statements and functions**, and `settings/` as a whole at 99.54 % lines. **99.08 % lines**, 93.11 % branches, **99.77 % functions**, 99.11 % statements (S37, 688 tests in 48 files) — the new `settings/` module at **99.28 % lines**, with `settings.ts` and `settingswindow.tsx` at **100 % on every column**, `devices.tsx` and `machine.tsx` at 100 % lines, `outputs.tsx` 99.03 % and `showfiles.tsx` 96.77 %. The two lines left are a guard this interface cannot reach — an apply with no draft and a submit with nothing typed. **98.95 % lines**, 93.98 % branches, **99.72 % functions**, 98.98 % statements (S40, 615 tests in 46 files) — the new `desk/exists.ts` and `show/grouppool.tsx` at **100 % lines**, `desk/console.ts` 99.72 %, `desk/shell.tsx` 97.77 %, `desk/consoleshell.ts` 93.33 %. **99.05 % lines**, 93.78 % branches, **99.68 % functions**, 99.09 % statements (S39, `vitest run --coverage`; **542 tests in 43 files**). `src/show` reads **99.68 % lines** with the mode chooser, the Edit key and the blinking Update in it — `cueviewer.tsx`, `presetpool.tsx`, `sequencesheet.tsx` and the new `storemode.tsx` all at **100 % lines**. Unchanged to two decimal places against S34 and S28 before it, on seven more tests. The end-to-end suite (Playwright, **24** tests) is **not** in this figure. S28's measurement: **99.05 % lines**, 93.68 % branches, **99.68 % functions**, 99.08 % statements (S28, `vitest run --coverage`; **533 tests in 43 files**). The new `show/` module reads **99.64 % lines**, 92.73 % branches and **100 % functions**: `looks.ts`, `store.ts`, `cueviewer.tsx` and `presetpool.tsx` at **100 % lines**, `sequencesheet.tsx` 98.98 %. The one uncovered line in it is the guard for a sequence chosen with no executor selected, which the interface cannot reach because the chips are disabled. The end-to-end suite (Playwright, **21** tests) is **not** in this figure. S35's measurement: **99.11 % lines**, 94.09 % branches, **99.81 % functions**, 99.14 % statements (S35, `vitest run --coverage`; **470 tests in 39 files**). Every file this session touched reads **100 % lines**: `canvas/viewbar.tsx`, `desk/encoderbar.tsx`, `desk/executorbar.tsx` and `desk/programmer.ts`. The end-to-end suite (Playwright, **18** tests, all green on CI) is **not** in this figure. S44's measurement: **99.07 % lines**, 94.01 % branches, **99.80 % functions**, 99.10 % statements (S44, `vitest run --coverage`; **441 tests in 39 files**). The `patch/` module reads **98.88 % lines** and **100 % functions** with the library search in it. The end-to-end suite (Playwright, **15** tests) is **not** in this figure. S27's measurement: **99.07 % lines**, 93.96 % branches, **99.79 % functions**, 99.09 % statements (S27, `vitest run --coverage`, v8 provider, over `ui/src` with the generated `bindings/`, `main.tsx` and the test scenery excluded; **440 tests in 39 files**). The new `patch/` module reads **98.80 % lines**, 87.36 % branches and **100 % functions**: `patch.ts` and `preview.ts` at **100 % lines**, `live.ts` 98.64 %, `patchwindow.tsx` 98.73 %, `sheet.tsx` 97.05 %. The end-to-end suite (Playwright, **14** tests) is **not** in this figure. S26's measurement: **99.07 % lines**, 94.74 % branches, **100 % functions**, 99.10 % statements (S26, `vitest run --coverage`, v8 provider, over `ui/src` with the generated `bindings/`, `main.tsx` and the test scenery excluded; **371 tests in 34 files**). The new `desk/` module reads **99.75 % lines**, 96.28 % branches and **100 % functions**: `level.ts`, `session.ts` and `valuedrag.ts` at **100 % on every column**, `console.ts`, `programmer.ts`, `encoderbar.tsx` and `executorbar.tsx` at **100 % lines**, `commandline.tsx` 97.56 %. The fifteen uncovered lines across the whole tree were read, not counted: S23's, S24's and S25's fourteen, plus the console mirror's clear-the-timer-on-unmount arm reached with nothing pending. The end-to-end suite (Playwright, **11** tests) is **not** in this figure. S25's measurement: **98.82 % lines**, 94.06 % branches, **100 % functions**, 98.86 % statements (S25, `vitest run --coverage`, v8 provider, over `ui/src` with the generated `bindings/`, `main.tsx` and the test scenery excluded; **298 tests in 29 files**). The new `canvas/` module reads **99.39 % lines**, 95.49 % branches and **100 % functions**: `geometry.ts`, `drag.ts`, `windows.ts`, `window.tsx`, `content.tsx` and `viewbar.tsx` at **100 % lines**, `canvas.tsx` 91.66 %. The fourteen uncovered lines across the whole tree were read, not counted: S23's and S24's eleven, plus the canvas element being `null` when a drag asks for its box and three `documents === null` guards under a connection that is already *connected*. The end-to-end suite (Playwright, **6** tests) is **not** in this figure. S24's measurement: **98.91 % lines**, 94.08 % branches, **100 % functions**, 98.94 % statements (S24, `vitest run --coverage`, v8 provider, over `ui/src` with the generated `bindings/`, `main.tsx` and the test scenery excluded; 235 tests in 22 files). The new `telemetry/` module reads **99.45 % lines** and **100 % functions**: `driver.ts` and `context.ts` at **100 % on every column**, `frame.ts`, `painter.ts` and `stats.ts` at **100 % lines**, `panel.tsx` 95.45 %. The eleven uncovered lines across the whole tree were read, not counted: S23's nine *cannot happen* arms, plus a canvas ref that is `null` when the effect runs and the `typeof window === "undefined"` arm of the resize fallback. The end-to-end suite (Playwright, **3** tests) is **not** in this figure. S23's measurement: **98.60 % lines**, 96.06 % branches, **100 % functions**, 98.64 % statements (S23, `vitest run --coverage`, v8 provider, over `ui/src` with the generated `bindings/`, `main.tsx` and the test scenery excluded). At **100 % lines**: `log/logger.ts`, `ipc/protocol.ts`, `ipc/endpoint.ts`, `ipc/telemetry.ts`, `ipc/codec.ts`, `mirror/mirror.ts`, `mirror/select.ts`, `store/hooks.ts`, `store/context.tsx`, `status.ts`, `desk.ts`. Then `ipc/connection.ts` 99.35 %, `mirror/patch.ts` 99.20 %, `App.tsx` 97.05 %, `store/desk.ts` 95.52 %, `ipc/shape.ts` 95.00 %. **The nine uncovered lines were read, not counted**, and each is an arm that cannot be reached from inside this interface: the `SharedArrayBuffer` branch of `overArrayBuffer`, a re-throw for a fault that is not a `MirrorFault`, a retry scheduled on a connection that has been stopped, the store set to the state it already holds, a non-`Error` cause in the decoder's `catch`, and a `return null` in a panel that only renders when the documents exist. 158 tests in 15 files; the end-to-end suite (Playwright, 2 tests) is **not** in this figure — it runs against a real daemon and measures the same code from outside | 2026-08-20 (S34) |

#### S43: one workspace run, and why it is not in the column above

The figures above are per crate — `cargo llvm-cov -p <crate> --summary-only` —
and mixing a workspace run into that series would break it: a workspace run
attributes a crate's coverage from *every* target that exercises it, so the two
numbers answer different questions and only one of them is comparable with the
history in each cell. S43 measured the workspace in one run instead, after
`cargo llvm-cov clean --workspace`, because it changed nine crates and the
question worth asking was `CLAUDE.md`'s global one.

**Workspace total: 97.98 % lines, 97.53 % regions, 98.01 % functions** (S46,
re-measured over 43 254 lines counted by `llvm-cov`'s own roll-up — the figure
S45 recorded over 65 173 lines was taken with a different set of targets
instrumented, so the two totals are comparable as percentages and not as counts)
— against a target of ≥ 85 % global. Rolled up per crate from the same run:

| Crate | Lines | Regions | Functions |
|---|---|---|---|
| `prism-domain` | **99.77 %** | 98.79 % | 99.76 % |
| `prism-engine` | **99.29 %** | 99.29 % | 98.95 % |
| `prism-surface` | **99.10 %** | 98.47 % | 98.45 % |
| `prism-core` | **98.69 %** | 97.53 % | 98.27 % |
| `prism-protocols` | **98.38 %** | 97.81 % | 97.49 % |
| `prism-ipc` | **96.91 %** | 96.94 % | 99.20 % |
| `prismd` | **94.64 %** | 95.03 % | 97.14 % |
| `prism-midi` | **90.09 %** | 89.55 % | 85.57 % |
| `prism-app` | 0.00 % | 0.00 % | 0.00 % |

The three `CLAUDE.md` names its stricter target for are all above it: the engine
at 99.29 %, the programmer — which lives in `prism-core` — at 97.03 %, and the
protocols at **98.38 %**, which is where this session's new files sit. What is
uncovered in `prism-protocols` and `prism-midi` is the same known shape it has
been since S10 and S36: the open-a-real-device path, which `CLAUDE.md` forbids a
test to reach. `prism-app` is the empty Tauri crate S29 fills.

Before that: **97.47 % lines, 97.97 % regions, 97.96 % functions** (S45,
re-measured over 65 173 lines — unchanged to two decimal places on a workspace
that grew by a thousand). Rolled up per crate from **that** run:

| Crate | Lines | Regions | Functions |
|---|---|---|---|
| `prism-engine` | **99.29 %** | 98.95 % | 99.29 % |
| `prism-domain` | **98.74 %** | 99.76 % | 99.76 % |
| `prism-surface` | **98.47 %** | 99.08 % | 98.45 % |
| `prism-core` | **97.32 %** | 98.16 % | 98.41 % |
| `prism-protocols` | **97.49 %** | 98.17 % | 97.35 % |
| `prism-ipc` | **96.96 %** | 97.06 % | 99.20 % |
| `prismd` | **94.56 %** | 94.12 % | 96.68 % |
| `prism-midi` | **89.55 %** | 90.09 % | 85.57 % |
| `prism-app` | 0.00 % | 0.00 % | 0.00 % |

(That table is S45's reading and is kept because the series is what makes a
regression visible; the one above it is this session's.)

### Performance gates

| Gate | Requirement | Measured | Date |
|---|---|---|---|
| Tick jitter | p99.9 < 2 ms, 64 universes, 10 min | **p99.9 = 200 µs**, p50 and p99 ≤ 100 µs, max 588 µs, **0 of 26 401 ticks missed** — at the thread priority `ARCHITECTURE_SPEC.md` §3 specifies. See the note below. **S45 could not re-measure this on the machine it ran on, and says so rather than reporting a number taken under load.** The short gates CI runs are clean — 3 s at 64 universes with four drivers: **0 of 133 missed, p99 200 µs, p99.9 500 µs, max 435 µs**; the whole pipeline in a tick period: **0 of 133 missed, p99 400 µs, p99.9 1.3 ms, max 1.27 ms**, both inside the 2 ms budget. The ten-minute run read 58 missed with p99.9 179 ms and, on a second attempt taken while this session's own suites were compiling, 148 missed with p99.9 80.6 ms. **The control settles it**: `the_tick_alone_holds_its_deadline_for_ten_minutes` — the bare tick with **no merge body at all** — missed **956** of 26 401 with p99.9 **162 ms** on the same machine in the same hour, six times worse than the full pipeline. The misses are the scheduler's, not the merge's, and this machine idles at 35 % with a live audio stack on it (`voicemeeter8`, `audiodg`). The figure above stands and the re-measurement is an open item — §5 | 2026-08-10 |
| Tick jitter under 100 % CPU load | p99.9 < 2 ms, 64 universes, 10 min | **p99.9 = 600 µs**, p50 100 µs, p99 200 µs, max 3.55 ms, **0 of 26 401 ticks missed**, 0 panics, 79 616 commands over 600.02 s — re-measured at S34 with the accumulated player clock, the flash layer and the readback all in the pipeline. **The first attempt failed and the harness was the reason, not the code:** run through `cargo test` the process gets no priority elevation, and the machine was compiling and running other suites at the time — 816 missed, p99.9 187 ms. In the configuration `realtime.rs` documents (release exe at High priority, one normal-priority burner per core, nothing else running) it is the figure above. S6's measurement: **p99.9 = 200 µs**, p50 and p99 ≤ 100 µs, max 464 µs, 0 of 26 401 missed. **The load has to come from outside the process** — see §3.1 and the decision log | 2026-08-20 (S34) |
| Tick allocations | zero inside the tick after warm-up | **0 allocator calls** on all **eight** paths, re-run at **S46** and unchanged — the session touched neither `prism-engine` nor any `DmxOutput` implementation, because the receive path is a **second** seam and a thread of its own; the nine tests of `tick_allocations.rs` are green. Before that: **0 allocator calls** on all **eight** paths, re-run at S45 and unchanged — the session touched the tick in two places and neither allocates: `TickCommand` gave back the kind byte it grew in S40 (`MAX_ENCODED` is 7 again, because a playback is one number), and the playback set is built per cue list rather than per executor at *rebuild* time, which is a core-thread job. **0 allocator calls** on all **eight** paths, re-run at S38 and unchanged — the engine is never told what a desk's keys do, which is why an editor costs the tick nothing. **0 allocator calls** on all **eight** paths, re-run at S37 and unchanged — the engine is never told what this machine is set to, which is the whole of why a settings window costs the tick nothing. **0 allocator calls** on all **eight** paths, re-run at S36 and unchanged — nothing this session went near the tick, which is the point: `docs/MCU_MAPPING.md` §5.3's rule is that the engine is never told about a cable, and the hot-plug edge S36 added is asserted to cost **0 missed ticks** in both directions. **0 allocator calls** on all **eight** paths (S33, which added the eighth: a tick that **takes on four subscribers and gives up four** at once, then runs 100 more at 64 universes with twice the subscribers. That is hot output reconfiguration measured rather than argued, and the first attempt read **1** — the retirement vector's first `push`, on the tick thread, which `Vec::with_capacity(MAX_SUBSCRIBERS)` fixed. The seven S40 measured are unchanged): **0 allocator calls** on all **seven** paths (S40, re-measured after every playback was rekeyed from `ExecutorId` to `PlaybackId`, which is the change that could have cost it): a warm tick over 64 universes and 4 subscribers, the merge over 8 sources, the encoder over 768 slots, 8 cue lists fading, the programmer and the masters, the readback with S34's commands, and 44 ticks on the real system clock. **0 allocator calls** in 1 000 ticks with the **readback publishing and all four of S34's commands** — flash, speed, tap and crossfade — on top of 8 cue lists over 768 slots, the programmer filling and clearing and the grand master moving, with eight playbacks reported and every one of them on a cue afterwards, so it is not a measurement of a rig at rest; **0 allocator calls** in 2 000 ticks, 64 universes, 4 subscribers; **0** in 1 000 ticks with the merge active — 768 slots, 8 loaded sources, executors switching; **0** in 1 000 ticks with merge **and** encoding — the same 768 slots patched 16-bit over 4 universes, half the fixtures inverted, 1 536 channel writes per tick; **0** in 1 000 ticks with **8 cue lists running** — every cue touching all 768 slots, ten-second fades, cues following on by themselves, Gos and on/off over the queue; and **0** in 1 000 ticks with the **programmer and the masters** as well — values arriving and being cleared on the tick, 16 group masters and the grand master moving | 2026-08-21 |
| Tick drift | < one tick period after 100 000 ticks | within one period, and the error does not grow with the tick count | 2026-08-10 |
| Triple buffer integrity | no torn frame under concurrent load | 1 000 000 frames × 64 universes → 4 readers, clean; 3 `loom` models | 2026-08-10 |
| Frame determinism | identical input → byte-identical frames | three runs of the same 100-tick script compared byte for byte on the driver's frames, 8 command changes, > 20 distinct frames | 2026-08-11 |
| Open DMX frame rate | measure real rate on SH-RS09B | **35.53 Hz** over 60 s through D2XX (2 132 frames), **38.35 Hz** through the virtual COM port. Both with the frame timing corrected; before the correction the same code reported 43.1 Hz, and that figure was the *symptom* — see the decision log. `DeviceProfile::SH_RS09B` now carries `verified: true` | 2026-08-11 (S8) |
| Telemetry render | 64 universes @ 30 Hz, zero React re-renders | **0 React commits** (S45, re-checked with the executor control editor in the tree — six `<select>`s and a text box whose value follows the show document, which is exactly the shape that would cost a commit per delta if it were subscribed to the wrong thing). **0 React commits** (S38, re-checked with a control editor in the tree — a panel that put a seventy-three-row table into `useState` on every delta would not move this number, and one that put a *level* there would). **0 React commits** (S37, re-checked with a settings window in the tree — a panel that polled its counters into `useState` at 1 Hz would not move this number, and one that put a *level* there would; the guard is what says the difference is understood). **0 React commits** (S36, re-checked with `DeskState` carrying the configured surface port — a field that changes when an operator picks a desk and never at frame rate, which is why it costs nothing). **0 React commits** (S40, re-checked with every key on the desk routed through the console shell — a provider above the whole tree is exactly the shape that could have cost a commit per keystroke, and does not). **0 React commits over 300 frames** — re-checked at S34 with the cue index arriving from the tick, which moves the *show* document while a playback runs and would show up here if it reached the telemetry path. Re-checked at S35 with the two bars rebuilt into one band above the canvas, and with the encoder bar paging. **0 React commits over 300 frames** — re-checked at S27 with the Fixture Sheet's **second** canvas in the interface, which is what says the output column is outside React: a sheet that put a level into a `useState` would turn this red. Re-checked at S26 with the executor bar, the encoder bar and the command line on the screen as well, and at S25 with the panel **inside a window**, so the count is of an interface that has a canvas, a window frame and a View Selector Bar in it. Originally: **0 React commits over 300 frames** of the recorded 64-universe frame — ten seconds at §7's rate, counted with a `<Profiler>` round the whole interface, with 300 paints recorded on the surface over the same run. See the note below | 2026-08-21 |
| Telemetry frame budget | < 8 ms per frame at 64 universes | **0.10 ms median, 0.30 ms p99 over 155 frames at 30.5 Hz (S45)**, nothing lost and nothing dropped, re-measured with the `Executors` window carrying a control editor. **0.20 ms median, 0.70 ms p99 over 153 frames at 29.9 Hz (S38)**, re-measured with the control editor built and a fifth settings panel in the tree. **0.20 ms median, 1.00 ms p99 over 155 frames at 30.4 Hz (S37)**, re-measured with the settings window built and its counter poll running. **0.20 ms median, 1.10 ms p99 over 154 frames at 29.8 Hz (S36)**, re-measured because a surface poll now asks a port whether it is still there — once a second, not once a millisecond, which is the point of the figure being unchanged. Two frames lost, which is §8's coalescing; **zero dropped**, which is what is asserted. **0.10 ms mean, 0.30 ms p99 over 155 frames at 30.3 Hz (S40)**, re-measured with the console shell above the canvas. **0.20 ms median, 0.70 ms p99 over 153 frames at 30.1 Hz (S39)**, re-measured with the store bars asking a fourth kind of question and the cue sheet drawing two more keys per row. **0.20 ms median, 0.60 ms p99 over 164 frames at 30.4 Hz (S34)**, nothing lost and nothing dropped, with the readback moving the show document while a playback runs. **0.20 ms median, 1.50 ms p99 over 154 frames at 30.2 Hz (S35)**, with both bars rebuilt into one band above the canvas; three consecutive runs of the gate alone read **30.4 Hz with nothing lost**. **⚠ This gate is a throughput measurement on a desktop and it reads a busy machine as a code regression** — taken at 80–96 % CPU it reports 15–19 Hz with 138–152 frames lost, which is indistinguishable from a real fault. Check the load first: `Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'"` (the `Get-Counter` path is localised and fails on a German install). See the decision log. S44's measurement: **0.20 ms median, 0.50 ms p99 over 154 frames at 30.3 Hz (S44)**, with the fixture library in the daemon. S27's measurement: **0.20 ms median, 0.40 ms p99 over 160 frames at 30.0 Hz**, with the patch module built and a second telemetry loop in the interface. S26's measurement: **0.20 ms median, 0.40 ms p99 over 156 frames at 30.4 Hz**, re-measured with the executor bar and the encoder bar rendering above the canvas — two more bands in the same column, and the figure does not move. S25's measurement: **0.20 ms median, 0.50 ms p99 over 155 frames at 30.2 Hz**, with the DMX Sheet **opened through the interface** — an `OpenWindow` out and a `SessionPatch` back before the first frame is drawn, and a canvas sized by the window rather than by `34vh`. S24's measurement: **0.30 ms median, 1.10 ms p99** over 163 frames (and 0.20 / 1.00 on the run before). Decode **and** paint, `performance.now()`, in Chromium against a real `prismd` on the 64-universe rig at 30.2 Hz. **On the CI runner: 0.20 ms median, 0.40 ms p99 over 164 frames at 30.3 Hz** — Linux, software rasteriser, debug daemon, nothing lost and nothing dropped. See the note below | 2026-08-21 |

**Thread priority is part of the tick jitter figure.** The same ten-minute run at
the shell's default priority missed 45 ticks and had a p99.9 of 54 ms. The engine
is not the difference: a three-minute run with *no* subscriber threads at all
still stalled every twenty seconds or so, which is the Windows scheduler
preempting a normal-priority thread. `prism-engine` cannot set its own priority —
it is platform-neutral by rule — so the tick thread's priority is raised in
`prismd`, and the figure above is measured in that configuration.

**S17 does that**, in `crates/prismd/src/engine.rs`: the tick thread's first act
is `thread_priority::set_current_thread_priority(Max)` on **itself**, so the
process keeps its ordinary class and the runtime, the driver threads and the
surface stay where they were. A refusal — an unprivileged Linux container, which
is what D10's Raspberry Pi is — is a `WARN` and not a failure to start. The
long-run figures above have **not** been re-measured through `prismd`; they are
`prism-engine`'s own, taken the way §3.1 describes, and the daemon reproducing
them at 64 universes over ten minutes is a measurement S18 or later should take
rather than one this session claims.

Measured on: Windows 11 26200, Rust 1.97.1 msvc, release profile, engine plus four
subscriber threads polling at 5 ms, machine otherwise idle but not quiesced.

**The two telemetry figures, and how to re-run them.** *Zero re-renders* is a
count, not a judgement: `ui/src/telemetry/render.test.tsx` wraps the whole
interface in a `<Profiler>`, delivers 300 telemetry frames of the recorded
64-universe frame through a real `Connection`, and asserts the commit count is
the one it had after the handshake. It is an ordinary unit test —
`npm run test` in `ui/` — and it runs on every commit. A mutation that puts a
`useState` setter in the readout callback turns it red, which is what says the
counter is measuring something.

The **frame budget** cannot be measured in `jsdom`, which has no rasteriser, so
it is measured where it matters:

```bash
npx playwright test telemetry
```

in `ui/`, after `npx playwright install chromium`. It starts a real `prismd` on a
copy of `ui/tests/fixtures/wide-rig.prism` — a show patched across all 64
universes, because the daemon only publishes the universes a show actually
patches — serves the production build, and lets the interface run for about five
seconds. The interface times its own frames with `performance.now()` **around the
decode and the paint together**, keeps a four-second window of them, and writes
median and p99 into the readout line, which the spec parses, asserts against the
8 ms budget and prints:

```text
[telemetry] 64 universes · 30.2 Hz · paint 0.30 ms (p99 1.10 ms) · 163 frames · 1 lost
```

Measured on: Windows 11 26200, Chromium 141 via Playwright 1.62, the daemon a
**debug** build (which is what the CI job runs too), the browser at its default
1280 × 720 viewport, machine otherwise idle but not quiesced. *Lost* frames are
not a fault: §8 says the daemon coalesces and drops, and one or two in five
seconds is that mechanism working. What must be zero is *dropped* — frames that
arrived and could not be read — and it is asserted as zero.

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

### 3.4 Running the X-Touch probe (S20)

`tools/xtouch-probe` is the only code in the repository that opens a MIDI port,
and it is **not a workspace member** — see the decision log for why. It has its
own `Cargo.lock` and `target/`, so it is built and run from its own directory:

```bash
cd tools/xtouch-probe && cargo run --quiet -- ports
```

Everything below assumes that directory. **The desk must be in MC mode over USB**
(hold channel 1 SELECT while switching on to check). `PRISM_XT_IN` and
`PRISM_XT_OUT` pick a port by name substring or index if the default guess is
wrong; `PRISM_XT_CAPTURE` writes a capture file.

What the desk is, and what firmware it runs — the first thing any session should
ask, and it answers in under a second:

```bash
cargo run --quiet -- identify
```

Log and analyse everything the surface sends. The analysis reconciles what
arrived against `profile::X_TOUCH` — notes with their names, pitch-bend channels
with their faders, CC numbers with their encoders, magnitudes, and the gaps
between messages:

```bash
cargo run --quiet -- capture 60
```

**The note map, checked in both directions without an order to follow.** The host
lights one LED at a time; press whichever button is lit. This is the command that
verified 60 of the 64 panel buttons, and it is the method to reuse — a printed
press order makes the person the thing under test:

```bash
cargo run --quiet -- pair
```

`pair <from> <count>` repeats part of it. `PRISM_XT_PATIENCE` (seconds, default
30) is how long it waits before moving on, which is how a button with no LED
shows up.

The outbound experiments. With no argument each lists its steps; `<name> <n>`
runs one step; `<name> all` runs the lot with `PRISM_XT_STEP_MS` (default 6000)
between them. Running one step at a time is what makes an observation reliable —
the person watching is told exactly what to expect and sees only that:

```bash
cargo run --quiet -- colors
```

```bash
cargo run --quiet -- leds all
```

`leds`, `motors`, `rings`, `meters`, `segments`, `lcd`, `colors` and `oddities`.
The last one sends what the codec **refuses** to encode — a meter for strip 15, a
ring LED past the eighth, a colour message of the wrong length — which is the only
way to ask what the surface does with a message it should never receive.

Round-trip latency, measured through the handshake because a motor fader generates
no reply:

```bash
cargo run --quiet -- latency 60
```

**The pacing tests, and a warning.** `flood <gap_us> <count> <mode>` where mode is
`silent` (send and ask nothing), `interleave` (query after each message, each
answer waited for), `burst` (query after each message, nothing waited for) or
`moving` (flood while an operator sweeps a fader, counting what still gets
through):

```bash
cargo run --quiet -- flood 0 64 silent
```

`burst` mode, and the older `pacing` command, are the two that **stop the surface
transmitting** — twice out of two attempts. The desk then receives normally,
displays whatever is written to it, and sends nothing at all until it is
**power-cycled**; reopening the port does not help. That is the finding
(`docs/MCU_MAPPING.md` §2.7), so it is reproducible on purpose — but run it
knowing it ends with somebody reaching for the switch.

Arbitrary bytes, for a question that has not been given a subcommand yet:

```bash
cargo run --quiet -- raw F0 00 00 66 14 13 F7
```

The captures the tool produced live in `crates/prism-surface/tests/captures/` and
are replayed by `crates/prism-surface/tests/hardware_capture.rs` **in the ordinary
suite** — so re-running any of this is how a *new* claim gets measured, not how an
existing one gets checked:

```bash
cargo test -p prism-surface --test hardware_capture
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

Every one is recorded as plain data so verification is a data update, not a refactor. See `ARCHITECTURE_SPEC.md` §14.

| Item | Blocks | Status |
|---|---|---|
| ~~MCU note and CC numbers vs. real X-Touch~~ | — | ✅ **verified 2026-08-13 (S20)** — a Behringer X-Touch in **MC mode over USB, firmware V1.25, serial `0156406`**, worked control by control: all 40 strip notes, all 64 panel notes (60 of them in both directions at once, by lighting one LED and pressing the button that lit), all nine faders, all nine relative controls and every outbound message type. **Not one number was wrong** — the whole verification was three added profile fields, one corrected character mapping and a new test target, which is what holding the table as one constant was for. `prism_surface::X_TOUCH.verified` is `true` and the `const` assertion S19 planted was rewritten in the same edit. **The evidence is in the repository rather than in this table:** `crates/prism-surface/tests/captures/` holds four recordings of what the desk sent and `crates/prism-surface/tests/hardware_capture.rs` replays them in the ordinary suite, so a device-specific claim is checked on a build server with nothing plugged in. Six things no source had stated were corrected (§2.21) and one real fault was found: **the surface can stop transmitting while still receiving, and only a power cycle recovers it** — `docs/MCU_MAPPING.md` §2.7, and S21 designs around it. Two items are recorded ◻ untested rather than ticked: the foot switches need a pedal nobody had, and the cable-pull case belongs to S21 |
| The control editor against a real X-Touch | nothing — S38 is complete without it | ☐ unverified, and **deliberately not blocking**. Everything a *daemon* can answer for is asserted with nothing plugged in: a key rebound over the protocol changes what the same three bytes do, learn names a control of every shape §2.1 has and does not fire it, the reserved control is refused from three directions, and two editors produce one table. What only the desk can answer is whether **learn is comfortable on the real panel** — whether pressing a key while an editor is open names the key the operator meant, on a surface whose modifiers are held rather than latched (§2.1) and whose V-Pots accelerate (§2.7). The second half needs the sound console as well: §4.3's *permanently ours* set is drawn in the editor straight out of `McuProfile::permanent`, so confirming or correcting it is the same one-line data edit `docs/MCU_MAPPING.md` §7's ◻ row has described since S21. `ARCHITECTURE_SPEC.md` §14 |
| The settings window against a venue's real rig | nothing — S37 is complete without it | ☐ unverified, and **deliberately not blocking**. Everything a *daemon* can answer for is asserted with nothing plugged in: the five-output rig is built from the window against recording doubles and every counter is watched climbing, the MIDI enumeration answers and opens nothing, and a listener that cannot bind is produced by binding the address first. What only a venue can answer is whether the rig an installer types into this window is the rig their nodes receive — which is the Art-Net and sACN rows below, met from the other end. Verifying it is an afternoon in a hall with the panel open, not a code change. `ARCHITECTURE_SPEC.md` §14 |
| The D11 gate over a **real** MIDI port | nothing — S36 is complete without it | ☐ unverified, and **deliberately not blocking**. S36 chose the backend, gave it a home (`prism-midi`, §10.1's fourth exception) and wired it: enumeration, a name that survives the platform's decoration, a doubling reconnect backoff to a five-second ceiling, both edges of a cable, and the rule that a desk which has merely gone *quiet* is never reopened. All of it is asserted with **nothing plugged in**, on a machine that has a real X-Touch attached. What only the desk can answer is whether `midir` produces exactly one disappearance and one reappearance for one real unplug and replug — which is `docs/MCU_MAPPING.md` §7's last ◻ row, open since S21 and now half closed, because the backend it was waiting to name is named. The recipe is two commands: `prismd --midi-ports` to read the name, then `prismd --surface "<name>" --mock-output`, press `Channel ▶` and F1, and look for the view and the window in a client's snapshot. `ARCHITECTURE_SPEC.md` §14 |
| ALSA on the Raspberry Pi | nothing — Windows is the release target (D10) | ☐ unverified, and **it is a build rather than a measurement**. `midir` is target-gated so the ARM64 cross-check needs no `libasound2-dev`, which means the Linux MIDI path is compiled by nobody today; a Pi build is `--features prism-midi/alsa` with that package installed. What a Pi can answer, and only a Pi: that the build succeeds, that ALSA's port names go through `prism_midi::normalise` as `naming.rs` predicts — the ` 24:0` client address coming off — and that the client number really does change across a boot, which is the whole reason that rule exists. One build and one test. `ARCHITECTURE_SPEC.md` §14 |
| Two Open DMX adapters told apart by their serials | nothing — S33 is complete without it | ☐ unverified, and **deliberately not blocking**. S33 made the rig data and `OutputKind::OpenDmx { serial }` is what pins one output to one physical cable. Everything short of a second cable is asserted: the serial reaches the descriptor the driver hands the backend (`opendmx.rs::a_serial_from_the_configuration_is_what_the_bus_is_asked_for`), and S8's matching rule is asserted against enumerated devices. What only **two** SH-RS09Bs can answer is whether the two outputs open the adapters an installer expects rather than both taking the first; this machine has one. Both halves are data — `DeviceDescriptor::serial` and the `machine.json` row — so verifying it is a data update and one test |
| sACN against a real receiver | nothing — S10 is complete without it | ☐ unverified, and **deliberately not blocking**. Everything a socket can answer is asserted, including the datagram as received and the group address over the whole 1…63999 range. What only a gateway and a real switch can answer is whether the **multicast path** works end to end — IGMP snooping on the switch, and whether a hop limit of 1 reaches the venue's nodes. Both are held as data (`SacnDestination`, `SacnConfig::multicast_ttl`), so verifying them is a configuration change. **No test sends multicast on purpose:** a suite that put sACN on the network it runs on is the same fault as one that broadcasts. `ARCHITECTURE_SPEC.md` §14 |
| ArtNet against a real node | nothing — S9 is complete without it | ☐ unverified, and **deliberately not blocking**. Everything a socket can answer is asserted, including the datagram as received. What only a node can answer is whether it agrees about the port-address mapping (0-based or 1-based on that front panel) and whether it wants ArtSync. Both are held as data — `PortAddress` per universe and `ArtNetConfig::sync` — so verifying them is a configuration change, not a code change. **S46 made the first half answerable without reading a front panel:** a node that answers `ArtPoll` states its own port-address table, and the Outputs panel names the universes the two sides disagree about in both directions. `ARCHITECTURE_SPEC.md` §14 |
| Art-Net **discovery** against a real node | nothing — S46 is complete without it | ☐ unverified, and **deliberately not blocking**. Everything a socket can answer is asserted with nothing on the network: a mock node on loopback is polled, answers, is named and shown (`artpoll_wire.rs`, over a real `UdpSocket`); one that stops reads *stopped* with the age, inside one poll interval; one that never answers never reads well, over the protocol against a running daemon (`crates/prismd/tests/artnet_nodes.rs`); and a quarter of a million random bytes produce no panic and **zero** allocator calls. What only a real node can answer is two things, and both are about **other people's firmware**: whether it replies to a **unicast** `ArtPoll` rather than only to a broadcast one — §6 says it must, and this desk deliberately sends no broadcast — and whether it announces itself at power-up, which is what makes an unconfigured node appear at all. If a venue's nodes turn out to answer only broadcasts, the change is one address and one permission flag in `DiscoveryConfig`, not a code change. `ARCHITECTURE_SPEC.md` §14 |
| The ten-minute tick-deadline gate, re-measured after S45 | nothing — the short gates CI runs are green | ☐ **unmeasured, and the reason is the machine rather than the code.** S45 rekeyed every playback from the executor to the cue list, which is a change in the tick's neighbourhood, so the ten-minute gate is worth re-running. It was run twice on 2026-08-29 and read 58 missed / p99.9 179 ms and 148 missed / p99.9 80.6 ms. **The control says the machine:** `the_tick_alone_holds_its_deadline_for_ten_minutes` — the bare tick with no merge body at all — missed **956** with p99.9 **162 ms** in the same hour, six times worse than the full pipeline, and this machine idles at 35 % with a live audio stack (`voicemeeter8`, `audiodg`) that owns the scheduler slots the tick wants. Re-run it on a quiet machine: `cargo test -p prism-engine --release --test realtime -- --ignored the_tick_holds_its_deadline_for_ten_minutes --nocapture`, and run the bare-tick control beside it whenever the answer is not zero. §3.1 has the harness, §3's note has why the numbers move |
| ~~SH-RS09B USB VID/PID and real frame rate~~ | — | ✅ **verified 2026-08-11 (S8)** — `0403:6001`, serial `B0037HIY`, `FT232R USB UART`, 35.5 Hz over 60 s. `DeviceProfile::SH_RS09B` carries `verified: true` and the tests assert the measurements. **Holding it as data paid for itself:** the whole verification was three fields and one test, with no code changed anywhere else — see `ARCHITECTURE_SPEC.md` §14 |

---

## 6. Decision log

Architectural decisions D1–D11 are in `ARCHITECTURE_SPEC.md` §1. This log records **changes and discoveries made during implementation** — things that turned out differently from the plan.

| Date | Session | Finding | Consequence |
|---|---|---|---|
| 2026-08-29 | S46 | **A discovered node is neither show nor machine, and putting it in either would have been wrong in a way a file remembers.** The rig is `prism_core::MachineConfig` because it is a decision somebody made (S33); the MIDI ports are a `Query` because they change when a person moves a plug (S36). A node that answers is the second shape and not the first: it changes while nobody does anything, no command causes it, and it is gone at the next start. Writing it into the configuration would mean a `machine.json` that changes because somebody switched a node off | **`Query::ArtNetNodes`**, and the answer carries `listening` as well as the list. One thing differs from `MidiPorts` and is why this could not be *enumerate on the asking thread*: there is no call that answers *what is on this network* — discovery is a conversation over time. So the daemon holds a table its own receive thread keeps and the question **reads** it, which leaves §5.2's first rule intact: the query sends nothing and changes nothing |
| 2026-08-29 | S46 | **Three-valued health is a second word, not three more `OutputHealth` variants.** The plan asked for *answering*, *never answered* and *stopped at 20:14*. Putting those on `OutputHealth` would have given an Open DMX cable and an sACN stream a state they could enter and never leave, because neither has an answer-back: nothing on the far end of a DMX line ever says anything | **`NodeHealth`** belongs to a **node**, and `OutputStatusInfo::nodes` is one row per configured node — empty for every other kind, and empty while nothing is listening. What an output *reports* goes on being `OutputHealth`, folded down by `prismd::outputs::reported_health`: an Art-Net output whose nodes are silent is **`Degraded`**, which is what that word has meant since S7 — sending, and not sending cleanly. The fold happens in the snapshot, in `Delta::OutputHealth` **and** in the answer, so a delta and an answer cannot say different things about one output |
| 2026-08-29 | S46 | **A desk that is not listening must not report `Degraded` either.** The obvious fold is *no reply means not well*, and it is wrong in one case that matters: a daemon whose discovery socket never bound knows nothing at all about the far end, and reporting a fault out of ignorance is B6's mistake pointed the other way | The fold is made **only while the discovery is listening**, and `Answer::ArtNetNodes` carries the flag so a panel can draw *not listening* before it draws an empty list. The two causes are told apart by `error`: *nothing is configured* is an absence with no reason, and *this run was told not to listen* is a reason (`Discovery::disabled`). A panel that guessed would tell an operator to configure an output that would not help |
| 2026-08-29 | S46 | **The receive path needed a seam of its own, and `udp.rs` had said so two sessions before it was needed.** `UdpSender`'s documentation since S9: *`ArtPoll` and node discovery are a later session's problem and will need their own seam rather than a `recv` bolted onto this one.* The sentence was right for a reason worth restating: an output's thread exists to keep a DMX line fed, and a `recv_from` on the trait it holds would put a blocking call within reach of three drivers that must never make one | **`UdpNode`**, a second trait with `SystemUdpNode` and `MockUdpNode` beside it, and `recv_from` carries its **own timeout** rather than the socket carrying a mode — the discovery thread decides per call how long to wait, because it wants to be woken to send the next poll and to notice it has been asked to stop. `ArtNetOutput`, `SacnOutput` and `OpenDmxUsb` are unchanged, which is the measurement that says the seam was the right shape |
| 2026-08-29 | S46 | **A parser fed by strangers is measured, not asserted.** An Art-Net network is a flat unauthenticated broadcast domain that a school's whole building is on, and the thread reading it is inside the process driving the show. `prism-surface` established the shape for the other stream of bytes a stranger controls (`docs/MCU_MAPPING.md` §6): no panic, no allocation growth, and a fate written down for every message | `ArtPollReply` is **plain data with no owned allocation in it** — the three name fields stay as the byte arrays they arrived in and are handed out as `&str` — so `parse_art_poll_reply` measures **zero** allocator calls over 4 096 hostile datagrams. The table is bounded at 64 nodes and one read pass at 64 datagrams, because an unbounded `Vec` grown by a stranger is an allocation storm with extra steps. The accounting identity is the surface's: every datagram delivered is a reply or a malformed one, and nothing is silently swallowed |
| 2026-08-29 | S46 | **A discovery socket bound on every interface is a socket a test suite must not open, and one test found it by failing.** `wiring.rs`'s Art-Net test binds a loopback receiver and points an output at it. With discovery on, that receiver got a fourteen-byte `ArtPoll` before the 530-byte `ArtDmx` and the length assertion failed — which is the *correct* behaviour of *poll where you already send*, met by a socket standing in for a node without being one. The larger half is that the same daemon bound `0.0.0.0:6454`, which `udp.rs`'s own tests have forbidden since S9 | **`--no-artnet-discovery`**, on by default and off in the daemon test targets that put a real Art-Net output on loopback — the same sentence `websocket: Listen::Off` has carried in those files since S37, and the same fixed-port argument: two `prismd` processes on one machine cannot both hold 6454, so an operator running a second desk should be able to say so rather than read a warning every morning |
| 2026-08-29 | S45 | **An executor's assignment is show content, and it took an argument to say why.** Both precedents were real and pointed opposite ways: what an executor *carries* travels with the show (S11), and what a desk's keys *do* must not (S38 — a show carried to another hall must not bring the old hall's F-keys). The session could not derive the answer from either | **`Command::ConfigureExecutor`**, in the show group, undoable, journalled with one image per executor row. What settles it is that an executor is **not a control of this building**: it is one of the six numbered things `ObjectRef` names, it is copied, moved, labelled and coloured by show verbs, and a Web Remote with no X-Touch anywhere near it still has eight of them with four keys each. S38's table binds *hardware* — this strip's third key — and `ExecutorButtonRef` has kept that apart from *what the third key does* since S34. The consequence is what makes it testable: carry a show to another hall and its Go keys are still Go keys. `docs/IPC_PROTOCOL.md` §5 |
| 2026-08-29 | S45 | **The fix for two executors moving independently was not to forbid the second one.** The obvious repair for punch-list B18 is to refuse a second executor on a cue list. It is wrong twice over: an operator asking for a `Master` and an `XFade` on one list is asking for something reasonable and the entry says so in as many words; and refusing would still leave `On Sequence 1` and a Go on the fader as two different playbacks, which is the hole S40 had already had to paper over with *the two are never both live for one list* | **The state moved instead.** `PlaybackId` is the cue list's own number, `Show::playback_of` resolves an executor to the list standing on it, `MergeBody` holds one playback per list, and `masterLevel`, `speed`, `isActive` and `currentCueIndex` went from `Executor` onto `Sequence`. Two sentences became one. It also made `AssignExecutor` reach the engine **not at all** — the playback set does not mention slots — which is §6.1's rule about undo arrived at from the other side: an Oops that put an executor row back used to answer `ExecutorOff` and would have stopped a list somebody was watching |
| 2026-08-29 | S45 | **A React dependency on a document subtree is a claim about what else writes into it, and moving a field invalidates it silently.** The cue viewer re-asked `Query::StorePreview` whenever `/sequences` moved, which S34 had made safe by putting the playback state on the *executor*. Putting it back on the sequence made a cue advance rewrite that subtree — one question to the daemon per cue of a chase, which is exactly the cost S28 wrote the warning about | The dependency is that list's `cues` node (`ui/src/show/looks.ts::cuesDocument`), which a cue advance does not touch. **The test that caught it was S39's own** — *does not ask again when a playback advances a cue* — and it caught it because it counts the questions rather than checking that the answer is right. The general shape to carry: when a field moves between documents, grep for what depends on the *container* it left and the one it arrived in |
| 2026-08-29 | S45 | **A generated table stopped being generated, and the fix belonged in the generator.** `ExecutorButtonFunction` gained a variant with a field — the custom row — so it is no longer a union of string literals and `prism_domain::export::string_union` dropped `EXECUTOR_BUTTON_FUNCTION_VARIANTS` entirely. Four TypeScript files import that table to **check** a string they were given, and `CLAUDE.md` forbids the `as` that would replace it | `string_union` **skips an object arm** rather than being disqualified by one: the literals are still a table and the object is not a choice. A union of *only* objects still comes back `None`, so `Command`, `PlaybackTarget` and `ExecutorButtonRef` are unchanged. **Writing the eight out by hand in TypeScript was the tempting fix and would have been a second list** — the thing `ui/src/bindings/` exists to prevent |
| 2026-08-29 | S45 | **A ten-minute real-time gate needs a control window, and this session needed it twice.** The tick-deadline gate read 58 missed ticks with a p99.9 of 179 ms after the change. Nothing in S45 touches the tick's timing — `TickCommand` got *smaller* — but a number is not an argument | `the_tick_alone_holds_its_deadline_for_ten_minutes`, which the harness has carried since S6 as a diagnostic, run on the same machine in the same hour: the **bare tick with no merge body at all** missed **956** with a p99.9 of **162 ms**, six times worse than the full pipeline. That is the control, and it says the scheduler. The machine idles at 35 % with a live audio stack on it. **The gate is recorded as unmeasured in §5 with the recipe**, rather than reported green from a run that was taken under load or reported red from one that measured the machine — which is `surface_gate.rs`'s rule (S43) applied to a different gate |
| 2026-08-22 | S36 | **A MIDI backend could not live outside the workspace the way S20's probe does, and the reason is S37 rather than taste.** The probe is `tools/xtouch-probe/` because opening a port is platform code and §10.1 allows `prism-surface` none — but a settings window has to *enumerate* ports over the protocol, and a separate process cannot answer that for the daemon. The two rules the probe satisfies are different rules: *platform code is confined* and *no test may need hardware* | `prism-midi`, a workspace member and §10.1's **fourth** exception, with the split in `system.rs` and nowhere else. The precedent named in the plan is exact: `thread-priority` (S17) is a dependency taken on for the same purpose one device along, and `prismd`'s crate documentation now lists four places platform code would otherwise have been rather than three. The probe stays where it is, because what *it* does — walking a panel button by button, flooding a surface until it stops answering — genuinely needs a device |
| 2026-08-22 | S36 | **`midir` on Linux is a `pkg-config` probe for `libasound2-dev`, and the ARM64 cross-check must not need a cross-installed C library.** That job exists to catch portability breaks in *our* code; making it install `libasound2-dev:arm64` would have added an apt architecture, a mirror dependency and a failure mode unrelated to any code, to a job that runs in 40 seconds | The dependency is declared **twice**: unconditional off Linux, optional on it behind an `alsa` feature. Measured consequence: on `aarch64-unknown-linux-gnu`, `cargo tree -p prism-midi` shows a crate with **no dependencies at all**. A build with no backend is not a degraded mode — `NoBackend` enumerates an empty list and refuses to open, which is the answer a machine with an empty USB bus gives, and it is what both Linux CI jobs run against. **The cost is named rather than hidden:** the Linux MIDI path is compiled by nobody today, and it is a 🔌 row in `ARCHITECTURE_SPEC.md` §14 for the Raspberry Pi |
| 2026-08-22 | S36 | **A port index is not a configuration and a port name is not stable either.** `midir`'s identifiers are positions in a list that renumbers itself when somebody moves a plug, so a stored index points at a different device afterwards. But the *name* carries decoration the platform adds and changes: Windows prefixes a driver-instance number (`2- X-Touch`), ALSA appends a client address that changes at every boot (` 24:0`) | `prism_midi::selects`: three rules in order — the name as it stands, the name with the decoration stripped, the name as a case-insensitive fragment. **The order is the design**: looser rules last, so a venue with two X-Touches writes both names out in full and neither row can drift onto the other. An empty name selects nothing, because a desk that attached itself to whatever was plugged in would move somebody else's faders |
| 2026-08-22 | S36 | **A port layer that reconnects on silence undoes the one diagnosis that matters.** S20 measured an X-Touch that stopped transmitting while receiving perfectly — writes still landed on the display — and established that only a power cycle recovers it. `SurfaceHealth::Unresponsive` reports that to the operator; a reconnecting port underneath it would churn the device state *and* take the reading away | Exactly two things close a port: it leaving the enumeration, and a write the operating system refused. **Silence is not one of them**, and `a_desk_that_has_gone_quiet_is_not_reopened` is that sentence as a test — ten seconds of nothing inbound with writes still landing leaves the count of opens at one. The finding now lives in two layers on purpose, which is the shape to copy for any fault whose remedy is not *reconnect* |
| 2026-08-22 | S36 | **`SurfacePort::connected()` cannot discover that a cable has come out.** Neither of the two things that close a real port is visible to it: an enumeration that no longer lists the device, and a write that failed. The trait needed something that *acts* rather than reports | `fn refresh(&mut self, now: Duration)`, defaulting to nothing, and it takes `now` for S21's reason kept one layer further down — the clock is an argument all the way, which is what lets the reconnection schedule be walked in a test in milliseconds. **The call order in `SurfaceLink::poll` is load-bearing**: the port is asked *after* the send, because a failed write is one of the two discoveries, and asking first would report a port gone one poll late |
| 2026-08-22 | S36 | **The one part of a real MIDI port that is arithmetic was inside the `#[cfg]` module, where no test could reach it.** `MidirPort::read` held the queue that hands a caller as much of a message as it has room for and keeps the rest — which matters because the layer above is a stream decoder with SysEx reassembly (S19), and truncating would turn a scribble-strip reply into a *different* message the codec would count rather than notice. Found by reading the coverage report line by line: `system.rs` at 49 % while the rest of the crate was near 100 | `prism_midi::Inbox`, platform-neutral and asserted — a 63-byte SysEx read four bytes at a time comes back byte-identical. **The rule to reuse: when a `#[cfg]` module is far below the rest of its crate, look for what is inside it that did not need to be.** Two unreachable branches went the other way in the same pass and were *removed* rather than covered, which is S21's rule |
| 2026-08-22 | S36 | **A daemon whose *outputs* came off the command line refuses to configure its *surface* too, and that is right rather than incidental.** It caught this session's own first draft of `tests/surface_port.rs`, which started its daemon with `--mock-output` and then could not choose a port. The rule S33 wrote is *nothing is written down this run*, and that applies to a port name exactly as it applies to a rig | Kept as it stands, and `--surface` got a **second** flag and a **second** error rather than reusing the first: a daemon may take its rig from `machine.json` and its surface from a command line at the same time, and `MachineError::SurfaceOnTheCommandLine` names `--surface` so an operator does not go looking at the output flags. The test target now starts its daemon the way S37's settings window will — no output flags, `--mock-devices`, and `AddOutput` |
| 2026-08-22 | S33 | **`rmp-serde` is not codec-symmetric about `SocketAddr`, and each half is separately reasonable.** Its serialiser reports itself as *not* human-readable and writes the address as a tagged map of octets; its deserialiser reports itself as human-readable and asks for a string. A value written with `to_vec_named` therefore cannot be read back with `from_slice`. `wire.rs`'s round-trip property found it within a minute of the type existing, which is exactly what that property is for | `prism_domain::socket`, a `serde` module that writes an address as **text on both codecs**. Not a serializer setting, because the IPC frame is built in `prism-ipc` and `machine.json` is written in `prismd` and there would be two places to remember it. Text is also what a technician reading the file expects and what `ts-rs` renders the field as, so the three agree by construction. **The general rule: a `std` type whose `serde` implementation branches on `is_human_readable` needs a module of its own** |
| 2026-08-22 | S33 | **A serial that pins one cable cannot come from a `&'static str` once the rig is configuration.** `DeviceDescriptor::with_serial` was built in S8 for exactly this case — two adapters, one output each — but its field could only hold a constant, and a `machine.json` cannot fill one | `Option<Cow<'static, str>>`. `SH_RS09B` stays a `const` with a borrowed string in it; `DeviceDescriptor` and `DeviceProfile` lose `Copy` and the whole ripple was two `clone()` calls. Whether two cables actually open as two is §14's new 🔌 row |
| 2026-08-22 | S33 | **The first attempt at a zero-allocation subscriber hand-over allocated exactly once, and it was the *retirement* vector rather than the link vector.** The links were `reserve_exact`ed at construction and the retired ones were not, so the first output ever removed cost the tick one `Vec::push` into an empty vector | `Vec::with_capacity(MAX_SUBSCRIBERS)` for both, and a note that `collect()` has to be called after every reconfiguration or a rig churned more than sixty-four times between collections would reallocate on a tick. The eighth path in `tick_allocations.rs` is what found it and what keeps it |
| 2026-08-22 | S33 | **A dark-universe report has to ask the *running* rig, not the configured one.** Asking the configuration says universe 5 is covered by a row whose thread would not start, or whose hop limit was refused — which is the reassuring answer and the wrong one | `prism_core::dark_universes` takes the universes rather than the configuration, so the caller decides whose answer it is: `MachineConfig::dark_universes` asks about the rig as configured and `prismd::Core::dark_universes` asks about the rig that is actually carrying frames. The second is what an operator is told |
| 2026-08-22 | S33 | **A health record keyed by position stops being true the first time the rig changes.** The daemon's housekeeping loop compared `Vec<OutputHealth>` against the output list by index, which S18 was right to do when the list was fixed at start-up and which would compare one output's health against another's the moment one was removed | A `BTreeMap<OutputId, OutputHealth>`, and an output that has gone is dropped from the record rather than reported as a change. A client learns it has gone from `Delta::OutputsChanged`, which is the fact rather than a symptom |
| 2026-08-20 | S39 | **A bar that wraps squeezes the table above it to nothing, and a table with no room for a row has a first row nobody can click.** The store bar is `flex-wrap: wrap`; the mode chooser made it wide enough to wrap at a Linux runner's font metrics, `.sheet-scroll` was `min-height: 0` and collapsed, and Playwright's `scrollIntoView` then put cue 1 under the sticky `<thead>`. CI failed on the one end-to-end test that clicks a cue cell; this machine passed | `.sheet-scroll` has a **floor** — `min-height: 3rem`, two rows' worth — so it still shrinks below its content but cannot disappear. Beside it: `scroll-margin-top` on `.sheet td`, so a row scrolled into view leaves room for the sticky header; the chooser lost its text label, because *Mode* beside a select that reads *Merge* says nothing twice; and the Edit and × keys share one cell, so the cue sheet keeps the seven columns it had. Reproduced at 1280 × 560 before the fix and asserted by mutation afterwards: putting `min-height: 0` back turns the repro red again |
| 2026-08-20 | S39 | **`rust-toolchain.toml` says `channel = "stable"`, so a new stable is a new lint set — and CI gets it before this machine does.** Rust 1.98.0 added `clippy::chunks_exact_to_as_chunks`, which failed the Windows job on six sites in `prism-engine` that had been there since S2. Nothing in the session touched them | The six are `as_chunks::<N>()` now, which is what the lint asks for and is better in `triple_buffer.rs` besides: the chunk arrives as `&[u8; 8]`, so the copy into a fixed array that `u64::from_le_bytes` needs *is* the chunk. **The general point for later sessions:** a green local `clippy` is a claim about the toolchain that happens to be installed, and CI is the one that finds out first. Pinning a version in `rust-toolchain.toml` would trade this for a different problem — never noticing — so it stays on `stable` |
| 2026-08-20 | S39 | **A crash test that kills a process at six fixed delays proves nothing on a machine where all six land between saves.** `a_process_killed_mid_write_leaves_the_last_committed_state` reported *6 kills, 0 inside a save* on a Linux runner and asserted, correctly, that it had tested nothing about a crash. The window is a fraction of a millisecond wide and how often a kill lands in it depends entirely on the disk | Twenty-four delays instead of six, and the loop **stops as soon as it has seen all three things it is looking for** — usually after one kill, so the ordinary run got *faster*. Running the whole list is now the unlucky case, and twenty-four consecutive misses would be a finding about the store rather than about the runner |
| 2026-08-20 | S39 | **A store's validation stopped being idempotent, and the second run of it refused a command that had already been applied.** `ShowFile::finish_programmer` writes the cue and *then* routes the command through `Programmer::apply`, which re-derives the store as a guard for a direct caller. Merging twice answers the same both times, so that cost nothing for four sessions — but a `StoreMode::Remove` that has been applied has taken its values out, so the second derivation answers *there is nothing to remove*, the command is refused, and the show has already moved | `Programmer::stored` does the Clear-stage half and nothing else, and `ShowFile::finish_programmer` calls it for the four store commands; the guard in `apply` stays for the direct caller it was written for, with a paragraph on each saying which is which. **Found by `crates/prismd/tests/ui_show.rs`, and only because it makes two comparisons** — a replay of the deltas *and* a fresh client's snapshot, which is exactly the pair a store that changed the show without broadcasting a delta falls between |
| 2026-08-20 | S39 | **The selected sequence had to become a field, and the argument that settled it is `Store Cue 5` with nothing selected.** S28 read it off `selectedExecutor` and marked the assumption; both readings are defensible, and the executor one has two things it cannot do — answer a store typed with no executor selected, and reach a cue list nobody has put on a fader | `Session::selectedSequence` and `Command::SelectSequence` (`ARCHITECTURE_SPEC.md` §4.1, §4.4). Deliberately **not** coupled to `selectedExecutor`: programming cue list 7 while executor 3 plays the show is the ordinary case on a console. The cost is a second selection on the screen and one rule the interface keeps — the playback marker is drawn only where the two are looking at the same list |
| 2026-08-20 | S39 | **`StorePreview`'s three counts could not describe an Override.** *Kept* is what a Merge leaves alone; under an Override the same values are what it throws away, and a preview that reported them as kept would tell an operator the opposite of what was about to happen | A fourth count, `removed`, and a table in the type's own documentation: the four of them partition every value on both sides for each of the three modes, so `kept + replaced + removed` is what is filed there now. Asserted as a **sum** in three places rather than as three separate numbers, because a mode added later that forgot a column then shows up as arithmetic that does not add up |
| 2026-08-20 | S39 | **The mode had to move from the answer into the question.** S28 put `StoreMode` on `Answer::StorePreview` precisely so no client would spell it. With three modes and an operator choosing, an answer that still decided the mode would describe a store the button was not offering | `Query::StorePreview` carries the mode and the answer **echoes** it. The client still renders `preview.mode` rather than the mode it sent, which is what stops a bar drawing one mode's word over another mode's numbers for the round trip after every change — asserted by answering an Override question while the chooser reads Merge |
| 2026-08-20 | S39 | **A `Merge` into the cue being edited must not stop the Update key blinking; an `Override` must.** A merged cue holds everything the programmer holds *and more*, so *nothing has changed since it was loaded* is false of it | One condition in `ShowFile::follow_cue_edit`, and `Command::Update` is *resolved into* a `StoreCue` in Override mode rather than implemented twice — so the rule is one rule. The neighbouring decision: a **renumber** of the cue being edited carries the update state rather than clearing it, because an Update stores into the number it remembers and would otherwise recreate the cue at its old number |
| 2026-08-20 | S34 | **The readback and the daemon's own optimistic write are two authors for one field, and the loser is whichever arrives second.** `Core::carry_out` had written `is_active` on the way past a Go since S17, because nothing else could. With `PlaybackReport` beside it the first poll after a Go sees the tick's state from *before* the command — the tick has not run yet — and writes `false` back over it. The visible symptom is a strip that lights on the command, goes dark 25 ms later and lights again a tick after that | **The tick is the only author of `Executor::is_active` and `currentCueIndex`.** `Core::record_executor` is gone and `ExecutorGo` is acknowledged with no delta at all; `Core::poll_playback` is the one place either field is written. The cost is one poll of latency on the strip lamp — under 50 ms including the tick — and one real window: a `Toggle` pressed within a poll of *another client's* Go reads the state from just before it. That is the latency any desk with two operators has, and it is named in §7 rather than hidden |
| 2026-08-20 | S34 | **A `Flash` cannot be a `SetExecutorMaster`, and the reason is not the one that is usually given.** The obvious objection is that the release has to restore the old value, which a save-and-restore would handle. The real one is the *third* command: a fader moved while the flash is held would be overwritten by the restore, and the operator would watch their own fader move snap back | The held level lives beside the stored one on `PlaybackSource`, and `docs/DMX_MERGE.md` §2.1's `masterLevel` term is the one in force. `set_master` writes the stored level whatever is happening above it, so a level that arrives during a flash is what stands when the key comes up — asserted byte for byte at three levels: the engine's frame, the show the daemon holds, and the percentage a browser draws |
| 2026-08-20 | S34 | **A speed master cannot be a multiplier on the tick, only on the player's own clock.** The first shape considered — scale the tick index a player reads — is wrong for two reasons at once: two executors cannot then have different rates, and a rate change mid-fade jumps the fade rather than slowing it | `CuePlayer` accumulates `speed`/`SPEED_UNITY` of a tick per **tick slot** and carries the remainder. At unity that is exactly `+1`, so every fade in the project is the number it was and all 280 of `prism-engine`'s existing tests passed unchanged — which is what says the accumulator is a generalisation rather than a rewrite. Advancing by *slots* rather than by one call is what keeps a stalled tick from leaving the show behind wall-clock time |
| 2026-08-20 | S34 | **`Command::ExecutorButton` cannot carry only a slot index, and cannot carry only a function.** A slot alone cannot express `docs/MCU_MAPPING.md` §4.1's transport row, which assigns Play the function `On` in prose. A function alone would let a client read `buttonFunctions` and send what it decided, which is D3 with the label filed off | `ExecutorButtonRef` is `Slot { index }` **or** `Function { function }`, and the distinction is between *configuration* and *inference*: a profile row is written by a person and a client's run-time reading is not. Either way `prism_core::Show::apply` decides what the function comes out as — a `Function { Toggle }` is still resolved against `is_active` by the daemon. S26's mutation check still guards the other half |
| 2026-08-20 | S34 | **Adding a field to a persisted domain type broke every existing show file, and the frozen version-1 fixture found it in under a minute.** `Executor::speed` made `store::tests::the_frozen_fixture_is_the_show_it_was_written_from` fail with *executor row 17: missing field `speed`* | `#[serde(default)]` with a named function that says why: an executor written before S34 was playing at unity, which is the only rate it could have been playing at. **A schema migration cannot help here** — `MIGRATIONS` rewrites *tables* and this lives inside a MessagePack blob (S15). Worth knowing for every later field added to a persisted type: the fixture is the guard, and the answer is a documented default rather than a migration |
| 2026-08-20 | S34 | **The thirty-sixth `Command` variant overflowed the stack of two property tests, with no failing case to read.** `prism-core`'s `tests/oops.rs` and `prism-ipc`'s `tests/framing.rs` both died with `STATUS_STACK_OVERFLOW` in a debug build; `RUST_MIN_STACK=32M` made both pass, which is what identified the cause. `proptest_derive` builds one value tree holding every variant's tree at once, and the aggregate is moved by value through the generator | Both strategies `.boxed()`, with the reasoning written beside them in each file. S27 had already met the outer half of this in `framing.rs` and boxed the union; what was left was `any::<Command>()` **inside** an arm. Recorded in §7 so a later session reaches for `.boxed()` rather than for the environment variable |
| 2026-08-20 | S34 | **An asynchronous delta breaks a frozen recording quietly.** The six recorders stop at the `Ack` and then take a fresh client's snapshot. With `ExecutorState` arriving a poll later, the delta lands in the *next* step's list while the snapshot beside it already contains it — so a per-step replay disagrees with a per-step snapshot, which is exactly the claim three of the browser's suites make | `ui_recording`, `ui_programmer` and `ui_show` drain until 150 ms pass without a **delta**. Only a delta resets the window: the daemon publishes telemetry thirty times a second, and the first implementation waited for silence on the socket and hung for ever. Recorded in §7 for any later recorder that issues a playback command |
| 2026-08-20 | S34 | **A store preview asked on every show change becomes a question per frame once a playback moves the show.** S28 left the warning and S27 left it before that about `PatchConflicts`; S34's readback is what makes it real, because `Delta::ExecutorState` patches `/executors/<id>/currentCueIndex` | The two store bars depend on the **subtree** a preview actually reads — `looks.ts::sequencesDocument` and `presetsDocument` — rather than on the show root. `mirror/patch.ts`'s structural sharing is what makes that identity stable while an executor runs. Asserted rather than argued: *does not ask again when a playback advances a cue*. The delta itself is also only broadcast on a change, so the two mechanisms are independent |
| 2026-08-19 | S28 | **A preset link had never been a link.** `prism_domain::preset` has said since S1 that *a cue part that carries a `presetRef` follows later edits of the preset*, `Show::issues` reports one that dangles, `Programmer::cue` carefully drops a reference to a preset that has gone — and nothing anywhere made the value follow. A cue stored from an applied preset kept the number the preset had when it was stored, for ever | `Show::store_preset` now rewrites every cue part linked to the preset (`Show::relink`), and answers with the operations that describe it. It is done in the **show** rather than in the engine, because the tick may not resolve anything (§3.1): the value in a cue is always the value that will be output, and the link is what keeps it current. A part the new preset does not mention keeps **both** its value and its link — dropping the value would change light nobody asked to change, and dropping the link would mean a preset that regained the value could never reach the cue again |
| 2026-08-19 | S28 | **A store preview that counted the merged result was wrong in the one case it exists for.** The first implementation asked `Programmer::cue` for the merged cue and compared *that* against what was stored — which reports every value the merge left alone as one it had replaced, so a store that touched one value in a cue of forty said *40 replaced* | The counts come from `Programmer::stored_keys`, which is the same filter the store itself runs, so what is counted is what the store **writes**. `kept` is then `stored − replaced`, and it is the number that makes Merge legible: exactly what an Override would have thrown away. Found by writing the assertion for the pool case before the implementation was believed |
| 2026-08-19 | S28 | **`AssignExecutor { sequenceId: null }` on an empty slot made a row an Oops could not remove.** S14's property test — undo every command, compare the serialised bytes — is what found it: clearing a slot that had nothing on it created a default executor, and restoring the *absence* of an executor was something `Show` had no operation for | Two rules rather than a special case: clearing a slot with nothing on it changes nothing at all, and `Show::remove_executor` exists so an Oops can put the grid back exactly as it was. `Image::Executor` carries the absence, as `Image::Fixture` has since S27. **The property test is the reason this was a five-minute fix rather than a bug report from a venue** |
| 2026-08-19 | S28 | **A store mode a client spells is a client that will be wrong in S39.** The exit criterion asks for the mode on the button *even where the only mode available is Merge*, and the obvious way to satisfy it is to write the word in the interface | `StoreMode` is a one-variant enum in `prism-domain`, carried on `StorePreview`, and the interface renders the word it is **given**. The type exists for the day it has three values: `merge_is_the_only_store_mode_this_build_has` and the browser's *every recorded preview is a Merge* both go red then, which is a note that cannot be forgotten. No command carries a mode, because a command carrying one the daemon did not honour would describe an outcome that did not happen |
| 2026-08-19 | S28 | **A show could not be written at all from an interface, and the missing pieces were not the ones the plan named.** `StoreCue` needs a sequence to store into and `ExecutorGo` needs one on an executor to reach, and the protocol had no way to make either — so every cue in the project so far came out of a show file written by a test | `CreateSequence` and `AssignExecutor`, both refused where a *create* would destroy something: a sequence number that is taken, and a sequence that does not exist. They are deliberately **not** S39's `StoreSequence`, which is a different act with a mode on it — this pair makes the cue list *exist*, and S39 decides what storing the programmer into one means. The executor's defaults are the daemon's, for `PatchFixture`'s reason: what a fader and four buttons do is show content |
| 2026-08-19 | S28 | **A playback that is already running does not follow a preset edit.** The end-to-end test fired a cue, edited the preset it referenced, and found the rig still at the old level — `prismd::Core::rebuild` installs a new merge body and the player keeps the levels it had faded to | Recorded rather than changed. It is the safe behaviour for a show in progress and it is not the behaviour a console has, and playback is **S34**'s. The end-to-end test asserts the reachable half — the *same Go* after the edit puts a different level on the rig — and says in as many words why the Go is needed. §7 carries it |
| 2026-08-19 | S35 | **The same mirror outage then took the ARM64 job**, which is the workflow's *other* `apt-get update` — `sudo apt-get update && sudo apt-get install -y gcc-aarch64-linux-gnu`, needed since S15 because `rusqlite`'s bundled SQLite has to be *compiled* for the target even though `cargo check` does not link. It stalled for fifteen minutes and was killed by its own `timeout-minutes`, in a job that normally takes **41 s**. Unlike the Playwright case the package is genuinely required, so the flag could not simply be dropped | The update is off the path when it is not needed: the runner image ships package lists and this is an ordinary package, so the plain `apt-get install` is tried first and the index refresh is the **fallback** — three attempts, each under a `timeout`, with an `::error::` that names the cause. A stalled mirror now costs a minute and says what happened instead of consuming the job. **The general shape of both fixes: never let an unbounded package-manager call sit on a critical path**, and give every network step a timeout so an outage fails loudly and quickly rather than looking like a hang |
| 2026-08-19 | S35 | **A test was skipping itself on CI and saying something false while it did.** `e2e/patch.spec.ts`'s Open Fixture Library test decides whether a library is installed by counting the search matches — but the search is a `Query` round trip and Playwright's `locator.count()` does **not** wait. So it was really asking *has the answer arrived yet*, and reading *no* as *there is no library*. On the green run of `d822c02` it skipped with **634 fixtures installed**, reporting `no fixture library is installed`. It had passed on luck before, and a skip is silent — the suite reported *17 passed, 1 skipped* and looked healthy | Whether a library exists is the **daemon's** answer: the search box's placeholder carries the count out of the snapshot, which is a fact by the time the window is open, so the test skips only when the daemon says zero and otherwise waits for the match with `expect(...).toBeVisible()`. S44's test, touched here because the hole was demonstrated here and a silently skipped test is a hole in every later session's CI too. **The general lesson for any spec: `count()` is the one Playwright locator method with no auto-wait, so it must never be the thing that decides whether a feature exists** |
| 2026-08-19 | S35 | **`npx playwright install --with-deps chromium` made the end-to-end job depend on the Ubuntu package mirrors, and they went down.** The flag runs `apt-get update` before fetching the browser. On this day `azure.archive.ubuntu.com` stopped answering (`Ign:` on every index), apt fell back to `archive.ubuntu.com`, fetched three `InRelease` files and then produced **no output for twenty-two minutes** until the job was killed — on two consecutive runners, in a step that had taken **27 s** the run before. The browser binary was never reached, and nothing in the commit touched the workflow, `package.json`, the lockfile or the Playwright config | **The flag is gone.** `ubuntu-latest` already ships the shared libraries Chromium needs, so `--with-deps` bought nothing except a hard dependency on a mirror; without it no apt runs at all. A genuinely missing library now shows up as Chromium failing to launch, which the end-to-end tests report loudly — a better failure than a package manager that never returns. `~/.cache/ms-playwright` is cached on the lockfile hash as well, so the download itself is needed once rather than every run. **The other four jobs were green throughout**, including the full UI typecheck/lint/test/build, and all eighteen end-to-end tests had already passed locally against a real daemon |
| 2026-08-19 | S35 | **The telemetry frame-budget gate is a throughput measurement on a desktop, and it reads a busy machine as a code regression.** Taken while an unrelated Java process was holding the CPU at 80–96 %, it reported **15.5–18.6 Hz with 138–152 frames lost** against a baseline run that happened to land in a quieter moment at 28.4 Hz — which is exactly the shape of a real regression, and an hour went into bisecting the stylesheet for it. On a quiet machine the same build reads **30.4 Hz with nothing lost** | **Check the load before taking the measurement**, and re-run *both* sides on a confirmed-quiet machine before calling anything a regression: `Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'"` (the `Get-Counter` path is localised and fails on a German install). The `contain: layout` this false alarm produced was **removed** once it could be measured properly — with and without are the same to within noise, and a declaration justified by a number nobody can reproduce is worse than none. §3's note beside the two telemetry figures now carries the caveat |
| 2026-08-19 | S35 | **A view library carries its order in one of two places, and only one can be right.** `views` is a `BTreeMap<ViewId, View>`, the View Selector Bar draws in number order, `SelectView` names a number and `prismd::surface::context_of` resolves `Channel ◀▶` by comparing numbers. "Move a view" therefore means either *exchange the two numbers* or *keep an ordering beside the map* | **The number is the order.** `MoveView` exchanges the two views' numbers, so there is exactly one order and nothing to keep in step — the failure the alternative invites is the console stepping to a view other than the one drawn next, and it would have been invisible until an operator met it mid-show. `SessionState::neighbour` is the single place the order is expressed, and `delete_view`, `move_view` and `context_of` all resolve through it. **The price, paid deliberately and recorded here:** after a move `SelectView 3` names a different layout, and an F-key bound to a view number follows the *place* rather than the layout that used to be there — which is how a console's page numbers behave. Asserted in `session.rs`, in `ui_session.rs`'s guard, and observed end to end with a real console in `e2e/session.spec.ts` |
| 2026-08-19 | S35 | Deleting the **active** view leaves `activeViewId` naming something that is not there, and whichever client sent the command is the wrong place to resolve it — two screens would answer differently | `SessionState::delete_view` selects the neighbour **before** it, or the one after it when there is none, exactly as `select_view` would have — so the canvas afterwards is a *stored layout* rather than whatever happened to be open. The interface sends the command and draws what comes back; there is no successor-picking code in TypeScript. The recording carries the answer and `ui_session.rs` asserts `activeViewId` still names a stored view and that the canvas is that view's layout |
| 2026-08-19 | S35 | The **last** view cannot be deleted: `activeViewId` names a view from the first moment (`SessionState::new`) and `ShowStore` refuses a file whose active view is not stored, so a session with no views could satisfy neither | `SessionError::LastView`, a floor in the daemon rather than something a caller has to remember — and the menu item is disabled as well, so the refusal is not the first an operator hears of it. `the_session_invariants_survive_any_command_sequence` generates arbitrary command sequences and asserts `session.view(active_view_id).is_some()` throughout, which is what says the floor holds under `DeleteView` |
| 2026-08-19 | S35 | `programmerPage` has been in `ARCHITECTURE_SPEC.md` §4.1 since S12 and **nothing read it**. Giving it meaning needs a page size, and a page size implies an upper bound — but `prism-core` deliberately does not know how many parameters a bank has (S13) | The page **size** is the interface's (four, `ENCODERS_PER_PAGE`) and so is the **bound**, which is the split `SelectProgrammerParam` already had — §4.4 records it. `encoderPage` clamps the session's number to the bank and the two page buttons go dead at the ends. **A consequence worth knowing:** the console's `Zoom ▼` resolves its step against the session (`prism_surface::binding::step_page`) and saturates only at zero, so holding it down on a short bank runs `programmerPage` past the end and the bar shows the last page until the number is walked back. Bounding it in the daemon would mean the daemon knowing bank sizes, which S13 decided against; **S43 or S40** could give the bar a correcting command instead |
| 2026-08-19 | S35 | The plan asked for *store the canvas as a new view **in that place***, which under the decision above would mean renumbering every view above the insertion point | The menu stores at **one past the highest** and the operator moves it, which is one command that always works. Inserting would have been a fourth command and, worse, would silently change what every F-key bound to a view number reaches. The menu item says *at the end of the bar* rather than implying otherwise |
| 2026-08-19 | S35 | **`.strip` was defined twice in `App.css`** — the executor strip (`display: grid`, five rows) and the status strip in the footer (`display: flex; flex-wrap: wrap`). The later rule wins at equal specificity, so the eight executor strips had **never** used the grid their own rule sets up. Present since S26 and invisible because the flex fallback looked plausible | The footer's is now `.status-strip`. Found by reading the stylesheet for the one-band rebuild rather than by any test — **no test can see this**, which is the note for a later session: a stylesheet has no type system, and a duplicated class name is a silent override. S43's polish pass should sweep for repeated selectors |
| 2026-08-19 | S35 | A command can be **accepted and change nothing** — `MoveView` on the view already at the end of the bar. `ui/src/canvas/windows.test.ts` asserted *no deltas ⇔ refused*, an equivalence that had been accidentally exact because no earlier script contained a successful no-op | Weakened to the direction that is true: a refusal produces no delta. `prism-core` answers a no-op with no delta rather than a `SessionPatch` holding no operations — a broadcast that says nothing — and the script now contains one on purpose so the property is exercised |
| 2026-08-19 | S35 | **`console.log` was in shipped code** (`patch/sheet.tsx`, a row click that logged and did nothing else) and `oxlint` had no rule against it, so `CLAUDE.md`'s prohibition was enforced by nobody | `eslint/no-console` is now an error, with `src/log/logger.ts` — the structured sink the rule exists to require — and the test and end-to-end trees exempted. The rule found the stray call and nothing else |
| 2026-08-19 | S35 | The notices panel had grown a dismiss button that kept a **second list**: a component-local set of dismissed identifiers beside the store's `notices`. `notices` is a ring buffer capped at `NOTICE_LIMIT`, so once an id was evicted the two lengths disagreed and the panel stayed on screen as an empty bordered box the operator could not get rid of | `DeskStore::dismissNotice` — one list, and the view holds only *which item is currently sliding out*, which is §4.2's category. The removal waits for the collapse's `transitionend` rather than a `setTimeout`, so the duration lives in the stylesheet alone; `App.css` carries the note that the component names the property |
| 2026-08-19 | S35 | The executor strip's select target had been moved from the head `button` onto the strip **container**, which wraps the fader and the four function buttons. A pointer down and up on a fader synthesises a click that bubbles, so every master move and every Go sent a `SelectExecutor` as well | The head is a `button` again — focusable, keyboard-reachable, announced — and the nine executor-bar tests that had been commented out are restored and green. They are S26's exit criteria, which S35's own criteria require to still hold; a criterion whose test is commented out is not a criterion |
| 2026-08-15 | S44 | **Reading the library is 395 ms in release and 1.5 s in debug, and it was on the daemon's start-up path *ahead of the engine*.** That is the one place this project says nothing may wait: a desk's first duty is to put light on stage. It surfaced as a Windows CI failure in `wiring.rs` — a client shown a telemetry frame of all zeros for a rig whose first dimmer is at full — which was the second defect rather than the first | The read moved to a **thread of its own**, started before the engine and joined after it, so the engine and the outputs come up while the profiles are parsed and DMX begins at the same instant it always did. Measured on this machine, release build: **1 348 ms without a library, 1 674 ms with one** — so what waits is a *client connecting*, by about 330 ms, and not one frame of output. A joined thread that panicked leaves the desk with its built-in profiles rather than stopping it |
| 2026-08-15 | S44 | **A client could be shown a telemetry frame the engine had never produced.** A triple buffer starts blank, and the telemetry task published whatever the subscriber held — so a client connecting between the listeners binding and the first tick saw a picture of a dark rig that was in fact lit. It held by *timing* until S44 put 8.5 MB of parsing near the start-up path, and then it did not | The daemon publishes nothing until the engine has published something. `FrameSubscriber::refresh` already answered the question and its answer was being thrown away — it means *was there a new frame since last time*, which is false for a rig that has settled as well as for one that has not started, so the daemon latches it. Telemetry may be **dropped** (§7); it may not be **wrong**, and those are different permissions |
| 2026-08-15 | S44 | **The reported tick rate read 97 Hz for an engine ticking at 44.** `TickHealth` counts ticks from the moment the tick thread starts; `Core` measured the time from the moment it was *constructed*, which is after the outputs are open — and after S44's library join. Two numbers with different origins divided by each other | The origin is the engine's: `EngineThread::uptime`, recorded when the thread is spawned, and `Core::uptime` delegates to it. A pre-existing inaccuracy that anything slow between the two would have exposed; S44 was slow enough |
| 2026-08-15 | S44 | **An end-to-end test raced against the round trip, and passed on this machine three runs out of five.** `a sheet with more rows than it has room for` patches forty fixtures in a loop, and it took the number from the form's own suggestion. `nextFreeFixtureId` proposes the lowest number **the client currently holds no fixture for** — and a client holds what the daemon has sent it, which is D3 working exactly as designed. On a runner where the round trip is slower than the next click, two drafts get the same number and the second patch is a *repatch* of the first: thirty-eight fixtures, and a test that failed for a reason with nothing to do with scrolling. It surfaced on the two **docs-only** pushes, which is the most misleading place it could have surfaced — a PROGRESS.md edit cannot break CI, so the run that fails after one looks like something it is not | The loop **types the number** instead of accepting the suggestion, which removes the dependence on arrival order rather than widening a timeout. The suggestion is documented as a convenience an operator types over, so a test typing over it is the ordinary use of it. Recorded rather than quietly fixed for two reasons: a flake that passes three times in five is one somebody will later *re-introduce*, and `IMPLEMENTATION_PLAN.md`'s session protocol says a PROGRESS-only push need not be watched — which is true of what the push changes and not of what the run reveals. **Watch it anyway** |
| 2026-08-15 | S44 | **The fixture library cannot be committed, and that is a decision rather than a preference.** The Open Fixture Library is 8.5 MB of JSON with an upstream, a schema version and a release cadence. Vendored here it would be a second copy of somebody else's data — and the copy that is out of date, because nothing in this repository would ever learn that it had moved | **Downloaded at install time.** `tools/fetch-fixtures/fetch-fixtures.sh` and `.ps1` use nothing but `curl`/`Invoke-WebRequest` and `tar`, all of which ship with Windows 10 and later, every Linux this runs on and macOS — a downloader with a dependency is a dependency somebody has to install before they can install anything. The revision is **pinned rather than `master`**, which is what buys back the reproducibility vendoring is usually for: two machines that install a week apart get the same profiles, so a show patched on one opens the same way on the other. `.gitignore` keeps everything but `SOURCE.md` out; CI installs it, so the corpus tests run on every commit rather than skipping |
| 2026-08-15 | S44 | **Two thousand profiles do not fit in a snapshot, so S27's `fixtureLibrary` field was wrong the moment the library got real.** 634 fixtures become 2 157 profiles, which is several megabytes against `docs/IPC_PROTOCOL.md` §3's 1 MiB frame — and a menu of two thousand entries is not a menu whatever the wire could carry | The field became a **count**, and the list moved behind `Query::SearchLibrary` → `Answer::LibraryMatches`. The answer carries a `LibraryEntry` — key, manufacturer, name, mode, footprint — and never a `FixtureType`: **the profile itself does not leave the daemon**, which is the same rule that keeps channels out of `PatchFixture`. The `limit` is clamped by the daemon, and the worst query the corpus admits is asserted to encode under 256 kB. This is the second caller of the query channel S27 added a day earlier and the first that *needed* it rather than merely suiting it — which is the best evidence that shape was right |
| 2026-08-15 | S44 | **A JSON Pointer built from a key with a `/` in it names the wrong thing, and three shipped readers did exactly that.** An OFL key is `manufacturer/fixture/mode`. `prism_core::show::escape` has done RFC 6901 §3 correctly since S11 and both mirrors unescape on the way in — but a *view* that interpolates a key into a pointer had no escaping at all, so `patchRows`, `liveFixtures` and **S26's `groupOf`** answered `null` for every OFL fixture: a fixture sheet with no attributes, an encoder bar with no banks, a patch row with no footprint | `mirror/select.ts::pointerToken`, used at every site that puts a key into a pointer. Found by the recording's own guard rather than by review, which is what those guards are for. Worth recording because the bug was **invisible until a key contained a slash** — every key in the project did not, for four sessions, and the readers were written against that accident |
| 2026-08-15 | S44 | **Seven of the 634 files are not fixtures.** They are OFL *redirects*: a stub left under the old key when a fixture is renamed, or when two brands turn out to sell the same light — the Lixada Mini Moving Head RGBW and the Stage Right Stage Wash 7×10W are one device, and the request that started this session linked the Lixada one | **Followed rather than skipped.** The alias is filed under the redirecting key, carries the name on the box and the channels of the fixture it points at, so an operator holding a Lixada searches for *Lixada* and finds it. 73 of the 2 157 profiles are aliases. Skipping them would have been defensible and would have hidden a light from the person holding it |
| 2026-08-15 | S44 | **The conversion is lossy in a way that names a later session.** This domain model has fifteen `AttributeType`s; OFL has some ninety capability types. 714 of 2 798 modes are skipped because their channel list contains a matrix insert or a switching channel — the layout depends on state and a footprint does not — and the colours there is no attribute for (UV, Cyan, Yellow, Magenta, Lime, Indigo) are dropped, so **a CMY fixture patches with its colour mixing missing** | Measured rather than estimated, counted in `ofl::Conversion`, printed by the corpus test on every run, and shaped by assertions rather than pinned to numbers a re-import would break. Widening `AttributeType` is a `prism-domain` change that reaches the merge, the encoder banks and the generated bindings — a session of its own, and §7 carries it. Matrix fixtures want a different model again |
| 2026-08-15 | S44 | `EmbedFixtureType` could no longer be applied by the show model: it names a key, and the show has no library — as it has no disk | `Show::apply` answers `Effect::EmbedProfile` and `ShowFile::apply` carries it out, which is exactly the shape `Effect::Programmer` and `Effect::Save` already had. It matters that it is `ShowFile` and not the daemon: the journal is filed there, so an embed stays undoable, and `finish_embed` runs **before** the record. `ShowFile` gained a `library` field, `#[serde(skip)]` like the journal and defaulting to the built-in four, because a file is not where a desk's profiles live |
| 2026-08-15 | S44 | **`cargo llvm-cov` invoked twice in one shell reports figures tens of points low.** Two runs' profile data are merged, and the first reading of `prism-core` here came out at 92 % against a true 99.13 % — with `show.rs` at 66 % and `session.rs` at 73 %, which looked exactly like a real regression | `cargo llvm-cov clean --workspace` between crates, and the caveat recorded in §3 beside the figures. Worth writing down because the wrong number is *plausible* — it is the kind of thing that gets recorded in this file and then defended |
| 2026-08-14 | S27 → S44 | **Four generic profiles cannot patch a real rig, and the session that shipped them said so.** S27's *carried out of* list names the fixture library as the largest thing it left; the request to close it came the same day. Two things about it change what S27 built rather than merely adding to it. The Open Fixture Library is **2 798 modes across 634 fixtures**, so the `Snapshot.fixtureLibrary` S27 added — sound for four profiles — would put several megabytes into a frame the protocol caps at 1 MiB (§3), and a client cannot hold a menu of two thousand entries anyway. And the conversion is **lossy**: this domain model has fifteen `AttributeType`s, and OFL fixtures carry capabilities there is no room for | Logged as **S44** rather than reopening a session that is closed and CI-verified — the plan's own rule is that added sessions take the next free number and the running order says what to do next, so S44 runs second. The library moves out of the snapshot and behind `Query::SearchLibrary`, which is the second caller of the channel S27 added and the first that **needs** it. The loss is measured rather than estimated and will be recorded with the session: 2 084 modes convert, 714 are skipped for a matrix insert or a switching channel this model cannot express, and the colours it has no attribute for — UV, Cyan, Yellow, Magenta, Lime, Indigo — are dropped rather than guessed at, which is a finding for a later session that widens `AttributeType` |
| 2026-08-14 | S27 | **The protocol could not answer a question, and S27's central exit criterion is one.** *Address conflicts are shown before they are committed* needs the daemon to say what patching a fixture at an address **would** do, and a `Command` expresses intent while a `Delta` describes a change that has already happened. Both alternatives were considered and both are worse: a client that intersected the address spans itself would be a second opinion about something `prism_core::conflict` already decides — the duplication **D3** exists to prevent, and the copy that would drift the first time a footprint rule changed — and a command that patched and then offered an Oops would show the operator the conflict by *making* it, on a rig that is on stage | `docs/IPC_PROTOCOL.md` §5.2 is new: `ClientMessage::Query` and `ServerMessage::Answer`, sharing the command numbering because both travel on one ordered channel. `Query::PatchConflicts` and `Query::PatchPreview`; answered by `Show::preview_patch`, which takes its refusal from the **same** `check_patch` the edit runs. Four rules, and the first three are asserted rather than described: a query changes nothing (`ui_patch.rs::a_question_changes_nothing` — no deltas at all, and the patch identical to the step before); the answer goes to the client that asked rather than being broadcast, because what one operator is typing into a form is nobody else's business (§4.2); there is no `Query::Show`, because a question returning state a client already mirrors would be a second path to the same fact; and the answer is not droppable, because a client that asked and never heard back would wait for ever |
| 2026-08-14 | S27 | **A brand-new show could not be patched at all from an interface, and the hole was in the middle of this session's goal.** `PatchFixture` names a `type_id` the show must already carry — the embedding rule S11 took from S1 — and `Show::embed_fixture_type` had no command in front of it. A fresh show carries no profiles, so the patch window of one was a form with an empty menu and no way out of it. There is no fixture library either: `ARCHITECTURE_SPEC.md` §9 reserves `profiles/fixtures/` and nothing has ever written it | `Command::EmbedFixtureType { typeId }` carries **a key and nothing else**, resolved by the daemon against the new `prism_core::library` — a dimmer, two PARs and an eleven-channel moving head. A client that sent a whole `FixtureType` would be authoring show content for the daemon to validate, which is the same rule that keeps the channel layout out of `PatchFixture`. The library rides in the `Snapshot` rather than behind a query: it never changes while the daemon runs, and a client needs it in order to *offer* the list at all. **What this is not:** a fixture library in the sense a venue means — no import, no GDTF, no modes, and no way for an operator to author a profile. That is a session of its own and §7 carries it as the largest thing S27 left |
| 2026-08-14 | S27 | **A patch that can only be added to is not one an operator can correct.** `PatchFixture` repatches an existing number, so a name, a type, a universe and an address were all reachable — but a fixture could never be *removed*, and its **number** could not be changed at all, because the number is the key the patch is filed under | `Command::UnpatchFixture { id }` and `Command::RenumberFixture { id, to }`. The renumber is deliberately **one** command and not an unpatch plus a patch: two commands leave the rig without that fixture in between, and leave it deleted if the second is refused. It is journalled over **both** numbers — one image would restore half of it — and renumbering to the number a fixture already has is accepted, changes nothing and is **not a step**, because an operator pressing Oops after one would watch nothing happen and press it again. Neither cascades into groups, presets or cues: a show outlives the rig it was written on (S11), and `Show::issues` reports what now dangles |
| 2026-08-14 | S27 | **`PatchConflict` had to move to travel.** It lived in `prism-core`, which no client depends on and which exports no TypeScript, so an answer carrying one could not be typed on the wire | Moved to `prism_domain::query`, re-exported from `prism-core` so every existing caller is unchanged, and `ts-rs` now generates it. `UndoScope` lost its `Copy` in the same session, because `EmbedFixtureType`'s scope is a profile *key* rather than a number; a scope is compared and printed and never counted on in a hot path, so a move is not a cost worth naming a variant vaguely to avoid |
| 2026-08-14 | S27 | **Two things a patch sheet could work out for itself, and both are the same rule.** Whether an address clashes is one. The other is subtler and was nearly written: *which channel a fixture ends on* is `address + footprint − 1`, which looks like formatting and is exactly the arithmetic the overlap search runs | Neither is computed in the client. The sheet shows the start address and the footprint as the document holds them, and the **span** is only ever shown from a `PatchPreview`, where the daemon computed it. Written down at the top of `ui/src/patch/patch.ts`, because the next person to want a *Channels* column will want to add one line and it is the wrong line |
| 2026-08-14 | S27 | **A question per keystroke means several are in flight, and they are not answered in order.** An answer to a draft that has since been typed over is not merely stale: drawn, it tells an operator that the address in front of them clashes when it is the one they have already replaced that did | `PreviewRequester` numbers every request and delivers only the newest, and `DeskStore.ask` answers with `null` — never a guess — when there is no daemon, when nothing comes back within five seconds, or when the connection goes while a question is in flight. The last of those is what stops a patch form waiting out a timeout and then drawing an answer about a show nobody is holding any more |
| 2026-08-14 | S27 | **The snapshot grew a field, so every recorded snapshot payload went stale at once.** All four frozen recordings failed to decode with `missing field fixtureLibrary` — in Rust, on the first `cargo test`, which is what those guards are for | All four regenerated and the diffs read. It is the second time the fixtures have earned their keep this way (S24 wrote the rule down), and the cost of the rule is exactly this: a session that changes the shape of a message regenerates five files and looks at what moved |
| 2026-08-14 | S27 | **Adding a third arm to `prop_oneof!` overflowed the stack of a debug test thread.** `framing.rs`'s two-message property builds two nested `TupleUnion` value trees with `Command`'s derived tree inside each, and the extra layer took it past the 2 MiB a test thread gets. It passed alone and failed when the target ran its seven tests in parallel, which is the kind of failure that gets blamed on the machine | The two message strategies are `.boxed()`, which erases the type and puts the recursion behind a vtable. Worth recording rather than fixing quietly: the next session to add a message variant will meet it again, and *the property test suddenly overflows* is a symptom nobody would connect to *the enum grew* |
| 2026-08-14 | S26 | **S22's executor-button gap is still there, and S26 met it from the other end — but it is an *engine* session, not an interface one.** The executor bar draws the four buttons a show assigns each strip, and four of the eight functions (`On`, `Flash`, `Toggle`, `LearnSpeed`) have no command behind them. Working out what closing it would take is the part S22 could not see from a binding table: `On`, `Off` and `Toggle` are reachable **today** through `prism_engine::TickCommand::SetExecutorActive`, which S5 built and which the daemon may resolve against `isActive` because the daemon owns it; `Flash` needs a *temporary* master override that does not disturb the stored master, which neither the engine nor the vocabulary has; and `LearnSpeed` needs speed masters, which nothing implements | Recorded, not half-built. The bar draws every assigned button, presses the three that resolve (`Go+`, `Go-`, `Off`) and draws the other four **disabled with the reason on the button** — resolving `Toggle` against `isActive` in a *client* would be a client deciding what a show's own setting means, and two clients doing it would race. The shape the command wants is `ExecutorButton { executorId, button, pressed }` — `pressed` is what `Flash` needs — and it costs `prism-domain`, `prism-core` routing, an `Effect` and engine work for two of the eight. `docs/MCU_MAPPING.md` §4.2.1 now carries the table. This is the third finding of S22's shape and the first one **not** taken; the two S25 took were a command and an enum variant, and this is a feature |
| 2026-08-14 | S26 | **The encoder bar and the jog wheel had to agree on an order, and S22 could only leave a warning about it.** `prismd::surface::parameter_of` walked `AttributeType::ALL` filtered by the encoder bank; the encoder bar needed the same list, and TypeScript had no way to compute it — `AttributeType::feature_group` is Rust logic and nothing in `ui/src/bindings/` carried it. Hand-writing the list in the interface is precisely the drift `variants.ts` exists to remove, and the failure it produces is nasty: the wheel turns one parameter while another one is highlighted, and nobody thinks to blame a table | **One table.** `FeatureGroup::attributes()` in `prism-domain` — asserted to be exactly the filter over `AttributeType::ALL`, so the `const` slice cannot drift from `feature_group()` — and `parameter_of` is now `group.parameter(index)` and nothing else. `export_bindings` emits it as `FEATURE_GROUP_ATTRIBUTES` beside the variant tables, with the spellings still read back out of the unions `ts-rs` wrote. The agreement is then **watched** rather than argued: `ui/e2e/desk.spec.ts` presses Encoder Assign and `Zoom ▶` on a `--mock-surface` console, reads the highlight off the browser, turns the jog wheel and asserts the highlighted parameter is the one that moved |
| 2026-08-14 | S26 | **`Executor::currentCueIndex` is in the domain, on the wire, and never filled.** `prismd::core::record_executor` reads the cue index back out of the *show* before writing it, and nothing ever writes it there: what cue a playback is on lives in `prism_engine::CuePlayer` on the tick thread, and there is no channel from the tick back into the core. The daemon's own test has said `cue_index: None` since S17 and nobody had noticed what that meant for a screen | The executor bar shows a dash rather than a number it made up, and the *absence* is asserted: `the_recording_is_of_a_desk_being_used` demands that every recorded strip's `currentCueIndex` is `null`, with a message telling whoever fixes it to regenerate the recording and give the bar a cue number. A test that goes red when a gap is closed is the only kind of note that cannot be forgotten. Building the channel is engine work — a readback path that allocates nothing inside the tick (§3.1) — and belongs with the executor-button command above |
| 2026-08-14 | S26 | **A console's command line is a parser *in the client*, and that is not a breach of D3.** `1 thru 3 at 50` is not a command; it is a `SelectFixtures` and a `SetAttribute`. Something has to turn one into the other, and it cannot be the daemon: `CommandLineInput` carries the **text** because `ARCHITECTURE_SPEC.md` §4.1 puts the console line in the session for every client and the scribble strips to show. The risk is the obvious one — a parser tested against itself passes with every rule wrong | The parser decides what was *asked for* and never what the desk *is*: it does not consult the patch, so `9` on a show with no fixture 9 parses perfectly and is refused by the daemon — which is in the recording as a step of its own. And it is held to a daemon: every typed line in `ui/tests/fixtures/desk-recording.json` sits beside the commands a real `prismd` accepted for it, grouped by a recorded line **number** rather than by equal text (because `clear` is pressed three times in a row and those are three lines). The exit criterion — *it never throws* — is ten thousand generated strings through it, not the six a person would think of |
| 2026-08-14 | S26 | **The command line mirrors what is typed into the session as it is typed, which changes what Enter means.** S23's demonstration sent `CommandLineInput` on Enter and displayed the daemon's line underneath. But §4.1 calls `commandLine` *the contents of the console line*, and the X-Touch's display shows it: a line that only appeared when it was executed would be a line nobody could read over your shoulder | Typing mirrors (paced, at most one command every 33 ms, first keystroke immediately), and **Enter executes** — sending the parsed commands and then `CommandLineInput { text: "" }`, which is what `prism_core::session` documents as *clearing the line*. `ui/e2e/reconnect.spec.ts` was updated with it: S23's assertion pressed Enter and read the line back, and the same claim is now made by typing, which is the stronger form because it does not need the line to survive being executed |
| 2026-08-14 | S25 | **The protocol had no way to move a window, and `ARCHITECTURE_SPEC.md` §4.1 says a window's position is session state.** §4.4's eleven commands are what the *console* issues, and an X-Touch opens and closes windows without ever dragging one. A canvas does. The two ways out were both bad: keep the geometry in the client, which fails all three of this session's exit criteria and is exactly the drift **D11** exists to stop; or invent a delta, which would be a client writing to a document the daemon owns | **`Command::PlaceWindow { instanceId, x, y, w, h }`** — the twelfth session command, in `prism-domain`, routed in `prism_core::SessionState::apply` to the `place_window` S12 had already written and left a note on. `docs/IPC_PROTOCOL.md` §5 carries it with the reasoning; `ARCHITECTURE_SPEC.md` §4.4 says why it is not in its list. The four coordinates carry `crate::finite`'s guard in **both** directions, so a NaN is refused at the decoder rather than written into a session that then cannot be saved. This is the second finding of S22's shape and, like that one, it was taken rather than worked around |
| 2026-08-14 | S25 | **`WindowType` had no name for the thing S24 built.** The level view — the DMX output, channel by channel — is not a `FixtureSheet`: one shows what the *fixtures* are set to and the other shows what is on the *cable*, and the whole point of the second is that it can disagree with the first. §7's *carried out of S24* asked for the panel to move into a window, and there was no window to move it into | **`WindowType::DmxSheet`**, and `ARCHITECTURE_SPEC.md` §6's list is eleven. The telemetry panel is now a window body sized by its window rather than by `34vh`, and the frame-budget measurement goes through the window system as a result — the end-to-end spec opens the window with an `OpenWindow` before it measures anything |
| 2026-08-14 | S25 | **A drag is smooth or it is honest, and the way to have both is *cadence* rather than ownership.** Holding the window's position in component state during a drag and reconciling afterwards is optimistic application under another name — **D3** — and it breaks the moment the daemon refuses a command or a second client moves the same window. Sending a command per pointer event is the other extreme: a pointer reports at 120 Hz on the screens this runs on, and S21 established at the other end of the desk that an unpaced stream of small messages is how a device is saturated | Ownership never moves — `Canvas` renders `openWindows(session)` and holds no layout at all. What is local is the *rate* (`PlaceWindow` at most every 33 ms, plus one when the button comes up) and *what the screen shows in between* (the rectangle the pointer describes, which is §4.2's own "drag state"). **The local rectangle is dropped the instant the button comes up**, so a drag against a daemon that never answers leaves the window where it started — asserted, and it is the test an optimistic implementation fails |
| 2026-08-14 | S25 | **`@msgpack/msgpack` and `rmp-serde` encode the same number differently, and S23's byte-for-byte encoder check found it the first time a command carried a float.** `rmp-serde` writes an `f64` as a float64 whatever its value; JavaScript has one kind of number, so a canvas sending the whole coordinate 240 produces a uint8. Both are valid MessagePack for the same value — but *the daemon accepting both* is a claim about `prism-ipc`, and a browser cannot check it | `crates/prismd/tests/ui_session.rs` records **both** encodings of every `PlaceWindow` — `rmp-serde`'s own and an integer form built by hand — and `the_integer_form_of_a_command_is_the_same_command` asserts against `prism_ipc::decode` that they are the same command. The interface then compares its bytes with the integer form. The check stays byte for byte and both sides still come from Rust; what is new is that the equivalence is verified rather than assumed |
| 2026-08-14 | S25 | **D11 could be asserted from either end and observed from neither, because nothing outside the daemon's own process could press a button.** `MockSurfacePort` is a Rust type: a test in `crates/prismd` can press `Channel ▶`, and that is how S22's gate works. A browser cannot. So the strongest available claim was *this delta, which a console would produce, moves the interface* — which is a reconstruction, not an observation | **`--mock-surface <PATH>`**: `prismd::surface::FileSurfacePort` reads MIDI bytes appended to a file and drops its feedback, because a file has no motor faders. It is the console's `--mock-output` and it is production code with a flag on it for the same reason. `ui/e2e/session.spec.ts` now watches a real Chromium follow three bytes written by neither the browser nor the daemon, and `surface_gate.rs` watches a connected client be *sent* the delta. D11 is observed at both ends |
| 2026-08-14 | S25 | **`OpenWindow` carries no geometry, so every window opens on top of the last one.** The daemon has no screen to centre a window on and puts each one at 0, 0 — which is right, but means opening three windows produces three identical rectangles in a pile. The obvious fix, having the client offset a newly opened window, is a client writing session state on its own initiative and a race between two clients doing it | Left as it is, deliberately, and written down here instead. The operator drags the window, which is how a good many consoles behave. If it becomes a nuisance the fix belongs in the protocol — geometry on `OpenWindow`, or a *cascade* the daemon performs — and not in a client. It has one visible consequence today: `ui/e2e/session.spec.ts` drags its window **before** opening the second one, or the pointer would land on the window in front |
| 2026-08-14 | S24 | **The decode belongs on the *paint* side of the sink, and putting it there is what makes the third exit criterion structural.** The obvious place to decode a telemetry frame is where it arrives — in the connection's `onTelemetry`, beside every other message. Two things are wrong with that. It is work done for pictures nobody sees: telemetry arrives at 30 Hz, a background tab paints at 0 Hz, and the browser decides which. And it puts the decoder on the *control* side of the sink, where a malformed frame is one mistake away from the store | `TelemetrySink.accept` stays what S23 built — it takes bytes and holds the latest — and the decode happens in the animation-frame loop, whose only outputs are a canvas and one `textContent`. *Dropped telemetry never desynchronises control state* stops being a promise about care and becomes a fact about which side of a boundary the code is on. It is still asserted: 180 malformed frames through a real connection, then a delta that has to arrive |
| 2026-08-14 | S24 | **There is no telemetry encoder in the interface, and that is a decision rather than an omission.** The natural way to test a decoder is to build frames to feed it — and a TypeScript encoder feeding a TypeScript decoder passes with the header misread, the endianness reversed and the stride wrong, as long as all three are wrong together. It is the trap S19 (round trips), S20 (recorded round trips), S21 (self-classifying messages) and S23 (self-computed patches) each found in their own layer | The bytes come from `prism_ipc::TelemetryFrame::encode` off two running daemons, and **every expectation is `TelemetryFrame::decode`'s own answer**, written into `ui/tests/fixtures/telemetry-recording.json` beside the payload. `ui/src/telemetry` contains no encoder at all, which is also correct on its own terms: §4 makes `Telemetry` daemon → client, so a client that could build one would be a client that could lie about the rig |
| 2026-08-14 | S24 | **Measuring an 8 ms frame budget needs 64 real universes, and the daemon will not publish universes a show does not patch.** `prismd` filters telemetry down to `patched_universes()` (S17, and for a good reason: 64 universes of nothing at 30 Hz is a megabyte a second on a school network). So a daemon started with `--universes 64` and an empty show publishes **none**, and the protocol has no command that embeds a fixture type — a client cannot patch its way to a wide rig | A 64-universe show is a committed fixture, `ui/tests/fixtures/wide-rig.prism`, written by the same `#[ignore]`d regenerator as the frame recording and opened by a non-ignored Rust test on every commit. `.gitignore`'s `*.prism` gained a second exception, beside S15's frozen migration file. The end-to-end spec copies it before starting a daemon on it, because a daemon writes to the show it opens |
| 2026-08-14 | S24 | **The canvas is a device, so it is an argument — and `jsdom` is why that is not merely tidy.** `getContext("2d")` answers `null` in `jsdom`, so a test that painted on a real canvas would skip itself, and the raster arithmetic — the part where a stride error puts universe 64's levels a channel to the left — would be checked by nobody | `LevelSurface` is five operations with no canvas types in them: `clear`, `fill`, `label`, `blit`, and the two sizes. `canvasSurface` is the browser's, `RecordingSurface` keeps the pixels, and `painter.test.ts` asserts **every one of 32 768 pixels** against the frame it was drawn from. What is left — the code that turns those five operations into canvas calls — is covered by `canvas.test.ts` against a context it constructs, which is the one type assertion this session's tests make |
| 2026-08-14 | S24 | **The picture is a raster, not a tree of elements, and the difference is 32 768 canvas calls.** One `fillRect` per channel is the obvious rendering and it is 32 768 calls into the canvas API per frame, thirty times a second | One pixel per channel through a 256-entry palette into an `ImageData` 512 wide, blitted into the grid rectangle scaled with smoothing off: one `putImageData` and one `drawImage` for the whole picture. The `ImageData` is kept between frames — `createImageData` is 131 kB at 64 universes, which is four megabytes a second of rubbish for the collector to deal with in the middle of a show — and the palette's byte order is derived at run time rather than assumed. Measured: **0.30 ms** median for decode *and* paint |
| 2026-08-14 | S24 | **A dropped frame and a stale picture want different answers, and "clear the canvas" is wrong for both.** A frame that cannot be read is one missing picture; the last good one is still the best answer until the next arrives. A channel that has stopped arriving is different — what is on the canvas may be minutes old, and an operator reading it as current is the failure this whole panel exists to avoid | Three states, and each says so. A dropped frame leaves the picture standing and counts the fault by kind (logged **once** per kind, not thirty times a second). A picture older than 600 ms is **dimmed where it stands** and the readout begins *not live* — *these were the levels a second ago* is worth more than an empty rectangle, as long as it cannot be mistaken for now. A connection that has gone clears the sink, and the picture goes with the readouts, which is S23's rule about stale values applied to pixels |
| 2026-08-14 | S24 | **The telemetry channel reaches the tree as a *device*, not as state — and that is what makes the rule impossible to break rather than merely written down.** A context carrying a decoded frame, or a hook answering with one, would put 32 768 numbers into React's dependency graph without anybody deciding to | `TelemetryContext` carries a sink, a scheduler and a way of getting a surface. Nothing in it changes, ever, so nothing a component reads from it can cause a render; there is no `useTelemetryFrame` because there is nothing to subscribe to. `deskEvents` still has no `onTelemetry`, which is S23's guard, and the render counter is the measurement that says the two together work |
| 2026-08-14 | S24 | **The sequence number is a `u64` and JavaScript's `number` is exact to 2^53.** Nothing here needs the range — thirty frames a second reaches 2^53 in nine million years — and narrowing it would never be noticed | Read with `getBigUint64` and kept as a `bigint`; the gap arithmetic is done in `bigint` and narrowed once, in the one place it could overflow if it were not. A decoder that quietly narrows a wire field is the kind of thing that is right until it is not, and this one costs nothing |
| 2026-08-13 | S23 | **Neither Zustand nor Immer is in the interface, and the plan named both.** Immer exists to make a deep update read like a mutation — but the show and the session are patched by **RFC 6902 operations against a document root**, so the update is `applyOps`, which already answers with a new document that shares every untouched container by reference. A draft proxy in front of that is a second immutability mechanism over data that is already immutable, and it would have to reconcile with a patch applier that does not use it. Zustand is a store with selector subscriptions, which React 19 has as `useSyncExternalStore` | `store/desk.ts` is about a hundred lines and `store/hooks.ts` is forty, both testable with no renderer. The measurement that decided it: a `replace` on one fixture's name leaves every *other* fixture object identical by reference, which is what a selector compares — asserted in `mirror/patch.test.ts`. The other half of the argument is that the interface ships inside a Tauri bundle (S29), so a dependency here is one an operator installs |
| 2026-08-13 | S23 | **`@msgpack/msgpack` was taken, and a hand-written codec was not.** The daemon uses `rmp-serde`; the browser needs the other half. Writing it by hand would put a second implementation of a binary format in the project, in the language where a decoding mistake is silent, and there is nothing project-specific about MessagePack | One runtime dependency, no dependencies of its own, and it decodes into `unknown` — which is exactly the shape the readers want, because the checking that matters is *is this a message* and not *is this MessagePack*. Its decoder walks with an explicit stack rather than recursing, so the attack `prism-ipc`'s `scan.rs` guards against cannot overflow the JavaScript stack; the depth limit in `shape.ts` is about what happens afterwards, when the value is walked as a document. **Checked rather than assumed:** the recording carries thirteen client messages encoded by `rmp-serde` and the browser reproduces all thirteen byte for byte |
| 2026-08-13 | S23 | **A TypeScript string union is erased at run time, and a decoder needs the values.** `ts-rs` renders a unit-only enum as `"Dimmer" \| "Position" \| …`, which is right for a type and useless to code deciding whether the string it just read off a socket *is* one. Writing the lists by hand in `ui` would have been a second enumeration to keep in step — the drift the generated bindings exist to prevent | `prism_domain::export::write_variants` emits `ui/src/bindings/variants.ts`, and it derives each list **from the generated union itself** rather than from a second enumeration in Rust: it reads back the `.ts` file `ts-rs` has just written. Fourteen tables come out of it for free, including the ones no session has needed yet, and `MergeMode` proves the method — its variants travel as `HTP`/`LTP` because of a `rename_all`, and a table written from the Rust identifiers would have type-checked and never matched a message |
| 2026-08-13 | S23 | **The envelope of `prism-ipc` is hand-written TypeScript, and that needs a guard.** `ui/src/bindings/` is `prism-domain`'s, which owns the vocabulary; `Hello`, `Snapshot`, `ServerMessage` and `RejectReason` belong to `prism-ipc`, which exports no TypeScript. Adding `ts-rs` there would mean two crates writing one directory — and the first thing `export_bindings` does is delete the `.ts` files it finds | The envelope is transcribed in `ui/src/ipc/protocol.ts` and `crates/prism-ipc/tests/interface_protocol.rs` `include_str!`s it back: the protocol version, every `RejectReason` **and the count**, every `ClientKind`, every message tag in both directions, and the exact text of `closesTheConnection` against `RejectReason::closes_the_connection`. Compiled in, so a file that moved fails the build rather than a test. The same shape as S22's check that the shipped profile *is* the built-in table |
| 2026-08-13 | S23 | **Losing the daemon drops the three documents, rather than greying them out.** The alternative reading of §8 — keep the last state and mark it stale — is the one an interface usually takes, and it is wrong here: a fader reading 63 % after the engine that knew it has stopped is a number somebody may run a show off | `DeskStore.disconnected()` sets `documents`, `outputs` and `health` to `null`, and the telemetry sink is cleared with them. The notices stay, because a message about what went wrong is the one thing still true. It also turns the exit criterion into something checkable rather than promised: the assertion is that the old text is **nowhere in the document**, in jsdom and in a real browser |
| 2026-08-13 | S23 | **A delta that does not fit the mirror is a divergence, not an error to log and continue from.** `prism_core::mirror` already says why — clients apply deltas without validating them, so an operation that does not fit means the two have *already* disagreed | `DeskStore.applyDelta` answers `false` and leaves the state untouched; `deskEvents` turns that into `Connection.resync`, which drops the socket and reconnects through the ordinary backoff. Reconnecting is not a heavy remedy: it is the same path a daemon restart takes, and the snapshot at the end of it is the only honest way back |
| 2026-08-13 | S23 | **Commands are not queued while there is no daemon.** The tempting behaviour is to hold them and send them on reconnect | `Connection.send` answers `null` and logs a warning. A Go or a fader movement that arrived four seconds late is an instruction the operator has already given up on, and §8 says a client that reconnects **re-snapshots** rather than replaying what it meant to say. The command line demonstrates the same rule from the other side: what is typed is local input and the readout underneath is the daemon's, so nothing has to be rolled back when a command is refused |
| 2026-08-13 | S23 | **`vite preview` binds `localhost`, and on this machine that resolves to `::1` first.** The first Playwright run timed out waiting three minutes for `http://127.0.0.1:4173`, with a server that was answering perfectly on the same port over IPv6 | `--host 127.0.0.1` in the `webServer` command, and the reason written next to it. Worth recording because the same trap is waiting for anything else in this project that binds a loopback listener by name — the daemon's own `--websocket` takes an address rather than a host name, which is why it has never met it |
| 2026-08-13 | S23 | **A pointer that names nothing is an ordinary answer for a *view* and a fault for a *delta*.** The same `MirrorFault` would otherwise mean two different things: a window open on a fixture that has just been deleted, and a client that has drifted from the daemon | `mirror/select.ts` answers `null` for a path that is not there and lets a **malformed** pointer through — that one is a typo in a view rather than a statement about the document, and hiding it behind `null` would leave a panel permanently empty with nothing to explain it |
| 2026-08-13 | S22 | **Three rows of `docs/MCU_MAPPING.md` §4.1 name behaviour the command vocabulary cannot express, and they are one gap rather than three.** The main fader's `XFade`, Play's `On`, and the strip buttons' configurable list (`LearnSpeed`, `Off`, `On`, `Flash`, `Toggle`) are all `ExecutorFaderFunction` and `ExecutorButtonFunction` values — **show data on the executor** (`ARCHITECTURE_SPEC.md` §6) — and `docs/IPC_PROTOCOL.md` §5 has no command that presses an executor's button and lets the executor decide what that means. There is one fader command for all four fader functions, and `ExecutorGo`/`ExecutorOff` for two of the eight button functions | The bindings resolve to the commands that **exist** — the main fader moves the selected executor's master, Play and Forward are both `ExecutorGo`/`Next` — and the gap is written down in three places rather than papered over: `docs/MCU_MAPPING.md` §4.2.1, the shipped profile's `deviationsFromSection41` block, and here. The alternative was to invent an `ExecutorButton { executor, index }` command, which would have touched `prism-domain`, `prism-core`, the protocol document and the generated TypeScript — a protocol change made in passing by the session that noticed it, which is exactly what S20's rule about tap-for-speed says not to do. **The session that adds it closes all three rows at once**, and it is the same session that owes a tap against a speed master |
| 2026-08-13 | S22 | **The exit criterion "never blocks startup" is better as a type than as a test.** A loader that returns `Result` can be called correctly by every caller written so far and wrongly by the next one, and the criterion is about *every* future startup path | `Bindings::load(text, profile) -> (Bindings, Option<ProfileError>)` has **no error path at all**: the table it answers with is the profile's when it parses and the built-in defaults when it does not, and the error is a warning to log rather than a decision to take. The mutation check is what makes this more than a shape: falling back to an *empty* table instead of the defaults leaves the type identical, every unit test green, and turns the end-to-end criterion red in `prismd` — a daemon that started fine and had a dead console |
| 2026-08-13 | S22 | **A binding profile is refused whole, not row by row.** The tempting behaviour for a user-editable file is to keep the rows that parsed and warn about the rest. A table half of which was understood is a desk that does *some* of what its author intended, and an operator cannot tell which half by looking at it | Any problem — bad JSON, an unknown control, a control bound twice, a misspelled key inside a row, a profile for another device, a binding on the reserved button — falls back to the whole default table with one message naming the cause. The document itself may carry prose (the verification record and the deployment notes live in the same file), because refusing unknown *top-level* keys would push that documentation into a second file nobody would open; a **binding** may not, because there a typo is an argument that silently did not arrive |
| 2026-08-13 | S22 | **`prism-surface` cannot hold a session, so the session's answers come to it as data.** Half of `docs/IPC_PROTOCOL.md` §5's commands carry an argument the surface has not got: an executor number where a strip only knows it is the fourth strip, a view number where `Channel ▶` only means *the next one*. Handing the crate a `SessionState` would have made layer 3 depend on `prism-core` and put the view library on the wrong side of a boundary §10.1 draws | `SurfaceContext` — executor page, selected executor, the two **neighbouring** views, the programmer page, and the attribute under the jog wheel — plain `Copy` answers resolved by `prismd::surface::context_of` once per batch of events. The neighbours rather than the library is the load-bearing part: it is what keeps a relative `SelectView` (D8) out of a crate that has no views, and it costs the daemon a walk of a `BTreeMap` it already holds |
| 2026-08-13 | S22 | **No MIDI backend was written, and the D11 gate did not need one.** `prismd` may hold no `#[cfg(target_os = …)]` either (§10.1), so a real port means a dependency — `midir`, or something behind it — and a decision about which crate owns it | `SurfacePort` is the trait and `MockSurfacePort` is the only implementation. Choosing a backend is a question about a dependency and about §10.1's list, and answering it was not needed to pass a gate that is about a *daemon*, not about a device. The open question the shipped profile already records — whether a real unplug and replug produces exactly one disconnection from that backend — is still the backend's, and it is still open |
| 2026-08-13 | S21 | **"Exactly one resynchronisation" is a claim about the shadow model, not about the value.** §5.1 says a released fader is resynchronised after 150 ms. The obvious implementation — suppress while touched, then let the ordinary diff run — sends a message only if the *value* changed meanwhile, and says nothing at all about a fader the show did not touch. But the operator moved the motor, so the desk is somewhere this layer never put it, and there is nothing in the picture to notice | A touch **invalidates** what the shadow model believes about that fader, so the resync is unconditional and the criterion becomes a guarantee. The mutation check is the evidence and it is unusually sharp: removing the invalidation leaves `releasing_a_fader_resynchronises_it_exactly_once` **green**, because that test happens to move the value, and turns exactly one test red — `a_fader_nobody_moved_is_still_resynchronised_after_a_release`. Without that second test the session would have claimed the criterion and not met it |
| 2026-08-13 | S21 | **An inbound fader *move* must not invalidate the shadow, only a *touch* must.** §5.1's own account of the oscillation says the motor's movement generates an inbound value — so a layer that forgot its belief on every inbound position would resend after its **own echo**, once per frame, for ever: the fight §5.1 exists to prevent, arriving through the door marked *being careful* | Only touch invalidates. A move without a touch is either that echo or a fader moved with a pen, and the second case costs a stale value until the next change rather than a permanent fight. Written into `docs/MCU_MAPPING.md` §5.4 as a deliberate non-rule, because it is the kind of line a later reader deletes for looking like an omission |
| 2026-08-13 | S21 | **The two acceleration curves are two mechanisms, not one mechanism with two tables.** S20 measured that a V-Pot reports 1…8 detents per message and the jog wheel reports ±1 however hard it is spun. The V-Pot has therefore *already measured the speed*; the wheel has not and cannot — it raises its **rate** | `VPotAcceleration` reads the reported magnitude; `JogAcceleration` reads the **interval since the previous message**, which is why S21 owning the clock matters beyond the SysEx timeout. A shared curve would make the wheel a control that cannot be hurried, which is exactly what an operator reaches for it to do. Both are data (`SurfaceController::set_curves`) because *how much a detent is worth* is taste, where the 1…8 is measurement |
| 2026-08-13 | S21 | **Silence needs two states before it needs a message.** §2.7 asks the surface layer to notice a desk that has gone quiet. Implemented as a single timeout it is a false alarm generator: an X-Touch speaks only when it is touched, so ten seconds of silence during a show is ordinary | Four states, and the useful one is the pair: `Connected` (attached, never heard from — say nothing) against `Live` (has spoken). Only a desk that *was* talking is suspected, which is §5.3's own wording read literally. It is then asked **once**, with the device query, and only when the send queue is empty — the failure of §2.7 needs replies in flight, so one question with nothing else outstanding is the opposite of polling. `SurfaceHealth::Unresponsive::remedy()` returns the words themselves, and a test asserts they contain *power-cycle* and *will not*, because the whole value of noticing is in saying the one thing that works |
| 2026-08-13 | S21 | **A test that classifies messages with the code under test passes with the answer reversed.** S19 found this for round trips and S20 for real captures. The priority criterion has the same shape: `Priority::of` is layer 2's own opinion about which class a message belongs to, and a test that grouped the output with it would agree with any ordering the diff produced | `tests/feedback_rules.rs` classifies every message by a `match` on its **status byte**, transcribed from `docs/MCU_MAPPING.md` §2.2 by hand — pitch bend is a fader, CC 48–55 is a ring, CC 64–75 is the display, channel pressure is a meter. The same rule as the note tables of S19 and the eleven hand-read captures of S20, now stated once more for a rule rather than for a byte format |
| 2026-08-13 | S21 | **`prism-domain` is used at last, for one type.** The manifest has carried it unused since S19, and the crate documentation said so | `RgbColor` on the way into the colour quantiser, which is the only domain type the surface model needs: everything else it holds is topology. S22 will need many more, and the seam is unchanged |
| 2026-08-13 | S20 | **"Tap for speed" on a speed master is a wanted binding and has no target to bind to.** The operator's use for the always-available transport section is free assignment — a tap tempo against *different speed masters*, a macro, a look. Today `LearnSpeed` exists only as an **executor button** function (`ExecutorButtonFunction` in `ARCHITECTURE_SPEC.md` §6; `XTouch.txt` offers it on a strip's buttons and not even on the selected executor's), and **speed masters** are named in `docs/DMX_MERGE.md` §4 item 3 — playback rate, applied in step 2 of the tick — with nothing implementing them and no domain type carrying one | A **forward dependency**, recorded so it is not rediscovered from the operator a second time. S22 must not invent the command: binding a tap needs a speed master to tap. The session that builds speed masters owes (a) the domain type, (b) a command to tap one, and (c) an entry in the transport row's function list — `docs/MCU_MAPPING.md` §4.1 and §4.3. Until then the transport row keeps its executor defaults, which are usable and are explicitly not a layout |
| 2026-08-13 | S20 | **The X-Touch is to be run in the combined Xctl+MC mode, driving the venue's sound console and PrismDMX at the same time — and then most of the panel is not ours.** Stated by the operator after the session's measurements, and marked as such: everything in §2.7 was read off the desk, this was not. In that mode the surface splits the panel between the two hosts, and only what Xctl leaves unused keeps driving MC output and listening to MC input — **in practice the transport section (notes 91–95) and the jog wheel (CC 60)** | `docs/MCU_MAPPING.md` §4.3. **And it is a convenience rather than a restriction** — the first draft of that section got this backwards and was corrected the same day: the operator can switch the whole surface to MC at any time with the SMPTE button, so nothing is unreachable and the profile may contain whatever it likes. What the permanent set buys is **no switching**, which is what matters *during a show*. So: **the transport section is the always-hot part of the console and should be spent on live-show work, free assignments included** — a tap for speed against a particular speed master, a macro, a look — rather than on a transport metaphor PrismDMX does not have; §4.1's defaults are a starting point, not a layout. **D7 and D8 cost a mode change**, which is fine for paging and view switching (setup-shaped, not cue-shaped) and is a reason not to put anything time-critical there. **Feedback must not assume it owns a control**: S21 should hold the ownership set as data on the profile, the way `unlit_buttons` is, rather than as a condition scattered through the diffing. Nothing in §2 changes — the MC half of the combined mode is the same MC — so this costs no measurement, only assumptions |
| 2026-08-13 | S20 | **SMPTE/Beats (note 53) is reserved and must never be bound.** In the combined Xctl+MC mode it is the button that **switches the surface between the two hosts** — the operator's way back to the sound desk. A console that binds it is a console somebody has to power-cycle to escape | Left unbound **in every mode**, not only the shared one, so that one profile is safe on a desk whose mode nobody has checked; `profiles/surface/xtouch.json` carries it in a `reserved` block with `bindable: false`, and S22's loader should refuse a profile that binds it rather than merely defaulting away from it. A quiet corroboration, offered as consistency rather than proof: note 53 is one of the two buttons S20 measured to have **no LED at all**, which is what one would expect of a button the firmware keeps for itself — though Name/Value has no LED either and switches nothing |
| 2026-08-13 | S20 | **The X-Touch can stop transmitting while it goes on receiving, and only a power cycle brings it back.** Flooding it with 63-byte scribble strip writes *while device-query replies are outstanding* loses 17–34 of 64 replies and then kills its MIDI transmitter outright: no buttons, no faders, no SysEx. It keeps **receiving perfectly** — text written in that state appears on the display. Closing and reopening the port does not help; a fresh process does not help. It happened **twice out of two attempts**, and each half of the recipe is harmless alone: 64 small messages back to back lose nothing, 64 large ones with nothing asked of the desk are fine, and even large-plus-query is fine if each reply is waited for. A 2000-message flood while an operator swept a fader did not hang it but **thinned the inbound stream from ~40 to ~11 messages a second** | Three things for S21, all in `docs/MCU_MAPPING.md` §2.7 and §5.3. **Pace the outbound path** — §5.2's 30 Hz coalescing against a shadow model is not an optimisation, it is what keeps the desk out of this state, and a minimum gap belongs in the send queue. **Never poll the handshake**: the device query is the only reply the surface generates, and the failure needs replies in flight. And **silence is a fault state that §5.3 did not cover** — it assumed disappearance, where the port stays open and writes still land. A surface layer that only watches for disconnection would show a green light beside a dead console, so it must notice a desk that has gone quiet and say *power-cycle it*, because reconnecting is the one thing that will not work |
| 2026-08-13 | S20 | **The X-Touch's faders are 12-bit inside a 14-bit field.** Every one of 576 captured positions is a multiple of 4; the low byte only ever takes the 32 values `00,04,…,7C`; the top of travel is **16380** and 16383 is never sent. No source mentions it. The motor accepts all 16 384 values — it simply cannot report that finely | `McuProfile::fader_step` and `max_reported_position()`, with the reasoning in the field's own documentation: a layer that turns an inbound position into a percentage by dividing by 16383 gives **99.98 %** for a fader against its end stop, and an executor master that cannot reach full is wrong in a way an operator will find and nobody will be able to explain. S21 scales against the profile, not against `FADER_MAX` |
| 2026-08-13 | S20 | **The V-Pots accelerate and the jog wheel does not, on the same encoding.** §2.1 described both as "relative, sign-magnitude" and left the magnitude open, and it is open in two different ways: a V-Pot spun hard carries **1…8 detents** in one message, while the jog wheel sent **±1 and nothing else in 404 messages** however hard it was spun — it raises its rate, not its magnitude | S19's decision to pass the magnitude through uninterpreted was right for a reason it could not have known: it is the only behaviour correct for *both* controls. **Layer 2 needs two curves**, not one — an acceleration curve on the V-Pots and none on the jog wheel — and a single shared curve would make one of them wrong. Recorded in `control.rs`'s module documentation beside the encoding itself |
| 2026-08-13 | S20 | **A round trip over recorded device bytes is still a function composed with its own inverse.** The new capture target asserts that every message the desk sent re-encodes to exactly those bytes — and the S19 mutation (swapping the two halves of the 14-bit fader split *in both directions*) leaves it **green**, on genuine recordings, because decode and encode still agree with each other. What turned red was the *quantisation* test: with the halves swapped the positions are no longer multiples of 4 | S19's finding generalises further than S19 could state it: **real input does not make a round trip self-validating.** So `tests/hardware_capture.rs` carries a second table of eleven captured messages whose meaning is **worked out by hand** (`E0 7C 7F` is 124 + 128 × 127 = 16380), plus the measured properties — multiples of 4, maximum 16380, magnitudes 1…8, jog ±1. The rule for any future device: a capture is evidence, and evidence still needs one end written down independently |
| 2026-08-13 | S20 | **Printing a press order makes the person the thing under test.** The first attempt at verifying the 64-button panel printed the expected order and compared the capture against it. It reported **71 mismatches**, every one spurious: the walk had been done strip by strip where the list ran row by row. Nothing was wrong with the desk or the table | Replaced by a protocol with no order in it: **the host lights one LED and the operator presses whichever button is lit.** That verified 60 of 64 buttons with zero ambiguity and, because the LED and the note are checked in the same act, it verifies the **outbound** map at the same time — which the press-order method could not do at all. This is the method for the next surface, and `tools/xtouch-probe pair` is it |
| 2026-08-13 | S20 | **Two panel buttons have no LED, and the encoders have no lamp.** Name/Value (52) and SMPTE/Beats (53) are printed on the panel, send their notes, and stay dark at every velocity; the ring value's bit 6 — the MCU's "small LED beneath the encoder" — has nothing to light on this surface. The Ardour manual's "a direct emulation … with no deviations" is not quite true | `McuProfile::unlit_buttons` holds the two as data, because a console that lights a lamp which does not exist is one whose feedback silently lies about part of itself: S21's shadow model can skip them and S26 can decline to offer them as indicators. The encoder lamp is documented rather than modelled — setting a bit that lights nothing costs nothing, where a missing LED that the UI believes in costs a support call. `profiles/surface/xtouch.json` leaves both buttons unbound on purpose |
| 2026-08-13 | S20 | **A 7-segment value of 0 blanks the digit, so `'@'` cannot be displayed at all.** §2.2 carried two claims that could not both be true — the character set is "ASCII with bit 6 stripped", under which 0 is `'@'`, and "0 is a space". The desk blanks both 0 and 32 | The rule stops one character short, and the codec now says so: `SegmentChar::from_ascii` **refuses `'@'`** rather than returning a code that draws nothing, and `to_ascii(0)` answers a space. This is the session's only change to shipping behaviour, and the reasoning is the one that matters for a console: a name written through this type either appears or is refused, and never comes back with a hole in it that only the desk can see |
| 2026-08-13 | S20 | **The measurement tool belongs outside the workspace, and the evidence belongs inside it.** Verifying the codec needed a MIDI port; `ARCHITECTURE_SPEC.md` §10.1 allows `prism-surface` no platform code, and adding `midir` as a dev-dependency would have put an ALSA C build in front of the Linux job and the ARM64 cross-check | `tools/xtouch-probe` is its own crate with its own `[workspace]` table, so `cargo test --workspace`, `cargo clippy --workspace --all-targets` and the ARM64 check never see it — **`prism-surface` gained no dependency at all.** It depends on `prism-surface` by path, so every byte it sent was `Feedback::encode_into`'s and every byte it read was `McuCodec`'s: what was verified is the shipping codec, not a transcription. What comes *back* into the workspace is recorded captures replayed by an ordinary test. **Platform code and hardware in a tool, evidence in a fixture** — the pattern for the next device, written into §10.1 |
| 2026-08-13 | S20 | **`profiles/surface/xtouch.json` is layer 3 only, and deliberately holds no note map.** `docs/MCU_MAPPING.md` §2.1 used to say the JSON held the note and CC numbers "as data", while §4 said it was the binding table. Both cannot be true without two copies of a 104-row table | The JSON is the **binding table** plus the verification record (firmware, serial, mode, and the measured device facts a loader may want). The note map stays in `profile::X_TOUCH`, one constant — which is the whole reason this session was a data edit rather than a refactor, and a second copy would have thrown that away. §2.1 now says so explicitly, and the file says it about itself. Three open questions for S22 are written into the file where the loader will find them, the sharpest being that `Command::SelectView` takes an absolute `view_id` where `Channel ◀▶` needs a relative move |
| 2026-08-13 | S20 | **The handshake is answered, and there is an undocumented firmware request beside it.** The device query returns `F0 00 00 66 14 01` plus eleven printable ASCII bytes — a seven-character serial (`0156406`) and a four-character challenge (`5BC5`), not the `0x06` some sources expected. And `F0 00 00 66 14 13 F7` is answered with `…14 "V1.25"`, which no source mentions at all | Both are written into `docs/MCU_MAPPING.md` §2.3, which closes the last of S19's three open questions. The codec still **counts inbound SysEx rather than decoding it** — not for want of data now, but because nothing above layer 1 has asked for a serial number and an API invented for no caller is a guess of a different kind. The firmware request is the useful one: it makes "does this desk have the colour extension?" a run-time question rather than a request that an operator power-cycle the desk and read the boot screen |
| 2026-08-13 | S19 | **A round-trip test that computes its expected bytes from the table it is testing is a function composed with its own inverse, and a mutation check proved it.** The obvious way to write `bytes → event → bytes` is to build the bytes from `X_TOUCH` and compare. Swapping the two halves of the 14-bit fader split **in both directions** — decode reads MSB first *and* encode writes MSB first — leaves every such test green, and leaves both property tests green, because the pair still agrees with itself. On a real desk the fader is in the wrong place | `tests/round_trip.rs` holds **literal byte arrays and literal note numbers**, transcribed from `docs/MCU_MAPPING.md` §2 by hand. The symmetric mutation turns four of them red. This is S16's `to_vec` / `to_vec_named` finding in a second setting and it generalises: **a round trip is only a test of a codec if one end of it is written down independently.** The consequence for S20 is deliberate — correcting a number is an edit in two places, the profile and this table, and if they disagree the build says so |
| 2026-08-13 | S19 | **The codec is handed the time rather than holding a clock, and that is more testable rather than less.** The session plan said the SysEx timeout should sit behind a `Clock` the way `prism-protocols::OutputRunner`'s backoff does, tested against `ManualClock`. It does not. `MidiDecoder::push` takes `now: Duration` | Three reasons, and the third settles it. The arrival time of a packet is a **property of the packet**, so a decoder that reads a clock is guessing at something its caller knows exactly. A test then needs no simulated clock at all, only arithmetic — the timeout is asserted from both sides of its deadline in microseconds. And `Clock` carries `sleep_until`, which a codec must never call: holding one would mean holding a method whose use would be a defect. It also keeps `prism-surface`'s dependency list at one crate, where a `Clock` would have pulled in `prism-engine`. **The rule for later layers:** S21's surface thread owns a clock and passes the instant down; nothing below it should |
| 2026-08-13 | S19 | **A `panic!` is not the only way for a codec to fail an operator; losing its place in the stream is worse, and it is invisible.** `CLAUDE.md` says an invalid MIDI packet must never propagate a failure, which reads as *do not panic*. The failure that actually reaches a desk is quieter: a parser that mishandles a Program Change's data-byte count, or drops the data bytes of a Song Position Pointer, spends the rest of the burst reporting orphans — and the Select button stops working until something resynchronises | The decoder knows **all seven** channel voice messages, not the four the MCU uses, and swallows system common data bytes; deciding a message is uninteresting happens one layer up, where it is counted as `unmapped` rather than as damage. The claim is stated as a property: *after any byte stream at all, the next Play press still arrives* — `proptest` over arbitrary packets, and the same thing asserted on every prefix of every valid message. **A second property carries the same weight:** a random stream decodes identically however it is cut into packets, whole, byte by byte, or at an arbitrary interior point. That is what a USB MIDI transport does to a parser, and it fails for any parser that keeps state on the stack of `push` |
| 2026-08-13 | S19 | **Inbound SysEx is counted, not interpreted — because the alternative is inventing data in a table made of citations.** The MCU handshake is optional (`docs/MCU_MAPPING.md` §2.3) and no source settles what an X-Touch's *Device Ready* payload contains. A decoder for it would have been a guess sitting beside 64 note numbers that all carry a source | `McuCodec` counts a complete Mackie SysEx as `sysex_ignored` and hands it to nobody. The *question* — `F0 00 00 66 14 00 F7` — is written, since that side is known, and the reassembly, the bounded buffer and the timeout are all exercised by the scribble strip messages, which are longer than anything a device sends back. Added to §7 as something to record at the desk. **The general rule this session worked to:** where a source could not settle something, the codec does the least it can defend and the question goes on §7's list — three did |
| 2026-08-13 | S19 | **Two mutations that every functional test survives, and only the allocator sees.** Reassembling a SysEx into a `Vec` instead of the fixed array, and assembling an outbound SysEx into a `Vec` before copying it into the caller's buffer, both leave 87 lib tests, 9 fuzz tests and 12 round-trip tests green — same events, same counters, same bytes | Only `tests/codec_allocations.rs` turns red, which is why it exists. It follows `prism-engine`'s and `prism-ipc`'s pattern with one addition worth carrying: **"no allocation growth" is a comparison, so the hostile window is fed twelve times what the ordinary one is**, and a guard test at the end allocates on purpose to prove the probe can still see one. A measurement that reports zero needs something that makes it report non-zero, or it is indistinguishable from a probe that stopped counting |
| 2026-08-13 | S19 | **A discard counter is only useful if it separates *the cable is broken* from *the profile is wrong*.** Both produce "nothing happened when I pressed it", and a single `discarded` number would leave an operator and a later session with no way to tell them apart | Ten counters on the wire layer — orphan data bytes, truncated messages, interrupted, overflowed and timed-out SysEx, stray `F7`, system common and real-time — plus `unmapped` and `sysex_ignored` above it, which count **well-formed** messages this profile does not describe. The distinction is exactly the one S20 needs: a surface sending notes nobody has mapped moves `unmapped` and leaves `wire.discarded()` at zero, which is a table to fix rather than a cable. The counts are asserted **exactly** against a deterministic hostile stream rather than as "more than zero", because a counter that is out by a factor is a counter nobody can reason from |
| 2026-08-12 | S18 | **"No gap" is two claims, and a mutation check is what proved one of them is not enough.** The criterion says *the output frame sequence has no gap across the whole run*, and the obvious reading is about the frames arriving. It is the wrong half on its own: a daemon that blacks the stage out when its last client disconnects — three words in `DeskHandler::disconnected` — produces a **perfect** recording, 79 frames, longest gap 24 ms, no silence anywhere, and a dark hall. The mirror-image defect is a daemon that freezes holding a good frame, which any content-only assertion passes | Both, and neither is optional. **(1) No silence:** consecutive frames for one universe are never more than **250 ms** apart — eleven output cadences, against a measured 24–51 ms — over the whole run, with the kill asserted to fall *inside* the recorded window. **(2) No unbidden darkness:** from the frame the show's look is first complete on, every later frame still carries it, the light in question being the one the *dead client* asked for. The 250 ms is chosen against the failure rather than against the jitter: an output coupled to a client stalls for as long as the client takes to die, which is not 250 ms, and a DMX receiver holds its last look for about a second. **This needed a change in `prism-protocols`:** `MockOutputHandle::frames()` had no times in it, and a list of frames without times cannot tell a driver's own cadence from a stage that stopped — so `MockOutput` records a `FrameRecord { at, universe, data }` and `timeline()` is what the gate reads |
| 2026-08-12 | S18 | **The gate passed without a line of the daemon changing, and that is the result rather than an anticlimax.** S18 was set up as a session that might find defects — S8 found one in the break timing, S15 found one in the save path, S16 found a leaked named pipe. This one found none: `prismd` was already a process in which a client's death touches nothing but a log line and a `HashMap` entry | What the session delivered is a **proof** and three affordances the proof needed: `MockOutputHandle::timeline` (`prism-protocols`), `ServerHandle::clients` (`prism-ipc` — a per-client counter that can only be read by a caller who already knows the id is a counter nothing outside the module can read), and `Daemon::server`. Worth recording because the temptation in a gate session is to add machinery so that something has been built. The architecture is what was being tested, and the four deliberate regressions are the evidence the tests would have said so |
| 2026-08-12 | S18 | **A killed client and a client that said goodbye are different tests, and the difference is what the D2 gate is about.** `Client::disconnect` flushes and shuts the socket down in order; S17's snapshot test uses it and it proves the polite case. The case D2 means is the machine being switched off | The client lives in a task that is **aborted while it is waiting**, with messages already queued for it that it will never read: the socket closes with no goodbye, mid-conversation, under backpressure. The daemon is then asserted to have *let go* — `client_count()` back to zero — which is S16's leaked named pipe stated as a test at the daemon level rather than at the transport's. The same shape is used for the reconnect criterion, so what a fresh client is compared against is a state a **dead** client had accumulated |
| 2026-08-12 | S18 | **A "fast client" is one that is being *read*, and the first version of the backpressure gate was not reading it.** CI failed on the commit *after* the green one — a commit that changed nothing but PROGRESS.md — with `the fast client dropped 28 telemetry frames and the slow one 28`. The cause is embarrassing and exact: while the test waited for the *slow* client to fall behind, it was not polling the fast one either, so both were equally behind. On this machine the two numbers happened to differ; on a two-core runner they tied | `drain_ready(&mut fast)` on every turn of that wait — a one-millisecond read of whatever is already there — and the assertion became a **ratio** rather than an inequality: the fast client must drop less than a quarter of what the slow one does. Measured after the fix, under six CPU burners: **0 against 21**. Two rules come out of it. A test about "a client that is behind" has to be explicit about *every* client's reading, because not being polled is indistinguishable from not reading. And an assertion of the form `a < b` between two measured quantities is a coin toss unless the two are expected to differ by a margin — say the margin |
| 2026-08-12 | S18 | **An edge detector that polls cannot see a state that flips twice between two polls, and a test that pulls a cable the instant it is plugged in is asking it to.** The same CI run failed on Windows in S17's `an_output_that_falls_over_is_reported_to_every_client`, which had been green since it was written. `Daemon::run` learns an output's health by **polling** it every 500 ms; the test waited a fixed 200 ms and then pulled the cable. Under load the driver had not connected within 200 ms — S18's first fix waited for `health == Ok` instead, which made it *worse*: the cable came out in the same housekeeping interval it went in, so the daemon's before and after were both `Disconnected`, no delta was ever sent, and the test sat until its deadline having proved nothing | The cable now stays in for **1.2 s — over two housekeeping intervals — and the client has to be connected** before it comes out, and the test's own deadline moved from 10 s to 30 s so that a slow machine fails the assertion rather than the budget. **The daemon is not changed:** a polled light always converges on the current health, and missing a flap that lasted less than one interval is what polling *is*. Three things are worth carrying: a fixed sleep is a guess at a condition, and the condition is what to wait for; the condition here is not "it happened" but "it has been true long enough to be observed"; and **a timing-sensitive test should be run under deliberate CPU load before it is pushed** — six burners reproduced this in two runs out of three, and the same load then confirmed all three fixes |
| 2026-08-12 | S18 | **Backpressure could not be measured against the real daemon until the show was made wider, and the reason is arithmetic rather than design.** §7's telemetry frame is 514 bytes per **patched** universe. Against the three-fixture test rig — two universes, about a kilobyte a frame at 30 Hz — a client that stops reading takes several seconds to fill a socket buffer, and the first version of the test asserted `telemetry_dropped > 0` and was satisfied by **two** dropped frames, which is not a client that is behind | The backpressure gate opens a rig across **24 patched universes** (`common::write_wide_show`), so a frame is about twelve kilobytes and the buffer fills in a fraction of a second; and the assertion is scale-free rather than a count — the slow client must have **dropped more frames than it was given** (measured: 21 against 17). Scale-free on purpose: how big a socket buffer is differs between a Windows named pipe and a Unix domain socket, and CI runs this on both. The other two thirds of §8's sentence are measured separately because they fail separately — 40 command round trips in 27–59 ms for the client that *is* reading, and all 40 control deltas in order for the slow one once it comes back |
| 2026-08-12 | S17 | **The liveness check is a file lock, not a process id — and that is stronger rather than merely more convenient.** `ARCHITECTURE_SPEC.md` §10.3 says a stale lock file is "detected through a PID liveness check". Two things are wrong with taking that literally. A **process id is reusable**: a daemon killed at three o'clock and a text editor started at four can have the same number, and a probe would then report a stale lock as live for ever — a desk that refuses to start and cannot say why. And a probe is **platform code**: `kill(pid, 0)` and `OpenProcess` are two implementations of one question, both need `unsafe` or a `#[cfg]`, and §10.1 allows this crate neither | `std::fs::File::try_lock`, stabilised in Rust 1.89: an advisory lock the operating system releases when the process ends, *including when it is killed*, which is the case the criterion is about. A second daemon does not ask whether process 4711 is alive, it asks whether anybody is holding the guard, and that answer cannot be wrong. The workspace `rust-version` moves 1.85 → 1.89 for it. **Two files, and that is the one wrinkle:** an exclusive lock on Windows stops *other processes reading the locked bytes*, and the whole point of §2.2's file is that clients read it — so the lock is held on an empty `prismd.guard` and the discovery document is an ordinary `prismd.lock` beside it. The process id is still written into it, because §2.2 says so and because it is what a person looks at; it is reported, not believed. **Checked by mutation:** removing the refusal on `WouldBlock` turns `a_second_daemon_is_refused_and_told_where_the_first_one_is` red |
| 2026-08-12 | S17 | **The frame layout is the desk's whole universe range, not the show's — because a `FramePublisher`'s layout is fixed for its lifetime.** Every output thread and the telemetry channel is a subscriber attached before the tick starts (S2: `subscribe` allocates), and the layout is decided when the publisher is built. A layout built from `Show::universes()` would therefore mean that patching a fixture into a universe the show did not have yet takes effect **only after a restart** — in the middle of a get-in, which is exactly when a rig gains a universe | `frame_layout(count)` covers universes 1..=`count`, default 64 (`UniverseId::MAX`), overridable with `--universes`. The cost is 512 bytes of frame per unpatched universe, copied once per subscriber per tick: 32 KiB a frame at the full 64, which is the size S6's stress gate was measured at. **Telemetry is filtered back down to the universes the show actually patches**, because 64 universes at 30 Hz is a megabyte a second of nothing to every client, and `docs/IPC_PROTOCOL.md` §7's channel is droppable rather than free. The alternative — rebuilding the publisher and every output thread on a layout change — would have terminated the sACN streams mid-show to add a universe |
| 2026-08-12 | S17 | **A rebuild replaces the whole `MergeBody`, and the tick thread neither allocates nor frees to accept it.** Everything that changes what the engine *is* allocates and says so: `MergeBody::for_patch`, `load_groups`, `load_sequence`, `load_programmer`. None may run on the tick (§3.1). Reaching into a body that lives on the tick thread is therefore not available, and the body cannot be moved without something standing in its place | The core thread builds a new body and leaves it in `BodySwap`. The tick reads **one atomic per tick** to find out whether there is one — free, and false almost always — and takes it with a `try_lock` that never blocks: a failed attempt costs a compare-and-swap and the body arrives 23 ms later. The body it replaces goes back the same way, so the *deallocation* is the core thread's too. The frame is blanked on the tick the new body arrives, which is the half of `Effect::Repatch` S11 named and the easy one to forget — the encoder writes only patched channels, so an unpatched one would keep what the old rig put there. **Checked by mutation:** dropping the blank turns three tests red, including `an_unpatched_channel_does_not_keep_what_the_old_rig_put_there`. **The cost, and it is a real one: a rebuild stops every playback**, where `MergeBody::load_sequence` alone stops only the executor whose cue list changed — `CuePlayer::load` leaves *its* playback stopped by design (S5), so the difference is about the other executors. **S28 requirement:** a cue editor that stores while a show is running wants a job mailbox carrying a prepared `SequencePlan` and returning the old one for the core thread to drop, rather than a whole body |
| 2026-08-12 | S17 | **`prismd` contains no `#[cfg(target_os = …)]`, and the three places it would have needed one were each solved rather than avoided.** §10.1 names `prism-protocols`, `prism-app` and `prism-ipc`'s `transport/local.rs`, and not this crate | **The user data directory** is resolved from the *environment* — `PRISMD_DATA_DIR`, then `APPDATA`, then `XDG_DATA_HOME`, then `HOME` — which is what the platform conventions are actually written in, and `paths::data_dir` takes the environment as a **function** rather than reading it, because `std::env::set_var` is `unsafe` in edition 2024 and the workspace forbids `unsafe_code`: a test that set `APPDATA` could not be written and one that passes a lookup can. **The tick thread's priority** is the `thread-priority` crate's, which is the same move S16 made with tokio's `net` feature for the pipe and the socket: a dependency whose whole purpose is to hold that split, and it cross-compiles clean for `aarch64-unknown-linux-gnu` and compiles no C. **The IPC endpoint** is `prism_ipc::local::daemon_address(label)`, added to the module §10.1 already exempts; the label is a hash of the data directory, so two accounts on one machine do not ask for the same pipe name. CI runs `cargo test -p prismd` in the **Linux** job from this session, which is what keeps the claim honest |
| 2026-08-12 | S17 | **Only the tick *thread* is raised, and a refusal is not fatal.** S6 measured both halves: at the shell's default priority the same ten-minute run missed **45 ticks with a p99.9 of 54 ms**, and with the whole *process* raised — a Windows priority class applies to every thread in it — the stress gate missed **7 484 of 26 455** with a median jitter of one scheduler quantum | `thread_priority::set_current_thread_priority(Max)` is called by the tick thread itself, once, as its first act; the runtime, the driver threads and the surface stay ordinary. **A refusal is logged at `WARN` and the daemon carries on**, because an unprivileged Linux container refuses it and D10's Raspberry Pi is exactly that — a daemon that would not start over it is a daemon that does not run on the machine D10 exists for |
| 2026-08-12 | S17 | **`prismd` is a library with a binary on top, and that is what makes the exit criteria assertable at all.** Every criterion this session was set is a statement about a *running process* — it drives DMX with no client, a second instance refuses, a stale lock is taken over, the handshake serves the world — and `tests/` links a crate's **library** target, of which a binary has none | `src/lib.rs` is the daemon; `src/main.rs` is thirty lines that read a command line, build the runtime and wait. `main.rs` is consequently the one file in the crate with no coverage at all, which is honest rather than hidden: what is in it is `--help`, `--version`, the two `eprintln!`s a person sees when a daemon will not start, and `tokio::signal::ctrl_c`. The interesting half of that file — the run loop — is `daemon::Daemon::run`, and it is tested |
| 2026-08-12 | S17 | **Several daemons in one test process measure each other, which is S6's finding one level up.** Eight `#[tokio::test]`s each starting a daemon means eight tick threads at the priority §3 asks for, each spinning for the last millisecond of every 22.7 ms slot, on a machine with four cores. It showed up as a client task that never got a core to run on: `an_output_that_falls_over_is_reported_to_every_client` passed alone and timed out beside its neighbours | The daemon tests take turns behind a mutex, the way `prism-protocols`' hardware tests do (`common::one_daemon_at_a_time`). Worth writing down because the diagnosis is not obvious from the failure: the test that failed was about output health and had nothing to do with timing, and the first two things suspected were the housekeeping interval and the mock's reconnect — both of which were fine. **The rule S6 stated for two timing tests in one binary holds for two real-time *threads* in one process**, and a daemon has one by design |
| 2026-08-12 | S17 | **A new clippy lint failed the whole workspace on code nobody had touched, and the cause is that `stable` moved.** `manual_is_multiple_of` fired eight times across `prism-engine`, `prism-core` and `prism-protocols` — all of it code that was green when S16 pushed it. CI pins `dtolnay/rust-toolchain@stable`, so the same run would have failed there | Fixed everywhere rather than allowed. The entry exists because it is the S16 lesson in a different disguise: **a check that passed on Tuesday is not a check that passes on Thursday**, and the only thing that establishes the workspace is clean is running it now. A session that had trusted "clippy was green last time" would have pushed a red build and spent the first CI run finding out why |
| 2026-08-12 | S17 | **One mutation check did *not* go red, and that is the finding.** Diffing the programmer *before* the rebuild instead of after — putting slot indices from the plan that has just been replaced onto the wire — leaves all 78 tests green. The reason is that `rebuild` reloads the whole programmer into the new body through `MergeBody::load_programmer` and resets what the engine is believed to hold, so the diff afterwards has nothing to say | The ordering is **belt and braces**, exactly as S14 recorded of its own pass ordering, and it is documented as such rather than defended as load-bearing. What *is* load-bearing is the reload: removing `load_programmer` from `build_body` turns `a_programmer_value_survives_a_repatch_and_lands_on_the_new_slot` red. Worth recording because a mutation that changes nothing is evidence about the design, not a gap in the tests — and the next session to touch this should know which of the two lines is holding the roof up |
| 2026-08-12 | S17 | **`Delta::ExecutorState` carries the cue index the *show* holds, because there is no channel back from the tick yet.** The daemon knows what it dispatched — a Go makes an executor active, an Off makes it inactive — and `Show::record_executor_state` is where that is written down. What cue a playback has reached, and when a `Follow` moved it on by itself, is `CuePlayer` state on the tick thread | The active flag travels and the cue index is whatever the show already had. **S18/S26 requirement:** an executor bar that shows the current cue needs a reverse channel — the same shape as `TickHealth`, atomics written by the tick body and read by the daemon — and until it exists a `Follow` cue advancing is invisible to a client. Not invented here, because a number the daemon guessed at would be worse than one it does not claim to have |
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

**Every button on an executor does what its name says.** `Command::ExecutorButton`
carries *which button* and the executor decides what that means: `Go+`, `Go-`,
`On`, `Off`, `Toggle`, `Flash`, `LearnSpeed`, `Empty`. `Toggle` is resolved
against `isActive` by the daemon, which owns it, and by nobody else; `Flash` is a
layer over the master rather than a write into it, so releasing one gives back
exactly the level that was there — including one that arrived while it was held.
The three rows of `docs/MCU_MAPPING.md` §4.1 that S22 could not bind are bound as
written, and the shipped profile's `deviationsFromSection41` block is empty.

**The tick answers back.** `prism_engine::PlaybackReport` is a table of atomic
words published at the end of every tick and sampled by the daemon at 25 ms:
which executor is running and which cue it is on. `Executor::currentCueIndex` has
been in the domain since S1 and on the wire since S11 with nothing filling it —
the desk drew a dash and two tests demanded that dash so the gap could not be
forgotten. Both are inverted now, and the strip shows `Q1`.

**Speed masters exist.** `docs/DMX_MERGE.md` §4 item 3 named them in S3 and
nothing implemented them; `Executor::speed` is one per executor, the player's own
clock runs at it, and `LearnSpeed` taps it in.

**A store says what it will do and then does that.** `StoreMode` has three
values, the **operator** chooses one, and it travels in the command — so the
daemon never guesses and the outcome does not depend on which client sent it.
Merge writes and keeps, Override makes the cue exactly what the programmer
holds, Remove takes the programmer's values out and uses none of their levels.
`StorePreview` grew a fourth count, `removed`, and the four of them account for
every value on both sides whichever mode was asked about.

**A cue can be opened, changed and put down again.** `EditCue` loads a stored
cue into the programmer with every `presetRef` kept; `Update` puts it back in
Override mode and carries nothing, because which cue and which mode are both the
desk's. `Session::editingCue` is the state that makes an Update key blink, and
it clears when the programmer is cleared, when the cue is deleted and when
another cue is loaded — and **follows** a renumber, because an Update that
stored into the number it remembered would otherwise recreate the cue.

**The selected sequence is a field.** `ARCHITECTURE_SPEC.md` §4.1 gained
`selectedSequence` and §4.4 gained `SelectSequence`, which is S28's marked
assumption settled the other way. A cue list can now be written before anybody
decides which fader it goes on, and `Store Cue 5` means something with no
executor selected.

**The command line is the interface.** `ARCHITECTURE_SPEC.md` §4.5 is the
decision and every window keeps it: a key writes a word into
`Session::commandLine` and does not act, a key that needs an argument writes and
waits, an argument keyword is appended, and an item picked out of a list writes
the line *and* submits it — because the pointer has supplied the argument the
line was waiting for. The exceptions are the ones a line cannot express and are
small: the executor keys and faders (a Go is a gesture with timing in it, §4.3),
the encoders, and dragging a window on the canvas (§4.2).

**Four verbs cover six pools.** `Delete`, `Copy`, `Move` and `Label` take an
`ObjectRef`, so `Delete Cue 3` and `Delete View 2` are one command each rather
than twenty-four commands between them. Which applier owns one is a property of
its target: a view is session state, the other five are the show's, and
`Command::is_session_command` reads it. S35's `DeleteView`, `RenameView` and
`MoveView`, S28's `DeleteCue` and `CreateSequence` were folded into them rather
than left beside them.

**A cue list nobody has put on a fader can be played.** `PlaybackTarget` says
which playback a line means — an executor, a sequence, or the selected one — and
the daemon resolves it, because which executor holds a sequence is show state.
The engine carries a playback per `PlaybackId`, and a sequence gets one of its
own only while no executor holds it, so two players never fight over one cue
list. This is the other half of S39's argument for `selectedSequence`: a list you
can write but cannot hear is a list you cannot check.

**A cue list made on a free number is the one being edited.** `Store Sequence 4`
is both acts, and it selects what it made, because `Store Cue 1` names no cue
list and means the selected one. A store into a list that already exists moves
nothing. The journal images the field with the sequence, so an Oops takes both
back together.

**A venue's rig is data.** `OutputInstance` is a number, a name, a kind with
its parameters, the universes it carries and whether it is enabled — one row per
interface, several outputs of several kinds at once, and each carrying only what
it is wired for. `AddOutput`, `ConfigureOutput`, `RemoveOutput` and
`SetOutputEnabled` edit it while the show runs.

**And the rig belongs to the building, not to the show.** It lives in
`prism_core::MachineConfig` beside the desk identity, which is that module's own
argument for the sACN CID with one more beside it: a show carried to another hall
on a stick must not bring the first hall's cabling with it. That makes the four
commands a **third applier** — neither the show's nor the session's — and none of
them is undoable, because the Oops journal is the show's and an undo that
re-addressed a node would move light on a stage.

**Reconfiguring costs the tick nothing.** `prism_engine::FrameEnrolment` is how a
subscriber joins or leaves a running publisher: one atomic flag, a `try_lock`
that never blocks, and a retirement slot so a departing buffer is freed on the
caller's thread. The outputs that did not change send without a gap, which is
asserted on the recorded frames the way S18 asserts the D2 gate — and the tick
that takes on four subscribers and gives up four makes **no allocator call at
all**.

**A universe with nowhere to go is reported.** `ShowIssue::UniverseNotOutput`,
said on the way up and again as a `Delta::Notice` whenever the rig changes.
Refusing to patch into an unwired universe would be a desk that cannot be
prepared before the get-in; dropping it in silence is how a universe ends up dark
with every light in the interface green.

**The desk in the rack is a device.** `prism-midi` is the workspace's fourth
`#[cfg(target_os = …)]` exception and its whole purpose is to hold the split:
`midir` is in `system.rs` and nowhere else, target-gated so the ARM64
cross-check never compiles it. A configured port is a **name**, chosen out of
`Query::MidiPorts` and kept in `MachineConfig`; a port that is not there is a
warning and a daemon that starts; a cable pulled mid-show is
`SurfaceHealth::Disconnected` and one put back redraws the whole surface — and
the engine hears about neither.

**A desk is configured where an operator can see it.** `WindowType::Settings`
draws four panels — the rig of S33, the MIDI ports of S36, the show file, and
everything `prismd` used to be told on a command line. It is a **window**: on the
canvas, draggable, on a second screen, behind an F-key, and every panel is a
reader over daemon state, so closing it and opening it again shows the same thing
and a second client sees the change.

**Every operational flag is a setting.** `prism_core::Settings` holds the network
exposure and the §2.1 token, the log level, the universe count, the exit action,
the autostart flag, the fixture library's path and the surface binding profile,
and `Command::ConfigureMachine` writes one field at a time. A flag is still the
value for that run — and `MachineSettings::overrides` says which rows a command
line is holding, so a panel greys them out and names the flag rather than
offering a box that does nothing.

**The WebSocket listener is on by default, on loopback**, and a listener that
cannot bind is a warning and a daemon that starts. Two daemons on one machine
both want 7373; one stray process must not make a desk unstartable half an hour
before a show.

**A show file can be opened.** `SaveShowAs`, `OpenShow`, `NewShow`, `ExportShow`
and `ImportShow` — five commands the interface had no way to send, so the file an
operator was working in was whatever `--show` had named at start-up. An open never
creates, a new show never replaces, and an import never saves.

**A desk's keys are edited at the desk.** `docs/MCU_MAPPING.md` §4's binding
table was read once, from a file, at start-up; an operator who wanted a key to do
something else edited JSON beside the daemon and restarted it. The *Controls*
panel draws all seventy-three controls of the surface, says what each one does
now, offers the whole vocabulary of §4 for what it should do, and marks which
controls stay PrismDMX's while the desk is also driving a sound console.

**Learn is S20's method rule run backwards.** That session established that the
way to find out what a control sends is to press it and read what arrives, never
to ask the profile. An editor has the same question, so it gets the same answer:
the operator presses the key they mean and the daemon names it — and **while
learn is armed the key does not fire**, because finding out what Record is called
must not cost an operator their programmer.

**The table belongs to the building, and a file is an import.** It lives in
`prism_core::MachineConfig` beside the rig and the MIDI port, so a show carried to
another hall does not bring the last hall's F-keys. Naming a profile reads that
file *into* the table; a desk starts with the keys it was left with, because an
operator who rebound one at the desk must not find it back the way it was the next
morning.

**An executor is a handle on a cue list's playback, and which handle it is, is
editable.** Punch-list B18 and B15 are one sentence: a playback belongs to the
*list* — one cue pointer, one fade, one master level, one rate — and an executor
says which list, what its fader does, what its encoder does and what each of its
four keys does. Two `Master` faders on one list are two handles on one number; a
`Master` and an `XFade` on it stay independent; `Go` on either advances one
pointer. All three are asserted on the frames a mock output received.

**What the five controls do is set from the window, the line and the desk, and
they are one path.** `Command::ConfigureExecutor` says it, the grammar has the
words (`Assign Executor 1 Fader Master`), and the editor **writes that line**
rather than sending a command — so the exit criterion *the three produce the same
state* is true by construction. The ninth button function carries a line the
operator wrote, which is the owner's answer to *every desk needs a different
number of them*.

**An Art-Net output knows whether anything is listening.** `prism-protocols` has
a **receive** path for the first time: `ArtPoll` out on a cadence of its own,
`ArtPollReply` parsed field by field out of §6 of the specification, and a
discovery thread that keeps a table the daemon answers `Query::ArtNetNodes` from.
A node is *answering*, *never answered* or *stopped* with the age of its last
reply beside it, and the daemon folds that into the health every client is told —
so punch-list **B6** is closed at the line an installer actually reads.

**Begin S48** (tracking, and a cue list that lands in the same place twice — see
§8 for the prompt that starts it, and the running order below). It has what it
was waiting for: S45's *one sequence, one playback*, without which a tracking
state per playback of the same list would be two answers to the question that
session exists to give one answer to. **S29** (the Tauri shell) is still waiting
behind the UI work with one loose end named for it: S37's autostart switch is
written down, read back and reported, and **acted on by nobody**.

Carried out of S46:
- **A promise left in a doc comment is a design decision waiting to be kept.**
  `udp.rs` has said since S9 that discovery *will need its own seam rather than a
  `recv` bolted onto this one*. It was right, and the reason is worth restating
  rather than merely obeying: an output's thread exists to keep a DMX line fed,
  and a `recv_from` on the trait it holds would put a blocking call within reach
  of three drivers that must never make one. The measurement that says the seam
  was the right shape is that `ArtNetOutput`, `SacnOutput` and `OpenDmxUsb`
  compile **unchanged**.
- **When a status word is wrong, ask whether it is the wrong word or the wrong
  subject.** B6 looked like *`OutputHealth` needs more values*. It was not: what
  answers is a **node**, and giving an Open DMX cable a `NeverAnswered` would
  have been a state it could enter and never leave. The new word belongs to the
  new subject, and the old word is **folded down** from it in one place so a
  delta and an answer cannot disagree.
- **A fold made out of ignorance is the same bug pointed the other way.** The
  obvious rule is *no reply means not well*; it is wrong for a desk whose socket
  never bound, which has observed nothing. So the fold happens only while the
  discovery is listening, the answer carries `listening` so a panel can draw it
  **before** the list, and the two causes of not listening are told apart by
  whether there is a reason — an absence has none.
- **A parser fed by strangers is measured rather than asserted.** `ArtPollReply`
  holds its three name fields as the byte arrays they arrived in and hands them
  out as `&str`, so parsing allocates **nothing** and 4 096 hostile datagrams
  cost zero allocator calls. The table is bounded and one read pass is bounded,
  because an unbounded `Vec` grown by whoever can reach a UDP port is an
  allocation storm with extra steps.
- **A test that fails for the right reason is worth more than the assertion it
  broke.** `wiring.rs` caught *poll where you already send* arriving at a
  loopback socket standing in for a node — and, behind it, a daemon binding
  `0.0.0.0:6454` in a test suite, which `udp.rs`'s own tests have forbidden since
  S9. **A session adding a listener should ask which existing target will now
  open a socket it did not open before**, and give it the flag rather than the
  warning.

Carried out of S45:
- **When a field moves between documents, grep for what depended on the container
  it left *and* the one it arrived in.** A React effect keyed on `/sequences` was
  correct for four sessions because the playback state was on the executor; the
  moment it moved back, a cue advance rewrote that subtree and the cue viewer
  asked the daemon what a store would do once per cue of a chase. The test that
  caught it counts the *questions* rather than checking the answer, which is the
  only shape that could have.
- **A generated table that stops being generated is a second list waiting to
  happen.** `ExecutorButtonFunction` gained a variant with a field and
  `prism_domain::export::string_union` dropped its whole table — four TypeScript
  files import it to *check* a string. The fix belonged in the generator (skip an
  object arm, keep the literals), not in a hand-written array in TypeScript.
  **A session adding a payload variant to a string enum should expect this and
  look at `ui/src/bindings/variants.ts` before writing anything.**
- **A real-time gate that goes red needs a control window before it needs a
  diagnosis.** The ten-minute tick gate read 58 missed ticks; the *bare tick with
  no merge body* read 956 on the same machine in the same hour. The control is
  `the_tick_alone_holds_its_deadline_for_ten_minutes` and it has been in the
  harness since S6 for exactly this. **Run it before believing the other one**,
  and record the gate as unmeasured rather than green or red when the machine is
  the thing being measured.
- **A `#[serde(default)]` answers *what does a file that never had this say*, and
  cannot answer *what did the operator leave the fader at*.** Moving
  `masterLevel` from the executor to the sequence needed the executor row read a
  **second time**, as a struct that is only the moved fields
  (`ShowStore::carry_levels_onto_their_cue_lists`). A `.prism` file keeps rows as
  MessagePack **maps** (`to_vec_named`, S1), so a partial decode is an ordinary
  read rather than a trick — and that is the shape any later field move should
  copy. The test for it writes the pre-move rows by hand, because a fixture built
  by the code under test cannot say anything about a file that code never wrote.
- **An interface that grows a new editor should ask whether the grammar can say
  it before deciding it is an exception.** `ARCHITECTURE_SPEC.md` §4.5's list of
  exceptions is *things a line cannot express*; the control editor is not one,
  and giving `Assign` a second sentence cost less than a second code path would
  have. What it bought is the exit criterion for nothing: window, line and bound
  key converge on one line.

Carried out of S38:
- **A wide unit enum is the most expensive thing in the domain, and boxing
  cannot fix it.** `proptest_derive` builds one value tree with a slot per
  variant costing **560 bytes whatever the variant carries** — a bare unit
  variant costs the same as one with a payload. `GlobalButton` is sixty-four
  buttons carrying nothing and its derived tree was **36 064 bytes**, larger than
  the whole of `Command`; everything holding one inherited it. `crate::arb::boxed`
  cannot reach that, because boxing a field replaces the subtree with a pointer
  and the subtree is still *built* as a local before the box takes it.
  `crate::arb::arbitrary_from_list!` maps a range onto the enum's own `ALL` and
  is 16 bytes. **A session adding a unit enum with more than a handful of
  variants reaches for that; one adding a payload still reaches for `boxed`.**
- **The budget was above the demonstrated ceiling, and now it is the measured
  figure.** S37 set 32 KiB with two measurements either side of it — 36 160
  overflows, 30 992 does not — and 32 768 sat in the untested gap. S38 landed in
  that gap at 32 144: this test green, `prism-core`'s suite overflowing. It is
  **30 992** now, the largest tree the whole suite has ever run on, and it
  watches all four wire enums rather than two, because `Answer` went over while
  it was green.
- **A `Command` variant is expensive and a `MachineChange` variant is free.**
  S38 wanted to say two new things about the desk and added **no command**: both
  are `MachineChange` variants inside `ConfigureMachine`, which is already boxed.
  That is also the better protocol — what a desk's keys do is one of this
  machine's settings, beside the profile file they used to be read from — so the
  budget and the design agreed for once. **Check whether a new command is really
  a new command before adding one.**
- **Where an editable thing is stored is a question about who edits it.** A
  binding table could have lived in a file beside the daemon; it lives in
  `MachineConfig` because a file is written by one person at a time and a
  protocol is not. One row per command, one writer under one lock, and *two
  editors, one table* stops being a race anybody has to win.
- **A fallback has to name what it falls back *on*.** S22's rule was *a
  malformed profile falls back to the built-in defaults*, and that was one
  sentence while the defaults were the only other thing there was. Once a desk
  has a table of its own the sentence splits: the table **in force** stands,
  which for a fresh desk is the defaults and for an edited one is its own. The
  test that caught it was S37's, whose own wording — *the keys it had* — was
  already the stronger claim its assertion did not make.
- **A rule stated once is a rule.** SMPTE/Beats can now be named from three
  directions — a file, a command, and a stored table — so it is refused from all
  three, and the sentence is `prism_domain::RESERVED_REASON` **once**;
  `McuProfile::reserved_buttons` points at `RESERVED_BUTTONS` rather than copying
  it. An operator meeting two wordings would reasonably conclude they were two
  rules.
- **A vocabulary that travels is the domain's; a measurement is the device's.**
  `BoundControl`, `SurfaceAction`, `GlobalButton` and `StripButton` moved to
  `prism-domain` and are re-exported from `prism-surface`, because a command
  carries them and the interface's copy is generated from them. §2.1's note and
  CC numbers did **not** move and must not — nor did the codec, the shadow model,
  or the resolution of an action into a `Command`, which needs an event and a
  session. The line is *names* against *meaning*, and `IMPLEMENTATION_PLAN.md`
  S32 predicted it from the other side.
- **A mode that a client can arm must be one shot, or a client that goes away
  can break the desk.** Learn disarms itself on the first control, so a browser
  closed mid-learn cannot leave a console whose keys do nothing — and it is
  written down nowhere, so a restart cannot either.
- **A suppressed press has a release, and the release is not nothing.** Layer 3
  forwards both edges of an `ExecutorButton`, so swallowing a learned press and
  letting its release through would release a `Flash` that was never held.
  `SurfaceLink::swallow` is one button remembered until its release has been
  eaten.
- **The decoder gap S37 named is closed by a test rather than by a recording.**
  Both new deltas and the new answer go through the real `readDelta` and
  `readAnswer` in `controls.test.tsx`, because the fake daemon encodes genuine
  MessagePack. A recording could not have carried them: every one of the six
  runs a daemon whose rig came from its command line, and such a daemon refuses
  every machine command by design.
- **The recordings gained a fourth ordinary difference and it is S37's
  fields settling in.** `machine.dataDir`, `machine.deskId` and `showFile.path`
  differ on every regeneration, because each runs in a fresh temporary directory
  and generates a new desk identity. S37 added the two fields; S38 is the first
  regeneration since, so it is the session that finds out they are per-run
  values. Nothing structural changed in any of the six.

Carried out of S37:
- **`prism_core::Settings` is where anything else about this machine goes**, and
  the list S33 started is now closed: the desk identity, the rig, the surface
  port, and the nine operational settings. Adding a tenth is one field with
  `#[serde(default)]`, one arm in `MachineConfig::configure`, one variant of
  `MachineChange`, one row in `cli::resolve` and one row in the panel — and a
  decision about `needs_restart`, which is the only part that needs thought.
- **A flag overriding a setting is decided once, in `prismd::cli::resolve`.**
  It answers both the value in force and the list of held rows, so the two cannot
  drift; written inline at each use it would have been fifteen places to get
  wrong and a second list for the interface. **Any later flag that shadows a
  setting adds a `MachineOverride` variant**, or the panel will offer a box that
  does nothing.
- **A value a client must not choose is asked for rather than sent.**
  `MachineChange::NewToken` and `NewIdentity` carry nothing: both need entropy
  `prism-core` may not have, and a client choosing either would be choosing this
  desk's password or giving two desks one sACN CID. `Effect::NewToken` and
  `Effect::NewDeskIdentity` are the shape — `Effect::EmbedProfile`'s, one layer
  out — and anything else generated by the machine follows it.
- **A rule that guards a door has to guard it from both sides.** §2.1 refuses a
  listener off loopback with no token; taking the *token* away has to close the
  same door, or the state the first rule forbids is reachable from behind. That
  is `MachineConfig::close_network_listener`, and the general shape is worth
  keeping: a validation on one field is half a rule when a second field can undo
  it.
- **`Effect` is `Clone` and no longer `Copy`**, because three of them carry a
  path. It costs a `clone` at the two places the daemon matches on one; the
  alternative was a second channel out of `Applied` for one string.
- **A counter a panel watches is asked for, not pushed.** `Query::OutputStatus`
  closes the gap S33 named on its way out. The rule it settles: **state that moves
  at a rate nobody can broadcast is a query with a cadence of the client's own**,
  and the answer carries only what moves — the configuration arrives by delta and
  repeating it once a second would be a rig on the wire for the sake of a number.
  `Query::MidiPorts`' `SurfaceStatus` is the same shape for the other device.
- **A delta or an answer with no caller is a decoder arm nobody writes.**
  `readDelta` had no `OutputsChanged` (S33) and no `SurfaceChanged` (S36), and
  `readAnswer` had no `MidiPorts` (S36); a daemon that changed its rig would have
  faulted a client and resynchronised the connection. Nothing had noticed because
  nothing had ever changed a rig from a browser. **A session that adds a message
  should add its reader and something that sends it in a recording**, because the
  round-trip properties in `prism-domain` do not reach TypeScript.
- **The `Command` value tree now has a budget, and it is a test.**
  `wire.rs::a_generated_command_fits_in_a_test_thread` fails at 32 KiB, with both
  numbers measured: 36 160 bytes overflows `prism-core`'s `tests/oops.rs` and
  30 992 does not. Three sessions in four have met this; the remedy is always
  `crate::arb::boxed` on a *field*, and never `RUST_MIN_STACK`.
- **A test target that starts a daemon says `websocket: Listen::Off`.** The
  listener is a setting now and the setting is on, so a target that said nothing
  would bind 127.0.0.1:7373 — and several run at once. A suite must not open a
  socket it does not use.
- **A settings command is not a command-line word, and that is the test to
  apply.** `ARCHITECTURE_SPEC.md` §4.5 asks *could a line say this?* — S40's
  grammar has no noun for a file or for a log level, exactly as it has none for a
  window type, so these are sent the way `OpenWindow` is. A later panel should ask
  the same question rather than assume either answer.
- **The data directory is the setting that cannot be a setting**, because the
  settings live in it. The panel names it and the files under it and offers no
  way to move it; a redirect would be a mechanism §10.3 does not describe, whose
  failure mode is a desk that starts with somebody else's rig.
- **A timeout in a test is a claim about the harness, and should say so.**
  `App.test.tsx`'s command-line test has 20 s of its own because it asserts a
  sequence of messages rather than a duration, and v8 instrumentation over
  forty-eight files makes the 5 s default a measurement of the machine. S36 met
  the same test and re-took the measurement; the second time is when it becomes a
  number.
- **The biggest thing S37 did not build is the fixture library's own panel.**
  S27 predicted the *Show files* panel would be where a library import lands, and
  it is not there: `Settings` names the library's **directory** and nothing more.
  Importing, browsing and authoring a profile is still a session of its own, and
  `ARCHITECTURE_SPEC.md` §9's `profiles/fixtures/` is still written by
  `tools/fetch-fixtures` alone.
- **And the autostart flag is written down and acted on by nobody.** S29's shell
  is the one process that can add a start-up entry without administrator rights;
  until it exists, the switch is a preference a desk records and reports. The
  panel says so in as many words, because a switch that appears to do something
  and does not is worse than one that explains itself.

Carried out of S36:
- **`prism-midi` is where a platform goes, and the rule is the manifest rather
  than a habit.** A dependency that needs a system package on one target is
  declared per target, so the ARM64 cross-check sees a crate with **no
  dependencies at all** and the Linux CI jobs exercise the no-backend path for
  real. **Any later device follows this shape**: the split in one crate, the
  seam above it (`MidiBackend`) so the schedule can be asserted with a fake, and
  the cost of the platform that is left out written into
  `ARCHITECTURE_SPEC.md` §14 rather than left to be discovered.
- **A device is named, never numbered.** `prism_midi::selects` is three rules in
  order — exact, undecorated, fragment — and the order is the design: looser
  rules last, so a venue with two of the same device can always write both names
  out in full. A later session naming any other enumerated device (a network
  interface, a second surface) should reuse `normalise` rather than write a
  second matcher.
- **Silence is not a reason to reconnect, and that rule lives one layer below
  the health state that reports it.** S20's finding is now in two places on
  purpose: `SurfaceHealth::Unresponsive` says *power-cycle it* to the operator,
  and `prism_midi::MidiSurfacePort` refuses to reopen on silence so the
  diagnosis is not churned away underneath. Only two things close a port — it
  leaving the enumeration, and a write that failed.
- **`SurfacePort` has four methods now, and `refresh(now)` is the new one.**
  Whoever adds a third implementation implements it or accepts the default of
  doing nothing; the clock stays an argument all the way down, which is what
  lets the whole reconnection schedule be walked in a test in milliseconds. The
  ordering in `SurfaceLink::poll` is load-bearing: the port is asked **after**
  the send, because a failed write is one of the two ways a cable is discovered
  to have come out.
- **The machine's own things are a growing list and they all live in one
  place.** `MachineConfig` now holds the desk identity, the rig and the surface
  port, `Command::is_machine_command` names five, and the three example lists in
  `crates/prism-core/tests/common/mod.rs` are **53 forms**. S29's autostart flag
  and S37's data directory and network exposure belong here too, and each one is
  a `#[serde(default)]` field, one applier arm, one delta and one line in three
  test lists.
- **Two command-line flags, two reasons, two errors.**
  `MachineError::ConfiguredOnTheCommandLine` is *nothing is written down this
  run* and covers every machine command; `SurfaceOnTheCommandLine` is
  *`--surface` named this port*. They are separate because a daemon may take its
  rig from a file and its surface from a flag, and an operator told the wrong
  one goes looking in the wrong place. **A test target that wants to configure
  the machine must not use `--mock-output`** — use `--mock-devices` and build the
  rig with `AddOutput`, which is what `tests/outputs.rs` and
  `tests/surface_port.rs` do and what S37's settings window will do.
- **What the operating system knows is asked for, not mirrored.**
  `Query::MidiPorts` enumerates on the thread that asked, because a list built at
  start-up is a list of what was plugged in then — and the gesture the query
  exists for is somebody plugging a desk in and pressing *rescan*. Anything else
  a later session needs from the machine rather than from the show (network
  interfaces, serial ports, disks) is the same shape, and the answer should carry
  the **configuration beside the enumeration** for the same reason: a list with
  no mark against the chosen row is a list nobody can act on.
- **A status a client reads is refreshed on the housekeeping tick, not the
  surface's.** `Desk::set_open_surface` is written every 500 ms; the surface
  polls every millisecond and the name is a `String`. Half a second is the right
  freshness for a fact that changes when a person moves a plug, and the
  millisecond path stays free of an allocation.
- **The coverage report is worth reading line by line, and this is the third
  session it has paid for.** It found two unreachable branches (removed, per
  S21) and one piece of real logic sitting in the platform module where no test
  could reach it — the framing that hands a caller as much of a message as it has
  room for. `prism_midi::Inbox` is that logic somewhere testable. **The rule to
  reuse: if a `#[cfg]` module is under 60 % and the rest of the crate is at 100,
  look at what is *inside* the module that did not need to be.**

Carried out of S33:
- **The rig is `prism_core::MachineConfig`'s, and that is structural rather than
  a convention.** `Show` has no field for an output, `ShowStore` has no table for
  one, and `desk.rs::outputs_are_not_show_content` asserts it on the show's own
  bytes beside `desk_id_is_not_show_content`. **A later session that wants
  anything else about *this building* — a MIDI port name (S36), a network
  interface, an autostart flag (S29) — puts it here**, and gets the same
  guarantee for free.
- **`Command::is_machine_command` is a third routing predicate**, and
  `prismd::Core::apply` is the only place that reads it. A command added to the
  protocol is still a compile error in `Show::apply`, `SessionState::apply`,
  `Programmer::apply` and `ShowFile::image` — four exhaustive matches — and now in
  a fourth applier as well. **The three example lists in
  `crates/prism-core/tests/common/mod.rs` are the checklist**: 52 forms across
  `show_commands`, `session_commands` and `machine_commands`, and every command is
  in exactly one group.
- **`prism_engine::FrameEnrolment` is how anything joins or leaves a running
  tick.** One atomic flag, a `try_lock` that never blocks, and a retirement slot
  so the departing buffer is freed on the caller's thread. Two rules bind whoever
  uses it next: the vector the tick walks is **built with room and never grown**
  (`MAX_SUBSCRIBERS`), and `collect()` must be called after every reconfiguration
  or the retired vector eventually reallocates on a tick. `tick_allocations.rs`'s
  eighth path is what says so.
- **`prismd::outputs::OutputFactory` is the seam for a driver chosen at run
  time**, and `--mock-devices` is what an operator asks for it with. A new output
  *kind* is a new arm in **both** implementations, which is two places on purpose:
  the mock has to know what it is standing in for, or a test would assert routing
  against a double that carries different universes from the real thing.
- **`OutputInstance::needs_restart` is the whole hot-reconfiguration policy**, and
  it is narrow on purpose. Anything a later session adds to `OutputInstance` has
  to decide which side it is on, and the default should be *not* a restart: a
  field a driver was not built from must not cost a rig its frames.
- **An age, not a time.** `OutputStatus::last_error` answers *how long ago*
  against an elapsed duration the caller supplies, because the daemon and a
  browser have no shared clock. Any later status a client has to show — a
  reconnect, a surface that stopped transmitting (S36) — should travel the same
  way rather than as a timestamp nobody can compare.
- **`prism_domain::socket` exists because `rmp-serde` is not symmetric about
  `SocketAddr`.** A domain type that carries an address uses it. The general
  shape: **any `std` type with a `serde` implementation that behaves differently
  under `is_human_readable` needs a module of its own**, and `wire.rs`'s
  round-trip property is what finds them.
- **`DeviceDescriptor` and `DeviceProfile` are no longer `Copy`**, because the
  serial that pins one cable comes off a `machine.json` now. `SH_RS09B` is still a
  `const`; what changed is that `with_serial` takes anything that can be a `Cow`.
- **A row that will not validate is reported and skipped, never fatal.** A
  hand-edited `machine.json` with one bad hop limit in it must not be a desk that
  will not start half an hour before a show — and the broken row is still *shown*,
  red, because an output that vanished would be worse than one that is wrong.
- **A daemon whose outputs came off its command line refuses to change them.**
  Every test target in this repository starts one that way, so the alternative —
  editing a rig nothing writes down — would be an `AddOutput` that appeared to
  work and vanished at the next restart. S37's settings window starts its daemon
  **without** output flags, and with `--mock-devices` if it wants drivers it can
  read.
- **S37 has everything it needs and one thing it does not.** `OutputSnapshot`
  carries the kind, the parameters, the universes, the health, the frames sent and
  the last error with its age; `Delta::OutputsChanged` carries the rig whole
  whenever it moves. What there is **no** channel for is a *live* frame counter
  between snapshots — the counter moves at 44 Hz and a delta per frame is out of
  the question. S37 either asks (a `Query`, §5.2) or accepts that the number is as
  old as the connection; the second is what a status panel usually wants and the
  first is one variant away.

Carried out of S43 *(the session is still open — see §2.41)*:
- **A home value is a statement about the rig, not about a knob.** Punch-list B1
  moved colour channels to a home of full, and the reasoning was entirely about
  what an *encoder* should read when the programmer is empty. It is the same
  number the bottom of the merge holds, so it also decided what a fixture puts on
  stage with nothing programmed — and for a fixture with no dimmer those are the
  same eight bits. **A session changing `AttributeDef::default_value` is changing
  the output**, and the question to ask is what the fixture does at rest, not
  what the knob says.
- **The picture is the instrument, and it will answer a question the readouts
  cannot.** Two end-to-end failures looked like drift in the tests. What settled
  it was a throwaway Playwright spec that dumped the telemetry canvas's colour
  histogram and the x-range of the lit pixels: the picture was identical before
  and after `red at 100`, which no readout was going to say. **Twelve minutes of
  scaffolding beat an hour of reasoning**, and the spec was deleted afterwards.
- **A predicate that means *intensity* and a predicate that means *has a dimmer
  channel* are different, and the difference is a crash.** `FeatureGroup::Dimmer`
  is what the masters scale; `AttributeType::Dimmer` is what would collide. A
  profile that files its dimmer on the colour bank has the second without the
  first, and supplying it a software dimmer names the same attribute twice —
  `MergeError::DuplicateAttribute`, a rig that will not patch. A unit test
  carrying exactly that odd profile is what caught it.
- **A value the store keeps but the documentation calls *a moment* is a bug
  waiting for a second client.** `surfaceLearned` is broadcast to every client
  and was kept as state; arming learn anywhere then bound whatever the last learn
  had named, from a screen that had nothing to do with the press. **Look for the
  same shape wherever a delta carries an event rather than a fact** — and note
  that the two-editor end-to-end test had been passing for a different reason,
  which is what made it worth writing.
- **`Insert` is ignored inside a text field, and a test that wants the chooser
  must leave the field.** Six e2e failures were one rule being enforced. The
  helper blurs first, which is what an operator's hand does; asserting the rule
  away would have been the easy fix and the wrong one.
- **`evaluateAll` does not wait, and a guard that turns a missing element into a
  skip turns a broken interface into a green test.** `desk.spec.ts` read the
  window chooser one React render after pressing the key that opens it and got an
  empty list on a loaded runner; underneath, `if ((await key.count()) > 0)` meant
  that whenever the chooser had not rendered — or an `Insert` was swallowed by a
  window that had taken the focus — the loop opened **nothing** and the test
  passed. Locators wait for actions and assertions and for nothing else. The
  4K case went from 1.1 s to 7.7 s once the click became unconditional, which is
  the size of what the guard had been hiding.
- **A gesture is a command out and a delta back, in the tests as much as in the
  interface.** `closeWindows` took a snapshot count and clicked that many times;
  on the runner one delta had not landed and the last window stayed open. **D3
  applies to the suite**: a click proves nothing about the state until the state
  is waited for. The three end-to-end failures of this session are one sentence
  written three ways — a chooser read a render too early, a guard that turned
  that into a skip, and a loop that assumed a close had happened.
- **A test id that no longer exists is not always a rename.** Half of this
  session's e2e failures were behaviour that had moved (readings into a window,
  a store bar into another window) and half were claims that had **stopped being
  true**: `patch-no-profiles` went with the toolbar's library search, and the
  overflow probe on `executor-bar` and `encoder-bar` had been measuring `null`
  and reading `[0, 0]` as a pass ever since those bands became windows. **A
  vacuous green is the expensive kind.**
- **Coverage below the house norm in this session's own new files is a gap, not
  a number.** `presetpool.tsx` read 59 % because the context menu the owner asked
  for had no test at all, and `clock.tsx` 78 % because the window had no test
  file. Both are 100 % lines now. The global figure was 97 % throughout and would
  have hidden both.

Carried out of S40:
- **A field mirrored into shared state must not adopt the echo of its own
  writes.** The session holds one command line and every client writes to it, so
  the input has to follow the session or two screens drift apart — but a delta
  carrying *our own* keystroke from 40 ms ago is not news, it is a prefix of what
  has been typed since, and adopting it eats the line. `useMirror` keeps a queue
  of what this client has sent and the adopter ignores the session while any of
  it is outstanding. **The general shape:** any later field that works this way
  — a name being typed into a pool box, a cue number, a search box — needs the
  same guard, and none of them will show the fault on a fast machine.
- **A green local Playwright run is not a green CI one.** This was found by CI
  and not by 28 local tests that passed twice; the runner is slower and the round
  trip is longer there. Worth remembering before treating a CI-only e2e failure
  as flakiness.
- **Playback addressed to a sequence is a second author of the same state, and
  the engine must never let both run.** `Show::playback_of` prefers the executor
  when one holds the cue list, which is what keeps it to one — but the rule lives
  in a function rather than in a type, and a later session that adds a third way
  to name a playback (a *page*, a macro, an OSC route) has to go through the same
  door. Two players of one list would each fade the same slots from a different
  place and neither would be wrong.
- **What *Delete Executor* means had to be decided, and the answer is that the
  place survives.** The row goes out of the show and executor 3 is still executor
  3 on the bar, because the eight strips of a page are `page * 8 + slot`
  arithmetic (**D7**) and not a list. A *move* onto an empty slot therefore
  empties the one it came from, and a move onto a full one **swaps**. Any later
  feature that gives executors an order rather than numbers has to revisit this
  one first.
- **`Store Preset 1` gets its pool from `Session::encoderBank`**, and the grammar
  takes an optional pool word beside it for a line typed with the encoders
  showing something else. What the pool **tab** of the Preset Pool window shows
  stays client-local — §4.2 says a window's own tab legitimately differs per
  screen, and two operators reading different pools is the ordinary case. The
  colour a preset carries is kept by the daemon when the command does not carry
  one, because a command line has no way to type a swatch; giving it one is
  S43's.
- **Cue times and triggers stay `SetCueProperty` and are deliberately not in the
  vocabulary.** `Label` names a *thing*; a fade time is a value on a thing, and
  the line that set it would be a second grammar for every field a cue has. The
  boundary is worth keeping in mind for anything later that wants to type a
  value: it belongs in the cell that draws it, not in the line.
- **A stale `cargo llvm-cov` profile reads tens of points low, and it looks
  exactly like a coverage regression.** `prism-core` read **74.55 %** and was
  **97.95 %** after `cargo llvm-cov clean --workspace`. S28 recorded the same
  effect for two runs in one shell; this is the same finding with a bigger number
  on it, and the rule is simply that a measurement without a `clean` in front of
  it is not a measurement.
- **`#[ts(optional)]` generates `field?: T`, which cannot hold `null`.** The five
  S40 fields that mean *the one the session has* are `#[serde(default)]` only, so
  TypeScript gets `SequenceId | null` and a `null` says *ask the desk*. An
  optional field would have made *absent* and *explicitly none* the same value on
  the wire, and the daemon distinguishes them.
- **A `Command` variant costs stack in the property test, and the budget is
  real.** `Command`'s `proptest` value tree measured **69 280 bytes** and
  overflowed the stack in `wire::command` and `oops.rs`; seventeen
  `crate::arb::boxed()` strategies on the enum-typed fields brought it to
  **28 240**. `RUST_MIN_STACK` would have hidden it. Any later session adding a
  variant with an enum field should expect to box it.
- **A store that creates must build the thing beside the show and write it
  once.** `StoreSequence` created the cue list and then stored into it, so a
  refused store left a half-made list behind — S11's rule broken by a two-step
  edit, and `delta_round_trip.rs` is what caught it. The same shape applies to
  every generic verb: `check_source` runs before `transfer`, and `Copy`/`Move`
  do all their fallible work before anything is written.
- **A green *incremental* `cargo clippy` is not the gate.** An
  `unused_qualifications` in `prism-engine/src/player.rs` appeared only when the
  crate was rebuilt from cold, several runs after the line was written. Worth a
  `cargo clean`-adjacent run before calling clippy green, and worth remembering
  that CI always builds cold.
- **A gesture now sends three messages, and any test that reads `.at(-1)` will
  be wrong.** A key writes the line (`CommandLineInput`), sends the command, and
  clears the line (`CommandLineInput { text: "" }`). Four test files needed an
  `acted()` helper that filters the line traffic out; the command-line tests keep
  the raw list, because they are *about* the line. Any later test that asserts on
  what a window sent needs the same filter.
- **The three shapes are a closed set and the keypad table is private.** Adding a
  key means adding a row to `CONSOLE_KEYS` with `shape: "run" | "write" |
  "append"`, and nothing else — the table is not exported, because `oxlint` is
  right about `only-export-components` and a shared table belongs in a file with
  no component in it.

Carried out of S39:
- **A store may not be re-derived after it has been written.** The defect this
  session actually had: `ShowFile::finish_programmer` writes the cue and then
  routes the command through `Programmer::apply`, which re-derives the store as
  a guard for a direct caller. Merging twice answers the same both times, so
  that cost nothing until `StoreMode::Remove` existed — a Remove that has been
  applied has taken its values out, so the second derivation says *there is
  nothing to remove*, the command is **refused**, and the show has already
  moved. `Programmer::stored` is the fix and the guard stays where it was. **Any
  later session that gives a store a new mode should ask whether its validation
  is still idempotent**, because the failure is silent in the return value and
  visible only in the state.
- **What found it was the recording's two halves disagreeing**, not a unit test.
  `crates/prismd/tests/ui_show.rs` replays the recorded deltas *and* compares the
  result against the per-step snapshot a fresh client was served; a store that
  changed the show without broadcasting a delta is exactly the shape of fault
  that only shows up between them. Worth knowing before anybody is tempted to
  drop one of the two comparisons for being redundant.
- **`StoreSequence` has no preview, and that is a decision.** The three counts
  are about the *values* of one cue and a sequence store is about *cues*; an
  honest answer needs a different shape — how many cues would be added, replaced
  or written into. `SequenceStoreMode::Override` is therefore the most
  destructive command in `docs/IPC_PROTOCOL.md` §5 with nothing in front of it
  but Oops. **S40** should either ask before sending one or add the fourth
  `StoreTarget` and the shape that goes with it.
- **A Merge into the cue being edited does not stop the Update key blinking, and
  an Override does.** A merged cue holds everything the programmer holds *and
  more*, so *nothing has changed since it was loaded* is not true of it. The rule
  is one line in `ShowFile::follow_cue_edit` and it is the subtlest thing in the
  session; `an_override_into_the_cue_being_edited_is_an_update_by_another_name`
  is what holds it.
- **The update state is not persisted and the programmer is not either.** A
  `.prism` file that carried `editingCue` would open blinking an Update with
  nothing to put back, which is why both new `Session` fields are
  `#[serde(default)]` and why `editingCue` is dropped rather than restored. A
  later session that gives the programmer a home on disk has to decide this
  again.
- **`Programmer::load_cue` drops a part the show can no longer resolve**, which
  is the same filter the store runs on the way out — so a cue naming a fixture
  somebody has unpatched loads as the part of it that still means something, and
  an `Update` immediately afterwards would *write that loss into the cue*. It is
  the right answer for an edit and it is worth an operator being told; there is
  no notice for it today, and `StoreCue`'s dropped-value notice (S6) is the shape
  one would take.
- **The sheet and the transport line can be looking at different cue lists.**
  That is the point of the decision, and it costs a rule the interface has to
  keep: the playback marker is drawn only where `executorInForce.sequenceId`
  equals the sequence on the screen, or it would point at a row of a list nobody
  is on. Any later window that draws both needs the same guard.
- **A fourth store mode would need three files and no more.**
  `prism_domain::StoreMode`, the arithmetic in `prism_core::file::counted`, and
  the two arms in `Programmer::cue` and `Programmer::preset`. The interface picks
  it up for free, because the chooser is generated from `STORE_MODE_VARIANTS` —
  which is what that table is for.
- **A flex scroll area needs a floor, not just `min-height: 0`.** The idiom that
  lets a scroll area shrink below its content also lets its neighbours squeeze it
  to nothing, and a sheet with no room for a row has a first row `scrollIntoView`
  puts under the sticky header. `.sheet-scroll` has `min-height: 3rem` and
  `.sheet td` has `scroll-margin-top`; **every other scroll area in the interface
  should be looked at the same way** — S43's list is where that belongs.
- **A green local `cargo clippy` is a claim about the installed toolchain.**
  `rust-toolchain.toml` says `channel = "stable"`, so CI meets each new lint set
  first; 1.98's `chunks_exact_to_as_chunks` failed a build over code untouched
  since S2. Worth expecting once a quarter, and worth *not* pinning: a pinned
  version would trade a day's surprise for never finding out at all.
- **A timing test with a fixed number of shots is a coin the runner flips.**
  `a_process_killed_mid_write_leaves_the_last_committed_state` needs a kill to
  land inside a save; six delays were enough for five sessions and then were not.
  It stops as soon as it has what it needs now, and has twenty-four tries. The
  shape — *keep going until the interesting case has been seen at least once,
  bounded* — is the one any later timing test should take.
- **`oxlint` is right about `only-export-components`.** A guard exported beside a
  component costs React's fast refresh, so `isStoreMode` is private in
  `storemode.tsx` and `ipc/protocol.ts` keeps its own. Two three-line guards over
  one generated table is the cheaper of the two; a shared one belongs in a file
  with no component in it, if a third reader ever wants it.

Carried out of S34:
- **The report's length moves on whichever side of its entries keeps the window
  narrow.** Written after them when the executor grid grows, before them when it
  shrinks — otherwise a reader gets one poll in which the length covers an entry
  this tick did not write. Neither case was a *fault* (a sample may be one tick
  out of date, and `record_executor_state` refuses an executor the show has not
  got), but both are a reading of a playback that is not there, and the fix is
  one comparison. Found by reading the code back after it was green, and green
  again on run **32326458934**.
- **The tick is the only author of `isActive` and `currentCueIndex`, and that
  cost the immediate delta.** An `ExecutorGo` is acknowledged with no
  `Delta::ExecutorState` at all now; the state follows within a poll. The
  daemon used to write it on the way past because nothing else could, and with a
  readback beside it the two race — the symptom was a strip that lit, went dark
  on the next poll because the tick had not run yet, and lit again. **One
  consequence to know:** a `Toggle` pressed within 25 ms of *another client's*
  Go reads the state from just before it. That is the latency any desk with two
  operators has and it is far shorter than a person's reaction, but it is a real
  window and it is the only one in the design.
- **`ExecutorButtonRef` has two shapes and only one of them is D3.** `Slot` is a
  hardware position and the executor decides; `Function` is a row of the
  *profile*, which is the desk's own configuration written by a person. The
  second is what let §4.1's transport row (Play = `On`) be bound as written. A
  later session tempted to let a *client* choose the `Function` form at run time
  should re-read `docs/MCU_MAPPING.md` §4.2.1 first: the distinction is between
  configuration and inference, not between two ways of sending the same thing.
- **A speed master is an executor's, and a named one shared between executors
  does not exist.** `Executor::speed` is what the domain carries and what an
  X-Touch strip addresses. `docs/MCU_MAPPING.md` §4.3 asked for *a tap for
  speed*, which this satisfies; if a venue ever asks for one speed master over
  several executors, that is a new domain type and a new command.
- **The crossfade completes at the end it was heading for, and the next Go
  re-bases it.** That rule is a choice rather than a deduction —
  `prism_engine::player`'s module documentation states it beside S5's four — and
  it is the behaviour of a desk with *one* crossfade fader. A desk with a split
  A/B pair works differently, and `ExecutorFaderFunction` has no way to say so.
- **A `Follow` cue still follows on time while a manual crossfade is engaged.**
  The crossfade replaces the clock of the *transition*; the trigger check reads
  the player's elapsed time. It is the right answer for the cues an operator puts
  on a crossfade fader (which are `Go`-triggered) and it is not obviously right
  for the others. **S39 or S43** should decide whether a crossfade freezes
  follows.
- **`Executor::speed` is `#[serde(default)]`, and that is how an opaque-document
  schema grows.** A `.prism` file keeps each executor as a MessagePack blob
  (S15), so a field added later is a field older files do not carry — and the
  migration mechanism rewrites *tables*. The frozen version-1 fixture found this
  within a minute of the field being added, which is what it is for.
- **A 36th `Command` variant cost two property tests their stack.**
  `proptest_derive` builds one value tree holding every variant's tree at once,
  and in a debug build on Windows the thirty-sixth tipped `prism-core`'s
  `tests/oops.rs` and `prism-ipc`'s `tests/framing.rs` into
  `STATUS_STACK_OVERFLOW` — with no failing case to read, which is the confusing
  part. Both are `.boxed()` now and carry the note. **A session adding a command
  with a large payload should reach for `.boxed()` rather than for
  `RUST_MIN_STACK`.**
- **Three recorders wait for quiet after the `Ack` now.** `ui_recording`,
  `ui_programmer` and `ui_show` drain deltas until 150 ms have passed without
  one, because the readback is asynchronous and a recording that stopped at the
  receipt would file an `ExecutorState` under the *next* step while the snapshot
  beside it already had it. Only a **delta** resets the window: the daemon
  publishes telemetry thirty times a second, so a drain that waited for silence
  on the socket would wait for ever. Any later recorder that issues a playback
  command wants the same three lines.
- **`jsdom` has no Pointer Capture API at all.** A strip's key captures the
  pointer so a finger sliding off a `Flash` still releases it; `ui/src/testing/
  setup.ts` shims the three methods, because without them the line throws in
  every test and works in every browser.
- **The store bars depend on a *subtree* now, not on the show.** Since the show
  document moves whenever a playback changes cue, an effect keyed on the show
  root would ask `Query::StorePreview` once per cue of a chase — S28 left that
  warning and S27 left it before that about `PatchConflicts`.
  `looks.ts::sequencesDocument` and `presetsDocument` are the dependencies, and
  structural sharing is what makes them stable. The test is *does not ask again
  when a playback advances a cue*.
- **The DMX Sheet's canvas has two things to count and they are not the same.**
  `looks.spec.ts` counts grid pixels of `colourOf(255)`; `executors.spec.ts`
  counts the **peak meter** at `INK.meter`, because its rig lights *one* channel
  of five hundred and twelve and the grid gives that a fraction of a pixel, so
  what lands on the canvas is a blend. Worth knowing before writing a third one.

Carried out of S28:
- **A playback that is already running does not follow a preset edit.** The
  show and the merge body both change, but the player keeps the levels it had
  faded to, so the new values reach the rig on the **next Go**. It is the safe
  behaviour for a show in progress and it is not what a console does;
  `ui/e2e/looks.spec.ts` says so in as many words at the point where it presses
  Go a second time. **S34** owns playback and is where a live update belongs, if
  it belongs anywhere.
- **`ui/src/show/looks.ts` is where a typed reading of the looks goes**, and it
  is the fourth such file beside `canvas/windows.ts`, `desk/session.ts` and
  `patch/patch.ts`. Its header carries the two rules that are easiest to break:
  what a store *would* do is asked and never counted here, and the value beside
  a `presetRef` is already the preset's — resolving the preset in a view would be
  a second answer to a question the show has answered.
- **There is still no *selected sequence*, and the assumption is marked in three
  places.** The sheets follow `selectedExecutor`'s sequence; `ARCHITECTURE_SPEC.md`
  §4.4 now names **S39** as the session that decides whether it should be a
  session field of its own. Whichever way S39 goes, `show/looks.ts::executorInForce`
  is the one reader that has to change.
- **A preset pool has no delete and no colour picker.** `Show::remove_preset`
  exists with its cue-link semantics documented and no command reaches it, and
  `Preset::color` is carried through a relabel but never chosen — the pool draws
  the swatch and cannot set it. Both are small and both are **S40**'s or S43's:
  a console reaches a preset through the command line, and that is the shell.
- **A sequence cannot be renamed, deleted or set to loop from the interface.**
  `Show::remove_sequence` exists and `Sequence::looping` is on the wire; neither
  has a command. `CreateSequence` names the sequence and `StorePreset` names the
  preset, so the *creating* half is covered and the *correcting* half is not —
  which is exactly the asymmetry S27 met with `PatchFixture` before
  `UnpatchFixture` and `RenumberFixture` closed it. **S40 or S43**, in the shape
  those two took.
- **`Query::StorePreview` is asked again whenever the show, the session or the
  programmer moves.** That is once per delta, which is fine at the rate a
  cue sheet changes and is *not* fine at playback rates — the same warning S27
  left about `PatchConflicts`. **S34**'s cue-index readback is the candidate that
  makes the show document move on every tick; a session that builds it should
  check that these have not become a question per frame.
- **`StoreMode` has one value and two readers.** `prism-domain`'s
  `merge_is_the_only_store_mode_this_build_has` and the browser's *every recorded
  preview is a Merge*. **S39** adds Override and Remove; both tests go red, and
  what they are protecting is the sentence on the Store button, which is the one
  thing on the screen that tells an operator what they are about to lose.
- **`ShowError` is no longer `Eq`.** `NegativeTime` carries the number that was
  refused, so an operator is told what was wrong with the time they typed rather
  than that something was. Nothing compares a refusal outside a test, and
  `ShowFileError` followed it.
- **The recording is 43 steps and five of them are refusals.**
  `crates/prismd/tests/ui_show.rs`; the guards find their steps by the `what`
  text rather than by index, so appending is safe and inserting is too. A session
  changing the shape of a message now regenerates **six** recordings and reads the
  diffs — `ui_recording`, `ui_session`, `ui_telemetry`, `ui_programmer`,
  `ui_patch` and `ui_show`.
- **The five older recordings differ only in `tickHz` when regenerated.** That
  field is a live measurement rather than a shape, so a regeneration that changes
  nothing else is noise and the files were left as they were. Worth knowing
  before reading a six-file diff: if a regeneration changes anything *but*
  `tickHz` and `initialSnapshot`'s tick rate, the shape moved.
- **`ui/src/testing/show-recording.ts` is where a shared recording is parsed.**
  Three test files read the S28 recording, and the first arrangement had one of
  them importing another — which runs that file's suite twice and counts it
  twice. Scenery belongs in `src/testing/`, which is excluded from coverage for
  exactly this reason.
- **One duplicated stylesheet selector was swept.** `.encbar-page` was defined
  twice (S35's own §7 note asked for the sweep and this is one of them); the two
  rules are merged and `App.css` now has no repeated simple selector at all,
  checked with `grep -oE '^\.[a-zA-Z0-9_-]+ \{' | sort | uniq -d`. That command is
  worth running before S43 looks at the file by hand.

Carried out of S35:
- **The console can page `programmerPage` past the end of a bank.**
  `prism_surface::binding::step_page` saturates at zero and has no upper bound —
  it cannot have one, because `prism-core` deliberately does not know how many
  parameters a bank has (S13). So holding `Zoom ▼` on a short bank runs the
  number up and the bar shows its last page until the number is walked back. The
  bar's own buttons stop at the end, so this is reachable only from the console.
  Bounding it properly means either the daemon knowing bank sizes — which S13
  decided against — or the bar sending a correcting `SetProgrammerPage` when it
  clamps. **S43 or S40**, and it is a line of code either way.
- **`ENCODERS_PER_PAGE` is four because four fits the band.** It is the
  interface's number and nothing else reads it, so a later screen — S29's Tauri
  shell on a real desk, or S30's second monitor — may want a different one. It is
  a single `export const` in `desk/programmer.ts` with the reasoning beside it.
- **The view menu offers *store as a new view at the end*, not *insert here*.**
  Under this session's ordering decision an insert would renumber every view
  above it, which would silently change what an F-key bound to `SelectView 4`
  reaches. If a later session wants a true insert, it is a fourth command and it
  has to answer that question first.
- **A stylesheet has no type system, and a duplicated selector is a silent
  override.** `.strip` was the executor strip *and* the footer's status strip;
  the later rule won and nothing could see it — not `tsc`, not `oxlint`, not a
  test. **S43**'s polish pass should sweep `App.css` for repeated selectors; it
  is now 1 100 lines and this will not be the last one.
- **The telemetry gate reads a busy machine as a code regression.** At 80–96 %
  CPU it reports 15–19 Hz with 138–152 frames lost against 30.4 Hz on a quiet
  one — the same shape a real fault has, and it cost an hour of bisecting the
  stylesheet before the load was checked. Any session taking a performance number
  should check the load first; §3 now carries the command beside the figure.
- **`sourceText` is the first thing to read `ProgrammerValueSource` and there is
  no way to *act* on it.** An operator can see that an encoder is holding a
  preset value, but not jump to the preset, and `presetRef` is carried through
  the reading and dropped. **S28** builds the preset pools and is where that
  becomes a gesture.
- **The recorded script is twenty steps now and four of them are refusals.**
  `crates/prismd/tests/ui_session.rs`; the guards find their steps by the `what`
  text rather than by index, so appending is safe and inserting is too. A session
  changing the shape of a message still regenerates all five recordings and reads
  the diffs — `ui_recording`, `ui_session`, `ui_telemetry`, `ui_programmer`,
  `ui_patch`.
- **Two hard-coded protocol counts had to move, and that is them working.**
  `session_commands.rs`'s *fifteen* and `command_application.rs`'s *thirty* exist
  so a command variant cannot be added without being given a home in both
  appliers. A session adding one will meet them both.
- **`eslint/no-console` is on now**, with `src/log/logger.ts` and the test trees
  exempted. A rule that `CLAUDE.md` states and nothing enforces is worth what the
  `console.log` it missed was worth.

Carried out of S44:
- **`AttributeType` is too narrow for the library the desk now carries.** UV,
  Cyan, Yellow, Magenta, Lime and Indigo have no attribute, so a **CMY fixture
  patches with its colour mixing missing** — the channels are occupied and
  nothing can drive them. This is the largest single loss in the conversion and
  it is a `prism-domain` change that reaches the merge, the encoder banks
  (`FeatureGroup::attributes`) and the generated bindings. A session of its own,
  and the numbers to size it are in `library/ofl.rs`'s counters.
- **714 of 2 798 modes are skipped**, all of them because the mode's channel
  list contains a matrix insert or a switching channel. Both make the layout
  depend on state, which a fixed `footprint` cannot express. Pixel bars and
  LED battens are the fixtures this loses, and they are common in the rooms
  this desk is for. Supporting them is a model change, not a reader change.
- **The installer is two scripts and nobody runs them yet.** S29's Tauri shell
  is where `tools/fetch-fixtures` becomes part of *installing PrismDMX*; until
  then a person clones and runs it by hand, and a desk without it starts on four
  profiles and says so in the log. S37's settings window is where *which library,
  and re-fetch it* would naturally live.
- **`profiles/fixtures/` is `.gitignore`d except for `SOURCE.md`.** A session
  that adds anything else under `profiles/` should check that pattern:
  `/profiles/fixtures/*` with a negation is deliberately narrow, and a file added
  beside the data would be ignored without saying so.
- **The recording pins its own library.** `crates/prismd/tests/ui_patch.rs`
  writes two OFL files into a temp directory and starts the daemon with
  `--fixtures`. Any later recording that touches the library must do the same:
  one made against whatever the developer had installed would fail on a fresh
  clone and change whenever upstream did.
- **A recorded step is found by what it is *for*, not by its number.** S44
  inserted three steps into the middle of S27's script and every hard-coded
  index in two languages went quietly wrong. Both `preview.test.ts` and
  `ui_patch.rs` now look steps up by their `what` text; later recordings should
  be read the same way.
- **`mirror/select.ts::pointerToken` exists and must be used.** Any pointer built
  from a *key* — a fixture type, a preset name, anything an operator can name —
  goes through it. The three readers that did not are the reason it exists.
- **`cargo llvm-cov` must be given `clean --workspace` between crates.** Two
  runs in one shell merge their profile data and report figures tens of points
  low. §3 carries the caveat beside the numbers.
- **The conversion counters are the way to read a re-import.** Raising the pinned
  revision in `tools/fetch-fixtures` and re-running
  `cargo test -p prism-core --test fixture_library -- --nocapture` prints what
  changed; a drop in the modes that convert is worth reading before it is
  committed.

Carried out of S27:
- **The largest thing this session did not build is a fixture library.**
  `prism_core::library` is four generic profiles and a way to copy one into a
  show. There is no import, no GDTF, no per-manufacturer modes, and **no way for
  an operator to author a profile of their own** — so a rig of real fixtures
  cannot be patched accurately yet, only approximated with a generic profile of
  the right footprint. `ARCHITECTURE_SPEC.md` §9 reserves `profiles/fixtures/`
  and nothing writes it. The protocol path exists and is the cheap half:
  `EmbedFixtureType` already carries a key, so a library read from disk needs the
  daemon to offer more keys and nothing else to change. **S37** is where it would
  naturally live (its *Show files* panel is the only place that reads and writes
  files), or a session of its own.
- **`Query`/`Answer` is a general mechanism with two variants in it**, and the
  next session to want a derived answer should add a third rather than compute
  one. The rules are in `docs/IPC_PROTOCOL.md` §5.2 and in
  `crates/prism-domain/src/query.rs`: it changes nothing, it is answered to the
  one client that asked, and it never returns state a client already mirrors.
  S37's settings window is the obvious next caller — *what would this output
  configuration carry* is the same shape of question.
- **`ui/src/patch/patch.ts` is where a typed reading of the patch goes**, and it
  is the third such file beside `canvas/windows.ts` and `desk/session.ts`. Its
  header carries the rule that is easiest to break: `address + footprint − 1` is
  **not** computed here, because it is the same arithmetic the overlap search
  runs. A later session wanting a *Channels* column should ask for a preview, not
  add a line.
- **`patch/live.ts` is `telemetry/driver.ts` for a list**, and the shape to copy
  for any later view with per-row live values (S28's cue sheet, S30's viewer):
  a canvas the size of the window, `scrollTop` read off the container each frame,
  and only the visible rows drawn. Four hundred rows cost twenty.
- **A fixture on a universe the daemon is not publishing is drawn as *absent*,
  not as dark**, and that is deliberately visible. It is also what a patched
  universe with **no output configured** looks like, which is **S33**'s to fix:
  today every network output is handed every universe and an Open DMX adapter
  carries exactly one, so a show patched across four universes on a laptop with
  one mock output has three of them showing absent. The sheet is now the place
  that makes it obvious.
- **The Patch window edits fixtures and nothing else.** `Fixture` also carries
  `position`, `rotation`, `invertPan` and `invertTilt`, and `PatchFixture`
  deliberately carries none of them (S1) — a repatch keeps them. So there is no
  way to hang a moving head in the 3D view or reverse its pan from the
  interface. **S30** needs the first two and S43 could take the inverts; either
  way it is a command that does not exist, in the shape S27's three took.
- **A patch edit is undoable and there is no Oops button.** The three new
  commands are journalled, and `Command::Oops` reaches them — the recording's
  last step is one — but nothing on the screen sends it. S26 left the same gap
  for the programmer. `UndoRecord::command` exists precisely so a button can say
  *Oops: Patch Fixture* rather than *Oops*, and that is S43's or S40's.
- **`PatchConflicts` is asked again whenever the show document moves**, which is
  once per delta and is fine at the rate a patch changes. A session that makes
  the show document move at *playback* rates — S34's cue-index readback is the
  candidate — should check that this has not become a question per tick.
- **The five recordings are now a set, and they all carry a `Snapshot`.** A
  session that changes the shape of a message regenerates all five and reads the
  diffs. The commands are:
  `cargo test -p prismd --test ui_recording -- --ignored`, and the same for
  `ui_session`, `ui_telemetry`, `ui_programmer` and `ui_patch`.
- **`prop_oneof!` costs stack per arm.** Adding a third message variant to
  `crates/prism-ipc/tests/framing.rs` overflowed a debug test thread, and only
  when the target's seven tests ran in parallel. Both message strategies are
  `.boxed()` now. The next session to add a message will meet it again.
- **`devicePixelRatio` and `resizeCanvas` live in `telemetry/painter.ts`**, and
  both canvases use them. Two copies of *what scale is this screen* is two
  answers, and the wrong one makes a canvas look blurred rather than making a
  test fail.

Carried out of S26:
- **`ui/src/desk/session.ts` is where a typed reading of the session goes**, and
  it is now the second such file beside `canvas/windows.ts`. `executorPage`,
  `selectedExecutor`, `encoderBank`, `programmerPage`, `programmerParamIndex`,
  `commandLine` and `pageStrips` — readers over the document, every string
  narrowed against `bindings/variants.ts`, nothing parsed and kept. S27 wants
  the *patch* the same way, and the fixture sheet's live values are the
  programmer's, which `desk/programmer.ts` already reads.
- **`FEATURE_GROUP_ATTRIBUTES` is generated, and a view must not write its own
  copy of anything like it.** It is in `variants.ts` beside the fourteen variant
  tables, it comes from `FeatureGroup::attributes()`, and `parameter_of` reads
  the same function. If S27 needs *which attributes a fixture type has*, that is
  a different question with a different answer — it is in the **show document**
  (`/fixtureTypes/<id>/attributes`), per fixture and per mode, and
  `desk/programmer.ts`'s `groupOf` is the reader that already walks it.
- **The command line is a parser and its grammar is in the module
  documentation.** `desk/console.ts`. S27 and S28 will want to extend it —
  `store 1 cue 2`, `group 3`, `preset 4` — and the two rules that must survive
  are: it never consults the show, and it never throws. The fuzz test is the
  second one; the first is why `9` on a rig without fixture 9 is a *daemon*
  refusal and a recorded step.
- **`ui/tests/fixtures/desk-recording.json` and `desk-rig.prism` are the fourth
  frozen fixture pair.** Written by
  `cargo test -p prismd --test ui_programmer -- --ignored`, opened by four
  non-ignored Rust tests on every commit. The rig is **dark at home** and has a
  moving head with attributes on four banks — which is the rig S27's fixture
  sheet wants too, and adding to it means regenerating the recording and reading
  the diff.
- **A recorded script's typed lines carry a line *number*, not just text.**
  `clear` pressed three times in a row is three lines. A browser grouping by
  equal text reads them as one, and the Rust guard asserts the numbers are
  consecutive and unreused. Any later recording with typed input wants the same
  field.
- **`--mock-surface` can now turn the jog wheel as well as press buttons.**
  `turnConsoleWheel(path, detents)` in `ui/e2e/daemon.ts` writes
  `B0 3C <sign|magnitude>` — §2.1's encoding, written out by hand, and §2.7's
  measurement that this wheel only ever sends ±1.
- **Both bars take height from the same column as the canvas and the canvas
  shrinks.** `.encbar` is 3.1 rem and `.execbar` is 6.4 rem, both
  `flex: 0 0 auto` with a *fixed* height — a bar sized by its contents would
  move every window on the screen the moment an executor was assigned. The
  end-to-end suite reads `scrollWidth − clientWidth` and
  `scrollHeight − clientHeight` off the document, the canvas **and both bars**
  and demands eight zeros.
- **`SelectProgrammerParam` has no absolute form and does not need one.**
  Clicking an encoder composes the steps; the bar stops offering *next* at the
  end of the bank, because `prism-core` deliberately does not know how many
  parameters a bank has and an index past the end leaves the wheel turning
  nothing. `ARCHITECTURE_SPEC.md` §4.4 records it.
- **`percentOfLevel` and `levelFromPercent` live in `desk/level.ts` and there is
  one of each.** `at 50` is 32 767 — truncating, so nothing rounds up past what
  was asked for and `at 100` is exactly full. A second conversion somewhere else
  would disagree in the last digit and only show up in a cue somebody stored.

Carried out of S25:
- **The layout question is settled and the answer is `cadence`, not ownership.**
  `ui/src/canvas/drag.ts` is the whole of it: the daemon owns the rectangle, the
  screen may show the pointer's rectangle while the button is down, a command
  goes out at most every 33 ms plus one on release, and **the local rectangle is
  dropped the instant the button comes up**. S26's faders and encoders have the
  identical problem one layer down — a fader dragged in the interface is a
  stream of `SetExecutorMaster` — and this is the shape to copy. It is also the
  shape the *surface* already uses in the other direction (S21's coalescing).
- **`canvas/windows.ts` is where a typed reading of the session goes.**
  `openWindows`, `focusedWindow`, `activeViewId`, `storedViews` — readers over
  the document, never a parsed copy, and every string narrowed against
  `bindings/variants.ts`. S26 wants `executorPage`, `selectedExecutor`,
  `encoderBank`, `programmerPage` and `programmerParamIndex` the same way; they
  belong beside these, and `mirror/select.ts` is the layer under both.
- **`prismd::surface::context_of` and the encoder bar must agree, and now
  somebody can check.** S22 left the warning: `parameter_of` walks
  `AttributeType::ALL` filtered by the encoder bank, and if S26's encoder bar
  orders its parameters differently the jog wheel turns something other than
  what is highlighted. With `--mock-surface` that is now testable end to end —
  turn the wheel, watch the interface.
- **`--mock-surface <PATH>` is the console for tests, and `pressConsole` in
  `ui/e2e/daemon.ts` is how to press one.** Bytes from `docs/MCU_MAPPING.md`
  §2.1 written out by hand; never ask the profile which note to send, because
  that is asking the code under test what to press.
- **`ui/tests/fixtures/session-recording.json` is a third frozen fixture.**
  Written by `cargo test -p prismd --test ui_session -- --ignored`, opened by
  three non-ignored Rust tests on every commit, and it carries **both**
  MessagePack encodings of every command with a float in it — because
  `@msgpack/msgpack` writes a whole number as an integer and `rmp-serde` writes
  an `f64` as a float64, and the daemon accepting both is a claim about
  `prism-ipc` rather than something a browser can check. S26 adds commands with
  no floats, so it can compare bytes directly again — but if it ever adds one,
  this is the pattern.
- **`OpenWindow` carries no geometry, so every window opens at 0, 0 on top of
  the last.** Left alone deliberately (decision log): a client that offset its
  own new windows would be writing session state on its own initiative, and two
  clients doing it would race. If it becomes a nuisance the fix is geometry on
  `OpenWindow` or a cascade in the daemon — not in a client.
- **There is no `z-index` in the stylesheet and there must not be one.** The
  stacking order is the order of `openWindows`, which `FocusWindow` changes in
  the daemon, so document order is the only thing allowed to express it. A
  `z-index` would be a second source of truth for something the session decides,
  and `canvas.test.tsx` asserts the drawn order is the daemon's rather than the
  sorted one.
- **Nothing outside the canvas scrolls, and that is now checked in a browser.**
  `index.css` is `height: 100%` and `overflow: hidden`; the end-to-end suite
  reads `scrollWidth − clientWidth` off the document element *and* the canvas
  with four windows open and demands zeros. S26's executor and encoder bars take
  height from the same column, so they have to fit — a bar that does not fit is
  a layout decision, not a scrollbar.
- **The window bodies are placeholders on purpose, and four of them say so by
  name.** `content.tsx` switches on `WindowType` without a wildcard, so a window
  type added to `prism-domain` is a compile error there. S27 fills in the
  fixture sheet and the patch; S28 the sequences, cues and pools.
- **The telemetry panel now lives in a `DmxSheet` window** and is sized by it.
  Both S24 gates were re-measured through the window system and both hold
  (§3). If S26 puts more on the screen, `npx playwright test telemetry` is the
  spec that would notice, because it asserts a p99.

Carried out of S24:
- **Telemetry must stay out of React, and there are now two guards.** S23's:
  `deskEvents` has no `onTelemetry` and a hundred frames cost the store zero
  notifications. S24's: `ui/src/telemetry/render.test.tsx` counts **React
  commits** over 300 frames of 64 universes and requires the number not to move.
  A view that wants levels reads them in the paint loop, from
  `TelemetryFrameView`; it does not ask a hook for them, because there is no hook
  to ask.
- **The telemetry channel arrives as a context carrying a *device*.**
  `TelemetryContext` holds a sink, and optionally a scheduler, a clock and a way
  of building a surface — which is how the tests replace all three. Nothing in it
  ever changes. S25's window system should hand the same object down; a second
  provider per window is fine, because a provider publishes nothing.
- **The frame budget is measured in Chromium, not in jsdom**, and the way to
  re-run it is `npx playwright test telemetry` (§3). If S25 or S26 puts more on
  the canvas, that spec is where the cost shows up — it asserts a p99, so a
  regression that only bites on one frame in a hundred still fails it.
- **`ui/tests/fixtures/wide-rig.prism` is a committed show file**, and the only
  way to have 64 universes of telemetry in a browser: the daemon publishes the
  universes a show *patches*, not the ones the layout has room for. It and
  `telemetry-recording.json` are both rewritten by
  `cargo test -p prismd --test ui_telemetry -- --ignored`, and both are opened by
  non-ignored Rust tests on every commit. A session that changes the show format,
  the telemetry layout or `TelemetryFrame` should expect to regenerate them — and
  to read the diff, because that diff *is* the change.
- **`LevelSurface` is the drawing seam, and S25's canvas should use it or
  something like it.** Five operations, no canvas types, `RecordingSurface` on
  the other side. It is what lets `painter.test.ts` assert all 32 768 pixels
  rather than that something was drawn, and it is why the whole renderer is
  covered in a runtime with no rasteriser in it.
- **Client-local state has its first real instance and it stayed local.**
  `ARCHITECTURE_SPEC.md` §4.2 names zoom, scroll and camera; the canvas's pixel
  size is the same kind of thing, and it lives in the element and a ref with no
  command sent about it. S25 has the harder version of this question — a window's
  *position* is session state (§4.1) and its scroll offset is not.
- **`tsconfig.node.json` now includes the `DOM` library**, for the bodies of
  `page.evaluate`, which run in the browser. Everything else under that config is
  Node and must not touch a DOM global; the tooling cannot enforce that any more,
  so it is written down in the file.
- **The panel is a diagnostic, not the level view S25 will want.** It is one
  section in a column: a grid, a peak meter per universe and a line of numbers.
  When windows exist it belongs in one, sized by the window rather than by `vh`.
  What should survive the move is the arrangement, not the layout.

Carried out of S23:
- **The telemetry channel already arrives, and it must stay out of React.**
  `ipc/telemetry.ts` is a sink with no way to notify anybody, `deskEvents` has
  no `onTelemetry` on purpose, and `telemetry.test.ts` asserts that a hundred
  frames cost the store **zero** notifications and leave the state object
  identical. S24 decodes `prism_ipc::TelemetryFrame` — a 16-byte header
  (`"PTLM"`, layout version, reserved, universe count, sequence, little-endian)
  and one 514-byte section per universe — and renders it on a canvas. A frame
  announcing a layout version this build does not know is **dropped**, not
  guessed at.
- **`TelemetrySink.clear()` is called when the connection goes**, and the reason
  is the same one the store drops its documents for: a picture of the rig from a
  daemon that has stopped is exactly as stale as a fader value from one.
- **The recording is frozen and its regenerator is `#[ignore]`d.**
  `cargo test -p prismd --test ui_recording -- --ignored` rewrites
  `ui/tests/fixtures/daemon-recording.json`; the non-ignored tests beside it
  replay the committed file through `prism_core`'s mirror on every commit, so a
  wire-format change fails in Rust rather than going stale in `ui/`. A session
  that changes `Delta`, `Snapshot` or the framing should expect to regenerate it
  — and to look at the diff, because that diff *is* the protocol change.
- **`ui/src/bindings/variants.ts` is generated and is where run-time vocabulary
  comes from.** Fourteen tables, derived from the unions `ts-rs` wrote. A view
  that needs the list of window types or attribute types has it already; a view
  that hand-writes one is reintroducing the drift this file removes.
- **Selectors must be stable and pure**, because `useDesk` caches per state
  object *and* per selector identity. Module scope or `useCallback`. A selector
  that builds a new object every call defeats the whole arrangement, and the
  test that proves the arrangement works (`store/context.test.tsx`) is the one
  that would go quiet if it were broken.
- **The pointers the views read are `prism-core`'s document shapes, not a
  model.** `/session/executorPage`, `/session/openWindows`, `/fixtures`,
  `/executors/<id>/isActive`. S25 will want typed accessors over the session;
  they belong beside `mirror/select.ts`, and they should stay *readers* of the
  document rather than a parsed copy of it — a parsed copy is a second model to
  keep in step, and RFC 6902 operations only mean anything against a root.
- **There is one interactive control in the interface and it is a
  demonstration.** The command line sends `CommandLineInput` and displays the
  daemon's own `commandLine` underneath. S26 owns the real one, including the
  parser; what should survive is the shape — local input in the input, the
  daemon's fact in the readout, nothing optimistic in between.
- **The end-to-end suite is a separate CI job and it compiles a daemon.**
  `ui-e2e` installs Chromium and runs `cargo build -p prismd`; a spec that needs
  a daemon calls `startDaemon(port, dataDir?)` from `ui/e2e/daemon.ts`, which
  finds it by reading the lock file (§2.2) rather than by sleeping. Use a port
  of your own, not 7373 — a developer may have a daemon running.
- **`vite preview` must be told `--host 127.0.0.1`** or it binds `localhost`,
  which can resolve to `::1` and leave Playwright waiting on an IPv4 address
  nothing is listening on.

Carried out of S22:
- **There is no MIDI backend yet, and `SurfacePort` is where it goes.**
  `prismd::surface::SurfacePort` is a two-method trait with one implementation
  (`MockSurfacePort`). Attaching a real X-Touch means choosing a crate — `midir`
  or something under it — and deciding which crate may hold it, because
  `ARCHITECTURE_SPEC.md` §10.1 allows neither `prism-surface` nor `prismd` any
  `#[cfg(target_os = …)]`. The tool of S20 is outside the workspace for exactly
  this reason and is the precedent to weigh against a dependency whose whole
  purpose is to hold the split (which is how `thread-priority` got in).
- **The protocol has no executor-button command, and three rows of §4.1 want
  one.** `XFade`, `On`, `Flash`, `Toggle` and `LearnSpeed` are executor
  *functions* — show data — and the surface can only send commands. One command
  that presses an executor's button and lets the executor decide closes all
  three rows, and it is the same session that owes a tap against a speed master.
  Nothing was invented in the meantime; the gap is in `docs/MCU_MAPPING.md`
  §4.2.1 and in the decision log.
- **The daemon paints only what §4.1 gives it**: the current page's executor
  masters onto the faders, their sequence names and master percentages onto the
  scribble strips, the active ones onto the Select LEDs and the unsaved-changes
  flag onto the Save lamp. Colours are deliberately not driven — an executor has
  no colour field, and §2.3 says a strip that rounded itself onto black is one
  whose text cannot be read. S26 owns what else belongs there.
- **`SurfaceContext` is the seam the UI session work will meet.** If S23–S26
  change what the session holds, `prismd::surface::context_of` is the one place
  the surface reads it, and `parameter_of` — the encoder bank plus the parameter
  index over `AttributeType::ALL` — is a guess at what S26's encoder bar will
  show. If the encoder bar orders its parameters differently, these two must
  agree or the jog wheel will turn something other than what is highlighted.
- **The gate target is the shape later daemon tests should copy.**
  `run_until` runs the daemon in 5 ms slices and looks at the desk between them,
  because the surface is polled *inside* `Daemon::run` — and every wait has a
  ten-second deadline, per S16's lesson about a CI job that never ended.

Carried out of S21:
- **`SurfaceEvent` is the seam S22 binds.** `Button`, `Touch`, `Moved` (a level,
  0…65535, already scaled), `Encoder` and `Jog` (parameter steps, already through
  their curves). Layer 3's job is the map from those to `Command`, and it should
  need no arithmetic at all: everything that needed a measurement to get right has
  been done one layer down.
- **The profile already refuses the button that must never be bound.**
  `McuProfile::is_reserved` names SMPTE/Beats and layer 2 **drops its presses**,
  counting them. S22's loader should still refuse a profile that binds it —
  belt and braces, and the error message is what tells the person who wrote the
  profile why.
- **Ownership is on the profile, as data.** `McuProfile::permanent` and
  `SurfaceMode::{Dedicated, Shared}`. A binding table may name anything (§4.3:
  the full surface is one button away); what `Shared` changes is only which LEDs
  are *driven*. S22 should not re-implement that check in the binding layer.
- **The clock is an argument all the way up.** `push(bytes, now, sink)` and
  `pump(now, sink)`. Whatever S22 adds keeps that shape, and the surface thread
  in `prismd` is the one thing that reads a real clock.
- **Pump at least as often as `SurfaceTiming::min_gap`.** The pacing is enforced
  against the caller's clock, so a surface thread that pumps every 10 ms sends at
  most 100 messages a second and a resync burst takes a second and a half. One
  millisecond is the intended cadence; it is also the floor that keeps the desk
  out of §2.7's failure.
- **A full resync burst is 156 messages, ~156 ms.** Worth knowing before a
  binding table triggers one on every profile reload: reloading a profile should
  not invalidate the shadow model unless the *picture* changed.
- **The colour quantiser is `prism_surface::quantize` and it never returns
  black.** Only an exactly black `RgbColor` does. An executor with no colour
  wants `StripColor::White`, not `Off` — §2.3, and the reason is that black is
  the backlight off and its text cannot be read.

Carried out of S20:
- **Pace the outbound path, and treat §5.2's 30 Hz as a safety limit rather than
  an optimisation.** The X-Touch can be made to **stop transmitting altogether**
  while it goes on receiving perfectly: no buttons, no faders, no replies, and
  only a power cycle brings it back. It took a burst of 63-byte scribble strip
  writes with device-query replies outstanding, it happened twice out of two
  attempts, and each half of the recipe is harmless alone. A minimum gap between
  outbound messages belongs in the send queue.
- **Never poll the handshake.** The device query is the only reply this surface
  generates, and the failure above needs replies in flight. Use it once at
  connect, if at all.
- **Silence is a fault state, and §5.3 does not cover it.** That rule assumes the
  device *disappears*; here the port stays open and writes still land on the
  display. A surface layer that only watches for disconnection will show a green
  light beside a dead console. Notice a desk that has gone quiet, and say
  **power-cycle it** — reconnecting is the one thing that will not help.
- **Scale faders against `McuProfile::fader_step`, not against `FADER_MAX`.** The
  faders report in steps of 4 and stop at **16380**; dividing by 16383 gives
  99.98 % for a fader against its end stop, and an executor master that cannot
  reach full is a fault an operator will find and nobody will be able to explain.
  `max_reported_position()` is the number to use.
- **Two acceleration curves, not one.** A V-Pot spun hard carries 1…8 detents per
  message; the jog wheel sends ±1 and nothing else however fast it goes. One
  shared curve would be wrong about one of them. The codec passes the magnitude
  through unchanged, which is correct for both — the interpretation is S21's.
- **A moving fader is reported every 19.8 ms**, which is the real bound on
  `ARCHITECTURE_SPEC.md` §4.3's first row. Round-trip through the surface is
  0.71 ms median, so nothing in the transport needs optimising.
- **The shadow model can hold text and colour separately.** Writing text does not
  reset the colour — measured, not assumed — so the two diff independently. And
  the colour message is **all eight strips or nothing**: a message with any other
  number of colour bytes is ignored outright, so there is no per-strip update to
  find.
- **Skip the lamps that do not exist.** `McuProfile::unlit_buttons` names the two
  buttons with no LED (Name/Value, SMPTE/Beats), and the encoders have no lamp
  under them at all, so ring bit 6 lights nothing on this device.
- **Meters need refreshing faster than the documentation implies** — they fall to
  empty in under a second, not the 2–4 s a decay of ~300 ms per division would
  give. §5.2's permission to drop them first still holds: a dropped meter falls
  rather than freezing.
- **The nearest-corner colour quantisation is still S21's** (§2.3, hue-first,
  because a pastel is still the colour it is a pastel of) and the eight values it
  maps onto are now confirmed: bit 0 red, bit 1 green, bit 2 blue, and bits above
  that are masked away by the surface — there is no inverted variant in MC mode.
- **In shared operation only part of the panel is permanently ours.** The desk is
  to be run in the X-Touch's combined **Xctl+MC** mode, driving the venue's sound
  console *and* PrismDMX at once. Only what Xctl leaves unused reaches MC
  permanently — in practice **the transport section (91–95) and the jog wheel
  (CC 60)** — and everything else follows the operator's switch between the two
  hosts. **Nothing is unreachable**: pressing SMPTE/Beats gives PrismDMX the whole
  surface. What the permanent set buys is *no switching*, which is what matters
  mid-show, so those five buttons and the wheel want live-show functions and free
  assignments rather than a transport metaphor. D7's paging and D8's `SelectView`
  cost a mode change, which is fine for setup-shaped work and a reason not to put
  anything time-critical there. **S21's part:** the shadow model must not drive
  LEDs for controls MC does not currently hold, and the ownership set belongs as
  **data on the profile**, the way `unlit_buttons` is, rather than as a condition
  threaded through the diffing. `docs/MCU_MAPPING.md` §4.3 has it in full. This is
  the operator's account and not a measurement — §7's checklist records it as an
  open item.
- **A tap for speed has nowhere to land yet.** The intended use of those
  always-available keys includes a tap tempo against different **speed masters**,
  and neither exists: `LearnSpeed` is only an executor-button function today, and
  speed masters are named in `docs/DMX_MERGE.md` §4 item 3 with nothing
  implementing them. S22 must not invent the command — the session that builds
  speed masters owes the type, the command and the entry in the transport row's
  function list.
- **Never bind SMPTE/Beats (note 53).** In the shared mode it is the button that
  switches the surface between the two hosts, so binding it strands the operator
  away from their sound desk. Unbound in *every* mode, so one profile is safe on a
  desk whose mode nobody has checked; S22's loader should refuse a profile that
  binds it rather than merely defaulting away from it.
- **`tools/xtouch-probe` is there when a new question needs the desk**, and
  §3.4 has the commands. Two rules from using it: run one step at a time so the
  person watching is told exactly what to expect, and **never verify a map by
  printing a press order** — light one LED and ask for the button that lit, which
  checks both directions at once and cannot be thrown off by the order somebody
  walks the panel in.
- **A capture is evidence and evidence still needs one end written down.** The new
  replay target asserts that every recorded message re-encodes to the bytes the
  desk sent — and the S19 fader mutation leaves that **green**, on real data,
  because decode and encode still agree with each other. What catches it is the
  measured *properties* (multiples of 4, maximum 16380) and eleven captured
  messages whose meaning was worked out by hand. Any future device inherits both
  halves.

Carried out of S19:
- **`profile::X_TOUCH` is the whole of what S20 edits**, and
  `tests/round_trip.rs`'s hand-written byte table is the second half of it. The
  two are deliberately independent: a table that agreed with itself whatever it
  said would make the verification session meaningless. **Do not "simplify" the
  test to compute its bytes from the profile** — a mutation check proved that
  version passes with the fader's two bytes swapped.
- **`X_TOUCH.verified` is asserted in a `const` block.** Setting it to `true`
  without changing that assertion stops the build, which is the intended way for
  S20 to be reminded of what it is claiming.
- **Time is an argument, not a clock.** `MidiDecoder::push(bytes, now, sink)`.
  S21 owns the clock on the surface thread and passes the instant down; nothing
  below it should hold one. Same for `poll(now)`, which is what an idle port
  needs so a half-arrived message eventually ages out.
- **The counters separate a broken cable from a wrong profile.**
  `wire.discarded()` is malformed input; `unmapped` is a well-formed message
  this profile does not describe. S20 will live in the second one, and S26's
  status panel should show them apart.
- **Layer 1 knows no domain type**, and `prism-domain` is in the manifest
  unused. S21 may need it; S22 certainly will. Keep `ControlEvent` `Copy` and
  free of owned fields — it crosses a thread boundary on the path §4.3 budgets
  in milliseconds.
- **`Feedback` decodes as well as encodes**, which is what let the outbound
  round trip be a claim about bytes. S20's MIDI monitor capture can be compared
  against it directly, and S21's tests can act as the surface.
- **Three questions the sources could not settle went onto §7's list** rather
  than being guessed: what a 7-segment `0` draws, whether a strip index above
  seven is ignored, and what a *Device Ready* contains. Inbound SysEx is
  therefore counted (`sysex_ignored`) and not interpreted.
- **What S21 must add, and this crate deliberately did not:** touch suppression
  (§5.1 — no outbound pitch bend while a fader reports touch, one resync 150 ms
  after release), 30 Hz coalescing against a shadow model (§5.2), the send
  priority faders → LEDs → LCD → meters, the resync burst on reconnect (§5.3),
  and the **nearest-corner colour quantisation** — hue-first, per §2.3, because
  a pastel is still the colour it is a pastel of. A minimum gap between outbound
  messages may also belong there: two independent projects report the X-Touch
  losing the tail of a burst, and §7 measures it.

Carried out of S18:
- **`MockOutputHandle::timeline()` is the shape every later gate should be
  written against.** `FrameRecord { at, universe, data }` — the time is what
  turns a list of frames into an assertion about a stage. `frames()` is still
  there and is now a projection of it.
- **"No gap" is two claims** — no silence over 250 ms, and the look never
  changing by itself — because a mutation check proved a timing-only gate passes
  a daemon that blacks out on disconnection. Any later gate about output
  continuity inherits both halves.
- **`ServerHandle::clients()` names the connections**, so `stats(id)` is
  readable from outside; `Daemon::server()` hands the whole server to a status
  panel (S27) or a test.
- **A killed client is a task that is aborted**, not `Client::disconnect`. Every
  later resilience test should kill rather than disconnect, and should then
  assert `client_count()` returns to zero — that is S16's leaked pipe, one level
  up.
- **A backpressure test needs a wide show.** Telemetry is 514 bytes per patched
  universe; against a two-universe rig nothing is under pressure for seconds.
  `common::write_wide_show` is the fixture, and the assertion is *more dropped
  than delivered* rather than a count, because CI measures this over a named
  pipe and a Unix domain socket.
- **`prismd` needed no change to pass its own gate.** The next session to touch
  the daemon should know the resilience suite is a *regression* suite now: it
  will notice anything that couples a client's life to the rig.
- **Run a new timing-sensitive test under deliberate CPU load before pushing
  it.** Six busy processes beside `cargo test -p prismd` reproduced both of the
  failures CI found — one of them in a test that had been green since S17 — in
  two runs out of three, and the same load is what confirmed the fixes. It costs
  a minute and it is the only thing that stands in for a two-core runner.
- **The daemon reports an output's health by *polling* it every 500 ms.** A
  state that appears and disappears between two polls was never there as far as
  any client is concerned. That is what polling is rather than a defect — the
  light converges on the current health — but a test that toggles an output has
  to hold each state across an interval, and S26's UI has to expect the same.
- **`a < b` between two measured numbers is a coin toss unless a margin is
  stated.** Both halves of the backpressure comparison now carry one.

Carried out of S17:
- **`Daemon::recorded_outputs()` is what S18's gate is written against.** Every
  `--mock-output` the daemon opened hands back a `MockOutputHandle`, and the
  frames it was given are in order and complete. *Assert the output frame
  sequence has no gap across the whole run* is a statement about that list.
- **The daemon tests take turns behind `common::one_daemon_at_a_time`.** A
  daemon owns a real-time tick thread, and several in one process measure each
  other rather than the daemon — S6's finding one level up. Any new target that
  starts a daemon takes the same turn.
- **A rebuild replaces the whole `MergeBody`,** built on the core thread and
  handed over through an atomic and a `try_lock` that never blocks. The tick
  neither allocates nor frees to accept it. It also **stops every playback**,
  which is the debt S28 inherits.
- **The frame layout is the desk's whole universe range**, fixed for the
  publisher's lifetime; telemetry is filtered back down to the universes the
  show patches. `--universes` changes the range.
- **`prismd` has no `#[cfg(target_os = …)]`,** and CI runs its tests in the
  Linux job to keep that true. The three places it would have needed one are in
  the decision log.
- **`Delta::ExecutorState` carries no cue index the daemon did not already
  have.** There is no channel back from the tick yet; S18 or S26 builds one, in
  the shape `TickHealth` already has.
- **The CLI is the daemon's whole configuration** (`--help` is the
  documentation), and S29's shell will spawn a daemon by passing the same
  arguments. `--run-for` exists so a daemon can be started in a test or a CI job
  without being interrupted by hand.

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

Paste the block below into a fresh session. It is deliberately self-contained:
it assumes no memory of this conversation and no knowledge of the project.

**The next session is S48, and not S29.** The running order in
`IMPLEMENTATION_PLAN.md` puts it at number 15, and it has what it was waiting
for: S45's *one sequence, one playback*. Without that, a tracking state per
playback of the same cue list would be two answers to the question S48 exists to
give one answer to. It is the deepest thing the owner's punch list turned up —
today the output at a cue depends on how you got there, so a cue cannot be
rehearsed — and it is the last **XL** in the plan.

**S43 is still open** while this is written: its third movement ends when the
owner calls the interface fit for a pre-release. Whoever finishes it should read
§2.41 first. That does not block S48, which touches no interface except the cue
sheet.

**The punch list has one entry left open and it is not S48's**: **B36**, added by
the owner after S45 — the crossfade fader should not spring back, and there should
be two modes. It is an executor question and belongs to whoever picks it up next;
S48 will change what a crossfade *lands on*, not how the fader behaves.

**One thing is left open on purpose and is written up in §5**: the ten-minute
tick-deadline gate has not been re-measured since S45, because the machine it
would be measured on cannot measure it. The recipe and the control are there, and
**S48 will have to face it**, because it adds the ninth allocation-free path and
puts a tracking state in the tick's neighbourhood.

---

```
PrismDMX — Session S48: `prism-domain` + `prism-core` + `prism-engine` — Tracking

Projektverzeichnis: C:\Users\Milan\Prismdmx

Der Daemon hält den Zustand und treibt DMX ohne jeden Client (D2, S18); das Pult
bedient ihn ohne Oberfläche (D11, S22); die Oberfläche ist seit S23–S28, S34,
S35, S39, S40, S43, S44, S45 und S46 ein vollständiges Pult. Seit **S45** gilt:
ein Executor ist ein **Griff** auf das Playback einer Cue-Liste, und die Liste
hält den Zustand — `PlaybackId` ist die Nummer der Liste, nicht die des
Executors.

**Was fehlt, ist die Antwort auf *wo bin ich eigentlich*.** Ein `Cue` ist eine
Liste von `CuePart`s und **dünn besetzt von Natur aus**: er trägt, was im
Programmer stand, als er gespeichert wurde, und sonst nichts
(`docs/DMX_MERGE.md` §3). Alles, was ein Cue nicht nennt, bleibt dort, wo der
*vorige* Cue es gelassen hat — das ist **Tracking**, und es ist die richtige
Voreinstellung. Was fehlt, ist die andere Hälfte: nichts rechnet aus, was
Tracking für einen Cue bedeutet, den man **außer der Reihe** anspringt.

Heute heißt das: Cue 1 stellt den Wash auf 50 %, Cue 5 auf 100 %, Cue 3 nennt ihn
gar nicht. Wer 1→2→3 durchgeht, sieht 50 %. Wer auf 7 springt und dann Goto 3
macht, sieht 100 %. **Derselbe Cue, zwei Ausgaben, und welche man bekommt, hängt
davon ab, wo man vorher war.** Ein Operator kann Cue 3 nicht proben, und eine Show
kann nicht weitergegeben werden. Der Eigentümer hat das am 2026-08-27 aus der
Handprobe von S43 gemeldet.

Bitte lies zuerst in dieser Reihenfolge, bevor du irgendetwas änderst:

1.  CLAUDE.md                     — verbindliche Qualitäts-, Architektur- und
                                    Teststandards: Zero-Crash-Invariante des
                                    DMX-Threads, ≥ 85 % Coverage global, > 95 %
                                    auf engine/, programmer/ und protocols/,
                                    „kein Test darf ein Gerät brauchen", und die
                                    UI-Regel (wie ein Geräte-Bildschirm; nichts
                                    scrollt außerhalb der Canvas)
2.  IMPLEMENTATION_PLAN.md        — **die Definition von S48**. Die sieben
                                    Deliverables und die sieben Exit-Kriterien
                                    dort sind die Anforderung, wortwörtlich.
                                    Danach die Laufreihenfolge am Ende (S48 ist
                                    Nummer 15) und die Konvention
                                    „Session-Nummern sind Identität, die
                                    Reihenfolge ist der Fahrplan"
3.  docs/DMX_MERGE.md             — §3 (ein Cue ist dünn besetzt und **warum**),
                                    §4 (die Merge-Reihenfolge, in die das
                                    Ergebnis fällt)
4.  crates/prism-domain/src/sequence.rs
                                  — `Cue`, `CuePart`, `CueProperty`, und seit
                                    S45 der Playback-Zustand der Liste
5.  crates/prism-engine/src/cue.rs und player.rs
                                  — `Player::goto`, die Fades, und was ein Tick
                                    von einem Cue liest. **Hier fällt heute die
                                    Antwort aus, die von der Vergangenheit
                                    abhängt**
6.  crates/prism-core/src/programmer.rs
                                  — was *überschreibend* heißt (S43/B1): genau
                                    die Unterscheidung, die ein Cue tragen muss
7.  PROGRESS.md                   — §2.42 (S45 — ein Playback pro Liste), §2.43
                                    (S46 — was gerade gebaut wurde), §2.33 (S39
                                    — Store-Modi und der Update-Zustand), §7 mit
                                    allen „Carried out of"-Listen, §3 (die
                                    Zahlen, die weiter stimmen müssen), §5 (die
                                    offenen Verifikationen — **eine davon ist
                                    der Zehn-Minuten-Gate, und sie gehört dir**)
8.  ARCHITECTURE_SPEC.md          — §3.1 (was in den Tick darf und was nicht),
                                    §6 (das Datenmodell), §5 (Undo/Oops)
9.  docs/PRERELEASE_PUNCHLIST.md  — der Wortlaut des Eigentümers, und **B36**,
                                    das offen ist und **nicht** deins

Aufgabe: Session S48 umsetzen.

**Die Deliverables und Exit-Kriterien in `IMPLEMENTATION_PLAN.md` unter S48 sind
die Anforderung, vollständig und ohne Ausnahme.**

## Die Entscheidungen, die diese Session treffen und begründen muss

**Erstens: wo der Tracking-Zustand wohnt.** Er ist **abgeleitet** — aus den Cues,
die die Show trägt — und darf deshalb nicht in der `.prism`-Datei stehen: eine
Datei, die den aufgelösten Zustand speicherte, wäre eine Datei, die man durch das
Bearbeiten von Cue 2 nicht mehr korrigieren kann. Das ist dieselbe Regel, aus der
`Query::DarkUniverses` (S37) und `Query::ArtNetNodes` (S46) entstanden sind. Wo er
*stattdessen* liegt, wann er neu gerechnet wird und wer ihn liest, ist die
Entscheidung — und sie gehört mit dem Grund in den Entscheidungslog in
PROGRESS.md §6.

**Zweitens: was ein Cue über ein Attribut *sagt*.** Heute gibt es zwei Zustände —
der Cue nennt das Attribut, oder er nennt es nicht. Der Plan verlangt drei:
*setzt*, *erbt*, und *setzt und nimmt es am Ende zurück* (cue-only). Das ist eine
Änderung an einem Typ, der in der Show-Datei steht, also braucht das neue Feld
`#[serde(default)]` **und** der Default muss die Bedeutung sein, die eine alte
Datei schon hatte.

## Randbedingungen

- **Der Tick bleibt allokationsfrei.** Heute sind es **acht** Pfade
  (`crates/prism-engine/tests/tick_allocations.rs`); diese Session fügt den
  **neunten** hinzu. Der Tracking-Zustand wird **nicht** im Tick gebaut — er wird
  gelesen. Bauen ist Arbeit des Core-Threads, bei Laden und bei jeder Cue-Änderung.
- **Auf der Frame-Folge zusichern, nicht behaupten.** Das Kernexit-Kriterium ist
  eine **byte-identische** Frame-Folge zwischen *durchgehen* und *hinspringen*.
  `MockOutputHandle::timeline` ist die Form, die S18 dafür etabliert hat.
- **Ein neues Kommando braucht eine Heimat in allen drei Appliern** und in den
  fest verdrahteten Zählungen (heute **63**) — aber prüfe zuerst, ob es eines
  braucht: S38 wollte zwei neue Dinge sagen und hat **kein** Kommando
  hinzugefügt, S45 eines und hat es begründet, S46 keines.
- **Das Wertbaum-Budget ist gemessen und knapp**:
  `crates/prism-domain/src/wire.rs::a_generated_wire_value_fits_in_a_test_thread`
  fällt bei **30 992** Byte. Wer es reißt: ein *Feld* boxen
  (`crate::arb::boxed()`), und für ein **weites Unit-Enum** stattdessen
  `crate::arb::arbitrary_from_list!`. Niemals `RUST_MIN_STACK`.
- **`ui/src/bindings/` wird aus Rust erzeugt** (`cargo test -p prism-domain`),
  nicht von Hand geschrieben. Wer einem String-Enum eine Variante **mit Feld**
  gibt, schaut vorher in `ui/src/bindings/variants.ts` — S45 hat gelernt, warum.
- **Eine neue Nachricht braucht ihren Leser in `ui/src/ipc/protocol.ts`**, und
  am besten einen Test, der sie durch den echten Decoder schickt.
- **Ein neues Feld auf einem persistierten Typ braucht `#[serde(default)]`**,
  sonst öffnet keine ältere Datei mehr — und der Default muss die Bedeutung
  tragen, die die alte Datei schon hatte. Wenn ein Feld *umzieht*, reicht
  `#[serde(default)]` nicht: S45 musste die alte Zeile ein **zweites Mal** lesen
  (`ShowStore::carry_levels_onto_their_cue_lists`), und der Test dafür schreibt
  die Vorher-Zeilen von Hand.
- **Ein Test-Target, das einen Daemon startet, setzt `websocket: Listen::Off`.**
  Seit S46 gilt dasselbe für `artnet_discovery: false`, sobald ein echter
  Art-Net-Output auf Loopback konfiguriert wird — sonst bindet der Daemon
  `0.0.0.0:6454`.
- **Die Gates laufen einzeln.** `cargo test --workspace` und `npm run test`
  gleichzeitig lassen zwei UI-Tests durch Zeitüberschreitung fallen, die einzeln
  grün sind.
- **Vor Zeitmessungen die Systemlast prüfen**, sonst sieht eine beschäftigte
  Maschine aus wie eine Regression:
  `Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'"`
  — der `Get-Counter`-Weg ist lokalisiert und schlägt auf einer deutschen
  Installation fehl. **Und stelle jede Behauptung über den Tick gegen ein
  Kontrollfenster**, nie gegen Null: S45 hat den Zehn-Minuten-Gate rot gesehen
  und der blanke Tick ohne Merge-Body war sechsmal schlechter — siehe §5. Der
  Gate steht dort als **ungemessen** und ist diese Session zu klären.
- **Eine Coverage-Messung ohne `cargo llvm-cov clean --workspace` davor ist
  keine Messung.**
- **Die sechs Recordings sind ein Satz.** Ändert sich das Protokoll, werden alle
  sechs neu erzeugt (`cargo test -p prismd --test ui_* -- --ignored`) und die
  Diffs **strukturell** gelesen, nicht als Base64. Wer einen Schritt in ein
  Skript einfügt, nummeriert die folgenden `typedLine`-Nummern nach und prüft
  die hartkodierten Schritt-Indizes in `ui/src/desk/desk.test.tsx`.
- **Die `data-testid`s sind ein Vertrag** zwischen Oberfläche und den 44
  Playwright-Tests. Ein umbenanntes Element zieht seinen Test **mit**; ein Test
  wird nie gelöscht, weil er rot ist. Gilt seine Behauptung nicht mehr, steht in
  einem Satz da, warum.
- **CI hat `channel = "stable"`**, also trifft sie jede neue Lint-Menge zuerst.

## Zum Schluss

- Alle Gates grün: `cargo test --workspace`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo fmt --all --check`, und in `ui/`:
  `npx tsc -b --force`, `npm run lint`, `npm run test`, `npm run build`, sowie
  die Playwright-Tests (heute 44)
- Die Zahlen aus S24–S28, S33, S34, S39, S40, S36, S37, S38, S43, S45 und S46
  müssen weiter stimmen: null Allokationen im Tick (**acht** Pfade plus der
  neunte, den diese Session hinzufügt), null React-Commits, das
  Telemetrie-Frame-Budget
- **Die Markdown-Dateien werden im selben Durchgang nachgezogen wie der Code**,
  nicht in einem Folge-Commit. Der Eigentümer hat das am 2026-08-28
  ausdrücklich verlangt. Betroffen sind je nach Änderung:
  `ARCHITECTURE_SPEC.md` (§3.1, §6 — und prüfe auf **tote Querverweise**: der
  Code zitiert Abschnitte nach Nummer), `docs/DMX_MERGE.md` (§3 — die Regel, die
  sich ändert), `docs/IPC_PROTOCOL.md` (Draht),
  `docs/PRERELEASE_PUNCHLIST.md` (den Eintrag abhaken, der dazugehört)
- PROGRESS.md aktualisieren: S48-Status, ein §2.44-Verifikationsprotokoll mit
  **jeder gemessenen Zahl**, die Coverage- und Performance-Tabellen in §3, die
  Zeile zum Zehn-Minuten-Gate in §5, und in §7 eine neue Liste
  „Carried out of S48"
- PROGRESS.md §8 mit einem neuen, kontextfreien Folge-Prompt überschreiben —
  nach der Laufreihenfolge ist das **S49** (die Kommandozeile zieht in den
  Daemon), das S48 als Unterbau braucht, weil S48 der Grammatik Wörter hinzufügt
- Mit einer Conventional-Commit-Nachricht committen, pushen, CI beobachten und
  das Ergebnis in PROGRESS.md festhalten
```
