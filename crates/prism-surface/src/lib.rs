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
//! There is **no domain type in layer 1**. No `Command`, no show, no session:
//! `GlobalButton::Play` is note 94, and that pressing it starts an executor is
//! layer 3's opinion (S22).
//!
//! # What S21 built: layer 2, the shadow model
//!
//! ```text
//!   bytes ──▶ [`SurfaceController::push`] ──▶ [`SurfaceEvent`]
//!              scaling, two acceleration     what the operator
//!              curves, touch bookkeeping     meant
//!
//!   show state ──▶ setters ──▶ two [`SurfaceState`]s ──▶ diff ──▶ [`Feedback`]
//!                              what the show wants,     §5.2's   at most one
//!                              what the desk was told   order    per gap
//! ```
//!
//! [`SurfaceController`] is one object holding the codec, the two pictures and
//! the four rules of `docs/MCU_MAPPING.md` §5 — touch suppression, coalescing,
//! priority and pacing — plus the fault §5.3 did not cover until S20 met it: a
//! surface that goes on receiving perfectly while its transmitter is dead, and
//! which only a **power cycle** recovers ([`SurfaceHealth::Unresponsive`]).
//!
//! It owns no port, no thread and no clock. Every entry point takes `now`, so
//! the whole of §5 is tested with arithmetic and nothing waits for anything.
//!
//! # What S22 built: layer 3, the binding table
//!
//! ```text
//!   [`SurfaceEvent`] ──▶ [`Bindings`] ──▶ [`SurfaceAction`] ──▶ `Command`
//!                         JSON, and the   device-free, the      the arguments
//!                         built-in §4.1   §4.1 vocabulary       from a
//!                         defaults                             [`SurfaceContext`]
//! ```
//!
//! No arithmetic at all: a fader is already a level and a detent is already a
//! parameter step, so the top layer is a table lookup and a `match`. What it
//! cannot know — which executor is under strip 3, which view lies after this
//! one — arrives as [`SurfaceContext`], plain `Copy` answers resolved by
//! whoever holds the session. This crate still holds no show and no session.
//!
//! [`Bindings::load`] **cannot fail**: a profile that will not parse answers
//! with the built-in defaults and a [`ProfileError`] to log, because
//! `IMPLEMENTATION_PLAN.md` S22 asks that a malformed profile never block
//! startup and a function without an error path is the strongest way to say it.
//! It reads a `&str` rather than a path — there is no filesystem in this crate
//! either.
//!
//! The one domain type this crate uses arrives here: [`prism_domain::RgbColor`],
//! quantised onto the eight colours a scribble strip has — hue first, because a
//! pastel is still the colour it is a pastel of (`color`).
//!
//! There is **no MIDI port** anywhere in the crate. Binding to a real device is
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
//! - **Nothing allocates**, per message, per malformed packet or per frame of
//!   outbound traffic. The one buffer a sender can fill is the SysEx reassembly
//!   array, it is [`MAX_SYSEX_BYTES`] long, and it lives inside the decoder; the
//!   two pictures and the frame's send queue are fixed-size arrays inside
//!   [`SurfaceController`]. Measured in `tests/codec_allocations.rs` and
//!   `tests/surface_allocations.rs`, not asserted.
//! - **Nothing panics.** The crate denies `unwrap`, `expect`, `panic!` and
//!   slice indexing outside its tests, so a hostile byte stream has no path to
//!   a failure at all.
//! - **Everything discarded is counted.** [`CodecCounters`] distinguishes a
//!   cable dropping bytes from a surface sending something the profile does not
//!   describe, because those want different repairs.
//! - **A round trip is byte-equal.** Encoding an event produces the bytes the
//!   surface would have sent, not merely bytes that read back the same way.
//! - **Nothing is sent twice.** Outbound traffic is the difference between the
//!   two pictures, and the shadow only moves when a message actually goes out —
//!   so a frame that could not be finished is finished by the next one instead
//!   of being lost.

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

pub mod accel;
mod binding;
mod codec;
pub mod color;
mod control;
mod feedback;
mod midi;
mod model;
pub mod profile;
mod surface;

pub use accel::{COARSE, JOG_ACCELERATION, JogAcceleration, VPOT_ACCELERATION, VPotAcceleration};
pub use binding::{
    Bindings, BoundControl, ExecutorTarget, PROFILE_VERSION, ProfileError, Step, SurfaceAction,
    SurfaceContext,
};
pub use codec::{CodecCounters, McuCodec};
pub use color::quantize;
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
pub use model::{
    ALL_FADERS, Control, DISPLAY_LINES, DisplayLine, GLOBAL_BUTTONS, MAX_FADERS, MAX_SEGMENTS,
    MAX_STRIPS, Priority, SEGMENT_BLANK, STRIP_BUTTONS, STRIP_CHARS, StripDisplay, SurfaceEvent,
    SurfaceMode, SurfaceState, fader_at, fader_index,
};
pub use profile::{
    ButtonNote, DEVICE_ID_EXTENDER, DEVICE_ID_MCU, Fader, GlobalButton, MACKIE_MANUFACTURER_ID,
    McuProfile, SYSEX_DEVICE_QUERY, SYSEX_LCD_COLOR, SYSEX_LCD_TEXT, SYSEX_METER_MODE, StripButton,
    StripButtonRow, X_TOUCH,
};
pub use surface::{SurfaceController, SurfaceCounters, SurfaceHealth, SurfaceTiming};
