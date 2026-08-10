//! Mackie Control surface integration (Behringer X-Touch).
//!
//! Three layers, kept apart so protocol detail, device abstraction and user
//! configuration never mix - see `docs/MCU_MAPPING.md`:
//!
//! 1. **MCU codec** - MIDI bytes to logical control events. Pure, no domain logic.
//! 2. **Surface model** - device-independent controls plus the shadow model used
//!    for diff-based outbound traffic.
//! 3. **Binding table** - JSON, user-editable, maps controls to commands.
//!
//! Per `CLAUDE.md`, a malformed MIDI packet must never propagate a failure. The
//! codec discards and counts; nothing reaches the engine thread.
//!
//! Platform-neutral. Sessions **S19-S22**.
