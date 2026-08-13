//! Outbound: what the desk tells the surface to show.
//!
//! `docs/MCU_MAPPING.md` §2.2 and §2.3, as values. Nothing here decides *when*
//! to send anything — the shadow model, the diffing, the 30 Hz coalescing and
//! the touch suppression are layer 2 and belong to S21. This module turns a
//! [`Feedback`] into bytes and back.
//!
//! # Why the outbound direction can be read as well as written
//!
//! A codec that could only write would be tested against itself. [`Feedback`]
//! decodes too, which buys three things: the round-trip criterion becomes an
//! assertion about **bytes** rather than about a function composed with its own
//! inverse; a test can act as the surface and check what the desk sent it; and
//! S20's MIDI monitor session has something to compare a capture against
//! without anybody transcribing hex by hand.
//!
//! # The one message this device is bought for
//!
//! [`Feedback::DisplayColors`] is the X-Touch's own extension — eight scribble
//! strips, one additive RGB byte each, inside Mackie's manufacturer ID and
//! documented by Behringer nowhere. `docs/MCU_MAPPING.md` §2.3 has the
//! provenance. Two things it says that matter here: **black is off, not dark**
//! (a strip set to 0 has its backlight off and is unreadable, so "unassigned"
//! wants white), and **all eight colours are always sent** — there is no
//! per-strip form, which is why [`Feedback::DisplayColors`] carries an array
//! and not a strip index.
//!
//! Mapping an arbitrary colour onto the eight corners of the RGB cube is a
//! quantisation, and it is deliberately **not** here: it is a decision about
//! what looks right on a lighting desk, which makes it layer 2's.

use crate::control::ButtonId;
use crate::midi::{EncodeError, MidiMessage, data_byte};
use crate::profile::{
    Fader, MACKIE_MANUFACTURER_ID, McuProfile, SYSEX_DEVICE_QUERY, SYSEX_LCD_COLOR, SYSEX_LCD_TEXT,
    SYSEX_METER_MODE,
};

/// How many scribble strip colours one colour message carries.
///
/// A property of the message rather than of the surface: `docs/MCU_MAPPING.md`
/// §2.3 records that both shipping implementations always send exactly eight
/// and that nobody knows what a shorter message does. §7 asks the device.
pub const SCRIBBLE_STRIP_COLORS: usize = 8;

/// Velocity that turns a button LED off.
pub const LED_OFF: u8 = 0x00;

/// Velocity that makes a button LED flash. The surface flashes it by itself
/// once told; the host does not blink it.
pub const LED_FLASHING: u8 = 0x01;

/// Velocity that turns a button LED on.
pub const LED_ON: u8 = 0x7F;

/// What a button's LED is doing.
///
/// The surface never lights its own LEDs, which is why a control the host does
/// not drive stays dark — a fact worth knowing before somebody reports the
/// Flip button as broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedState {
    /// Dark.
    Off,
    /// Flashing, at the surface's own rate.
    Flashing,
    /// Lit.
    On,
}

impl LedState {
    /// The velocity that commands this state.
    #[must_use]
    pub const fn velocity(self) -> u8 {
        match self {
            Self::Off => LED_OFF,
            Self::Flashing => LED_FLASHING,
            Self::On => LED_ON,
        }
    }

    /// The state a velocity commands, or `None` for one the protocol does not
    /// define.
    #[must_use]
    pub const fn from_velocity(velocity: u8) -> Option<Self> {
        match velocity {
            LED_OFF => Some(Self::Off),
            LED_FLASHING => Some(Self::Flashing),
            LED_ON => Some(Self::On),
            _ => None,
        }
    }
}

/// How a V-Pot's ring of LEDs displays its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingMode {
    /// A single lit LED.
    Dot,
    /// Filled from the centre outwards, in whichever direction — a boost/cut
    /// control.
    BoostCut,
    /// Filled from the left.
    Wrap,
    /// Symmetrical about the centre, a width rather than a position. Uses only
    /// positions 0…6.
    Spread,
}

impl RingMode {
    /// Every mode, so a test walks the whole set.
    pub const ALL: [Self; 4] = [Self::Dot, Self::BoostCut, Self::Wrap, Self::Spread];

    /// The two-bit code this mode occupies in the CC value.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::Dot => 0b00,
            Self::BoostCut => 0b01,
            Self::Wrap => 0b10,
            Self::Spread => 0b11,
        }
    }

    /// The mode a two-bit code names.
    ///
    /// Total rather than fallible: all four codes are defined, so there is no
    /// `None` — and an `Option` here would have been a branch no input could
    /// take, which is a branch no test could cover either.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0b00 => Self::Dot,
            0b01 => Self::BoostCut,
            0b10 => Self::Wrap,
            _ => Self::Spread,
        }
    }

    /// The largest position this mode accepts. Spread is symmetrical about the
    /// centre, so it runs out of ring at half the count.
    #[must_use]
    pub const fn max_position(self) -> u8 {
        match self {
            Self::Spread => 6,
            _ => 11,
        }
    }
}

/// A V-Pot's ring of LEDs: a mode, a position, and the small LED beneath the
/// encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VPotRing {
    /// How the position is drawn.
    pub mode: RingMode,
    /// 1…11, or 0 for all off. [`RingMode::Spread`] uses 0…6.
    pub position: u8,
    /// The single LED under the encoder, which is separate from the ring.
    pub led: bool,
}

impl VPotRing {
    /// The CC value that commands this ring: `0b0LMMVVVV`.
    ///
    /// # Errors
    ///
    /// [`EncodeError::OutOfRange`] for a position the mode cannot draw.
    pub const fn value(&self) -> Result<u8, EncodeError> {
        if self.position > self.mode.max_position() {
            return Err(EncodeError::OutOfRange {
                field: "ring position",
                value: self.position as u32,
            });
        }
        let led = if self.led { 0x40 } else { 0x00 };
        Ok(led | (self.mode.bits() << 4) | self.position)
    }

    /// The ring a CC value commands, or `None` if the position is not one the
    /// mode can draw.
    #[must_use]
    pub const fn from_value(value: u8) -> Option<Self> {
        let position = value & 0x0F;
        let mode = RingMode::from_bits(value >> 4);
        if position > mode.max_position() {
            return None;
        }
        Some(Self {
            mode,
            position,
            led: value & 0x40 != 0,
        })
    }
}

/// The highest ordinary level a meter shows: 0 dB.
pub const METER_LEVEL_0DB: u8 = 0x0C;

/// The level that means *above 0 dB*.
pub const METER_LEVEL_OVER: u8 = 0x0D;

/// The code that sets a strip's overload marker.
pub const METER_SET_OVERLOAD: u8 = 0x0E;

/// The code that clears it.
pub const METER_CLEAR_OVERLOAD: u8 = 0x0F;

/// What one meter message says about one strip.
///
/// Meters decay in the surface at roughly 300 ms per division, so a host that
/// stops sending does not leave a meter lit — which is why
/// `docs/MCU_MAPPING.md` §5.2 can drop meters first when bandwidth is short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterSignal {
    /// A level: 0 is silence, [`METER_LEVEL_0DB`] is 0 dB and
    /// [`METER_LEVEL_OVER`] is above it.
    Level(u8),
    /// The overload marker, set or cleared. Only visible in horizontal LCD
    /// meter mode.
    Overload(bool),
}

impl MeterSignal {
    /// The low nibble this signal occupies.
    ///
    /// # Errors
    ///
    /// [`EncodeError::OutOfRange`] for a level above [`METER_LEVEL_OVER`],
    /// which would land on one of the two overload codes and set a marker
    /// nobody asked for.
    pub const fn nibble(&self) -> Result<u8, EncodeError> {
        match *self {
            Self::Level(level) => {
                if level > METER_LEVEL_OVER {
                    return Err(EncodeError::OutOfRange {
                        field: "meter level",
                        value: level as u32,
                    });
                }
                Ok(level)
            }
            Self::Overload(true) => Ok(METER_SET_OVERLOAD),
            Self::Overload(false) => Ok(METER_CLEAR_OVERLOAD),
        }
    }

    /// The signal a low nibble carries.
    #[must_use]
    pub const fn from_nibble(nibble: u8) -> Self {
        match nibble & 0x0F {
            METER_SET_OVERLOAD => Self::Overload(true),
            METER_CLEAR_OVERLOAD => Self::Overload(false),
            level => Self::Level(level),
        }
    }
}

/// One character of the 7-segment display.
///
/// The character set is ASCII with bit 6 stripped, which makes it a bijection
/// onto the printable range `' '`…`'_'`: `'A'` (0x41) becomes 1, `'0'` (0x30)
/// stays 48. The sources also say a value of 0 blanks the digit, and 0 is what
/// `'@'` maps to under the stripping rule — the two claims cannot both be the
/// whole truth, and `docs/MCU_MAPPING.md` §7 asks the device which it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentChar {
    /// The six-bit character code.
    pub code: u8,
    /// The dot to the right of the digit.
    pub dot: bool,
}

impl SegmentChar {
    /// The character an ASCII byte displays as, or `None` for one outside the
    /// range the display can show.
    ///
    /// Lower case is folded up: the display has no lower case, and a name that
    /// silently came out blank would be worse than one in capitals.
    #[must_use]
    pub const fn from_ascii(byte: u8) -> Option<Self> {
        let byte = byte.to_ascii_uppercase();
        if byte < 0x20 || byte > 0x5F {
            return None;
        }
        Some(Self {
            code: byte & 0x3F,
            dot: false,
        })
    }

    /// The ASCII byte this character came from.
    #[must_use]
    pub const fn to_ascii(self) -> u8 {
        if self.code < 0x20 {
            self.code | 0x40
        } else {
            self.code
        }
    }

    /// The same character with its dot lit.
    #[must_use]
    pub const fn with_dot(self) -> Self {
        Self { dot: true, ..self }
    }

    /// The CC value that draws it.
    ///
    /// # Errors
    ///
    /// [`EncodeError::OutOfRange`] for a code wider than six bits.
    pub const fn value(self) -> Result<u8, EncodeError> {
        if self.code > 0x3F {
            return Err(EncodeError::OutOfRange {
                field: "segment code",
                value: self.code as u32,
            });
        }
        let dot = if self.dot { 0x40 } else { 0x00 };
        Ok(dot | self.code)
    }

    /// The character a CC value draws.
    #[must_use]
    pub const fn from_value(value: u8) -> Self {
        Self {
            code: value & 0x3F,
            dot: value & 0x40 != 0,
        }
    }
}

/// A scribble strip's backlight colour.
///
/// The byte is an additive RGB triple, not a palette index: bit 0 is red, bit 1
/// green, bit 2 blue. The eight values are therefore exactly the eight corners
/// of the RGB cube.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StripColor {
    /// Backlight off. **Not "dark"** — the text on a black strip cannot be
    /// read, so this is the deliberate marker for *nothing here* rather than a
    /// sensible default. [`White`](Self::White) is the default for an
    /// unassigned strip.
    #[default]
    Off,
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Blue.
    Blue,
    /// Magenta.
    Magenta,
    /// Cyan.
    Cyan,
    /// White.
    White,
}

impl StripColor {
    /// Every colour, in the order their bytes run.
    pub const ALL: [Self; 8] = [
        Self::Off,
        Self::Red,
        Self::Green,
        Self::Yellow,
        Self::Blue,
        Self::Magenta,
        Self::Cyan,
        Self::White,
    ];

    /// The byte that commands this colour.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Red => 1,
            Self::Green => 2,
            Self::Yellow => 3,
            Self::Blue => 4,
            Self::Magenta => 5,
            Self::Cyan => 6,
            Self::White => 7,
        }
    }

    /// The colour a byte commands, or `None` for a byte outside the eight.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::Off),
            1 => Some(Self::Red),
            2 => Some(Self::Green),
            3 => Some(Self::Yellow),
            4 => Some(Self::Blue),
            5 => Some(Self::Magenta),
            6 => Some(Self::Cyan),
            7 => Some(Self::White),
            _ => None,
        }
    }

    /// Whether the red, green and blue lamps are on.
    #[must_use]
    pub const fn rgb(self) -> (bool, bool, bool) {
        let bits = self.bits();
        (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0)
    }
}

/// How the LCD draws the level meters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdMeterMode {
    /// Horizontal. The only mode in which the overload marker appears.
    Horizontal,
    /// Vertical. **Untested on the X-Touch** — `docs/MCU_MAPPING.md` §2.2.
    Vertical,
}

impl LcdMeterMode {
    /// The byte that selects this mode.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::Horizontal => 0x00,
            Self::Vertical => 0x01,
        }
    }

    /// The mode a byte selects.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0x00 => Some(Self::Horizontal),
            0x01 => Some(Self::Vertical),
            _ => None,
        }
    }
}

/// Something the desk shows on the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feedback<'a> {
    /// A button's LED.
    Led {
        /// Which button.
        button: ButtonId,
        /// What its LED should do.
        state: LedState,
    },
    /// A motor fader's position, 14-bit.
    ///
    /// **Must not be sent while that fader reports touch** — the motor would
    /// fight the operator's hand (`docs/MCU_MAPPING.md` §5.1). Enforcing that
    /// is layer 2's job; this type is only the message.
    Move {
        /// Which fader.
        fader: Fader,
        /// Position, 0…16383.
        position: u16,
    },
    /// A V-Pot's ring of LEDs.
    Ring {
        /// Strip index, 0 at the left.
        strip: u8,
        /// What to draw.
        ring: VPotRing,
    },
    /// A level meter.
    Meter {
        /// Strip index, 0 at the left.
        strip: u8,
        /// The level or the overload marker.
        signal: MeterSignal,
    },
    /// One digit of the 7-segment display. Digit 0 is the **rightmost**.
    Segment {
        /// Digit index, counted from the right.
        digit: u8,
        /// The character to draw.
        character: SegmentChar,
    },
    /// Characters written into the scribble strips' shared buffer.
    ///
    /// One buffer for the whole row, not one message per strip: offset 0 starts
    /// the upper line, [`McuProfile::lcd_line_offset`] the lower, and strip *n*
    /// owns [`McuProfile::lcd_chars_per_strip`] characters at each. Writing a
    /// single strip is therefore a matter of the offset.
    DisplayText {
        /// Where in the buffer the characters land.
        offset: u8,
        /// ASCII, seven bits each.
        text: &'a [u8],
    },
    /// All eight scribble strip colours. The X-Touch's own extension.
    DisplayColors([StripColor; SCRIBBLE_STRIP_COLORS]),
    /// How the LCD draws meters.
    MeterMode(LcdMeterMode),
    /// The optional handshake: *are you there*.
    ///
    /// A surface answers with a Device Ready whose exact shape no source
    /// settles, so this crate writes the question and does not pretend to know
    /// the answer — `docs/MCU_MAPPING.md` §2.3 and the decision log.
    DeviceQuery,
}

/// Writes bytes into a caller's buffer, one piece at a time.
struct Writer<'b> {
    buf: &'b mut [u8],
    at: usize,
}

impl Writer<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        let end = self.at + bytes.len();
        let Some(target) = self.buf.get_mut(self.at..end) else {
            return Err(EncodeError::BufferTooSmall {
                needed: end,
                offered: self.buf.len(),
            });
        };
        target.copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }
}

impl Feedback<'_> {
    /// Writes this message into a caller-provided buffer and returns how many
    /// bytes it used.
    ///
    /// Nothing is allocated: the SysEx forms are assembled straight into the
    /// caller's buffer, and the one intermediate — the eight colour bytes —
    /// lives on the stack.
    ///
    /// # Errors
    ///
    /// [`EncodeError::NoSuchControl`] for a control this surface does not have,
    /// [`EncodeError::OutOfRange`] for a field the wire cannot carry, and
    /// [`EncodeError::BufferTooSmall`] if the buffer is short.
    pub fn encode_into(&self, profile: &McuProfile, buf: &mut [u8]) -> Result<usize, EncodeError> {
        match *self {
            Self::Led { button, state } => {
                let note = match button {
                    ButtonId::Strip { strip, button } => profile.strip_note(strip, button),
                    ButtonId::Global(button) => profile.note_of(button),
                }
                .ok_or(EncodeError::NoSuchControl)?;
                MidiMessage::NoteOn {
                    channel: profile.channel,
                    note,
                    velocity: state.velocity(),
                }
                .encode_into(buf)
            }
            Self::Move { fader, position } => {
                let channel = profile
                    .fader_channel(fader)
                    .ok_or(EncodeError::NoSuchControl)?;
                MidiMessage::PitchBend {
                    channel,
                    value: position,
                }
                .encode_into(buf)
            }
            Self::Ring { strip, ring } => {
                let controller = profile
                    .ring_controller(strip)
                    .ok_or(EncodeError::NoSuchControl)?;
                MidiMessage::ControlChange {
                    channel: profile.channel,
                    controller,
                    value: ring.value()?,
                }
                .encode_into(buf)
            }
            Self::Meter { strip, signal } => {
                if strip >= profile.strips {
                    return Err(EncodeError::NoSuchControl);
                }
                MidiMessage::ChannelPressure {
                    channel: profile.channel,
                    pressure: (strip << 4) | signal.nibble()?,
                }
                .encode_into(buf)
            }
            Self::Segment { digit, character } => {
                let controller = profile
                    .segment_controller(digit)
                    .ok_or(EncodeError::NoSuchControl)?;
                MidiMessage::ControlChange {
                    channel: profile.channel,
                    controller,
                    value: character.value()?,
                }
                .encode_into(buf)
            }
            Self::DisplayText { offset, text } => {
                let end = usize::from(offset) + text.len();
                if end > usize::from(profile.lcd_buffer_len) {
                    return Err(EncodeError::OutOfRange {
                        field: "display offset",
                        value: end as u32,
                    });
                }
                for byte in text {
                    data_byte("display text", *byte)?;
                }
                write_sysex(profile, SYSEX_LCD_TEXT, &[offset], text, buf)
            }
            Self::DisplayColors(colors) => {
                let mut bytes = [0u8; SCRIBBLE_STRIP_COLORS];
                for (slot, color) in bytes.iter_mut().zip(colors) {
                    *slot = color.bits();
                }
                write_sysex(profile, SYSEX_LCD_COLOR, &bytes, &[], buf)
            }
            Self::MeterMode(mode) => {
                write_sysex(profile, SYSEX_METER_MODE, &[mode.bits()], &[], buf)
            }
            Self::DeviceQuery => write_sysex(profile, SYSEX_DEVICE_QUERY, &[], &[], buf),
        }
    }

    /// Reads a feedback message out of a MIDI message, or `None` if it is not
    /// one this surface understands.
    #[must_use]
    pub fn from_midi<'a>(profile: &McuProfile, message: &MidiMessage<'a>) -> Option<Feedback<'a>> {
        match *message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } if channel == profile.channel => {
                let button = if let Some((strip, button)) = profile.strip_button_at(note) {
                    ButtonId::Strip { strip, button }
                } else {
                    ButtonId::Global(profile.button_at(note)?)
                };
                Some(Feedback::Led {
                    button,
                    state: LedState::from_velocity(velocity)?,
                })
            }
            MidiMessage::PitchBend { channel, value } => Some(Feedback::Move {
                fader: profile.fader_on_channel(channel)?,
                position: value,
            }),
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => {
                if channel == profile.channel
                    && let Some(strip) = profile.ring_strip_at(controller)
                {
                    return Some(Feedback::Ring {
                        strip,
                        ring: VPotRing::from_value(value)?,
                    });
                }
                if profile.accepts_segment_channel(channel)
                    && let Some(digit) = profile.segment_at(controller)
                {
                    return Some(Feedback::Segment {
                        digit,
                        character: SegmentChar::from_value(value),
                    });
                }
                None
            }
            MidiMessage::ChannelPressure { channel, pressure } if channel == profile.channel => {
                let strip = pressure >> 4;
                (strip < profile.strips).then(|| Feedback::Meter {
                    strip,
                    signal: MeterSignal::from_nibble(pressure),
                })
            }
            MidiMessage::SysEx(payload) => sysex_feedback(profile, payload),
            _ => None,
        }
    }
}

/// Assembles `F0 00 00 66 <device> <command> <head> <tail> F7` into `buf`.
fn write_sysex(
    profile: &McuProfile,
    command: u8,
    head: &[u8],
    tail: &[u8],
    buf: &mut [u8],
) -> Result<usize, EncodeError> {
    let mut writer = Writer { buf, at: 0 };
    writer.put(&[0xF0])?;
    writer.put(&MACKIE_MANUFACTURER_ID)?;
    writer.put(&[data_byte("device id", profile.device_id)?, command])?;
    writer.put(head)?;
    writer.put(tail)?;
    writer.put(&[0xF7])?;
    Ok(writer.at)
}

/// Reads a Mackie SysEx payload — the bytes between `F0` and `F7`.
fn sysex_feedback<'a>(profile: &McuProfile, payload: &'a [u8]) -> Option<Feedback<'a>> {
    let (manufacturer, rest) = payload.split_at_checked(MACKIE_MANUFACTURER_ID.len())?;
    if manufacturer != MACKIE_MANUFACTURER_ID {
        return None;
    }
    let device_id = *rest.first()?;
    let command = *rest.get(1)?;
    let body = rest.get(2..)?;
    if device_id != profile.device_id {
        return None;
    }
    match command {
        SYSEX_DEVICE_QUERY if body.is_empty() => Some(Feedback::DeviceQuery),
        SYSEX_METER_MODE => {
            let [mode] = body else { return None };
            LcdMeterMode::from_bits(*mode).map(Feedback::MeterMode)
        }
        SYSEX_LCD_TEXT => {
            let (offset, text) = body.split_first()?;
            (usize::from(*offset) + text.len() <= usize::from(profile.lcd_buffer_len)).then_some(
                Feedback::DisplayText {
                    offset: *offset,
                    text,
                },
            )
        }
        SYSEX_LCD_COLOR => {
            let mut colors = [StripColor::Off; SCRIBBLE_STRIP_COLORS];
            if body.len() != SCRIBBLE_STRIP_COLORS {
                return None;
            }
            for (slot, byte) in colors.iter_mut().zip(body) {
                *slot = StripColor::from_bits(*byte)?;
            }
            Some(Feedback::DisplayColors(colors))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Feedback, LcdMeterMode, LedState, MeterSignal, RingMode, SCRIBBLE_STRIP_COLORS,
        SegmentChar, StripColor, VPotRing,
    };
    use crate::control::ButtonId;
    use crate::midi::{EncodeError, MAX_MESSAGE_BYTES, MidiMessage};
    use crate::profile::{Fader, GlobalButton, StripButton, X_TOUCH};

    fn encoded(feedback: &Feedback<'_>) -> Vec<u8> {
        let mut buf = [0u8; MAX_MESSAGE_BYTES];
        let written = feedback
            .encode_into(&X_TOUCH, &mut buf)
            .expect("a message the surface has");
        buf.get(..written).expect("written fits").to_vec()
    }

    #[test]
    fn an_led_is_a_note_on_with_three_defined_velocities() {
        // 0, 1 and 127, and the middle one is the surprise: the surface
        // flashes the LED itself, so a host that blinked it by hand would be
        // fighting the firmware and using the bandwidth §5.2 rations.
        let button = ButtonId::Global(GlobalButton::Save);
        assert_eq!(
            encoded(&Feedback::Led {
                button,
                state: LedState::On
            }),
            vec![0x90, 80, 0x7F]
        );
        assert_eq!(
            encoded(&Feedback::Led {
                button,
                state: LedState::Flashing
            }),
            vec![0x90, 80, 0x01]
        );
        assert_eq!(
            encoded(&Feedback::Led {
                button,
                state: LedState::Off
            }),
            vec![0x90, 80, 0x00]
        );
    }

    #[test]
    fn a_velocity_the_protocol_does_not_define_is_not_an_led_state() {
        assert_eq!(LedState::from_velocity(0x40), None);
        let message = MidiMessage::NoteOn {
            channel: 0,
            note: 80,
            velocity: 0x40,
        };
        assert_eq!(Feedback::from_midi(&X_TOUCH, &message), None);
    }

    #[test]
    fn nothing_addressed_to_another_surface_is_read_as_feedback() {
        // The outbound direction has to be as careful about the channel as the
        // inbound one: an extender's LED command must not be mistaken for this
        // desk's, and the three message kinds the MCU does not use are not
        // feedback at all.
        for message in [
            MidiMessage::NoteOn {
                channel: 2,
                note: 80,
                velocity: 0x7F,
            },
            MidiMessage::ChannelPressure {
                channel: 3,
                pressure: 0x2C,
            },
            MidiMessage::NoteOff {
                channel: 0,
                note: 80,
                velocity: 0,
            },
            MidiMessage::PolyPressure {
                channel: 0,
                note: 80,
                pressure: 1,
            },
            MidiMessage::ProgramChange {
                channel: 0,
                program: 1,
            },
        ] {
            assert_eq!(Feedback::from_midi(&X_TOUCH, &message), None, "{message:?}");
        }
        // ...and a meter for a strip beyond the eight.
        assert_eq!(
            Feedback::from_midi(
                &X_TOUCH,
                &MidiMessage::ChannelPressure {
                    channel: 0,
                    pressure: 0x9C
                }
            ),
            None
        );
    }

    #[test]
    fn a_fader_touch_note_is_not_a_lamp() {
        // Notes 104-112 report a hand, they do not light anything. Decoding one
        // as an LED would make the shadow model believe in a control that does
        // not exist.
        let message = MidiMessage::NoteOn {
            channel: 0,
            note: 104,
            velocity: 0x7F,
        };
        assert_eq!(Feedback::from_midi(&X_TOUCH, &message), None);
    }

    #[test]
    fn the_ring_packs_a_lamp_a_mode_and_a_position_into_one_byte() {
        let ring = VPotRing {
            mode: RingMode::Wrap,
            position: 9,
            led: true,
        };
        assert_eq!(ring.value(), Ok(0b0110_1001));
        assert_eq!(VPotRing::from_value(0b0110_1001), Some(ring));
        assert_eq!(
            encoded(&Feedback::Ring { strip: 5, ring }),
            vec![0xB0, 53, 0b0110_1001]
        );
    }

    #[test]
    fn every_ring_mode_round_trips_through_its_whole_position_range() {
        for mode in RingMode::ALL {
            for position in 0..=mode.max_position() {
                for led in [false, true] {
                    let ring = VPotRing {
                        mode,
                        position,
                        led,
                    };
                    let value = ring.value().expect("a position the mode can draw");
                    assert_eq!(VPotRing::from_value(value), Some(ring));
                }
            }
            // One past the end is refused rather than wrapped onto a position
            // that means something else.
            let too_far = VPotRing {
                mode,
                position: mode.max_position() + 1,
                led: false,
            };
            assert_eq!(
                too_far.value(),
                Err(EncodeError::OutOfRange {
                    field: "ring position",
                    value: u32::from(mode.max_position()) + 1
                })
            );
        }
    }

    #[test]
    fn spread_mode_uses_half_the_ring_and_says_so_in_both_directions() {
        // The one mode with a different range, and a decoder that ignored it
        // would accept a byte the surface draws as something else.
        assert_eq!(RingMode::Spread.max_position(), 6);
        assert_eq!(RingMode::Dot.max_position(), 11);
        assert_eq!(VPotRing::from_value(0b0011_0111), None);
        assert!(VPotRing::from_value(0b0010_0111).is_some());
    }

    #[test]
    fn a_meter_carries_the_strip_in_the_high_nibble() {
        assert_eq!(
            encoded(&Feedback::Meter {
                strip: 6,
                signal: MeterSignal::Level(9)
            }),
            vec![0xD0, 0x69]
        );
        assert_eq!(
            encoded(&Feedback::Meter {
                strip: 3,
                signal: MeterSignal::Overload(true)
            }),
            vec![0xD0, 0x3E]
        );
        assert_eq!(
            encoded(&Feedback::Meter {
                strip: 3,
                signal: MeterSignal::Overload(false)
            }),
            vec![0xD0, 0x3F]
        );
        assert_eq!(MeterSignal::from_nibble(0x0E), MeterSignal::Overload(true));
        assert_eq!(MeterSignal::from_nibble(0x0F), MeterSignal::Overload(false));
        assert_eq!(MeterSignal::from_nibble(0x0C), MeterSignal::Level(12));
    }

    #[test]
    fn a_meter_level_that_would_land_on_an_overload_code_is_refused() {
        // 0x0E and 0x0F are not levels. A level of 14 written as a level would
        // light an overload marker on a strip that is merely loud.
        assert_eq!(
            MeterSignal::Level(0x0E).nibble(),
            Err(EncodeError::OutOfRange {
                field: "meter level",
                value: 14
            })
        );
        assert_eq!(MeterSignal::Level(0x0D).nibble(), Ok(0x0D));
    }

    #[test]
    fn the_seven_segment_character_set_is_ascii_with_bit_six_stripped() {
        assert_eq!(SegmentChar::from_ascii(b'A').map(|c| c.code), Some(1));
        assert_eq!(SegmentChar::from_ascii(b'Z').map(|c| c.code), Some(26));
        assert_eq!(SegmentChar::from_ascii(b'0').map(|c| c.code), Some(48));
        assert_eq!(SegmentChar::from_ascii(b'9').map(|c| c.code), Some(57));
        assert_eq!(SegmentChar::from_ascii(b'#').map(|c| c.code), Some(35));
        assert_eq!(SegmentChar::from_ascii(b';').map(|c| c.code), Some(59));
        // Lower case is folded up rather than refused: the display has no lower
        // case, and a name that came out blank would be worse than capitals.
        assert_eq!(SegmentChar::from_ascii(b'a'), SegmentChar::from_ascii(b'A'));
        assert_eq!(SegmentChar::from_ascii(0x1F), None);
        assert_eq!(SegmentChar::from_ascii(b'{'), None);
    }

    #[test]
    fn every_character_the_display_can_show_survives_the_trip_to_ascii_and_back() {
        for byte in 0x20..=0x5Fu8 {
            let character = SegmentChar::from_ascii(byte).expect("inside the range");
            assert_eq!(character.to_ascii(), byte);
            let value = character.value().expect("six bits");
            assert_eq!(SegmentChar::from_value(value), character);
            let dotted = character.with_dot();
            let value = dotted.value().expect("six bits");
            assert_eq!(SegmentChar::from_value(value), dotted);
            assert_eq!(value & 0x40, 0x40);
        }
        assert_eq!(
            SegmentChar {
                code: 0x40,
                dot: false
            }
            .value(),
            Err(EncodeError::OutOfRange {
                field: "segment code",
                value: 0x40
            })
        );
    }

    #[test]
    fn a_seven_segment_digit_is_addressed_from_the_right() {
        assert_eq!(
            encoded(&Feedback::Segment {
                digit: 0,
                character: SegmentChar::from_ascii(b'7').expect("a digit")
            }),
            vec![0xB0, 64, 55]
        );
        assert_eq!(
            encoded(&Feedback::Segment {
                digit: 11,
                character: SegmentChar::from_ascii(b'C').expect("a letter")
            }),
            vec![0xB0, 75, 3]
        );
    }

    #[test]
    fn the_display_is_read_on_either_channel_and_written_on_one() {
        // §2.2: some hosts send these on channel 16. Accepting both and
        // emitting one keeps our own byte stream canonical while still working
        // with a surface that speaks the other dialect.
        let canonical = MidiMessage::ControlChange {
            channel: 0,
            controller: 70,
            value: 5,
        };
        let alternative = MidiMessage::ControlChange {
            channel: 15,
            controller: 70,
            value: 5,
        };
        assert_eq!(
            Feedback::from_midi(&X_TOUCH, &alternative),
            Feedback::from_midi(&X_TOUCH, &canonical)
        );
        let decoded = Feedback::from_midi(&X_TOUCH, &alternative).expect("a display digit");
        assert_eq!(encoded(&decoded), vec![0xB0, 70, 5]);
    }

    #[test]
    fn the_ring_and_the_display_do_not_read_each_others_messages() {
        // Both are CC on the same channel; only the number tells them apart.
        let ring = MidiMessage::ControlChange {
            channel: 0,
            controller: 50,
            value: 0x03,
        };
        assert!(matches!(
            Feedback::from_midi(&X_TOUCH, &ring),
            Some(Feedback::Ring { strip: 2, .. })
        ));
        let segment = MidiMessage::ControlChange {
            channel: 0,
            controller: 66,
            value: 0x03,
        };
        assert!(matches!(
            Feedback::from_midi(&X_TOUCH, &segment),
            Some(Feedback::Segment { digit: 2, .. })
        ));
        // A ring message on the alternative channel is nothing: only the
        // 7-segment display is reported to accept it.
        let ring_elsewhere = MidiMessage::ControlChange {
            channel: 15,
            controller: 50,
            value: 0x03,
        };
        assert_eq!(Feedback::from_midi(&X_TOUCH, &ring_elsewhere), None);
    }

    #[test]
    fn the_scribble_strip_colours_are_the_message_this_device_is_bought_for() {
        // docs/MCU_MAPPING.md §2.3, byte for byte: Mackie's manufacturer ID,
        // the device ID, command 0x72, and one additive RGB byte per strip.
        let mut colors = [StripColor::Off; SCRIBBLE_STRIP_COLORS];
        for (slot, color) in colors.iter_mut().zip(StripColor::ALL) {
            *slot = color;
        }
        assert_eq!(
            encoded(&Feedback::DisplayColors(colors)),
            vec![
                0xF0, 0x00, 0x00, 0x66, 0x14, 0x72, 0, 1, 2, 3, 4, 5, 6, 7, 0xF7
            ]
        );
    }

    #[test]
    fn a_colour_byte_is_an_additive_rgb_triple_rather_than_a_palette_index() {
        // Bit 0 red, bit 1 green, bit 2 blue - so the eight values are the
        // eight corners of the cube, which is what makes an arbitrary colour a
        // nearest-corner problem rather than a lookup.
        assert_eq!(StripColor::Red.rgb(), (true, false, false));
        assert_eq!(StripColor::Yellow.rgb(), (true, true, false));
        assert_eq!(StripColor::Cyan.rgb(), (false, true, true));
        assert_eq!(StripColor::White.rgb(), (true, true, true));
        assert_eq!(StripColor::Off.rgb(), (false, false, false));
        for color in StripColor::ALL {
            assert_eq!(StripColor::from_bits(color.bits()), Some(color));
        }
        assert_eq!(StripColor::from_bits(8), None);
        // Black is the deliberate marker for "nothing here", so it is what an
        // uninitialised array holds and White is what an unassigned strip wants.
        assert_eq!(StripColor::default(), StripColor::Off);
    }

    #[test]
    fn a_colour_message_of_the_wrong_length_is_not_understood() {
        // §2.3: both shipping implementations always send exactly eight and
        // nobody knows what a shorter message does. Refusing to guess is the
        // honest reading, and §7 asks the device.
        for length in [0usize, 7, 9] {
            let mut payload = vec![0x00, 0x00, 0x66, 0x14, 0x72];
            payload.extend(std::iter::repeat_n(0x01, length));
            assert_eq!(
                Feedback::from_midi(&X_TOUCH, &MidiMessage::SysEx(&payload)),
                None,
                "{length} colours"
            );
        }
        // And a byte outside the eight is not a colour.
        let payload = [0x00, 0x00, 0x66, 0x14, 0x72, 8, 1, 2, 3, 4, 5, 6, 7];
        assert_eq!(
            Feedback::from_midi(&X_TOUCH, &MidiMessage::SysEx(&payload)),
            None
        );
    }

    #[test]
    fn writing_one_strip_is_an_offset_and_not_a_different_message() {
        // §2.2: one 2x56 buffer for the whole row. Strip 3's lower line starts
        // at 0x38 + 21.
        let text = b"CYC 42";
        let offset = X_TOUCH.lcd_line_offset + 3 * X_TOUCH.lcd_chars_per_strip;
        assert_eq!(offset, 0x38 + 21);
        let mut expected = vec![0xF0, 0x00, 0x00, 0x66, 0x14, 0x12, offset];
        expected.extend_from_slice(text);
        expected.push(0xF7);
        assert_eq!(
            encoded(&Feedback::DisplayText {
                offset,
                text: text.as_slice()
            }),
            expected
        );
    }

    #[test]
    fn text_that_would_run_off_the_end_of_the_buffer_is_refused() {
        let text = [b'X'; 8];
        assert_eq!(
            Feedback::DisplayText {
                offset: X_TOUCH.lcd_buffer_len - 4,
                text: &text,
            }
            .encode_into(&X_TOUCH, &mut [0u8; MAX_MESSAGE_BYTES]),
            Err(EncodeError::OutOfRange {
                field: "display offset",
                value: u32::from(X_TOUCH.lcd_buffer_len) + 4
            })
        );
        // The last character of the buffer is still writable.
        assert!(
            Feedback::DisplayText {
                offset: X_TOUCH.lcd_buffer_len - 1,
                text: b"X",
            }
            .encode_into(&X_TOUCH, &mut [0u8; MAX_MESSAGE_BYTES])
            .is_ok()
        );
    }

    #[test]
    fn text_outside_seven_bit_ascii_is_refused_rather_than_masked() {
        // A masked byte is a character the operator did not ask for on a
        // display they are reading in a hurry.
        assert_eq!(
            Feedback::DisplayText {
                offset: 0,
                text: &[b'A', 0xC4],
            }
            .encode_into(&X_TOUCH, &mut [0u8; MAX_MESSAGE_BYTES]),
            Err(EncodeError::OutOfRange {
                field: "display text",
                value: 0xC4
            })
        );
    }

    #[test]
    fn the_meter_mode_and_the_device_query_are_whole_messages() {
        assert_eq!(
            encoded(&Feedback::MeterMode(LcdMeterMode::Horizontal)),
            vec![0xF0, 0x00, 0x00, 0x66, 0x14, 0x21, 0x00, 0xF7]
        );
        assert_eq!(
            encoded(&Feedback::MeterMode(LcdMeterMode::Vertical)),
            vec![0xF0, 0x00, 0x00, 0x66, 0x14, 0x21, 0x01, 0xF7]
        );
        assert_eq!(
            encoded(&Feedback::DeviceQuery),
            vec![0xF0, 0x00, 0x00, 0x66, 0x14, 0x00, 0xF7]
        );
        assert_eq!(LcdMeterMode::from_bits(0x02), None);
    }

    #[test]
    fn a_sysex_from_another_manufacturer_or_another_device_is_left_alone() {
        // Two surfaces on one port, or a synthesiser sharing it. Answering to
        // somebody else's manufacturer ID is how a codec corrupts a device it
        // was never talking to.
        for payload in [
            &[0x00, 0x00, 0x67, 0x14, 0x72, 1, 1, 1, 1, 1, 1, 1, 1][..],
            &[0x00, 0x00, 0x66, 0x15, 0x72, 1, 1, 1, 1, 1, 1, 1, 1][..],
            &[0x00, 0x00][..],
            &[0x00, 0x00, 0x66][..],
            &[0x00, 0x00, 0x66, 0x14, 0x7F][..],
            &[0x00, 0x00, 0x66, 0x14, 0x21][..],
            &[0x00, 0x00, 0x66, 0x14, 0x21, 0x00, 0x00][..],
            &[0x00, 0x00, 0x66, 0x14, 0x12][..],
            &[0x00, 0x00, 0x66, 0x14, 0x00, 0x01][..],
        ] {
            assert_eq!(
                Feedback::from_midi(&X_TOUCH, &MidiMessage::SysEx(payload)),
                None,
                "{payload:02X?}"
            );
        }
    }

    #[test]
    fn a_display_write_past_the_buffer_is_not_decoded_either() {
        let mut payload = vec![0x00, 0x00, 0x66, 0x14, 0x12, X_TOUCH.lcd_buffer_len - 2];
        payload.extend_from_slice(b"ABCD");
        assert_eq!(
            Feedback::from_midi(&X_TOUCH, &MidiMessage::SysEx(&payload)),
            None
        );
    }

    #[test]
    fn a_control_this_surface_does_not_have_cannot_be_lit_or_moved() {
        let mut buf = [0u8; MAX_MESSAGE_BYTES];
        for feedback in [
            Feedback::Led {
                button: ButtonId::Strip {
                    strip: 8,
                    button: StripButton::Solo,
                },
                state: LedState::On,
            },
            Feedback::Move {
                fader: Fader::Strip(8),
                position: 1,
            },
            Feedback::Ring {
                strip: 9,
                ring: VPotRing {
                    mode: RingMode::Dot,
                    position: 1,
                    led: false,
                },
            },
            Feedback::Meter {
                strip: 8,
                signal: MeterSignal::Level(1),
            },
            Feedback::Segment {
                digit: 12,
                character: SegmentChar::from_ascii(b'A').expect("a letter"),
            },
        ] {
            assert_eq!(
                feedback.encode_into(&X_TOUCH, &mut buf),
                Err(EncodeError::NoSuchControl),
                "{feedback:?}"
            );
        }
    }

    #[test]
    fn a_buffer_too_short_for_a_sysex_says_so_at_every_piece_of_it() {
        // The writer fills in pieces, so a short buffer has to be caught at
        // whichever piece runs off the end rather than at the first.
        for size in 0..15 {
            let mut buf = vec![0u8; size];
            let result = Feedback::DisplayColors([StripColor::White; SCRIBBLE_STRIP_COLORS])
                .encode_into(&X_TOUCH, &mut buf);
            assert!(
                matches!(result, Err(EncodeError::BufferTooSmall { .. })),
                "{size} bytes gave {result:?}"
            );
        }
        assert!(
            Feedback::DisplayColors([StripColor::White; SCRIBBLE_STRIP_COLORS])
                .encode_into(&X_TOUCH, &mut [0u8; 15])
                .is_ok()
        );
    }

    #[test]
    fn the_colour_message_carries_one_byte_per_strip_this_surface_has() {
        // The message's shape and the surface's strip count are two different
        // facts that happen to agree on this device. If a later profile has
        // more strips, this is the test that says the colour message needs
        // thinking about rather than a bigger array.
        assert_eq!(usize::from(X_TOUCH.strips), SCRIBBLE_STRIP_COLORS);
    }
}
