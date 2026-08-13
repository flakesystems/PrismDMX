# CLAUDE.md - Development & Quality Standards for PrismDMX

This document defines the mandatory guidelines, commands, and code quality expectations for AI assistants and human developers working on **PrismDMX**.

---

## Project Overview
PrismDMX is a high-reliability, real-time DMX control software designed for mission-critical lighting control in venues and schools. Reliability, deterministic performance, and clean maintainability are paramount.

---

## Development & Test Commands

> *Note: Exact commands will be finalized after tech stack selection in the Planning Phase.*

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