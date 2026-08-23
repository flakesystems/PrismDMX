//! The MCU protocol table, held as data rather than as code.
//!
//! Every note number, CC number and MIDI channel in `docs/MCU_MAPPING.md` §2
//! lives in [`X_TOUCH`]. Nothing in this crate matches on a literal number, and
//! that is the whole design: correcting a row is an edit to this file and
//! nothing else — the same bet `prism_protocols::DeviceProfile::SH_RS09B` made
//! in S7 and collected on in S8.
//!
//! A number sitting inside a `match` arm somewhere in the decoder would be in
//! the wrong place, because correcting it would then be a refactor.
//!
//! # Verified at the device (S20, 2026-08-13)
//!
//! [`McuProfile::verified`] is now `true`, and it means something specific: a
//! Behringer X-Touch in **MC mode over USB, firmware V1.25, serial `0156406`**
//! was worked control by control and answered on every number below.
//! `docs/MCU_MAPPING.md` §2.7 is the measurement and §7 is the checklist it
//! closes. The recordings are in `tests/captures/` and
//! `tests/hardware_capture.rs` replays them, so the claim survives as a test
//! rather than as a paragraph.
//!
//! **Not one note number, CC number, channel or offset had to change.** Three
//! sources that agree number for number turned out to be right, which is worth
//! knowing before the next device is guessed at. What the desk *did* correct was
//! four things no source stated: the faders report in steps of four rather than
//! all 16 384 positions ([`McuProfile::fader_step`]), two panel buttons have no
//! LED at all ([`McuProfile::unlit_buttons`]), a 7-segment value of 0 blanks the
//! digit rather than drawing `@`, and the V-Pots carry an acceleration magnitude
//! the jog wheel never uses.
//!
//! # Where the numbers came from before that
//!
//! `docs/MCU_MAPPING.md` §2.5: Ardour's production Mackie surface code, a
//! careful reverse-engineering write-up of the protocol, and Ableton's own
//! `MackieControl` remote script adapted for the X-Touch.

use core::fmt;

use crate::control::{ButtonId, FADER_MAX};
use crate::model::{Control, SurfaceMode};
use prism_domain::RESERVED_BUTTONS;
// The panel's names moved to `prism-domain` in S38 — see that crate's `surface`
// module for why a vocabulary that travels on the wire is the domain's. They are
// re-exported here so `prism_surface::GlobalButton` still names the same type: a
// caller of this crate is asking about a surface, and where the type is defined
// is not their business.
pub use prism_domain::{GlobalButton, StripButton};

/// Mackie's three-byte manufacturer ID, which every MCU SysEx carries.
///
/// The X-Touch's own extensions live inside it too rather than under
/// Behringer's ID — the surface is emulating a Mackie Control and says so all
/// the way down (`docs/MCU_MAPPING.md` §2.3).
pub const MACKIE_MANUFACTURER_ID: [u8; 3] = [0x00, 0x00, 0x66];

/// Device ID of a Mackie Control, which is what an X-Touch in MC mode calls
/// itself.
pub const DEVICE_ID_MCU: u8 = 0x14;

/// Device ID of an extender. Recorded so a second profile has somewhere to
/// start; nothing in this crate drives one yet.
pub const DEVICE_ID_EXTENDER: u8 = 0x15;

/// SysEx command: the host asking *are you there*. The handshake is optional —
/// a host that simply starts sending also works.
pub const SYSEX_DEVICE_QUERY: u8 = 0x00;

/// SysEx command: the scribble strip text buffer.
pub const SYSEX_LCD_TEXT: u8 = 0x12;

/// SysEx command: the global LCD meter mode.
pub const SYSEX_METER_MODE: u8 = 0x21;

/// SysEx command: the X-Touch's scribble strip colours. The vendor extension
/// this device is bought for, undocumented by Behringer, firmware ≥ 1.22.
pub const SYSEX_LCD_COLOR: u8 = 0x72;

/// One row of the strip button table: a button and the note number **strip 0**
/// uses for it. Strip *n* is that note plus *n*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StripButtonRow {
    /// The button this row is about.
    pub button: StripButton,
    /// The note number the leftmost strip sends for it.
    pub first_note: u8,
}

/// One row of the global button table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonNote {
    /// The button.
    pub button: GlobalButton,
    /// The note number it sends and its LED listens on.
    pub note: u8,
}

/// The global button note map of `docs/MCU_MAPPING.md` §2.1, sorted by note.
///
/// Sorted because the decoder binary-searches it and because a sorted table is
/// one a person can read against a MIDI monitor capture line by line, which is
/// what S20 did. A test asserts the ordering rather than trusting it.
const X_TOUCH_BUTTONS: [ButtonNote; 64] = [
    ButtonNote {
        button: GlobalButton::AssignTrack,
        note: 40,
    },
    ButtonNote {
        button: GlobalButton::AssignSend,
        note: 41,
    },
    ButtonNote {
        button: GlobalButton::AssignPan,
        note: 42,
    },
    ButtonNote {
        button: GlobalButton::AssignPlugin,
        note: 43,
    },
    ButtonNote {
        button: GlobalButton::AssignEq,
        note: 44,
    },
    ButtonNote {
        button: GlobalButton::AssignInstrument,
        note: 45,
    },
    ButtonNote {
        button: GlobalButton::BankLeft,
        note: 46,
    },
    ButtonNote {
        button: GlobalButton::BankRight,
        note: 47,
    },
    ButtonNote {
        button: GlobalButton::ChannelLeft,
        note: 48,
    },
    ButtonNote {
        button: GlobalButton::ChannelRight,
        note: 49,
    },
    ButtonNote {
        button: GlobalButton::Flip,
        note: 50,
    },
    ButtonNote {
        button: GlobalButton::GlobalView,
        note: 51,
    },
    ButtonNote {
        button: GlobalButton::NameValue,
        note: 52,
    },
    ButtonNote {
        button: GlobalButton::SmpteBeats,
        note: 53,
    },
    ButtonNote {
        button: GlobalButton::F1,
        note: 54,
    },
    ButtonNote {
        button: GlobalButton::F2,
        note: 55,
    },
    ButtonNote {
        button: GlobalButton::F3,
        note: 56,
    },
    ButtonNote {
        button: GlobalButton::F4,
        note: 57,
    },
    ButtonNote {
        button: GlobalButton::F5,
        note: 58,
    },
    ButtonNote {
        button: GlobalButton::F6,
        note: 59,
    },
    ButtonNote {
        button: GlobalButton::F7,
        note: 60,
    },
    ButtonNote {
        button: GlobalButton::F8,
        note: 61,
    },
    ButtonNote {
        button: GlobalButton::ViewMidiTracks,
        note: 62,
    },
    ButtonNote {
        button: GlobalButton::ViewInputs,
        note: 63,
    },
    ButtonNote {
        button: GlobalButton::ViewAudioTracks,
        note: 64,
    },
    ButtonNote {
        button: GlobalButton::ViewAudioInstruments,
        note: 65,
    },
    ButtonNote {
        button: GlobalButton::ViewAux,
        note: 66,
    },
    ButtonNote {
        button: GlobalButton::ViewBusses,
        note: 67,
    },
    ButtonNote {
        button: GlobalButton::ViewOutputs,
        note: 68,
    },
    ButtonNote {
        button: GlobalButton::ViewUser,
        note: 69,
    },
    ButtonNote {
        button: GlobalButton::ModShift,
        note: 70,
    },
    ButtonNote {
        button: GlobalButton::ModOption,
        note: 71,
    },
    ButtonNote {
        button: GlobalButton::ModControl,
        note: 72,
    },
    ButtonNote {
        button: GlobalButton::ModAlt,
        note: 73,
    },
    ButtonNote {
        button: GlobalButton::AutoRead,
        note: 74,
    },
    ButtonNote {
        button: GlobalButton::AutoWrite,
        note: 75,
    },
    ButtonNote {
        button: GlobalButton::AutoTrim,
        note: 76,
    },
    ButtonNote {
        button: GlobalButton::AutoTouch,
        note: 77,
    },
    ButtonNote {
        button: GlobalButton::AutoLatch,
        note: 78,
    },
    ButtonNote {
        button: GlobalButton::AutoGroup,
        note: 79,
    },
    ButtonNote {
        button: GlobalButton::Save,
        note: 80,
    },
    ButtonNote {
        button: GlobalButton::Undo,
        note: 81,
    },
    ButtonNote {
        button: GlobalButton::Cancel,
        note: 82,
    },
    ButtonNote {
        button: GlobalButton::Enter,
        note: 83,
    },
    ButtonNote {
        button: GlobalButton::Markers,
        note: 84,
    },
    ButtonNote {
        button: GlobalButton::Nudge,
        note: 85,
    },
    ButtonNote {
        button: GlobalButton::Cycle,
        note: 86,
    },
    ButtonNote {
        button: GlobalButton::Drop,
        note: 87,
    },
    ButtonNote {
        button: GlobalButton::Replace,
        note: 88,
    },
    ButtonNote {
        button: GlobalButton::Click,
        note: 89,
    },
    ButtonNote {
        button: GlobalButton::SoloClear,
        note: 90,
    },
    ButtonNote {
        button: GlobalButton::Rewind,
        note: 91,
    },
    ButtonNote {
        button: GlobalButton::FastForward,
        note: 92,
    },
    ButtonNote {
        button: GlobalButton::Stop,
        note: 93,
    },
    ButtonNote {
        button: GlobalButton::Play,
        note: 94,
    },
    ButtonNote {
        button: GlobalButton::Record,
        note: 95,
    },
    ButtonNote {
        button: GlobalButton::CursorUp,
        note: 96,
    },
    ButtonNote {
        button: GlobalButton::CursorDown,
        note: 97,
    },
    ButtonNote {
        button: GlobalButton::CursorLeft,
        note: 98,
    },
    ButtonNote {
        button: GlobalButton::CursorRight,
        note: 99,
    },
    ButtonNote {
        button: GlobalButton::Zoom,
        note: 100,
    },
    ButtonNote {
        button: GlobalButton::Scrub,
        note: 101,
    },
    ButtonNote {
        button: GlobalButton::FootSwitch1,
        note: 102,
    },
    ButtonNote {
        button: GlobalButton::FootSwitch2,
        note: 103,
    },
];

/// The buttons this surface has that have no LED, measured in S20.
///
/// Two of sixty-four. Both are printed on the panel, both send their note when
/// pressed, and neither lights whatever velocity is sent to it —
/// `docs/MCU_MAPPING.md` §2.7. The list is separate from the note table rather
/// than a field on every row because it is a two-item exception, and a `bool` on
/// all sixty-four rows would be sixty-two ways of writing "yes".
const X_TOUCH_UNLIT_BUTTONS: [GlobalButton; 2] =
    [GlobalButton::NameValue, GlobalButton::SmpteBeats];

/// What keeps reaching MC while the surface is also driving a sound console.
///
/// `docs/MCU_MAPPING.md` §4.3, and **this one is the operator's account rather
/// than a measurement**: the desk is to be run in the X-Touch's combined
/// **Xctl+MC** mode, driving the venue's sound console and PrismDMX at once, and
/// in that mode only what Xctl leaves unused reaches us permanently — the
/// transport section and the jog wheel. Everything else follows the operator's
/// switch between the two hosts, which costs one button press and gives PrismDMX
/// the whole surface.
///
/// Held as data beside [`X_TOUCH_UNLIT_BUTTONS`] for the same reason: *which
/// controls are ours* is then one edit rather than a condition threaded through
/// the diffing. §7 of the mapping document carries verifying it as an open item,
/// because checking it needs the sound console on the other end.
const X_TOUCH_PERMANENT: [Control; 6] = [
    Control::Button(ButtonId::Global(GlobalButton::Rewind)),
    Control::Button(ButtonId::Global(GlobalButton::FastForward)),
    Control::Button(ButtonId::Global(GlobalButton::Stop)),
    Control::Button(ButtonId::Global(GlobalButton::Play)),
    Control::Button(ButtonId::Global(GlobalButton::Record)),
    Control::Jog,
];

/// The strip button rows of `docs/MCU_MAPPING.md` §2.1.
const X_TOUCH_STRIP_BUTTONS: [StripButtonRow; 5] = [
    StripButtonRow {
        button: StripButton::Rec,
        first_note: 0,
    },
    StripButtonRow {
        button: StripButton::Solo,
        first_note: 8,
    },
    StripButtonRow {
        button: StripButton::Mute,
        first_note: 16,
    },
    StripButtonRow {
        button: StripButton::Select,
        first_note: 24,
    },
    StripButtonRow {
        button: StripButton::VPotPush,
        first_note: 32,
    },
];

/// Everything the codec needs to know about one MCU-speaking surface.
///
/// One constant per device. Adding an X-Touch Extender means adding a second
/// one of these and changing nothing else (`docs/MCU_MAPPING.md` §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McuProfile {
    /// Human-readable name, as a log line or the UI names the surface.
    pub name: &'static str,
    /// The name a binding profile calls this surface by — the `device` field of
    /// `profiles/surface/xtouch.json` (`docs/MCU_MAPPING.md` §4.2).
    ///
    /// Machine-readable where [`name`](Self::name) is not, and it exists so that
    /// a binding table written for another surface is **refused** rather than
    /// applied to whichever device happens to be plugged in. A profile that
    /// names controls this surface has not got would otherwise load quietly and
    /// bind nothing.
    pub key: &'static str,
    /// The device ID this surface answers to inside a Mackie SysEx.
    pub device_id: u8,
    /// The MIDI channel every note and CC uses, **zero-based**: 0 is what a
    /// person calls channel 1.
    pub channel: u8,
    /// How many channel strips the surface has.
    pub strips: u8,
    /// The strip button note map.
    pub strip_buttons: &'static [StripButtonRow],
    /// The global button note map, sorted by note.
    pub buttons: &'static [ButtonNote],
    /// CC number of strip 0's V-Pot rotation; strip *n* is this plus *n*.
    pub vpot_cc: u8,
    /// CC number of strip 0's V-Pot ring LEDs.
    pub vpot_ring_cc: u8,
    /// CC number of the jog wheel.
    pub jog_cc: u8,
    /// Note number of strip 0's fader touch sensor.
    pub touch_note: u8,
    /// Note number of the main fader's touch sensor.
    pub main_touch_note: u8,
    /// Pitch-bend channel of the main fader, zero-based. The strips use
    /// channels 0..`strips`.
    pub main_fader_channel: u8,
    /// CC number of the rightmost 7-segment digit. The display is addressed
    /// **right to left**, so digit *n* is this plus *n*.
    pub segment_cc: u8,
    /// How many 7-segment digits there are: ten of timecode and two of
    /// assignment.
    pub segments: u8,
    /// A second MIDI channel the surface is reported to accept the 7-segment
    /// display on, zero-based. Some hosts send these on channel 16 rather than
    /// 1, and a codec that takes both works with more surfaces — but it emits
    /// only [`channel`](Self::channel), so the byte stream stays canonical.
    pub segment_alt_channel: Option<u8>,
    /// Offset of the lower scribble strip line inside the one shared character
    /// buffer. The upper line starts at 0.
    pub lcd_line_offset: u8,
    /// Characters each strip owns on a scribble strip line.
    pub lcd_chars_per_strip: u8,
    /// Characters in the whole two-line buffer.
    pub lcd_buffer_len: u8,
    /// The granularity a fader **reports**, in 14-bit units.
    ///
    /// Measured at the device (S20): every one of 576 captured positions was a
    /// multiple of **4**, so the X-Touch's faders are 12-bit presented in a
    /// 14-bit field and the top of travel is
    /// [`max_reported_position`](Self::max_reported_position) — 16380, not
    /// [`FADER_MAX`]. No source mentioned this.
    ///
    /// It matters upstairs rather than here: a layer that turns an inbound
    /// position into a percentage by dividing by [`FADER_MAX`] gives 99.98 % for
    /// a fader against its end stop, and a master that cannot reach full is a
    /// master that is wrong. Outbound positions are unaffected — the motor
    /// accepts all 16 384 and simply cannot report back that finely.
    pub fader_step: u8,
    /// Buttons that send a note but have **no LED to light**.
    ///
    /// A deviation from the MCU standard, measured (S20): `docs/MCU_MAPPING.md`
    /// §2.2 says every button that has an LED accepts a Note On, and the Ardour
    /// manual calls the X-Touch a 1:1 emulation with no deviations. Name/Value
    /// and SMPTE/Beats are printed on this panel, send notes 52 and 53 when
    /// pressed, and are dark whatever is sent to them.
    ///
    /// Held as data because a console that lights a lamp which does not exist is
    /// a console whose feedback silently lies about part of itself — S21's shadow
    /// model can skip these, and S26 can decline to offer them as indicators.
    pub unlit_buttons: &'static [GlobalButton],
    /// The controls that reach PrismDMX even when the surface is shared with
    /// another host — [`SurfaceMode::Shared`].
    ///
    /// Data rather than a condition in the diffing, exactly as
    /// [`unlit_buttons`](Self::unlit_buttons) is, because *which controls are
    /// ours* is a fact about a deployment and changing it should be one edit.
    /// `docs/MCU_MAPPING.md` §4.3, and it is the operator's account rather than a
    /// measurement — §7 carries verifying it as an open item.
    pub permanent: &'static [Control],
    /// Buttons PrismDMX must never drive or bind, whatever the mode.
    ///
    /// SMPTE/Beats on this surface: in the combined Xctl+MC mode it is what
    /// switches the desk between the two hosts, so binding it strands the
    /// operator away from their sound console (§4.3). Reserved in **every** mode
    /// rather than only in the shared one, deliberately, so that one profile is
    /// safe on a desk whose mode nobody has checked.
    ///
    /// **The array is `prism_domain::RESERVED_BUTTONS` and this points at it** —
    /// S38. Until then the rule was stated in this file, which was right while
    /// this crate was the only thing that could refuse a binding; it stopped
    /// being right when a *command* could bind one, because the applier that has
    /// to refuse that (`prism_core::MachineConfig::configure`) must never depend
    /// on a MIDI codec. One array with two readers is a rule; two arrays would be
    /// a pair to keep in step, and the copy that drifted would be the one an
    /// operator relied on.
    ///
    /// Three layers refuse it and all three say
    /// `prism_domain::RESERVED_REASON`: layer 2 drops its inbound events and
    /// counts them, S22's loader refuses a profile file that names it, and S38
    /// refuses a command and a stored table that do.
    pub reserved_buttons: &'static [GlobalButton],
    /// Whether the numbers above have been read off a real device.
    ///
    /// A field rather than a comment so a log line, a status panel or a test can
    /// ask. **`true` since 2026-08-13 (S20)**: `docs/MCU_MAPPING.md` §7's
    /// checklist was worked through against a Behringer X-Touch in MC mode over
    /// USB on firmware V1.25, and §2.7 records what it said. The captures in
    /// `tests/captures/` are the evidence and `tests/hardware_capture.rs`
    /// replays them on every commit.
    pub verified: bool,
}

impl McuProfile {
    /// The button at a note number, if the table has one.
    ///
    /// Binary search rather than a linear scan: the table is sorted, and this
    /// runs once per inbound byte pair on the `midi-in` thread.
    #[must_use]
    pub fn button_at(&self, note: u8) -> Option<GlobalButton> {
        self.buttons
            .binary_search_by_key(&note, |row| row.note)
            .ok()
            .and_then(|index| self.buttons.get(index))
            .map(|row| row.button)
    }

    /// The note number a button sends and its LED listens on.
    ///
    /// Linear, because outbound traffic is coalesced at 30 Hz
    /// (`docs/MCU_MAPPING.md` §5.2) and sixty-four comparisons at that rate is
    /// not a cost worth a second table to keep in step.
    #[must_use]
    pub fn note_of(&self, button: GlobalButton) -> Option<u8> {
        self.buttons
            .iter()
            .find(|row| row.button == button)
            .map(|row| row.note)
    }

    /// The note number a strip's button sends, or `None` if there is no such
    /// strip or the surface does not have that button.
    #[must_use]
    pub fn strip_note(&self, strip: u8, button: StripButton) -> Option<u8> {
        if strip >= self.strips {
            return None;
        }
        self.strip_buttons
            .iter()
            .find(|row| row.button == button)
            .and_then(|row| row.first_note.checked_add(strip))
    }

    /// The strip and button a note number belongs to, if it is a strip button.
    #[must_use]
    pub fn strip_button_at(&self, note: u8) -> Option<(u8, StripButton)> {
        self.strip_buttons.iter().find_map(|row| {
            let offset = note.checked_sub(row.first_note)?;
            (offset < self.strips).then_some((offset, row.button))
        })
    }

    /// The CC number a strip's V-Pot rotation arrives on.
    #[must_use]
    pub fn vpot_controller(&self, strip: u8) -> Option<u8> {
        (strip < self.strips).then_some(())?;
        self.vpot_cc.checked_add(strip)
    }

    /// The strip whose V-Pot sits on a CC number, if any.
    #[must_use]
    pub fn vpot_strip_at(&self, controller: u8) -> Option<u8> {
        let strip = controller.checked_sub(self.vpot_cc)?;
        (strip < self.strips).then_some(strip)
    }

    /// The CC number a strip's V-Pot **ring LEDs** listen on.
    #[must_use]
    pub fn ring_controller(&self, strip: u8) -> Option<u8> {
        (strip < self.strips).then_some(())?;
        self.vpot_ring_cc.checked_add(strip)
    }

    /// The strip whose ring LEDs sit on a CC number, if any.
    #[must_use]
    pub fn ring_strip_at(&self, controller: u8) -> Option<u8> {
        let strip = controller.checked_sub(self.vpot_ring_cc)?;
        (strip < self.strips).then_some(strip)
    }

    /// The CC number a 7-segment digit listens on. Digit 0 is the **rightmost**
    /// one.
    #[must_use]
    pub fn segment_controller(&self, digit: u8) -> Option<u8> {
        (digit < self.segments).then_some(())?;
        self.segment_cc.checked_add(digit)
    }

    /// The 7-segment digit a CC number addresses, if any.
    #[must_use]
    pub fn segment_at(&self, controller: u8) -> Option<u8> {
        let digit = controller.checked_sub(self.segment_cc)?;
        (digit < self.segments).then_some(digit)
    }

    /// Whether a MIDI channel is one this surface's 7-segment display is
    /// accepted on.
    ///
    /// Two, because some hosts drive it on channel 16 rather than 1 and a codec
    /// that takes both works with more surfaces. Only
    /// [`channel`](Self::channel) is ever written.
    #[must_use]
    pub fn accepts_segment_channel(&self, channel: u8) -> bool {
        channel == self.channel || self.segment_alt_channel == Some(channel)
    }

    /// The note number a fader's touch sensor sends.
    #[must_use]
    pub fn touch_note_of(&self, fader: Fader) -> Option<u8> {
        match fader {
            Fader::Strip(strip) if strip < self.strips => self.touch_note.checked_add(strip),
            Fader::Strip(_) => None,
            Fader::Main => Some(self.main_touch_note),
        }
    }

    /// The fader whose touch sensor sends this note, if any.
    #[must_use]
    pub fn touch_at(&self, note: u8) -> Option<Fader> {
        if note == self.main_touch_note {
            return Some(Fader::Main);
        }
        let offset = note.checked_sub(self.touch_note)?;
        (offset < self.strips).then_some(Fader::Strip(offset))
    }

    /// The pitch-bend channel a fader's position travels on, zero-based.
    #[must_use]
    pub fn fader_channel(&self, fader: Fader) -> Option<u8> {
        match fader {
            Fader::Strip(strip) if strip < self.strips => Some(strip),
            Fader::Strip(_) => None,
            Fader::Main => Some(self.main_fader_channel),
        }
    }

    /// The fader that owns a pitch-bend channel, if any.
    #[must_use]
    pub fn fader_on_channel(&self, channel: u8) -> Option<Fader> {
        if channel == self.main_fader_channel {
            return Some(Fader::Main);
        }
        (channel < self.strips).then_some(Fader::Strip(channel))
    }

    /// The highest position this surface's faders can report.
    ///
    /// [`FADER_MAX`] rounded down to a whole [`fader_step`](Self::fader_step):
    /// 16380 on the X-Touch. A fader is at the top when it reads *this*, not
    /// when it reads 16383, which it never will.
    #[must_use]
    pub const fn max_reported_position(&self) -> u16 {
        let step = self.fader_step as u16;
        if step < 2 {
            return FADER_MAX;
        }
        FADER_MAX - (FADER_MAX % step)
    }

    /// The level a reported fader position means, 0…65535.
    ///
    /// **Scaled against [`max_reported_position`](Self::max_reported_position)
    /// and not against [`FADER_MAX`]**, which is the whole reason S20 measured
    /// the step: this surface's faders stop at 16380, so dividing by 16383 gives
    /// 99.98 % for a fader against its end stop. An executor master that cannot
    /// reach full is a fault an operator finds and nobody can explain.
    ///
    /// A position above the reported top — which this surface never sends, and
    /// another might — answers full rather than wrapping.
    #[must_use]
    #[allow(
        clippy::integer_division,
        reason = "a level is a ratio; the remainder is the quantisation the \
                  12-bit fader already has"
    )]
    pub const fn level_from_position(&self, position: u16) -> u16 {
        let top = self.max_reported_position();
        if top == 0 || position >= top {
            return u16::MAX;
        }
        ((position as u32 * u16::MAX as u32) / top as u32) as u16
    }

    /// The position that puts a motor fader at a level.
    ///
    /// The inverse of [`level_from_position`](Self::level_from_position), and
    /// scaled against the same number: a master at full parks the fader exactly
    /// where the surface itself reports full, so a value driven out and the value
    /// that comes back agree at both end stops.
    #[must_use]
    #[allow(
        clippy::integer_division,
        reason = "rounds to nearest deliberately; the numerator carries the half"
    )]
    pub const fn position_from_level(&self, level: u16) -> u16 {
        let top = self.max_reported_position() as u32;
        let full = u16::MAX as u32;
        (((level as u32 * top) + full / 2) / full) as u16
    }

    /// Whether this surface has a control at all.
    #[must_use]
    pub fn has(&self, control: Control) -> bool {
        match control {
            Control::Fader(fader) => self.fader_channel(fader).is_some(),
            Control::Button(ButtonId::Strip { strip, button }) => {
                self.strip_note(strip, button).is_some()
            }
            Control::Button(ButtonId::Global(button)) => self.note_of(button).is_some(),
            Control::Encoder(strip) | Control::Display(strip) | Control::Meter(strip) => {
                strip < self.strips
            }
            Control::Segment(digit) => digit < self.segments,
            Control::Jog => true,
        }
    }

    /// Whether PrismDMX may drive a control in this mode.
    ///
    /// In [`SurfaceMode::Dedicated`] that is every control the surface has. In
    /// [`SurfaceMode::Shared`] it is only [`permanent`](Self::permanent) — the
    /// rest of the panel is showing the sound console, where lighting a Select
    /// LED is at best ignored and at worst fights that console's own feedback
    /// (`docs/MCU_MAPPING.md` §4.3).
    #[must_use]
    pub fn holds(&self, control: Control, mode: SurfaceMode) -> bool {
        match mode {
            SurfaceMode::Dedicated => self.has(control),
            SurfaceMode::Shared => self.permanent.contains(&control) && self.has(control),
        }
    }

    /// Whether a button is one PrismDMX must never drive or bind.
    #[must_use]
    pub fn is_reserved(&self, button: GlobalButton) -> bool {
        self.reserved_buttons.contains(&button)
    }

    /// Whether a button's LED exists and can be driven.
    ///
    /// `false` for a button this surface has not got at all, and for the ones in
    /// [`unlit_buttons`](Self::unlit_buttons) — which are buttons it *does* have
    /// and cannot light.
    #[must_use]
    pub fn has_led(&self, button: GlobalButton) -> bool {
        self.note_of(button).is_some() && !self.unlit_buttons.contains(&button)
    }

    /// The X-Touch as `docs/MCU_MAPPING.md` §2 describes it, **verified against
    /// the device on 2026-08-13** (§2.7).
    pub const X_TOUCH: Self = Self {
        name: "Behringer X-Touch (MC mode)",
        key: "behringer-x-touch",
        device_id: DEVICE_ID_MCU,
        channel: 0,
        strips: 8,
        strip_buttons: &X_TOUCH_STRIP_BUTTONS,
        buttons: &X_TOUCH_BUTTONS,
        vpot_cc: 16,
        vpot_ring_cc: 48,
        jog_cc: 60,
        touch_note: 104,
        main_touch_note: 112,
        main_fader_channel: 8,
        segment_cc: 64,
        segments: 12,
        segment_alt_channel: Some(15),
        lcd_line_offset: 0x38,
        lcd_chars_per_strip: 7,
        lcd_buffer_len: 112,
        fader_step: 4,
        unlit_buttons: &X_TOUCH_UNLIT_BUTTONS,
        permanent: &X_TOUCH_PERMANENT,
        reserved_buttons: &RESERVED_BUTTONS,
        verified: true,
    };
}

/// The X-Touch profile, at the name this crate refers to it by.
pub const X_TOUCH: McuProfile = McuProfile::X_TOUCH;

/// Which fader — one of the eight strips, or the main fader to their right.
///
/// The main fader is not strip 8: it sits on its own pitch-bend channel and its
/// own touch note, and `docs/MCU_MAPPING.md` §4.1 gives it a different default
/// binding. Making it a separate variant is what stops an off-by-one from
/// silently addressing the master.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fader {
    /// A channel strip's fader, counted from the left.
    Strip(u8),
    /// The main fader.
    Main,
}

impl fmt::Display for Fader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strip(strip) => write!(f, "Strip[{strip}].Fader"),
            Self::Main => f.write_str("Main.Fader"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEVICE_ID_EXTENDER, DEVICE_ID_MCU, Fader, GlobalButton, MACKIE_MANUFACTURER_ID,
        StripButton, X_TOUCH,
    };
    use crate::control::ButtonId;
    use crate::model::{Control, SurfaceMode};

    #[test]
    fn the_profile_has_been_seen_on_a_device_and_says_which_one() {
        // S19 asserted the opposite here, in a `const` block, precisely so that
        // setting the flag would stop the *build* until somebody rewrote this
        // test - which is this edit. What is being claimed now is narrow and
        // worth stating in full: a Behringer X-Touch in MC mode over USB,
        // firmware V1.25, serial 0156406, worked control by control on
        // 2026-08-13. docs/MCU_MAPPING.md §2.7 is the measurement; §7 is the
        // checklist it closes.
        //
        // The claim does not rest on this line. `tests/hardware_capture.rs`
        // replays what that surface actually sent - four captures, several
        // thousand messages - and asserts every one of them re-encodes to the
        // bytes it arrived as. A note number changed by hand turns that red on
        // a build server with nothing plugged in.
        const { assert!(X_TOUCH.verified) }
        assert_eq!(X_TOUCH.name, "Behringer X-Touch (MC mode)");
    }

    #[test]
    fn the_faders_report_in_steps_of_four_so_the_top_of_travel_is_not_16383() {
        // Measured (S20): every captured position was a multiple of 4, so the
        // surface's faders are 12-bit inside a 14-bit field. Nothing in §2.5's
        // sources mentions it, and a percentage computed against 16383 is a
        // master that stops at 99.98 %.
        assert_eq!(X_TOUCH.fader_step, 4);
        assert_eq!(X_TOUCH.max_reported_position(), 16380);
        assert!(X_TOUCH.max_reported_position() < crate::control::FADER_MAX);
        assert_eq!(
            X_TOUCH.max_reported_position() % u16::from(X_TOUCH.fader_step),
            0
        );
    }

    #[test]
    fn a_surface_that_reported_every_position_would_reach_the_top() {
        // The rounding is a property of the profile rather than of this device:
        // a step of 1 - or of 0, which is what a half-filled profile would say -
        // must not lose the last value.
        let mut ideal = X_TOUCH;
        ideal.fader_step = 1;
        assert_eq!(ideal.max_reported_position(), crate::control::FADER_MAX);
        ideal.fader_step = 0;
        assert_eq!(ideal.max_reported_position(), crate::control::FADER_MAX);
    }

    #[test]
    fn two_buttons_on_this_panel_have_no_led_and_the_profile_says_so() {
        // A deviation from the MCU standard, measured rather than assumed: both
        // send their note when pressed and neither can be lit. §2.2's "every
        // button that has an LED accepts this" is still true; the Ardour
        // manual's "no deviations" is not.
        assert!(!X_TOUCH.has_led(GlobalButton::NameValue));
        assert!(!X_TOUCH.has_led(GlobalButton::SmpteBeats));
        assert_eq!(X_TOUCH.unlit_buttons.len(), 2);
        // They are buttons the surface has, which is the whole point of the
        // exception: their notes decode, they just do not light.
        assert_eq!(X_TOUCH.note_of(GlobalButton::NameValue), Some(52));
        assert_eq!(X_TOUCH.note_of(GlobalButton::SmpteBeats), Some(53));
        // Everything else lights, including all four transport buttons and Save,
        // which is the one the dirty flag drives (§4.1).
        for button in GlobalButton::ALL {
            let unlit = X_TOUCH.unlit_buttons.contains(&button);
            assert_eq!(X_TOUCH.has_led(button), !unlit, "{button:?}");
        }
    }

    #[test]
    fn every_global_button_has_exactly_one_note_and_every_note_one_button() {
        // Two lists that have to agree: GlobalButton::ALL and the note table.
        // A button added to one and forgotten in the other is the mistake this
        // catches. S20 walked all sixty-four against the device and found none.
        assert_eq!(X_TOUCH.buttons.len(), GlobalButton::ALL.len());
        for button in GlobalButton::ALL {
            let note = X_TOUCH
                .note_of(button)
                .unwrap_or_else(|| panic!("{button:?} has no note"));
            assert_eq!(X_TOUCH.button_at(note), Some(button));
        }
        for row in X_TOUCH.buttons {
            assert!(
                GlobalButton::ALL.contains(&row.button),
                "note {} maps to {:?}, which is not in ALL",
                row.note,
                row.button
            );
        }
    }

    #[test]
    fn the_note_table_is_sorted_and_free_of_duplicates() {
        // button_at binary-searches it, so an unsorted table would not fail
        // loudly - it would fail on some notes and not others.
        let mut previous: Option<u8> = None;
        for row in X_TOUCH.buttons {
            if let Some(last) = previous {
                assert!(
                    row.note > last,
                    "note {} follows {last}, so the table is not sorted",
                    row.note
                );
            }
            previous = Some(row.note);
        }
    }

    #[test]
    fn the_global_notes_are_the_range_the_mapping_document_gives_them() {
        // 40 through 103 with nothing missing. Stated as its own claim because
        // the sortedness test above would be satisfied by a table with a hole
        // in it, and a hole is a button that reports nothing when pressed.
        let first = X_TOUCH.buttons.first().expect("table is not empty");
        let last = X_TOUCH.buttons.last().expect("table is not empty");
        assert_eq!(first.note, 40);
        assert_eq!(last.note, 103);
        for note in 40..=103 {
            assert!(X_TOUCH.button_at(note).is_some(), "note {note} is missing");
        }
    }

    #[test]
    fn a_note_outside_the_global_range_is_not_a_global_button() {
        assert_eq!(X_TOUCH.button_at(39), None);
        assert_eq!(X_TOUCH.button_at(104), None);
        assert_eq!(X_TOUCH.button_at(127), None);
    }

    #[test]
    fn the_strip_buttons_tile_notes_0_to_39_without_overlapping() {
        // Five rows of eight. If two rows overlapped, one strip's Mute would
        // arrive as another strip's Solo, which is the kind of fault an
        // operator reports as "the desk is possessed".
        let mut seen = [false; 40];
        for strip in 0..X_TOUCH.strips {
            for button in StripButton::ALL {
                let note = X_TOUCH
                    .strip_note(strip, button)
                    .unwrap_or_else(|| panic!("strip {strip} has no {button}"));
                let slot = seen
                    .get_mut(usize::from(note))
                    .unwrap_or_else(|| panic!("note {note} is outside the strip range"));
                assert!(!*slot, "note {note} is claimed twice");
                *slot = true;
                assert_eq!(X_TOUCH.strip_button_at(note), Some((strip, button)));
            }
        }
        assert!(seen.into_iter().all(|used| used));
    }

    #[test]
    fn the_strip_buttons_do_not_reach_into_the_global_range() {
        // The two tables are searched independently, so nothing but a test
        // stops them from both claiming a note.
        for note in 0..=127u8 {
            let strip = X_TOUCH.strip_button_at(note).is_some();
            let global = X_TOUCH.button_at(note).is_some();
            assert!(!(strip && global), "note {note} is in both tables");
        }
    }

    #[test]
    fn a_strip_the_surface_does_not_have_gets_no_note() {
        // The X-Touch has eight. Asking for the ninth is a paging bug upstairs,
        // and it must produce nothing rather than note 8 - which belongs to
        // strip 0's Solo.
        assert_eq!(X_TOUCH.strip_note(8, StripButton::Rec), None);
        assert_eq!(X_TOUCH.strip_note(255, StripButton::Select), None);
        assert_eq!(X_TOUCH.strip_note(7, StripButton::Rec), Some(7));
    }

    #[test]
    fn the_fader_touch_notes_cover_the_strips_and_the_main_fader() {
        for strip in 0..X_TOUCH.strips {
            let note = X_TOUCH
                .touch_note_of(Fader::Strip(strip))
                .expect("a strip the surface has");
            assert_eq!(note, 104 + strip);
            assert_eq!(X_TOUCH.touch_at(note), Some(Fader::Strip(strip)));
        }
        assert_eq!(X_TOUCH.touch_note_of(Fader::Main), Some(112));
        assert_eq!(X_TOUCH.touch_at(112), Some(Fader::Main));
        assert_eq!(X_TOUCH.touch_note_of(Fader::Strip(8)), None);
        assert_eq!(X_TOUCH.touch_at(103), None);
        assert_eq!(X_TOUCH.touch_at(113), None);
    }

    #[test]
    fn the_main_fader_is_not_strip_eight() {
        // It has its own channel and its own touch note, and §4.1 gives it a
        // different default binding. A profile that let Fader::Strip(8) resolve
        // would put a crossfade on an executor master.
        assert_eq!(X_TOUCH.fader_channel(Fader::Main), Some(8));
        assert_eq!(X_TOUCH.fader_channel(Fader::Strip(8)), None);
        assert_eq!(X_TOUCH.fader_on_channel(8), Some(Fader::Main));
        assert_eq!(X_TOUCH.fader_on_channel(7), Some(Fader::Strip(7)));
        assert_eq!(X_TOUCH.fader_on_channel(9), None);
        assert_ne!(X_TOUCH.touch_note_of(Fader::Main), Some(112 - 1));
    }

    #[test]
    fn the_cc_blocks_do_not_collide() {
        // V-Pots 16-23, ring LEDs 48-55, jog 60, 7-segment 64-75. Four blocks
        // on one channel, and nothing but arithmetic keeps them apart.
        let vpots = X_TOUCH.vpot_cc..X_TOUCH.vpot_cc + X_TOUCH.strips;
        let rings = X_TOUCH.vpot_ring_cc..X_TOUCH.vpot_ring_cc + X_TOUCH.strips;
        let segments = X_TOUCH.segment_cc..X_TOUCH.segment_cc + X_TOUCH.segments;
        assert!(!vpots.contains(&X_TOUCH.jog_cc));
        assert!(!rings.contains(&X_TOUCH.jog_cc));
        assert!(!segments.contains(&X_TOUCH.jog_cc));
        for cc in vpots {
            assert!(!rings.contains(&cc) && !segments.contains(&cc));
        }
        for cc in rings {
            assert!(!segments.contains(&cc));
        }
    }

    #[test]
    fn the_cc_blocks_answer_only_for_controls_the_surface_has() {
        for strip in 0..X_TOUCH.strips {
            let vpot = X_TOUCH.vpot_controller(strip).expect("a strip it has");
            let ring = X_TOUCH.ring_controller(strip).expect("a strip it has");
            assert_eq!(vpot, 16 + strip);
            assert_eq!(ring, 48 + strip);
            assert_eq!(X_TOUCH.vpot_strip_at(vpot), Some(strip));
            assert_eq!(X_TOUCH.ring_strip_at(ring), Some(strip));
        }
        assert_eq!(X_TOUCH.vpot_controller(8), None);
        assert_eq!(X_TOUCH.ring_controller(8), None);
        assert_eq!(X_TOUCH.vpot_strip_at(24), None);
        assert_eq!(X_TOUCH.vpot_strip_at(15), None);
        assert_eq!(X_TOUCH.ring_strip_at(56), None);
        assert_eq!(X_TOUCH.ring_strip_at(47), None);
    }

    #[test]
    fn the_seven_segment_display_is_addressed_right_to_left() {
        // Digit 0 is the rightmost, which is the opposite of how a person reads
        // the number - and the opposite of how anybody writes the loop first.
        for digit in 0..X_TOUCH.segments {
            let cc = X_TOUCH.segment_controller(digit).expect("a digit it has");
            assert_eq!(cc, 64 + digit);
            assert_eq!(X_TOUCH.segment_at(cc), Some(digit));
        }
        assert_eq!(X_TOUCH.segment_controller(12), None);
        assert_eq!(X_TOUCH.segment_at(76), None);
        assert_eq!(X_TOUCH.segment_at(63), None);
        // Ten timecode digits and two assignment digits.
        assert_eq!(X_TOUCH.segments, 12);
    }

    #[test]
    fn the_seven_segment_display_is_accepted_on_either_channel_it_may_use() {
        // §2.2: some hosts drive it on channel 16 rather than 1. Accepting both
        // costs nothing and is the difference between working with a surface
        // and not.
        assert!(X_TOUCH.accepts_segment_channel(0));
        assert!(X_TOUCH.accepts_segment_channel(15));
        assert!(!X_TOUCH.accepts_segment_channel(1));
        assert!(!X_TOUCH.accepts_segment_channel(14));
    }

    #[test]
    fn the_scribble_strip_buffer_holds_two_lines_of_seven_per_strip() {
        // §2.2: one 2x56 buffer for the whole row, not one message per strip.
        // 8 x 7 = 56 per line, and the lower line starts at 0x38.
        assert_eq!(
            X_TOUCH.lcd_line_offset,
            X_TOUCH.strips * X_TOUCH.lcd_chars_per_strip
        );
        assert_eq!(X_TOUCH.lcd_buffer_len, X_TOUCH.lcd_line_offset * 2);
    }

    #[test]
    fn the_sysex_identity_is_mackies_rather_than_behringers() {
        // §2.3: the vendor extension lives inside the emulated vendor's
        // namespace. A codec that used Behringer's ID would be talking to
        // nothing.
        assert_eq!(MACKIE_MANUFACTURER_ID, [0x00, 0x00, 0x66]);
        assert_eq!(X_TOUCH.device_id, DEVICE_ID_MCU);
        assert_eq!(DEVICE_ID_MCU, 0x14);
        assert_eq!(DEVICE_ID_EXTENDER, 0x15);
    }

    #[test]
    fn a_fader_scales_against_what_the_surface_reports_and_not_against_the_field() {
        // The measurement that matters most upstairs (§2.7): the top of travel
        // is 16380 and a master computed against 16383 stops at 99.98 %.
        // 16380 is transcribed here rather than computed, because a test that
        // asked the profile the same question the code asks it would agree with
        // any answer.
        assert_eq!(X_TOUCH.level_from_position(16380), u16::MAX);
        assert_eq!(X_TOUCH.level_from_position(0), 0);
        assert_eq!(X_TOUCH.position_from_level(u16::MAX), 16380);
        assert_eq!(X_TOUCH.position_from_level(0), 0);
        // Half travel is half a level, to within the 12 bits the fader has.
        assert_eq!(X_TOUCH.position_from_level(32768), 8190);
        assert!(X_TOUCH.level_from_position(8190).abs_diff(32768) <= 2);
        // A position the field allows but this surface never sends still reads
        // as full rather than wrapping past it.
        assert_eq!(X_TOUCH.level_from_position(16383), u16::MAX);
        assert_eq!(X_TOUCH.level_from_position(super::FADER_MAX), u16::MAX);
    }

    #[test]
    fn a_position_driven_out_and_read_back_lands_on_the_same_level() {
        // The round trip an operator sees: the desk is told where to put a
        // fader, reports it back, and the two must not disagree by enough to
        // start a fight between the motor and the shadow model.
        for level in (0..=u16::MAX).step_by(0x111) {
            let position = X_TOUCH.position_from_level(level);
            assert!(position <= X_TOUCH.max_reported_position());
            let read_back = X_TOUCH.level_from_position(position);
            assert!(
                read_back.abs_diff(level) <= 16,
                "level {level} drove to {position} and read back {read_back}"
            );
        }
    }

    #[test]
    fn a_profile_with_no_travel_at_all_answers_rather_than_dividing_by_zero() {
        // `max_reported_position` cannot be zero for any step a `u8` can hold,
        // but the crate denies panicking and a half-filled profile must not be
        // the exception that proves it.
        let mut broken = X_TOUCH;
        broken.fader_step = 0;
        assert_eq!(broken.max_reported_position(), super::FADER_MAX);
        assert_eq!(broken.level_from_position(super::FADER_MAX), u16::MAX);
    }

    #[test]
    fn the_shared_mode_keeps_the_transport_section_and_the_jog_wheel() {
        // §4.3, held as data: in the combined Xctl+MC mode only what Xctl leaves
        // unused reaches PrismDMX. The list is the operator's account and not a
        // measurement, which is why it is one edit rather than a condition in
        // the diffing.
        for button in [
            GlobalButton::Rewind,
            GlobalButton::FastForward,
            GlobalButton::Stop,
            GlobalButton::Play,
            GlobalButton::Record,
        ] {
            let control = Control::Button(ButtonId::Global(button));
            assert!(X_TOUCH.holds(control, SurfaceMode::Shared), "{button:?}");
            assert!(X_TOUCH.holds(control, SurfaceMode::Dedicated));
        }
        assert!(X_TOUCH.holds(Control::Jog, SurfaceMode::Shared));
        assert_eq!(X_TOUCH.permanent.len(), 6);
    }

    #[test]
    fn the_shared_mode_keeps_nothing_else_and_the_dedicated_one_keeps_it_all() {
        // The other half of the claim, and the one that stops feedback from
        // fighting a sound console: every strip control, every other panel
        // button and every display belongs to the other host until the operator
        // switches back.
        for strip in 0..X_TOUCH.strips {
            for control in [
                Control::Fader(Fader::Strip(strip)),
                Control::Encoder(strip),
                Control::Display(strip),
                Control::Meter(strip),
            ] {
                assert!(!X_TOUCH.holds(control, SurfaceMode::Shared), "{control:?}");
                assert!(X_TOUCH.holds(control, SurfaceMode::Dedicated));
            }
            for button in StripButton::ALL {
                let control = Control::Button(ButtonId::Strip { strip, button });
                assert!(!X_TOUCH.holds(control, SurfaceMode::Shared));
                assert!(X_TOUCH.holds(control, SurfaceMode::Dedicated));
            }
        }
        assert!(!X_TOUCH.holds(Control::Fader(Fader::Main), SurfaceMode::Shared));
        assert!(!X_TOUCH.holds(Control::Segment(0), SurfaceMode::Shared));
        for button in GlobalButton::ALL {
            let control = Control::Button(ButtonId::Global(button));
            assert_eq!(
                X_TOUCH.holds(control, SurfaceMode::Shared),
                X_TOUCH.permanent.contains(&control),
                "{button:?}"
            );
        }
    }

    #[test]
    fn a_control_the_surface_has_not_got_is_held_in_neither_mode() {
        for control in [
            Control::Fader(Fader::Strip(8)),
            Control::Encoder(8),
            Control::Display(8),
            Control::Meter(8),
            Control::Segment(12),
            Control::Button(ButtonId::Strip {
                strip: 8,
                button: StripButton::Rec,
            }),
        ] {
            assert!(!X_TOUCH.has(control), "{control:?}");
            assert!(!X_TOUCH.holds(control, SurfaceMode::Dedicated));
            assert!(!X_TOUCH.holds(control, SurfaceMode::Shared));
        }
        assert!(X_TOUCH.has(Control::Jog));
        assert!(X_TOUCH.has(Control::Segment(11)));
    }

    #[test]
    fn smpte_beats_is_reserved_in_every_mode_and_it_is_the_only_one() {
        // §4.3: in the combined mode it is the operator's way back to the sound
        // desk. Reserved in the dedicated mode as well, deliberately, so one
        // profile is safe on a desk whose mode nobody has checked.
        assert!(X_TOUCH.is_reserved(GlobalButton::SmpteBeats));
        assert_eq!(X_TOUCH.reserved_buttons.len(), 1);
        for button in GlobalButton::ALL {
            assert_eq!(
                X_TOUCH.is_reserved(button),
                button == GlobalButton::SmpteBeats,
                "{button:?}"
            );
        }
        // It is a button the surface has - it decodes, it is simply not ours -
        // and it happens to be one of the two with no lamp at all.
        assert_eq!(X_TOUCH.note_of(GlobalButton::SmpteBeats), Some(53));
        assert!(!X_TOUCH.has_led(GlobalButton::SmpteBeats));
    }

    #[test]
    fn a_button_indexes_itself_the_way_the_shadow_model_stores_it() {
        for (index, button) in GlobalButton::ALL.into_iter().enumerate() {
            assert_eq!(button.index(), index, "{button:?}");
        }
        for (index, button) in StripButton::ALL.into_iter().enumerate() {
            assert_eq!(button.index(), index, "{button}");
        }
    }

    #[test]
    fn every_control_has_a_name_a_person_can_read_in_a_log() {
        assert_eq!(StripButton::VPotPush.to_string(), "VPotPush");
        assert_eq!(StripButton::Rec.to_string(), "Rec");
        assert_eq!(StripButton::Solo.to_string(), "Solo");
        assert_eq!(StripButton::Mute.to_string(), "Mute");
        assert_eq!(StripButton::Select.to_string(), "Select");
        assert_eq!(Fader::Strip(3).to_string(), "Strip[3].Fader");
        assert_eq!(Fader::Main.to_string(), "Main.Fader");
    }
}
