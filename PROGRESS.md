# PROGRESS.md — PrismDMX Status Tracker

**Last updated:** 2026-08-10
**Current phase:** Phase 1 — Domain and engine
**Current session:** S2 — `prism-engine` tick loop and triple buffer (not started; see §8 for the prompt that starts it)
**Last completed:** S1 — `prism-domain` ✅
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
| 13 | CI verified green | ✅ | Latest: run **31384082300** on `1ab1fac` (S1). First verified: run **31346581991** — all four jobs: Windows 55 s, ARM64 check 20 s, UI 15 s, Linux neutral 14 s. No annotations |

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
| S2 | `prism-engine` — tick loop, triple buffer | ☐ | | |
| S3 | `prism-engine` — HTP/LTP merge | ☐ | | Highest-value TDD target |
| S4 | `prism-engine` — DMX encoding | ☐ | | |
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

**Done:** 2 / 33 · **In progress:** 0 · **Blocked:** 0

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

---

## 3. Coverage tracking

Targets from `CLAUDE.md`: ≥ 85 % global, > 95 % on engine, programmer and protocols. Record **measured** figures only — leave blank until a run produces a number.

Measured with `cargo llvm-cov` 0.8.7 (installed 2026-08-10, `llvm-tools-preview`).
Command: `cargo llvm-cov -p <crate> --summary-only`.

| Crate | Target | Measured | Date |
|---|---|---|---|
| `prism-domain` | ≥ 85 % | **99.77 % lines**, 97.86 % regions, 100 % functions | 2026-08-10 |
| `prism-engine` | **> 95 %** | — | |
| `prism-core` | **> 95 %** (programmer) | — | |
| `prism-protocols` | **> 95 %** | — | |
| `prism-surface` | **> 95 %** | — | |
| `prism-ipc` | ≥ 85 % | — | |
| `ui` | ≥ 85 % | — | |

### Performance gates

| Gate | Requirement | Measured | Date |
|---|---|---|---|
| Tick jitter | p99.9 < 2 ms, 64 universes, 100 % CPU, 10 min | — | |
| Tick allocations | zero inside the tick after warm-up | — | |
| Open DMX frame rate | measure real rate on SH-RS09B | — | |
| Telemetry render | 64 universes @ 30 Hz, zero React re-renders | — | |

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

The domain vocabulary exists and is verified. Begin **S2** (`prism-engine` — tick loop and triple buffer). Use the prompt in §8.

Carried into S2 and beyond:
- `prism-domain` has an optional `proptest` feature exposing `arb` and the `Arbitrary` impls. `prism-engine` should enable it in `[dev-dependencies]` from S3 onward rather than growing its own generators.
- `prism-ipc` (S16) must serialise MessagePack with `to_vec_named`, must enforce a **nesting depth limit** on decode, and must treat serialisation as fallible — see the decision log.
- `prism-core` (S11) must embed the used fixture types in the show file — see the decision log.
- S15 must not assume bit-identical floats through a JSON export — see the decision log.

---

## 8. Follow-up prompt for the next session

> Rewritten at the close of every session, per `IMPLEMENTATION_PLAN.md`. Written to be **self-contained**: it assumes no loaded context, no memory of previous conversations and no knowledge of the project. Paste it into a fresh session to continue.

**Next up: S2 — `prism-engine`: Tick-Loop und Triple Buffer**

```text
PrismDMX — Session S2: prism-engine, Tick-Loop und Triple Buffer

Projektverzeichnis: C:\Users\Milan\Prismdmx

Bitte lies zuerst in dieser Reihenfolge, bevor du irgendetwas änderst:
1. CLAUDE.md                     — verbindliche Qualitäts-, Architektur- und Teststandards
2. PROGRESS.md                   — aktueller Stand, Decision Log, gemessene Coverage
3. IMPLEMENTATION_PLAN.md        — Session-Protokoll und die Definition von S2
4. ARCHITECTURE_SPEC.md §3, §3.1, §3.2, §5 — Threading-Modell, die harten Regeln für
   den engine-tick, die exakte Frame-Rate und die Pipeline-Reihenfolge
5. crates/prism-domain/src/lib.rs — das Vokabular, das in S1 entstanden ist

Aufgabe: Session S2 umsetzen — die Crate `prism-engine` unter crates/prism-engine
ist derzeit ein leerer Stub und bekommt den Herzschlag des Systems: einen 44-Hz-Tick,
der seine Deadline hält, und die Übergabe fertiger Frames an die Ausgabetreiber ohne
Locks.

Vorgehen strikt test-driven (CLAUDE.md): erst der fehlschlagende Test, dann die
Implementierung. Diese Crate trägt die strengste Coverage-Anforderung des Projekts
(> 95 %), abgenommen wird sie aber erst in S6.

Umzusetzen:
- Triple Buffer: ein Schreiber (die Engine), viele Leser (Ausgabetreiber), wait-free.
  Kein Leser darf einen halb geschriebenen Frame sehen.
- Tick-Loop mit absoluten Deadlines — `sleep(deadline − 1 ms)` plus Spin bis zur
  Deadline. Kein `sleep(periode)`, weil sich der Fehler sonst aufsummiert.
- SPSC-Kommando-Queue in den Tick hinein. Der Tick darf beim Leeren der Queue weder
  blockieren noch allozieren.
- Zähl-Allokator als Test-Harness, um Allokationen im Tick nachzuweisen.

Die harten Regeln aus ARCHITECTURE_SPEC.md §3.1 gelten ab hier: im Tick keine
Allokation nach dem Warmlaufen, kein Logging, kein Lock, kein I/O. Ein Panic darf
den DMX-Ausgang nicht mitreißen (`panic = "unwind"` ist im Release-Profil bereits
gesetzt).

Exit-Kriterien — die Session gilt erst als fertig, wenn diese wirklich zutreffen:
- 10-Minuten-Lauf: kein verpasster Tick, p99.9-Jitter < 2 ms — gemessen und als Zahl
  in PROGRESS.md §3 eingetragen, nicht per Augenmaß beurteilt
- Allokations-Test: null Allokationen im Tick nach dem Warmlaufen
- Triple Buffer: nebenläufiger Lese-/Schreib-Stresstest unter `loom` oder gleichwertig,
  keine zerrissenen Frames
- Drift-Test: nach 100 000 Ticks absoluter Zeitfehler < eine Tick-Periode
- cargo test -p prism-engine ist grün
- cargo clippy --workspace --all-targets -- -D warnings ist sauber
- cargo fmt --all --check ist sauber

Hinweis zu langen Tests: der 10-Minuten-Lauf und der Stresstest gehören hinter
`#[ignore]` oder in ein eigenes Test-Target, damit `cargo test --workspace` in CI
schnell bleibt. Wie man sie ausführt, gehört dokumentiert in PROGRESS.md — ein
Kriterium, das niemand mehr ausführen kann, ist nicht gemessen.

Wichtige Randbedingungen:
- prism-engine ist plattformneutral und I/O-frei — kein #[cfg(target_os = ...)],
  keine UI-Abhängigkeit, keine Hardware. CI testet die Crate auch unter Linux.
- prism-domain (Session S1) ist fertig und liefert das gesamte Typvokabular.
  Es hat ein optionales Feature `proptest`, das `prism_domain::arb` und die
  `Arbitrary`-Impls freischaltet — in [dev-dependencies] aktivieren statt eigene
  Generatoren zu schreiben.
- Toolchain ist eingerichtet und funktioniert (Rust 1.97.1 msvc, MSVC Build Tools 2022,
  Node 24.11). cargo-llvm-cov ist installiert; Coverage misst man mit
  `cargo llvm-cov -p prism-engine --summary-only`.
- Es ist kein Setup mehr nötig.

Zum Abschluss der Session:
- PROGRESS.md aktualisieren: S2-Status, die gemessenen Jitter- und Drift-Zahlen in
  §3 (Performance-Gates), gemessene Coverage, Decision Log bei Abweichungen vom Plan
- PROGRESS.md §8 mit einem neuen, ebenfalls kontextfreien Follow-up-Prompt für
  Session S3 (prism-engine: HTP/LTP-Merge nach docs/DMX_MERGE.md) überschreiben
- Mit Conventional-Commit-Message committen, z. B. feat(engine): …
```
