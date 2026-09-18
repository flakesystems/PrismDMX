# CLAUDE.md - Development & Quality Standards for PrismDMX

This document defines the mandatory guidelines, commands, and code quality expectations for AI assistants and human developers working on **PrismDMX**.

---

## Project Overview
PrismDMX is a high-reliability, real-time DMX control software designed for mission-critical lighting control in venues and schools. Reliability, deterministic performance, and clean maintainability are paramount.

---

## Development & Test Commands

The full list, and what each gate is for, is `docs/manual/developer.en.md` §8.
Run the Rust and the interface suites one at a time, never overlapping.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cd ui && npx tsc -b --force && npm run lint && npm run test && npm run build && npm run e2e
```

## CI Policy — local in full, GitHub Actions minimal (since 2026-09-18)

GitHub Actions minutes cost money and **Windows minutes cost twice as much as
Linux ones**; the Windows jobs ran about twenty minutes on every push. So:

-   **Locally, every gate runs in full, every time**, before a commit is pushed —
    the whole workspace on Windows, the interface, and the Playwright suite. This
    is where a change is verified; CI repeats, it does not replace.
-   **On every push and pull request, CI runs Linux only** (`ci.yml`): format,
    clippy and `cargo doc` over every crate but `prism-app`, the platform-neutral
    crates' tests, the ARM64 cross-check, the interface and its end-to-end
    suite, and the web jobs. Watch that run to green.
-   **The complete pass — Windows included, the installer included — runs only
    for a release** (`release.yml`, on a `v*` tag). It can be started by hand
    with `workflow_dispatch` (it builds and publishes nothing then) when a change
    needs the Windows half before a release: the shell, the installer, a
    platform-specific crate. Do that deliberately, not by habit.
-   Do not add a Windows job, or a heavy job, to `ci.yml`. A new check belongs on
    Linux if Linux can answer it, and in `release.yml` if only Windows can.

## Code Style & Architecture Guidelines

### Core Principles

1.  Zero-Crash Invariants: The DMX output thread/process MUST NEVER crash or freeze during runtime. If a non-fatal error occurs (e.g., invalid MIDI packet, UI crash), the DMX output loop must continue emitting stable frame rates.

2.  Separation of Concerns:
    -   DMX Engine: Pure calculation, HTP/LTP merging, playback execution, timing. No direct UI dependencies.
    -   Programmer: State machine tracking active user tweaks before storing them into presets/cues.
    -   Surface Controller (MCU): Bi-directional translation between Mackie Control MIDI messages and internal PrismDMX action dispatchers.
    -   UI Layer: Reactive, high-density visualization rendering data from state hooks. Make sure the UI is built like an in device screen, so avoid scrolling outside the canvas with a design focused on reliability and fast recognition of sections instead of asthetics (don't make it completely ugly though). 

3.  Immutability & Predictable State: Use immutable state updates for UI-facing state to guarantee fast re-renders and deterministic undo/redo (Oops engine).

### TypeScript / JavaScript Standards (if JS/TS Stack is selected)

-   Strict Typing: Always enable strict: true in tsconfig.json. No usage of any. Use unknown with type guards or strict type aliases.
-   Explicit Interfaces: Define explicit TypeScript interfaces/types for all domain entities (Fixture, Cue, Sequence, DmxFrame, McuPacket).
-   Functional & Pure Core: Keep DMX math and frame-merging functions pure and stateless where possible.

### Error Handling & Logging

-   Do NOT use plain console.log in production code. Use a structured logger with log levels (DEBUG, INFO, WARN, ERROR).
-   Always wrap hardware I/O (FTDI, Serial, MIDI sockets, ArtNet sockets) in robust error boundaries and auto-reconnection logic.

## Testing Policy & Production Readiness

### Requirements for PRs & Code Generation

-   Test-Driven First: Any pull request or code addition altering core logic (Programmer, Cue Engine, DMX Merging, MCU Translation) MUST include corresponding tests.
-   Coverage Goal: Minimum 85% global test coverage, >95% coverage on engine/, programmer/, and protocols/.
-   Mocking Strategy:
    -   Mock hardware interfaces (FTDI, USB, MIDI, Sockets) in unit and integration tests.
    -   Tests must execute deterministically without physical hardware connected.

## Test Categories

1.  Unit Tests: Math operations, Cue evaluation, Programmer priority resolution, Syntax string parsing.
2.  Integration Tests: MIDI Input -> Action Dispatch -> Programmer State Change -> DMX Output Frame verification.
3.  Stress & Latency Tests: Verify DMX frame emission stays stable at ~44 FPS under 100% CPU load / 64 active DMX universes.

## Git & Commit Standards

-   Commit messages follow Conventional Commits:
    -   feat(engine): add HTP dimmer merging algorithm
    -   fix(mcu): correct fader touch feedback loop
    -   test(programmer): add tests for clear button 3-stage behavior
    -   docs(arch): update MIDI surface mapping spec


---