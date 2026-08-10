# INSTRUCTIONS.md - PrismDMX Project Guidelines

## Project Context
**PrismDMX** is a professional yet accessible DMX lighting control software inspired by grandMA3 and ChamSys MagicQ. It specifically targets schools, community centers, small-to-medium venues, and theater productions. 

### Key Characteristics
- **Professional & Budget Standards:** Supports high-end network protocols (ArtNet, sACN, PSN) as well as cost-effective hardware interfaces (e.g., FTDI USB-to-DMX adapters).
- **Physical Control Surface:** Uses native DAW MIDI protocol (Mackie Control Universal / MCU) targeting the Behringer X-Touch as its primary physical interface.
- **System Integrations:** Integrates real-time position tracking data from `openfollow.app` via PSN/OSC.
- **Architectural Reference:** Refer to `Architecture.txt` (UI/System structure) and `XTouch.txt` (Mackie Control mapping layout).

---

## Operating Mode & Workflow

### Phase 1: Planning Mode (MANDATORY FIRST STEP)
Before writing any production code, you MUST operate strictly in **Planning Mode**.

1. **Architecture & Tech Stack Decision Process:**
   - Analyze the requirements and existing documentation (`Architecture.txt`, `XTouch.txt`).
   - Identify unresolved architecture decisions (e.g., Frontend framework, Backend runtime/language, State Management, Real-time DMX Output Loop, Inter-Process Communication, IPC between MCU and UI).
   - **Formulate specific architectural questions.** For each question, provide:
     - Clear context on why the decision is critical.
     - 2–4 viable options.
     - Comprehensive pros and cons for each option (considering real-time latency, memory safety, cross-platform support, and ease of maintenance for school/small venue environments).
     - A clear recommendation based on the project goals.

2. **System Specification & Module Blueprinting:**
   - Draft a comprehensive architecture specification (`ARCHITECTURE_SPEC.md`).
   - Define data model interfaces (Fixtures, Cues, Sequences, Pools, Programmer State, DMX Universes).
   - Specify the DMX Engine pipeline (HTP/LTP merging logic, submastering, fading, refresh rates target 44Hz).
   - Map out the MCU / Behringer X-Touch protocol handler architecture.

3. **Transition Criterion:**
   - Do NOT enter Phase 2 (Implementation) until the user explicitly approves the planning phase decisions.

---

### Phase 2: Implementation & Refactoring Mode
Once planning is approved, execute changes using strict Test-Driven Development (TDD).

1. **Incremental Execution:**
   - Implement features module by module (Core DMX Engine -> State Management -> MCU Driver -> UI Components -> Protocols).
2. **Strict Testing Requirements:**
   - Every core logic module must be accompanied by unit tests.
   - DMX merging logic, timing controls, syntax parsing, and MIDI byte-stream parsing must have high test coverage (>90%).
3. **Safety & Real-Time Performance:**
   - Lighting setups must not crash during a show. DMX Engine output loop must run decoupled from the UI rendering thread.

---

## Communication Guidelines
- Be direct, explicit, and concise.
- Use clear visual representations (Mermaid diagrams, ASCII layouts) when proposing architectural changes.
- Language: English for code, comments, and technical documentation; German for conversational user interaction if the user prompts in German.