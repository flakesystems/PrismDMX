# PROGRESS.md — PrismDMX Status Tracker

**Last updated:** 2026-08-10
**Current phase:** Phase 1 — Domain and engine
**Current session:** S1 — `prism-domain` (not started; see §8 for the prompt that starts it)
**Last completed:** S0 — toolchain and workspace ✅
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
| 12 | GitHub CLI | ▶ | `gh` 2.97.0 installed at `C:\Program Files\GitHub CLI\gh.exe`. **Not authenticated** — needs one interactive `gh auth login` |
| 13 | CI first run verified | ☐ | Triggered by the push, but unreadable until item 12 is done. See B2 |

---

## 2. Session status

### Phase 0 — Foundation
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S0 | Toolchain and workspace | ✅ | 2026-08-10 | All local exit criteria verified — see §2.1. The one criterion not met is "CI green on a pushed branch": no remote exists, tracked as **B2**. Not treated as blocking S1 |

### Phase 1 — Domain and engine
| Session | Title | Status | Date | Note |
|---|---|---|---|---|
| S1 | `prism-domain` — types | ☐ | | |
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

**Done:** 1 / 33 · **In progress:** 0 · **Blocked:** 0

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
| CI green on a pushed branch | ⛔ triggered but unverified — private repo, no API access (B2) |

---

## 3. Coverage tracking

Targets from `CLAUDE.md`: ≥ 85 % global, > 95 % on engine, programmer and protocols. Record **measured** figures only — leave blank until a run produces a number.

| Crate | Target | Measured | Date |
|---|---|---|---|
| `prism-domain` | ≥ 85 % | — | |
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

*No blocking issues. One non-blocking item below.*

### B2 — CI run not yet verified ☐ (non-blocking)
**State:** the remote is configured and `master` is pushed, so `.github/workflows/ci.yml` has been **triggered**. Whether it passed is unknown.

**Why it cannot be checked from the development shell:** the repository is private, so the unauthenticated GitHub API returns 404. The `gh` CLI is now installed (2.97.0) but not logged in, and `gh auth login` is an interactive browser or device flow that cannot be driven from a non-interactive shell. Pushing works only because git uses the stored credential helper, which is not a token `gh` can read.

**Why it is not blocking:** every check the CI performs also runs locally and is green (§2.1). CI protects against regressions over time; its absence does not stop S1.

**Resolution:** run `gh auth login` once (interactive, user action). After that, run status is readable directly from the shell for the rest of the project. Alternatively, read the Actions tab in a browser.

**Likely first failure, if any:** the `ui` job runs `npm ci`, which requires `ui/package-lock.json` to match `package.json`. It is committed, so this should hold — but it is the step most sensitive to the scaffold.

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
| 2026-08-10 | S0 | The remote already carried an MIT `LICENSE` from repository creation, while the workspace manifest declared `license = "UNLICENSED"` | Rebased onto the existing commit instead of overwriting it; manifest corrected to `MIT` in a separate commit |
| 2026-08-10 | S0 | The CI workflow cannot be validated without a remote, so a green local build is not the same as a green pipeline | Tracked as B2 rather than silently assuming CI works. S0 closed with this criterion explicitly unmet |
| 2026-08-10 | S0 | The Vite `react-ts` template does **not** set `"strict": true` — the default is `false`, so the template ships non-strict TypeScript despite its name | `CLAUDE.md` violation caught before any code was written. `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride` and `noImplicitReturns` added to both `tsconfig.app.json` and `tsconfig.node.json`; typecheck and build re-verified green |

---

## 7. Next actions

1. Optional: add a git remote so the CI workflow gets its first real run (B2).
2. Begin **S1** (`prism-domain`) — the dependency root for everything else. Use the prompt in §8.

---

## 8. Follow-up prompt for the next session

> Rewritten at the close of every session, per `IMPLEMENTATION_PLAN.md`. Written to be **self-contained**: it assumes no loaded context, no memory of previous conversations and no knowledge of the project. Paste it into a fresh session to continue.

**Next up: S1 — `prism-domain`**

```text
PrismDMX — Session S1: prism-domain

Projektverzeichnis: C:\Users\Milan\Prismdmx

Bitte lies zuerst in dieser Reihenfolge, bevor du irgendetwas änderst:
1. CLAUDE.md                  — verbindliche Qualitäts-, Architektur- und Teststandards
2. PROGRESS.md                — aktueller Stand, Blocker, Decision Log, Coverage
3. IMPLEMENTATION_PLAN.md     — Session-Protokoll und die Definition von S1
4. ARCHITECTURE_SPEC.md §6    — das Datenmodell, das du umsetzen sollst
5. docs/IPC_PROTOCOL.md §5–6  — die Command- und Delta-Wire-Typen

Aufgabe: Session S1 umsetzen — die Crate `prism-domain` unter crates/prism-domain
ist derzeit ein leerer Stub und soll das gesamte Domain-Vokabular bekommen.

Vorgehen strikt test-driven (CLAUDE.md, Phase 2): erst der fehlschlagende Test,
dann die Implementierung.

Umzusetzen:
- Alle Typen aus ARCHITECTURE_SPEC.md §6 als Rust-Typen mit serde-Ableitungen
- Command- und Delta-Enums nach docs/IPC_PROTOCOL.md §5–6
- Newtype-IDs (FixtureId, ExecutorId, PresetId, UniverseId, …) statt blanker u32,
  damit sie nicht versehentlich vertauscht werden können
- TypeScript-Export nach ui/src/bindings/ via ts-rs oder specta

Exit-Kriterien — die Session gilt erst als fertig, wenn diese wirklich zutreffen:
- cargo test -p prism-domain ist grün
- cargo clippy --workspace --all-targets -- -D warnings ist sauber
- cargo fmt --all --check ist sauber
- Die generierten TypeScript-Bindings kompilieren: in ui/ läuft `npx tsc -b --force`
  fehlerfrei durch (strict ist aktiviert)
- Kein `any` in den generierten Bindings
- Property-Test: jeder Typ übersteht serialisieren → deserialisieren unverändert

Wichtige Randbedingungen:
- prism-domain ist plattformneutral — kein #[cfg(target_os = ...)] in dieser Crate
- Toolchain ist eingerichtet und funktioniert (Rust 1.97.1 msvc, MSVC Build Tools 2022,
  Node 24.11). Es ist kein Setup mehr nötig.

Zum Abschluss der Session:
- PROGRESS.md aktualisieren: S1-Status, gemessene Coverage, Decision Log bei
  Abweichungen vom Plan
- PROGRESS.md §8 mit einem neuen, ebenfalls kontextfreien Follow-up-Prompt für
  Session S2 (prism-engine: Tick-Loop und Triple Buffer) überschreiben
- Mit Conventional-Commit-Message committen, z. B. feat(domain): …
```
