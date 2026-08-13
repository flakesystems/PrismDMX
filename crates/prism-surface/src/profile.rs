//! The MCU protocol table, held as data rather than as code.
//!
//! Every note number, CC number and MIDI channel in `docs/MCU_MAPPING.md` §2
//! lives in [`X_TOUCH`]. Nothing in this crate matches on a literal number, and
//! that is the whole design: the table is **unverified against a real device**
//! ([`McuProfile::verified`] says so out loud), and S20 holds it against a MIDI
//! monitor. When a row turns out to be wrong, the correction is an edit to this
//! file and nothing else — the same bet `prism_protocols::DeviceProfile::SH_RS09B`
//! made in S7 and collected on in S8, where the whole hardware verification came
//! to three fields and one test.
//!
//! A number sitting inside a `match` arm somewhere in the decoder would be in
//! the wrong place, because correcting it would then be a refactor.
//!
//! # Where the numbers come from
//!
//! `docs/MCU_MAPPING.md` §2.5. In short: Ardour's production Mackie surface
//! code, a careful reverse-engineering write-up of the protocol, and Ableton's
//! own `MackieControl` remote script adapted for the X-Touch — three
//! independent sources that agree number for number. That is enough to *write*
//! a codec against and not enough to sign one off.

use core::fmt;

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

/// Which button on a channel strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StripButton {
    /// Rec / Arm, the top button of the strip.
    Rec,
    /// Solo.
    Solo,
    /// Mute.
    Mute,
    /// Select.
    Select,
    /// Pushing the V-Pot in. A button, not a rotation — see
    /// [`ControlEvent::VPot`](crate::ControlEvent::VPot) for the turn.
    VPotPush,
}

impl StripButton {
    /// Every strip button, so a test can walk the whole set rather than the
    /// ones somebody remembered.
    pub const ALL: [Self; 5] = [
        Self::Rec,
        Self::Solo,
        Self::Mute,
        Self::Select,
        Self::VPotPush,
    ];
}

impl fmt::Display for StripButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Rec => "Rec",
            Self::Solo => "Solo",
            Self::Mute => "Mute",
            Self::Select => "Select",
            Self::VPotPush => "VPotPush",
        };
        f.write_str(name)
    }
}

/// One row of the strip button table: a button and the note number **strip 0**
/// uses for it. Strip *n* is that note plus *n*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StripButtonRow {
    /// The button this row is about.
    pub button: StripButton,
    /// The note number the leftmost strip sends for it.
    pub first_note: u8,
}

/// Every button on the panel that does not belong to a channel strip.
///
/// The names are the ones printed on the surface, which is deliberate: this
/// layer knows MIDI and topology, and nothing about what PrismDMX does when one
/// is pressed. `Play` is a note number here; that it starts an executor is
/// layer 3's opinion (`docs/MCU_MAPPING.md` §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GlobalButton {
    /// Encoder Assign: Track.
    AssignTrack,
    /// Encoder Assign: Send.
    AssignSend,
    /// Encoder Assign: Pan/Surround.
    AssignPan,
    /// Encoder Assign: Plug-in.
    AssignPlugin,
    /// Encoder Assign: EQ.
    AssignEq,
    /// Encoder Assign: Instrument.
    AssignInstrument,
    /// Fader bank left — executor page down (D7).
    BankLeft,
    /// Fader bank right — executor page up (D7).
    BankRight,
    /// Channel left — previous UI view (D8).
    ChannelLeft,
    /// Channel right — next UI view (D8).
    ChannelRight,
    /// Flip.
    Flip,
    /// Global View.
    GlobalView,
    /// Display: Name/Value.
    NameValue,
    /// Display: SMPTE/Beats.
    SmpteBeats,
    /// Function key 1.
    F1,
    /// Function key 2.
    F2,
    /// Function key 3.
    F3,
    /// Function key 4.
    F4,
    /// Function key 5.
    F5,
    /// Function key 6.
    F6,
    /// Function key 7.
    F7,
    /// Function key 8.
    F8,
    /// Global View group: MIDI Tracks.
    ViewMidiTracks,
    /// Global View group: Inputs.
    ViewInputs,
    /// Global View group: Audio Tracks.
    ViewAudioTracks,
    /// Global View group: Audio Instruments.
    ViewAudioInstruments,
    /// Global View group: Aux.
    ViewAux,
    /// Global View group: Busses.
    ViewBusses,
    /// Global View group: Outputs.
    ViewOutputs,
    /// Global View group: User.
    ViewUser,
    /// Modifier: Shift. Held, not latched.
    ModShift,
    /// Modifier: Option. Held, not latched.
    ModOption,
    /// Modifier: Control. Held, not latched.
    ModControl,
    /// Modifier: Alt. Held, not latched.
    ModAlt,
    /// Automation: Read/Off.
    AutoRead,
    /// Automation: Write.
    AutoWrite,
    /// Automation: Trim.
    AutoTrim,
    /// Automation: Touch. Not the fader touch sensor — that is
    /// [`ControlEvent::Touch`](crate::ControlEvent::Touch).
    AutoTouch,
    /// Automation: Latch.
    AutoLatch,
    /// Automation: Group.
    AutoGroup,
    /// Utility: Save.
    Save,
    /// Utility: Undo.
    Undo,
    /// Utility: Cancel.
    Cancel,
    /// Utility: Enter.
    Enter,
    /// Upper transport row: Markers.
    Markers,
    /// Upper transport row: Nudge.
    Nudge,
    /// Upper transport row: Cycle.
    Cycle,
    /// Upper transport row: Drop.
    Drop,
    /// Upper transport row: Replace.
    Replace,
    /// Upper transport row: Click.
    Click,
    /// Upper transport row: Solo. The global one that clears solos, not a
    /// strip's — see [`StripButton::Solo`].
    SoloClear,
    /// Transport: Rewind.
    Rewind,
    /// Transport: Fast forward.
    FastForward,
    /// Transport: Stop.
    Stop,
    /// Transport: Play.
    Play,
    /// Transport: Record.
    Record,
    /// Cursor: up.
    CursorUp,
    /// Cursor: down.
    CursorDown,
    /// Cursor: left.
    CursorLeft,
    /// Cursor: right.
    CursorRight,
    /// Cursor cluster: Zoom.
    Zoom,
    /// Cursor cluster: Scrub.
    Scrub,
    /// Foot switch 1.
    FootSwitch1,
    /// Foot switch 2.
    FootSwitch2,
}

impl GlobalButton {
    /// Every global button.
    ///
    /// Kept beside [`X_TOUCH`]'s note table rather than derived from it, so
    /// that the two have to agree — and a test asserts they do, in both
    /// directions. A button added here without a note, or a note added there
    /// without a button, turns that test red.
    pub const ALL: [Self; 64] = [
        Self::AssignTrack,
        Self::AssignSend,
        Self::AssignPan,
        Self::AssignPlugin,
        Self::AssignEq,
        Self::AssignInstrument,
        Self::BankLeft,
        Self::BankRight,
        Self::ChannelLeft,
        Self::ChannelRight,
        Self::Flip,
        Self::GlobalView,
        Self::NameValue,
        Self::SmpteBeats,
        Self::F1,
        Self::F2,
        Self::F3,
        Self::F4,
        Self::F5,
        Self::F6,
        Self::F7,
        Self::F8,
        Self::ViewMidiTracks,
        Self::ViewInputs,
        Self::ViewAudioTracks,
        Self::ViewAudioInstruments,
        Self::ViewAux,
        Self::ViewBusses,
        Self::ViewOutputs,
        Self::ViewUser,
        Self::ModShift,
        Self::ModOption,
        Self::ModControl,
        Self::ModAlt,
        Self::AutoRead,
        Self::AutoWrite,
        Self::AutoTrim,
        Self::AutoTouch,
        Self::AutoLatch,
        Self::AutoGroup,
        Self::Save,
        Self::Undo,
        Self::Cancel,
        Self::Enter,
        Self::Markers,
        Self::Nudge,
        Self::Cycle,
        Self::Drop,
        Self::Replace,
        Self::Click,
        Self::SoloClear,
        Self::Rewind,
        Self::FastForward,
        Self::Stop,
        Self::Play,
        Self::Record,
        Self::CursorUp,
        Self::CursorDown,
        Self::CursorLeft,
        Self::CursorRight,
        Self::Zoom,
        Self::Scrub,
        Self::FootSwitch1,
        Self::FootSwitch2,
    ];
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
/// what S20 does. A test asserts the ordering rather than trusting it.
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
    /// Whether the numbers above have been read off a real device.
    ///
    /// **`false`**, and it is a field rather than a comment so a log line, a
    /// status panel or a test can ask. `docs/MCU_MAPPING.md` §7 is the list of
    /// claims S20 falsifies at the desk; when it is done, this becomes `true`
    /// in the same edit that corrects whatever was wrong.
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

    /// The X-Touch as `docs/MCU_MAPPING.md` §2 describes it, **unverified**.
    pub const X_TOUCH: Self = Self {
        name: "Behringer X-Touch (MC mode)",
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
        verified: false,
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

    #[test]
    fn the_profile_says_it_has_not_been_seen_on_a_device() {
        // docs/MCU_MAPPING.md carries an UNVERIFIED banner and this is that
        // banner as a value. S20 flips it in the same edit that corrects
        // whatever the MIDI monitor disagrees with; until then a status panel
        // or a log line can say so, which a comment could not.
        //
        // A `const` block rather than a plain assertion, and not only because
        // clippy asks: it means the day somebody sets the flag without doing
        // S20's work, the *build* stops rather than one test.
        const { assert!(!X_TOUCH.verified) }
        assert_eq!(X_TOUCH.name, "Behringer X-Touch (MC mode)");
    }

    #[test]
    fn every_global_button_has_exactly_one_note_and_every_note_one_button() {
        // Two lists that have to agree: GlobalButton::ALL and the note table.
        // A button added to one and forgotten in the other is the mistake this
        // catches, and it is the mistake S20 will be making all evening.
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
