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
//!
//! # What S19 built: layer 1, and only layer 1
//!
//! ```text
//!   MIDI bytes ──▶ [`MidiDecoder`] ──▶ [`MidiMessage`] ──▶ [`ControlEvent`]
//!                   running status,      one whole            what the
//!                   SysEx reassembly,    message              operator did
//!                   counters
//!
//!   [`Feedback`] ──▶ bytes                       [`McuCodec`] = the two
//!    what the desk    into a caller's buffer,    inbound halves plus the
//!    wants shown      nothing allocated          counters, for `midi-in`
//! ```
//!
//! There is **no domain type in this crate**. No `Command`, no show, no
//! session: `GlobalButton::Play` is note 94, and that pressing it starts an
//! executor is layer 3's opinion (S22). The only dependency is `prism-domain`,
//! and layer 1 does not yet need it.
//!
//! There is also **no MIDI port**. Binding to a real device is
//! platform-dependent and belongs to the surface thread; this crate is bytes in
//! and bytes out, which is what lets its whole test suite run on a build server
//! with nothing plugged in — `CLAUDE.md`'s mocking rule, and the reason
//! `ARCHITECTURE_SPEC.md` §10.1 can list `prism-surface` among the
//! platform-neutral crates.
//!
//! # The numbers are verified, and they are data because they had to be
//!
//! Every note number, CC number and channel is in [`profile::X_TOUCH`], which
//! carries **`verified: true` since 2026-08-13 (S20)**: a Behringer X-Touch in MC
//! mode over USB, firmware V1.25, worked control by control.
//! `docs/MCU_MAPPING.md` §2.7 is the measurement and §7 is the checklist it
//! closes.
//!
//! Holding the table as data rather than as `match` arms is what made that
//! session a data edit — the bet `prism-protocols` made with
//! `DeviceProfile::SH_RS09B` in S7 and collected on in S8. **It paid better than
//! S8's did: not one number changed.** What the desk corrected was four things no
//! source had stated — the faders report in steps of four
//! ([`McuProfile::fader_step`]), two panel buttons have no LED
//! ([`McuProfile::unlit_buttons`]), a 7-segment `0` blanks the digit rather than
//! drawing `@`, and a V-Pot carries an acceleration magnitude the jog wheel never
//! uses.
//!
//! The evidence is in the repository rather than in a paragraph:
//! `tests/captures/` holds what the surface sent and
//! `tests/hardware_capture.rs` replays it in the ordinary suite, on a build
//! server, with nothing plugged in.
//!
//! # What this crate promises
//!
//! - **Nothing allocates**, per message or per malformed packet. The one buffer
//!   a sender can fill is the SysEx reassembly array, it is
//!   [`MAX_SYSEX_BYTES`] long, and it lives inside the decoder. Measured in
//!   `tests/codec_allocations.rs`, not asserted.
//! - **Nothing panics.** The crate denies `unwrap`, `expect`, `panic!` and
//!   slice indexing outside its tests, so a hostile byte stream has no path to
//!   a failure at all.
//! - **Everything discarded is counted.** [`CodecCounters`] distinguishes a
//!   cable dropping bytes from a surface sending something the profile does not
//!   describe, because those want different repairs.
//! - **A round trip is byte-equal.** Encoding an event produces the bytes the
//!   surface would have sent, not merely bytes that read back the same way.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::integer_division,
    )
)]

mod codec;
mod control;
mod feedback;
mod midi;
pub mod profile;

pub use codec::{CodecCounters, McuCodec};
pub use control::{
    ButtonId, ControlEvent, FADER_MAX, MAX_RELATIVE_STEPS, PRESS_VELOCITY, RELEASE_VELOCITY,
    relative_steps, relative_value,
};
pub use feedback::{
    Feedback, LED_FLASHING, LED_OFF, LED_ON, LcdMeterMode, LedState, METER_CLEAR_OVERLOAD,
    METER_LEVEL_0DB, METER_LEVEL_OVER, METER_SET_OVERLOAD, MeterSignal, RingMode,
    SCRIBBLE_STRIP_COLORS, SegmentChar, StripColor, VPotRing,
};
pub use midi::{
    DEFAULT_SYSEX_TIMEOUT, DecodeCounters, EncodeError, MAX_MESSAGE_BYTES, MAX_SYSEX_BYTES,
    MidiDecoder, MidiMessage,
};
pub use profile::{
    ButtonNote, DEVICE_ID_EXTENDER, DEVICE_ID_MCU, Fader, GlobalButton, MACKIE_MANUFACTURER_ID,
    McuProfile, SYSEX_DEVICE_QUERY, SYSEX_LCD_COLOR, SYSEX_LCD_TEXT, SYSEX_METER_MODE, StripButton,
    StripButtonRow, X_TOUCH,
};
